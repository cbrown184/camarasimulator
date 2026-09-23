//! CamaraSim — a virtual CAMARA operator for testing telecom API integrations.
//!
//! Bootstrap only: a non-blocking axum server exposing a health check and an (empty)
//! API catalog. CAMARA APIs, auth, and versioning are added incrementally —
//! see docs/DESIGN.md.

// Raised for headroom on the larger `json!` literals in this crate (the default
// is 128).
#![recursion_limit = "256"]

mod apis;
mod auth;
mod errors;
mod registry;
mod scenarios;

use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

/// Build the application router. Kept separate from `main` so tests can exercise it
/// in-process without binding a socket.
fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(catalog))
        .merge(auth::routes())
        .merge(apis::routes())
}

/// Liveness probe.
async fn health() -> &'static str {
    "ok"
}

/// Catalog of mounted CAMARA APIs and versions. Each entry advertises its
/// `base_path` and the `spec_url` where its vendored OpenAPI spec is served
/// (canonical URL versioning — see docs/DESIGN.md §9). Derived from the shared
/// [`registry::APIS`] source of truth, so it can never drift from the specs the
/// server actually serves.
async fn catalog() -> Json<Value> {
    let apis: Vec<Value> = registry::APIS
        .iter()
        .map(|api| {
            json!({
                "name": api.name,
                "version": api.version,
                "base_path": api.base_path(),
                "spec_url": api.spec_url(),
            })
        })
        .collect();
    Json(json!({
        "service": "camarasimulator",
        "version": env!("CARGO_PKG_VERSION"),
        "apis": apis,
        "authorization_servers": [{
            "issuer": "/",
            "openid_configuration": "/.well-known/openid-configuration",
        }],
    }))
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("bind listener");

    println!("camarasimulator listening on 0.0.0.0:{port}");
    axum::serve(listener, app()).await.expect("serve");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn health_returns_ok() {
        let response = app()
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn catalog_lists_mounted_apis() {
        let response = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["service"], "camarasimulator");
        let apis = body["apis"].as_array().unwrap();
        assert!(apis.iter().any(|a| a["name"] == "number-verification"
            && a["version"] == "v1"
            && a["base_path"] == "/number-verification/v1"
            && a["spec_url"] == "/number-verification/v1/openapi.yaml"));
        // Every catalogued API advertises where its OpenAPI spec is served.
        assert!(apis.iter().all(|a| a["spec_url"].is_string()));
        assert!(apis.iter().any(|a| a["name"] == "sim-swap"
            && a["version"] == "v2"
            && a["base_path"] == "/sim-swap/v2"));
        assert!(apis.iter().any(|a| a["name"] == "kyc-match"
            && a["version"] == "v0.3"
            && a["base_path"] == "/kyc-match/v0.3"));
        assert!(apis.iter().any(|a| a["name"] == "device-reachability-status"
            && a["version"] == "v1"
            && a["base_path"] == "/device-reachability-status/v1"));
        assert!(apis.iter().any(|a| a["name"] == "device-roaming-status"
            && a["version"] == "v1"
            && a["base_path"] == "/device-roaming-status/v1"));
        assert!(apis.iter().any(|a| a["name"] == "device-identifier"
            && a["version"] == "v0.3"
            && a["base_path"] == "/device-identifier/v0.3"));
        assert!(apis.iter().any(|a| a["name"] == "one-time-password-sms"
            && a["version"] == "v1"
            && a["base_path"] == "/one-time-password-sms/v1"));
        assert!(apis.iter().any(|a| a["name"] == "quality-on-demand"
            && a["version"] == "v1"
            && a["base_path"] == "/quality-on-demand/v1"));
        assert!(apis.iter().any(|a| a["name"] == "location-verification"
            && a["version"] == "v3"
            && a["base_path"] == "/location-verification/v3"
            && a["spec_url"] == "/location-verification/v3/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "location-retrieval"
            && a["version"] == "v0.4"
            && a["base_path"] == "/location-retrieval/v0.4"
            && a["spec_url"] == "/location-retrieval/v0.4/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "geofencing-subscriptions"
            && a["version"] == "v0.4"
            && a["base_path"] == "/geofencing-subscriptions/v0.4"
            && a["spec_url"] == "/geofencing-subscriptions/v0.4/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "carrier-billing"
            && a["version"] == "v0.5"
            && a["base_path"] == "/carrier-billing/v0.5"
            && a["spec_url"] == "/carrier-billing/v0.5/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "call-forwarding-signal"
            && a["version"] == "v0.4"
            && a["base_path"] == "/call-forwarding-signal/v0.4"
            && a["spec_url"] == "/call-forwarding-signal/v0.4/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "number-recycling"
            && a["version"] == "v0.2"
            && a["base_path"] == "/number-recycling/v0.2"
            && a["spec_url"] == "/number-recycling/v0.2/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "kyc-age-verification"
            && a["version"] == "v0.1"
            && a["base_path"] == "/kyc-age-verification/v0.1"
            && a["spec_url"] == "/kyc-age-verification/v0.1/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "device-swap"
            && a["version"] == "v1"
            && a["base_path"] == "/device-swap/v1"
            && a["spec_url"] == "/device-swap/v1/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "kyc-fill-in"
            && a["version"] == "v0.3"
            && a["base_path"] == "/kyc-fill-in/v0.3"
            && a["spec_url"] == "/kyc-fill-in/v0.3/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "home-devices-qod"
            && a["version"] == "v0.4"
            && a["base_path"] == "/home-devices-qod/v0.4"
            && a["spec_url"] == "/home-devices-qod/v0.4/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "qos-profiles"
            && a["version"] == "v1"
            && a["base_path"] == "/qos-profiles/v1"
            && a["spec_url"] == "/qos-profiles/v1/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "kyc-tenure"
            && a["version"] == "v0.2"
            && a["base_path"] == "/kyc-tenure/v0.2"
            && a["spec_url"] == "/kyc-tenure/v0.2/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "blockchain-public-address"
            && a["version"] == "v0.3"
            && a["base_path"] == "/blockchain-public-address/v0.3"
            && a["spec_url"] == "/blockchain-public-address/v0.3/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "simple-edge-discovery"
            && a["version"] == "v2"
            && a["base_path"] == "/simple-edge-discovery/v2"
            && a["spec_url"] == "/simple-edge-discovery/v2/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "customer-insights"
            && a["version"] == "v0.2"
            && a["base_path"] == "/customer-insights/v0.2"
            && a["spec_url"] == "/customer-insights/v0.2/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "connectivity-insights"
            && a["version"] == "v0.6"
            && a["base_path"] == "/connectivity-insights/v0.6"
            && a["spec_url"] == "/connectivity-insights/v0.6/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "connected-network-type"
            && a["version"] == "v0.2"
            && a["base_path"] == "/connected-network-type/v0.2"
            && a["spec_url"] == "/connected-network-type/v0.2/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "device-data-volume"
            && a["version"] == "vwip"
            && a["base_path"] == "/device-data-volume/vwip"
            && a["spec_url"] == "/device-data-volume/vwip/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "region-device-count"
            && a["version"] == "v0.2"
            && a["base_path"] == "/region-device-count/v0.2"
            && a["spec_url"] == "/region-device-count/v0.2/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "population-density-data"
            && a["version"] == "vwip"
            && a["base_path"] == "/population-density-data/vwip"
            && a["spec_url"] == "/population-density-data/vwip/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "media-streaming-rate"
            && a["version"] == "vwip"
            && a["base_path"] == "/media-streaming-rate/vwip"
            && a["spec_url"] == "/media-streaming-rate/vwip/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "optimal-edge-discovery"
            && a["version"] == "vwip"
            && a["base_path"] == "/optimal-edge-discovery/vwip"
            && a["spec_url"] == "/optimal-edge-discovery/vwip/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "verified-caller"
            && a["version"] == "vwip"
            && a["base_path"] == "/verified-caller/vwip"
            && a["spec_url"] == "/verified-caller/vwip/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "device-authenticity"
            && a["version"] == "vwip"
            && a["base_path"] == "/device-authenticity/vwip"
            && a["spec_url"] == "/device-authenticity/vwip/openapi.yaml"));
        assert!(apis.iter().any(|a| a["name"] == "session-insights"
            && a["version"] == "vwip"
            && a["base_path"] == "/session-insights/vwip"
            && a["spec_url"] == "/session-insights/vwip/openapi.yaml"));
        assert_eq!(
            body["authorization_servers"][0]["openid_configuration"],
            "/.well-known/openid-configuration"
        );
    }

    #[tokio::test]
    async fn catalog_spec_urls_match_served_specs_and_resolve() {
        // The `/` catalog (this file) and the served-spec table (`apis::openapi`)
        // are two independently hand-maintained lists that must agree
        // (docs/DESIGN.md §9): every API CamaraSim serves a spec for is
        // catalogued, and every catalogued `spec_url` actually resolves. Without
        // this test the two silently drift — a new API can be mounted but left
        // out of the catalog, or catalogued with a `spec_url` that 404s.
        let response = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        let apis = body["apis"].as_array().unwrap();

        let mut catalog_urls: Vec<String> = apis
            .iter()
            .map(|a| a["spec_url"].as_str().expect("spec_url is a string").to_string())
            .collect();
        catalog_urls.sort();
        // Each API is catalogued once.
        let unique = catalog_urls.len();
        catalog_urls.dedup();
        assert_eq!(unique, catalog_urls.len(), "duplicate spec_url in catalog");

        let mut served: Vec<String> = crate::apis::openapi::api_spec_urls().collect();
        served.sort();

        // Exactly the same set — nothing served-but-uncatalogued or
        // catalogued-but-unserved.
        assert_eq!(
            catalog_urls, served,
            "catalog spec_urls must match the served API specs"
        );

        // Every advertised spec_url resolves through the full app as YAML.
        for url in &catalog_urls {
            let resp = app()
                .oneshot(Request::builder().uri(url.as_str()).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "catalog spec_url {url} must resolve");
            assert_eq!(
                resp.headers().get(axum::http::header::CONTENT_TYPE).unwrap(),
                "application/yaml",
                "catalog spec_url {url} served as YAML",
            );
        }
    }

    #[tokio::test]
    async fn catalog_apis_serve_html_docs() {
        // DESIGN §9's third discovery endpoint: every catalogued API also serves
        // a human-readable docs page at `{base_path}/docs` (the sibling of its
        // `{base_path}/openapi.yaml`). Driving this off the catalog's `base_path`
        // ties the docs pages to the catalog exactly as the spec_urls are —
        // catalog↔served wiring, no separate hand-maintained list.
        let response = app()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        let apis = body["apis"].as_array().unwrap();

        for api in apis {
            let base = api["base_path"].as_str().expect("base_path is a string");
            let docs = format!("{base}/docs");
            let resp = app()
                .oneshot(Request::builder().uri(docs.as_str()).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "docs page {docs} must resolve");
            let ct = resp
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            assert!(ct.starts_with("text/html"), "docs page {docs} served as HTML, got {ct}");
        }
    }

    #[tokio::test]
    async fn docs_page_is_reachable_through_the_app() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/number-verification/v1/docs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .starts_with("text/html"));
    }

    #[tokio::test]
    async fn openapi_spec_is_reachable_through_the_app() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/number-verification/v1/openapi.yaml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "application/yaml"
        );
    }

    #[tokio::test]
    async fn discovery_is_reachable_through_the_app() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/.well-known/openid-configuration")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
