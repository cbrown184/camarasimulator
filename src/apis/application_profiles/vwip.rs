//! Application Profiles **vwip** (CAMARA application-profiles, work-in-progress —
//! no released version yet), mounted at `/application-profiles/vwip`.
//!
//! This slice implements the create + read-by-id set:
//! - `POST /application-profiles` (operationId `createApplicationProfile`, scope
//!   `application-profiles:create`) — registers an application's quality
//!   requirements (network-quality and/or compute-resource thresholds), mints an
//!   opaque `applicationProfileId` ([`super::store`]), remembers the rendered
//!   `ApplicationProfile`, and returns `201`.
//! - `GET /application-profiles/{applicationProfileId}` (operationId
//!   `readApplicationProfile`, scope `application-profiles:read`) — reads a stored
//!   profile back (`200`) or `404 NOT_FOUND` for an unknown id.
//! - `DELETE /application-profiles/{applicationProfileId}` (operationId
//!   `deleteApplicationProfile`, scope `application-profiles:delete`) — evicts a
//!   stored profile (`204 No Content`, single-use) or `404 NOT_FOUND` for an
//!   unknown/already-deleted id.
//!
//! `PATCH` (`updateApplicationProfile`) is the remaining follow-up sub-item
//! (see PROGRESS.md).
//!
//! ## No identifier / no `device`
//!
//! Unlike the device-keyed APIs, an application profile is not tied to a device
//! or a subscriber line — it is a free-standing catalog resource. There is no
//! `device`, no phone number, and no token subject to key off, so there is **no**
//! reserved-identifier control plane and no two-legged / three-legged rule. The
//! opaque `applicationProfileId` is minted by the server; for read/delete the
//! store state is the only control plane (mirroring QoS Provisioning's
//! `getQosAssignmentById`).
//!
//! ## Functional cases — the request body is the control plane (docs/DESIGN.md §7)
//!
//! `createApplicationProfile` is driven entirely by the request body:
//! - **`anyOf`** — the body must carry at least one of `networkQualityThresholds`
//!   or `computeResources` → else `400 INVALID_ARGUMENT`.
//! - **`minProperties: 1`** — a supplied `networkQualityThresholds` /
//!   `computeResources` object must carry at least one property → else
//!   `400 INVALID_ARGUMENT`.
//! - **Range constraints** drive `400 OUT_OF_RANGE`: a `Duration` (`packetDelayBudget`,
//!   `jitter`) `value` must be `>= 1`; a `Rate` (`targetMinDownstreamRate`,
//!   `targetMinUpstreamRate`) `value` must be `0..=1024`; `packetLossErrorRate`
//!   must be `1..=10`; a `Compute` (`targetMin*Memory` / `*Storage`) `value` must
//!   be `0..=1024`.
//! - **Enum / type** violations (unknown `unit`, unknown `gpuVendorType`, wrong
//!   JSON type, unknown field) are rejected at parse time → `400 INVALID_ARGUMENT`.
//!
//! The `targetMinCPU` / `targetMinGPU` / `gpuVendorType` / `gpuModelName` compute
//! fields are accepted with schema-level (type/enum) validation only — the CAMARA
//! spec places no numeric range on them — a documented trim.
//!
//! `readApplicationProfile` is driven by the `applicationProfileId` path param: a
//! value violating the `uuid` shape → `400 INVALID_ARGUMENT`; a well-formed but
//! unknown id → `404 NOT_FOUND`; a stored id → `200`.
//!
//! `deleteApplicationProfile` is keyed the same way against the store: a value
//! violating the `uuid` shape → `400 INVALID_ARGUMENT`; a stored id → `204 No
//! Content` (single-use eviction); a well-formed but unknown or already-deleted
//! id → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;

/// Scope required to create a profile (CAMARA application-profiles).
const CREATE_SCOPE: &str = "application-profiles:create";
/// Scope required to read a profile (CAMARA application-profiles).
const READ_SCOPE: &str = "application-profiles:read";
/// Scope required to delete a profile (CAMARA application-profiles).
const DELETE_SCOPE: &str = "application-profiles:delete";

/// Routes for Application Profiles vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/application-profiles/vwip/application-profiles",
            post(create_application_profile),
        )
        .route(
            "/application-profiles/vwip/application-profiles/:application_profile_id",
            axum::routing::get(read_application_profile).delete(delete_application_profile),
        )
}

/// `ApplicationProfileRequest` (CAMARA application-profiles). `anyOf`: at least
/// one of the two threshold objects must be present (checked in the handler, as
/// serde cannot express `anyOf`). Deriving `Serialize` lets the handler echo the
/// validated request back as the created `ApplicationProfile`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ApplicationProfileRequest {
    #[serde(
        rename = "networkQualityThresholds",
        skip_serializing_if = "Option::is_none"
    )]
    network_quality_thresholds: Option<NetworkQualityThresholds>,
    #[serde(rename = "computeResources", skip_serializing_if = "Option::is_none")]
    compute_resources: Option<ComputeResourcesThresholds>,
}

/// `NetworkQualityThresholds` (`minProperties: 1`, checked in the handler).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NetworkQualityThresholds {
    #[serde(
        rename = "packetDelayBudget",
        skip_serializing_if = "Option::is_none"
    )]
    packet_delay_budget: Option<Duration>,
    #[serde(
        rename = "targetMinDownstreamRate",
        skip_serializing_if = "Option::is_none"
    )]
    target_min_downstream_rate: Option<Rate>,
    #[serde(
        rename = "targetMinUpstreamRate",
        skip_serializing_if = "Option::is_none"
    )]
    target_min_upstream_rate: Option<Rate>,
    #[serde(
        rename = "packetLossErrorRate",
        skip_serializing_if = "Option::is_none"
    )]
    packet_loss_error_rate: Option<i64>,
    #[serde(rename = "jitter", skip_serializing_if = "Option::is_none")]
    jitter: Option<Duration>,
}

impl NetworkQualityThresholds {
    /// Whether at least one threshold is present (`minProperties: 1`).
    fn is_non_empty(&self) -> bool {
        self.packet_delay_budget.is_some()
            || self.target_min_downstream_rate.is_some()
            || self.target_min_upstream_rate.is_some()
            || self.packet_loss_error_rate.is_some()
            || self.jitter.is_some()
    }
}

/// A `Duration` value (`value` >= 1, `unit` from [`TimeUnitEnum`]).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Duration {
    value: i64,
    unit: TimeUnitEnum,
}

/// A `Rate` value (`value` 0..=1024, `unit` from [`RateUnitEnum`]).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Rate {
    value: i64,
    unit: RateUnitEnum,
}

/// A `Compute` value (`value` 0..=1024, `unit` from [`ComputeUnitEnum`]).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Compute {
    value: i64,
    unit: ComputeUnitEnum,
}

/// `ComputeResourcesThresholds` (`minProperties: 1`, checked in the handler).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ComputeResourcesThresholds {
    #[serde(rename = "targetMinCPU", skip_serializing_if = "Option::is_none")]
    target_min_cpu: Option<f64>,
    #[serde(rename = "targetMinGPU", skip_serializing_if = "Option::is_none")]
    target_min_gpu: Option<i64>,
    #[serde(rename = "gpuVendorType", skip_serializing_if = "Option::is_none")]
    gpu_vendor_type: Option<GpuVendorType>,
    #[serde(rename = "gpuModelName", skip_serializing_if = "Option::is_none")]
    gpu_model_name: Option<String>,
    #[serde(rename = "targetMinMemory", skip_serializing_if = "Option::is_none")]
    target_min_memory: Option<Compute>,
    #[serde(
        rename = "targetMinGPUMemory",
        skip_serializing_if = "Option::is_none"
    )]
    target_min_gpu_memory: Option<Compute>,
    #[serde(
        rename = "targetMinEphemeralStorage",
        skip_serializing_if = "Option::is_none"
    )]
    target_min_ephemeral_storage: Option<Compute>,
    #[serde(
        rename = "targetMinPersistentStorage",
        skip_serializing_if = "Option::is_none"
    )]
    target_min_persistent_storage: Option<Compute>,
}

impl ComputeResourcesThresholds {
    /// Whether at least one threshold is present (`minProperties: 1`).
    fn is_non_empty(&self) -> bool {
        self.target_min_cpu.is_some()
            || self.target_min_gpu.is_some()
            || self.gpu_vendor_type.is_some()
            || self.gpu_model_name.is_some()
            || self.target_min_memory.is_some()
            || self.target_min_gpu_memory.is_some()
            || self.target_min_ephemeral_storage.is_some()
            || self.target_min_persistent_storage.is_some()
    }
}

/// `TimeUnitEnum` (CAMARA Commonalities). Unknown values are rejected at parse.
#[derive(Debug, Deserialize, Serialize)]
enum TimeUnitEnum {
    Days,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

/// `RateUnitEnum` (CAMARA Commonalities). Unknown values are rejected at parse.
#[derive(Debug, Deserialize, Serialize)]
enum RateUnitEnum {
    Bps,
    Kbps,
    Mbps,
    Gbps,
    Tbps,
}

/// `ComputeUnitEnum` (CAMARA application-profiles). Unknown values rejected at parse.
#[derive(Debug, Deserialize, Serialize)]
enum ComputeUnitEnum {
    Kb,
    Mb,
    Gb,
    Tb,
}

/// `gpuVendorType` enum. Unknown values are rejected at parse.
#[derive(Debug, Deserialize, Serialize)]
enum GpuVendorType {
    Nvidia,
    AMD,
}

/// `POST /application-profiles/vwip/application-profiles`.
async fn create_application_profile(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory; parse strictly. Unknown fields, wrong types, and unknown
    // enum variants (`unit` / `gpuVendorType`) all fail here → INVALID_ARGUMENT.
    let req: ApplicationProfileRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid ApplicationProfileRequest.",
                &correlator,
            )
        }
    };

    // `anyOf`: at least one of the two threshold objects must be present.
    if req.network_quality_thresholds.is_none() && req.compute_resources.is_none() {
        return invalid_argument(
            "At least one of `networkQualityThresholds` or `computeResources` is required.",
            &correlator,
        );
    }

    // Validate the supplied threshold objects (minProperties + numeric ranges).
    if let Some(nqt) = &req.network_quality_thresholds {
        if let Err(resp) = validate_network_quality(nqt, &correlator) {
            return resp;
        }
    }
    if let Some(cr) = &req.compute_resources {
        if let Err(resp) = validate_compute_resources(cr, &correlator) {
            return resp;
        }
    }

    // Mint the id, render the ApplicationProfile (the validated request echoed
    // back with the id), remember it, and return 201.
    let profile_id = store::new_profile_id();
    let mut profile = serde_json::to_value(&req).unwrap_or_else(|_| json!({}));
    profile["applicationProfileId"] = json!(profile_id);
    store::insert(profile_id, profile.clone());

    with_correlator(
        (StatusCode::CREATED, Json(profile)).into_response(),
        &correlator,
    )
}

/// `GET /application-profiles/vwip/application-profiles/{applicationProfileId}`.
async fn read_application_profile(
    claims: Claims,
    headers: HeaderMap,
    Path(application_profile_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path parameter is `format: uuid`; a malformed value is a 400 (mirrors
    // QoS Profiles' `getQosProfile` bad-pattern → 400 vs unknown → 404).
    if !is_uuid_shaped(&application_profile_id) {
        return invalid_argument(
            "`applicationProfileId` must be a UUID.",
            &correlator,
        );
    }

    match store::get(&application_profile_id) {
        Some(profile) => {
            with_correlator((StatusCode::OK, Json(profile)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No application profile found for the provided applicationProfileId.")
                .into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /application-profiles/vwip/application-profiles/{applicationProfileId}`.
async fn delete_application_profile(
    claims: Claims,
    headers: HeaderMap,
    Path(application_profile_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path parameter is `format: uuid`; a malformed value is a 400 (mirrors
    // `readApplicationProfile`).
    if !is_uuid_shaped(&application_profile_id) {
        return invalid_argument("`applicationProfileId` must be a UUID.", &correlator);
    }

    // The store state is the only control plane: present → 204 (single-use
    // eviction), absent → 404.
    if store::remove(&application_profile_id) {
        with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
    } else {
        with_correlator(
            CamaraError::not_found(
                "No application profile found for the provided applicationProfileId.",
            )
            .into_response(),
            &correlator,
        )
    }
}

/// Validate `NetworkQualityThresholds`: `minProperties: 1`, then per-field ranges.
fn validate_network_quality(
    nqt: &NetworkQualityThresholds,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    if !nqt.is_non_empty() {
        return Err(invalid_argument(
            "`networkQualityThresholds` must contain at least one threshold.",
            correlator,
        ));
    }
    // A Duration `value` is `minimum: 1`.
    for (name, dur) in [
        ("packetDelayBudget", &nqt.packet_delay_budget),
        ("jitter", &nqt.jitter),
    ] {
        if let Some(dur) = dur {
            if dur.value < 1 {
                return Err(out_of_range(
                    &format!("`{name}.value` must be at least 1."),
                    correlator,
                ));
            }
        }
    }
    // A Rate `value` is `0..=1024`.
    for (name, rate) in [
        ("targetMinDownstreamRate", &nqt.target_min_downstream_rate),
        ("targetMinUpstreamRate", &nqt.target_min_upstream_rate),
    ] {
        if let Some(rate) = rate {
            if !(0..=1024).contains(&rate.value) {
                return Err(out_of_range(
                    &format!("`{name}.value` must be in 0..=1024."),
                    correlator,
                ));
            }
        }
    }
    // `packetLossErrorRate` is `1..=10`.
    if let Some(plr) = nqt.packet_loss_error_rate {
        if !(1..=10).contains(&plr) {
            return Err(out_of_range(
                "`packetLossErrorRate` must be in 1..=10.",
                correlator,
            ));
        }
    }
    Ok(())
}

/// Validate `ComputeResourcesThresholds`: `minProperties: 1`, then `Compute` ranges.
fn validate_compute_resources(
    cr: &ComputeResourcesThresholds,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    if !cr.is_non_empty() {
        return Err(invalid_argument(
            "`computeResources` must contain at least one threshold.",
            correlator,
        ));
    }
    // Every `Compute` `value` is `0..=1024`.
    for (name, comp) in [
        ("targetMinMemory", &cr.target_min_memory),
        ("targetMinGPUMemory", &cr.target_min_gpu_memory),
        ("targetMinEphemeralStorage", &cr.target_min_ephemeral_storage),
        ("targetMinPersistentStorage", &cr.target_min_persistent_storage),
    ] {
        if let Some(comp) = comp {
            if !(0..=1024).contains(&comp.value) {
                return Err(out_of_range(
                    &format!("`{name}.value` must be in 0..=1024."),
                    correlator,
                ));
            }
        }
    }
    Ok(())
}

/// Whether `s` is a lowercase `8-4-4-4-12` hex UUID (matching the ids
/// [`store::new_profile_id`] mints and CAMARA's `format: uuid`).
fn is_uuid_shaped(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "appprofiles.local:8080";
    const PROFILES: &str = "/application-profiles/vwip/application-profiles";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_shape_check() {
        assert!(is_uuid_shaped("00000000-0000-4000-8000-000000000000"));
        assert!(is_uuid_shaped(&store::new_profile_id()));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("0000-0000-4000-8000-000000000000")); // wrong lengths
        assert!(!is_uuid_shaped("gggggggg-0000-4000-8000-000000000000")); // non-hex
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=appprofiles-client&scope={scope}");
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

    async fn request(
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
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

    async fn post_profile(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        request("POST", PROFILES, token, Some(body), correlator).await
    }

    async fn get_profile(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{PROFILES}/{id}");
        request("GET", &path, token, None, correlator).await
    }

    async fn delete_profile(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let path = format!("{PROFILES}/{id}");
        request("DELETE", &path, token, None, correlator).await
    }

    /// Create a profile and return its minted `applicationProfileId`.
    async fn create_and_get_id() -> String {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({ "computeResources": { "targetMinGPU": 1 } }).to_string();
        let (status, _, created) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        created["applicationProfileId"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn create_network_quality_profile_echoes_and_mints_uuid() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "networkQualityThresholds": {
                "packetDelayBudget": { "value": 10, "unit": "Milliseconds" },
                "targetMinDownstreamRate": { "value": 100, "unit": "Mbps" },
                "packetLossErrorRate": 3,
            }
        })
        .to_string();
        let (status, headers, profile) = post_profile(Some(&token), &body, Some("corr-1")).await;
        assert_eq!(status, StatusCode::CREATED);
        // UUID-shaped id, and the thresholds are echoed back verbatim.
        let id = profile["applicationProfileId"].as_str().unwrap();
        assert_eq!(id.split('-').count(), 5);
        assert_eq!(
            profile["networkQualityThresholds"]["packetDelayBudget"]["unit"],
            "Milliseconds"
        );
        assert_eq!(profile["networkQualityThresholds"]["packetLossErrorRate"], 3);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-1");
    }

    #[tokio::test]
    async fn create_compute_resources_profile_is_accepted() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "computeResources": {
                "targetMinCPU": 2.5,
                "gpuVendorType": "Nvidia",
                "targetMinMemory": { "value": 512, "unit": "Mb" },
            }
        })
        .to_string();
        let (status, _, profile) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(profile["computeResources"]["gpuVendorType"], "Nvidia");
        assert_eq!(profile["computeResources"]["targetMinMemory"]["value"], 512);
    }

    #[tokio::test]
    async fn create_then_read_reads_the_same_profile() {
        let create = mint_token(CREATE_SCOPE).await;
        let body = json!({ "computeResources": { "targetMinGPU": 1 } }).to_string();
        let (status, _, created) = post_profile(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["applicationProfileId"].as_str().unwrap();

        let read = mint_token(READ_SCOPE).await;
        let (status, _, got) = get_profile(Some(&read), id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(got, created);
    }

    #[tokio::test]
    async fn read_unknown_profile_is_404() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            get_profile(Some(&read), "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_malformed_id_is_400() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = get_profile(Some(&read), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_body_fails_anyof_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, err) = post_profile(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_network_quality_object_violates_min_properties_400() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({ "networkQualityThresholds": {} }).to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn rate_value_out_of_range_is_400_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "networkQualityThresholds": {
                "targetMinDownstreamRate": { "value": 2048, "unit": "Mbps" }
            }
        })
        .to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn packet_loss_error_rate_out_of_range_is_400_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({ "networkQualityThresholds": { "packetLossErrorRate": 42 } }).to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn duration_value_below_one_is_400_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "networkQualityThresholds": { "packetDelayBudget": { "value": 0, "unit": "Seconds" } }
        })
        .to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn compute_value_out_of_range_is_400_out_of_range() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "computeResources": { "targetMinMemory": { "value": 5000, "unit": "Gb" } }
        })
        .to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn unknown_unit_enum_is_400_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "networkQualityThresholds": { "jitter": { "value": 5, "unit": "Fortnights" } }
        })
        .to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_400_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = json!({
            "networkQualityThresholds": { "packetLossErrorRate": 2 },
            "surprise": true
        })
        .to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_requires_the_create_scope() {
        // A token with only the read scope may not create → 403.
        let token = mint_token(READ_SCOPE).await;
        let body = json!({ "computeResources": { "targetMinGPU": 1 } }).to_string();
        let (status, _, err) = post_profile(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn read_requires_the_read_scope() {
        // A create-only token may not read → 403.
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, err) =
            get_profile(Some(&token), "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_401() {
        let body = json!({ "computeResources": { "targetMinGPU": 1 } }).to_string();
        let (status, _, _) = post_profile(None, &body, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- deleteApplicationProfile ------------------------------------------

    #[tokio::test]
    async fn delete_evicts_the_profile_204_then_read_is_404() {
        let id = create_and_get_id().await;
        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, _) = delete_profile(Some(&del), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-del");

        // A read after the delete now 404s.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = get_profile(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_404() {
        let id = create_and_get_id().await;
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_profile(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _, body) = delete_profile(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_profile_is_404() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) =
            delete_profile(Some(&del), "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_malformed_id_is_400() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) = delete_profile(Some(&del), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn delete_requires_the_delete_scope() {
        // A read-only token may not delete → 403 (and the profile survives).
        let id = create_and_get_id().await;
        let read = mint_token(READ_SCOPE).await;
        let (status, _, err) = delete_profile(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
        // Still readable — the forbidden delete did not evict it.
        let (status, _, _) = get_profile(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_missing_token_is_401() {
        let (status, _, _) =
            delete_profile(None, "00000000-0000-4000-8000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
