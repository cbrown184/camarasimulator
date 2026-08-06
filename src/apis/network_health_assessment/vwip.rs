//! Network Health Assessment **vwip** (CAMARA NetworkInsights, work-in-progress).
//!
//! One endpoint:
//! - `GET /network-health-assessment/vwip/health-scores` — the latest aggregate
//!   health score of a network module.
//!
//! ## What it does
//!
//! The caller names a network (`networkId`) and a module (`netType`) and the
//! operator answers with the module's latest health score:
//!
//! ```json
//! { "networkId": "…", "netType": "NET", "score": 94.5, "scoringTime": "2025-…Z" }
//! ```
//!
//! - `score` — the `0..100` health value (higher is healthier), or `null` when
//!   the platform has no data for the network/module.
//! - `scoringTime` — the RFC 3339 instant the score was last calculated, or
//!   `null` alongside a `null` score.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `network-health-assessment:health-scores:read` scope. It is a two-legged
//! (`client_credentials`) service query — the data is aggregated network-level
//! only, so there is no device/line identifier and no three-legged dance.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer:
//!
//! - **Reserved error suffix (`networkId`).** The `networkId` UUID is the
//!   identifier: if its trailing three digits name a reserved CAMARA status
//!   (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`),
//!   the endpoint answers with that canonical CAMARA error (shared
//!   [`crate::scenarios`]). A UUID contains digits, so the convention applies
//!   unchanged — e.g. `…-000000000404` selects `404 NOT_FOUND`.
//! - **Score (`networkId` digits × `netType`).** Otherwise the `networkId`'s
//!   trailing three digits `d` (`000`–`999`) fix the score and `netType` shifts
//!   it per module, so both are genuine planes:
//!   - `d == 000` (or a `networkId` with fewer than three digits) → **no data**:
//!     `score: null`, `scoringTime: null` (the spec's no-data 200).
//!   - otherwise `score = clamp(d / 10 − moduleIndex, 0, 100)` where
//!     `moduleIndex` is `NET`=0, `NET_WIRELESS`=1, `NET_TRANSPORT`=2, `NET_CORE`=3.
//!     So the trailing digits set the band directly (`…950` → ~95, excellent;
//!     `…650` → ~65, issues; `…300` → ~30, unhealthy) and each module reads a
//!     little lower than the whole-network composite.
//!
//! Example: `networkId=…-000000000950`, `netType=NET` → `score: 95`;
//! `netType=NET_CORE` → `score: 92`; `…-000000000000` → `score: null`;
//! `…-000000000404` → `404 NOT_FOUND`.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::RawQuery;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `GET /health-scores` endpoint requires (CAMARA Network
/// Health Assessment).
const READ_SCOPE: &str = "network-health-assessment:health-scores:read";

/// Routes for Network Health Assessment vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/network-health-assessment/vwip/health-scores",
        get(health_scores),
    )
}

/// The four CAMARA `NetTypeEnum` network modules, in enum order. The index is a
/// second control plane: it shifts the score so each module reads a little lower
/// than the whole-network composite (`NET`).
const NET_TYPES: [&str; 4] = ["NET", "NET_WIRELESS", "NET_TRANSPORT", "NET_CORE"];

/// `GET /network-health-assessment/vwip/health-scores`.
async fn health_scores(claims: Claims, headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Both query parameters are required (CAMARA marks them `required: true`).
    let (network_id, net_type) = match parse_query(query.as_deref(), &correlator) {
        Ok(parsed) => parsed,
        Err(response) => return response,
    };

    // The `networkId` is the identifier and the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&network_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // The trailing three digits fix the score; `netType` shifts it per module.
    let (score, scoring_time): (Value, Value) = match scenarios::trailing_three_digits(&network_id) {
        // `…000` (or a networkId with fewer than three digits) → no data.
        None | Some(0) => (Value::Null, Value::Null),
        Some(d) => {
            let module_index = NET_TYPES.iter().position(|&t| t == net_type).unwrap_or(0);
            let value = ((d as f64) / 10.0 - module_index as f64).clamp(0.0, 100.0);
            (json!(value), json!(rfc3339_utc(now_unix_secs())))
        }
    };

    with_correlator(
        (
            StatusCode::OK,
            Json(json!({
                "networkId": network_id,
                "netType": net_type,
                "score": score,
                "scoringTime": scoring_time,
            })),
        )
            .into_response(),
        &correlator,
    )
}

/// Parse and validate the two required query parameters, returning
/// `(networkId, netType)`. Unknown query parameters are ignored (serde skips
/// fields it does not know), mirroring the other query-driven endpoints.
///
/// - a query string that is not valid `x-www-form-urlencoded` → 400 INVALID_ARGUMENT.
/// - missing / non-UUID `networkId` → 400 INVALID_ARGUMENT.
/// - missing / unknown `netType` → 400 INVALID_ARGUMENT.
fn parse_query(
    query: Option<&str>,
    correlator: &Option<HeaderValue>,
) -> Result<(String, String), Response> {
    #[derive(serde::Deserialize, Default)]
    struct Raw {
        #[serde(rename = "networkId")]
        network_id: Option<String>,
        #[serde(rename = "netType")]
        net_type: Option<String>,
    }

    let raw: Raw = serde_urlencoded::from_str(query.unwrap_or("")).map_err(|_| {
        invalid_argument(
            "The query string is not valid application/x-www-form-urlencoded.",
            correlator,
        )
    })?;

    let network_id = match raw.network_id {
        Some(id) if is_uuid(&id) => id,
        Some(_) => {
            return Err(invalid_argument(
                "`networkId` must be a UUID (e.g. 123e4567-e89b-12d3-a456-426614174000).",
                correlator,
            ))
        }
        None => {
            return Err(invalid_argument(
                "`networkId` is a required query parameter.",
                correlator,
            ))
        }
    };

    let net_type = match raw.net_type {
        Some(t) if NET_TYPES.contains(&t.as_str()) => t,
        Some(_) => {
            return Err(invalid_argument(
                "`netType` must be one of: NET, NET_WIRELESS, NET_TRANSPORT, NET_CORE.",
                correlator,
            ))
        }
        None => {
            return Err(invalid_argument(
                "`netType` is a required query parameter.",
                correlator,
            ))
        }
    };

    Ok((network_id, net_type))
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

/// Whether `s` is a canonical UUID string (8-4-4-4-12 lowercase/uppercase hex
/// with hyphens), matching the CAMARA `format: uuid` / `maxLength: 36` schema.
fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 / ISO 8601 instant with
/// a `Z` offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs
/// no date/time dependency (mirrors `connected_network_type::v0_2`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for any date).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
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

    const HOST: &str = "nha.local:8080";
    const BASE: &str = "/network-health-assessment/vwip/health-scores";
    // A valid UUID whose trailing three digits are `TTT`, so tests can pick the
    // score/error case from the identifier alone.
    fn nid(tail: &str) -> String {
        format!("00000000-0000-0000-0000-000000000{tail}")
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_validation_follows_the_camara_format() {
        assert!(is_uuid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000404"));
        assert!(!is_uuid("123e4567e89b12d3a456426614174000")); // no hyphens
        assert!(!is_uuid("123e4567-e89b-12d3-a456-42661417400")); // too short
        assert!(!is_uuid("g23e4567-e89b-12d3-a456-426614174000")); // non-hex
        assert!(!is_uuid("nha-client")); // client-credentials subject
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=nha-client&scope={scope}");
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

    /// GET the endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_scores(
        token: Option<&str>,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{BASE}?{query}"))
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

    /// Mint a scoped token and call the endpoint.
    async fn scores_ok(query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        get_scores(Some(&token), query, None).await
    }

    // --- Score control plane -----------------------------------------------

    #[tokio::test]
    async fn trailing_digits_set_the_score_band() {
        // …950 → 95.0 (excellent) for the whole-network composite.
        let (status, _, body) =
            scores_ok(&format!("networkId={}&netType=NET", nid("950"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["networkId"], nid("950"));
        assert_eq!(body["netType"], "NET");
        assert_eq!(body["score"].as_f64().unwrap(), 95.0);
        assert!(body["scoringTime"].as_str().unwrap().ends_with('Z'));

        // …650 → 65.0 (issues band).
        let (_, _, body) = scores_ok(&format!("networkId={}&netType=NET", nid("650"))).await;
        assert_eq!(body["score"].as_f64().unwrap(), 65.0);
    }

    #[tokio::test]
    async fn net_type_shifts_the_score_per_module() {
        // Same networkId, each module reads a little lower than the composite.
        let q = |t: &str| format!("networkId={}&netType={t}", nid("950"));
        let (_, _, net) = scores_ok(&q("NET")).await;
        let (_, _, wireless) = scores_ok(&q("NET_WIRELESS")).await;
        let (_, _, transport) = scores_ok(&q("NET_TRANSPORT")).await;
        let (_, _, core) = scores_ok(&q("NET_CORE")).await;
        assert_eq!(net["score"].as_f64().unwrap(), 95.0);
        assert_eq!(wireless["score"].as_f64().unwrap(), 94.0);
        assert_eq!(transport["score"].as_f64().unwrap(), 93.0);
        assert_eq!(core["score"].as_f64().unwrap(), 92.0);
    }

    #[tokio::test]
    async fn score_clamps_into_the_zero_hundred_range() {
        // …001 with the deepest module penalty would be 0.1 − 3 → clamps to 0.
        let (_, _, body) =
            scores_ok(&format!("networkId={}&netType=NET_CORE", nid("001"))).await;
        assert_eq!(body["score"].as_f64().unwrap(), 0.0);
    }

    #[tokio::test]
    async fn zero_tail_is_the_no_data_case() {
        let (status, _, body) =
            scores_ok(&format!("networkId={}&netType=NET", nid("000"))).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["score"].is_null());
        assert!(body["scoringTime"].is_null());
        // The echoed identity fields are still present.
        assert_eq!(body["netType"], "NET");
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            scores_ok(&format!("networkId={}&netType=NET", nid("404"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            scores_ok(&format!("networkId={}&netType=NET", nid("429"))).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");

        let (status, _, body) =
            scores_ok(&format!("networkId={}&netType=NET", nid("503"))).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_network_id_is_rejected() {
        let (status, _, body) = scores_ok("netType=NET").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_uuid_network_id_is_rejected() {
        let (status, _, body) = scores_ok("networkId=not-a-uuid&netType=NET").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_net_type_is_rejected() {
        let (status, _, body) = scores_ok(&format!("networkId={}", nid("950"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_net_type_is_rejected() {
        let (status, _, body) =
            scores_ok(&format!("networkId={}&netType=NET_SATELLITE", nid("950"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            get_scores(Some(&token), &format!("networkId={}&netType=NET", nid("950")), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            get_scores(None, &format!("networkId={}&netType=NET", nid("950")), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        // Success.
        let (status, headers, _) = get_scores(
            Some(&token),
            &format!("networkId={}&netType=NET", nid("950")),
            Some("corr-nha"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nha")
        );
        // Business error (reserved suffix).
        let (status, headers, _) = get_scores(
            Some(&token),
            &format!("networkId={}&netType=NET", nid("404")),
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
