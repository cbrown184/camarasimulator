//! Session Insights **vwip** (CAMARA SessionInsights, work-in-progress).
//!
//! Four endpoints in this slice — the create/read/delete trio of the session
//! resource plus list-by-device:
//! - `POST   /session-insights/vwip/sessions` — create a session (`createSession`).
//! - `GET    /session-insights/vwip/sessions/{sessionId}` — read it (`getSession`).
//! - `DELETE /session-insights/vwip/sessions/{sessionId}` — delete it
//!   (`deleteSession`).
//! - `POST   /session-insights/vwip/retrieve-sessions` — list a device's sessions
//!   (`retrieveSessionsByDevice`).
//!
//! (`POST /sessions/{id}/metrics` and the CloudEvents notifications on `sink` are
//! deferred to later passes.)
//!
//! ## What it does
//!
//! `POST /sessions` registers interest in the network quality of an application
//! session — a device talking to a named application server — and mints an
//! opaque `id`:
//!
//! ```json
//! {
//!   "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
//!   "applicationSessionId": "game-42",
//!   "device": { "phoneNumber": "+123456789012" },
//!   "applicationServer": { "ipv4Address": "198.51.100.1" },
//!   "sink": "https://callback.example/notify",
//!   "startsAt": "2026-08-05T10:41:38Z",
//!   "expiresAt": "2026-08-06T10:41:38Z",
//!   "status": "ACTIVE"
//! }
//! ```
//!
//! - `status` — one of `ACTIVE` | `EXPIRED` | `DELETED`. A freshly created session
//!   is always `ACTIVE` (the adverse states arrive via the lifecycle endpoints /
//!   notifications, which are deferred).
//! - `startsAt` — always present, the moment of creation (RFC 3339 UTC).
//! - `expiresAt` — present unless the session is open-ended (see the control
//!   planes below).
//!
//! `GET /sessions/{sessionId}` returns the stored representation verbatim (`200`),
//! or `404 NOT_FOUND` for an unknown id.
//!
//! Both endpoints are protected: `POST` requires the
//! `session-insights:sessions:create` scope, `GET` the
//! `session-insights:sessions:read` scope
//! ([`crate::auth::verify::Claims`]).
//!
//! ## Identifier
//!
//! The session is keyed by the CAMARA `Device` — the submitted `device`
//! identifier (phoneNumber, else networkAccessIdentifier, else the IPv4
//! `publicAddress`, else ipv6Address), or, when no `device` is supplied, the
//! access-token subject (three-legged fallback), mirroring Quality on Demand.
//! Neither present → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! - **Reserved error suffix (identifier).** If the resolved identifier's trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`), `POST` answers with that canonical
//!   CAMARA error (shared [`crate::scenarios`]). In particular `…409` drives the
//!   `createSession` `409 CONFLICT` (duplicate session) case and `…422` the
//!   `422 SERVICE_NOT_APPLICABLE`.
//! - **Open-ended vs time-bounded (identifier digits).** Otherwise the session is
//!   `ACTIVE`; its `expiresAt` is present (`startsAt + 24 h`) unless the
//!   identifier's trailing three digits are `…000` (or it has no digits), which
//!   marks an **open-ended** session (no `expiresAt`).

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

use super::store;

/// Scope required to create a session (CAMARA SessionInsights).
const CREATE_SCOPE: &str = "session-insights:sessions:create";
/// Scope required to read a session (CAMARA SessionInsights).
const READ_SCOPE: &str = "session-insights:sessions:read";
/// Scope required to delete a session (CAMARA SessionInsights).
const DELETE_SCOPE: &str = "session-insights:sessions:delete";

/// A created session's lifetime when it is time-bounded (24 hours), added to
/// `startsAt` to compute `expiresAt`.
const SESSION_LIFETIME_SECS: i64 = 86_400;

/// Routes for Session Insights vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/session-insights/vwip/sessions", post(create_session))
        .route(
            "/session-insights/vwip/sessions/:session_id",
            get(get_session).delete(delete_session),
        )
        .route(
            "/session-insights/vwip/retrieve-sessions",
            post(retrieve_sessions),
        )
}

/// `POST /sessions` request body (CAMARA `CreateSession`). `applicationProfileId`,
/// `applicationServer` and `sink` are required; `device` is required only for a
/// two-legged token. Unknown fields are tolerated (the CAMARA request body does
/// not set `additionalProperties: false`).
#[derive(Debug, Deserialize)]
struct CreateSession {
    #[serde(rename = "applicationProfileId")]
    application_profile_id: Option<String>,
    device: Option<Device>,
    /// Accepted as an opaque object (echoed verbatim); validated for at least one
    /// endpoint field below.
    #[serde(rename = "applicationServer")]
    application_server: Option<Value>,
    #[serde(rename = "applicationSessionId")]
    application_session_id: Option<String>,
    sink: Option<String>,
    /// Accepted for schema fidelity; never echoed (it carries a secret) and not
    /// used in this slice — notification delivery is deferred.
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
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

/// `POST /session-insights/vwip/sessions`.
async fn create_session(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (three required fields); parse.
    let req: CreateSession = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid CreateSession.", &correlator)
        }
    };

    // Required fields.
    match req.application_profile_id.as_deref() {
        Some(id) if !id.is_empty() => {}
        _ => return invalid_argument("`applicationProfileId` is required.", &correlator),
    }
    let application_server = match &req.application_server {
        Some(a) if application_server_has_endpoint(a) => a.clone(),
        Some(_) => {
            return invalid_argument(
                "`applicationServer` must contain `domainName`, `ipv4Address`, and/or `ipv6Address`.",
                &correlator,
            )
        }
        None => return invalid_argument("`applicationServer` is required.", &correlator),
    };
    let sink = match req.application_session_id.as_deref() {
        Some(s) if s.chars().count() > 256 => {
            return invalid_argument(
                "`applicationSessionId` must be at most 256 characters.",
                &correlator,
            )
        }
        _ => match req.sink.as_deref() {
            Some(s) if is_http_uri(s) => s.to_string(),
            Some(_) => {
                return invalid_argument("`sink` must be an http(s) URI.", &correlator)
            }
            None => return invalid_argument("`sink` is required.", &correlator),
        },
    };

    // Resolve the identifier: the submitted device identifier, else the token
    // subject (three-legged fallback). Missing both → 422 MISSING_IDENTIFIER.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (incl. `…409` → the createSession 409 CONFLICT duplicate-session case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Build the SessionInfo, remember it, and return 201. A session is `ACTIVE`
    // with `startsAt = now`; it is time-bounded (`expiresAt = now + 24 h`) unless
    // the identifier tail is `…000`/no-digits, which marks it open-ended.
    let id = store::new_session_id();
    let now = now_unix_secs();
    let mut info = json!({
        "id": id,
        "applicationServer": application_server,
        "sink": sink,
        "startsAt": rfc3339_utc(now),
        "status": "ACTIVE",
    });
    if let Some(echo) = resolved.echo {
        info["device"] = echo;
    }
    if let Some(app_session_id) = req.application_session_id {
        info["applicationSessionId"] = json!(app_session_id);
    }
    let open_ended = matches!(scenarios::trailing_three_digits(&resolved.id), None | Some(0));
    if !open_ended {
        info["expiresAt"] = json!(rfc3339_utc(now + SESSION_LIFETIME_SECS));
    }

    store::insert(id, info.clone());
    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// `GET /session-insights/vwip/sessions/{sessionId}`.
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

/// `DELETE /session-insights/vwip/sessions/{sessionId}`.
///
/// Deletes the session resource. Keyed only on the stored state (docs/DESIGN.md
/// §7): a session that exists is evicted → `204 No Content` (single-use); an
/// unknown (or already-deleted) id → `404 NOT_FOUND`. No CloudEvent is emitted
/// (`session-ended` notifications on `sink` are deferred to a later pass).
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
        Some(_) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No session found for the provided sessionId.").into_response(),
            &correlator,
        ),
    }
}

/// `POST /retrieve-sessions` request body (CAMARA `RetrieveSessionsInput`). The
/// `device` is optional — omit it on a three-legged token to select the token
/// subject. Unknown fields are tolerated (the request body does not set
/// `additionalProperties: false`).
#[derive(Debug, Deserialize)]
struct RetrieveSessionsInput {
    device: Option<Device>,
}

/// `POST /session-insights/vwip/retrieve-sessions`.
///
/// Lists a device's session-insights resources as an array of `SessionInfo`
/// (`200`; an empty array when the device has none — CAMARA never 404s on an
/// empty result). The device is the submitted `device` identifier, else the token
/// subject (three-legged fallback; neither present → `422 MISSING_IDENTIFIER`).
/// Requires the `session-insights:sessions:read` scope (a read, like
/// `getSession`). Two control planes (docs/DESIGN.md §7): the identifier — a
/// reserved error suffix selects a canonical CAMARA error (so `…404` → `404
/// NOT_FOUND` for an unknown device) — and, on the happy path, the in-memory
/// store, matched by each session's echoed `device`. A resolved identifier with no
/// `device` echo (a non-E.164 token subject and no submitted device) matches
/// nothing → `200 []`.
async fn retrieve_sessions(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
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

    with_correlator((StatusCode::OK, Json(sessions)).into_response(), &correlator)
}

/// The resolved device identifier: the string CamaraSim keys its functional cases
/// off, plus the single-property `device` object to echo in the response (`None`
/// when the identifier came from a token subject that is not a phone number).
struct Resolved {
    id: String,
    echo: Option<Value>,
}

/// Resolve the device identifier: the first present identifier in the supplied
/// `device` (validated when it is a `phoneNumber`), else the token subject
/// (three-legged fallback). On failure returns the CAMARA error `Response` to
/// send — 400 `INVALID_ARGUMENT` for a malformed `phoneNumber` or a `device`
/// carrying no identifier, 422 `MISSING_IDENTIFIER` when neither a device nor a
/// token subject is present.
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

/// Whether an `applicationServer` object carries at least one endpoint field
/// (`domainName`, `ipv4Address`, or `ipv6Address`), so the server is reachable.
fn application_server_has_endpoint(server: &Value) -> bool {
    server
        .as_object()
        .map(|o| {
            o.contains_key("domainName")
                || o.contains_key("ipv4Address")
                || o.contains_key("ipv6Address")
        })
        .unwrap_or(false)
}

/// Whether `s` is an http(s) URI — a minimal `sink` sanity check.
fn is_http_uri(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Whether `s` is a valid CAMARA `phoneNumber`: E.164 (`+` then 5–15 digits, the
/// first `1`–`9`).
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
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as RFC 3339, e.g.
/// `2026-08-05T14:27:08Z`. Second precision. Self-contained (no date-time
/// dependency) via the civil-from-days algorithm below.
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a day count since 1970-01-01 into a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian). Month and day are
/// 1-based.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day-of-era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp = (5 * doy + 2) / 153; // month, shifted so March = 0 [0, 11]
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

    const HOST: &str = "si.local:8080";
    const SESSIONS: &str = "/session-insights/vwip/sessions";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn rfc3339_utc_formats_a_known_epoch() {
        // 2026-04-05T10:41:38Z = 1_775_385_698 seconds since the epoch.
        assert_eq!(rfc3339_utc(1_775_385_698), "2026-04-05T10:41:38Z");
    }

    #[test]
    fn e164_validation() {
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789012")); // no +
        assert!(!is_valid_e164("+0123456789")); // leading 0
        assert!(!is_valid_e164("+12")); // too short
    }

    #[test]
    fn http_uri_check() {
        assert!(is_http_uri("https://callback.example/notify"));
        assert!(is_http_uri("http://callback.example/notify"));
        assert!(!is_http_uri("ftp://callback.example"));
        assert!(!is_http_uri("callback.example"));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=si-client&scope={scope}");
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

    async fn post_session(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(SESSIONS)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    async fn get_session_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{SESSIONS}/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    async fn delete_session_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("{SESSIONS}/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    async fn retrieve_sessions_req(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/session-insights/vwip/retrieve-sessions")
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    /// A retrieve-sessions body naming `phone` as the device.
    fn retrieve_body(phone: &str) -> String {
        format!(r#"{{"device":{{"phoneNumber":"{phone}"}}}}"#)
    }

    async fn collect(response: Response) -> (StatusCode, HeaderMap, Value) {
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// A well-formed create body for the given `phoneNumber`.
    fn body_for(phone: &str) -> String {
        format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"{phone}"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "applicationSessionId":"game-42",
                 "sink":"https://callback.example/notify"}}"#
        )
    }

    async fn create_ok(phone: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_session(Some(&token), &body_for(phone), None).await
    }

    // --- Happy path & control planes ---------------------------------------

    #[tokio::test]
    async fn create_returns_201_active_time_bounded_session_echoing_inputs() {
        let (status, _, body) = create_ok("+123456789012").await;
        assert_eq!(status, StatusCode::CREATED);
        // Opaque id, minted.
        assert!(body["id"].as_str().unwrap().contains('-'));
        assert_eq!(body["status"], "ACTIVE");
        // startsAt always present; a non-…000 tail is time-bounded.
        assert!(body["startsAt"].as_str().unwrap().ends_with('Z'));
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
        // Inputs echoed.
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
        assert_eq!(body["applicationServer"]["ipv4Address"], "198.51.100.1");
        assert_eq!(body["applicationSessionId"], "game-42");
        assert_eq!(body["sink"], "https://callback.example/notify");
    }

    #[tokio::test]
    async fn a_zero_tail_identifier_is_open_ended_without_an_expiry() {
        let (status, _, body) = create_ok("+123456789000").await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVE");
        assert!(body["startsAt"].as_str().unwrap().ends_with('Z'));
        assert!(body.get("expiresAt").is_none(), "open-ended → no expiresAt");
    }

    #[tokio::test]
    async fn created_session_can_be_read_back() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_session(Some(&token), &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let read_token = mint_token(READ_SCOPE).await;
        let (status, _, read) = get_session_req(Some(&read_token), id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read, created, "GET returns the stored representation verbatim");
    }

    #[tokio::test]
    async fn get_unknown_session_is_not_found() {
        let token = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            get_session_req(Some(&token), "no-such-session-id", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject_open_ended() {
        // A client_credentials subject (`si-client`) has no digits → open-ended,
        // and is not E.164 → no `device` echo.
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "applicationServer":{"domainName":"app.example.com"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVE");
        assert!(body.get("expiresAt").is_none());
        assert!(body.get("device").is_none());
    }

    // --- Delete -------------------------------------------------------------

    #[tokio::test]
    async fn create_then_delete_returns_204_and_the_session_is_gone() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_session(Some(&token), &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // Delete it → 204 No Content, with an empty body.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null, "204 carries no body");

        // …and it can no longer be read back.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_session_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_not_found() {
        let token = mint_token(CREATE_SCOPE).await;
        let (_, _, created) = post_session(Some(&token), &body_for("+123456789012"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

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
    async fn delete_without_the_scope_is_forbidden() {
        // A read token must not satisfy the delete scope.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = delete_session_req(Some(&read), "any-id", None).await;
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
        let token = mint_token(CREATE_SCOPE).await;
        let (_, _, created) = post_session(Some(&token), &body_for("+123456789012"), None).await;
        let id = created["id"].as_str().unwrap().to_string();
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

    // --- Retrieve sessions by device ---------------------------------------

    #[tokio::test]
    async fn retrieve_sessions_returns_only_the_requested_devices_sessions() {
        // Two distinct devices; create one session for each.
        let dev_a = "+15550170101";
        let dev_b = "+15550170202";
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, made_a) = post_session(Some(&create), &body_for(dev_a), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, _, _made_b) = post_session(Some(&create), &body_for(dev_b), None).await;
        assert_eq!(status, StatusCode::CREATED);

        // Retrieving device A's sessions returns only A's.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) = retrieve_sessions_req(Some(&read), &retrieve_body(dev_a), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = list.as_array().expect("array");
        assert!(!arr.is_empty(), "device A has at least one session");
        assert!(
            arr.iter().all(|s| s["device"]["phoneNumber"] == dev_a),
            "only device A's sessions are listed"
        );
        // The created session is present.
        let created_id = made_a["id"].as_str().unwrap();
        assert!(arr.iter().any(|s| s["id"] == created_id));
    }

    #[tokio::test]
    async fn retrieve_sessions_for_a_device_with_none_is_empty_array() {
        // A device with no created sessions → 200 [] (never 404).
        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550179988"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn retrieve_sessions_reserved_suffix_selects_a_camara_error() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550170404"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550170429"), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn retrieve_sessions_with_a_non_e164_subject_and_no_device_is_empty() {
        // An empty body → device from the token subject; a client_credentials
        // subject (`si-client`) is not E.164, so it has no device echo → 200 [].
        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) = retrieve_sessions_req(Some(&read), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn retrieve_sessions_rejects_a_bad_body() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = retrieve_sessions_req(Some(&read), "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_sessions_requires_the_read_scope() {
        let other = mint_token("some:other-scope").await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&other), &retrieve_body("+15550170101"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_sessions_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            retrieve_sessions_req(None, &retrieve_body("+15550170101"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_retrieve_sessions() {
        let read = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550179988"), Some("corr-ret"))
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ret")
        );
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = create_ok("+123456789404").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = create_ok("+123456789409").await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "CONFLICT");

        let (status, _, body) = create_ok("+123456789422").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");

        let (status, _, body) = create_ok("+123456789429").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn missing_application_profile_id_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationServer":{"ipv4Address":"198.51.100.1"},
                       "device":{"phoneNumber":"+123456789012"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_application_server_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn application_server_without_an_endpoint_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "applicationServer":{"port":443},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_or_malformed_sink_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        // Missing.
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "applicationServer":{"ipv4Address":"198.51.100.1"}}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Malformed (not http(s)).
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "applicationServer":{"ipv4Address":"198.51.100.1"},
                       "sink":"ftp://nope"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_phone_number_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"12345"},
                       "applicationServer":{"ipv4Address":"198.51.100.1"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_body_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = post_session(Some(&token), "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn create_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_session(Some(&token), &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn read_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_session_req(Some(&token), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_session(None, &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // Success.
        let (status, headers, _) =
            post_session(Some(&token), &body_for("+123456789012"), Some("corr-si")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-si")
        );
        // Business error.
        let (status, headers, _) =
            post_session(Some(&token), &body_for("+123456789404"), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
