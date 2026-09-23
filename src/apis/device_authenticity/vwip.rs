//! Device Authenticity **vwip** (CAMARA DeviceAuthenticity, work-in-progress).
//!
//! One endpoint:
//! - `POST /device-authenticity/vwip/check-status` — the register / operational
//!   status of a device, keyed by its IMEI (`checkImeiStatus`).
//!
//! ## What it does
//!
//! The caller submits an `imei` (15 digits) and the operator answers with its
//! register status:
//!
//! ```json
//! {
//!   "imei": "490154203237518",
//!   "operationalStatus": "allowed",
//!   "lastChecked": "2026-08-05T10:41:38Z"
//! }
//! ```
//!
//! - `operationalStatus` — one of `allowed` | `lost` | `stolen` | `blacklisted` |
//!   `blocked` | `fraud` | `non-payment` | `regulatory` | `unknown`.
//! - `lastChecked` — when the status was last confirmed (RFC 3339 UTC, always the
//!   moment of the call in the simulator).
//! - `reportedDate` — present only for a non-`allowed` (adverse) status: when the
//!   IMEI was reported with that status (RFC 3339 UTC).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `device-authenticity:check-status`
//! scope.
//!
//! ## Identifier
//!
//! The IMEI is the identifier and is **required** in the body — there is no
//! two-legged / three-legged token-subject fallback (unlike the phone-number
//! APIs), because the device is named explicitly. A missing or non-15-digit
//! `imei` is a `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer, both read from the submitted
//! `imei`:
//!
//! - **Reserved error suffix (identifier).** If the IMEI's trailing three digits
//!   name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`,
//!   `…422`, `…429`, `…500`, `…503`), the endpoint answers with that canonical
//!   CAMARA error (shared [`crate::scenarios`]). In particular `…404` gives the
//!   canonical `NOT_FOUND` (the API's own 404 code is `IDENTIFIER_NOT_FOUND`) and
//!   `…422` gives `SERVICE_NOT_APPLICABLE` (the API's own service-level 422 code).
//! - **Operational status (identifier digits).** Otherwise the IMEI's trailing
//!   three digits `d` (every reserved suffix is `≥ 400`, so all small `d` are
//!   free) pick the status by `d % 9` over the nine-value enum, in order — so
//!   `…000` (index 0) is the healthy `allowed` default, `…001` → `lost`,
//!   `…002` → `stolen`, …, `…008` → `unknown`. A non-`allowed` status also carries
//!   a deterministic `reportedDate` of `now − d hours` (the report is `d` hours
//!   old); `allowed` carries none.
//!
//! Example: `490154203237000` → `allowed`; `490154203237001` → `lost`;
//! `490154203237002` → `stolen`; `490154203237404` → `404 NOT_FOUND`;
//! `490154203237422` → `422 SERVICE_NOT_APPLICABLE`.

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

/// The OAuth2 scope the `POST /check-status` endpoint requires
/// (CAMARA DeviceAuthenticity).
const CHECK_STATUS_SCOPE: &str = "device-authenticity:check-status";

/// The nine `operationalStatus` enum values, in the CAMARA schema order. Index 0
/// (`allowed`) is the healthy default; the rest are adverse register states. The
/// identifier's trailing three digits pick one by `d % 9`.
const STATUSES: [&str; 9] = [
    "allowed",
    "lost",
    "stolen",
    "blacklisted",
    "blocked",
    "fraud",
    "non-payment",
    "regulatory",
    "unknown",
];

/// Routes for Device Authenticity vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/device-authenticity/vwip/check-status", post(check_status))
}

/// `POST /check-status` request body (CAMARA `RequestBody`): the required 15-digit
/// `imei`. Unknown fields are tolerated (the CAMARA schema does not set
/// `additionalProperties: false`), so extra properties are ignored.
#[derive(Debug, Deserialize)]
struct CheckStatusRequest {
    imei: Option<String>,
}

/// `POST /device-authenticity/vwip/check-status`.
async fn check_status(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CHECK_STATUS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required and must parse.
    let req: CheckStatusRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid check-status request.", &correlator)
        }
    };

    // The IMEI is required and must be exactly 15 digits.
    let imei = match req.imei.as_deref() {
        Some(imei) if is_valid_imei(imei) => imei,
        _ => {
            return invalid_argument(
                "`imei` is required and must be a 15-digit string.",
                &correlator,
            )
        }
    };

    // Plane 1: the identifier's reserved error suffix (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(imei) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Plane 2: the identifier's trailing three digits pick the register status.
    let d = scenarios::trailing_three_digits(imei).unwrap_or(0);
    let status = STATUSES[(d % 9) as usize];

    let mut response = json!({
        "imei": imei,
        "operationalStatus": status,
        "lastChecked": rfc3339_utc(now_unix_secs()),
    });
    // A non-`allowed` (adverse) status carries a deterministic report time.
    if status != "allowed" {
        response["reportedDate"] = json!(rfc3339_utc(now_unix_secs() - d as i64 * 3600));
    }

    with_correlator((StatusCode::OK, Json(response)).into_response(), &correlator)
}

/// Whether `s` is a valid CAMARA IMEI: exactly 15 ASCII digits (`^[0-9]{15}$`).
fn is_valid_imei(s: &str) -> bool {
    s.len() == 15 && s.bytes().all(|b| b.is_ascii_digit())
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

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as RFC 3339, e.g.
/// `2026-08-05T14:27:08Z`. Second precision — the CAMARA schema requires RFC 3339
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "da.local:8080";
    const PATH: &str = "/device-authenticity/vwip/check-status";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn imei_validation_requires_exactly_15_digits() {
        assert!(is_valid_imei("490154203237518"));
        assert!(!is_valid_imei("49015420323751")); // 14 digits
        assert!(!is_valid_imei("4901542032375188")); // 16 digits
        assert!(!is_valid_imei("49015420323751a")); // non-digit
        assert!(!is_valid_imei("")); // empty
    }

    #[test]
    fn rfc3339_utc_formats_a_known_epoch() {
        // 2026-04-05T10:41:38Z = 1_775_385_698 seconds since the epoch.
        assert_eq!(rfc3339_utc(1_775_385_698), "2026-04-05T10:41:38Z");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=da-client&scope={scope}");
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

    async fn post_check(
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

    /// Mint the scope and call the endpoint with the given IMEI.
    async fn check_ok(imei: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CHECK_STATUS_SCOPE).await;
        post_check(Some(&token), &format!(r#"{{"imei":"{imei}"}}"#), None).await
    }

    // --- Operational-status control plane ----------------------------------

    #[tokio::test]
    async fn zero_tail_is_the_allowed_default_without_a_reported_date() {
        let (status, _, body) = check_ok("490154203237000").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imei"], "490154203237000");
        assert_eq!(body["operationalStatus"], "allowed");
        assert!(body["lastChecked"].as_str().unwrap().ends_with('Z'));
        // `allowed` carries no `reportedDate`.
        assert!(body.get("reportedDate").is_none());
    }

    #[tokio::test]
    async fn trailing_digits_select_each_status_in_order() {
        // …001 → lost, …002 → stolen, …005 → fraud, …008 → unknown.
        for (imei, expected) in [
            ("490154203237001", "lost"),
            ("490154203237002", "stolen"),
            ("490154203237005", "fraud"),
            ("490154203237008", "unknown"),
        ] {
            let (status, _, body) = check_ok(imei).await;
            assert_eq!(status, StatusCode::OK, "{imei}");
            assert_eq!(body["operationalStatus"], expected, "{imei}");
        }
    }

    #[tokio::test]
    async fn an_adverse_status_carries_a_reported_date() {
        let (status, _, body) = check_ok("490154203237002").await; // stolen
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operationalStatus"], "stolen");
        assert!(body["reportedDate"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn status_wraps_modulo_nine() {
        // …009 → 9 % 9 == 0 → allowed again (no reportedDate).
        let (status, _, body) = check_ok("490154203237009").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operationalStatus"], "allowed");
        assert!(body.get("reportedDate").is_none());
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = check_ok("490154203237404").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = check_ok("490154203237422").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");

        let (status, _, body) = check_ok("490154203237429").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_imei_is_invalid_argument() {
        let token = mint_token(CHECK_STATUS_SCOPE).await;
        let (status, _, body) = post_check(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_15_digit_imei_is_invalid_argument() {
        let (status, _, body) = check_ok("12345").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_body_is_invalid_argument() {
        let token = mint_token(CHECK_STATUS_SCOPE).await;
        let (status, _, body) = post_check(Some(&token), "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_fields_are_tolerated() {
        // The CAMARA RequestBody does not forbid extra properties, so they are
        // ignored rather than rejected.
        let token = mint_token(CHECK_STATUS_SCOPE).await;
        let (status, _, body) = post_check(
            Some(&token),
            r#"{"imei":"490154203237000","extra":true}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["operationalStatus"], "allowed");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_check(Some(&token), r#"{"imei":"490154203237000"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_check(None, r#"{"imei":"490154203237000"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CHECK_STATUS_SCOPE).await;
        // Success.
        let (status, headers, _) = post_check(
            Some(&token),
            r#"{"imei":"490154203237000"}"#,
            Some("corr-da"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-da")
        );
        // Business error.
        let (status, headers, _) = post_check(
            Some(&token),
            r#"{"imei":"490154203237404"}"#,
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
