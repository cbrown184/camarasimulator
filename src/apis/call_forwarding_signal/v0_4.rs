//! Call Forwarding Signal **v0.4** (CAMARA Call Forwarding Signal 0.4.0, r3.3).
//!
//! One endpoint so far:
//! - `POST /call-forwarding-signal/v0.4/unconditional-call-forwardings` —
//!   report whether *unconditional* call forwarding is active for a line.
//!
//! ## What it does
//!
//! The caller asks whether the line has unconditional call forwarding turned on
//! (every incoming call diverted, regardless of state) and the operator answers
//! `{ "active": true|false }`. It is a fraud signal: a hijacked line often has
//! forwarding switched on to intercept calls and one-time passwords.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `call-forwarding-signal:unconditional-call-forwardings:read` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by
//! the token **subject**, so resubmitting it is an error:
//!
//! - `phoneNumber` present **and** the token subject is itself an E.164 number
//!   (a line-authenticated three-legged token) → `422 UNNECESSARY_IDENTIFIER`.
//! - `phoneNumber` present, subject not a line → the submitted number is the
//!   identifier (two-legged).
//! - `phoneNumber` absent, subject is an E.164 number → the subject is the
//!   identifier (three-legged).
//! - `phoneNumber` absent **and** the subject is not a line → the line cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Once the identifier is resolved, the result is deterministic from it:
//!
//! - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!   status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!   `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Active marker** — an **odd** trailing-three-digit tail → `active: true`
//!   (unconditional forwarding is on).
//! - **Happy path** — an **even** tail (including `…000`) → `active: false`
//!   (no unconditional forwarding — the common case).
//!
//! Examples: `+123456789013` → `true`; `+123456789012` → `false`;
//! `+123456789404` → `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /unconditional-call-forwardings` endpoint requires
/// (CAMARA Call Forwarding Signal 0.4.0).
const UNCONDITIONAL_SCOPE: &str = "call-forwarding-signal:unconditional-call-forwardings:read";

/// Routes for Call Forwarding Signal v0.4, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/call-forwarding-signal/v0.4/unconditional-call-forwardings",
        post(unconditional_call_forwardings),
    )
}

/// `POST /unconditional-call-forwardings` request body
/// (CAMARA `CreateCallForwardingSignal`): an optional `phoneNumber`, valid only
/// in two-legged auth.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCallForwardingSignal {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
}

/// `POST /call-forwarding-signal/v0.4/unconditional-call-forwardings`.
async fn unconditional_call_forwardings(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(UNCONDITIONAL_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: CreateCallForwardingSignal = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CreateCallForwardingSignal.",
                &correlator,
            )
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
    // Active marker: an odd trailing-three-digit tail → forwarding is on.
    let active = matches!(scenarios::trailing_three_digits(&identifier), Some(d) if d % 2 == 1);

    with_correlator(
        (StatusCode::OK, Json(json!({ "active": active }))).into_response(),
        &correlator,
    )
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules.
///
/// See the module docs for the four cases (`UNNECESSARY_IDENTIFIER`,
/// two-legged, three-legged, `MISSING_IDENTIFIER`).
fn resolve_identifier(
    req: &CreateCallForwardingSignal,
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

    const HOST: &str = "sim.local:8080";

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("+1234567890123456")); // too long
        assert!(!is_valid_e164("cfs-client")); // client-credentials subject
    }

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "cfs-client").await
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

    /// POST to `/unconditional-call-forwardings` with an optional Bearer token
    /// and optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_unconditional(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/call-forwarding-signal/v0.4/unconditional-call-forwardings")
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
    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(UNCONDITIONAL_SCOPE).await;
        post_unconditional(Some(&token), body, None).await
    }

    // --- Two-legged (submitted phoneNumber) success cases ------------------

    #[tokio::test]
    async fn even_tail_is_inactive() {
        // …012 (even) → no unconditional forwarding.
        let (status, _, body) = call_ok(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["active"], false);
    }

    #[tokio::test]
    async fn odd_tail_is_active() {
        // …013 (odd) → unconditional forwarding is on.
        let (status, _, body) = call_ok(r#"{"phoneNumber":"+123456789013"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["active"], true);
    }

    #[tokio::test]
    async fn zero_tail_is_inactive() {
        // …000 (even) → inactive.
        let (status, _, body) = call_ok(r#"{"phoneNumber":"+123456789000"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["active"], false);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = call_ok(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; subject is an E.164 line → drive from the subject.
        // …013 (odd) → active.
        let token = mint_token_with_client(UNCONDITIONAL_SCOPE, "+123456789013").await;
        let (status, _, body) = post_unconditional(Some(&token), r#"{}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["active"], true);
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(UNCONDITIONAL_SCOPE, "+123456789503").await;
        let (status, _, body) = post_unconditional(Some(&token), r#"{}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        // Subject is a line (three-legged) AND a phoneNumber is submitted → 422.
        let token = mint_token_with_client(UNCONDITIONAL_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_unconditional(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        // Two-legged token (sub = cfs-client) with no phoneNumber → can't identify.
        let (status, _, body) = call_ok(r#"{}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = call_ok(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(r#"{"phoneNumber":"+123456789012","x":1}"#).await;
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
        let (status, _, body) =
            post_unconditional(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_unconditional(None, r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(UNCONDITIONAL_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_unconditional(
            Some(&token),
            r#"{"phoneNumber":"+123456789012"}"#,
            Some("corr-cfs"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-cfs")
        );
        // Business error.
        let (status, headers, _) =
            post_unconditional(Some(&token), r#"{}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
