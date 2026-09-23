//! Geofencing Subscriptions **v0.4** (CAMARA geofencing-subscriptions 0.4.0,
//! release r3.2).
//!
//! The subscription lifecycle (CRUD):
//! - `POST /geofencing-subscriptions/v0.4/subscriptions` — create a geofencing
//!   subscription and return its `SubscriptionInfo` (operationId
//!   `createSubscription`, scope `geofencing-subscriptions:subscriptions:create`).
//! - `GET /geofencing-subscriptions/v0.4/subscriptions` — list the stored
//!   subscriptions (operationId `retrieveSubscriptionList`, scope
//!   `geofencing-subscriptions:subscriptions:read`).
//! - `GET /geofencing-subscriptions/v0.4/subscriptions/{subscriptionId}` — read a
//!   subscription back by id (operationId `retrieveSubscription`, scope
//!   `geofencing-subscriptions:subscriptions:read`).
//! - `DELETE /geofencing-subscriptions/v0.4/subscriptions/{subscriptionId}` —
//!   delete a subscription (operationId `deleteSubscription`, scope
//!   `geofencing-subscriptions:subscriptions:delete`).
//!
//! ## What it does
//!
//! The caller registers a subscription: a `device`, a circular `area`, and the
//! event `types` of interest (`area-entered` / `area-left`), plus a `sink`
//! callback URL the network would POST CloudEvents to on each matching
//! transition. `createSubscription` mints an opaque, UUID-shaped `id`
//! ([`super::store`]), renders the `SubscriptionInfo`, remembers it, and returns
//! `201`; `retrieveSubscription` returns the stored `SubscriptionInfo` (`200`) or
//! `404 NOT_FOUND`.
//!
//! Both endpoints are protected: they require a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the relevant scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! `createSubscription` reads two control planes:
//!
//! **1. The identifier** (the `config.subscriptionDetail.device` id —
//! `phoneNumber`, else `networkAccessIdentifier`, else the IPv4 `publicAddress`,
//! else `ipv6Address` — or, when no `device` is supplied, the access token
//! subject `sub`). Its trailing three digits select the outcome:
//!
//! - **Reserved error suffix** (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//!   `…429`, `…500`, `…503`) → the canonical CAMARA error (shared convention,
//!   [`crate::scenarios`]).
//! - **`…000`** (and an identifier with no trailing digits) →
//!   `status: "ACTIVATION_REQUESTED"` (the network has accepted the request but
//!   not yet activated the subscription).
//! - **any other input** (the happy-path default) → `status: "ACTIVE"`.
//!
//! **2. The area** (`radius`). Geofencing only supports a `CIRCLE`, whose radius
//! CAMARA bounds to [`MIN_RADIUS_METRES`]–[`MAX_RADIUS_METRES`] m: a `radius`
//! outside that band → `400 OUT_OF_RANGE`. A `center` outside the valid
//! latitude/longitude range → `400 OUT_OF_RANGE`; a missing `area`/`center`/
//! `radius`, or an `areaType` other than `CIRCLE`, → `400 INVALID_ARGUMENT`.
//!
//! Examples: `+123456789012` in a 5000 m circle → `ACTIVE`; `+123456789000` →
//! `ACTIVATION_REQUESTED`; `+123456789404` → `404 NOT_FOUND`; any device in a
//! 500 m circle → `400 OUT_OF_RANGE`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{notifications, store};
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to create a subscription. CAMARA geofencing-subscriptions
/// publishes per-event-type scopes; CamaraSim collapses them to a single
/// resource-action scope (a single subscription can carry several event types),
/// a documented simplification mirroring the QoD `sessions:*` scopes.
const CREATE_SCOPE: &str = "geofencing-subscriptions:subscriptions:create";
/// Scope required to read a subscription back (single read or the list).
const READ_SCOPE: &str = "geofencing-subscriptions:subscriptions:read";
/// Scope required to delete a subscription.
const DELETE_SCOPE: &str = "geofencing-subscriptions:subscriptions:delete";

/// Minimum circle radius, in metres (CAMARA geofencing `radius.minimum`). A
/// radius below this is out of range.
const MIN_RADIUS_METRES: f64 = 2000.0;
/// Maximum circle radius, in metres (CAMARA geofencing `radius.maximum`). A
/// radius above this is out of range.
const MAX_RADIUS_METRES: f64 = 200_000.0;

/// The only `protocol` CamaraSim delivers over (CAMARA subscription-template
/// allows MQTT/AMQP/NATS/KAFKA too; only HTTP is supported here — a documented
/// cut).
const SUPPORTED_PROTOCOL: &str = "HTTP";

/// How long after creation a simulated boundary crossing (`…001`/`…002` tail) is
/// delivered, in seconds — a short, fixed grace so a headless caller can observe
/// the movement event just after the `201`. Mirrors QoD's
/// `NETWORK_TERMINATION_GRACE_SECS`.
const MOVEMENT_GRACE_SECS: u64 = 1;

/// The geofencing event types CamaraSim accepts in `types` (CAMARA
/// geofencing-subscriptions 0.4.0 → major version `v0`).
const KNOWN_EVENT_TYPES: &[&str] = &[
    "org.camaraproject.geofencing-subscriptions.v0.area-entered",
    "org.camaraproject.geofencing-subscriptions.v0.area-left",
];

/// Routes for Geofencing Subscriptions v0.4, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/geofencing-subscriptions/v0.4/subscriptions",
            post(create_subscription).get(list_subscriptions),
        )
        .route(
            "/geofencing-subscriptions/v0.4/subscriptions/:subscription_id",
            get(retrieve_subscription).delete(delete_subscription),
        )
}

/// `POST /subscriptions` request body (CAMARA `SubscriptionRequest`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSubscription {
    protocol: Option<String>,
    sink: Option<String>,
    // Carries a secret, so it is never echoed. An ACCESSTOKEN credential is applied
    // to every callback as an RFC 6750 `Authorization: Bearer` header, and a PLAIN
    // credential as an RFC 7617 `Authorization: Basic` header (see
    // [`notifications::sink_authorization`]); REFRESHTOKEN is accepted but not
    // applied — a documented cut.
    #[serde(rename = "sinkCredential")]
    sink_credential: Option<Value>,
    types: Option<Vec<String>>,
    config: Option<Config>,
}

/// The CAMARA subscription `config`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(rename = "subscriptionDetail")]
    subscription_detail: Option<SubscriptionDetail>,
    #[serde(rename = "subscriptionExpireTime")]
    subscription_expire_time: Option<String>,
    #[serde(rename = "subscriptionMaxEvents")]
    subscription_max_events: Option<i64>,
    #[serde(rename = "initialEvent")]
    initial_event: Option<bool>,
}

/// The geofencing `subscriptionDetail`: which `device`, and which `area` to watch.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubscriptionDetail {
    device: Option<Device>,
    area: Option<Area>,
}

/// The CAMARA `Device` object: at least one identifier must be present.
/// CamaraSim keys its functional cases off the first present identifier, in the
/// precedence order below.
#[derive(Debug, Default, Deserialize)]
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

/// The CAMARA `DeviceIpv4Addr` object. CamaraSim reads the `publicAddress` as the
/// identifier; the other fields are accepted for schema fidelity.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceIpv4Addr {
    #[serde(rename = "publicAddress")]
    public_address: Option<String>,
    #[serde(rename = "privateAddress")]
    #[allow(dead_code)]
    private_address: Option<String>,
    #[serde(rename = "publicPort")]
    #[allow(dead_code)]
    public_port: Option<i64>,
}

/// The CAMARA `Area`. The discriminator is `areaType`; only `CIRCLE` is
/// supported, so `center`/`radius` are parsed leniently and their presence is
/// validated in the handler for a precise error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Area {
    #[serde(rename = "areaType")]
    area_type: String,
    center: Option<Point>,
    radius: Option<f64>,
}

/// A CAMARA `Point` — a WGS-84 latitude/longitude in decimal degrees.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    latitude: f64,
    longitude: f64,
}

/// `POST /geofencing-subscriptions/v0.4/subscriptions`.
async fn create_subscription(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (several required fields); parse strictly.
    let req: CreateSubscription = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid SubscriptionRequest.",
                &correlator,
            )
        }
    };

    // --- Envelope validation (syntactic 400s first) -----------------------
    match req.protocol.as_deref() {
        Some(SUPPORTED_PROTOCOL) => {}
        Some(_) => {
            return invalid_argument(
                "Only `protocol: \"HTTP\"` is supported.",
                &correlator,
            )
        }
        None => return invalid_argument("`protocol` is required.", &correlator),
    }
    match req.sink.as_deref() {
        Some(s) if !s.is_empty() => {}
        _ => return invalid_argument("`sink` is required and must be a non-empty URI.", &correlator),
    }
    let types = match &req.types {
        Some(t) if !t.is_empty() => t,
        _ => return invalid_argument("`types` is required and must be non-empty.", &correlator),
    };
    if let Some(bad) = types.iter().find(|t| !KNOWN_EVENT_TYPES.contains(&t.as_str())) {
        return invalid_argument(
            &format!("`{bad}` is not a supported geofencing event type."),
            &correlator,
        );
    }

    let config = match req.config {
        Some(c) => c,
        None => return invalid_argument("`config` is required.", &correlator),
    };
    // `subscriptionMaxEvents`, when supplied, must be at least 1 (CAMARA minimum:
    // a subscription that ends before its first event is meaningless).
    if let Some(m) = config.subscription_max_events {
        if m < 1 {
            return out_of_range(
                "`config.subscriptionMaxEvents` must be at least 1.",
                &correlator,
            );
        }
    }
    let detail = match config.subscription_detail {
        Some(d) => d,
        None => return invalid_argument("`config.subscriptionDetail` is required.", &correlator),
    };

    // --- Area validation --------------------------------------------------
    let area = match &detail.area {
        Some(a) => a,
        None => {
            return invalid_argument(
                "`config.subscriptionDetail.area` is required.",
                &correlator,
            )
        }
    };
    if area.area_type != "CIRCLE" {
        return invalid_argument("Only `CIRCLE` `areaType` is supported.", &correlator);
    }
    let Some(center) = &area.center else {
        return invalid_argument("`area.center` is required for a CIRCLE.", &correlator);
    };
    let Some(radius) = area.radius else {
        return invalid_argument("`area.radius` is required for a CIRCLE.", &correlator);
    };
    if !(-90.0..=90.0).contains(&center.latitude) || !(-180.0..=180.0).contains(&center.longitude) {
        return out_of_range(
            "`area.center` latitude must be in [-90, 90] and longitude in [-180, 180].",
            &correlator,
        );
    }
    if !(MIN_RADIUS_METRES..=MAX_RADIUS_METRES).contains(&radius) {
        return out_of_range(
            &format!(
                "`area.radius` must be between {} and {} metres.",
                MIN_RADIUS_METRES as i64, MAX_RADIUS_METRES as i64
            ),
            &correlator,
        );
    }

    // --- Identifier resolution + reserved-error convention ----------------
    let (identifier, device_echo) = match resolve_identifier(detail.device, &claims, &correlator) {
        Ok(resolved) => resolved,
        Err(resp) => return resp,
    };
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // --- Status (identifier is the control plane) -------------------------
    let status = match scenarios::trailing_three_digits(&identifier) {
        Some(0) | None => "ACTIVATION_REQUESTED",
        _ => "ACTIVE",
    };

    // Build the SubscriptionInfo, remember it, and return 201.
    let id = store::new_subscription_id();
    let info = build_subscription_info(
        &id,
        req.sink.as_deref().unwrap_or_default(),
        types,
        device_echo,
        area,
        center,
        radius,
        &config.subscription_expire_time,
        config.subscription_max_events,
        config.initial_event,
        status,
    );
    store::insert(id.clone(), info.clone());

    // Register the subscriptionMaxEvents budget (if any), so each delivered domain
    // event (area-entered / area-left) is counted and the subscription ends once the
    // budget is spent (see `deliver_counted`). Validated `>= 1` above.
    if let Some(m) = config.subscription_max_events {
        store::set_event_budget(id.clone(), m as u64);
    }

    // Initial event (config.initialEvent): if the subscription is ACTIVE and the
    // caller asked for the device's current in/out state, deliver a single
    // `area-entered`/`area-left` CloudEvent to the sink (fire-and-forget, off the
    // request path). The event type is chosen from the identifier's trailing three
    // digits and filtered to the subscribed `types` (see notifications module). An
    // ACCESSTOKEN/PLAIN `sinkCredential` is applied to the callback (bearer/basic).
    if let Some(event_type) = notifications::initial_event_type(
        config.initial_event,
        status,
        scenarios::trailing_three_digits(&identifier),
        types,
    ) {
        if let Some(sink) = req.sink.as_deref() {
            let detail = &info["config"]["subscriptionDetail"];
            let device = detail.get("device").cloned();
            let area_json = detail["area"].clone();
            let event = notifications::geofencing_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                event_type,
                &id,
                device.as_ref(),
                &area_json,
            );
            let auth = req
                .sink_credential
                .as_ref()
                .and_then(notifications::sink_authorization);
            deliver_counted(&id, sink.to_string(), event, auth);
        }
    }

    // Movement (simulated boundary crossing): a `…001`/`…002` identifier tail on an
    // ACTIVE, sink-bearing subscription instructs the (simulated) network to report
    // a crossing shortly after creation — `…001` → the device enters (`area-entered`),
    // `…002` → it leaves (`area-left`) — filtered to the subscribed `types` and
    // delivered off the request path by a short fire-and-forget timer (mirroring
    // QoD's `…001` NETWORK_TERMINATED). The subscription stays ACTIVE unless the
    // movement event exhausts its `subscriptionMaxEvents` budget, in which case it
    // ends (see `deliver_counted`). An ACCESSTOKEN/PLAIN `sinkCredential` is applied
    // to the callback (bearer/basic).
    if let Some(event_type) = notifications::movement_event_type(
        status,
        scenarios::trailing_three_digits(&identifier),
        types,
    ) {
        if let Some(sink) = req.sink.as_deref() {
            let detail = &info["config"]["subscriptionDetail"];
            let device = detail.get("device").cloned();
            let area_json = detail["area"].clone();
            let auth = req
                .sink_credential
                .as_ref()
                .and_then(notifications::sink_authorization);
            spawn_movement(
                id.clone(),
                sink.to_string(),
                event_type,
                device,
                area_json,
                auth,
            );
        }
    }

    // Expiry (config.subscriptionExpireTime): if the caller set an expiry time and
    // the sink is deliverable, schedule a fire-and-forget timer that ends the
    // subscription at that instant — evicting it and delivering a
    // `subscription-ended` CloudEvent (terminationReason: SUBSCRIPTION_EXPIRED) —
    // off the request path. Only the RFC 3339 UTC (`…Z`) form drives the timer (a
    // documented cut); an unparseable value is still echoed but arms no timer. An
    // ACCESSTOKEN/PLAIN `sinkCredential` is applied to the callback (captured here).
    if let Some(expires) = config
        .subscription_expire_time
        .as_deref()
        .and_then(parse_rfc3339_utc)
    {
        if let Some(sink) = req.sink.as_deref() {
            let auth = req
                .sink_credential
                .as_ref()
                .and_then(notifications::sink_authorization);
            spawn_expiry(id.clone(), sink.to_string(), expires, auth);
        }
    }

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// Schedule the `subscription-ended` (SUBSCRIPTION_EXPIRED) transition for a
/// stored subscription.
///
/// Spawns a fire-and-forget async timer (never on the request path, DESIGN §11)
/// that waits until `expires_at` (Unix seconds, UTC), then — if the subscription
/// still exists — evicts it and delivers a `subscription-ended` CloudEvent
/// (`terminationReason: SUBSCRIPTION_EXPIRED`) to `sink`. A `deleteSubscription`
/// that removed the subscription first makes this a no-op (`store::remove` is then
/// `None`), so at most one terminal outcome occurs. Geofencing subscriptions have
/// no "extend" operation, so — unlike QoD — the expiry instant is fixed and the
/// timer sleeps just once. The sleep is async, so the (single-node, in-memory)
/// runtime is never blocked. `auth`, when present, applies the subscription's
/// `sinkCredential` as an `Authorization` header (ACCESSTOKEN → `Bearer`, RFC 6750;
/// PLAIN → `Basic`, RFC 7617; see [`notifications::sink_authorization`]).
fn spawn_expiry(subscription_id: String, sink: String, expires_at: i64, auth: Option<String>) {
    tokio::spawn(async move {
        let now = now_unix_secs();
        if now < expires_at {
            tokio::time::sleep(Duration::from_secs((expires_at - now) as u64)).await;
        }
        // Evict it; if a concurrent delete beat us, `remove` is None and we send
        // nothing (deletion is a synchronous, event-less cut).
        if store::remove(&subscription_id).is_some() {
            let event = notifications::subscription_ended_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                &subscription_id,
                "SUBSCRIPTION_EXPIRED",
            );
            notifications::spawn_delivery(sink, event, auth);
        }
    });
}

/// Schedule a simulated boundary-crossing movement CloudEvent for a `…001`/`…002`
/// subscription.
///
/// Spawns a fire-and-forget async timer (never on the request path, DESIGN §11)
/// that waits [`MOVEMENT_GRACE_SECS`] — a short, fixed grace, not a real device
/// motion — then, **if the subscription still exists**, delivers a single
/// `area-entered` (`event_type`) / `area-left` movement CloudEvent to `sink`. A
/// `deleteSubscription` or an expiry that evicted the subscription first makes this
/// a no-op (the `store::get` check is `None`), so a movement event is never
/// delivered for a subscription that has already ended. The sleep is async, so the
/// (single-node, in-memory) runtime is never blocked. `auth`, when present, applies
/// the subscription's `sinkCredential` as an `Authorization` header (ACCESSTOKEN →
/// `Bearer`, RFC 6750; PLAIN → `Basic`, RFC 7617; see
/// [`notifications::sink_authorization`]).
fn spawn_movement(
    subscription_id: String,
    sink: String,
    event_type: &'static str,
    device: Option<Value>,
    area: Value,
    auth: Option<String>,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(MOVEMENT_GRACE_SECS)).await;
        // Only deliver if the subscription is still live (not deleted/expired).
        if store::get(&subscription_id).is_some() {
            let event = notifications::geofencing_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                event_type,
                &subscription_id,
                device.as_ref(),
                &area,
            );
            deliver_counted(&subscription_id, sink, event, auth);
        }
    });
}

/// Deliver a domain (`area-entered` / `area-left`) CloudEvent to `sink`, counting it
/// against the subscription's `subscriptionMaxEvents` budget (docs/DESIGN.md §7).
///
/// A subscription that set no `subscriptionMaxEvents` is unbounded: the event is
/// delivered and the subscription is left untouched. Otherwise each delivered event
/// consumes one unit of budget ([`store::consume_event`]):
/// - while budget remains → deliver the event;
/// - the event that spends the **last** unit → deliver it and then **end** the
///   subscription: evict it and POST a `subscription-ended` CloudEvent
///   (`terminationReason: MAX_EVENTS_REACHED`) to the same `sink`, after the
///   triggering event and in order (via [`notifications::spawn_delivery_seq`]);
/// - a budget already spent → suppress the event (defensive).
///
/// The eviction is synchronous (done before this returns), so a still-pending
/// movement/expiry timer for the same subscription then sees it gone and becomes a
/// no-op — exactly one terminal outcome fires. Only the network I/O is spawned, never
/// on the request path (DESIGN §11). An ACCESSTOKEN/PLAIN `sinkCredential` (`auth`)
/// is applied to every callback, including the terminal `subscription-ended` event.
fn deliver_counted(subscription_id: &str, sink: String, event: Value, auth: Option<String>) {
    match store::consume_event(subscription_id) {
        store::EventBudget::Unbounded | store::EventBudget::Allowed => {
            notifications::spawn_delivery(sink, event, auth);
        }
        store::EventBudget::Last => {
            // Evict now so any pending movement/expiry timer becomes a no-op, then
            // deliver the triggering event followed (in order) by subscription-ended.
            store::remove(subscription_id);
            let ended = notifications::subscription_ended_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                subscription_id,
                "MAX_EVENTS_REACHED",
            );
            notifications::spawn_delivery_seq(sink, vec![event, ended], auth);
        }
        store::EventBudget::Exhausted => { /* budget already spent: suppress */ }
    }
}

/// `GET /geofencing-subscriptions/v0.4/subscriptions/{subscriptionId}`.
async fn retrieve_subscription(
    claims: Claims,
    headers: HeaderMap,
    Path(subscription_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&subscription_id) {
        Some(info) => {
            with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No subscription exists for the supplied id.").into_response(),
            &correlator,
        ),
    }
}

/// `GET /geofencing-subscriptions/v0.4/subscriptions`.
///
/// Lists the stored subscriptions as an array of `SubscriptionInfo` (`200`), or
/// an empty array when there are none. CamaraSim does not scope subscriptions per
/// client, so this returns every subscription in the store — a documented
/// simplification (see [`store::all`]).
async fn list_subscriptions(claims: Claims, headers: HeaderMap) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let subscriptions = store::all();
    with_correlator(
        (StatusCode::OK, Json(subscriptions)).into_response(),
        &correlator,
    )
}

/// `DELETE /geofencing-subscriptions/v0.4/subscriptions/{subscriptionId}`.
///
/// Deletes the subscription, stopping any future notifications. Keyed only on the
/// stored state: a subscription that exists is evicted → `204 No Content`; an
/// unknown (or already deleted) id → `404 NOT_FOUND`. No `subscription-ended`
/// CloudEvent is emitted (notification delivery is deferred — a documented cut),
/// so the deletion is synchronous and CamaraSim answers `204` rather than the
/// template's async `202`.
async fn delete_subscription(
    claims: Claims,
    headers: HeaderMap,
    Path(subscription_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::remove(&subscription_id) {
        Some(_) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No subscription exists for the supplied id.").into_response(),
            &correlator,
        ),
    }
}

/// Render the `SubscriptionInfo` returned by `createSubscription` and stored for
/// `retrieveSubscription`. Echoes the request's `sink`/`types`/`config` (the
/// device is echoed only when one was supplied), and adds the server-assigned
/// `id`, `startsAt`, and `status`. `sinkCredential` is never echoed.
#[allow(clippy::too_many_arguments)]
fn build_subscription_info(
    id: &str,
    sink: &str,
    types: &[String],
    device_echo: Option<Value>,
    area: &Area,
    center: &Point,
    radius: f64,
    expire_time: &Option<String>,
    max_events: Option<i64>,
    initial_event: Option<bool>,
    status: &str,
) -> Value {
    let mut subscription_detail = json!({
        "area": {
            "areaType": area.area_type,
            "center": { "latitude": center.latitude, "longitude": center.longitude },
            "radius": radius,
        }
    });
    if let Some(echo) = device_echo {
        subscription_detail["device"] = echo;
    }

    let mut config = json!({ "subscriptionDetail": subscription_detail });
    if let Some(t) = expire_time {
        config["subscriptionExpireTime"] = json!(t);
    }
    if let Some(m) = max_events {
        config["subscriptionMaxEvents"] = json!(m);
    }
    if let Some(i) = initial_event {
        config["initialEvent"] = json!(i);
    }

    json!({
        "id": id,
        "protocol": SUPPORTED_PROTOCOL,
        "sink": sink,
        "types": types,
        "config": config,
        "startsAt": rfc3339_utc(now_unix_secs()),
        "status": status,
    })
}

/// Resolve the device identifier for a request and, when a `device` was
/// supplied, the single-identifier echo for the response's
/// `config.subscriptionDetail.device`.
///
/// The identifier is the first present identifier in the supplied `device`
/// (validated when it is a `phoneNumber`), else the token subject (three-legged
/// fallback). On failure returns the CAMARA error `Response` to send — 400
/// `INVALID_ARGUMENT` for a malformed `phoneNumber` or a `device` carrying no
/// identifier, 422 `MISSING_IDENTIFIER` when neither a device nor a token subject
/// is present.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<(String, Option<Value>), Response> {
    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                let echo = json!({ "phoneNumber": phone });
                Ok((phone, Some(echo)))
            }
            Some(DeviceId::Nai(nai)) => {
                let echo = json!({ "networkAccessIdentifier": nai });
                Ok((nai, Some(echo)))
            }
            Some(DeviceId::Ipv4(addr)) => {
                let echo = json!({ "ipv4Address": { "publicAddress": addr } });
                Ok((addr, Some(echo)))
            }
            Some(DeviceId::Ipv6(addr)) => {
                let echo = json!({ "ipv6Address": addr });
                Ok((addr.clone(), Some(echo)))
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            let subject = claims.subject().unwrap_or("");
            if subject.is_empty() {
                return Err(with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "No `device` supplied and the access token identifies no device.",
                    )
                    .into_response(),
                    correlator,
                ));
            }
            Ok((subject.to_string(), None))
        }
    }
}

/// The identifier CamaraSim reads from a `Device`, kept distinct so the
/// `phoneNumber` E.164 check and the correct echo key can be applied. Precedence:
/// phoneNumber, networkAccessIdentifier, the IPv4 `publicAddress`, then
/// ipv6Address.
enum DeviceId {
    PhoneNumber(String),
    Nai(String),
    Ipv4(String),
    Ipv6(String),
}

/// The first present identifier of a `Device`, in precedence order, or `None`
/// when the device carries no identifier at all (`minProperties: 1` violated).
fn device_identifier(device: &Device) -> Option<DeviceId> {
    if let Some(phone) = &device.phone_number {
        return Some(DeviceId::PhoneNumber(phone.clone()));
    }
    if let Some(nai) = &device.network_access_identifier {
        return Some(DeviceId::Nai(nai.clone()));
    }
    if let Some(ipv4) = &device.ipv4_address {
        if let Some(addr) = &ipv4.public_address {
            return Some(DeviceId::Ipv4(addr.clone()));
        }
    }
    if let Some(ipv6) = &device.ipv6_address {
        return Some(DeviceId::Ipv6(ipv6.clone()));
    }
    None
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

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
}

/// Seconds since the Unix epoch, UTC.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as RFC 3339, e.g.
/// `2026-08-03T14:27:08Z`. Self-contained (no date-time dependency) via the
/// civil-from-days algorithm below (shared shape with the location APIs).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a day count since 1970-01-01 into a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian). 1-based month/day.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parse an RFC 3339 UTC instant of the form `YYYY-MM-DDTHH:MM:SSZ` into Unix
/// seconds. Only the 20-character `Z` (UTC) form is accepted — a numeric offset
/// (e.g. `+01:00`) or fractional seconds returns `None`, so it arms no expiry
/// timer (a documented cut). The inverse of [`rfc3339_utc`].
fn parse_rfc3339_utc(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    let hh: i64 = s.get(11..13)?.parse().ok()?;
    let mm: i64 = s.get(14..16)?.parse().ok()?;
    let ss: i64 = s.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// Days since 1970-01-01 for a civil `(year, month, day)` — the inverse of
/// [`civil_from_days`] (Howard Hinnant's `days_from_civil`, proleptic Gregorian).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let m = m as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (d as u64 - 1); // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "geo.local:8080";

    // A valid geofencing config with a CIRCLE area, radius within bounds, and one
    // event type, as a JSON snippet (device inserted by the caller).
    const AREA: &str =
        r#""area":{"areaType":"CIRCLE","center":{"latitude":51.5,"longitude":-0.12},"radius":5000}"#;
    const TYPE_ENTERED: &str = "org.camaraproject.geofencing-subscriptions.v0.area-entered";
    const TYPE_LEFT: &str = "org.camaraproject.geofencing-subscriptions.v0.area-left";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
    }

    #[test]
    fn device_identifier_follows_precedence() {
        let phone = Device {
            phone_number: Some("+123456789012".into()),
            network_access_identifier: Some("user@nai".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&phone), Some(DeviceId::PhoneNumber(p)) if p == "+123456789012"));

        let nai = Device {
            network_access_identifier: Some("user@nai".into()),
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.12".into()),
                private_address: None,
                public_port: None,
            }),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&nai), Some(DeviceId::Nai(p)) if p == "user@nai"));

        let ipv6 = Device {
            ipv6_address: Some("2001:db8::11".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&ipv6), Some(DeviceId::Ipv6(p)) if p == "2001:db8::11"));

        assert!(device_identifier(&Device::default()).is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "geo-client").await
    }

    async fn mint_token_with_client(scope: &str, client_id: &str) -> String {
        let enc = client_id.replace('+', "%2B");
        let body = format!("grant_type=client_credentials&client_id={enc}&scope={scope}");
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

    async fn post_subscriptions(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/geofencing-subscriptions/v0.4/subscriptions")
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
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

    async fn get_subscription(token: &str, id: &str) -> (StatusCode, Value) {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/geofencing-subscriptions/v0.4/subscriptions/{id}"
                    ))
                    .header("host", HOST)
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn list_subscriptions_req(token: Option<&str>) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/geofencing-subscriptions/v0.4/subscriptions")
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        let response = app().oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn delete_subscription_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("/geofencing-subscriptions/v0.4/subscriptions/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app().oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Build a request body with the given device fragment, the OK area, and one
    /// event type.
    fn body_with_device(device: &str) -> String {
        format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{"device":{device},{AREA}}}}}}}"#
        )
    }

    async fn create_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_subscriptions(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn happy_path_creates_an_active_subscription() {
        let (status, _, body) = create_ok(&body_with_device(r#"{"phoneNumber":"+123456789012"}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVE");
        assert_eq!(body["protocol"], "HTTP");
        assert_eq!(body["sink"], "https://example.com/cb");
        assert_eq!(body["types"][0], TYPE_ENTERED);
        // The id is a UUID-shaped string; startsAt is an RFC 3339 UTC instant.
        assert!(body["id"].as_str().unwrap().contains('-'));
        assert!(body["startsAt"].as_str().unwrap().ends_with('Z'));
        // The device is echoed inside config.subscriptionDetail.
        assert_eq!(
            body["config"]["subscriptionDetail"]["device"]["phoneNumber"],
            "+123456789012"
        );
        assert_eq!(
            body["config"]["subscriptionDetail"]["area"]["radius"].as_f64(),
            Some(5000.0)
        );
        // The secret is never echoed.
        assert!(body.get("sinkCredential").is_none());
    }

    #[tokio::test]
    async fn triple_zero_tail_is_activation_requested() {
        let (status, _, body) = create_ok(&body_with_device(r#"{"phoneNumber":"+123456789000"}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVATION_REQUESTED");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = create_ok(&body_with_device(r#"{"phoneNumber":"+123456789404"}"#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = create_ok(&body_with_device(r#"{"phoneNumber":"+123456789429"}"#)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn created_subscription_can_be_read_back() {
        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let (status, fetched) = get_subscription(&token, id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched, created);
    }

    #[tokio::test]
    async fn unknown_subscription_id_is_not_found() {
        let token = mint_token(READ_SCOPE).await;
        let (status, body) = get_subscription(&token, "no-such-id").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn nai_and_ipv6_identifiers_are_accepted_and_echoed() {
        // NAI ending …000 → ACTIVATION_REQUESTED, echoed as networkAccessIdentifier.
        let (status, _, body) =
            create_ok(&body_with_device(r#"{"networkAccessIdentifier":"user000@nai"}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVATION_REQUESTED");
        assert_eq!(
            body["config"]["subscriptionDetail"]["device"]["networkAccessIdentifier"],
            "user000@nai"
        );

        // ipv6 ending …012 → ACTIVE, echoed as ipv6Address.
        let (status, _, body) =
            create_ok(&body_with_device(r#"{"ipv6Address":"2001:db8::012"}"#)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVE");
        assert_eq!(
            body["config"]["subscriptionDetail"]["device"]["ipv6Address"],
            "2001:db8::012"
        );
    }

    #[tokio::test]
    async fn radius_below_minimum_is_out_of_range() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},"area":{{"areaType":"CIRCLE","center":{{"latitude":51.5,"longitude":-0.12}},"radius":500}}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn radius_above_maximum_is_out_of_range() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},"area":{{"areaType":"CIRCLE","center":{{"latitude":51.5,"longitude":-0.12}},"radius":500000}}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn center_out_of_range_is_out_of_range() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},"area":{{"areaType":"CIRCLE","center":{{"latitude":123.0,"longitude":-0.12}},"radius":5000}}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn non_circle_area_type_is_invalid_argument() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},"area":{{"areaType":"POLYGON"}}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unsupported_protocol_is_invalid_argument() {
        let body = format!(
            r#"{{"protocol":"MQTT5","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},{AREA}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_event_type_is_invalid_argument() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["org.camaraproject.geofencing-subscriptions.v0.area-teleported"],"config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},{AREA}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_types_is_invalid_argument() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","config":{{"subscriptionDetail":{{"device":{{"phoneNumber":"+123456789012"}},{AREA}}}}}}}"#
        );
        let (status, _, body) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_invalid_argument() {
        let (status, _, body) = create_ok(&body_with_device("{}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject() {
        // Subject is an E.164 number with an even tail → ACTIVE, no device echo.
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{{AREA}}}}}}}"#
        );
        let (status, _, resp) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp["status"], "ACTIVE");
        assert!(resp["config"]["subscriptionDetail"].get("device").is_none());
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789503").await;
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{{AREA}}}}}}}"#
        );
        let (status, _, resp) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn non_numeric_subject_without_device_is_activation_requested() {
        // Default synthetic subject "geo-client" has no digits → ACTIVATION_REQUESTED.
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://example.com/cb","types":["{TYPE_ENTERED}"],"config":{{"subscriptionDetail":{{{AREA}}}}}}}"#
        );
        let (status, _, resp) = create_ok(&body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp["status"], "ACTIVATION_REQUESTED");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_subscriptions(
            None,
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, _) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            Some("corr-geo"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-geo")
        );

        let (status, headers, _) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"0123"}"#),
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- GET /subscriptions (list) -----------------------------------------

    #[tokio::test]
    async fn list_includes_a_created_subscription() {
        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        let (status, list) = list_subscriptions_req(Some(&token)).await;
        assert_eq!(status, StatusCode::OK);
        let items = list.as_array().expect("list is a JSON array");
        // The store is process-global, so other tests may add entries; assert the
        // created subscription is present rather than an exact length.
        let found = items
            .iter()
            .find(|s| s.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .expect("the created subscription appears in the list");
        assert_eq!(*found, created);
    }

    #[tokio::test]
    async fn list_requires_the_read_scope() {
        let token = mint_token("some:other-scope").await;
        let (status, body) = list_subscriptions_req(Some(&token)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn list_without_a_token_is_unauthenticated() {
        let (status, body) = list_subscriptions_req(None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- DELETE /subscriptions/{id} ----------------------------------------

    #[tokio::test]
    async fn delete_evicts_a_subscription_then_get_is_404() {
        let token =
            mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE} {DELETE_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // First delete evicts it → 204 No Content, no body.
        let (status, _, _) = delete_subscription_req(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // The subscription is gone: a read-back is 404.
        let (status, body) = get_subscription(&token, &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_of_unknown_id_is_not_found() {
        let token = mint_token(DELETE_SCOPE).await;
        let (status, _, body) =
            delete_subscription_req(Some(&token), "no-such-id", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_requires_the_delete_scope() {
        let token = mint_token(READ_SCOPE).await;
        let (status, _, body) = delete_subscription_req(Some(&token), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn delete_without_a_token_is_unauthenticated() {
        let (status, _, body) = delete_subscription_req(None, "any-id", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_delete() {
        let token = mint_token(&format!("{CREATE_SCOPE} {DELETE_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        let (status, headers, _) =
            delete_subscription_req(Some(&token), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del")
        );

        // A 404 delete also echoes the correlator.
        let (status, headers, _) =
            delete_subscription_req(Some(&token), "no-such-id", Some("corr-del-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-404")
        );
    }

    // --- initialEvent CloudEvent delivery ----------------------------------

    /// Build a create body with an http `sink`, both event types, and
    /// `initialEvent: true`, keyed off the given phone number.
    fn body_with_initial_event(sink: &str, phone: &str) -> String {
        json!({
            "protocol": "HTTP",
            "sink": sink,
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": phone },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "initialEvent": true,
            },
        })
        .to_string()
    }

    #[tokio::test]
    async fn initial_event_even_tail_delivers_area_entered_to_the_sink() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        // A loopback receiver stands in for the consumer's `sink`.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-initial");

        // …012 is an even ACTIVE tail → the device is currently inside → area-entered.
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_subscriptions(Some(&token), &body_with_initial_event(&sink, "+123456789012"), None)
                .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "ACTIVE");
        let id = created["id"].as_str().unwrap().to_string();

        // Receive the fire-and-forget notification the handler spawned.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /geo-initial HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENTERED);
        assert_eq!(event["specversion"], "1.0");
        assert!(event["id"].is_string() && event["time"].is_string());
        assert_eq!(event["data"]["subscriptionId"], json!(id));
        assert_eq!(event["data"]["device"]["phoneNumber"], "+123456789012");
        assert_eq!(event["data"]["area"]["radius"].as_f64(), Some(5000.0));
    }

    #[tokio::test]
    async fn initial_event_odd_tail_delivers_area_left_to_the_sink() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-initial-left");

        // …013 is an odd ACTIVE tail → the device is currently outside → area-left.
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_subscriptions(Some(&token), &body_with_initial_event(&sink, "+123456789013"), None)
                .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "ACTIVE");
        let id = created["id"].as_str().unwrap().to_string();

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (_, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_LEFT);
        assert_eq!(event["data"]["subscriptionId"], json!(id));
    }

    #[tokio::test]
    async fn initial_event_applies_the_accesstoken_sink_credential() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-auth");

        // A create body with an ACCESSTOKEN sinkCredential and initialEvent: true.
        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "ACCESSTOKEN",
                "accessToken": "sink-secret-token",
                "accessTokenType": "bearer",
            },
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789012" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "initialEvent": true,
            },
        })
        .to_string();

        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "ACTIVE");
        // The secret is never echoed back.
        assert!(created.get("sinkCredential").is_none());

        // The fire-and-forget callback carries the RFC 6750 bearer header.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Bearer sink-secret-token\r\n"),
            "authorization header present: {head}"
        );
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENTERED);
    }

    #[tokio::test]
    async fn initial_event_applies_the_plain_sink_credential_as_basic() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-plain");

        // A create body with a PLAIN sinkCredential (identifier + secret) and
        // initialEvent: true. base64("aladdin:opensesame") == "YWxhZGRpbjpvcGVuc2VzYW1l".
        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "PLAIN",
                "identifier": "aladdin",
                "secret": "opensesame",
            },
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789012" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "initialEvent": true,
            },
        })
        .to_string();

        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "ACTIVE");
        // The secret is never echoed back.
        assert!(created.get("sinkCredential").is_none());

        // The fire-and-forget callback carries the RFC 7617 HTTP Basic header.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Basic YWxhZGRpbjpvcGVuc2VzYW1l\r\n"),
            "basic authorization header present: {head}"
        );
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENTERED);
    }

    #[tokio::test]
    async fn initial_event_without_a_credential_is_unauthenticated() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-noauth");

        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, _) =
            post_subscriptions(Some(&token), &body_with_initial_event(&sink, "+123456789012"), None)
                .await;
        assert_eq!(status, StatusCode::CREATED);

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, _) = raw.split_once("\r\n\r\n").expect("headers then body");
        // No sinkCredential → callback sent unauthenticated.
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
    }

    // --- movement-triggered CloudEvent delivery ----------------------------

    /// Build a create body with an http `sink`, both event types, and **no**
    /// `initialEvent` (so the only callback is the movement event), keyed off the
    /// given phone number.
    fn body_movement(sink: &str, phone: &str) -> String {
        json!({
            "protocol": "HTTP",
            "sink": sink,
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": phone },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
            },
        })
        .to_string()
    }

    #[tokio::test]
    async fn movement_001_tail_delivers_area_entered_to_the_sink() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-move-in");

        // …001 is an ACTIVE movement tail → the device enters → area-entered.
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_subscriptions(Some(&token), &body_movement(&sink, "+123456789001"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "ACTIVE");
        let id = created["id"].as_str().unwrap().to_string();

        // The movement CloudEvent arrives after the (short) simulated-crossing grace.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /geo-move-in HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENTERED);
        assert_eq!(event["specversion"], "1.0");
        assert!(event["id"].is_string() && event["time"].is_string());
        assert_eq!(event["data"]["subscriptionId"], json!(id));
        assert_eq!(event["data"]["device"]["phoneNumber"], "+123456789001");
        assert_eq!(event["data"]["area"]["radius"].as_f64(), Some(5000.0));
    }

    #[tokio::test]
    async fn movement_002_tail_delivers_area_left_to_the_sink() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-move-out");

        // …002 is an ACTIVE movement tail → the device leaves → area-left.
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_subscriptions(Some(&token), &body_movement(&sink, "+123456789002"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["status"], "ACTIVE");
        let id = created["id"].as_str().unwrap().to_string();

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (_, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_LEFT);
        assert_eq!(event["data"]["subscriptionId"], json!(id));
    }

    #[tokio::test]
    async fn movement_applies_the_accesstoken_sink_credential() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-move-auth");

        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "ACCESSTOKEN",
                "accessToken": "move-secret-token",
                "accessTokenType": "bearer",
            },
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789001" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
            },
        })
        .to_string();

        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed back.
        assert!(created.get("sinkCredential").is_none());

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Bearer move-secret-token\r\n"),
            "authorization header present: {head}"
        );
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENTERED);
    }

    // --- subscriptionExpireTime → subscription-ended -----------------------

    #[test]
    fn parse_rfc3339_utc_is_the_inverse_of_rfc3339_utc() {
        for &epoch in &[0_i64, 1_000_000_000, 1_722_695_228] {
            assert_eq!(parse_rfc3339_utc(&rfc3339_utc(epoch)), Some(epoch));
        }
        // Only the `Z` (UTC) form is accepted (documented cut).
        assert_eq!(parse_rfc3339_utc("2024-01-01T00:00:00+01:00"), None);
        assert_eq!(parse_rfc3339_utc("2024-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_utc("not-a-date"), None);
    }

    const TYPE_ENDED: &str = "org.camaraproject.geofencing-subscriptions.v0.subscription-ended";

    /// Build a create body with an http `sink`, one event type, and a
    /// `subscriptionExpireTime` already in the past (so the timer fires at once).
    fn body_with_expiry(sink: &str, phone: &str, expire: &str) -> String {
        json!({
            "protocol": "HTTP",
            "sink": sink,
            "types": [TYPE_ENTERED],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": phone },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "subscriptionExpireTime": expire,
            },
        })
        .to_string()
    }

    #[tokio::test]
    async fn expiry_delivers_subscription_ended_and_evicts_the_subscription() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-expiry");

        // A past expireTime → the subscription ends immediately. No initialEvent,
        // so the only callback the sink receives is the subscription-ended event.
        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(
            Some(&token),
            &body_with_expiry(&sink, "+123456789012", "2020-01-01T00:00:00Z"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // Receive the fire-and-forget subscription-ended CloudEvent.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /geo-expiry HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENDED);
        assert_eq!(event["specversion"], "1.0");
        assert_eq!(event["data"]["subscriptionId"], json!(id));
        assert_eq!(event["data"]["terminationReason"], "SUBSCRIPTION_EXPIRED");

        // The subscription is gone (remove precedes delivery): a read-back is 404.
        let (status, body) = get_subscription(&token, &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn expiry_applies_the_accesstoken_sink_credential() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-expiry-auth");

        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "sinkCredential": {
                "credentialType": "ACCESSTOKEN",
                "accessToken": "expiry-secret-token",
                "accessTokenType": "bearer",
            },
            "types": [TYPE_ENTERED],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789012" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "subscriptionExpireTime": "2020-01-01T00:00:00Z",
            },
        })
        .to_string();

        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed back.
        assert!(created.get("sinkCredential").is_none());

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Bearer expiry-secret-token\r\n"),
            "authorization header present: {head}"
        );
        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(event["type"], TYPE_ENDED);
        assert_eq!(event["data"]["terminationReason"], "SUBSCRIPTION_EXPIRED");
    }

    #[tokio::test]
    async fn an_unparseable_expire_time_is_echoed_but_arms_no_timer() {
        // A non-`Z` offset form is echoed back (schema fidelity) but drives no
        // expiry timer, so the subscription persists and can be read back.
        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(
            Some(&token),
            &body_with_expiry("https://example.com/cb", "+123456789012", "2020-01-01T00:00:00+01:00"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(
            created["config"]["subscriptionExpireTime"],
            "2020-01-01T00:00:00+01:00"
        );

        // The subscription is still there (no timer fired).
        let (status, fetched) = get_subscription(&token, &id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["id"], json!(id));
    }

    // --- subscriptionMaxEvents enforcement ---------------------------------

    /// Accept one fire-and-forget HTTP POST on `listener` and return its
    /// `(head, parsed CloudEvent body)`. Reads to EOF (delivery sends
    /// `Connection: close`).
    async fn recv_post(listener: &tokio::net::TcpListener) -> (String, Value) {
        use tokio::io::AsyncReadExt;
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        (
            head.to_string(),
            serde_json::from_str(body).expect("body is JSON"),
        )
    }

    #[tokio::test]
    async fn max_events_below_one_is_out_of_range() {
        let body = json!({
            "protocol": "HTTP",
            "sink": "https://example.com/cb",
            "types": [TYPE_ENTERED],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789012" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "subscriptionMaxEvents": 0,
            },
        })
        .to_string();
        let (status, _, resp) = create_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn max_events_one_ends_the_subscription_after_the_initial_event() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-max1");

        // …012 (even) → initial area-entered; no movement tail; subscriptionMaxEvents=1.
        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789012" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "initialEvent": true,
                "subscriptionMaxEvents": 1,
            },
        })
        .to_string();

        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The maxEvents budget is echoed back in the stored config.
        assert_eq!(created["config"]["subscriptionMaxEvents"], json!(1));
        let id = created["id"].as_str().unwrap().to_string();

        // The single allowed domain event, then the terminal subscription-ended
        // (delivered in order over the same sink by spawn_delivery_seq).
        let (_, first) = recv_post(&listener).await;
        assert_eq!(first["type"], TYPE_ENTERED);
        assert_eq!(first["data"]["subscriptionId"], json!(id));

        let (_, ended) = recv_post(&listener).await;
        assert_eq!(ended["type"], TYPE_ENDED);
        assert_eq!(ended["data"]["subscriptionId"], json!(id));
        assert_eq!(ended["data"]["terminationReason"], "MAX_EVENTS_REACHED");

        // The subscription ended: a read-back is 404.
        let (status, body) = get_subscription(&token, &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn max_events_one_ends_the_subscription_after_a_movement_event() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-max1-move");

        // …001 → a movement area-entered (no initialEvent); subscriptionMaxEvents=1.
        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789001" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "subscriptionMaxEvents": 1,
            },
        })
        .to_string();

        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // The movement event exhausts the budget → the crossing, then subscription-ended.
        let (_, first) = recv_post(&listener).await;
        assert_eq!(first["type"], TYPE_ENTERED);
        assert_eq!(first["data"]["subscriptionId"], json!(id));

        let (_, ended) = recv_post(&listener).await;
        assert_eq!(ended["type"], TYPE_ENDED);
        assert_eq!(ended["data"]["terminationReason"], "MAX_EVENTS_REACHED");

        let (status, body) = get_subscription(&token, &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn max_events_budget_spans_initial_and_movement_events() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/geo-max2");

        // …001 (odd) with initialEvent=true fires TWO domain events: the initial
        // area-left (current state), then a movement area-entered (the …001 crossing).
        // subscriptionMaxEvents=2 admits both, and the second ends the subscription.
        let body = json!({
            "protocol": "HTTP",
            "sink": sink,
            "types": [TYPE_ENTERED, TYPE_LEFT],
            "config": {
                "subscriptionDetail": {
                    "device": { "phoneNumber": "+123456789001" },
                    "area": {
                        "areaType": "CIRCLE",
                        "center": { "latitude": 51.5, "longitude": -0.12 },
                        "radius": 5000,
                    },
                },
                "initialEvent": true,
                "subscriptionMaxEvents": 2,
            },
        })
        .to_string();

        let token = mint_token(&format!("{CREATE_SCOPE} {READ_SCOPE}")).await;
        let (status, _, created) = post_subscriptions(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // 1) initial area-left (immediate); 2) movement area-entered (after the grace);
        // 3) the terminal subscription-ended, in order behind the movement event.
        let (_, initial) = recv_post(&listener).await;
        assert_eq!(initial["type"], TYPE_LEFT);

        let (_, movement) = recv_post(&listener).await;
        assert_eq!(movement["type"], TYPE_ENTERED);

        let (_, ended) = recv_post(&listener).await;
        assert_eq!(ended["type"], TYPE_ENDED);
        assert_eq!(ended["data"]["terminationReason"], "MAX_EVENTS_REACHED");

        // The subscription ended after its second (final) event.
        let (status, _) = get_subscription(&token, &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
