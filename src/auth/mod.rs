//! CAMARA auth surface (OIDC / OAuth2).
//!
//! CAMARA uses OpenID Connect per its *Security & Interoperability Profile*
//! (see docs/DESIGN.md §6). This module grows one endpoint per pass. So far:
//!
//! - `GET /.well-known/openid-configuration` — OIDC discovery metadata.
//! - `GET /oauth2/jwks` — JWK Set for the token signing key.
//!
//! Planned (advertised by discovery, filled in by later passes):
//! `/oauth2/token`, `/oauth2/authorize`, `/bc-authorize`.

mod keys;

use axum::{http::HeaderMap, routing::get, Json, Router};
use serde_json::{json, Value};

/// Auth routes, merged into the top-level router by `main`.
pub fn routes() -> Router {
    Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/oauth2/jwks", get(jwks))
}

/// Resolve the externally-visible base URL used to build absolute endpoint URLs
/// in the discovery document.
///
/// Precedence:
/// 1. `CAMARASIM_ISSUER` env var (verbatim, trailing slash trimmed) — lets an
///    operator pin the issuer when hosted behind a rewriting proxy.
/// 2. Otherwise reconstructed from the request: `X-Forwarded-Proto` (default
///    `http`) + `Host` (default `localhost:8080`).
///
/// OIDC requires the discovery URLs to be absolute, so we must derive a base.
pub fn base_url(headers: &HeaderMap) -> String {
    if let Ok(issuer) = std::env::var("CAMARASIM_ISSUER") {
        let trimmed = issuer.trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        // A proxy may append a comma-separated list; the first hop is ours.
        .map(|s| s.split(',').next().unwrap_or(s).trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("http");

    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("localhost:8080");

    format!("{scheme}://{host}")
}

/// Build the OIDC discovery metadata for `base`.
///
/// Advertises the full CAMARA auth surface from docs/DESIGN.md §6: the three
/// grant types (`client_credentials`, `authorization_code`, CIBA), RS256 token
/// signing, S256 PKCE, and CIBA poll delivery. Endpoints are absolute URLs under
/// `base`. Later passes implement each advertised endpoint.
pub fn metadata(base: &str) -> Value {
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth2/authorize"),
        "token_endpoint": format!("{base}/oauth2/token"),
        "jwks_uri": format!("{base}/oauth2/jwks"),
        "backchannel_authentication_endpoint": format!("{base}/bc-authorize"),
        "response_types_supported": ["code"],
        "grant_types_supported": [
            "client_credentials",
            "authorization_code",
            "urn:openid:params:grant-type:ciba"
        ],
        "token_endpoint_auth_methods_supported": [
            "client_secret_basic",
            "client_secret_post",
            "private_key_jwt"
        ],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["openid"],
        "backchannel_token_delivery_modes_supported": ["poll"],
        "backchannel_authentication_request_signing_alg_values_supported": ["RS256"],
        "backchannel_user_code_parameter_supported": false
    })
}

/// `GET /.well-known/openid-configuration` — OIDC discovery.
async fn discovery(headers: HeaderMap) -> Json<Value> {
    Json(metadata(&base_url(&headers)))
}

/// `GET /oauth2/jwks` — the JWK Set for the token signing key.
///
/// Static: the simulator holds a single fixed key (see [`keys`]), so the
/// document does not depend on the request. The `jwks_uri` in discovery points
/// here, and later passes sign tokens with the matching private key.
async fn jwks() -> Json<Value> {
    Json(keys::jwks())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    #[test]
    fn base_url_defaults_when_no_headers() {
        let headers = HeaderMap::new();
        assert_eq!(base_url(&headers), "http://localhost:8080");
    }

    #[test]
    fn base_url_uses_host_and_forwarded_proto() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "api.example.com".parse().unwrap());
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(base_url(&headers), "https://api.example.com");
    }

    #[test]
    fn base_url_takes_first_forwarded_proto_hop() {
        let mut headers = HeaderMap::new();
        headers.insert("host", "api.example.com".parse().unwrap());
        headers.insert("x-forwarded-proto", "https, http".parse().unwrap());
        assert_eq!(base_url(&headers), "https://api.example.com");
    }

    #[test]
    fn metadata_endpoints_are_absolute_under_base() {
        let m = metadata("https://api.example.com");
        assert_eq!(m["issuer"], "https://api.example.com");
        assert_eq!(m["token_endpoint"], "https://api.example.com/oauth2/token");
        assert_eq!(m["jwks_uri"], "https://api.example.com/oauth2/jwks");
        assert_eq!(
            m["authorization_endpoint"],
            "https://api.example.com/oauth2/authorize"
        );
        assert_eq!(
            m["backchannel_authentication_endpoint"],
            "https://api.example.com/bc-authorize"
        );
    }

    #[test]
    fn metadata_advertises_camara_grant_types() {
        let m = metadata("http://localhost:8080");
        let grants = m["grant_types_supported"].as_array().unwrap();
        assert!(grants.iter().any(|g| g == "client_credentials"));
        assert!(grants.iter().any(|g| g == "authorization_code"));
        assert!(grants
            .iter()
            .any(|g| g == "urn:openid:params:grant-type:ciba"));
    }

    #[test]
    fn metadata_advertises_rs256_and_s256() {
        let m = metadata("http://localhost:8080");
        assert!(m["id_token_signing_alg_values_supported"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "RS256"));
        assert!(m["code_challenge_methods_supported"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a == "S256"));
    }

    #[tokio::test]
    async fn discovery_route_returns_json_metadata() {
        let response = routes()
            .oneshot(
                Request::builder()
                    .uri("/.well-known/openid-configuration")
                    .header("host", "sim.local:9000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["issuer"], "http://sim.local:9000");
        assert_eq!(
            body["token_endpoint"],
            "http://sim.local:9000/oauth2/token"
        );
    }

    #[tokio::test]
    async fn jwks_route_serves_the_signing_key() {
        let response = routes()
            .oneshot(
                Request::builder()
                    .uri("/oauth2/jwks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();

        let keys = body["keys"].as_array().expect("keys array");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0]["kty"], "RSA");
        assert_eq!(keys[0]["use"], "sig");
        assert_eq!(keys[0]["alg"], "RS256");
        assert!(keys[0]["kid"].is_string());
    }

    #[test]
    fn discovery_jwks_uri_points_at_the_jwks_route() {
        // The advertised jwks_uri must be the path the JWK Set is actually
        // served at, so a client following discovery reaches the real keys.
        let m = metadata("https://sim.example");
        assert_eq!(m["jwks_uri"], "https://sim.example/oauth2/jwks");
    }
}
