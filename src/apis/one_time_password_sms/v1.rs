//! One Time Password SMS **v1** (CAMARA one-time-password-sms 1.1.1).
//!
//! Two endpoints:
//! - `POST /one-time-password-sms/v1/send-code` — "send" an OTP to a phone
//!   number and return the `authenticationId` that identifies it.
//! - `POST /one-time-password-sms/v1/validate-code` — validate a `code` against
//!   a previously issued `authenticationId`.
//!
//! ## What it does
//!
//! This is CamaraSim's first **stateful** API. `send-code` mints an
//! `authenticationId`, remembers the code it "sent" for that id (in
//! [`super::store`]), and returns the id; `validate-code` redeems the id against
//! the submitted `code`. There is no real SMS — so the code is **deterministic
//! from the phone number** (the last six digits, zero-padded to six) so a
//! headless caller can compute exactly what to validate. Both endpoints require
//! the single CAMARA scope `one-time-password-sms:send-validate`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! - **`send-code`** is keyed on the submitted `phoneNumber`. If its trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`) the endpoint answers with that
//!   canonical CAMARA error ([`crate::scenarios`]); otherwise it issues an OTP
//!   whose code is `otp_code(phoneNumber)` and returns the `authenticationId`.
//! - **`validate-code`** is keyed on the live state behind the `authenticationId`:
//!   the matching code → `204`; a wrong code → `INVALID_OTP` until the attempt
//!   budget ([`super::store::MAX_ATTEMPTS`]) is exhausted, then `VERIFICATION_FAILED`;
//!   an unknown, already-consumed, or expired id → `VERIFICATION_EXPIRED`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::store::{self, Verdict};
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The single OAuth2 scope both OTP endpoints require (CAMARA 1.1.1).
const SCOPE: &str = "one-time-password-sms:send-validate";

/// Maximum length of the `message` (CAMARA `SendCodeRequest.message`).
const MESSAGE_MAX_LEN: usize = 160;
/// The mandatory placeholder the `message` must contain (CAMARA pattern).
const CODE_PLACEHOLDER: &str = "{{code}}";
/// Maximum length of a submitted `code` (CAMARA `ValidateCodeRequest.code`).
const CODE_MAX_LEN: usize = 10;

/// Routes for One Time Password SMS v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/one-time-password-sms/v1/send-code", post(send_code))
        .route("/one-time-password-sms/v1/validate-code", post(validate_code))
}

/// `POST /send-code` request body (CAMARA `SendCodeRequest`). Both fields are
/// required.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SendCodeRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    message: Option<String>,
}

/// `POST /one-time-password-sms/v1/send-code`.
async fn send_code(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory here (both fields required); parse strictly.
    let req: SendCodeRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid SendCodeRequest.", &correlator)
        }
    };

    // `phoneNumber` is required and must be E.164.
    let phone = match req.phone_number {
        Some(p) if is_valid_e164(&p) => p,
        Some(_) => {
            return invalid_argument(
                "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                &correlator,
            )
        }
        None => return invalid_argument("`phoneNumber` is required.", &correlator),
    };

    // `message` is required, at most 160 chars, and must contain `{{code}}`.
    match req.message {
        Some(m) if m.chars().count() > MESSAGE_MAX_LEN => {
            return invalid_argument("`message` must be at most 160 characters.", &correlator)
        }
        Some(m) if !m.contains(CODE_PLACEHOLDER) => {
            return invalid_argument(
                "`message` must contain the `{{code}}` placeholder.",
                &correlator,
            )
        }
        Some(_) => {}
        None => return invalid_argument("`message` is required.", &correlator),
    }

    // Reserved error suffix on the phone number selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&phone) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: "send" the deterministic code and return its id.
    let auth_id = store::issue(otp_code(&phone));

    with_correlator(
        (StatusCode::OK, Json(json!({ "authenticationId": auth_id }))).into_response(),
        &correlator,
    )
}

/// The deterministic OTP code the simulator "sends" for `phone`: the last six
/// digits of the number, right-aligned and zero-padded to six. Documented in the
/// spec so a headless caller can compute what to validate (e.g. `+123456789012`
/// → `789012`).
fn otp_code(phone: &str) -> String {
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    let start = digits.len().saturating_sub(6);
    format!("{:0>6}", &digits[start..])
}

/// `POST /validate-code` request body (CAMARA `ValidateCodeRequest`). Both fields
/// are required.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidateCodeRequest {
    #[serde(rename = "authenticationId")]
    authentication_id: Option<String>,
    code: Option<String>,
}

/// `POST /one-time-password-sms/v1/validate-code`.
async fn validate_code(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: ValidateCodeRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid ValidateCodeRequest.",
                &correlator,
            )
        }
    };

    let auth_id = match req.authentication_id {
        Some(id) if !id.is_empty() => id,
        _ => return invalid_argument("`authenticationId` is required.", &correlator),
    };
    let code = match req.code {
        Some(c) if c.is_empty() || c.chars().count() > CODE_MAX_LEN => {
            return invalid_argument("`code` must be 1 to 10 characters.", &correlator)
        }
        Some(c) => c,
        None => return invalid_argument("`code` is required.", &correlator),
    };

    // Redeem against the live state; map the verdict onto the CAMARA OTP errors.
    match store::validate(&auth_id, &code) {
        Verdict::Ok => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        Verdict::InvalidOtp => otp_error(
            "ONE_TIME_PASSWORD_SMS.INVALID_OTP",
            "The provided OTP is not valid for this authenticationId.",
            &correlator,
        ),
        Verdict::Failed => otp_error(
            "ONE_TIME_PASSWORD_SMS.VERIFICATION_FAILED",
            "The maximum number of validation attempts has been exceeded.",
            &correlator,
        ),
        Verdict::Expired => otp_error(
            "ONE_TIME_PASSWORD_SMS.VERIFICATION_EXPIRED",
            "The authenticationId is unknown or has expired.",
            &correlator,
        ),
    }
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 400 CAMARA error carrying an API-specific `ONE_TIME_PASSWORD_SMS.*` code.
fn otp_error(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, code, message).into_response(),
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

    const HOST: &str = "otp.local:8080";
    const SEND: &str = "/one-time-password-sms/v1/send-code";
    const VALIDATE: &str = "/one-time-password-sms/v1/validate-code";
    /// A well-formed message carrying the mandatory placeholder.
    const MSG: &str = "Your code is {{code}}";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn otp_code_is_the_zero_padded_last_six_digits() {
        assert_eq!(otp_code("+123456789012"), "789012");
        assert_eq!(otp_code("+12345"), "012345"); // fewer than 6 digits → padded
        assert_eq!(otp_code("+123456"), "123456");
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("+1234567890123456")); // too long
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
        let body = format!("grant_type=client_credentials&client_id=otp-client&scope={scope}");
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

    /// POST a JSON body to `path` with an optional Bearer token and `x-correlator`.
    /// Returns status, headers and the parsed JSON body (`Null` when empty, e.g. 204).
    async fn post_json(
        path: &str,
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(path)
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

    /// Send a code for `phone` with a valid token and return the `authenticationId`.
    async fn send_ok(phone: &str) -> String {
        let token = mint_token(SCOPE).await;
        let body = json!({ "phoneNumber": phone, "message": MSG }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK, "send-code should succeed for {phone}");
        json["authenticationId"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn send_code_returns_an_authentication_id() {
        let id = send_ok("+123456789012").await;
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn full_send_then_validate_happy_path() {
        // The code is deterministic from the phone number.
        let phone = "+123456789012";
        let id = send_ok(phone).await;
        let token = mint_token(SCOPE).await;
        let body = json!({ "authenticationId": id, "code": otp_code(phone) }).to_string();
        let (status, _, _) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn validate_is_single_use() {
        let phone = "+123456789012";
        let id = send_ok(phone).await;
        let token = mint_token(SCOPE).await;
        let body = json!({ "authenticationId": id, "code": otp_code(phone) }).to_string();
        let (first, _, _) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(first, StatusCode::NO_CONTENT);
        // Replaying the consumed id → VERIFICATION_EXPIRED.
        let (second, _, json) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(second, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "ONE_TIME_PASSWORD_SMS.VERIFICATION_EXPIRED");
    }

    #[tokio::test]
    async fn wrong_code_is_invalid_then_verification_failed() {
        let id = send_ok("+123456789012").await;
        let token = mint_token(SCOPE).await;
        let body = json!({ "authenticationId": id, "code": "000000" }).to_string();
        // First two wrong tries → INVALID_OTP (MAX_ATTEMPTS is 3).
        for _ in 0..(store::MAX_ATTEMPTS - 1) {
            let (status, _, json) = post_json(VALIDATE, Some(&token), &body, None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(json["code"], "ONE_TIME_PASSWORD_SMS.INVALID_OTP");
        }
        // The attempt that exhausts the budget → VERIFICATION_FAILED.
        let (status, _, json) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "ONE_TIME_PASSWORD_SMS.VERIFICATION_FAILED");
    }

    #[tokio::test]
    async fn validate_unknown_id_is_verification_expired() {
        let token = mint_token(SCOPE).await;
        let body = json!({ "authenticationId": "no-such-id", "code": "123456" }).to_string();
        let (status, _, json) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "ONE_TIME_PASSWORD_SMS.VERIFICATION_EXPIRED");
    }

    #[tokio::test]
    async fn send_code_reserved_suffix_selects_a_canonical_camara_error() {
        let token = mint_token(SCOPE).await;
        let body = json!({ "phoneNumber": "+123456789404", "message": MSG }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["code"], "NOT_FOUND");

        let body = json!({ "phoneNumber": "+123456789429", "message": MSG }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(json["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn send_code_missing_or_bad_phone_number_is_rejected() {
        let token = mint_token(SCOPE).await;
        // Missing phoneNumber.
        let (status, _, json) =
            post_json(SEND, Some(&token), &json!({ "message": MSG }).to_string(), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
        // Bad E.164.
        let body = json!({ "phoneNumber": "0123", "message": MSG }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn send_code_message_must_carry_the_placeholder_and_fit() {
        let token = mint_token(SCOPE).await;
        // Missing {{code}} placeholder.
        let body = json!({ "phoneNumber": "+123456789012", "message": "no placeholder" })
            .to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
        // Over 160 characters.
        let long = format!("{}{{{{code}}}}", "x".repeat(160));
        let body = json!({ "phoneNumber": "+123456789012", "message": long }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn send_code_unknown_field_is_rejected() {
        let token = mint_token(SCOPE).await;
        let body =
            json!({ "phoneNumber": "+123456789012", "message": MSG, "x": 1 }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn validate_code_missing_fields_or_too_long_code_is_rejected() {
        let token = mint_token(SCOPE).await;
        // Missing code.
        let body = json!({ "authenticationId": "some-id" }).to_string();
        let (status, _, json) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
        // Code longer than 10 chars.
        let body = json!({ "authenticationId": "some-id", "code": "12345678901" }).to_string();
        let (status, _, json) = post_json(VALIDATE, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let body = json!({ "phoneNumber": "+123456789012", "message": MSG }).to_string();
        let (status, _, json) = post_json(SEND, Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "PERMISSION_DENIED");

        let vbody = json!({ "authenticationId": "id", "code": "123456" }).to_string();
        let (status, _, json) = post_json(VALIDATE, Some(&token), &vbody, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let body = json!({ "phoneNumber": "+123456789012", "message": MSG }).to_string();
        let (status, _, json) = post_json(SEND, None, &body, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_send_success_and_validate_204() {
        let phone = "+123456789012";
        // send-code success echoes the correlator.
        let token = mint_token(SCOPE).await;
        let body = json!({ "phoneNumber": phone, "message": MSG }).to_string();
        let (status, headers, json) =
            post_json(SEND, Some(&token), &body, Some("corr-send")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-send")
        );
        // validate-code 204 (no body) still echoes the correlator.
        let id = json["authenticationId"].as_str().unwrap();
        let vbody = json!({ "authenticationId": id, "code": otp_code(phone) }).to_string();
        let (status, headers, _) =
            post_json(VALIDATE, Some(&token), &vbody, Some("corr-val")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-val")
        );
    }
}
