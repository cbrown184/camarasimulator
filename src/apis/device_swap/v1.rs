//! Device Swap **v1** (CAMARA Device Swap 1.0.0, r3.2).
//!
//! Endpoints:
//! - `POST /device-swap/v1/check` — has the device behind a phone number been
//!   swapped within the last `maxAge` hours?
//! - `POST /device-swap/v1/retrieve-date` — *when* was the device behind a phone
//!   number last swapped?
//!
//! ## What it does
//!
//! For `check`, the caller optionally submits a `phoneNumber` (two-legged auth
//! only) and a `maxAge` window in hours, and the operator answers
//! `{ "swapped": true|false }`: `true` when the device bound to the line was last
//! changed **within** the `maxAge` window, `false` otherwise. It is the device
//! counterpart of SIM Swap.
//!
//! `retrieve-date` is the companion: it reports **when** the device was last
//! changed as `{ "latestDeviceChange": <RFC 3339 UTC | null>, "monitoredPeriod":
//! <days> }` — the timestamp when the last swap is inside the fixed monitored
//! period ([`MONITORED_PERIOD_HOURS`], aligned to `check`'s default `maxAge`),
//! else `null`.
//!
//! Each endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying its scope (`device-swap:check` /
//! `device-swap:retrieve-date`).
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

/// The OAuth2 scope the `POST /check` endpoint requires (CAMARA Device Swap
/// 1.0.0).
const CHECK_SCOPE: &str = "device-swap:check";
/// The OAuth2 scope the `POST /retrieve-date` endpoint requires (CAMARA Device
/// Swap 1.0.0).
const RETRIEVE_DATE_SCOPE: &str = "device-swap:retrieve-date";

/// Default `maxAge` window in hours when the caller omits it (CAMARA default).
const DEFAULT_MAX_AGE: u16 = 240;
/// Inclusive `maxAge` bounds in hours (CAMARA Device Swap 1.0.0).
const MAX_AGE_MIN: u16 = 1;
const MAX_AGE_MAX: u16 = 2400;

/// The device-change supervision window `retrieve-date` reports, in **hours**. A
/// swap older than this is beyond what the operator monitors, so
/// `latestDeviceChange` is reported as `null`. Chosen to equal `check`'s default
/// `maxAge` (240 h) so the two operations agree: a device changed inside the
/// monitored period is exactly one for which `check` (default window) → `true`.
const MONITORED_PERIOD_HOURS: u16 = 240;
/// The same monitored window in **days** (CAMARA `monitoredPeriod` unit).
const MONITORED_PERIOD_DAYS: u16 = MONITORED_PERIOD_HOURS / 24;

/// Routes for Device Swap v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/device-swap/v1/check", post(check))
        .route("/device-swap/v1/retrieve-date", post(retrieve_date))
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
    let identifier = match resolve_identifier(req.phone_number.as_deref(), &claims, &correlator) {
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

/// `POST /retrieve-date` request body (CAMARA `CreateDeviceSwapDate`): an optional
/// `phoneNumber` (valid only in two-legged auth). Unlike `check` there is no
/// `maxAge` control plane. Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateDeviceSwapDate {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
}

/// `POST /device-swap/v1/retrieve-date` — the timestamp of the last device swap.
///
/// The companion to `check`: where `check` answers a boolean against a `maxAge`
/// window, `retrieve-date` reports **when** the device was last changed
/// (`latestDeviceChange`, RFC 3339 UTC) — or `null` when that swap is older than
/// the fixed monitored period ([`MONITORED_PERIOD_HOURS`]) or the identifier has
/// no trailing digits. `monitoredPeriod` (days) is always reported. The swap
/// recency is the identifier's trailing three digits (hours-ago), exactly as
/// `check` reads them, so the two operations always agree.
async fn retrieve_date(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(RETRIEVE_DATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (phoneNumber optional); anything present must parse.
    let req: CreateDeviceSwapDate = if body.is_empty() {
        CreateDeviceSwapDate { phone_number: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid CreateDeviceSwapDate.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the line identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(req.phone_number.as_deref(), &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // `latestDeviceChange` is `null` when the last swap is outside the monitored
    // period (or the identifier has no digits); `serde_json` maps `None` to JSON
    // `null`. `monitoredPeriod` (days) is always reported.
    let latest_device_change = last_swap_timestamp(&identifier);

    with_correlator(
        (
            StatusCode::OK,
            Json(json!({
                "latestDeviceChange": latest_device_change,
                "monitoredPeriod": MONITORED_PERIOD_DAYS,
            })),
        )
            .into_response(),
        &correlator,
    )
}

/// The timestamp of the last device swap behind `identifier`, as RFC 3339 UTC, or
/// `None` when the last swap is older than the monitored period
/// ([`MONITORED_PERIOD_HOURS`]) or the identifier carries no digits. The trailing
/// three digits encode hours-since-swap (docs/DESIGN.md §7), so the swap timestamp
/// is *now* minus that many hours — consistent with `check`'s recency semantics.
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
/// `2026-08-02T14:27:08Z`. Second precision — the CAMARA schema requires RFC 3339
/// with a time zone but not sub-second digits. Self-contained (no date-time
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

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Number Recycling). See the module docs for the four cases.
fn resolve_identifier(
    phone_number: Option<&str>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match phone_number {
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
            Ok(phone.to_string())
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

    // === retrieve-date =====================================================

    // --- Pure time helpers -------------------------------------------------

    #[test]
    fn rfc3339_utc_formats_known_epochs() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(946_684_800), "2000-01-01T00:00:00Z");
        // 2000-02-29 exists (leap year).
        assert_eq!(rfc3339_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(1_704_067_200), "2024-01-01T00:00:00Z");
        // Time-of-day is rendered too.
        assert_eq!(
            rfc3339_utc(1_704_067_200 + 14 * 3600 + 27 * 60 + 8),
            "2024-01-01T14:27:08Z"
        );
    }

    #[test]
    fn civil_from_days_handles_leap_boundaries() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn last_swap_timestamp_is_none_outside_the_monitored_period() {
        // hoursAgo == monitored period is not inside it (strict <).
        assert!(last_swap_timestamp("+123456789240").is_none());
        assert!(last_swap_timestamp("+123456789365").is_none());
        // No trailing digits → nothing to report.
        assert!(last_swap_timestamp("ds-client").is_none());
    }

    #[test]
    fn last_swap_timestamp_is_now_minus_hours_ago() {
        // …000 → swapped now; the timestamp is ~now (allow a 1 s test window).
        let now = now_unix_secs();
        let (before, after) = (rfc3339_utc(now - 1), rfc3339_utc(now + 1));
        let ts = last_swap_timestamp("+123456789000").expect("inside the monitored period");
        assert!(
            ts == rfc3339_utc(now) || ts == before || ts == after,
            "expected ~now, got {ts}"
        );
        // More hours ago → strictly earlier (fixed-width RFC 3339 UTC sorts).
        let recent = last_swap_timestamp("+123456789001").unwrap();
        let older = last_swap_timestamp("+123456789100").unwrap();
        assert!(older < recent, "{older} should be earlier than {recent}");
    }

    // --- Integration through the real router --------------------------------

    /// POST to `/retrieve-date` with an optional Bearer token and `x-correlator`.
    async fn post_retrieve_date(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/device-swap/v1/retrieve-date")
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

    /// Mint a `retrieve-date`-scoped (two-legged, non-line subject) token and call
    /// the endpoint.
    async fn retrieve_date_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_DATE_SCOPE).await;
        post_retrieve_date(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn recent_swap_returns_a_timestamp_and_monitored_period() {
        let (status, _, body) = retrieve_date_ok(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::OK);
        let ts = body["latestDeviceChange"].as_str().expect("a non-null timestamp");
        assert!(ts.ends_with('Z') && ts.contains('T'), "RFC 3339 UTC, got {ts}");
        assert_eq!(body["monitoredPeriod"], MONITORED_PERIOD_DAYS);
    }

    #[tokio::test]
    async fn swap_older_than_monitored_period_is_null() {
        // …365 h ago is outside the 240 h monitored window → latestDeviceChange null.
        let (status, _, body) = retrieve_date_ok(r#"{"phoneNumber":"+123456789365"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["latestDeviceChange"].is_null());
        assert_eq!(body["monitoredPeriod"], MONITORED_PERIOD_DAYS);
    }

    #[tokio::test]
    async fn retrieve_date_reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = retrieve_date_ok(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn retrieve_date_does_not_accept_max_age() {
        // `maxAge` is a `check`-only field; retrieve-date rejects unknown fields.
        let (status, _, body) =
            retrieve_date_ok(r#"{"phoneNumber":"+123456789012","maxAge":240}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_date_invalid_phone_is_rejected() {
        let (status, _, body) = retrieve_date_ok(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_date_empty_body_falls_back_to_the_token_subject() {
        // Three-legged: no body, subject is an E.164 line swapped 12 h ago.
        let token = mint_token_with_client(RETRIEVE_DATE_SCOPE, "+123456789012").await;
        let (status, _, body) = post_retrieve_date(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["latestDeviceChange"].as_str().is_some());
    }

    #[tokio::test]
    async fn retrieve_date_resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(RETRIEVE_DATE_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_retrieve_date(Some(&token), r#"{"phoneNumber":"+123456789012"}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn retrieve_date_no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = retrieve_date_ok(r#"{}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
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
