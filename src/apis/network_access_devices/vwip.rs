//! Network Access Devices **vwip** (CAMARA NetworkAccessManagement / Network
//! Access Devices, wip).
//!
//! Three endpoints:
//! - `GET /network-access-devices/vwip/network-access-devices` — list the
//!   operator-supplied network access devices associated with the subscriber.
//! - `GET /network-access-devices/vwip/network-access-devices/{networkAccessDeviceId}`
//!   — read one of those devices by id. The device set is deterministic from the
//!   token subject, so the read regenerates it and looks the id up — no store.
//! - `POST /network-access-devices/vwip/reboot-requests` — create a **stateful**
//!   reboot request targeting one or more of the subscriber's devices, persisted
//!   in the shared in-memory [`super::store`] (operationId `createRebootRequest`).
//!   The read/patch/delete legs of the lifecycle are later slices.
//!
//! ## What it does
//!
//! Returns the operator-managed access equipment (gateways/routers/access
//! points) associated with the subscriber the access token authenticated, as a
//! `NetworkAccessDeviceList`. Each device carries an `id` (UUID), a
//! `deviceStatus` (`connected`/`disconnected`/`unavailable`), a `name`, a
//! `description`, and an EUI-48 `hardwareAddress`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `network-access-devices:reboot`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The listing has no request body, so — like Number Verification's
//! `GET /device-phone-number` — its functional cases are driven by the token
//! **subject** (`sub`):
//!
//! - **Reserved error suffix** — if the subject's trailing three digits name a
//!   reserved CAMARA status (shared convention, [`crate::scenarios`]), the
//!   endpoint answers with that canonical CAMARA error instead of a list.
//! - **Device set** — otherwise the subject's trailing three digits `d` (or `0`
//!   when the subject has no digits) drive two facets of the returned list:
//!   the **count** (`d == 0` → one device, else `((d - 1) % 3) + 1`, i.e. 1–3)
//!   and each device's **status** (device `i` reports
//!   `[connected, disconnected, unavailable][(d + i) % 3]`). Each device's `id`
//!   and `hardwareAddress` are deterministic from the subject and index
//!   (SHA-256).
//!
//! `serviceSite` and the inherited Commonalities `Device` end-user identifier
//! fields are omitted for these operator devices (a documented, schema-valid
//! cut — only `id` is required).

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The single OAuth2 scope every Network Access Devices operation requires
/// (CAMARA NAM wip) — the listing, the by-id read, and the reboot-request
/// lifecycle all gate on `network-access-devices:reboot`.
const SCOPE: &str = "network-access-devices:reboot";

/// Collection path for the reboot-request resource (used for the create
/// `Location` header and the route mount).
const REBOOT_REQUESTS: &str = "/network-access-devices/vwip/reboot-requests";

/// The three device statuses, in the CAMARA enum order the digit plane indexes.
const STATUSES: [&str; 3] = ["connected", "disconnected", "unavailable"];

/// Routes for Network Access Devices vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/network-access-devices/vwip/network-access-devices",
            get(list_devices),
        )
        .route(
            "/network-access-devices/vwip/network-access-devices/:network_access_device_id",
            get(get_device),
        )
        .route(REBOOT_REQUESTS, post(create_reboot_request))
}

/// `GET /network-access-devices/vwip/network-access-devices`.
async fn list_devices(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the control plane (docs/DESIGN.md §7).
    let subject = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(subject) {
        return with_correlator(err.into_response(), &correlator);
    }

    let devices = device_list(subject);
    with_correlator(
        (StatusCode::OK, Json(Value::Array(devices))).into_response(),
        &correlator,
    )
}

/// `GET /network-access-devices/vwip/network-access-devices/{networkAccessDeviceId}`
/// — read one of the subscriber's Network Access Devices by id (operationId
/// `getNetworkAccessDevice`).
///
/// The subscriber's device set is fully deterministic from the token subject (see
/// [`device_list`]), so — unlike CAMARA's stateful resource read — this endpoint
/// needs no store: it regenerates the subject's set and returns the device whose
/// `id` matches the path parameter. Two control planes (docs/DESIGN.md §7):
///
/// - **Reserved error suffix (subject)** — an account-level plane, mirroring the
///   listing: if the subject's trailing three digits name a reserved CAMARA
///   status, the endpoint answers that canonical error regardless of the id.
/// - **The id vs the subject's set** — a `networkAccessDeviceId` that belongs to
///   the subject's deterministic set → `200` with that device; any other id
///   (unknown, belonging to a different subscriber, or malformed) → `404
///   NOT_FOUND`. The id is opaque to the caller (UUID-shaped), so it is not itself
///   a scenario plane.
///
/// Requires a token carrying the `network-access-devices:reboot` scope.
async fn get_device(claims: Claims, headers: HeaderMap, Path(id): Path<String>) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the account-level control plane (docs/DESIGN.md §7):
    // a reserved suffix takes the whole account into a canonical error, matching
    // the listing endpoint.
    let subject = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(subject) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Regenerate the subject's deterministic set and look the id up. An id that is
    // not one of this subscriber's devices — unknown, another subscriber's, or
    // malformed — is a `404 NOT_FOUND` (there is no store to distinguish them).
    match device_list(subject)
        .into_iter()
        .find(|device| device["id"] == json!(id))
    {
        Some(device) => {
            with_correlator((StatusCode::OK, Json(device)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No Network Access Device found for the provided id.")
                .into_response(),
            &correlator,
        ),
    }
}

/// `RebootRequestCreate` body (CAMARA NAM wip). All three fields are optional:
/// `devices` names the target devices (omit / empty → reboot *all* the
/// subscriber's devices), `message` is a free-text note, and `atTime` schedules
/// the reboot (omit for an immediate reboot).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RebootRequestCreate {
    message: Option<String>,
    #[serde(rename = "atTime")]
    at_time: Option<String>,
    devices: Option<Vec<String>>,
}

/// `POST /network-access-devices/vwip/reboot-requests` — create a reboot request
/// for one or more of the subscriber's Network Access Devices (operationId
/// `createRebootRequest`).
///
/// This is the create leg of the **stateful** reboot-request lifecycle: the
/// created `RebootRequest` is persisted in the shared in-memory [`super::store`]
/// so later slices (`getRebootRequest` / `updateRebootRequest` /
/// `deleteRebootRequest`) can address it by its minted `id`. Answers `201` with
/// the created resource and a `Location` header.
///
/// Two control planes (docs/DESIGN.md §7):
///
/// - **Reserved error suffix (subject).** As for the listing / by-id read, a
///   subject whose trailing three digits name a reserved CAMARA status answers
///   that canonical error (an account-level plane) before anything is created.
/// - **`devices` vs the subject's set.** The reboot targets are matched against
///   the subject's deterministic device set ([`device_list`]): an explicit
///   `devices` list must hold well-formed UUIDs (else `400 INVALID_ARGUMENT`),
///   each belonging to the subscriber (else `404 NOT_FOUND`); an omitted or empty
///   list reboots *all* the subscriber's devices (the schema requires `devices`,
///   so the inferred set is materialised on the created resource).
///
/// `message` (≤ 255 chars) and `atTime` (RFC 3339) are validated and echoed; a
/// malformed body / field → `400 INVALID_ARGUMENT`. Requires the
/// `network-access-devices:reboot` scope.
async fn create_reboot_request(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Account-level control plane: a reserved subject suffix answers a canonical
    // CAMARA error before any resource is created (mirrors list / read).
    let subject = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(subject) {
        return with_correlator(err.into_response(), &correlator);
    }

    // An empty body is a valid create — an immediate reboot of all the
    // subscriber's devices — so treat empty bytes as an all-defaults request.
    let req: RebootRequestCreate = if body.is_empty() {
        RebootRequestCreate {
            message: None,
            at_time: None,
            devices: None,
        }
    } else {
        match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RebootRequestCreate.",
                    &correlator,
                )
            }
        }
    };

    // `message` — schema `maxLength: 255`.
    if let Some(message) = &req.message {
        if message.chars().count() > 255 {
            return invalid_argument("`message` must be at most 255 characters.", &correlator);
        }
    }

    // `atTime` — optional RFC 3339 date-time; validate its format.
    if let Some(at_time) = &req.at_time {
        if !is_rfc3339(at_time) {
            return invalid_argument(
                "`atTime` must be an RFC 3339 date-time (e.g. 2024-01-01T14:00:00Z).",
                &correlator,
            );
        }
    }

    // Resolve the reboot targets against the subscriber's deterministic set.
    let owned = device_list(subject);
    let targets: Vec<Value> = match req.devices.as_deref() {
        // Explicit, non-empty targets: each must be a well-formed UUID (else 400)
        // that belongs to the subscriber (else 404).
        Some(list) if !list.is_empty() => {
            if list.len() > 100 {
                return invalid_argument(
                    "`devices` must contain at most 100 entries.",
                    &correlator,
                );
            }
            let mut resolved = Vec::with_capacity(list.len());
            for dev in list {
                if !is_uuid_shaped(dev) {
                    return invalid_argument("`devices` entries must be UUIDs.", &correlator);
                }
                if !owned.iter().any(|d| d["id"] == json!(dev)) {
                    return with_correlator(
                        CamaraError::not_found(
                            "A targeted device is not one of the subscriber's Network Access Devices.",
                        )
                        .into_response(),
                        &correlator,
                    );
                }
                resolved.push(json!(dev));
            }
            resolved
        }
        // Omitted or empty → reboot all the subscriber's devices.
        _ => owned.iter().map(|d| d["id"].clone()).collect(),
    };

    // Build the RebootRequest, persist it, and return 201 + Location.
    let id = store::new_reboot_request_id();
    let now = rfc3339_utc(now_unix_secs());
    let mut resource = json!({
        "id": id,
        "devices": targets,
        "createdAt": now,
        "modifiedAt": now,
    });
    if let Some(message) = req.message {
        resource["message"] = json!(message);
    }
    if let Some(at_time) = req.at_time {
        resource["atTime"] = json!(at_time);
    }
    store::insert(id.clone(), resource.clone());

    let location = format!("{REBOOT_REQUESTS}/{id}");
    let mut response = (StatusCode::CREATED, Json(resource)).into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("location"), value);
    }
    with_correlator(response, &correlator)
}

/// The subscriber's device list, deterministic from the token subject.
///
/// The subject's trailing three digits `d` (or `0` when it has no digits) fix
/// the count (`d == 0` → 1, else `((d - 1) % 3) + 1`) and each device's status
/// (device `i` → `STATUSES[(d + i) % 3]`).
fn device_list(subject: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(subject).unwrap_or(0) as usize;
    let count = if d == 0 { 1 } else { ((d - 1) % 3) + 1 };
    (0..count).map(|i| device(subject, i, (d + i) % 3)).collect()
}

/// One `NetworkAccessDevice`. `id` and `hardwareAddress` are derived from the
/// subject and device index via SHA-256 (deterministic, no new dependency).
fn device(subject: &str, index: usize, status_idx: usize) -> Value {
    let digest = Sha256::digest(format!("nad-device:{subject}:{index}").as_bytes());
    json!({
        "id": uuid_shaped(&digest),
        "deviceStatus": STATUSES[status_idx],
        "name": format!("Gateway-{}", index + 1),
        "description": "Operator-supplied network access gateway",
        "hardwareAddress": {
            "hardwareAddressType": "EUI-48",
            "value": mac_address(&digest),
        },
    })
}

/// A stable, UUID-shaped identifier from the first 16 bytes of a SHA-256 digest.
/// UUID-*shaped* (not a real v4/v5 UUID) — a documented cut mirroring Simple
/// Edge Discovery's `edgeCloudZoneId` and the Blockchain Public Address ids.
fn uuid_shaped(h: &[u8]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// An EUI-48 hardware address (`XX:XX:XX:XX:XX:XX`) from six digest bytes.
fn mac_address(h: &[u8]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        h[16], h[17], h[18], h[19], h[20], h[21]
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

/// A `400 INVALID_ARGUMENT` CAMARA error with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
}

/// True if `s` has the UUID shape `8-4-4-4-12` lowercase/uppercase hex — the shape
/// of a device `id` in the subscriber's set. A reboot target that is not even
/// UUID-shaped is a body-validation error (`400`), distinct from a well-formed id
/// that is simply not one of the subscriber's devices (`404`).
fn is_uuid_shaped(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.iter().map(|g| g.len()).collect::<Vec<_>>() == vec![8, 4, 4, 4, 12]
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// True if `s` parses as an RFC 3339 date-time. Thin wrapper over [`parse_rfc3339`]
/// used to validate the optional `atTime` (a malformed value → `400`).
fn is_rfc3339(s: &str) -> bool {
    parse_rfc3339(s).is_some()
}

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs no
/// date/time dependency (mirrors `quality_on_demand::v1`).
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

/// Parse an RFC 3339 / ISO 8601 date-time, returning `Some(())` when well-formed.
/// Accepts a `Z` or `±HH:MM` offset and an optional fractional second (ignored);
/// an offset is required (RFC 3339). Self-contained (mirrors
/// `device_visit_location::vwip`); only validity matters here, so the epoch value
/// is discarded.
fn parse_rfc3339(s: &str) -> Option<()> {
    let (date, rest) = s.split_once(['T', 't'])?;
    let mut d = date.splitn(3, '-');
    let year = d.next()?;
    if year.len() != 4 || year.parse::<i64>().is_err() {
        return None;
    }
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let time = if let Some(t) = rest.strip_suffix(['Z', 'z']) {
        t
    } else if let Some(idx) = rest.rfind(['+', '-']) {
        let (t, off) = rest.split_at(idx);
        let (oh, om) = off[1..].split_once(':')?;
        let oh: i64 = oh.parse().ok()?;
        let om: i64 = om.parse().ok()?;
        if oh > 14 || om >= 60 {
            return None;
        }
        t
    } else {
        return None; // RFC 3339 requires an offset
    };

    let time = time.split_once('.').map_or(time, |(hms, _)| hms);
    let mut hms = time.splitn(3, ':');
    let hh: i64 = hms.next()?.parse().ok()?;
    let mm: i64 = hms.next()?.parse().ok()?;
    let ss: i64 = hms.next()?.parse().ok()?;
    if hms.next().is_some() || !(0..=23).contains(&hh) || !(0..=59).contains(&mm) || !(0..=60).contains(&ss)
    {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "sim.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn count_and_status_follow_the_subject_digit_plane() {
        // No digits / …000 → a single connected device.
        let no_digits = device_list("nad-client");
        assert_eq!(no_digits.len(), 1);
        assert_eq!(no_digits[0]["deviceStatus"], "connected");

        let zero = device_list("+123456789000");
        assert_eq!(zero.len(), 1);
        assert_eq!(zero[0]["deviceStatus"], "connected");

        // …001 → one disconnected device.
        let one = device_list("+123456789001");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0]["deviceStatus"], "disconnected");

        // …002 → two devices [unavailable, connected].
        let two = device_list("+123456789002");
        assert_eq!(two.len(), 2);
        assert_eq!(two[0]["deviceStatus"], "unavailable");
        assert_eq!(two[1]["deviceStatus"], "connected");

        // …003 → three devices [connected, disconnected, unavailable].
        let three = device_list("+123456789003");
        assert_eq!(three.len(), 3);
        assert_eq!(three[0]["deviceStatus"], "connected");
        assert_eq!(three[1]["deviceStatus"], "disconnected");
        assert_eq!(three[2]["deviceStatus"], "unavailable");

        // …004 wraps the count back to 1.
        assert_eq!(device_list("+123456789004").len(), 1);
    }

    #[test]
    fn ids_and_macs_are_deterministic_and_well_shaped() {
        let a = device_list("+123456789002");
        let b = device_list("+123456789002");
        // Deterministic per (subject, index).
        assert_eq!(a, b);
        // Distinct devices get distinct ids.
        assert_ne!(a[0]["id"], a[1]["id"]);

        let id = a[0]["id"].as_str().unwrap();
        // UUID-shaped: 8-4-4-4-12 lowercase hex.
        let groups: Vec<&str> = id.split('-').collect();
        assert_eq!(groups.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));

        let mac = a[0]["hardwareAddress"]["value"].as_str().unwrap();
        let octets: Vec<&str> = mac.split(':').collect();
        assert_eq!(octets.len(), 6);
        assert!(octets.iter().all(|o| o.len() == 2 && o.chars().all(|c| c.is_ascii_hexdigit())));
        assert_eq!(a[0]["hardwareAddress"]["hardwareAddressType"], "EUI-48");
    }

    // --- Integration through the real router -------------------------------

    /// App with the auth routes (token endpoint) and the Network Access Devices
    /// routes, so a real token can be minted and presented.
    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials` with a caller-chosen
    /// `client_id` (which becomes the token `sub`), host-pinned so its `aud`
    /// matches the route's audience. `+` is percent-encoded so an E.164 client
    /// id survives the urlencoded body.
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

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "nad-client").await
    }

    /// GET the listing with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_devices(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-devices/vwip/network-access-devices")
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

    /// GET one device by id with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_device_by_id(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/network-access-devices/vwip/network-access-devices/{id}"
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

    #[tokio::test]
    async fn happy_path_lists_a_single_connected_device() {
        // sub = client_id "nad-client" (no digits) → one connected device.
        let token = mint_token(SCOPE).await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["deviceStatus"], "connected");
        assert_eq!(arr[0]["name"], "Gateway-1");
        assert!(arr[0]["id"].is_string());
    }

    #[tokio::test]
    async fn subject_digits_drive_the_device_count_and_status() {
        // …002 → two devices [unavailable, connected].
        let token = mint_token_with_client(SCOPE, "+123456789002").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["deviceStatus"], "unavailable");
        assert_eq!(arr[1]["deviceStatus"], "connected");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        // …404 → 404 NOT_FOUND.
        let token = mint_token_with_client(SCOPE, "+123456789404").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // user-503 → 503 UNAVAILABLE (subject need not be a phone number).
        let token = mint_token_with_client(SCOPE, "user-503").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_devices(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        // Success.
        let token = mint_token(SCOPE).await;
        let (status, headers, _) = get_devices(Some(&token), Some("corr-nad")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nad")
        );

        // Business error.
        let token = mint_token_with_client(SCOPE, "+123456789404").await;
        let (status, headers, _) = get_devices(Some(&token), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- getNetworkAccessDevice (read by id) -------------------------------

    #[tokio::test]
    async fn read_by_id_returns_the_device_from_the_subjects_set() {
        // …002 → two devices; read each id back and confirm it round-trips.
        let token = mint_token_with_client(SCOPE, "+123456789002").await;
        let (_, _, list) = get_devices(Some(&token), None).await;
        let arr = list.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        for expected in arr {
            let id = expected["id"].as_str().unwrap();
            let (status, _, body) = get_device_by_id(Some(&token), id, None).await;
            assert_eq!(status, StatusCode::OK);
            // The read returns exactly the listed device (not an array).
            assert_eq!(&body, expected);
        }
    }

    #[tokio::test]
    async fn read_by_unknown_id_is_not_found() {
        let token = mint_token(SCOPE).await;
        // An id that is not part of this subject's deterministic set.
        let (status, _, body) =
            get_device_by_id(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // A malformed (non-UUID) id is likewise simply not found — no 400.
        let (status, _, body) = get_device_by_id(Some(&token), "not-a-real-id", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_by_id_belonging_to_another_subscriber_is_not_found() {
        // Mint the id from one subject, then try to read it as a different one.
        let owner = "+123456789002";
        let owner_list = device_list(owner);
        let foreign_id = owner_list[0]["id"].as_str().unwrap().to_string();

        // A different subject (no digits → its own single-device set).
        let token = mint_token_with_client(SCOPE, "other-subscriber").await;
        let (status, _, body) = get_device_by_id(Some(&token), &foreign_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_by_id_reserved_subject_suffix_selects_a_canonical_error() {
        // A reserved-suffix subject takes the whole account into a canonical
        // error, regardless of the id (mirrors the listing).
        let token = mint_token_with_client(SCOPE, "+123456789503").await;
        let (status, _, body) =
            get_device_by_id(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn read_by_id_requires_the_scope_and_a_token() {
        // Wrong scope → 403.
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            get_device_by_id(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");

        // No token → 401.
        let (status, _, body) =
            get_device_by_id(None, "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn read_by_id_echoes_x_correlator_on_success_and_error() {
        let token = mint_token(SCOPE).await;
        let (_, _, list) = get_devices(Some(&token), None).await;
        let id = list.as_array().unwrap()[0]["id"].as_str().unwrap().to_string();

        // Success.
        let (status, headers, _) = get_device_by_id(Some(&token), &id, Some("corr-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        // Not-found error.
        let (status, headers, _) =
            get_device_by_id(Some(&token), "not-a-real-id", Some("corr-nf")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nf")
        );
    }

    // --- Pure units for the reboot-request helpers -------------------------

    #[test]
    fn uuid_shape_and_rfc3339_validation() {
        assert!(is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid_shaped("3FA85F64-5717-4562-B3FC-2C963F66AFA6"));
        assert!(!is_uuid_shaped("not-a-real-id"));
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa")); // 11 in last group
        assert!(!is_uuid_shaped("zzzzzzzz-5717-4562-b3fc-2c963f66afa6")); // non-hex

        assert!(is_rfc3339("2024-01-01T14:00:00Z"));
        assert!(is_rfc3339("2024-01-01T14:00:00.312+02:00"));
        assert!(!is_rfc3339("2024-01-01")); // no time
        assert!(!is_rfc3339("2024-01-01T14:00:00")); // no offset
        assert!(!is_rfc3339("2024-13-01T00:00:00Z")); // bad month
    }

    // --- createRebootRequest (POST /reboot-requests) -----------------------

    /// POST a reboot-request body (raw JSON string, or `None` for an empty body)
    /// with an optional Bearer token and `x-correlator`.
    async fn post_reboot_request(
        token: Option<&str>,
        body: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/network-access-devices/vwip/reboot-requests")
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder
            .body(Body::from(body.unwrap_or("").to_string()))
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn create_defaults_to_rebooting_all_the_subscribers_devices() {
        // …002 → two devices; a create with no `devices` targets both, in order.
        let token = mint_token_with_client(SCOPE, "+123456789002").await;
        let expected: Vec<Value> = device_list("+123456789002")
            .into_iter()
            .map(|d| d["id"].clone())
            .collect();

        // An empty body is a valid "reboot everything now" create.
        let (status, headers, body) = post_reboot_request(Some(&token), None, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["devices"], Value::Array(expected));
        assert!(body["id"].is_string());
        assert!(body["createdAt"].is_string());
        assert_eq!(body["createdAt"], body["modifiedAt"]);
        // No optional fields were sent, so none are echoed.
        assert!(body.get("message").is_none());
        assert!(body.get("atTime").is_none());
        // A Location header points at the created resource.
        let id = body["id"].as_str().unwrap();
        assert_eq!(
            headers.get("location").and_then(|v| v.to_str().ok()),
            Some(format!("/network-access-devices/vwip/reboot-requests/{id}").as_str())
        );
    }

    #[tokio::test]
    async fn create_persists_the_resource_in_the_store() {
        let token = mint_token(SCOPE).await;
        let (status, _, body) = post_reboot_request(Some(&token), Some("{}"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = body["id"].as_str().unwrap();
        // The created representation is readable back from the shared store.
        assert_eq!(store::get(id), Some(body));
    }

    #[tokio::test]
    async fn create_with_explicit_valid_device_echoes_only_that_target() {
        // …002 → two devices; target just the first by its id.
        let token = mint_token_with_client(SCOPE, "+123456789002").await;
        let owned = device_list("+123456789002");
        let target = owned[0]["id"].as_str().unwrap().to_string();

        let body = format!(r#"{{"devices":["{target}"]}}"#);
        let (status, _, resp) = post_reboot_request(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp["devices"], json!([target]));
    }

    #[tokio::test]
    async fn create_echoes_message_and_at_time() {
        let token = mint_token(SCOPE).await;
        let body = r#"{"message":"nightly maintenance","atTime":"2024-06-01T02:00:00Z"}"#;
        let (status, _, resp) = post_reboot_request(Some(&token), Some(body), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp["message"], "nightly maintenance");
        assert_eq!(resp["atTime"], "2024-06-01T02:00:00Z");
    }

    #[tokio::test]
    async fn create_with_unknown_but_well_formed_device_is_not_found() {
        let token = mint_token(SCOPE).await;
        // A UUID that is not one of the subscriber's devices → 404.
        let body = r#"{"devices":["00000000-0000-4000-8000-000000000000"]}"#;
        let (status, _, resp) = post_reboot_request(Some(&token), Some(body), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn create_with_malformed_inputs_is_invalid_argument() {
        let token = mint_token(SCOPE).await;

        // A device entry that is not UUID-shaped → 400 (distinct from the 404 above).
        let (status, _, resp) =
            post_reboot_request(Some(&token), Some(r#"{"devices":["nope"]}"#), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");

        // A malformed `atTime` → 400.
        let (status, _, _) =
            post_reboot_request(Some(&token), Some(r#"{"atTime":"soon"}"#), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // A `message` over 255 chars → 400.
        let long = "x".repeat(256);
        let (status, _, _) =
            post_reboot_request(Some(&token), Some(&format!(r#"{{"message":"{long}"}}"#)), None)
                .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // An unknown field → 400 (deny_unknown_fields).
        let (status, _, _) =
            post_reboot_request(Some(&token), Some(r#"{"foo":1}"#), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Non-JSON body → 400.
        let (status, _, _) = post_reboot_request(Some(&token), Some("not json"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_reserved_subject_suffix_selects_a_canonical_error() {
        // A reserved subject suffix answers a canonical error before creating.
        let token = mint_token_with_client(SCOPE, "+123456789503").await;
        let (status, _, resp) = post_reboot_request(Some(&token), Some("{}"), None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn create_requires_the_scope_and_a_token() {
        // Wrong scope → 403.
        let token = mint_token("some:other-scope").await;
        let (status, _, resp) = post_reboot_request(Some(&token), Some("{}"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");

        // No token → 401.
        let (status, _, resp) = post_reboot_request(None, Some("{}"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn create_echoes_x_correlator_on_success_and_error() {
        // Success.
        let token = mint_token(SCOPE).await;
        let (status, headers, _) =
            post_reboot_request(Some(&token), Some("{}"), Some("corr-rb")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-rb")
        );

        // Error.
        let (status, headers, _) =
            post_reboot_request(Some(&token), Some("bad"), Some("corr-rberr")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-rberr")
        );
    }
}
