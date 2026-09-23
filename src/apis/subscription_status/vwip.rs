//! Subscription Status **vwip** (CAMARA SubscriptionStatus, work-in-progress).
//!
//! One endpoint:
//! - `POST /subscription-status/vwip/retrieve-subscription-status` — the current
//!   service status of a mobile line.
//!
//! ## What it does
//!
//! The caller identifies a line and the operator answers with the live status of
//! its three service groups:
//!
//! ```json
//! { "voiceSmsIn": "active", "voiceSmsOut": "active", "dataService": "active" }
//! ```
//!
//! - `voiceSmsIn` / `voiceSmsOut` — inbound / outbound Voice & SMS
//!   (`active` | `suspended`).
//! - `dataService` — mobile data (`active` | `suspended` | `throttled`).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `subscription-status:retrieve-subscription-status` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by the
//! token **subject**, so resubmitting it is an error (mirrors Number Recycling):
//!
//! - `phoneNumber` present **and** the subject is itself an E.164 number (a
//!   line-authenticated three-legged token) → `422 UNNECESSARY_IDENTIFIER`.
//! - `phoneNumber` present, subject not a line → the submitted number is the
//!   identifier (two-legged).
//! - `phoneNumber` absent, subject is an E.164 number → the subject is the
//!   identifier (three-legged).
//! - `phoneNumber` absent **and** the subject is not a line → the line cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer:
//!
//! - **Reserved error suffix (identifier).** If the resolved identifier's
//!   trailing three digits name a reserved CAMARA status (`…400`, `…401`, `…403`,
//!   `…404`, `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with
//!   that canonical CAMARA error (shared [`crate::scenarios`]). In particular the
//!   `…404` suffix gives the canonical `NOT_FOUND` (the API's own 404 code is
//!   `IDENTIFIER_NOT_FOUND`) and `…422` gives `SERVICE_NOT_APPLICABLE` (the API's
//!   own 422 service-level code).
//! - **Service status (identifier digits).** Otherwise the identifier's trailing
//!   three digits `d` (`000`–`999`, all reserved suffixes being `≥ 400`, so every
//!   small `d` is free) are read as a **status bitfield**, giving the caller
//!   independent control of all three fields:
//!   - bit 0 (`d & 1`) → `voiceSmsIn` is `suspended` (else `active`),
//!   - bit 1 (`d & 2`) → `voiceSmsOut` is `suspended` (else `active`),
//!   - `(d >> 2) % 3` → `dataService`: `0` = `active`, `1` = `suspended`,
//!     `2` = `throttled`.
//!
//!   So `…000` (or an identifier with no digits) is the all-`active` healthy
//!   default, `…001` suspends inbound Voice/SMS only, `…008` throttles data only,
//!   and so on.
//!
//! Example: `+123456789000` → all `active`; `+123456789001` → `voiceSmsIn:
//! suspended`; `+123456789008` → `dataService: throttled`; `+123456789404` →
//! `404 NOT_FOUND`; `+123456789422` → `422 SERVICE_NOT_APPLICABLE`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /retrieve-subscription-status` endpoint requires
/// (CAMARA SubscriptionStatus).
const RETRIEVE_SCOPE: &str = "subscription-status:retrieve-subscription-status";

/// Routes for Subscription Status vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/subscription-status/vwip/retrieve-subscription-status",
        post(retrieve),
    )
}

/// `POST /retrieve-subscription-status` request body
/// (CAMARA `SubscriptionStatusRequest`): an optional `phoneNumber` (valid only
/// in two-legged auth). Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubscriptionStatusRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
}

/// `POST /subscription-status/vwip/retrieve-subscription-status`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (phoneNumber optional → three-legged); anything
    // present must parse.
    let req: SubscriptionStatusRequest = if body.is_empty() {
        SubscriptionStatusRequest { phone_number: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid SubscriptionStatusRequest.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the line identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(&req, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // The trailing three digits are a status bitfield over the three fields.
    let d = scenarios::trailing_three_digits(&identifier).unwrap_or(0);
    let voice_sms_in = if d & 0b001 != 0 { "suspended" } else { "active" };
    let voice_sms_out = if d & 0b010 != 0 { "suspended" } else { "active" };
    let data_service = match (d >> 2) % 3 {
        0 => "active",
        1 => "suspended",
        _ => "throttled",
    };

    with_correlator(
        (
            StatusCode::OK,
            Json(json!({
                "voiceSmsIn": voice_sms_in,
                "voiceSmsOut": voice_sms_out,
                "dataService": data_service,
            })),
        )
            .into_response(),
        &correlator,
    )
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Number Recycling). See the module docs for the four cases.
fn resolve_identifier(
    req: &SubscriptionStatusRequest,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match &req.phone_number {
        Some(phone) => {
            if !is_valid_e164(phone) {
                return Err(invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    correlator,
                ));
            }
            // The line is already identified by a three-legged token; the number
            // must not be resubmitted.
            if subject_is_line {
                return Err(unprocessable(
                    "UNNECESSARY_IDENTIFIER",
                    "The phone number is already identified by the access token.",
                    correlator,
                ));
            }
            Ok(phone.clone())
        }
        None => {
            if subject_is_line {
                Ok(subject.to_string())
            } else {
                Err(unprocessable(
                    "MISSING_IDENTIFIER",
                    "The phone number cannot be identified: supply `phoneNumber` or use a token that identifies a line.",
                    correlator,
                ))
            }
        }
    }
}

/// A 422 CAMARA error with a caller-chosen `code`, correlator echoed.
fn unprocessable(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message).into_response(),
        correlator,
    )
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

    const HOST: &str = "ss.local:8080";
    const PATH: &str = "/subscription-status/vwip/retrieve-subscription-status";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("ss-client")); // client-credentials subject
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "ss-client").await
    }

    /// Mint an access token via `client_credentials` with a caller-chosen
    /// `client_id` (which becomes the token `sub`). `+` is percent-encoded so an
    /// E.164 client id survives the urlencoded body.
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

    /// POST to the endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(PATH)
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

    /// Mint a scoped (two-legged, non-line subject) token and call the endpoint.
    async fn retrieve_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    // --- Service-status control plane --------------------------------------

    #[tokio::test]
    async fn all_active_healthy_default_for_zero_tail() {
        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789000"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["voiceSmsIn"], "active");
        assert_eq!(body["voiceSmsOut"], "active");
        assert_eq!(body["dataService"], "active");
    }

    #[tokio::test]
    async fn each_field_is_independently_controllable() {
        // …001 → bit 0 → inbound Voice/SMS suspended only.
        let (_, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789001"}"#).await;
        assert_eq!(body["voiceSmsIn"], "suspended");
        assert_eq!(body["voiceSmsOut"], "active");
        assert_eq!(body["dataService"], "active");

        // …002 → bit 1 → outbound Voice/SMS suspended only.
        let (_, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789002"}"#).await;
        assert_eq!(body["voiceSmsIn"], "active");
        assert_eq!(body["voiceSmsOut"], "suspended");
        assert_eq!(body["dataService"], "active");

        // …003 → bits 0+1 → both Voice/SMS directions suspended, data active.
        let (_, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789003"}"#).await;
        assert_eq!(body["voiceSmsIn"], "suspended");
        assert_eq!(body["voiceSmsOut"], "suspended");
        assert_eq!(body["dataService"], "active");
    }

    #[tokio::test]
    async fn data_service_covers_all_three_states() {
        // …004 → (4>>2)%3 = 1 → data suspended.
        let (_, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789004"}"#).await;
        assert_eq!(body["dataService"], "suspended");
        assert_eq!(body["voiceSmsIn"], "active");
        assert_eq!(body["voiceSmsOut"], "active");

        // …008 → (8>>2)%3 = 2 → data throttled.
        let (_, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789008"}"#).await;
        assert_eq!(body["dataService"], "throttled");
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789422"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");

        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; subject is an E.164 line whose tail is …001.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789001").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["voiceSmsIn"], "suspended");
        assert_eq!(body["voiceSmsOut"], "active");
    }

    #[tokio::test]
    async fn three_legged_empty_body_is_accepted() {
        // A three-legged caller may send no body at all.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789000").await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dataService"], "active");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = retrieve_ok("{}").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            retrieve_ok(r#"{"phoneNumber":"+123456789012","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = retrieve_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_retrieve(None, r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"phoneNumber":"+123456789000"}"#,
            Some("corr-ss"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ss")
        );
        // Business error.
        let (status, headers, _) =
            post_retrieve(Some(&token), "{}", Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
