//! Optimal Edge Discovery **vwip** (CAMARA Optimal Edge Discovery, work-in-progress).
//!
//! Two endpoints:
//! - `POST /optimal-edge-discovery/vwip/retrieve-optimal-edge-cloud-zones` —
//!   return a **ranked** list of the edge cloud zones optimal for a device
//!   (operationId `discoverOptimalEdge`).
//! - `GET /optimal-edge-discovery/vwip/regions` — the read-only helper that lists
//!   the edge cloud **regions** where zones are available (operationId
//!   `getRegions`). No request body / identifier: a static catalog of the distinct
//!   regions of the fixed [`EDGE_ZONES`] table, protected by the
//!   `optimal-edge-discovery:regions:read` scope. `x-correlator` echoed.
//!
//! ## What it does
//!
//! Where Simple Edge Discovery answers with the single **closest** zone, Optimal
//! Edge Discovery answers with a short **ranked list** of the zones best suited to
//! run an application's edge workload for a device — never the device's
//! coordinates. The caller identifies an `applicationProfileId` (the workload's
//! requirements) and a device (either a `device` object in the body — two-legged /
//! CIBA — or the identity a three-legged access token authenticated), and may
//! narrow the search to a single `edgeCloudRegion`.
//!
//! The response is an `EdgeDiscoveryResponse`
//! (`{ edgeCloudZones: [EdgeCloudZone], applicationProfileId?, device? }`); the
//! top zone is the recommended (`active`) one, the rest are ranked alternatives.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `optimal-edge-discovery:edge-zones:read` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA (mirrors Simple Edge Discovery): the `device` in the body is
//! only meaningful in two-legged auth. In a three-legged token the device is
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
//! Three planes drive the answer:
//!
//! 1. **`applicationProfileId`** — required; a missing or non-UUID value →
//!    `400 INVALID_ARGUMENT` (a structural plane, checked before the identifier).
//! 2. **`edgeCloudRegion`** — optional; when present it filters the candidate
//!    zones to that region. A malformed region (violating the pattern) →
//!    `400 INVALID_ARGUMENT`; a well-formed but unknown region (no zones there) →
//!    `404 NOT_FOUND`. A genuine second plane over the ranking.
//! 3. **Identifier** — once resolved: a **reserved error suffix** (trailing three
//!    digits naming a reserved CAMARA status — `…400`, `…401`, `…403`, `…404`,
//!    `…409`, `…422`, `…429`, `…500`, `…503`) → that canonical CAMARA error
//!    (shared [`crate::scenarios`], checked first among the business planes).
//!    Otherwise the trailing three digits `d` fix the ranking over the candidate
//!    zones: the optimal (top) zone is index `d % len`, and the list length is
//!    `(d % 3) + 1` (1–3 zones), wrapping the fixed [`EDGE_ZONES`] table. `…000` /
//!    no digits → a single zone at index 0. So the same input always returns the
//!    same ranked list.
//!
//! The response echoes the device back only when the identifier is a
//! `phoneNumber` (the CAMARA `DeviceResponse` carries only `phoneNumber`); for an
//! IP-/NAI-keyed request the `device` field is omitted.
//!
//! Examples: `+123456789000` → 1 zone `[eu-west-1]` (`active`); `+123456789013` →
//! 2 zones starting at index `13 % 6 == 1` (`[eu-central-1(active), us-east-1]`);
//! `+123456789404` → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `discoverOptimalEdge` requires (CAMARA Optimal Edge Discovery).
const READ_SCOPE: &str = "optimal-edge-discovery:edge-zones:read";

/// The OAuth2 scope the `getRegions` helper requires.
const REGIONS_SCOPE: &str = "optimal-edge-discovery:regions:read";

/// The operator's fixed edge cloud zones, each `(edgeCloudZoneName,
/// edgeCloudProvider, edgeCloudRegion)`. The identifier's trailing three digits
/// pick the optimal (top) zone and the list length over this table, so the ranked
/// answer is deterministic from the device (docs/DESIGN.md §7). The `edgeCloudRegion`
/// on each entry is what the optional request `edgeCloudRegion` filters against.
/// The `edgeCloudZoneId` is derived from the name (stable per zone — see [`zone_id`]).
const EDGE_ZONES: [(&str, &str, &str); 6] = [
    ("camarasim-edge-eu-west-1", "CamaraSim Edge", "eu-west-1"),
    ("camarasim-edge-eu-central-1", "CamaraSim Edge", "eu-central-1"),
    ("camarasim-edge-us-east-1", "CamaraSim Edge", "us-east-1"),
    ("camarasim-edge-us-west-2", "CamaraSim Edge", "us-west-2"),
    ("camarasim-edge-ap-south-1", "CamaraSim Edge", "ap-south-1"),
    ("camarasim-edge-ap-northeast-1", "CamaraSim Edge", "ap-northeast-1"),
];

/// Routes for Optimal Edge Discovery vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/optimal-edge-discovery/vwip/retrieve-optimal-edge-cloud-zones",
            post(retrieve_optimal_edge_cloud_zones),
        )
        .route("/optimal-edge-discovery/vwip/regions", get(get_regions))
}

/// Request body (CAMARA `OptimalEdgeDiscoveryInfo`). `applicationProfileId` is
/// required; `device` and `edgeCloudRegion` are optional.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptimalEdgeDiscoveryInfo {
    device: Option<Device>,
    #[serde(rename = "applicationProfileId")]
    application_profile_id: Option<String>,
    #[serde(rename = "edgeCloudRegion")]
    edge_cloud_region: Option<String>,
}

/// The CAMARA `Device` object: at least one identifier must be present
/// (`minProperties: 1`). CamaraSim keys its functional cases off the first
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

/// `POST /optimal-edge-discovery/vwip/retrieve-optimal-edge-cloud-zones`.
async fn retrieve_optimal_edge_cloud_zones(
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

    // The body is required — `applicationProfileId` is mandatory, so an empty or
    // malformed body cannot satisfy the schema.
    let req: OptimalEdgeDiscoveryInfo = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid OptimalEdgeDiscoveryInfo.",
                &correlator,
            )
        }
    };

    // Structural plane: `applicationProfileId` is required and must be a UUID.
    let application_profile_id = match req.application_profile_id.as_deref() {
        Some(id) if is_uuid_shaped(id) => id.to_string(),
        Some(_) => {
            return invalid_argument(
                "`applicationProfileId` must be a UUID.",
                &correlator,
            )
        }
        None => {
            return invalid_argument(
                "`applicationProfileId` is required.",
                &correlator,
            )
        }
    };

    // Structural plane: a supplied `edgeCloudRegion` must match the pattern.
    if let Some(region) = req.edge_cloud_region.as_deref() {
        if !is_valid_region(region) {
            return invalid_argument(
                "`edgeCloudRegion` must match `^[A-Za-z0-9-]+$` (max 64 chars).",
                &correlator,
            );
        }
    }

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7); reserved-error first.
    if let Some(err) = scenarios::reserved_error(&resolved.identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Rank the candidate zones from the identifier, filtered by `edgeCloudRegion`.
    // A well-formed but unknown region matches no zone → 404 NOT_FOUND.
    let zones = match optimal_zones(&resolved.identifier, req.edge_cloud_region.as_deref()) {
        Some(zones) => zones,
        None => {
            return with_correlator(
                CamaraError::for_status(404)
                    .expect("404 is a canonical CAMARA status")
                    .into_response(),
                &correlator,
            )
        }
    };

    let edge_cloud_zones: Vec<_> = zones
        .iter()
        .map(|z| {
            json!({
                "edgeCloudZoneId": zone_id(z.name),
                "edgeCloudZoneName": z.name,
                "edgeCloudProvider": z.provider,
                "edgeCloudRegion": z.region,
                "edgeCloudZoneStatus": z.status,
            })
        })
        .collect();

    let mut out = json!({
        "edgeCloudZones": edge_cloud_zones,
        "applicationProfileId": application_profile_id,
    });
    // The CAMARA DeviceResponse carries only `phoneNumber`, so echo the device
    // only for a phone-number-keyed request.
    if let Some(phone) = resolved.phone_number {
        out["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// `GET /optimal-edge-discovery/vwip/regions` (`getRegions`).
///
/// A read-only helper that lists the edge cloud **regions** where zones are
/// available. There is no request body and no device identifier — it is a static
/// catalog, so it has no functional/control planes beyond the auth error set. The
/// regions are the **distinct** `edgeCloudRegion` values of the fixed
/// [`EDGE_ZONES`] table, in table order (the canonical schema caps the list at 20;
/// the table holds 6). Protected by the `optimal-edge-discovery:regions:read` scope.
async fn get_regions(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the regions scope.
    if let Err(e) = claims.require_scope(REGIONS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let regions = distinct_regions();
    with_correlator(
        (StatusCode::OK, Json(json!(regions))).into_response(),
        &correlator,
    )
}

/// The distinct `edgeCloudRegion` values of the fixed [`EDGE_ZONES`] table, in
/// table order (first occurrence wins). The `getRegions` catalog.
fn distinct_regions() -> Vec<&'static str> {
    let mut regions: Vec<&'static str> = Vec::new();
    for (_, _, region) in EDGE_ZONES {
        if !regions.contains(&region) {
            regions.push(region);
        }
    }
    regions
}

/// A ranked candidate edge cloud zone: the fixed-table fields plus the
/// `edgeCloudZoneStatus` assigned by the ranking (`active` for the optimal top
/// zone, `inactive` for the alternatives).
struct RankedZone {
    name: &'static str,
    provider: &'static str,
    region: &'static str,
    status: &'static str,
}

/// The ranked list of edge cloud zones for `identifier`, optionally narrowed to
/// `region`. Returns `None` when `region` is `Some` but names no zone in the fixed
/// [`EDGE_ZONES`] table (→ the caller answers `404 NOT_FOUND`).
///
/// The candidate set is the whole table (or the region's subset). The identifier's
/// trailing three digits `d` fix the ranking: the optimal (top) zone is index
/// `d % len`, and the list length is `(d % 3) + 1` (1–3 zones), wrapping the
/// candidates. The top zone's status is `active`, the rest `inactive`.
fn optimal_zones(identifier: &str, region: Option<&str>) -> Option<Vec<RankedZone>> {
    // Candidate indices into EDGE_ZONES, filtered by region when one was supplied.
    let candidates: Vec<usize> = (0..EDGE_ZONES.len())
        .filter(|&i| region.is_none_or(|r| EDGE_ZONES[i].2 == r))
        .collect();
    if candidates.is_empty() {
        return None;
    }

    let d = scenarios::trailing_three_digits(identifier).unwrap_or(0) as usize;
    let start = d % candidates.len();
    let count = (d % 3) + 1; // 1..=3
    let count = count.min(candidates.len());

    let zones = (0..count)
        .map(|rank| {
            let (name, provider, region) = EDGE_ZONES[candidates[(start + rank) % candidates.len()]];
            RankedZone {
                name,
                provider,
                region,
                status: if rank == 0 { "active" } else { "inactive" },
            }
        })
        .collect();
    Some(zones)
}

/// A stable, UUID-shaped `edgeCloudZoneId` for a zone, derived from its name via
/// SHA-256 (deterministic, no new dependency). Depends only on the zone, so a zone
/// always reports the same id regardless of which device resolves to it.
/// UUID-*shaped* (not a real v4/v5 UUID) — a documented cut mirroring Simple Edge
/// Discovery / the Blockchain Public Address ids.
fn zone_id(zone_name: &str) -> String {
    let h = Sha256::digest(format!("oed-zone:{zone_name}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// A resolved device identifier plus, when it is a phone number, that number
/// (for echoing back in the `DeviceResponse`).
struct Resolved {
    identifier: String,
    phone_number: Option<String>,
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors Simple
/// Edge Discovery). See the module docs for the cases.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                // The device is already identified by a three-legged token; the
                // identifier must not be resubmitted.
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                Ok(Resolved {
                    identifier: phone.clone(),
                    phone_number: Some(phone),
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
                    phone_number: None,
                })
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            if subject_is_line {
                Ok(Resolved {
                    identifier: subject.to_string(),
                    phone_number: Some(subject.to_string()),
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

/// Whether `s` is a canonical UUID (the `applicationProfileId` format): 36 chars,
/// hyphens at positions 8/13/18/23, hex elsewhere.
fn is_uuid_shaped(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    s.bytes().enumerate().all(|(i, b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// Whether `s` matches the CAMARA `edgeCloudRegion` pattern `^[A-Za-z0-9-]+$`
/// (non-empty, max 64 chars).
fn is_valid_region(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "oed.local:8080";
    const PATH: &str = "/optimal-edge-discovery/vwip/retrieve-optimal-edge-cloud-zones";
    // A valid UUID `applicationProfileId` used across the happy-path tests.
    const APP_PROFILE: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn ranking_is_derived_from_the_trailing_digits() {
        // …000 / no digits → a single zone at index 0, active.
        let z = optimal_zones("+123456789000", None).unwrap();
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].name, EDGE_ZONES[0].0);
        assert_eq!(z[0].status, "active");

        let z = optimal_zones("oed-client", None).unwrap();
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].name, EDGE_ZONES[0].0);

        // …013 → start 13 % 6 == 1, count (13 % 3) + 1 == 2.
        let z = optimal_zones("+123456789013", None).unwrap();
        assert_eq!(z.len(), 2);
        assert_eq!(z[0].name, EDGE_ZONES[1].0);
        assert_eq!(z[0].status, "active");
        assert_eq!(z[1].name, EDGE_ZONES[2].0);
        assert_eq!(z[1].status, "inactive");

        // …014 → start 14 % 6 == 2, count (14 % 3) + 1 == 3, wrapping.
        let z = optimal_zones("+123456789014", None).unwrap();
        assert_eq!(z.len(), 3);
        assert_eq!(z[0].name, EDGE_ZONES[2].0);
        assert_eq!(z[2].name, EDGE_ZONES[4].0);
    }

    #[test]
    fn ranking_length_never_exceeds_the_schema_maximum() {
        // 1..=3 zones per the count rule — comfortably inside the schema's 1..=20.
        for tail in [0u16, 1, 2, 250, 500, 999] {
            let id = format!("+12345678{tail:03}");
            let z = optimal_zones(&id, None).unwrap();
            assert!((1..=20).contains(&z.len()), "{} out of range for {id}", z.len());
            assert_eq!(z[0].status, "active");
        }
    }

    #[test]
    fn region_filters_the_candidate_zones() {
        // A known region narrows to that region's single zone.
        let z = optimal_zones("+123456789014", Some("us-east-1")).unwrap();
        assert!(z.iter().all(|z| z.region == "us-east-1"));
        assert_eq!(z.len(), 1); // only one zone in the region → count capped
        assert_eq!(z[0].name, EDGE_ZONES[2].0);

        // An unknown region matches no zone → None (→ 404).
        assert!(optimal_zones("+123456789012", Some("no-such-region")).is_none());
    }

    #[test]
    fn zone_id_is_deterministic_uuid_shaped_and_stable_per_zone() {
        let a = zone_id(EDGE_ZONES[0].0);
        let b = zone_id(EDGE_ZONES[0].0);
        assert_eq!(a, b, "same zone → same id");
        assert_ne!(zone_id(EDGE_ZONES[0].0), zone_id(EDGE_ZONES[1].0));
        let groups: Vec<&str> = a.split('-').collect();
        assert_eq!(groups.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn uuid_and_region_validation() {
        assert!(is_uuid_shaped(APP_PROFILE));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("3fa85f645717-4562-b3fc-2c963f66afa6")); // missing hyphen
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afag")); // non-hex

        assert!(is_valid_region("us-east-1"));
        assert!(!is_valid_region("")); // empty
        assert!(!is_valid_region("us east")); // space
        assert!(!is_valid_region(&"x".repeat(65))); // too long
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("+1234567890123456")); // too long
        assert!(!is_valid_e164("oed-client"));
    }

    #[test]
    fn device_identifier_follows_precedence() {
        let phone = Device {
            phone_number: Some("+123456789012".into()),
            network_access_identifier: Some("nai@example.com".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&phone), Some(DeviceId::PhoneNumber(p)) if p == "+123456789012"));

        let ipv6 = Device {
            ipv6_address: Some("2001:db8::11".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&ipv6), Some(DeviceId::Other(p)) if p == "2001:db8::11"));

        assert!(device_identifier(&Device::default()).is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "oed-client").await
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

    // --- Two-legged (submitted device) success cases ----------------------

    #[tokio::test]
    async fn returns_the_ranked_zones_with_required_fields() {
        // …000 → single optimal zone at index 0, phone-keyed → device echoed.
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789000"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let zones = body["edgeCloudZones"].as_array().unwrap();
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0]["edgeCloudZoneName"], EDGE_ZONES[0].0);
        assert_eq!(zones[0]["edgeCloudProvider"], EDGE_ZONES[0].1);
        assert_eq!(zones[0]["edgeCloudRegion"], EDGE_ZONES[0].2);
        assert_eq!(zones[0]["edgeCloudZoneId"], zone_id(EDGE_ZONES[0].0));
        assert_eq!(zones[0]["edgeCloudZoneStatus"], "active");
        assert_eq!(body["applicationProfileId"], APP_PROFILE);
        assert_eq!(body["device"]["phoneNumber"], "+123456789000");
    }

    #[tokio::test]
    async fn different_tails_select_different_rankings() {
        // …013 → 2 zones starting at index 1.
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789013"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let zones = body["edgeCloudZones"].as_array().unwrap();
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[0]["edgeCloudZoneName"], EDGE_ZONES[1].0);
        assert_eq!(zones[0]["edgeCloudZoneStatus"], "active");
        assert_eq!(zones[1]["edgeCloudZoneName"], EDGE_ZONES[2].0);
        assert_eq!(zones[1]["edgeCloudZoneStatus"], "inactive");
    }

    #[tokio::test]
    async fn region_filter_narrows_the_result() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","edgeCloudRegion":"us-west-2","device":{{"phoneNumber":"+123456789014"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let zones = body["edgeCloudZones"].as_array().unwrap();
        assert!(zones.iter().all(|z| z["edgeCloudRegion"] == "us-west-2"));
        assert_eq!(zones[0]["edgeCloudZoneName"], EDGE_ZONES[3].0);
    }

    #[tokio::test]
    async fn unknown_region_is_not_found() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","edgeCloudRegion":"zz-nowhere-9","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn non_phone_identifier_omits_the_device_echo() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"ipv4Address":{{"publicAddress":"203.0.113.005"}}}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let zones = body["edgeCloudZones"].as_array().unwrap();
        // …005 → start 5, count (5 % 3) + 1 == 3.
        assert_eq!(zones[0]["edgeCloudZoneName"], EDGE_ZONES[5].0);
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789404"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789429"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …013 → start 1, 2 zones, device echoed.
        let token = mint_token_with_client(READ_SCOPE, "+123456789013").await;
        let body = format!(r#"{{"applicationProfileId":"{APP_PROFILE}"}}"#);
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        let zones = body["edgeCloudZones"].as_array().unwrap();
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[0]["edgeCloudZoneName"], EDGE_ZONES[1].0);
        assert_eq!(body["device"]["phoneNumber"], "+123456789013");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(READ_SCOPE, "+123456789503").await;
        let body = format!(r#"{{"applicationProfileId":"{APP_PROFILE}"}}"#);
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_device_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(READ_SCOPE, "+123456789012").await;
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        // Two-legged token (sub = oed-client) with no device → can't identify.
        let body = format!(r#"{{"applicationProfileId":"{APP_PROFILE}"}}"#);
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_application_profile_id_is_rejected() {
        let (status, _, body) =
            call_ok(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_application_profile_id_is_rejected() {
        let (status, _, body) =
            call_ok(r#"{"applicationProfileId":"not-a-uuid","device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_region_is_rejected() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","edgeCloudRegion":"bad region!","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let body = format!(r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{}}}}"#);
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"0123"}}}}"#
        );
        let (status, _, body) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789012"}},"x":1}}"#
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
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, _, body) = post_retrieve(None, &body, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- getRegions (GET /regions) ----------------------------------------

    const REGIONS_PATH: &str = "/optimal-edge-discovery/vwip/regions";

    /// GET the regions helper with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_regions_req(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(REGIONS_PATH)
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

    #[test]
    fn distinct_regions_are_the_tables_regions_in_order() {
        let regions = distinct_regions();
        // The fixed table's six regions are all distinct, so all six are listed.
        assert_eq!(
            regions,
            vec![
                "eu-west-1",
                "eu-central-1",
                "us-east-1",
                "us-west-2",
                "ap-south-1",
                "ap-northeast-1",
            ]
        );
        // Comfortably inside the schema's maxItems: 20.
        assert!(regions.len() <= 20);
    }

    #[tokio::test]
    async fn get_regions_returns_the_region_catalog() {
        let token = mint_token(REGIONS_SCOPE).await;
        let (status, _, body) = get_regions_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let regions = body.as_array().unwrap();
        assert_eq!(regions.len(), distinct_regions().len());
        assert_eq!(regions[0], "eu-west-1");
        // Every region matches the canonical `^[A-Za-z0-9-]+$` pattern.
        assert!(regions
            .iter()
            .all(|r| is_valid_region(r.as_str().unwrap())));
    }

    #[tokio::test]
    async fn get_regions_requires_its_own_scope() {
        // The zone-discovery scope is not the regions scope → 403.
        let token = mint_token(READ_SCOPE).await;
        let (status, _, body) = get_regions_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn get_regions_without_a_token_is_unauthenticated() {
        let (status, _, body) = get_regions_req(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn get_regions_echoes_the_correlator() {
        let token = mint_token(REGIONS_SCOPE).await;
        let (status, headers, _) = get_regions_req(Some(&token), Some("corr-regions")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-regions")
        );
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        // Success (two-legged).
        let body = format!(
            r#"{{"applicationProfileId":"{APP_PROFILE}","device":{{"phoneNumber":"+123456789012"}}}}"#
        );
        let (status, headers, _) = post_retrieve(Some(&token), &body, Some("corr-oed")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-oed")
        );
        // Business error.
        let body = format!(r#"{{"applicationProfileId":"{APP_PROFILE}"}}"#);
        let (status, headers, _) = post_retrieve(Some(&token), &body, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
