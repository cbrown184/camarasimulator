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
//! - `GET /network-access-devices/vwip/reboot-requests/{rebootRequestId}` — read a
//!   created reboot request back by its opaque id (operationId `getRebootRequest`).
//! - `DELETE /network-access-devices/vwip/reboot-requests/{rebootRequestId}` —
//!   cancel/delete a created reboot request, evicting it from the store
//!   (operationId `deleteRebootRequest`). The `PATCH` (update) leg is a later slice.
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
        .route(
            "/network-access-devices/vwip/reboot-requests/:reboot_request_id",
            get(get_reboot_request)
                .patch(update_reboot_request)
                .delete(delete_reboot_request),
        )
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

/// `GET /network-access-devices/vwip/reboot-requests/{rebootRequestId}` — read a
/// previously created reboot request by its id (operationId `getRebootRequest`).
///
/// This is the read leg of the **stateful** reboot-request lifecycle: it returns
/// the `RebootRequest` persisted by `createRebootRequest` under the minted `id`,
/// verbatim. The `rebootRequestId` is server-minted and opaque to the caller, so
/// — unlike the subject-keyed device operations — it is **not** a reserved-error
/// scenario plane; the **store state** is the sole control plane (docs/DESIGN.md
/// §7, mirroring QoS Provisioning's `getQosAssignmentById`):
///
/// - **A stored id** → `200` with that `RebootRequest`.
/// - **Any other id** (never created, or already deleted) → `404 NOT_FOUND`.
///
/// Reboot requests are not scoped per subscriber, so CAMARA's `sub`-ownership
/// check on the read is not enforced (a documented cut, mirroring Blockchain
/// Public Address). Requires the `network-access-devices:reboot` scope.
async fn get_reboot_request(
    claims: Claims,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the only control plane: a stored id reads back, anything
    // else (never created / already deleted) is a 404.
    match store::get(&id) {
        Some(resource) => {
            with_correlator((StatusCode::OK, Json(resource)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No reboot request found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /network-access-devices/vwip/reboot-requests/{rebootRequestId}` —
/// cancel/delete a previously created reboot request (operationId
/// `deleteRebootRequest`).
///
/// This is the delete leg of the **stateful** reboot-request lifecycle: it evicts
/// the `RebootRequest` persisted by `createRebootRequest` from the shared
/// in-memory [`super::store`]. Like the read leg, the `rebootRequestId` is
/// server-minted and opaque, so it is **not** a reserved-error scenario plane; the
/// **store state** is the sole control plane (docs/DESIGN.md §7, mirroring QoD's
/// `deleteSession` / Traffic Influence's `deleteTrafficInfluence`):
///
/// - **A stored id** → `204 No Content` (single-use eviction — a later
///   `getRebootRequest`/`deleteRebootRequest` for the same id is a `404`).
/// - **Any other id** (never created, or already deleted) → `404 NOT_FOUND`.
///
/// Reboot requests are not scoped per subscriber, so CAMARA's `sub`-ownership
/// check on the delete is not enforced (a documented cut, mirroring the read leg).
/// Requires the `network-access-devices:reboot` scope.
async fn delete_reboot_request(
    claims: Claims,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the only control plane: an atomic single-use remove →
    // `204` when a request was present, `404` when nothing was there to delete.
    match store::remove(&id) {
        Some(_) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No reboot request found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// An ordered list of merge operations to apply to the stored `RebootRequest`.
/// `Some(v)` sets a field, `None` clears it. Built by [`validate_reboot_patch`],
/// applied by [`apply_reboot_merge`].
type Merge = Vec<(&'static str, Option<Value>)>;

/// `PATCH /network-access-devices/vwip/reboot-requests/{rebootRequestId}` — update
/// a previously created reboot request (operationId `updateRebootRequest`).
///
/// This is the update leg of the **stateful** reboot-request lifecycle. The body
/// is a `merge-patch+json` document (RFC 7386, the CAMARA convention, mirroring
/// Traffic Influence's `patchTrafficInfluence`) over the **mutable** fields: a
/// supplied `atTime` / `message` is replaced, an explicit `null` clears it, and
/// the identity / target / audit fields (`id`, `devices`, `createdAt`,
/// `modifiedAt`) plus any unknown key are ignored. On a successful update
/// `modifiedAt` is bumped to now.
///
/// Two control planes (docs/DESIGN.md §7). The request **body** is validated
/// first (a malformed `atTime` or an over-long `message` → `400 INVALID_ARGUMENT`,
/// before the store is touched). Then the opaque, server-minted id selects the
/// **store state**, and the stored request's **schedule state** gates the update:
///
/// - **Unknown / already-deleted id** → `404 NOT_FOUND`.
/// - **A pending *scheduled* reboot** (the stored request carries an `atTime`) →
///   `200` with the updated `RebootRequest`; the change persists (a later
///   `getRebootRequest` sees it).
/// - **An *immediate* reboot** (no `atTime` — it has already fired) → `409
///   NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE`: an in-progress/completed reboot
///   cannot be modified. CamaraSim runs no reboot engine, so "already fired" is
///   modelled by the absence of a future `atTime` rather than a live status
///   transition (a documented simplification of CAMARA's scheduled-reboot
///   semantics). The store is left unchanged.
///
/// Reboot requests are not scoped per subscriber, so CAMARA's `sub`-ownership
/// check on the update is not enforced (a documented cut, mirroring the read /
/// delete legs). Requires the `network-access-devices:reboot` scope.
async fn update_reboot_request(
    claims: Claims,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is a JSON merge-patch document; parse then validate (400 before store).
    // An empty body is a valid no-op patch (an empty merge-patch object).
    let value: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return invalid_argument("Request body is not valid JSON.", &correlator),
        }
    };
    let merge = match validate_reboot_patch(&value) {
        Ok(m) => m,
        Err(message) => return invalid_argument(&message, &correlator),
    };

    // Atomic get-modify-write. The schedule state gates the update: a stored
    // request with an `atTime` (pending scheduled reboot) is modifiable; one
    // without (an immediate reboot, already fired) declines with a 409. The
    // decline happens before any mutation, so the store is left unchanged.
    let now = rfc3339_utc(now_unix_secs());
    match store::update_with(&id, |resource| {
        if resource.get("atTime").is_none() {
            return Err(()); // immediate reboot → INCOMPATIBLE_STATE
        }
        apply_reboot_merge(resource, &merge);
        if let Some(obj) = resource.as_object_mut() {
            obj.insert("modifiedAt".to_string(), json!(now));
        }
        Ok(())
    }) {
        Some(Ok(updated)) => {
            with_correlator((StatusCode::OK, Json(updated)).into_response(), &correlator)
        }
        Some(Err(())) => with_correlator(
            CamaraError::new(
                StatusCode::CONFLICT,
                "NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE",
                "The reboot request has already started and can no longer be modified.",
            )
            .into_response(),
            &correlator,
        ),
        None => with_correlator(
            CamaraError::not_found("No reboot request found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// Validate a `merge-patch+json` body against the **mutable** `RebootRequest`
/// fields (`atTime`, `message`), returning the merge operations to apply or a
/// `400` message. Identity / target / audit fields (`id`, `devices`, `createdAt`,
/// `modifiedAt`) and any unknown key are ignored (mirrors Traffic Influence's
/// `validate_patch`). Pure over its input, so it is unit-testable exactly.
fn validate_reboot_patch(body: &Value) -> Result<Merge, String> {
    let obj = body
        .as_object()
        .ok_or_else(|| "Request body must be a JSON object.".to_string())?;
    let mut merge: Merge = Vec::new();
    for (key, value) in obj {
        match key.as_str() {
            // Reschedule (or, with null, clear → immediate).
            "atTime" => match value {
                Value::Null => merge.push(("atTime", None)),
                Value::String(s) if is_rfc3339(s) => merge.push(("atTime", Some(json!(s)))),
                _ => {
                    return Err(
                        "`atTime` must be an RFC 3339 date-time (e.g. 2024-01-01T14:00:00Z) or null."
                            .to_string(),
                    )
                }
            },
            "message" => match value {
                Value::Null => merge.push(("message", None)),
                Value::String(s) if s.chars().count() <= 255 => {
                    merge.push(("message", Some(json!(s))))
                }
                _ => {
                    return Err("`message` must be a string of at most 255 characters or null."
                        .to_string())
                }
            },
            // Identity / target / audit / unknown keys are ignored (merge-patch).
            _ => {}
        }
    }
    Ok(merge)
}

/// Apply the validated merge operations to the stored `RebootRequest` in place:
/// `Some(v)` sets a field, `None` removes it. Pure over its inputs.
fn apply_reboot_merge(resource: &mut Value, merge: &Merge) {
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

    // --- getRebootRequest (GET /reboot-requests/{id}) ----------------------

    /// GET a reboot request by id with an optional Bearer token and `x-correlator`.
    async fn get_reboot_request_by_id(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/network-access-devices/vwip/reboot-requests/{id}"))
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
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn created_reboot_request_reads_back_verbatim() {
        let token = mint_token(SCOPE).await;
        let (status, _, created) = post_reboot_request(Some(&token), Some("{}"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // The read leg returns the persisted resource verbatim.
        let (status, _, read) = get_reboot_request_by_id(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read, created);
    }

    #[tokio::test]
    async fn get_unknown_reboot_request_is_not_found() {
        let token = mint_token(SCOPE).await;
        // A well-formed UUID that was never created → 404 (store state is the plane).
        let (status, _, resp) = get_reboot_request_by_id(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_reboot_request_requires_the_scope_and_a_token() {
        // First create one to read.
        let owner = mint_token(SCOPE).await;
        let (_, _, created) = post_reboot_request(Some(&owner), Some("{}"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

        // Wrong scope → 403.
        let bad = mint_token("some:other-scope").await;
        let (status, _, resp) = get_reboot_request_by_id(Some(&bad), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");

        // No token → 401.
        let (status, _, resp) = get_reboot_request_by_id(None, &id, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn get_reboot_request_echoes_x_correlator_on_success_and_error() {
        let token = mint_token(SCOPE).await;
        let (_, _, created) = post_reboot_request(Some(&token), Some("{}"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

        // Success.
        let (status, headers, _) =
            get_reboot_request_by_id(Some(&token), &id, Some("corr-getrb")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-getrb")
        );

        // Error (unknown id).
        let (status, headers, _) = get_reboot_request_by_id(
            Some(&token),
            "11111111-1111-4111-8111-111111111111",
            Some("corr-getrberr"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-getrberr")
        );
    }

    // --- deleteRebootRequest (DELETE /reboot-requests/{id}) -----------------

    /// DELETE a reboot request by id with an optional Bearer token and `x-correlator`.
    async fn delete_reboot_request_by_id(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("/network-access-devices/vwip/reboot-requests/{id}"))
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
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn delete_created_reboot_request_is_no_content_then_gone() {
        let token = mint_token(SCOPE).await;
        let (status, _, created) = post_reboot_request(Some(&token), Some("{}"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // First delete evicts the resource → 204 No Content (empty body).
        let (status, _, body) = delete_reboot_request_by_id(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null, "204 carries no body");
        assert!(store::get(&id).is_none(), "gone from the store after delete");

        // The read leg now 404s for the same id.
        let (status, _, resp) = get_reboot_request_by_id(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_not_found() {
        let token = mint_token(SCOPE).await;
        let (_, _, created) = post_reboot_request(Some(&token), Some("{}"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

        let (status, _, _) = delete_reboot_request_by_id(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // A second delete of the same id → 404 (store state is the plane).
        let (status, _, resp) = delete_reboot_request_by_id(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_reboot_request_is_not_found() {
        let token = mint_token(SCOPE).await;
        // A well-formed UUID that was never created → 404.
        let (status, _, resp) = delete_reboot_request_by_id(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_reboot_request_requires_the_scope_and_a_token() {
        // First create one to target.
        let owner = mint_token(SCOPE).await;
        let (_, _, created) = post_reboot_request(Some(&owner), Some("{}"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

        // Wrong scope → 403 (and the resource is untouched).
        let bad = mint_token("some:other-scope").await;
        let (status, _, resp) = delete_reboot_request_by_id(Some(&bad), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");

        // No token → 401 (and the resource is untouched).
        let (status, _, resp) = delete_reboot_request_by_id(None, &id, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");

        // The rejected deletes did not evict it — it still reads back.
        let (status, _, _) = get_reboot_request_by_id(Some(&owner), &id, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_reboot_request_echoes_x_correlator_on_success_and_error() {
        let token = mint_token(SCOPE).await;
        let (_, _, created) = post_reboot_request(Some(&token), Some("{}"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

        // Success (204) still echoes the correlator.
        let (status, headers, _) =
            delete_reboot_request_by_id(Some(&token), &id, Some("corr-delrb")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-delrb")
        );

        // Error (unknown id) echoes it too.
        let (status, headers, _) = delete_reboot_request_by_id(
            Some(&token),
            "11111111-1111-4111-8111-111111111111",
            Some("corr-delrberr"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-delrberr")
        );
    }

    // --- updateRebootRequest (PATCH /reboot-requests/{id}) ------------------

    #[test]
    fn validate_reboot_patch_accepts_mutable_fields_and_ignores_read_only() {
        // atTime + message set; identity/target/audit + unknown keys ignored.
        let merge = validate_reboot_patch(&json!({
            "atTime": "2024-06-01T02:00:00Z",
            "message": "rescheduled",
            "id": "ignored",
            "devices": ["ignored"],
            "createdAt": "ignored",
            "modifiedAt": "ignored",
            "unknown": 1,
        }))
        .expect("valid patch");
        assert_eq!(
            merge,
            vec![
                ("atTime", Some(json!("2024-06-01T02:00:00Z"))),
                ("message", Some(json!("rescheduled"))),
            ]
        );

        // Explicit nulls are clears.
        let merge = validate_reboot_patch(&json!({ "atTime": null, "message": null }))
            .expect("null clears");
        assert_eq!(merge, vec![("atTime", None), ("message", None)]);

        // An empty object is a valid no-op merge.
        assert_eq!(validate_reboot_patch(&json!({})).unwrap(), Vec::new());

        // Malformed values → Err.
        assert!(validate_reboot_patch(&json!({ "atTime": "soon" })).is_err());
        assert!(validate_reboot_patch(&json!({ "atTime": 5 })).is_err());
        assert!(validate_reboot_patch(&json!({ "message": "x".repeat(256) })).is_err());
        assert!(validate_reboot_patch(&json!([])).is_err()); // not an object
    }

    #[test]
    fn apply_reboot_merge_sets_and_clears_fields() {
        let mut r = json!({ "id": "x", "atTime": "2024-06-01T02:00:00Z", "message": "old" });
        apply_reboot_merge(
            &mut r,
            &vec![("message", Some(json!("new"))), ("atTime", None)],
        );
        assert_eq!(r["message"], "new");
        assert!(r.get("atTime").is_none(), "null op removed atTime");
        assert_eq!(r["id"], "x", "untouched fields survive");
    }

    /// PATCH a reboot request by id with an optional Bearer token and `x-correlator`.
    async fn patch_reboot_request_by_id(
        token: Option<&str>,
        id: &str,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("PATCH")
            .uri(format!("/network-access-devices/vwip/reboot-requests/{id}"))
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
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Create a scheduled reboot request (carrying an `atTime`, so it is
    /// modifiable) and return its id.
    async fn create_scheduled(token: &str) -> String {
        let (status, _, created) =
            post_reboot_request(Some(token), Some(r#"{"atTime":"2024-06-01T02:00:00Z"}"#), None)
                .await;
        assert_eq!(status, StatusCode::CREATED);
        created["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn patch_updates_a_scheduled_reboot_and_persists() {
        let token = mint_token(SCOPE).await;
        let id = create_scheduled(&token).await;

        let (status, _, updated) = patch_reboot_request_by_id(
            Some(&token),
            &id,
            r#"{"message":"rescheduled","atTime":"2024-07-01T03:00:00Z"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["message"], "rescheduled");
        assert_eq!(updated["atTime"], "2024-07-01T03:00:00Z");
        assert!(updated["modifiedAt"].is_string());
        // Identity/target fields are untouched.
        assert_eq!(updated["id"], id);
        assert!(updated["devices"].is_array());

        // The change persists: a later read sees it.
        let (status, _, read) = get_reboot_request_by_id(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read, updated);
    }

    #[tokio::test]
    async fn patch_null_clears_message_and_empty_body_is_a_noop() {
        let token = mint_token(SCOPE).await;
        let (_, _, created) = post_reboot_request(
            Some(&token),
            Some(r#"{"atTime":"2024-06-01T02:00:00Z","message":"note"}"#),
            None,
        )
        .await;
        let id = created["id"].as_str().unwrap().to_string();

        // Empty body → no-op 200, message survives.
        let (status, _, updated) = patch_reboot_request_by_id(Some(&token), &id, "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["message"], "note");

        // Explicit null clears message.
        let (status, _, updated) =
            patch_reboot_request_by_id(Some(&token), &id, r#"{"message":null}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(updated.get("message").is_none(), "message cleared");
    }

    #[tokio::test]
    async fn patch_read_only_and_unknown_fields_are_ignored() {
        let token = mint_token(SCOPE).await;
        let id = create_scheduled(&token).await;

        // Attempts to change identity/target/audit/unknown keys are ignored.
        let (status, _, updated) = patch_reboot_request_by_id(
            Some(&token),
            &id,
            r#"{"id":"hacked","devices":["11111111-1111-4111-8111-111111111111"],"createdAt":"1999-01-01T00:00:00Z","foo":1}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["id"], id, "id is immutable");
        assert_ne!(updated["devices"], json!(["11111111-1111-4111-8111-111111111111"]));
        assert_ne!(updated["createdAt"], "1999-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn patch_an_immediate_reboot_is_incompatible_state() {
        let token = mint_token(SCOPE).await;
        // An immediate reboot carries no `atTime` — it has already fired.
        let (_, _, created) = post_reboot_request(Some(&token), Some("{}"), None).await;
        let id = created["id"].as_str().unwrap().to_string();
        assert!(created.get("atTime").is_none());

        let (status, _, resp) =
            patch_reboot_request_by_id(Some(&token), &id, r#"{"message":"too late"}"#, None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(resp["code"], "NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE");

        // The store is left unchanged by the declined patch.
        assert_eq!(store::get(&id), Some(created));
    }

    #[tokio::test]
    async fn patch_unknown_id_is_not_found() {
        let token = mint_token(SCOPE).await;
        let (status, _, resp) = patch_reboot_request_by_id(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            r#"{"message":"x"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn patch_bad_body_400_wins_over_state_and_unknown_id() {
        let token = mint_token(SCOPE).await;
        let id = create_scheduled(&token).await;

        // Malformed atTime → 400 (validated before the store/state is consulted).
        let (status, _, resp) =
            patch_reboot_request_by_id(Some(&token), &id, r#"{"atTime":"soon"}"#, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");

        // A body 400 beats even an unknown id (no store lookup happens).
        let (status, _, _) = patch_reboot_request_by_id(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            "not json",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // An over-long message → 400.
        let long = format!(r#"{{"message":"{}"}}"#, "x".repeat(256));
        let (status, _, _) = patch_reboot_request_by_id(Some(&token), &id, &long, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn patch_requires_the_scope_and_a_token() {
        let owner = mint_token(SCOPE).await;
        let id = create_scheduled(&owner).await;

        // Wrong scope → 403 (resource untouched).
        let bad = mint_token("some:other-scope").await;
        let (status, _, resp) =
            patch_reboot_request_by_id(Some(&bad), &id, r#"{"message":"x"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");

        // No token → 401 (resource untouched).
        let (status, _, resp) =
            patch_reboot_request_by_id(None, &id, r#"{"message":"x"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");

        // The rejected patches did not mutate it.
        let (_, _, read) = get_reboot_request_by_id(Some(&owner), &id, None).await;
        assert!(read.get("message").is_none());
    }

    #[tokio::test]
    async fn patch_echoes_x_correlator_on_success_and_error() {
        let token = mint_token(SCOPE).await;
        let id = create_scheduled(&token).await;

        // Success.
        let (status, headers, _) =
            patch_reboot_request_by_id(Some(&token), &id, r#"{"message":"m"}"#, Some("corr-pat"))
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-pat")
        );

        // Error (unknown id).
        let (status, headers, _) = patch_reboot_request_by_id(
            Some(&token),
            "11111111-1111-4111-8111-111111111111",
            r#"{"message":"m"}"#,
            Some("corr-paterr"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-paterr")
        );
    }
}
