//! QoS Provisioning **v0.3** (CAMARA qos-provisioning 0.3.0, release r3.2),
//! mounted at `/qos-provisioning/v0.3`.
//!
//! This slice implements the create + read-by-id + revoke + retrieve-by-device set:
//! - `POST /qos-assignments` (operationId `createQosAssignment`, scope
//!   `qos-provisioning:qos-assignments:create`) — provisions a QoS profile for a
//!   device, mints an opaque `assignmentId` ([`super::store`]), remembers the
//!   rendered `AssignmentInfo`, and returns `201`.
//! - `GET /qos-assignments/{assignmentId}` (operationId `getQosAssignmentById`,
//!   scope `qos-provisioning:qos-assignments:read`) — reads a stored assignment
//!   back (`200`) or `404 NOT_FOUND` for an unknown id.
//! - `DELETE /qos-assignments/{assignmentId}` (operationId `revokeQosAssignment`,
//!   scope `qos-provisioning:qos-assignments:delete`) — revokes a stored
//!   assignment, evicting it from the store (`204 No Content`, single-use) or
//!   `404 NOT_FOUND` for an unknown / already-revoked id. If the revoked
//!   assignment recorded a `sink`, a `status-changed` CloudEvent
//!   (`status: UNAVAILABLE`, `statusInfo: DELETE_REQUESTED`) is delivered to it
//!   fire-and-forget (see [`super::notifications`]); the response is still the
//!   synchronous `204` (CAMARA's async `202 Accepted` form is a documented cut,
//!   mirroring QoD's `deleteSession`).
//! - `POST /retrieve-qos-assignment` (operationId `getQosAssignmentByDevice`,
//!   scope `qos-provisioning:qos-assignments:read-by-device`) — returns the
//!   assignment currently provisioned for a device (`200`) or `404 NOT_FOUND`
//!   when the device has none. Two control planes: the resolved identifier's
//!   reserved error suffix → canonical CAMARA error, else the in-memory store
//!   (mirroring QoD's `retrieveSessionsByDevice`).
//!
//! CloudEvents notifications on `sink` (see [`super::notifications`]):
//! - **`AVAILABLE`-on-provisioning** — creating an `AVAILABLE`, sink-bearing
//!   assignment delivers a `status-changed` CloudEvent (`status: AVAILABLE`, no
//!   `statusInfo`) once, fire-and-forget, signalling the provisioning is active.
//! - **`NETWORK_TERMINATED`** — a `…001` identifier tail on an `AVAILABLE`,
//!   sink-bearing assignment schedules an early network drop
//!   ([`spawn_network_termination`]): a short fire-and-forget timer evicts it and
//!   delivers `status: UNAVAILABLE`, `statusInfo: NETWORK_TERMINATED` (in place of
//!   the `AVAILABLE` event).
//! - **`DELETE_REQUESTED`** — revoking a sink-bearing assignment delivers
//!   `status: UNAVAILABLE`, `statusInfo: DELETE_REQUESTED` (see
//!   `revokeQosAssignment` below).
//!
//! An `AVAILABLE` assignment holds its derived `sinkCredential` `Authorization`
//! (ACCESSTOKEN → Bearer, PLAIN → Basic) across callbacks (the AVAILABLE event
//! *peeks* it; the terminal revoke / network-drop event *takes* it single-use). Only
//! TLS (`https://` sink) delivery remains a documented cut.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Like its QoD sibling, the `device` object is meaningful only in two-legged
//! auth; a three-legged token already identifies the device by its subject
//! (mirrors [`crate::apis::connected_network_type`]):
//! - `device` supplied **and** the token subject is itself an E.164 line →
//!   `422 UNNECESSARY_IDENTIFIER`.
//! - `device` supplied, subject not a line → that identifier is used
//!   (precedence: phoneNumber → networkAccessIdentifier → the IPv4
//!   `publicAddress` → ipv6Address).
//! - `device` absent, subject an E.164 line → the subject is the identifier.
//! - `device` absent **and** subject not a line → `422 MISSING_IDENTIFIER`.
//! - `device` present but carrying no identifier → `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! - **Reserved error suffix** on the identifier → the canonical CAMARA error
//!   (shared convention; `…409` → `409 CONFLICT`, the "existing provisioning for
//!   the same device" case).
//! - **Grant status** from the identifier's trailing three digits: `…000` / no
//!   digits → `REQUESTED` (no `startedAt` yet); any other tail → `AVAILABLE`
//!   (`startedAt` = now). QoS Provisioning has no `duration`, so there is no
//!   `expiresAt` — the assignment is open-ended.
//! - **`qosProfile`** whose name contains `unavailable` →
//!   `422 QOS_PROVISIONING.QOS_PROFILE_NOT_APPLICABLE`.
//! - **`sink`** (optional) must be an `http://` or `https://` callback URL → else
//!   `400 INVALID_SINK`. CAMARA's schema requires `https://`; CamaraSim also
//!   accepts `http://` so a loopback receiver can observe callbacks, and delivers
//!   only to `http://` (no TLS client — `https://` is a documented no-op cut,
//!   mirroring QoD).

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{notifications, store};
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create an assignment (CAMARA qos-provisioning 0.3.0).
const CREATE_SCOPE: &str = "qos-provisioning:qos-assignments:create";
/// Scope required to read an assignment (CAMARA qos-provisioning 0.3.0).
const READ_SCOPE: &str = "qos-provisioning:qos-assignments:read";
/// Scope required to revoke an assignment (CAMARA qos-provisioning 0.3.0).
const DELETE_SCOPE: &str = "qos-provisioning:qos-assignments:delete";
/// Scope required to retrieve an assignment by device (CAMARA qos-provisioning 0.3.0).
const READ_BY_DEVICE_SCOPE: &str = "qos-provisioning:qos-assignments:read-by-device";

/// The trailing-three-digits tail (`…001`) selecting the `NETWORK_TERMINATED`
/// transition: an `AVAILABLE`, sink-bearing assignment whose identifier ends
/// `…001` is dropped *early* by the (simulated) network shortly after
/// provisioning, rather than delivering the ordinary `AVAILABLE` event (mirrors
/// QoD's `NETWORK_TERMINATION_TAIL`).
const NETWORK_TERMINATION_TAIL: u16 = 1;

/// How long a `…001` `AVAILABLE` assignment survives before the (simulated)
/// network drops it — a short, fixed grace, independent of the (open-ended)
/// provisioning (mirrors QoD's `NETWORK_TERMINATION_GRACE_SECS`).
const NETWORK_TERMINATION_GRACE_SECS: u64 = 1;

/// Routes for QoS Provisioning v0.3, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/qos-provisioning/v0.3/qos-assignments",
            post(create_qos_assignment),
        )
        .route(
            "/qos-provisioning/v0.3/qos-assignments/:assignment_id",
            get(get_qos_assignment_by_id).delete(revoke_qos_assignment),
        )
        .route(
            "/qos-provisioning/v0.3/retrieve-qos-assignment",
            post(retrieve_qos_assignment_by_device),
        )
}

/// `CreateAssignment` request body (CAMARA 0.3.0). `qosProfile` is required;
/// `device` is required only for a two-legged token.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAssignment {
    device: Option<Device>,
    #[serde(rename = "qosProfile")]
    qos_profile: Option<String>,
    sink: Option<String>,
    // Accepted for schema fidelity and never echoed (it carries a secret). When a
    // `sink` is supplied, the credential is applied to the status-change callback's
    // `Authorization` header (via `notifications::sink_authorization`): `ACCESSTOKEN`
    // → `Bearer <token>` (RFC 6750), `PLAIN` → `Basic base64(identifier:secret)`
    // (RFC 7617); REFRESHTOKEN is a documented cut. Held in memory only (single node)
    // and taken single-use at delivery.
    #[serde(rename = "sinkCredential")]
    sink_credential: Option<Value>,
}

/// The CAMARA `Device` object: at least one identifier must be present
/// (`minProperties: 1`). CamaraSim keys its functional cases off the first
/// present identifier, in the precedence order below.
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

/// `POST /qos-provisioning/v0.3/qos-assignments`.
async fn create_qos_assignment(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (`qosProfile` is required); parse strictly.
    let req: CreateAssignment = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid CreateAssignment.", &correlator)
        }
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

    // An optional `sink` must be a well-formed `http://` or `https://` callback
    // URL → else 400 INVALID_SINK. CAMARA's schema requires `https://`; CamaraSim
    // also accepts `http://` so a loopback test receiver can observe callbacks,
    // and delivers only to `http://` (no TLS client — an `https://` sink is a
    // documented no-op cut, mirroring QoD / Geofencing / Carrier Billing).
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
    // (incl. `…409` → the 409 CONFLICT "existing provisioning" case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // A profile whose name marks it unavailable is not applicable (422).
    if qos_profile.to_ascii_lowercase().contains("unavailable") {
        return with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "QOS_PROVISIONING.QOS_PROFILE_NOT_APPLICABLE",
                "The requested QoS profile is not applicable for this device.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Build the AssignmentInfo, remember it, and return 201.
    let assignment_id = store::new_assignment_id();
    let info = build_assignment_info(
        &assignment_id,
        resolved.echo,
        &qos_profile,
        req.sink,
        &resolved.id,
    );

    // If the assignment records a `sink` and a `sinkCredential`, stash the derived
    // `Authorization` (ACCESSTOKEN → Bearer, PLAIN → Basic) so a later status-change
    // callback (e.g. `DELETE_REQUESTED` on revoke) can authenticate. Kept apart from
    // the `AssignmentInfo` so the secret is never echoed (mirrors QoD).
    if info.get("sink").is_some() {
        if let Some(auth) = req
            .sink_credential
            .as_ref()
            .and_then(notifications::sink_authorization)
        {
            store::insert_credential(assignment_id.clone(), auth);
        }
    }

    // Remember the assignment *before* scheduling any notification, so a spawned
    // network-termination timer always sees the stored assignment to evict.
    store::insert(assignment_id.clone(), info.clone());

    // Notify a recorded `sink` of the provisioning's status. A REQUESTED
    // assignment is not yet active, so it emits nothing (it would fire an
    // AVAILABLE event only once it transitioned — which CamaraSim does not model).
    // An AVAILABLE, sink-bearing assignment either:
    //   - `…001` (`NETWORK_TERMINATION_TAIL`): the (simulated) network drops it
    //     early — a short fire-and-forget timer evicts it and delivers the terminal
    //     `UNAVAILABLE`/`NETWORK_TERMINATED` event (`spawn_network_termination`); or
    //   - any other tail: the provisioning is active — deliver a single
    //     `AVAILABLE` `status-changed` CloudEvent (no `statusInfo`), fire-and-forget
    //     off the request path. The ACCESSTOKEN `sinkCredential` bearer is applied
    //     but *peeked* (not consumed), so a later `DELETE_REQUESTED` on revoke can
    //     still authenticate.
    if info["status"] == "AVAILABLE" {
        if let Some(sink) = info.get("sink").and_then(Value::as_str) {
            if scenarios::trailing_three_digits(&resolved.id) == Some(NETWORK_TERMINATION_TAIL) {
                spawn_network_termination(assignment_id.clone(), sink.to_string());
            } else {
                let event = notifications::status_changed_event(
                    store::new_event_id(),
                    rfc3339_utc(now_unix_secs()),
                    &assignment_id,
                    "AVAILABLE",
                    None,
                );
                notifications::spawn_delivery(
                    sink.to_string(),
                    event,
                    store::peek_credential(&assignment_id),
                );
            }
        }
    }

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// Schedule the `NETWORK_TERMINATED` status transition for a `…001` assignment.
///
/// Spawns a fire-and-forget async timer (never on the request path, DESIGN §11)
/// that waits [`NETWORK_TERMINATION_GRACE_SECS`] — a short, fixed grace — then, if
/// the assignment still exists, evicts it and delivers a `status-changed`
/// CloudEvent (`status: UNAVAILABLE`, `statusInfo: NETWORK_TERMINATED`) to `sink`,
/// modelling the network dropping a freshly-provisioned assignment early. A
/// `revokeQosAssignment` that removed the assignment first makes this a no-op (the
/// concurrent revoke already fired `DELETE_REQUESTED`; `store::remove` is then
/// `None`, so exactly one terminal event fires). The ACCESSTOKEN `sinkCredential`
/// bearer (if any) is taken single-use. Mirrors QoD's `spawn_network_termination`.
fn spawn_network_termination(assignment_id: String, sink: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(NETWORK_TERMINATION_GRACE_SECS)).await;
        if store::remove(&assignment_id).is_some() {
            let event = notifications::status_changed_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                &assignment_id,
                "UNAVAILABLE",
                Some("NETWORK_TERMINATED"),
            );
            notifications::spawn_delivery(sink, event, store::take_credential(&assignment_id));
        }
    });
}

/// `GET /qos-provisioning/v0.3/qos-assignments/{assignmentId}`.
async fn get_qos_assignment_by_id(
    claims: Claims,
    headers: HeaderMap,
    Path(assignment_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&assignment_id) {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No assignment found for the provided assignmentId.")
                .into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /qos-provisioning/v0.3/qos-assignments/{assignmentId}`.
///
/// Revokes a stored assignment. Keyed only on the store state (the `assignmentId`
/// is opaque, so there is no reserved-identifier control plane): an existing
/// assignment is evicted → `204 No Content` (single-use); an unknown or
/// already-revoked id → `404 NOT_FOUND`. The asynchronous `202 Accepted` +
/// `DELETE_REQUESTED` notification form is deferred with `sink` notifications
/// (mirrors QoD's synchronous `deleteSession`).
async fn revoke_qos_assignment(
    claims: Claims,
    headers: HeaderMap,
    Path(assignment_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::remove(&assignment_id) {
        Some(info) => {
            // If the revoked assignment recorded a `sink`, notify it that the
            // assignment is now UNAVAILABLE with `statusInfo: DELETE_REQUESTED`.
            // Fire-and-forget, off the request path (see `notifications`), so a
            // slow or unreachable sink never delays this 204. The ACCESSTOKEN
            // `sinkCredential` bearer (if any) is taken single-use.
            if let Some(sink) = info.get("sink").and_then(Value::as_str) {
                let event = notifications::status_changed_event(
                    store::new_event_id(),
                    rfc3339_utc(now_unix_secs()),
                    &assignment_id,
                    "UNAVAILABLE",
                    Some("DELETE_REQUESTED"),
                );
                notifications::spawn_delivery(
                    sink.to_string(),
                    event,
                    store::take_credential(&assignment_id),
                );
            }
            with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No assignment found for the provided assignmentId.")
                .into_response(),
            &correlator,
        ),
    }
}

/// `RetrieveAssignmentByDevice` request body (CAMARA 0.3.0): an optional
/// `device`, required only for a two-legged token (in three-legged auth the
/// device is identified by the token subject, so the body may be `{}` / empty).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveByDevice {
    device: Option<Device>,
}

/// `POST /qos-provisioning/v0.3/retrieve-qos-assignment`.
///
/// Returns the QoS assignment currently provisioned for a device, if any (CAMARA
/// uses `POST` rather than `GET` because the `device` may carry PII). The device
/// is resolved by the same two-legged / three-legged rule as `createQosAssignment`
/// (a `RetrieveAssignmentByDevice` carrying an optional `device`, else the token
/// subject). Two control planes (docs/DESIGN.md §7): the resolved identifier's
/// reserved error suffix → the canonical CAMARA error (mirroring QoD's
/// `retrieveSessionsByDevice`); otherwise the in-memory store — the device's
/// stored `AssignmentInfo` (`200`) or `404 NOT_FOUND` when the device has none.
async fn retrieve_qos_assignment_by_device(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_BY_DEVICE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Every field is optional, so an empty body is a valid `{}` (three-legged
    // auth need not send one). Parse strictly when a body is present.
    let req: RetrieveByDevice = if body.is_empty() {
        RetrieveByDevice { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrieveAssignmentByDevice.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the identifier, enforcing the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (mirroring QoD's retrieve-by-device — the shared convention exposes the
    // error set from the input alone).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // The store is the remaining control plane: the device's stored assignment,
    // else 404 when the device has none.
    match resolved.echo.and_then(|echo| store::find_by_device(&echo)) {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No assignment found for the provided device.").into_response(),
            &correlator,
        ),
    }
}

/// Render the `AssignmentInfo` for a created assignment.
fn build_assignment_info(
    assignment_id: &str,
    device_echo: Option<Value>,
    qos_profile: &str,
    sink: Option<String>,
    identifier: &str,
) -> Value {
    let mut info = json!({
        "assignmentId": assignment_id,
        "qosProfile": qos_profile,
    });

    // Grant state from the identifier's trailing three digits. Unlike QoD there
    // is no `duration`/`expiresAt` — provisioning is open-ended until revoked.
    let available = !matches!(scenarios::trailing_three_digits(identifier), Some(0) | None);
    if available {
        info["status"] = json!("AVAILABLE");
        info["startedAt"] = json!(rfc3339_utc(now_unix_secs()));
    } else {
        // A pending grant: no start time yet (CAMARA omits it when REQUESTED).
        info["status"] = json!("REQUESTED");
    }

    if let Some(echo) = device_echo {
        info["device"] = echo;
    }
    if let Some(s) = sink {
        info["sink"] = json!(s);
    }
    info
}

/// A resolved request identifier: the string CamaraSim keys functional cases off,
/// plus the single-property `device` object to echo in the response (`None` when
/// the identifier came from a token subject that is not a phone number).
struct Resolved {
    id: String,
    echo: Option<Value>,
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Connected Network Type). See the module docs for the cases.
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
                Ok(Resolved {
                    id: subject.to_string(),
                    echo: Some(echo),
                })
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

/// The first present identifier of a `Device`, in precedence order, or `None`
/// when the device carries no identifier at all (`minProperties: 1` violated).
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

/// Whether `s` is a well-formed callback sink URL — `http://…` or `https://…`
/// with a non-empty authority. CAMARA's schema requires `https://`
/// (`^https:\/\/.+$`); CamaraSim also accepts `http://` so a loopback receiver can
/// observe callbacks, delivering only to `http://` (no TLS client — a documented
/// cut, mirroring QoD). Any other scheme → `INVALID_SINK`.
fn is_valid_sink(s: &str) -> bool {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty())
}

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`:
/// a leading `+`, then 5–15 digits, the first of which is non-zero.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
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

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs no
/// date/time dependency (mirrors `quality_on_demand::v1`).
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

    const HOST: &str = "qosprov.local:8080";
    const ASSIGNMENTS: &str = "/qos-provisioning/v0.3/qos-assignments";
    const RETRIEVE: &str = "/qos-provisioning/v0.3/retrieve-qos-assignment";

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
        assert!(is_valid_sink("http://example.com/callback")); // accepted for testability
        assert!(!is_valid_sink("https://")); // empty authority
        assert!(!is_valid_sink("http://")); // empty authority
        assert!(!is_valid_sink("ftp://example.com")); // other scheme → INVALID_SINK
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
            network_access_identifier: Some("nai@x".into()),
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(matches!(device_identifier(&d), Some(DeviceId::Nai(_))));
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

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the route's audience. Scope granted verbatim; `client_id` becomes
    /// the token `sub` (used to drive the subject-keyed / three-legged cases).
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
        mint_token_with_client(scope, "qosprov-client").await
    }

    async fn request(
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder
            .body(Body::from(body.unwrap_or("").to_string()))
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    async fn post_assignment(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        request("POST", ASSIGNMENTS, token, Some(body), correlator).await
    }

    async fn get_assignment(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{ASSIGNMENTS}/{id}");
        request("GET", &path, token, None, correlator).await
    }

    async fn delete_assignment(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{ASSIGNMENTS}/{id}");
        request("DELETE", &path, token, None, correlator).await
    }

    async fn retrieve_by_device(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        request("POST", RETRIEVE, token, Some(body), correlator).await
    }

    /// A well-formed CreateAssignment body for a two-legged `device`/`profile`.
    fn create_body(phone: &str, profile: &str) -> String {
        json!({
            "device": { "phoneNumber": phone },
            "qosProfile": profile,
        })
        .to_string()
    }

    #[tokio::test]
    async fn create_available_and_echoes_the_request() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, body) =
            post_assignment(Some(&token), &create_body("+123456789012", "QOS_E"), Some("corr-1"))
                .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "AVAILABLE");
        assert_eq!(body["qosProfile"], "QOS_E");
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
        // UUID-shaped assignmentId, startedAt set for an AVAILABLE assignment,
        // and no expiresAt (provisioning is open-ended).
        assert_eq!(body["assignmentId"].as_str().unwrap().split('-').count(), 5);
        assert!(body["startedAt"].as_str().unwrap().ends_with('Z'));
        assert!(body.get("expiresAt").is_none());
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-1");
    }

    #[tokio::test]
    async fn triple_zero_identifier_yields_a_requested_assignment_without_start_time() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_assignment(Some(&token), &create_body("+123456789000", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "REQUESTED");
        assert!(body.get("startedAt").is_none());
    }

    #[tokio::test]
    async fn create_then_get_reads_the_same_assignment() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_assignment(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["assignmentId"].as_str().unwrap();

        let read = mint_token(READ_SCOPE).await;
        let (status, _, got) = get_assignment(Some(&read), id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(got, created);
    }

    #[tokio::test]
    async fn get_unknown_assignment_is_404() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            get_assignment(Some(&read), "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_the_canonical_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // `…409` → the QoS-Provisioning 409 CONFLICT "existing provisioning" case.
        let (status, _, body) =
            post_assignment(Some(&token), &create_body("+123456789409", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "CONFLICT");
        // `…404` → 404 NOT_FOUND.
        let (status, _, _) =
            post_assignment(Some(&token), &create_body("+123456789404", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unavailable_profile_is_not_applicable_422() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = post_assignment(
            Some(&token),
            &create_body("+123456789012", "profile-unavailable"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "QOS_PROVISIONING.QOS_PROFILE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn missing_qos_profile_is_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({ "device": { "phoneNumber": "+123456789012" } }).to_string();
        let (status, _, err) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_http_scheme_sink_is_400_invalid_sink() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789012" },
            "qosProfile": "QOS_E",
            "sink": "ftp://not-a-callback.example/x",
        })
        .to_string();
        let (status, _, err) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_SINK");
    }

    #[tokio::test]
    async fn http_sink_is_accepted_and_echoed() {
        // CamaraSim accepts `http://` sinks (for a loopback receiver) as well as
        // `https://`, delivering only to `http://` (no TLS client).
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789012" },
            "qosProfile": "QOS_E",
            "sink": "http://receiver.example/callback",
        })
        .to_string();
        let (status, _, info) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(info["sink"], "http://receiver.example/callback");
    }

    #[tokio::test]
    async fn https_sink_is_accepted_and_echoed() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789012" },
            "qosProfile": "QOS_E",
            "sink": "https://example.com/callback",
        })
        .to_string();
        let (status, _, info) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(info["sink"], "https://example.com/callback");
    }

    #[tokio::test]
    async fn device_with_no_identifier_is_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({ "device": {}, "qosProfile": "QOS_E" }).to_string();
        let (status, _, err) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn three_legged_keys_off_the_subject_and_echoes_it() {
        // A three-legged (line) token: the client id is an E.164 number, so the
        // subject identifies the device. No `device` in the body.
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let body = json!({ "qosProfile": "QOS_E" }).to_string();
        let (status, _, info) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(info["status"], "AVAILABLE");
        assert_eq!(info["device"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn device_on_a_line_token_is_422_unnecessary_identifier() {
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let (status, _, err) =
            post_assignment(Some(&token), &create_body("+123456789034", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_422_missing_identifier() {
        let token = mint_token(CREATE_SCOPE).await; // subject "qosprov-client" is not a line
        let body = json!({ "qosProfile": "QOS_E" }).to_string();
        let (status, _, err) = post_assignment(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn create_requires_the_create_scope() {
        // A token with only the read scope may not create.
        let token = mint_token(READ_SCOPE).await;
        let (status, _, err) =
            post_assignment(Some(&token), &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_401() {
        let (status, _, _) =
            post_assignment(None, &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- revokeQosAssignment (DELETE) --------------------------------------

    #[tokio::test]
    async fn revoke_evicts_the_assignment_204_then_get_is_404() {
        // Create an assignment, then revoke it: 204, and it is gone afterwards.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_assignment(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["assignmentId"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, _) = delete_assignment(Some(&del), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // The correlator is echoed even on the empty 204.
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-del");

        // The assignment no longer reads back.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = get_assignment(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn revoke_is_single_use_second_delete_is_404() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_assignment(Some(&create), &create_body("+123456789012", "QOS_E"), None).await;
        let id = created["assignmentId"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_assignment(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // A second revoke of the same id finds nothing → 404.
        let (status, _, body) = delete_assignment(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn revoke_unknown_assignment_is_404() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) =
            delete_assignment(Some(&del), "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn revoke_requires_the_delete_scope() {
        // A token with only the read scope may not revoke → 403.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, err) =
            delete_assignment(Some(&read), "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn revoke_without_a_token_is_401() {
        let (status, _, _) =
            delete_assignment(None, "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- CloudEvents notifications on `sink` (DELETE_REQUESTED) --------------

    #[tokio::test]
    async fn revoking_an_assignment_with_a_sink_fires_a_delete_requested_cloudevent() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        // A loopback receiver stands in for the consumer's `sink`.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-notify");

        // Create an assignment that records the sink. A REQUESTED (`…000`)
        // identifier is used so no AVAILABLE-on-provisioning event fires first —
        // this test isolates the DELETE_REQUESTED transition (the AVAILABLE event
        // and the revoke of an AVAILABLE assignment are covered separately below).
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789000" },
            "qosProfile": "QOS_E",
            "sink": sink,
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "REQUESTED");
        let assignment_id = created["assignmentId"].as_str().unwrap().to_string();

        // …then revoke it: 204 to the caller, and a CloudEvent to the sink.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_assignment(Some(&del), &assignment_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // Receive the fire-and-forget notification the handler spawned.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /qosprov-notify HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        // No sinkCredential → unauthenticated.
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(
            event["type"],
            "org.camaraproject.qos-provisioning.v0.status-changed"
        );
        assert_eq!(event["specversion"], "1.0");
        assert_eq!(event["datacontenttype"], "application/json");
        assert!(event["id"].is_string() && event["time"].is_string());
        assert_eq!(event["data"]["assignmentId"], json!(assignment_id));
        assert_eq!(event["data"]["status"], "UNAVAILABLE");
        assert_eq!(event["data"]["statusInfo"], "DELETE_REQUESTED");
    }

    #[tokio::test]
    async fn a_sink_credential_authenticates_the_callback_and_is_never_echoed() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-auth");

        // Create with a sink AND an ACCESSTOKEN sinkCredential. A REQUESTED
        // (`…000`) identifier isolates the DELETE_REQUESTED callback (no AVAILABLE
        // event fires first).
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789000" },
            "qosProfile": "QOS_E",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "ACCESSTOKEN",
                "accessToken": "sink-secret-123",
                "accessTokenType": "bearer",
            },
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed in the AssignmentInfo.
        assert!(created.get("sinkCredential").is_none());
        let assignment_id = created["assignmentId"].as_str().unwrap().to_string();

        // Revoke → the DELETE_REQUESTED callback carries the bearer.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_assignment(Some(&del), &assignment_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, _) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Bearer sink-secret-123\r\n"),
            "authorization header present: {head}"
        );
    }

    #[tokio::test]
    async fn a_plain_sink_credential_authenticates_the_callback_as_basic() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-plain");

        // Create with a sink AND a PLAIN sinkCredential. A REQUESTED (`…000`)
        // identifier isolates the DELETE_REQUESTED callback (no AVAILABLE event first).
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789000" },
            "qosProfile": "QOS_E",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "PLAIN",
                "identifier": "cbid",
                "secret": "cbsecret",
            },
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed in the AssignmentInfo.
        assert!(created.get("sinkCredential").is_none());
        let assignment_id = created["assignmentId"].as_str().unwrap().to_string();

        // Revoke → the DELETE_REQUESTED callback carries a Basic header.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_assignment(Some(&del), &assignment_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, _) = raw.split_once("\r\n\r\n").expect("headers then body");
        // base64("cbid:cbsecret") == "Y2JpZDpjYnNlY3JldA==".
        assert!(
            head.contains("Authorization: Basic Y2JpZDpjYnNlY3JldA==\r\n"),
            "basic authorization header present: {head}"
        );
    }

    // --- CloudEvents notifications on `sink` (AVAILABLE / NETWORK_TERMINATED) -

    /// Accept one fire-and-forget notification connection and parse the CloudEvent,
    /// returning the raw request head and the parsed JSON body.
    async fn read_one_event(listener: &tokio::net::TcpListener) -> (String, Value) {
        use tokio::io::AsyncReadExt;
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        (
            head.to_string(),
            serde_json::from_str(body).expect("body is JSON"),
        )
    }

    #[tokio::test]
    async fn creating_an_available_assignment_with_a_sink_fires_an_available_cloudevent() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-available");

        // …612 → AVAILABLE (not …001, not a reserved suffix).
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789612" },
            "qosProfile": "QOS_E",
            "sink": sink,
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "AVAILABLE");
        let assignment_id = created["assignmentId"].as_str().unwrap().to_string();

        // The provisioning-active notification arrives on the sink, fire-and-forget.
        let (head, event) = read_one_event(&listener).await;
        assert!(
            head.starts_with("POST /qosprov-available HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        assert_eq!(
            event["type"],
            "org.camaraproject.qos-provisioning.v0.status-changed"
        );
        assert_eq!(event["data"]["assignmentId"], json!(assignment_id));
        assert_eq!(event["data"]["status"], "AVAILABLE");
        assert!(
            event["data"].get("statusInfo").is_none(),
            "an AVAILABLE transition carries no statusInfo"
        );
    }

    #[tokio::test]
    async fn a_001_assignment_with_a_sink_fires_network_terminated_early() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-netterm");

        // …001 → the NETWORK_TERMINATED tail: created AVAILABLE, then dropped early.
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789001" },
            "qosProfile": "QOS_E",
            "sink": sink,
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "AVAILABLE");
        let assignment_id = created["assignmentId"].as_str().unwrap().to_string();

        // The one and only event delivered is the terminal NETWORK_TERMINATED (no
        // AVAILABLE event fires for the …001 tail).
        let (_head, event) = read_one_event(&listener).await;
        assert_eq!(event["data"]["assignmentId"], json!(assignment_id));
        assert_eq!(event["data"]["status"], "UNAVAILABLE");
        assert_eq!(event["data"]["statusInfo"], "NETWORK_TERMINATED");

        // The termination evicted the assignment — a later GET is 404.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_assignment(Some(&read), &assignment_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_available_event_is_authenticated_and_a_later_revoke_is_too() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-avail-auth");

        // …613 → AVAILABLE, with an ACCESSTOKEN sinkCredential.
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789613" },
            "qosProfile": "QOS_E",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "ACCESSTOKEN",
                "accessToken": "avail-secret-9",
                "accessTokenType": "bearer",
            },
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let assignment_id = created["assignmentId"].as_str().unwrap().to_string();

        // The AVAILABLE creation event carries the bearer…
        let (head1, ev1) = read_one_event(&listener).await;
        assert_eq!(ev1["data"]["status"], "AVAILABLE");
        assert!(
            head1.contains("Authorization: Bearer avail-secret-9\r\n"),
            "auth on the AVAILABLE event: {head1}"
        );

        // …and because it only *peeked* the credential, a later revoke's
        // DELETE_REQUESTED callback is still authenticated (single-use take there).
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_assignment(Some(&del), &assignment_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (head2, ev2) = read_one_event(&listener).await;
        assert_eq!(ev2["data"]["status"], "UNAVAILABLE");
        assert_eq!(ev2["data"]["statusInfo"], "DELETE_REQUESTED");
        assert!(
            head2.contains("Authorization: Bearer avail-secret-9\r\n"),
            "auth on the DELETE_REQUESTED event: {head2}"
        );
    }

    #[tokio::test]
    async fn a_requested_assignment_fires_no_event() {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qosprov-requested");

        // …000 → REQUESTED: not yet active, so no AVAILABLE event is delivered.
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456780000" },
            "qosProfile": "QOS_E",
            "sink": sink,
        })
        .to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "REQUESTED");

        // Nothing ever connects to the sink within a generous window.
        let accepted =
            tokio::time::timeout(Duration::from_millis(500), listener.accept()).await;
        assert!(
            accepted.is_err(),
            "a REQUESTED assignment must not notify the sink"
        );
    }

    // --- getQosAssignmentByDevice (POST /retrieve-qos-assignment) -----------
    //
    // Each test uses a device number unique to itself so the process-global
    // store — shared across the whole test binary — returns exactly the
    // assignment that test created (find_by_device returns the single match).

    #[tokio::test]
    async fn create_then_retrieve_by_device_returns_the_assignment() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_assignment(Some(&create), &create_body("+123456789521", "QOS_E"), None).await;
        assert_eq!(status, StatusCode::CREATED);

        let read = mint_token(READ_BY_DEVICE_SCOPE).await;
        let body = json!({ "device": { "phoneNumber": "+123456789521" } }).to_string();
        let (status, headers, got) = retrieve_by_device(Some(&read), &body, Some("corr-r")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(got, created);
        // The correlator is echoed on the 200.
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-r");
    }

    #[tokio::test]
    async fn retrieve_by_device_with_no_assignment_is_404() {
        // A number never provisioned → the device has no assignment.
        let read = mint_token(READ_BY_DEVICE_SCOPE).await;
        let body = json!({ "device": { "phoneNumber": "+123456789522" } }).to_string();
        let (status, _, err) = retrieve_by_device(Some(&read), &body, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn retrieve_by_device_reserved_suffix_selects_the_canonical_error() {
        // The reserved-error plane is checked before the store (mirrors QoD):
        // `…409` → 409 CONFLICT, `…404` → 404 NOT_FOUND.
        let read = mint_token(READ_BY_DEVICE_SCOPE).await;
        let body = json!({ "device": { "phoneNumber": "+123456789409" } }).to_string();
        let (status, _, err) = retrieve_by_device(Some(&read), &body, None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(err["code"], "CONFLICT");

        let body = json!({ "device": { "phoneNumber": "+123456789404" } }).to_string();
        let (status, _, _) = retrieve_by_device(Some(&read), &body, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn retrieve_by_device_three_legged_keys_off_the_subject() {
        // Create via a line token: the subject identifies the device.
        let create = mint_token_with_client(CREATE_SCOPE, "+123456789531").await;
        let body = json!({ "qosProfile": "QOS_E" }).to_string();
        let (status, _, created) = post_assignment(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);

        // Retrieve with the same line token and no device body (empty body is `{}`).
        let read = mint_token_with_client(READ_BY_DEVICE_SCOPE, "+123456789531").await;
        let (status, _, got) = retrieve_by_device(Some(&read), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(got, created);
    }

    #[tokio::test]
    async fn retrieve_by_device_on_a_line_token_is_422_unnecessary_identifier() {
        let read = mint_token_with_client(READ_BY_DEVICE_SCOPE, "+123456789012").await;
        let body = json!({ "device": { "phoneNumber": "+123456789034" } }).to_string();
        let (status, _, err) = retrieve_by_device(Some(&read), &body, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn retrieve_by_device_no_device_and_non_line_subject_is_422_missing_identifier() {
        // Subject "qosprov-client" is not a line and no device is supplied.
        let read = mint_token(READ_BY_DEVICE_SCOPE).await;
        let (status, _, err) = retrieve_by_device(Some(&read), "", None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn retrieve_by_device_requires_the_read_by_device_scope() {
        // The plain read (by-id) scope does not grant retrieve-by-device → 403.
        let token = mint_token(READ_SCOPE).await;
        let body = json!({ "device": { "phoneNumber": "+123456789521" } }).to_string();
        let (status, _, err) = retrieve_by_device(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_by_device_without_a_token_is_401() {
        let body = json!({ "device": { "phoneNumber": "+123456789521" } }).to_string();
        let (status, _, _) = retrieve_by_device(None, &body, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
