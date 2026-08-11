//! Dedicated Network — Areas **vwip** (CAMARA DedicatedNetworks,
//! work-in-progress).
//!
//! Endpoints:
//! - `GET /dedicated-network-areas/vwip/areas/{areaId}` — look up a single
//!   network service area by its id (operationId `readNetworkServiceArea`).
//! - `POST /dedicated-network-areas/vwip/retrieve-service-areas` — list the
//!   service-area catalog, narrowed by optional search filters (operationId
//!   `retrieveNetworkServiceAreas`).
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
//! The **collection query** (`retrieveNetworkServiceAreas`) returns the catalog
//! narrowed by the request body's optional filters, ANDed together: spatial
//! (`atLocation` — a point inside the area; `overlappingArea` — a CIRCLE that
//! intersects the area; `coveringArea` — a CIRCLE the area fully contains) and
//! attribute (`byName`, `byNetworkProfileId`, `byQosProfileName`). Each returned
//! area carries its stable canonical `id`, so a client can then re-address it via
//! `readNetworkServiceArea`. Like every CAMARA list it never `404`s — an
//! over-narrow filter simply yields `[]` — so the request body is the sole
//! control plane (DESIGN §7) and there is no reserved-identifier error plane.
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

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the Dedicated Network — Areas read operations require (CAMARA
/// Dedicated Network — Areas). Both `readNetworkServiceArea` and
/// `retrieveNetworkServiceAreas` share the same read scope.
const READ_SCOPE: &str = "dedicated-network-areas:areas:read";

/// Routes for Dedicated Network — Areas vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/dedicated-network-areas/vwip/areas/:areaId",
            get(read_network_service_area),
        )
        .route(
            "/dedicated-network-areas/vwip/retrieve-service-areas",
            post(retrieve_network_service_areas),
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

/// `POST /dedicated-network-areas/vwip/retrieve-service-areas` — the collection
/// query (`retrieveNetworkServiceAreas`).
///
/// Lists the service-area catalog, narrowed by the optional search filters in the
/// request body. Unlike the single-area lookup this operation never `404`s — a
/// filter that matches nothing returns an empty array (CAMARA lists never 404 on
/// an empty result), so there is no reserved-identifier error plane; the request
/// body is the control plane (DESIGN §7). Each returned area carries its stable
/// canonical `id` (so a client can then address it via `readNetworkServiceArea`).
async fn retrieve_network_service_areas(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (all filters optional); anything present must parse.
    let req: RetrieveRequest = if body.is_empty() {
        RetrieveRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrieveServiceAreasRequest.",
                    &correlator,
                )
            }
        }
    };

    // Validate the (present) filters before scanning the catalog.
    let filters = match Filters::validate(&req, &correlator) {
        Ok(f) => f,
        Err(resp) => return resp,
    };

    // Apply every present filter (AND) over the fixed catalog; render survivors
    // with their canonical ids.
    let areas: Vec<Value> = TEMPLATES
        .iter()
        .enumerate()
        .filter(|(_, t)| filters.matches(t))
        .map(|(i, t)| render_area(&template_id(i), t))
        .collect();

    with_correlator((StatusCode::OK, Json(Value::Array(areas))).into_response(), &correlator)
}

/// The `retrieveNetworkServiceAreas` request body — a set of optional search
/// filters. Absent fields do not constrain the result; present fields combine
/// with AND. Unknown fields are ignored (CamaraSim only models the filters it
/// serves; `atLocation`/`overlappingArea`/`coveringArea` are CIRCLE-only here).
#[derive(Deserialize, Default)]
struct RetrieveRequest {
    #[serde(rename = "atLocation")]
    at_location: Option<QueryPoint>,
    #[serde(rename = "overlappingArea")]
    overlapping_area: Option<QueryArea>,
    #[serde(rename = "coveringArea")]
    covering_area: Option<QueryArea>,
    #[serde(rename = "byName")]
    by_name: Option<String>,
    #[serde(rename = "byNetworkProfileId")]
    by_network_profile_id: Option<String>,
    #[serde(rename = "byQosProfileName")]
    by_qos_profile_name: Option<String>,
}

/// A WGS-84 point in a request filter (the `atLocation` filter, and the `center`
/// of a query `Area`).
#[derive(Deserialize, Clone, Copy)]
struct QueryPoint {
    latitude: f64,
    longitude: f64,
}

/// A geographical `Area` in a request filter. CamaraSim serves and filters over
/// `CIRCLE` areas only (matching its fixed catalog); a non-`CIRCLE` `areaType`
/// (e.g. `POLYGON`) is a documented cut → `400 INVALID_ARGUMENT`.
#[derive(Deserialize)]
struct QueryArea {
    #[serde(rename = "areaType")]
    area_type: Option<String>,
    center: Option<QueryPoint>,
    radius: Option<f64>,
}

/// A validated query `CIRCLE`: a centre and a radius in metres.
#[derive(Clone, Copy)]
struct Circle {
    center: QueryPoint,
    radius: f64,
}

/// The validated, ready-to-apply search filters. Each `Option` that is `Some`
/// constrains the result; `matches` ANDs them together over one template.
struct Filters {
    at_location: Option<QueryPoint>,
    overlapping: Option<Circle>,
    covering: Option<Circle>,
    by_name: Option<String>,
    by_network_profile_id: Option<String>,
    by_qos_profile_name: Option<String>,
}

impl Filters {
    /// Validate every present filter, mapping the first violation to a CAMARA
    /// error response (`400 INVALID_ARGUMENT` for malformed values, `400
    /// OUT_OF_RANGE` for out-of-range coordinates / radii).
    fn validate(req: &RetrieveRequest, correlator: &Option<HeaderValue>) -> Result<Self, Response> {
        // atLocation: a point, coordinates in range.
        if let Some(p) = &req.at_location {
            check_point(p, correlator)?;
        }

        // overlappingArea / coveringArea: CIRCLE-only, valid centre and radius.
        let overlapping = validate_area(req.overlapping_area.as_ref(), correlator)?;
        let covering = validate_area(req.covering_area.as_ref(), correlator)?;

        // byName: the CAMARA schema caps it at 1024 characters.
        if let Some(name) = &req.by_name {
            if name.chars().count() > 1024 {
                return Err(invalid_argument(
                    "`byName` must be at most 1024 characters.",
                    correlator,
                ));
            }
        }

        // byNetworkProfileId: a UUID (NetworkProfileId, format: uuid).
        if let Some(id) = &req.by_network_profile_id {
            if !is_uuid_shaped(id) {
                return Err(invalid_argument(
                    "`byNetworkProfileId` must be a UUID.",
                    correlator,
                ));
            }
        }

        // byQosProfileName: the CAMARA QosProfileName pattern/length.
        if let Some(name) = &req.by_qos_profile_name {
            if !is_valid_qos_profile_name(name) {
                return Err(invalid_argument(
                    "`byQosProfileName` must match ^[a-zA-Z0-9_.-]+$ and be 3–256 characters.",
                    correlator,
                ));
            }
        }

        Ok(Filters {
            at_location: req.at_location,
            overlapping,
            covering,
            by_name: req.by_name.clone(),
            by_network_profile_id: req.by_network_profile_id.clone(),
            by_qos_profile_name: req.by_qos_profile_name.clone(),
        })
    }

    /// Whether a template satisfies every present filter (AND).
    fn matches(&self, t: &ServiceAreaTemplate) -> bool {
        // byName: exact name match.
        if let Some(name) = &self.by_name {
            if t.name != name {
                return false;
            }
        }
        // byQosProfileName: the area must carry that QoS profile.
        if let Some(name) = &self.by_qos_profile_name {
            let carried = matches!(&t.profiles, Profiles::Qos(names) if names.contains(&name.as_str()));
            if !carried {
                return false;
            }
        }
        // byNetworkProfileId: the area must carry that network profile.
        if let Some(id) = &self.by_network_profile_id {
            let carried = matches!(&t.profiles, Profiles::Network(ids) if ids.contains(&id.as_str()));
            if !carried {
                return false;
            }
        }
        // atLocation: the point lies within the area's circle.
        if let Some(p) = &self.at_location {
            if haversine_m(p.latitude, p.longitude, t.latitude, t.longitude) > t.radius {
                return false;
            }
        }
        // overlappingArea: the query circle and the area's circle intersect
        // (centre distance ≤ sum of radii).
        if let Some(c) = &self.overlapping {
            let d = haversine_m(c.center.latitude, c.center.longitude, t.latitude, t.longitude);
            if d > c.radius + t.radius {
                return false;
            }
        }
        // coveringArea: the area's circle fully contains the query circle
        // (centre distance + query radius ≤ area radius).
        if let Some(c) = &self.covering {
            let d = haversine_m(c.center.latitude, c.center.longitude, t.latitude, t.longitude);
            if d + c.radius > t.radius {
                return false;
            }
        }
        true
    }
}

/// Validate a query `Area` filter (if present) down to a `Circle`. CamaraSim
/// filters over `CIRCLE` areas only: `areaType` must be `CIRCLE`, with a valid
/// `center` (coordinates in range) and a `radius` ≥ 1 m.
fn validate_area(
    area: Option<&QueryArea>,
    correlator: &Option<HeaderValue>,
) -> Result<Option<Circle>, Response> {
    let Some(area) = area else { return Ok(None) };
    // Only CIRCLE query areas are modelled (POLYGON is a documented cut).
    if area.area_type.as_deref() != Some("CIRCLE") {
        return Err(invalid_argument(
            "Only `areaType: CIRCLE` query areas are supported.",
            correlator,
        ));
    }
    let Some(center) = area.center else {
        return Err(invalid_argument(
            "A CIRCLE area requires a `center`.",
            correlator,
        ));
    };
    check_point(&center, correlator)?;
    let Some(radius) = area.radius else {
        return Err(invalid_argument(
            "A CIRCLE area requires a `radius`.",
            correlator,
        ));
    };
    if radius < 1.0 {
        return Err(out_of_range("`radius` must be at least 1 metre.", correlator));
    }
    Ok(Some(Circle { center, radius }))
}

/// Validate a WGS-84 point's coordinate ranges (`latitude` ∈ [-90, 90],
/// `longitude` ∈ [-180, 180]) → `400 OUT_OF_RANGE` when out of range.
fn check_point(p: &QueryPoint, correlator: &Option<HeaderValue>) -> Result<(), Response> {
    if !(-90.0..=90.0).contains(&p.latitude) || !(-180.0..=180.0).contains(&p.longitude) {
        return Err(out_of_range(
            "`latitude` must be in [-90, 90] and `longitude` in [-180, 180].",
            correlator,
        ));
    }
    Ok(())
}

/// Great-circle distance in metres between two WGS-84 points (the haversine
/// formula, spherical Earth). Self-contained — no geospatial dependency; the
/// small spherical-vs-ellipsoidal error is immaterial for the fixed catalog.
fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    /// Mean Earth radius in metres (IUGG).
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let (rlat1, rlat2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + rlat1.cos() * rlat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().atan2((1.0 - h).sqrt())
}

/// Whether `s` satisfies the CAMARA `QosProfileName` schema: `^[a-zA-Z0-9_.-]+$`,
/// length 3–256.
fn is_valid_qos_profile_name(s: &str) -> bool {
    let len = s.chars().count();
    (3..=256).contains(&len) && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
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
/// deterministically (`service_area` selects `digits % N`). The collection query
/// (`retrieveNetworkServiceAreas`) mints these ids on the areas it returns, so a
/// client can then address any of them through the single-area `readNetworkServiceArea`
/// lookup and get the same area back.
fn template_id(index: usize) -> String {
    format!("00000000-0000-4000-8000-000000000{index:03}")
}

/// Render the `ServiceArea` an `areaId` maps to: the `digits % N` template, with
/// the requested `id` echoed back (the schema requires `id` be the area's own
/// identifier), and a `CIRCLE` `area`.
fn service_area(area_id: &str, digits: u16) -> Value {
    render_area(area_id, &TEMPLATES[(digits as usize) % TEMPLATES.len()])
}

/// Render a `ServiceArea` from a template, stamping in the supplied `id` and a
/// `CIRCLE` `area`. Shared by the single-area lookup (which echoes the requested
/// `areaId`) and the collection query (which mints each area's canonical id).
fn render_area(id: &str, t: &ServiceAreaTemplate) -> Value {
    let mut area = json!({
        "id": id,
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

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed.
fn out_of_range(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, "OUT_OF_RANGE", message).into_response(),
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

    // --- Collection query: retrieveNetworkServiceAreas ---------------------

    // POST a (optional) JSON body to the collection endpoint.
    async fn post_retrieve(
        token: Option<&str>,
        body: Option<Value>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/dedicated-network-areas/vwip/retrieve-service-areas")
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = match body {
            Some(v) => builder
                .header("content-type", "application/json")
                .body(Body::from(v.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    async fn retrieve_ok(body: Option<Value>) -> (StatusCode, Value) {
        let token = mint_token(READ_SCOPE).await;
        let (status, _, json) = post_retrieve(Some(&token), body, None).await;
        (status, json)
    }

    // Collect the `name` of every area in a result array.
    fn names(body: &Value) -> Vec<String> {
        body.as_array()
            .unwrap()
            .iter()
            .map(|a| a["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[tokio::test]
    async fn empty_body_returns_the_whole_catalog_with_canonical_ids() {
        // No filters (empty body) → every template, each carrying its canonical id.
        let (status, body) = retrieve_ok(None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), TEMPLATES.len());
        for (i, a) in arr.iter().enumerate() {
            assert_eq!(a["id"], template_id(i), "area {i} carries its canonical id");
            assert_eq!(a["name"], TEMPLATES[i].name);
            assert_eq!(a["area"]["areaType"], "CIRCLE");
        }
    }

    #[tokio::test]
    async fn empty_json_object_is_also_the_whole_catalog() {
        // An explicit `{}` body means "no filters" too.
        let (status, body) = retrieve_ok(Some(json!({}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), TEMPLATES.len());
    }

    #[tokio::test]
    async fn returned_ids_round_trip_through_the_single_area_lookup() {
        // Every id the collection mints addresses the same area via readNetworkServiceArea.
        let (_, body) = retrieve_ok(None).await;
        let token = mint_token(READ_SCOPE).await;
        for a in body.as_array().unwrap() {
            let id = a["id"].as_str().unwrap();
            let (status, _, single) = get_area(Some(&token), id, None).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(single["name"], a["name"], "id {id} round-trips");
            assert_eq!(single["id"], id);
        }
    }

    #[tokio::test]
    async fn by_name_filters_to_the_matching_area() {
        let (status, body) = retrieve_ok(Some(json!({"byName": "edinburgh-campus"}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(names(&body), vec!["edinburgh-campus"]);
    }

    #[tokio::test]
    async fn by_name_unknown_returns_empty_array_not_404() {
        let (status, body) = retrieve_ok(Some(json!({"byName": "atlantis"}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn by_qos_profile_name_is_a_second_control_plane() {
        // QOS_M is offered in both london-city and manchester-metro.
        let (status, body) = retrieve_ok(Some(json!({"byQosProfileName": "QOS_M"}))).await;
        assert_eq!(status, StatusCode::OK);
        let n = names(&body);
        assert!(n.contains(&"london-city".to_string()));
        assert!(n.contains(&"manchester-metro".to_string()));
        // The network-profile area never carries a QoS profile.
        assert!(!n.contains(&"port-of-felixstowe".to_string()));
    }

    #[tokio::test]
    async fn by_network_profile_id_filters_the_network_profile_area() {
        let (status, body) = retrieve_ok(Some(
            json!({"byNetworkProfileId": "00000000-0000-4000-8000-000000000000"}),
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(names(&body), vec!["port-of-felixstowe"]);
    }

    #[tokio::test]
    async fn at_location_inside_an_area_selects_it() {
        // A point at london-city's centre lies within its circle and no other.
        let (status, body) = retrieve_ok(Some(
            json!({"atLocation": {"latitude": 51.5074, "longitude": -0.1278}}),
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(names(&body), vec!["london-city"]);
    }

    #[tokio::test]
    async fn at_location_far_from_every_area_is_empty() {
        // Gulf of Guinea (0,0) — inside no UK service-area circle.
        let (status, body) = retrieve_ok(Some(
            json!({"atLocation": {"latitude": 0.0, "longitude": 0.0}}),
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn overlapping_area_matches_intersecting_circles() {
        // A 300 km circle over London overlaps the southern areas but not Edinburgh.
        let (status, body) = retrieve_ok(Some(json!({
            "overlappingArea": {
                "areaType": "CIRCLE",
                "center": {"latitude": 51.5074, "longitude": -0.1278},
                "radius": 300000.0
            }
        })))
        .await;
        assert_eq!(status, StatusCode::OK);
        let n = names(&body);
        assert!(n.contains(&"london-city".to_string()));
        assert!(n.contains(&"manchester-metro".to_string()));
        assert!(!n.contains(&"edinburgh-campus".to_string()), "Edinburgh is >300km away");
    }

    #[tokio::test]
    async fn covering_area_matches_only_a_containing_area() {
        // A tiny circle at london-city's centre is fully contained only by london-city.
        let (status, body) = retrieve_ok(Some(json!({
            "coveringArea": {
                "areaType": "CIRCLE",
                "center": {"latitude": 51.5074, "longitude": -0.1278},
                "radius": 100.0
            }
        })))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(names(&body), vec!["london-city"]);

        // A circle larger than every catalog area is covered by none.
        let (status, body) = retrieve_ok(Some(json!({
            "coveringArea": {
                "areaType": "CIRCLE",
                "center": {"latitude": 51.5074, "longitude": -0.1278},
                "radius": 100000.0
            }
        })))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn filters_combine_with_and() {
        // byQosProfileName QOS_M narrows to {london, manchester}; atLocation at
        // London's centre narrows further to just london-city.
        let (status, body) = retrieve_ok(Some(json!({
            "byQosProfileName": "QOS_M",
            "atLocation": {"latitude": 51.5074, "longitude": -0.1278}
        })))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(names(&body), vec!["london-city"]);
    }

    #[tokio::test]
    async fn malformed_body_is_400() {
        let token = mint_token(READ_SCOPE).await;
        // A JSON body of the wrong shape (atLocation as a string) → 400.
        let (status, _, body) =
            post_retrieve(Some(&token), Some(json!({"atLocation": "here"})), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_circle_query_area_is_400() {
        let (status, body) = retrieve_ok(Some(json!({
            "overlappingArea": {"areaType": "POLYGON", "boundary": []}
        })))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn out_of_range_coordinate_is_out_of_range() {
        let (status, body) = retrieve_ok(Some(
            json!({"atLocation": {"latitude": 200.0, "longitude": 0.0}}),
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn sub_minimum_radius_is_out_of_range() {
        let (status, body) = retrieve_ok(Some(json!({
            "coveringArea": {
                "areaType": "CIRCLE",
                "center": {"latitude": 51.5, "longitude": -0.1},
                "radius": 0.0
            }
        })))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn bad_qos_profile_name_pattern_is_400() {
        let (status, body) =
            retrieve_ok(Some(json!({"byQosProfileName": "bad name!"}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bad_network_profile_id_is_400() {
        let (status, body) =
            retrieve_ok(Some(json!({"byNetworkProfileId": "not-a-uuid"}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_without_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_retrieve(Some(&token), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_missing_token_is_unauthenticated() {
        let (status, _, body) = post_retrieve(None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn retrieve_echoes_x_correlator() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) = post_retrieve(Some(&token), None, Some("corr-list")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-list")
        );
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn haversine_matches_a_known_distance() {
        // London ↔ Manchester is ~262 km great-circle; allow a small tolerance.
        let d = haversine_m(51.5074, -0.1278, 53.4808, -2.2426);
        assert!((250_000.0..275_000.0).contains(&d), "distance was {d} m");
        // A point to itself is zero.
        assert!(haversine_m(10.0, 20.0, 10.0, 20.0) < 1.0);
    }

    #[test]
    fn qos_profile_name_validation_matches_the_schema() {
        assert!(is_valid_qos_profile_name("QOS_M"));
        assert!(is_valid_qos_profile_name("voice.5g-low"));
        assert!(!is_valid_qos_profile_name("ab")); // too short
        assert!(!is_valid_qos_profile_name("has space"));
        assert!(!is_valid_qos_profile_name("bang!"));
    }
}
