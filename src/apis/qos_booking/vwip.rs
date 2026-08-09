//! QoS Booking **vwip** (CAMARA qos-booking, work-in-progress), mounted at
//! `/qos-booking/vwip`.
//!
//! Implemented so far:
//! - `POST /device-qos-bookings` (operationId `createBooking`, scope
//!   `qos-booking:device-qos-bookings:create`) — books a QoS profile for a device
//!   over a bounded time window in a service area, mints an opaque `bookingId`
//!   ([`super::store`]), remembers the rendered `BookingInfo`, and returns `201`.
//! - `GET /device-qos-bookings/{bookingId}` (operationId `getBooking`, scope
//!   `qos-booking:device-qos-bookings:read`) — reads a created booking back from the
//!   store by its opaque, server-minted id → `200` `BookingInfo` / `404 NOT_FOUND`.
//! - `DELETE /device-qos-bookings/{bookingId}` (operationId `deleteBooking`, scope
//!   `qos-booking:device-qos-bookings:delete`) — evicts a stored booking → `204` /
//!   `404 NOT_FOUND`.
//! - `POST /retrieve-device-qos-bookings` (operationId `retrieveBookingByDevice`,
//!   scope `qos-booking:device-qos-bookings:retrieve-by-device`) — lists a device's
//!   bookings as an array of `BookingInfo` (`200`, empty array when none).
//!
//! CloudEvents notifications on `sink` are a later pass; a supplied
//! `sink`/`sinkCredential` is validated and echoed but not yet acted on (a
//! documented cut).
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Like its QoD / QoS Provisioning siblings, the `device` object is meaningful only
//! in two-legged auth; a three-legged token already identifies the device by its
//! subject (mirrors [`crate::apis::qos_provisioning`]):
//! - `device` supplied **and** the token subject is itself an E.164 line →
//!   `422 UNNECESSARY_IDENTIFIER`.
//! - `device` supplied, subject not a line → that identifier is used (precedence:
//!   phoneNumber → networkAccessIdentifier → the IPv4 `publicAddress` → ipv6Address).
//! - `device` absent, subject an E.164 line → the subject is the identifier.
//! - `device` absent **and** subject not a line → `422 MISSING_IDENTIFIER`.
//! - `device` present but carrying no identifier → `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! - **Reserved error suffix** on the identifier → the canonical CAMARA error
//!   (shared convention; `…409` → `409 CONFLICT`, the "a booking already exists for
//!   the device" case).
//! - **Booking status** from the identifier's trailing three digits: `…000` / no
//!   digits → `REQUESTED` (no `startedAt` yet); an odd tail → `SCHEDULED` (a future
//!   booking, no `startedAt`); any other (even, non-zero) tail → `ACTIVATED`
//!   (`startedAt` = now). CamaraSim validates `startTime` for shape but keys the
//!   status off the identifier so every state is reachable from the input alone (a
//!   documented cut — `startTime` is not used to compute the status).
//! - **`duration`** (seconds, required): `< 1` → `400 OUT_OF_RANGE`;
//!   `> 31_622_400` (366 days, the `BookingInfo` ceiling) → `400
//!   QOS_BOOKING.DURATION_OUT_OF_RANGE`.
//! - **`serviceArea`** (required): a `CIRCLE` with a `center` out of range → `400
//!   OUT_OF_RANGE`, a degenerate radius (`< 1`) → `422 QOS_BOOKING.INVALID_AREA`; an
//!   `AREANAME` whose `areaName` contains `uncovered` → `422
//!   QOS_BOOKING.AREA_NOT_COVERED`; a `POLYGON` → `422
//!   QOS_BOOKING.NOT_MANAGED_AREA_TYPE` (CamaraSim manages `CIRCLE` + `AREANAME`, a
//!   documented cut); an unknown/missing `areaType` → `400 INVALID_ARGUMENT`.
//! - **`qosProfile`** whose name contains `unavailable` →
//!   `422 QOS_BOOKING.QOS_PROFILE_NOT_APPLICABLE`.
//! - **`sink`** (optional) must be an `http://` or `https://` callback URL → else
//!   `400 INVALID_SINK`.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create a booking (CAMARA qos-booking wip).
const CREATE_SCOPE: &str = "qos-booking:device-qos-bookings:create";

/// Scope required to read a booking back (CAMARA qos-booking wip).
const READ_SCOPE: &str = "qos-booking:device-qos-bookings:read";

/// Scope required to delete a booking (CAMARA qos-booking wip).
const DELETE_SCOPE: &str = "qos-booking:device-qos-bookings:delete";

/// Scope required to list a device's bookings (CAMARA qos-booking wip).
const RETRIEVE_SCOPE: &str = "qos-booking:device-qos-bookings:retrieve-by-device";

/// The maximum `duration` (seconds) a booking can span — the CAMARA
/// `BookingInfo.duration` ceiling, `31_622_400` = 366 days.
const MAX_DURATION_SECS: i64 = 31_622_400;

/// Routes for QoS Booking vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/qos-booking/vwip/device-qos-bookings",
            post(create_booking),
        )
        .route(
            "/qos-booking/vwip/device-qos-bookings/:booking_id",
            get(get_booking).delete(delete_booking),
        )
        .route(
            "/qos-booking/vwip/retrieve-device-qos-bookings",
            post(retrieve_bookings),
        )
}

/// `CreateBooking` request body (CAMARA qos-booking wip). `qosProfile`,
/// `startTime`, `duration`, and `serviceArea` are required; `device` is required
/// only for a two-legged token.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBooking {
    device: Option<Device>,
    #[serde(rename = "qosProfile")]
    qos_profile: Option<String>,
    #[serde(rename = "startTime")]
    start_time: Option<String>,
    duration: Option<i64>,
    #[serde(rename = "serviceArea")]
    service_area: Option<Value>,
    sink: Option<String>,
    // Accepted for schema fidelity and never echoed (it carries a secret).
    // Notifications on `sink` are a later slice, so it is not yet applied.
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
    sink_credential: Option<Value>,
    // Accepted for schema fidelity and echoed back verbatim when present.
    #[serde(rename = "applicationServer")]
    application_server: Option<Value>,
    #[serde(rename = "devicePorts")]
    device_ports: Option<Value>,
    #[serde(rename = "applicationServerPorts")]
    application_server_ports: Option<Value>,
}

/// The CAMARA `Device` object: at least one identifier must be present
/// (`minProperties: 1`). CamaraSim keys its functional cases off the first present
/// identifier, in the precedence order below.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Device {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "networkAccessIdentifier")]
    network_access_identifier: Option<String>,
    #[serde(rename = "ipv4Address")]
    ipv4_address: Option<DeviceIpv4Addr>,
    #[serde(rename = "ipv6Address")]
    ipv6_address: Option<String>,
}

/// The CAMARA `DeviceIpv4Addr` object. CamaraSim reads the `publicAddress` as the
/// identifier; the other fields are accepted for schema fidelity.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceIpv4Addr {
    #[serde(rename = "publicAddress")]
    public_address: Option<String>,
    #[serde(rename = "privateAddress")]
    #[allow(dead_code)]
    private_address: Option<String>,
    #[serde(rename = "publicPort")]
    #[allow(dead_code)]
    public_port: Option<i64>,
}

/// `POST /qos-booking/vwip/device-qos-bookings`.
async fn create_booking(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory; parse strictly.
    let req: CreateBooking = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => return invalid_argument("Request body is not a valid CreateBooking.", &correlator),
    };

    // Required `qosProfile`, validated against the CAMARA `QosProfileName` pattern.
    let qos_profile = match req.qos_profile {
        Some(p) if is_valid_qos_profile(&p) => p,
        Some(_) => {
            return invalid_argument(
                "`qosProfile` must match `^[a-zA-Z0-9_.-]+$` and be 3–256 characters.",
                &correlator,
            )
        }
        None => return invalid_argument("`qosProfile` is required.", &correlator),
    };

    // Required `startTime`, an RFC 3339 date-time (validated for shape only).
    let start_time = match req.start_time {
        Some(t) if is_valid_rfc3339(&t) => t,
        Some(_) => {
            return invalid_argument("`startTime` must be an RFC 3339 date-time.", &correlator)
        }
        None => return invalid_argument("`startTime` is required.", &correlator),
    };

    // Required `duration` (seconds). `< 1` → OUT_OF_RANGE; over the ceiling → the
    // API-specific DURATION_OUT_OF_RANGE.
    let duration = match req.duration {
        Some(d) if d < 1 => {
            return out_of_range("`duration` must be at least 1 second.", &correlator)
        }
        Some(d) if d > MAX_DURATION_SECS => {
            return with_correlator(
                CamaraError::new(
                    StatusCode::BAD_REQUEST,
                    "QOS_BOOKING.DURATION_OUT_OF_RANGE",
                    "`duration` exceeds the maximum bookable window (31622400 seconds).",
                )
                .into_response(),
                &correlator,
            )
        }
        Some(d) => d,
        None => return invalid_argument("`duration` is required.", &correlator),
    };

    // Required `serviceArea`, validated (CIRCLE / AREANAME managed, POLYGON a cut).
    let service_area = match req.service_area {
        Some(area) => match validate_service_area(&area, &correlator) {
            Ok(()) => area,
            Err(resp) => return resp,
        },
        None => return invalid_argument("`serviceArea` is required.", &correlator),
    };

    // An optional `sink` must be a well-formed `http://` or `https://` callback URL.
    if let Some(sink) = &req.sink {
        if !is_valid_sink(sink) {
            return with_correlator(
                CamaraError::new(
                    StatusCode::BAD_REQUEST,
                    "INVALID_SINK",
                    "`sink` must be a valid `http://` or `https://` callback URL.",
                )
                .into_response(),
                &correlator,
            );
        }
    }

    // Resolve the identifier, enforcing the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (incl. `…409` → the 409 CONFLICT "a booking already exists" case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // A profile whose name marks it unavailable is not applicable (422).
    if qos_profile.to_ascii_lowercase().contains("unavailable") {
        return with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "QOS_BOOKING.QOS_PROFILE_NOT_APPLICABLE",
                "The requested QoS profile is not applicable for this device.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Build the BookingInfo, remember it, and return 201.
    let booking_id = store::new_booking_id();
    let info = build_booking_info(
        &booking_id,
        &qos_profile,
        &start_time,
        duration,
        service_area,
        resolved.echo,
        req.sink,
        req.application_server,
        req.device_ports,
        req.application_server_ports,
        &resolved.id,
    );
    store::insert(booking_id, info.clone());

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// `GET /qos-booking/vwip/device-qos-bookings/{bookingId}` (operationId `getBooking`).
///
/// Reads a created booking back from the store by its opaque, server-minted
/// `bookingId`. Store state is the sole control plane (the id is opaque, so there is
/// no reserved-identifier plane; mirrors QoS Provisioning's `getQosAssignmentById`
/// and QoD's `getSession`): a stored id → `200` with the persisted `BookingInfo`
/// verbatim, any other id (never created / already deleted) → `404 NOT_FOUND`.
/// `x-correlator` echoed on both.
async fn get_booking(
    claims: Claims,
    headers: HeaderMap,
    Path(booking_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&booking_id) {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No booking found for the provided bookingId.").into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /qos-booking/vwip/device-qos-bookings/{bookingId}` (`deleteBooking`).
///
/// Deletes a stored booking. Keyed only on the store state (the `bookingId` is
/// opaque, so there is no reserved-identifier control plane): an existing booking
/// is evicted → `204 No Content` (single-use); an unknown or already-deleted id →
/// `404 NOT_FOUND`. CAMARA's asynchronous `202 Accepted` (returning `BookingInfo`)
/// form is deferred with `sink` notifications — CamaraSim answers the synchronous
/// `204` (mirrors QoS Provisioning's `revokeQosAssignment` / QoD's `deleteSession`).
/// `x-correlator` echoed on both outcomes.
async fn delete_booking(
    claims: Claims,
    headers: HeaderMap,
    Path(booking_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::remove(&booking_id) {
        Some(_) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No booking found for the provided bookingId.").into_response(),
            &correlator,
        ),
    }
}

/// `RetrieveBookingsInput` request body (CAMARA qos-booking wip): an optional
/// `device`. Omitted entirely for a three-legged token (the device comes from the
/// subject).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveBookingsInput {
    device: Option<Device>,
}

/// `POST /qos-booking/vwip/retrieve-device-qos-bookings` (operationId
/// `retrieveBookingByDevice`).
///
/// Lists the QoS bookings for a device as an array of `BookingInfo` (`200`; an
/// empty array when the device has none — CAMARA never 404s on an empty result).
/// The device is the submitted `device` identifier, else the token subject
/// (three-legged fallback; neither present → `422 MISSING_IDENTIFIER`). Two control
/// planes (docs/DESIGN.md §7): the identifier — a reserved error suffix selects a
/// canonical CAMARA error (so `…404` → `404 NOT_FOUND` for an unknown device) — and,
/// on the happy path, the in-memory store, matched by each booking's echoed
/// `device`. A resolved identifier with no `device` echo matches nothing → `200 []`.
/// `x-correlator` echoed on every response. Mirrors QoD's `retrieveSessionsByDevice`.
async fn retrieve_bookings(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `device` is optional, so an empty body is accepted as `{}` (three-legged:
    // the device comes from the token subject). A non-empty body must be valid.
    let req: RetrieveBookingsInput = if body.is_empty() {
        RetrieveBookingsInput { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrieveBookingsInput.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the identifier (submitted device, else token subject), enforcing the
    // two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (…404 → 404 NOT_FOUND, the device-identifier-not-found case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Match stored bookings by their echoed `device`. A resolved identifier with
    // no device echo matches nothing.
    let bookings = match resolved.echo {
        Some(echo) => store::find_by_device(&echo),
        None => Vec::new(),
    };

    with_correlator((StatusCode::OK, Json(bookings)).into_response(), &correlator)
}

/// Render the `BookingInfo` for a created booking.
#[allow(clippy::too_many_arguments)]
fn build_booking_info(
    booking_id: &str,
    qos_profile: &str,
    start_time: &str,
    duration: i64,
    service_area: Value,
    device_echo: Option<Value>,
    sink: Option<String>,
    application_server: Option<Value>,
    device_ports: Option<Value>,
    application_server_ports: Option<Value>,
    identifier: &str,
) -> Value {
    let mut info = json!({
        "bookingId": booking_id,
        "qosProfile": qos_profile,
        "startTime": start_time,
        "duration": duration,
        "serviceArea": service_area,
    });

    // Booking state from the identifier's trailing three digits (docs/DESIGN.md §7):
    //   `…000` / none → REQUESTED (accepted, not yet scheduled — no startedAt)
    //   odd tail      → SCHEDULED (a future booking — no startedAt)
    //   other tail    → ACTIVATED (window active — startedAt = now)
    match scenarios::trailing_three_digits(identifier) {
        Some(0) | None => {
            info["bookingStatus"] = json!("REQUESTED");
        }
        Some(d) if d % 2 == 1 => {
            info["bookingStatus"] = json!("SCHEDULED");
        }
        Some(_) => {
            info["bookingStatus"] = json!("ACTIVATED");
            info["startedAt"] = json!(rfc3339_utc(now_unix_secs()));
        }
    }

    if let Some(echo) = device_echo {
        info["device"] = echo;
    }
    if let Some(s) = sink {
        info["sink"] = json!(s);
    }
    if let Some(v) = application_server {
        info["applicationServer"] = v;
    }
    if let Some(v) = device_ports {
        info["devicePorts"] = v;
    }
    if let Some(v) = application_server_ports {
        info["applicationServerPorts"] = v;
    }
    info
}

/// Validate the required `serviceArea` (CAMARA `Area`, discriminated by `areaType`).
///
/// CamaraSim manages `CIRCLE` and `AREANAME`; a `POLYGON` is a documented cut
/// (`422 QOS_BOOKING.NOT_MANAGED_AREA_TYPE`). See the module docs for the cases.
fn validate_service_area(area: &Value, correlator: &Option<HeaderValue>) -> Result<(), Response> {
    let Some(area_type) = area.get("areaType").and_then(Value::as_str) else {
        return Err(invalid_argument(
            "`serviceArea` requires an `areaType` (CIRCLE, POLYGON, or AREANAME).",
            correlator,
        ));
    };
    match area_type {
        "CIRCLE" => {
            let center = area.get("center");
            let radius = area.get("radius").and_then(Value::as_f64);
            let (Some(center), Some(radius)) = (center, radius) else {
                return Err(invalid_argument(
                    "A CIRCLE `serviceArea` requires a `center` and a numeric `radius`.",
                    correlator,
                ));
            };
            let lat = center.get("latitude").and_then(Value::as_f64);
            let lon = center.get("longitude").and_then(Value::as_f64);
            let (Some(lat), Some(lon)) = (lat, lon) else {
                return Err(invalid_argument(
                    "A CIRCLE `center` requires numeric `latitude` and `longitude`.",
                    correlator,
                ));
            };
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                return Err(out_of_range(
                    "`center.latitude` must be in [-90, 90] and `center.longitude` in [-180, 180].",
                    correlator,
                ));
            }
            if !radius.is_finite() || radius < 1.0 {
                return Err(unprocessable(
                    "QOS_BOOKING.INVALID_AREA",
                    "The CIRCLE `radius` must be a positive number of metres.",
                    correlator,
                ));
            }
            Ok(())
        }
        "AREANAME" => match area.get("areaName").and_then(Value::as_str) {
            Some(name) if !name.trim().is_empty() => {
                if name.to_ascii_lowercase().contains("uncovered") {
                    return Err(unprocessable(
                        "QOS_BOOKING.AREA_NOT_COVERED",
                        "The requested area is not covered by the network.",
                        correlator,
                    ));
                }
                Ok(())
            }
            _ => Err(invalid_argument(
                "An AREANAME `serviceArea` requires a non-empty `areaName`.",
                correlator,
            )),
        },
        "POLYGON" => Err(unprocessable(
            "QOS_BOOKING.NOT_MANAGED_AREA_TYPE",
            "CamaraSim manages CIRCLE and AREANAME service areas; POLYGON is not managed.",
            correlator,
        )),
        _ => Err(invalid_argument(
            "`serviceArea.areaType` must be one of CIRCLE, POLYGON, or AREANAME.",
            correlator,
        )),
    }
}

/// A resolved request identifier: the string CamaraSim keys functional cases off,
/// plus the single-property `device` object to echo in the response (`None` when the
/// identifier came from a token subject that is not a phone number).
struct Resolved {
    id: String,
    echo: Option<Value>,
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors QoS
/// Provisioning). See the module docs for the cases.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                let echo = json!({ "phoneNumber": phone });
                Ok(Resolved { id: phone, echo: Some(echo) })
            }
            Some(DeviceId::Nai(id)) => {
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                let echo = json!({ "networkAccessIdentifier": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv4(id)) => {
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                let echo = json!({ "ipv4Address": { "publicAddress": id } });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv6(id)) => {
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                let echo = json!({ "ipv6Address": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            if subject_is_line {
                let echo = json!({ "phoneNumber": subject });
                Ok(Resolved { id: subject.to_string(), echo: Some(echo) })
            } else {
                Err(unprocessable(
                    "MISSING_IDENTIFIER",
                    "The device cannot be identified: supply a `device` or use a token that identifies a device.",
                    correlator,
                ))
            }
        }
    }
}

/// The identifier CamaraSim reads from a `Device`, tagged by kind so the response
/// can echo the right `DeviceResponse` field. Precedence: phoneNumber,
/// networkAccessIdentifier, the IPv4 `publicAddress`, then ipv6Address.
enum DeviceId {
    PhoneNumber(String),
    Nai(String),
    Ipv4(String),
    Ipv6(String),
}

/// The first present identifier of a `Device`, in precedence order, or `None` when
/// the device carries no identifier at all (`minProperties: 1` violated).
fn device_identifier(device: &Device) -> Option<DeviceId> {
    if let Some(phone) = &device.phone_number {
        return Some(DeviceId::PhoneNumber(phone.clone()));
    }
    if let Some(nai) = &device.network_access_identifier {
        return Some(DeviceId::Nai(nai.clone()));
    }
    if let Some(ipv4) = &device.ipv4_address {
        if let Some(addr) = &ipv4.public_address {
            return Some(DeviceId::Ipv4(addr.clone()));
        }
    }
    if let Some(ipv6) = &device.ipv6_address {
        return Some(DeviceId::Ipv6(ipv6.clone()));
    }
    None
}

/// Whether `s` is a valid CAMARA `qosProfile`: `^[a-zA-Z0-9_.-]+$`, length 3–256.
fn is_valid_qos_profile(s: &str) -> bool {
    let len = s.chars().count();
    (3..=256).contains(&len)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// Whether `s` is a well-formed callback sink URL — `http://…` or `https://…` with a
/// non-empty authority. CAMARA's schema requires `https://`; CamaraSim also accepts
/// `http://` so a loopback receiver can observe callbacks once delivery lands (a
/// later slice). Any other scheme → `INVALID_SINK`.
fn is_valid_sink(s: &str) -> bool {
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .is_some_and(|rest| !rest.is_empty())
}

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
}

/// A minimal structural check that `s` is an RFC 3339 date-time:
/// `YYYY-MM-DDTHH:MM:SS` with an optional fractional part and a `Z` or `±HH:MM`
/// offset. CamaraSim validates the shape (so a malformed `startTime` is a 400) but
/// does not parse it into an instant this slice — the booking status is keyed off
/// the identifier (a documented cut). Self-contained (no date/time dependency).
fn is_valid_rfc3339(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 20 {
        return false;
    }
    let date_ok = b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit()
        && (b[10] == b'T' || b[10] == b't');
    let time_ok = b[11].is_ascii_digit()
        && b[12].is_ascii_digit()
        && b[13] == b':'
        && b[14].is_ascii_digit()
        && b[15].is_ascii_digit()
        && b[16] == b':'
        && b[17].is_ascii_digit()
        && b[18].is_ascii_digit();
    if !date_ok || !time_ok {
        return false;
    }
    // Remainder after seconds: an optional `.fraction`, then `Z`/`z` or an offset.
    let mut rest = &s[19..];
    if let Some(frac) = rest.strip_prefix('.') {
        let n = frac.bytes().take_while(u8::is_ascii_digit).count();
        if n == 0 {
            return false;
        }
        rest = &frac[n..];
    }
    matches!(rest, "Z" | "z") || is_utc_offset(rest)
}

/// Whether `s` is an RFC 3339 numeric offset `±HH:MM`.
fn is_utc_offset(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 6
        && matches!(b[0], b'+' | b'-')
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3] == b':'
        && b[4].is_ascii_digit()
        && b[5].is_ascii_digit()
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
}

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed.
fn out_of_range(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, "OUT_OF_RANGE", message).into_response(),
        correlator,
    )
}

/// A 422 CAMARA error with a caller-chosen `code`, correlator echoed.
fn unprocessable(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message).into_response(),
        correlator,
    )
}

/// Echo the request's `x-correlator` onto a response, if one was supplied.
fn with_correlator(mut response: Response, correlator: &Option<HeaderValue>) -> Response {
    if let Some(value) = correlator {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-correlator"), value.clone());
    }
    response
}

/// Seconds since the Unix epoch, UTC.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z` offset.
/// Self-contained so CamaraSim needs no date/time dependency (mirrors QoS Provisioning).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for any date).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "qosbook.local:8080";
    const BOOKINGS: &str = "/qos-booking/vwip/device-qos-bookings";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn qos_profile_validation_follows_the_camara_pattern() {
        assert!(is_valid_qos_profile("QOS_E"));
        assert!(is_valid_qos_profile("low-latency.v1"));
        assert!(!is_valid_qos_profile("ab")); // too short
        assert!(!is_valid_qos_profile("has space"));
        assert!(!is_valid_qos_profile("bad!char"));
    }

    #[test]
    fn sink_validation_accepts_http_and_https_only() {
        assert!(is_valid_sink("https://example.com/callback"));
        assert!(is_valid_sink("http://example.com/callback"));
        assert!(!is_valid_sink("https://"));
        assert!(!is_valid_sink("ftp://example.com"));
    }

    #[test]
    fn rfc3339_validation_accepts_z_fraction_and_offset_only() {
        assert!(is_valid_rfc3339("2024-06-01T12:00:00Z"));
        assert!(is_valid_rfc3339("2024-06-01T12:00:00.500Z"));
        assert!(is_valid_rfc3339("2024-06-01T12:00:00+01:00"));
        assert!(!is_valid_rfc3339("2024-06-01 12:00:00")); // no T
        assert!(!is_valid_rfc3339("2024-06-01T12:00:00")); // no zone
        assert!(!is_valid_rfc3339("not-a-date"));
    }

    #[test]
    fn device_identifier_follows_precedence() {
        let d = Device {
            phone_number: Some("+123456789012".into()),
            network_access_identifier: Some("nai@x".into()),
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(matches!(device_identifier(&d), Some(DeviceId::PhoneNumber(_))));
        let d = Device {
            phone_number: None,
            network_access_identifier: None,
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(device_identifier(&d).is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token_with_client(scope: &str, client_id: &str) -> String {
        let enc = client_id.replace('+', "%2B");
        let body = format!("grant_type=client_credentials&client_id={enc}&scope={scope}");
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/oauth2/token")
                    .header("host", HOST)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        json["access_token"].as_str().unwrap().to_string()
    }

    /// A two-legged token (client id is not a phone number) with `scope`.
    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "qosbook-client").await
    }

    async fn post_booking(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(BOOKINGS)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    fn valid_area() -> Value {
        json!({"areaType": "CIRCLE", "center": {"latitude": 51.5, "longitude": -0.12}, "radius": 2000})
    }

    /// A well-formed CreateBooking body for a two-legged `device`/`profile`.
    fn create_body(phone: &str, profile: &str) -> String {
        json!({
            "device": { "phoneNumber": phone },
            "qosProfile": profile,
            "startTime": "2024-06-01T12:00:00Z",
            "duration": 3600,
            "serviceArea": valid_area(),
        })
        .to_string()
    }

    #[tokio::test]
    async fn create_activated_and_echoes_the_request() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, body) =
            post_booking(Some(&token), &create_body("+123456789012", "QOS_E"), Some("corr-1")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["bookingStatus"], "ACTIVATED"); // …012 → even, non-zero
        assert_eq!(body["qosProfile"], "QOS_E");
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
        assert_eq!(body["startTime"], "2024-06-01T12:00:00Z");
        assert_eq!(body["duration"], 3600);
        assert_eq!(body["serviceArea"], valid_area());
        assert_eq!(body["bookingId"].as_str().unwrap().split('-').count(), 5);
        assert!(body["startedAt"].as_str().unwrap().ends_with('Z'));
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-1");
        // Persisted: the created booking reads back from the store verbatim.
        let id = body["bookingId"].as_str().unwrap();
        assert_eq!(store::get(id), Some(body));
    }

    #[tokio::test]
    async fn odd_identifier_tail_is_scheduled_without_start_time() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_booking(Some(&token), &create_body("+123456789013", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["bookingStatus"], "SCHEDULED"); // …013 → odd
        assert!(body.get("startedAt").is_none());
    }

    #[tokio::test]
    async fn triple_zero_identifier_is_requested_without_start_time() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_booking(Some(&token), &create_body("+123456789000", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["bookingStatus"], "REQUESTED"); // …000
        assert!(body.get("startedAt").is_none());
    }

    #[tokio::test]
    async fn reserved_suffix_selects_the_canonical_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_booking(Some(&token), &create_body("+123456789404", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn reserved_409_suffix_is_conflict() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_booking(Some(&token), &create_body("+123456789409", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "CONFLICT");
    }

    #[tokio::test]
    async fn missing_or_bad_qos_profile_is_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let no_profile = json!({
            "device": {"phoneNumber": "+123456789012"},
            "startTime": "2024-06-01T12:00:00Z", "duration": 3600, "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &no_profile, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let (status, _, _) =
            post_booking(Some(&token), &create_body("+123456789012", "ab"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_or_bad_start_time_is_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let no_start = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "duration": 3600, "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &no_start, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let bad_start = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "startTime": "yesterday", "duration": 3600,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, _) = post_booking(Some(&token), &bad_start, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn duration_below_one_is_out_of_range_and_over_ceiling_is_duration_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let zero = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 0,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &zero, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");

        let over = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 31_622_401_i64,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &over, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "QOS_BOOKING.DURATION_OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn service_area_is_required() {
        let token = mint_token(CREATE_SCOPE).await;
        let no_area = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &no_area, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    async fn post_with_area(token: &str, phone: &str, area: Value) -> (StatusCode, Value) {
        let body = json!({
            "device": {"phoneNumber": phone},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": area,
        })
        .to_string();
        let (status, _, body) = post_booking(Some(token), &body, None).await;
        (status, body)
    }

    #[tokio::test]
    async fn circle_with_degenerate_radius_is_invalid_area() {
        let token = mint_token(CREATE_SCOPE).await;
        let area = json!({"areaType": "CIRCLE", "center": {"latitude": 51.5, "longitude": -0.12}, "radius": 0});
        let (status, body) = post_with_area(&token, "+123456789012", area).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "QOS_BOOKING.INVALID_AREA");
    }

    #[tokio::test]
    async fn circle_center_out_of_range_is_400_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let area = json!({"areaType": "CIRCLE", "center": {"latitude": 100.0, "longitude": 0.0}, "radius": 2000});
        let (status, body) = post_with_area(&token, "+123456789012", area).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn areaname_is_accepted_and_uncovered_is_area_not_covered() {
        let token = mint_token(CREATE_SCOPE).await;
        let ok = json!({"areaType": "AREANAME", "areaName": "Greater London"});
        let (status, body) = post_with_area(&token, "+123456789012", ok).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["bookingStatus"], "ACTIVATED");

        let uncovered = json!({"areaType": "AREANAME", "areaName": "Deep Ocean (uncovered)"});
        let (status, body) = post_with_area(&token, "+123456789012", uncovered).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "QOS_BOOKING.AREA_NOT_COVERED");
    }

    #[tokio::test]
    async fn polygon_is_not_managed_and_unknown_area_type_is_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let polygon = json!({"areaType": "POLYGON", "boundary": [
            {"latitude": 0.0, "longitude": 0.0},
            {"latitude": 0.0, "longitude": 1.0},
            {"latitude": 1.0, "longitude": 0.0}
        ]});
        let (status, body) = post_with_area(&token, "+123456789012", polygon).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "QOS_BOOKING.NOT_MANAGED_AREA_TYPE");

        let unknown = json!({"areaType": "SQUARE"});
        let (status, body) = post_with_area(&token, "+123456789012", unknown).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unavailable_qos_profile_is_not_applicable() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_booking(Some(&token), &create_body("+123456789012", "qos-unavailable"), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "QOS_BOOKING.QOS_PROFILE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn invalid_sink_is_400_and_valid_sink_is_echoed() {
        let token = mint_token(CREATE_SCOPE).await;
        let bad = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": valid_area(), "sink": "ftp://nope",
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &bad, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_SINK");

        let good = json!({
            "device": {"phoneNumber": "+123456789012"},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": valid_area(), "sink": "https://example.com/cb",
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &good, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["sink"], "https://example.com/cb");
    }

    #[tokio::test]
    async fn device_without_identifier_is_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": {},
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn device_on_a_line_token_is_422_unnecessary_identifier() {
        // Three-legged: the token subject is itself an E.164 line.
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_booking(Some(&token), &create_body("+123456789050", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_422_missing_identifier() {
        let token = mint_token(CREATE_SCOPE).await; // subject "qosbook-client" (not a line)
        let body = json!({
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn three_legged_line_subject_is_used_when_no_device() {
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let body = json!({
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, body) = post_booking(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["bookingStatus"], "ACTIVATED"); // …012
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn auth_is_required_and_scoped() {
        // No token → 401.
        let (status, _, _) =
            post_booking(None, &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // Wrong scope → 403.
        let token = mint_token("qos-booking:something-else").await;
        let (status, _, _) =
            post_booking(Some(&token), &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // --- Read-back: GET /device-qos-bookings/{bookingId} -------------------

    async fn get_booking_req(
        token: Option<&str>,
        booking_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{BOOKINGS}/{booking_id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn get_reads_a_created_booking_back_verbatim() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_booking(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["bookingId"].as_str().unwrap().to_string();

        let read = mint_token(READ_SCOPE).await;
        let (status, headers, body) = get_booking_req(Some(&read), &id, Some("corr-get")).await;
        assert_eq!(status, StatusCode::OK);
        // The read-back is the created representation, byte for byte.
        assert_eq!(body, created);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-get");
    }

    #[tokio::test]
    async fn get_unknown_booking_is_404_not_found() {
        let read = mint_token(READ_SCOPE).await;
        let (status, headers, body) = get_booking_req(
            Some(&read),
            "00000000-0000-4000-8000-000000000000",
            Some("corr-404"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-404");
    }

    #[tokio::test]
    async fn get_requires_auth_and_the_read_scope() {
        // Create a booking to have a real id to target.
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_booking(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        let id = created["bookingId"].as_str().unwrap().to_string();

        // No token → 401.
        let (status, _, _) = get_booking_req(None, &id, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // The create scope is not the read scope → 403.
        let (status, _, _) = get_booking_req(Some(&create), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // --- Delete: DELETE /device-qos-bookings/{bookingId} -------------------

    async fn delete_booking_req(
        token: Option<&str>,
        booking_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("{BOOKINGS}/{booking_id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn delete_evicts_the_booking_204_then_get_is_404() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_booking(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["bookingId"].as_str().unwrap().to_string();

        // Delete → 204 No Content (no body), x-correlator echoed.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, body) = delete_booking_req(Some(&del), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null, "204 carries no body");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-del");

        // The booking is gone: a subsequent read is 404.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_booking_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_404() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_booking(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        let id = created["bookingId"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_booking_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // A second delete of the same id is 404 NOT_FOUND.
        let (status, _, body) = delete_booking_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_booking_is_404_not_found() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, body) = delete_booking_req(
            Some(&del),
            "00000000-0000-4000-8000-000000000000",
            Some("corr-404"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-404");
    }

    #[tokio::test]
    async fn delete_requires_auth_and_the_delete_scope() {
        // Create a booking to have a real id to target.
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_booking(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        let id = created["bookingId"].as_str().unwrap().to_string();

        // No token → 401.
        let (status, _, _) = delete_booking_req(None, &id, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // The read scope is not the delete scope → 403 (booking still present).
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = delete_booking_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // --- Retrieve-by-device: POST /retrieve-device-qos-bookings ------------

    const RETRIEVE_BOOKINGS: &str = "/qos-booking/vwip/retrieve-device-qos-bookings";

    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(RETRIEVE_BOOKINGS)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// A retrieve-by-device body naming `phone` as the device.
    fn retrieve_body(phone: &str) -> String {
        json!({ "device": { "phoneNumber": phone } }).to_string()
    }

    #[tokio::test]
    async fn retrieve_returns_only_the_requested_devices_bookings() {
        // Two bookings for device A, one for device B — each a distinct phone so
        // this test is isolated from other tests sharing the process-global store.
        let create = mint_token(CREATE_SCOPE).await;
        let dev_a = "+199900010012";
        let dev_b = "+199900020012";
        for _ in 0..2 {
            let (status, _, _) =
                post_booking(Some(&create), &create_body(dev_a, "QOS_E"), None).await;
            assert_eq!(status, StatusCode::CREATED);
        }
        let (status, _, _) = post_booking(Some(&create), &create_body(dev_b, "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);

        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, body) =
            post_retrieve(Some(&retrieve), &retrieve_body(dev_a), Some("corr-ret")).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("a JSON array of BookingInfo");
        assert_eq!(arr.len(), 2, "only device A's two bookings");
        assert!(arr.iter().all(|b| b["device"]["phoneNumber"] == dev_a));
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-ret");

        let (status, _, body) = post_retrieve(Some(&retrieve), &retrieve_body(dev_b), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1, "only device B's one booking");
    }

    #[tokio::test]
    async fn retrieve_for_a_device_with_none_is_empty_array() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) =
            post_retrieve(Some(&retrieve), &retrieve_body("+199988870013"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]), "a device with no bookings is 200 []");
    }

    #[tokio::test]
    async fn retrieve_reserved_suffix_selects_a_camara_error() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) =
            post_retrieve(Some(&retrieve), &retrieve_body("+199988870404"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        let (status, _, body) =
            post_retrieve(Some(&retrieve), &retrieve_body("+199988870429"), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn retrieve_falls_back_to_the_token_subject() {
        // Three-legged: a booking created on the subject line is listed when the
        // retrieve omits `device` (the subject identifies the device).
        let subject = "+199900030012";
        let create = mint_token_with_client(CREATE_SCOPE, subject).await;
        let body = json!({
            "qosProfile": "QOS_E", "startTime": "2024-06-01T12:00:00Z", "duration": 3600,
            "serviceArea": valid_area(),
        })
        .to_string();
        let (status, _, _) = post_booking(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);

        let retrieve = mint_token_with_client(RETRIEVE_SCOPE, subject).await;
        let (status, _, body) = post_retrieve(Some(&retrieve), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert!(!arr.is_empty(), "the subject's booking is listed");
        assert!(arr.iter().all(|b| b["device"]["phoneNumber"] == subject));
    }

    #[tokio::test]
    async fn retrieve_no_device_and_non_line_subject_is_422_missing_identifier() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await; // subject not a line
        let (status, _, body) = post_retrieve(Some(&retrieve), "{}", None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn retrieve_device_on_a_line_token_is_422_unnecessary_identifier() {
        let retrieve = mint_token_with_client(RETRIEVE_SCOPE, "+199900030099").await;
        let (status, _, body) =
            post_retrieve(Some(&retrieve), &retrieve_body("+199900030098"), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn retrieve_rejects_a_bad_body() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) = post_retrieve(Some(&retrieve), "{ not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_requires_auth_and_the_retrieve_scope() {
        // No token → 401.
        let (status, _, _) = post_retrieve(None, &retrieve_body("+199988870012"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // The read scope is not the retrieve scope → 403.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) =
            post_retrieve(Some(&read), &retrieve_body("+199988870012"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
