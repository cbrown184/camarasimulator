//! In-Home Device Management **v1** (CAMARA InHomeDeviceManagement 1.0.0, sandbox).
//!
//! Endpoints:
//! - `GET /in-home-device-management/v1/devices` — list the devices attached to
//!   the household fixed-line network named by the required `ssid` query
//!   parameter, optionally narrowed by `connectionStatus` (operationId
//!   `listDevices`, scope `inhome.device.read`).
//! - `GET /in-home-device-management/v1/devices/{deviceId}` — read a single
//!   device of the household named by the required `ssid` query parameter, by its
//!   `deviceId` (operationId `getDevice`, scope `inhome.device.read`). Stateless:
//!   the roster is regenerated from the `ssid` and the matching device returned,
//!   else `404 NOT_FOUND`.
//! - `GET /in-home-device-management/v1/devices/{deviceId}/network-health` — read
//!   the network-health telemetry of a single household device (operationId
//!   `getDeviceNetworkHealth`, scope `inhome.device.read`). Stateless, mirroring
//!   `getDevice`: the roster is regenerated from the `ssid`, the matching device
//!   looked up, and a deterministic [`DeviceNetworkHealth`] derived from it, else
//!   `404 NOT_FOUND`.
//!
//! ## What it does
//!
//! There is no real home network behind CamaraSim, so the household roster is
//! **derived deterministically from the `ssid`** (docs/DESIGN.md §7) — the input
//! is the control plane. Every household always carries one infrastructure
//! device (the home gateway / modem); the number and composition of the
//! *client* devices behind it are fixed by the `ssid`'s trailing three digits,
//! so a caller can reproduce any inventory from the id alone.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `inhome.device.read` scope. It
//! is a two-legged, service-to-service query — the `ssid` (not a subscriber
//! identity) names the target — so no three-legged token is required.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer:
//!
//! 1. **Reserved error suffix (`ssid`).** The `ssid` is the identifier: if its
//!    trailing three digits name a reserved CAMARA status, the endpoint answers
//!    with that canonical CAMARA error (shared convention, [`crate::scenarios`]):
//!    `…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`.
//!    An `ssid` need not contain digits — one with fewer than three is simply a
//!    happy-path input.
//! 2. **Roster shape (`ssid` digits) + the `connectionStatus` filter.** Otherwise
//!    the `ssid`'s trailing three digits `d` (`000`–`999`) fix the number of
//!    client devices (`d % 6`, so `…000`/no-digits → gateway only) and their
//!    types/connection states, and the optional `connectionStatus` query
//!    parameter narrows the returned list to devices in that state (a genuine
//!    second plane; `total` reflects the filtered count).
//!
//! Examples: `ssid=Home` → gateway only; `ssid=Home-005` → gateway + 5 client
//! devices; `ssid=Home-404` → `404 NOT_FOUND`; `ssid=Home-005&connectionStatus=blocked`
//! → only the blocked devices of that household.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, RawQuery};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `listDevices` requires (CAMARA InHomeDeviceManagement 1.0.0).
const READ_SCOPE: &str = "inhome.device.read";

/// Routes for In-Home Device Management v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/in-home-device-management/v1/devices",
            get(list_devices),
        )
        .route(
            "/in-home-device-management/v1/devices/:device_id",
            get(get_device),
        )
        .route(
            "/in-home-device-management/v1/devices/:device_id/network-health",
            get(get_device_network_health),
        )
}

/// The client device types CamaraSim rotates through for a household's attached
/// devices (CAMARA `deviceType` enum, minus `other` which is reserved for the
/// infrastructure gateway). Indexed by the `ssid` digits so the mix is a control
/// plane.
const CLIENT_TYPES: [&str; 6] = ["laptop", "mobile", "tv", "printer", "iot", "desktop"];

/// The CAMARA `connectionStatus` enum, in the order CamaraSim cycles them for a
/// household's client devices. `connected` appears twice so it is the common
/// state, and every value is reachable so the `connectionStatus` filter is a
/// meaningful control plane.
const STATUS_CYCLE: [&str; 5] = ["connected", "connected", "paused", "blocked", "disconnected"];

/// `GET /in-home-device-management/v1/devices` — list a household's attached
/// devices (`listDevices`).
async fn list_devices(claims: Claims, headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Parse + validate the query string (ssid required, connectionStatus optional).
    let params = match parse_params(query.as_deref(), &correlator) {
        Ok(p) => p,
        Err(response) => return response,
    };

    // Error plane: a reserved trailing-three-digit suffix on the `ssid` selects a
    // canonical CAMARA error (shared convention).
    if let Some(err) = scenarios::reserved_error(&params.ssid) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Build the household roster deterministically from the ssid, then apply the
    // optional connectionStatus filter.
    let devices: Vec<Value> = household(&params.ssid)
        .into_iter()
        .filter(|d| {
            params
                .connection_status
                .as_deref()
                .is_none_or(|want| d["connectionStatus"] == want)
        })
        .collect();

    let body = json!({ "total": devices.len(), "devices": devices });
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// `GET /in-home-device-management/v1/devices/{deviceId}` — read a single device
/// of a household by its id (`getDevice`).
///
/// A household is fully deterministic from its `ssid` (see [`household`]), so —
/// like the sibling Network Access Devices `getNetworkAccessDevice` — this
/// endpoint needs no store: it regenerates the `ssid`'s roster and returns the
/// device whose `deviceId` matches the path parameter. The `ssid` query parameter
/// is **required** (it names the household, upstream CAMARA `getDevice`).
///
/// Two control planes (docs/DESIGN.md §7):
///
/// 1. **Reserved error suffix (`ssid`).** A household-level plane, mirroring
///    `listDevices`: if the `ssid`'s trailing three digits name a reserved CAMARA
///    status, the endpoint answers that canonical error regardless of the id.
/// 2. **The `deviceId` vs the household's roster.** A `deviceId` that belongs to
///    the `ssid`'s deterministic roster → `200` with that `Device`; any other id
///    (unknown, from another household, or malformed) → `404 NOT_FOUND`. The id is
///    opaque to the caller (`dev-…`), so it is not itself a scenario plane.
async fn get_device(
    claims: Claims,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The `ssid` query parameter is required (it names the household).
    let ssid = match parse_ssid(query.as_deref(), &correlator) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Error plane: a reserved trailing-three-digit suffix on the `ssid` selects a
    // canonical CAMARA error (household-level, checked first — mirrors listDevices).
    if let Some(err) = scenarios::reserved_error(&ssid) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Regenerate the household roster and look the id up. An id that is not one of
    // this household's devices — unknown, another household's, or malformed — is a
    // `404 NOT_FOUND` (there is no store to distinguish them).
    match household(&ssid)
        .into_iter()
        .find(|d| d["deviceId"] == json!(device_id))
    {
        Some(device) => {
            with_correlator((StatusCode::OK, Json(device)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No device found for the provided id on this household.")
                .into_response(),
            &correlator,
        ),
    }
}

/// `GET /in-home-device-management/v1/devices/{deviceId}/network-health` — read a
/// household device's network-health telemetry (`getDeviceNetworkHealth`).
///
/// Like [`get_device`], CamaraSim implements this **statelessly**: a household's
/// roster is fully deterministic from its `ssid` ([`household`]), so the endpoint
/// regenerates the roster, finds the device whose `deviceId` matches the path
/// parameter, and derives a deterministic [`DeviceNetworkHealth`] from it. The
/// `ssid` query parameter is **required** (it names the household).
///
/// Two control planes (docs/DESIGN.md §7), identical to `getDevice`:
///
/// 1. **Reserved error suffix (`ssid`).** A household-level plane, checked first:
///    if the `ssid`'s trailing three digits name a reserved CAMARA status, the
///    endpoint answers that canonical error regardless of the id.
/// 2. **The `deviceId` vs the household's roster.** A member id → `200` with that
///    device's `DeviceNetworkHealth`; any other id (unknown, another household's,
///    or malformed) → `404 NOT_FOUND`.
///
/// The telemetry itself is derived deterministically from the matched device (its
/// opaque `deviceId`, `interfaceType`, and `connectionStatus`), so the health
/// figures are stable and reproducible from the input — a wired gateway reports
/// an Ethernet link (no radio), a Wi-Fi client reports a band, RSSI and Wi-Fi
/// generation, and a blocked/disconnected or weakly-signalled device reports
/// `red` congestion.
async fn get_device_network_health(
    claims: Claims,
    headers: HeaderMap,
    Path(device_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The `ssid` query parameter is required (it names the household).
    let ssid = match parse_ssid(query.as_deref(), &correlator) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Error plane: a reserved trailing-three-digit suffix on the `ssid` selects a
    // canonical CAMARA error (household-level, checked first — mirrors getDevice).
    if let Some(err) = scenarios::reserved_error(&ssid) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Regenerate the household roster, find the addressed device, and derive its
    // network-health telemetry. An id that is not one of this household's devices
    // is a `404 NOT_FOUND` (there is no store to distinguish the reasons).
    match household(&ssid)
        .into_iter()
        .find(|d| d["deviceId"] == json!(device_id))
    {
        Some(device) => with_correlator(
            (StatusCode::OK, Json(network_health(&device))).into_response(),
            &correlator,
        ),
        None => with_correlator(
            CamaraError::not_found("No device found for the provided id on this household.")
                .into_response(),
            &correlator,
        ),
    }
}

/// Parse and validate the required `ssid` query parameter for a per-device
/// operation. A missing or empty `ssid` → 400 `INVALID_ARGUMENT`. Other query
/// parameters are ignored (Commonalities).
fn parse_ssid(query: Option<&str>, correlator: &Option<HeaderValue>) -> Result<String, Response> {
    #[derive(serde::Deserialize, Default)]
    struct Raw {
        ssid: Option<String>,
    }

    let raw: Raw = serde_urlencoded::from_str(query.unwrap_or("")).map_err(|_| {
        invalid_argument(
            "the query string is not valid application/x-www-form-urlencoded",
            correlator,
        )
    })?;

    match raw.ssid {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(invalid_argument(
            "`ssid` is required and must be a non-empty string.",
            correlator,
        )),
    }
}

/// The validated `listDevices` query controls.
struct Params {
    /// The household network's SSID (required, non-empty). The control-plane id.
    ssid: String,
    /// Optional `connectionStatus` filter (one of the CAMARA enum values).
    connection_status: Option<String>,
}

/// Parse and validate the `listDevices` query string. A missing or empty `ssid`
/// → 400 `INVALID_ARGUMENT`; an unknown `connectionStatus` value → 400
/// `INVALID_ARGUMENT`. Unknown query params are ignored (Commonalities).
fn parse_params(query: Option<&str>, correlator: &Option<HeaderValue>) -> Result<Params, Response> {
    #[derive(serde::Deserialize, Default)]
    struct Raw {
        ssid: Option<String>,
        #[serde(rename = "connectionStatus")]
        connection_status: Option<String>,
    }

    let raw: Raw = serde_urlencoded::from_str(query.unwrap_or("")).map_err(|_| {
        invalid_argument(
            "the query string is not valid application/x-www-form-urlencoded",
            correlator,
        )
    })?;

    let ssid = match raw.ssid {
        Some(s) if !s.is_empty() => s,
        _ => {
            return Err(invalid_argument(
                "`ssid` is required and must be a non-empty string.",
                correlator,
            ))
        }
    };

    if let Some(status) = &raw.connection_status {
        // The CAMARA connectionStatus enum.
        if !["connected", "disconnected", "blocked", "paused"].contains(&status.as_str()) {
            return Err(invalid_argument(
                "`connectionStatus` must be one of connected, disconnected, blocked, paused.",
                correlator,
            ));
        }
    }

    Ok(Params {
        ssid,
        connection_status: raw.connection_status,
    })
}

/// The household's attached devices, derived deterministically from the `ssid`.
///
/// Every household carries the infrastructure gateway (a `modem`) plus `d % 6`
/// client devices, where `d` is the `ssid`'s trailing three digits (`0` when it
/// has fewer than three). So `…000`/no-digits → the gateway alone, `…005` →
/// gateway + 5 client devices; each client's type and connection state are fixed
/// by the same digits so the whole roster is reproducible from the id.
fn household(ssid: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(ssid).unwrap_or(0) as u64;

    // The always-present infrastructure gateway (the home modem/router).
    let mut devices = vec![json!({
        "deviceId": device_id(ssid, 0),
        "ssid": ssid,
        "deviceName": "Home Gateway",
        "deviceType": "other",
        "macAddress": mac_address(ssid, 0),
        "ipAddress": "192.168.1.1",
        "connectionStatus": "connected",
        "blocked": false,
        "paused": false,
        "infraDevice": "modem",
        "interfaceType": "ethernet",
    })];

    let client_count = (d % 6) as usize;
    for i in 0..client_count {
        let n = i + 1; // 1-based index in the household
        let device_type = CLIENT_TYPES[(d as usize + i) % CLIENT_TYPES.len()];
        let status = STATUS_CYCLE[(d as usize + 3 * i) % STATUS_CYCLE.len()];
        let interface = if device_type == "desktop" {
            "ethernet"
        } else {
            "wifi"
        };
        devices.push(json!({
            "deviceId": device_id(ssid, n),
            "ssid": ssid,
            "deviceName": format!("{}-{n}", title_case(device_type)),
            "deviceType": device_type,
            "macAddress": mac_address(ssid, n),
            "ipAddress": format!("192.168.1.{}", 20 + i),
            "connectionStatus": status,
            "blocked": status == "blocked",
            "paused": status == "paused",
            "interfaceType": interface,
        }));
    }

    devices
}

/// A stable, opaque device id derived from the `ssid` and the device's household
/// index (FNV-1a, no dependency). Deterministic so a caller can reproduce it.
fn device_id(ssid: &str, index: usize) -> String {
    format!("dev-{:016x}", fnv1a(ssid, index, 0x01))
}

/// A stable, locally-administered MAC address derived from the `ssid` and index
/// (FNV-1a). The first octet is forced to `02` (the locally-administered,
/// unicast bit pattern) so it never collides with a real OUI.
fn mac_address(ssid: &str, index: usize) -> String {
    let h = fnv1a(ssid, index, 0x02).to_be_bytes();
    format!(
        "02:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        h[1], h[2], h[3], h[4], h[5]
    )
}

/// A 64-bit FNV-1a hash of `ssid`, mixed with a device index and a domain
/// separator so ids and MACs derived from the same device don't coincide. Pure,
/// no dependency.
fn fnv1a(ssid: &str, index: usize, domain: u8) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mix = |mut hash: u64, byte: u8| {
        hash ^= byte as u64;
        hash.wrapping_mul(0x0000_0100_0000_01b3)
    };
    hash = mix(hash, domain);
    for byte in ssid.bytes() {
        hash = mix(hash, byte);
    }
    for byte in (index as u64).to_be_bytes() {
        hash = mix(hash, byte);
    }
    hash
}

/// Capitalise the first ASCII letter of a device-type slug for a display name
/// (`laptop` → `Laptop`). Small and self-contained.
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Derive a deterministic `DeviceNetworkHealth` for a household device
/// (`getDeviceNetworkHealth`).
///
/// The telemetry is a pure function of the matched device: its opaque `deviceId`
/// (hashed for the variable figures), its `interfaceType` (a wired gateway
/// reports an Ethernet link with no radio; a Wi-Fi client reports a band, RSSI
/// and a Wi-Fi generation), and its `connectionStatus` (a blocked or
/// disconnected device — or a weak Wi-Fi signal — reports `red` congestion). Only
/// `networkCongestion` is required by the CAMARA schema; the radio fields are
/// present for Wi-Fi devices and omitted for wired ones. `measuredAt` is the
/// current instant (the simulator always treats telemetry as fresh).
fn network_health(device: &Value) -> Value {
    let device_id = device["deviceId"].as_str().unwrap_or_default();
    let ssid = device["ssid"].as_str().unwrap_or_default();
    let interface = device["interfaceType"].as_str().unwrap_or("wifi");
    let status = device["connectionStatus"].as_str().unwrap_or("connected");
    // A device that is off the network reports congestion regardless of its link.
    let down = matches!(status, "disconnected" | "blocked");
    let h = fnv1a(device_id, 0, 0x03);

    let mut health = json!({
        "deviceId": device_id,
        "ssid": ssid,
        "interfaceType": interface,
        "measuredAt": rfc3339_utc(now_unix_secs()),
    });
    let obj = health.as_object_mut().expect("network_health is a JSON object");

    // Echo the infrastructure class on infra devices (the household gateway).
    if let Some(infra) = device.get("infraDevice") {
        obj.insert("infraDevice".into(), infra.clone());
    }

    if interface == "ethernet" {
        // A wired link: no radio metrics; a steady high physical rate.
        obj.insert("maxPhyRateMbps".into(), json!(1000.0));
        obj.insert(
            "lastNetworkSpeedMbps".into(),
            json!((300 + h % 700) as f64),
        );
        // Wired links seldom congest; only a down device (or a hashed minority) is red.
        let congested = down || h % 8 == 0;
        obj.insert(
            "networkCongestion".into(),
            json!(if congested { "red" } else { "green" }),
        );
    } else {
        // A Wi-Fi link: band, signal strength and Wi-Fi generation.
        let band = ["2.4", "5", "6"][(h % 3) as usize];
        let rssi = -30 - (h % 60) as i64; // −30 dBm (strong) … −89 dBm (weak)
        let compat = ["wifi4", "wifi5", "wifi6", "wifi7"][((h >> 3) % 4) as usize];
        // Peak physical rate climbs with the band.
        let max_phy: f64 = match band {
            "2.4" => 300.0,
            "5" => 1200.0,
            _ => 2400.0,
        };
        obj.insert("radioFrequency".into(), json!(band));
        obj.insert("rssiDbm".into(), json!(rssi as f64));
        obj.insert("wifiCompatibility".into(), json!(compat));
        obj.insert("maxPhyRateMbps".into(), json!(max_phy));
        // Achieved speed is a fraction (40–89 %) of the peak, scaled by the hash.
        let frac = 40 + h % 50;
        obj.insert(
            "lastNetworkSpeedMbps".into(),
            json!((max_phy * frac as f64 / 100.0).round()),
        );
        // A down device, or a weak signal (< −75 dBm), reports congestion.
        let congested = down || rssi < -75;
        obj.insert(
            "networkCongestion".into(),
            json!(if congested { "red" } else { "green" }),
        );
    }

    health
}

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 / ISO 8601 instant with
/// a `Z` offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs
/// no date/time dependency (mirrors `device_data_volume::vwip`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for any date).
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

    const HOST: &str = "inhome.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn no_digit_ssid_is_gateway_only() {
        let devices = household("HomeWiFi");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0]["infraDevice"], "modem");
        assert_eq!(devices[0]["deviceType"], "other");
    }

    #[test]
    fn ssid_digits_fix_the_client_count() {
        // trailing digits d → 1 gateway + (d % 6) client devices.
        for (ssid, expected_clients) in [
            ("net-000", 0),
            ("net-001", 1),
            ("net-005", 5),
            ("net-006", 0), // 6 % 6 == 0
            ("net-011", 5), // 11 % 6 == 5
        ] {
            let devices = household(ssid);
            assert_eq!(
                devices.len(),
                expected_clients + 1,
                "ssid {ssid}: {expected_clients} clients + gateway"
            );
        }
    }

    #[test]
    fn roster_is_deterministic_and_wellformed() {
        let a = household("MyHouse-042");
        let b = household("MyHouse-042");
        assert_eq!(a, b, "same ssid → identical roster");
        for d in &a {
            // Every device carries the required CAMARA fields with valid enums.
            assert!(d["deviceId"].as_str().unwrap().starts_with("dev-"));
            assert_eq!(d["ssid"], "MyHouse-042");
            assert!(CLIENT_TYPES.contains(&d["deviceType"].as_str().unwrap())
                || d["deviceType"] == "other");
            assert!(["connected", "disconnected", "blocked", "paused"]
                .contains(&d["connectionStatus"].as_str().unwrap()));
            assert!(d["macAddress"].as_str().unwrap().starts_with("02:"));
            // blocked/paused mirror the connectionStatus.
            assert_eq!(d["blocked"], d["connectionStatus"] == "blocked");
            assert_eq!(d["paused"], d["connectionStatus"] == "paused");
        }
    }

    #[test]
    fn device_ids_are_unique_within_a_household() {
        let devices = household("net-005");
        let mut ids: Vec<&str> = devices.iter().map(|d| d["deviceId"].as_str().unwrap()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "device ids are unique");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=inhome-client&scope={scope}");
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

    async fn list(
        token: Option<&str>,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/in-home-device-management/v1/devices?{query}"))
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

    async fn list_ok(query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        list(Some(&token), query, None).await
    }

    #[tokio::test]
    async fn lists_the_household_roster() {
        let (status, _, body) = list_ok("ssid=Home-005").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 6); // gateway + 5 clients
        let devices = body["devices"].as_array().unwrap();
        assert_eq!(devices.len(), 6);
        // The gateway is first and is the infrastructure modem.
        assert_eq!(devices[0]["infraDevice"], "modem");
    }

    #[tokio::test]
    async fn gateway_only_household() {
        let (status, _, body) = list_ok("ssid=QuietHouse").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["total"], 1);
        assert_eq!(body["devices"][0]["deviceType"], "other");
    }

    #[tokio::test]
    async fn connection_status_filter_narrows_the_list() {
        // The full roster, then filtered — the filtered count must not exceed it,
        // and every returned device must match the requested status.
        let (_, _, full) = list_ok("ssid=Home-013").await;
        let full_total = full["total"].as_u64().unwrap();

        let (status, _, body) = list_ok("ssid=Home-013&connectionStatus=connected").await;
        assert_eq!(status, StatusCode::OK);
        let filtered_total = body["total"].as_u64().unwrap();
        assert!(filtered_total <= full_total);
        assert!(filtered_total >= 1, "the gateway is always connected");
        for d in body["devices"].as_array().unwrap() {
            assert_eq!(d["connectionStatus"], "connected");
        }
    }

    #[tokio::test]
    async fn total_matches_the_returned_array_length() {
        let (_, _, body) = list_ok("ssid=Home-004&connectionStatus=blocked").await;
        assert_eq!(
            body["total"].as_u64().unwrap() as usize,
            body["devices"].as_array().unwrap().len()
        );
    }

    #[tokio::test]
    async fn reserved_ssid_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = list_ok("ssid=Home-404").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = list_ok("ssid=Home-429").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn missing_ssid_is_rejected() {
        let (status, _, body) = list_ok("connectionStatus=connected").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_ssid_is_rejected() {
        let (status, _, body) = list_ok("ssid=").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_connection_status_is_rejected() {
        let (status, _, body) = list_ok("ssid=Home-005&connectionStatus=frozen").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = list(Some(&token), "ssid=Home-005", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = list(None, "ssid=Home-005", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) = list(Some(&token), "ssid=Home-005", Some("corr-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        let (status, headers, _) = list(Some(&token), "ssid=Home-404", Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- getDevice: GET /devices/{deviceId} --------------------------------

    async fn get_one(
        token: Option<&str>,
        device_id: &str,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/in-home-device-management/v1/devices/{device_id}?{query}"
            ))
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

    async fn get_one_ok(device_id: &str, query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        get_one(Some(&token), device_id, query, None).await
    }

    #[tokio::test]
    async fn get_device_returns_the_matching_device() {
        // Pick a real device out of a household roster, then read it back by id.
        let roster = household("Home-005");
        let target = &roster[2]; // a client device (the gateway is index 0)
        let id = target["deviceId"].as_str().unwrap();

        let (status, _, body) = get_one_ok(id, "ssid=Home-005").await;
        assert_eq!(status, StatusCode::OK);
        // A bare Device object (not the DeviceList wrapper), matching the roster.
        assert_eq!(&body, target);
        assert_eq!(body["ssid"], "Home-005");
    }

    #[tokio::test]
    async fn get_device_gateway_is_readable() {
        let roster = household("QuietHouse");
        let id = roster[0]["deviceId"].as_str().unwrap();
        let (status, _, body) = get_one_ok(id, "ssid=QuietHouse").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["infraDevice"], "modem");
    }

    #[tokio::test]
    async fn get_device_unknown_id_is_404() {
        let (status, _, body) = get_one_ok("dev-000000000000dead", "ssid=Home-005").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_device_from_another_household_is_404() {
        // A valid id for Home-005, queried against a different household → 404.
        let id = household("Home-005")[1]["deviceId"]
            .as_str()
            .unwrap()
            .to_string();
        let (status, _, body) = get_one_ok(&id, "ssid=OtherHouse-007").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_device_missing_ssid_is_400() {
        let (status, _, body) = get_one_ok("dev-abc", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn get_device_reserved_ssid_suffix_selects_a_canonical_camara_error() {
        // The household-level error plane fires before the id lookup: …429 → 429,
        // a status the id-not-found path can never produce.
        let (status, _, body) = get_one_ok("dev-anything", "ssid=Home-429").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn get_device_scope_and_auth_are_enforced() {
        let id = household("Home-005")[0]["deviceId"]
            .as_str()
            .unwrap()
            .to_string();

        // No token → 401.
        let (status, _, body) = get_one(None, &id, "ssid=Home-005", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");

        // Token without the scope → 403.
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_one(Some(&token), &id, "ssid=Home-005", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn get_device_echoes_the_correlator() {
        let id = household("Home-005")[0]["deviceId"]
            .as_str()
            .unwrap()
            .to_string();
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            get_one(Some(&token), &id, "ssid=Home-005", Some("corr-get")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get")
        );
    }

    // --- getDeviceNetworkHealth: GET /devices/{deviceId}/network-health -----

    #[test]
    fn network_health_of_a_wired_gateway_reports_ethernet_no_radio() {
        // The gateway is index 0, an `other`/`modem` device on Ethernet.
        let gateway = &household("Home-005")[0];
        let health = network_health(gateway);
        assert_eq!(health["interfaceType"], "ethernet");
        assert_eq!(health["infraDevice"], "modem");
        // Wired → no radio metrics.
        assert!(health.get("radioFrequency").is_none());
        assert!(health.get("rssiDbm").is_none());
        assert!(health.get("wifiCompatibility").is_none());
        // Required field present and a valid enum value.
        assert!(["green", "red"].contains(&health["networkCongestion"].as_str().unwrap()));
        assert_eq!(health["deviceId"], gateway["deviceId"]);
        assert_eq!(health["ssid"], "Home-005");
        assert!(health["measuredAt"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn network_health_of_a_wifi_client_reports_the_radio() {
        // Find a Wi-Fi client in a populated household.
        let roster = household("Home-005");
        let wifi = roster
            .iter()
            .find(|d| d["interfaceType"] == "wifi")
            .expect("Home-005 has a wifi client");
        let health = network_health(wifi);
        assert_eq!(health["interfaceType"], "wifi");
        assert!(["2.4", "5", "6"].contains(&health["radioFrequency"].as_str().unwrap()));
        assert!(health["rssiDbm"].is_number());
        assert!(["wifi4", "wifi5", "wifi6", "wifi7"]
            .contains(&health["wifiCompatibility"].as_str().unwrap()));
        assert!(health["maxPhyRateMbps"].as_f64().unwrap() > 0.0);
        assert!(health["lastNetworkSpeedMbps"].as_f64().unwrap() >= 0.0);
        assert!(["green", "red"].contains(&health["networkCongestion"].as_str().unwrap()));
    }

    #[test]
    fn network_health_is_deterministic() {
        let a = &household("MyHouse-005")[1];
        assert_eq!(network_health(a), network_health(a));
    }

    #[test]
    fn a_device_off_the_network_reports_red_congestion() {
        // Build a disconnected client and confirm the congestion verdict follows.
        let down = json!({
            "deviceId": "dev-000000000000beef",
            "ssid": "Home-005",
            "connectionStatus": "disconnected",
            "interfaceType": "wifi",
        });
        assert_eq!(network_health(&down)["networkCongestion"], "red");
    }

    async fn get_health(
        token: Option<&str>,
        device_id: &str,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/in-home-device-management/v1/devices/{device_id}/network-health?{query}"
            ))
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

    async fn get_health_ok(device_id: &str, query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        get_health(Some(&token), device_id, query, None).await
    }

    #[tokio::test]
    async fn get_network_health_returns_the_matching_device_health() {
        let roster = household("Home-005");
        let target = &roster[2]; // a client device
        let id = target["deviceId"].as_str().unwrap();

        let (status, _, body) = get_health_ok(id, "ssid=Home-005").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["deviceId"], target["deviceId"]);
        assert_eq!(body["ssid"], "Home-005");
        assert_eq!(body["interfaceType"], target["interfaceType"]);
        assert!(["green", "red"].contains(&body["networkCongestion"].as_str().unwrap()));
    }

    #[tokio::test]
    async fn get_network_health_unknown_id_is_404() {
        let (status, _, body) =
            get_health_ok("dev-000000000000dead", "ssid=Home-005").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_network_health_reserved_ssid_suffix_selects_a_canonical_camara_error() {
        // The household-level error plane fires before the id lookup (mirrors getDevice).
        let (status, _, body) = get_health_ok("dev-anything", "ssid=Home-429").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn get_network_health_missing_ssid_is_400() {
        let (status, _, body) = get_health_ok("dev-abc", "").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn get_network_health_scope_and_auth_are_enforced() {
        let id = household("Home-005")[0]["deviceId"]
            .as_str()
            .unwrap()
            .to_string();

        // No token → 401.
        let (status, _, body) = get_health(None, &id, "ssid=Home-005", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");

        // Token without the scope → 403.
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_health(Some(&token), &id, "ssid=Home-005", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn get_network_health_echoes_the_correlator() {
        let id = household("Home-005")[0]["deviceId"]
            .as_str()
            .unwrap()
            .to_string();
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            get_health(Some(&token), &id, "ssid=Home-005", Some("corr-nh")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nh")
        );
    }
}
