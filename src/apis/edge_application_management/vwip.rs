//! Edge Application Management **vwip** (CAMARA EdgeApplicationManagement `wip`).
//!
//! Endpoints so far:
//! - `GET /edge-application-management/vwip/edge-cloud-zones` — list the edge
//!   cloud zones the operator offers for application placement (operationId
//!   `getEdgeCloudZones`, scope `edge-application-management:edge-cloud-zones:read`).
//! - `POST /edge-application-management/vwip/apps` — submit (onboard) an
//!   application, minting an `appId` (operationId `submitApp`, scope
//!   `edge-application-management:apps:write`). The first **stateful** leg — see
//!   [`submit_app`] and [`crate::apis::edge_application_management::store`].
//! - `GET /edge-application-management/vwip/apps/{appId}` — read back an
//!   onboarded application as the CAMARA `AppManifestInfo` (operationId `getApp`,
//!   scope `edge-application-management:apps:read`). See [`get_app`].
//! - `GET /edge-application-management/vwip/apps` — list every onboarded
//!   application as an array of `AppManifestInfo` (operationId `getApps`, scope
//!   `edge-application-management:apps:read`). See [`get_apps`].
//! - `DELETE /edge-application-management/vwip/apps/{appId}` — delete (de-board)
//!   an onboarded application (operationId `deleteApp`, scope
//!   `edge-application-management:apps:delete`). See [`delete_app`].
//!
//! ## What it does
//!
//! CamaraSim serves a **fixed catalog** of edge cloud zones (there is no upstream
//! orchestrator to query — DESIGN §7). Each zone is a static
//! `(edgeCloudZoneName, edgeCloudProvider, edgeCloudRegion, edgeCloudZoneStatus)`
//! tuple, mirroring the EdgeCloud zone shape shared across the already-mounted
//! EdgeCloud family (Simple / Optimal Edge Discovery, Application Endpoint
//! Discovery). The endpoint returns the catalog as a JSON array, filtered by the
//! request's query parameters.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes drive *which* zones come back (there is no device
//! identifier here, so no reserved-error plane — this is a pure catalog, like
//! QoS Profiles):
//!
//! 1. **`region`** — return only zones whose `edgeCloudRegion` matches exactly.
//!    An unknown region yields an empty array (a *list* never 404s).
//! 2. **`status`** — return only zones in that lifecycle state (`active`,
//!    `inactive`, `unknown`). An unrecognised value → `400 INVALID_ARGUMENT`.
//!
//! Both filters combine (AND). With neither, the whole catalog is returned.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `edge-application-management:edge-cloud-zones:read` scope. `x-correlator` is
//! echoed on every response.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;

use super::instance_store;
use super::store;

/// The OAuth2 scope `getEdgeCloudZones` requires (CAMARA EdgeApplicationManagement).
const ZONES_SCOPE: &str = "edge-application-management:edge-cloud-zones:read";

/// The OAuth2 scope `submitApp` requires (CAMARA EdgeApplicationManagement).
const APPS_WRITE_SCOPE: &str = "edge-application-management:apps:write";

/// The OAuth2 scope `getApp` requires (CAMARA EdgeApplicationManagement).
const APPS_READ_SCOPE: &str = "edge-application-management:apps:read";

/// The OAuth2 scope `deleteApp` requires (CAMARA EdgeApplicationManagement).
const APPS_DELETE_SCOPE: &str = "edge-application-management:apps:delete";

/// The OAuth2 scope `createAppInstance` requires (CAMARA EdgeApplicationManagement).
const INSTANCES_WRITE_SCOPE: &str = "edge-application-management:instances:write";

/// The five CAMARA `AppManifest.packageType` values.
const PACKAGE_TYPES: [&str; 5] = ["QCOW2", "OVA", "CONTAINER", "HELM", "CSAR"];

/// Routes for Edge Application Management vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/edge-application-management/vwip/edge-cloud-zones",
            get(get_edge_cloud_zones),
        )
        .route(
            "/edge-application-management/vwip/apps",
            post(submit_app).get(get_apps),
        )
        .route(
            "/edge-application-management/vwip/apps/:app_id",
            get(get_app).delete(delete_app),
        )
        .route(
            "/edge-application-management/vwip/app-instances",
            post(create_app_instance),
        )
}

/// The operator's fixed edge cloud zones:
/// `(edgeCloudZoneName, edgeCloudProvider, edgeCloudRegion, edgeCloudZoneStatus)`.
///
/// The zone names mirror the EdgeCloud family's naming (optimal-edge-discovery);
/// the providers and statuses are spread so both the `region` and `status`
/// filters are meaningful control planes (every `EdgeCloudZoneStatus` value is
/// represented). The `edgeCloudZoneId` is derived from the name (stable per
/// zone — see [`zone_id`]).
const EDGE_ZONES: [(&str, &str, &str, &str); 6] = [
    ("camarasim-edge-eu-west-1", "CamaraSim Edge", "eu-west-1", "active"),
    ("camarasim-edge-eu-central-1", "CamaraSim Edge", "eu-central-1", "active"),
    ("camarasim-edge-us-east-1", "CamaraSim Edge", "us-east-1", "active"),
    ("camarasim-edge-us-west-2", "Partner Cloud", "us-west-2", "inactive"),
    ("camarasim-edge-ap-south-1", "Partner Cloud", "ap-south-1", "unknown"),
    ("camarasim-edge-ap-northeast-1", "CamaraSim Edge", "ap-northeast-1", "active"),
];

/// The three CAMARA `EdgeCloudZoneStatus` values, accepted by the `status` filter.
const VALID_STATUS: [&str; 3] = ["active", "inactive", "unknown"];

/// `GET /edge-application-management/vwip/edge-cloud-zones` (`getEdgeCloudZones`).
async fn get_edge_cloud_zones(
    claims: Claims,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(ZONES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Decode the optional `region` / `status` query filters.
    let raw: Raw = match serde_urlencoded::from_str(query.as_deref().unwrap_or("")) {
        Ok(raw) => raw,
        Err(_) => {
            return invalid_argument(
                "the query string is not valid application/x-www-form-urlencoded",
                &correlator,
            )
        }
    };

    // A supplied `status` must be one of the three EdgeCloudZoneStatus values.
    if let Some(status) = &raw.status {
        if !VALID_STATUS.contains(&status.as_str()) {
            return invalid_argument(
                "`status` must be one of `active`, `inactive`, `unknown`.",
                &correlator,
            );
        }
    }

    // Filter the fixed catalog by the (validated) region/status filters (AND).
    let zones: Vec<Value> = EDGE_ZONES
        .iter()
        .filter(|z| {
            raw.region.as_deref().is_none_or(|r| z.2 == r)
                && raw.status.as_deref().is_none_or(|s| z.3 == s)
        })
        .map(|z| {
            json!({
                "edgeCloudZoneId": zone_id(z.0),
                "edgeCloudZoneName": z.0,
                "edgeCloudZoneStatus": z.3,
                "edgeCloudProvider": z.1,
                "edgeCloudRegion": z.2,
            })
        })
        .collect();

    with_correlator(
        (StatusCode::OK, Json(Value::Array(zones))).into_response(),
        &correlator,
    )
}

/// Query parameters for `getEdgeCloudZones` — both optional (an empty query
/// returns the whole catalog). Unknown keys are rejected (`deny_unknown_fields`).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    region: Option<String>,
    status: Option<String>,
}

/// `POST /edge-application-management/vwip/apps` (`submitApp`).
///
/// Submits (onboards) an application: the caller sends an `AppManifest`, the
/// simulator validates it, mints an `appId`, persists the manifest in the
/// in-memory [`store`], and returns `201 { appId }` (`SubmittedApp`).
///
/// There is no upstream orchestrator, so the two control planes are the input
/// alone (docs/DESIGN.md §7):
///
/// 1. **Request validation** — a missing/blank required field, a `name` that
///    violates the CAMARA pattern, an unknown `packageType`, or an empty
///    `componentSpec` → `400 INVALID_ARGUMENT`.
/// 2. **Store state** — the `appId` is derived deterministically from the app's
///    identity (`name` + `version` + `appProvider`; see [`app_id`]), so
///    submitting the *same* application twice collides → `409 ALREADY_EXISTS`.
///
/// The nested `appRepo` / `requiredResources` / `componentSpec` item shapes are
/// checked only for presence/non-emptiness — the full `oneOf`
/// (`KubernetesResources` / `VmResources` / …) validation is a documented cut.
async fn submit_app(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's write scope.
    if let Err(e) = claims.require_scope(APPS_WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Parse the AppManifest body (malformed JSON / wrong field types → 400).
    let manifest: AppManifest = match serde_json::from_slice(&body) {
        Ok(m) => m,
        Err(_) => {
            return invalid_argument(
                "the request body is not a valid AppManifest JSON object",
                &correlator,
            )
        }
    };

    // Control plane 1 — required-field / shape validation.
    if let Err(message) = manifest.validate() {
        return invalid_argument(&message, &correlator);
    }

    // Identity fields are guaranteed present by `validate()` above.
    let name = manifest.name.as_deref().unwrap_or_default();
    let version = manifest.version.as_deref().unwrap_or_default();
    let provider = manifest.app_provider.clone().unwrap_or(Value::Null);
    let app_id = app_id(name, version, &provider);

    // Control plane 2 — store state (re-submitting the same app → 409).
    let stored: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    if !store::insert(app_id.clone(), stored) {
        return with_correlator(
            CamaraError::new(
                StatusCode::CONFLICT,
                "ALREADY_EXISTS",
                "App already exists",
            )
            .into_response(),
            &correlator,
        );
    }

    with_correlator(
        (StatusCode::CREATED, Json(json!({ "appId": app_id }))).into_response(),
        &correlator,
    )
}

/// `GET /edge-application-management/vwip/apps/{appId}` (`getApp`).
///
/// Reads back an application onboarded by [`submit_app`]. On success it returns
/// the stored `AppManifest` with the minted `appId` merged in — the CAMARA
/// `AppManifestInfo` (`allOf` `AppManifest` + `appId`) — with a `200`.
///
/// The `appId` is an opaque, simulator-minted UUID (there is no reserved-error
/// plane — the identifier is not caller-chosen), so the **store state is the
/// only control plane** (docs/DESIGN.md §7): a known id → `200 AppManifestInfo`;
/// an unknown or malformed id → `404 NOT_FOUND` (the canonical 400 malformed-path
/// case is folded into 404, mirroring the sibling `readAccess`/`readNetwork`
/// read legs). `x-correlator` is echoed on every response.
async fn get_app(claims: Claims, headers: HeaderMap, Path(app_id): Path<String>) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's read scope.
    if let Err(e) = claims.require_scope(APPS_READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&app_id) {
        Some(manifest) => with_correlator(
            (StatusCode::OK, Json(app_manifest_info(app_id, manifest))).into_response(),
            &correlator,
        ),
        None => with_correlator(
            CamaraError::not_found("No application found for the provided appId.").into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /edge-application-management/vwip/apps/{appId}` (`deleteApp`).
///
/// Deletes (de-boards) an application previously onboarded with [`submit_app`],
/// evicting its stored `AppManifest` from the in-memory store. Requires the
/// delete scope `edge-application-management:apps:delete`.
///
/// Keyed only on the **store state** (docs/DESIGN.md §7): the `appId` is an
/// opaque, simulator-minted UUID (not caller-chosen), so there is no
/// reserved-error plane. A known id evicts its app and returns `204 No Content`
/// (single-use); an unknown, already-deleted, *or malformed* id → `404
/// NOT_FOUND` (the canonical 400 malformed-path case is folded into 404,
/// mirroring [`get_app`] and the sibling `deleteNetwork`/`deleteAccess` legs).
///
/// Deletion is synchronous — CAMARA EdgeApplicationManagement's async
/// `202 Accepted`/`DELETE_REQUESTED` form and any `sink` notification are a
/// documented cut, mirroring every other CamaraSim delete leg (QoD
/// `deleteSession`, `deleteNetwork`, `deleteAccess`). `x-correlator` is echoed on
/// every response.
async fn delete_app(claims: Claims, headers: HeaderMap, Path(app_id): Path<String>) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's delete scope.
    if let Err(e) = claims.require_scope(APPS_DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::remove(&app_id) {
        Some(_) => with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No application found for the provided appId.").into_response(),
            &correlator,
        ),
    }
}

/// `GET /edge-application-management/vwip/apps` (`getApps`).
///
/// Lists every application onboarded by [`submit_app`], as a JSON array of
/// CAMARA `AppManifestInfo` (each stored `AppManifest` with its minted `appId`
/// merged in — the same representation [`get_app`] returns for one app). This
/// mirrors the sibling CamaraSim list legs (`listAccesses`, `retrievePayments`):
/// a list returns the same resource shape as its single-item read.
///
/// The apps are simulator-minted and not caller-chosen, so there is no
/// reserved-error plane — the in-memory store is the only control plane
/// (docs/DESIGN.md §7): the response is the current store snapshot (an empty
/// array when nothing has been onboarded — a *list* never 404s). `x-correlator`
/// is echoed on every response.
async fn get_apps(claims: Claims, headers: HeaderMap) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's read scope.
    if let Err(e) = claims.require_scope(APPS_READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let apps: Vec<Value> = store::all()
        .into_iter()
        .map(|(app_id, manifest)| app_manifest_info(app_id, manifest))
        .collect();

    with_correlator(
        (StatusCode::OK, Json(Value::Array(apps))).into_response(),
        &correlator,
    )
}

/// Build a CAMARA `AppManifestInfo` — the stored `AppManifest` with its assigned
/// `appId` merged in (`allOf` `AppManifest` + `appId`). Shared by `getApp` and
/// `getApps` so both render the onboarded application identically.
fn app_manifest_info(app_id: String, mut manifest: Value) -> Value {
    if let Value::Object(map) = &mut manifest {
        map.insert("appId".to_string(), Value::String(app_id));
    }
    manifest
}

/// The CAMARA `createAppInstance` request body. Every field is captured as an
/// `Option` so a *missing* required field is reported as a precise `400
/// INVALID_ARGUMENT` (rather than a serde rejection). `subscriptionRequest` is
/// accepted-not-applied (status-change notifications are a later slice), so it
/// is not modelled here.
#[derive(Debug, Deserialize)]
struct CreateAppInstance {
    name: Option<String>,
    #[serde(rename = "appId")]
    app_id: Option<String>,
    #[serde(rename = "edgeCloudZoneId")]
    edge_cloud_zone_id: Option<String>,
    #[serde(rename = "kubernetesClusterRef")]
    kubernetes_cluster_ref: Option<String>,
}

/// `POST /edge-application-management/vwip/app-instances` (`createAppInstance`).
///
/// Instantiates an onboarded application onto a specific edge cloud zone: the
/// caller sends the app instance `name`, the `appId` of an onboarded app, and
/// the `edgeCloudZoneId` of a target zone; the simulator validates the request,
/// mints an `appInstanceId`, renders the `AppInstanceInfo`, persists it in the
/// in-memory [`instance_store`], and returns `202 Accepted` with a `Location`
/// header (CAMARA models instantiation as asynchronous).
///
/// There is no upstream orchestrator, so the outcome is driven by the input plus
/// the two in-memory stores (docs/DESIGN.md §7):
///
/// 1. **Request validation** — a missing/blank/invalid `name`, a missing or
///    non-UUID `appId` / `edgeCloudZoneId`, or a non-UUID `kubernetesClusterRef`
///    → `400 INVALID_ARGUMENT`.
/// 2. **Cross-reference** — the `appId` must name an app onboarded via
///    `submitApp` and the `edgeCloudZoneId` must name a zone in the fixed
///    catalog; either miss → `404 NOT_FOUND`.
/// 3. **Store state** — the `appInstanceId` is derived deterministically from
///    the `(appId, edgeCloudZoneId)` pair (see [`instance_id`]), so instantiating
///    the *same* app onto the *same* zone collides → `409 ALREADY_EXISTS` (the
///    CAMARA "already instantiated in the given Edge Cloud Zone" conflict).
///
/// The reported `status` is a genuine second control plane: it is derived from
/// the target zone's own catalog `edgeCloudZoneStatus` (see [`instance_status`]),
/// so the chosen zone selects `ready` / `failed` / `instantiating`. The
/// `componentEndpointInfo` (runtime endpoints) is omitted — there is no live
/// workload — a documented cut. `x-correlator` is echoed on every response.
async fn create_app_instance(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's instances scope.
    if let Err(e) = claims.require_scope(INSTANCES_WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Parse the request body (malformed JSON / wrong field types → 400).
    let req: CreateAppInstance = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            return invalid_argument(
                "the request body is not a valid createAppInstance JSON object",
                &correlator,
            )
        }
    };

    // Control plane 1 — required-field / shape validation.
    let name = match req.name.as_deref() {
        None => return invalid_argument("`name` is required", &correlator),
        Some(n) if !is_valid_app_name(n) => {
            return invalid_argument(
                "`name` must match `^[A-Za-z][A-Za-z0-9_]{1,63}$`",
                &correlator,
            )
        }
        Some(n) => n,
    };
    let app_id = match req.app_id.as_deref() {
        None => return invalid_argument("`appId` is required", &correlator),
        Some(a) if !is_uuid(a) => {
            return invalid_argument("`appId` must be a UUID", &correlator)
        }
        Some(a) => a,
    };
    let zone_id_in = match req.edge_cloud_zone_id.as_deref() {
        None => return invalid_argument("`edgeCloudZoneId` is required", &correlator),
        Some(z) if !is_uuid(z) => {
            return invalid_argument("`edgeCloudZoneId` must be a UUID", &correlator)
        }
        Some(z) => z,
    };
    if let Some(k) = req.kubernetes_cluster_ref.as_deref() {
        if !is_uuid(k) {
            return invalid_argument("`kubernetesClusterRef` must be a UUID", &correlator);
        }
    }

    // Control plane 2 — cross-reference the two in-memory stores. The app must be
    // onboarded (so its `appProvider` can be echoed) and the zone must exist.
    let manifest = match store::get(app_id) {
        Some(m) => m,
        None => {
            return with_correlator(
                CamaraError::not_found("No application found for the provided appId.")
                    .into_response(),
                &correlator,
            )
        }
    };
    let zone = match zone_by_id(zone_id_in) {
        Some(z) => z,
        None => {
            return with_correlator(
                CamaraError::not_found(
                    "No edge cloud zone found for the provided edgeCloudZoneId.",
                )
                .into_response(),
                &correlator,
            )
        }
    };

    // Render the AppInstanceInfo. `appProvider` is echoed from the onboarded
    // app's manifest; `status` is derived from the target zone's catalog status.
    let instance_id = instance_id(app_id, zone_id_in);
    let provider = manifest.get("appProvider").cloned().unwrap_or(Value::Null);
    let mut info = json!({
        "appInstanceId": instance_id,
        "name": name,
        "appId": app_id,
        "appProvider": provider,
        "edgeCloudZoneId": zone_id_in,
        "status": instance_status(zone.3),
    });
    if let Some(k) = req.kubernetes_cluster_ref.as_deref() {
        info["kubernetesClusterRef"] = json!(k);
    }

    // Control plane 3 — store state (same app on the same zone → 409).
    if !instance_store::insert(instance_id.clone(), info.clone()) {
        return with_correlator(
            CamaraError::new(
                StatusCode::CONFLICT,
                "ALREADY_EXISTS",
                "Application already instantiated in the given Edge Cloud Zone",
            )
            .into_response(),
            &correlator,
        );
    }

    let location = format!("/edge-application-management/vwip/app-instances/{instance_id}");
    let mut response = (StatusCode::ACCEPTED, Json(info)).into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("location"), value);
    }
    with_correlator(response, &correlator)
}

/// The subset of CAMARA `AppManifest` fields CamaraSim validates. All are
/// captured as `Option`/`Value` so a *missing* required field is reported as a
/// precise `400 INVALID_ARGUMENT` (rather than a serde rejection), and the
/// nested shapes we don't fully model pass through untouched.
#[derive(Debug, Deserialize)]
struct AppManifest {
    name: Option<String>,
    version: Option<String>,
    #[serde(rename = "appProvider")]
    app_provider: Option<Value>,
    #[serde(rename = "packageType")]
    package_type: Option<String>,
    #[serde(rename = "appRepo")]
    app_repo: Option<Value>,
    #[serde(rename = "requiredResources")]
    required_resources: Option<Value>,
    #[serde(rename = "componentSpec")]
    component_spec: Option<Value>,
}

impl AppManifest {
    /// Validate the required fields, returning a human-readable message on the
    /// first violation (mapped by the caller to `400 INVALID_ARGUMENT`).
    fn validate(&self) -> Result<(), String> {
        match self.name.as_deref() {
            None => return Err("`name` is required".into()),
            Some(name) if !is_valid_app_name(name) => {
                return Err(
                    "`name` must match `^[A-Za-z][A-Za-z0-9_]{1,63}$`".into(),
                )
            }
            Some(_) => {}
        }
        if self.version.as_deref().unwrap_or_default().is_empty() {
            return Err("`version` is required".into());
        }
        if !self.app_provider.as_ref().is_some_and(|v| !v.is_null()) {
            return Err("`appProvider` is required".into());
        }
        match self.package_type.as_deref() {
            None => return Err("`packageType` is required".into()),
            Some(pt) if !PACKAGE_TYPES.contains(&pt) => {
                return Err(
                    "`packageType` must be one of QCOW2, OVA, CONTAINER, HELM, CSAR"
                        .into(),
                )
            }
            Some(_) => {}
        }
        if !self.app_repo.as_ref().is_some_and(|v| !v.is_null()) {
            return Err("`appRepo` is required".into());
        }
        if !self.required_resources.as_ref().is_some_and(|v| !v.is_null()) {
            return Err("`requiredResources` is required".into());
        }
        match self.component_spec.as_ref() {
            Some(Value::Array(items)) if !items.is_empty() => {}
            _ => return Err("`componentSpec` must be a non-empty array".into()),
        }
        Ok(())
    }
}

/// Whether `name` matches the CAMARA `AppManifest.name` pattern
/// `^[A-Za-z][A-Za-z0-9_]{1,63}$` (2–64 chars, starting with a letter, the rest
/// letters/digits/underscore). Checked by hand — no `regex` dependency.
fn is_valid_app_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if !(2..=64).contains(&bytes.len()) {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&c| c.is_ascii_alphanumeric() || c == b'_')
}

/// A stable, RFC 4122 (version 5, name-based) UUID `appId` for a submitted
/// application, derived from its identity — the `(name, version, appProvider)`
/// triple — via SHA-256 (deterministic, no new dependency). The version (`5`)
/// and variant nibbles are forced so the id satisfies the strict
/// `SubmittedApp.appId` UUID pattern the CAMARA schema requires. Deriving the id
/// from the identity is what makes a duplicate submission collide (→ `409
/// ALREADY_EXISTS`).
pub fn app_id(name: &str, version: &str, provider: &Value) -> String {
    // Canonical JSON of the provider so the identity is stable regardless of
    // whether `appProvider` is a string or a structured object.
    let provider = serde_json::to_string(provider).unwrap_or_default();
    let mut h = Sha256::digest(format!("eam-app:{name}\u{1f}{version}\u{1f}{provider}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x50; // version 5
    h[8] = (h[8] & 0x3f) | 0x80; // variant (10xx)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// A stable, RFC 4122 (version 5, name-based) UUID `edgeCloudZoneId` for a zone,
/// derived from its name via SHA-256 (deterministic, no new dependency). The
/// version (`5`) and variant nibbles are forced so the id satisfies the strict
/// `EdgeCloudZone.edgeCloudZoneId` UUID pattern the CAMARA schema requires.
fn zone_id(zone_name: &str) -> String {
    let mut h = Sha256::digest(format!("eam-zone:{zone_name}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x50; // version 5
    h[8] = (h[8] & 0x3f) | 0x80; // variant (10xx)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// Find the fixed-catalog zone whose derived [`zone_id`] equals `id`, if any.
/// Backs `createAppInstance`'s zone cross-reference: an `edgeCloudZoneId` that
/// names no catalog zone → `404 NOT_FOUND`.
fn zone_by_id(id: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    EDGE_ZONES.iter().find(|z| zone_id(z.0) == id)
}

/// Map a target zone's catalog `edgeCloudZoneStatus` to the app instance's
/// `status`, making the chosen zone a genuine second control plane
/// (docs/DESIGN.md §7): an `active` zone instantiates cleanly (`ready`), an
/// `inactive` zone can't host the workload (`failed`), and an `unknown`-status
/// zone is still bringing it up (`instantiating`). The `terminating` / `unknown`
/// instance states are teardown/degraded transitions not reachable on create (a
/// documented cut).
fn instance_status(zone_status: &str) -> &'static str {
    match zone_status {
        "active" => "ready",
        "inactive" => "failed",
        _ => "instantiating",
    }
}

/// A stable, RFC 4122 (version 5, name-based) UUID `appInstanceId` for an app
/// instance, derived from the `(appId, edgeCloudZoneId)` pair via SHA-256
/// (deterministic, no new dependency). The version (`5`) and variant nibbles are
/// forced so the id satisfies the strict CAMARA UUID pattern. Deriving the id
/// from the (app, zone) pair is what makes a duplicate instantiation collide (→
/// `409 ALREADY_EXISTS`), matching CAMARA's "already instantiated in the given
/// Edge Cloud Zone" conflict.
fn instance_id(app_id: &str, zone_id: &str) -> String {
    let mut h =
        Sha256::digest(format!("eam-instance:{app_id}\u{1f}{zone_id}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x50; // version 5
    h[8] = (h[8] & 0x3f) | 0x80; // variant (10xx)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// Whether `s` matches the strict CAMARA UUID pattern
/// `^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`.
/// Checked by hand — no `regex` dependency. Used by `createAppInstance` to
/// reject a malformed `appId` / `edgeCloudZoneId` / `kubernetesClusterRef` with
/// `400 INVALID_ARGUMENT`.
fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    let hex = |c: u8| c.is_ascii_digit() || (b'a'..=b'f').contains(&c);
    for (i, &c) in b.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if c != b'-' {
                    return false;
                }
            }
            14 => {
                if !(b'1'..=b'5').contains(&c) {
                    return false;
                }
            }
            19 => {
                if !matches!(c, b'8' | b'9' | b'a' | b'b') {
                    return false;
                }
            }
            _ => {
                if !hex(c) {
                    return false;
                }
            }
        }
    }
    true
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

    const HOST: &str = "edgeappmgmt.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn catalog_covers_every_status_and_ids_are_valid_uuids() {
        // Every EdgeCloudZoneStatus value is represented (so the `status` filter
        // is a real control plane), and each derived id is a strict RFC 4122 UUID.
        for status in VALID_STATUS {
            assert!(
                EDGE_ZONES.iter().any(|z| z.3 == status),
                "catalog covers status {status}"
            );
        }
        for z in EDGE_ZONES {
            let id = zone_id(z.0);
            assert!(is_uuid(&id), "zone id {id} is a UUID");
            assert_eq!(&id[14..15], "5", "version nibble is 5 for {id}");
            assert!(
                matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b'),
                "variant nibble is 8/9/a/b for {id}"
            );
        }
    }

    #[test]
    fn zone_id_is_stable_and_distinct_per_zone() {
        assert_eq!(zone_id("camarasim-edge-eu-west-1"), zone_id("camarasim-edge-eu-west-1"));
        assert_ne!(zone_id("camarasim-edge-eu-west-1"), zone_id("camarasim-edge-us-east-1"));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body =
            format!("grant_type=client_credentials&client_id=eam-client&scope={scope}");
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

    async fn get_zones(
        token: Option<&str>,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let uri = if query.is_empty() {
            "/edge-application-management/vwip/edge-cloud-zones".to_string()
        } else {
            format!("/edge-application-management/vwip/edge-cloud-zones?{query}")
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
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    async fn get_zones_ok(query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(ZONES_SCOPE).await;
        get_zones(Some(&token), query, None).await
    }

    #[tokio::test]
    async fn no_filter_returns_the_whole_catalog() {
        let (status, _, body) = get_zones_ok("").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("array response");
        assert_eq!(arr.len(), EDGE_ZONES.len());
        // Each item carries the required CAMARA fields.
        assert!(arr.iter().all(|z| z["edgeCloudZoneId"].is_string()
            && z["edgeCloudZoneName"].is_string()
            && z["edgeCloudProvider"].is_string()
            && z["edgeCloudZoneStatus"].is_string()));
    }

    #[tokio::test]
    async fn region_filter_narrows_to_matching_zones() {
        let (status, _, body) = get_zones_ok("region=us-east-1").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["edgeCloudRegion"], "us-east-1");
        assert_eq!(arr[0]["edgeCloudZoneName"], "camarasim-edge-us-east-1");
    }

    #[tokio::test]
    async fn unknown_region_returns_an_empty_array_not_404() {
        let (status, _, body) = get_zones_ok("region=no-such-region").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn status_filter_narrows_the_catalog() {
        let (status, _, body) = get_zones_ok("status=inactive").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert!(!arr.is_empty());
        assert!(arr.iter().all(|z| z["edgeCloudZoneStatus"] == "inactive"));

        let (status, _, body) = get_zones_ok("status=active").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.as_array().unwrap().iter().all(|z| z["edgeCloudZoneStatus"] == "active"));
    }

    #[tokio::test]
    async fn region_and_status_combine() {
        // eu-west-1 is active → matches.
        let (status, _, body) = get_zones_ok("region=eu-west-1&status=active").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1);

        // us-west-2 is inactive, not active → empty.
        let (status, _, body) = get_zones_ok("region=us-west-2&status=active").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn unknown_status_value_is_rejected() {
        let (status, _, body) = get_zones_ok("status=RETIRED").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_query_param_is_rejected() {
        let (status, _, body) = get_zones_ok("foo=bar").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_zones(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_zones(None, "", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(ZONES_SCOPE).await;
        let (status, headers, _) = get_zones(Some(&token), "", Some("corr-eam")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-eam")
        );

        let (status, headers, _) =
            get_zones(Some(&token), "status=nope", Some("corr-err")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- submitApp: pure units --------------------------------------------

    #[test]
    fn app_name_pattern_is_enforced() {
        assert!(is_valid_app_name("Ab"));
        assert!(is_valid_app_name("My_App_1"));
        assert!(is_valid_app_name(&format!("A{}", "b".repeat(63)))); // 64 chars
        assert!(!is_valid_app_name("A")); // too short (min 2)
        assert!(!is_valid_app_name("1abc")); // must start with a letter
        assert!(!is_valid_app_name("my-app")); // hyphen not allowed
        assert!(!is_valid_app_name("")); // empty
        assert!(!is_valid_app_name(&format!("A{}", "b".repeat(64)))); // 65 chars
    }

    #[test]
    fn app_id_is_deterministic_uuid_shaped_and_identity_keyed() {
        let p = json!("Acme");
        let a = app_id("MyApp", "1.0.0", &p);
        assert_eq!(a, app_id("MyApp", "1.0.0", &p), "stable per identity");
        // A UUID shape satisfying the strict CAMARA pattern.
        assert!(is_uuid(&a), "app id {a} is a UUID");
        assert_eq!(&a[14..15], "5", "version nibble is 5");
        // Each identity coordinate changes the id.
        assert_ne!(a, app_id("MyApp", "2.0.0", &p));
        assert_ne!(a, app_id("OtherApp", "1.0.0", &p));
        assert_ne!(a, app_id("MyApp", "1.0.0", &json!("Beta")));
    }

    // --- submitApp: integration through the real router -------------------

    /// A complete, valid `AppManifest` body for app `name` (identity keyed on
    /// `name`/`version`/`appProvider`, so a distinct `name` per test avoids the
    /// process-global store colliding across parallel tests).
    fn manifest(name: &str) -> Value {
        json!({
            "name": name,
            "version": "1.0.0",
            "appProvider": "CamaraSim Test",
            "packageType": "CONTAINER",
            "appRepo": { "type": "PUBLICREPO", "imagePath": "https://repo.example/app:1" },
            "requiredResources": { "infraKind": "CONTAINER" },
            "componentSpec": [
                { "componentName": "web", "networkInterfaces": [
                    { "interfaceId": "eth0", "protocol": "TCP", "port": 8080,
                      "visibilityType": "VISIBILITY_EXTERNAL" }
                ] }
            ]
        })
    }

    async fn post_app(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/edge-application-management/vwip/apps")
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

    async fn submit_ok(token: &str, body: &Value) -> (StatusCode, HeaderMap, Value) {
        post_app(Some(token), &body.to_string(), None).await
    }

    #[tokio::test]
    async fn submit_app_mints_a_uuid_app_id_and_persists_the_manifest() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let body = manifest("submit_ok_app");
        let (status, _, resp) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::CREATED);
        let app_id = resp["appId"].as_str().expect("appId string");
        assert!(is_uuid(app_id), "appId {app_id} is a UUID");
        // The manifest is persisted under the minted id.
        assert_eq!(store::get(app_id), Some(body));
    }

    #[tokio::test]
    async fn resubmitting_the_same_app_is_already_exists() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let body = manifest("duplicate_app");
        let (status, _, first) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, _, second) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(second["code"], "ALREADY_EXISTS");
        assert_eq!(second["status"], 409);
        // Sanity: the first call really did succeed with an id.
        assert!(first["appId"].is_string());
    }

    #[tokio::test]
    async fn a_different_version_of_the_same_app_gets_a_new_id() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let (s1, _, r1) = submit_ok(&token, &manifest("versioned_app")).await;
        let mut v2 = manifest("versioned_app");
        v2["version"] = json!("2.0.0");
        let (s2, _, r2) = submit_ok(&token, &v2).await;
        assert_eq!(s1, StatusCode::CREATED);
        assert_eq!(s2, StatusCode::CREATED);
        assert_ne!(r1["appId"], r2["appId"], "distinct version → distinct appId");
    }

    #[tokio::test]
    async fn missing_required_field_is_invalid_argument() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let mut body = manifest("missing_pkg_app");
        body.as_object_mut().unwrap().remove("packageType");
        let (status, _, resp) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bad_name_pattern_is_invalid_argument() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let mut body = manifest("placeholder");
        body["name"] = json!("1-bad-name"); // starts with a digit, has hyphens
        let (status, _, resp) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_package_type_is_invalid_argument() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let mut body = manifest("bad_pkg_app");
        body["packageType"] = json!("ZIP");
        let (status, _, resp) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_component_spec_is_invalid_argument() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let mut body = manifest("empty_components_app");
        body["componentSpec"] = json!([]);
        let (status, _, resp) = submit_ok(&token, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_invalid_argument() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let (status, _, resp) = post_app(Some(&token), "{not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn submit_app_without_the_write_scope_is_forbidden() {
        // A token carrying only the zones *read* scope must not submit apps.
        let token = mint_token(ZONES_SCOPE).await;
        let (status, _, resp) = submit_ok(&token, &manifest("forbidden_app")).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn submit_app_missing_token_is_unauthenticated() {
        let (status, _, resp) =
            post_app(None, &manifest("noauth_app").to_string(), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn submit_app_echoes_x_correlator_on_success() {
        let token = mint_token(APPS_WRITE_SCOPE).await;
        let (status, headers, _) =
            post_app(Some(&token), &manifest("correlated_app").to_string(), Some("corr-apps")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-apps")
        );
    }

    // --- getApp: integration through the real router ----------------------

    async fn get_app_req(
        token: Option<&str>,
        app_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/edge-application-management/vwip/apps/{app_id}"))
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

    /// Submit an app (write scope) and return its minted `appId`.
    async fn submit_and_get_id(name: &str) -> (Value, String) {
        let write = mint_token(APPS_WRITE_SCOPE).await;
        let body = manifest(name);
        let (status, _, resp) = submit_ok(&write, &body).await;
        assert_eq!(status, StatusCode::CREATED);
        (body, resp["appId"].as_str().unwrap().to_string())
    }

    #[tokio::test]
    async fn get_app_returns_the_manifest_with_the_app_id_merged_in() {
        let (body, app_id) = submit_and_get_id("read_back_app").await;
        let read = mint_token(APPS_READ_SCOPE).await;
        let (status, _, resp) = get_app_req(Some(&read), &app_id, None).await;
        assert_eq!(status, StatusCode::OK);
        // AppManifestInfo = the submitted manifest + the assigned appId.
        assert_eq!(resp["appId"], json!(app_id));
        assert_eq!(resp["name"], body["name"]);
        assert_eq!(resp["version"], body["version"]);
        assert_eq!(resp["packageType"], body["packageType"]);
        assert_eq!(resp["appProvider"], body["appProvider"]);
    }

    #[tokio::test]
    async fn get_app_unknown_id_is_not_found() {
        let read = mint_token(APPS_READ_SCOPE).await;
        // A well-formed UUID that was never submitted.
        let (status, _, resp) =
            get_app_req(Some(&read), "00000000-0000-5000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
        assert_eq!(resp["status"], 404);
    }

    #[tokio::test]
    async fn get_app_malformed_id_is_not_found() {
        // A non-UUID path segment folds into 404 (not 400), mirroring readAccess.
        let read = mint_token(APPS_READ_SCOPE).await;
        let (status, _, resp) = get_app_req(Some(&read), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_app_without_the_read_scope_is_forbidden() {
        // The write scope onboards apps but must not read them back.
        let (_, app_id) = submit_and_get_id("scope_gated_app").await;
        let write = mint_token(APPS_WRITE_SCOPE).await;
        let (status, _, resp) = get_app_req(Some(&write), &app_id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn get_app_missing_token_is_unauthenticated() {
        let (status, _, resp) =
            get_app_req(None, "00000000-0000-5000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn get_app_echoes_x_correlator_on_success_and_error() {
        let (_, app_id) = submit_and_get_id("correlated_read_app").await;
        let read = mint_token(APPS_READ_SCOPE).await;

        let (status, headers, _) = get_app_req(Some(&read), &app_id, Some("corr-get-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-ok")
        );

        let (status, headers, _) = get_app_req(
            Some(&read),
            "00000000-0000-5000-8000-000000000000",
            Some("corr-get-404"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-404")
        );
    }

    // --- getApps: integration through the real router ---------------------

    async fn get_apps_req(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/edge-application-management/vwip/apps")
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
    async fn get_apps_lists_a_submitted_app_as_app_manifest_info() {
        // Onboard an app, then list: the store snapshot must contain it, rendered
        // as an AppManifestInfo (the manifest + its minted appId).
        let (body, app_id) = submit_and_get_id("listed_app").await;
        let read = mint_token(APPS_READ_SCOPE).await;
        let (status, _, resp) = get_apps_req(Some(&read), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = resp.as_array().expect("array response");
        // Every item is an AppManifestInfo — carries a UUID `appId`.
        assert!(arr.iter().all(|a| a["appId"].as_str().is_some_and(is_uuid)));
        // The app we just submitted appears, with its manifest fields merged.
        let ours = arr
            .iter()
            .find(|a| a["appId"] == json!(app_id))
            .expect("submitted app is listed");
        assert_eq!(ours["name"], body["name"]);
        assert_eq!(ours["version"], body["version"]);
        assert_eq!(ours["packageType"], body["packageType"]);
    }

    #[tokio::test]
    async fn get_apps_without_the_read_scope_is_forbidden() {
        // The write scope onboards apps but must not list them.
        let write = mint_token(APPS_WRITE_SCOPE).await;
        let (status, _, resp) = get_apps_req(Some(&write), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn get_apps_missing_token_is_unauthenticated() {
        let (status, _, resp) = get_apps_req(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn get_apps_echoes_x_correlator() {
        let read = mint_token(APPS_READ_SCOPE).await;
        let (status, headers, _) = get_apps_req(Some(&read), Some("corr-get-apps")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-apps")
        );
    }

    // --- deleteApp: integration through the real router -------------------

    async fn delete_app_req(
        token: Option<&str>,
        app_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("/edge-application-management/vwip/apps/{app_id}"))
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
        // A 204 carries no body; an error carries a CamaraError JSON.
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn delete_app_removes_a_submitted_app() {
        // Onboard, delete (204), then the app is gone — a later read 404s.
        let (_, app_id) = submit_and_get_id("deletable_app").await;
        let del = mint_token(APPS_DELETE_SCOPE).await;
        let (status, _, body) = delete_app_req(Some(&del), &app_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null); // 204 has an empty body

        let read = mint_token(APPS_READ_SCOPE).await;
        let (status, _, resp) = get_app_req(Some(&read), &app_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_app_is_single_use() {
        // The first delete evicts (204); a second delete of the same id 404s.
        let (_, app_id) = submit_and_get_id("single_use_delete_app").await;
        let del = mint_token(APPS_DELETE_SCOPE).await;

        let (status, _, _) = delete_app_req(Some(&del), &app_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _, resp) = delete_app_req(Some(&del), &app_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
        assert_eq!(resp["status"], 404);
    }

    #[tokio::test]
    async fn delete_app_unknown_id_is_not_found() {
        // A well-formed UUID that was never submitted.
        let del = mint_token(APPS_DELETE_SCOPE).await;
        let (status, _, resp) =
            delete_app_req(Some(&del), "00000000-0000-5000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
        assert_eq!(resp["status"], 404);
    }

    #[tokio::test]
    async fn delete_app_malformed_id_is_not_found() {
        // A non-UUID path segment folds into 404 (not 400), mirroring get_app.
        let del = mint_token(APPS_DELETE_SCOPE).await;
        let (status, _, resp) = delete_app_req(Some(&del), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_app_without_the_delete_scope_is_forbidden() {
        // The read scope reads apps but must not delete them; the app survives.
        let (_, app_id) = submit_and_get_id("scope_gated_delete_app").await;
        let read = mint_token(APPS_READ_SCOPE).await;
        let (status, _, resp) = delete_app_req(Some(&read), &app_id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");

        // The forbidden call did not evict — the app is still readable.
        let read2 = mint_token(APPS_READ_SCOPE).await;
        let (status, _, _) = get_app_req(Some(&read2), &app_id, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_app_missing_token_is_unauthenticated() {
        let (status, _, resp) =
            delete_app_req(None, "00000000-0000-5000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn delete_app_echoes_x_correlator_on_success_and_error() {
        let (_, app_id) = submit_and_get_id("correlated_delete_app").await;
        let del = mint_token(APPS_DELETE_SCOPE).await;

        // Success (204) echoes x-correlator.
        let (status, headers, _) = delete_app_req(Some(&del), &app_id, Some("corr-del-ok")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-ok")
        );

        // Error (404, id now gone) echoes x-correlator too.
        let (status, headers, _) = delete_app_req(Some(&del), &app_id, Some("corr-del-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-404")
        );
    }

    // --- createAppInstance: pure units -------------------------------------

    #[test]
    fn instance_id_is_stable_uuid_shaped_and_keyed_on_app_and_zone() {
        let app = "5e3a8c2f-1b4d-5a6e-8f90-2c1d3e4f5a6b";
        let zone_a = zone_id("camarasim-edge-eu-west-1");
        let zone_b = zone_id("camarasim-edge-us-east-1");
        let id = instance_id(app, &zone_a);
        assert!(is_uuid(&id), "instance id {id} is a UUID");
        assert_eq!(id, instance_id(app, &zone_a), "stable per (app, zone)");
        // A different zone → a different instance; a different app → different too.
        assert_ne!(id, instance_id(app, &zone_b));
        assert_ne!(
            id,
            instance_id("00000000-0000-4000-8000-000000000000", &zone_a)
        );
    }

    #[test]
    fn instance_status_follows_the_target_zone_status() {
        assert_eq!(instance_status("active"), "ready");
        assert_eq!(instance_status("inactive"), "failed");
        assert_eq!(instance_status("unknown"), "instantiating");
    }

    #[test]
    fn is_uuid_accepts_the_camara_pattern_and_rejects_junk() {
        assert!(is_uuid("5e3a8c2f-1b4d-5a6e-8f90-2c1d3e4f5a6b"));
        assert!(is_uuid("00000000-0000-4000-8000-000000000000"));
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("5E3A8C2F-1B4D-5A6E-8F90-2C1D3E4F5A6B")); // uppercase rejected
        assert!(!is_uuid("00000000-0000-6000-8000-000000000000")); // version 6 rejected
        assert!(!is_uuid("00000000-0000-4000-c000-000000000000")); // variant c rejected
    }

    // --- createAppInstance: integration through the real router ------------

    async fn post_instance(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/edge-application-management/vwip/app-instances")
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

    /// An active catalog zone (→ `ready`).
    fn active_zone() -> String {
        zone_id("camarasim-edge-eu-west-1")
    }

    #[tokio::test]
    async fn create_app_instance_mints_a_uuid_echoes_provider_and_persists() {
        let (_, app_id) = submit_and_get_id("instance_ok_app").await;
        let zone = active_zone();
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let body = json!({ "name": "prod", "appId": app_id, "edgeCloudZoneId": zone });
        let (status, headers, resp) = post_instance(Some(&token), &body.to_string(), None).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        let instance_id = resp["appInstanceId"].as_str().expect("appInstanceId string");
        assert!(is_uuid(instance_id), "appInstanceId {instance_id} is a UUID");
        assert_eq!(resp["name"], "prod");
        assert_eq!(resp["appId"], json!(app_id));
        assert_eq!(resp["edgeCloudZoneId"], json!(zone));
        // appProvider is echoed from the onboarded app's manifest.
        assert_eq!(resp["appProvider"], "CamaraSim Test");
        // active zone → ready.
        assert_eq!(resp["status"], "ready");
        // Location header points at the instance resource.
        assert_eq!(
            headers.get("location").and_then(|v| v.to_str().ok()),
            Some(
                format!("/edge-application-management/vwip/app-instances/{instance_id}")
                    .as_str()
            )
        );
        // The rendered AppInstanceInfo is persisted under the minted id.
        assert_eq!(instance_store::get(instance_id), Some(resp.clone()));
    }

    #[tokio::test]
    async fn kubernetes_cluster_ref_is_echoed_when_supplied() {
        let (_, app_id) = submit_and_get_id("instance_k8s_app").await;
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let cluster = "00000000-0000-4000-8000-000000000000";
        let body = json!({
            "name": "withcluster", "appId": app_id, "edgeCloudZoneId": active_zone(),
            "kubernetesClusterRef": cluster,
        });
        let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(resp["kubernetesClusterRef"], cluster);
    }

    #[tokio::test]
    async fn the_target_zone_status_drives_the_instance_status() {
        let (_, app_id) = submit_and_get_id("instance_zone_status_app").await;
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        // inactive zone → failed.
        let inactive = zone_id("camarasim-edge-us-west-2");
        let body = json!({ "name": "oninactive", "appId": app_id, "edgeCloudZoneId": inactive });
        let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(resp["status"], "failed");
        // unknown-status zone → instantiating.
        let unknown = zone_id("camarasim-edge-ap-south-1");
        let body = json!({ "name": "onunknown", "appId": app_id, "edgeCloudZoneId": unknown });
        let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(resp["status"], "instantiating");
    }

    #[tokio::test]
    async fn same_app_same_zone_is_already_exists_but_a_new_zone_is_a_new_instance() {
        let (_, app_id) = submit_and_get_id("instance_dup_app").await;
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let zone_a = active_zone();
        let body = json!({ "name": "dup", "appId": app_id, "edgeCloudZoneId": zone_a });

        let (s1, _, r1) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(s1, StatusCode::ACCEPTED);
        // Same (app, zone) again → 409 ALREADY_EXISTS.
        let (s2, _, r2) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(s2, StatusCode::CONFLICT);
        assert_eq!(r2["code"], "ALREADY_EXISTS");
        assert_eq!(r2["status"], 409);

        // Same app on a different zone → a fresh 202 with a distinct id.
        let zone_b = zone_id("camarasim-edge-us-east-1");
        let body_b = json!({ "name": "dup", "appId": app_id, "edgeCloudZoneId": zone_b });
        let (s3, _, r3) = post_instance(Some(&token), &body_b.to_string(), None).await;
        assert_eq!(s3, StatusCode::ACCEPTED);
        assert_ne!(r1["appInstanceId"], r3["appInstanceId"]);
    }

    #[tokio::test]
    async fn unknown_app_id_is_not_found() {
        // A well-formed but never-onboarded appId → 404.
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let body = json!({
            "name": "orphan", "appId": "00000000-0000-4000-8000-000000000000",
            "edgeCloudZoneId": active_zone(),
        });
        let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn unknown_zone_id_is_not_found() {
        // A real app but a valid UUID naming no catalog zone → 404.
        let (_, app_id) = submit_and_get_id("instance_bad_zone_app").await;
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let body = json!({
            "name": "nozone", "appId": app_id,
            "edgeCloudZoneId": "11111111-1111-4111-8111-111111111111",
        });
        let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(resp["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn missing_or_malformed_fields_are_invalid_argument() {
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let zone = active_zone();
        let valid_app = "00000000-0000-4000-8000-000000000000";
        let cases = vec![
            // missing name
            json!({ "appId": valid_app, "edgeCloudZoneId": zone }),
            // bad name pattern
            json!({ "name": "1bad", "appId": valid_app, "edgeCloudZoneId": zone }),
            // missing appId
            json!({ "name": "ok", "edgeCloudZoneId": zone }),
            // non-UUID appId
            json!({ "name": "ok", "appId": "not-a-uuid", "edgeCloudZoneId": zone }),
            // missing edgeCloudZoneId
            json!({ "name": "ok", "appId": valid_app }),
            // non-UUID zone
            json!({ "name": "ok", "appId": valid_app, "edgeCloudZoneId": "nope" }),
            // non-UUID kubernetesClusterRef
            json!({ "name": "ok", "appId": valid_app, "edgeCloudZoneId": zone,
                    "kubernetesClusterRef": "nope" }),
        ];
        for body in cases {
            let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "body {body} → 400");
            assert_eq!(resp["code"], "INVALID_ARGUMENT", "body {body}");
        }
    }

    #[tokio::test]
    async fn create_app_instance_malformed_json_body_is_invalid_argument() {
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let (status, _, resp) = post_instance(Some(&token), "{ not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_app_instance_without_the_scope_is_forbidden() {
        let (_, app_id) = submit_and_get_id("instance_scope_gated_app").await;
        // A token with the apps read scope, not the instances write scope.
        let token = mint_token(APPS_READ_SCOPE).await;
        let body = json!({ "name": "nope", "appId": app_id, "edgeCloudZoneId": active_zone() });
        let (status, _, resp) = post_instance(Some(&token), &body.to_string(), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn create_app_instance_missing_token_is_unauthenticated() {
        let body = json!({
            "name": "nope", "appId": "00000000-0000-4000-8000-000000000000",
            "edgeCloudZoneId": active_zone(),
        });
        let (status, _, resp) = post_instance(None, &body.to_string(), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn create_app_instance_echoes_x_correlator_on_success_and_error() {
        let (_, app_id) = submit_and_get_id("instance_correlated_app").await;
        let token = mint_token(INSTANCES_WRITE_SCOPE).await;
        let body = json!({ "name": "corr", "appId": app_id, "edgeCloudZoneId": active_zone() });

        // Success (202) echoes x-correlator.
        let (status, headers, _) =
            post_instance(Some(&token), &body.to_string(), Some("corr-inst-ok")).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-inst-ok")
        );

        // Error (409, same app+zone) echoes x-correlator too.
        let (status, headers, _) =
            post_instance(Some(&token), &body.to_string(), Some("corr-inst-409")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-inst-409")
        );
    }
}
