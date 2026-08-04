//! Device Swap **v1** (CAMARA Device Swap 1.0.0, r3.2).
//!
//! One endpoint (this slice):
//! - `POST /device-swap/v1/check` — has the device behind a phone number been
//!   swapped within the last `maxAge` hours?
//!
//! ## What it does
//!
//! The caller optionally submits a `phoneNumber` (two-legged auth only) and a
//! `maxAge` window in hours, and the operator answers
//! `{ "swapped": true|false }`: `true` when the device bound to the line was last
//! changed **within** the `maxAge` window, `false` otherwise. It is the device
//! counterpart of SIM Swap.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `device-swap:check` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by the
//! token **subject**, so resubmitting it is an error (mirrors Number Recycling /
//! Call Forwarding Signal):
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
//!   that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Swap recency (identifier digits × `maxAge`).** Otherwise the identifier's
//!   trailing three digits are read as **hours since the device was last swapped**
//!   (`0`–`999`), and `swapped = hoursAgo < maxAge` — the swap falls inside the
//!   queried window. This makes `maxAge` a genuine second control plane: the
//!   *same* number reads `swapped: true` under the default 240 h window
//!   (`+123456789012`, 12 h ago) but `swapped: false` for a device changed
//!   `+123456789365` (365 h ago) — unless the caller widens `maxAge` past 365. An
//!   identifier with no trailing digits (e.g. a `client_credentials` subject) is
//!   treated as never swapped → `swapped: false`.
//!
//! `maxAge` is validated: outside the CAMARA range `[1, 2400]` → `400
//! OUT_OF_RANGE`. When omitted it defaults to `240` hours.
//!
//! Example: `+123456789012` (device changed 12 h ago) → `swapped: true` at the
//! default window; the same number with `maxAge: 1` → `swapped: false`;
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

/// The OAuth2 scope the `POST /check` endpoint requires (CAMARA Device Swap
/// 1.0.0).
const CHECK_SCOPE: &str = "device-swap:check";

/// Default `maxAge` window in hours when the caller omits it (CAMARA default).
const DEFAULT_MAX_AGE: u16 = 240;
/// Inclusive `maxAge` bounds in hours (CAMARA Device Swap 1.0.0).
const MAX_AGE_MIN: u16 = 1;
const MAX_AGE_MAX: u16 = 2400;

/// Routes for Device Swap v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/device-swap/v1/check", post(check))
}

/// `POST /check` request body (CAMARA `CreateCheckDeviceSwap`): an optional
/// `phoneNumber` (valid only in two-legged auth) and an optional `maxAge` window
/// in hours. Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCheckDeviceSwap {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "maxAge")]
    max_age: Option<i64>,
}

/// `POST /device-swap/v1/check`.
async fn check(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CHECK_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: CreateCheckDeviceSwap = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CreateCheckDeviceSwap.",
                &correlator,
            )
        }
    };

    // `maxAge` must be within the CAMARA range → 400 OUT_OF_RANGE otherwise.
    let max_age = match req.max_age {
        None => DEFAULT_MAX_AGE,
        Some(v) if (MAX_AGE_MIN as i64..=MAX_AGE_MAX as i64).contains(&v) => v as u16,
        Some(_) => {
            return out_of_range("`maxAge` must be between 1 and 2400 hours.", &correlator)
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

    let swapped = is_swapped(&identifier, max_age);

    with_correlator(
        (StatusCode::OK, Json(json!({ "swapped": swapped }))).into_response(),
        &correlator,
    )
}

/// Whether the device behind `identifier` was swapped within the last `max_age`
/// hours. The identifier's trailing three digits are hours-since-the-last-swap
/// (docs/DESIGN.md §7); a swap inside the window means `hoursAgo < max_age`
/// (strict — a swap exactly `max_age` hours ago is on the window's edge and not
/// counted). An identifier without trailing digits is treated as never swapped.
fn is_swapped(identifier: &str, max_age: u16) -> bool {
    scenarios::trailing_three_digits(identifier).is_some_and(|hours_ago| hours_ago < max_age)
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Number Recycling). See the module docs for the four cases.
fn resolve_identifier(
    req: &CreateCheckDeviceSwap,
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

/// A 400 `OUT_OF_RANGE` CAMARA error (used for `maxAge` bounds), correlator echoed.
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

    const HOST: &str = "ds.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn swap_recency_is_driven_by_the_trailing_digits_and_max_age() {
        // …012 → swapped 12 h ago: inside the default 240 h window.
        assert!(is_swapped("+123456789012", DEFAULT_MAX_AGE));
        // …365 → swapped 365 h ago: outside the default window, inside a wide one.
        assert!(!is_swapped("+123456789365", DEFAULT_MAX_AGE));
        assert!(is_swapped("+123456789365", 2400));
        // Boundary: hoursAgo == maxAge is *not* inside the window (strict <).
        assert!(!is_swapped("+123456789240", 240));
        assert!(is_swapped("+123456789239", 240));
        // No trailing digits → never swapped.
        assert!(!is_swapped("ds-client", 2400));
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("ds-client")); // client-credentials subject
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "ds-client").await
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

    /// POST to `/check` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_check(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/device-swap/v1/check")
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
    async fn check_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CHECK_SCOPE).await;
        post_check(Some(&token), body, None).await
    }

    // --- Swap verdict: identifier digits × maxAge --------------------------

    #[tokio::test]
    async fn recently_swapped_number_is_true() {
        let (status, _, body) = check_ok(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], true);
    }

    #[tokio::test]
    async fn not_recently_swapped_number_is_false() {
        // …365 → 365 h ago, outside the default 240 h window.
        let (status, _, body) = check_ok(r#"{"phoneNumber":"+123456789365"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], false);
    }

    #[tokio::test]
    async fn max_age_widens_the_window() {
        // Same number that was false at default becomes true with a wider maxAge.
        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"+123456789365","maxAge":2400}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], true);
    }

    #[tokio::test]
    async fn max_age_narrows_the_window() {
        // …012 (12 h ago) is inside the default window but not a 1 h window —
        // maxAge is a real second control plane over the same number.
        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"+123456789012","maxAge":1}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], false);
    }

    #[tokio::test]
    async fn max_age_out_of_range_is_rejected() {
        for bad in ["0", "2401", "-5"] {
            let body = format!(r#"{{"phoneNumber":"+123456789012","maxAge":{bad}}}"#);
            let (status, _, json) = check_ok(&body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "maxAge {bad}");
            assert_eq!(json["code"], "OUT_OF_RANGE", "maxAge {bad}");
        }
    }

    #[tokio::test]
    async fn max_age_at_the_bounds_is_accepted() {
        for good in ["1", "2400"] {
            let body = format!(r#"{{"phoneNumber":"+123456789012","maxAge":{good}}}"#);
            let (status, _, _) = check_ok(&body).await;
            assert_eq!(status, StatusCode::OK, "maxAge {good}");
        }
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = check_ok(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = check_ok(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; subject is an E.164 line swapped 12 h ago → swapped.
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789012").await;
        let (status, _, body) = post_check(Some(&token), r#"{}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], true);
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789503").await;
        let (status, _, body) = post_check(Some(&token), r#"{}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_check(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = check_ok(r#"{}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = check_ok(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = check_ok(r#"{"phoneNumber":"+123456789012","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = check_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_check(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_check(None, r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CHECK_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_check(
            Some(&token),
            r#"{"phoneNumber":"+123456789012"}"#,
            Some("corr-ds"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ds")
        );
        // Business error.
        let (status, headers, _) =
            post_check(Some(&token), r#"{}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
