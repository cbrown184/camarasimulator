//! Quality on Demand **v1** (CAMARA quality-on-demand 1.1.0, release r3.2).
//!
//! First slice of the QoD session lifecycle:
//! - `POST /quality-on-demand/v1/sessions` — create a QoS session and return its
//!   `SessionInfo` (operationId `createSession`, scope
//!   `quality-on-demand:sessions:create`).
//! - `GET /quality-on-demand/v1/sessions/{sessionId}` — read back a session by id
//!   (operationId `getSession`, scope `quality-on-demand:sessions:read`).
//!
//! `DELETE`, `extend`, `retrieve-sessions`, and the CloudEvents notifications on
//! `sink` are deferred to later passes (see `PROGRESS.md`); `sink`/`sinkCredential`
//! are accepted for schema fidelity but no notification is emitted yet.
//!
//! ## What it does
//!
//! `createSession` mints an opaque, UUID-shaped `sessionId` ([`super::store`]),
//! renders the `SessionInfo` for the request, remembers it, and returns `201`;
//! `getSession` returns the stored `SessionInfo` (`200`) or `404 NOT_FOUND`.
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

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create a session (CAMARA quality-on-demand 1.1.0).
const CREATE_SCOPE: &str = "quality-on-demand:sessions:create";
/// Scope required to read a session (CAMARA quality-on-demand 1.1.0).
const READ_SCOPE: &str = "quality-on-demand:sessions:read";

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
            get(get_session),
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
        Some(d) if d > MAX_DURATION_SECS => {
            return with_correlator(
                CamaraError::new(
                    StatusCode::BAD_REQUEST,
                    "QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE",
                    "The requested duration is out of the allowed range for the QoS profile.",
                )
                .into_response(),
                &correlator,
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "qod.local:8080";
    const SESSIONS: &str = "/quality-on-demand/v1/sessions";
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
