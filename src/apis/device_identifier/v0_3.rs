//! Device Identifier **v0.3** (CAMARA Device Identifier 0.3.0, release r2.2).
//!
//! Endpoints so far:
//! - `POST /device-identifier/v0.3/retrieve-type` — the device's *type*
//!   (manufacturer / model / Type Allocation Code), with no full identifier.
//! - `POST /device-identifier/v0.3/retrieve-identifier` — the device's full
//!   identity (`imei` / `imeisv`) alongside its type (operationId
//!   `retrieveIdentifier`).
//!
//! ## What it does
//!
//! The caller asks about a device, identified either by a `device` object in the
//! request body (`phoneNumber`, `networkAccessIdentifier`, `ipv4Address`, or
//! `ipv6Address`) or — when `device` is omitted — by the identity a three-legged
//! access token authenticated. `retrieve-type` answers with the device's
//! `{ tac, manufacturer, model, lastChecked }`; `retrieve-identifier` adds the
//! synthesised `imei` (15-digit, TAC + serial + Luhn check digit) and `imeisv`
//! (16-digit, TAC + serial + software-version). Both echo back the `device`
//! identifier they used.
//!
//! Each endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying, respectively, the
//! `device-identifier:retrieve-type` and `device-identifier:retrieve-identifier`
//! scopes.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The identifier is the submitted device identifier (phoneNumber, else network
//! access identifier, else the IPv4 `publicAddress`, else ipv6Address) or, when
//! no `device` is supplied, the access token's subject (`sub`). Its trailing
//! three digits select the case:
//!
//! - **Reserved error suffix** — if those trailing three digits name a reserved
//!   CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`,
//!   `…500`, `…503`), the endpoint answers with that canonical CAMARA error
//!   instead of a result (shared convention, [`crate::scenarios`]).
//! - **any other tail** (including `…000` and an identifier with no trailing
//!   digits, such as the synthetic `camarasim-user` subject) — the endpoint
//!   returns a device type chosen deterministically from the trailing three
//!   digits (`DEVICE_TYPES[digits % len]`, `…000`/none → the default entry), so
//!   the reported device model is itself a control plane.
//!
//! Examples: `+123456789001` → OnePlus 12 (`1 % 6`); `+123456789002` → Google
//! Pixel 8 Pro (`2 % 6`); `+123456789000` → the default (Apple iPhone 15 Pro);
//! `+123456789404` → `404 NOT_FOUND`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /retrieve-type` endpoint requires (CAMARA Device
/// Identifier 0.3.0).
const RETRIEVE_TYPE_SCOPE: &str = "device-identifier:retrieve-type";

/// The OAuth2 scope the `POST /retrieve-identifier` endpoint requires (CAMARA
/// Device Identifier 0.3.0).
const RETRIEVE_IDENTIFIER_SCOPE: &str = "device-identifier:retrieve-identifier";

/// Device types the endpoint can report, as `(TAC, manufacturer, model)` triples.
/// The identifier's trailing three digits index this table (`digits % len`), so
/// the reported device type is deterministic from the input; `…000` and an
/// identifier with no trailing digits fall to the first (default) entry. Each
/// TAC is exactly 8 digits, matching the CAMARA `^[0-9]{8}$` pattern.
const DEVICE_TYPES: &[(&str, &str, &str)] = &[
    ("35692005", "Apple", "iPhone 15 Pro"),      // default (…000 / no digits)
    ("35847104", "OnePlus", "OnePlus 12"),
    ("35438509", "Google", "Pixel 8 Pro"),
    ("86234502", "Xiaomi", "Redmi Note 13"),
    ("35315106", "Samsung", "Galaxy S24 Ultra"),
    ("35201607", "Motorola", "Edge 50"),
];

/// Routes for Device Identifier v0.3, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/device-identifier/v0.3/retrieve-type",
            post(retrieve_type),
        )
        .route(
            "/device-identifier/v0.3/retrieve-identifier",
            post(retrieve_identifier),
        )
}

/// `POST /retrieve-type` request body (CAMARA `RequestBody`). `device` is
/// optional — omit it when a three-legged token identifies the device.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestBody {
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
    // Accepted for schema fidelity but not used to key scenarios.
    #[serde(rename = "privateAddress")]
    #[allow(dead_code)]
    private_address: Option<String>,
    #[serde(rename = "publicPort")]
    #[allow(dead_code)]
    public_port: Option<i64>,
}

/// `POST /device-identifier/v0.3/retrieve-type`.
async fn retrieve_type(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_TYPE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (device optional); anything present must parse.
    let req: RequestBody = if body.is_empty() {
        RequestBody { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument("Request body is not a valid RequestBody.", &correlator)
            }
        }
    };

    // The identifier is the submitted device identifier, else the token subject
    // (three-legged fallback). Missing both → 422 MISSING_IDENTIFIER.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    let (tac, manufacturer, model) = device_type(&resolved.id);
    let mut response_body = json!({
        "lastChecked": rfc3339_utc(now_unix_secs()),
        "tac": tac,
        "manufacturer": manufacturer,
        "model": model,
    });
    // Echo the device identifier that was used, when we can represent it as a
    // single-property `DeviceResponse` (CommonResponseBody, maxProperties: 1).
    if let Some(echo) = resolved.echo {
        response_body["device"] = echo;
    }

    with_correlator(
        (StatusCode::OK, Json(response_body)).into_response(),
        &correlator,
    )
}

/// `POST /device-identifier/v0.3/retrieve-identifier`.
///
/// Like `retrieve-type`, but answers with the device's full identity — a
/// synthesised `imei` and `imeisv` — alongside its type. Same identifier
/// resolution and reserved-error convention (docs/DESIGN.md §7).
async fn retrieve_identifier(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_IDENTIFIER_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // An empty body is allowed (device optional); anything present must parse.
    let req: RequestBody = if body.is_empty() {
        RequestBody { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument("Request body is not a valid RequestBody.", &correlator)
            }
        }
    };

    // The identifier is the submitted device identifier, else the token subject
    // (three-legged fallback). Missing both → 422 MISSING_IDENTIFIER.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // The type (and its TAC) is the same control plane as retrieve-type; the
    // full IMEI is synthesised from the selected TAC plus a serial derived from
    // the identifier, with a valid Luhn check digit (GSMA IMEI).
    let (tac, manufacturer, model) = device_type(&resolved.id);
    let serial = serial_from(&resolved.id);
    let imei = imei_from(tac, &serial);
    let imeisv = format!("{tac}{serial}{SOFTWARE_VERSION}");
    let mut response_body = json!({
        "lastChecked": rfc3339_utc(now_unix_secs()),
        "imei": imei,
        "imeisv": imeisv,
        "tac": tac,
        "manufacturer": manufacturer,
        "model": model,
    });
    // Echo the device identifier that was used, when we can represent it as a
    // single-property `DeviceResponse` (CommonResponseBody, maxProperties: 1).
    if let Some(echo) = resolved.echo {
        response_body["device"] = echo;
    }

    with_correlator(
        (StatusCode::OK, Json(response_body)).into_response(),
        &correlator,
    )
}

/// The two-digit Software Version Number (SVN) appended to a TAC + serial to form
/// the 16-digit IMEISV. Fixed for the simulator's single-node, in-memory model.
const SOFTWARE_VERSION: &str = "00";

/// The 6-digit serial number (SNR) portion of the synthesised IMEI/IMEISV,
/// derived from the identifier's trailing three digits so the identity is
/// deterministic from the input (`…000` and an identifier with no trailing
/// digits → `"000000"`). Reserved suffixes never reach here — they are answered
/// as errors first.
fn serial_from(identifier: &str) -> String {
    let digits = scenarios::trailing_three_digits(identifier).unwrap_or(0);
    format!("{digits:06}")
}

/// Synthesise a 15-digit IMEI = TAC (8) + serial (6) + Luhn check digit (1),
/// matching the GSMA IMEI structure and the CAMARA `^[0-9]{15}$` pattern.
fn imei_from(tac: &str, serial: &str) -> String {
    let base = format!("{tac}{serial}"); // 14 digits: TAC + SNR
    let check = luhn_check_digit(&base);
    format!("{base}{check}")
}

/// The Luhn (mod-10) check digit for a numeric string, as used by the GSMA to
/// close an IMEI. Doubles every second digit counting from the right of the
/// (check-digit-less) base, then returns the digit that makes the total a
/// multiple of ten.
fn luhn_check_digit(digits: &str) -> u32 {
    let sum: u32 = digits
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| {
            let d = (b - b'0') as u32;
            if i % 2 == 0 {
                // This digit sits one place left of the future check digit, so
                // it is doubled (9 → 18 → 1+8 = 9, i.e. subtract 9 when > 9).
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();
    (10 - (sum % 10)) % 10
}

/// The device type reported for `identifier`, driven by its trailing three digits
/// (docs/DESIGN.md §7): `DEVICE_TYPES[digits % len]`, with `…000` and an
/// identifier that has no trailing digits falling to the first (default) entry.
fn device_type(identifier: &str) -> (&'static str, &'static str, &'static str) {
    let idx = match scenarios::trailing_three_digits(identifier) {
        Some(0) | None => 0,
        Some(n) => (n as usize) % DEVICE_TYPES.len(),
    };
    DEVICE_TYPES[idx]
}

/// A resolved request identifier: the string CamaraSim keys functional cases off,
/// plus the single-property `device` object to echo in the response (`None` when
/// the identifier came from a token subject that is not a phone number).
struct Resolved {
    id: String,
    echo: Option<Value>,
}

/// Resolve the device identifier for a request: the first present identifier in
/// the supplied `device` (validated when it is a `phoneNumber`), else the token
/// subject (three-legged fallback). On failure returns the CAMARA error
/// `Response` to send — 400 `INVALID_ARGUMENT` for a malformed `phoneNumber` or a
/// `device` carrying no identifier, 422 `MISSING_IDENTIFIER` when neither a
/// device nor a token subject is present.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                let echo = json!({ "phoneNumber": phone });
                Ok(Resolved { id: phone, echo: Some(echo) })
            }
            Some(DeviceId::Nai(id)) => {
                let echo = json!({ "networkAccessIdentifier": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv4(id)) => {
                let echo = json!({ "ipv4Address": { "publicAddress": id } });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv6(id)) => {
                let echo = json!({ "ipv6Address": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            let subject = claims.subject().unwrap_or("");
            if subject.is_empty() {
                return Err(with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "No `device` supplied and the access token identifies no device.",
                    )
                    .into_response(),
                    correlator,
                ));
            }
            // A three-legged token whose subject is a phone number is echoed as
            // the device's phoneNumber; any other subject is not a device
            // identifier, so no `device` is echoed.
            let echo = is_valid_e164(subject).then(|| json!({ "phoneNumber": subject }));
            Ok(Resolved { id: subject.to_string(), echo })
        }
    }
}

/// The identifier CamaraSim reads from a `Device`, tagged by kind so the response
/// can echo the right `DeviceResponse` field. Precedence: phoneNumber,
/// networkAccessIdentifier, the IPv4 `publicAddress`, then ipv6Address.
enum DeviceId {
    PhoneNumber(String),
    Nai(String),
    Ipv4(String),
    Ipv6(String),
}

/// The first present identifier of a `Device`, in precedence order, or `None`
/// when the device carries no identifier at all (`minProperties: 1` violated).
fn device_identifier(device: &Device) -> Option<DeviceId> {
    if let Some(phone) = &device.phone_number {
        return Some(DeviceId::PhoneNumber(phone.clone()));
    }
    if let Some(nai) = &device.network_access_identifier {
        return Some(DeviceId::Nai(nai.clone()));
    }
    if let Some(ipv4) = &device.ipv4_address {
        if let Some(addr) = &ipv4.public_address {
            return Some(DeviceId::Ipv4(addr.clone()));
        }
    }
    if let Some(ipv6) = &device.ipv6_address {
        return Some(DeviceId::Ipv6(ipv6.clone()));
    }
    None
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
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 / ISO 8601 instant with
/// a `Z` offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs
/// no date/time dependency (mirrors `sim_swap::v2`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for any date).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "di.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn device_type_is_driven_by_the_trailing_digits() {
        // …000 → default (index 0).
        assert_eq!(device_type("+123456789000").1, "Apple");
        // no trailing digits → default.
        assert_eq!(device_type("camarasim-user").1, "Apple");
        // …001 → 1 % 6 = 1 → OnePlus.
        assert_eq!(device_type("+123456789001").1, "OnePlus");
        // …002 → 2 % 6 = 2 → Google.
        assert_eq!(device_type("+123456789002").1, "Google");
    }

    #[test]
    fn every_tac_matches_the_camara_pattern() {
        for (tac, _, _) in DEVICE_TYPES {
            assert_eq!(tac.len(), 8, "TAC {tac} must be 8 digits");
            assert!(tac.bytes().all(|b| b.is_ascii_digit()), "TAC {tac} must be digits");
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
    }

    #[test]
    fn rfc3339_utc_formats_known_epochs() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_704_067_200), "2024-01-01T00:00:00Z");
        assert_eq!(
            rfc3339_utc(1_704_067_200 + 14 * 3600 + 27 * 60 + 8),
            "2024-01-01T14:27:08Z"
        );
    }

    #[test]
    fn luhn_check_digit_closes_a_known_imei() {
        // 490154203237518 is a well-known Luhn-valid IMEI (check digit 8).
        assert_eq!(luhn_check_digit("49015420323751"), 8);
    }

    #[test]
    fn imei_is_15_digits_and_luhn_valid() {
        // …001 → OnePlus TAC 35847104, serial 000001.
        let serial = serial_from("+123456789001");
        assert_eq!(serial, "000001");
        let imei = imei_from("35847104", &serial);
        assert_eq!(imei.len(), 15);
        assert!(imei.bytes().all(|b| b.is_ascii_digit()));
        assert!(imei.starts_with("35847104000001"));
        // A Luhn-valid number: doubling from the rightmost digit sums to 0 mod 10.
        let sum: u32 = imei
            .bytes()
            .rev()
            .enumerate()
            .map(|(i, b)| {
                let d = (b - b'0') as u32;
                if i % 2 == 1 {
                    let x = d * 2;
                    if x > 9 { x - 9 } else { x }
                } else {
                    d
                }
            })
            .sum();
        assert_eq!(sum % 10, 0, "IMEI {imei} must satisfy the Luhn check");
    }

    #[test]
    fn serial_defaults_to_zeros_without_trailing_digits() {
        assert_eq!(serial_from("camarasim-user"), "000000");
        assert_eq!(serial_from("+123456789000"), "000000");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the route's audience. Scope granted verbatim.
    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "di-client").await
    }

    /// As [`mint_token`], but with a caller-chosen `client_id` — which becomes
    /// the token `sub`. Used to drive the subject-keyed (no-device) cases.
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

    /// POST a JSON body to `/retrieve-type` with an optional Bearer token and `x-correlator`.
    async fn post_retrieve_type(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/device-identifier/v0.3/retrieve-type")
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

    /// Mint a scoped token and call retrieve-type with the given body.
    async fn retrieve_type_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_TYPE_SCOPE).await;
        post_retrieve_type(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn returns_device_type_and_echoes_the_device() {
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"phoneNumber":"+123456789001"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tac"], "35847104");
        assert_eq!(body["manufacturer"], "OnePlus");
        assert_eq!(body["model"], "OnePlus 12");
        assert!(body["lastChecked"].as_str().unwrap().ends_with('Z'));
        assert_eq!(body["device"]["phoneNumber"], "+123456789001");
    }

    #[tokio::test]
    async fn a_different_tail_reports_a_different_type() {
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"phoneNumber":"+123456789002"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Google");
        assert_eq!(body["model"], "Pixel 8 Pro");
        assert_eq!(body["tac"], "35438509");
    }

    #[tokio::test]
    async fn triple_zero_tail_reports_the_default_type() {
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"phoneNumber":"+123456789000"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Apple");
        assert_eq!(body["model"], "iPhone 15 Pro");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"phoneNumber":"+123456789404"}}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"phoneNumber":"+123456789429"}}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn non_phone_identifiers_are_accepted_and_echoed() {
        // networkAccessIdentifier ending in …001 → OnePlus, echoed as NAI.
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"networkAccessIdentifier":"user001@nai"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "OnePlus");
        assert_eq!(body["device"]["networkAccessIdentifier"], "user001@nai");

        // ipv4 publicAddress ending in …002 → Google, echoed as ipv4Address.
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.002"}}}"#)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Google");
        assert_eq!(body["device"]["ipv4Address"]["publicAddress"], "203.0.113.002");

        // ipv6Address ending in …001 → OnePlus, echoed as ipv6Address.
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"ipv6Address":"2001:db8::001"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["device"]["ipv6Address"], "2001:db8::001");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = retrieve_type_ok(r#"{"device":{"phoneNumber":"0123"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = retrieve_type_ok(r#"{"device":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            retrieve_type_ok(r#"{"device":{"phoneNumber":"+123456789001"},"x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject() {
        // Subject is an E.164 number ending …002 → Google, empty body; echoed.
        let token = mint_token_with_client(RETRIEVE_TYPE_SCOPE, "+123456789002").await;
        let (status, _, body) = post_retrieve_type(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Google");
        assert_eq!(body["device"]["phoneNumber"], "+123456789002");
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(RETRIEVE_TYPE_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve_type(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn non_numeric_subject_without_device_reports_the_default_and_no_echo() {
        // Default synthetic subject "di-client" has no digits → default type,
        // and — not being a phone number — is not echoed as a device.
        let (status, _, body) = retrieve_type_ok("{}").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Apple");
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_retrieve_type(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789001"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_retrieve_type(None, r#"{"device":{"phoneNumber":"+123456789001"}}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_TYPE_SCOPE).await;
        let (status, headers, _) = post_retrieve_type(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789001"}}"#,
            Some("corr-di"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-di")
        );

        let (status, headers, _) = post_retrieve_type(
            Some(&token),
            r#"{"device":{"phoneNumber":"0123"}}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- retrieve-identifier -----------------------------------------------

    /// POST a JSON body to `/retrieve-identifier` with an optional Bearer token
    /// and `x-correlator`.
    async fn post_retrieve_identifier(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/device-identifier/v0.3/retrieve-identifier")
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

    /// Mint a scoped token and call retrieve-identifier with the given body.
    async fn retrieve_identifier_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_IDENTIFIER_SCOPE).await;
        post_retrieve_identifier(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn returns_imei_imeisv_and_type_and_echoes_the_device() {
        // …001 → OnePlus TAC 35847104, serial 000001.
        let (status, _, body) =
            retrieve_identifier_ok(r#"{"device":{"phoneNumber":"+123456789001"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["tac"], "35847104");
        assert_eq!(body["manufacturer"], "OnePlus");
        assert_eq!(body["model"], "OnePlus 12");
        // IMEI = TAC + serial + Luhn check digit (15 digits), IMEISV 16 digits.
        let imei = body["imei"].as_str().unwrap();
        assert_eq!(imei.len(), 15);
        assert!(imei.starts_with("35847104000001"));
        assert_eq!(body["imeisv"], "3584710400000100");
        assert!(body["lastChecked"].as_str().unwrap().ends_with('Z'));
        assert_eq!(body["device"]["phoneNumber"], "+123456789001");
    }

    #[tokio::test]
    async fn a_different_tail_reports_a_different_identity() {
        let (status, _, body) =
            retrieve_identifier_ok(r#"{"device":{"phoneNumber":"+123456789002"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Google");
        assert_eq!(body["tac"], "35438509");
        assert!(body["imei"].as_str().unwrap().starts_with("35438509000002"));
        assert_eq!(body["imeisv"], "3543850900000200");
    }

    #[tokio::test]
    async fn triple_zero_tail_reports_the_default_identity() {
        let (status, _, body) =
            retrieve_identifier_ok(r#"{"device":{"phoneNumber":"+123456789000"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Apple");
        assert_eq!(body["tac"], "35692005");
        assert!(body["imei"].as_str().unwrap().starts_with("35692005000000"));
        assert_eq!(body["imeisv"], "3569200500000000");
    }

    #[tokio::test]
    async fn identifier_reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            retrieve_identifier_ok(r#"{"device":{"phoneNumber":"+123456789404"}}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn identifier_non_phone_ids_are_accepted_and_echoed() {
        let (status, _, body) =
            retrieve_identifier_ok(r#"{"device":{"networkAccessIdentifier":"user002@nai"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Google");
        assert_eq!(body["device"]["networkAccessIdentifier"], "user002@nai");
        assert!(body["imei"].as_str().unwrap().starts_with("35438509000002"));
    }

    #[tokio::test]
    async fn identifier_invalid_phone_and_empty_device_are_rejected() {
        let (status, _, body) =
            retrieve_identifier_ok(r#"{"device":{"phoneNumber":"0123"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");

        let (status, _, body) = retrieve_identifier_ok(r#"{"device":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn identifier_no_device_falls_back_to_the_token_subject() {
        // Subject E.164 …002 → Google identity, empty body; echoed.
        let token = mint_token_with_client(RETRIEVE_IDENTIFIER_SCOPE, "+123456789002").await;
        let (status, _, body) = post_retrieve_identifier(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Google");
        assert_eq!(body["device"]["phoneNumber"], "+123456789002");
        assert!(body["imei"].as_str().unwrap().starts_with("35438509000002"));
    }

    #[tokio::test]
    async fn identifier_subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(RETRIEVE_IDENTIFIER_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve_identifier(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn identifier_non_numeric_subject_reports_the_default_and_no_echo() {
        // Default synthetic subject "di-client" has no digits → default identity,
        // and — not being a phone number — is not echoed as a device.
        let (status, _, body) = retrieve_identifier_ok("{}").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manufacturer"], "Apple");
        assert_eq!(body["imeisv"], "3569200500000000");
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn identifier_token_without_the_scope_is_forbidden() {
        // A retrieve-type token must not reach retrieve-identifier.
        let token = mint_token(RETRIEVE_TYPE_SCOPE).await;
        let (status, _, body) = post_retrieve_identifier(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789001"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn identifier_missing_token_is_unauthenticated() {
        let (status, _, body) = post_retrieve_identifier(
            None,
            r#"{"device":{"phoneNumber":"+123456789001"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn identifier_x_correlator_is_echoed() {
        let token = mint_token(RETRIEVE_IDENTIFIER_SCOPE).await;
        let (status, headers, _) = post_retrieve_identifier(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789001"}}"#,
            Some("corr-di-id"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-di-id")
        );
    }
}
