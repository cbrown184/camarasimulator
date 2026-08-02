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

/// Catalog of mounted CAMARA APIs and versions. Empty until the agent mounts APIs
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
            },
            {
                "name": "sim-swap",
                "version": "v2",
                "base_path": "/sim-swap/v2",
            },
            {
                "name": "kyc-match",
                "version": "v0.3",
                "base_path": "/kyc-match/v0.3",
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
            && a["base_path"] == "/number-verification/v1"));
        assert!(apis.iter().any(|a| a["name"] == "sim-swap"
            && a["version"] == "v2"
            && a["base_path"] == "/sim-swap/v2"));
        assert!(apis.iter().any(|a| a["name"] == "kyc-match"
            && a["version"] == "v0.3"
            && a["base_path"] == "/kyc-match/v0.3"));
        assert_eq!(
            body["authorization_servers"][0]["openid_configuration"],
            "/.well-known/openid-configuration"
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
