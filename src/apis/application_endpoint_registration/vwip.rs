//! Application Endpoint Registration **vwip** (CAMARA
//! ApplicationEndpointRegistration, work-in-progress — no released version yet),
//! mounted at `/application-endpoint-registration/vwip`.
//!
//! This module implements:
//! - `POST /application-endpoint-lists` (operationId `registerApplicationEndpoints`,
//!   scope `application-endpoint-registration:application-endpoints:write`) —
//!   registers a set of application endpoints, mints an opaque
//!   `applicationEndpointListId` ([`super::store`]), remembers the rendered
//!   registration, and returns `200` with that id.
//! - `GET /application-endpoint-lists/{applicationEndpointListId}` (operationId
//!   `getApplicationEndpointsById`, scope
//!   `application-endpoint-registration:application-endpoints:read`) — the
//!   **read-back** leg: returns the stored registration (`200`) or `404
//!   NOT_FOUND` for an unknown id. The id is server-minted (`format: uuid`), so a
//!   malformed path value is a `400 INVALID_ARGUMENT`; the opaque id is the only
//!   control plane (it was never caller-chosen, so there is no reserved-identifier
//!   suffix on it).
//! - `GET /application-endpoint-lists` (operationId
//!   `getAllRegisteredApplicationEndpoints`, scope
//!   `application-endpoint-registration:application-endpoints:read`) — the
//!   **list** leg: returns every registered list as an array of
//!   `ApplicationEndpointList` (`200`, an empty array when none — a list never
//!   `404`s). The store state is the only control plane; registrations are not
//!   scoped per client (a documented simplification).
//!
//! Both GETs return the canonical CAMARA `ApplicationEndpointList` shape
//! (`applicationEndpointListId` + a nested `applicationEndpointsInfo`), which is
//! also how a registration is stored, so the read-back and list legs agree.
//!
//! The update (`PUT`) and deregister (`DELETE`) legs by
//! `{applicationEndpointListId}` land in later passes.
//!
//! ## No device identifier — the request body is the control plane (DESIGN §7)
//!
//! A registration is a free-standing catalog resource (a provider publishing its
//! endpoints); it is not tied to a device or a subscriber line, so there is no
//! `device`, no phone number, no token subject, and thus **no** two-legged /
//! three-legged rule. The `applicationEndpointListId` is server-minted, so on
//! create there is no reserved-identifier plane on *it*; the control planes are:
//!
//! - **Request-body validation** → `400`:
//!   - `applicationEndpoints` must be a non-empty array of at most 20 endpoints
//!     (`INVALID_ARGUMENT`).
//!   - Each endpoint must carry `port` (`1..=65535`, else `OUT_OF_RANGE`) and at
//!     least one of `domainName` / `ipv4Address` / `ipv6Address` (the CAMARA
//!     `anyOf`, else `INVALID_ARGUMENT`); a malformed IPv4/IPv6 address or a
//!     malformed `edgeCloudZone.edgeCloudZoneId` (which is `format: uuid`) →
//!     `INVALID_ARGUMENT`.
//!   - `applicationProviderName` (≤256 chars, no CR/LF) and, when present,
//!     `applicationDescription` (≤2048 chars, no CR/LF) → `INVALID_ARGUMENT`.
//!   - `applicationProfileId` must be a UUID → else `INVALID_ARGUMENT`.
//!   - Unknown fields / wrong JSON types are rejected at parse → `INVALID_ARGUMENT`.
//! - **`applicationProfileId` reserved-error suffix** → canonical CAMARA error.
//!   A `UUID` carries digits, so the shared reserved-suffix convention
//!   ([`crate::scenarios`]) applies unchanged to `applicationProfileId`: its
//!   trailing three digits `…404` → `404 NOT_FOUND`, `…422` → `422
//!   SERVICE_NOT_APPLICABLE`, `…429` → `429`, etc. (mirrors Network Health
//!   Assessment keying off `networkId`). CamaraSim exposes the **full** shared
//!   reserved set so every case is reachable; canonical
//!   ApplicationEndpointRegistration declares a subset (400/401/403/404/422/429).
//! - **The nil UUID sentinel** → `422 UNIDENTIFIABLE_APPLICATION_PROFILE`. The
//!   simulator does not resolve `applicationProfileId` against a real profile
//!   store, so the all-zeros UUID
//!   (`00000000-0000-0000-0000-000000000000`) is the reserved value that names an
//!   *unidentifiable* application profile — the API's own 422 code — while every
//!   other well-formed UUID is treated as registered.
//!
//! On success the endpoints are stored under the minted id (so the later read leg
//! can return them) and the id is returned as the `200` body (CAMARA's
//! `ApplicationEndpointListId`, a bare `string`). `x-correlator` is echoed on
//! every response.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

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
use crate::scenarios;

/// Scope required to register endpoints (CAMARA ApplicationEndpointRegistration).
const WRITE_SCOPE: &str = "application-endpoint-registration:application-endpoints:write";

/// Scope required to read a registration back (CAMARA ApplicationEndpointRegistration).
const READ_SCOPE: &str = "application-endpoint-registration:application-endpoints:read";

/// The nil UUID — the reserved `applicationProfileId` that names an
/// *unidentifiable* application profile (→ `422 UNIDENTIFIABLE_APPLICATION_PROFILE`).
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// Routes for Application Endpoint Registration vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/application-endpoint-registration/vwip/application-endpoint-lists",
            post(register_application_endpoints).get(get_all_registered_application_endpoints),
        )
        .route(
            "/application-endpoint-registration/vwip/application-endpoint-lists/:application_endpoint_list_id",
            axum::routing::get(get_application_endpoints_by_id),
        )
}

/// `ApplicationEndpointsInfo` (CAMARA ApplicationEndpointRegistration). Deriving
/// `Serialize` lets the handler echo the validated registration back into the
/// store under the minted `applicationEndpointListId`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ApplicationEndpointsInfo {
    #[serde(rename = "applicationEndpoints")]
    application_endpoints: Vec<ApplicationEndpoint>,
    #[serde(rename = "applicationProviderName")]
    application_provider_name: String,
    #[serde(
        rename = "applicationDescription",
        skip_serializing_if = "Option::is_none"
    )]
    application_description: Option<String>,
    #[serde(rename = "applicationProfileId")]
    application_profile_id: String,
}

/// A single `ApplicationEndpoint`. `port` is required; at least one of
/// `domainName` / `ipv4Address` / `ipv6Address` must be present (the CAMARA
/// `anyOf`, checked in the handler since serde cannot express it).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ApplicationEndpoint {
    #[serde(rename = "domainName", skip_serializing_if = "Option::is_none")]
    domain_name: Option<String>,
    #[serde(rename = "ipv4Address", skip_serializing_if = "Option::is_none")]
    ipv4_address: Option<String>,
    #[serde(rename = "ipv6Address", skip_serializing_if = "Option::is_none")]
    ipv6_address: Option<String>,
    port: i64,
    #[serde(rename = "edgeCloudZone", skip_serializing_if = "Option::is_none")]
    edge_cloud_zone: Option<EdgeCloudZone>,
    #[serde(
        rename = "applicationEndpointDescription",
        skip_serializing_if = "Option::is_none"
    )]
    application_endpoint_description: Option<String>,
}

impl ApplicationEndpoint {
    /// Whether at least one address form is present (the CAMARA `anyOf`).
    fn has_address(&self) -> bool {
        self.domain_name.is_some() || self.ipv4_address.is_some() || self.ipv6_address.is_some()
    }
}

/// An `EdgeCloudZone` placement identifier (not a coordinate). `edgeCloudZoneId`
/// / `edgeCloudZoneName` / `edgeCloudProvider` are required by the schema (parse
/// enforces presence); `edgeCloudZoneId` is `format: uuid` (checked in the handler).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EdgeCloudZone {
    #[serde(rename = "edgeCloudZoneId")]
    edge_cloud_zone_id: String,
    #[serde(rename = "edgeCloudZoneName")]
    edge_cloud_zone_name: String,
    #[serde(rename = "edgeCloudProvider")]
    edge_cloud_provider: String,
    #[serde(rename = "edgeCloudRegion", skip_serializing_if = "Option::is_none")]
    edge_cloud_region: Option<String>,
    #[serde(
        rename = "edgeCloudZoneStatus",
        skip_serializing_if = "Option::is_none"
    )]
    edge_cloud_zone_status: Option<EdgeCloudZoneStatus>,
}

/// `edgeCloudZoneStatus` enum. Unknown values are rejected at parse.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum EdgeCloudZoneStatus {
    Active,
    Inactive,
    Unknown,
}

/// `POST /application-endpoint-registration/vwip/application-endpoint-lists`.
async fn register_application_endpoints(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the write scope.
    if let Err(e) = claims.require_scope(WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Parse strictly (unknown fields / wrong types / missing required → 400).
    let req: ApplicationEndpointsInfo = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid ApplicationEndpointsInfo.",
                &correlator,
            )
        }
    };

    // Validate the body (endpoint count, per-endpoint anyOf/port/address forms,
    // provider/description text, applicationProfileId shape).
    if let Err(resp) = validate(&req, &correlator) {
        return resp;
    }

    // `applicationProfileId` control planes: the shared reserved-error suffix
    // first (a canonical CAMARA error), then the nil-UUID sentinel (the API's own
    // UNIDENTIFIABLE_APPLICATION_PROFILE); every other well-formed UUID registers.
    if let Some(err) = scenarios::reserved_error(&req.application_profile_id) {
        return with_correlator(err.into_response(), &correlator);
    }
    if req.application_profile_id == NIL_UUID {
        return with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNIDENTIFIABLE_APPLICATION_PROFILE",
                "The provided applicationProfileId does not identify a known application profile.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Mint the id, remember the rendered registration in the canonical CAMARA
    // `ApplicationEndpointList` shape (`applicationEndpointListId` +
    // `applicationEndpointsInfo`), so the read-back and list legs return it
    // verbatim, and return 200 with the id (CAMARA `ApplicationEndpointListId`,
    // a bare string).
    let list_id = store::new_list_id();
    let info = serde_json::to_value(&req).unwrap_or_else(|_| json!({}));
    let rendered = json!({
        "applicationEndpointListId": list_id,
        "applicationEndpointsInfo": info,
    });
    store::insert(list_id.clone(), rendered);

    with_correlator(
        (StatusCode::OK, Json(json!(list_id))).into_response(),
        &correlator,
    )
}

/// `GET /application-endpoint-registration/vwip/application-endpoint-lists/{applicationEndpointListId}`.
///
/// The **read-back** leg: returns the registration stored under the opaque
/// `applicationEndpointListId` (`200`) or `404 NOT_FOUND` for an unknown id. The
/// id is server-minted (`format: uuid`), so a malformed path value is a `400
/// INVALID_ARGUMENT` (mirroring Application Profiles' `getApplicationProfile`);
/// the opaque id is the only control plane — it was never caller-chosen, so there
/// is no reserved-identifier suffix on it. `x-correlator` is echoed.
async fn get_application_endpoints_by_id(
    claims: Claims,
    headers: HeaderMap,
    Path(application_endpoint_list_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the read scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path parameter is `format: uuid`; a malformed value is a `400` (mirrors
    // Application Profiles' read/update/delete bad-shape → 400 vs unknown → 404).
    if !is_uuid_shaped(&application_endpoint_list_id) {
        return invalid_argument("`applicationEndpointListId` must be a UUID.", &correlator);
    }

    match store::get(&application_endpoint_list_id) {
        Some(registration) => with_correlator(
            (StatusCode::OK, Json(registration)).into_response(),
            &correlator,
        ),
        None => with_correlator(
            CamaraError::not_found(
                "No application-endpoint list found for the provided applicationEndpointListId.",
            )
            .into_response(),
            &correlator,
        ),
    }
}

/// `GET /application-endpoint-registration/vwip/application-endpoint-lists`.
///
/// The **list** leg (operationId `getAllRegisteredApplicationEndpoints`): returns
/// every registered application-endpoint list as an array of the canonical CAMARA
/// `ApplicationEndpointList` (`applicationEndpointListId` +
/// `applicationEndpointsInfo`), the same shape the read-back leg returns for one
/// id. The store state is the only control plane — there is no request body and no
/// device identifier — so a list never `404`s: with nothing registered the body is
/// an empty array (`200`). CamaraSim does not scope registrations per client (a
/// documented simplification, mirroring the Carrier Billing / Geofencing lists).
/// `x-correlator` is echoed.
async fn get_all_registered_application_endpoints(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the read scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    with_correlator(
        (StatusCode::OK, Json(json!(store::all()))).into_response(),
        &correlator,
    )
}

/// Validate an `ApplicationEndpointsInfo` body (everything except the
/// `applicationProfileId` reserved-error / sentinel planes, which the handler
/// applies after this).
fn validate(
    req: &ApplicationEndpointsInfo,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    // `applicationEndpoints`: non-empty, at most 20 (maxItems).
    if req.application_endpoints.is_empty() {
        return Err(invalid_argument(
            "`applicationEndpoints` must contain at least one endpoint.",
            correlator,
        ));
    }
    if req.application_endpoints.len() > 20 {
        return Err(invalid_argument(
            "`applicationEndpoints` must contain at most 20 endpoints.",
            correlator,
        ));
    }

    for (i, ep) in req.application_endpoints.iter().enumerate() {
        // anyOf: at least one address form.
        if !ep.has_address() {
            return Err(invalid_argument(
                &format!(
                    "`applicationEndpoints[{i}]` must carry at least one of `domainName`, `ipv4Address`, or `ipv6Address`."
                ),
                correlator,
            ));
        }
        // `port` is `1..=65535`.
        if !(1..=65535).contains(&ep.port) {
            return Err(out_of_range(
                &format!("`applicationEndpoints[{i}].port` must be in 1..=65535."),
                correlator,
            ));
        }
        // Address forms, when present, must be well-formed.
        if let Some(v4) = &ep.ipv4_address {
            if Ipv4Addr::from_str(v4).is_err() {
                return Err(invalid_argument(
                    &format!("`applicationEndpoints[{i}].ipv4Address` is not a valid IPv4 address."),
                    correlator,
                ));
            }
        }
        if let Some(v6) = &ep.ipv6_address {
            if Ipv6Addr::from_str(v6).is_err() {
                return Err(invalid_argument(
                    &format!("`applicationEndpoints[{i}].ipv6Address` is not a valid IPv6 address."),
                    correlator,
                ));
            }
        }
        if let Some(dn) = &ep.domain_name {
            if !(4..=253).contains(&dn.chars().count()) {
                return Err(invalid_argument(
                    &format!("`applicationEndpoints[{i}].domainName` must be 4..=253 characters."),
                    correlator,
                ));
            }
        }
        // `edgeCloudZone.edgeCloudZoneId` is `format: uuid`.
        if let Some(zone) = &ep.edge_cloud_zone {
            if !is_uuid_shaped(&zone.edge_cloud_zone_id) {
                return Err(invalid_argument(
                    &format!(
                        "`applicationEndpoints[{i}].edgeCloudZone.edgeCloudZoneId` must be a UUID."
                    ),
                    correlator,
                ));
            }
        }
    }

    // `applicationProviderName`: ≤256 chars, no CR/LF (`^[^\r\n]*$`).
    if !is_single_line(&req.application_provider_name, 256) {
        return Err(invalid_argument(
            "`applicationProviderName` must be at most 256 characters and contain no line breaks.",
            correlator,
        ));
    }
    // `applicationDescription`: ≤2048 chars, no CR/LF, when present.
    if let Some(desc) = &req.application_description {
        if !is_single_line(desc, 2048) {
            return Err(invalid_argument(
                "`applicationDescription` must be at most 2048 characters and contain no line breaks.",
                correlator,
            ));
        }
    }

    // `applicationProfileId` is `format: uuid`.
    if !is_uuid_shaped(&req.application_profile_id) {
        return Err(invalid_argument(
            "`applicationProfileId` must be a UUID.",
            correlator,
        ));
    }

    Ok(())
}

/// Whether `s` is at most `max` characters and carries no CR/LF (CAMARA's
/// `^[^\r\n]*$` single-line pattern).
fn is_single_line(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.contains(['\r', '\n'])
}

/// Whether `s` is a lowercase `8-4-4-4-12` hex UUID (matching the ids
/// [`store::new_list_id`] mints and CAMARA's `format: uuid`).
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

    const HOST: &str = "appendpointreg.local:8080";
    const LISTS: &str = "/application-endpoint-registration/vwip/application-endpoint-lists";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_shape_check() {
        assert!(is_uuid_shaped("00000000-0000-4000-8000-000000000000"));
        assert!(is_uuid_shaped(NIL_UUID));
        assert!(is_uuid_shaped(&store::new_list_id()));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("0000-0000-4000-8000-000000000000")); // wrong lengths
        assert!(!is_uuid_shaped("gggggggg-0000-4000-8000-000000000000")); // non-hex
    }

    #[test]
    fn single_line_check() {
        assert!(is_single_line("Acme Corp", 256));
        assert!(is_single_line("", 256));
        assert!(!is_single_line("line\nbreak", 256));
        assert!(!is_single_line("carriage\rreturn", 256));
        assert!(!is_single_line(&"x".repeat(257), 256));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body =
            format!("grant_type=client_credentials&client_id=appendpointreg-client&scope={scope}");
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

    async fn post_list(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(LISTS)
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
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    async fn get_list(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{LISTS}/{id}"))
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

    /// Register `body` and return the minted `applicationEndpointListId`.
    async fn register_and_get_id(token: &str, body: &str) -> String {
        let (status, _, out) = post_list(Some(token), body, None).await;
        assert_eq!(status, StatusCode::OK);
        out.as_str().expect("200 body is a list id").to_string()
    }

    /// A minimal valid registration body with the given `applicationProfileId`.
    fn valid_body(profile_id: &str) -> String {
        json!({
            "applicationEndpoints": [
                {
                    "ipv4Address": "192.0.2.10",
                    "port": 443,
                    "edgeCloudZone": {
                        "edgeCloudZoneId": "abcdef01-0000-4000-8000-000000000001",
                        "edgeCloudZoneName": "zone-eu-1",
                        "edgeCloudProvider": "AcmeCloud",
                        "edgeCloudRegion": "eu-west",
                        "edgeCloudZoneStatus": "active"
                    }
                }
            ],
            "applicationProviderName": "Acme Corp",
            "applicationProfileId": profile_id
        })
        .to_string()
    }

    // A well-formed, non-reserved, non-nil profile id (trailing digits 012).
    const OK_PROFILE: &str = "abcdef01-0000-4000-8000-000000000012";

    #[tokio::test]
    async fn register_returns_200_with_a_uuid_list_id() {
        let token = mint_token(WRITE_SCOPE).await;
        let (status, headers, body) =
            post_list(Some(&token), &valid_body(OK_PROFILE), Some("corr-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-1");
        // The body is a bare UUID string (CAMARA ApplicationEndpointListId).
        let id = body.as_str().expect("200 body is a JSON string");
        assert_eq!(id.split('-').count(), 5);
        assert!(is_uuid_shaped(id));
    }

    #[tokio::test]
    async fn register_with_domain_and_ipv6_is_accepted() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [
                { "domainName": "api.example.com", "port": 8443 },
                { "ipv6Address": "2001:db8::1", "port": 9000 }
            ],
            "applicationProviderName": "Acme Corp",
            "applicationDescription": "Two endpoints",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let (status, _, out) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(is_uuid_shaped(out.as_str().unwrap()));
    }

    #[tokio::test]
    async fn empty_endpoints_array_is_400() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [],
            "applicationProviderName": "Acme",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn endpoint_without_any_address_is_400() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [ { "port": 443 } ],
            "applicationProviderName": "Acme",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn port_out_of_range_is_400_out_of_range() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [ { "ipv4Address": "192.0.2.1", "port": 70000 } ],
            "applicationProviderName": "Acme",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn malformed_ipv4_is_400_invalid_argument() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [ { "ipv4Address": "999.1.1.1", "port": 443 } ],
            "applicationProviderName": "Acme",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_uuid_edge_cloud_zone_id_is_400() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [ {
                "ipv4Address": "192.0.2.1", "port": 443,
                "edgeCloudZone": {
                    "edgeCloudZoneId": "not-a-uuid",
                    "edgeCloudZoneName": "z", "edgeCloudProvider": "p"
                }
            } ],
            "applicationProviderName": "Acme",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_400_invalid_argument() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [ { "ipv4Address": "192.0.2.1", "port": 443 } ],
            "applicationProviderName": "Acme",
            "applicationProfileId": OK_PROFILE,
            "surprise": true
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_uuid_application_profile_id_is_400() {
        let token = mint_token(WRITE_SCOPE).await;
        let body = json!({
            "applicationEndpoints": [ { "ipv4Address": "192.0.2.1", "port": 443 } ],
            "applicationProviderName": "Acme",
            "applicationProfileId": "not-a-uuid"
        })
        .to_string();
        let (status, _, err) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn reserved_profile_suffix_404_maps_to_not_found() {
        let token = mint_token(WRITE_SCOPE).await;
        // applicationProfileId trailing digits …404 → canonical 404 NOT_FOUND.
        let profile = "abcdef01-0000-4000-8000-000000000404";
        let (status, headers, err) =
            post_list(Some(&token), &valid_body(profile), Some("corr-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-404");
    }

    #[tokio::test]
    async fn reserved_profile_suffix_422_maps_to_service_not_applicable() {
        let token = mint_token(WRITE_SCOPE).await;
        let profile = "abcdef01-0000-4000-8000-000000000422";
        let (status, _, err) = post_list(Some(&token), &valid_body(profile), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn nil_uuid_profile_is_422_unidentifiable_application_profile() {
        let token = mint_token(WRITE_SCOPE).await;
        let (status, _, err) = post_list(Some(&token), &valid_body(NIL_UUID), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err["code"], "UNIDENTIFIABLE_APPLICATION_PROFILE");
    }

    #[tokio::test]
    async fn register_requires_the_write_scope() {
        // A token with an unrelated scope may not register → 403.
        let token = mint_token("application-endpoint-registration:application-endpoints:read").await;
        let (status, _, err) = post_list(Some(&token), &valid_body(OK_PROFILE), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_401() {
        let (status, _, _) = post_list(None, &valid_body(OK_PROFILE), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- Read-back: GET /application-endpoint-lists/{applicationEndpointListId} ---

    #[tokio::test]
    async fn register_then_read_back_returns_the_stored_registration() {
        let write = mint_token(WRITE_SCOPE).await;
        let id = register_and_get_id(&write, &valid_body(OK_PROFILE)).await;

        let read = mint_token(READ_SCOPE).await;
        let (status, headers, body) = get_list(Some(&read), &id, Some("corr-read")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-read");
        // The stored registration is the canonical nested `ApplicationEndpointList`:
        // the minted id at top level and the submitted fields under
        // `applicationEndpointsInfo`.
        assert_eq!(body["applicationEndpointListId"], id);
        let info = &body["applicationEndpointsInfo"];
        assert_eq!(info["applicationProviderName"], "Acme Corp");
        assert_eq!(info["applicationProfileId"], OK_PROFILE);
        assert_eq!(info["applicationEndpoints"][0]["ipv4Address"], "192.0.2.10");
        assert_eq!(info["applicationEndpoints"][0]["port"], 443);
    }

    #[tokio::test]
    async fn read_back_unknown_id_is_404_not_found() {
        let read = mint_token(READ_SCOPE).await;
        // Well-formed UUID that was never registered.
        let unknown = "abcdef01-0000-4000-8000-000000000abc";
        let (status, headers, err) = get_list(Some(&read), unknown, Some("corr-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-404");
    }

    #[tokio::test]
    async fn read_back_malformed_id_is_400_invalid_argument() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, err) = get_list(Some(&read), "not-a-uuid", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn read_back_requires_the_read_scope() {
        // Register with the write scope, then try to read it back with only the
        // write scope (not read) → 403.
        let write = mint_token(WRITE_SCOPE).await;
        let id = register_and_get_id(&write, &valid_body(OK_PROFILE)).await;
        let (status, _, err) = get_list(Some(&write), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn read_back_missing_token_is_401() {
        let (status, _, _) = get_list(None, OK_PROFILE, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // --- List: GET /application-endpoint-lists (getAllRegisteredApplicationEndpoints) ---

    /// `GET /application-endpoint-lists` (no path id) → the whole list.
    async fn get_all(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(LISTS)
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
    async fn list_returns_200_array_containing_registered_lists_in_nested_shape() {
        let write = mint_token(WRITE_SCOPE).await;
        // Register a list with a distinctive provider name so we can find it
        // among the process-global store's other entries.
        let body = json!({
            "applicationEndpoints": [ { "ipv4Address": "192.0.2.55", "port": 8080 } ],
            "applicationProviderName": "ListProbe Ltd",
            "applicationProfileId": OK_PROFILE
        })
        .to_string();
        let id = register_and_get_id(&write, &body).await;

        let read = mint_token(READ_SCOPE).await;
        let (status, headers, out) = get_all(Some(&read), Some("corr-list")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-correlator").unwrap(), "corr-list");

        let arr = out.as_array().expect("list body is a JSON array");
        // The store is process-global, so assert containment (not an exact count):
        // our freshly registered id must be present, in the nested
        // `ApplicationEndpointList` shape.
        let mine = arr
            .iter()
            .find(|v| v["applicationEndpointListId"] == json!(id))
            .expect("the registered list id must appear in the list");
        assert_eq!(
            mine["applicationEndpointsInfo"]["applicationProviderName"],
            "ListProbe Ltd"
        );
        assert_eq!(
            mine["applicationEndpointsInfo"]["applicationEndpoints"][0]["port"],
            8080
        );
    }

    #[tokio::test]
    async fn list_is_sorted_by_id() {
        // Register a couple of lists, then confirm the whole response is sorted by
        // applicationEndpointListId (deterministic ordering, per store::all).
        let write = mint_token(WRITE_SCOPE).await;
        register_and_get_id(&write, &valid_body(OK_PROFILE)).await;
        register_and_get_id(&write, &valid_body(OK_PROFILE)).await;

        let read = mint_token(READ_SCOPE).await;
        let (status, _, out) = get_all(Some(&read), None).await;
        assert_eq!(status, StatusCode::OK);
        let ids: Vec<&str> = out
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v["applicationEndpointListId"].as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[tokio::test]
    async fn list_requires_the_read_scope() {
        // A token with only the write scope may not list → 403.
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, err) = get_all(Some(&write), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn list_missing_token_is_401() {
        let (status, _, _) = get_all(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
