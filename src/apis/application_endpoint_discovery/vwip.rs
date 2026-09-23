//! Application Endpoint Discovery **vwip** (CAMARA Application Endpoint Discovery,
//! work-in-progress).
//!
//! One endpoint:
//! - `POST /application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints` —
//!   return the optimal application **endpoint(s)** (`port` + IP/FQDN) that the
//!   identified device should connect to (operationId `getOptimalAppEndpoints`).
//!
//! ## What it does
//!
//! Where Simple / Optimal Edge Discovery answer with an edge cloud **zone**,
//! Application Endpoint Discovery goes one step further and returns the concrete
//! **application endpoints** — the `port` and IP address(es) or FQDN of the
//! application instance(s) with the shortest network path to the device — so a
//! developer can connect a client straight to them. The caller names the
//! application (`appId` for an app onboarded via Edge Application Management, or
//! `applicationEndpointsId` for endpoints registered via Edge Application
//! Registration) and identifies the device (a `device` object in the body —
//! two-legged / CIBA — or the identity a three-legged access token
//! authenticated).
//!
//! The response is an `EndpointDiscoveryResult`
//! (`{ applicationEndpoints: [ApplicationEndpoint], appId?|applicationEndpointsId?,
//! applicationServerProviderName?, device? }`).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `application-endpoint-discovery:app-endpoints:read` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA (mirrors Optimal Edge Discovery): the `device` in the body
//! is only meaningful in two-legged auth. In a three-legged token the device is
//! already identified by the token **subject**, so resubmitting it is an error:
//!
//! - `device` carries an identifier **and** the token subject is itself an E.164
//!   line (a device-authenticated three-legged token) →
//!   `422 UNNECESSARY_IDENTIFIER`.
//! - `device` carries an identifier, subject not a line → that identifier is used
//!   (two-legged). Precedence: `phoneNumber` → `networkAccessIdentifier` → the
//!   IPv4 `publicAddress` → `ipv6Address`.
//! - `device` absent, subject is an E.164 line → the subject is the identifier
//!   (three-legged).
//! - `device` absent **and** the subject is not a line → the device cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//! - `device` present but carrying **no** identifier (`minProperties: 1`
//!   violated) → `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two planes drive the answer:
//!
//! 1. **Application identifier** (structural, checked first) — exactly one of
//!    `appId` / `applicationEndpointsId` is required and must be a UUID. Neither,
//!    both, or a non-UUID → `400 INVALID_ARGUMENT`. (CAMARA's `anyOf` allows one
//!    *or* both; CamaraSim requires exactly one so the echoed identifier is
//!    unambiguous — a documented divergence.) The application identifier's
//!    trailing three digits `d` then drive the endpoint set: `…000` → the
//!    application/endpoints are **not registered** → `404 NOT_FOUND`; otherwise
//!    `(d % 3) + 1` endpoints (1–3) are returned.
//! 2. **Device identifier** — once resolved, a **reserved error suffix** (trailing
//!    three digits naming a reserved CAMARA status — `…400`, `…401`, `…403`,
//!    `…404`, `…409`, `…422`, `…429`, `…500`, `…503`) → that canonical CAMARA
//!    error (shared [`crate::scenarios`], checked first among the business planes,
//!    before the application-not-found `404`). This is where the device
//!    `404 IDENTIFIER_NOT_FOUND` / `429` / … cases live.
//!
//! Each returned endpoint is deterministic from the application identifier and its
//! rank: a rotating address family (`fqdn` / a single `ipv4Addresses` / a single
//! `ipv6Addresses`), a `port`, an `edgeCloudZone`, and a description — so the same
//! request always returns the same endpoints.
//!
//! The response echoes the `device` (CAMARA `DeviceResponse`, which carries only
//! `phoneNumber`) **only when the request `device` object carried multiple
//! identifiers** (faithful to the CAMARA rule "only included when multiple device
//! identifiers come in the request") and one of them is a `phoneNumber`; otherwise
//! the `device` field is omitted.
//!
//! Examples: `appId …012` → 1 endpoint (fqdn); `appId …014` → 3 endpoints (all
//! three address families); `appId …000` → `404 NOT_FOUND`;
//! `device.phoneNumber +123456789404` → `404 NOT_FOUND` (device not found).

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `getOptimalAppEndpoints` requires (CAMARA Application Endpoint
/// Discovery).
const READ_SCOPE: &str = "application-endpoint-discovery:app-endpoints:read";

/// The Application Provider name echoed on every successful response.
const PROVIDER_NAME: &str = "CamaraSim Edge";

/// The operator's fixed edge cloud zones, each `(edgeCloudZoneName,
/// edgeCloudProvider, edgeCloudRegion)`. Each returned application endpoint is
/// placed in one of these zones (chosen deterministically from the application
/// identifier and the endpoint's rank), so the placement is stable per request.
const EDGE_ZONES: [(&str, &str, &str); 6] = [
    ("camarasim-edge-eu-west-1", "CamaraSim Edge", "eu-west-1"),
    ("camarasim-edge-eu-central-1", "CamaraSim Edge", "eu-central-1"),
    ("camarasim-edge-us-east-1", "CamaraSim Edge", "us-east-1"),
    ("camarasim-edge-us-west-2", "CamaraSim Edge", "us-west-2"),
    ("camarasim-edge-ap-south-1", "CamaraSim Edge", "ap-south-1"),
    ("camarasim-edge-ap-northeast-1", "CamaraSim Edge", "ap-northeast-1"),
];

/// Routes for Application Endpoint Discovery vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints",
        post(retrieve_optimal_app_endpoints),
    )
}

/// Request body (CAMARA `EndpointDiscoveryInfo`). Exactly one of `appId` /
/// `applicationEndpointsId` is required; `device` is optional.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointDiscoveryInfo {
    device: Option<Device>,
    #[serde(rename = "appId")]
    app_id: Option<String>,
    #[serde(rename = "applicationEndpointsId")]
    application_endpoints_id: Option<String>,
}

/// The CAMARA `Device` object: at least one identifier must be present
/// (`minProperties: 1`). CamaraSim keys its device functional cases off the first
/// present identifier, in the precedence order below.
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

/// The application identifier the caller supplied — echoed back verbatim in the
/// response under the same field name.
enum AppRef {
    AppId(String),
    AppEndpointsId(String),
}

impl AppRef {
    /// The identifier string, whichever kind it is (its trailing three digits are
    /// the content control plane).
    fn value(&self) -> &str {
        match self {
            AppRef::AppId(v) | AppRef::AppEndpointsId(v) => v,
        }
    }
}

/// `POST /application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints`.
async fn retrieve_optimal_app_endpoints(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required — an application identifier is mandatory, so an empty
    // or malformed body cannot satisfy the schema.
    let req: EndpointDiscoveryInfo = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid EndpointDiscoveryInfo.",
                &correlator,
            )
        }
    };

    // Structural plane: exactly one of `appId` / `applicationEndpointsId`, UUID.
    let app_ref = match resolve_app_ref(&req, &correlator) {
        Ok(app_ref) => app_ref,
        Err(resp) => return resp,
    };

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The device identifier is a control plane (docs/DESIGN.md §7); a reserved
    // error suffix wins first, before the application-not-found case.
    if let Some(err) = scenarios::reserved_error(&resolved.identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Application content plane: the application identifier's trailing three
    // digits pick the endpoint set. `…000` → the application / its endpoints are
    // not registered on the Edge Cloud → 404 NOT_FOUND (the spec's app-not-found).
    let d = scenarios::trailing_three_digits(app_ref.value()).unwrap_or(0);
    if d == 0 {
        return with_correlator(
            CamaraError::for_status(404)
                .expect("404 is a canonical CAMARA status")
                .into_response(),
            &correlator,
        );
    }

    let endpoints = build_endpoints(app_ref.value(), d);

    let mut out = json!({ "applicationEndpoints": endpoints });
    match &app_ref {
        AppRef::AppId(v) => out["appId"] = json!(v),
        AppRef::AppEndpointsId(v) => out["applicationEndpointsId"] = json!(v),
    }
    out["applicationServerProviderName"] = json!(PROVIDER_NAME);
    // The CAMARA DeviceResponse carries only `phoneNumber`, and is echoed only
    // when the request device carried multiple identifiers.
    if let Some(phone) = resolved.echo_phone_number {
        out["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// Resolve the required application identifier: exactly one of `appId` /
/// `applicationEndpointsId`, each a UUID. Neither, both, or a non-UUID value →
/// `400 INVALID_ARGUMENT`.
fn resolve_app_ref(
    req: &EndpointDiscoveryInfo,
    correlator: &Option<HeaderValue>,
) -> Result<AppRef, Response> {
    match (req.app_id.as_deref(), req.application_endpoints_id.as_deref()) {
        (Some(_), Some(_)) => Err(invalid_argument(
            "Provide exactly one of `appId` / `applicationEndpointsId`.",
            correlator,
        )),
        (Some(app_id), None) => {
            if is_uuid_shaped(app_id) {
                Ok(AppRef::AppId(app_id.to_string()))
            } else {
                Err(invalid_argument("`appId` must be a UUID.", correlator))
            }
        }
        (None, Some(id)) => {
            if is_uuid_shaped(id) {
                Ok(AppRef::AppEndpointsId(id.to_string()))
            } else {
                Err(invalid_argument(
                    "`applicationEndpointsId` must be a UUID.",
                    correlator,
                ))
            }
        }
        (None, None) => Err(invalid_argument(
            "One of `appId` / `applicationEndpointsId` is required.",
            correlator,
        )),
    }
}

/// Build the deterministic application endpoint set for `app_id` whose trailing
/// three digits are `d` (`1..=999`). Returns `(d % 3) + 1` endpoints (1–3), each
/// with a rotating address family, a port, an edge cloud zone, and a description.
fn build_endpoints(app_id: &str, d: u16) -> Vec<Value> {
    let count = ((d % 3) + 1) as usize; // 1..=3
    (0..count).map(|rank| build_endpoint(app_id, d, rank)).collect()
}

/// One application endpoint. The address family rotates by `(d + rank) % 3`
/// (`0` → fqdn, `1` → a single IPv4, `2` → a single IPv6) so a 3-endpoint answer
/// exercises all three; every field is derived from `app_id` + `rank`.
fn build_endpoint(app_id: &str, d: u16, rank: usize) -> Value {
    let h = Sha256::digest(format!("aed-ep:{app_id}:{rank}").as_bytes());
    let mix = (d as usize) + rank;
    let port = 1024 + (mix % 60000);
    let (zone_name, provider, region) = EDGE_ZONES[mix % EDGE_ZONES.len()];

    let mut endpoint = json!({
        "port": port,
        "edgeCloudZone": {
            "edgeCloudZoneId": zone_id(zone_name),
            "edgeCloudZoneName": zone_name,
            "edgeCloudProvider": provider,
            "edgeCloudRegion": region,
            "edgeCloudZoneStatus": "active",
        },
        "applicationEndpointDescription":
            format!("Optimal application endpoint #{} for the requested application.", rank + 1),
    });

    match mix % 3 {
        0 => {
            // A deterministic, well-formed FQDN under a documentation-style domain.
            endpoint["fqdn"] = json!(format!(
                "app-{:02x}{:02x}{:02x}{:02x}.edge.camarasim.example.com",
                h[0], h[1], h[2], h[3]
            ));
        }
        1 => {
            // A single IPv4 in the TEST-NET-2 documentation range (RFC 5737).
            endpoint["ipv4Addresses"] = json!([format!("198.51.100.{}", (h[4] % 254) + 1)]);
        }
        _ => {
            // A single IPv6 in the documentation range 2001:db8::/32 (RFC 3849).
            let suffix = ((u16::from(h[5]) << 8) | u16::from(h[6])).max(1);
            endpoint["ipv6Addresses"] = json!([format!("2001:db8::{:x}", suffix)]);
        }
    }
    endpoint
}

/// A stable, UUID-shaped `edgeCloudZoneId` for a zone, derived from its name via
/// SHA-256 (deterministic, no new dependency). UUID-*shaped* (not a real v4/v5
/// UUID) — a documented cut mirroring Optimal / Simple Edge Discovery ids.
fn zone_id(zone_name: &str) -> String {
    let h = Sha256::digest(format!("aed-zone:{zone_name}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// A resolved device identifier plus the phone number to echo back in the
/// `DeviceResponse`, which — per CAMARA — is populated only when the request
/// `device` carried multiple identifiers (and one is a `phoneNumber`).
struct Resolved {
    identifier: String,
    echo_phone_number: Option<String>,
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Optimal Edge Discovery). See the module docs for the cases.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match device {
        Some(device) => {
            // Whether the request device carried multiple identifiers governs
            // whether the DeviceResponse is echoed (CAMARA rule).
            let multi = device_identifier_count(&device) >= 2;
            match device_identifier(&device) {
                Some(DeviceId::PhoneNumber(phone)) => {
                    if !is_valid_e164(&phone) {
                        return Err(invalid_argument(
                            "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                            correlator,
                        ));
                    }
                    if subject_is_line {
                        return Err(unprocessable(
                            "UNNECESSARY_IDENTIFIER",
                            "The device is already identified by the access token.",
                            correlator,
                        ));
                    }
                    Ok(Resolved {
                        identifier: phone.clone(),
                        echo_phone_number: if multi { Some(phone) } else { None },
                    })
                }
                Some(DeviceId::Other(id)) => {
                    if subject_is_line {
                        return Err(unprocessable(
                            "UNNECESSARY_IDENTIFIER",
                            "The device is already identified by the access token.",
                            correlator,
                        ));
                    }
                    Ok(Resolved {
                        identifier: id,
                        echo_phone_number: None,
                    })
                }
                None => Err(invalid_argument(
                    "`device` must contain at least one identifier.",
                    correlator,
                )),
            }
        }
        None => {
            if subject_is_line {
                // Three-legged: no request `device`, so the DeviceResponse is not
                // echoed (there were no "multiple identifiers in the request").
                Ok(Resolved {
                    identifier: subject.to_string(),
                    echo_phone_number: None,
                })
            } else {
                Err(unprocessable(
                    "MISSING_IDENTIFIER",
                    "The device cannot be identified: supply a `device` or use a token that identifies a device.",
                    correlator,
                ))
            }
        }
    }
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

/// How many top-level device identifiers the request carried (phoneNumber,
/// networkAccessIdentifier, ipv4Address, ipv6Address). Drives the CAMARA
/// "echo the device only when multiple identifiers were provided" rule.
fn device_identifier_count(device: &Device) -> usize {
    [
        device.phone_number.is_some(),
        device.network_access_identifier.is_some(),
        device.ipv4_address.is_some(),
        device.ipv6_address.is_some(),
    ]
    .iter()
    .filter(|present| **present)
    .count()
}

/// A 422 CAMARA error with a caller-chosen `code`, correlator echoed.
fn unprocessable(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message).into_response(),
        correlator,
    )
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
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

/// Whether `s` is a canonical UUID (the `appId` / `applicationEndpointsId`
/// format): 36 chars, hyphens at positions 8/13/18/23, hex elsewhere.
fn is_uuid_shaped(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    s.bytes().enumerate().all(|(i, b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "aed.local:8080";
    const PATH: &str = "/application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints";
    // Valid UUID application identifiers with controlled trailing digits. The last
    // three *digit characters* (non-digits skipped) are the content control plane.
    const APP_012: &str = "3fa85f64-5717-4562-b3fc-2c963f66a012"; // d = 012 → 1 endpoint
    const APP_013: &str = "3fa85f64-5717-4562-b3fc-2c963f66a013"; // d = 013 → 2 endpoints
    const APP_014: &str = "3fa85f64-5717-4562-b3fc-2c963f66a014"; // d = 014 → 3 endpoints
    const APP_000: &str = "3fa85f64-5717-4562-b3fc-2c963f66a000"; // d = 000 → 404
    const APP_ENDPOINTS_ID: &str = "4d596ac1-7822-4927-a3c5-d72e1f92a012";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_validation_follows_the_format() {
        assert!(is_uuid_shaped(APP_012));
        assert!(is_uuid_shaped(APP_ENDPOINTS_ID));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("3fa85f645717-4562-b3fc-2c963f66a012")); // missing hyphen
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66a01g")); // non-hex
    }

    #[test]
    fn endpoint_count_derives_from_the_trailing_digits() {
        // d=012 → (12 % 3) + 1 = 1; d=013 → 2; d=014 → 3.
        assert_eq!(build_endpoints(APP_012, 12).len(), 1);
        assert_eq!(build_endpoints(APP_013, 13).len(), 2);
        assert_eq!(build_endpoints(APP_014, 14).len(), 3);
    }

    #[test]
    fn every_endpoint_has_port_and_exactly_one_address_family() {
        for (app, d) in [(APP_012, 12u16), (APP_013, 13), (APP_014, 14)] {
            for ep in build_endpoints(app, d) {
                assert!(ep["port"].is_number(), "port present for {app}");
                let families = ["fqdn", "ipv4Addresses", "ipv6Addresses"]
                    .iter()
                    .filter(|k| ep.get(**k).is_some())
                    .count();
                assert_eq!(families, 1, "exactly one address family for {app}: {ep}");
                assert!(ep["edgeCloudZone"]["edgeCloudZoneId"].is_string());
            }
        }
    }

    #[test]
    fn a_three_endpoint_answer_exercises_all_address_families() {
        // d=014: rank 0 → (14%3)=2 ipv6, 1 → (15%3)=0 fqdn, 2 → (16%3)=1 ipv4.
        let eps = build_endpoints(APP_014, 14);
        assert_eq!(eps.len(), 3);
        assert!(eps[0].get("ipv6Addresses").is_some());
        assert!(eps[1].get("fqdn").is_some());
        assert!(eps[2].get("ipv4Addresses").is_some());
    }

    #[test]
    fn endpoints_are_deterministic() {
        assert_eq!(build_endpoints(APP_013, 13), build_endpoints(APP_013, 13));
    }

    #[test]
    fn zone_id_is_deterministic_uuid_shaped_and_stable_per_zone() {
        let a = zone_id(EDGE_ZONES[0].0);
        assert_eq!(a, zone_id(EDGE_ZONES[0].0));
        assert_ne!(zone_id(EDGE_ZONES[0].0), zone_id(EDGE_ZONES[1].0));
        let groups: Vec<&str> = a.split('-').collect();
        assert_eq!(groups.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
    }

    #[test]
    fn device_identifier_count_and_precedence() {
        let two = Device {
            phone_number: Some("+123456789012".into()),
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.5".into()),
                private_address: None,
                public_port: None,
            }),
            ..Device::default()
        };
        assert_eq!(device_identifier_count(&two), 2);
        assert!(matches!(device_identifier(&two), Some(DeviceId::PhoneNumber(p)) if p == "+123456789012"));

        let one = Device {
            network_access_identifier: Some("nai@example.com".into()),
            ..Device::default()
        };
        assert_eq!(device_identifier_count(&one), 1);
        assert!(matches!(device_identifier(&one), Some(DeviceId::Other(p)) if p == "nai@example.com"));
        assert_eq!(device_identifier_count(&Device::default()), 0);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "aed-client").await
    }

    /// Mint an access token via `client_credentials` with a caller-chosen
    /// `client_id` (which becomes the token `sub`). `+` is percent-encoded so an
    /// E.164 client id survives the urlencoded body.
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

    /// POST to the endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(PATH)
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

    /// Mint a scoped (two-legged, non-line subject) token and call the endpoint.
    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    // --- Happy paths -------------------------------------------------------

    #[tokio::test]
    async fn returns_a_single_endpoint_with_required_fields() {
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let eps = body["applicationEndpoints"].as_array().unwrap();
        assert_eq!(eps.len(), 1);
        assert!(eps[0]["port"].is_number());
        // d=012 → rank 0 → (12 % 3) == 0 → fqdn family.
        assert!(eps[0]["fqdn"].is_string());
        assert_eq!(body["appId"], APP_012);
        assert_eq!(body["applicationServerProviderName"], PROVIDER_NAME);
        // Single device identifier → device is NOT echoed.
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn different_app_tails_select_different_endpoint_counts() {
        let body = format!(
            r#"{{"appId":"{APP_014}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["applicationEndpoints"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn application_endpoints_id_is_echoed_back() {
        let body = format!(
            r#"{{"applicationEndpointsId":"{APP_ENDPOINTS_ID}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["applicationEndpointsId"], APP_ENDPOINTS_ID);
        assert!(body.get("appId").is_none());
        // …012 tail on the endpoints id → 1 endpoint.
        assert_eq!(body["applicationEndpoints"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn multiple_device_identifiers_echo_the_phone_number() {
        // Two identifiers (phone + ipv4) → DeviceResponse echoed with the phone.
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012","ipv4Address":{{"publicAddress":"203.0.113.5"}}}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn multiple_identifiers_without_a_phone_omit_the_device() {
        // Two identifiers but no phoneNumber → DeviceResponse (phone-only) omitted.
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"networkAccessIdentifier":"nai@example.com","ipv4Address":{{"publicAddress":"203.0.113.5"}}}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("device").is_none());
    }

    // --- Not-found / error planes -----------------------------------------

    #[tokio::test]
    async fn app_id_ending_000_is_not_found() {
        let body = format!(
            r#"{{"appId":"{APP_000}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn device_reserved_suffix_selects_a_canonical_error_before_app_lookup() {
        // The device …404 wins even though the appId is a happy-path …012.
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789404"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789429"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        let token = mint_token_with_client(READ_SCOPE, "+123456789012").await;
        let body = format!(r#"{{"appId":"{APP_013}"}}"#);
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["applicationEndpoints"].as_array().unwrap().len(), 2);
        // Three-legged (no request device) → device not echoed.
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(READ_SCOPE, "+123456789503").await;
        let body = format!(r#"{{"appId":"{APP_012}"}}"#);
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_device_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(READ_SCOPE, "+123456789012").await;
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        let body = format!(r#"{{"appId":"{APP_012}"}}"#);
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_application_identifier_is_rejected() {
        let (status, _, body) =
            call_ok(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn both_application_identifiers_are_rejected() {
        let body = format!(
            r#"{{"appId":"{APP_012}","applicationEndpointsId":"{APP_ENDPOINTS_ID}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_uuid_application_identifier_is_rejected() {
        let (status, _, body) =
            call_ok(r#"{"appId":"not-a-uuid","device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let body = format!(r#"{{"appId":"{APP_012}","device":{{}}}}"#);
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let body = format!(r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"0123"}}}}"#);
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012"}},"x":1}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = call_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = post_retrieve(None, &body, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        // Success (two-legged).
        let body = format!(
            r#"{{"appId":"{APP_012}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, headers, _) = post_retrieve(Some(&token), &body, Some("corr-aed")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-aed")
        );
        // Business error (missing identifier).
        let body = format!(r#"{{"appId":"{APP_012}"}}"#);
        let (status, headers, _) = post_retrieve(Some(&token), &body, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
