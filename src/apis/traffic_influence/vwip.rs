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
//! - `PATCH /traffic-influence/vwip/traffic-influences/{trafficInfluenceID}` —
//!   update a created resource's mutable placement/filter fields in place
//!   (operationId `patchTrafficInfluence`, scope
//!   `traffic-influence:traffic-influences:write`). The body is a
//!   `merge-patch+json` document over the mutable fields: a supplied field is
//!   replaced, an explicit `null` clears an optional field, and identity /
//!   read-only fields (`trafficInfluenceID`, `appId`, `state`) are left untouched.
//!   Two control planes (docs/DESIGN.md §7): the opaque, operator-minted id →
//!   store state (`200` updated / `404 NOT_FOUND`), and the request body (a
//!   malformed field → `400 INVALID_ARGUMENT`, a port out of range → `400
//!   OUT_OF_RANGE`), validated before the store so a body `400` wins over `404`.
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

use super::notifications;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create a Traffic Influence resource. Declared by the
/// upstream `wip` contract (`traffic-influence:traffic-influences:write`).
const WRITE_SCOPE: &str = "traffic-influence:traffic-influences:write";

/// Scope required to create a **per-device** Traffic Influence resource. The
/// upstream `wip` contract gives the device create its own resource scope
/// (`traffic-influence:traffic-influence-devices:write`), distinct from the
/// collection write scope above.
const DEVICE_WRITE_SCOPE: &str = "traffic-influence:traffic-influence-devices:write";

/// Scope required to read a Traffic Influence resource back. Declared by the
/// upstream `wip` contract (`traffic-influence:traffic-influences:read`).
const READ_SCOPE: &str = "traffic-influence:traffic-influences:read";

/// Scope required to delete a Traffic Influence resource. Declared by the
/// upstream `wip` contract (`traffic-influence:traffic-influences:delete`).
const DELETE_SCOPE: &str = "traffic-influence:traffic-influences:delete";

/// Base path of the resource collection, used both to mount the route and to
/// build the `201` `Location` header.
const COLLECTION: &str = "/traffic-influence/vwip/traffic-influences";

/// Path of the per-device create collection (`postTrafficInfluenceDevice`). It
/// creates the **same** `TrafficInfluence` resource as [`COLLECTION`] (read back
/// via [`ITEM`]), so the `201` `Location` still points under [`COLLECTION`].
const DEVICE_COLLECTION: &str = "/traffic-influence/vwip/traffic-influence-devices";

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
        .route(DEVICE_COLLECTION, post(post_traffic_influence_device))
        .route(
            ITEM,
            get(get_traffic_influence)
                .patch(patch_traffic_influence)
                .delete(delete_traffic_influence),
        )
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
    /// Optional CAMARA event subscription. When present and valid, the operator
    /// delivers `traffic-influence-change` CloudEvents to its `sink`. CamaraSim
    /// models the **initial event** (`config.initialEvent: true`) fired on create.
    #[serde(rename = "subscriptionRequest")]
    subscription_request: Option<SubscriptionRequest>,
}

/// The upstream `SubscriptionRequest` (a CAMARA event subscription embedded in the
/// create body). CamaraSim requires `sink` + `protocol` + `types` + `config`
/// (`config.subscriptionDetail`), validates them, and — for `config.initialEvent:
/// true` — fires a single `traffic-influence-change` CloudEvent on create. The
/// ongoing state-change stream, `subscriptionExpireTime` / `subscriptionMaxEvents`
/// lifecycle, and non-`HTTP` protocols are documented cuts. No `deny_unknown_fields`
/// so accepted-not-applied config keys (e.g. `subscriptionExpireTime`) are ignored.
#[derive(Deserialize)]
struct SubscriptionRequest {
    sink: Option<String>,
    protocol: Option<String>,
    types: Option<Vec<String>>,
    #[serde(rename = "sinkCredential")]
    sink_credential: Option<Value>,
    config: Option<SubscriptionConfig>,
}

/// The `subscriptionRequest.config` object. `subscriptionDetail` is required
/// upstream (accepted for shape only — CamaraSim reads no event-type-specific
/// detail); `initialEvent` gates the create-time notification. Other predefined
/// config keys (`subscriptionExpireTime`, `subscriptionMaxEvents`) are
/// accepted-not-applied (deferred lifecycle, a documented cut).
#[derive(Deserialize)]
struct SubscriptionConfig {
    #[serde(rename = "subscriptionDetail")]
    subscription_detail: Option<Value>,
    #[serde(rename = "initialEvent")]
    initial_event: Option<bool>,
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

/// The `postTrafficInfluenceDevice` request body (`PostTrafficInfluenceDevice`).
/// Upstream this schema **extends** `PostTrafficInfluence` with a required
/// `device`, so the shared base fields are flattened in and reuse the collection
/// create's validation ([`validate_base`]). The `device` names the individual
/// end-user equipment the rule applies to; the collection variant instead applies
/// to every user in the named region/zone.
#[derive(Deserialize)]
struct PostTrafficInfluenceDevice {
    #[serde(flatten)]
    base: PostTrafficInfluence,
    device: Option<Device>,
}

/// The CAMARA `Device` object (`minProperties: 1`). CamaraSim validates each
/// supplied identifier's shape, but — unlike the device-query APIs — the device
/// is **never echoed** (upstream: "for privacy reasons, if a resource is related
/// to a user, the parameter Device is not exchanged") and is **not** a control
/// plane: `appId` stays the sole control plane, exactly as on the collection
/// create. `deny_unknown_fields` so an unrecognised identifier key is rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Device {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "networkAccessIdentifier")]
    network_access_identifier: Option<String>,
    #[serde(rename = "ipv4Address")]
    ipv4_address: Option<DeviceIpv4Addr>,
    #[serde(rename = "ipv6Address")]
    ipv6_address: Option<String>,
}

/// The CAMARA `DeviceIpv4Addr` object. CamaraSim treats `publicAddress` as the
/// device's public IPv4 identifier; the other fields are accepted for schema
/// fidelity (`publicPort` is range-checked when present).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceIpv4Addr {
    #[serde(rename = "publicAddress")]
    public_address: Option<String>,
    #[serde(rename = "privateAddress")]
    #[allow(dead_code)]
    private_address: Option<String>,
    #[serde(rename = "publicPort")]
    public_port: Option<i64>,
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
    /// The validated event subscription, if the body carried one. [`finalize`]
    /// fires the initial `traffic-influence-change` CloudEvent from it when
    /// `initial_event` is set.
    subscription: Option<ValidSubscription>,
}

/// A validated `subscriptionRequest`: the callback `sink`, the derived
/// `Authorization` header (from an ACCESSTOKEN or PLAIN `sinkCredential`, else
/// `None`; see [`notifications::sink_authorization`]), and whether the consumer
/// asked for the create-time initial event.
struct ValidSubscription {
    sink: String,
    auth: Option<String>,
    initial_event: bool,
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

    // Validate the base fields into a ready-to-render `ValidInput`, then apply the
    // `appId` control planes and emit the `201`.
    match validate_base(req, &correlator) {
        Ok(input) => finalize(input, &correlator),
        Err(e) => e,
    }
}

/// `POST /traffic-influence/vwip/traffic-influence-devices` (`postTrafficInfluenceDevice`).
///
/// The **per-device** create. It creates the same `TrafficInfluence` resource as
/// [`post_traffic_influence`] — read back via [`get_traffic_influence`], and the
/// `201` `Location` points under [`COLLECTION`] — but scopes the rule to a single
/// end-user device named by a required `device` object, and carries its own
/// resource scope (`traffic-influence:traffic-influence-devices:write`).
///
/// The `device` is validated for shape (`minProperties: 1`, each supplied
/// identifier well-formed → otherwise `400 INVALID_ARGUMENT`, a `publicPort` out
/// of range → `400 OUT_OF_RANGE`) but is **never echoed** in the response
/// (upstream privacy rule) and is **not** a control plane: `appId` stays the sole
/// control plane, exactly as on the collection create (reserved error suffix →
/// canonical CAMARA error; trailing three digits → resource `state`). The base
/// fields are validated first (same order and errors as the collection create),
/// then the `device`, so a malformed base field's `400` wins over a malformed
/// `device`, and both `400`s win over an `appId` reserved-suffix scenario.
/// `x-correlator` is echoed on every response.
async fn post_traffic_influence_device(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the per-device write scope.
    if let Err(e) = claims.require_scope(DEVICE_WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory; parse strictly.
    let req: PostTrafficInfluenceDevice = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid PostTrafficInfluenceDevice.",
                &correlator,
            )
        }
    };

    // Validate the shared base fields first (same order/errors as the collection
    // create), then the required per-device `device` object.
    let input = match validate_base(req.base, &correlator) {
        Ok(input) => input,
        Err(e) => return e,
    };
    if let Err(e) = validate_device(req.device, &correlator) {
        return e;
    }

    finalize(input, &correlator)
}

/// Validate a `PostTrafficInfluence` body into the owned [`ValidInput`] a happy
/// path renders. Shared by both the collection create ([`post_traffic_influence`])
/// and the per-device create ([`post_traffic_influence_device`], which layers its
/// own `device` validation on top). Returns the ready `400` response on the first
/// malformed field.
fn validate_base(
    req: PostTrafficInfluence,
    correlator: &Option<HeaderValue>,
) -> Result<ValidInput, Response> {
    // Required fields.
    let api_consumer_id = match req.api_consumer_id.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        Some(_) => return Err(invalid_argument("`apiConsumerId` must not be empty.", correlator)),
        None => return Err(invalid_argument("`apiConsumerId` is required.", correlator)),
    };
    let app_id = match req.app_id.as_deref() {
        Some(s) if is_uuid_any(s) => s.to_string(),
        Some(_) => return Err(invalid_argument("`appId` must be a UUID.", correlator)),
        None => return Err(invalid_argument("`appId` is required.", correlator)),
    };

    // Optional placement fields — validated only when present.
    let app_instance_id = match req.app_instance_id.as_deref() {
        None => None,
        Some(s) if is_uuid_any(s) => Some(s.to_string()),
        Some(_) => return Err(invalid_argument("`appInstanceId` must be a UUID.", correlator)),
    };
    let edge_cloud_zone_id = match req.edge_cloud_zone_id.as_deref() {
        None => None,
        Some(s) if is_uuid_any(s) => Some(s.to_string()),
        Some(_) => return Err(invalid_argument("`edgeCloudZoneId` must be a UUID.", correlator)),
    };
    let edge_cloud_region = match req.edge_cloud_region.as_deref() {
        None => None,
        Some(s) if !s.is_empty() => Some(s.to_string()),
        Some(_) => return Err(invalid_argument("`edgeCloudRegion` must not be empty.", correlator)),
    };

    // Optional traffic filters — ports are range-checked.
    let source_port = match req.source_traffic_filters.and_then(|f| f.source_port) {
        None => None,
        Some(p) if (MIN_PORT..=MAX_PORT).contains(&p) => Some(p),
        Some(_) => {
            return Err(out_of_range(
                "`sourcePort` must be between 0 and 65535.",
                correlator,
            ))
        }
    };
    let (destination_port, destination_protocol) = match req.destination_traffic_filters {
        None => (None, None),
        Some(f) => {
            let port = match f.destination_port {
                None => None,
                Some(p) if (MIN_PORT..=MAX_PORT).contains(&p) => Some(p),
                Some(_) => {
                    return Err(out_of_range(
                        "`destinationPort` must be between 0 and 65535.",
                        correlator,
                    ))
                }
            };
            (port, f.destination_protocol)
        }
    };

    // Optional event subscription — validated last, so a malformed resource field
    // wins over a malformed subscription (and both `400`s win over the `appId`
    // reserved-error scenario applied later in `finalize`).
    let subscription = match req.subscription_request {
        None => None,
        Some(sr) => Some(validate_subscription(sr, correlator)?),
    };

    Ok(ValidInput {
        api_consumer_id,
        app_id,
        app_instance_id,
        edge_cloud_region,
        edge_cloud_zone_id,
        source_port,
        destination_port,
        destination_protocol,
        subscription,
    })
}

/// Validate an optional `subscriptionRequest` into a [`ValidSubscription`].
///
/// The upstream `SubscriptionRequest` requires `sink`, `protocol`, `types` and
/// `config` (with a required `config.subscriptionDetail`). CamaraSim additionally
/// requires `protocol: HTTP` (the only delivery protocol it implements — raw-TCP,
/// no message brokers) and the single `types` value
/// (`org.camaraproject.traffic-influence.v1.traffic-influence-change`). The `sink`
/// is accepted as `http://` (loopback receivers, raw TCP) or `https://` (upstream
/// mandates `https://`, delivered over verified `rustls` TLS). Any problem →
/// `400 INVALID_ARGUMENT`.
fn validate_subscription(
    sr: SubscriptionRequest,
    correlator: &Option<HeaderValue>,
) -> Result<ValidSubscription, Response> {
    let sink = match sr.sink.as_deref() {
        Some(s) if is_valid_sink(s) => s.to_string(),
        Some(_) => {
            return Err(invalid_argument(
                "`subscriptionRequest.sink` must be a valid `http://` or `https://` callback URL.",
                correlator,
            ))
        }
        None => return Err(invalid_argument("`subscriptionRequest.sink` is required.", correlator)),
    };
    match sr.protocol.as_deref() {
        Some("HTTP") => {}
        Some(_) => {
            return Err(invalid_argument(
                "`subscriptionRequest.protocol` must be `HTTP` (the only delivery protocol CamaraSim supports).",
                correlator,
            ))
        }
        None => {
            return Err(invalid_argument(
                "`subscriptionRequest.protocol` is required.",
                correlator,
            ))
        }
    }
    match sr.types {
        Some(ref v) if v.len() == 1 && v[0] == notifications::EVENT_TYPE => {}
        Some(_) => {
            return Err(invalid_argument(
                "`subscriptionRequest.types` must be exactly [\"org.camaraproject.traffic-influence.v1.traffic-influence-change\"].",
                correlator,
            ))
        }
        None => return Err(invalid_argument("`subscriptionRequest.types` is required.", correlator)),
    }
    let config = match sr.config {
        Some(c) => c,
        None => return Err(invalid_argument("`subscriptionRequest.config` is required.", correlator)),
    };
    if config.subscription_detail.is_none() {
        return Err(invalid_argument(
            "`subscriptionRequest.config.subscriptionDetail` is required.",
            correlator,
        ));
    }
    let auth = sr
        .sink_credential
        .as_ref()
        .and_then(notifications::sink_authorization);
    Ok(ValidSubscription {
        sink,
        auth,
        initial_event: config.initial_event.unwrap_or(false),
    })
}

/// Validate the per-device create's required `device` object. It must be present
/// and carry at least one identifier (`minProperties: 1`), each supplied
/// identifier well-formed. The device is not stored or echoed (privacy), so this
/// only gates the request. Returns the ready `400` response on any problem.
fn validate_device(device: Option<Device>, correlator: &Option<HeaderValue>) -> Result<(), Response> {
    let device = match device {
        Some(d) => d,
        None => return Err(invalid_argument("`device` is required.", correlator)),
    };

    let mut any = false;
    if let Some(phone) = device.phone_number.as_deref() {
        if !is_valid_e164(phone) {
            return Err(invalid_argument(
                "`device.phoneNumber` must be in E.164 format (e.g. +123456789).",
                correlator,
            ));
        }
        any = true;
    }
    if let Some(nai) = device.network_access_identifier.as_deref() {
        if nai.is_empty() {
            return Err(invalid_argument(
                "`device.networkAccessIdentifier` must not be empty.",
                correlator,
            ));
        }
        any = true;
    }
    if let Some(v4) = &device.ipv4_address {
        match v4.public_address.as_deref() {
            Some(a) if !a.is_empty() => {}
            _ => {
                return Err(invalid_argument(
                    "`device.ipv4Address.publicAddress` is required.",
                    correlator,
                ))
            }
        }
        if let Some(port) = v4.public_port {
            if !(MIN_PORT..=MAX_PORT).contains(&port) {
                return Err(out_of_range(
                    "`device.ipv4Address.publicPort` must be between 0 and 65535.",
                    correlator,
                ));
            }
        }
        any = true;
    }
    if let Some(v6) = device.ipv6_address.as_deref() {
        if v6.is_empty() {
            return Err(invalid_argument(
                "`device.ipv6Address` must not be empty.",
                correlator,
            ));
        }
        any = true;
    }
    if !any {
        return Err(invalid_argument(
            "`device` must contain at least one identifier.",
            correlator,
        ));
    }
    Ok(())
}

/// Apply the `appId` control planes and emit the `201`. Shared by the collection
/// and per-device creates: both mint an opaque `trafficInfluenceID`, persist the
/// rendered `TrafficInfluence` in the shared store (so it reads back via
/// [`get_traffic_influence`]), and answer `201` with a `Location` under
/// [`COLLECTION`].
fn finalize(input: ValidInput, correlator: &Option<HeaderValue>) -> Response {
    // The `appId` is the identifier and a control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&input.app_id) {
        return with_correlator(err.into_response(), correlator);
    }

    // Second control plane: the `appId` tail selects the created resource state.
    let state = derive_state(&input.app_id);

    // Mint the resource, render it, persist it, and return `201` with `Location`.
    let id = mint_id();
    let resource = build_response(&id, state, &input);
    super::store::insert(id.clone(), resource.clone());

    // If the create carried a valid `subscriptionRequest` asking for the initial
    // event, deliver a single `traffic-influence-change` CloudEvent reflecting the
    // created resource's current state to the sink (fire-and-forget, off the
    // request path; ACCESSTOKEN Bearer / PLAIN Basic `sinkCredential` applied;
    // `http://` over raw TCP, `https://` over verified rustls TLS).
    if let Some(sub) = &input.subscription {
        if sub.initial_event {
            let event = notifications::traffic_influence_change_event(
                mint_id(),
                rfc3339_utc(now_unix_secs()),
                &resource,
            );
            notifications::spawn_delivery(sub.sink.clone(), event, sub.auth.clone());
        }
    }

    let location = format!("{COLLECTION}/{id}");
    let mut response = (StatusCode::CREATED, Json(resource)).into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("location"), value);
    }
    with_correlator(response, correlator)
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

/// `PATCH /traffic-influence/vwip/traffic-influences/{trafficInfluenceID}`.
///
/// Updates a Traffic Influence resource's **mutable** fields in place, addressed
/// by its opaque, operator-minted `trafficInfluenceID`. The body is a
/// `merge-patch+json` document (CAMARA convention): a supplied mutable field is
/// replaced, an explicit `null` clears an optional field, and the identity /
/// read-only fields (`trafficInfluenceID`, `appId`, `state`, plus any unknown
/// key) are ignored — the resource keeps its create-time `appId` and lifecycle
/// `state` (CamaraSim runs no provisioning worker, so a PATCH does not re-derive
/// `state`; a documented cut).
///
/// Two control planes (docs/DESIGN.md §7). The request **body** is validated
/// first: a malformed `apiConsumerId` / `appInstanceId` / `edgeCloudRegion` /
/// `edgeCloudZoneId` / traffic filter → `400 INVALID_ARGUMENT`, a `sourcePort` /
/// `destinationPort` outside `0..=65535` → `400 OUT_OF_RANGE`. Then the opaque id
/// selects the **store state**: a resource still held → `200` with the updated
/// resource; an unknown/already-deleted id → `404 NOT_FOUND`. Validating the body
/// before the store means a body `400` wins over the unknown-id `404` (mirrors
/// Application Profiles' `updateApplicationProfile`). `x-correlator` is echoed on
/// every response.
async fn patch_traffic_influence(
    claims: Claims,
    headers: HeaderMap,
    Path(traffic_influence_id): Path<String>,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the write scope (update is a write).
    if let Err(e) = claims.require_scope(WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is a JSON merge-patch document; parse then validate (400 before store).
    let value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return invalid_argument("Request body is not valid JSON.", &correlator),
    };
    let merge = match validate_patch(&value) {
        Ok(m) => m,
        Err((code, message)) => {
            return with_correlator(
                CamaraError::new(StatusCode::BAD_REQUEST, code, &message).into_response(),
                &correlator,
            )
        }
    };

    // Apply the merge to the stored resource atomically (get-modify-write under
    // one lock hold): the opaque id is the store-state control plane.
    match super::store::update_with(&traffic_influence_id, |resource| {
        apply_merge(resource, &merge)
    }) {
        Some(updated) => {
            with_correlator((StatusCode::OK, Json(updated)).into_response(), &correlator)
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

/// An ordered list of merge operations to apply to the stored resource. `Some(v)`
/// sets a field, `None` clears it. Built by [`validate_patch`], applied by
/// [`apply_merge`].
type Merge = Vec<(&'static str, Option<Value>)>;

/// Validate a `merge-patch+json` body against the mutable Traffic Influence
/// fields, returning the merge operations to apply or `(code, message)` for a
/// `400`. Identity / read-only fields (`trafficInfluenceID`, `appId`, `state`)
/// and any unknown key are ignored (mirrors create ignoring read-only fields).
/// Pure over its input, so it is unit-testable exactly.
fn validate_patch(body: &Value) -> Result<Merge, (&'static str, String)> {
    let obj = body.as_object().ok_or((
        "INVALID_ARGUMENT",
        "Request body must be a JSON object.".to_string(),
    ))?;
    let mut merge: Merge = Vec::new();
    for (key, value) in obj {
        match key.as_str() {
            // `apiConsumerId` is required on the resource, so it may be replaced
            // (non-empty string) but not cleared.
            "apiConsumerId" => match value {
                Value::String(s) if !s.is_empty() => merge.push(("apiConsumerId", Some(json!(s)))),
                _ => {
                    return Err((
                        "INVALID_ARGUMENT",
                        "`apiConsumerId` must be a non-empty string.".into(),
                    ))
                }
            },
            "appInstanceId" => match value {
                Value::Null => merge.push(("appInstanceId", None)),
                Value::String(s) if is_uuid_any(s) => merge.push(("appInstanceId", Some(json!(s)))),
                _ => {
                    return Err((
                        "INVALID_ARGUMENT",
                        "`appInstanceId` must be a UUID or null.".into(),
                    ))
                }
            },
            "edgeCloudRegion" => match value {
                Value::Null => merge.push(("edgeCloudRegion", None)),
                Value::String(s) if !s.is_empty() => {
                    merge.push(("edgeCloudRegion", Some(json!(s))))
                }
                _ => {
                    return Err((
                        "INVALID_ARGUMENT",
                        "`edgeCloudRegion` must be a non-empty string or null.".into(),
                    ))
                }
            },
            "edgeCloudZoneId" => match value {
                Value::Null => merge.push(("edgeCloudZoneId", None)),
                Value::String(s) if is_uuid_any(s) => merge.push(("edgeCloudZoneId", Some(json!(s)))),
                _ => {
                    return Err((
                        "INVALID_ARGUMENT",
                        "`edgeCloudZoneId` must be a UUID or null.".into(),
                    ))
                }
            },
            "sourceTrafficFilters" => match value {
                Value::Null => merge.push(("sourceTrafficFilters", None)),
                Value::Object(_) => {
                    merge.push(("sourceTrafficFilters", Some(validate_source_filters(value)?)))
                }
                _ => {
                    return Err((
                        "INVALID_ARGUMENT",
                        "`sourceTrafficFilters` must be an object or null.".into(),
                    ))
                }
            },
            "destinationTrafficFilters" => match value {
                Value::Null => merge.push(("destinationTrafficFilters", None)),
                Value::Object(_) => merge.push((
                    "destinationTrafficFilters",
                    Some(validate_destination_filters(value)?),
                )),
                _ => {
                    return Err((
                        "INVALID_ARGUMENT",
                        "`destinationTrafficFilters` must be an object or null.".into(),
                    ))
                }
            },
            // Identity / read-only / unknown keys are ignored (see the doc comment).
            _ => {}
        }
    }
    Ok(merge)
}

/// Validate a single port value (`sourcePort` / `destinationPort`): an integer in
/// `0..=65535`, else `OUT_OF_RANGE` (out of range) or `INVALID_ARGUMENT` (not an
/// integer).
fn validate_port(value: &Value, field: &str) -> Result<i64, (&'static str, String)> {
    match value.as_i64() {
        Some(p) if (MIN_PORT..=MAX_PORT).contains(&p) => Ok(p),
        Some(_) => Err((
            "OUT_OF_RANGE",
            format!("`{field}` must be between 0 and 65535."),
        )),
        None => Err(("INVALID_ARGUMENT", format!("`{field}` must be an integer."))),
    }
}

/// Validate + rebuild a `sourceTrafficFilters` object into a clean representation
/// (the response schema forbids unknown keys, so only recognised fields survive).
fn validate_source_filters(value: &Value) -> Result<Value, (&'static str, String)> {
    let obj = value.as_object().expect("caller checked object");
    let mut out = serde_json::Map::new();
    if let Some(p) = obj.get("sourcePort") {
        out.insert("sourcePort".into(), json!(validate_port(p, "sourcePort")?));
    }
    Ok(Value::Object(out))
}

/// Validate + rebuild a `destinationTrafficFilters` object into a clean
/// representation (unknown keys dropped; port range-checked).
fn validate_destination_filters(value: &Value) -> Result<Value, (&'static str, String)> {
    let obj = value.as_object().expect("caller checked object");
    let mut out = serde_json::Map::new();
    if let Some(p) = obj.get("destinationPort") {
        out.insert(
            "destinationPort".into(),
            json!(validate_port(p, "destinationPort")?),
        );
    }
    if let Some(proto) = obj.get("destinationProtocol") {
        match proto {
            Value::String(s) => {
                out.insert("destinationProtocol".into(), json!(s));
            }
            _ => {
                return Err((
                    "INVALID_ARGUMENT",
                    "`destinationProtocol` must be a string.".into(),
                ))
            }
        }
    }
    Ok(Value::Object(out))
}

/// Apply the validated merge operations to the stored resource JSON in place:
/// `Some(v)` sets a field, `None` removes it. Pure over its inputs.
fn apply_merge(resource: &mut Value, merge: &Merge) {
    if let Some(obj) = resource.as_object_mut() {
        for (key, op) in merge {
            match op {
                Some(v) => {
                    obj.insert((*key).to_string(), v.clone());
                }
                None => {
                    obj.remove(*key);
                }
            }
        }
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

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`:
/// a leading `+`, then 5–15 digits, the first of which is non-zero. Mirrors
/// `connected_network_type::v0_2::is_valid_e164`.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
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

/// Whether `s` is a callback `sink` CamaraSim accepts: an `http://` (loopback
/// receivers, raw TCP) or `https://` (upstream mandates `https://`, delivered over
/// verified `rustls` TLS) URL with a non-empty authority. Mirrors
/// `qos_provisioning::v0_3::is_valid_sink`.
fn is_valid_sink(s: &str) -> bool {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty())
}

/// Seconds since the Unix epoch, non-blocking (`SystemTime::now`).
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format `unix_secs` as an RFC 3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`) for a
/// CloudEvent `time`. Self-contained (no `chrono`/`time` dep; Howard Hinnant's
/// civil-from-days), mirroring `qos_provisioning::v0_3`.
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since the Unix epoch to a `(year, month, day)` civil
/// date (Howard Hinnant's algorithm). Shared shape with `qos_provisioning`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
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
            subscription: None,
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
            subscription: None,
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

    // --- patchTrafficInfluence (update) ------------------------------------

    #[test]
    fn validate_patch_accepts_known_fields_and_ignores_read_only() {
        let body = json!({
            "apiConsumerId": "consumer-99",
            "edgeCloudRegion": "us-east-1",
            "appInstanceId": ZONE,
            "sourceTrafficFilters": { "sourcePort": 9090 },
            // Read-only / identity / unknown keys are silently ignored.
            "trafficInfluenceID": "spoofed",
            "appId": APP_ORDERED,
            "state": "error",
            "somethingElse": 1,
        });
        let merge = validate_patch(&body).expect("valid patch");
        // Only the four recognised mutable fields produced ops.
        assert_eq!(merge.len(), 4);
        assert!(merge.iter().all(|(k, _)| matches!(
            *k,
            "apiConsumerId" | "edgeCloudRegion" | "appInstanceId" | "sourceTrafficFilters"
        )));
    }

    #[test]
    fn validate_patch_null_clears_optional_fields() {
        let body = json!({ "edgeCloudRegion": null, "destinationTrafficFilters": null });
        let merge = validate_patch(&body).expect("valid patch");
        for (key, op) in &merge {
            assert!(op.is_none(), "{key} should be a clear op");
        }
    }

    #[test]
    fn validate_patch_rejects_malformed_fields() {
        // Non-empty required, bad UUIDs, out-of-range ports, wrong types.
        let cases: [(Value, &str); 6] = [
            (json!({ "apiConsumerId": "" }), "INVALID_ARGUMENT"),
            (json!({ "apiConsumerId": null }), "INVALID_ARGUMENT"),
            (json!({ "appInstanceId": "nope" }), "INVALID_ARGUMENT"),
            (json!({ "edgeCloudZoneId": 42 }), "INVALID_ARGUMENT"),
            (json!({ "sourceTrafficFilters": { "sourcePort": 70000 } }), "OUT_OF_RANGE"),
            (json!({ "destinationTrafficFilters": { "destinationPort": -1 } }), "OUT_OF_RANGE"),
        ];
        for (body, want) in cases {
            let err = validate_patch(&body).expect_err(&format!("{body} should fail"));
            assert_eq!(err.0, want, "{body}");
        }
        // A non-object body is rejected outright.
        assert_eq!(validate_patch(&json!([1, 2, 3])).unwrap_err().0, "INVALID_ARGUMENT");
    }

    #[test]
    fn apply_merge_sets_and_clears_fields() {
        let mut resource = json!({
            "trafficInfluenceID": "ti-x",
            "apiConsumerId": "consumer-1",
            "appId": APP_ACTIVE,
            "state": "active",
            "edgeCloudRegion": "eu-west-1",
        });
        let merge: Merge = vec![
            ("edgeCloudRegion", Some(json!("us-east-1"))),
            ("appInstanceId", Some(json!(ZONE))),
            ("edgeCloudRegion", Some(json!("ap-south-1"))), // last write wins
            ("apiConsumerId", Some(json!("consumer-2"))),
        ];
        apply_merge(&mut resource, &merge);
        assert_eq!(resource["edgeCloudRegion"], "ap-south-1");
        assert_eq!(resource["appInstanceId"], ZONE);
        assert_eq!(resource["apiConsumerId"], "consumer-2");
        // Identity / state untouched.
        assert_eq!(resource["appId"], APP_ACTIVE);
        assert_eq!(resource["state"], "active");

        // A clear op removes the key entirely.
        apply_merge(&mut resource, &vec![("appInstanceId", None)]);
        assert!(resource.get("appInstanceId").is_none());
    }

    async fn patch_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
        body: Value,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("PATCH")
            .uri(format!("{COLLECTION}/{id}"))
            .header("host", HOST)
            .header("content-type", "application/merge-patch+json");
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

    async fn create_active() -> String {
        let create = json!({
            "apiConsumerId": CONSUMER,
            "appId": APP_ACTIVE,
            "edgeCloudRegion": "eu-west-1",
            "sourceTrafficFilters": { "sourcePort": 8080 },
        });
        let (status, _, created) = post(Some(&token().await), None, create).await;
        assert_eq!(status, StatusCode::CREATED);
        created["trafficInfluenceID"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn patch_updates_mutable_fields_and_persists() {
        let id = create_active().await;
        let patch = json!({
            "edgeCloudRegion": "us-east-1",
            "edgeCloudZoneId": ZONE,
            "destinationTrafficFilters": { "destinationPort": 443, "destinationProtocol": "UDP" },
        });
        let (status, _, updated) = patch_req(Some(&token().await), &id, None, patch).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["edgeCloudRegion"], "us-east-1");
        assert_eq!(updated["edgeCloudZoneId"], ZONE);
        assert_eq!(updated["destinationTrafficFilters"]["destinationProtocol"], "UDP");
        // Identity / state / untouched create-time fields survive.
        assert_eq!(updated["trafficInfluenceID"], id);
        assert_eq!(updated["appId"], APP_ACTIVE);
        assert_eq!(updated["state"], "active");
        assert_eq!(updated["sourceTrafficFilters"]["sourcePort"], 8080);

        // The change persisted: a read-back sees it.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched, updated);
    }

    #[tokio::test]
    async fn patch_null_clears_an_optional_field() {
        let id = create_active().await;
        let (status, _, updated) =
            patch_req(Some(&token().await), &id, None, json!({ "edgeCloudRegion": null })).await;
        assert_eq!(status, StatusCode::OK);
        assert!(updated.get("edgeCloudRegion").is_none(), "cleared");
        // The create-time source filter is untouched (merge patch, not replace).
        assert_eq!(updated["sourceTrafficFilters"]["sourcePort"], 8080);
    }

    #[tokio::test]
    async fn patch_empty_body_is_a_noop_200() {
        let id = create_active().await;
        let read = mint_token(READ_SCOPE).await;
        let (_, _, before) = get_req(Some(&read), &id, None).await;
        let (status, _, after) = patch_req(Some(&token().await), &id, None, json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(after, before, "empty merge changes nothing");
    }

    #[tokio::test]
    async fn patch_ignores_read_only_and_identity_fields() {
        let id = create_active().await;
        let patch = json!({
            "trafficInfluenceID": "spoofed",
            "appId": APP_ORDERED,
            "state": "error",
            "edgeCloudRegion": "ap-south-1",
        });
        let (status, _, updated) = patch_req(Some(&token().await), &id, None, patch).await;
        assert_eq!(status, StatusCode::OK);
        // Only the mutable field changed; identity/state ignored.
        assert_eq!(updated["trafficInfluenceID"], id);
        assert_eq!(updated["appId"], APP_ACTIVE);
        assert_eq!(updated["state"], "active");
        assert_eq!(updated["edgeCloudRegion"], "ap-south-1");
    }

    #[tokio::test]
    async fn patch_unknown_id_is_not_found() {
        let (status, _, err) = patch_req(
            Some(&token().await),
            "55555555-5555-4555-8555-555555555555",
            None,
            json!({ "edgeCloudRegion": "us-east-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn patch_bad_body_400_wins_over_unknown_id_404() {
        // A malformed body is a 400 even when the id does not exist (body checked first).
        let (status, _, err) = patch_req(
            Some(&token().await),
            "66666666-6666-4666-8666-666666666666",
            None,
            json!({ "appInstanceId": "not-a-uuid" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn patch_out_of_range_port_is_rejected() {
        let id = create_active().await;
        let (status, _, err) = patch_req(
            Some(&token().await),
            &id,
            None,
            json!({ "sourceTrafficFilters": { "sourcePort": 99999 } }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn patch_non_json_body_is_invalid_argument() {
        let id = create_active().await;
        let request = Request::builder()
            .method("PATCH")
            .uri(format!("{COLLECTION}/{id}"))
            .header("host", HOST)
            .header("authorization", format!("Bearer {}", token().await))
            .header("content-type", "application/merge-patch+json")
            .body(Body::from("{not json"))
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn patch_missing_token_is_unauthenticated() {
        let (status, _, _) = patch_req(
            None,
            "77777777-7777-4777-8777-777777777777",
            None,
            json!({ "edgeCloudRegion": "us-east-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn patch_wrong_scope_is_permission_denied_and_resource_survives() {
        let id = create_active().await;
        // A read-only token must not be able to patch.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) =
            patch_req(Some(&read), &id, None, json!({ "edgeCloudRegion": "us-east-1" })).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // Unchanged — the 403 did not mutate it.
        let (status, _, fetched) = get_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["edgeCloudRegion"], "eu-west-1");
    }

    #[tokio::test]
    async fn patch_echoes_x_correlator_on_success_and_error() {
        let id = create_active().await;
        let (status, headers, _) = patch_req(
            Some(&token().await),
            &id,
            Some("corr-patch-1"),
            json!({ "edgeCloudRegion": "us-east-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-patch-1")
        );

        let (status, headers, _) = patch_req(
            Some(&token().await),
            "88888888-8888-4888-8888-888888888888",
            Some("corr-patch-2"),
            json!({ "edgeCloudRegion": "us-east-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-patch-2")
        );
    }

    // --- postTrafficInfluenceDevice (per-device create) --------------------

    async fn device_token() -> String {
        mint_token(DEVICE_WRITE_SCOPE).await
    }

    /// A minimal per-device create body: base fields + a `device` with one id.
    fn device_body(app_id: &str) -> Value {
        json!({
            "apiConsumerId": CONSUMER,
            "appId": app_id,
            "device": { "phoneNumber": "+123456789012" },
        })
    }

    async fn post_device(
        token: Option<&str>,
        correlator: Option<&str>,
        body: Value,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(DEVICE_COLLECTION)
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

    #[test]
    fn validate_device_accepts_each_identifier_and_rejects_malformed() {
        let none: Option<HeaderValue> = None;
        // Each single identifier is accepted.
        let phone = Device {
            phone_number: Some("+123456789012".into()),
            network_access_identifier: None,
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(validate_device(Some(phone), &none).is_ok());
        let nai = Device {
            phone_number: None,
            network_access_identifier: Some("123456789@nai.example".into()),
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(validate_device(Some(nai), &none).is_ok());
        let v4 = Device {
            phone_number: None,
            network_access_identifier: None,
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.1".into()),
                private_address: None,
                public_port: Some(443),
            }),
            ipv6_address: None,
        };
        assert!(validate_device(Some(v4), &none).is_ok());
        // Absent device → error; empty device (minProperties 1) → error.
        assert!(validate_device(None, &none).is_err());
        let empty = Device {
            phone_number: None,
            network_access_identifier: None,
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(validate_device(Some(empty), &none).is_err());
        // Malformed phone number → error.
        let bad_phone = Device {
            phone_number: Some("12345".into()),
            network_access_identifier: None,
            ipv4_address: None,
            ipv6_address: None,
        };
        assert!(validate_device(Some(bad_phone), &none).is_err());
        // ipv4 without a publicAddress → error; out-of-range publicPort → error.
        let v4_no_addr = Device {
            phone_number: None,
            network_access_identifier: None,
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: None,
                private_address: None,
                public_port: None,
            }),
            ipv6_address: None,
        };
        assert!(validate_device(Some(v4_no_addr), &none).is_err());
        let v4_bad_port = Device {
            phone_number: None,
            network_access_identifier: None,
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.1".into()),
                private_address: None,
                public_port: Some(70000),
            }),
            ipv6_address: None,
        };
        assert!(validate_device(Some(v4_bad_port), &none).is_err());
    }

    #[tokio::test]
    async fn device_happy_path_creates_a_readable_resource_and_never_echoes_the_device() {
        let (status, headers, body) =
            post_device(Some(&device_token().await), None, device_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
        // Same TrafficInfluence resource shape: id/appId/state, and appId is the
        // control plane (…174002 tail → active).
        assert_eq!(body["appId"], APP_ACTIVE);
        assert_eq!(body["state"], "active");
        let id = body["trafficInfluenceID"].as_str().expect("id minted");
        assert!(is_uuid_any(id));
        // The device is never echoed (upstream privacy rule).
        assert!(body.get("device").is_none(), "device must not be echoed");
        // Location points under the collection, and the resource reads back there.
        let location = headers
            .get("location")
            .and_then(|v| v.to_str().ok())
            .expect("Location header");
        assert_eq!(location, format!("{COLLECTION}/{id}"));
        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_req(Some(&read), id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched, body, "read-back is verbatim");
    }

    #[tokio::test]
    async fn device_state_reflects_the_app_id_tail() {
        let t = device_token().await;
        for (app, want) in [(APP_ORDERED, "ordered"), (APP_CREATED, "created"), (APP_ACTIVE, "active")]
        {
            let (status, _, body) = post_device(Some(&t), None, device_body(app)).await;
            assert_eq!(status, StatusCode::CREATED, "app {app}");
            assert_eq!(body["state"], want, "app {app}");
        }
    }

    #[tokio::test]
    async fn device_reserved_app_id_suffix_selects_a_canonical_camara_error() {
        let t = device_token().await;
        for (suffix, want) in [
            ("404", StatusCode::NOT_FOUND),
            ("409", StatusCode::CONFLICT),
            ("429", StatusCode::TOO_MANY_REQUESTS),
        ] {
            let app = format!("123e4567-e89b-12d3-a456-426614174{suffix}");
            let (status, _, _) = post_device(Some(&t), None, device_body(&app)).await;
            assert_eq!(status, want, "suffix {suffix}");
        }
    }

    #[tokio::test]
    async fn device_accepts_each_identifier_kind_over_the_router() {
        let t = device_token().await;
        let devices = [
            json!({ "phoneNumber": "+123456789012" }),
            json!({ "networkAccessIdentifier": "123456789@nai.example" }),
            json!({ "ipv4Address": { "publicAddress": "203.0.113.1", "publicPort": 443 } }),
            json!({ "ipv6Address": "2001:db8::1" }),
        ];
        for device in devices {
            let body = json!({ "apiConsumerId": CONSUMER, "appId": APP_ACTIVE, "device": device });
            let (status, _, _) = post_device(Some(&t), None, body.clone()).await;
            assert_eq!(status, StatusCode::CREATED, "{body}");
        }
    }

    #[tokio::test]
    async fn device_missing_or_empty_is_invalid_argument() {
        let t = device_token().await;
        let cases = [
            json!({ "apiConsumerId": CONSUMER, "appId": APP_ACTIVE }), // no device
            json!({ "apiConsumerId": CONSUMER, "appId": APP_ACTIVE, "device": {} }), // minProperties 1
        ];
        for body in cases {
            let (status, _, err) = post_device(Some(&t), None, body.clone()).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(err["code"], "INVALID_ARGUMENT", "{body}");
        }
    }

    #[tokio::test]
    async fn device_malformed_identifiers_are_rejected() {
        let t = device_token().await;
        // Malformed phone → 400 INVALID_ARGUMENT.
        let bad_phone =
            json!({ "apiConsumerId": CONSUMER, "appId": APP_ACTIVE, "device": { "phoneNumber": "12345" } });
        let (status, _, err) = post_device(Some(&t), None, bad_phone).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
        // ipv4 without publicAddress → 400 INVALID_ARGUMENT.
        let v4_no_addr = json!({
            "apiConsumerId": CONSUMER, "appId": APP_ACTIVE,
            "device": { "ipv4Address": { "privateAddress": "10.0.0.1" } },
        });
        let (status, _, err) = post_device(Some(&t), None, v4_no_addr).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
        // Unknown identifier key → 400 (deny_unknown_fields on Device).
        let unknown = json!({
            "apiConsumerId": CONSUMER, "appId": APP_ACTIVE,
            "device": { "imsi": "123456789012345" },
        });
        let (status, _, _) = post_device(Some(&t), None, unknown).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn device_ipv4_public_port_out_of_range_is_out_of_range() {
        let t = device_token().await;
        let body = json!({
            "apiConsumerId": CONSUMER, "appId": APP_ACTIVE,
            "device": { "ipv4Address": { "publicAddress": "203.0.113.1", "publicPort": 70000 } },
        });
        let (status, _, err) = post_device(Some(&t), None, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn device_shares_base_field_validation() {
        // The base fields are validated by the same code as the collection create:
        // a bad appId → 400, and a base 400 wins over any device problem.
        let t = device_token().await;
        let bad_app =
            json!({ "apiConsumerId": CONSUMER, "appId": "nope", "device": { "phoneNumber": "bad" } });
        let (status, _, err) = post_device(Some(&t), None, bad_app).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
        let no_consumer = json!({ "appId": APP_ACTIVE, "device": { "phoneNumber": "+123456789012" } });
        let (status, _, err) = post_device(Some(&t), None, no_consumer).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn device_create_requires_the_device_scope() {
        // The collection write scope is NOT sufficient for the per-device create.
        let collection = mint_token(WRITE_SCOPE).await;
        let (status, _, _) = post_device(Some(&collection), None, device_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // The dedicated device scope works.
        let device = device_token().await;
        let (status, _, _) = post_device(Some(&device), None, device_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn device_missing_token_is_unauthenticated() {
        let (status, _, _) = post_device(None, None, device_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn device_non_json_body_is_invalid_argument() {
        let request = Request::builder()
            .method("POST")
            .uri(DEVICE_COLLECTION)
            .header("host", HOST)
            .header("authorization", format!("Bearer {}", device_token().await))
            .header("content-type", "application/json")
            .body(Body::from("{not json"))
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn device_x_correlator_is_echoed_on_success_and_error() {
        let t = device_token().await;
        let (status, headers, _) =
            post_device(Some(&t), Some("corr-dev-1"), device_body(APP_ACTIVE)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-dev-1")
        );
        let app404 = "123e4567-e89b-12d3-a456-426614174404";
        let (status, headers, _) =
            post_device(Some(&t), Some("corr-dev-2"), device_body(app404)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-dev-2")
        );
    }

    // --- subscriptionRequest: create-time initial CloudEvent ----------------

    use crate::apis::traffic_influence::notifications::EVENT_TYPE;

    /// Build a valid `subscriptionRequest` for the traffic-influence-change type.
    fn sub_request(sink: &str, initial_event: bool, cred: Option<Value>) -> Value {
        let mut sr = json!({
            "sink": sink,
            "protocol": "HTTP",
            "types": [EVENT_TYPE],
            "config": { "subscriptionDetail": {}, "initialEvent": initial_event },
        });
        if let Some(c) = cred {
            sr["sinkCredential"] = c;
        }
        sr
    }

    /// A create body carrying `subscriptionRequest`.
    fn create_body_sub(app_id: &str, sub: Value) -> Value {
        json!({ "apiConsumerId": CONSUMER, "appId": app_id, "subscriptionRequest": sub })
    }

    /// Accept one fire-and-forget notification and return the raw head + JSON body.
    async fn read_one_event(listener: &tokio::net::TcpListener) -> (String, Value) {
        use tokio::io::AsyncReadExt;
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        (head.to_string(), serde_json::from_str(body).expect("body is JSON"))
    }

    #[test]
    fn subscription_validation_accepts_well_formed_and_rejects_bad_requests() {
        // Happy path → initial_event + derived bearer.
        let sr: SubscriptionRequest = serde_json::from_value(sub_request(
            "http://127.0.0.1:9/cb",
            true,
            Some(json!({ "credentialType": "ACCESSTOKEN", "accessToken": "tok", "accessTokenType": "bearer" })),
        ))
        .unwrap();
        let ok = validate_subscription(sr, &None).expect("valid subscription");
        assert_eq!(ok.sink, "http://127.0.0.1:9/cb");
        assert!(ok.initial_event);
        assert_eq!(ok.auth.as_deref(), Some("Bearer tok"));

        // https sink accepted (delivered as a no-op cut); initialEvent defaults false.
        let sr: SubscriptionRequest =
            serde_json::from_value(json!({ "sink": "https://x.test/cb", "protocol": "HTTP", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } })).unwrap();
        let ok = validate_subscription(sr, &None).expect("https accepted");
        assert!(!ok.initial_event);
        assert!(ok.auth.is_none());

        // Each malformed shape → Err.
        for bad in [
            json!({ "protocol": "HTTP", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // no sink
            json!({ "sink": "ftp://x/cb", "protocol": "HTTP", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // bad sink scheme
            json!({ "sink": "http://x/cb", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // no protocol
            json!({ "sink": "http://x/cb", "protocol": "MQTT5", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // unsupported protocol
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "config": { "subscriptionDetail": {} } }), // no types
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "types": ["org.example.other"], "config": { "subscriptionDetail": {} } }), // wrong type
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "types": [EVENT_TYPE, EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // too many types
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "types": [EVENT_TYPE] }), // no config
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "types": [EVENT_TYPE], "config": {} }), // no subscriptionDetail
        ] {
            let sr: SubscriptionRequest = serde_json::from_value(bad.clone()).unwrap();
            assert!(validate_subscription(sr, &None).is_err(), "should reject: {bad}");
        }
    }

    #[test]
    fn sink_validation_accepts_http_and_https_only() {
        assert!(is_valid_sink("http://127.0.0.1:8080/cb"));
        assert!(is_valid_sink("https://endpoint.example.com/sink"));
        assert!(!is_valid_sink("ftp://x/cb"));
        assert!(!is_valid_sink("http://"));
        assert!(!is_valid_sink("nonsense"));
    }

    #[tokio::test]
    async fn create_with_initial_event_fires_a_traffic_influence_change_cloudevent() {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/ti-notify");

        let body = create_body_sub(APP_ACTIVE, sub_request(&sink, true, None));
        let (status, _, created) = post(Some(&token().await), None, body).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["trafficInfluenceID"].as_str().unwrap().to_string();
        // The subscription/credential is never echoed in the resource.
        assert!(created.get("subscriptionRequest").is_none());

        let (head, event) = read_one_event(&listener).await;
        assert!(head.starts_with("POST /ti-notify HTTP/1.1\r\n"), "request line: {head}");
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        assert_eq!(event["type"], EVENT_TYPE);
        assert_eq!(event["specversion"], "1.0");
        assert!(event["id"].is_string() && event["time"].is_string());
        // data is the created resource.
        assert_eq!(event["data"]["trafficInfluenceID"], json!(id));
        assert_eq!(event["data"]["appId"], APP_ACTIVE);
        assert_eq!(event["data"]["state"], "active");
    }

    #[tokio::test]
    async fn the_initial_event_callback_carries_the_sink_credential_bearer() {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/ti-auth");

        let cred = json!({
            "credentialType": "ACCESSTOKEN",
            "accessToken": "ti-sink-secret",
            "accessTokenType": "bearer",
        });
        let body = create_body_sub(APP_ACTIVE, sub_request(&sink, true, Some(cred)));
        let (status, _, created) = post(Some(&token().await), None, body).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed.
        assert!(created.get("sinkCredential").is_none());

        let (head, _) = read_one_event(&listener).await;
        assert!(
            head.contains("Authorization: Bearer ti-sink-secret\r\n"),
            "authorization header present: {head}"
        );
    }

    #[tokio::test]
    async fn the_initial_event_callback_carries_a_plain_sink_credential_as_basic() {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/ti-basic");

        // A PLAIN sinkCredential → RFC 7617 HTTP Basic on the callback.
        // base64("cbid:cbsecret") == "Y2JpZDpjYnNlY3JldA==".
        let cred = json!({
            "credentialType": "PLAIN",
            "identifier": "cbid",
            "secret": "cbsecret",
        });
        let body = create_body_sub(APP_ACTIVE, sub_request(&sink, true, Some(cred)));
        let (status, _, created) = post(Some(&token().await), None, body).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed.
        assert!(created.get("sinkCredential").is_none());

        let (head, _) = read_one_event(&listener).await;
        assert!(
            head.contains("Authorization: Basic Y2JpZDpjYnNlY3JldA==\r\n"),
            "basic authorization header present: {head}"
        );
    }

    #[tokio::test]
    async fn create_without_initial_event_fires_no_event() {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/ti-silent");

        // A valid subscription but initialEvent:false → the subscription is created
        // (accepted) but no create-time event fires (ongoing stream is a cut).
        let body = create_body_sub(APP_ACTIVE, sub_request(&sink, false, None));
        let (status, _, _) = post(Some(&token().await), None, body).await;
        assert_eq!(status, StatusCode::CREATED);

        let accepted = tokio::time::timeout(
            std::time::Duration::from_millis(400),
            listener.accept(),
        )
        .await;
        assert!(accepted.is_err(), "initialEvent:false must not notify the sink");
    }

    #[tokio::test]
    async fn an_unreachable_https_sink_initial_event_still_returns_201() {
        // https sinks are now delivered to over verified rustls TLS (see
        // `notifications::deliver_tls_posts_a_cloudevent_over_a_verified_tls_session`
        // for the round-trip). Delivery is fire-and-forget off the request path, so
        // an unresolvable/unreachable https sink fails silently and never blocks or
        // fails the `201`.
        let body = create_body_sub(APP_ACTIVE, sub_request("https://sink.example.test/cb", true, None));
        let (status, _, _) = post(Some(&token().await), None, body).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn a_malformed_subscription_request_is_invalid_argument() {
        let t = token().await;
        for bad in [
            json!({ "protocol": "HTTP", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // no sink
            json!({ "sink": "http://x/cb", "protocol": "MQTT5", "types": [EVENT_TYPE], "config": { "subscriptionDetail": {} } }), // unsupported protocol
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "types": ["org.example.other"], "config": { "subscriptionDetail": {} } }), // wrong type
            json!({ "sink": "http://x/cb", "protocol": "HTTP", "types": [EVENT_TYPE], "config": {} }), // no subscriptionDetail
        ] {
            let (status, _, _) = post(Some(&t), None, create_body_sub(APP_ACTIVE, bad.clone())).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "should reject: {bad}");
        }
    }
}
