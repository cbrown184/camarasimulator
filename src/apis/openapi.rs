//! Serve the **vendored OpenAPI specs** over HTTP (docs/DESIGN.md §9).
//!
//! Each mounted CAMARA API's annotated spec (and the shared fragments it
//! `$ref`s) is served at a stable, canonical URL so an integrator can fetch the
//! exact contract CamaraSim implements — the same file that lives under
//! `specs/…`, which the agent keeps in lock-step with the code ("spec and code
//! must not drift", docs/AGENT.md). A published, machine-readable spec is what
//! makes the served surface self-describing (Swagger/Redoc/codegen can consume
//! it directly).
//!
//! ## URLs
//!
//! - `/{api}/v{n}/openapi.yaml` — one per mounted API/version, mirroring the
//!   API's own base path (e.g. `/number-verification/v1/openapi.yaml`).
//! - `/auth/openapi.yaml` — the authored authorization-server spec.
//! - `/shared/errors.yaml` — the shared CAMARA error-model / scenario fragment.
//!
//! The last two are served because every API spec `$ref`s them with the
//! relative paths `../../auth/openapi.yaml` and `../../shared/errors.yaml`;
//! resolved against an API's `…/v{n}/openapi.yaml` URL those land exactly on the
//! two URLs above, so a client that follows the `$ref`s finds them and every
//! served spec is fully resolvable.
//!
//! ## Simulator constraints
//!
//! - **Single binary, no runtime I/O.** Every spec is embedded at compile time
//!   with [`include_str!`], so serving one is an in-memory `&'static str` copy —
//!   no filesystem read on the request path (non-blocking, DESIGN §11) and the
//!   binary stays self-contained (no `specs/` dir needed at runtime).
//! - **No new dependency.** Pure `axum` routing + a static body.
//!
//! These are simulator meta-endpoints (they publish the contracts), not a CAMARA
//! business API, so there is no upstream CAMARA spec for them to vendor.

use axum::{http::header, routing::get, Router};

/// The media type OpenAPI documents are served with. OpenAPI 3.x itself is
/// content-type-agnostic; `application/yaml` is the widely-understood generic
/// YAML type (Swagger UI / Redoc / most codegen accept it).
const YAML_CONTENT_TYPE: &str = "application/yaml";

/// `(url path, embedded spec body)` for every vendored spec CamaraSim serves.
///
/// The API entries mirror each mounted API's base path with `/openapi.yaml`
/// appended; `auth`/`shared` are the fragments the API specs `$ref`. Bodies are
/// embedded from `specs/…` at compile time so the served copy can never drift
/// from the file the agent maintains.
const SPECS: &[(&str, &str)] = &[
    (
        "/number-verification/v1/openapi.yaml",
        include_str!("../../specs/number-verification/v1/openapi.yaml"),
    ),
    (
        "/sim-swap/v2/openapi.yaml",
        include_str!("../../specs/sim-swap/v2/openapi.yaml"),
    ),
    (
        "/kyc-match/v0.3/openapi.yaml",
        include_str!("../../specs/kyc-match/v0.3/openapi.yaml"),
    ),
    (
        "/device-reachability-status/v1/openapi.yaml",
        include_str!("../../specs/device-reachability-status/v1/openapi.yaml"),
    ),
    (
        "/device-roaming-status/v1/openapi.yaml",
        include_str!("../../specs/device-roaming-status/v1/openapi.yaml"),
    ),
    (
        "/device-identifier/v0.3/openapi.yaml",
        include_str!("../../specs/device-identifier/v0.3/openapi.yaml"),
    ),
    (
        "/one-time-password-sms/v1/openapi.yaml",
        include_str!("../../specs/one-time-password-sms/v1/openapi.yaml"),
    ),
    (
        "/quality-on-demand/v1/openapi.yaml",
        include_str!("../../specs/quality-on-demand/v1/openapi.yaml"),
    ),
    (
        "/location-verification/v3/openapi.yaml",
        include_str!("../../specs/location-verification/v3/openapi.yaml"),
    ),
    (
        "/location-retrieval/v0.4/openapi.yaml",
        include_str!("../../specs/location-retrieval/v0.4/openapi.yaml"),
    ),
    (
        "/geofencing-subscriptions/v0.4/openapi.yaml",
        include_str!("../../specs/geofencing-subscriptions/v0.4/openapi.yaml"),
    ),
    (
        "/carrier-billing/v0.5/openapi.yaml",
        include_str!("../../specs/carrier-billing/v0.5/openapi.yaml"),
    ),
    (
        "/call-forwarding-signal/v0.4/openapi.yaml",
        include_str!("../../specs/call-forwarding-signal/v0.4/openapi.yaml"),
    ),
    (
        "/number-recycling/v0.2/openapi.yaml",
        include_str!("../../specs/number-recycling/v0.2/openapi.yaml"),
    ),
    (
        "/kyc-age-verification/v0.1/openapi.yaml",
        include_str!("../../specs/kyc-age-verification/v0.1/openapi.yaml"),
    ),
    (
        "/device-swap/v1/openapi.yaml",
        include_str!("../../specs/device-swap/v1/openapi.yaml"),
    ),
    (
        "/auth/openapi.yaml",
        include_str!("../../specs/auth/openapi.yaml"),
    ),
    (
        "/shared/errors.yaml",
        include_str!("../../specs/shared/errors.yaml"),
    ),
];

/// A `GET` route for every vendored spec, each returning its embedded YAML with
/// `Content-Type: application/yaml`.
pub fn routes() -> Router {
    let mut router = Router::new();
    for &(path, body) in SPECS {
        router = router.route(
            path,
            get(move || async move { ([(header::CONTENT_TYPE, YAML_CONTENT_TYPE)], body) }),
        );
    }
    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    async fn fetch(path: &str) -> (StatusCode, String, String) {
        let response = routes()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, content_type, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn serves_an_api_spec_as_yaml() {
        let (status, content_type, body) = fetch("/number-verification/v1/openapi.yaml").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, YAML_CONTENT_TYPE);
        // The served body is byte-for-byte the vendored file.
        assert_eq!(body, include_str!("../../specs/number-verification/v1/openapi.yaml"));
        assert!(body.contains("openapi: 3.0.3"));
        assert!(body.contains("number-verification/v1"));
    }

    #[tokio::test]
    async fn serves_every_mounted_api_spec() {
        // Each mounted API's spec URL resolves and looks like an OpenAPI doc.
        for path in [
            "/number-verification/v1/openapi.yaml",
            "/sim-swap/v2/openapi.yaml",
            "/kyc-match/v0.3/openapi.yaml",
            "/device-reachability-status/v1/openapi.yaml",
            "/device-roaming-status/v1/openapi.yaml",
            "/device-identifier/v0.3/openapi.yaml",
            "/one-time-password-sms/v1/openapi.yaml",
            "/quality-on-demand/v1/openapi.yaml",
            "/location-verification/v3/openapi.yaml",
            "/location-retrieval/v0.4/openapi.yaml",
            "/geofencing-subscriptions/v0.4/openapi.yaml",
            "/carrier-billing/v0.5/openapi.yaml",
            "/call-forwarding-signal/v0.4/openapi.yaml",
            "/number-recycling/v0.2/openapi.yaml",
            "/kyc-age-verification/v0.1/openapi.yaml",
            "/device-swap/v1/openapi.yaml",
        ] {
            let (status, content_type, body) = fetch(path).await;
            assert_eq!(status, StatusCode::OK, "spec {path} should be served");
            assert_eq!(content_type, YAML_CONTENT_TYPE, "spec {path} content-type");
            assert!(body.starts_with("#") || body.contains("openapi:"), "spec {path} is YAML");
        }
    }

    #[tokio::test]
    async fn serves_the_shared_ref_targets_so_specs_resolve() {
        // Every API spec `$ref`s these two by relative path; they must be served
        // at the resolved URLs or the served specs are not resolvable.
        let (status, ct, errors) = fetch("/shared/errors.yaml").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct, YAML_CONTENT_TYPE);
        assert!(errors.contains("components:"));

        let (status, ct, auth) = fetch("/auth/openapi.yaml").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct, YAML_CONTENT_TYPE);
        assert!(auth.contains("camaraOAuth"));
    }

    #[tokio::test]
    async fn unknown_spec_path_is_404() {
        let (status, _, _) = fetch("/no-such-api/v9/openapi.yaml").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
