//! Network Access Domains **vwip** (CAMARA NetworkAccessManagement / Network
//! Access Domains, wip).
//!
//! Endpoints so far:
//! - `GET /network-access-domains/vwip/trust-domains/capabilities` — the
//!   provider-level Trust Domain capabilities document (operationId
//!   `getTrustDomainCapabilities`).
//! - `GET /network-access-domains/vwip/services` — the caller's **Services**
//!   catalog (operationId `getServices`): the logical commercial subscriptions /
//!   account relationships the authenticated identity holds.
//! - `GET /network-access-domains/vwip/services/{serviceId}` — a single
//!   **Service** from that catalog by id (operationId `getService`).
//! - `POST /network-access-domains/vwip/trust-domains` — the first **stateful**
//!   leg: create a Trust Domain (operationId `createTrustDomain`), persisted in
//!   the in-memory [`store`].
//!
//! ## What they do
//!
//! `getTrustDomainCapabilities` returns the set of Trust Domain configuration
//! capabilities this API provider supports: which access types (Wi-Fi
//! WPA-Personal/Enterprise, Thread) and their properties, plus the policy limits
//! (max devices, per-domain up/downstream bandwidth bands, egress allow-list
//! constraints) a caller may configure when creating a Trust Domain. It requires
//! the `network-access-domains:trust-domains` scope.
//!
//! `getServices` returns the caller's `ServiceList` — the services (each a
//! logical commercial subscription tied to a `serviceSite`) associated with the
//! authenticated identity. It requires the `network-access-domains:services:read`
//! scope. Each `serviceSite` carries a deterministic `location.geographicPoint`
//! (a WGS-84 point stable per identity/slot); the canonical `propertyAddress`
//! (civic address) remains a documented cut. The point is a fixed, renderable
//! coordinate, not a queryable spatial field, so it is not a control plane.
//!
//! Both are protected: they require a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the endpoint's scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! **`getTrustDomainCapabilities`** — provider capabilities are fixed operator
//! configuration, not keyed to any device or subscriber, so this operation has
//! **no** parameter-driven control plane: given a scoped token it always returns
//! the same deterministic document (`200`).
//!
//! **`getServices`** has no request body either, but it *is* keyed to the
//! authenticated identity, so its control plane is the **token subject** (DESIGN
//! §7, mirroring the other subject-keyed reads): a reserved error suffix on the
//! subject → the canonical CAMARA error; otherwise the subject's trailing three
//! digits `d` fix the catalog deterministically — `d == 0` (`…000` / no digits) →
//! `200 []` (nothing associated; a list never 404s), else `((d - 1) % 3) + 1`
//! services (1–3), each with a deterministic UUID-shaped `id` and `serviceSite`.
//!
//! **`getService`** (`GET /services/{serviceId}`) reads a single `Service` from
//! that same identity-keyed catalog. It regenerates the subject's catalog (no
//! store) and matches the `serviceId` path parameter. Two control planes (DESIGN
//! §7): the subject's reserved error suffix → the canonical CAMARA error (an
//! account-level plane, checked first, mirroring `getServices`); otherwise the
//! `serviceId` vs the catalog — an id the identity holds → `200` that `Service`,
//! any other id (unknown / another identity's / malformed) → `404 NOT_FOUND`
//! (the opaque id is not itself a plane, so malformed folds into the `404`).
//!
//! **`createTrustDomain`** (`POST /trust-domains`) creates a Trust Domain from a
//! `TrustDomainCreate` body and persists it. Three control planes (DESIGN §7):
//! (1) the subject's reserved error suffix → the canonical CAMARA error (an
//! account-level plane, checked first, mirroring the reads); (2) request
//! validation → `400 INVALID_ARGUMENT` (missing/blank/oversized `name`, missing
//! `enabled`, missing/malformed `serviceId`, an `accessDetails` that is empty /
//! longer than 4, or an entry whose `accessType` is unknown, is not advertised,
//! or lacks its variant's required keys); (3) store state → `409` when a Trust
//! Domain with the *same* `name` already exists for the *same* `serviceId` (the
//! `trustDomainId` is derived from that pair, so a duplicate collides).
//!
//! For all of them, a missing/invalid token → `401 UNAUTHENTICATED`; a token
//! without the endpoint's scope → `403 PERMISSION_DENIED` (both from the shared
//! resource-server layer). `x-correlator` is echoed on every response.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the Trust Domain legs require — both the read-only
/// `GET /trust-domains/capabilities` and the stateful `POST /trust-domains`
/// (`createTrustDomain`) carry the single `network-access-domains:trust-domains`
/// scope (CAMARA NetworkAccessManagement / Network Access Domains).
const TRUST_DOMAINS_SCOPE: &str = "network-access-domains:trust-domains";

/// The OAuth2 scope the `GET /services` endpoint requires.
const SERVICES_SCOPE: &str = "network-access-domains:services:read";

/// The OAuth2 scope the Trust Domain Device legs require (CAMARA
/// NetworkAccessManagement / Network Access Domains — the whole device sub-resource
/// carries the single `network-access-domains:devices` scope).
const DEVICES_SCOPE: &str = "network-access-domains:devices";

/// The device types a `TrustDomainDeviceCreate` may declare (the CAMARA
/// `deviceType` enum). A present `deviceType` outside this set → `400`.
const DEVICE_TYPES: [&str; 10] = [
    "AUTOMOTIVE",
    "DESKTOP",
    "IOT",
    "LAPTOP",
    "MEDICAL",
    "OTHER",
    "SMARTPHONE",
    "TABLET",
    "TV",
    "WEARABLE",
];

/// The advertised Trust Domain access types (the discriminator values this
/// provider supports, from [`trust_domain_capabilities`]). `createTrustDomain`
/// admits only these — a well-formed `accessType` outside this set (e.g. the
/// canonical `"Thread:TLV"`, which this provider does not advertise) → `400`.
const ADVERTISED_ACCESS_TYPES: [&str; 3] = [
    "Wi-Fi:WPA_PERSONAL",
    "Wi-Fi:WPA_ENTERPRISE",
    "Thread:STRUCTURED",
];

/// Routes for Network Access Domains vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/network-access-domains/vwip/trust-domains/capabilities",
            get(get_capabilities),
        )
        .route(
            "/network-access-domains/vwip/trust-domains",
            post(create_trust_domain),
        )
        .route(
            "/network-access-domains/vwip/trust-domains/:trust_domain_id",
            get(get_trust_domain)
                .patch(update_trust_domain)
                .delete(delete_trust_domain),
        )
        .route(
            "/network-access-domains/vwip/trust-domains/:trust_domain_id/devices",
            get(get_trust_domain_devices).post(create_trust_domain_device),
        )
        .route(
            "/network-access-domains/vwip/trust-domains/:trust_domain_id/devices/:device_id",
            get(get_trust_domain_device),
        )
        .route(
            "/network-access-domains/vwip/services",
            get(get_services),
        )
        .route(
            "/network-access-domains/vwip/services/:service_id",
            get(get_service),
        )
}

/// `GET /network-access-domains/vwip/trust-domains/capabilities`.
async fn get_capabilities(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    with_correlator(
        (StatusCode::OK, Json(trust_domain_capabilities())).into_response(),
        &correlator,
    )
}

/// `POST /network-access-domains/vwip/trust-domains` (`createTrustDomain`).
///
/// Creates a Trust Domain: the caller sends a `TrustDomainCreate` (a `serviceId`,
/// a human-readable `name`, an `enabled` flag, and 1–4 `accessDetails`), the
/// simulator validates it, mints a `trustDomainId`, renders the full
/// `TrustDomain` (adding the read-only `id` + audit stamps), persists it in the
/// in-memory [`store`], and returns `201` with that `TrustDomain`.
///
/// Three control planes (docs/DESIGN.md §7):
///
/// 1. **Reserved error suffix (token subject)** — an account-level plane checked
///    first, mirroring `getServices`/`getService`: if the subject's trailing
///    three digits name a reserved CAMARA status, the create answers that
///    canonical error regardless of the body.
/// 2. **Request validation** — a missing/blank/oversized `name`, a missing
///    `enabled`, a missing/malformed `serviceId` (must be a UUID), an
///    `accessDetails` that is absent / empty / longer than 4, or an
///    `accessDetail` whose `accessType` is unknown, is not one this provider
///    advertises (see [`ADVERTISED_ACCESS_TYPES`]), or is missing its variant's
///    required fields → `400 INVALID_ARGUMENT`.
/// 3. **Store state** — the `trustDomainId` is derived deterministically from the
///    `(serviceId, name)` identity ([`trust_domain_id`]), so creating a Trust
///    Domain with the *same* name for the *same* service collides → `409`
///    (duplicate name for service).
///
/// The nested `accessDetails` field *values* (SSID pattern, hex lengths, Thread
/// channel range, …) and the optional `policies` object are validated only for
/// presence/shape of the discriminator's required keys; the full per-field
/// pattern/range validation is a documented cut. The write-only WPA password is
/// stripped from the echoed `TrustDomain` (it never appears in a response).
/// `x-correlator` is echoed on every response.
async fn create_trust_domain(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's Trust Domain scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Control plane 1 — account-level reserved-error suffix on the token subject
    // (checked first, mirroring the read legs).
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Parse the TrustDomainCreate body (malformed JSON / non-object → 400).
    let req: Value = match serde_json::from_slice::<Value>(&body) {
        Ok(v) if v.is_object() => v,
        _ => {
            return invalid_argument(
                "the request body is not a valid TrustDomainCreate JSON object",
                &correlator,
            )
        }
    };

    // Control plane 2 — required-field / shape validation.
    if let Err(message) = validate_trust_domain_create(&req) {
        return invalid_argument(&message, &correlator);
    }

    // Identity fields are guaranteed present + well-typed by `validate` above.
    let service_id = req["serviceId"].as_str().unwrap_or_default();
    let name = req["name"].as_str().unwrap_or_default();
    let id = trust_domain_id(service_id, name);

    // Render the full TrustDomain (adds the read-only id + audit stamps; strips
    // the write-only WPA password from the echoed accessDetails).
    let now = rfc3339_utc(now_unix_secs());
    let actor = deterministic_uuid_v5("nad-td-actor", identity);
    let trust_domain = render_trust_domain(&id, &req, &now, &actor);

    // Control plane 3 — store state (same (serviceId, name) → duplicate → 409).
    if !store::insert(id, trust_domain.clone()) {
        return with_correlator(
            CamaraError::conflict("A Trust Domain with this name already exists for the service.")
                .into_response(),
            &correlator,
        );
    }

    with_correlator(
        (StatusCode::CREATED, Json(trust_domain)).into_response(),
        &correlator,
    )
}

/// `POST /network-access-domains/vwip/trust-domains/{trustDomainId}/devices`
/// (`createTrustDomainDevice`).
///
/// Registers a device inside an existing Trust Domain. The caller sends a
/// `TrustDomainDeviceCreate` (a required `deviceName` + `enabled`, plus the
/// optional `externalId`/`deviceType`/`blocked`/`hardwareAddress`/
/// `bootstrappingInfo`/`deviceCredential`), the simulator validates it, mints a
/// server-assigned `deviceId`, renders the full `TrustDomainDevice` (adding the
/// read-only `id`, the `connected`/`associated` lifecycle flags, and the audit
/// stamps), persists it in the in-memory device [`store`], and returns `201`.
///
/// Four control planes (docs/DESIGN.md §7):
///
/// 1. **Reserved error suffix (token subject)** — an account-level plane checked
///    first, mirroring `createTrustDomain`: the subject's trailing three digits
///    naming a reserved CAMARA status answer that canonical error regardless of
///    the path or body.
/// 2. **Request validation** — a missing/blank/oversized `deviceName`, a missing
///    or non-boolean `enabled`, an out-of-range `externalId`, a non-boolean
///    `blocked`, an unknown `deviceType`, or a malformed `hardwareAddress`
///    (`hardwareAddressType` other than `EUI-48`, or a `value` that is not an
///    EUI-48 MAC) → `400 INVALID_ARGUMENT`.
/// 3. **Parent cross-reference** — the `{trustDomainId}` path must name a Trust
///    Domain already in the store, else → `404 NOT_FOUND` (a device cannot be
///    created in a Trust Domain that does not exist).
/// 4. **Store state** — the `deviceId` is derived deterministically from the
///    `(trustDomainId, deviceName)` identity ([`trust_domain_device_id`]), so
///    creating a device with the *same* `deviceName` in the *same* Trust Domain
///    collides → `409` (duplicate device name in the Trust Domain).
///
/// A freshly created device is not yet `associated`/`connected` and holds no
/// assigned network address, so `connected`/`associated` are rendered `false`
/// and `ipv4Address`/`ipv6Address` are omitted until association (a documented
/// cut — the simulator has no live onboarding). The write-only `deviceCredential`
/// (it may carry a shared/assigned secret) is stripped from the echoed
/// `TrustDomainDevice`, and the nested `bootstrappingInfo`/`deviceCredential`
/// contents are validated only for object shape (documented cuts, mirroring
/// `createTrustDomain`'s `policies`). `x-correlator` is echoed on every response.
async fn create_trust_domain_device(
    claims: Claims,
    headers: HeaderMap,
    Path(trust_domain_id): Path<String>,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the device scope.
    if let Err(e) = claims.require_scope(DEVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Control plane 1 — account-level reserved-error suffix on the token subject
    // (checked first, mirroring `createTrustDomain`).
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Parse the TrustDomainDeviceCreate body (malformed JSON / non-object → 400).
    let req: Value = match serde_json::from_slice::<Value>(&body) {
        Ok(v) if v.is_object() => v,
        _ => {
            return invalid_argument(
                "the request body is not a valid TrustDomainDeviceCreate JSON object",
                &correlator,
            )
        }
    };

    // Control plane 2 — required-field / shape validation (a body 400 is reported
    // before the parent 404, mirroring `createAppInstance`).
    if let Err(message) = validate_trust_domain_device_create(&req) {
        return invalid_argument(&message, &correlator);
    }

    // Control plane 3 — the parent Trust Domain must exist.
    if store::get(&trust_domain_id).is_none() {
        return with_correlator(
            CamaraError::not_found("No Trust Domain found for the provided id.").into_response(),
            &correlator,
        );
    }

    // `deviceName` is guaranteed present + well-typed by `validate` above.
    let device_name = req["deviceName"].as_str().unwrap_or_default();
    let id = trust_domain_device_id(&trust_domain_id, device_name);

    // Render the full TrustDomainDevice (adds the read-only id, lifecycle flags,
    // and audit stamps; strips the write-only deviceCredential from the echo).
    let now = rfc3339_utc(now_unix_secs());
    let actor = deterministic_uuid_v5("nad-td-actor", identity);
    let device = render_trust_domain_device(&id, &req, &now, &actor);

    // Control plane 4 — store state (same (trustDomainId, deviceName) → 409).
    if !store::insert_device(&trust_domain_id, &id, device.clone()) {
        return with_correlator(
            CamaraError::conflict("A device with this name already exists in the Trust Domain.")
                .into_response(),
            &correlator,
        );
    }

    with_correlator(
        (StatusCode::CREATED, Json(device)).into_response(),
        &correlator,
    )
}

/// `GET /network-access-domains/vwip/trust-domains/{trustDomainId}/devices`
/// (`getTrustDomainDevices`).
///
/// Lists every device registered in the Trust Domain named by `{trustDomainId}`
/// as a plain JSON array (the CAMARA `TrustDomainDeviceList`, an array of
/// `TrustDomainDevice`; there is no page wrapper). There is no request body and
/// no query parameter, so the outcome keys only on state (docs/DESIGN.md §7):
///
/// 1. **Parent Trust Domain existence** — the collection lives *under* a Trust
///    Domain, so an unknown/never-created (or malformed) `{trustDomainId}` →
///    `404 NOT_FOUND` (the parent path segment must resolve; the upstream spec
///    declares the `404`). The `trustDomainId` is an opaque, server-minted UUID,
///    so it has no reserved-suffix plane — the store is the only judge, mirroring
///    `getTrustDomain`.
/// 2. **The device store** — the Trust Domain's own device roster
///    ([`store::list_devices`]), rendered verbatim (each device's write-only
///    `deviceCredential` was already stripped at create). A Trust Domain with no
///    devices yet → `200 []` (a list never `404`s on an empty *result*; the
///    parent still had to exist to reach here). The list is scoped to this Trust
///    Domain: a device created in a different Trust Domain never appears.
///
/// Like the device read leg, the token subject is **not** a control plane (a
/// reserved suffix on the subject does not shape a store-only read); the leg is
/// store-only. Requires a token carrying the `network-access-domains:devices`
/// scope (the same scope guards `createTrustDomainDevice`/`getTrustDomainDevice`).
/// `x-correlator` is echoed on every response.
async fn get_trust_domain_devices(
    claims: Claims,
    headers: HeaderMap,
    Path(trust_domain_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the device scope.
    if let Err(e) = claims.require_scope(DEVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Control plane 1 — the parent Trust Domain must exist (the collection is
    // scoped under it). An unknown/malformed id has no store entry → 404,
    // mirroring `getTrustDomain`.
    if store::get(&trust_domain_id).is_none() {
        return with_correlator(
            CamaraError::not_found("No Trust Domain found for the provided id.").into_response(),
            &correlator,
        );
    }

    // Control plane 2 — the device store. An existing Trust Domain with no
    // devices yields `200 []` (a list never 404s on an empty result).
    let devices = store::list_devices(&trust_domain_id);
    with_correlator(
        (StatusCode::OK, Json(Value::Array(devices))).into_response(),
        &correlator,
    )
}

/// `GET /network-access-domains/vwip/trust-domains/{trustDomainId}/devices/{deviceId}`
/// (`getTrustDomainDevice`).
///
/// Reads a Trust Domain Device back by its opaque, server-minted `deviceId` inside
/// its owning `trustDomainId`. Like `getTrustDomain`, the `deviceId` is not
/// derivable by the caller (it is a SHA-256-derived UUID over the
/// `(trustDomainId, deviceName)` pair, see [`trust_domain_device_id`]), so — unlike
/// the device *create* leg, which keys off the token subject to reach the
/// account-level reserved-error set — the **in-memory device store is the only
/// control plane** (docs/DESIGN.md §7): a stored `(trustDomainId, deviceId)` pair →
/// `200` with the persisted `TrustDomainDevice` verbatim (write-only
/// `deviceCredential` already stripped at create); any other pair → `404
/// NOT_FOUND`. Because the store is keyed by the full pair, an unknown parent Trust
/// Domain, an unknown device, a device that belongs to a *different* Trust Domain,
/// and a malformed id all fold into the same `404` (one store lookup, mirroring
/// `getTrustDomain`).
///
/// Requires a token carrying the `network-access-domains:devices` scope (the same
/// scope guards `createTrustDomainDevice`). `x-correlator` is echoed on every
/// response.
async fn get_trust_domain_device(
    claims: Claims,
    headers: HeaderMap,
    Path((trust_domain_id, device_id)): Path<(String, String)>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the device scope.
    if let Err(e) = claims.require_scope(DEVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the only control plane — the opaque minted device id has no
    // reserved-suffix plane. A hit renders the stored TrustDomainDevice; a miss
    // (unknown parent Trust Domain, unknown/other-domain device, or malformed id)
    // is a 404.
    match store::get_device(&trust_domain_id, &device_id) {
        Some(device) => {
            with_correlator((StatusCode::OK, Json(device)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No device found for the provided id in the Trust Domain.")
                .into_response(),
            &correlator,
        ),
    }
}

/// `GET /network-access-domains/vwip/trust-domains/{trustDomainId}`
/// (`getTrustDomain`).
///
/// Reads a Trust Domain back by the opaque, server-minted `trustDomainId` that
/// `createTrustDomain` returned. The id is not derivable by the caller (it is a
/// SHA-256-derived UUID over the `(serviceId, name)` pair), so — mirroring the
/// sibling `readNetwork` / `getApp` read legs — the **in-memory store is the only
/// control plane** (docs/DESIGN.md §7): a stored id → `200` with the persisted
/// `TrustDomain` verbatim (write-only WPA password already stripped at create);
/// any other id (never created, already deleted in a later slice, or malformed) →
/// `404 NOT_FOUND`. There is no store entry to distinguish a malformed id from an
/// unknown one, so both fold into the same `404` (mirroring `getService`).
///
/// Requires a token carrying the `network-access-domains:trust-domains` scope
/// (the same scope guards `createTrustDomain` and the capabilities document).
/// `x-correlator` is echoed on every response.
async fn get_trust_domain(
    claims: Claims,
    headers: HeaderMap,
    Path(trust_domain_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's Trust Domain scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the only control plane — the opaque minted id has no
    // reserved-suffix plane. A hit renders the stored TrustDomain; a miss (unknown
    // or malformed id) is a 404.
    match store::get(&trust_domain_id) {
        Some(trust_domain) => {
            with_correlator((StatusCode::OK, Json(trust_domain)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No Trust Domain found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /network-access-domains/vwip/trust-domains/{trustDomainId}`
/// (`deleteTrustDomain`).
///
/// Deletes a Trust Domain by the opaque, server-minted `trustDomainId` that
/// `createTrustDomain` returned. Like `getTrustDomain`, the id is not derivable
/// by the caller (a SHA-256-derived UUID over the `(serviceId, name)` pair), so
/// the **in-memory store is the only control plane** (docs/DESIGN.md §7): a
/// stored id is evicted → `204 No Content` (single-use — a second delete of the
/// same id finds nothing); any other id (never created, already deleted, or
/// malformed) → `404 NOT_FOUND`. There is no store entry to distinguish a
/// malformed id from an unknown one, so both fold into the same `404` (mirroring
/// `getTrustDomain`). Deletion is synchronous with no `subscription-ended`-style
/// CloudEvent (the API has no `sink` on Trust Domains).
///
/// Requires a token carrying the `network-access-domains:trust-domains` scope
/// (the same scope guards `createTrustDomain` / `getTrustDomain` / the
/// capabilities document). `x-correlator` is echoed on every response, including
/// the `204`.
async fn delete_trust_domain(
    claims: Claims,
    headers: HeaderMap,
    Path(trust_domain_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's Trust Domain scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the only control plane — the opaque minted id has no
    // reserved-suffix plane. A present id is evicted (204); a miss (unknown,
    // already-deleted, or malformed id) is a 404.
    if store::remove(&trust_domain_id) {
        with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
    } else {
        with_correlator(
            CamaraError::not_found("No Trust Domain found for the provided id.").into_response(),
            &correlator,
        )
    }
}

/// `PATCH /network-access-domains/vwip/trust-domains/{trustDomainId}`
/// (`updateTrustDomain`).
///
/// Updates a created Trust Domain in place. The caller sends a `TrustDomainUpdate`
/// — the mutable subset of a Trust Domain, every field **optional**
/// (`name`/`description`/`enabled`/`expiration`/`policies`/`accessDetails`) — and
/// only the fields present are changed. Per the CAMARA schema `accessDetails` is a
/// **full replacement** (a supplied array replaces the stored one wholesale), and
/// the read-only identity/audit fields (`id`, `serviceId`, `createdAt`/`createdBy`)
/// are immutable — a caller that includes them in the body has them ignored. Every
/// successful update refreshes the `modifiedAt`/`modifiedBy` audit stamps.
///
/// Two control planes (docs/DESIGN.md §7), mirroring the repo's other PATCH legs
/// (`updateAppDeployment`, `patchTrafficInfluence`):
///
/// 1. **Request body** — validated first, so a body `400` wins over a `404`. A
///    malformed body, a present-but-ill-typed field (blank/oversized `name`,
///    non-boolean `enabled`, over-long `description`, non-string `expiration`,
///    non-object `policies`, an `accessDetails` that is not a 1–4 array or whose
///    entries fail the same access-detail validation as `createTrustDomain`) →
///    `400 INVALID_ARGUMENT`.
/// 2. **Store state** — the opaque, server-minted `trustDomainId` has no
///    reserved-suffix plane (mirroring `getTrustDomain`/`deleteTrustDomain`): a
///    stored id is patched and the updated `TrustDomain` returned (`200`); any
///    other id (never created, already deleted, or malformed) → `404 NOT_FOUND`.
///
/// Unlike `createTrustDomain` there is **no `409`**: `updateTrustDomain` addresses
/// an existing resource by its fixed id and never re-derives it, so a rename can't
/// collide (the CAMARA `updateTrustDomain` response set is `200`/`400`/`404`).
/// The write-only WPA `password` is stripped from any replacement `accessDetails`
/// so it never appears in the echoed `TrustDomain`. Requires the
/// `network-access-domains:trust-domains` scope. `x-correlator` is echoed on every
/// response.
async fn update_trust_domain(
    claims: Claims,
    headers: HeaderMap,
    Path(trust_domain_id): Path<String>,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's Trust Domain scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Parse the TrustDomainUpdate body (malformed JSON / non-object → 400).
    let req: Value = match serde_json::from_slice::<Value>(&body) {
        Ok(v) if v.is_object() => v,
        _ => {
            return invalid_argument(
                "the request body is not a valid TrustDomainUpdate JSON object",
                &correlator,
            )
        }
    };

    // Control plane 1 — request-body validation (checked before the store lookup,
    // so a body 400 wins over a 404).
    if let Err(message) = validate_trust_domain_update(&req) {
        return invalid_argument(&message, &correlator);
    }

    // Control plane 2 — store state. The whole get-modify-write runs under one lock
    // hold (see `store::update`): a hit applies the patch, refreshes the audit
    // stamps, and returns the updated resource; a miss is a 404.
    let now = rfc3339_utc(now_unix_secs());
    let actor = deterministic_uuid_v5("nad-td-actor", claims.subject().unwrap_or(""));
    match store::update(&trust_domain_id, |td| {
        apply_trust_domain_update(td, &req, &now, &actor)
    }) {
        Some(updated) => {
            with_correlator((StatusCode::OK, Json(updated)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No Trust Domain found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// `GET /network-access-domains/vwip/services`.
///
/// Returns the caller's `ServiceList` — the services associated with the
/// authenticated identity. Keyed on the token subject (DESIGN §7): a reserved
/// error suffix → the canonical CAMARA error; otherwise the subject's trailing
/// three digits fix the catalog deterministically.
async fn get_services(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SERVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The authenticated identity is the token subject (client_credentials sets
    // `sub` = client_id; the three-legged grants set a user subject). Reserved
    // error suffix on that identity → the canonical CAMARA error (DESIGN §7).
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    let list = Value::Array(services_for(identity));
    with_correlator((StatusCode::OK, Json(list)).into_response(), &correlator)
}

/// `GET /network-access-domains/vwip/services/{serviceId}`.
///
/// Reads a single `Service` from the caller's catalog by id (operationId
/// `getService`). The caller's `ServiceList` is fully deterministic from the
/// token subject (see [`services_for`]), so — mirroring the sibling
/// `getNetworkAccessDevice` — this endpoint needs no store: it regenerates the
/// subject's catalog and returns the service whose `id` matches the path
/// parameter. Two control planes (docs/DESIGN.md §7):
///
/// - **Reserved error suffix (subject)** — an account-level plane, mirroring
///   `getServices`: if the subject's trailing three digits name a reserved CAMARA
///   status, the endpoint answers that canonical error regardless of the id.
/// - **The `serviceId` vs the subject's catalog** — an id drawn from the
///   subject's deterministic catalog → `200` with that `Service`; any other id
///   (unknown, belonging to a different identity, or malformed) → `404
///   NOT_FOUND`. The id is opaque to the caller (SHA-256-derived UUID), so it is
///   not itself a scenario plane (there is no store to distinguish an unknown id
///   from a malformed one — both fold into the `404`, mirroring
///   `getNetworkAccessDevice`).
///
/// Requires a token carrying the `network-access-domains:services:read` scope.
async fn get_service(
    claims: Claims,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SERVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the account-level control plane (docs/DESIGN.md §7):
    // a reserved suffix takes the whole account into a canonical error, matching
    // the listing endpoint.
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Regenerate the subject's deterministic catalog and look the id up. An id
    // that is not one of this identity's services — unknown, another identity's,
    // or malformed — is a `404 NOT_FOUND` (there is no store to distinguish them).
    match services_for(identity)
        .into_iter()
        .find(|svc| svc["id"] == json!(service_id))
    {
        Some(svc) => with_correlator((StatusCode::OK, Json(svc)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No service found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// Fixed service templates `(name, description, siteName, siteDescription)`, one
/// per catalog slot. The count and per-slot selection are chosen deterministically
/// from the identity (below), so the catalog is reproducible per caller.
const SERVICE_TEMPLATES: [(&str, &str, &str, &str); 3] = [
    (
        "Main Home Internet",
        "Internet subscription for primary residence",
        "Primary Residence",
        "123 Main Street",
    ),
    (
        "Vacation Home Internet",
        "Internet subscription for vacation home",
        "Vacation Home",
        "45 Lakeside Road",
    ),
    (
        "Small Business Fibre",
        "Fibre subscription for a small business site",
        "Downtown Office",
        "500 Market Avenue",
    ),
];

/// The `ServiceList` (0..=3 `Service` records) for an authenticated `identity`.
///
/// Deterministic from the identity's trailing three digits `d` (DESIGN §7):
/// `d == 0` (or no digits) → an empty list (nothing associated; a list never
/// 404s), else `((d - 1) % 3) + 1` services (1–3).
fn services_for(identity: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(identity).unwrap_or(0);
    if d == 0 {
        return Vec::new();
    }
    let count = ((d - 1) % 3) + 1; // 1..=3
    (0..count as usize).map(|i| service(identity, i)).collect()
}

/// A single deterministic `Service` for `identity` at catalog slot `i`.
fn service(identity: &str, i: usize) -> Value {
    let (name, description, site_name, site_description) =
        SERVICE_TEMPLATES[i % SERVICE_TEMPLATES.len()];
    let (latitude, longitude) = deterministic_point(identity, i);
    json!({
        "id": deterministic_uuid("nad-service", identity, i),
        "name": name,
        "description": description,
        "serviceSite": {
            "id": deterministic_uuid("nad-service-site", identity, i),
            "name": site_name,
            "description": site_description,
            // The site's physical location (docs/DESIGN.md §7). Only the WGS-84
            // `geographicPoint` is emitted; the canonical `propertyAddress` (a
            // 20-field civic address) remains a documented cut. This is a fixed,
            // representative coordinate the caller can render, not a queryable
            // spatial field, so it is not a scenario control plane.
            "location": {
                "geographicPoint": { "latitude": latitude, "longitude": longitude }
            }
        }
    })
}

/// A deterministic WGS-84 point (decimal degrees) for `identity`'s service site
/// at catalog slot `index`.
///
/// Derived from a domain-tagged SHA-256 (a tag distinct from the id tags, so the
/// coordinate never collides with an id) mapped onto the valid latitude
/// (`[-90, 90]`) and longitude (`[-180, 180]`) ranges, rounded to five decimal
/// places (~1 m). Stable per `(identity, slot)` yet unrelated to the ids. No
/// `rand`/geo dependency (reuses sha2).
fn deterministic_point(identity: &str, index: usize) -> (f64, f64) {
    let h = Sha256::digest(format!("nad-service-site-geo:{identity}:{index}").as_bytes());
    // Two independent unit fractions in `[0, 1]` from disjoint hash bytes.
    let lat_unit = u16::from_be_bytes([h[0], h[1]]) as f64 / u16::MAX as f64;
    let lon_unit = u16::from_be_bytes([h[2], h[3]]) as f64 / u16::MAX as f64;
    let round5 = |x: f64| (x * 1e5).round() / 1e5;
    (round5(lat_unit * 180.0 - 90.0), round5(lon_unit * 360.0 - 180.0))
}

/// A deterministic, UUID-shaped id from the first 16 bytes of a domain-tagged
/// SHA-256 over `(tag, identity, index)` (distinct tags never collide). Mirrors
/// Blockchain Public Address's record-id rendering; no `uuid`/`rand` dependency.
fn deterministic_uuid(tag: &str, identity: &str, index: usize) -> String {
    let h = Sha256::digest(format!("{tag}:{identity}:{index}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// The fixed `TrustDomainCapabilities` document this provider advertises.
///
/// A representative, schema-valid set covering the three access-type families
/// (Wi-Fi WPA-Personal, Wi-Fi WPA-Enterprise, Thread STRUCTURED) and all four
/// policy capabilities. Deterministic — capabilities are provider configuration,
/// not keyed to any request input (DESIGN §7).
fn trust_domain_capabilities() -> Value {
    json!({
        "supportedAccessTypes": [
            {
                "accessType": "Wi-Fi:WPA_PERSONAL",
                "wifiProperties": {
                    "ssidRequired": true,
                    "supportedSecurityModes": [
                        "WPA2-Personal",
                        "WPA3-Personal",
                        "WPA2-WPA3-Personal"
                    ],
                    "passwordConstraints": {
                        "minLength": 8,
                        "maxLength": 63,
                        "requireSpecialCharacters": false
                    },
                    "authServerRequired": false
                }
            },
            {
                "accessType": "Wi-Fi:WPA_ENTERPRISE",
                "wifiProperties": {
                    "ssidRequired": true,
                    "supportedSecurityModes": [
                        "WPA2-Enterprise",
                        "WPA3-Enterprise"
                    ],
                    "authServerRequired": true
                }
            },
            {
                "accessType": "Thread:STRUCTURED",
                "threadProperties": {
                    "supportedChannels": { "min": 11, "max": 26 },
                    "networkKeyFormat": "32-hex-digits",
                    "networkNameConstraints": { "minLength": 1, "maxLength": 16 },
                    "maxDatasetLength": 254
                }
            }
        ],
        "supportedPolicies": {
            "maxDevices": { "minValue": 1, "maxValue": 100 },
            "maxDomainDownstreamRate": { "minValue": 1_000_000, "maxValue": 1_000_000_000 },
            "maxDomainUpstreamRate": { "minValue": 1_000_000, "maxValue": 1_000_000_000 },
            "egressAllowedList": {
                "maxEntries": 50,
                "supportedDestinationTypes": ["IP", "FQDN", "CIDR"]
            }
        }
    })
}

/// Validate a `TrustDomainCreate` body. Returns `Err(message)` on the first
/// violation (mapped by the caller to `400 INVALID_ARGUMENT`).
///
/// Checks the required top-level fields (`name`, `enabled`, `serviceId`,
/// `accessDetails`) and, for each access detail, that its `accessType` is one
/// this provider advertises ([`ADVERTISED_ACCESS_TYPES`]) and that the
/// discriminated variant carries its required keys. The nested field *values*
/// (SSID/key patterns, Thread channel range, …) and the optional `policies`
/// object are a documented cut — only presence/shape of the required keys is
/// enforced here.
fn validate_trust_domain_create(req: &Value) -> Result<(), String> {
    // name — required, non-blank, ≤ 64 characters.
    match req.get("name").and_then(Value::as_str) {
        None => return Err("`name` is required and must be a string".into()),
        Some(n) if n.trim().is_empty() => return Err("`name` must not be blank".into()),
        Some(n) if n.chars().count() > 64 => {
            return Err("`name` must be at most 64 characters".into())
        }
        Some(_) => {}
    }

    // enabled — required boolean.
    if !req.get("enabled").map(Value::is_boolean).unwrap_or(false) {
        return Err("`enabled` is required and must be a boolean".into());
    }

    // serviceId — required, a UUID (the caller obtains it from `getServices`;
    // validated as a lowercase-hex UUID, matching the ServiceId `format: uuid`,
    // not the stricter version/variant Uuid pattern the response id carries).
    match req.get("serviceId").and_then(Value::as_str) {
        None => return Err("`serviceId` is required and must be a string".into()),
        Some(s) if !is_uuid(s) => return Err("`serviceId` must be a valid UUID".into()),
        Some(_) => {}
    }

    // accessDetails — required array of 1..=4 access-detail objects.
    let details = match req.get("accessDetails").and_then(Value::as_array) {
        None => return Err("`accessDetails` is required and must be an array".into()),
        Some(d) => d,
    };
    if details.is_empty() {
        return Err("`accessDetails` must contain at least one entry".into());
    }
    if details.len() > 4 {
        return Err("`accessDetails` must contain at most 4 entries".into());
    }
    for detail in details {
        validate_access_detail(detail)?;
    }

    // description — optional, but if present a string ≤ 255 characters.
    if let Some(desc) = req.get("description").filter(|v| !v.is_null()) {
        match desc.as_str() {
            Some(s) if s.chars().count() <= 255 => {}
            Some(_) => return Err("`description` must be at most 255 characters".into()),
            None => return Err("`description` must be a string".into()),
        }
    }

    // expiration — optional, but if present a string (RFC 3339 date-time).
    if let Some(exp) = req.get("expiration").filter(|v| !v.is_null()) {
        if !exp.is_string() {
            return Err("`expiration` must be an RFC 3339 date-time string".into());
        }
    }

    // policies — optional, but if present an object (contents not validated).
    if let Some(pol) = req.get("policies").filter(|v| !v.is_null()) {
        if !pol.is_object() {
            return Err("`policies` must be an object".into());
        }
    }

    Ok(())
}

/// Validate one `AccessDetail` item — its `accessType` must be advertised and the
/// discriminated variant must carry its required keys.
fn validate_access_detail(detail: &Value) -> Result<(), String> {
    let obj = detail
        .as_object()
        .ok_or("each `accessDetails` entry must be an object")?;
    let access_type = obj
        .get("accessType")
        .and_then(Value::as_str)
        .ok_or("each `accessDetails` entry requires an `accessType`")?;
    if !ADVERTISED_ACCESS_TYPES.contains(&access_type) {
        return Err(format!(
            "`accessType` \"{access_type}\" is not supported by this provider (see GET /trust-domains/capabilities)"
        ));
    }

    match access_type {
        "Wi-Fi:WPA_PERSONAL" => {
            let mode = obj
                .get("securityMode")
                .and_then(Value::as_object)
                .ok_or("a Wi-Fi:WPA_PERSONAL access detail requires a `securityMode` object")?;
            if !mode.get("password").map(Value::is_string).unwrap_or(false) {
                return Err(
                    "a Wi-Fi:WPA_PERSONAL `securityMode` requires a string `password`".into(),
                );
            }
        }
        "Wi-Fi:WPA_ENTERPRISE" => {
            let mode = obj
                .get("securityMode")
                .and_then(Value::as_object)
                .ok_or("a Wi-Fi:WPA_ENTERPRISE access detail requires a `securityMode` object")?;
            if !mode
                .get("securityModeType")
                .map(Value::is_string)
                .unwrap_or(false)
            {
                return Err(
                    "a Wi-Fi:WPA_ENTERPRISE `securityMode` requires a string `securityModeType`"
                        .into(),
                );
            }
        }
        "Thread:STRUCTURED" => {
            for key in ["extendedPanId", "networkKey", "networkName", "panId"] {
                if !obj.get(key).map(Value::is_string).unwrap_or(false) {
                    return Err(format!(
                        "a Thread:STRUCTURED access detail requires a string `{key}`"
                    ));
                }
            }
            if !obj.get("channel").map(Value::is_number).unwrap_or(false) {
                return Err(
                    "a Thread:STRUCTURED access detail requires a numeric `channel`".into(),
                );
            }
        }
        _ => unreachable!("accessType already checked against the advertised set"),
    }
    Ok(())
}

/// Validate a `TrustDomainUpdate` (PATCH) body. Every field is **optional**, so
/// this only checks the ones that are present; an absent field is left unchanged
/// by [`apply_trust_domain_update`]. Returns `Err(message)` on the first violation
/// (mapped by the caller to `400 INVALID_ARGUMENT`).
///
/// The clearable optional fields (`description`/`expiration`/`policies`) accept an
/// explicit JSON `null` (which [`apply_trust_domain_update`] removes); the required
/// resource fields (`name`/`enabled`) and the full-replacement `accessDetails`
/// array do **not** — a present `null` for those is a `400`. Read-only fields the
/// body may carry (`id`/`serviceId`/audit stamps) are neither validated nor applied
/// (a documented cut, mirroring `updateAppDeployment`'s immutable `appId`).
fn validate_trust_domain_update(req: &Value) -> Result<(), String> {
    // name — if present, a non-blank string ≤ 64 characters (not clearable).
    if let Some(name) = req.get("name") {
        match name.as_str() {
            None => return Err("`name` must be a string".into()),
            Some(n) if n.trim().is_empty() => return Err("`name` must not be blank".into()),
            Some(n) if n.chars().count() > 64 => {
                return Err("`name` must be at most 64 characters".into())
            }
            Some(_) => {}
        }
    }

    // enabled — if present, a boolean (not clearable).
    if let Some(enabled) = req.get("enabled") {
        if !enabled.is_boolean() {
            return Err("`enabled` must be a boolean".into());
        }
    }

    // description — if present, either null (clear) or a string ≤ 255 characters.
    if let Some(desc) = req.get("description").filter(|v| !v.is_null()) {
        match desc.as_str() {
            Some(s) if s.chars().count() <= 255 => {}
            Some(_) => return Err("`description` must be at most 255 characters".into()),
            None => return Err("`description` must be a string".into()),
        }
    }

    // expiration — if present, either null (clear) or an RFC 3339 date-time string.
    if let Some(exp) = req.get("expiration").filter(|v| !v.is_null()) {
        if !exp.is_string() {
            return Err("`expiration` must be an RFC 3339 date-time string".into());
        }
    }

    // policies — if present, either null (clear) or an object (contents not validated).
    if let Some(pol) = req.get("policies").filter(|v| !v.is_null()) {
        if !pol.is_object() {
            return Err("`policies` must be an object".into());
        }
    }

    // accessDetails — if present, a full-replacement array of 1..=4 valid entries
    // (same per-entry validation as `createTrustDomain`; not clearable).
    if let Some(access) = req.get("accessDetails") {
        let details = match access.as_array() {
            None => return Err("`accessDetails` must be an array".into()),
            Some(d) => d,
        };
        if details.is_empty() {
            return Err("`accessDetails` must contain at least one entry".into());
        }
        if details.len() > 4 {
            return Err("`accessDetails` must contain at most 4 entries".into());
        }
        for detail in details {
            validate_access_detail(detail)?;
        }
    }

    Ok(())
}

/// Apply a validated `TrustDomainUpdate` `req` to the stored `TrustDomain` `td`
/// in place (called under the store lock by [`store::update`]). Only the mutable
/// fields present in the body change: `name`/`enabled`/`accessDetails` are set from
/// a present (non-null) value; the clearable `description`/`expiration`/`policies`
/// are set from a present non-null value and **removed** on an explicit `null`;
/// `accessDetails` replaces wholesale (write-only WPA `password` stripped). Every
/// call refreshes the audit stamps (`modifiedAt` = `now`, `modifiedBy` = `actor`),
/// so an empty `{}` body is an accepted no-op that only re-stamps `modified*`. The
/// read-only identity fields (`id`/`serviceId`/`createdAt`/`createdBy`) are left
/// untouched even if the body carries them.
fn apply_trust_domain_update(td: &mut Value, req: &Value, now: &str, actor: &str) {
    let obj = match td.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    // Non-clearable scalar fields: set when present (validation already rejected a
    // present-but-null name/enabled).
    for key in ["name", "enabled"] {
        if let Some(v) = req.get(key) {
            obj.insert(key.into(), v.clone());
        }
    }

    // Clearable optional fields: a present non-null value sets, an explicit null
    // removes the field.
    for key in ["description", "expiration", "policies"] {
        if let Some(v) = req.get(key) {
            if v.is_null() {
                obj.remove(key);
            } else {
                obj.insert(key.into(), v.clone());
            }
        }
    }

    // accessDetails: full replacement, write-only password stripped from the echo.
    if let Some(access) = req.get("accessDetails") {
        obj.insert("accessDetails".into(), sanitised_access_details(access));
    }

    // Refresh the audit stamps on every successful update.
    obj.insert("modifiedAt".into(), json!(now));
    obj.insert("modifiedBy".into(), json!(actor));
}

/// Render the full `TrustDomain` response from a validated `TrustDomainCreate`
/// `req`: the minted read-only `id`, the echoed create fields, and the audit
/// stamps (`createdAt`/`modifiedAt` = `now`, `createdBy`/`modifiedBy` = `actor`).
/// The write-only WPA `password` is stripped from the echoed access details (it
/// never appears in a response).
fn render_trust_domain(id: &str, req: &Value, now: &str, actor: &str) -> Value {
    let mut td = serde_json::Map::new();
    td.insert("id".into(), json!(id));
    td.insert("serviceId".into(), req["serviceId"].clone());
    td.insert("name".into(), req["name"].clone());
    td.insert("enabled".into(), req["enabled"].clone());
    for key in ["description", "expiration", "policies"] {
        if let Some(v) = req.get(key).filter(|v| !v.is_null()) {
            td.insert(key.into(), v.clone());
        }
    }
    td.insert(
        "accessDetails".into(),
        sanitised_access_details(&req["accessDetails"]),
    );
    td.insert("createdAt".into(), json!(now));
    td.insert("createdBy".into(), json!(actor));
    td.insert("modifiedAt".into(), json!(now));
    td.insert("modifiedBy".into(), json!(actor));
    Value::Object(td)
}

/// Strip the write-only WPA `password` from each `accessDetails` entry so it is
/// never echoed in the `TrustDomain` response (CAMARA marks it `writeOnly`).
fn sanitised_access_details(details: &Value) -> Value {
    let items = details.as_array().cloned().unwrap_or_default();
    let cleaned = items.into_iter().map(|mut detail| {
        if let Some(mode) = detail
            .get_mut("securityMode")
            .and_then(Value::as_object_mut)
        {
            mode.remove("password");
        }
        detail
    });
    Value::Array(cleaned.collect())
}

/// A deterministic, server-assigned `trustDomainId` for a Trust Domain, derived
/// from its `(serviceId, name)` identity. Deriving it from the identity is what
/// makes a duplicate (same name for the same service) collide → the CAMARA `409`.
/// It is a strict RFC 4122 (version 5, name-based) UUID so it satisfies the
/// `TrustDomain.id` `Uuid` pattern the CAMARA schema requires.
pub fn trust_domain_id(service_id: &str, name: &str) -> String {
    deterministic_uuid_v5("nad-trust-domain", &format!("{service_id}\u{1f}{name}"))
}

/// A deterministic, server-assigned `deviceId` for a Trust Domain Device, derived
/// from its `(trustDomainId, deviceName)` identity. Deriving it from the identity
/// is what makes a duplicate (same `deviceName` in the same Trust Domain) collide
/// → the CAMARA `409`. It is a strict RFC 4122 (version 5, name-based) UUID so it
/// satisfies the `TrustDomainDevice.id` `Uuid` pattern the CAMARA schema requires.
pub fn trust_domain_device_id(trust_domain_id: &str, device_name: &str) -> String {
    deterministic_uuid_v5(
        "nad-td-device",
        &format!("{trust_domain_id}\u{1f}{device_name}"),
    )
}

/// Validate a `TrustDomainDeviceCreate` body. `deviceName` and `enabled` are
/// required; the remaining fields are optional and checked only when present.
/// Returns `Err(message)` on the first violation (mapped by the caller to
/// `400 INVALID_ARGUMENT`). The nested `bootstrappingInfo`/`deviceCredential`
/// objects are validated only for object shape (their protocol/credential
/// internals are a documented cut, mirroring the Trust Domain `policies`).
fn validate_trust_domain_device_create(req: &Value) -> Result<(), String> {
    // deviceName — required, non-blank string ≤ 255 characters.
    match req.get("deviceName").map(Value::as_str) {
        None => return Err("`deviceName` is required".into()),
        Some(None) => return Err("`deviceName` must be a string".into()),
        Some(Some(n)) if n.trim().is_empty() => return Err("`deviceName` must not be blank".into()),
        Some(Some(n)) if n.chars().count() > 255 => {
            return Err("`deviceName` must be at most 255 characters".into())
        }
        Some(Some(_)) => {}
    }

    // enabled — required boolean.
    match req.get("enabled") {
        None => return Err("`enabled` is required".into()),
        Some(v) if !v.is_boolean() => return Err("`enabled` must be a boolean".into()),
        Some(_) => {}
    }

    // externalId — if present, a string of 1..=255 characters.
    if let Some(v) = req.get("externalId") {
        match v.as_str() {
            Some(s) if (1..=255).contains(&s.chars().count()) => {}
            _ => return Err("`externalId` must be a string of 1 to 255 characters".into()),
        }
    }

    // blocked — if present, a boolean.
    if let Some(v) = req.get("blocked") {
        if !v.is_boolean() {
            return Err("`blocked` must be a boolean".into());
        }
    }

    // deviceType — if present, one of the advertised device types.
    if let Some(v) = req.get("deviceType") {
        match v.as_str() {
            Some(s) if DEVICE_TYPES.contains(&s) => {}
            _ => return Err("`deviceType` must be one of the supported device types".into()),
        }
    }

    // hardwareAddress — if present, an object with an EUI-48 type + a valid MAC.
    if let Some(v) = req.get("hardwareAddress") {
        let obj = match v.as_object() {
            Some(o) => o,
            None => return Err("`hardwareAddress` must be an object".into()),
        };
        match obj.get("hardwareAddressType").and_then(Value::as_str) {
            Some("EUI-48") => {}
            _ => return Err("`hardwareAddress.hardwareAddressType` must be \"EUI-48\"".into()),
        }
        match obj.get("value").and_then(Value::as_str) {
            Some(mac) if is_eui48(mac) => {}
            _ => return Err("`hardwareAddress.value` must be an EUI-48 MAC address".into()),
        }
    }

    // bootstrappingInfo / deviceCredential — if present, objects (contents cut).
    for key in ["bootstrappingInfo", "deviceCredential"] {
        if let Some(v) = req.get(key) {
            if !v.is_object() {
                return Err(format!("`{key}` must be an object"));
            }
        }
    }

    Ok(())
}

/// Whether `s` is an EUI-48 MAC address — six 2-hex-digit groups separated by a
/// `:` or `-` (each separator independently either), matching the CAMARA
/// `^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$` pattern. No regex dependency.
fn is_eui48(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 17 {
        return false;
    }
    bytes.iter().enumerate().all(|(i, &c)| {
        if i % 3 == 2 {
            c == b':' || c == b'-'
        } else {
            c.is_ascii_hexdigit()
        }
    })
}

/// Render the full `TrustDomainDevice` response from a validated
/// `TrustDomainDeviceCreate` `req`: the minted read-only `id`, the echoed create
/// fields, the read-only lifecycle flags, and the audit stamps
/// (`createdAt`/`modifiedAt` = `now`, `createdBy`/`modifiedBy` = `actor`). The
/// write-only `deviceCredential` is stripped (it never appears in a response); a
/// freshly created device is `connected: false` / `associated: false` and holds
/// no assigned `ipv4Address`/`ipv6Address` (omitted until association).
fn render_trust_domain_device(id: &str, req: &Value, now: &str, actor: &str) -> Value {
    let mut d = serde_json::Map::new();
    d.insert("id".into(), json!(id));
    d.insert("deviceName".into(), req["deviceName"].clone());
    // Echoed optional create fields (present, non-null); deviceCredential is
    // write-only and deliberately excluded.
    for key in [
        "externalId",
        "deviceType",
        "blocked",
        "hardwareAddress",
        "bootstrappingInfo",
    ] {
        if let Some(v) = req.get(key).filter(|v| !v.is_null()) {
            d.insert(key.into(), v.clone());
        }
    }
    d.insert("enabled".into(), req["enabled"].clone());
    // Read-only lifecycle: a just-created device is not yet associated/connected.
    d.insert("connected".into(), json!(false));
    d.insert("associated".into(), json!(false));
    d.insert("createdAt".into(), json!(now));
    d.insert("createdBy".into(), json!(actor));
    d.insert("modifiedAt".into(), json!(now));
    d.insert("modifiedBy".into(), json!(actor));
    Value::Object(d)
}

/// A stable, strict RFC 4122 (version 5, name-based) UUID from a domain-tagged
/// SHA-256 over `key`, with the version (`5`) and variant nibbles forced so it
/// satisfies the strict `Uuid` pattern (`[1-5]` version, `[89ab]` variant) the
/// CAMARA `TrustDomain.id`/audit fields require. No `uuid`/`rand` dependency
/// (mirrors `edge_application_management::vwip::app_id`).
fn deterministic_uuid_v5(tag: &str, key: &str) -> String {
    let mut h = Sha256::digest(format!("{tag}:{key}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x50; // version 5
    h[8] = (h[8] & 0x3f) | 0x80; // variant (10xx)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// Whether `s` is a lowercase-hex, hyphenated 8-4-4-4-12 UUID string — the loose
/// `format: uuid` check applied to the caller-supplied `serviceId` (a strict
/// version/variant match is not required of it; the ServiceId schema is just
/// `format: uuid`).
fn is_uuid(s: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    let is_lower_hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    parts.len() == groups.len()
        && parts
            .iter()
            .zip(groups)
            .all(|(p, n)| p.len() == n && p.bytes().all(is_lower_hex))
}

/// A `400 INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
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
/// date/time dependency (mirrors `qos_provisioning::v0_3`).
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

    const HOST: &str = "nad.local:8080";

    // --- Pure unit: the fixed capabilities document -----------------------

    #[test]
    fn capabilities_document_is_schema_shaped() {
        let caps = trust_domain_capabilities();

        // `supportedAccessTypes` — required, 1..=4, each carries its discriminator.
        let access = caps["supportedAccessTypes"].as_array().unwrap();
        assert!((1..=4).contains(&access.len()));
        for entry in access {
            let ty = entry["accessType"].as_str().unwrap();
            assert!(matches!(
                ty,
                "Wi-Fi:WPA_PERSONAL"
                    | "Wi-Fi:WPA_ENTERPRISE"
                    | "Thread:STRUCTURED"
                    | "Thread:TLV"
            ));
            // A Wi-Fi variant carries wifiProperties; a Thread variant threadProperties.
            if ty.starts_with("Wi-Fi") {
                assert!(entry["wifiProperties"].is_object());
            } else {
                assert!(entry["threadProperties"].is_object());
            }
        }

        // All four policy capabilities are advertised, in range.
        let pol = &caps["supportedPolicies"];
        assert_eq!(pol["maxDevices"]["minValue"], 1);
        assert_eq!(pol["maxDevices"]["maxValue"], 100);
        assert!(pol["maxDomainDownstreamRate"]["maxValue"].as_i64().unwrap() <= 10_000_000_000);
        assert!(pol["maxDomainUpstreamRate"]["minValue"].as_i64().unwrap() >= 0);
        let egress = &pol["egressAllowedList"];
        assert_eq!(egress["maxEntries"], 50);
        assert_eq!(
            egress["supportedDestinationTypes"].as_array().unwrap().len(),
            3
        );
    }

    #[test]
    fn wifi_password_constraints_are_within_the_camara_bounds() {
        let caps = trust_domain_capabilities();
        let personal = &caps["supportedAccessTypes"][0]["wifiProperties"]["passwordConstraints"];
        let min = personal["minLength"].as_i64().unwrap();
        let max = personal["maxLength"].as_i64().unwrap();
        assert!((8..=63).contains(&min));
        assert!((8..=63).contains(&max));
        assert!(min <= max);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials` with the given scope.
    async fn mint_token(scope: &str) -> String {
        mint_token_as("nad-client", scope).await
    }

    /// Mint a token whose subject (`sub` = `client_id`) is `client_id` — lets a
    /// test choose the identity the subject-keyed `getServices` reads.
    async fn mint_token_as(client_id: &str, scope: &str) -> String {
        let body =
            format!("grant_type=client_credentials&client_id={client_id}&scope={scope}");
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

    /// GET the capabilities endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_caps(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-domains/vwip/trust-domains/capabilities")
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
    async fn returns_the_capabilities_document() {
        let token = mint_token(TRUST_DOMAINS_SCOPE).await;
        let (status, _, body) = get_caps(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["supportedAccessTypes"].as_array().unwrap().len() >= 1);
        assert!(body["supportedPolicies"]["maxDevices"].is_object());
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_caps(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_caps(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success() {
        let token = mint_token(TRUST_DOMAINS_SCOPE).await;
        let (status, headers, _) = get_caps(Some(&token), Some("corr-nad")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nad")
        );
    }

    // === getServices ======================================================

    // --- Pure unit: the deterministic catalog ------------------------------

    #[test]
    fn services_count_is_deterministic_from_trailing_digits() {
        // `…000` / no digits → empty list.
        assert!(services_for("nad-000").is_empty());
        assert!(services_for("no-digits").is_empty());
        // d>0 → ((d-1) % 3) + 1 services (1..=3).
        assert_eq!(services_for("nad-001").len(), 1); // (0%3)+1 = 1
        assert_eq!(services_for("nad-005").len(), 2); // (4%3)+1 = 2
        assert_eq!(services_for("nad-003").len(), 3); // (2%3)+1 = 3
    }

    #[test]
    fn services_are_well_shaped_and_deterministic() {
        let a = services_for("nad-005");
        let b = services_for("nad-005");
        assert_eq!(a, b, "same identity yields the same catalog");

        for svc in &a {
            // `id` is required and UUID-shaped; `serviceSite.id` likewise.
            let id = svc["id"].as_str().unwrap();
            assert!(is_uuid_shaped(id), "service id {id}");
            assert!(svc["name"].is_string());
            let site = &svc["serviceSite"];
            let site_id = site["id"].as_str().unwrap();
            assert!(is_uuid_shaped(site_id), "site id {site_id}");
            // A service id and its site id are drawn from distinct SHA-256 tags.
            assert_ne!(id, site_id);
            assert!(site["name"].is_string());
        }

        // A different identity yields a different first id.
        assert_ne!(a[0]["id"], services_for("nad-006")[0]["id"]);
    }

    #[test]
    fn service_site_location_point_is_valid_and_deterministic() {
        let catalog = services_for("nad-003"); // 3 services → 3 sites
        assert_eq!(catalog.len(), 3);
        for svc in &catalog {
            let point = &svc["serviceSite"]["location"]["geographicPoint"];
            let lat = point["latitude"].as_f64().unwrap();
            let lon = point["longitude"].as_f64().unwrap();
            // Within the WGS-84 valid ranges.
            assert!((-90.0..=90.0).contains(&lat), "latitude {lat} out of range");
            assert!((-180.0..=180.0).contains(&lon), "longitude {lon} out of range");
            // Rounded to at most five decimal places.
            assert_eq!((lat * 1e5).round() / 1e5, lat);
            assert_eq!((lon * 1e5).round() / 1e5, lon);
        }
        // Deterministic per identity+slot, and distinct across slots (the geo tag
        // is seeded by the slot index, so sites don't share one coordinate).
        assert_eq!(deterministic_point("nad-003", 0), deterministic_point("nad-003", 0));
        assert_ne!(deterministic_point("nad-003", 0), deterministic_point("nad-003", 1));
        // A different identity yields a different point for the same slot.
        assert_ne!(deterministic_point("nad-003", 0), deterministic_point("nad-004", 0));
    }

    /// Whether `s` is a lowercase-hex, hyphenated 8-4-4-4-12 UUID string.
    fn is_uuid_shaped(s: &str) -> bool {
        let parts: Vec<&str> = s.split('-').collect();
        parts.len() == 5
            && [8, 4, 4, 4, 12] == [
                parts[0].len(),
                parts[1].len(),
                parts[2].len(),
                parts[3].len(),
                parts[4].len(),
            ]
            && s.chars().all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    // --- Integration through the real router -------------------------------

    /// GET `/services` with an optional Bearer token and optional `x-correlator`.
    async fn get_services_req(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-domains/vwip/services")
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
    async fn services_returns_the_catalog_for_the_identity() {
        // sub = client_id = "nad-005" → digits 005 → 2 services.
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let list = body.as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0]["id"].is_string());
        assert!(list[0]["serviceSite"]["id"].is_string());
        // The serviceSite carries a WGS-84 geographicPoint through the router.
        let point = &list[0]["serviceSite"]["location"]["geographicPoint"];
        assert!((-90.0..=90.0).contains(&point["latitude"].as_f64().unwrap()));
        assert!((-180.0..=180.0).contains(&point["longitude"].as_f64().unwrap()));
    }

    #[tokio::test]
    async fn services_is_empty_for_a_zero_tail_identity() {
        // sub = "nad-000" → digits 000 → empty list (a list never 404s).
        let token = mint_token_as("nad-000", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn services_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404 NOT_FOUND.
        let token = mint_token_as("nad-404", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn services_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn services_missing_token_is_unauthenticated() {
        let (status, _, body) = get_services_req(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn services_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, headers, _) = get_services_req(Some(&token), Some("corr-svc")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-svc")
        );
    }

    // === getService (single-service read) =================================

    /// GET `/services/{serviceId}` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_service_req(
        service_id: &str,
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/network-access-domains/vwip/services/{service_id}"))
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
    async fn service_read_returns_a_member_of_the_identitys_catalog() {
        // sub = "nad-005" → 2 services; read the first back by its id.
        let catalog = services_for("nad-005");
        let wanted = catalog[0].clone();
        let id = wanted["id"].as_str().unwrap();

        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req(id, Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        // The read returns exactly the same Service the catalog listing holds.
        assert_eq!(body, wanted);
        assert_eq!(body["id"].as_str().unwrap(), id);
        assert!(body["serviceSite"]["id"].is_string());
    }

    #[tokio::test]
    async fn service_read_unknown_id_is_not_found() {
        // A well-formed but unowned id → 404 (a list read of the catalog never
        // holds this id).
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) =
            get_service_req("00000000-0000-0000-0000-000000000000", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_of_another_identitys_service_is_not_found() {
        // An id that belongs to a *different* identity's catalog is not this
        // caller's → 404 (the catalog is per-identity).
        let other_id = services_for("nad-006")[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req(&other_id, Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_malformed_id_is_not_found() {
        // A malformed (non-UUID) id folds into the same 404 (the opaque id is not
        // a plane; mirrors getNetworkAccessDevice).
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req("not-a-uuid", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_empty_catalog_identity_is_not_found() {
        // sub = "nad-000" → empty catalog, so any id → 404.
        let token = mint_token_as("nad-000", SERVICES_SCOPE).await;
        let (status, _, body) =
            get_service_req("3fa85f64-5717-4562-b3fc-2c963f66afa6", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404, regardless of the id.
        let token = mint_token_as("nad-404", SERVICES_SCOPE).await;
        let (status, _, body) =
            get_service_req("3fa85f64-5717-4562-b3fc-2c963f66afa6", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_reserved_429_suffix_beats_a_valid_id() {
        // sub = "nad-429" → reserved suffix → 429, even for an otherwise-valid id.
        // (The …429 catalog would hold a service, but the account-level plane wins.)
        let catalog = services_for("nad-429");
        assert!(!catalog.is_empty(), "…429 tail yields a non-empty catalog");
        let id = catalog[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-429", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req(&id, Some(&token), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn service_read_token_without_the_scope_is_forbidden() {
        let id = services_for("nad-005")[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, body) = get_service_req(&id, Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn service_read_missing_token_is_unauthenticated() {
        let (status, _, body) =
            get_service_req("3fa85f64-5717-4562-b3fc-2c963f66afa6", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn service_read_x_correlator_is_echoed() {
        let id = services_for("nad-005")[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, headers, _) = get_service_req(&id, Some(&token), Some("corr-svc-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-svc-1")
        );
    }

    // === createTrustDomain (POST /trust-domains) ==========================

    /// A representative, valid `TrustDomainCreate` body (one Wi-Fi WPA-Personal
    /// access detail). `service_id` / `name` are the (serviceId, name) identity
    /// pair that keys the minted `trustDomainId` and the `409` duplicate case.
    fn td_body(service_id: &str, name: &str) -> Value {
        json!({
            "serviceId": service_id,
            "name": name,
            "enabled": true,
            "description": "Primary home Wi-Fi",
            "accessDetails": [
                {
                    "accessType": "Wi-Fi:WPA_PERSONAL",
                    "ssid": "my-ssid",
                    "securityMode": {
                        "password": "s3cr3t-pass",
                        "securityModeType": "WPA3-Personal"
                    }
                }
            ]
        })
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn trust_domain_id_is_strict_uuid_and_identity_keyed() {
        let id = trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        // 8-4-4-4-12, lowercase hex, version nibble 5, variant nibble in 8..=b.
        assert!(is_uuid_shaped(&id), "id {id}");
        let raw: String = id.chars().filter(|c| *c != '-').collect();
        assert_eq!(raw.as_bytes()[12], b'5', "version nibble is 5");
        assert!(matches!(raw.as_bytes()[16], b'8' | b'9' | b'a' | b'b'), "variant nibble");
        // Deterministic per (serviceId, name); differs when either changes.
        assert_eq!(id, trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home"));
        assert_ne!(id, trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Office"));
        assert_ne!(id, trust_domain_id("123e4567-e89b-12d3-a456-426614174000", "Home"));
    }

    #[test]
    fn is_uuid_accepts_lowercase_and_rejects_others() {
        assert!(is_uuid("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(!is_uuid("3FA85F64-5717-4562-B3FC-2C963F66AFA6")); // uppercase
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("3fa85f64-5717-4562-b3fc-2c963f66afa")); // last group short
        assert!(!is_uuid("3fa85f64571745 62b3fc2c963f66afa6")); // wrong shape
    }

    #[test]
    fn validate_accepts_a_well_formed_body() {
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        assert!(validate_trust_domain_create(&body).is_ok());
    }

    #[test]
    fn validate_rejects_missing_and_malformed_fields() {
        let base = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");

        // name missing / blank / too long.
        let mut b = base.clone();
        b.as_object_mut().unwrap().remove("name");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["name"] = json!("   ");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["name"] = json!("x".repeat(65));
        assert!(validate_trust_domain_create(&b).is_err());

        // enabled missing / wrong type.
        let mut b = base.clone();
        b.as_object_mut().unwrap().remove("enabled");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["enabled"] = json!("yes");
        assert!(validate_trust_domain_create(&b).is_err());

        // serviceId missing / not a UUID.
        let mut b = base.clone();
        b.as_object_mut().unwrap().remove("serviceId");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["serviceId"] = json!("not-a-uuid");
        assert!(validate_trust_domain_create(&b).is_err());

        // accessDetails empty / too many.
        let mut b = base.clone();
        b["accessDetails"] = json!([]);
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        let one = base["accessDetails"][0].clone();
        b["accessDetails"] = json!([one, one.clone(), one.clone(), one.clone(), one.clone()]);
        assert!(validate_trust_domain_create(&b).is_err());
    }

    #[test]
    fn validate_rejects_unadvertised_and_incomplete_access_types() {
        let base = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");

        // Thread:TLV is a canonical enum but NOT advertised by this provider → 400.
        let mut b = base.clone();
        b["accessDetails"] = json!([{
            "accessType": "Thread:TLV",
            "operationalDataset": "0e08000000000000010010"
        }]);
        assert!(validate_trust_domain_create(&b).is_err());

        // An entirely unknown accessType → 400.
        let mut b = base.clone();
        b["accessDetails"] = json!([{ "accessType": "Bluetooth:LE" }]);
        assert!(validate_trust_domain_create(&b).is_err());

        // Advertised but missing the variant's required key (no securityMode) → 400.
        let mut b = base.clone();
        b["accessDetails"] = json!([{ "accessType": "Wi-Fi:WPA_PERSONAL", "ssid": "x" }]);
        assert!(validate_trust_domain_create(&b).is_err());

        // Thread:STRUCTURED missing a required field (panId) → 400.
        let mut b = base;
        b["accessDetails"] = json!([{
            "accessType": "Thread:STRUCTURED",
            "channel": 13,
            "extendedPanId": "d63e8e3e495ebbc3",
            "networkKey": "dfd34f0f05cad978ec4e32b0413038ff",
            "networkName": "Spec-Thread"
        }]);
        assert!(validate_trust_domain_create(&b).is_err());
    }

    #[test]
    fn render_strips_password_and_stamps_audit_fields() {
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        let id = trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        let td = render_trust_domain(&id, &body, "2024-01-01T00:00:00Z", "actor-uuid");

        assert_eq!(td["id"], json!(id));
        assert_eq!(td["name"], json!("Home"));
        assert_eq!(td["enabled"], json!(true));
        assert_eq!(td["description"], json!("Primary home Wi-Fi"));
        assert_eq!(td["createdAt"], json!("2024-01-01T00:00:00Z"));
        assert_eq!(td["modifiedAt"], json!("2024-01-01T00:00:00Z"));
        assert_eq!(td["createdBy"], json!("actor-uuid"));
        assert_eq!(td["modifiedBy"], json!("actor-uuid"));
        // The write-only password is stripped; the rest of securityMode remains.
        let mode = &td["accessDetails"][0]["securityMode"];
        assert!(mode.get("password").is_none(), "password must not be echoed");
        assert_eq!(mode["securityModeType"], json!("WPA3-Personal"));
    }

    // --- Integration through the real router -------------------------------

    /// POST `/trust-domains` with an optional Bearer token, optional JSON body,
    /// and optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_trust_domain(
        token: Option<&str>,
        body: Option<&Value>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/network-access-domains/vwip/trust-domains")
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let req_body = match body {
            Some(v) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(v).unwrap())
            }
            None => Body::empty(),
        };
        let response = app().oneshot(builder.body(req_body).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn create_trust_domain_persists_and_returns_the_resource() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD create ok");
        let (status, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::CREATED);

        // A minted, strict-UUID id + echoed create fields + audit stamps.
        let id = td["id"].as_str().unwrap();
        assert!(is_uuid_shaped(id), "id {id}");
        assert_eq!(td["serviceId"], body["serviceId"]);
        assert_eq!(td["name"], json!("TD create ok"));
        assert_eq!(td["enabled"], json!(true));
        assert!(td["createdAt"].as_str().unwrap().ends_with('Z'));
        assert!(td["createdBy"].is_string());
        assert!(td["modifiedAt"].as_str().unwrap().ends_with('Z'));
        // The write-only password never appears in the response.
        assert!(td["accessDetails"][0]["securityMode"].get("password").is_none());

        // It is persisted under its minted id (backs the later getTrustDomain leg).
        let stored = store::get(id).expect("the created Trust Domain is stored");
        assert_eq!(stored["name"], json!("TD create ok"));
    }

    #[tokio::test]
    async fn create_duplicate_name_for_service_conflicts() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("123e4567-e89b-12d3-a456-426614174000", "TD dup name");

        let (first, _, _) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(first, StatusCode::CREATED);
        // Same (serviceId, name) → same minted id → duplicate → 409.
        let (second, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(second, StatusCode::CONFLICT);
        assert_eq!(err["code"], "CONFLICT");

        // A different name for the same service is a distinct Trust Domain → 201.
        let other = td_body("123e4567-e89b-12d3-a456-426614174000", "TD dup name other");
        let (third, _, _) = post_trust_domain(Some(&token), Some(&other), None).await;
        assert_eq!(third, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404, regardless of the body.
        let token = mint_token_as("nad-404", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD reserved 404");
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn create_reserved_429_suffix_beats_a_valid_body() {
        // sub = "nad-429" → reserved suffix → 429, even for an otherwise-valid body.
        let token = mint_token_as("nad-429", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD reserved 429");
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn create_invalid_body_is_bad_request() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // Missing accessDetails → 400 INVALID_ARGUMENT.
        let body = json!({
            "serviceId": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "name": "TD invalid",
            "enabled": true
        });
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_unadvertised_access_type_is_bad_request() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let mut body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD tlv");
        body["accessDetails"] = json!([{
            "accessType": "Thread:TLV",
            "operationalDataset": "0e08000000000000010010"
        }]);
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD forbidden");
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn create_missing_token_is_unauthenticated() {
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD noauth");
        let (status, _, err) = post_trust_domain(None, Some(&body), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn create_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD correlator");
        let (status, headers, _) =
            post_trust_domain(Some(&token), Some(&body), Some("corr-td-1")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-td-1")
        );
    }

    // === createTrustDomainDevice (POST /trust-domains/{id}/devices) =======

    /// A representative, valid `TrustDomainDeviceCreate` body.
    fn device_body(device_name: &str) -> Value {
        json!({
            "deviceName": device_name,
            "enabled": true,
            "deviceType": "LAPTOP",
            "hardwareAddress": { "hardwareAddressType": "EUI-48", "value": "00:11:22:33:44:55" }
        })
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn device_id_is_strict_uuid_and_identity_keyed() {
        let td = "b1e0f6a2-9c3d-5e4f-8a1b-2c3d4e5f6a7b";
        let id = trust_domain_device_id(td, "Laptop");
        assert!(is_uuid_shaped(&id), "id {id}");
        // Deterministic per (trustDomainId, deviceName); differs when either changes.
        assert_eq!(id, trust_domain_device_id(td, "Laptop"));
        assert_ne!(id, trust_domain_device_id(td, "Phone"));
        assert_ne!(id, trust_domain_device_id("123e4567-e89b-12d3-a456-426614174000", "Laptop"));
    }

    #[test]
    fn is_eui48_accepts_colons_and_hyphens_only() {
        assert!(is_eui48("00:11:22:33:44:55"));
        assert!(is_eui48("aA-bB-cC-dD-eE-fF")); // mixed case, hyphen separators
        assert!(is_eui48("00-11:22-33:44-55")); // each separator independent
        assert!(!is_eui48("0011.2233.4455")); // dotted form not accepted
        assert!(!is_eui48("00:11:22:33:44")); // too few groups
        assert!(!is_eui48("00:11:22:33:44:5")); // short final group
        assert!(!is_eui48("0g:11:22:33:44:55")); // non-hex digit
    }

    #[test]
    fn validate_device_accepts_a_minimal_and_a_full_body() {
        assert!(validate_trust_domain_device_create(&json!({
            "deviceName": "Minimal", "enabled": false
        }))
        .is_ok());
        assert!(validate_trust_domain_device_create(&device_body("Full")).is_ok());
    }

    #[test]
    fn validate_device_rejects_missing_and_malformed_fields() {
        // deviceName missing / blank / too long.
        assert!(validate_trust_domain_device_create(&json!({ "enabled": true })).is_err());
        assert!(validate_trust_domain_device_create(&json!({ "deviceName": "  ", "enabled": true })).is_err());
        assert!(validate_trust_domain_device_create(
            &json!({ "deviceName": "x".repeat(256), "enabled": true })
        )
        .is_err());

        // enabled missing / wrong type.
        assert!(validate_trust_domain_device_create(&json!({ "deviceName": "D" })).is_err());
        assert!(validate_trust_domain_device_create(
            &json!({ "deviceName": "D", "enabled": "yes" })
        )
        .is_err());

        // deviceType unknown.
        assert!(validate_trust_domain_device_create(
            &json!({ "deviceName": "D", "enabled": true, "deviceType": "SERVER" })
        )
        .is_err());

        // externalId out of range / wrong type.
        assert!(validate_trust_domain_device_create(
            &json!({ "deviceName": "D", "enabled": true, "externalId": "" })
        )
        .is_err());

        // hardwareAddress malformed type / bad MAC.
        assert!(validate_trust_domain_device_create(&json!({
            "deviceName": "D", "enabled": true,
            "hardwareAddress": { "hardwareAddressType": "EUI-64", "value": "00:11:22:33:44:55" }
        }))
        .is_err());
        assert!(validate_trust_domain_device_create(&json!({
            "deviceName": "D", "enabled": true,
            "hardwareAddress": { "hardwareAddressType": "EUI-48", "value": "not-a-mac" }
        }))
        .is_err());
    }

    #[test]
    fn render_device_strips_credential_and_sets_lifecycle_flags() {
        let req = json!({
            "deviceName": "Laptop", "enabled": true, "deviceType": "LAPTOP", "blocked": false,
            "deviceCredential": { "credentialAction": "GENERATE" }
        });
        let id = trust_domain_device_id("td-id", "Laptop");
        let dev = render_trust_domain_device(&id, &req, "2024-01-01T00:00:00Z", "actor-uuid");

        assert_eq!(dev["id"], json!(id));
        assert_eq!(dev["deviceName"], json!("Laptop"));
        assert_eq!(dev["enabled"], json!(true));
        assert_eq!(dev["deviceType"], json!("LAPTOP"));
        assert_eq!(dev["connected"], json!(false));
        assert_eq!(dev["associated"], json!(false));
        assert_eq!(dev["createdAt"], json!("2024-01-01T00:00:00Z"));
        assert_eq!(dev["createdBy"], json!("actor-uuid"));
        // The write-only deviceCredential is never echoed; no address until associated.
        assert!(dev.get("deviceCredential").is_none(), "credential must not be echoed");
        assert!(dev.get("ipv4Address").is_none());
    }

    // --- Integration through the real router -------------------------------

    /// POST `/trust-domains/{id}/devices` with an optional Bearer token, optional
    /// JSON body, and optional `x-correlator`. Returns (status, headers, json).
    async fn post_device(
        token: Option<&str>,
        trust_domain_id: &str,
        body: Option<&Value>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!(
                "/network-access-domains/vwip/trust-domains/{trust_domain_id}/devices"
            ))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let req_body = match body {
            Some(v) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(v).unwrap())
            }
            None => Body::empty(),
        };
        let response = app().oneshot(builder.body(req_body).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Create a Trust Domain via the real router and return its minted id.
    async fn make_trust_domain(subject: &str, service_id: &str, name: &str) -> String {
        let token = mint_token_as(subject, TRUST_DOMAINS_SCOPE).await;
        let (status, _, td) =
            post_trust_domain(Some(&token), Some(&td_body(service_id, name)), None).await;
        assert_eq!(status, StatusCode::CREATED);
        td["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn create_device_persists_and_returns_the_resource() {
        let td_id =
            make_trust_domain("nad-005", "3fa85f64-5717-4562-b3fc-2c963f66afa6", "TDD ok").await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, dev) =
            post_device(Some(&token), &td_id, Some(&device_body("Laptop")), None).await;
        assert_eq!(status, StatusCode::CREATED);

        let id = dev["id"].as_str().unwrap();
        assert!(is_uuid_shaped(id), "id {id}");
        assert_eq!(dev["deviceName"], json!("Laptop"));
        assert_eq!(dev["enabled"], json!(true));
        assert_eq!(dev["connected"], json!(false));
        assert_eq!(dev["associated"], json!(false));
        assert!(dev["createdAt"].as_str().unwrap().ends_with('Z'));
        assert!(dev["createdBy"].is_string());

        // It is persisted under (trustDomainId, deviceId) — backs later read legs.
        let stored = store::get_device(&td_id, id).expect("the created device is stored");
        assert_eq!(stored["deviceName"], json!("Laptop"));
    }

    #[tokio::test]
    async fn create_device_duplicate_name_conflicts() {
        let td_id =
            make_trust_domain("nad-005", "123e4567-e89b-12d3-a456-426614174000", "TDD dup").await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;

        let (first, _, _) =
            post_device(Some(&token), &td_id, Some(&device_body("Dup Laptop")), None).await;
        assert_eq!(first, StatusCode::CREATED);
        // Same (trustDomainId, deviceName) → same minted id → duplicate → 409.
        let (second, _, err) =
            post_device(Some(&token), &td_id, Some(&device_body("Dup Laptop")), None).await;
        assert_eq!(second, StatusCode::CONFLICT);
        assert_eq!(err["code"], "CONFLICT");
        // A different name in the same Trust Domain is a distinct device → 201.
        let (third, _, _) =
            post_device(Some(&token), &td_id, Some(&device_body("Dup Laptop 2")), None).await;
        assert_eq!(third, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_device_in_unknown_trust_domain_is_not_found() {
        // A valid body + a well-formed but never-created trustDomainId → 404.
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, err) = post_device(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            Some(&device_body("Orphan")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn create_device_invalid_body_beats_the_parent_404() {
        // An invalid body against an unknown Trust Domain still reports the body 400
        // (validation is checked before the parent cross-reference).
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let bad = json!({ "enabled": true }); // deviceName missing
        let (status, _, err) = post_device(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            Some(&bad),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_device_reserved_suffix_on_subject_beats_a_valid_body() {
        // sub = "nad-429" → reserved suffix → 429, even before the parent lookup.
        let token = mint_token_as("nad-429", DEVICES_SCOPE).await;
        let (status, _, err) = post_device(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            Some(&device_body("Reserved")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn create_device_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, err) = post_device(
            Some(&token),
            "00000000-0000-4000-8000-000000000000",
            Some(&device_body("Forbidden")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn create_device_missing_token_is_unauthenticated() {
        let (status, _, err) = post_device(
            None,
            "00000000-0000-4000-8000-000000000000",
            Some(&device_body("NoAuth")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn create_device_x_correlator_is_echoed() {
        let td_id =
            make_trust_domain("nad-005", "3fa85f64-5717-4562-b3fc-2c963f66afa6", "TDD corr").await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, headers, _) = post_device(
            Some(&token),
            &td_id,
            Some(&device_body("Corr Laptop")),
            Some("corr-tdd-1"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-tdd-1")
        );
    }

    // === getTrustDomainDevice (GET /trust-domains/{id}/devices/{deviceId}) ==

    /// GET `/trust-domains/{td}/devices/{dev}` with an optional Bearer token and
    /// optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_device_req(
        token: Option<&str>,
        trust_domain_id: &str,
        device_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/network-access-domains/vwip/trust-domains/{trust_domain_id}/devices/{device_id}"
            ))
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

    /// Create a Trust Domain + a device inside it, returning `(trustDomainId,
    /// deviceId)` for the read-leg tests.
    async fn make_device(subject: &str, service_id: &str, td_name: &str, device_name: &str)
        -> (String, String) {
        let td_id = make_trust_domain(subject, service_id, td_name).await;
        let token = mint_token_as(subject, DEVICES_SCOPE).await;
        let (status, _, dev) =
            post_device(Some(&token), &td_id, Some(&device_body(device_name)), None).await;
        assert_eq!(status, StatusCode::CREATED);
        (td_id, dev["id"].as_str().unwrap().to_string())
    }

    #[tokio::test]
    async fn read_device_returns_the_created_device() {
        let (td_id, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD read ok",
            "Read Laptop",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;

        let (status, _, got) = get_device_req(Some(&token), &td_id, &dev_id, None).await;
        assert_eq!(status, StatusCode::OK);
        // Returned verbatim: the persisted id/name/lifecycle flags, no credential.
        assert_eq!(got["id"], json!(dev_id));
        assert_eq!(got["deviceName"], json!("Read Laptop"));
        assert_eq!(got["enabled"], json!(true));
        assert_eq!(got["connected"], json!(false));
        assert_eq!(got["associated"], json!(false));
        assert!(got.get("deviceCredential").is_none(), "credential must not be echoed");
    }

    #[tokio::test]
    async fn read_device_unknown_id_is_not_found() {
        let (td_id, _) = make_device(
            "nad-005",
            "123e4567-e89b-12d3-a456-426614174000",
            "TDD read unknown",
            "Present",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        // A well-formed but never-minted device id in an existing Trust Domain → 404.
        let (status, _, err) = get_device_req(
            Some(&token),
            &td_id,
            "00000000-0000-4000-8000-000000000000",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_device_in_unknown_trust_domain_is_not_found() {
        // The device store is keyed by the full (trustDomainId, deviceId) pair, so a
        // valid-shaped but unknown parent id finds nothing → 404.
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, err) = get_device_req(
            Some(&token),
            "11111111-1111-4111-8111-111111111111",
            "22222222-2222-4222-8222-222222222222",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_device_of_another_trust_domain_is_not_found() {
        // A device that exists in Trust Domain A is not visible under Trust Domain B
        // (the key mismatch folds into the same 404).
        let (_td_a, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD owner A",
            "Owned",
        )
        .await;
        let td_b = make_trust_domain(
            "nad-005",
            "9d5e6f70-1a2b-4c3d-8e4f-5a6b7c8d9e0f",
            "TDD owner B",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, err) = get_device_req(Some(&token), &td_b, &dev_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_device_malformed_id_is_not_found() {
        let (td_id, _) = make_device(
            "nad-005",
            "123e4567-e89b-12d3-a456-426614174000",
            "TDD read malformed",
            "Present2",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        // A malformed device id has no store entry to distinguish it → 404 (folded).
        let (status, _, err) = get_device_req(Some(&token), &td_id, "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_device_ignores_the_subject_reserved_suffix_plane() {
        // Unlike the create leg, the read leg is store-only (mirroring getTrustDomain):
        // the token subject is not a control plane, so a reserved-suffix subject that
        // would 429 on create still reads an existing device as 200.
        let (td_id, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD read no-subject-plane",
            "Shared",
        )
        .await;
        let token = mint_token_as("nad-429", DEVICES_SCOPE).await;
        let (status, _, got) = get_device_req(Some(&token), &td_id, &dev_id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(got["id"], json!(dev_id));
    }

    #[tokio::test]
    async fn read_device_token_without_the_scope_is_forbidden() {
        let (td_id, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD read forbidden",
            "Guarded",
        )
        .await;
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, err) = get_device_req(Some(&token), &td_id, &dev_id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn read_device_missing_token_is_unauthenticated() {
        let (status, _, err) = get_device_req(
            None,
            "11111111-1111-4111-8111-111111111111",
            "22222222-2222-4222-8222-222222222222",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn read_device_x_correlator_is_echoed() {
        let (td_id, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD read corr",
            "Corr Device",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, headers, _) =
            get_device_req(Some(&token), &td_id, &dev_id, Some("corr-tdd-read-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-tdd-read-1")
        );
    }

    // === getTrustDomainDevices (GET /trust-domains/{id}/devices) ===========

    /// GET `/trust-domains/{td}/devices` (the device list leg) with an optional
    /// Bearer token and optional `x-correlator`. Returns (status, headers,
    /// json-or-null).
    async fn list_devices_req(
        token: Option<&str>,
        trust_domain_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/network-access-domains/vwip/trust-domains/{trust_domain_id}/devices"
            ))
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

    #[tokio::test]
    async fn list_devices_returns_the_created_devices() {
        // A Trust Domain with two devices lists both, as a plain array of the
        // persisted TrustDomainDevice objects (no page wrapper, credential stripped).
        let td_id = make_trust_domain(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD list two",
        )
        .await;
        let dev_token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (s1, _, d1) =
            post_device(Some(&dev_token), &td_id, Some(&device_body("List One")), None).await;
        let (s2, _, d2) =
            post_device(Some(&dev_token), &td_id, Some(&device_body("List Two")), None).await;
        assert_eq!(s1, StatusCode::CREATED);
        assert_eq!(s2, StatusCode::CREATED);

        let (status, _, list) = list_devices_req(Some(&dev_token), &td_id, None).await;
        assert_eq!(status, StatusCode::OK);
        let items = list.as_array().expect("TrustDomainDeviceList is a JSON array");
        assert_eq!(items.len(), 2);
        let ids: Vec<&str> = items.iter().map(|d| d["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&d1["id"].as_str().unwrap()));
        assert!(ids.contains(&d2["id"].as_str().unwrap()));
        // Each entry is a full TrustDomainDevice with the write-only credential stripped.
        for d in items {
            assert!(d["deviceName"].is_string());
            assert_eq!(d["connected"], json!(false));
            assert!(d.get("deviceCredential").is_none(), "credential must not be echoed");
        }
    }

    #[tokio::test]
    async fn list_devices_empty_trust_domain_is_ok_empty_array() {
        // An existing Trust Domain with no devices yet → 200 with an empty array
        // (a list never 404s on an empty result once the parent resolves).
        let td_id = make_trust_domain(
            "nad-005",
            "123e4567-e89b-12d3-a456-426614174000",
            "TDD list empty",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, list) = list_devices_req(Some(&token), &td_id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn list_devices_unknown_trust_domain_is_not_found() {
        // A well-formed but never-created parent id → 404 (the collection is
        // scoped under a Trust Domain that must exist).
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, err) = list_devices_req(
            Some(&token),
            "11111111-1111-4111-8111-111111111111",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn list_devices_is_scoped_to_the_trust_domain() {
        // A device created in Trust Domain A does not appear in Trust Domain B's list.
        let (_td_a, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD list scope A",
            "Owned By A",
        )
        .await;
        let td_b = make_trust_domain(
            "nad-005",
            "9d5e6f70-1a2b-4c3d-8e4f-5a6b7c8d9e0f",
            "TDD list scope B",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, _, list) = list_devices_req(Some(&token), &td_b, None).await;
        assert_eq!(status, StatusCode::OK);
        // Trust Domain B has no devices of its own; A's device must not leak in.
        assert_eq!(list, json!([]));
        let ids: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id"].as_str().unwrap())
            .collect();
        assert!(!ids.contains(&dev_id.as_str()));
    }

    #[tokio::test]
    async fn list_devices_ignores_the_subject_reserved_suffix_plane() {
        // The list is store-only, so a reserved-error suffix on the token subject
        // (…429) does not shape it. Create the Trust Domain + device with a normal
        // subject, then list with a …429 token — the device still lists (200).
        let (td_id, dev_id) = make_device(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD list subj",
            "Subject Insensitive",
        )
        .await;
        let token = mint_token_as("nad-429", DEVICES_SCOPE).await;
        let (status, _, list) = list_devices_req(Some(&token), &td_id, None).await;
        assert_eq!(status, StatusCode::OK);
        let ids: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec![dev_id.as_str()]);
    }

    #[tokio::test]
    async fn list_devices_token_without_the_scope_is_forbidden() {
        let td_id = make_trust_domain(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD list forbidden",
        )
        .await;
        let token = mint_token_as("nad-005", "some:other:scope").await;
        let (status, _, _) = list_devices_req(Some(&token), &td_id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_devices_missing_token_is_unauthenticated() {
        let (status, _, _) =
            list_devices_req(None, "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn list_devices_x_correlator_is_echoed() {
        let td_id = make_trust_domain(
            "nad-005",
            "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "TDD list corr",
        )
        .await;
        let token = mint_token_as("nad-005", DEVICES_SCOPE).await;
        let (status, headers, _) =
            list_devices_req(Some(&token), &td_id, Some("corr-tdd-list-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-tdd-list-1")
        );
    }

    // === getTrustDomain (GET /trust-domains/{trustDomainId}) ==============

    /// GET `/trust-domains/{id}` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_trust_domain_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/network-access-domains/vwip/trust-domains/{id}"))
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

    #[tokio::test]
    async fn read_returns_the_created_trust_domain() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // Create one, then read it back by its minted id.
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD read ok");
        let (created, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(created, StatusCode::CREATED);
        let id = td["id"].as_str().unwrap().to_string();

        let (status, _, got) = get_trust_domain_req(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        // The stored resource is returned verbatim (same id/name/serviceId; the
        // write-only WPA password is still absent).
        assert_eq!(got["id"], json!(id));
        assert_eq!(got["name"], json!("TD read ok"));
        assert_eq!(got["serviceId"], body["serviceId"]);
        assert!(got["accessDetails"][0]["securityMode"].get("password").is_none());
    }

    #[tokio::test]
    async fn read_unknown_id_is_not_found() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // A well-formed but never-created id → 404 NOT_FOUND.
        let (status, _, err) =
            get_trust_domain_req(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_malformed_id_is_not_found() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // A non-UUID id folds into the same 404 (the opaque id is not a store key,
        // so unknown and malformed are indistinguishable), mirroring getService.
        let (status, _, err) = get_trust_domain_req(Some(&token), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_does_not_shadow_the_capabilities_path() {
        // `/trust-domains/capabilities` must still reach getTrustDomainCapabilities,
        // not the {trustDomainId} param route (static segments win in the router).
        let token = mint_token(TRUST_DOMAINS_SCOPE).await;
        let (status, _, doc) =
            get_trust_domain_req(Some(&token), "capabilities", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(doc["supportedAccessTypes"].is_array(), "got the capabilities doc");
    }

    #[tokio::test]
    async fn read_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, err) =
            get_trust_domain_req(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn read_missing_token_is_unauthenticated() {
        let (status, _, err) =
            get_trust_domain_req(None, "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn read_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("123e4567-e89b-12d3-a456-426614174000", "TD read corr");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        let (status, headers, _) =
            get_trust_domain_req(Some(&token), &id, Some("corr-td-read")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-td-read")
        );
    }

    // === deleteTrustDomain (DELETE /trust-domains/{trustDomainId}) =========

    /// DELETE `/trust-domains/{id}` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null — the `204` body is
    /// empty, so the value is `Null`).
    async fn delete_trust_domain_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("/network-access-domains/vwip/trust-domains/{id}"))
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

    #[tokio::test]
    async fn delete_evicts_the_created_trust_domain() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // Create one, delete it, then confirm a subsequent read is a 404.
        let body = td_body("7c9e6679-7425-40de-944b-e07fc1f90ae7", "TD delete ok");
        let (created, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(created, StatusCode::CREATED);
        let id = td["id"].as_str().unwrap().to_string();

        let (status, _, _) = delete_trust_domain_req(Some(&token), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(store::get(&id).is_none(), "the store entry is gone");

        // A read after delete is a 404.
        let (read, _, err) = get_trust_domain_req(Some(&token), &id, None).await;
        assert_eq!(read, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_is_single_use() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("16fd2706-8baf-433b-82eb-8c7fada847da", "TD delete twice");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        // First delete evicts (204); the second finds nothing → 404.
        let (first, _, _) = delete_trust_domain_req(Some(&token), &id, None).await;
        assert_eq!(first, StatusCode::NO_CONTENT);
        let (second, _, err) = delete_trust_domain_req(Some(&token), &id, None).await;
        assert_eq!(second, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_id_is_not_found() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // A well-formed but never-created id → 404 NOT_FOUND (store is the only plane).
        let (status, _, err) =
            delete_trust_domain_req(Some(&token), "00000000-0000-0000-0000-000000000000", None)
                .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_malformed_id_is_not_found() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // A non-UUID id folds into the same 404 (mirroring getTrustDomain).
        let (status, _, err) = delete_trust_domain_req(Some(&token), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, err) =
            delete_trust_domain_req(Some(&token), "00000000-0000-0000-0000-000000000000", None)
                .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn delete_missing_token_is_unauthenticated() {
        let (status, _, err) =
            delete_trust_domain_req(None, "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn delete_x_correlator_is_echoed_on_the_204() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("6ba7b810-9dad-11d1-80b4-00c04fd430c8", "TD delete corr");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        let (status, headers, _) =
            delete_trust_domain_req(Some(&token), &id, Some("corr-td-delete")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-td-delete")
        );
    }

    // === updateTrustDomain (PATCH /trust-domains/{trustDomainId}) ==========

    // --- Pure units --------------------------------------------------------

    #[test]
    fn validate_update_allows_empty_and_partial_bodies() {
        // Every field is optional — an empty body and any single-field body validate.
        assert!(validate_trust_domain_update(&json!({})).is_ok());
        assert!(validate_trust_domain_update(&json!({ "name": "Renamed" })).is_ok());
        assert!(validate_trust_domain_update(&json!({ "enabled": false })).is_ok());
        // The clearable optional fields accept an explicit null.
        assert!(validate_trust_domain_update(&json!({ "description": null })).is_ok());
        assert!(validate_trust_domain_update(&json!({ "expiration": null })).is_ok());
        assert!(validate_trust_domain_update(&json!({ "policies": null })).is_ok());
    }

    #[test]
    fn validate_update_rejects_ill_typed_present_fields() {
        // name present but blank / over-long / not a string / null → error.
        assert!(validate_trust_domain_update(&json!({ "name": "   " })).is_err());
        assert!(validate_trust_domain_update(&json!({ "name": "x".repeat(65) })).is_err());
        assert!(validate_trust_domain_update(&json!({ "name": 7 })).is_err());
        assert!(validate_trust_domain_update(&json!({ "name": null })).is_err());
        // enabled present but not a boolean / null → error (not clearable).
        assert!(validate_trust_domain_update(&json!({ "enabled": "yes" })).is_err());
        assert!(validate_trust_domain_update(&json!({ "enabled": null })).is_err());
        // description over-long; policies not an object.
        assert!(validate_trust_domain_update(&json!({ "description": "d".repeat(256) })).is_err());
        assert!(validate_trust_domain_update(&json!({ "policies": "nope" })).is_err());
        // accessDetails empty / too many / invalid entry / null → error.
        assert!(validate_trust_domain_update(&json!({ "accessDetails": [] })).is_err());
        assert!(validate_trust_domain_update(&json!({ "accessDetails": null })).is_err());
        let one = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "x")["accessDetails"][0].clone();
        assert!(validate_trust_domain_update(
            &json!({ "accessDetails": [one, one.clone(), one.clone(), one.clone(), one.clone()] })
        )
        .is_err());
        assert!(validate_trust_domain_update(
            &json!({ "accessDetails": [{ "accessType": "Thread:TLV" }] })
        )
        .is_err());
    }

    #[test]
    fn apply_update_sets_clears_replaces_and_restamps() {
        // Start from a rendered TrustDomain (as the store holds it).
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        let id = trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        let mut td = render_trust_domain(&id, &body, "2024-01-01T00:00:00Z", "creator-uuid");

        let patch = json!({
            "name": "Home Renamed",
            "enabled": false,
            "description": null,
            "accessDetails": [
                {
                    "accessType": "Wi-Fi:WPA_PERSONAL",
                    "ssid": "new-ssid",
                    "securityMode": { "password": "hidden", "securityModeType": "WPA2-Personal" }
                }
            ]
        });
        apply_trust_domain_update(&mut td, &patch, "2024-06-01T12:00:00Z", "editor-uuid");

        // Present scalars set; a null clears the optional field.
        assert_eq!(td["name"], json!("Home Renamed"));
        assert_eq!(td["enabled"], json!(false));
        assert!(td.get("description").is_none(), "null cleared description");
        // accessDetails replaced wholesale, write-only password stripped.
        assert_eq!(td["accessDetails"][0]["ssid"], json!("new-ssid"));
        assert!(td["accessDetails"][0]["securityMode"].get("password").is_none());
        // Immutable identity + creation audit untouched; modified* re-stamped.
        assert_eq!(td["id"], json!(id));
        assert_eq!(td["serviceId"], body["serviceId"]);
        assert_eq!(td["createdAt"], json!("2024-01-01T00:00:00Z"));
        assert_eq!(td["createdBy"], json!("creator-uuid"));
        assert_eq!(td["modifiedAt"], json!("2024-06-01T12:00:00Z"));
        assert_eq!(td["modifiedBy"], json!("editor-uuid"));
    }

    // --- Integration through the real router -------------------------------

    /// PATCH `/trust-domains/{id}` with an optional Bearer token, optional JSON
    /// body, and optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn patch_trust_domain_req(
        token: Option<&str>,
        id: &str,
        body: Option<&Value>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("PATCH")
            .uri(format!("/network-access-domains/vwip/trust-domains/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let req_body = match body {
            Some(v) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(v).unwrap())
            }
            None => Body::empty(),
        };
        let response = app().oneshot(builder.body(req_body).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn update_patches_fields_and_persists() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD patch ok");
        let (created, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(created, StatusCode::CREATED);
        let id = td["id"].as_str().unwrap().to_string();
        let created_at = td["createdAt"].as_str().unwrap().to_string();

        // Patch name + enabled; a serviceId in the body is ignored (immutable).
        let patch = json!({
            "name": "TD patched",
            "enabled": false,
            "serviceId": "00000000-0000-0000-0000-000000000000"
        });
        let (status, _, updated) = patch_trust_domain_req(Some(&token), &id, Some(&patch), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["name"], json!("TD patched"));
        assert_eq!(updated["enabled"], json!(false));
        // Immutable fields survive: id + serviceId unchanged, createdAt preserved.
        assert_eq!(updated["id"], json!(id));
        assert_eq!(updated["serviceId"], body["serviceId"]);
        assert_eq!(updated["createdAt"], json!(created_at));
        assert!(updated["modifiedBy"].is_string());

        // The change is persisted — a fresh read reflects it.
        let (read, _, got) = get_trust_domain_req(Some(&token), &id, None).await;
        assert_eq!(read, StatusCode::OK);
        assert_eq!(got["name"], json!("TD patched"));
        assert_eq!(got["enabled"], json!(false));
        assert_eq!(got["serviceId"], body["serviceId"]);
    }

    #[tokio::test]
    async fn update_replaces_access_details_and_strips_password() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("123e4567-e89b-12d3-a456-426614174000", "TD patch access");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        // Full-replacement accessDetails with a Thread entry; password never echoed.
        let patch = json!({
            "accessDetails": [
                {
                    "accessType": "Thread:STRUCTURED",
                    "channel": 15,
                    "extendedPanId": "d63e8e3e495ebbc3",
                    "networkKey": "dfd34f0f05cad978ec4e32b0413038ff",
                    "networkName": "Patched-Thread",
                    "panId": "0x1234"
                }
            ]
        });
        let (status, _, updated) = patch_trust_domain_req(Some(&token), &id, Some(&patch), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["accessDetails"].as_array().unwrap().len(), 1);
        assert_eq!(updated["accessDetails"][0]["accessType"], json!("Thread:STRUCTURED"));
        assert_eq!(updated["accessDetails"][0]["networkName"], json!("Patched-Thread"));
    }

    #[tokio::test]
    async fn update_clears_optional_field_with_null() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // td_body carries a description; a null in the patch removes it.
        let body = td_body("7c9e6679-7425-40de-944b-e07fc1f90ae7", "TD patch clear");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();
        assert_eq!(td["description"], json!("Primary home Wi-Fi"));

        let (status, _, updated) =
            patch_trust_domain_req(Some(&token), &id, Some(&json!({ "description": null })), None)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert!(updated.get("description").is_none(), "description cleared");
    }

    #[tokio::test]
    async fn update_empty_body_is_a_no_op_ok() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("16fd2706-8baf-433b-82eb-8c7fada847da", "TD patch empty");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        let (status, _, updated) =
            patch_trust_domain_req(Some(&token), &id, Some(&json!({})), None).await;
        assert_eq!(status, StatusCode::OK);
        // Nothing changed except the (re-stamped) modified audit fields.
        assert_eq!(updated["name"], json!("TD patch empty"));
        assert_eq!(updated["enabled"], json!(true));
    }

    #[tokio::test]
    async fn update_unknown_id_is_not_found() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let (status, _, err) = patch_trust_domain_req(
            Some(&token),
            "00000000-0000-0000-0000-000000000000",
            Some(&json!({ "enabled": false })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn update_malformed_id_is_not_found() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // A non-UUID id folds into the same 404 (mirroring getTrustDomain).
        let (status, _, err) =
            patch_trust_domain_req(Some(&token), "not-a-uuid", Some(&json!({})), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn update_invalid_body_is_bad_request() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("6ba7b810-9dad-11d1-80b4-00c04fd430c8", "TD patch bad body");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        // A present-but-blank name → 400 INVALID_ARGUMENT.
        let (status, _, err) =
            patch_trust_domain_req(Some(&token), &id, Some(&json!({ "name": "  " })), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn update_body_400_beats_unknown_id_404() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // Body is validated before the store lookup, so a bad body on an
        // never-created id is a 400, not a 404.
        let (status, _, err) = patch_trust_domain_req(
            Some(&token),
            "00000000-0000-0000-0000-000000000000",
            Some(&json!({ "enabled": "not-a-bool" })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn update_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, err) = patch_trust_domain_req(
            Some(&token),
            "00000000-0000-0000-0000-000000000000",
            Some(&json!({})),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn update_missing_token_is_unauthenticated() {
        let (status, _, err) = patch_trust_domain_req(
            None,
            "00000000-0000-0000-0000-000000000000",
            Some(&json!({})),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn update_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("a1b2c3d4-0000-4000-8000-000000000000", "TD patch corr");
        let (_, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        let id = td["id"].as_str().unwrap().to_string();

        let (status, headers, _) =
            patch_trust_domain_req(Some(&token), &id, Some(&json!({})), Some("corr-td-patch")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-td-patch")
        );
    }
}
