//! Quality on Demand **v1** (CAMARA quality-on-demand 1.1.0, release r3.2).
//!
//! First slice of the QoD session lifecycle:
//! - `POST /quality-on-demand/v1/sessions` — create a QoS session and return its
//!   `SessionInfo` (operationId `createSession`, scope
//!   `quality-on-demand:sessions:create`).
//! - `GET /quality-on-demand/v1/sessions/{sessionId}` — read back a session by id
//!   (operationId `getSession`, scope `quality-on-demand:sessions:read`).
//! - `DELETE /quality-on-demand/v1/sessions/{sessionId}` — delete a session by id
//!   (operationId `deleteSession`, scope `quality-on-demand:sessions:delete`).
//! - `POST /quality-on-demand/v1/sessions/{sessionId}/extend` — extend a session's
//!   duration (operationId `extendQosSession`, scope
//!   `quality-on-demand:sessions:update`).
//! - `POST /quality-on-demand/v1/retrieve-sessions` — list a device's active
//!   sessions (operationId `retrieveSessionsByDevice`, scope
//!   `quality-on-demand:sessions:retrieve-by-device`).
//!
//! CloudEvents notifications on `sink` have begun: a `deleteSession` on a session
//! that was created with a `sink` fires a `qos-status-changed` CloudEvent
//! (`qosStatus: UNAVAILABLE`, `statusInfo: DELETE_REQUESTED`) to that sink,
//! best-effort and fire-and-forget (see [`super::notifications`]). The other
//! status transitions (`DURATION_EXPIRED`, `NETWORK_TERMINATED`) are still
//! deferred; `sinkCredential` is accepted for schema fidelity but not used.
//!
//! ## What it does
//!
//! `createSession` mints an opaque, UUID-shaped `sessionId` ([`super::store`]),
//! renders the `SessionInfo` for the request, remembers it, and returns `201`;
//! `getSession` returns the stored `SessionInfo` (`200`) or `404 NOT_FOUND`;
//! `deleteSession` evicts the session from the store and returns `204 No Content`,
//! or `404 NOT_FOUND` when no session exists for the id; `extendQosSession` bumps a
//! stored session's `duration` (and, for an `AVAILABLE` session, its `expiresAt`) by
//! the requested seconds in place and returns the updated `SessionInfo` (`200`), or
//! `404 NOT_FOUND` for an unknown id.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! `createSession` reads three control planes:
//!
//! 1. **The identifier** — the submitted `device` identifier (phoneNumber, else
//!    networkAccessIdentifier, else the IPv4 `publicAddress`, else ipv6Address)
//!    or, when no `device` is supplied, the access token subject. If its trailing
//!    three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!    `…409`, `…422`, `…429`, `…500`, `…503`) the endpoint answers with that
//!    canonical CAMARA error ([`crate::scenarios`]) — so e.g. `…409` exercises the
//!    QoD `409 CONFLICT` (duplicate-session) case. Otherwise the trailing digits
//!    pick the grant state: `…000` (and an identifier with no digits) →
//!    `qosStatus: REQUESTED` (no `startedAt`/`expiresAt` yet); any other tail →
//!    `qosStatus: AVAILABLE`, granted from now for `duration` seconds.
//! 2. **`duration`** — seconds the session is requested for. `< 1` → `400
//!    INVALID_ARGUMENT`; `> `[`MAX_DURATION_SECS`] → `400`
//!    `QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE` (the simulator's fixed QoS-profile
//!    ceiling); otherwise it sets `expiresAt = startedAt + duration`.
//! 3. **`qosProfile`** — the requested profile name. A profile whose name (case-
//!    insensitively) contains `unavailable` → `422`
//!    `QUALITY_ON_DEMAND.QOS_PROFILE_NOT_APPLICABLE`; any other well-formed name is
//!    accepted.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{notifications, store};
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create a session (CAMARA quality-on-demand 1.1.0).
const CREATE_SCOPE: &str = "quality-on-demand:sessions:create";
/// Scope required to read a session (CAMARA quality-on-demand 1.1.0).
const READ_SCOPE: &str = "quality-on-demand:sessions:read";
/// Scope required to delete a session (CAMARA quality-on-demand 1.1.0).
const DELETE_SCOPE: &str = "quality-on-demand:sessions:delete";
/// Scope required to extend a session (CAMARA quality-on-demand 1.1.0).
const UPDATE_SCOPE: &str = "quality-on-demand:sessions:update";
/// Scope required to list a device's sessions (CAMARA quality-on-demand 1.1.0).
const RETRIEVE_SCOPE: &str = "quality-on-demand:sessions:retrieve-by-device";

/// The simulator's fixed QoS-profile maximum duration, in seconds (24 h). A
/// requested `duration` beyond this is out of range for the (single, simulated)
/// profile → `QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE`.
const MAX_DURATION_SECS: i64 = 86_400;

/// Routes for Quality on Demand v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/quality-on-demand/v1/sessions", post(create_session))
        .route(
            "/quality-on-demand/v1/sessions/:session_id",
            get(get_session).delete(delete_session),
        )
        .route(
            "/quality-on-demand/v1/sessions/:session_id/extend",
            post(extend_session),
        )
        .route(
            "/quality-on-demand/v1/retrieve-sessions",
            post(retrieve_sessions),
        )
}

/// `CreateSession` request body (CAMARA 1.1.0). `applicationServer`, `qosProfile`
/// and `duration` are required; `device` is required only for a two-legged token.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSession {
    device: Option<Device>,
    #[serde(rename = "applicationServer")]
    application_server: Option<ApplicationServer>,
    #[serde(rename = "qosProfile")]
    qos_profile: Option<String>,
    duration: Option<i64>,
    // Accepted for schema fidelity and echoed back when present.
    #[serde(rename = "devicePorts")]
    device_ports: Option<Value>,
    #[serde(rename = "applicationServerPorts")]
    application_server_ports: Option<Value>,
    sink: Option<String>,
    // Accepted but never echoed (it carries a credential) and not yet used —
    // CloudEvents notifications are deferred.
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
    sink_credential: Option<Value>,
}

/// The CAMARA `ApplicationServer`: at least one of `ipv4Address` / `ipv6Address`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationServer {
    #[serde(rename = "ipv4Address")]
    ipv4_address: Option<String>,
    #[serde(rename = "ipv6Address")]
    ipv6_address: Option<String>,
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

/// `POST /quality-on-demand/v1/sessions`.
async fn create_session(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (three required fields); parse strictly.
    let req: CreateSession = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid CreateSession.", &correlator)
        }
    };

    // Required fields.
    let application_server = match req.application_server {
        Some(a) if a.ipv4_address.is_some() || a.ipv6_address.is_some() => a,
        Some(_) => {
            return invalid_argument(
                "`applicationServer` must contain `ipv4Address` and/or `ipv6Address`.",
                &correlator,
            )
        }
        None => return invalid_argument("`applicationServer` is required.", &correlator),
    };
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
    let duration = match req.duration {
        Some(d) if d < 1 => {
            return invalid_argument("`duration` must be at least 1 second.", &correlator)
        }
        Some(d) if d > MAX_DURATION_SECS => return duration_out_of_range(&correlator),
        Some(d) => d,
        None => return invalid_argument("`duration` is required.", &correlator),
    };

    // Resolve the identifier: the submitted device identifier, else the token
    // subject (three-legged fallback). Missing both → 422 MISSING_IDENTIFIER.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (incl. `…409` → the QoD 409 CONFLICT duplicate-session case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // A profile whose name marks it unavailable is not applicable (422).
    if qos_profile.to_ascii_lowercase().contains("unavailable") {
        return with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "QUALITY_ON_DEMAND.QOS_PROFILE_NOT_APPLICABLE",
                "The requested QoS profile is not applicable for this session.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Build the SessionInfo, remember it, and return 201.
    let session_id = store::new_session_id();
    let info = build_session_info(
        &session_id,
        resolved.echo,
        &application_server,
        &qos_profile,
        duration,
        req.device_ports,
        req.application_server_ports,
        req.sink,
        &resolved.id,
    );
    store::insert(session_id, info.clone());

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// `GET /quality-on-demand/v1/sessions/{sessionId}`.
async fn get_session(claims: Claims, headers: HeaderMap, Path(session_id): Path<String>) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&session_id) {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No session found for the provided sessionId.").into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /quality-on-demand/v1/sessions/{sessionId}`.
///
/// Deletes the session, releasing its QoS grant. Keyed only on the stored state:
/// a session that exists is evicted → `204 No Content`; an unknown (or already
/// deleted) id → `404 NOT_FOUND`. No CloudEvents `DELETE_REQUESTED` notification
/// is emitted — notifications are deferred (see the module docs).
async fn delete_session(
    claims: Claims,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::remove(&session_id) {
        Some(info) => {
            // Notify the session's `sink` (if any) that it is now UNAVAILABLE
            // because the consumer requested deletion. Fire-and-forget so a slow
            // or unreachable sink never delays this response (see `notifications`).
            if let Some(sink) = info.get("sink").and_then(Value::as_str) {
                let event = notifications::qos_status_changed_event(
                    store::new_event_id(),
                    rfc3339_utc(now_unix_secs()),
                    &session_id,
                    "UNAVAILABLE",
                    Some("DELETE_REQUESTED"),
                );
                notifications::spawn_delivery(sink.to_string(), event);
            }
            with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No session found for the provided sessionId.").into_response(),
            &correlator,
        ),
    }
}

/// `ExtendSessionDuration` request body (CAMARA 1.1.0): the extra seconds to add
/// to the session's granted duration.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtendSessionDuration {
    #[serde(rename = "requestedAdditionalDuration")]
    requested_additional_duration: Option<i64>,
}

/// `POST /quality-on-demand/v1/sessions/{sessionId}/extend`.
///
/// Extends the duration of an existing session by `requestedAdditionalDuration`
/// seconds and returns the updated `SessionInfo` (`200`). Two control planes
/// (docs/DESIGN.md §7): the stored session (unknown/deleted id → `404 NOT_FOUND`)
/// and the requested seconds — `< 1` → `400 INVALID_ARGUMENT`, greater than the
/// profile ceiling of [`MAX_DURATION_SECS`], **or** a new total duration beyond
/// that ceiling → `400 QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE` (so the ceiling
/// case depends on the session's current duration — a genuine stateful control
/// plane). The update is written back to the store, so a later `getSession`
/// reflects the longer duration; for an `AVAILABLE` session `expiresAt` is pushed
/// forward by the same amount.
async fn extend_session(
    claims: Claims,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(UPDATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: ExtendSessionDuration = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid ExtendSessionDuration.",
                &correlator,
            )
        }
    };
    let additional = match req.requested_additional_duration {
        Some(d) if d < 1 => {
            return invalid_argument(
                "`requestedAdditionalDuration` must be at least 1 second.",
                &correlator,
            )
        }
        Some(d) if d > MAX_DURATION_SECS => return duration_out_of_range(&correlator),
        Some(d) => d,
        None => return invalid_argument("`requestedAdditionalDuration` is required.", &correlator),
    };

    // The session must exist to be extended, and the resulting total duration
    // must stay within the profile ceiling (a state-dependent case).
    let current = match store::get(&session_id) {
        Some(info) => info,
        None => return session_not_found(&correlator),
    };
    let current_duration = current["duration"].as_i64().unwrap_or(0);
    if current_duration + additional > MAX_DURATION_SECS {
        return duration_out_of_range(&correlator);
    }

    // Apply the extension in place. A concurrent delete between the read above
    // and this update leaves nothing to extend → 404.
    let updated = store::update(&session_id, |info| {
        let new_duration = info["duration"].as_i64().unwrap_or(0) + additional;
        info["duration"] = json!(new_duration);
        // An AVAILABLE session has a concrete expiry; push it out by `additional`.
        if info["qosStatus"] == "AVAILABLE" {
            if let Some(exp) = info["expiresAt"].as_str().and_then(parse_rfc3339_utc) {
                info["expiresAt"] = json!(rfc3339_utc(exp + additional));
            }
        }
    });
    match updated {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => session_not_found(&correlator),
    }
}

/// `RetrieveSessionsInput` request body (CAMARA 1.1.0): the device whose active
/// sessions to list. `device` is optional — omitted for a three-legged token,
/// where the device is taken from the token subject.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveSessionsInput {
    device: Option<Device>,
}

/// `POST /quality-on-demand/v1/retrieve-sessions`.
///
/// Lists the active QoS sessions for a device as an array of `SessionInfo`
/// (`200`; an empty array when the device has none — CAMARA never 404s on an
/// empty result). The device is the submitted `device` identifier, else the
/// token subject (three-legged fallback; neither present → `422
/// MISSING_IDENTIFIER`). Two control planes (docs/DESIGN.md §7): the identifier —
/// a reserved error suffix selects a canonical CAMARA error (so `…404` → `404
/// NOT_FOUND` for an unknown device) — and, on the happy path, the in-memory
/// store, which is matched by each session's echoed `device`. A resolved
/// identifier with no `device` echo (a non-E.164 token subject and no submitted
/// device) matches nothing → `200 []`.
async fn retrieve_sessions(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `device` is optional, so an empty body is accepted as `{}` (three-legged:
    // the device comes from the token subject). A non-empty body must be valid.
    let req: RetrieveSessionsInput = if body.is_empty() {
        RetrieveSessionsInput { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrieveSessionsInput.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the identifier (submitted device, else token subject).
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (…404 → 404 NOT_FOUND, the device-identifier-not-found case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Match stored sessions by their echoed `device`. A resolved identifier with
    // no device echo (a non-E.164 subject, no submitted device) matches nothing.
    let sessions = match resolved.echo {
        Some(echo) => store::find_by_device(&echo),
        None => Vec::new(),
    };

    with_correlator(
        (StatusCode::OK, Json(sessions)).into_response(),
        &correlator,
    )
}

/// Render the `SessionInfo` for a freshly created session. The trailing three
/// digits of the identifier pick the grant state: `…000` / no digits →
/// `REQUESTED` (no `startedAt`/`expiresAt`), any other tail → `AVAILABLE` for
/// `duration` seconds from now.
#[allow(clippy::too_many_arguments)]
fn build_session_info(
    session_id: &str,
    device_echo: Option<Value>,
    application_server: &ApplicationServer,
    qos_profile: &str,
    duration: i64,
    device_ports: Option<Value>,
    application_server_ports: Option<Value>,
    sink: Option<String>,
    identifier: &str,
) -> Value {
    let mut info = json!({
        "sessionId": session_id,
        "qosProfile": qos_profile,
        "duration": duration,
        "applicationServer": application_server_json(application_server),
    });

    // Grant state from the identifier's trailing three digits.
    let available = !matches!(scenarios::trailing_three_digits(identifier), Some(0) | None);
    if available {
        let started = now_unix_secs();
        info["qosStatus"] = json!("AVAILABLE");
        info["startedAt"] = json!(rfc3339_utc(started));
        info["expiresAt"] = json!(rfc3339_utc(started + duration));
    } else {
        // A pending grant: no start/expiry yet (CAMARA omits them when REQUESTED).
        info["qosStatus"] = json!("REQUESTED");
    }

    if let Some(echo) = device_echo {
        info["device"] = echo;
    }
    if let Some(dp) = device_ports {
        info["devicePorts"] = dp;
    }
    if let Some(asp) = application_server_ports {
        info["applicationServerPorts"] = asp;
    }
    if let Some(s) = sink {
        info["sink"] = json!(s);
    }
    info
}

/// The `applicationServer` echo, carrying only the addresses that were supplied.
fn application_server_json(server: &ApplicationServer) -> Value {
    let mut v = json!({});
    if let Some(ip4) = &server.ipv4_address {
        v["ipv4Address"] = json!(ip4);
    }
    if let Some(ip6) = &server.ipv6_address {
        v["ipv6Address"] = json!(ip6);
    }
    v
}

/// A resolved request identifier: the string CamaraSim keys functional cases off,
/// plus the single-property `device` object to echo in the response (`None` when
/// the identifier came from a token subject that is not a phone number).
struct Resolved {
    id: String,
    echo: Option<Value>,
}

/// Resolve the device identifier for a request: the first present identifier in
/// the supplied `device` (validated when it is a `phoneNumber`), else the token
/// subject (three-legged fallback). On failure returns the CAMARA error
/// `Response` to send — 400 `INVALID_ARGUMENT` for a malformed `phoneNumber` or a
/// `device` carrying no identifier, 422 `MISSING_IDENTIFIER` when neither a
/// device nor a token subject is present.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                let echo = json!({ "phoneNumber": phone });
                Ok(Resolved { id: phone, echo: Some(echo) })
            }
            Some(DeviceId::Nai(id)) => {
                let echo = json!({ "networkAccessIdentifier": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv4(id)) => {
                let echo = json!({ "ipv4Address": { "publicAddress": id } });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv6(id)) => {
                let echo = json!({ "ipv6Address": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            let subject = claims.subject().unwrap_or("");
            if subject.is_empty() {
                return Err(with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "No `device` supplied and the access token identifies no device.",
                    )
                    .into_response(),
                    correlator,
                ));
            }
            let echo = is_valid_e164(subject).then(|| json!({ "phoneNumber": subject }));
            Ok(Resolved { id: subject.to_string(), echo })
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

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
}

/// A 400 `QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE` error — the requested (or
/// resulting total) duration exceeds the QoS profile's ceiling. Correlator echoed.
fn duration_out_of_range(correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(
            StatusCode::BAD_REQUEST,
            "QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE",
            "The requested duration is out of the allowed range for the QoS profile.",
        )
        .into_response(),
        correlator,
    )
}

/// A 404 `NOT_FOUND` for an unknown/deleted `sessionId`, with the correlator echoed.
fn session_not_found(correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::not_found("No session found for the provided sessionId.").into_response(),
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
/// date/time dependency (mirrors `sim_swap::v2` / `device_identifier::v0_3`).
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

/// Parse an RFC 3339 UTC instant in exactly the shape [`rfc3339_utc`] produces
/// (`YYYY-MM-DDTHH:MM:SSZ`) back to a Unix timestamp (seconds). Returns `None` for
/// anything else — used only on strings the simulator itself wrote, so the strict
/// shape is sufficient (no offset/fraction handling needed).
fn parse_rfc3339_utc(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':'
        || b[16] != b':' || b[19] != b'Z'
    {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    let hh: i64 = s.get(11..13)?.parse().ok()?;
    let mm: i64 = s.get(14..16)?.parse().ok()?;
    let ss: i64 = s.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// Days since 1970-01-01 for a civil `(year, month, day)` — the inverse of
/// [`civil_from_days`] (Howard Hinnant's `days_from_civil`, proleptic Gregorian).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let m = m as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (d as u64 - 1); // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "qod.local:8080";
    const SESSIONS: &str = "/quality-on-demand/v1/sessions";
    const RETRIEVE_SESSIONS: &str = "/quality-on-demand/v1/retrieve-sessions";
    /// A well-formed CreateSession body for `phone`, `profile`, `duration`.
    fn create_body(phone: &str, profile: &str, duration: i64) -> String {
        json!({
            "device": { "phoneNumber": phone },
            "applicationServer": { "ipv4Address": "203.0.113.0/24" },
            "qosProfile": profile,
            "duration": duration,
        })
        .to_string()
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn qos_profile_validation_follows_the_camara_pattern() {
        assert!(is_valid_qos_profile("QOS_L"));
        assert!(is_valid_qos_profile("low-latency.v1"));
        assert!(!is_valid_qos_profile("ab")); // too short
        assert!(!is_valid_qos_profile("has space"));
        assert!(!is_valid_qos_profile("bad!char"));
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(!is_valid_e164("123456789"));
        assert!(!is_valid_e164("+0234567"));
    }

    #[test]
    fn rfc3339_utc_formats_known_epochs() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn parse_rfc3339_utc_is_the_inverse_of_rfc3339_utc() {
        for epoch in [0, 1_704_067_200, 1_704_070_808, 253_402_300_799] {
            assert_eq!(parse_rfc3339_utc(&rfc3339_utc(epoch)), Some(epoch));
        }
        // Rejects anything not in the exact `YYYY-MM-DDTHH:MM:SSZ` shape.
        assert_eq!(parse_rfc3339_utc("2024-01-01T00:00:00+01:00"), None);
        assert_eq!(parse_rfc3339_utc("2024-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_utc("not-a-date"), None);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the route's audience. Scope granted verbatim.
    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "qod-client").await
    }

    /// As [`mint_token`], but with a caller-chosen `client_id` — which becomes the
    /// token `sub`. Used to drive the subject-keyed (no-device) cases.
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

    /// POST a JSON body to `/sessions` with an optional Bearer token and correlator.
    async fn post_sessions(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        request("POST", SESSIONS, token, Some(body), correlator).await
    }

    /// GET a session by id with an optional Bearer token and correlator.
    async fn get_session_req(
        token: Option<&str>,
        session_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{SESSIONS}/{session_id}");
        request("GET", &path, token, None, correlator).await
    }

    /// DELETE a session by id with an optional Bearer token and correlator.
    async fn delete_session_req(
        token: Option<&str>,
        session_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{SESSIONS}/{session_id}");
        request("DELETE", &path, token, None, correlator).await
    }

    /// POST an extend body for `session_id` with an optional Bearer token/correlator.
    async fn extend_session_req(
        token: Option<&str>,
        session_id: &str,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{SESSIONS}/{session_id}/extend");
        request("POST", &path, token, Some(body), correlator).await
    }

    /// A well-formed ExtendSessionDuration body for `additional` seconds.
    fn extend_body(additional: i64) -> String {
        json!({ "requestedAdditionalDuration": additional }).to_string()
    }

    /// POST a retrieve-sessions body with an optional Bearer token/correlator.
    async fn retrieve_sessions_req(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        request("POST", RETRIEVE_SESSIONS, token, Some(body), correlator).await
    }

    /// A retrieve-sessions body naming `phone` as the device.
    fn retrieve_body(phone: &str) -> String {
        json!({ "device": { "phoneNumber": phone } }).to_string()
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

    #[tokio::test]
    async fn create_session_available_and_echoes_the_request() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_sessions(Some(&token), &create_body("+123456789012", "QOS_L", 3600), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["qosStatus"], "AVAILABLE");
        assert_eq!(body["qosProfile"], "QOS_L");
        assert_eq!(body["duration"], 3600);
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
        assert_eq!(body["applicationServer"]["ipv4Address"], "203.0.113.0/24");
        // UUID-shaped sessionId, and start/expiry set for an AVAILABLE session.
        assert_eq!(body["sessionId"].as_str().unwrap().split('-').count(), 5);
        assert!(body["startedAt"].as_str().unwrap().ends_with('Z'));
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn triple_zero_identifier_yields_a_requested_session_without_times() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_sessions(Some(&token), &create_body("+123456789000", "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["qosStatus"], "REQUESTED");
        assert!(body.get("startedAt").is_none());
        assert!(body.get("expiresAt").is_none());
    }

    #[tokio::test]
    async fn create_then_get_reads_the_same_session() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 120), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["sessionId"].as_str().unwrap();

        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_session_req(Some(&read), id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched, created);
    }

    #[tokio::test]
    async fn get_unknown_session_is_not_found() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            get_session_req(Some(&read), "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn reserved_identifier_suffix_selects_a_canonical_camara_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // …409 → the QoD 409 CONFLICT (duplicate-session) case via the shared set.
        let (status, _, body) =
            post_sessions(Some(&token), &create_body("+123456789409", "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "CONFLICT");
        // …404 → NOT_FOUND.
        let (status, _, body) =
            post_sessions(Some(&token), &create_body("+123456789404", "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn duration_below_one_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_sessions(Some(&token), &create_body("+123456789012", "QOS_L", 0), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn duration_above_the_maximum_is_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = post_sessions(
            Some(&token),
            &create_body("+123456789012", "QOS_L", MAX_DURATION_SECS + 1),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn unavailable_profile_is_not_applicable() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_sessions(Some(&token), &create_body("+123456789012", "QOS_UNAVAILABLE", 60), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "QUALITY_ON_DEMAND.QOS_PROFILE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn missing_required_fields_and_bad_shapes_are_rejected() {
        let token = mint_token(CREATE_SCOPE).await;
        // Missing qosProfile.
        let b = json!({
            "device": { "phoneNumber": "+123456789012" },
            "applicationServer": { "ipv4Address": "203.0.113.0/24" },
            "duration": 60,
        })
        .to_string();
        let (status, _, body) = post_sessions(Some(&token), &b, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // applicationServer with no address.
        let b = json!({
            "device": { "phoneNumber": "+123456789012" },
            "applicationServer": {},
            "qosProfile": "QOS_L",
            "duration": 60,
        })
        .to_string();
        let (status, _, body) = post_sessions(Some(&token), &b, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Unknown field.
        let b = json!({
            "applicationServer": { "ipv4Address": "203.0.113.0/24" },
            "qosProfile": "QOS_L",
            "duration": 60,
            "x": 1,
        })
        .to_string();
        let (status, _, body) = post_sessions(Some(&token), &b, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bad_phone_number_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let b = json!({
            "device": { "phoneNumber": "0123" },
            "applicationServer": { "ipv4Address": "203.0.113.0/24" },
            "qosProfile": "QOS_L",
            "duration": 60,
        })
        .to_string();
        let (status, _, body) = post_sessions(Some(&token), &b, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject() {
        // Subject is an E.164 number ending …012 → AVAILABLE, no device in body.
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let b = json!({
            "applicationServer": { "ipv6Address": "2001:db8::1" },
            "qosProfile": "QOS_L",
            "duration": 60,
        })
        .to_string();
        let (status, _, body) = post_sessions(Some(&token), &b, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["qosStatus"], "AVAILABLE");
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
        assert_eq!(body["applicationServer"]["ipv6Address"], "2001:db8::1");
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789503").await;
        let b = json!({
            "applicationServer": { "ipv4Address": "203.0.113.0/24" },
            "qosProfile": "QOS_L",
            "duration": 60,
        })
        .to_string();
        let (status, _, body) = post_sessions(Some(&token), &b, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn create_then_delete_returns_204_and_the_session_is_gone() {
        // Create a session…
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 120), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["sessionId"].as_str().unwrap().to_string();

        // …delete it → 204 No Content, with an empty body.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null);

        // …and it is gone: a subsequent GET is 404.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = get_session_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_not_found() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // A second delete of the same id finds nothing.
        let (status, _, body) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_session_is_not_found() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) =
            delete_session_req(Some(&del), "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_requires_the_delete_scope() {
        // A read token must not satisfy the delete scope.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = delete_session_req(Some(&read), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
        // A delete token must not satisfy the read or create scope.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) = get_session_req(Some(&del), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
        let (status, _, body) =
            post_sessions(Some(&del), &create_body("+123456789012", "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn delete_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            delete_session_req(None, "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_delete_204_and_404() {
        // Echoed on the 204 (created-then-deleted)…
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();
        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, _) = delete_session_req(Some(&del), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del")
        );
        // …and on the 404 (unknown id).
        let (status, headers, _) =
            delete_session_req(Some(&del), "no-such-id", Some("corr-del-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-404")
        );
    }

    #[tokio::test]
    async fn deleting_a_session_with_a_sink_fires_a_delete_requested_cloudevent() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        // A loopback receiver stands in for the consumer's `sink`.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/qod-notify");

        // Create a session that records the sink…
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "device": { "phoneNumber": "+123456789012" },
            "applicationServer": { "ipv4Address": "203.0.113.0/24" },
            "qosProfile": "QOS_L",
            "duration": 3600,
            "sink": sink,
        })
        .to_string();
        let (status, _, created) = post_sessions(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let session_id = created["sessionId"].as_str().unwrap().to_string();

        // …then delete it: 204 to the caller, and a CloudEvent to the sink.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &session_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // Receive the fire-and-forget notification the handler spawned.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /qod-notify HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(
            event["type"],
            "org.camaraproject.quality-on-demand.v1.qos-status-changed"
        );
        assert_eq!(event["specversion"], "1.0");
        assert_eq!(event["datacontenttype"], "application/json");
        assert!(event["id"].is_string() && event["time"].is_string());
        assert_eq!(event["data"]["sessionId"], json!(session_id));
        assert_eq!(event["data"]["qosStatus"], "UNAVAILABLE");
        assert_eq!(event["data"]["statusInfo"], "DELETE_REQUESTED");
    }

    #[tokio::test]
    async fn extend_available_session_grows_duration_and_pushes_expiry() {
        // Create an AVAILABLE session (…012) for 3600 s.
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 3600), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();
        let expiry_before = parse_rfc3339_utc(created["expiresAt"].as_str().unwrap()).unwrap();

        // Extend by 60 s → duration grows, expiry moves out by exactly 60 s.
        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) = extend_session_req(Some(&upd), &id, &extend_body(60), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["duration"], 3660);
        assert_eq!(body["qosStatus"], "AVAILABLE");
        let expiry_after = parse_rfc3339_utc(body["expiresAt"].as_str().unwrap()).unwrap();
        assert_eq!(expiry_after - expiry_before, 60);

        // The extension persists: a later GET reflects the longer duration.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_session_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["duration"], 3660);
    }

    #[tokio::test]
    async fn extend_requested_session_grows_duration_without_times() {
        // A …000 identifier creates a REQUESTED session (no startedAt/expiresAt).
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789000", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();

        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) = extend_session_req(Some(&upd), &id, &extend_body(30), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["duration"], 90);
        assert_eq!(body["qosStatus"], "REQUESTED");
        assert!(body.get("expiresAt").is_none());
    }

    #[tokio::test]
    async fn extend_unknown_session_is_not_found() {
        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) = extend_session_req(
            Some(&upd),
            "11111111-1111-4111-8111-111111111111",
            &extend_body(60),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn extend_below_one_is_invalid_argument() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();

        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) = extend_session_req(Some(&upd), &id, &extend_body(0), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn extend_above_the_maximum_is_out_of_range() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();

        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) =
            extend_session_req(Some(&upd), &id, &extend_body(MAX_DURATION_SECS + 1), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn extend_beyond_the_ceiling_is_out_of_range_and_depends_on_current_duration() {
        // Create at the ceiling itself; any positive extension overflows it.
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) = post_sessions(
            Some(&create),
            &create_body("+123456789012", "QOS_L", MAX_DURATION_SECS),
            None,
        )
        .await;
        let id = created["sessionId"].as_str().unwrap().to_string();

        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) = extend_session_req(Some(&upd), &id, &extend_body(1), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn extend_missing_field_or_unknown_field_is_invalid_argument() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();
        let upd = mint_token(UPDATE_SCOPE).await;

        // Missing requestedAdditionalDuration.
        let (status, _, body) = extend_session_req(Some(&upd), &id, "{}", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Unknown field.
        let b = json!({ "requestedAdditionalDuration": 60, "x": 1 }).to_string();
        let (status, _, body) = extend_session_req(Some(&upd), &id, &b, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn extend_requires_the_update_scope() {
        // A read token must not satisfy the update scope.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            extend_session_req(Some(&read), "any-id", &extend_body(60), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
        // An update token must not satisfy the read scope.
        let upd = mint_token(UPDATE_SCOPE).await;
        let (status, _, body) = get_session_req(Some(&upd), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn extend_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            extend_session_req(None, "any-id", &extend_body(60), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_extend_200_and_404() {
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) =
            post_sessions(Some(&create), &create_body("+123456789012", "QOS_L", 60), None).await;
        let id = created["sessionId"].as_str().unwrap().to_string();
        let upd = mint_token(UPDATE_SCOPE).await;
        // Echoed on the 200…
        let (status, headers, _) =
            extend_session_req(Some(&upd), &id, &extend_body(60), Some("corr-ext")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ext")
        );
        // …and on the 404 (unknown id).
        let (status, headers, _) =
            extend_session_req(Some(&upd), "no-such-id", &extend_body(60), Some("corr-ext-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ext-404")
        );
    }

    #[tokio::test]
    async fn retrieve_sessions_returns_only_the_requested_devices_sessions() {
        // Two globally-unique devices so this shares the process-global store
        // with no other test. Create two sessions for A and one for B.
        let create = mint_token(CREATE_SCOPE).await;
        let dev_a = "+19998880012"; // …012 → AVAILABLE, not a reserved suffix
        let dev_b = "+19998883016";
        for _ in 0..2 {
            let (status, _, _) =
                post_sessions(Some(&create), &create_body(dev_a, "QOS_L", 60), None).await;
            assert_eq!(status, StatusCode::CREATED);
        }
        let (status, _, _) =
            post_sessions(Some(&create), &create_body(dev_b, "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::CREATED);

        // Retrieving A's sessions returns exactly the two, each carrying A.
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body(dev_a), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("array of SessionInfo");
        assert_eq!(arr.len(), 2);
        assert!(arr
            .iter()
            .all(|s| s["device"]["phoneNumber"] == dev_a && s["qosProfile"] == "QOS_L"));

        // Retrieving B's sessions returns exactly the one.
        let (status, _, body) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body(dev_b), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn retrieve_sessions_for_a_device_with_none_is_empty_array() {
        // A globally-unique device with no sessions created for it.
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body("+19998881013"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn retrieve_sessions_reserved_suffix_selects_a_camara_error() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        // …404 → 404 NOT_FOUND (device identifier not found).
        let (status, _, body) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body("+19998880404"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …429 → 429 TOO_MANY_REQUESTS.
        let (status, _, body) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body("+19998880429"), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn retrieve_sessions_falls_back_to_the_token_subject() {
        // A three-legged token whose subject is the (globally-unique) device.
        let subject = "+19998884017";
        // Create a session for that device with a create token…
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, _) =
            post_sessions(Some(&create), &create_body(subject, "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::CREATED);

        // …then retrieve with no device in the body — the subject supplies it.
        let retrieve = mint_token_with_client(RETRIEVE_SCOPE, subject).await;
        let (status, _, body) = retrieve_sessions_req(Some(&retrieve), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["device"]["phoneNumber"], subject);
    }

    #[tokio::test]
    async fn retrieve_sessions_with_a_non_e164_subject_and_no_device_is_empty() {
        // The default client_credentials subject is non-E.164, so there is no
        // `device` echo to match → an empty array (not an error).
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) = retrieve_sessions_req(Some(&retrieve), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn retrieve_sessions_rejects_a_bad_body() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        // Unknown field.
        let b = json!({ "device": { "phoneNumber": "+19998880012" }, "x": 1 }).to_string();
        let (status, _, body) = retrieve_sessions_req(Some(&retrieve), &b, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Malformed phoneNumber.
        let (status, _, body) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body("0123"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_sessions_requires_the_retrieve_scope() {
        // A read token must not satisfy the retrieve-by-device scope.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+19998880012"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
        // A retrieve token must not satisfy the read scope.
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, body) = get_session_req(Some(&retrieve), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_sessions_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            retrieve_sessions_req(None, &retrieve_body("+19998880012"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_retrieve_sessions() {
        let retrieve = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) =
            retrieve_sessions_req(Some(&retrieve), &retrieve_body("+19998881013"), Some("corr-ret"))
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ret")
        );
    }

    #[tokio::test]
    async fn create_token_cannot_read_and_read_token_cannot_create() {
        // A create token must not satisfy the read scope.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = get_session_req(Some(&create), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
        // A read token must not satisfy the create scope.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            post_sessions(Some(&read), &create_body("+123456789012", "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_sessions(None, &create_body("+123456789012", "QOS_L", 60), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_create_and_get() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, headers, created) = post_sessions(
            Some(&create),
            &create_body("+123456789012", "QOS_L", 60),
            Some("corr-qod"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-qod")
        );
        let id = created["sessionId"].as_str().unwrap();
        let read = mint_token(READ_SCOPE).await;
        let (status, headers, _) = get_session_req(Some(&read), id, Some("corr-get")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get")
        );
    }
}
