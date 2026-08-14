//! Network Access Domains **vwip** (CAMARA NetworkAccessManagement / Network
//! Access Domains, wip).
//!
//! Endpoints so far:
//! - `GET /network-access-domains/vwip/trust-domains/capabilities` — the
//!   provider-level Trust Domain capabilities document (operationId
//!   `getTrustDomainCapabilities`).
//! - `GET /network-access-domains/vwip/services` — the caller's **Services**
//!   catalog (operationId `getServices`): the logical commercial subscriptions /
//!   account relationships the authenticated identity holds.
//!
//! ## What they do
//!
//! `getTrustDomainCapabilities` returns the set of Trust Domain configuration
//! capabilities this API provider supports: which access types (Wi-Fi
//! WPA-Personal/Enterprise, Thread) and their properties, plus the policy limits
//! (max devices, per-domain up/downstream bandwidth bands, egress allow-list
//! constraints) a caller may configure when creating a Trust Domain. It requires
//! the `network-access-domains:trust-domains` scope.
//!
//! `getServices` returns the caller's `ServiceList` — the services (each a
//! logical commercial subscription tied to a `serviceSite`) associated with the
//! authenticated identity. It requires the `network-access-domains:services:read`
//! scope.
//!
//! Both are protected: they require a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the endpoint's scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! **`getTrustDomainCapabilities`** — provider capabilities are fixed operator
//! configuration, not keyed to any device or subscriber, so this operation has
//! **no** parameter-driven control plane: given a scoped token it always returns
//! the same deterministic document (`200`).
//!
//! **`getServices`** has no request body either, but it *is* keyed to the
//! authenticated identity, so its control plane is the **token subject** (DESIGN
//! §7, mirroring the other subject-keyed reads): a reserved error suffix on the
//! subject → the canonical CAMARA error; otherwise the subject's trailing three
//! digits `d` fix the catalog deterministically — `d == 0` (`…000` / no digits) →
//! `200 []` (nothing associated; a list never 404s), else `((d - 1) % 3) + 1`
//! services (1–3), each with a deterministic UUID-shaped `id` and `serviceSite`.
//!
//! For both, a missing/invalid token → `401 UNAUTHENTICATED`; a token without the
//! endpoint's scope → `403 PERMISSION_DENIED` (both from the shared
//! resource-server layer). `x-correlator` is echoed on every response.

use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::scenarios;

/// The OAuth2 scope the `GET /trust-domains/capabilities` endpoint requires
/// (CAMARA NetworkAccessManagement / Network Access Domains).
const CAPABILITIES_SCOPE: &str = "network-access-domains:trust-domains";

/// The OAuth2 scope the `GET /services` endpoint requires.
const SERVICES_SCOPE: &str = "network-access-domains:services:read";

/// Routes for Network Access Domains vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/network-access-domains/vwip/trust-domains/capabilities",
            get(get_capabilities),
        )
        .route(
            "/network-access-domains/vwip/services",
            get(get_services),
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

/// `GET /network-access-domains/vwip/services`.
///
/// Returns the caller's `ServiceList` — the services associated with the
/// authenticated identity. Keyed on the token subject (DESIGN §7): a reserved
/// error suffix → the canonical CAMARA error; otherwise the subject's trailing
/// three digits fix the catalog deterministically.
async fn get_services(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SERVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The authenticated identity is the token subject (client_credentials sets
    // `sub` = client_id; the three-legged grants set a user subject). Reserved
    // error suffix on that identity → the canonical CAMARA error (DESIGN §7).
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    let list = Value::Array(services_for(identity));
    with_correlator((StatusCode::OK, Json(list)).into_response(), &correlator)
}

/// Fixed service templates `(name, description, siteName, siteDescription)`, one
/// per catalog slot. The count and per-slot selection are chosen deterministically
/// from the identity (below), so the catalog is reproducible per caller.
const SERVICE_TEMPLATES: [(&str, &str, &str, &str); 3] = [
    (
        "Main Home Internet",
        "Internet subscription for primary residence",
        "Primary Residence",
        "123 Main Street",
    ),
    (
        "Vacation Home Internet",
        "Internet subscription for vacation home",
        "Vacation Home",
        "45 Lakeside Road",
    ),
    (
        "Small Business Fibre",
        "Fibre subscription for a small business site",
        "Downtown Office",
        "500 Market Avenue",
    ),
];

/// The `ServiceList` (0..=3 `Service` records) for an authenticated `identity`.
///
/// Deterministic from the identity's trailing three digits `d` (DESIGN §7):
/// `d == 0` (or no digits) → an empty list (nothing associated; a list never
/// 404s), else `((d - 1) % 3) + 1` services (1–3).
fn services_for(identity: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(identity).unwrap_or(0);
    if d == 0 {
        return Vec::new();
    }
    let count = ((d - 1) % 3) + 1; // 1..=3
    (0..count as usize).map(|i| service(identity, i)).collect()
}

/// A single deterministic `Service` for `identity` at catalog slot `i`.
fn service(identity: &str, i: usize) -> Value {
    let (name, description, site_name, site_description) =
        SERVICE_TEMPLATES[i % SERVICE_TEMPLATES.len()];
    json!({
        "id": deterministic_uuid("nad-service", identity, i),
        "name": name,
        "description": description,
        "serviceSite": {
            "id": deterministic_uuid("nad-service-site", identity, i),
            "name": site_name,
            "description": site_description
        }
    })
}

/// A deterministic, UUID-shaped id from the first 16 bytes of a domain-tagged
/// SHA-256 over `(tag, identity, index)` (distinct tags never collide). Mirrors
/// Blockchain Public Address's record-id rendering; no `uuid`/`rand` dependency.
fn deterministic_uuid(tag: &str, identity: &str, index: usize) -> String {
    let h = Sha256::digest(format!("{tag}:{identity}:{index}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
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
        mint_token_as("nad-client", scope).await
    }

    /// Mint a token whose subject (`sub` = `client_id`) is `client_id` — lets a
    /// test choose the identity the subject-keyed `getServices` reads.
    async fn mint_token_as(client_id: &str, scope: &str) -> String {
        let body =
            format!("grant_type=client_credentials&client_id={client_id}&scope={scope}");
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

    // === getServices ======================================================

    // --- Pure unit: the deterministic catalog ------------------------------

    #[test]
    fn services_count_is_deterministic_from_trailing_digits() {
        // `…000` / no digits → empty list.
        assert!(services_for("nad-000").is_empty());
        assert!(services_for("no-digits").is_empty());
        // d>0 → ((d-1) % 3) + 1 services (1..=3).
        assert_eq!(services_for("nad-001").len(), 1); // (0%3)+1 = 1
        assert_eq!(services_for("nad-005").len(), 2); // (4%3)+1 = 2
        assert_eq!(services_for("nad-003").len(), 3); // (2%3)+1 = 3
    }

    #[test]
    fn services_are_well_shaped_and_deterministic() {
        let a = services_for("nad-005");
        let b = services_for("nad-005");
        assert_eq!(a, b, "same identity yields the same catalog");

        for svc in &a {
            // `id` is required and UUID-shaped; `serviceSite.id` likewise.
            let id = svc["id"].as_str().unwrap();
            assert!(is_uuid_shaped(id), "service id {id}");
            assert!(svc["name"].is_string());
            let site = &svc["serviceSite"];
            let site_id = site["id"].as_str().unwrap();
            assert!(is_uuid_shaped(site_id), "site id {site_id}");
            // A service id and its site id are drawn from distinct SHA-256 tags.
            assert_ne!(id, site_id);
            assert!(site["name"].is_string());
        }

        // A different identity yields a different first id.
        assert_ne!(a[0]["id"], services_for("nad-006")[0]["id"]);
    }

    /// Whether `s` is a lowercase-hex, hyphenated 8-4-4-4-12 UUID string.
    fn is_uuid_shaped(s: &str) -> bool {
        let parts: Vec<&str> = s.split('-').collect();
        parts.len() == 5
            && [8, 4, 4, 4, 12] == [
                parts[0].len(),
                parts[1].len(),
                parts[2].len(),
                parts[3].len(),
                parts[4].len(),
            ]
            && s.chars().all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    // --- Integration through the real router -------------------------------

    /// GET `/services` with an optional Bearer token and optional `x-correlator`.
    async fn get_services_req(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-domains/vwip/services")
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
    async fn services_returns_the_catalog_for_the_identity() {
        // sub = client_id = "nad-005" → digits 005 → 2 services.
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let list = body.as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0]["id"].is_string());
        assert!(list[0]["serviceSite"]["id"].is_string());
    }

    #[tokio::test]
    async fn services_is_empty_for_a_zero_tail_identity() {
        // sub = "nad-000" → digits 000 → empty list (a list never 404s).
        let token = mint_token_as("nad-000", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn services_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404 NOT_FOUND.
        let token = mint_token_as("nad-404", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn services_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn services_missing_token_is_unauthenticated() {
        let (status, _, body) = get_services_req(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn services_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, headers, _) = get_services_req(Some(&token), Some("corr-svc")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-svc")
        );
    }
}
