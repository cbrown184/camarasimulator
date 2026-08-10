//! Edge Application Management **vwip** (CAMARA EdgeApplicationManagement `wip`).
//!
//! One endpoint so far:
//! - `GET /edge-application-management/vwip/edge-cloud-zones` — list the edge
//!   cloud zones the operator offers for application placement (operationId
//!   `getEdgeCloudZones`, scope `edge-application-management:edge-cloud-zones:read`).
//!
//! ## What it does
//!
//! CamaraSim serves a **fixed catalog** of edge cloud zones (there is no upstream
//! orchestrator to query — DESIGN §7). Each zone is a static
//! `(edgeCloudZoneName, edgeCloudProvider, edgeCloudRegion, edgeCloudZoneStatus)`
//! tuple, mirroring the EdgeCloud zone shape shared across the already-mounted
//! EdgeCloud family (Simple / Optimal Edge Discovery, Application Endpoint
//! Discovery). The endpoint returns the catalog as a JSON array, filtered by the
//! request's query parameters.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes drive *which* zones come back (there is no device
//! identifier here, so no reserved-error plane — this is a pure catalog, like
//! QoS Profiles):
//!
//! 1. **`region`** — return only zones whose `edgeCloudRegion` matches exactly.
//!    An unknown region yields an empty array (a *list* never 404s).
//! 2. **`status`** — return only zones in that lifecycle state (`active`,
//!    `inactive`, `unknown`). An unrecognised value → `400 INVALID_ARGUMENT`.
//!
//! Both filters combine (AND). With neither, the whole catalog is returned.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `edge-application-management:edge-cloud-zones:read` scope. `x-correlator` is
//! echoed on every response.

use axum::extract::RawQuery;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;

/// The OAuth2 scope `getEdgeCloudZones` requires (CAMARA EdgeApplicationManagement).
const ZONES_SCOPE: &str = "edge-application-management:edge-cloud-zones:read";

/// Routes for Edge Application Management vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/edge-application-management/vwip/edge-cloud-zones",
        get(get_edge_cloud_zones),
    )
}

/// The operator's fixed edge cloud zones:
/// `(edgeCloudZoneName, edgeCloudProvider, edgeCloudRegion, edgeCloudZoneStatus)`.
///
/// The zone names mirror the EdgeCloud family's naming (optimal-edge-discovery);
/// the providers and statuses are spread so both the `region` and `status`
/// filters are meaningful control planes (every `EdgeCloudZoneStatus` value is
/// represented). The `edgeCloudZoneId` is derived from the name (stable per
/// zone — see [`zone_id`]).
const EDGE_ZONES: [(&str, &str, &str, &str); 6] = [
    ("camarasim-edge-eu-west-1", "CamaraSim Edge", "eu-west-1", "active"),
    ("camarasim-edge-eu-central-1", "CamaraSim Edge", "eu-central-1", "active"),
    ("camarasim-edge-us-east-1", "CamaraSim Edge", "us-east-1", "active"),
    ("camarasim-edge-us-west-2", "Partner Cloud", "us-west-2", "inactive"),
    ("camarasim-edge-ap-south-1", "Partner Cloud", "ap-south-1", "unknown"),
    ("camarasim-edge-ap-northeast-1", "CamaraSim Edge", "ap-northeast-1", "active"),
];

/// The three CAMARA `EdgeCloudZoneStatus` values, accepted by the `status` filter.
const VALID_STATUS: [&str; 3] = ["active", "inactive", "unknown"];

/// `GET /edge-application-management/vwip/edge-cloud-zones` (`getEdgeCloudZones`).
async fn get_edge_cloud_zones(
    claims: Claims,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(ZONES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Decode the optional `region` / `status` query filters.
    let raw: Raw = match serde_urlencoded::from_str(query.as_deref().unwrap_or("")) {
        Ok(raw) => raw,
        Err(_) => {
            return invalid_argument(
                "the query string is not valid application/x-www-form-urlencoded",
                &correlator,
            )
        }
    };

    // A supplied `status` must be one of the three EdgeCloudZoneStatus values.
    if let Some(status) = &raw.status {
        if !VALID_STATUS.contains(&status.as_str()) {
            return invalid_argument(
                "`status` must be one of `active`, `inactive`, `unknown`.",
                &correlator,
            );
        }
    }

    // Filter the fixed catalog by the (validated) region/status filters (AND).
    let zones: Vec<Value> = EDGE_ZONES
        .iter()
        .filter(|z| {
            raw.region.as_deref().is_none_or(|r| z.2 == r)
                && raw.status.as_deref().is_none_or(|s| z.3 == s)
        })
        .map(|z| {
            json!({
                "edgeCloudZoneId": zone_id(z.0),
                "edgeCloudZoneName": z.0,
                "edgeCloudZoneStatus": z.3,
                "edgeCloudProvider": z.1,
                "edgeCloudRegion": z.2,
            })
        })
        .collect();

    with_correlator(
        (StatusCode::OK, Json(Value::Array(zones))).into_response(),
        &correlator,
    )
}

/// Query parameters for `getEdgeCloudZones` — both optional (an empty query
/// returns the whole catalog). Unknown keys are rejected (`deny_unknown_fields`).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    region: Option<String>,
    status: Option<String>,
}

/// A stable, RFC 4122 (version 5, name-based) UUID `edgeCloudZoneId` for a zone,
/// derived from its name via SHA-256 (deterministic, no new dependency). The
/// version (`5`) and variant nibbles are forced so the id satisfies the strict
/// `EdgeCloudZone.edgeCloudZoneId` UUID pattern the CAMARA schema requires.
fn zone_id(zone_name: &str) -> String {
    let mut h = Sha256::digest(format!("eam-zone:{zone_name}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x50; // version 5
    h[8] = (h[8] & 0x3f) | 0x80; // variant (10xx)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "edgeappmgmt.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn catalog_covers_every_status_and_ids_are_valid_uuids() {
        // Every EdgeCloudZoneStatus value is represented (so the `status` filter
        // is a real control plane), and each derived id is a strict RFC 4122 UUID.
        for status in VALID_STATUS {
            assert!(
                EDGE_ZONES.iter().any(|z| z.3 == status),
                "catalog covers status {status}"
            );
        }
        for z in EDGE_ZONES {
            let id = zone_id(z.0);
            assert!(is_uuid(&id), "zone id {id} is a UUID");
            assert_eq!(&id[14..15], "5", "version nibble is 5 for {id}");
            assert!(
                matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
                "variant nibble is 8/9/a/b for {id}"
            );
        }
    }

    #[test]
    fn zone_id_is_stable_and_distinct_per_zone() {
        assert_eq!(zone_id("camarasim-edge-eu-west-1"), zone_id("camarasim-edge-eu-west-1"));
        assert_ne!(zone_id("camarasim-edge-eu-west-1"), zone_id("camarasim-edge-us-east-1"));
    }

    /// Whether `s` matches the strict CAMARA UUID pattern
    /// `^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`.
    fn is_uuid(s: &str) -> bool {
        let b = s.as_bytes();
        if b.len() != 36 {
            return false;
        }
        let hex = |c: u8| c.is_ascii_digit() || (b'a'..=b'f').contains(&c);
        for (i, &c) in b.iter().enumerate() {
            match i {
                8 | 13 | 18 | 23 => {
                    if c != b'-' {
                        return false;
                    }
                }
                14 => {
                    if !(b'1'..=b'5').contains(&c) {
                        return false;
                    }
                }
                19 => {
                    if !matches!(c, b'8' | b'9' | b'a' | b'b') {
                        return false;
                    }
                }
                _ => {
                    if !hex(c) {
                        return false;
                    }
                }
            }
        }
        true
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body =
            format!("grant_type=client_credentials&client_id=eam-client&scope={scope}");
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

    async fn get_zones(
        token: Option<&str>,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let uri = if query.is_empty() {
            "/edge-application-management/vwip/edge-cloud-zones".to_string()
        } else {
            format!("/edge-application-management/vwip/edge-cloud-zones?{query}")
        };
        let mut builder = Request::builder().method("GET").uri(uri).header("host", HOST);
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

    async fn get_zones_ok(query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(ZONES_SCOPE).await;
        get_zones(Some(&token), query, None).await
    }

    #[tokio::test]
    async fn no_filter_returns_the_whole_catalog() {
        let (status, _, body) = get_zones_ok("").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("array response");
        assert_eq!(arr.len(), EDGE_ZONES.len());
        // Each item carries the required CAMARA fields.
        assert!(arr.iter().all(|z| z["edgeCloudZoneId"].is_string()
            && z["edgeCloudZoneName"].is_string()
            && z["edgeCloudProvider"].is_string()
            && z["edgeCloudZoneStatus"].is_string()));
    }

    #[tokio::test]
    async fn region_filter_narrows_to_matching_zones() {
        let (status, _, body) = get_zones_ok("region=us-east-1").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["edgeCloudRegion"], "us-east-1");
        assert_eq!(arr[0]["edgeCloudZoneName"], "camarasim-edge-us-east-1");
    }

    #[tokio::test]
    async fn unknown_region_returns_an_empty_array_not_404() {
        let (status, _, body) = get_zones_ok("region=no-such-region").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn status_filter_narrows_the_catalog() {
        let (status, _, body) = get_zones_ok("status=inactive").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert!(!arr.is_empty());
        assert!(arr.iter().all(|z| z["edgeCloudZoneStatus"] == "inactive"));

        let (status, _, body) = get_zones_ok("status=active").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.as_array().unwrap().iter().all(|z| z["edgeCloudZoneStatus"] == "active"));
    }

    #[tokio::test]
    async fn region_and_status_combine() {
        // eu-west-1 is active → matches.
        let (status, _, body) = get_zones_ok("region=eu-west-1&status=active").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1);

        // us-west-2 is inactive, not active → empty.
        let (status, _, body) = get_zones_ok("region=us-west-2&status=active").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn unknown_status_value_is_rejected() {
        let (status, _, body) = get_zones_ok("status=RETIRED").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_query_param_is_rejected() {
        let (status, _, body) = get_zones_ok("foo=bar").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_zones(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_zones(None, "", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(ZONES_SCOPE).await;
        let (status, headers, _) = get_zones(Some(&token), "", Some("corr-eam")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-eam")
        );

        let (status, headers, _) =
            get_zones(Some(&token), "status=nope", Some("corr-err")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
