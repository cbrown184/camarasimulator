//! Click to Dial **vwip** (CAMARA Click to Dial, `wip`).
//!
//! One endpoint so far:
//! - `POST /click-to-dial/vwip/calls` — create a Click to Dial call session
//!   between a `caller` and a `callee`.
//!
//! ## What it does
//!
//! The application submits the `caller` (the party dialled first) and the
//! `callee` (the party bridged to), optionally enabling call recording and a
//! `sink` for status callbacks. The platform "creates the call" and returns the
//! newly-minted [`Call`](https://github.com/camaraproject/ClickToDial) resource:
//! its opaque `callId`, the two participants echoed back, the current `status`
//! (always `initiating` at creation — later transitions arrive via `sink`
//! notifications, a deferred slice), the `createdAt` instant, and whether
//! recording is enabled.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `click-to-dial:calls:create`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Both participants are carried explicitly (two-legged, business-facing — no
//! line subject, so no two-legged/three-legged identifier dance). The `callee`
//! (the party being reached) is the identifier, and the two numbers plus
//! `recordingEnabled` drive every case:
//!
//! - **Reserved error suffix (identifier).** If the `callee` number's trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with that
//!   canonical CAMARA error (shared [`crate::scenarios`]).
//! - **`callee` not available (`…000`).** A `callee` number whose trailing three
//!   digits are `000` cannot be reached → `422 CALLEE_NOT_AVAILABLE`.
//! - **`caller` not available (`…000`).** Likewise a `caller` number ending
//!   `000` cannot be dialled → `422 CALLER_NOT_AVAILABLE` (checked before the
//!   callee, so a caller problem is surfaced first).
//! - **Recording not supported (`…777` + `recordingEnabled`).** When
//!   `recordingEnabled` is `true` and the `callee` number ends `777`, that line
//!   cannot be recorded → `422 RECORDING_NOT_SUPPORTED`. (`777` is a documented
//!   CamaraSim sentinel; recording on any other line is honoured.)
//! - **Happy path.** Any other pair → `201` with a `Call` whose `status` is
//!   `initiating`. `callId` is a deterministic, UUID-shaped token derived from
//!   the two numbers (SHA-256; no `uuid`/`rand` dependency).
//!
//! Validation: a malformed body, an unknown field, or a missing
//! `caller`/`callee`/`number` → `400 INVALID_ARGUMENT`; a participant number
//! present but not in E.164 form → `422 INVALID_PHONE_NUMBER`; identical
//! `caller` and `callee` numbers → `422 SAME_CALLER_CALLEE`. `x-correlator` is
//! echoed on every response.
//!
//! ## Documented cuts
//!
//! - **Stateless create.** The `201` is fully determined by the request; the
//!   call is not persisted, so the stateful `getCall` / `terminateCall` /
//!   `getRecording` operations (and the `409 ALREADY_EXISTS` duplicate-call
//!   case, which needs stored state) are a later slice.
//! - **Notifications deferred.** `sink` / `sinkCredential` are accepted for
//!   schema fidelity but not delivered to — the `status-changed` CloudEvents are
//!   a later slice (like QoD's first create pass).

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /calls` endpoint requires (CAMARA Click to Dial).
const CREATE_SCOPE: &str = "click-to-dial:calls:create";

/// The trailing-three-digit sentinel that marks a line as unreachable
/// (`caller`/`callee` not available). Not a reserved *error* suffix, so it is
/// free for this API to use as a happy-path-adjacent marker.
const NOT_AVAILABLE_TAIL: u16 = 0;

/// The trailing-three-digit sentinel that marks a `callee` line as
/// non-recordable (only relevant when `recordingEnabled` is `true`).
const RECORDING_UNSUPPORTED_TAIL: u16 = 777;

/// Routes for Click to Dial vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/click-to-dial/vwip/calls", post(create_call))
}

/// A call participant (`caller` / `callee`) — an object carrying a `number`
/// (CAMARA `Caller`/`Callee`). Modelled as a struct (not a bare string) to match
/// the CAMARA schema exactly; unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Party {
    number: String,
}

/// `POST /calls` request body (CAMARA `CreateCallRequest`): the two required
/// participants plus optional recording and callback controls. Unknown fields
/// are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCallRequest {
    caller: Party,
    callee: Party,
    #[serde(rename = "recordingEnabled")]
    recording_enabled: Option<bool>,
    /// Accepted for schema fidelity (so `deny_unknown_fields` does not reject a
    /// request that carries it); notifications are a deferred slice, so it is
    /// never read.
    #[allow(dead_code)]
    sink: Option<String>,
    /// Accepted for schema fidelity; not applied (deferred with `sink`).
    #[allow(dead_code)]
    #[serde(rename = "sinkCredential")]
    sink_credential: Option<Value>,
}

/// `POST /click-to-dial/vwip/calls`.
async fn create_call(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // A malformed body, an unknown field, or a missing caller/callee/number all
    // surface here as a 400 (structural).
    let req: CreateCallRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CreateCallRequest.",
                &correlator,
            )
        }
    };

    // A number that is present but not E.164 is a semantic phone error (422),
    // distinct from a structurally-missing field (400 above).
    if !is_valid_e164(&req.caller.number) {
        return unprocessable(
            "INVALID_PHONE_NUMBER",
            "`caller.number` must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }
    if !is_valid_e164(&req.callee.number) {
        return unprocessable(
            "INVALID_PHONE_NUMBER",
            "`callee.number` must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }

    // A caller may not dial itself.
    if req.caller.number == req.callee.number {
        return unprocessable(
            "SAME_CALLER_CALLEE",
            "`caller` and `callee` must be different phone numbers.",
            &correlator,
        );
    }

    // The callee is the identifier (docs/DESIGN.md §7): a reserved trailing
    // suffix selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&req.callee.number) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Reachability: a `…000` tail marks a line that cannot be set up. The caller
    // is checked first so a caller-side problem is surfaced before the callee's.
    if scenarios::trailing_three_digits(&req.caller.number) == Some(NOT_AVAILABLE_TAIL) {
        return unprocessable(
            "CALLER_NOT_AVAILABLE",
            "The caller line is not available to place the call.",
            &correlator,
        );
    }
    if scenarios::trailing_three_digits(&req.callee.number) == Some(NOT_AVAILABLE_TAIL) {
        return unprocessable(
            "CALLEE_NOT_AVAILABLE",
            "The callee line is not available to receive the call.",
            &correlator,
        );
    }

    // Recording capability: only relevant when recording is requested. A `…777`
    // callee line cannot be recorded.
    let recording_enabled = req.recording_enabled.unwrap_or(false);
    if recording_enabled
        && scenarios::trailing_three_digits(&req.callee.number) == Some(RECORDING_UNSUPPORTED_TAIL)
    {
        return unprocessable(
            "RECORDING_NOT_SUPPORTED",
            "Call recording is not supported for the callee line.",
            &correlator,
        );
    }

    // Happy path: the platform accepts the call, which starts in `initiating`.
    let call = json!({
        "callId": call_id(&req.caller.number, &req.callee.number),
        "caller": { "number": req.caller.number },
        "callee": { "number": req.callee.number },
        "status": "initiating",
        "createdAt": rfc3339_utc(now_unix_secs()),
        "recordingEnabled": recording_enabled,
    });
    with_correlator((StatusCode::CREATED, Json(call)).into_response(), &correlator)
}

/// Derive a deterministic, UUID-shaped `callId` from the two participant
/// numbers (SHA-256; version/variant nibbles set so it is a well-formed
/// v4-shaped UUID). Deterministic per participant pair, and needs no
/// `uuid`/`rand` dependency (mirrors Verified Caller's `pre_announcement_id`).
fn call_id(caller: &str, callee: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(caller.as_bytes());
    hasher.update([0]); // field separator, so a|b never collides with ab|""
    hasher.update(callee.as_bytes());
    let d = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs no
/// date/time dependency (mirrors `verified_caller::vwip`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian).
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

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 422 CAMARA error with a Click-to-Dial-specific `code`, correlator echoed.
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "ctd.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("ctd-client")); // client-credentials subject
    }

    #[test]
    fn call_id_is_uuid_v4_shaped_and_deterministic() {
        let a = call_id("+123456789111", "+123456789012");
        let b = call_id("+123456789111", "+123456789012");
        assert_eq!(a, b, "same pair → same id (deterministic)");
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'));
        assert_eq!(parts[2].as_bytes()[0], b'4', "version 4");
        assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn call_id_varies_with_the_participants() {
        assert_ne!(
            call_id("+123456789111", "+123456789012"),
            call_id("+123456789111", "+123456789013")
        );
        // Order matters: caller/callee are not symmetric.
        assert_ne!(
            call_id("+123456789111", "+123456789012"),
            call_id("+123456789012", "+123456789111")
        );
    }

    #[test]
    fn rfc3339_utc_formats_a_known_epoch() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_704_119_228), "2024-01-01T14:27:08Z");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=ctd-client&scope={scope}");
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

    /// POST to `/calls` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_calls(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/click-to-dial/vwip/calls")
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
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Mint a scoped token and call the endpoint.
    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_calls(Some(&token), body, None).await
    }

    // --- Happy path --------------------------------------------------------

    #[tokio::test]
    async fn create_returns_201_with_an_initiating_call() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = body["callId"].as_str().unwrap();
        assert_eq!(id.split('-').count(), 5, "UUID-shaped callId");
        assert_eq!(body["status"], "initiating");
        assert_eq!(body["caller"]["number"], "+123456789111");
        assert_eq!(body["callee"]["number"], "+123456789012");
        assert_eq!(body["recordingEnabled"], false, "recording defaults to false");
        let created = body["createdAt"].as_str().unwrap();
        assert!(created.ends_with('Z') && created.len() == 20, "RFC 3339 createdAt");
    }

    #[tokio::test]
    async fn recording_enabled_is_echoed_on_the_happy_path() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"},"recordingEnabled":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["recordingEnabled"], true);
    }

    #[tokio::test]
    async fn sink_and_credential_are_accepted_but_do_not_change_the_result() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"},"sink":"http://cb.test/notify","sinkCredential":{"credentialType":"ACCESSTOKEN","accessToken":"x","accessTokenType":"bearer"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "initiating");
    }

    // --- Reserved-error convention (callee) --------------------------------

    #[tokio::test]
    async fn reserved_suffix_on_callee_selects_a_canonical_camara_error() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789404"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789429"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn a_reserved_caller_is_not_the_error_plane() {
        // The identifier is the callee; a reserved suffix on the caller must not
        // trigger the reserved-error convention (…404 caller is a happy path).
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789404"},"callee":{"number":"+123456789012"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "initiating");
    }

    // --- Availability & recording 422 planes -------------------------------

    #[tokio::test]
    async fn callee_ending_000_is_callee_not_available() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789000"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "CALLEE_NOT_AVAILABLE");
    }

    #[tokio::test]
    async fn caller_ending_000_is_caller_not_available() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789000"},"callee":{"number":"+123456789012"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "CALLER_NOT_AVAILABLE");
    }

    #[tokio::test]
    async fn recording_on_a_777_callee_is_recording_not_supported() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789777"},"recordingEnabled":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "RECORDING_NOT_SUPPORTED");
    }

    #[tokio::test]
    async fn a_777_callee_without_recording_is_a_happy_path() {
        // The recording-not-supported plane only fires when recording is asked
        // for; `777` alone is a normal, reachable line.
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789777"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "initiating");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn same_caller_and_callee_is_same_caller_callee() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789012"},"callee":{"number":"+123456789012"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SAME_CALLER_CALLEE");
    }

    #[tokio::test]
    async fn non_e164_number_is_invalid_phone_number() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"0123"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "INVALID_PHONE_NUMBER");
    }

    #[tokio::test]
    async fn missing_callee_is_invalid_argument() {
        let (status, _, body) =
            call_ok(r#"{"caller":{"number":"+123456789111"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_number_inside_a_party_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"},"x":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = call_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_calls(
            Some(&token),
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_calls(
            None,
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_201_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // 201.
        let (status, headers, _) = post_calls(
            Some(&token),
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789012"}}"#,
            Some("corr-201"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-201")
        );
        // Business error (reserved suffix).
        let (status, headers, _) = post_calls(
            Some(&token),
            r#"{"caller":{"number":"+123456789111"},"callee":{"number":"+123456789404"}}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
