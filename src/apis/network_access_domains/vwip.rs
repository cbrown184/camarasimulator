//! Network Access Domains **vwip** (CAMARA NetworkAccessManagement / Network
//! Access Domains, wip).
//!
//! One endpoint so far:
//! - `GET /network-access-domains/vwip/trust-domains/capabilities` — the
//!   provider-level Trust Domain capabilities document (operationId
//!   `getTrustDomainCapabilities`).
//!
//! ## What it does
//!
//! Returns the set of Trust Domain configuration capabilities this API provider
//! supports: which access types (Wi-Fi WPA-Personal/Enterprise, Thread) and their
//! properties, plus the policy limits (max devices, per-domain up/downstream
//! bandwidth bands, egress allow-list constraints) a caller may configure when
//! creating a Trust Domain.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `network-access-domains:trust-domains` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Provider capabilities are fixed operator configuration, not keyed to any device
//! or subscriber, so this operation has **no** parameter-driven control plane:
//! given a scoped token it always returns the same deterministic document
//! (`200`). A missing/invalid token → `401 UNAUTHENTICATED`; a token without the
//! scope → `403 PERMISSION_DENIED` (both from the shared resource-server layer).
//! `x-correlator` is echoed on the success response.

use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::auth::verify::Claims;

/// The OAuth2 scope the `GET /trust-domains/capabilities` endpoint requires
/// (CAMARA NetworkAccessManagement / Network Access Domains).
const CAPABILITIES_SCOPE: &str = "network-access-domains:trust-domains";

/// Routes for Network Access Domains vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/network-access-domains/vwip/trust-domains/capabilities",
        get(get_capabilities),
    )
}

/// `GET /network-access-domains/vwip/trust-domains/capabilities`.
async fn get_capabilities(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CAPABILITIES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    with_correlator(
        (StatusCode::OK, Json(trust_domain_capabilities())).into_response(),
        &correlator,
    )
}

/// The fixed `TrustDomainCapabilities` document this provider advertises.
///
/// A representative, schema-valid set covering the three access-type families
/// (Wi-Fi WPA-Personal, Wi-Fi WPA-Enterprise, Thread STRUCTURED) and all four
/// policy capabilities. Deterministic — capabilities are provider configuration,
/// not keyed to any request input (DESIGN §7).
fn trust_domain_capabilities() -> Value {
    json!({
        "supportedAccessTypes": [
            {
                "accessType": "Wi-Fi:WPA_PERSONAL",
                "wifiProperties": {
                    "ssidRequired": true,
                    "supportedSecurityModes": [
                        "WPA2-Personal",
                        "WPA3-Personal",
                        "WPA2-WPA3-Personal"
                    ],
                    "passwordConstraints": {
                        "minLength": 8,
                        "maxLength": 63,
                        "requireSpecialCharacters": false
                    },
                    "authServerRequired": false
                }
            },
            {
                "accessType": "Wi-Fi:WPA_ENTERPRISE",
                "wifiProperties": {
                    "ssidRequired": true,
                    "supportedSecurityModes": [
                        "WPA2-Enterprise",
                        "WPA3-Enterprise"
                    ],
                    "authServerRequired": true
                }
            },
            {
                "accessType": "Thread:STRUCTURED",
                "threadProperties": {
                    "supportedChannels": { "min": 11, "max": 26 },
                    "networkKeyFormat": "32-hex-digits",
                    "networkNameConstraints": { "minLength": 1, "maxLength": 16 },
                    "maxDatasetLength": 254
                }
            }
        ],
        "supportedPolicies": {
            "maxDevices": { "minValue": 1, "maxValue": 100 },
            "maxDomainDownstreamRate": { "minValue": 1_000_000, "maxValue": 1_000_000_000 },
            "maxDomainUpstreamRate": { "minValue": 1_000_000, "maxValue": 1_000_000_000 },
            "egressAllowedList": {
                "maxEntries": 50,
                "supportedDestinationTypes": ["IP", "FQDN", "CIDR"]
            }
        }
    })
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

    const HOST: &str = "nad.local:8080";

    // --- Pure unit: the fixed capabilities document -----------------------

    #[test]
    fn capabilities_document_is_schema_shaped() {
        let caps = trust_domain_capabilities();

        // `supportedAccessTypes` — required, 1..=4, each carries its discriminator.
        let access = caps["supportedAccessTypes"].as_array().unwrap();
        assert!((1..=4).contains(&access.len()));
        for entry in access {
            let ty = entry["accessType"].as_str().unwrap();
            assert!(matches!(
                ty,
                "Wi-Fi:WPA_PERSONAL"
                    | "Wi-Fi:WPA_ENTERPRISE"
                    | "Thread:STRUCTURED"
                    | "Thread:TLV"
            ));
            // A Wi-Fi variant carries wifiProperties; a Thread variant threadProperties.
            if ty.starts_with("Wi-Fi") {
                assert!(entry["wifiProperties"].is_object());
            } else {
                assert!(entry["threadProperties"].is_object());
            }
        }

        // All four policy capabilities are advertised, in range.
        let pol = &caps["supportedPolicies"];
        assert_eq!(pol["maxDevices"]["minValue"], 1);
        assert_eq!(pol["maxDevices"]["maxValue"], 100);
        assert!(pol["maxDomainDownstreamRate"]["maxValue"].as_i64().unwrap() <= 10_000_000_000);
        assert!(pol["maxDomainUpstreamRate"]["minValue"].as_i64().unwrap() >= 0);
        let egress = &pol["egressAllowedList"];
        assert_eq!(egress["maxEntries"], 50);
        assert_eq!(
            egress["supportedDestinationTypes"].as_array().unwrap().len(),
            3
        );
    }

    #[test]
    fn wifi_password_constraints_are_within_the_camara_bounds() {
        let caps = trust_domain_capabilities();
        let personal = &caps["supportedAccessTypes"][0]["wifiProperties"]["passwordConstraints"];
        let min = personal["minLength"].as_i64().unwrap();
        let max = personal["maxLength"].as_i64().unwrap();
        assert!((8..=63).contains(&min));
        assert!((8..=63).contains(&max));
        assert!(min <= max);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials` with the given scope.
    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=nad-client&scope={scope}");
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

    /// GET the capabilities endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_caps(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-domains/vwip/trust-domains/capabilities")
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
    async fn returns_the_capabilities_document() {
        let token = mint_token(CAPABILITIES_SCOPE).await;
        let (status, _, body) = get_caps(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["supportedAccessTypes"].as_array().unwrap().len() >= 1);
        assert!(body["supportedPolicies"]["maxDevices"].is_object());
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_caps(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_caps(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success() {
        let token = mint_token(CAPABILITIES_SCOPE).await;
        let (status, headers, _) = get_caps(Some(&token), Some("corr-nad")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nad")
        );
    }
}
