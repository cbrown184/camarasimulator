//! Dedicated Network — Accesses **vwip** (CAMARA DedicatedNetworks,
//! work-in-progress).
//!
//! Endpoints:
//! - `POST /dedicated-network-accesses/vwip/accesses` — create a dedicated
//!   network access (operationId `createAccess`).
//! - `GET /dedicated-network-accesses/vwip/accesses` — list created accesses,
//!   optionally filtered by `networkId` (operationId `listAccesses`).
//! - `GET /dedicated-network-accesses/vwip/accesses/{accessId}` — read a created
//!   access back by id (operationId `readAccess`).
//! - `DELETE /dedicated-network-accesses/vwip/accesses/{accessId}` — delete an
//!   access by id (operationId `deleteAccess`).
//! - `GET /dedicated-network-accesses/vwip/accesses/{accessId}/devices` — list
//!   the devices bound to an access, paginated (operationId `listDevices`).
//! - `POST /dedicated-network-accesses/vwip/accesses/{accessId}/devices/add` —
//!   add devices to an access (operationId `addDevicesToAccess`).
//! - `POST /dedicated-network-accesses/vwip/accesses/{accessId}/devices/remove` —
//!   remove devices from an access (operationId `removeDevicesFromAccess`).
//!
//! ## What it does
//!
//! An **access** binds a set of devices (each a CAMARA `Device`) to a dedicated
//! network (`networkId`, from the companion **Networks** API), optionally
//! restricting the QoS profiles those devices may use (`qosProfiles` /
//! `defaultQosProfile`). `createAccess` accepts a `CreateAccessRequest`, mints an
//! opaque UUID `id`, evaluates each submitted device to a per-device
//! `GRANTED`/`DENIED` status, records the rendered `AccessInfo` in an in-memory
//! store ([`super::store`]) so later passes' read/list/delete and device legs can
//! address it, and returns `201`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `dedicated-network-accesses:accesses:create` scope. It is a two-legged
//! (`client_credentials`) provisioning call — an access provisions a network for
//! a fleet of devices, not for the caller's own line, so there is no
//! three-legged dance.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Three control planes:
//!
//! 1. **Request validation → 400.** A body that is not a valid
//!    `CreateAccessRequest`, a missing/non-UUID `networkId`, a `devices` array
//!    outside `1..=100` items, a device carrying **no** identifier (none of
//!    `phoneNumber` / `networkAccessIdentifier` / `ipv4Address` / `ipv6Address`),
//!    a non-E.164 `phoneNumber`, a `qosProfiles` array outside `1..=32` items or
//!    an empty profile name, or a `sink` not matching `^https://.+$` → `400
//!    INVALID_ARGUMENT`.
//! 2. **`networkId` identifier plane.** The required `networkId` UUID is the
//!    top-level identifier. Its trailing three digits drive the result
//!    (docs/DESIGN.md §7): a reserved suffix ([`crate::scenarios`]) → the
//!    canonical CAMARA error — e.g. `…404` → `404 NOT_FOUND` (no such network),
//!    `…422` → `422`, `…409` → `409`. Otherwise the access is created.
//! 3. **Per-device grant plane.** Each submitted device's outcome is driven by
//!    its own identifier (`phoneNumber`, else `networkAccessIdentifier`, else
//!    `ipv6Address`, else the IPv4 `publicAddress`): a reserved-suffix identifier
//!    → that device is `DENIED`, any other → `GRANTED`. This drives the
//!    aggregate `stats` (`totalDevices` / `totalGranted` / `totalDenied`) and the
//!    per-device `recentAccessDevices` list, so `devices` is a genuine second
//!    plane.
//!
//! `sinkCredential` is accepted but never echoed (it is a secret). This slice
//! being create-only, the following are **documented cuts** (later passes):
//! the CAMARA `207` multi-status form with a `ResultForDevice[]` body — CamaraSim
//! represents partial denials as data inside the `201 AccessInfo`
//! (`stats`/`recentAccessDevices`) rather than as a `207` — the `409`/`422`
//! request-level device conflicts and `sink` notification delivery.
//! `x-correlator` is echoed on every response.
//!
//! ## `addDevicesToAccess` — add devices to an access
//!
//! `POST /accesses/{accessId}/devices/add` takes a bare `AddDevicesRequest` (a
//! JSON array of `1..=100` CAMARA `Device`s — the array *is* the body), grants
//! each device by the same per-device plane as `createAccess`, appends them to
//! the stored access's `recentAccessDevices` roster and recomputes its `stats`
//! atomically ([`super::store::update`]), and returns the added devices as an
//! `AddDevicesSuccess` (`AccessDevices` array, `201`). Request validation runs
//! before the store lookup (a bad body → `400 INVALID_ARGUMENT`, winning over a
//! `404`); an unknown/malformed `accessId` (a server-minted opaque UUID, no
//! reserved-suffix plane) → `404 NOT_FOUND`. It requires the
//! `dedicated-network-accesses:devices:add` scope. The `207`
//! partial-success/`422 NO_VALID_DEVICE` forms remain documented cuts (denials
//! ride inside each `AccessDevice.status`, as in `createAccess`).
//!
//! ## `removeDevicesFromAccess` — remove devices from an access
//!
//! `POST /accesses/{accessId}/devices/remove` takes a bare `RemoveDevicesRequest`
//! (a JSON array of `1..=100` CAMARA `Device`s — the array *is* the body, mirroring
//! `addDevicesToAccess`), and evicts the matching devices from the stored access's
//! `recentAccessDevices` roster, recomputing its `stats` atomically
//! ([`super::store::update`]). A roster entry matches a submitted device when their
//! primary identifiers (the same first-present-of `phoneNumber` /
//! `networkAccessIdentifier` / `ipv6Address` / IPv4 `publicAddress` used by the
//! grant plane) are equal. On success the response is `204 No Content` (there is no
//! body — the CAMARA `207` partial-success form is a documented cut: a submitted
//! device absent from the roster is a per-device no-op folded into the access-level
//! `204`, so the operation is idempotent). Request validation runs before the store
//! lookup (a bad body → `400 INVALID_ARGUMENT`, winning over a `404`); an
//! unknown/malformed `accessId` (a server-minted opaque UUID, no reserved-suffix
//! plane) → `404 NOT_FOUND`. It requires the
//! `dedicated-network-accesses:devices:remove` scope. `x-correlator` is echoed on
//! every response, including the `204`.
//!
//! ## `readAccess` — read an access back
//!
//! `GET /accesses/{accessId}` returns the `AccessInfo` stored by a prior
//! `createAccess` (`200`), or `404 NOT_FOUND` when no such access exists. In
//! CamaraSim the `accessId` is a server-minted opaque UUID, so the only control
//! plane is the in-memory store state — a valid id from a prior `createAccess`
//! reads back, anything else (unknown or malformed) is `404` (there is no
//! reserved-suffix plane on a minted id, mirroring the sibling `readNetwork`).
//! It requires the `dedicated-network-accesses:accesses:read` scope.
//!
//! ## `listAccesses` — list accesses
//!
//! `GET /accesses` returns a bare JSON array of every stored `AccessInfo`
//! (`200`, empty when none — CAMARA never 404s on an empty list). The optional
//! `networkId` query parameter filters to the accesses created against that
//! network (a genuine second control plane); a present but non-UUID `networkId`
//! → `400 INVALID_ARGUMENT`, and an unknown-but-valid `networkId` → an empty
//! array. It requires the `dedicated-network-accesses:accesses:read` scope.
//!
//! ## `deleteAccess` — delete an access
//!
//! `DELETE /accesses/{accessId}` evicts the access named by the opaque
//! `accessId`: a known id → `204 No Content` (single-use); an unknown or
//! already-deleted one → `404 NOT_FOUND`. Keyed only on the store state (no
//! reserved-suffix plane on a minted id), synchronous (no async
//! `202`/`DELETE_REQUESTED`), no `sink` notification (documented cut); mirrors
//! the sibling Networks API's `deleteNetwork`. It requires the
//! `dedicated-network-accesses:accesses:delete` scope.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

use super::store;

/// The OAuth2 scope `createAccess` requires (CAMARA Dedicated Network —
/// Accesses).
const CREATE_SCOPE: &str = "dedicated-network-accesses:accesses:create";

/// The OAuth2 scope `readAccess` **and** `listAccesses` require (CAMARA Dedicated
/// Network — Accesses).
const READ_SCOPE: &str = "dedicated-network-accesses:accesses:read";

/// The OAuth2 scope `deleteAccess` requires (CAMARA Dedicated Network —
/// Accesses).
const DELETE_SCOPE: &str = "dedicated-network-accesses:accesses:delete";

/// The OAuth2 scope `listDevices` requires (CAMARA Dedicated Network —
/// Accesses).
const DEVICES_READ_SCOPE: &str = "dedicated-network-accesses:devices:read";

/// The OAuth2 scope `addDevicesToAccess` requires (CAMARA Dedicated Network —
/// Accesses).
const DEVICES_ADD_SCOPE: &str = "dedicated-network-accesses:devices:add";

/// The OAuth2 scope `removeDevicesFromAccess` requires (CAMARA Dedicated
/// Network — Accesses).
const DEVICES_REMOVE_SCOPE: &str = "dedicated-network-accesses:devices:remove";

/// Routes for Dedicated Network — Accesses vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/dedicated-network-accesses/vwip/accesses",
            post(create_access).get(list_access),
        )
        .route(
            "/dedicated-network-accesses/vwip/accesses/:access_id",
            get(read_access).delete(delete_access),
        )
        .route(
            "/dedicated-network-accesses/vwip/accesses/:access_id/devices",
            get(list_devices),
        )
        .route(
            "/dedicated-network-accesses/vwip/accesses/:access_id/devices/add",
            post(add_devices),
        )
        .route(
            "/dedicated-network-accesses/vwip/accesses/:access_id/devices/remove",
            post(remove_devices),
        )
}

/// A `CreateAccessRequest` body (CAMARA `BaseAccessInfo` + `devices`). Every
/// field is optional at the serde layer; required-ness and bounds are enforced in
/// the handler so each failure maps to the right CAMARA error. `devices` is kept
/// as raw JSON so the submitted `Device` objects are echoed verbatim in
/// `recentAccessDevices`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateAccessRequest {
    #[serde(rename = "networkId")]
    network_id: Option<String>,
    devices: Option<Vec<Value>>,
    #[serde(rename = "qosProfiles")]
    qos_profiles: Option<Vec<String>>,
    #[serde(rename = "defaultQosProfile")]
    default_qos_profile: Option<String>,
    sink: Option<String>,
    // Accepted but never echoed (a secret; notifications are a later pass). Kept
    // in the struct so `deny_unknown_fields` still admits it.
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
    sink_credential: Option<Value>,
}

/// `POST /dedicated-network-accesses/vwip/accesses` — create a dedicated network
/// access (`createAccess`).
async fn create_access(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory; parse strictly (`deny_unknown_fields`).
    let req: CreateAccessRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CreateAccessRequest.",
                &correlator,
            )
        }
    };

    // `networkId` (required): must be UUID-shaped (schema `format: uuid`).
    let network_id = match req.network_id.as_deref() {
        Some(id) if is_uuid_shaped(id) => id.to_string(),
        Some(_) => return invalid_argument("`networkId` must be a UUID.", &correlator),
        None => return invalid_argument("`networkId` is required.", &correlator),
    };

    // `devices` (optional): when present, `1..=100` items, each a valid Device.
    if let Some(devices) = req.devices.as_deref() {
        if !(1..=100).contains(&devices.len()) {
            return invalid_argument(
                "`devices` must contain between 1 and 100 items.",
                &correlator,
            );
        }
        for device in devices {
            if let Err(msg) = validate_device(device) {
                return invalid_argument(&msg, &correlator);
            }
        }
    }

    // `qosProfiles` (optional): when present, `1..=32` non-empty names.
    if let Some(profiles) = req.qos_profiles.as_deref() {
        if !(1..=32).contains(&profiles.len()) {
            return invalid_argument(
                "`qosProfiles` must contain between 1 and 32 items.",
                &correlator,
            );
        }
        if profiles.iter().any(|p| p.is_empty()) {
            return invalid_argument("`qosProfiles` names must not be empty.", &correlator);
        }
    }

    // `defaultQosProfile` (optional): non-empty when present (schema `minLength: 1`).
    if let Some(name) = req.default_qos_profile.as_deref() {
        if name.is_empty() {
            return invalid_argument("`defaultQosProfile` must not be empty.", &correlator);
        }
    }

    // `sink` (optional): schema `pattern: ^https://.+$`.
    if let Some(sink) = req.sink.as_deref() {
        if !(sink.starts_with("https://") && sink.len() > "https://".len()) {
            return invalid_argument("`sink` must be an `https://` URI.", &correlator);
        }
    }

    // Control plane: a reserved suffix on `networkId` selects a canonical CAMARA
    // error (e.g. `…404` → no such network).
    if let Some(err) = scenarios::reserved_error(&network_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Per-device grant plane: each device's status is driven by its identifier.
    let devices = req.devices.unwrap_or_default();
    let recent: Vec<Value> = devices
        .iter()
        .map(|device| {
            let status = device_status(device);
            json!({ "device": device, "status": status })
        })
        .collect();
    let total_granted = recent
        .iter()
        .filter(|d| d["status"] == "GRANTED")
        .count();
    let total_denied = recent.len() - total_granted;

    // Build the AccessInfo, remember it, and return 201.
    let id = store::new_access_id();
    let info = build_access_info(
        &id,
        &network_id,
        recent,
        total_granted,
        total_denied,
        req.qos_profiles.as_deref(),
        req.default_qos_profile.as_deref(),
        req.sink.as_deref(),
    );
    store::insert(id.clone(), info.clone());

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// `GET /dedicated-network-accesses/vwip/accesses/{accessId}` — read a dedicated
/// network access back by id (`readAccess`).
///
/// Keyed only on the store state (the `accessId` is a server-minted opaque UUID,
/// so there is no reserved-suffix plane): a known id returns its stored
/// `AccessInfo` (`200`); an unknown or malformed one → `404 NOT_FOUND`. Mirrors
/// the sibling `readNetwork`.
async fn read_access(claims: Claims, headers: HeaderMap, Path(access_id): Path<String>) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&access_id) {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No access found for the provided accessId.").into_response(),
            &correlator,
        ),
    }
}

/// `GET /dedicated-network-accesses/vwip/accesses` — list dedicated network
/// accesses (`listAccesses`).
///
/// Returns a bare JSON array of the stored `AccessInfo`s (`200`, empty when
/// none — CAMARA never 404s on an empty list). The optional `networkId` query
/// parameter filters to the accesses whose `networkId` equals it (a genuine
/// control plane); a present but non-UUID `networkId` → `400 INVALID_ARGUMENT`,
/// an unknown-but-valid one → an empty array. Requires the read scope.
/// `x-correlator` echoed.
async fn list_access(claims: Claims, headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Optional `networkId` filter (schema `format: uuid`). Only `networkId` is a
    // defined query param; any other is ignored (CAMARA has no others here that
    // CamaraSim models — `x-device` filtering is a documented cut).
    let network_id_filter = match parse_network_id_filter(query.as_deref()) {
        Ok(f) => f,
        Err(msg) => return invalid_argument(&msg, &correlator),
    };

    let accesses: Vec<Value> = store::all()
        .into_iter()
        .filter(|a| match &network_id_filter {
            Some(want) => a.get("networkId").and_then(Value::as_str) == Some(want.as_str()),
            None => true,
        })
        .collect();

    with_correlator((StatusCode::OK, Json(accesses)).into_response(), &correlator)
}

/// `DELETE /dedicated-network-accesses/vwip/accesses/{accessId}` — delete a
/// dedicated network access by id (`deleteAccess`).
///
/// Keyed only on the store state (the `accessId` is a server-minted opaque
/// UUID, so there is no reserved-suffix plane): a known id evicts its access and
/// returns `204 No Content` (single-use); an unknown or already-deleted id →
/// `404 NOT_FOUND`. Synchronous deletion (no async `202`/`DELETE_REQUESTED`), no
/// `sink` notification (a documented cut). Mirrors the sibling `deleteNetwork`.
async fn delete_access(
    claims: Claims,
    headers: HeaderMap,
    Path(access_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::remove(&access_id) {
        Some(_) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No access found for the provided accessId.").into_response(),
            &correlator,
        ),
    }
}

/// `GET /dedicated-network-accesses/vwip/accesses/{accessId}/devices` — list the
/// devices bound to a dedicated network access (`listDevices`).
///
/// Reads the roster the access recorded at `createAccess` — its stored
/// `recentAccessDevices` (each an `AccessDevice` of `{ device, status }`) — and
/// returns it as a paginated `AccessDevicesPage` (`{ items, pagination }`).
/// Three control planes (docs/DESIGN.md §7): the opaque server-minted `accessId`
/// → store state (an unknown/malformed id → `404 NOT_FOUND`; there is no
/// reserved-suffix plane on a minted id, mirroring `readAccess`); the optional
/// `deviceStatus` filter (`REQUESTED`/`GRANTED`/`DENIED`) narrows the roster (a
/// genuine second plane; an unknown value → `400 INVALID_ARGUMENT`); and the
/// `page`/`perPage` pagination window (non-integer → `400 INVALID_ARGUMENT`, a
/// value `< 1` → `400 OUT_OF_RANGE`). Query validation runs before the store
/// lookup, so a bad query wins over a `404`. Requires the
/// `dedicated-network-accesses:devices:read` scope. `x-correlator` echoed.
async fn list_devices(
    claims: Claims,
    headers: HeaderMap,
    Path(access_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DEVICES_READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Validate + default the query parameters (page/perPage/deviceStatus) before
    // the store lookup, so a malformed query is a 400 regardless of whether the
    // access exists (mirrors the PATCH "body 400 wins over 404" convention).
    let params = match parse_devices_list_params(query.as_deref()) {
        Ok(p) => p,
        Err(ParamError::Invalid(msg)) => return invalid_argument(&msg, &correlator),
        Err(ParamError::OutOfRange(msg)) => return out_of_range(&msg, &correlator),
    };

    // Store state: an unknown/malformed accessId → 404 (no reserved-suffix plane).
    let info = match store::get(&access_id) {
        Some(info) => info,
        None => {
            return with_correlator(
                CamaraError::not_found("No access found for the provided accessId.").into_response(),
                &correlator,
            )
        }
    };

    // The roster is the access's recorded `recentAccessDevices` (AccessDevice[]).
    let mut devices: Vec<Value> = info
        .get("recentAccessDevices")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // `deviceStatus` filter (a genuine second control plane).
    if let Some(want) = params.device_status.as_deref() {
        devices.retain(|d| d.get("status").and_then(Value::as_str) == Some(want));
    }

    let page = build_access_devices_page(devices, &params);
    with_correlator((StatusCode::OK, Json(page)).into_response(), &correlator)
}

/// `POST /dedicated-network-accesses/vwip/accesses/{accessId}/devices/add` — add
/// devices to an existing dedicated network access (`addDevicesToAccess`).
///
/// The request body is a bare `AddDevicesRequest` — a JSON array of `1..=100`
/// CAMARA `Device`s (the array *is* the body; there is no wrapping object).
/// Each device is evaluated to the same per-device `GRANTED`/`DENIED` grant as
/// `createAccess` (a reserved-suffix identifier → `DENIED`, any other →
/// `GRANTED`), appended to the access's stored `recentAccessDevices` roster, and
/// the aggregate `stats` recomputed — all atomically via [`store::update`]. On
/// success the added devices are returned as an `AddDevicesSuccess`
/// (`AccessDevices` array) with `201`.
///
/// Two control planes (docs/DESIGN.md §7), in order: **request validation** —
/// a body that is not a JSON array, an array outside `1..=100` items, a device
/// carrying no identifier, or a non-E.164 `phoneNumber` → `400 INVALID_ARGUMENT`
/// (validated before the store lookup, so a bad body wins over a `404`); and the
/// **store state** — the opaque server-minted `accessId` (no reserved-suffix
/// plane) resolves to a known access (→ `201`) or an unknown/malformed one (→
/// `404 NOT_FOUND`). The **per-device grant** plane rides on the submitted
/// devices exactly as in `createAccess`. The CAMARA `207` partial-success form
/// (a `ResultForDevice[]` body) and the `422 NO_VALID_DEVICE` case are documented
/// cuts — CamaraSim folds every device's outcome into its `AccessDevice.status`
/// inside the `201`, mirroring `createAccess`. Requires the
/// `dedicated-network-accesses:devices:add` scope. `x-correlator` echoed.
async fn add_devices(
    claims: Claims,
    headers: HeaderMap,
    Path(access_id): Path<String>,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DEVICES_ADD_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is a bare `AddDevicesRequest` — a JSON array of Device objects.
    let devices: Vec<Value> = match serde_json::from_slice(&body) {
        Ok(devices) => devices,
        Err(_) => {
            return invalid_argument(
                "Request body must be a JSON array of Device objects.",
                &correlator,
            )
        }
    };

    // Array bounds (`minItems: 1`, `maxItems: 100`) — mirrors `createAccess`'s
    // `devices` bound, so the two device-array validations behave identically.
    if !(1..=100).contains(&devices.len()) {
        return invalid_argument(
            "The request body must contain between 1 and 100 devices.",
            &correlator,
        );
    }
    for device in &devices {
        if let Err(msg) = validate_device(device) {
            return invalid_argument(&msg, &correlator);
        }
    }

    // Resolve each submitted device to its grant status (the per-device plane).
    let added: Vec<Value> = devices
        .iter()
        .map(|device| {
            let status = device_status(device);
            json!({ "device": device, "status": status })
        })
        .collect();

    // Append to the access's roster and recompute its stats atomically; `None`
    // means no such access (→ 404). Body validation already ran, so a bad body
    // wins over this 404 (mirrors `listDevices`).
    let hit = store::update(&access_id, |info| {
        let Some(obj) = info.as_object_mut() else {
            return;
        };
        let roster = obj
            .entry("recentAccessDevices")
            .or_insert_with(|| Value::Array(Vec::new()));
        if !roster.is_array() {
            *roster = Value::Array(Vec::new());
        }
        let arr = roster.as_array_mut().expect("roster is an array");
        arr.extend(added.iter().cloned());
        // Keep only the most-recent 100 (schema `recentAccessDevices` maxItems:
        // 100); `stats` is recomputed from this same window so the two agree.
        if arr.len() > 100 {
            let excess = arr.len() - 100;
            arr.drain(0..excess);
        }
        let total_granted = arr.iter().filter(|d| d["status"] == "GRANTED").count();
        let total_denied = arr.len() - total_granted;
        obj.insert(
            "stats".to_string(),
            json!({
                "totalDevices": total_granted + total_denied,
                "totalGranted": total_granted,
                "totalDenied": total_denied,
            }),
        );
    });

    match hit {
        Some(()) => with_correlator(
            (StatusCode::CREATED, Json(added)).into_response(),
            &correlator,
        ),
        None => with_correlator(
            CamaraError::not_found("No access found for the provided accessId.").into_response(),
            &correlator,
        ),
    }
}

/// `POST /dedicated-network-accesses/vwip/accesses/{accessId}/devices/remove` —
/// remove devices from an existing dedicated network access
/// (`removeDevicesFromAccess`).
///
/// The request body is a bare `RemoveDevicesRequest` — a JSON array of `1..=100`
/// CAMARA `Device`s (the array *is* the body, mirroring `addDevicesToAccess`).
/// Each matching device is evicted from the access's stored `recentAccessDevices`
/// roster and the aggregate `stats` recomputed — all atomically via
/// [`store::update`]. A roster entry matches a submitted device when their primary
/// identifiers ([`device_identifier`], the same first-present-of identifier the
/// grant plane uses) are equal. On success the response is `204 No Content`.
///
/// Two control planes (docs/DESIGN.md §7), in order: **request validation** — a
/// body that is not a JSON array, an array outside `1..=100` items, a device
/// carrying no identifier, or a non-E.164 `phoneNumber` → `400 INVALID_ARGUMENT`
/// (validated before the store lookup, so a bad body wins over a `404`); and the
/// **store state** — the opaque server-minted `accessId` (no reserved-suffix
/// plane) resolves to a known access (→ `204`) or an unknown/malformed one (→
/// `404 NOT_FOUND`). A submitted device that is not in the roster is a per-device
/// no-op folded into the access-level `204` (so the operation is idempotent) — the
/// CAMARA `207` partial-success form is a documented cut, mirroring
/// `addDevicesToAccess`. Requires the `dedicated-network-accesses:devices:remove`
/// scope. `x-correlator` echoed, including on the `204`.
async fn remove_devices(
    claims: Claims,
    headers: HeaderMap,
    Path(access_id): Path<String>,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DEVICES_REMOVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is a bare `RemoveDevicesRequest` — a JSON array of Device objects.
    let devices: Vec<Value> = match serde_json::from_slice(&body) {
        Ok(devices) => devices,
        Err(_) => {
            return invalid_argument(
                "Request body must be a JSON array of Device objects.",
                &correlator,
            )
        }
    };

    // Array bounds (`minItems: 1`, `maxItems: 100`) — mirrors `addDevicesToAccess`.
    if !(1..=100).contains(&devices.len()) {
        return invalid_argument(
            "The request body must contain between 1 and 100 devices.",
            &correlator,
        );
    }
    for device in &devices {
        if let Err(msg) = validate_device(device) {
            return invalid_argument(&msg, &correlator);
        }
    }

    // The set of primary identifiers to evict (validation guarantees each device
    // carries one).
    let targets: std::collections::HashSet<String> =
        devices.iter().filter_map(device_identifier).collect();

    // Evict the matching roster entries and recompute stats atomically; `None`
    // means no such access (→ 404). Body validation already ran, so a bad body
    // wins over this 404 (mirrors `addDevicesToAccess`).
    let hit = store::update(&access_id, |info| {
        let Some(obj) = info.as_object_mut() else {
            return;
        };
        let roster = obj
            .entry("recentAccessDevices")
            .or_insert_with(|| Value::Array(Vec::new()));
        if !roster.is_array() {
            *roster = Value::Array(Vec::new());
        }
        let arr = roster.as_array_mut().expect("roster is an array");
        arr.retain(|entry| {
            !entry
                .get("device")
                .and_then(device_identifier)
                .map(|id| targets.contains(&id))
                .unwrap_or(false)
        });
        // `stats` is recomputed from the surviving roster so the two agree.
        let total_granted = arr.iter().filter(|d| d["status"] == "GRANTED").count();
        let total_denied = arr.len() - total_granted;
        obj.insert(
            "stats".to_string(),
            json!({
                "totalDevices": total_granted + total_denied,
                "totalGranted": total_granted,
                "totalDenied": total_denied,
            }),
        );
    });

    match hit {
        Some(()) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No access found for the provided accessId.").into_response(),
            &correlator,
        ),
    }
}

/// The validated `page`/`perPage`/`deviceStatus` list controls for `listDevices`.
#[derive(Debug, PartialEq)]
struct DevicesListParams {
    /// 1-based page index (schema `minimum: 1`, default `1`).
    page: i64,
    /// Page size (schema `minimum: 1`, default `10`).
    per_page: i64,
    /// Optional `DeviceStatus` filter (`REQUESTED`/`GRANTED`/`DENIED`).
    device_status: Option<String>,
}

/// Why a `listDevices` query parameter was rejected — kept distinct so the
/// handler maps each to the right CAMARA error (`400 INVALID_ARGUMENT` vs
/// `400 OUT_OF_RANGE`). Keeping the parse pure over its input lets it be
/// unit-tested directly.
#[derive(Debug, PartialEq)]
enum ParamError {
    /// Non-integer `page`/`perPage`, or an unknown `deviceStatus` → INVALID_ARGUMENT.
    Invalid(String),
    /// A `page`/`perPage` below the schema minimum of 1 → OUT_OF_RANGE.
    OutOfRange(String),
}

/// Parse and validate the `listDevices` query string. Absent `page`/`perPage`
/// fall back to their schema defaults (`1`/`10`); a non-integer → `Invalid`; a
/// value `< 1` → `OutOfRange`. An unknown `deviceStatus` (not one of the three
/// enum values) → `Invalid`. Unknown query params are ignored. Pure over its
/// input, so it is unit-tested directly.
fn parse_devices_list_params(query: Option<&str>) -> Result<DevicesListParams, ParamError> {
    let raw = query.unwrap_or("");
    let mut page_raw: Option<String> = None;
    let mut per_page_raw: Option<String> = None;
    let mut device_status: Option<String> = None;
    for (key, value) in url_form_pairs(raw) {
        match key.as_str() {
            "page" => page_raw = Some(value),
            "perPage" => per_page_raw = Some(value),
            "deviceStatus" => device_status = Some(value),
            _ => {} // unknown params ignored
        }
    }

    let page = parse_positive_int(page_raw.as_deref(), "page", 1)?;
    let per_page = parse_positive_int(per_page_raw.as_deref(), "perPage", 10)?;

    if let Some(status) = device_status.as_deref() {
        if !matches!(status, "REQUESTED" | "GRANTED" | "DENIED") {
            return Err(ParamError::Invalid(
                "`deviceStatus` must be one of REQUESTED, GRANTED, DENIED.".to_string(),
            ));
        }
    }

    Ok(DevicesListParams {
        page,
        per_page,
        device_status,
    })
}

/// Parse an optional integer query parameter with a schema `minimum: 1`: absent
/// → `default`; a non-integer → `Invalid`; a value `< 1` → `OutOfRange`. Pure
/// over its input.
fn parse_positive_int(value: Option<&str>, name: &str, default: i64) -> Result<i64, ParamError> {
    match value {
        None => Ok(default),
        Some(raw) => {
            let parsed: i64 = raw
                .parse()
                .map_err(|_| ParamError::Invalid(format!("`{name}` must be an integer.")))?;
            if parsed < 1 {
                Err(ParamError::OutOfRange(format!(
                    "`{name}` must be greater than or equal to 1."
                )))
            } else {
                Ok(parsed)
            }
        }
    }
}

/// Build the `AccessDevicesPage` (`{ items, pagination }`) for the (already
/// `deviceStatus`-filtered) device roster: the `page`-th window of `per_page`
/// items plus a pagination envelope (`page`/`perPage`/`totalCount`/`totalPages`,
/// the CamaraSim house convention — mirrors the sibling Network Profiles list).
/// Pure over its input, so it is unit-tested directly.
fn build_access_devices_page(devices: Vec<Value>, params: &DevicesListParams) -> Value {
    let total_count = devices.len() as i64;
    // Ceil-divide; 0 pages when there are no matching devices.
    let total_pages = (total_count + params.per_page - 1) / params.per_page;

    // `page` and `per_page` are both `>= 1`; `saturating_mul` guards against an
    // absurd `page * per_page` overflowing `i64`.
    let start = (params.page - 1).saturating_mul(params.per_page).min(total_count);
    let end = start.saturating_add(params.per_page).min(total_count);
    let items = &devices[start as usize..end as usize];

    json!({
        "items": items,
        "pagination": {
            "page": params.page,
            "perPage": params.per_page,
            "totalCount": total_count,
            "totalPages": total_pages,
        },
    })
}

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed.
fn out_of_range(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, "OUT_OF_RANGE", message).into_response(),
        correlator,
    )
}

/// Extract and validate the optional `networkId` query parameter for
/// `listAccesses`. `Ok(Some(id))` when a valid UUID-shaped `networkId` is
/// present, `Ok(None)` when absent, and `Err(message)` when a present
/// `networkId` is not UUID-shaped (schema `format: uuid`). Pure over its input,
/// so it is unit-tested directly.
fn parse_network_id_filter(query: Option<&str>) -> Result<Option<String>, String> {
    let raw = query.unwrap_or("");
    for (key, value) in url_form_pairs(raw) {
        if key == "networkId" {
            if !is_uuid_shaped(&value) {
                return Err("`networkId` must be a UUID.".to_string());
            }
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// Parse an `application/x-www-form-urlencoded` query string into decoded
/// `(key, value)` pairs. A self-contained decoder (`+` → space, `%XX` → byte),
/// so `listAccesses` needs no query-string dependency (mirrors the sibling
/// Networks API).
fn url_form_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (url_decode(k), url_decode(v))
        })
        .collect()
}

/// Percent/`+` decode a single form component (lossy-UTF-8 for the decoded
/// bytes). Unknown `%` escapes are left verbatim.
fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < b.len() => {
                let hi = (b[i + 1] as char).to_digit(16);
                let lo = (b[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The per-device `DeviceStatus` a submitted device maps to: a reserved-suffix
/// identifier → `DENIED`, any other → `GRANTED`. The identifier is the device's
/// first present of `phoneNumber` / `networkAccessIdentifier` / `ipv6Address` /
/// the IPv4 `publicAddress`. A device with no identifier never reaches here (it
/// is rejected in validation), so the fallback is the happy `GRANTED`. Pure over
/// its input, so it is unit-tested directly.
fn device_status(device: &Value) -> &'static str {
    match device_identifier(device) {
        Some(id) if scenarios::is_reserved_error(&id) => "DENIED",
        _ => "GRANTED",
    }
}

/// The device's primary identifier string, or `None` when the device carries
/// none of the four CAMARA `Device` identifiers. Order: `phoneNumber`,
/// `networkAccessIdentifier`, `ipv6Address`, then the IPv4 `publicAddress`.
fn device_identifier(device: &Value) -> Option<String> {
    for key in ["phoneNumber", "networkAccessIdentifier", "ipv6Address"] {
        if let Some(s) = device.get(key).and_then(Value::as_str) {
            return Some(s.to_string());
        }
    }
    device
        .get("ipv4Address")
        .and_then(|v| v.get("publicAddress"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Validate a single submitted `Device`: it must carry at least one identifier,
/// and a present `phoneNumber` must be E.164. `Ok(())` when valid, `Err(message)`
/// otherwise. Pure over its input, so it is unit-tested directly.
fn validate_device(device: &Value) -> Result<(), String> {
    if !device.is_object() {
        return Err("each `devices` item must be a Device object.".to_string());
    }
    if let Some(phone) = device.get("phoneNumber").and_then(Value::as_str) {
        if !is_e164(phone) {
            return Err("device `phoneNumber` must be E.164 (`+` then 1–15 digits).".to_string());
        }
    }
    if device_identifier(device).is_none() {
        return Err(
            "each device must carry at least one identifier (`phoneNumber`, \
             `networkAccessIdentifier`, `ipv4Address`, or `ipv6Address`)."
                .to_string(),
        );
    }
    Ok(())
}

/// Render an `AccessInfo`: the minted `id`, the associated `networkId`, aggregate
/// `stats`, the per-device `recentAccessDevices`, plus the caller's optional
/// fields echoed back. `sinkCredential` is deliberately omitted (a secret). Pure
/// over its inputs, so it is unit-tested directly.
#[allow(clippy::too_many_arguments)]
fn build_access_info(
    id: &str,
    network_id: &str,
    recent: Vec<Value>,
    total_granted: usize,
    total_denied: usize,
    qos_profiles: Option<&[String]>,
    default_qos_profile: Option<&str>,
    sink: Option<&str>,
) -> Value {
    let mut info = json!({
        "id": id,
        "networkId": network_id,
        "stats": {
            "totalDevices": total_granted + total_denied,
            "totalGranted": total_granted,
            "totalDenied": total_denied,
        },
        "recentAccessDevices": recent,
    });
    let obj = info.as_object_mut().expect("info is an object");
    if let Some(profiles) = qos_profiles {
        obj.insert("qosProfiles".into(), json!(profiles));
    }
    if let Some(v) = default_qos_profile {
        obj.insert("defaultQosProfile".into(), json!(v));
    }
    if let Some(v) = sink {
        obj.insert("sink".into(), json!(v));
    }
    info
}

/// Whether `s` is UUID-shaped: five hyphen-separated hex groups of lengths
/// `8-4-4-4-12` (the schema's `format: uuid`). Mirrors the other UUID-keyed APIs.
fn is_uuid_shaped(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// Whether `s` is an E.164 phone number: a leading `+` then 1–15 decimal digits.
fn is_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    (1..=15).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        crate::errors::CamaraError::invalid_argument(message).into_response(),
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

    // A UUID whose trailing three digits are `d` (zero-padded), so a test can aim
    // at a specific reserved suffix.
    fn uuid_ending(d: u16) -> String {
        format!("00000000-0000-4000-8000-000000000{d:03}")
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_shape_matches_the_schema() {
        assert!(is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid_shaped(&uuid_ending(1)));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa")); // 11 in last group
        assert!(!is_uuid_shaped("zzzzzzzz-5717-4562-b3fc-2c963f66afa6")); // non-hex
    }

    #[test]
    fn e164_shape_check() {
        assert!(is_e164("+123456789012"));
        assert!(is_e164("+1"));
        assert!(!is_e164("123456789012")); // no +
        assert!(!is_e164("+")); // no digits
        assert!(!is_e164("+12345678901234567")); // >15 digits
        assert!(!is_e164("+12ab")); // non-digit
    }

    #[test]
    fn device_identifier_prefers_phone_then_nai_then_ipv6_then_ipv4() {
        assert_eq!(
            device_identifier(&json!({ "phoneNumber": "+123", "ipv6Address": "::1" })),
            Some("+123".to_string())
        );
        assert_eq!(
            device_identifier(&json!({ "networkAccessIdentifier": "nai@x" })),
            Some("nai@x".to_string())
        );
        assert_eq!(
            device_identifier(&json!({ "ipv6Address": "2001:db8::1" })),
            Some("2001:db8::1".to_string())
        );
        assert_eq!(
            device_identifier(&json!({ "ipv4Address": { "publicAddress": "1.1.1.1" } })),
            Some("1.1.1.1".to_string())
        );
        assert_eq!(device_identifier(&json!({ "foo": "bar" })), None);
    }

    #[test]
    fn device_status_denies_reserved_suffix_grants_otherwise() {
        // A phoneNumber ending in a reserved error suffix → DENIED.
        assert_eq!(device_status(&json!({ "phoneNumber": "+123456789404" })), "DENIED");
        // A normal number → GRANTED.
        assert_eq!(device_status(&json!({ "phoneNumber": "+123456789012" })), "GRANTED");
        // NAI with no digits → GRANTED (no reserved suffix).
        assert_eq!(device_status(&json!({ "networkAccessIdentifier": "user@op" })), "GRANTED");
    }

    #[test]
    fn validate_device_requires_an_identifier_and_e164_phone() {
        assert!(validate_device(&json!({ "phoneNumber": "+123456789012" })).is_ok());
        assert!(validate_device(&json!({ "ipv6Address": "2001:db8::1" })).is_ok());
        assert!(validate_device(&json!({ "foo": "bar" })).is_err()); // no identifier
        assert!(validate_device(&json!({ "phoneNumber": "12345" })).is_err()); // not E.164
        assert!(validate_device(&json!("not-an-object")).is_err());
    }

    #[test]
    fn build_access_info_shapes_stats_and_omits_secret() {
        let recent = vec![json!({ "device": { "phoneNumber": "+1" }, "status": "GRANTED" })];
        let info = build_access_info(
            "acc-1",
            "net-1",
            recent,
            1,
            0,
            Some(&["profile-a".to_string()]),
            Some("profile-a"),
            None,
        );
        assert_eq!(info["id"], "acc-1");
        assert_eq!(info["networkId"], "net-1");
        assert_eq!(info["stats"]["totalDevices"], 1);
        assert_eq!(info["stats"]["totalGranted"], 1);
        assert_eq!(info["stats"]["totalDenied"], 0);
        assert_eq!(info["qosProfiles"][0], "profile-a");
        assert_eq!(info["defaultQosProfile"], "profile-a");
        assert!(info.get("sink").is_none());
        assert!(info.get("sinkCredential").is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(routes())
    }

    async fn mint_token(scope: &str) -> String {
        let form = format!("grant_type=client_credentials&client_id=dna-client&scope={scope}");
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/oauth2/token")
                    .header("host", HOST)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "token mint should succeed");
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        body["access_token"].as_str().unwrap().to_string()
    }

    async fn post_access(token: &str, body: Value) -> (StatusCode, HeaderMap, Value) {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dedicated-network-accesses/vwip/accesses")
                    .header("host", HOST)
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .header("x-correlator", "corr-123")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn happy_path_creates_an_access_and_grants_devices() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, body) = post_access(
            &token,
            json!({
                "networkId": uuid_ending(12),
                "devices": [
                    { "phoneNumber": "+123456789012" },
                    { "networkAccessIdentifier": "user@operator" }
                ],
                "qosProfiles": ["QOS_A", "QOS_B"],
                "defaultQosProfile": "QOS_A"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-123");
        // A minted UUID id + echoed networkId.
        assert!(is_uuid_shaped(body["id"].as_str().unwrap()));
        assert_eq!(body["networkId"], uuid_ending(12));
        assert_eq!(body["stats"]["totalDevices"], 2);
        assert_eq!(body["stats"]["totalGranted"], 2);
        assert_eq!(body["stats"]["totalDenied"], 0);
        assert_eq!(body["recentAccessDevices"].as_array().unwrap().len(), 2);
        assert_eq!(body["recentAccessDevices"][0]["status"], "GRANTED");
        assert_eq!(body["qosProfiles"][1], "QOS_B");
        assert_eq!(body["defaultQosProfile"], "QOS_A");
    }

    #[tokio::test]
    async fn devices_can_be_omitted_and_stats_are_zero() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _h, body) =
            post_access(&token, json!({ "networkId": uuid_ending(12) })).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["stats"]["totalDevices"], 0);
        assert_eq!(body["recentAccessDevices"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn a_reserved_suffix_device_is_denied_and_counted() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _h, body) = post_access(
            &token,
            json!({
                "networkId": uuid_ending(12),
                "devices": [
                    { "phoneNumber": "+123456789012" }, // GRANTED
                    { "phoneNumber": "+123456789404" }  // reserved → DENIED
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["stats"]["totalDevices"], 2);
        assert_eq!(body["stats"]["totalGranted"], 1);
        assert_eq!(body["stats"]["totalDenied"], 1);
        // The denied device is the one ending in 404.
        let recent = body["recentAccessDevices"].as_array().unwrap();
        let denied = recent.iter().find(|d| d["status"] == "DENIED").unwrap();
        assert_eq!(denied["device"]["phoneNumber"], "+123456789404");
    }

    #[tokio::test]
    async fn reserved_network_id_suffix_selects_a_canonical_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, body) =
            post_access(&token, json!({ "networkId": uuid_ending(404) })).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], 404);
        assert_eq!(body["code"], "NOT_FOUND");
        // Correlator echoed on the error path too.
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-123");
    }

    #[tokio::test]
    async fn malformed_requests_are_400() {
        let token = mint_token(CREATE_SCOPE).await;
        // Missing networkId.
        let (s, _h, _b) = post_access(&token, json!({ "devices": [{ "phoneNumber": "+1" }] })).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // Non-UUID networkId.
        let (s, _h, _b) = post_access(&token, json!({ "networkId": "nope" })).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // Empty devices array.
        let (s, _h, _b) =
            post_access(&token, json!({ "networkId": uuid_ending(12), "devices": [] })).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // Device with no identifier.
        let (s, _h, _b) = post_access(
            &token,
            json!({ "networkId": uuid_ending(12), "devices": [{ "foo": "bar" }] }),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // Non-E.164 phoneNumber.
        let (s, _h, _b) = post_access(
            &token,
            json!({ "networkId": uuid_ending(12), "devices": [{ "phoneNumber": "12345" }] }),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // Unknown top-level field.
        let (s, _h, _b) =
            post_access(&token, json!({ "networkId": uuid_ending(12), "bogus": true })).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn qos_profiles_bounds_are_enforced() {
        let token = mint_token(CREATE_SCOPE).await;
        // Empty qosProfiles array → 400.
        let (s, _h, _b) = post_access(
            &token,
            json!({ "networkId": uuid_ending(12), "qosProfiles": [] }),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        // >32 profiles → 400.
        let many: Vec<String> = (0..33).map(|i| format!("P{i}")).collect();
        let (s, _h, _b) = post_access(
            &token,
            json!({ "networkId": uuid_ending(12), "qosProfiles": many }),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn non_https_sink_is_rejected() {
        let token = mint_token(CREATE_SCOPE).await;
        let (s, _h, _b) = post_access(
            &token,
            json!({ "networkId": uuid_ending(12), "sink": "http://insecure.example/cb" }),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other:scope").await;
        let (s, _h, _b) = post_access(&token, json!({ "networkId": uuid_ending(12) })).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/dedicated-network-accesses/vwip/accesses")
                    .header("host", HOST)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({ "networkId": uuid_ending(12) })).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // --- readAccess --------------------------------------------------------

    async fn get_access(token: &str, access_id: &str) -> (StatusCode, HeaderMap, Value) {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/dedicated-network-accesses/vwip/accesses/{access_id}"
                    ))
                    .header("host", HOST)
                    .header("authorization", format!("Bearer {token}"))
                    .header("x-correlator", "corr-read")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn create_then_read_the_access_back() {
        // Create an access, then read it back by its minted id.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [{ "phoneNumber": "+123456789012" }],
                "defaultQosProfile": "QOS_A"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let read = mint_token(READ_SCOPE).await;
        let (status, headers, body) = get_access(&read, id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-read");
        // The stored AccessInfo is returned verbatim.
        assert_eq!(body, created);
    }

    #[tokio::test]
    async fn unknown_access_is_not_found() {
        let read = mint_token(READ_SCOPE).await;
        let (status, headers, body) = get_access(&read, &uuid_ending(999)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], 404);
        assert_eq!(body["code"], "NOT_FOUND");
        // Correlator echoed on the error path too.
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-read");
    }

    #[tokio::test]
    async fn read_without_the_scope_is_forbidden() {
        // A create-scoped token cannot read (distinct scope), so `readAccess`
        // requires its own `…:accesses:read` scope.
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _h, _b) = get_access(&token, &uuid_ending(12)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn read_without_a_token_is_unauthenticated() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/dedicated-network-accesses/vwip/accesses/{}",
                        uuid_ending(12)
                    ))
                    .header("host", HOST)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // --- listAccesses ------------------------------------------------------

    #[test]
    fn network_id_filter_validates_uuid_shape() {
        assert_eq!(parse_network_id_filter(None), Ok(None));
        assert_eq!(parse_network_id_filter(Some("")), Ok(None));
        assert_eq!(
            parse_network_id_filter(Some(&format!("networkId={}", uuid_ending(7)))),
            Ok(Some(uuid_ending(7)))
        );
        // Unknown params ignored; only networkId is honoured.
        assert_eq!(parse_network_id_filter(Some("foo=bar")), Ok(None));
        // A present but non-UUID networkId is rejected.
        assert!(parse_network_id_filter(Some("networkId=nope")).is_err());
    }

    async fn list_accesses_req(
        token: Option<&str>,
        query: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let uri = match query {
            Some(q) => format!("/dedicated-network-accesses/vwip/accesses?{q}"),
            None => "/dedicated-network-accesses/vwip/accesses".to_string(),
        };
        let mut builder = Request::builder().method("GET").uri(uri).header("host", HOST);
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
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn list_filtered_by_network_id_returns_only_matching_accesses() {
        // The store is process-global and shared across parallel tests, so we
        // isolate by filtering on a networkId no other test uses.
        let net = uuid_ending(51); // non-reserved suffix
        let create = mint_token(CREATE_SCOPE).await;
        for _ in 0..2 {
            let (status, _h, _b) = post_access(&create, json!({ "networkId": net })).await;
            assert_eq!(status, StatusCode::CREATED);
        }

        let read = mint_token(READ_SCOPE).await;
        let (status, headers, body) =
            list_accesses_req(Some(&read), Some(&format!("networkId={net}")), Some("corr-list"))
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-list");
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 2, "exactly the two accesses on this networkId");
        assert!(items.iter().all(|a| a["networkId"] == net));
    }

    #[tokio::test]
    async fn list_with_unknown_network_id_is_an_empty_array() {
        let read = mint_token(READ_SCOPE).await;
        // A valid UUID that no access uses → empty array (a list never 404s).
        let (status, _h, body) =
            list_accesses_req(Some(&read), Some(&format!("networkId={}", uuid_ending(852))), None)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_with_non_uuid_network_id_is_400() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _h, _b) = list_accesses_req(Some(&read), Some("networkId=not-a-uuid"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_without_the_scope_is_forbidden() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _h, _b) = list_accesses_req(Some(&token), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_without_a_token_is_unauthenticated() {
        let (status, _h, _b) = list_accesses_req(None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- deleteAccess ------------------------------------------------------

    async fn delete_access_req(
        token: Option<&str>,
        access_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!(
                "/dedicated-network-accesses/vwip/accesses/{access_id}"
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
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn delete_evicts_the_access_single_use_then_read_is_404() {
        // Create, then delete → 204; second delete → 404; a read is then 404 too.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) =
            post_access(&create, json!({ "networkId": uuid_ending(12) })).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, body) = delete_access_req(Some(&del), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-del");
        assert_eq!(body, Value::Null, "204 carries no body");

        // Single-use: a second delete is 404.
        let (status, _h, body) = delete_access_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // And the access no longer reads back.
        let read = mint_token(READ_SCOPE).await;
        let (status, _h, _b) = get_access(&read, &id).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_unknown_access_is_404() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, body) =
            delete_access_req(Some(&del), &uuid_ending(777), Some("corr-del")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["status"], 404);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-del");
    }

    #[tokio::test]
    async fn delete_without_the_delete_scope_is_forbidden() {
        // A read-scoped token cannot delete (distinct scope).
        let read = mint_token(READ_SCOPE).await;
        let (status, _h, _b) = delete_access_req(Some(&read), &uuid_ending(12), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn delete_without_a_token_is_unauthenticated() {
        let (status, _h, _b) = delete_access_req(None, &uuid_ending(12), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- listDevices -------------------------------------------------------

    #[test]
    fn devices_list_params_default_and_validate() {
        // Absent → schema defaults.
        let p = parse_devices_list_params(None).unwrap();
        assert_eq!((p.page, p.per_page), (1, 10));
        assert_eq!(p.device_status, None);
        // Explicit values are honoured; unknown params ignored.
        let p = parse_devices_list_params(Some("page=2&perPage=5&foo=bar&deviceStatus=GRANTED"))
            .unwrap();
        assert_eq!((p.page, p.per_page), (2, 5));
        assert_eq!(p.device_status.as_deref(), Some("GRANTED"));
        // Non-integer page → Invalid; below-minimum perPage → OutOfRange.
        assert_eq!(
            parse_devices_list_params(Some("page=abc")),
            Err(ParamError::Invalid("`page` must be an integer.".to_string()))
        );
        assert!(matches!(
            parse_devices_list_params(Some("perPage=0")),
            Err(ParamError::OutOfRange(_))
        ));
        // Unknown deviceStatus → Invalid.
        assert!(matches!(
            parse_devices_list_params(Some("deviceStatus=BOGUS")),
            Err(ParamError::Invalid(_))
        ));
    }

    #[test]
    fn access_devices_page_windows_and_envelopes() {
        let devices: Vec<Value> = (0..3)
            .map(|i| json!({ "device": { "phoneNumber": format!("+1000000000{i:02}") }, "status": "GRANTED" }))
            .collect();
        // Page 1 of perPage 2 → first two items, 2 pages total.
        let page = build_access_devices_page(
            devices.clone(),
            &DevicesListParams { page: 1, per_page: 2, device_status: None },
        );
        assert_eq!(page["items"].as_array().unwrap().len(), 2);
        assert_eq!(page["pagination"]["page"], 1);
        assert_eq!(page["pagination"]["perPage"], 2);
        assert_eq!(page["pagination"]["totalCount"], 3);
        assert_eq!(page["pagination"]["totalPages"], 2);
        // Page 2 → the remaining single item.
        let page = build_access_devices_page(
            devices.clone(),
            &DevicesListParams { page: 2, per_page: 2, device_status: None },
        );
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        // A page beyond the end → empty items, envelope still correct.
        let page = build_access_devices_page(
            devices,
            &DevicesListParams { page: 9, per_page: 2, device_status: None },
        );
        assert_eq!(page["items"].as_array().unwrap().len(), 0);
        assert_eq!(page["pagination"]["totalCount"], 3);
    }

    async fn get_devices(
        token: Option<&str>,
        access_id: &str,
        query: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let uri = match query {
            Some(q) => format!("/dedicated-network-accesses/vwip/accesses/{access_id}/devices?{q}"),
            None => format!("/dedicated-network-accesses/vwip/accesses/{access_id}/devices"),
        };
        let mut builder = Request::builder().method("GET").uri(uri).header("host", HOST);
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
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn create_then_list_the_devices_of_an_access() {
        // Create an access with one granted + one denied device, then list them.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [
                    { "phoneNumber": "+123456789012" }, // GRANTED
                    { "phoneNumber": "+123456789404" }  // reserved → DENIED
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let read = mint_token(DEVICES_READ_SCOPE).await;
        let (status, headers, body) = get_devices(Some(&read), id, None, Some("corr-dev")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-dev");
        // The page carries both AccessDevices and a pagination envelope.
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(body["pagination"]["totalCount"], 2);
        assert_eq!(body["pagination"]["page"], 1);
        assert_eq!(body["pagination"]["perPage"], 10);
        assert_eq!(body["pagination"]["totalPages"], 1);
        // Items are the AccessDevice roster the create recorded.
        assert!(items.iter().any(|d| d["status"] == "GRANTED"));
        assert!(items.iter().any(|d| d["status"] == "DENIED"));
    }

    #[tokio::test]
    async fn list_devices_filters_by_device_status() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [
                    { "phoneNumber": "+123456789012" }, // GRANTED
                    { "phoneNumber": "+123456789404" }  // DENIED
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let read = mint_token(DEVICES_READ_SCOPE).await;
        // Filter to just the denied device.
        let (status, _h, body) = get_devices(Some(&read), id, Some("deviceStatus=DENIED"), None).await;
        assert_eq!(status, StatusCode::OK);
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["status"], "DENIED");
        assert_eq!(items[0]["device"]["phoneNumber"], "+123456789404");
        assert_eq!(body["pagination"]["totalCount"], 1);
        // A status the roster has none of → empty page (never 404s).
        let (status, _h, body) =
            get_devices(Some(&read), id, Some("deviceStatus=REQUESTED"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn list_devices_paginates() {
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [
                    { "phoneNumber": "+123456780001" },
                    { "phoneNumber": "+123456780002" },
                    { "phoneNumber": "+123456780003" }
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let read = mint_token(DEVICES_READ_SCOPE).await;
        let (status, _h, body) = get_devices(Some(&read), id, Some("page=2&perPage=2"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["items"].as_array().unwrap().len(), 1, "second page holds the third device");
        assert_eq!(body["pagination"]["page"], 2);
        assert_eq!(body["pagination"]["totalCount"], 3);
        assert_eq!(body["pagination"]["totalPages"], 2);
    }

    #[tokio::test]
    async fn list_devices_bad_query_is_400_before_the_store() {
        let read = mint_token(DEVICES_READ_SCOPE).await;
        // Non-integer page on an unknown access → 400 (query validated first), not 404.
        let (status, _h, body) =
            get_devices(Some(&read), &uuid_ending(999), Some("page=abc"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // perPage below the minimum → 400 OUT_OF_RANGE.
        let (status, _h, body) =
            get_devices(Some(&read), &uuid_ending(999), Some("perPage=0"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
        // Unknown deviceStatus → 400 INVALID_ARGUMENT.
        let (status, _h, body) =
            get_devices(Some(&read), &uuid_ending(999), Some("deviceStatus=BOGUS"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn list_devices_unknown_access_is_404() {
        let read = mint_token(DEVICES_READ_SCOPE).await;
        let (status, headers, body) =
            get_devices(Some(&read), &uuid_ending(888), None, Some("corr-dev")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-dev");
    }

    #[tokio::test]
    async fn list_devices_without_the_scope_is_forbidden() {
        // The access read scope does not grant the devices read scope.
        let token = mint_token(READ_SCOPE).await;
        let (status, _h, _b) = get_devices(Some(&token), &uuid_ending(12), None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_devices_without_a_token_is_unauthenticated() {
        let (status, _h, _b) = get_devices(None, &uuid_ending(12), None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- addDevicesToAccess ------------------------------------------------

    async fn post_add_devices(
        token: Option<&str>,
        access_id: &str,
        body: Value,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!(
                "/dedicated-network-accesses/vwip/accesses/{access_id}/devices/add"
            ))
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn add_devices_appends_to_the_roster_and_updates_stats() {
        // Create an access with one granted device …
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [{ "phoneNumber": "+123456789012" }] // GRANTED
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // … then add two more (one granted, one reserved → denied). The body is a
        // bare JSON array (AddDevicesRequest), not an object.
        let add = mint_token(DEVICES_ADD_SCOPE).await;
        let (status, headers, body) = post_add_devices(
            Some(&add),
            &id,
            json!([
                { "networkAccessIdentifier": "user@operator" }, // GRANTED
                { "phoneNumber": "+123456789404" }              // reserved → DENIED
            ]),
            Some("corr-add"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-add");
        // The 201 body is the AccessDevices for the added devices only.
        let added = body.as_array().unwrap();
        assert_eq!(added.len(), 2);
        assert!(added.iter().any(|d| d["status"] == "GRANTED"));
        let denied = added.iter().find(|d| d["status"] == "DENIED").unwrap();
        assert_eq!(denied["device"]["phoneNumber"], "+123456789404");

        // The roster + stats now reflect the create (1) plus the two added (3).
        let read = mint_token(READ_SCOPE).await;
        let (status, _h, info) = get_access(&read, &id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(info["stats"]["totalDevices"], 3);
        assert_eq!(info["stats"]["totalGranted"], 2);
        assert_eq!(info["stats"]["totalDenied"], 1);
        assert_eq!(info["recentAccessDevices"].as_array().unwrap().len(), 3);

        // listDevices sees the extended roster too.
        let list = mint_token(DEVICES_READ_SCOPE).await;
        let (status, _h, page) = get_devices(Some(&list), &id, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["pagination"]["totalCount"], 3);
    }

    #[tokio::test]
    async fn add_devices_rejects_a_malformed_body_before_the_store() {
        let add = mint_token(DEVICES_ADD_SCOPE).await;
        // A non-array body (an object) → 400 INVALID_ARGUMENT, even on an unknown
        // access (body validated before the store lookup).
        let (status, _h, body) = post_add_devices(
            Some(&add),
            &uuid_ending(999),
            json!({ "devices": [{ "phoneNumber": "+1" }] }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Empty array (below minItems 1) → 400.
        let (status, _h, _b) = post_add_devices(Some(&add), &uuid_ending(999), json!([]), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // More than 100 devices → 400.
        let many: Vec<Value> = (0..101).map(|_| json!({ "networkAccessIdentifier": "u@o" })).collect();
        let (status, _h, _b) =
            post_add_devices(Some(&add), &uuid_ending(999), json!(many), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // A device with no identifier → 400.
        let (status, _h, _b) =
            post_add_devices(Some(&add), &uuid_ending(999), json!([{ "foo": "bar" }]), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // A non-E.164 phoneNumber → 400.
        let (status, _h, _b) = post_add_devices(
            Some(&add),
            &uuid_ending(999),
            json!([{ "phoneNumber": "12345" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn add_devices_to_an_unknown_access_is_404() {
        // A well-formed body on a never-created access → 404 NOT_FOUND (the minted
        // accessId has no reserved-suffix plane), correlator echoed.
        let add = mint_token(DEVICES_ADD_SCOPE).await;
        let (status, headers, body) = post_add_devices(
            Some(&add),
            &uuid_ending(777),
            json!([{ "phoneNumber": "+123456789012" }]),
            Some("corr-add"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-add");
    }

    #[tokio::test]
    async fn add_devices_without_the_scope_is_forbidden() {
        // The devices *read* scope does not grant the devices *add* scope.
        let token = mint_token(DEVICES_READ_SCOPE).await;
        let (status, _h, _b) = post_add_devices(
            Some(&token),
            &uuid_ending(12),
            json!([{ "phoneNumber": "+123456789012" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn add_devices_without_a_token_is_unauthenticated() {
        let (status, _h, _b) = post_add_devices(
            None,
            &uuid_ending(12),
            json!([{ "phoneNumber": "+123456789012" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- removeDevicesFromAccess -------------------------------------------

    async fn post_remove_devices(
        token: Option<&str>,
        access_id: &str,
        body: Value,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!(
                "/dedicated-network-accesses/vwip/accesses/{access_id}/devices/remove"
            ))
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, json)
    }

    #[tokio::test]
    async fn remove_devices_evicts_matching_entries_and_updates_stats() {
        // Create an access with three devices: two granted, one reserved → denied.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [
                    { "phoneNumber": "+123456789012" },          // GRANTED
                    { "networkAccessIdentifier": "user@operator" }, // GRANTED
                    { "phoneNumber": "+123456789404" }           // reserved → DENIED
                ]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["stats"]["totalDevices"], 3);

        // Remove one granted and the denied one (identified by their identifiers,
        // not byte-identical Device objects). Body is a bare JSON array.
        let remove = mint_token(DEVICES_REMOVE_SCOPE).await;
        let (status, headers, body) = post_remove_devices(
            Some(&remove),
            &id,
            json!([
                { "phoneNumber": "+123456789012" },
                { "phoneNumber": "+123456789404" }
            ]),
            Some("corr-rm"),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-rm");
        assert!(body.is_null(), "204 carries no body");

        // Only the NAI-granted device survives; stats recomputed to match.
        let read = mint_token(READ_SCOPE).await;
        let (status, _h, info) = get_access(&read, &id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(info["stats"]["totalDevices"], 1);
        assert_eq!(info["stats"]["totalGranted"], 1);
        assert_eq!(info["stats"]["totalDenied"], 0);
        let roster = info["recentAccessDevices"].as_array().unwrap();
        assert_eq!(roster.len(), 1);
        assert_eq!(
            roster[0]["device"]["networkAccessIdentifier"],
            "user@operator"
        );
    }

    #[tokio::test]
    async fn remove_devices_is_idempotent_for_absent_devices() {
        // Create an access with one device.
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _h, created) = post_access(
            &create,
            json!({
                "networkId": uuid_ending(12),
                "devices": [{ "phoneNumber": "+123456789012" }]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // Removing a device that was never in the roster is a no-op → still 204.
        let remove = mint_token(DEVICES_REMOVE_SCOPE).await;
        let (status, _h, _b) = post_remove_devices(
            Some(&remove),
            &id,
            json!([{ "phoneNumber": "+199999999999" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // The original roster is untouched.
        let read = mint_token(READ_SCOPE).await;
        let (_s, _h, info) = get_access(&read, &id).await;
        assert_eq!(info["stats"]["totalDevices"], 1);
        assert_eq!(info["recentAccessDevices"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn remove_devices_rejects_a_malformed_body_before_the_store() {
        let remove = mint_token(DEVICES_REMOVE_SCOPE).await;
        // Non-array body → 400, even for an unknown access (body validated first).
        let (status, _h, body) = post_remove_devices(
            Some(&remove),
            &uuid_ending(999),
            json!({ "devices": [{ "phoneNumber": "+1" }] }),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Empty array (below minItems 1) → 400.
        let (status, _h, _b) =
            post_remove_devices(Some(&remove), &uuid_ending(999), json!([]), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // More than 100 devices → 400.
        let many: Vec<Value> = (0..101).map(|_| json!({ "networkAccessIdentifier": "u@o" })).collect();
        let (status, _h, _b) =
            post_remove_devices(Some(&remove), &uuid_ending(999), json!(many), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // A device with no identifier → 400.
        let (status, _h, _b) = post_remove_devices(
            Some(&remove),
            &uuid_ending(999),
            json!([{ "foo": "bar" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // A non-E.164 phoneNumber → 400.
        let (status, _h, _b) = post_remove_devices(
            Some(&remove),
            &uuid_ending(999),
            json!([{ "phoneNumber": "12345" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn remove_devices_from_an_unknown_access_is_404() {
        // A well-formed body on a never-created access → 404 NOT_FOUND (the minted
        // accessId has no reserved-suffix plane), correlator echoed.
        let remove = mint_token(DEVICES_REMOVE_SCOPE).await;
        let (status, headers, body) = post_remove_devices(
            Some(&remove),
            &uuid_ending(777),
            json!([{ "phoneNumber": "+123456789012" }]),
            Some("corr-rm"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-rm");
    }

    #[tokio::test]
    async fn remove_devices_without_the_scope_is_forbidden() {
        // The devices *add* scope does not grant the devices *remove* scope.
        let token = mint_token(DEVICES_ADD_SCOPE).await;
        let (status, _h, _b) = post_remove_devices(
            Some(&token),
            &uuid_ending(12),
            json!([{ "phoneNumber": "+123456789012" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn remove_devices_without_a_token_is_unauthenticated() {
        let (status, _h, _b) = post_remove_devices(
            None,
            &uuid_ending(12),
            json!([{ "phoneNumber": "+123456789012" }]),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
