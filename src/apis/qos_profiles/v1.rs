//! QoS Profiles **v1** (CAMARA QoS Profiles 1.1.0, release r3.2).
//!
//! Two endpoints:
//! - `POST /qos-profiles/v1/retrieve-qos-profiles` — list the QoS profiles the
//!   operator offers, optionally narrowed by `name`, `status`, and `device`
//!   (operationId `retrieveQoSProfiles`).
//! - `GET /qos-profiles/v1/qos-profiles/{name}` — look up a single profile by
//!   its exact name (operationId `getQosProfile`). The `name` path parameter is
//!   the control plane (DESIGN §7): a known name → `200` with that profile, an
//!   unknown (but well-formed) name → `404 NOT_FOUND` (unlike the list, which
//!   never 404s), a malformed name → `400 INVALID_ARGUMENT`. It reads the same
//!   fixed catalog as the list; there is no `device`/body, so no error plane.
//!
//! ## What it does
//!
//! CamaraSim serves a **fixed catalog** of QoS profiles (there is no upstream
//! network to query — DESIGN §7). A profile is a rich, static description of a
//! service level (rates, priority, packet-delay budget, service class, …). The
//! endpoint returns the catalog as a JSON array, filtered by the request:
//!
//! - `name` — return only the profile with that exact name (an unknown name →
//!   an empty array; a *list* never 404s, unlike the single-profile lookup).
//! - `status` — return only profiles in that lifecycle state (`ACTIVE`,
//!   `INACTIVE`, `DEPRECATED`).
//!
//! Both filters combine (AND). With neither, the whole catalog is returned.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `qos-profiles:read` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes drive the outcome:
//!
//! 1. **The `device` (error plane).** `device` is *optional* — the catalog
//!    exists independently of any subscriber — so when it is omitted there is no
//!    identifier case and the (filtered) catalog is returned. When it *is*
//!    supplied, its first present identifier (phoneNumber → networkAccessIdentifier
//!    → IPv4 publicAddress → ipv6Address) is read, and its trailing three digits
//!    select a reserved CAMARA error (shared convention, [`crate::scenarios`]):
//!    `…404` → `404 NOT_FOUND`, `…422` → `422 SERVICE_NOT_APPLICABLE`, etc.
//!    Faithful to CAMARA's two-legged / three-legged rule, supplying a `device`
//!    on a three-legged token whose subject already identifies a line (an E.164
//!    `sub`) → `422 UNNECESSARY_IDENTIFIER`.
//! 2. **The `name`/`status` filters.** Genuine control planes over *which*
//!    profiles come back — e.g. `status: DEPRECATED` returns only the deprecated
//!    profile(s), and `name: voice` returns just that one.
//!
//! Examples: an empty body → the full catalog; `{"status":"ACTIVE"}` → the
//! active profiles; `{"name":"voice"}` → `[ voice ]`; `{"name":"nope"}` → `[]`;
//! `{"device":{"phoneNumber":"+123456789404"}}` → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope both QoS Profiles v1 endpoints require (CAMARA QoS Profiles
/// 1.1.0). `retrieveQoSProfiles` and `getQosProfile` share `qos-profiles:read`.
const RETRIEVE_SCOPE: &str = "qos-profiles:read";

/// Routes for QoS Profiles v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/qos-profiles/v1/retrieve-qos-profiles",
            post(retrieve_qos_profiles),
        )
        // Single-profile lookup by name. The static `/retrieve-qos-profiles`
        // above and this `/qos-profiles/:name` param route live under different
        // first segments, so `matchit` never has to disambiguate them.
        .route("/qos-profiles/v1/qos-profiles/:name", get(get_qos_profile))
}

/// `POST /retrieve-qos-profiles` request body (CAMARA `QosProfileDeviceRequest`).
/// Every field is optional: an empty body returns the whole catalog.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    device: Option<Device>,
    name: Option<String>,
    status: Option<QosProfileStatus>,
}

/// The QoS-profile lifecycle state (CAMARA `QosProfileStatusEnum`). Doubles as
/// the `status` request filter and each catalog entry's `status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum QosProfileStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "INACTIVE")]
    Inactive,
    #[serde(rename = "DEPRECATED")]
    Deprecated,
}

impl QosProfileStatus {
    /// The canonical CAMARA wire string.
    fn as_str(self) -> &'static str {
        match self {
            QosProfileStatus::Active => "ACTIVE",
            QosProfileStatus::Inactive => "INACTIVE",
            QosProfileStatus::Deprecated => "DEPRECATED",
        }
    }
}

/// The CAMARA `Device` object: at least one identifier must be present
/// (`minProperties: 1`). CamaraSim keys its error case off the first present
/// identifier, in the precedence order below.
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

/// `POST /qos-profiles/v1/retrieve-qos-profiles`.
async fn retrieve_qos_profiles(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (all fields optional); anything present must parse.
    let req: RetrieveRequest = if body.is_empty() {
        RetrieveRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid QosProfileDeviceRequest.",
                    &correlator,
                )
            }
        }
    };

    // A supplied `name` must satisfy the CAMARA QosProfileName pattern.
    if let Some(name) = &req.name {
        if !is_valid_profile_name(name) {
            return invalid_argument(
                "`name` must match ^[a-zA-Z0-9_.-]+$ and be 3–256 characters.",
                &correlator,
            );
        }
    }

    // Error plane: a supplied `device` may select a reserved CAMARA error, and —
    // on a three-legged line token — is redundant (UNNECESSARY_IDENTIFIER).
    if let Some(device) = &req.device {
        if let Err(resp) = check_device(device, &claims, &correlator) {
            return resp;
        }
    }

    // Filter the fixed catalog by the (validated) name/status filters.
    let profiles: Vec<Value> = catalog()
        .into_iter()
        .filter(|p| {
            req.name
                .as_deref()
                .is_none_or(|n| p["name"] == n)
                && req
                    .status
                    .is_none_or(|s| p["status"] == s.as_str())
        })
        .collect();

    with_correlator((StatusCode::OK, Json(Value::Array(profiles))).into_response(), &correlator)
}

/// `GET /qos-profiles/v1/qos-profiles/{name}` — the single-profile lookup
/// (`getQosProfile`).
///
/// Unlike the list, this operation *does* 404: the `name` path parameter is the
/// sole control plane (DESIGN §7). A well-formed name that names a catalog entry
/// returns that entry (`200`); a well-formed name that does not → `404
/// NOT_FOUND`; a name violating the CAMARA `QosProfileName` schema → `400
/// INVALID_ARGUMENT`. There is no request body and no `device`, so no error
/// plane beyond the name itself.
async fn get_qos_profile(claims: Claims, headers: HeaderMap, Path(name): Path<String>) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path segment must satisfy the CAMARA QosProfileName pattern/length.
    if !is_valid_profile_name(&name) {
        return invalid_argument(
            "`name` must match ^[a-zA-Z0-9_.-]+$ and be 3–256 characters.",
            &correlator,
        );
    }

    // Look the name up in the fixed catalog; found → the profile, else 404.
    match catalog().into_iter().find(|p| p["name"] == name) {
        Some(profile) => {
            with_correlator((StatusCode::OK, Json(profile)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No QoS profile with the given name exists.").into_response(),
            &correlator,
        ),
    }
}

/// Apply the `device` error plane: resolve its identifier and enforce the CAMARA
/// two-legged / three-legged rule plus the shared reserved-error convention.
///
/// Returns `Ok(())` when the request may proceed, or `Err(response)` carrying the
/// CAMARA error to send: `400 INVALID_ARGUMENT` (malformed `phoneNumber` or a
/// `device` with no identifier), `422 UNNECESSARY_IDENTIFIER` (a `device` on a
/// line-authenticated three-legged token), or the reserved-suffix error.
fn check_device(
    device: &Device,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    let identifier = match device_identifier(device) {
        Some(DeviceId::PhoneNumber(phone)) => {
            if !is_valid_e164(&phone) {
                return Err(invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    correlator,
                ));
            }
            phone
        }
        Some(DeviceId::Other(id)) => id,
        None => {
            return Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            ))
        }
    };

    // Three-legged: the token subject already identifies the line, so an explicit
    // device is redundant (CAMARA UNNECESSARY_IDENTIFIER).
    if is_valid_e164(claims.subject().unwrap_or("")) {
        return Err(unprocessable(
            "UNNECESSARY_IDENTIFIER",
            "The device is already identified by the access token; do not also supply `device`.",
            correlator,
        ));
    }

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return Err(with_correlator(err.into_response(), correlator));
    }

    Ok(())
}

/// The fixed catalog of QoS profiles CamaraSim offers. Static data — a real
/// operator would source these from its policy configuration. Chosen to cover
/// every `QosProfileStatusEnum` value and a spread of service classes / L4S queue
/// types so the `name`/`status` filters are meaningful control planes.
fn catalog() -> Vec<Value> {
    vec![
        json!({
            "name": "voice",
            "description": "Low-latency profile for real-time interactive voice.",
            "status": "ACTIVE",
            "targetMinUpstreamRate": { "value": 64, "unit": "kbps" },
            "maxUpstreamRate": { "value": 128, "unit": "kbps" },
            "targetMinDownstreamRate": { "value": 64, "unit": "kbps" },
            "maxDownstreamRate": { "value": 128, "unit": "kbps" },
            "minDuration": { "value": 1, "unit": "Minutes" },
            "maxDuration": { "value": 24, "unit": "Hours" },
            "priority": 20,
            "packetDelayBudget": { "value": 100, "unit": "Milliseconds" },
            "jitter": { "value": 20, "unit": "Milliseconds" },
            "packetErrorLossRate": 6,
            "l4sQueueType": "non-l4s-queue",
            "serviceClass": "real_time_interactive"
        }),
        json!({
            "name": "video",
            "description": "High-throughput profile for multimedia streaming.",
            "status": "ACTIVE",
            "targetMinDownstreamRate": { "value": 5, "unit": "Mbps" },
            "maxDownstreamRate": { "value": 50, "unit": "Mbps" },
            "targetMinUpstreamRate": { "value": 1, "unit": "Mbps" },
            "maxUpstreamRate": { "value": 5, "unit": "Mbps" },
            "minDuration": { "value": 1, "unit": "Minutes" },
            "maxDuration": { "value": 12, "unit": "Hours" },
            "priority": 40,
            "packetDelayBudget": { "value": 150, "unit": "Milliseconds" },
            "packetErrorLossRate": 4,
            "l4sQueueType": "mixed-queue",
            "serviceClass": "multimedia_streaming"
        }),
        json!({
            "name": "low-latency",
            "description": "L4S low-latency data profile for interactive applications.",
            "status": "ACTIVE",
            "targetMinDownstreamRate": { "value": 2, "unit": "Mbps" },
            "maxDownstreamRate": { "value": 20, "unit": "Mbps" },
            "priority": 10,
            "packetDelayBudget": { "value": 30, "unit": "Milliseconds" },
            "jitter": { "value": 5, "unit": "Milliseconds" },
            "packetErrorLossRate": 5,
            "l4sQueueType": "l4s-queue",
            "serviceClass": "low_latency_data"
        }),
        json!({
            "name": "standard",
            "description": "Best-effort baseline service class.",
            "status": "ACTIVE",
            "priority": 60,
            "serviceClass": "standard"
        }),
        json!({
            "name": "legacy-broadcast",
            "description": "Deprecated broadcast-video profile, retained for existing integrations.",
            "status": "DEPRECATED",
            "targetMinDownstreamRate": { "value": 3, "unit": "Mbps" },
            "maxDownstreamRate": { "value": 10, "unit": "Mbps" },
            "priority": 50,
            "serviceClass": "broadcast_video"
        }),
        json!({
            "name": "experimental-throughput",
            "description": "Inactive high-throughput profile, not yet available for deployment.",
            "status": "INACTIVE",
            "targetMinDownstreamRate": { "value": 50, "unit": "Mbps" },
            "maxDownstreamRate": { "value": 500, "unit": "Mbps" },
            "priority": 70,
            "serviceClass": "high_throughput_data"
        }),
    ]
}

/// The identifier CamaraSim reads from a `Device`, kept distinct for the
/// `phoneNumber` E.164 check. Precedence: phoneNumber, networkAccessIdentifier,
/// the IPv4 `publicAddress`, then ipv6Address.
enum DeviceId {
    PhoneNumber(String),
    Other(String),
}

/// The first present identifier of a `Device`, in precedence order, or `None`
/// when the device carries no identifier at all (`minProperties: 1` violated).
fn device_identifier(device: &Device) -> Option<DeviceId> {
    if let Some(phone) = &device.phone_number {
        return Some(DeviceId::PhoneNumber(phone.clone()));
    }
    if let Some(nai) = &device.network_access_identifier {
        return Some(DeviceId::Other(nai.clone()));
    }
    if let Some(ipv4) = &device.ipv4_address {
        if let Some(addr) = &ipv4.public_address {
            return Some(DeviceId::Other(addr.clone()));
        }
    }
    if let Some(ipv6) = &device.ipv6_address {
        return Some(DeviceId::Other(ipv6.clone()));
    }
    None
}

/// Whether `name` satisfies the CAMARA `QosProfileName` schema: pattern
/// `^[a-zA-Z0-9_.-]+$` and length 3–256.
fn is_valid_profile_name(name: &str) -> bool {
    (3..=256).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 422 CAMARA error with a caller-chosen `code`, correlator echoed.
fn unprocessable(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message).into_response(),
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

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`:
/// a leading `+`, then 5–15 digits, the first of which is non-zero.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "qosprofiles.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn catalog_covers_every_status_and_names_are_unique() {
        let profiles = catalog();
        // Every entry has the required name + status, and the name is valid.
        for p in &profiles {
            let name = p["name"].as_str().expect("name present");
            assert!(is_valid_profile_name(name), "catalog name {name} is valid");
            assert!(p["status"].is_string(), "status present for {name}");
        }
        // Names are unique (so the `name` filter selects at most one).
        let mut names: Vec<&str> = profiles.iter().map(|p| p["name"].as_str().unwrap()).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "catalog names are unique");
        // All three lifecycle states are represented.
        for status in ["ACTIVE", "INACTIVE", "DEPRECATED"] {
            assert!(
                profiles.iter().any(|p| p["status"] == status),
                "catalog covers {status}"
            );
        }
    }

    #[test]
    fn profile_name_validation_follows_the_camara_pattern() {
        assert!(is_valid_profile_name("voice"));
        assert!(is_valid_profile_name("low-latency"));
        assert!(is_valid_profile_name("a_b.c-1"));
        assert!(!is_valid_profile_name("ab")); // too short
        assert!(!is_valid_profile_name("has space"));
        assert!(!is_valid_profile_name("bad/slash"));
        assert!(!is_valid_profile_name(""));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "qos-client").await
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

    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/qos-profiles/v1/retrieve-qos-profiles")
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

    async fn retrieve_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn empty_body_returns_the_whole_catalog() {
        let (status, _, body) = retrieve_ok("").await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("array response");
        assert_eq!(arr.len(), catalog().len());
        // Each item carries the required CAMARA fields.
        assert!(arr.iter().all(|p| p["name"].is_string() && p["status"].is_string()));
    }

    #[tokio::test]
    async fn empty_json_object_also_returns_the_whole_catalog() {
        let (status, _, body) = retrieve_ok("{}").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), catalog().len());
    }

    #[tokio::test]
    async fn name_filter_selects_one_profile() {
        let (status, _, body) = retrieve_ok(r#"{"name":"voice"}"#).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "voice");
        assert_eq!(arr[0]["serviceClass"], "real_time_interactive");
    }

    #[tokio::test]
    async fn unknown_name_returns_an_empty_array_not_404() {
        let (status, _, body) = retrieve_ok(r#"{"name":"no-such-profile"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn status_filter_narrows_the_catalog() {
        let (status, _, body) = retrieve_ok(r#"{"status":"DEPRECATED"}"#).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert!(!arr.is_empty());
        assert!(arr.iter().all(|p| p["status"] == "DEPRECATED"));

        let (status, _, body) = retrieve_ok(r#"{"status":"ACTIVE"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.as_array().unwrap().iter().all(|p| p["status"] == "ACTIVE"));
    }

    #[tokio::test]
    async fn name_and_status_combine() {
        // voice is ACTIVE → matches.
        let (status, _, body) = retrieve_ok(r#"{"name":"voice","status":"ACTIVE"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1);

        // voice is not DEPRECATED → empty.
        let (status, _, body) = retrieve_ok(r#"{"name":"voice","status":"DEPRECATED"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn device_reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            retrieve_ok(r#"{"device":{"phoneNumber":"+123456789404"}}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            retrieve_ok(r#"{"device":{"phoneNumber":"+123456789422"}}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn device_without_reserved_suffix_still_returns_the_catalog() {
        let (status, _, body) =
            retrieve_ok(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), catalog().len());
    }

    #[tokio::test]
    async fn non_phone_device_identifiers_are_accepted() {
        // ipv4 publicAddress ending in …404 → reserved error.
        let (status, _, body) =
            retrieve_ok(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.404"}}}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // networkAccessIdentifier with a happy-path tail → catalog.
        let (status, _, body) =
            retrieve_ok(r#"{"device":{"networkAccessIdentifier":"user012@nai"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), catalog().len());
    }

    #[tokio::test]
    async fn device_on_a_three_legged_line_token_is_unnecessary() {
        // Subject is an E.164 line; also supplying a device → 422.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"device":{"phoneNumber":"+123456789034"}}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_on_a_line_token_is_fine() {
        // A line-authenticated token without a device just lists the catalog;
        // the device is optional here (unlike identifier-required APIs).
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), catalog().len());
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = retrieve_ok(r#"{"device":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = retrieve_ok(r#"{"device":{"phoneNumber":"0123"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_name_is_rejected() {
        let (status, _, body) = retrieve_ok(r#"{"name":"bad name!"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_status_value_is_rejected() {
        let (status, _, body) = retrieve_ok(r#"{"status":"RETIRED"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = retrieve_ok(r#"{"foo":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_retrieve(None, "{}", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- getQosProfile (GET /qos-profiles/{name}) --------------------------

    async fn get_profile(
        token: Option<&str>,
        name: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/qos-profiles/v1/qos-profiles/{name}"))
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

    async fn get_profile_ok(name: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_SCOPE).await;
        get_profile(Some(&token), name, None).await
    }

    #[tokio::test]
    async fn get_known_profile_returns_the_single_object() {
        let (status, _, body) = get_profile_ok("voice").await;
        assert_eq!(status, StatusCode::OK);
        // A single object, not an array (unlike the list endpoint).
        assert!(body.is_object());
        assert_eq!(body["name"], "voice");
        assert_eq!(body["status"], "ACTIVE");
        assert_eq!(body["serviceClass"], "real_time_interactive");
    }

    #[tokio::test]
    async fn get_every_catalog_name_succeeds() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        for p in catalog() {
            let name = p["name"].as_str().unwrap();
            let (status, _, body) = get_profile(Some(&token), name, None).await;
            assert_eq!(status, StatusCode::OK, "GET {name}");
            assert_eq!(body["name"], name);
        }
    }

    #[tokio::test]
    async fn get_unknown_but_valid_name_is_404() {
        // Unlike the list (which returns []), the single lookup 404s.
        let (status, _, body) = get_profile_ok("no-such-profile").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_malformed_name_is_400() {
        // Too short (< 3) violates the QosProfileName schema.
        let (status, _, body) = get_profile_ok("ab").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn get_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_profile(Some(&token), "voice", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn get_missing_token_is_unauthenticated() {
        let (status, _, body) = get_profile(None, "voice", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn get_echoes_x_correlator_on_success_and_404() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) = get_profile(Some(&token), "voice", Some("corr-get")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get")
        );

        let (status, headers, _) = get_profile(Some(&token), "nope", Some("corr-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-404")
        );
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) = post_retrieve(Some(&token), "{}", Some("corr-qos")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-qos")
        );

        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789404"}}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
