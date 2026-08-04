//! Connectivity Insights **v0.6** (CAMARA Connectivity Insights 0.6.0, r3.2).
//!
//! One endpoint:
//! - `POST /connectivity-insights/v0.6/check-network-quality` — can the network
//!   meet an application's quality requirements for a device right now?
//!   (operationId `checkNetworkQuality`).
//!
//! ## What it does
//!
//! The caller names an `applicationProfileId` (which application's requirements
//! to check against), a `device` (or relies on a three-legged token), and the
//! `applicationServer` the traffic flows to. The operator answers a set of
//! per-KPI *policy fulfilment* verdicts — for each of `packetDelayBudget`,
//! `targetMinDownstreamRate`, `targetMinUpstreamRate`, `packetlossErrorRate` and
//! `jitter`, either `"meets the application requirements"` or `"unable to meet
//! the application requirements"` — plus coarse `additionalKPIs`
//! (`signalStrength`, `connectivityType`). It never returns raw measurements or
//! the device's location.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `connectivity-insights:check`
//! scope.
//!
//! ## Application profile & server (accepted, lightly validated)
//!
//! `applicationProfileId` is **required** and must be a well-formed UUID, but
//! CamaraSim has no Application Profiles store, so it is not resolved against a
//! stored profile (a documented cut — the profile's thresholds don't shape the
//! answer here; the device identifier does). `applicationServer` is **required**
//! and must carry at least one of `ipv4Address` / `ipv6Address` (CIDR).
//! `applicationServerPorts` (if present) must carry ports / ranges within
//! `0..=65535`. `monitoringTimeStamp` (if present) must look like an RFC 3339
//! instant.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `device` in the body is only needed in two-legged
//! auth; a three-legged token identifies the device by its **subject**. Unlike
//! some device APIs, Connectivity Insights 0.6.0 does **not** define
//! `UNNECESSARY_IDENTIFIER`, so a `device` submitted alongside a line token is
//! simply used (no 422 for resubmission):
//!
//! - `device` carries an identifier → that identifier is used. Precedence:
//!   `phoneNumber` → `networkAccessIdentifier` → the IPv4 `publicAddress` →
//!   `ipv6Address`.
//! - `device` absent, subject is an E.164 line → the subject is the identifier
//!   (three-legged).
//! - `device` absent **and** the subject is not a line → the device cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//! - `device` present but carrying **no** identifier (`minProperties: 1`
//!   violated) → `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Once the identifier is resolved, the answer is deterministic from it:
//!
//! - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!   status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!   `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Network-quality verdicts** — otherwise the identifier's trailing three
//!   digits `d` drive every field. The low five bits of `d` (`d & 0x1F`) are an
//!   *unmet mask*, one bit per KPI in the order `packetDelayBudget`,
//!   `targetMinDownstreamRate`, `targetMinUpstreamRate`, `packetlossErrorRate`,
//!   `jitter`: a set bit → that KPI is `unable to meet…`, a clear bit → `meets…`.
//!   So `…000` (or no digits) → all five KPIs met (a healthy default); a tail
//!   whose low bits are all set → all five unmet.
//!
//! The `additionalKPIs` track how many KPIs are met (`met` = `5 −` the number of
//! set bits): `signalStrength` and `connectivityType` both degrade coherently
//! from `met` (5 → `excellent` / `5G-SA`, down to 0–1 → `no signal` / `3G`), so
//! the whole response is internally consistent and driven by the one input.
//!
//! The response echoes the device back only when the identifier is a
//! `phoneNumber` (the CAMARA `DeviceResponse` carries only `phoneNumber`); for an
//! IP-/NAI-keyed request the `device` field is omitted.
//!
//! Examples: `+123456789000` → all KPIs met, `excellent` / `5G-SA`;
//! `+123456789001` → `packetDelayBudget` unmet (four met), `good` / `5G-NSA`;
//! `+123456789031` → all five KPIs unmet, `no signal` / `3G`;
//! `+123456789404` → `404 NOT_FOUND`.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /check-network-quality` endpoint requires (CAMARA
/// Connectivity Insights 0.6.0).
const CHECK_SCOPE: &str = "connectivity-insights:check";

/// The CAMARA `PolicyFulfilmentConfidence` value for a KPI the network can meet.
const MEETS: &str = "meets the application requirements";
/// The CAMARA `PolicyFulfilmentConfidence` value for a KPI it cannot meet.
const UNABLE: &str = "unable to meet the application requirements";

/// Routes for Connectivity Insights v0.6, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/connectivity-insights/v0.6/check-network-quality",
        post(check_network_quality),
    )
}

/// `POST /check-network-quality` request body (CAMARA
/// `NetworkQualityInsightRequest`). `applicationProfileId` and `applicationServer`
/// are required; `device` is optional (omit it under a three-legged token).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRequest {
    #[serde(rename = "applicationProfileId")]
    application_profile_id: Option<String>,
    device: Option<Device>,
    #[serde(rename = "applicationServer")]
    application_server: Option<ApplicationServer>,
    #[serde(rename = "applicationServerPorts")]
    application_server_ports: Option<PortsSpec>,
    #[serde(rename = "monitoringTimeStamp")]
    monitoring_time_stamp: Option<String>,
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

/// The CAMARA `ApplicationServer` object (`minProperties: 1`): the server the
/// application traffic flows to, by IPv4 and/or IPv6 (CIDR notation).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationServer {
    #[serde(rename = "ipv4Address")]
    ipv4_address: Option<String>,
    #[serde(rename = "ipv6Address")]
    ipv6_address: Option<String>,
}

/// The CAMARA `PortsSpec` object (`minProperties: 1`): individual ports and/or
/// port ranges the application uses.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortsSpec {
    ports: Option<Vec<i64>>,
    ranges: Option<Vec<PortRange>>,
}

/// A `[from, to]` port range within `0..=65535`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortRange {
    from: Option<i64>,
    to: Option<i64>,
}

/// `POST /connectivity-insights/v0.6/check-network-quality`.
async fn check_network_quality(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CHECK_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required (applicationProfileId + applicationServer are required).
    if body.is_empty() {
        return invalid_argument("A NetworkQualityInsightRequest body is required.", &correlator);
    }
    let req: CheckRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid NetworkQualityInsightRequest.",
                &correlator,
            )
        }
    };

    // Required-field / shape validation (400) before identifier semantics.
    match req.application_profile_id.as_deref() {
        None => {
            return invalid_argument("`applicationProfileId` is required.", &correlator);
        }
        Some(id) if !is_uuid(id) => {
            return invalid_argument("`applicationProfileId` must be a UUID.", &correlator);
        }
        Some(_) => {}
    }
    if let Err(resp) = validate_application_server(req.application_server.as_ref(), &correlator) {
        return resp;
    }
    if let Err(resp) = validate_ports(req.application_server_ports.as_ref(), &correlator) {
        return resp;
    }
    if let Some(ts) = &req.monitoring_time_stamp {
        if !looks_like_datetime(ts) {
            return invalid_argument(
                "`monitoringTimeStamp` must be an RFC 3339 date-time.",
                &correlator,
            );
        }
    }

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&resolved.identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let (mut out, met) = network_quality(&resolved.identifier);
    out["additionalKPIs"] = json!({
        "signalStrength": signal_strength(met),
        "connectivityType": connectivity_type(met),
    });
    // The CAMARA DeviceResponse carries only `phoneNumber`, so echo the device
    // only for a phone-number-keyed request.
    if let Some(phone) = resolved.phone_number {
        out["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// The per-KPI policy-fulfilment verdicts for `identifier`, plus the count of
/// KPIs met. The low five bits of the identifier's trailing three digits are an
/// *unmet mask* (bit set → that KPI is `unable to meet…`), one bit per KPI in
/// the fixed order below; no trailing digits → mask 0 → all met.
fn network_quality(identifier: &str) -> (Value, u32) {
    let d = scenarios::trailing_three_digits(identifier).unwrap_or(0);
    let unmet = d & 0x1F; // low five bits: one per KPI.
    let conf = |bit: u16| if unmet & (1u16 << bit) != 0 { UNABLE } else { MEETS };
    let met = 5 - unmet.count_ones();
    let out = json!({
        "packetDelayBudget": conf(0),
        "targetMinDownstreamRate": conf(1),
        "targetMinUpstreamRate": conf(2),
        "packetlossErrorRate": conf(3),
        "jitter": conf(4),
    });
    (out, met)
}

/// Coarse `signalStrength` (CAMARA enum), degrading with the number of KPIs met.
fn signal_strength(met: u32) -> &'static str {
    match met {
        5 => "excellent",
        4 => "good",
        3 => "fair",
        2 => "poor",
        _ => "no signal",
    }
}

/// Coarse `connectivityType` (CAMARA enum), degrading with the number of KPIs met.
fn connectivity_type(met: u32) -> &'static str {
    match met {
        5 => "5G-SA",
        4 => "5G-NSA",
        3 | 2 => "4G",
        _ => "3G",
    }
}

/// A resolved device identifier plus, when it is a phone number, that number
/// (for echoing back in the `DeviceResponse`).
struct Resolved {
    identifier: String,
    phone_number: Option<String>,
}

/// Resolve the device identifier from the request body and the token subject.
/// Connectivity Insights 0.6.0 does not define `UNNECESSARY_IDENTIFIER`, so a
/// submitted `device` is always used (no resubmission error); an absent device
/// with a non-line subject is `422 MISSING_IDENTIFIER`.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    let subject = claims.subject().unwrap_or("");

    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                Ok(Resolved {
                    identifier: phone.clone(),
                    phone_number: Some(phone),
                })
            }
            Some(DeviceId::Other(id)) => Ok(Resolved {
                identifier: id,
                phone_number: None,
            }),
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            if is_valid_e164(subject) {
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

/// Validate the required `applicationServer` (present, `minProperties: 1`).
fn validate_application_server(
    server: Option<&ApplicationServer>,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    match server {
        None => Err(invalid_argument("`applicationServer` is required.", correlator)),
        Some(s) => {
            let has_addr = s.ipv4_address.as_deref().is_some_and(|a| !a.is_empty())
                || s.ipv6_address.as_deref().is_some_and(|a| !a.is_empty());
            if has_addr {
                Ok(())
            } else {
                Err(invalid_argument(
                    "`applicationServer` must contain an `ipv4Address` or `ipv6Address`.",
                    correlator,
                ))
            }
        }
    }
}

/// Validate the optional `applicationServerPorts`: ports / ranges must be
/// non-empty (`minItems: 1`) and lie within `0..=65535` with `from <= to`.
fn validate_ports(
    ports: Option<&PortsSpec>,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    let Some(spec) = ports else { return Ok(()) };
    if spec.ports.is_none() && spec.ranges.is_none() {
        return Err(invalid_argument(
            "`applicationServerPorts` must contain `ports` or `ranges`.",
            correlator,
        ));
    }
    if let Some(list) = &spec.ports {
        if list.is_empty() {
            return Err(invalid_argument("`ports` must not be empty.", correlator));
        }
        for &p in list {
            if !is_port(p) {
                return Err(out_of_range(
                    "A port must be within 0..=65535.",
                    correlator,
                ));
            }
        }
    }
    if let Some(ranges) = &spec.ranges {
        if ranges.is_empty() {
            return Err(invalid_argument("`ranges` must not be empty.", correlator));
        }
        for r in ranges {
            match (r.from, r.to) {
                (Some(from), Some(to)) if is_port(from) && is_port(to) && from <= to => {}
                (Some(_), Some(_)) => {
                    return Err(out_of_range(
                        "A port range must have `from <= to`, both within 0..=65535.",
                        correlator,
                    ));
                }
                _ => {
                    return Err(invalid_argument(
                        "A port range requires both `from` and `to`.",
                        correlator,
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Whether `p` is a valid TCP/UDP port number (`0..=65535`).
fn is_port(p: i64) -> bool {
    (0..=65535).contains(&p)
}

/// Whether `s` is a canonical `8-4-4-4-12` hex UUID string.
fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        let is_dash_pos = matches!(i, 8 | 13 | 18 | 23);
        if is_dash_pos {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

/// A light RFC 3339 sanity check: a date-time carries a `T` separator between a
/// plausible date and time (`YYYY-MM-DDThh:…`). Deep validation is out of scope.
fn looks_like_datetime(s: &str) -> bool {
    s.len() >= 20 && s.as_bytes().get(4) == Some(&b'-') && s.contains('T')
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

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) clamps to 0. (Reserved for future
/// `monitoringTimeStamp` echoing; not on the answer path today.)
#[allow(dead_code)]
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "ci.local:8080";
    const PROFILE: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
    const SERVER: &str = r#""applicationServer":{"ipv4Address":"198.51.100.1/24"}"#;

    /// A minimal valid request body for a submitted phone number.
    fn body_for(phone: &str) -> String {
        format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"phoneNumber":"{phone}"}}}}"#
        )
    }

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn all_kpis_met_for_a_zero_tail() {
        // …000 → mask 0 → every KPI meets, met == 5.
        let (out, met) = network_quality("+123456789000");
        assert_eq!(met, 5);
        for kpi in [
            "packetDelayBudget",
            "targetMinDownstreamRate",
            "targetMinUpstreamRate",
            "packetlossErrorRate",
            "jitter",
        ] {
            assert_eq!(out[kpi], MEETS, "{kpi} should meet");
        }
        // No trailing digits → same healthy default.
        let (_, met) = network_quality("ci-client");
        assert_eq!(met, 5);
    }

    #[test]
    fn unmet_mask_flips_individual_kpis() {
        // …001 → bit 0 set → only packetDelayBudget unmet, four met.
        let (out, met) = network_quality("+123456789001");
        assert_eq!(met, 4);
        assert_eq!(out["packetDelayBudget"], UNABLE);
        assert_eq!(out["targetMinDownstreamRate"], MEETS);
        // …002 → bit 1 set → only targetMinDownstreamRate unmet.
        let (out, met) = network_quality("+123456789002");
        assert_eq!(met, 4);
        assert_eq!(out["targetMinDownstreamRate"], UNABLE);
        assert_eq!(out["packetDelayBudget"], MEETS);
    }

    #[test]
    fn all_kpis_unmet_when_low_five_bits_set() {
        // …031 → mask 31 → all five KPIs unmet, met == 0.
        let (out, met) = network_quality("+123456789031");
        assert_eq!(met, 0);
        for kpi in [
            "packetDelayBudget",
            "targetMinDownstreamRate",
            "targetMinUpstreamRate",
            "packetlossErrorRate",
            "jitter",
        ] {
            assert_eq!(out[kpi], UNABLE, "{kpi} should be unable");
        }
    }

    #[test]
    fn signal_and_connectivity_degrade_with_met() {
        assert_eq!(signal_strength(5), "excellent");
        assert_eq!(signal_strength(0), "no signal");
        assert_eq!(connectivity_type(5), "5G-SA");
        assert_eq!(connectivity_type(4), "5G-NSA");
        assert_eq!(connectivity_type(3), "4G");
        assert_eq!(connectivity_type(0), "3G");
        // Every value is a valid CAMARA enum member.
        for m in 0..=5 {
            assert!(["excellent", "good", "fair", "poor", "no signal"].contains(&signal_strength(m)));
            assert!(["5G-SA", "5G-NSA", "4G", "3G"].contains(&connectivity_type(m)));
        }
    }

    #[test]
    fn uuid_validation() {
        assert!(is_uuid(PROFILE));
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("3fa85f645717-4562-b3fc-2c963f66afa6")); // missing dash
        assert!(!is_uuid("3fa85f64-5717-4562-b3fc-2c963f66afaZ")); // non-hex
        assert!(!is_uuid("")); // empty
    }

    #[test]
    fn port_and_datetime_helpers() {
        assert!(is_port(0) && is_port(65535));
        assert!(!is_port(-1) && !is_port(65536));
        assert!(looks_like_datetime("2024-01-01T14:27:08Z"));
        assert!(!looks_like_datetime("2024/01/01"));
        assert!(!looks_like_datetime("nope"));
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "ci-client").await
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

    async fn post_check(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/connectivity-insights/v0.6/check-network-quality")
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

    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CHECK_SCOPE).await;
        post_check(Some(&token), body, None).await
    }

    // --- Two-legged success cases -----------------------------------------

    #[tokio::test]
    async fn healthy_default_returns_all_kpis_met() {
        let (status, _, body) = call_ok(&body_for("+123456789000")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["packetDelayBudget"], MEETS);
        assert_eq!(body["jitter"], MEETS);
        assert_eq!(body["additionalKPIs"]["signalStrength"], "excellent");
        assert_eq!(body["additionalKPIs"]["connectivityType"], "5G-SA");
        // Phone-keyed → device echoed.
        assert_eq!(body["device"]["phoneNumber"], "+123456789000");
    }

    #[tokio::test]
    async fn tails_flip_kpis_and_degrade_kpis_block() {
        // …001 → packetDelayBudget unmet, four met → good / 5G-NSA.
        let (status, _, body) = call_ok(&body_for("+123456789001")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["packetDelayBudget"], UNABLE);
        assert_eq!(body["additionalKPIs"]["signalStrength"], "good");
        assert_eq!(body["additionalKPIs"]["connectivityType"], "5G-NSA");
        // …031 → all unmet → no signal / 3G.
        let (_, _, body) = call_ok(&body_for("+123456789031")).await;
        assert_eq!(body["jitter"], UNABLE);
        assert_eq!(body["additionalKPIs"]["signalStrength"], "no signal");
        assert_eq!(body["additionalKPIs"]["connectivityType"], "3G");
    }

    #[tokio::test]
    async fn non_phone_identifier_omits_the_device_echo() {
        let token = mint_token(CHECK_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"ipv4Address":{{"publicAddress":"203.0.113.7"}}}}}}"#
        );
        let (status, _, body) = post_check(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("device").is_none());
        assert!(body["packetDelayBudget"].is_string());
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = call_ok(&body_for("+123456789404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(&body_for("+123456789429")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");

        let (status, _, body) = call_ok(&body_for("+123456789422")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …001 → device echoed, four met.
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789001").await;
        let body = format!(r#"{{"applicationProfileId":"{PROFILE}",{SERVER}}}"#);
        let (status, _, body) = post_check(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["packetDelayBudget"], UNABLE);
        assert_eq!(body["device"]["phoneNumber"], "+123456789001");
    }

    #[tokio::test]
    async fn device_on_a_line_token_is_accepted_not_unnecessary() {
        // Connectivity Insights 0.6.0 has no UNNECESSARY_IDENTIFIER: a submitted
        // device on a line token is simply used.
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789000").await;
        let (status, _, body) = post_check(Some(&token), &body_for("+123456789002"), None).await;
        assert_eq!(status, StatusCode::OK);
        // The submitted device (…002) wins over the subject (…000).
        assert_eq!(body["targetMinDownstreamRate"], UNABLE);
        assert_eq!(body["device"]["phoneNumber"], "+123456789002");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        let token = mint_token(CHECK_SCOPE).await;
        let body = format!(r#"{{"applicationProfileId":"{PROFILE}",{SERVER}}}"#);
        let (status, _, body) = post_check(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn empty_body_is_rejected() {
        let token = mint_token(CHECK_SCOPE).await;
        let (status, _, body) = post_check(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_application_profile_id_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{{SERVER},"device":{{"phoneNumber":"+123456789000"}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_application_profile_id_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"nope",{SERVER},"device":{{"phoneNumber":"+123456789000"}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_application_server_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}","device":{{"phoneNumber":"+123456789000"}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn out_of_range_port_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"phoneNumber":"+123456789000"}},"applicationServerPorts":{{"ports":[70000]}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn inverted_port_range_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"phoneNumber":"+123456789000"}},"applicationServerPorts":{{"ranges":[{{"from":100,"to":50}}]}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn valid_ports_are_accepted() {
        let (status, _, _) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"phoneNumber":"+123456789000"}},"applicationServerPorts":{{"ports":[443],"ranges":[{{"from":1000,"to":2000}}]}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn malformed_monitoring_timestamp_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"phoneNumber":"+123456789000"}},"monitoringTimeStamp":"nope"}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{}}}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(&format!(
            r#"{{"applicationProfileId":"{PROFILE}",{SERVER},"device":{{"phoneNumber":"+123456789000"}},"x":1}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_check(Some(&token), &body_for("+123456789000"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_check(None, &body_for("+123456789000"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CHECK_SCOPE).await;
        let (status, headers, _) =
            post_check(Some(&token), &body_for("+123456789000"), Some("corr-ci")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ci")
        );
        // Business error still echoes.
        let (status, headers, _) =
            post_check(Some(&token), &body_for("+123456789404"), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
