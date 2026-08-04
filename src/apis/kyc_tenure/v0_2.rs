//! KYC Tenure **v0.2** (CAMARA KYC Tenure 0.2.0, r2.2).
//!
//! One endpoint:
//! - `POST /kyc-tenure/v0.2/check-tenure` — has the subscriber behind a phone
//!   number held the line since at least a caller-supplied date?
//!
//! ## What it does
//!
//! The caller submits a `tenureDate` (a reference calendar date) and,
//! optionally, a `phoneNumber` (two-legged auth only), and the operator answers
//! `{ "tenureDateCheck": true|false, "contractType": … }`: `tenureDateCheck` is
//! `true` when the current end user has held the number **since at least**
//! `tenureDate` (i.e. the tenure started on or before that date), `false`
//! otherwise. It never reveals the actual tenure length — a privacy-preserving
//! trust signal for onboarding and fraud scoring.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `kyc-tenure:check-tenure`
//! scope.
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
//! - **Tenure verdict (identifier digits × `tenureDate`).** Otherwise the
//!   identifier's trailing three digits are read as **days the subscriber has
//!   held the line** (`0`–`999`), so the tenure started *today − digits days*.
//!   The check passes iff the tenure started on or before `tenureDate` — i.e.
//!   `(today − tenureDate) <= digits`. This makes `tenureDate` a genuine second
//!   control plane: the *same* number reads `tenureDateCheck: true` against a
//!   recent reference date and `false` against an old one. An identifier with no
//!   digits is treated as *acquired today* (`digits = 0`), so it passes only when
//!   `tenureDate` is today.
//!
//! `tenureDate` is validated: a malformed or impossible calendar date →
//! `400 INVALID_ARGUMENT`; a date in the future → `400 OUT_OF_RANGE`.
//!
//! The response also carries a deterministic `contractType` derived from the
//! identifier's trailing three digits (`digits % 3` → `PAYG` / `PAYM` /
//! `Business`), so it too is reproducible from the input alone.
//!
//! Example: `+123456789050` (held 50 days) with a `tenureDate` 30 days ago →
//! `tenureDateCheck: true`; the same number with a `tenureDate` 100 days ago →
//! `false`; `+123456789404` → `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /check-tenure` endpoint requires (CAMARA KYC
/// Tenure 0.2.0).
const CHECK_TENURE_SCOPE: &str = "kyc-tenure:check-tenure";

/// The `contractType` values a `TenureInfo` may report (CAMARA KYC Tenure
/// 0.2.0). Indexed deterministically by the identifier's trailing digits `% 3`.
const CONTRACT_TYPES: [&str; 3] = ["PAYG", "PAYM", "Business"];

/// Routes for KYC Tenure v0.2, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/kyc-tenure/v0.2/check-tenure", post(check_tenure))
}

/// `POST /check-tenure` request body (CAMARA `Tenure`): a required reference
/// `tenureDate` and an optional `phoneNumber` (valid only in two-legged auth).
/// Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tenure {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "tenureDate")]
    tenure_date: String,
}

/// `POST /kyc-tenure/v0.2/check-tenure`.
async fn check_tenure(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CHECK_TENURE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: Tenure = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid Tenure.", &correlator)
        }
    };

    // `tenureDate` must be a well-formed calendar date, and not in the future.
    let Some(tenure_days_ref) = parse_date(&req.tenure_date) else {
        return invalid_argument(
            "`tenureDate` must be a valid RFC 3339 full-date (YYYY-MM-DD).",
            &correlator,
        );
    };
    let today = today_unix_days();
    if tenure_days_ref > today {
        return out_of_range("`tenureDate` must not be in the future.", &correlator);
    }

    // Resolve the line identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(&req, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // The subscriber has held the line for `held_days`; the tenure started
    // `today − held_days`. The check passes iff that start is on or before the
    // reference date — i.e. the requested look-back is within the actual tenure.
    let held_days = scenarios::trailing_three_digits(&identifier).unwrap_or(0) as i64;
    let requested_days_ago = today - tenure_days_ref;
    let tenure_date_check = requested_days_ago <= held_days;
    let contract_type = CONTRACT_TYPES[(held_days % 3) as usize];

    with_correlator(
        (
            StatusCode::OK,
            Json(json!({
                "tenureDateCheck": tenure_date_check,
                "contractType": contract_type,
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
    req: &Tenure,
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

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Today's date as a day count since 1970-01-01 (UTC).
fn today_unix_days() -> i64 {
    now_unix_secs().div_euclid(86_400)
}

/// Parse a strict `YYYY-MM-DD` calendar date into a day count since 1970-01-01
/// (UTC), or `None` if it is not a well-formed real date. Rejects wrong length,
/// non-digit components, out-of-range month/day, and impossible dates such as
/// `2026-02-30` (via a civil round-trip check). Self-contained — no date-time
/// dependency (mirrors Number Recycling).
fn parse_date(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let digits_ok = |lo: usize, hi: usize| bytes[lo..hi].iter().all(u8::is_ascii_digit);
    if !(digits_ok(0, 4) && digits_ok(5, 7) && digits_ok(8, 10)) {
        return None;
    }
    let y: i64 = s[0..4].parse().ok()?;
    let m: u32 = s[5..7].parse().ok()?;
    let d: u32 = s[8..10].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d);
    // Reject impossible dates (e.g. 2026-02-30) by round-tripping.
    if civil_from_days(days) != (y, m, d) {
        return None;
    }
    Some(days)
}

/// The day count since 1970-01-01 for a civil `(year, month, day)` — Howard
/// Hinnant's `days_from_civil` (proleptic Gregorian, inverse of
/// [`civil_from_days`]). Month and day are 1-based.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let (m, d) = (m as i64, d as i64);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Convert a day count since 1970-01-01 into a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian). Month and day are
/// 1-based.
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

    const HOST: &str = "kt.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn civil_date_conversions_round_trip() {
        for &(y, m, d) in &[
            (1970, 1, 1),
            (2000, 2, 29), // leap day
            (2026, 8, 4),
            (1999, 12, 31),
        ] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d), "round trip {y}-{m}-{d}");
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn parse_date_accepts_valid_and_rejects_malformed() {
        assert_eq!(parse_date("1970-01-01"), Some(0));
        assert!(parse_date("2000-02-29").is_some()); // real leap day
        assert!(parse_date("2026-08-04").is_some());
        assert_eq!(parse_date("2026-02-30"), None); // Feb 30 doesn't exist
        assert_eq!(parse_date("2001-02-29"), None); // not a leap year
        assert_eq!(parse_date("2026-13-01"), None); // month out of range
        assert_eq!(parse_date("2026-00-10"), None); // month zero
        assert_eq!(parse_date("2026-08-32"), None); // day out of range
        assert_eq!(parse_date("2026-8-4"), None); // wrong width
        assert_eq!(parse_date("2026/08/04"), None); // wrong separators
        assert_eq!(parse_date("not-a-date"), None);
        assert_eq!(parse_date("2026-08-04T00:00:00Z"), None); // date only
    }

    #[test]
    fn contract_type_is_deterministic_from_the_trailing_digits() {
        // digits % 3 → PAYG/PAYM/Business.
        assert_eq!(CONTRACT_TYPES[(0i64 % 3) as usize], "PAYG");
        assert_eq!(CONTRACT_TYPES[(1i64 % 3) as usize], "PAYM");
        assert_eq!(CONTRACT_TYPES[(2i64 % 3) as usize], "Business");
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("kt-client")); // client-credentials subject
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "kt-client").await
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

    /// POST to `/check-tenure` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_check(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/kyc-tenure/v0.2/check-tenure")
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
        let token = mint_token(CHECK_TENURE_SCOPE).await;
        post_check(Some(&token), body, None).await
    }

    /// A `YYYY-MM-DD` string for `n` days before today (UTC), built from the same
    /// civil-date maths the handler uses so tests are stable across the calendar.
    fn ymd_days_ago(n: i64) -> String {
        let (y, m, d) = civil_from_days(today_unix_days() - n);
        format!("{y:04}-{m:02}-{d:02}")
    }

    // --- Tenure verdict: identifier digits × tenureDate --------------------

    #[tokio::test]
    async fn recent_reference_date_passes_the_tenure_check() {
        // Subscriber has held the line 50 days; a reference date 30 days ago is
        // within that tenure → the tenure started before it → true.
        let body = format!(
            r#"{{"phoneNumber":"+123456789050","tenureDate":"{}"}}"#,
            ymd_days_ago(30)
        );
        let (status, _, out) = check_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], true);
    }

    #[tokio::test]
    async fn old_reference_date_fails_the_tenure_check() {
        // Held 50 days; a reference date 100 days ago predates the tenure → false.
        let body = format!(
            r#"{{"phoneNumber":"+123456789050","tenureDate":"{}"}}"#,
            ymd_days_ago(100)
        );
        let (status, _, out) = check_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], false);
    }

    #[tokio::test]
    async fn tenure_date_is_a_real_second_control_plane() {
        // Same number (held 200 days); the reference date alone flips it.
        let recent = format!(
            r#"{{"phoneNumber":"+123456789200","tenureDate":"{}"}}"#,
            ymd_days_ago(150) // within the 200-day tenure → passes
        );
        let (status, _, out) = check_ok(&recent).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], true);

        let older = format!(
            r#"{{"phoneNumber":"+123456789200","tenureDate":"{}"}}"#,
            ymd_days_ago(300) // predates the tenure → fails
        );
        let (status, _, out) = check_ok(&older).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], false);
    }

    #[tokio::test]
    async fn boundary_reference_date_equal_to_tenure_start_passes() {
        // Held exactly 60 days; a reference date exactly 60 days ago is the tenure
        // start — inclusive, so it passes (requested_days_ago <= held_days).
        let body = format!(
            r#"{{"phoneNumber":"+123456789060","tenureDate":"{}"}}"#,
            ymd_days_ago(60)
        );
        let (status, _, out) = check_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], true);
        // One day older than the tenure start → fails.
        let body = format!(
            r#"{{"phoneNumber":"+123456789060","tenureDate":"{}"}}"#,
            ymd_days_ago(61)
        );
        let (status, _, out) = check_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], false);
    }

    #[tokio::test]
    async fn response_reports_a_deterministic_contract_type() {
        // …050 → 50 % 3 == 2 → Business.
        let body = format!(
            r#"{{"phoneNumber":"+123456789050","tenureDate":"{}"}}"#,
            ymd_days_ago(10)
        );
        let (status, _, out) = check_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["contractType"], "Business");
        // …049 → 49 % 3 == 1 → PAYM.
        let body = format!(
            r#"{{"phoneNumber":"+123456789049","tenureDate":"{}"}}"#,
            ymd_days_ago(10)
        );
        let (_, _, out) = check_ok(&body).await;
        assert_eq!(out["contractType"], "PAYM");
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"+123456789404","tenureDate":"2000-01-01"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"+123456789429","tenureDate":"2000-01-01"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; subject is an E.164 line held 200 days; recent date.
        let token = mint_token_with_client(CHECK_TENURE_SCOPE, "+123456789200").await;
        let body = format!(r#"{{"tenureDate":"{}"}}"#, ymd_days_ago(30));
        let (status, _, out) = post_check(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["tenureDateCheck"], true);
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(CHECK_TENURE_SCOPE, "+123456789503").await;
        let (status, _, body) =
            post_check(Some(&token), r#"{"tenureDate":"2000-01-01"}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(CHECK_TENURE_SCOPE, "+123456789200").await;
        let (status, _, body) = post_check(
            Some(&token),
            r#"{"phoneNumber":"+123456789200","tenureDate":"2000-01-01"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = check_ok(r#"{"tenureDate":"2000-01-01"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn future_reference_date_is_out_of_range() {
        let body = format!(
            r#"{{"phoneNumber":"+123456789050","tenureDate":"{}"}}"#,
            ymd_days_ago(-3) // 3 days in the future
        );
        let (status, _, out) = check_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn malformed_reference_date_is_invalid_argument() {
        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"+123456789050","tenureDate":"2026-02-30"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_reference_date_is_invalid_argument() {
        let (status, _, body) = check_ok(r#"{"phoneNumber":"+123456789050"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"0123","tenureDate":"2000-01-01"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            check_ok(r#"{"phoneNumber":"+123456789050","tenureDate":"2000-01-01","x":1}"#).await;
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
        let (status, _, body) = post_check(
            Some(&token),
            r#"{"phoneNumber":"+123456789050","tenureDate":"2000-01-01"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_check(
            None,
            r#"{"phoneNumber":"+123456789050","tenureDate":"2000-01-01"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CHECK_TENURE_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_check(
            Some(&token),
            r#"{"phoneNumber":"+123456789050","tenureDate":"2000-01-01"}"#,
            Some("corr-kt"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-kt")
        );
        // Business error.
        let (status, headers, _) =
            post_check(Some(&token), r#"{"tenureDate":"2000-01-01"}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
