//! Dedicated Network — Areas **vwip** (CAMARA DedicatedNetworks,
//! work-in-progress).
//!
//! Endpoint:
//! - `GET /dedicated-network-areas/vwip/areas/{areaId}` — look up a single
//!   network service area by its id (operationId `readNetworkServiceArea`).
//!
//! ## What it does
//!
//! A **service area** is a named, operator-published geographical region — a
//! WGS-84 `CIRCLE` `area` (`center` + `radius`) — together with the network
//! and/or QoS profiles available within it. CamaraSim serves a **fixed catalog**
//! (there is no upstream network to query — DESIGN §7); the single-area lookup
//! returns the service area the requested `areaId` maps to, echoing that id back
//! in the body's `id`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `dedicated-network-areas:areas:read` scope. It is a two-legged
//! (`client_credentials`) catalog query — service areas exist independently of
//! any subscriber, so there is no device/line identifier and no three-legged
//! dance.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The `areaId` path parameter is the sole control plane, in three layers
//! (mirroring the sibling `readNetworkProfile`):
//!
//! 1. **Malformed id.** An `areaId` that is not UUID-shaped (the schema's
//!    `format: uuid`, `maxLength: 36`) → `400 INVALID_ARGUMENT`.
//! 2. **Reserved error suffix (identifier).** A UUID contains digits, so the
//!    shared reserved-error convention applies unchanged ([`crate::scenarios`]):
//!    if the id's trailing three digits name a reserved CAMARA status (`…400`,
//!    `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`) the
//!    endpoint answers with that canonical CAMARA error — e.g.
//!    `…-000000000404` → `404 NOT_FOUND` (no such service area).
//! 3. **Service-area selection.** Otherwise the id's trailing three digits `d`
//!    (`000`–`999`, or `0` when the id has fewer than three digits) select one of
//!    a fixed set of service-area templates (`d % N`), so the returned area is a
//!    genuine second plane. The returned `id` echoes the requested `areaId`.
//!
//! Example: `…-000000000001` → the `manchester-metro` template;
//! `…-000000000404` → `404 NOT_FOUND`; `not-a-uuid` → `400`.

use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `readNetworkServiceArea` requires (CAMARA Dedicated Network —
/// Areas).
const READ_SCOPE: &str = "dedicated-network-areas:areas:read";

/// Routes for Dedicated Network — Areas vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/dedicated-network-areas/vwip/areas/:areaId",
        get(read_network_service_area),
    )
}

/// `GET /dedicated-network-areas/vwip/areas/{areaId}` — the single-area lookup
/// (`readNetworkServiceArea`).
async fn read_network_service_area(
    claims: Claims,
    headers: HeaderMap,
    Path(area_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Layer 1: the path segment must be a UUID (schema `format: uuid`).
    if !is_uuid_shaped(&area_id) {
        return invalid_argument("`areaId` must be a UUID.", &correlator);
    }

    // Layer 2: a reserved trailing-digit suffix selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&area_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Layer 3: the trailing three digits pick a fixed service-area template; the
    // requested id is echoed as the returned `id`.
    let digits = scenarios::trailing_three_digits(&area_id).unwrap_or(0);
    let area = service_area(&area_id, digits);

    with_correlator((StatusCode::OK, Json(area)).into_response(), &correlator)
}

/// The fixed catalog of service-area templates CamaraSim offers, as
/// `(name, description, center lat/long, radius m, profiles)`. Static data — a
/// real operator would source these from its dedicated-network coverage
/// configuration. Chosen to span a spread of geographies and both `profiles`
/// branches (some areas carry `qosProfiles`, some `networkProfiles`) so the
/// `areaId` selection is a meaningful control plane. Radii stay well within the
/// schema's `minimum: 1`.
const TEMPLATES: &[ServiceAreaTemplate] = &[
    ServiceAreaTemplate {
        name: "london-city",
        description: "Central London coverage cell.",
        latitude: 51.5074,
        longitude: -0.1278,
        radius: 5000.0,
        profiles: Profiles::Qos(&["QOS_L", "QOS_M"]),
    },
    ServiceAreaTemplate {
        name: "manchester-metro",
        description: "Greater Manchester metropolitan cell.",
        latitude: 53.4808,
        longitude: -2.2426,
        radius: 8000.0,
        profiles: Profiles::Qos(&["QOS_M", "QOS_S"]),
    },
    ServiceAreaTemplate {
        name: "port-of-felixstowe",
        description: "Felixstowe port private-network zone.",
        latitude: 51.9542,
        longitude: 1.3464,
        radius: 3000.0,
        // Two fixed network-profile ids (the network-profile branch of the
        // either/or). Their trailing digits are the Network Profiles catalog
        // indices, so a client could look them up there.
        profiles: Profiles::Network(&[
            "00000000-0000-4000-8000-000000000000",
            "00000000-0000-4000-8000-000000000001",
        ]),
    },
    ServiceAreaTemplate {
        name: "edinburgh-campus",
        description: "Edinburgh enterprise-campus cell.",
        latitude: 55.9533,
        longitude: -3.1883,
        radius: 2000.0,
        profiles: Profiles::Qos(&["QOS_E"]),
    },
];

/// A static service-area template. Rendered to a CAMARA `ServiceArea` by
/// [`service_area`], which stamps in the caller-supplied `id`.
struct ServiceAreaTemplate {
    name: &'static str,
    description: &'static str,
    latitude: f64,
    longitude: f64,
    radius: f64,
    profiles: Profiles,
}

/// A service area carries **either** a list of QoS-profile names **or** a list of
/// network-profile ids (the CAMARA `ServiceArea` either/or constraint).
enum Profiles {
    Qos(&'static [&'static str]),
    Network(&'static [&'static str]),
}

/// The stable canonical id for template `index`: a UUID whose trailing three
/// digits equal the index, so a client can address a specific template
/// deterministically (`service_area` selects `digits % N`). Test-only for now —
/// the `readNetworkServiceArea` slice never mints ids (it echoes the requested
/// `areaId`); the collection query that would surface them is a later pass.
#[cfg(test)]
fn template_id(index: usize) -> String {
    format!("00000000-0000-4000-8000-000000000{index:03}")
}

/// Render the `ServiceArea` an `areaId` maps to: the `digits % N` template, with
/// the requested `id` echoed back (the schema requires `id` be the area's own
/// identifier), and a `CIRCLE` `area`.
fn service_area(area_id: &str, digits: u16) -> Value {
    let t = &TEMPLATES[(digits as usize) % TEMPLATES.len()];
    let mut area = json!({
        "id": area_id,
        "name": t.name,
        "description": t.description,
        "area": {
            "areaType": "CIRCLE",
            "center": {
                "latitude": t.latitude,
                "longitude": t.longitude,
            },
            "radius": t.radius,
        },
    });
    // Exactly one of the two profile branches (the either/or constraint).
    match &t.profiles {
        Profiles::Qos(names) => {
            area["qosProfiles"] = json!(names);
        }
        Profiles::Network(ids) => {
            area["networkProfiles"] = json!(ids);
        }
    }
    area
}

/// Whether `s` is UUID-shaped: five hyphen-separated hex groups of lengths
/// `8-4-4-4-12` (the schema's `format: uuid`). Mirrors the other UUID-keyed APIs.
fn is_uuid_shaped(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
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

    const HOST: &str = "dedicatednet.local:8080";

    // A UUID whose trailing three digits are `d` (zero-padded), so a test can
    // aim at a specific reserved suffix or template.
    fn uuid_ending(d: u16) -> String {
        format!("00000000-0000-4000-8000-000000000{d:03}")
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_shape_matches_the_schema() {
        assert!(is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid_shaped("3FA85F64-5717-4562-B3FC-2C963F66AFA6"));
        assert!(is_uuid_shaped(&uuid_ending(1)));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa")); // 11 in last group
        assert!(!is_uuid_shaped("zzzzzzzz-5717-4562-b3fc-2c963f66afa6")); // non-hex
    }

    #[test]
    fn every_template_is_a_valid_service_area() {
        // Each rendered template carries every required field, a well-formed
        // CIRCLE area, and exactly one of the profile branches.
        for (i, _) in TEMPLATES.iter().enumerate() {
            let id = uuid_ending(i as u16);
            let a = service_area(&id, i as u16);
            assert_eq!(a["id"], id, "id echoes the request");
            assert!(a["name"].is_string());
            assert_eq!(a["area"]["areaType"], "CIRCLE");
            let lat = a["area"]["center"]["latitude"].as_f64().unwrap();
            let lon = a["area"]["center"]["longitude"].as_f64().unwrap();
            assert!((-90.0..=90.0).contains(&lat), "latitude in range");
            assert!((-180.0..=180.0).contains(&lon), "longitude in range");
            assert!(a["area"]["radius"].as_f64().unwrap() >= 1.0, "radius >= 1");

            // Exactly one of networkProfiles / qosProfiles, and it is non-empty.
            let has_qos = a.get("qosProfiles").is_some();
            let has_net = a.get("networkProfiles").is_some();
            assert!(has_qos ^ has_net, "exactly one profile branch");
            let list = if has_qos {
                a["qosProfiles"].as_array().unwrap()
            } else {
                a["networkProfiles"].as_array().unwrap()
            };
            assert!(!list.is_empty(), "at least one profile");
        }
    }

    #[test]
    fn digit_selection_covers_every_template_and_wraps() {
        // Distinct low digits pick distinct templates; the index wraps mod N.
        let n = TEMPLATES.len() as u16;
        for d in 0..n {
            let a = service_area(&uuid_ending(d), d);
            assert_eq!(a["name"], TEMPLATES[d as usize].name);
        }
        // d == N wraps back to template 0.
        let a = service_area(&uuid_ending(n), n);
        assert_eq!(a["name"], TEMPLATES[0].name);
    }

    #[test]
    fn template_id_tail_selects_its_own_template() {
        // A canonical template id round-trips through the selector: the id whose
        // tail is the catalog index picks that same template.
        for i in 0..TEMPLATES.len() {
            let id = template_id(i);
            assert!(is_uuid_shaped(&id), "template id is UUID-shaped");
            let digits = scenarios::trailing_three_digits(&id).unwrap_or(0);
            let a = service_area(&id, digits);
            assert_eq!(a["name"], TEMPLATES[i].name, "id tail {i} selects template {i}");
            assert_eq!(a["id"], id);
        }
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=dn-client&scope={scope}");
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

    async fn get_area(
        token: Option<&str>,
        area_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/dedicated-network-areas/vwip/areas/{area_id}"))
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

    async fn get_ok(area_id: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        get_area(Some(&token), area_id, None).await
    }

    #[tokio::test]
    async fn happy_path_returns_the_selected_service_area() {
        let id = uuid_ending(1);
        let (status, _, body) = get_ok(&id).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_object());
        assert_eq!(body["id"], id);
        assert_eq!(body["name"], "manchester-metro");
        assert_eq!(body["area"]["areaType"], "CIRCLE");
        assert!(body["area"]["center"]["latitude"].is_number());
        assert!(body["area"]["radius"].is_number());
        assert!(body["qosProfiles"].as_array().unwrap().contains(&json!("QOS_M")));
    }

    #[tokio::test]
    async fn network_profile_branch_is_rendered() {
        // Template 2 (port-of-felixstowe) uses the networkProfiles branch.
        let (status, _, body) = get_ok(&uuid_ending(2)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "port-of-felixstowe");
        assert!(body.get("networkProfiles").is_some(), "networkProfiles present");
        assert!(body.get("qosProfiles").is_none(), "qosProfiles absent");
        assert!(is_uuid_shaped(
            body["networkProfiles"][0].as_str().unwrap()
        ));
    }

    #[tokio::test]
    async fn different_ids_select_different_areas() {
        let (_, _, a) = get_ok(&uuid_ending(1)).await; // manchester-metro
        let (_, _, b) = get_ok(&uuid_ending(2)).await; // port-of-felixstowe
        assert_ne!(a["name"], b["name"]);
        assert_eq!(a["name"], "manchester-metro");
        assert_eq!(b["name"], "port-of-felixstowe");
    }

    #[tokio::test]
    async fn no_significant_digits_selects_the_first_template() {
        // A UUID whose only digits are 0/4/8 in fixed positions → tail 000 → t0.
        let id = "abcdefab-0000-4000-8000-abcdefabcdef";
        let (status, _, body) = get_ok(id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], TEMPLATES[0].name);
        assert_eq!(body["id"], id);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = get_ok(&uuid_ending(404)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = get_ok(&uuid_ending(429)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");

        let (status, _, body) = get_ok(&uuid_ending(422)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn malformed_area_id_is_400() {
        let (status, _, body) = get_ok("not-a-uuid").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_area(Some(&token), &uuid_ending(1), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_area(None, &uuid_ending(1), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            get_area(Some(&token), &uuid_ending(1), Some("corr-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        let (status, headers, _) =
            get_area(Some(&token), &uuid_ending(404), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
