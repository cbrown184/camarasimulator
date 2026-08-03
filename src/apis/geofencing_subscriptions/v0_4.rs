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

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use super::store;
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
    // Accepted for schema fidelity; carries a secret, so it is never echoed and
    // (until notification delivery lands) not applied — a documented cut.
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
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

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
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
}
