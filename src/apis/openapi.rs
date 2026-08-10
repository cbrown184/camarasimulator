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
        "/kyc-fill-in/v0.3/openapi.yaml",
        include_str!("../../specs/kyc-fill-in/v0.3/openapi.yaml"),
    ),
    (
        "/home-devices-qod/v0.4/openapi.yaml",
        include_str!("../../specs/home-devices-qod/v0.4/openapi.yaml"),
    ),
    (
        "/qos-profiles/v1/openapi.yaml",
        include_str!("../../specs/qos-profiles/v1/openapi.yaml"),
    ),
    (
        "/kyc-tenure/v0.2/openapi.yaml",
        include_str!("../../specs/kyc-tenure/v0.2/openapi.yaml"),
    ),
    (
        "/blockchain-public-address/v0.3/openapi.yaml",
        include_str!("../../specs/blockchain-public-address/v0.3/openapi.yaml"),
    ),
    (
        "/simple-edge-discovery/v2/openapi.yaml",
        include_str!("../../specs/simple-edge-discovery/v2/openapi.yaml"),
    ),
    (
        "/customer-insights/v0.2/openapi.yaml",
        include_str!("../../specs/customer-insights/v0.2/openapi.yaml"),
    ),
    (
        "/connected-network-type/v0.2/openapi.yaml",
        include_str!("../../specs/connected-network-type/v0.2/openapi.yaml"),
    ),
    (
        "/device-data-volume/vwip/openapi.yaml",
        include_str!("../../specs/device-data-volume/vwip/openapi.yaml"),
    ),
    (
        "/connectivity-insights/v0.6/openapi.yaml",
        include_str!("../../specs/connectivity-insights/v0.6/openapi.yaml"),
    ),
    (
        "/region-device-count/v0.2/openapi.yaml",
        include_str!("../../specs/region-device-count/v0.2/openapi.yaml"),
    ),
    (
        "/device-visit-location/vwip/openapi.yaml",
        include_str!("../../specs/device-visit-location/vwip/openapi.yaml"),
    ),
    (
        "/population-density-data/vwip/openapi.yaml",
        include_str!("../../specs/population-density-data/vwip/openapi.yaml"),
    ),
    (
        "/qos-provisioning/v0.3/openapi.yaml",
        include_str!("../../specs/qos-provisioning/v0.3/openapi.yaml"),
    ),
    (
        "/qos-booking/vwip/openapi.yaml",
        include_str!("../../specs/qos-booking/vwip/openapi.yaml"),
    ),
    (
        "/media-streaming-rate/vwip/openapi.yaml",
        include_str!("../../specs/media-streaming-rate/vwip/openapi.yaml"),
    ),
    (
        "/network-health-assessment/vwip/openapi.yaml",
        include_str!("../../specs/network-health-assessment/vwip/openapi.yaml"),
    ),
    (
        "/network-traffic-analysis/vwip/openapi.yaml",
        include_str!("../../specs/network-traffic-analysis/vwip/openapi.yaml"),
    ),
    (
        "/optimal-edge-discovery/vwip/openapi.yaml",
        include_str!("../../specs/optimal-edge-discovery/vwip/openapi.yaml"),
    ),
    (
        "/verified-caller/vwip/openapi.yaml",
        include_str!("../../specs/verified-caller/vwip/openapi.yaml"),
    ),
    (
        "/application-profiles/vwip/openapi.yaml",
        include_str!("../../specs/application-profiles/vwip/openapi.yaml"),
    ),
    (
        "/subscription-status/vwip/openapi.yaml",
        include_str!("../../specs/subscription-status/vwip/openapi.yaml"),
    ),
    (
        "/device-authenticity/vwip/openapi.yaml",
        include_str!("../../specs/device-authenticity/vwip/openapi.yaml"),
    ),
    (
        "/session-insights/vwip/openapi.yaml",
        include_str!("../../specs/session-insights/vwip/openapi.yaml"),
    ),
    (
        "/consent-info/vwip/openapi.yaml",
        include_str!("../../specs/consent-info/vwip/openapi.yaml"),
    ),
    (
        "/iot-sim-fraud-prevention/vwip/openapi.yaml",
        include_str!("../../specs/iot-sim-fraud-prevention/vwip/openapi.yaml"),
    ),
    (
        "/sponsored-data/vwip/openapi.yaml",
        include_str!("../../specs/sponsored-data/vwip/openapi.yaml"),
    ),
    (
        "/click-to-dial/vwip/openapi.yaml",
        include_str!("../../specs/click-to-dial/vwip/openapi.yaml"),
    ),
    (
        "/most-frequent-location/vwip/openapi.yaml",
        include_str!("../../specs/most-frequent-location/vwip/openapi.yaml"),
    ),
    (
        "/traffic-influence/vwip/openapi.yaml",
        include_str!("../../specs/traffic-influence/vwip/openapi.yaml"),
    ),
    (
        "/application-endpoint-discovery/vwip/openapi.yaml",
        include_str!("../../specs/application-endpoint-discovery/vwip/openapi.yaml"),
    ),
    (
        "/application-endpoint-registration/vwip/openapi.yaml",
        include_str!("../../specs/application-endpoint-registration/vwip/openapi.yaml"),
    ),
    (
        "/predictive-connectivity-data/vwip/openapi.yaml",
        include_str!("../../specs/predictive-connectivity-data/vwip/openapi.yaml"),
    ),
    (
        "/network-access-devices/vwip/openapi.yaml",
        include_str!("../../specs/network-access-devices/vwip/openapi.yaml"),
    ),
    (
        "/sms/v0alpha1/openapi.yaml",
        include_str!("../../specs/sms/v0alpha1/openapi.yaml"),
    ),
    (
        "/capabilities-and-restrictions/vwip/openapi.yaml",
        include_str!("../../specs/capabilities-and-restrictions/vwip/openapi.yaml"),
    ),
    (
        "/dedicated-network-profiles/vwip/openapi.yaml",
        include_str!("../../specs/dedicated-network-profiles/vwip/openapi.yaml"),
    ),
    (
        "/dedicated-network/vwip/openapi.yaml",
        include_str!("../../specs/dedicated-network/vwip/openapi.yaml"),
    ),
    (
        "/dedicated-network-accesses/vwip/openapi.yaml",
        include_str!("../../specs/dedicated-network-accesses/vwip/openapi.yaml"),
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

/// The URL paths of every **API** OpenAPI spec served — every entry in [`SPECS`]
/// except the shared `/auth` and `/shared` `$ref` fragments (which are serving
/// infrastructure, not catalogued business APIs).
///
/// Exposed as the single source of truth so the `/` catalog can be checked
/// against the specs actually served: the two hand-maintained lists must agree
/// (docs/DESIGN.md §9 — every mounted API is catalogued and its `spec_url`
/// resolves), and a test asserts the sets are equal. Test-support only — it has
/// no role on the request path, so it is compiled only under `cfg(test)`.
#[cfg(test)]
pub fn api_spec_urls() -> impl Iterator<Item = &'static str> {
    SPECS
        .iter()
        .map(|&(path, _)| path)
        .filter(|path| !path.starts_with("/auth/") && !path.starts_with("/shared/"))
}

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
        // Driven by [`api_spec_urls`] (the single source of truth) rather than a
        // duplicated path list, so a newly served API can never be silently
        // omitted from this check. Each API spec URL resolves and looks like an
        // OpenAPI doc.
        let mut count = 0;
        for path in api_spec_urls() {
            count += 1;
            let (status, content_type, body) = fetch(path).await;
            assert_eq!(status, StatusCode::OK, "spec {path} should be served");
            assert_eq!(content_type, YAML_CONTENT_TYPE, "spec {path} content-type");
            assert!(body.starts_with("#") || body.contains("openapi:"), "spec {path} is YAML");
        }
        // Sanity: the source of truth is non-empty (guards a broken filter).
        assert!(count >= 28, "expected the full API-spec catalog, got {count}");
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
