//! IoT SIM Fraud Prevention **vwip** (CAMARA IoTSIMFraudPrevention, version `wip`).
//!
//! The full **`IMEIBIND`** round-trip is mounted over a shared in-memory binding
//! [`store`]:
//! - `POST /iot-sim-fraud-prevention/vwip/bind` — bind a device's SIM to its IMEI
//!   (operationId `bindDeviceImei`, scope `iot-sim-fraud-prevention:bind`).
//! - `POST /iot-sim-fraud-prevention/vwip/query` — the current IMEI binding status
//!   of a device's SIM (operationId `query`, scope `iot-sim-fraud-prevention:query`).
//! - `POST /iot-sim-fraud-prevention/vwip/unbind` — remove a device's IMEI binding
//!   (operationId `unBindDeviceImei`, scope `iot-sim-fraud-prevention:unbind`).
//!
//! The upstream CAMARA API also defines a second bind/query type `AREALIMIT`
//! (a geographic area restriction). That is **deferred** — it is spatial, lower
//! priority than this non-spatial slice (docs/DESIGN.md §12) — so the vendored
//! spec's `*Type` enums are trimmed to `[IMEIBIND]`, and the spec never claims
//! behaviour the server does not implement.
//!
//! ## What it does
//!
//! The caller identifies a device either by a `device` object in the request body
//! (`phoneNumber`, `networkAccessIdentifier`, `ipv4Address`, or `ipv6Address`) or
//! — when `device` is omitted — by the identity a three-legged access token
//! authenticated. `bind` records the SIM↔IMEI association, `query` answers
//! `{ "imeiBind": { "bindStatus": "BOUND"|"UNBOUND", "bindImei"?: "<15 digits>" } }`,
//! and `unbind` clears it (`{ "unbound": true }`, or `422 UNNECESSARY_UNBIND_IMEI`
//! when nothing is bound).
//!
//! Every endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying that operation's scope.
//!
//! ## Two-legged / three-legged identifier rule
//!
//! Exactly one identification path must be used (mirrors Number Recycling / Call
//! Forwarding Signal):
//! - a `device` **and** a three-legged token (an E.164 `sub`) → `422
//!   UNNECESSARY_IDENTIFIER`;
//! - no `device` **and** a two-legged token → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The identifier is the first present identifier of the submitted `device`
//! (precedence: phoneNumber, networkAccessIdentifier, the IPv4 `publicAddress`,
//! then ipv6Address) or, when no `device` is supplied, the token subject. After
//! the shared **reserved error suffix** plane (trailing three digits naming a
//! reserved CAMARA status: `…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//! `…429`, `…500`, `…503` → that canonical CAMARA error, [`crate::scenarios`]),
//! each operation is driven as follows:
//!
//! - **`bind`** — records the SIM↔IMEI binding in the [`store`] and answers
//!   `{ bound: true }` (idempotent). The bound IMEI is deterministic: a fixed TAC
//!   (`35209900`) + the identifier's zero-padded trailing-three-digit serial + a
//!   GSMA Luhn check digit.
//! - **`query`** — a stored binding wins: `bindStatus: "BOUND"` with the stored
//!   `bindImei`. Absent one, the stateless default: **odd trailing digits** →
//!   BOUND with the synthesised IMEI; **any other input** (even tail, `…000`, or
//!   no trailing digits) → `bindStatus: "UNBOUND"`, no `bindImei`.
//! - **`unbind`** — a stored binding is removed → `{ unbound: true }`; a device
//!   with no binding → `422 UNNECESSARY_UNBIND_IMEI`.
//!
//! So a `bind` flips an otherwise-`UNBOUND` device to `BOUND`, and an `unbind`
//! restores the default. Examples (no explicit binding): `+123456789011` →
//! query BOUND; `+123456789012` / `+123456789000` → query UNBOUND;
//! `+123456789404` → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /query` endpoint requires.
const QUERY_SCOPE: &str = "iot-sim-fraud-prevention:query";

/// The OAuth2 scope the `POST /bind` endpoint requires.
const BIND_SCOPE: &str = "iot-sim-fraud-prevention:bind";

/// The OAuth2 scope the `POST /unbind` endpoint requires.
const UNBIND_SCOPE: &str = "iot-sim-fraud-prevention:unbind";

/// The fixed 8-digit Type Allocation Code used for every synthesised bound IMEI
/// (mirrors the spec's `35-209900-…` example TAC).
const TAC: &str = "35209900";

/// Routes for IoT SIM Fraud Prevention vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/iot-sim-fraud-prevention/vwip/query", post(query))
        .route("/iot-sim-fraud-prevention/vwip/bind", post(bind))
        .route("/iot-sim-fraud-prevention/vwip/unbind", post(unbind))
}

/// `POST /query` request body (CAMARA `QueryRequest`). `queryType` is required;
/// `device` is optional (omit it when a three-legged token identifies the device).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QueryRequest {
    device: Option<Device>,
    #[serde(rename = "queryType")]
    query_type: QueryType,
}

/// CamaraSim implements the non-spatial `IMEIBIND` query only; `AREALIMIT` is a
/// deferred (spatial) case, so it is not a value this deployment accepts — an
/// `AREALIMIT` request fails to deserialise and is rejected `400 INVALID_ARGUMENT`.
#[derive(Debug, Deserialize, PartialEq)]
enum QueryType {
    #[serde(rename = "IMEIBIND")]
    ImeiBind,
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

/// `POST /iot-sim-fraud-prevention/vwip/query`.
async fn query(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(QUERY_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `queryType` is required, so the body must be present and parse. An unknown
    // `queryType` value (e.g. the deferred `AREALIMIT`) fails to deserialise.
    let req: QueryRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid QueryRequest (queryType must be \"IMEIBIND\").",
                &correlator,
            )
        }
    };
    // Only IMEIBIND exists in the trimmed enum; kept explicit for future types.
    let QueryType::ImeiBind = req.query_type;

    // The identifier is the submitted device identifier, else the token subject
    // (three-legged fallback), enforcing the two-/three-legged rule.
    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let imei_bind = imei_bind(&identifier);

    with_correlator(
        (StatusCode::OK, Json(json!({ "imeiBind": imei_bind }))).into_response(),
        &correlator,
    )
}

/// The IMEI-binding status of `identifier`.
///
/// An **explicit binding** in the shared [`store`] (set by `POST /bind`, cleared
/// by `POST /unbind`) takes precedence: the device is `BOUND` to the stored IMEI.
/// Absent one, the status falls back to the stateless default driven by the
/// identifier's trailing three digits (docs/DESIGN.md §7): odd → BOUND with a
/// synthesised 15-digit IMEI; anything else (even, `…000`, or no trailing digits)
/// → UNBOUND with no bound IMEI. So a `bind` flips an otherwise-`UNBOUND` device
/// to `BOUND`, and an `unbind` restores the default.
fn imei_bind(identifier: &str) -> serde_json::Value {
    if let Some(imei) = store::bound_imei(identifier) {
        return json!({ "bindStatus": "BOUND", "bindImei": imei });
    }
    match scenarios::trailing_three_digits(identifier) {
        Some(n) if n % 2 == 1 => json!({
            "bindStatus": "BOUND",
            "bindImei": synth_imei(n),
        }),
        _ => json!({ "bindStatus": "UNBOUND" }),
    }
}

/// `POST /bind` request body (CAMARA `BindDeviceImeiRequest`). `bindType` is
/// required; `device` is optional (omit it when a three-legged token identifies
/// the device).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindRequest {
    device: Option<Device>,
    #[serde(rename = "bindType")]
    bind_type: BindType,
}

/// CamaraSim implements the non-spatial `IMEIBIND` bind only; `AREALIMIT` is a
/// deferred (spatial) case, so it is not a value this deployment accepts — an
/// `AREALIMIT` bind fails to deserialise and is rejected `400 INVALID_ARGUMENT`.
#[derive(Debug, Deserialize, PartialEq)]
enum BindType {
    #[serde(rename = "IMEIBIND")]
    ImeiBind,
}

/// `POST /iot-sim-fraud-prevention/vwip/bind` — bind a device's SIM to its IMEI
/// (operationId `bindDeviceImei`, scope `iot-sim-fraud-prevention:bind`).
///
/// The SIM is bound to the IMEI the network observes for the device — in the
/// simulator, the deterministic [`synth_imei`] of the resolved identifier — and
/// the binding is remembered in the shared [`store`] so a later `query` reports
/// it and an `unbind` can clear it. Binding is idempotent → `200 { bound: true }`.
async fn bind(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(BIND_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `bindType` is required; an unknown value (e.g. the deferred `AREALIMIT`)
    // fails to deserialise → 400 INVALID_ARGUMENT.
    let req: BindRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid BindDeviceImeiRequest (bindType must be \"IMEIBIND\").",
                &correlator,
            )
        }
    };
    let BindType::ImeiBind = req.bind_type;

    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Bind the SIM to the IMEI the network would observe for this device.
    let imei = synth_imei(scenarios::trailing_three_digits(&identifier).unwrap_or(0));
    store::bind(identifier, imei);

    with_correlator(
        (StatusCode::OK, Json(json!({ "bound": true }))).into_response(),
        &correlator,
    )
}

/// `POST /unbind` request body (CAMARA `UnBindDeviceImeiRequest`). `unBindType`
/// is required; `device` is optional (three-legged fallback).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnbindRequest {
    device: Option<Device>,
    #[serde(rename = "unBindType")]
    unbind_type: UnbindType,
}

/// CamaraSim implements the non-spatial `IMEIBIND` unbind only; `AREALIMIT` is a
/// deferred (spatial) case → an `AREALIMIT` unbind fails to deserialise and is
/// rejected `400 INVALID_ARGUMENT`.
#[derive(Debug, Deserialize, PartialEq)]
enum UnbindType {
    #[serde(rename = "IMEIBIND")]
    ImeiBind,
}

/// `POST /iot-sim-fraud-prevention/vwip/unbind` — remove a device's IMEI binding
/// (operationId `unBindDeviceImei`, scope `iot-sim-fraud-prevention:unbind`).
///
/// Keyed on the shared [`store`] (after the identifier / reserved-error planes):
/// an existing binding is removed → `200 { unbound: true }`; a device with no
/// binding → `422 UNNECESSARY_UNBIND_IMEI` (there is nothing to unbind).
async fn unbind(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(UNBIND_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: UnbindRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid UnBindDeviceImeiRequest (unBindType must be \"IMEIBIND\").",
                &correlator,
            )
        }
    };
    let UnbindType::ImeiBind = req.unbind_type;

    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    match store::unbind(&identifier) {
        Some(_) => with_correlator(
            (StatusCode::OK, Json(json!({ "unbound": true }))).into_response(),
            &correlator,
        ),
        None => unprocessable(
            "UNNECESSARY_UNBIND_IMEI",
            "The device has no IMEI binding to remove.",
            &correlator,
        ),
    }
}

/// A deterministic 15-digit IMEI for a device whose trailing three digits are
/// `serial_tail`: the fixed [`TAC`] (8 digits) + a 6-digit serial (the
/// zero-padded `serial_tail`) + a GSMA Luhn check digit over the first 14.
fn synth_imei(serial_tail: u16) -> String {
    let mut imei = format!("{TAC}{:06}", serial_tail % 1000);
    imei.push(char::from(b'0' + luhn_check_digit(imei.as_bytes())));
    imei
}

/// The Luhn check digit (0..=9) for the ASCII-digit string `digits`, computed as
/// GSMA specifies for IMEIs: double every second digit from the right, sum the
/// decimal digits of the products with the rest, and return the amount needed to
/// reach the next multiple of ten.
fn luhn_check_digit(digits: &[u8]) -> u8 {
    let mut sum = 0u32;
    // The check digit will sit at the rightmost (even) position, so the last
    // input digit is doubled and we alternate leftward.
    for (i, &b) in digits.iter().rev().enumerate() {
        let mut d = u32::from(b - b'0');
        if i % 2 == 0 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    ((10 - (sum % 10)) % 10) as u8
}

/// Resolve the device identifier for a request, enforcing the CAMARA two-legged /
/// three-legged identifier rules. A three-legged token is recognised by an E.164
/// `sub`. On failure returns the CAMARA error `Response` to send.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_device = is_valid_e164(subject);

    match device {
        Some(device) => {
            let id = match device_identifier(&device) {
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
            // A three-legged token already identifies the device; the explicit
            // identifier is unnecessary.
            if subject_is_device {
                return Err(unprocessable(
                    "UNNECESSARY_IDENTIFIER",
                    "An explicit identifier has been provided for the device when this is already identified by the access token.",
                    correlator,
                ));
            }
            Ok(id)
        }
        None => {
            if subject_is_device {
                Ok(subject.to_string())
            } else {
                Err(unprocessable(
                    "MISSING_IDENTIFIER",
                    "No `device` supplied and the access token identifies no device.",
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

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 422 `Unprocessable Content` CAMARA error with the given code, correlator echoed.
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
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "iot.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn imei_bind_is_driven_by_the_trailing_digits() {
        // odd tail → BOUND with a 15-digit IMEI.
        let bound = imei_bind("+123456789011");
        assert_eq!(bound["bindStatus"], "BOUND");
        let imei = bound["bindImei"].as_str().unwrap();
        assert_eq!(imei.len(), 15);
        assert!(imei.bytes().all(|b| b.is_ascii_digit()));
        assert!(imei.starts_with(TAC));

        // even tail → UNBOUND, no bound IMEI.
        let unbound = imei_bind("+123456789012");
        assert_eq!(unbound["bindStatus"], "UNBOUND");
        assert!(unbound.get("bindImei").is_none());

        // …000 tail → UNBOUND.
        assert_eq!(imei_bind("+123456789000")["bindStatus"], "UNBOUND");
        // no trailing digits → default happy path (UNBOUND).
        assert_eq!(imei_bind("camarasim-user")["bindStatus"], "UNBOUND");
    }

    #[test]
    fn synth_imei_is_deterministic_and_luhn_valid() {
        let imei = synth_imei(11);
        assert_eq!(imei, synth_imei(11)); // stable
        assert_eq!(&imei[..8], TAC);
        assert_eq!(&imei[8..14], "000011"); // zero-padded serial
                                            // whole 15-digit string passes Luhn (check digit sums to 0 mod 10).
        assert_eq!(luhn_over(imei.as_bytes()) % 10, 0);
    }

    /// Full Luhn sum over an already-complete number (check digit at position 0
    /// from the right, i.e. not doubled) — used to verify the synthesised IMEI.
    fn luhn_over(digits: &[u8]) -> u32 {
        let mut sum = 0u32;
        for (i, &b) in digits.iter().rev().enumerate() {
            let mut d = u32::from(b - b'0');
            if i % 2 == 1 {
                d *= 2;
                if d > 9 {
                    d -= 9;
                }
            }
            sum += d;
        }
        sum
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
    fn device_identifier_follows_precedence() {
        let phone = Device {
            phone_number: Some("+123456789011".into()),
            network_access_identifier: Some("nai@example.com".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&phone), Some(DeviceId::PhoneNumber(p)) if p == "+123456789011"));

        let ipv4 = Device {
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.11".into()),
                private_address: None,
                public_port: None,
            }),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&ipv4), Some(DeviceId::Other(p)) if p == "203.0.113.11"));

        assert!(device_identifier(&Device::default()).is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "iot-client").await
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

    async fn post_query(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/iot-sim-fraud-prevention/vwip/query")
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

    async fn query_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(QUERY_SCOPE).await;
        post_query(Some(&token), body, None).await
    }

    /// POST to an arbitrary IoT SIM path (`bind`/`unbind`/`query`) with a Bearer
    /// token, returning `(status, headers, json-body)`.
    async fn post_path(
        path: &str,
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("/iot-sim-fraud-prevention/vwip/{path}"))
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

    /// Bind `device_json`'s SIM with a freshly-minted bind-scoped token.
    async fn bind_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(BIND_SCOPE).await;
        post_path("bind", Some(&token), body, None).await
    }

    /// Unbind `device_json`'s SIM with a freshly-minted unbind-scoped token.
    async fn unbind_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(UNBIND_SCOPE).await;
        post_path("unbind", Some(&token), body, None).await
    }

    /// Query `device_json` with a freshly-minted query-scoped token (helper for
    /// the round-trip tests, which query numbers disjoint from the older tests).
    async fn query_number(phone: &str) -> Value {
        let (status, _, body) = query_ok_token(&format!(
            r#"{{"device":{{"phoneNumber":"{phone}"}},"queryType":"IMEIBIND"}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::OK, "query for {phone} should be 200");
        body["imeiBind"].clone()
    }

    #[tokio::test]
    async fn odd_tail_is_bound_with_a_synthesised_imei() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789011"},"queryType":"IMEIBIND"}"#)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "BOUND");
        let imei = body["imeiBind"]["bindImei"].as_str().unwrap();
        assert_eq!(imei.len(), 15);
        assert!(imei.starts_with("35209900"));
    }

    #[tokio::test]
    async fn even_tail_is_unbound_without_an_imei() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789012"},"queryType":"IMEIBIND"}"#)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "UNBOUND");
        assert!(body["imeiBind"].get("bindImei").is_none());
    }

    #[tokio::test]
    async fn triple_zero_tail_is_unbound() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789000"},"queryType":"IMEIBIND"}"#)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "UNBOUND");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789404"},"queryType":"IMEIBIND"}"#)
                .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789429"},"queryType":"IMEIBIND"}"#)
                .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn non_phone_identifiers_are_accepted() {
        // networkAccessIdentifier ending in an odd tail → BOUND.
        let (status, _, body) = query_ok_token(
            r#"{"device":{"networkAccessIdentifier":"user011@nai"},"queryType":"IMEIBIND"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "BOUND");

        // ipv6 ending in an even hex-but-digit tail → UNBOUND (…012).
        let (status, _, body) = query_ok_token(
            r#"{"device":{"ipv6Address":"2001:db8::012"},"queryType":"IMEIBIND"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "UNBOUND");
    }

    #[tokio::test]
    async fn arealimit_query_type_is_rejected_as_invalid_argument() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789011"},"queryType":"AREALIMIT"}"#)
                .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_query_type_is_rejected() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"+123456789011"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{"phoneNumber":"0123"},"queryType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) =
            query_ok_token(r#"{"device":{},"queryType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = query_ok_token(
            r#"{"device":{"phoneNumber":"+123456789011"},"queryType":"IMEIBIND","x":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // E.164 subject with an odd tail → BOUND, no device in the body.
        let token = mint_token_with_client(QUERY_SCOPE, "+123456789011").await;
        let (status, _, body) = post_query(Some(&token), r#"{"queryType":"IMEIBIND"}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "BOUND");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(QUERY_SCOPE, "+123456789503").await;
        let (status, _, body) = post_query(Some(&token), r#"{"queryType":"IMEIBIND"}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn device_with_three_legged_token_is_unnecessary_identifier() {
        let token = mint_token_with_client(QUERY_SCOPE, "+123456789011").await;
        let (status, _, body) = post_query(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789022"},"queryType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_with_two_legged_token_is_missing_identifier() {
        // Default synthetic subject "iot-client" is not E.164 → two-legged.
        let (status, _, body) = query_ok_token(r#"{"queryType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_query(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789011"},"queryType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_query(
            None,
            r#"{"device":{"phoneNumber":"+123456789011"},"queryType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(QUERY_SCOPE).await;
        let (status, headers, _) = post_query(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789011"},"queryType":"IMEIBIND"}"#,
            Some("corr-iot"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-iot")
        );

        let (status, headers, _) = post_query(
            Some(&token),
            r#"{"device":{"phoneNumber":"0123"},"queryType":"IMEIBIND"}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- bind / unbind (stateful IMEIBIND) --------------------------------
    //
    // These tests use phone numbers disjoint from every other test in this file
    // (the store is a process-global shared by the whole test binary), so they
    // never pollute one another's state.

    #[test]
    fn store_round_trips_a_binding() {
        // Pure store unit: bind → read → unbind → read.
        let key = "+iot-store-unit-1".to_string();
        assert_eq!(store::bound_imei(&key), None);
        store::bind(key.clone(), "352099000000112".into());
        assert_eq!(store::bound_imei(&key).as_deref(), Some("352099000000112"));
        assert_eq!(store::unbind(&key).as_deref(), Some("352099000000112"));
        assert_eq!(store::bound_imei(&key), None);
        assert_eq!(store::unbind(&key), None); // second unbind is a no-op
    }

    #[tokio::test]
    async fn bind_query_unbind_round_trip() {
        // An even-tail device defaults to UNBOUND; bind flips it to BOUND, and
        // unbind restores the default.
        let dev = r#"{"device":{"phoneNumber":"+19990000112"},"bindType":"IMEIBIND"}"#;
        assert_eq!(query_number("+19990000112").await["bindStatus"], "UNBOUND");

        let (status, _, body) = bind_ok(dev).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["bound"], true);

        let bound = query_number("+19990000112").await;
        assert_eq!(bound["bindStatus"], "BOUND");
        let imei = bound["bindImei"].as_str().unwrap();
        assert_eq!(imei.len(), 15);
        assert!(imei.starts_with("35209900"));

        let (status, _, body) =
            unbind_ok(r#"{"device":{"phoneNumber":"+19990000112"},"unBindType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["unbound"], true);

        assert_eq!(query_number("+19990000112").await["bindStatus"], "UNBOUND");
    }

    #[tokio::test]
    async fn bind_is_idempotent() {
        let dev = r#"{"device":{"phoneNumber":"+19990000122"},"bindType":"IMEIBIND"}"#;
        for _ in 0..2 {
            let (status, _, body) = bind_ok(dev).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["bound"], true);
        }
        assert_eq!(query_number("+19990000122").await["bindStatus"], "BOUND");
    }

    #[tokio::test]
    async fn unbind_without_a_binding_is_unnecessary_unbind_imei() {
        let (status, _, body) =
            unbind_ok(r#"{"device":{"phoneNumber":"+19990000132"},"unBindType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_UNBIND_IMEI");
    }

    #[tokio::test]
    async fn bind_reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            bind_ok(r#"{"device":{"phoneNumber":"+19990000404"},"bindType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn unbind_reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            unbind_ok(r#"{"device":{"phoneNumber":"+19990000429"},"unBindType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn bind_arealimit_type_is_rejected_as_invalid_argument() {
        let (status, _, body) =
            bind_ok(r#"{"device":{"phoneNumber":"+19990000142"},"bindType":"AREALIMIT"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unbind_arealimit_type_is_rejected_as_invalid_argument() {
        let (status, _, body) =
            unbind_ok(r#"{"device":{"phoneNumber":"+19990000152"},"unBindType":"AREALIMIT"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bind_missing_bind_type_is_rejected() {
        let (status, _, body) =
            bind_ok(r#"{"device":{"phoneNumber":"+19990000162"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bind_no_device_with_two_legged_token_is_missing_identifier() {
        let (status, _, body) = bind_ok(r#"{"bindType":"IMEIBIND"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn bind_device_with_three_legged_token_is_unnecessary_identifier() {
        let token = mint_token_with_client(BIND_SCOPE, "+19990000182").await;
        let (status, _, body) = post_path(
            "bind",
            Some(&token),
            r#"{"device":{"phoneNumber":"+19990000172"},"bindType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn three_legged_bind_is_visible_to_a_three_legged_query() {
        // Bind keyed off the token subject (no device), then query the same
        // subject (no device) → the binding is visible.
        let bind_token = mint_token_with_client(BIND_SCOPE, "+19990000192").await;
        let (status, _, body) =
            post_path("bind", Some(&bind_token), r#"{"bindType":"IMEIBIND"}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["bound"], true);

        let query_token = mint_token_with_client(QUERY_SCOPE, "+19990000192").await;
        let (status, _, body) =
            post_query(Some(&query_token), r#"{"queryType":"IMEIBIND"}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["imeiBind"]["bindStatus"], "BOUND");
    }

    #[tokio::test]
    async fn bind_requires_the_bind_scope() {
        // A query-scoped token cannot bind.
        let token = mint_token(QUERY_SCOPE).await;
        let (status, _, body) = post_path(
            "bind",
            Some(&token),
            r#"{"device":{"phoneNumber":"+19990000202"},"bindType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn unbind_requires_the_unbind_scope() {
        let token = mint_token(QUERY_SCOPE).await;
        let (status, _, body) = post_path(
            "unbind",
            Some(&token),
            r#"{"device":{"phoneNumber":"+19990000212"},"unBindType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn bind_missing_token_is_unauthenticated() {
        let (status, _, body) = post_path(
            "bind",
            None,
            r#"{"device":{"phoneNumber":"+19990000222"},"bindType":"IMEIBIND"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn bind_x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(BIND_SCOPE).await;
        let (status, headers, _) = post_path(
            "bind",
            Some(&token),
            r#"{"device":{"phoneNumber":"+19990000232"},"bindType":"IMEIBIND"}"#,
            Some("corr-bind"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-bind")
        );

        let (status, headers, _) = post_path(
            "bind",
            Some(&token),
            r#"{"device":{"phoneNumber":"0123"},"bindType":"IMEIBIND"}"#,
            Some("corr-bind-err"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-bind-err")
        );
    }
}
