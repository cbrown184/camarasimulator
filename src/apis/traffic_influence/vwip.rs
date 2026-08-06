//! Traffic Influence **vwip** (CAMARA Traffic Influence, work-in-progress).
//!
//! Endpoints:
//! - `POST /traffic-influence/vwip/traffic-influences` — create a
//!   `TrafficInfluence` resource that steers an application's traffic toward a
//!   chosen edge-cloud placement (operationId `postTrafficInfluence`).
//! - `GET /traffic-influence/vwip/traffic-influences/{trafficInfluenceID}` —
//!   read a created resource back from the in-memory store (operationId
//!   `getTrafficInfluence`, scope `traffic-influence:traffic-influences:read`):
//!   the opaque, operator-minted id is the only control plane — a stored resource
//!   → `200` (returned verbatim), an unknown id → `404 NOT_FOUND`.
//! - `DELETE /traffic-influence/vwip/traffic-influences/{trafficInfluenceID}` —
//!   delete a created resource (operationId `deleteTrafficInfluence`, scope
//!   `traffic-influence:traffic-influences:delete`): the opaque id is again the
//!   only control plane — a stored resource → `202 Accepted` (evicted from the
//!   store, single-use), an unknown/already-deleted id → `404 NOT_FOUND`. The
//!   upstream deletion is asynchronous (`202`, resource → `deletion in
//!   progress`); the sim honours the `202` but evicts synchronously.
//!
//! ## What it does
//!
//! An API consumer names itself (`apiConsumerId`) and the application to
//! influence (`appId`), optionally pinning an `appInstanceId`, an
//! `edgeCloudRegion` / `edgeCloudZoneId` placement, and source/destination
//! traffic filters. The operator answers `201` with a freshly minted, opaque
//! `trafficInfluenceID`, the requested placement echoed back, and a `state`:
//!
//! ```json
//! {
//!   "trafficInfluenceID": "…-uuid-…",
//!   "apiConsumerId": "consumer-42",
//!   "appId": "123e4567-e89b-12d3-a456-426614174002",
//!   "state": "active"
//! }
//! ```
//!
//! The endpoint requires a valid access token ([`crate::auth::verify::Claims`])
//! carrying the `traffic-influence:traffic-influences:write` scope (declared by
//! the upstream `wip` contract). Creating a resource that can be read back later
//! makes the API stateful, so the rendered resource is persisted in the shared
//! in-memory [`super::store`].
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer:
//!
//! - **Reserved error suffix (`appId`).** The `appId` is the identifier: if its
//!   trailing three digits name a reserved CAMARA status (`…400`, `…401`, `…403`,
//!   `…404`, `…409`, `…422`, `…429`, `…500`, `…503`) the endpoint answers with
//!   that canonical CAMARA error (shared convention — [`crate::scenarios`]). A
//!   UUID is hex, so e.g. `…426614174404` selects `404 NOT_FOUND`.
//! - **Resource state (`appId`).** For a non-reserved `appId`, its trailing three
//!   digits `d` select the created resource's lifecycle `state` (a genuine second
//!   plane): `d % 3 == 0` → `ordered`, `== 1` → `created`, `== 2` → `active`
//!   (an `appId` with fewer than three digits → the `ordered` default). The
//!   `error` / `deletion in progress` / `deleted` states are lifecycle outcomes
//!   of activation and DELETE, not reachable at create (documented cut).
//!
//! Field validation returns `400 INVALID_ARGUMENT` (missing/malformed
//! `apiConsumerId` / `appId` / `appInstanceId` / `edgeCloudRegion` /
//! `edgeCloudZoneId`, or a non-JSON body) or `400 OUT_OF_RANGE` (a `sourcePort` /
//! `destinationPort` outside `0..=65535`). `x-correlator` is echoed on every
//! response.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create a Traffic Influence resource. Declared by the
/// upstream `wip` contract (`traffic-influence:traffic-influences:write`).
const WRITE_SCOPE: &str = "traffic-influence:traffic-influences:write";

/// Scope required to read a Traffic Influence resource back. Declared by the
/// upstream `wip` contract (`traffic-influence:traffic-influences:read`).
const READ_SCOPE: &str = "traffic-influence:traffic-influences:read";

/// Scope required to delete a Traffic Influence resource. Declared by the
/// upstream `wip` contract (`traffic-influence:traffic-influences:delete`).
const DELETE_SCOPE: &str = "traffic-influence:traffic-influences:delete";

/// Base path of the resource collection, used both to mount the route and to
/// build the `201` `Location` header.
const COLLECTION: &str = "/traffic-influence/vwip/traffic-influences";

/// Route for a single resource, keyed by its `trafficInfluenceID` path parameter
/// (Axum 0.6 `:param` syntax). Read back by [`get_traffic_influence`].
const ITEM: &str = "/traffic-influence/vwip/traffic-influences/:traffic_influence_id";

/// The smallest / largest TCP/UDP port a traffic filter may name (`Port` is
/// `minimum: 0`, `maximum: 65535`).
const MIN_PORT: i64 = 0;
const MAX_PORT: i64 = 65535;

/// Routes for Traffic Influence vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(COLLECTION, post(post_traffic_influence))
        .route(ITEM, get(get_traffic_influence).delete(delete_traffic_influence))
}

/// The `postTrafficInfluence` request body (`PostTrafficInfluence`). The
/// read-only `trafficInfluenceID` / `state` are intentionally absent — the
/// upstream contract says they must be ignored on create, and an unknown field
/// is ignored by serde.
#[derive(Deserialize)]
struct PostTrafficInfluence {
    #[serde(rename = "apiConsumerId")]
    api_consumer_id: Option<String>,
    #[serde(rename = "appId")]
    app_id: Option<String>,
    #[serde(rename = "appInstanceId")]
    app_instance_id: Option<String>,
    #[serde(rename = "edgeCloudRegion")]
    edge_cloud_region: Option<String>,
    #[serde(rename = "edgeCloudZoneId")]
    edge_cloud_zone_id: Option<String>,
    #[serde(rename = "sourceTrafficFilters")]
    source_traffic_filters: Option<SourceTrafficFilters>,
    #[serde(rename = "destinationTrafficFilters")]
    destination_traffic_filters: Option<DestinationTrafficFilters>,
}

#[derive(Deserialize)]
struct SourceTrafficFilters {
    #[serde(rename = "sourcePort")]
    source_port: Option<i64>,
}

#[derive(Deserialize)]
struct DestinationTrafficFilters {
    #[serde(rename = "destinationPort")]
    destination_port: Option<i64>,
    #[serde(rename = "destinationProtocol")]
    destination_protocol: Option<String>,
}

/// The validated, owned inputs a happy-path create renders and stores. Pure data,
/// so [`build_response`] is unit-testable exactly.
struct ValidInput {
    api_consumer_id: String,
    app_id: String,
    app_instance_id: Option<String>,
    edge_cloud_region: Option<String>,
    edge_cloud_zone_id: Option<String>,
    source_port: Option<i64>,
    destination_port: Option<i64>,
    destination_protocol: Option<String>,
}

/// `POST /traffic-influence/vwip/traffic-influences`.
async fn post_traffic_influence(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the write scope.
    if let Err(e) = claims.require_scope(WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory; parse strictly.
    let req: PostTrafficInfluence = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid PostTrafficInfluence.",
                &correlator,
            )
        }
    };

    // Required fields.
    let api_consumer_id = match req.api_consumer_id.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        Some(_) => return invalid_argument("`apiConsumerId` must not be empty.", &correlator),
        None => return invalid_argument("`apiConsumerId` is required.", &correlator),
    };
    let app_id = match req.app_id.as_deref() {
        Some(s) if is_uuid_any(s) => s.to_string(),
        Some(_) => return invalid_argument("`appId` must be a UUID.", &correlator),
        None => return invalid_argument("`appId` is required.", &correlator),
    };

    // Optional placement fields — validated only when present.
    let app_instance_id = match req.app_instance_id.as_deref() {
        None => None,
        Some(s) if is_uuid_any(s) => Some(s.to_string()),
        Some(_) => return invalid_argument("`appInstanceId` must be a UUID.", &correlator),
    };
    let edge_cloud_zone_id = match req.edge_cloud_zone_id.as_deref() {
        None => None,
        Some(s) if is_uuid_any(s) => Some(s.to_string()),
        Some(_) => return invalid_argument("`edgeCloudZoneId` must be a UUID.", &correlator),
    };
    let edge_cloud_region = match req.edge_cloud_region.as_deref() {
        None => None,
        Some(s) if !s.is_empty() => Some(s.to_string()),
        Some(_) => return invalid_argument("`edgeCloudRegion` must not be empty.", &correlator),
    };

    // Optional traffic filters — ports are range-checked.
    let source_port = match req.source_traffic_filters.and_then(|f| f.source_port) {
        None => None,
        Some(p) if (MIN_PORT..=MAX_PORT).contains(&p) => Some(p),
        Some(_) => return out_of_range("`sourcePort` must be between 0 and 65535.", &correlator),
    };
    let (destination_port, destination_protocol) = match req.destination_traffic_filters {
        None => (None, None),
        Some(f) => {
            let port = match f.destination_port {
                None => None,
                Some(p) if (MIN_PORT..=MAX_PORT).contains(&p) => Some(p),
                Some(_) => {
                    return out_of_range(
                        "`destinationPort` must be between 0 and 65535.",
                        &correlator,
                    )
                }
            };
            (port, f.destination_protocol)
        }
    };

    // The `appId` is the identifier and a control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&app_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Second control plane: the `appId` tail selects the created resource state.
    let state = derive_state(&app_id);

    // Mint the resource, render it, persist it, and return `201` with `Location`.
    let id = mint_id();
    let input = ValidInput {
        api_consumer_id,
        app_id,
        app_instance_id,
        edge_cloud_region,
        edge_cloud_zone_id,
        source_port,
        destination_port,
        destination_protocol,
    };
    let resource = build_response(&id, state, &input);
    super::store::insert(id.clone(), resource.clone());

    let location = format!("{COLLECTION}/{id}");
    let mut response = (StatusCode::CREATED, Json(resource)).into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("location"), value);
    }
    with_correlator(response, &correlator)
}

/// `GET /traffic-influence/vwip/traffic-influences/{trafficInfluenceID}`.
///
/// Reads back a Traffic Influence resource created by [`post_traffic_influence`].
/// The `trafficInfluenceID` is opaque and operator-minted, so it carries no
/// reserved-identifier control plane (unlike the `appId` on create): the stored
/// state is the only plane (docs/DESIGN.md §7). A resource still in the shared
/// in-memory [`super::store`] is returned verbatim (`200`); an unknown id — never
/// created, or minted in a different process — is `404 NOT_FOUND`. Mirrors
/// Quality on Demand's `getSession` and Click to Dial's `getCall`.
async fn get_traffic_influence(
    claims: Claims,
    headers: HeaderMap,
    Path(traffic_influence_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the read scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match super::store::get(&traffic_influence_id) {
        Some(resource) => {
            with_correlator((StatusCode::OK, Json(resource)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found(
                "No Traffic Influence resource found for the provided trafficInfluenceID.",
            )
            .into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /traffic-influence/vwip/traffic-influences/{trafficInfluenceID}`.
///
/// Deletes a Traffic Influence resource created by [`post_traffic_influence`],
/// addressed by its opaque, operator-minted `trafficInfluenceID`. Upstream the
/// deletion is asynchronous — the resource transitions to `deletion in progress`
/// and the operator answers `202 Accepted` — so CamaraSim honours the `202`
/// contract but evicts the resource synchronously from the shared in-memory
/// [`super::store`] (the sim has no background lifecycle worker): a `deletion in
/// progress` / `deleted` steady state is a documented cut (see the spec).
///
/// Like the read leg, the `trafficInfluenceID` is opaque, so it carries **no**
/// reserved-identifier control plane: the stored state is the only plane
/// (docs/DESIGN.md §7). A resource still in the store → `202 Accepted` (evicted,
/// single-use), so a subsequent `getTrafficInfluence` / `deleteTrafficInfluence`
/// on the same id → `404 NOT_FOUND`; an unknown id (never created, already
/// deleted, or minted in a different process) → `404 NOT_FOUND`. Mirrors Click to
/// Dial's `terminateCall` and QoS Provisioning's `revokeQosAssignment`.
async fn delete_traffic_influence(
    claims: Claims,
    headers: HeaderMap,
    Path(traffic_influence_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the delete scope.
    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    if super::store::remove(&traffic_influence_id) {
        // Async-deletion contract: 202 Accepted, no body.
        with_correlator(StatusCode::ACCEPTED.into_response(), &correlator)
    } else {
        with_correlator(
            CamaraError::not_found(
                "No Traffic Influence resource found for the provided trafficInfluenceID.",
            )
            .into_response(),
            &correlator,
        )
    }
}

/// Select the created resource's lifecycle `state` from the `appId` tail
/// (docs/DESIGN.md §7). Reserved-error suffixes are intercepted before this, so
/// `d` here is always a non-reserved tail; a UUID with fewer than three digits
/// (all-hex-letter) falls back to the `ordered` default.
fn derive_state(app_id: &str) -> &'static str {
    match scenarios::trailing_three_digits(app_id) {
        Some(d) => match d % 3 {
            0 => "ordered",
            1 => "created",
            _ => "active",
        },
        None => "ordered",
    }
}

/// Build the `201` `TrafficInfluence` representation. Pure over its inputs, so
/// the shape is unit-testable exactly. Optional placement/filter fields are
/// emitted only when they were supplied (the response schema forbids unknown
/// keys, so absent fields must stay absent).
fn build_response(id: &str, state: &str, input: &ValidInput) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("trafficInfluenceID".into(), json!(id));
    obj.insert("apiConsumerId".into(), json!(input.api_consumer_id));
    obj.insert("appId".into(), json!(input.app_id));
    obj.insert("state".into(), json!(state));
    if let Some(v) = &input.app_instance_id {
        obj.insert("appInstanceId".into(), json!(v));
    }
    if let Some(v) = &input.edge_cloud_region {
        obj.insert("edgeCloudRegion".into(), json!(v));
    }
    if let Some(v) = &input.edge_cloud_zone_id {
        obj.insert("edgeCloudZoneId".into(), json!(v));
    }
    if let Some(p) = input.source_port {
        obj.insert("sourceTrafficFilters".into(), json!({ "sourcePort": p }));
    }
    if input.destination_port.is_some() || input.destination_protocol.is_some() {
        let mut dst = serde_json::Map::new();
        if let Some(p) = input.destination_port {
            dst.insert("destinationPort".into(), json!(p));
        }
        if let Some(proto) = &input.destination_protocol {
            dst.insert("destinationProtocol".into(), json!(proto));
        }
        obj.insert("destinationTrafficFilters".into(), Value::Object(dst));
    }
    Value::Object(obj)
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

/// Whether `s` is a canonical UUID string (8-4-4-4-12 hex with hyphens), any
/// version. Mirrors `sponsored_data::vwip::is_uuid_any`.
fn is_uuid_any(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(i, &b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// Mint a fresh, opaque, UUID-v4-shaped `trafficInfluenceID`.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set (mirrors `sponsored_data::vwip::mint_session_id`; no `uuid`/`rand` dep).
fn mint_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(now.to_be_bytes());
    let d = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "traffic.local:8080";

    const CONSUMER: &str = "consumer-42";
    // A version-1 UUID (any-version accepted); trailing digits "002" → active.
    const APP_ACTIVE: &str = "123e4567-e89b-12d3-a456-426614174002";
    // Trailing digits "000" → ordered; "001" → created.
    const APP_ORDERED: &str = "123e4567-e89b-12d3-a456-426614174000";
    const APP_CREATED: &str = "123e4567-e89b-12d3-a456-426614174001";
    const ZONE: &str = "550e8400-e29b-41d4-a716-446655440000";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_validation() {
        assert!(is_uuid_any(APP_ACTIVE));
        assert!(is_uuid_any(ZONE));
        assert!(!is_uuid_any("not-a-uuid"));
        assert!(!is_uuid_any("123e4567e89b12d3a456426614174000")); // no hyphens
        assert!(!is_uuid_any("123e4567-e89b-12d3-a456-42661417400")); // 35 chars
        assert!(!is_uuid_any("123e4567-e89b-12d3-a456-42661417400g")); // non-hex
    }

    #[test]
    fn state_is_selected_from_the_app_id_tail() {
        assert_eq!(derive_state(APP_ORDERED), "ordered"); // 000 → 0 % 3
        assert_eq!(derive_state(APP_CREATED), "created"); // 001 → 1 % 3
        assert_eq!(derive_state(APP_ACTIVE), "active"); // 002 → 2 % 3
        // A UUID with fewer than three decimal digits → the ordered default.
        assert_eq!(derive_state("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"), "ordered"); // only two digits
        assert_eq!(derive_state("ffffffff-ffff-ffff-ffff-ffffffffffff"), "ordered"); // no digits
    }

    #[test]
    fn build_response_emits_required_fields_and_only_supplied_optionals() {
        let minimal = ValidInput {
            api_consumer_id: CONSUMER.to_string(),
            app_id: APP_ACTIVE.to_string(),
            app_instance_id: None,
            edge_cloud_region: None,
            edge_cloud_zone_id: None,
            source_port: None,
            destination_port: None,
            destination_protocol: None,
        };
        let body = build_response("ti-1", "active", &minimal);
        assert_eq!(body["trafficInfluenceID"], "ti-1");
        assert_eq!(body["apiConsumerId"], CONSUMER);
        assert_eq!(body["appId"], APP_ACTIVE);
        assert_eq!(body["state"], "active");
        // Absent optionals stay absent (response schema forbids unknown keys).
        for absent in [
            "appInstanceId",
            "edgeCloudRegion",
            "edgeCloudZoneId",
            "sourceTrafficFilters",
            "destinationTrafficFilters",
        ] {
            assert!(body.get(absent).is_none(), "{absent} should be absent");
        }

        let full = ValidInput {
            api_consumer_id: CONSUMER.to_string(),
            app_id: APP_ACTIVE.to_string(),
            app_instance_id: Some(ZONE.to_string()),
            edge_cloud_region: Some("eu-west-1".to_string()),
            edge_cloud_zone_id: Some(ZONE.to_string()),
            source_port: Some(8080),
            destination_port: Some(443),
            destination_protocol: Some("TCP".to_string()),
        };
        let body = build_response("ti-2", "ordered", &full);
        assert_eq!(body["appInstanceId"], ZONE);
        assert_eq!(body["edgeCloudRegion"], "eu-west-1");
        assert_eq!(body["edgeCloudZoneId"], ZONE);
        assert_eq!(body["sourceTrafficFilters"]["sourcePort"], 8080);
        assert_eq!(body["destinationTrafficFilters"]["destinationPort"], 443);
        assert_eq!(body["destinationTrafficFilters"]["destinationProtocol"], "TCP");
    }

    #[test]
    fn ids_are_unique_and_uuid_shaped() {
        let a = mint_id();
        let b = mint_id();
        assert_ne!(a, b);
        assert!(is_uuid_any(&a), "{a} should be UUID-shaped");
        assert!(is_uuid_any(&b));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the route's audience. Scope granted verbatim.
    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=ti-client&scope={scope}");
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

    async fn token() -> String {
        mint_token(WRITE_SCOPE).await
    }

    fn create_body(app_id: &str) -> Value {
        json!({ "apiConsumerId": CONSUMER, "appId": app_id })
    }

    async fn post(
        token: Option<&str>,
        correlator: Option<&str>,
        body: Value,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(COLLECTION)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, value)
    }

    #[tokio::test]
    async fn happy_path_returns_201_with_id_state_and_location() {
        let (status, headers, body) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["appId"], APP_ACTIVE);
        assert_eq!(body["state"], "active");
        let id = body["trafficInfluenceID"].as_str().expect("id minted");
        assert!(is_uuid_any(id));
        let location = headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .expect("Location header");
        assert_eq!(location, format!("{COLLECTION}/{id}"));
        // The created resource is persisted for a later read-back.
        assert!(super::super::store::get(id).is_some());
    }

    #[tokio::test]
    async fn state_reflects_the_app_id_tail() {
        let t = token().await;
        for (app, want) in [(APP_ORDERED, "ordered"), (APP_CREATED, "created"), (APP_ACTIVE, "active")] {
            let (status, _, body) = post(Some(&t), None, create_body(app)).await;
            assert_eq!(status, StatusCode::CREATED, "app {app}");
            assert_eq!(body["state"], want, "app {app}");
        }
    }

    #[tokio::test]
    async fn optional_placement_and_filters_are_echoed() {
        let body = json!({
            "apiConsumerId": CONSUMER,
            "appId": APP_ACTIVE,
            "appInstanceId": ZONE,
            "edgeCloudRegion": "eu-west-1",
            "edgeCloudZoneId": ZONE,
            "sourceTrafficFilters": { "sourcePort": 8080 },
            "destinationTrafficFilters": { "destinationPort": 443, "destinationProtocol": "TCP" },
        });
        let (status, _, got) = post(Some(&token().await), None, body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(got["edgeCloudZoneId"], ZONE);
        assert_eq!(got["sourceTrafficFilters"]["sourcePort"], 8080);
        assert_eq!(got["destinationTrafficFilters"]["destinationProtocol"], "TCP");
    }

    #[tokio::test]
    async fn reserved_app_id_suffix_selects_a_canonical_camara_error() {
        let t = token().await;
        for (suffix, want) in [
            ("404", StatusCode::NOT_FOUND),
            ("409", StatusCode::CONFLICT),
            ("422", StatusCode::UNPROCESSABLE_ENTITY),
            ("429", StatusCode::TOO_MANY_REQUESTS),
        ] {
            let app = format!("123e4567-e89b-12d3-a456-426614174{suffix}");
            let (status, _, _) = post(Some(&t), None, create_body(&app)).await;
            assert_eq!(status, want, "suffix {suffix}");
        }
    }

    #[tokio::test]
    async fn missing_or_malformed_required_fields_are_invalid_argument() {
        let t = token().await;
        let cases = [
            json!({ "appId": APP_ACTIVE }),                         // no apiConsumerId
            json!({ "apiConsumerId": CONSUMER }),                   // no appId
            json!({ "apiConsumerId": "", "appId": APP_ACTIVE }),    // empty consumer
            json!({ "apiConsumerId": CONSUMER, "appId": "nope" }),  // bad appId
        ];
        for body in cases {
            let (status, _, err) = post(Some(&t), None, body.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(err["code"], "INVALID_ARGUMENT", "{body}");
        }
    }

    #[tokio::test]
    async fn malformed_optional_ids_are_invalid_argument() {
        let t = token().await;
        for field in ["appInstanceId", "edgeCloudZoneId"] {
            let mut body = create_body(APP_ACTIVE);
            body[field] = json!("not-a-uuid");
            let (status, _, err) = post(Some(&t), None, body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{field}");
            assert_eq!(err["code"], "INVALID_ARGUMENT", "{field}");
        }
    }

    #[tokio::test]
    async fn out_of_range_ports_are_rejected() {
        let t = token().await;
        let src = json!({
            "apiConsumerId": CONSUMER, "appId": APP_ACTIVE,
            "sourceTrafficFilters": { "sourcePort": 70000 },
        });
        let (status, _, err) = post(Some(&t), None, src).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");

        let dst = json!({
            "apiConsumerId": CONSUMER, "appId": APP_ACTIVE,
            "destinationTrafficFilters": { "destinationPort": -1 },
        });
        let (status, _, err) = post(Some(&t), None, dst).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn bad_json_body_is_invalid_argument() {
        let request = Request::builder()
            .method("POST")
            .uri(COLLECTION)
            .header("host", HOST)
            .header("authorization", format!("Bearer {}", token().await))
            .header("content-type", "application/json")
            .body(Body::from("{not json"))
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, _) = post(None, None, create_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wrong_scope_is_permission_denied() {
        let wrong = mint_token("some:other-scope").await;
        let (status, _, _) = post(Some(&wrong), None, create_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let t = token().await;
        let (status, headers, _) = post(Some(&t), Some("corr-ti-1"), create_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(headers.get("x-correlator").and_then(|v| v.to_str().ok()), Some("corr-ti-1"));

        let app404 = "123e4567-e89b-12d3-a456-426614174404";
        let (status, headers, _) = post(Some(&t), Some("corr-ti-2"), create_body(app404)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(headers.get("x-correlator").and_then(|v| v.to_str().ok()), Some("corr-ti-2"));
    }

    // --- getTrafficInfluence (read-back) -----------------------------------

    async fn get_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{COLLECTION}/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, value)
    }

    #[tokio::test]
    async fn get_reads_back_a_created_resource_verbatim() {
        // Create with the full optional set so read-back proves the whole shape.
        let create = json!({
            "apiConsumerId": CONSUMER,
            "appId": APP_ACTIVE,
            "edgeCloudRegion": "eu-west-1",
            "edgeCloudZoneId": ZONE,
            "sourceTrafficFilters": { "sourcePort": 8080 },
            "destinationTrafficFilters": { "destinationPort": 443, "destinationProtocol": "TCP" },
        });
        let (status, _, created) = post(Some(&token().await), None, create).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["trafficInfluenceID"].as_str().expect("id minted");

        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_req(Some(&read), id, None).await;
        assert_eq!(status, StatusCode::OK);
        // The stored representation is returned verbatim.
        assert_eq!(fetched, created);
        assert_eq!(fetched["state"], "active");
        assert_eq!(fetched["edgeCloudZoneId"], ZONE);
        assert_eq!(fetched["destinationTrafficFilters"]["destinationProtocol"], "TCP");
    }

    #[tokio::test]
    async fn get_unknown_id_is_not_found() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            get_req(Some(&read), "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_missing_token_is_unauthenticated() {
        let (status, _, _) = get_req(None, "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_wrong_scope_is_permission_denied() {
        // The write scope does not grant read.
        let (status, _, created) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["trafficInfluenceID"].as_str().unwrap().to_string();

        let wrong = mint_token(WRITE_SCOPE).await;
        let (status, _, _) = get_req(Some(&wrong), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn get_echoes_x_correlator_on_success_and_error() {
        let (_, _, created) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        let id = created["trafficInfluenceID"].as_str().unwrap().to_string();
        let read = mint_token(READ_SCOPE).await;

        let (status, headers, _) = get_req(Some(&read), &id, Some("corr-get-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-1")
        );

        let (status, headers, _) =
            get_req(Some(&read), "22222222-2222-4222-8222-222222222222", Some("corr-get-2")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-2")
        );
    }

    // --- deleteTrafficInfluence --------------------------------------------

    async fn delete_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("{COLLECTION}/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, value)
    }

    #[tokio::test]
    async fn delete_evicts_a_created_resource_and_read_back_is_404() {
        // Create, then delete → 202, then a read-back is 404 (single-use eviction).
        let (status, _, created) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["trafficInfluenceID"].as_str().expect("id minted").to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) = delete_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        // 202 carries no body.
        assert_eq!(body, Value::Null);
        // Evicted from the store.
        assert!(super::super::store::get(&id).is_none());

        // A subsequent read is 404.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, err) = get_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_404() {
        let (_, _, created) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        let id = created["trafficInfluenceID"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let (status, _, err) = delete_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_id_is_not_found() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) =
            delete_req(Some(&del), "33333333-3333-4333-8333-333333333333", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_missing_token_is_unauthenticated() {
        let (status, _, _) =
            delete_req(None, "33333333-3333-4333-8333-333333333333", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn delete_wrong_scope_is_permission_denied_and_resource_survives() {
        // The write scope does not grant delete; the resource must survive a 403.
        let (_, _, created) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        let id = created["trafficInfluenceID"].as_str().unwrap().to_string();

        let wrong = mint_token(WRITE_SCOPE).await;
        let (status, _, _) = delete_req(Some(&wrong), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // Still readable — the 403 did not evict it.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_echoes_x_correlator_on_success_and_error() {
        let (_, _, created) = post(Some(&token().await), None, create_body(APP_ACTIVE)).await;
        let id = created["trafficInfluenceID"].as_str().unwrap().to_string();
        let del = mint_token(DELETE_SCOPE).await;

        let (status, headers, _) = delete_req(Some(&del), &id, Some("corr-del-1")).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-1")
        );

        let (status, headers, _) =
            delete_req(Some(&del), "44444444-4444-4444-8444-444444444444", Some("corr-del-2")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-2")
        );
    }
}
