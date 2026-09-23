//! Device Visit Location **vwip** (CAMARA device-visit-location, work-in-progress
//! spec — no released version yet).
//!
//! One endpoint:
//! - `POST /device-visit-location/vwip/retrieve` — where has a device been during
//!   a caller-supplied time window? (operationId `retrieveDeviceVisitLocation`).
//!
//! ## What it does
//!
//! The caller asks for the places a device visited between `startTime` and
//! `endTime`. The device is identified either by a `device` object in the request
//! body (two-legged / CIBA) or by the identity a three-legged access token
//! authenticated (in which case `device` is omitted). The operator answers with a
//! `geoCodeList` — a list of `{ countryCode, codeType: "PostalCode", codeValue }`
//! geographic codes — a coarse where-has-it-been signal, never precise
//! coordinates.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `device-visit-location:retrieve`
//! scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA (mirrors Connected Network Type / Simple Edge Discovery):
//! the `device` in the body is only meaningful in two-legged auth. In a
//! three-legged token the device is already identified by the token **subject**,
//! so resubmitting it is an error:
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
//! Two control planes drive the answer:
//!
//! 1. **The identifier** (the resolved device id):
//!    - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!      status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!      `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]). This
//!      plane is checked first, so it dominates the time-window and data planes.
//!    - **No visit data** — otherwise, an identifier whose trailing three digits
//!      are `000` (or that has no trailing digits) → `404
//!      DEVICE_VISIT_LOCATION.DATA_NOT_FOUND` (no location was recorded for the
//!      device in the window). This case has its own tail because the success
//!      `geoCodeList` is non-empty (`minItems: 1`), so "no data" cannot be an empty
//!      list — it is the 404 instead.
//!    - **Visited places** — any other tail `d` yields `((d - 1) % 3) + 1`
//!      geographic codes (1–3), the `i`-th on `COUNTRIES[(d + i) % 6]` with a
//!      deterministic postal code, so both the number of places and the countries
//!      are a genuine control plane (the same input always returns the same list).
//!
//! 2. **The time window** (`startTime` / `endTime`, both required, RFC 3339):
//!    - A malformed timestamp → `400 INVALID_ARGUMENT`.
//!    - `endTime` strictly before `startTime` → `400
//!      DEVICE_VISIT_LOCATION.INVALID_END_DATE`.
//!    The window is validated *after* the reserved-error plane but *before* the
//!    data plane, so it too is reachable deterministically from the input.
//!
//! Examples: `+123456789012` over a valid window → `200` with three postal codes
//! (`((12 - 1) % 3) + 1 == 3`); `+123456789001` → one code; `+123456789000` →
//! `404 DATA_NOT_FOUND`; `+123456789404` → `404 NOT_FOUND`; a window with
//! `endTime < startTime` → `400 DEVICE_VISIT_LOCATION.INVALID_END_DATE`.

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

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Device Visit
/// Location).
const RETRIEVE_SCOPE: &str = "device-visit-location:retrieve";

/// The fixed country table (ISO 3166-1 alpha-2). A visited place's country is
/// chosen from this table by the identifier's trailing three digits, so the
/// reported country is deterministic from the device — a genuine control plane
/// (docs/DESIGN.md §7).
const COUNTRIES: [&str; 6] = ["US", "GB", "DE", "FR", "ES", "IT"];

/// Routes for Device Visit Location vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/device-visit-location/vwip/retrieve", post(retrieve))
}

/// `POST /retrieve` request body (CAMARA `RetrieveVisitLocationRequest`).
/// `startTime`/`endTime` are required; `device` is optional — omit it when a
/// three-legged token identifies the device.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    device: Option<Device>,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "endTime")]
    end_time: String,
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

/// `POST /device-visit-location/vwip/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required (startTime/endTime are mandatory).
    let req: RetrieveRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid RetrieveVisitLocationRequest (startTime and endTime are required).",
                &correlator,
            )
        }
    };

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Control plane 1a: a reserved identifier suffix wins over everything else.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Control plane 2: the time window must be well-formed and ordered.
    let (Some(start), Some(end)) = (parse_rfc3339(&req.start_time), parse_rfc3339(&req.end_time))
    else {
        return invalid_argument(
            "`startTime` and `endTime` must be valid RFC 3339 date-times.",
            &correlator,
        );
    };
    if end < start {
        return with_correlator(
            CamaraError::new(
                StatusCode::BAD_REQUEST,
                "DEVICE_VISIT_LOCATION.INVALID_END_DATE",
                "`endTime` must not be earlier than `startTime`.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Control plane 1b: `…000` / no trailing digits → no recorded visit data.
    let d = scenarios::trailing_three_digits(&identifier).unwrap_or(0);
    if d == 0 {
        return with_correlator(
            CamaraError::new(
                StatusCode::NOT_FOUND,
                "DEVICE_VISIT_LOCATION.DATA_NOT_FOUND",
                "No visit location data was found for the device in the requested time window.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Control plane 1c: the visited places, deterministic from the identifier.
    let geo_code_list = visited_geocodes(d);
    let out = json!({ "geoCodeList": geo_code_list });
    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// The list of geographic codes a device with trailing digits `d` (`1..=999`)
/// visited: `((d - 1) % 3) + 1` entries (1–3), the `i`-th on `COUNTRIES[(d + i) %
/// 6]` with a deterministic postal code. Precondition: `d != 0` (the `…000` case
/// is the `DATA_NOT_FOUND` 404, handled by the caller).
fn visited_geocodes(d: u16) -> Vec<Value> {
    let count = ((d - 1) % 3) + 1; // 1..=3
    (0..count)
        .map(|i| {
            let country = COUNTRIES[(d as usize + i as usize) % COUNTRIES.len()];
            json!({
                "countryCode": country,
                "codeType": "PostalCode",
                "codeValue": postal_code(d, i),
            })
        })
        .collect()
}

/// A deterministic 5-digit postal code for the `i`-th visited place of a device
/// with trailing digits `d`. Purely synthetic (no real geography) but stable per
/// `(d, i)` so a given input always returns the same code.
fn postal_code(d: u16, i: u16) -> String {
    let n = (d as u32 * 137 + i as u32 * 971 + 7) % 100_000;
    format!("{n:05}")
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Connected Network Type). See the module docs for the cases.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
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
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                Ok(phone)
            }
            Some(DeviceId::Other(id)) => {
                if subject_is_line {
                    return Err(unprocessable(
                        "UNNECESSARY_IDENTIFIER",
                        "The device is already identified by the access token.",
                        correlator,
                    ));
                }
                Ok(id)
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            if subject_is_line {
                Ok(subject.to_string())
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

/// Parse an RFC 3339 / ISO 8601 date-time to epoch seconds (UTC). Accepts a `Z`
/// or `±HH:MM` offset (or none — assumed UTC) and an optional fractional second
/// (ignored). Returns `None` on any malformed input. Self-contained so CamaraSim
/// needs no date/time dependency (mirrors `region_device_count::v0_2`).
fn parse_rfc3339(s: &str) -> Option<i64> {
    let (date, rest) = s.split_once(['T', 't'])?;
    let mut d = date.splitn(3, '-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (time, offset_secs) = if let Some(t) = rest.strip_suffix(['Z', 'z']) {
        (t, 0i64)
    } else if let Some(idx) = rest.rfind(['+', '-']) {
        let (t, off) = rest.split_at(idx);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let (oh, om) = off[1..].split_once(':')?;
        let oh: i64 = oh.parse().ok()?;
        let om: i64 = om.parse().ok()?;
        if oh > 14 || om >= 60 {
            return None;
        }
        (t, sign * (oh * 3600 + om * 60))
    } else {
        (rest, 0i64)
    };

    let time = time.split_once('.').map_or(time, |(hms, _)| hms);
    let mut hms = time.splitn(3, ':');
    let hh: i64 = hms.next()?.parse().ok()?;
    let mm: i64 = hms.next()?.parse().ok()?;
    let ss: i64 = hms.next().unwrap_or("0").parse().ok()?;
    if !(0..=23).contains(&hh) || !(0..=59).contains(&mm) || !(0..=60).contains(&ss) {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss - offset_secs)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's `days_from_civil`,
/// proleptic Gregorian). Self-contained, no date dependency.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = m as i64;
    let d = d as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "dvl.local:8080";
    const WINDOW: &str = r#""startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-08T00:00:00Z""#;

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn geocode_count_cycles_one_to_three() {
        assert_eq!(visited_geocodes(1).len(), 1); // (0 % 3) + 1
        assert_eq!(visited_geocodes(2).len(), 2);
        assert_eq!(visited_geocodes(3).len(), 3);
        assert_eq!(visited_geocodes(4).len(), 1); // (3 % 3) + 1
        assert_eq!(visited_geocodes(12).len(), 3); // (11 % 3) + 1
    }

    #[test]
    fn geocodes_are_valid_and_deterministic() {
        let a = visited_geocodes(12);
        let b = visited_geocodes(12);
        assert_eq!(a, b, "same input → same list");
        for g in &a {
            assert!(COUNTRIES.contains(&g["countryCode"].as_str().unwrap()));
            assert_eq!(g["codeType"], "PostalCode");
            assert_eq!(g["codeValue"].as_str().unwrap().len(), 5);
            assert!(g["codeValue"].as_str().unwrap().bytes().all(|b| b.is_ascii_digit()));
        }
    }

    #[test]
    fn rfc3339_parses_and_orders() {
        let s = parse_rfc3339("2024-01-01T00:00:00Z").unwrap();
        let e = parse_rfc3339("2024-01-08T00:00:00Z").unwrap();
        assert!(e > s);
        assert_eq!(e - s, 7 * 86_400);
        // Offsets normalise to UTC.
        assert_eq!(
            parse_rfc3339("2024-01-01T02:00:00+02:00"),
            parse_rfc3339("2024-01-01T00:00:00Z")
        );
        assert!(parse_rfc3339("not-a-date").is_none());
        assert!(parse_rfc3339("2024-13-01T00:00:00Z").is_none());
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(!is_valid_e164("0123"));
        assert!(!is_valid_e164("dvl-client"));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "dvl-client").await
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
            .uri("/device-visit-location/vwip/retrieve")
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

    /// Build a body with the given device fragment and the standard valid window.
    fn body_with(device_frag: &str) -> String {
        format!("{{{device_frag},{WINDOW}}}")
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn returns_a_geocode_list() {
        // …012 → 3 places.
        let (status, _, body) =
            call_ok(&body_with(r#""device":{"phoneNumber":"+123456789012"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        let list = body["geoCodeList"].as_array().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0]["codeType"], "PostalCode");
        assert!(COUNTRIES.contains(&list[0]["countryCode"].as_str().unwrap()));
    }

    #[tokio::test]
    async fn single_place_for_a_one_count_tail() {
        // …001 → ((1-1)%3)+1 == 1 place.
        let (status, _, body) =
            call_ok(&body_with(r#""device":{"phoneNumber":"+123456789001"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["geoCodeList"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn works_for_a_non_phone_identifier() {
        // ipv6-keyed, tail …002 → 2 places.
        let (status, _, body) =
            call_ok(&body_with(r#""device":{"ipv6Address":"2001:db8::002"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["geoCodeList"].as_array().unwrap().len(), 2);
    }

    // --- Control plane: no data / reserved errors --------------------------

    #[tokio::test]
    async fn zero_tail_is_data_not_found() {
        let (status, _, body) =
            call_ok(&body_with(r#""device":{"phoneNumber":"+123456789000"}"#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "DEVICE_VISIT_LOCATION.DATA_NOT_FOUND");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            call_ok(&body_with(r#""device":{"phoneNumber":"+123456789404"}"#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            call_ok(&body_with(r#""device":{"phoneNumber":"+123456789429"}"#)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn reserved_suffix_wins_over_a_bad_window() {
        // …404 with an inverted window still returns the reserved 404, because the
        // identifier plane is checked before the time window.
        let body = r#"{"device":{"phoneNumber":"+123456789404"},"startTime":"2024-01-08T00:00:00Z","endTime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(out["code"], "NOT_FOUND");
    }

    // --- Control plane: the time window ------------------------------------

    #[tokio::test]
    async fn end_before_start_is_invalid_end_date() {
        let body = r#"{"device":{"phoneNumber":"+123456789012"},"startTime":"2024-01-08T00:00:00Z","endTime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "DEVICE_VISIT_LOCATION.INVALID_END_DATE");
    }

    #[tokio::test]
    async fn malformed_time_is_invalid_argument() {
        let body = r#"{"device":{"phoneNumber":"+123456789012"},"startTime":"nope","endTime":"2024-01-08T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_time_fields_are_rejected() {
        // startTime/endTime are required — a device-only body is invalid.
        let (status, _, out) = call_ok(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …012 → 3 places.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let body = format!("{{{WINDOW}}}");
        let (status, _, out) = post_retrieve(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["geoCodeList"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn resubmitting_the_device_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, out) =
            post_retrieve(Some(&token), &body_with(r#""device":{"phoneNumber":"+123456789012"}"#), None)
                .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        let body = format!("{{{WINDOW}}}");
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, out) = call_ok(&body_with(r#""device":{}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, out) = call_ok(&body_with(r#""device":{"phoneNumber":"0123"}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, out) =
            call_ok(&body_with(r#""device":{"phoneNumber":"+123456789012"},"x":1"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_body_is_rejected() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, out) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, out) =
            post_retrieve(Some(&token), &body_with(r#""device":{"phoneNumber":"+123456789012"}"#), None)
                .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(out["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, out) =
            post_retrieve(None, &body_with(r#""device":{"phoneNumber":"+123456789012"}"#), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(out["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) = post_retrieve(
            Some(&token),
            &body_with(r#""device":{"phoneNumber":"+123456789012"}"#),
            Some("corr-dvl"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-dvl")
        );
        // Business error echoes too.
        let (status, headers, _) = post_retrieve(
            Some(&token),
            &body_with(r#""device":{"phoneNumber":"+123456789000"}"#),
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
