//! Simple Edge Discovery **v2** (CAMARA Simple Edge Discovery 2.0.1, r2.3).
//!
//! One endpoint:
//! - `POST /simple-edge-discovery/v2/retrieve-closest-edge-cloud-zone` — return
//!   the edge cloud zone closest (lowest network latency) to a device
//!   (operationId `readClosestEdgeCloudZone`).
//!
//! ## What it does
//!
//! The caller asks which edge cloud zone (MEC region) is nearest a device and
//! the operator answers with a single `EdgeCloudZone`
//! (`{ edgeCloudZoneId, edgeCloudZoneName, edgeCloudProvider }`) — never the
//! device's coordinates. An application uses the answer to deploy its edge
//! workload in the zone with the best latency to that device.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `simple-edge-discovery:read`
//! scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `device` in the body is only meaningful in
//! two-legged auth. In a three-legged token the device is already identified by
//! the token **subject**, so resubmitting it is an error:
//!
//! - `device` carries an identifier **and** the token subject is itself an
//!   E.164 line (a device-authenticated three-legged token) →
//!   `422 UNNECESSARY_IDENTIFIER`.
//! - `device` carries an identifier, subject not a line → that identifier is
//!   used (two-legged). Precedence: `phoneNumber` → `networkAccessIdentifier` →
//!   the IPv4 `publicAddress` → `ipv6Address`.
//! - `device` absent, subject is an E.164 line → the subject is the identifier
//!   (three-legged).
//! - `device` absent **and** the subject is not a line → the device cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//! - `device` present but carrying **no** identifier (`minProperties: 1`
//!   violated) → `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Once the identifier is resolved, the returned zone is deterministic from it:
//!
//! - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!   status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!   `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Closest zone** — otherwise the identifier's trailing three digits index a
//!   fixed [`EDGE_ZONES`] table (`digits % 6`; `…000` / no digits → the first
//!   entry), so the reported zone is a genuine control plane (the same input
//!   always returns the same zone; different tails return different zones). The
//!   `edgeCloudZoneId` is stable **per zone** (a zone has one id regardless of
//!   which device resolves to it).
//!
//! The response echoes the device back only when the identifier is a
//! `phoneNumber` (the CAMARA `DeviceResponse` carries only `phoneNumber`); for an
//! IP-/NAI-keyed request the `device` field is omitted.
//!
//! Examples: `+123456789012` → zone index `12 % 6 == 0`; `+123456789013` → zone
//! index `13 % 6 == 1`; `+123456789404` → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the endpoint requires (CAMARA Simple Edge Discovery 2.0.1).
const READ_SCOPE: &str = "simple-edge-discovery:read";

/// The operator's fixed edge cloud zones. The identifier's trailing three digits
/// pick one (`digits % EDGE_ZONES.len()`), so the returned zone is deterministic
/// from the device — a genuine second control plane (docs/DESIGN.md §7). Each
/// entry is `(edgeCloudZoneName, edgeCloudProvider)`; the `edgeCloudZoneId` is
/// derived from the name (stable per zone — see [`zone_id`]).
const EDGE_ZONES: [(&str, &str); 6] = [
    ("camarasim-edge-eu-west-1", "CamaraSim Edge"),
    ("camarasim-edge-eu-central-1", "CamaraSim Edge"),
    ("camarasim-edge-us-east-1", "CamaraSim Edge"),
    ("camarasim-edge-us-west-2", "CamaraSim Edge"),
    ("camarasim-edge-ap-south-1", "CamaraSim Edge"),
    ("camarasim-edge-ap-northeast-1", "CamaraSim Edge"),
];

/// Routes for Simple Edge Discovery v2, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/simple-edge-discovery/v2/retrieve-closest-edge-cloud-zone",
        post(retrieve_closest_edge_cloud_zone),
    )
}

/// `POST /retrieve-closest-edge-cloud-zone` request body
/// (CAMARA `RetrieveClosestEdgeCloudZoneRequest`). `device` is optional — omit it
/// when a three-legged token identifies the device.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    device: Option<Device>,
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

/// `POST /simple-edge-discovery/v2/retrieve-closest-edge-cloud-zone`.
async fn retrieve_closest_edge_cloud_zone(
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

    // An empty body is allowed (device optional); anything present must parse.
    let req: RetrieveRequest = if body.is_empty() {
        RetrieveRequest { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrieveClosestEdgeCloudZoneRequest.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&resolved.identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let (zone_name, provider) = closest_zone(&resolved.identifier);
    let mut zone = json!({
        "edgeCloudZoneId": zone_id(zone_name),
        "edgeCloudZoneName": zone_name,
        "edgeCloudProvider": provider,
    });
    // The CAMARA DeviceResponse carries only `phoneNumber`, so echo the device
    // only for a phone-number-keyed request.
    if let Some(phone) = resolved.phone_number {
        zone["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(zone)).into_response(), &correlator)
}

/// The edge cloud zone closest to `identifier`, chosen from the fixed
/// [`EDGE_ZONES`] table by the identifier's trailing three digits
/// (`digits % EDGE_ZONES.len()`; no digits → the first entry). Returns
/// `(edgeCloudZoneName, edgeCloudProvider)`.
fn closest_zone(identifier: &str) -> (&'static str, &'static str) {
    let idx = scenarios::trailing_three_digits(identifier).unwrap_or(0) as usize
        % EDGE_ZONES.len();
    EDGE_ZONES[idx]
}

/// A stable, UUID-shaped `edgeCloudZoneId` for a zone, derived from its name via
/// SHA-256 (deterministic, no new dependency). Depends only on the zone, so a
/// zone always reports the same id regardless of which device resolves to it.
/// UUID-*shaped* (not a real v4/v5 UUID) — a documented cut mirroring the
/// Blockchain Public Address ids.
fn zone_id(zone_name: &str) -> String {
    let h = Sha256::digest(format!("sed-zone:{zone_name}").as_bytes());
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
/// enforcing the CAMARA two-legged / three-legged identifier rules.
///
/// See the module docs for the cases (`UNNECESSARY_IDENTIFIER`, two-legged,
/// three-legged, `MISSING_IDENTIFIER`, and the empty-`device` `INVALID_ARGUMENT`).
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "sed.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn closest_zone_is_indexed_by_the_trailing_digits() {
        // …012 → 12 % 6 == 0 → first entry.
        assert_eq!(closest_zone("+123456789012"), EDGE_ZONES[0]);
        // …013 → 13 % 6 == 1.
        assert_eq!(closest_zone("+123456789013"), EDGE_ZONES[1]);
        // …005 → 5 % 6 == 5 → last entry.
        assert_eq!(closest_zone("+123456789005"), EDGE_ZONES[5]);
        // …000 → 0 → first entry.
        assert_eq!(closest_zone("+123456789000"), EDGE_ZONES[0]);
        // No trailing digits → first entry (default).
        assert_eq!(closest_zone("sed-client"), EDGE_ZONES[0]);
    }

    #[test]
    fn zone_id_is_deterministic_uuid_shaped_and_stable_per_zone() {
        let a = zone_id(EDGE_ZONES[0].0);
        let b = zone_id(EDGE_ZONES[0].0);
        assert_eq!(a, b, "same zone → same id");
        assert_ne!(zone_id(EDGE_ZONES[0].0), zone_id(EDGE_ZONES[1].0));
        // 8-4-4-4-12 hex groups.
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
        assert!(!is_valid_e164("+1234567890123456")); // too long
        assert!(!is_valid_e164("sed-client"));
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
        mint_token_with_client(scope, "sed-client").await
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
            .uri("/simple-edge-discovery/v2/retrieve-closest-edge-cloud-zone")
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
    async fn returns_the_closest_zone_with_all_required_fields() {
        // …012 → zone index 0.
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["edgeCloudZoneName"], EDGE_ZONES[0].0);
        assert_eq!(body["edgeCloudProvider"], EDGE_ZONES[0].1);
        assert_eq!(body["edgeCloudZoneId"], zone_id(EDGE_ZONES[0].0));
        // Phone-keyed → device echoed.
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn different_tails_select_different_zones() {
        // …013 → zone index 1.
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789013"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["edgeCloudZoneName"], EDGE_ZONES[1].0);
        assert_ne!(body["edgeCloudZoneId"], zone_id(EDGE_ZONES[0].0));
    }

    #[tokio::test]
    async fn non_phone_identifier_omits_the_device_echo() {
        // ipv4 publicAddress ending …005 → zone index 5, no device echo
        // (DeviceResponse carries only phoneNumber).
        let (status, _, body) =
            call_ok(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.005"}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["edgeCloudZoneName"], EDGE_ZONES[5].0);
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789404"}}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789429"}}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …013 → zone index 1, device echoed.
        let token = mint_token_with_client(READ_SCOPE, "+123456789013").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["edgeCloudZoneName"], EDGE_ZONES[1].0);
        assert_eq!(body["device"]["phoneNumber"], "+123456789013");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(READ_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_device_on_a_line_token_is_unnecessary() {
        // Subject is a line (three-legged) AND a device is submitted → 422.
        let token = mint_token_with_client(READ_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"device":{"phoneNumber":"+123456789012"}}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        // Two-legged token (sub = sed-client) with no device → can't identify.
        let (status, _, body) = call_ok("{}").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");

        // Also for a wholly empty body.
        let token = mint_token(READ_SCOPE).await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = call_ok(r#"{"device":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"0123"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            call_ok(r#"{"device":{"phoneNumber":"+123456789012"},"x":1}"#).await;
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
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"device":{"phoneNumber":"+123456789012"}}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_retrieve(None, r#"{"device":{"phoneNumber":"+123456789012"}}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"}}"#,
            Some("corr-sed"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-sed")
        );
        // Business error.
        let (status, headers, _) = post_retrieve(Some(&token), "{}", Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
