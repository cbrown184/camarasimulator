//! Verified Caller **vwip** (CAMARA Verified Caller, `wip`).
//!
//! One endpoint:
//! - `POST /verified-caller/vwip/pre-announce` — pre-announce an outbound call
//!   so the platform can verify the calling party to the called party.
//!
//! ## What it does
//!
//! The caller submits the `callingParticipant` (the business placing the call)
//! and the `calledParticipant` (the consumer being called), optionally choosing
//! a verification `strategy` (`SMS` or `BRAND_DISPLAY`) and a `timeToLive`. The
//! platform "creates a pre-announcement" and either returns a handle for it —
//! `201 { preAnnouncementId, expiresAt }` — or acknowledges it with no content
//! (`204`).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `verified-caller:create` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The `calledParticipant` (the consumer whose call is being announced) is the
//! identifier, and two body fields shape a happy-path response:
//!
//! - **Reserved error suffix (identifier).** If the `calledParticipant`'s
//!   trailing three digits name a reserved CAMARA status (`…400`, `…401`,
//!   `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint
//!   answers with that canonical CAMARA error (shared [`crate::scenarios`]); the
//!   `…404` case is Verified Caller's participant-not-found.
//! - **Response shape (`strategy`).** Otherwise the `strategy` selects how the
//!   platform acknowledges the pre-announcement: `BRAND_DISPLAY` needs a
//!   client-side handle to correlate the branded display, so it returns
//!   `201 { preAnnouncementId, expiresAt }`; `SMS` (the default when `strategy`
//!   is omitted) is a fire-and-forget out-of-band message, so it returns
//!   `204 No Content`. `strategy` is thus a genuine control plane over the
//!   response.
//! - **Validity window (`timeToLive`).** On the `201` path, `expiresAt` is
//!   *now + timeToLive* (RFC 3339 UTC), so `timeToLive` (default `120` s when
//!   omitted) is a second happy-path control plane. `preAnnouncementId` is a
//!   deterministic, UUID-shaped token derived from the two participants and the
//!   strategy (SHA-256; no `uuid`/`rand` dependency).
//!
//! Validation: a malformed body, a participant not in E.164 form, an unknown
//! `strategy`, or an over-length `registrationId`/`dynamicDisplayName`/
//! `callReason` → `400 INVALID_ARGUMENT`; a `timeToLive` outside `1..=86400` →
//! `400 OUT_OF_RANGE`. `x-correlator` is echoed on every response, including the
//! `204`.
//!
//! Example: `POST /pre-announce` with `calledParticipant: "+123456789012"` and
//! `strategy: "BRAND_DISPLAY"` → `201` with a `preAnnouncementId`; the same with
//! `strategy: "SMS"` (or omitted) → `204`; `calledParticipant: "+123456789404"`
//! → `404 NOT_FOUND`.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /pre-announce` endpoint requires (CAMARA Verified
/// Caller).
const CREATE_SCOPE: &str = "verified-caller:create";

/// Default `timeToLive` (seconds) when the request omits one. Documented
/// simulator choice — the CAMARA schema sets no default, only the `1..=86400`
/// bounds.
const DEFAULT_TTL_SECS: i64 = 120;

/// Routes for Verified Caller vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/verified-caller/vwip/pre-announce", post(pre_announce))
}

/// The caller-chosen verification strategy (CAMARA `strategy` enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Strategy {
    #[serde(rename = "SMS")]
    Sms,
    #[serde(rename = "BRAND_DISPLAY")]
    BrandDisplay,
}

/// `POST /pre-announce` request body (CAMARA `CreatePreAnnouncementRequest`):
/// the two required participants plus optional verification controls. Unknown
/// fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePreAnnouncementRequest {
    #[serde(rename = "callingParticipant")]
    calling_participant: String,
    #[serde(rename = "calledParticipant")]
    called_participant: String,
    strategy: Option<Strategy>,
    #[serde(rename = "timeToLive")]
    time_to_live: Option<i64>,
    #[serde(rename = "registrationId")]
    registration_id: Option<String>,
    #[serde(rename = "dynamicDisplayName")]
    dynamic_display_name: Option<String>,
    #[serde(rename = "callReason")]
    call_reason: Option<String>,
}

/// `POST /verified-caller/vwip/pre-announce`.
async fn pre_announce(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An unknown `strategy` / unknown field / bad JSON all surface here.
    let req: CreatePreAnnouncementRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CreatePreAnnouncementRequest.",
                &correlator,
            )
        }
    };

    // Both participants are required and must be E.164 (two-legged; no subject
    // fallback for this business-facing API).
    if !is_valid_e164(&req.calling_participant) {
        return invalid_argument(
            "`callingParticipant` must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }
    if !is_valid_e164(&req.called_participant) {
        return invalid_argument(
            "`calledParticipant` must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }

    // `timeToLive`, when present, must be within the CAMARA bounds.
    if let Some(ttl) = req.time_to_live {
        if !(1..=86_400).contains(&ttl) {
            return out_of_range("`timeToLive` must be between 1 and 86400 seconds.", &correlator);
        }
    }

    // Bounded free-text / id fields (CAMARA maxLength constraints).
    if req.registration_id.as_deref().is_some_and(|s| s.is_empty() || s.len() > 256) {
        return invalid_argument("`registrationId` must be 1–256 characters.", &correlator);
    }
    if req.dynamic_display_name.as_deref().is_some_and(|s| s.len() > 32) {
        return invalid_argument("`dynamicDisplayName` must be at most 32 characters.", &correlator);
    }
    if req.call_reason.as_deref().is_some_and(|s| s.len() > 32) {
        return invalid_argument("`callReason` must be at most 32 characters.", &correlator);
    }

    // The called participant is the identifier (docs/DESIGN.md §7): a reserved
    // trailing suffix selects a canonical CAMARA error (…404 = participant not
    // found).
    if let Some(err) = scenarios::reserved_error(&req.called_participant) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Response shape is driven by the strategy (default SMS): BRAND_DISPLAY
    // hands back a correlation handle, SMS is fire-and-forget.
    match req.strategy.unwrap_or(Strategy::Sms) {
        Strategy::BrandDisplay => {
            let ttl = req.time_to_live.unwrap_or(DEFAULT_TTL_SECS);
            let expires_at = rfc3339_utc(now_unix_secs() + ttl);
            let id = pre_announcement_id(&req);
            with_correlator(
                (
                    StatusCode::CREATED,
                    Json(json!({ "preAnnouncementId": id, "expiresAt": expires_at })),
                )
                    .into_response(),
                &correlator,
            )
        }
        Strategy::Sms => {
            with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
        }
    }
}

/// Derive a deterministic, UUID-shaped `preAnnouncementId` from the request's
/// participants and strategy (SHA-256; version/variant nibbles set so it is a
/// well-formed v4-shaped UUID, matching CAMARA's `format: uuid`). Deterministic
/// per request, and needs no `uuid`/`rand` dependency (mirrors the QoD store's
/// `mint_uuid`).
fn pre_announcement_id(req: &CreatePreAnnouncementRequest) -> String {
    let strategy = match req.strategy.unwrap_or(Strategy::Sms) {
        Strategy::Sms => "SMS",
        Strategy::BrandDisplay => "BRAND_DISPLAY",
    };
    let mut hasher = Sha256::new();
    hasher.update(req.calling_participant.as_bytes());
    hasher.update([0]); // field separator, so a|b never collides with ab|""
    hasher.update(req.called_participant.as_bytes());
    hasher.update([0]);
    hasher.update(strategy.as_bytes());
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
/// date/time dependency (mirrors `quality_on_demand::v1` / `sim_swap::v2`).
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

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed.
fn out_of_range(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, "OUT_OF_RANGE", message).into_response(),
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

    const HOST: &str = "vc.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("vc-client")); // client-credentials subject
    }

    #[test]
    fn pre_announcement_id_is_uuid_v4_shaped_and_deterministic() {
        let req = CreatePreAnnouncementRequest {
            calling_participant: "+123456789111".into(),
            called_participant: "+123456789012".into(),
            strategy: Some(Strategy::BrandDisplay),
            time_to_live: None,
            registration_id: None,
            dynamic_display_name: None,
            call_reason: None,
        };
        let a = pre_announcement_id(&req);
        let b = pre_announcement_id(&req);
        assert_eq!(a, b, "same request → same id (deterministic)");
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
    fn pre_announcement_id_varies_with_the_participants() {
        let req = |called: &str| CreatePreAnnouncementRequest {
            calling_participant: "+123456789111".into(),
            called_participant: called.into(),
            strategy: Some(Strategy::BrandDisplay),
            time_to_live: None,
            registration_id: None,
            dynamic_display_name: None,
            call_reason: None,
        };
        assert_ne!(
            pre_announcement_id(&req("+123456789012")),
            pre_announcement_id(&req("+123456789013"))
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
        let body = format!("grant_type=client_credentials&client_id=vc-client&scope={scope}");
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

    /// POST to `/pre-announce` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_pre_announce(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/verified-caller/vwip/pre-announce")
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
        post_pre_announce(Some(&token), body, None).await
    }

    // --- Response-shape control plane (strategy) ---------------------------

    #[tokio::test]
    async fn brand_display_returns_201_with_a_handle() {
        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"BRAND_DISPLAY"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = body["preAnnouncementId"].as_str().unwrap();
        assert_eq!(id.split('-').count(), 5, "UUID-shaped id");
        let expires = body["expiresAt"].as_str().unwrap();
        assert!(expires.ends_with('Z') && expires.len() == 20, "RFC 3339 expiresAt");
    }

    #[tokio::test]
    async fn sms_strategy_returns_204_no_content() {
        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"SMS"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null, "204 carries no body");
    }

    #[tokio::test]
    async fn omitted_strategy_defaults_to_sms_204() {
        let (status, _, _) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // --- Validity window control plane (timeToLive) ------------------------

    #[tokio::test]
    async fn time_to_live_drives_expires_at() {
        // Two BRAND_DISPLAY requests with different TTLs → expiresAt differs by
        // the TTL delta (both stamped against ~the same now).
        let (_, _, short) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"BRAND_DISPLAY","timeToLive":10}"#,
        )
        .await;
        let (_, _, long) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"BRAND_DISPLAY","timeToLive":86400}"#,
        )
        .await;
        let s = parse_rfc3339_secs(short["expiresAt"].as_str().unwrap());
        let l = parse_rfc3339_secs(long["expiresAt"].as_str().unwrap());
        assert!(l - s >= 86_000, "a larger timeToLive pushes expiresAt out (got {})", l - s);
    }

    #[tokio::test]
    async fn time_to_live_out_of_range_is_out_of_range() {
        for ttl in ["0", "86401"] {
            let body = format!(
                r#"{{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"BRAND_DISPLAY","timeToLive":{ttl}}}"#
            );
            let (status, _, out) = call_ok(&body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "ttl={ttl}");
            assert_eq!(out["code"], "OUT_OF_RANGE", "ttl={ttl}");
        }
    }

    // --- Reserved-error convention (calledParticipant) ---------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789404","strategy":"BRAND_DISPLAY"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789429","strategy":"SMS"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn a_reserved_calling_participant_is_not_the_error_plane() {
        // The identifier is the *called* participant; a reserved suffix on the
        // *calling* participant must not trigger an error.
        let (status, _, _) = call_ok(
            r#"{"callingParticipant":"+123456789404","calledParticipant":"+123456789012","strategy":"SMS"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_called_participant_is_invalid_argument() {
        let (status, _, body) =
            call_ok(r#"{"callingParticipant":"+123456789111"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_e164_participant_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"0123"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_strategy_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"CARRIER_PIGEON"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn over_length_call_reason_is_invalid_argument() {
        let long = "x".repeat(33);
        let body = format!(
            r#"{{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"SMS","callReason":"{long}"}}"#
        );
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","x":1}"#,
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
        let (status, _, body) = post_pre_announce(
            Some(&token),
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"SMS"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_pre_announce(
            None,
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"SMS"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_201_204_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // 201 (BRAND_DISPLAY).
        let (status, headers, _) = post_pre_announce(
            Some(&token),
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"BRAND_DISPLAY"}"#,
            Some("corr-201"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-201")
        );
        // 204 (SMS) — correlator still echoed.
        let (status, headers, _) = post_pre_announce(
            Some(&token),
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789012","strategy":"SMS"}"#,
            Some("corr-204"),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-204")
        );
        // Business error.
        let (status, headers, _) = post_pre_announce(
            Some(&token),
            r#"{"callingParticipant":"+123456789111","calledParticipant":"+123456789404","strategy":"SMS"}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    /// Parse the RFC 3339 UTC shape this module emits back to a Unix timestamp
    /// (test-only inverse of [`rfc3339_utc`]).
    fn parse_rfc3339_secs(s: &str) -> i64 {
        let y: i64 = s[0..4].parse().unwrap();
        let mo: u32 = s[5..7].parse().unwrap();
        let d: u32 = s[8..10].parse().unwrap();
        let hh: i64 = s[11..13].parse().unwrap();
        let mm: i64 = s[14..16].parse().unwrap();
        let ss: i64 = s[17..19].parse().unwrap();
        days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mm * 60 + ss
    }

    /// Days since 1970-01-01 for a civil date (inverse of [`civil_from_days`]),
    /// test-only helper for [`parse_rfc3339_secs`].
    fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let (m, d) = (m as i64, d as i64);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}
