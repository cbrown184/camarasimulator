//! CamaraSim — a virtual CAMARA operator for testing telecom API integrations.
//!
//! Bootstrap only: a non-blocking axum server exposing a health check and an (empty)
//! API catalog. CAMARA APIs, auth, and versioning are added incrementally by the
//! autonomous build agent — see docs/DESIGN.md and PROGRESS.md.

mod apis;
mod auth;
mod errors;
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
/// (canonical URL versioning — see docs/DESIGN.md §9).
async fn catalog() -> Json<Value> {
    Json(json!({
        "service": "camarasimulator",
        "version": env!("CARGO_PKG_VERSION"),
        "apis": [
            {
                "name": "number-verification",
                "version": "v1",
                "base_path": "/number-verification/v1",
                "spec_url": "/number-verification/v1/openapi.yaml",
            },
            {
                "name": "sim-swap",
                "version": "v2",
                "base_path": "/sim-swap/v2",
                "spec_url": "/sim-swap/v2/openapi.yaml",
            },
            {
                "name": "kyc-match",
                "version": "v0.3",
                "base_path": "/kyc-match/v0.3",
                "spec_url": "/kyc-match/v0.3/openapi.yaml",
            },
            {
                "name": "device-reachability-status",
                "version": "v1",
                "base_path": "/device-reachability-status/v1",
                "spec_url": "/device-reachability-status/v1/openapi.yaml",
            },
            {
                "name": "device-roaming-status",
                "version": "v1",
                "base_path": "/device-roaming-status/v1",
                "spec_url": "/device-roaming-status/v1/openapi.yaml",
            },
            {
                "name": "device-identifier",
                "version": "v0.3",
                "base_path": "/device-identifier/v0.3",
                "spec_url": "/device-identifier/v0.3/openapi.yaml",
            },
            {
                "name": "one-time-password-sms",
                "version": "v1",
                "base_path": "/one-time-password-sms/v1",
                "spec_url": "/one-time-password-sms/v1/openapi.yaml",
            },
            {
                "name": "quality-on-demand",
                "version": "v1",
                "base_path": "/quality-on-demand/v1",
                "spec_url": "/quality-on-demand/v1/openapi.yaml",
            },
            {
                "name": "location-verification",
                "version": "v3",
                "base_path": "/location-verification/v3",
                "spec_url": "/location-verification/v3/openapi.yaml",
            },
            {
                "name": "location-retrieval",
                "version": "v0.4",
                "base_path": "/location-retrieval/v0.4",
                "spec_url": "/location-retrieval/v0.4/openapi.yaml",
            },
            {
                "name": "geofencing-subscriptions",
                "version": "v0.4",
                "base_path": "/geofencing-subscriptions/v0.4",
                "spec_url": "/geofencing-subscriptions/v0.4/openapi.yaml",
            },
            {
                "name": "carrier-billing",
                "version": "v0.5",
                "base_path": "/carrier-billing/v0.5",
                "spec_url": "/carrier-billing/v0.5/openapi.yaml",
            },
            {
                "name": "call-forwarding-signal",
                "version": "v0.4",
                "base_path": "/call-forwarding-signal/v0.4",
                "spec_url": "/call-forwarding-signal/v0.4/openapi.yaml",
            }
        ],
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
        assert_eq!(
            body["authorization_servers"][0]["openid_configuration"],
            "/.well-known/openid-configuration"
        );
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
