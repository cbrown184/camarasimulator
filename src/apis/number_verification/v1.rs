//! Number Verification **v1** (CAMARA Number Verification 1.0.0).
//!
//! Two endpoints:
//! - `POST /number-verification/v1/verify` — silently confirm a submitted
//!   number matches the device's own number.
//! - `GET /number-verification/v1/device-phone-number` — return the device's
//!   own number (see [`device_phone_number`]).
//!
//! ## What it does
//!
//! The caller submits the phone number it believes belongs to the user, either
//! in the clear (`phoneNumber`, E.164) or as its SHA-256 hash
//! (`hashedPhoneNumber`) — exactly one of the two. The operator compares it
//! against the number of the device the access token authenticated, and answers
//! `{ "devicePhoneNumberVerified": true|false }`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `number-verification:verify`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The result is chosen deterministically from the submitted `phoneNumber`:
//!
//! - **Reserved error suffix** — if the number's trailing three digits name a
//!   reserved CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//!   `…429`, `…500`, `…503`), the endpoint answers with that canonical CAMARA
//!   error instead of a verification result (shared convention,
//!   [`crate::scenarios`]).
//! - **No-match marker** — a number whose trailing three digits are `000`
//!   verifies `false` (the submitted number does *not* match the device).
//! - **Happy path** — any other number verifies `true`.
//!
//! A `hashedPhoneNumber` cannot be reversed to read its trailing digits, so it
//! cannot select the error or no-match cases: it always verifies `true`. Drive
//! the error / no-match cases with the clear-text `phoneNumber` form.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /verify` endpoint requires (CAMARA NV 1.0.0).
const VERIFY_SCOPE: &str = "number-verification:verify";

/// The OAuth2 scope the `GET /device-phone-number` endpoint requires (CAMARA NV 1.0.0).
const DEVICE_PHONE_NUMBER_SCOPE: &str = "number-verification:device-phone-number:read";

/// The simulator's default device line, returned by `GET /device-phone-number`
/// when the authenticated subject is not itself a phone identifier — e.g. the
/// synthetic `camarasim-user` minted by the `authorization_code`/CIBA flows.
const DEFAULT_DEVICE_NUMBER: &str = "+123456789012";

/// Routes for Number Verification v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/number-verification/v1/verify", post(verify))
        .route(
            "/number-verification/v1/device-phone-number",
            get(device_phone_number),
        )
}

/// `POST /verify` request body: exactly one of `phoneNumber` / `hashedPhoneNumber`
/// (CAMARA `NumberVerificationRequestBody`, `minProperties`/`maxProperties` 1).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "hashedPhoneNumber")]
    hashed_phone_number: Option<String>,
}

/// `POST /number-verification/v1/verify`.
async fn verify(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(VERIFY_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: VerifyRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid NumberVerificationRequestBody.",
                &correlator,
            )
        }
    };

    let verified = match (req.phone_number, req.hashed_phone_number) {
        // Exactly one identifier must be supplied.
        (None, None) => {
            return invalid_argument(
                "Exactly one of `phoneNumber` or `hashedPhoneNumber` is required.",
                &correlator,
            )
        }
        (Some(_), Some(_)) => {
            return invalid_argument(
                "Provide only one of `phoneNumber` or `hashedPhoneNumber`.",
                &correlator,
            )
        }

        // Clear-text number: the control plane for the functional cases.
        (Some(phone), None) => {
            if !is_valid_e164(&phone) {
                return invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    &correlator,
                );
            }
            if let Some(err) = scenarios::reserved_error(&phone) {
                return with_correlator(err.into_response(), &correlator);
            }
            // No-match marker: trailing three digits `000` → not verified.
            scenarios::trailing_three_digits(&phone) != Some(0)
        }

        // Hashed number: can't be reversed to select a case, so happy path only.
        (None, Some(hash)) => {
            if hash.trim().is_empty() {
                return invalid_argument("`hashedPhoneNumber` must not be empty.", &correlator);
            }
            true
        }
    };

    with_correlator(
        (StatusCode::OK, Json(json!({ "devicePhoneNumberVerified": verified }))).into_response(),
        &correlator,
    )
}

/// `GET /number-verification/v1/device-phone-number`.
///
/// Returns the phone number of the device the access token authenticated
/// (`NumberVerificationShareResponse`). This endpoint has no request body, so
/// its functional cases (docs/DESIGN.md §7) are driven by the token **subject**
/// (`sub`) — the identity the token was issued for:
///
/// - **Reserved error suffix** — if the subject's trailing three digits name a
///   reserved CAMARA status (shared convention, [`crate::scenarios`]), the
///   endpoint answers with that canonical CAMARA error instead of a number.
/// - **E.164 subject** — a subject that is itself a valid phone number *is* the
///   device's number and is returned verbatim (as in a line-authenticated
///   three-legged token).
/// - **Default** — any other subject (e.g. the synthetic `camarasim-user` from
///   an `authorization_code`/CIBA flow) yields the simulator's default line.
async fn device_phone_number(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's read scope.
    if let Err(e) = claims.require_scope(DEVICE_PHONE_NUMBER_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the control plane (docs/DESIGN.md §7).
    let subject = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(subject) {
        return with_correlator(err.into_response(), &correlator);
    }
    let number = if is_valid_e164(subject) {
        subject
    } else {
        DEFAULT_DEVICE_NUMBER
    };

    with_correlator(
        (StatusCode::OK, Json(json!({ "devicePhoneNumber": number }))).into_response(),
        &correlator,
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

    // --- Pure validator / marker units -------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        // Valid: leading +, non-zero first digit, 5–15 digits total.
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789"));
        assert!(is_valid_e164("+123456789012345"));
        // Invalid.
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short (4 digits)
        assert!(!is_valid_e164("+1234567890123456")); // too long (16 digits)
        assert!(!is_valid_e164("+12 34 56")); // non-digit
        assert!(!is_valid_e164("+")); // empty
    }

    // --- Integration through the real router -------------------------------

    /// App with the auth routes (token endpoint) and the Number Verification
    /// routes, so a real token can be minted and presented.
    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the verify route's audience. Scope is granted verbatim.
    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "nv-client").await
    }

    /// As [`mint_token`], but with a caller-chosen `client_id` — which becomes
    /// the token `sub`/`client_id`. Used to drive the device-phone-number
    /// endpoint's subject-keyed functional cases. `+` is percent-encoded so an
    /// E.164 client id survives the urlencoded body.
    async fn mint_token_with_client(scope: &str, client_id: &str) -> String {
        let enc = client_id.replace('+', "%2B");
        let body =
            format!("grant_type=client_credentials&client_id={enc}&scope={scope}");
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

    /// POST a body to `/verify` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_verify(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/number-verification/v1/verify")
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

    /// Mint a scoped token and call verify with the given body.
    async fn verify_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(VERIFY_SCOPE).await;
        post_verify(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn happy_path_phone_number_verifies_true() {
        let (status, _, body) = verify_ok_token(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["devicePhoneNumberVerified"], true);
    }

    #[tokio::test]
    async fn no_match_marker_verifies_false() {
        // Trailing three digits `000` → the submitted number does not match.
        let (status, _, body) = verify_ok_token(r#"{"phoneNumber":"+123456789000"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["devicePhoneNumberVerified"], false);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        // …404 → 404 NOT_FOUND, …403 → 403 PERMISSION_DENIED.
        let (status, _, body) = verify_ok_token(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], 404);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = verify_ok_token(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn hashed_phone_number_verifies_true() {
        let body = r#"{"hashedPhoneNumber":"32f67ab4e4e2c95cf81b2e8b7cf7d78f27a5a4e5f2a8f3c1b0d9e6a7b8c9d0e1"}"#;
        let (status, _, json) = verify_ok_token(body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["devicePhoneNumberVerified"], true);
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = verify_ok_token(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_identifier_is_rejected() {
        let (status, _, body) = verify_ok_token(r#"{}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn both_identifiers_is_rejected() {
        let body = r#"{"phoneNumber":"+123456789012","hashedPhoneNumber":"abc"}"#;
        let (status, _, json) = verify_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = verify_ok_token(r#"{"phoneNumber":"+123456789012","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = verify_ok_token("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_verify(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_verify(None, r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- GET /device-phone-number -----------------------------------------

    /// GET `/device-phone-number` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_device_phone_number(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/number-verification/v1/device-phone-number")
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
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn device_phone_number_defaults_when_subject_is_not_a_number() {
        // sub = client_id "nv-client" (not E.164) → the simulator's default line.
        let token = mint_token(DEVICE_PHONE_NUMBER_SCOPE).await;
        let (status, _, body) = get_device_phone_number(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["devicePhoneNumber"], DEFAULT_DEVICE_NUMBER);
    }

    #[tokio::test]
    async fn device_phone_number_echoes_an_e164_subject() {
        // sub = an E.164 client id → that number is the device's own number.
        let token = mint_token_with_client(DEVICE_PHONE_NUMBER_SCOPE, "+123456789012").await;
        let (status, _, body) = get_device_phone_number(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["devicePhoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn device_phone_number_reserved_suffix_selects_a_camara_error() {
        // A subject whose trailing three digits name a reserved status → that error.
        let token = mint_token_with_client(DEVICE_PHONE_NUMBER_SCOPE, "+123456789404").await;
        let (status, _, body) = get_device_phone_number(Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let token = mint_token_with_client(DEVICE_PHONE_NUMBER_SCOPE, "user-503").await;
        let (status, _, body) = get_device_phone_number(Some(&token), None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn device_phone_number_without_the_scope_is_forbidden() {
        let token = mint_token(VERIFY_SCOPE).await; // wrong scope for this endpoint
        let (status, _, body) = get_device_phone_number(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn device_phone_number_missing_token_is_unauthenticated() {
        let (status, _, body) = get_device_phone_number(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn device_phone_number_echoes_x_correlator() {
        let token = mint_token(DEVICE_PHONE_NUMBER_SCOPE).await;
        let (status, headers, _) =
            get_device_phone_number(Some(&token), Some("corr-dpn")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-dpn")
        );
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(VERIFY_SCOPE).await;
        // Success.
        let (status, headers, _) =
            post_verify(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, Some("corr-123")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-123")
        );
        // Business error.
        let (status, headers, _) =
            post_verify(Some(&token), r#"{}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
