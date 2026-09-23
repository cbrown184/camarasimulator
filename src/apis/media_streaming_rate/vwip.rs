//! Media Streaming Rate **vwip** (CAMARA Device Media Streaming Rate, work-in-progress).
//!
//! One endpoint:
//! - `POST /media-streaming-rate/vwip/retrieve-maximum-downstream-media-rate` —
//!   what is the maximum downstream media bit rate the network can currently
//!   sustain for the device? (operationId `retrieveMaximumDownstreamMediaRate`).
//!
//! ## What it does
//!
//! The caller asks about a device, identified either by a `device` object in the
//! request body (two-legged / CIBA) or by the identity a three-legged access
//! token authenticated (in which case `device` is omitted). The operator answers
//! `{ "maxDownstreamMediaBitRateSupported": …, "unit": …, "device"?: … }` — a
//! magnitude (`0..=1024`) plus a unit (`bps`/`kbps`/`Mbps`/`Gbps`/`Tbps`), so an
//! application can size its media streaming; never raw per-flow telemetry.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `media-streaming-rate:retrieve-maximum-downstream-media-rate` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA (mirrors Device Data Volume): the `device` in the body is
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
//! Once the identifier is resolved, the answer is deterministic from it:
//!
//! - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!   status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!   `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Rate + unit** — otherwise the identifier's trailing three digits `d`
//!   (`0..=999`; no digits → 0) are the reported magnitude
//!   `maxDownstreamMediaBitRateSupported` (always within the schema's `0..=1024`),
//!   and `d % 5` selects the [`UNITS`] entry. Both axes are lowest-first, so a
//!   `…000` / no-digit identifier is the floor (`0 bps`, an unmeasured device) and
//!   higher tails climb both magnitude and unit — a genuine control plane (the
//!   same input always returns the same rate and unit).
//!
//! The response echoes the device back only when the identifier is a
//! `phoneNumber` (the CAMARA `DeviceResponse` carries only `phoneNumber`); for an
//! IP-/NAI-keyed request the `device` field is omitted.
//!
//! Examples: `+123456789000` → `0 bps` (floor); `+123456789002` → `2 Mbps`
//! (`2 % 5 == 2`); `+123456789013` → `13 Gbps` (`13 % 5 == 3`); `+123456789404`
//! → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the endpoint requires (CAMARA Device Media Streaming Rate).
const RETRIEVE_SCOPE: &str = "media-streaming-rate:retrieve-maximum-downstream-media-rate";

/// The CAMARA bit-rate units, lowest-first (`unit` enum). The identifier's
/// trailing three digits pick one (`digits % UNITS.len()`), so the reported unit
/// is deterministic from the device — part of the control plane (docs/DESIGN.md
/// §7). Index 0 (`…000` / no digits) is the floor, `bps`.
const UNITS: [&str; 5] = ["bps", "kbps", "Mbps", "Gbps", "Tbps"];

/// Routes for Media Streaming Rate vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/media-streaming-rate/vwip/retrieve-maximum-downstream-media-rate",
        post(retrieve_maximum_downstream_media_rate),
    )
}

/// Request body (CAMARA `MediaStreamingRateRequest`). `device` is optional — omit
/// it when a three-legged token identifies the device.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaStreamingRateRequest {
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

/// `POST /media-streaming-rate/vwip/retrieve-maximum-downstream-media-rate`.
async fn retrieve_maximum_downstream_media_rate(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (device optional); anything present must parse.
    let req: MediaStreamingRateRequest = if body.is_empty() {
        MediaStreamingRateRequest { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid MediaStreamingRateRequest.",
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

    let (rate, unit) = max_downstream_rate(&resolved.identifier);
    let mut out = json!({
        "maxDownstreamMediaBitRateSupported": rate,
        "unit": unit,
    });
    // The CAMARA DeviceResponse carries only `phoneNumber`, so echo the device
    // only for a phone-number-keyed request.
    if let Some(phone) = resolved.phone_number {
        out["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// The maximum downstream media bit rate for `identifier`, deterministic from its
/// trailing three digits `d` (`0..=999`; no digits → 0): the magnitude is `d`
/// (always within the schema's `0..=1024`) and the unit is `UNITS[d % 5]`. Both
/// axes are lowest-first so `…000` / a no-digit identifier is the floor
/// (`0 bps`), and the same input always returns the same rate and unit.
fn max_downstream_rate(identifier: &str) -> (i32, &'static str) {
    let d = scenarios::trailing_three_digits(identifier).unwrap_or(0);
    let unit = UNITS[d as usize % UNITS.len()];
    (i32::from(d), unit)
}

/// A resolved device identifier plus, when it is a phone number, that number
/// (for echoing back in the `DeviceResponse`).
struct Resolved {
    identifier: String,
    phone_number: Option<String>,
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Device Data Volume). See the module docs for the cases.
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

    const HOST: &str = "msr.local:8080";
    const PATH: &str = "/media-streaming-rate/vwip/retrieve-maximum-downstream-media-rate";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn rate_and_unit_are_derived_from_the_trailing_digits() {
        // …000 / no digits → floor: 0 bps.
        assert_eq!(max_downstream_rate("+123456789000"), (0, "bps"));
        assert_eq!(max_downstream_rate("msr-client"), (0, "bps"));
        // The magnitude is the tail; the unit is tail % 5 (lowest-first).
        assert_eq!(max_downstream_rate("+123456789001"), (1, "kbps"));
        assert_eq!(max_downstream_rate("+123456789002"), (2, "Mbps"));
        assert_eq!(max_downstream_rate("+123456789003"), (3, "Gbps"));
        assert_eq!(max_downstream_rate("+123456789004"), (4, "Tbps"));
        // …005 → 5 % 5 == 0 → bps again (unit cycles).
        assert_eq!(max_downstream_rate("+123456789005"), (5, "bps"));
        assert_eq!(max_downstream_rate("+123456789013"), (13, "Gbps"));
        assert_eq!(max_downstream_rate("+123456789999"), (999, "Tbps"));
    }

    #[test]
    fn magnitude_never_exceeds_the_schema_maximum() {
        // The tail is 0..=999, always within the schema's 0..=1024.
        for tail in [0u16, 1, 250, 500, 999] {
            let id = format!("+12345678{tail:03}");
            let (rate, unit) = max_downstream_rate(&id);
            assert!((0..=1024).contains(&rate), "{rate} out of range for {id}");
            assert!(UNITS.contains(&unit), "{unit} not in the enum");
        }
    }

    #[test]
    fn every_unit_is_a_valid_camara_enum_value() {
        for u in UNITS {
            assert!(
                ["bps", "kbps", "Mbps", "Gbps", "Tbps"].contains(&u),
                "{u} not in the enum"
            );
        }
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("+1234567890123456")); // too long
        assert!(!is_valid_e164("msr-client"));
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
        mint_token_with_client(scope, "msr-client").await
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
        let token = mint_token(RETRIEVE_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    // --- Two-legged (submitted device) success cases ----------------------

    #[tokio::test]
    async fn returns_the_rate_with_required_fields() {
        // …000 → floor 0 bps, phone-keyed → device echoed.
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789000"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["maxDownstreamMediaBitRateSupported"], 0);
        assert_eq!(body["unit"], "bps");
        assert_eq!(body["device"]["phoneNumber"], "+123456789000");
    }

    #[tokio::test]
    async fn different_tails_select_different_rates_and_units() {
        // …002 → 2 Mbps.
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789002"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["maxDownstreamMediaBitRateSupported"], 2);
        assert_eq!(body["unit"], "Mbps");
        // …013 → 13 Gbps (unit is a genuine plane: 13 % 5 == 3).
        let (_, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789013"}}"#).await;
        assert_eq!(body["maxDownstreamMediaBitRateSupported"], 13);
        assert_eq!(body["unit"], "Gbps");
    }

    #[tokio::test]
    async fn non_phone_identifier_omits_the_device_echo() {
        // ipv4 publicAddress ending …002 → 2 Mbps, no device echo
        // (DeviceResponse carries only phoneNumber).
        let (status, _, body) =
            call_ok(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.002"}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["maxDownstreamMediaBitRateSupported"], 2);
        assert_eq!(body["unit"], "Mbps");
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

        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789503"}}"#).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …002 → 2 Mbps, device echoed.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789002").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["maxDownstreamMediaBitRateSupported"], 2);
        assert_eq!(body["unit"], "Mbps");
        assert_eq!(body["device"]["phoneNumber"], "+123456789002");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789404").await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn resubmitting_the_device_on_a_line_token_is_unnecessary() {
        // Subject is a line (three-legged) AND a device is submitted → 422.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"device":{"phoneNumber":"+123456789012"}}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        // Two-legged token (sub = msr-client) with no device → can't identify.
        let (status, _, body) = call_ok("{}").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");

        // Also for a wholly empty body.
        let token = mint_token(RETRIEVE_SCOPE).await;
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
        let token = mint_token(RETRIEVE_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789002"}}"#,
            Some("corr-msr"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-msr")
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
