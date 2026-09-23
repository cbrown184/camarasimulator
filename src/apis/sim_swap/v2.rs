//! SIM Swap **v2** (CAMARA SIM Swap 2.0.0).
//!
//! Two endpoints so far:
//! - `POST /sim-swap/v2/check` — has the SIM behind a phone number been swapped
//!   within the last `maxAge` hours?
//! - `POST /sim-swap/v2/retrieve-date` — when was the SIM behind a phone number
//!   last swapped?
//!
//! ## What it does
//!
//! The caller asks about the SIM card bound to a phone number. The number is
//! supplied either in the body (`phoneNumber`, E.164) or — when omitted — taken
//! from the identity the access token authenticated (a three-legged token),
//! matching the two ways CAMARA SIM Swap 2.0.0 identifies the device. `check`
//! answers `{ "swapped": true|false }`; `retrieve-date` answers with the
//! timestamp of the last swap (`{ "latestSimChange": …, "monitoredPeriod": … }`).
//!
//! Both endpoints are protected: they require a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the endpoint's scope
//! (`sim-swap:check` / `sim-swap:retrieve-date`).
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The identifier is the `phoneNumber` when supplied, otherwise the access
//! token's subject (`sub`). Its trailing three digits encode **how many hours
//! ago** the SIM was last swapped (`000`–`999`) — one coherent swap history that
//! both endpoints read the same way:
//!
//! - **Reserved error suffix** — if those trailing three digits name a reserved
//!   CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`,
//!   `…500`, `…503`), the endpoint answers with that canonical CAMARA error
//!   instead of a result (shared convention, [`crate::scenarios`]).
//! - **`check`** reports `swapped = hoursAgo < maxAge`, i.e. the swap falls inside
//!   the queried window. So `maxAge` is a real second control plane:
//!   `+123456789012` (12 h ago) is `swapped: true` under the default window
//!   (240 h) but `+123456789365` (365 h ago) is `swapped: false` — unless the
//!   caller widens `maxAge` past 365.
//! - **`retrieve-date`** reports `latestSimChange` = *now − hoursAgo hours* when
//!   the swap is inside the fixed monitored period ([`MONITORED_PERIOD_HOURS`],
//!   240 h = 10 days), or `null` when the last swap is older than that period or
//!   the identifier carries no digits; `monitoredPeriod` (10 days) is always
//!   reported. The 240 h boundary lines up with `check`'s default window, so
//!   `+123456789012` yields a non-null date and `+123456789365` yields `null`.
//! - **No digits** — an identifier with no trailing digits (e.g. the synthetic
//!   `camarasim-user` subject) is treated as never swapped → `check` false,
//!   `retrieve-date` `null`.

use std::time::{SystemTime, UNIX_EPOCH};

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
/// The OAuth2 scope the `POST /retrieve-date` endpoint requires (CAMARA SIM Swap 2.0.0).
const RETRIEVE_DATE_SCOPE: &str = "sim-swap:retrieve-date";

/// Default `maxAge` window in hours when the caller omits it (CAMARA default).
const DEFAULT_MAX_AGE: u16 = 240;
/// Inclusive `maxAge` bounds in hours (CAMARA SIM Swap 2.0.0).
const MAX_AGE_MIN: u16 = 1;
const MAX_AGE_MAX: u16 = 2400;

/// The simulator's fixed SIM-swap supervision window, in **hours**. A swap older
/// than this is beyond what the operator monitors, so `retrieve-date` reports
/// `latestSimChange: null`. Chosen to equal `check`'s default `maxAge` (240 h)
/// so the two endpoints tell one coherent story.
const MONITORED_PERIOD_HOURS: u16 = 240;
/// The same monitored window expressed in **days** (CAMARA `monitoredPeriod` unit).
const MONITORED_PERIOD_DAYS: u16 = MONITORED_PERIOD_HOURS / 24;

/// Routes for SIM Swap v2, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/sim-swap/v2/check", post(check))
        .route("/sim-swap/v2/retrieve-date", post(retrieve_date))
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
    let identifier = match resolve_identifier(req.phone_number, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
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

/// `POST /retrieve-date` request body (CAMARA `CreateSimSwapDate`). `phoneNumber`
/// is optional — omit it when a three-legged token identifies the device. Unlike
/// `check`, there is no `maxAge` control plane.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveDateRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
}

/// `POST /sim-swap/v2/retrieve-date` — the timestamp of the last SIM swap.
async fn retrieve_date(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(RETRIEVE_DATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (phoneNumber optional); anything present must parse.
    let req: RetrieveDateRequest = if body.is_empty() {
        RetrieveDateRequest { phone_number: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid CreateSimSwapDate.",
                    &correlator,
                )
            }
        }
    };

    let identifier = match resolve_identifier(req.phone_number, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // `latestSimChange` is `null` when the last swap is outside the monitored
    // period (or the identifier has no digits); `serde_json` maps `None` to JSON
    // `null`. `monitoredPeriod` (days) is always reported.
    let latest_sim_change = last_swap_timestamp(&identifier);

    with_correlator(
        (
            StatusCode::OK,
            Json(json!({
                "latestSimChange": latest_sim_change,
                "monitoredPeriod": MONITORED_PERIOD_DAYS,
            })),
        )
            .into_response(),
        &correlator,
    )
}

/// The timestamp of the last SIM swap behind `identifier`, as RFC 3339 UTC, or
/// `None` when the last swap is older than the monitored period ([`MONITORED_PERIOD_HOURS`])
/// or the identifier carries no digits. The trailing three digits encode
/// hours-since-swap (docs/DESIGN.md §7), so the swap timestamp is *now* minus
/// that many hours — consistent with `check`'s recency semantics.
fn last_swap_timestamp(identifier: &str) -> Option<String> {
    let hours_ago = scenarios::trailing_three_digits(identifier)?;
    if hours_ago >= MONITORED_PERIOD_HOURS {
        return None;
    }
    Some(rfc3339_utc(now_unix_secs() - hours_ago as i64 * 3600))
}

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as RFC 3339, e.g.
/// `2026-08-02T14:27:08Z`. Second precision — the CAMARA schema requires RFC
/// 3339 with a time zone but not sub-second digits. Self-contained (no date-time
/// dependency) via the civil-from-days algorithm below.
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a day count since 1970-01-01 into a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for any date
/// in `i64` range). Month and day are 1-based.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day-of-era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp = (5 * doy + 2) / 153; // month, shifted so March = 0 [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Resolve the device identifier for a request: the validated `phoneNumber` when
/// supplied, else the token subject (three-legged fallback). On failure returns
/// the CAMARA error `Response` to send — 400 `INVALID_ARGUMENT` for a malformed
/// `phoneNumber`, 422 `MISSING_IDENTIFIER` when neither is present.
fn resolve_identifier(
    phone_number: Option<String>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    match phone_number {
        Some(phone) => {
            if !is_valid_e164(&phone) {
                return Err(invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    correlator,
                ));
            }
            Ok(phone)
        }
        None => {
            let subject = claims.subject().unwrap_or("");
            if subject.is_empty() {
                return Err(with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "No `phoneNumber` supplied and the access token identifies no device.",
                    )
                    .into_response(),
                    correlator,
                ));
            }
            Ok(subject.to_string())
        }
    }
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
        post_json("/sim-swap/v2/check", token, body, correlator).await
    }

    /// POST a body to `/retrieve-date` with an optional Bearer token and `x-correlator`.
    async fn post_retrieve_date(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        post_json("/sim-swap/v2/retrieve-date", token, body, correlator).await
    }

    /// POST a JSON body to `path` with an optional Bearer token and `x-correlator`.
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

    // --- retrieve-date: pure time helpers ----------------------------------

    #[test]
    fn rfc3339_utc_formats_known_epochs() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(946_684_800), "2000-01-01T00:00:00Z");
        // 2000 is a leap year: the 60th day (index 59) is Feb 29.
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(1_704_067_200), "2024-01-01T00:00:00Z");
        // Time-of-day components.
        assert_eq!(rfc3339_utc(1_704_067_200 + 14 * 3600 + 27 * 60 + 8), "2024-01-01T14:27:08Z");
    }

    #[test]
    fn civil_from_days_handles_leap_boundaries() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11016), (2000, 2, 29)); // 951_782_400 / 86_400
        assert_eq!(civil_from_days(19723), (2024, 1, 1)); // 1_704_067_200 / 86_400
    }

    #[test]
    fn last_swap_timestamp_is_none_outside_the_monitored_period() {
        // hoursAgo == monitored period is not inside it (strict <).
        assert!(last_swap_timestamp("+123456789240").is_none());
        // 365 h ago is well outside the 240 h window.
        assert!(last_swap_timestamp("+123456789365").is_none());
        // No trailing digits → never swapped.
        assert!(last_swap_timestamp("camarasim-user").is_none());
    }

    #[test]
    fn last_swap_timestamp_is_now_minus_hours_ago() {
        // hoursAgo == 0 → the swap is "now". Bracket the wall clock so the
        // assertion is robust across a second/minute tick.
        let before = now_unix_secs();
        let ts = last_swap_timestamp("+123456789000").expect("inside the monitored period");
        let after = now_unix_secs();
        assert!(
            ts == rfc3339_utc(before) || ts == rfc3339_utc(after),
            "…000 should be ~now, got {ts}"
        );
        // More hours ago → strictly earlier timestamp. Fixed-width RFC 3339 UTC
        // strings sort chronologically, so a lexical compare is a time compare.
        let recent = last_swap_timestamp("+123456789006").unwrap(); // 6 h ago
        let older = last_swap_timestamp("+123456789012").unwrap(); // 12 h ago
        assert!(older < recent, "12 h ago ({older}) should precede 6 h ago ({recent})");
    }

    // --- retrieve-date: integration through the real router ----------------

    /// Mint a `retrieve-date`-scoped token and call the endpoint.
    async fn retrieve_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_DATE_SCOPE).await;
        post_retrieve_date(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn recent_swap_returns_a_timestamp_and_monitored_period() {
        let (status, _, body) = retrieve_ok_token(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::OK);
        let ts = body["latestSimChange"].as_str().expect("a non-null timestamp");
        assert!(ts.ends_with('Z') && ts.contains('T'), "RFC 3339 UTC, got {ts}");
        assert_eq!(body["monitoredPeriod"], MONITORED_PERIOD_DAYS);
    }

    #[tokio::test]
    async fn swap_older_than_monitored_period_is_null() {
        // …365 h ago is outside the 240 h monitored window → latestSimChange null.
        let (status, _, body) = retrieve_ok_token(r#"{"phoneNumber":"+123456789365"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["latestSimChange"].is_null());
        assert_eq!(body["monitoredPeriod"], MONITORED_PERIOD_DAYS);
    }

    #[tokio::test]
    async fn retrieve_date_reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = retrieve_ok_token(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn retrieve_date_does_not_accept_max_age() {
        // `maxAge` is a `check`-only field; retrieve-date rejects unknown fields.
        let (status, _, body) =
            retrieve_ok_token(r#"{"phoneNumber":"+123456789012","maxAge":240}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_date_invalid_phone_is_rejected() {
        let (status, _, body) = retrieve_ok_token(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_date_falls_back_to_the_token_subject() {
        // Subject is an E.164 number with a recent-swap tail → non-null timestamp,
        // with an empty body (phoneNumber optional).
        let token = mint_token_with_client(RETRIEVE_DATE_SCOPE, "+123456789012").await;
        let (status, _, body) = post_retrieve_date(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["latestSimChange"].as_str().is_some());
    }

    #[tokio::test]
    async fn retrieve_date_non_numeric_subject_is_null() {
        // Default synthetic subject "ss-client" has no digits → never swapped.
        let (status, _, body) = retrieve_ok_token("{}").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["latestSimChange"].is_null());
    }

    #[tokio::test]
    async fn retrieve_date_wrong_scope_is_forbidden() {
        // A token scoped only for `check` may not call retrieve-date.
        let token = mint_token(CHECK_SCOPE).await;
        let (status, _, body) =
            post_retrieve_date(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_date_missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_retrieve_date(None, r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn retrieve_date_echoes_x_correlator() {
        let token = mint_token(RETRIEVE_DATE_SCOPE).await;
        let (status, headers, _) = post_retrieve_date(
            Some(&token),
            r#"{"phoneNumber":"+123456789012"}"#,
            Some("corr-rd"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-rd")
        );
    }
}
