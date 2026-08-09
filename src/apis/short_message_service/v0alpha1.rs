//! Short Message Service **v0alpha1** (CAMARA Short Message Service,
//! `0.1.0-alpha.1`).
//!
//! One endpoint:
//! - `POST /sms/v0alpha1/short-message` — send an SMS to one or more recipients.
//!
//! ## What it does
//!
//! The caller submits the `from` sender MSISDN, the `to` recipients (at least
//! one), the `message` text, and optionally a `category`. The network "sends"
//! the SMS and returns `200 { msgId, timestamp }`. There is no real SMS, so the
//! send is simulated: `msgId` is deterministic from the request and `timestamp`
//! is the send time.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `send-sms:short-message` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The **first recipient** (`to[0]`) is the identifier:
//!
//! - **Reserved error suffix (identifier).** If `to[0]`'s trailing three digits
//!   name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`,
//!   `…422`, `…429`, `…500`, `…503`), the endpoint answers with that canonical
//!   CAMARA error (shared [`crate::scenarios`]); `…404` is SMS's
//!   recipient-not-found, `…503`/`…500` the network-unavailable / internal cases.
//! - **Happy path.** Otherwise the send succeeds with `200 { msgId, timestamp }`.
//!   `msgId` is a deterministic, UUID-shaped token derived from `from`, all of
//!   `to`, and `message` (SHA-256; no `uuid`/`rand` dependency), and `timestamp`
//!   is the send time (RFC 3339 UTC).
//!
//! A reserved suffix on `from`, or on a recipient other than `to[0]`, is **not**
//! the error plane — only `to[0]` selects the canonical error.
//!
//! Validation: a malformed body, an empty `to`, a `from`/recipient not in E.164
//! form, an empty `message`, or an unknown `category` → `400 INVALID_ARGUMENT`.
//! `x-correlator` is echoed on every response.
//!
//! Example: `POST /short-message` with `to: ["+123456789012"]`,
//! `from: "+123456789111"`, `message: "hi"` → `200` with a `msgId`;
//! `to: ["+123456789404"]` → `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /short-message` endpoint requires (CAMARA SMS).
const SEND_SCOPE: &str = "send-sms:short-message";

/// Routes for Short Message Service v0alpha1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/sms/v0alpha1/short-message", post(send_sms))
}

/// The message category (CAMARA `category` enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum Category {
    #[serde(rename = "PROMOTION")]
    Promotion,
    #[serde(rename = "SERVICE")]
    Service,
    #[serde(rename = "TRANSACTION")]
    Transaction,
}

/// `POST /short-message` request body (CAMARA `MessageRequest`): the recipients,
/// the sender, the message text, and an optional category. Unknown fields are
/// rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageRequest {
    to: Vec<String>,
    from: String,
    #[allow(dead_code)] // Accepted + validated at parse; does not shape the response.
    category: Option<Category>,
    message: String,
}

/// `POST /sms/v0alpha1/short-message`.
async fn send_sms(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SEND_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An unknown `category` / unknown field / bad JSON all surface here.
    let req: MessageRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid MessageRequest.", &correlator)
        }
    };

    // At least one recipient is required (schema `minItems: 1`).
    let Some(first_recipient) = req.to.first() else {
        return invalid_argument("`to` must contain at least one recipient.", &correlator);
    };

    // Every recipient and the sender must be E.164 (two-legged; no subject
    // fallback for this business-facing API).
    if !req.to.iter().all(|n| is_valid_e164(n)) {
        return invalid_argument(
            "Each `to` recipient must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }
    if !is_valid_e164(&req.from) {
        return invalid_argument(
            "`from` must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }

    // The message text must be present.
    if req.message.is_empty() {
        return invalid_argument("`message` must not be empty.", &correlator);
    }

    // The first recipient is the identifier (docs/DESIGN.md §7): a reserved
    // trailing suffix selects a canonical CAMARA error (…404 = recipient not
    // found). Only `to[0]` is the plane — `from`/later recipients are not.
    if let Some(err) = scenarios::reserved_error(first_recipient) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: the SMS is "sent". Return a deterministic handle + send time.
    let msg_id = message_id(&req);
    let timestamp = rfc3339_utc(now_unix_secs());
    with_correlator(
        (
            StatusCode::OK,
            Json(json!({ "msgId": msg_id, "timestamp": timestamp })),
        )
            .into_response(),
        &correlator,
    )
}

/// Derive a deterministic, UUID-shaped `msgId` from the request's sender, all
/// recipients, and message text (SHA-256; version/variant nibbles set so it is a
/// well-formed v4-shaped UUID, matching CAMARA's `format: uuid`). Deterministic
/// per request, and needs no `uuid`/`rand` dependency (mirrors Verified Caller's
/// `pre_announcement_id`).
fn message_id(req: &MessageRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(req.from.as_bytes());
    hasher.update([0]); // field separator, so a|b never collides with ab|""
    for recipient in &req.to {
        hasher.update(recipient.as_bytes());
        hasher.update([0]);
    }
    hasher.update([1]); // section separator between recipients and the message
    hasher.update(req.message.as_bytes());
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

    const HOST: &str = "sms.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("sms-client")); // client-credentials subject
    }

    fn req(to: &[&str], from: &str, message: &str) -> MessageRequest {
        MessageRequest {
            to: to.iter().map(|s| s.to_string()).collect(),
            from: from.into(),
            category: None,
            message: message.into(),
        }
    }

    #[test]
    fn message_id_is_uuid_v4_shaped_and_deterministic() {
        let r = req(&["+123456789012"], "+123456789111", "hello");
        let a = message_id(&r);
        let b = message_id(&r);
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
    fn message_id_varies_with_the_parties_and_text() {
        let base = message_id(&req(&["+123456789012"], "+123456789111", "hello"));
        assert_ne!(base, message_id(&req(&["+123456789013"], "+123456789111", "hello")));
        assert_ne!(base, message_id(&req(&["+123456789012"], "+123456789112", "hello")));
        assert_ne!(base, message_id(&req(&["+123456789012"], "+123456789111", "HELLO")));
        // A second recipient changes the id (the whole `to` list is hashed).
        assert_ne!(
            base,
            message_id(&req(&["+123456789012", "+123456789013"], "+123456789111", "hello"))
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
        let body = format!("grant_type=client_credentials&client_id=sms-client&scope={scope}");
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

    /// POST to `/short-message` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_send(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/sms/v0alpha1/short-message")
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
        let token = mint_token(SEND_SCOPE).await;
        post_send(Some(&token), body, None).await
    }

    // --- Happy path --------------------------------------------------------

    #[tokio::test]
    async fn send_returns_200_with_a_handle_and_timestamp() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let id = body["msgId"].as_str().unwrap();
        assert_eq!(id.split('-').count(), 5, "UUID-shaped msgId");
        let ts = body["timestamp"].as_str().unwrap();
        assert!(ts.ends_with('Z') && ts.len() == 20, "RFC 3339 timestamp");
    }

    #[tokio::test]
    async fn category_is_accepted_and_does_not_change_the_shape() {
        for cat in ["PROMOTION", "SERVICE", "TRANSACTION"] {
            let body = format!(
                r#"{{"to":["+123456789012"],"from":"+123456789111","message":"hi","category":"{cat}"}}"#
            );
            let (status, _, out) = call_ok(&body).await;
            assert_eq!(status, StatusCode::OK, "category={cat}");
            assert!(out["msgId"].is_string(), "category={cat}");
        }
    }

    #[tokio::test]
    async fn msg_id_is_deterministic_across_calls() {
        let body = r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi"}"#;
        let (_, _, a) = call_ok(body).await;
        let (_, _, b) = call_ok(body).await;
        assert_eq!(a["msgId"], b["msgId"], "same request → same msgId");
    }

    #[tokio::test]
    async fn multiple_recipients_are_accepted() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789012","+123456789013"],"from":"+123456789111","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["msgId"].is_string());
    }

    // --- Reserved-error convention (first recipient) -----------------------

    #[tokio::test]
    async fn reserved_suffix_on_first_recipient_selects_a_canonical_camara_error() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789404"],"from":"+123456789111","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(
            r#"{"to":["+123456789503"],"from":"+123456789111","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn a_reserved_from_is_not_the_error_plane() {
        // The identifier is the *first recipient*; a reserved suffix on `from`
        // must not trigger an error.
        let (status, _, _) = call_ok(
            r#"{"to":["+123456789012"],"from":"+123456789404","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_reserved_non_first_recipient_is_not_the_error_plane() {
        // Only `to[0]` selects the error; a reserved suffix on a later recipient
        // does not.
        let (status, _, _) = call_ok(
            r#"{"to":["+123456789012","+123456789404"],"from":"+123456789111","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn empty_to_is_invalid_argument() {
        let (status, _, body) =
            call_ok(r#"{"to":[],"from":"+123456789111","message":"hi"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_to_is_invalid_argument() {
        let (status, _, body) =
            call_ok(r#"{"from":"+123456789111","message":"hi"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_e164_recipient_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"to":["0123"],"from":"+123456789111","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_e164_from_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789012"],"from":"0123","message":"hi"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_message_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789012"],"from":"+123456789111","message":""}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_category_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi","category":"URGENT"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(
            r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi","x":1}"#,
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
        let (status, _, body) = post_send(
            Some(&token),
            r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_send(
            None,
            r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_200_and_error() {
        let token = mint_token(SEND_SCOPE).await;
        // 200.
        let (status, headers, _) = post_send(
            Some(&token),
            r#"{"to":["+123456789012"],"from":"+123456789111","message":"hi"}"#,
            Some("corr-200"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-200")
        );
        // Business error.
        let (status, headers, _) = post_send(
            Some(&token),
            r#"{"to":["+123456789404"],"from":"+123456789111","message":"hi"}"#,
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
