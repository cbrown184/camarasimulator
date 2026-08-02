//! SIM Swap **v2** (CAMARA SIM Swap 2.0.0).
//!
//! One endpoint so far:
//! - `POST /sim-swap/v2/check` — has the SIM behind a phone number been swapped
//!   within the last `maxAge` hours?
//!
//! ## What it does
//!
//! The caller asks whether the SIM card bound to a phone number was swapped
//! within a recent window. The number is supplied either in the body
//! (`phoneNumber`, E.164) or — when omitted — taken from the identity the access
//! token authenticated (a three-legged token), matching the two ways CAMARA
//! SIM Swap 2.0.0 identifies the device. The answer is
//! `{ "swapped": true|false }`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `sim-swap:check` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The identifier is the `phoneNumber` when supplied, otherwise the access
//! token's subject (`sub`). The result is chosen deterministically from it:
//!
//! - **Reserved error suffix** — if the identifier's trailing three digits name a
//!   reserved CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//!   `…429`, `…500`, `…503`), the endpoint answers with that canonical CAMARA
//!   error instead of a result (shared convention, [`crate::scenarios`]).
//! - **Recency of the swap** — otherwise the identifier's trailing three digits
//!   encode **how many hours ago** the SIM was last swapped (`000`–`999`). The
//!   endpoint reports `swapped = hoursAgo < maxAge`, i.e. the swap falls inside
//!   the queried window. So `maxAge` is a real second control plane:
//!   `+123456789012` (12 h ago) is `swapped: true` under the default window
//!   (240 h) but `+123456789365` (365 h ago) is `swapped: false` — unless the
//!   caller widens `maxAge` past 365.
//! - **No digits** — an identifier with no trailing digits (e.g. the synthetic
//!   `camarasim-user` subject) is treated as never swapped → `swapped: false`.

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

/// The OAuth2 scope the `POST /check` endpoint requires (CAMARA SIM Swap 2.0.0).
const CHECK_SCOPE: &str = "sim-swap:check";

/// Default `maxAge` window in hours when the caller omits it (CAMARA default).
const DEFAULT_MAX_AGE: u16 = 240;
/// Inclusive `maxAge` bounds in hours (CAMARA SIM Swap 2.0.0).
const MAX_AGE_MIN: u16 = 1;
const MAX_AGE_MAX: u16 = 2400;

/// Routes for SIM Swap v2, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/sim-swap/v2/check", post(check))
}

/// `POST /check` request body (CAMARA `CreateCheckSimSwap`). Both fields are
/// optional: `phoneNumber` may be omitted when a three-legged token identifies
/// the device, and `maxAge` defaults to 240 hours.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "maxAge")]
    max_age: Option<i64>,
}

/// `POST /sim-swap/v2/check`.
async fn check(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CHECK_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (all fields optional); anything present must parse.
    let req: CheckRequest = if body.is_empty() {
        CheckRequest { phone_number: None, max_age: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid CreateCheckSimSwap.",
                    &correlator,
                )
            }
        }
    };

    // `maxAge` must be within the CAMARA range → 400 OUT_OF_RANGE otherwise.
    let max_age = match req.max_age {
        None => DEFAULT_MAX_AGE,
        Some(v) if (MAX_AGE_MIN as i64..=MAX_AGE_MAX as i64).contains(&v) => v as u16,
        Some(_) => {
            return out_of_range(
                "`maxAge` must be between 1 and 2400 hours.",
                &correlator,
            )
        }
    };

    // The identifier is the submitted phoneNumber, else the token subject
    // (three-legged token). Missing both → 422 MISSING_IDENTIFIER.
    let identifier = match req.phone_number {
        Some(phone) => {
            if !is_valid_e164(&phone) {
                return invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    &correlator,
                );
            }
            phone
        }
        None => {
            let subject = claims.subject().unwrap_or("");
            if subject.is_empty() {
                return with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "No `phoneNumber` supplied and the access token identifies no device.",
                    )
                    .into_response(),
                    &correlator,
                );
            }
            subject.to_string()
        }
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let swapped = is_swapped(&identifier, max_age);

    with_correlator(
        (StatusCode::OK, Json(json!({ "swapped": swapped }))).into_response(),
        &correlator,
    )
}

/// Whether the SIM behind `identifier` was swapped within the last `max_age`
/// hours. The identifier's trailing three digits encode how many hours ago the
/// last swap happened; an identifier with no trailing digits is treated as never
/// swapped (docs/DESIGN.md §7).
fn is_swapped(identifier: &str, max_age: u16) -> bool {
    scenarios::trailing_three_digits(identifier).is_some_and(|hours_ago| hours_ago < max_age)
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

    const HOST: &str = "sim.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn swap_recency_is_driven_by_the_trailing_digits_and_max_age() {
        // …012 → swapped 12 h ago: inside the default 240 h window.
        assert!(is_swapped("+123456789012", DEFAULT_MAX_AGE));
        // …365 → 365 h ago: outside the default window, inside a wider one.
        assert!(!is_swapped("+123456789365", DEFAULT_MAX_AGE));
        assert!(is_swapped("+123456789365", 2400));
        // Boundary: hoursAgo == maxAge is *not* inside the window (strict <).
        assert!(!is_swapped("+123456789240", 240));
        assert!(is_swapped("+123456789239", 240));
        // No trailing digits → never swapped.
        assert!(!is_swapped("camarasim-user", 2400));
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
        mint_token_with_client(scope, "ss-client").await
    }

    /// As [`mint_token`], but with a caller-chosen `client_id` — which becomes
    /// the token `sub`. Used to drive the subject-keyed (no-phoneNumber) cases.
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

    /// POST a body to `/check` with an optional Bearer token and `x-correlator`.
    async fn post_check(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/sim-swap/v2/check")
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

    /// Mint a scoped token and call check with the given body.
    async fn check_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CHECK_SCOPE).await;
        post_check(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn recently_swapped_number_is_true() {
        let (status, _, body) = check_ok_token(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], true);
    }

    #[tokio::test]
    async fn not_recently_swapped_number_is_false() {
        // …365 h ago is outside the default 240 h window.
        let (status, _, body) = check_ok_token(r#"{"phoneNumber":"+123456789365"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], false);
    }

    #[tokio::test]
    async fn max_age_widens_the_window() {
        // Same number that was false at default becomes true with a wider maxAge.
        let (status, _, body) =
            check_ok_token(r#"{"phoneNumber":"+123456789365","maxAge":2400}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], true);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = check_ok_token(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = check_ok_token(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn max_age_out_of_range_is_rejected() {
        for bad in ["0", "2401"] {
            let body = format!(r#"{{"phoneNumber":"+123456789012","maxAge":{bad}}}"#);
            let (status, _, json) = check_ok_token(&body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "maxAge {bad}");
            assert_eq!(json["code"], "OUT_OF_RANGE", "maxAge {bad}");
        }
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = check_ok_token(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            check_ok_token(r#"{"phoneNumber":"+123456789012","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_phone_number_falls_back_to_the_token_subject() {
        // Subject is an E.164 number with a recent-swap tail → swapped true,
        // with an empty body (all fields optional).
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789012").await;
        let (status, _, body) = post_check(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], true);
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789503").await;
        let (status, _, body) = post_check(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn non_numeric_subject_without_phone_number_is_not_swapped() {
        // Default synthetic subject "ss-client" has no digits → never swapped.
        let (status, _, body) = check_ok_token("{}").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["swapped"], false);
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
        let (status, headers, _) = post_check(
            Some(&token),
            r#"{"phoneNumber":"+123456789012"}"#,
            Some("corr-ss"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ss")
        );

        let (status, headers, _) =
            post_check(Some(&token), r#"{"phoneNumber":"0123"}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
