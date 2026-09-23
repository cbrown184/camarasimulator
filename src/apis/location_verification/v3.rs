//! Location Verification **v3** (CAMARA Location Verification 3.0.0, release r3.2).
//!
//! One endpoint:
//! - `POST /location-verification/v3/verify` — is the device currently located
//!   within the requested circular area? (operationId `verifyLocation`).
//!
//! ## What it does
//!
//! The caller submits an `area` (currently only a `CIRCLE` — a `center`
//! lat/long plus a `radius` in metres) and, optionally, a `device` (identified
//! by `phoneNumber`, `ipv4Address`, or `ipv6Address`). When `device` is omitted
//! the device is the one a three-legged access token authenticated. The endpoint
//! answers a *verdict*, never a coordinate:
//! `{ "verificationResult": "TRUE"|"FALSE"|"PARTIAL", "lastLocationTime": … }`,
//! with a `matchRate` (1–99) added when the result is `PARTIAL`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `location-verification:verify`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes:
//!
//! **1. The identifier** (the submitted `device` id — `phoneNumber`, else the
//! IPv4 `publicAddress`, else `ipv6Address` — or, when no `device` is supplied,
//! the access token subject `sub`). Its trailing three digits select the verdict:
//!
//! - **Reserved error suffix** (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//!   `…429`, `…500`, `…503`) → the canonical CAMARA error (shared convention,
//!   [`crate::scenarios`]).
//! - **`…000`** → `verificationResult: "FALSE"` (device is elsewhere).
//! - **odd trailing digits** → `verificationResult: "PARTIAL"` with
//!   `matchRate = (digits % 99) + 1` (the device is partly inside the area).
//! - **any other input** (the happy-path default, including a non-numeric subject
//!   with no trailing digits) → `verificationResult: "TRUE"` (device is inside).
//!
//! **2. The area** (`radius`). CamaraSim enforces a documented regulatory minimum
//! radius of [`MIN_RADIUS_METRES`] m: a `radius` below it → `422
//! LOCATION_VERIFICATION.INVALID_AREA`. A `center` outside the valid
//! latitude/longitude range, or a `radius` below the schema minimum of 1 m, →
//! `400 OUT_OF_RANGE`; an `areaType` other than `CIRCLE` → `400 INVALID_ARGUMENT`.
//!
//! Examples: `+123456789012` in a 3000 m circle → `TRUE`; `+123456789011` →
//! `PARTIAL` (`matchRate` 12); `+123456789000` → `FALSE`; `+123456789404` →
//! `404 NOT_FOUND`; any device in a 500 m circle → `422 INVALID_AREA`.

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

/// The OAuth2 scope the `POST /verify` endpoint requires (CAMARA Location
/// Verification 3.0.0).
const VERIFY_SCOPE: &str = "location-verification:verify";

/// CamaraSim's documented regulatory minimum circle radius, in metres. A
/// `radius` below this is rejected with `422 LOCATION_VERIFICATION.INVALID_AREA`
/// — the CAMARA spec allows operators to enforce a local minimum larger than the
/// schema floor of 1 m (its example is ~1000 m); CamaraSim fixes it at 2000 m so
/// the boundary is deterministic and discoverable.
const MIN_RADIUS_METRES: f64 = 2000.0;

/// Routes for Location Verification v3, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/location-verification/v3/verify", post(verify))
}

/// `POST /verify` request body (CAMARA `VerifyLocationRequest`). `device` is
/// optional (omit it when a three-legged token identifies the device); `area` is
/// required.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyRequest {
    device: Option<Device>,
    area: Area,
    #[serde(rename = "maxAge")]
    max_age: Option<i64>,
}

/// The CAMARA `Device` object for Location Verification. Unlike the device-status
/// APIs this API does **not** accept `networkAccessIdentifier` (CAMARA guidelines
/// disallow it here), so an unknown identifier field is rejected by
/// `deny_unknown_fields`. CamaraSim keys its cases off the first present
/// identifier, in the precedence order below.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Device {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
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

/// The CAMARA `Area`. The discriminator is `areaType`; only `CIRCLE` is
/// supported, so `center`/`radius` are parsed leniently and their presence is
/// validated in the handler for a precise error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Area {
    #[serde(rename = "areaType")]
    area_type: String,
    center: Option<Point>,
    radius: Option<f64>,
}

/// A CAMARA `Point` — a WGS-84 latitude/longitude in decimal degrees.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    latitude: f64,
    longitude: f64,
}

/// `POST /location-verification/v3/verify`.
async fn verify(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(VERIFY_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `area` is required, so the body must be present and parse.
    let req: VerifyRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid VerifyLocationRequest (`area` is required).",
                &correlator,
            )
        }
    };

    // --- Area validation (syntactic 400s first) ---------------------------
    if req.area.area_type != "CIRCLE" {
        return invalid_argument(
            "Only `CIRCLE` `areaType` is supported.",
            &correlator,
        );
    }
    let Some(center) = &req.area.center else {
        return invalid_argument("`area.center` is required for a CIRCLE.", &correlator);
    };
    let Some(radius) = req.area.radius else {
        return invalid_argument("`area.radius` is required for a CIRCLE.", &correlator);
    };
    if !(-90.0..=90.0).contains(&center.latitude)
        || !(-180.0..=180.0).contains(&center.longitude)
    {
        return out_of_range(
            "`area.center` latitude must be in [-90, 90] and longitude in [-180, 180].",
            &correlator,
        );
    }
    if radius < 1.0 {
        return out_of_range("`area.radius` must be at least 1 metre.", &correlator);
    }

    // maxAge, when present, must be a non-negative int32 (CAMARA `maxAge`).
    if let Some(max_age) = req.max_age {
        if max_age < 0 || max_age > i64::from(i32::MAX) {
            return out_of_range(
                "`maxAge` must be between 0 and 2147483647 seconds.",
                &correlator,
            );
        }
    }

    // --- Identifier resolution + reserved-error convention ----------------
    let (identifier, device_echo) = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(resolved) => resolved,
        Err(resp) => return resp,
    };
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // --- Regulatory area check (business 422) -----------------------------
    if radius < MIN_RADIUS_METRES {
        return with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "LOCATION_VERIFICATION.INVALID_AREA",
                format!(
                    "The requested area is invalid: a circle radius must be at least {} metres.",
                    MIN_RADIUS_METRES as i64
                ),
            )
            .into_response(),
            &correlator,
        );
    }

    // --- Result (identifier is the control plane) -------------------------
    let (result, match_rate) = verdict(&identifier);
    let mut response = json!({
        "verificationResult": result,
        "lastLocationTime": rfc3339_utc(now_unix_secs()),
    });
    if let Some(rate) = match_rate {
        response["matchRate"] = json!(rate);
    }
    // The device is echoed only when the caller supplied one (2-legged), per the
    // CAMARA `VerifyLocationResponse.device` semantics.
    if let Some(echo) = device_echo {
        response["device"] = echo;
    }

    with_correlator((StatusCode::OK, Json(response)).into_response(), &correlator)
}

/// The verification verdict for `identifier`, driven by its trailing three digits
/// (docs/DESIGN.md §7): `…000` → `FALSE`; odd → `PARTIAL` with a `matchRate` of
/// `(digits % 99) + 1` (always 1–99); anything else (incl. an identifier with no
/// trailing digits) → `TRUE`. The second tuple element is the `matchRate`, set
/// only for `PARTIAL`.
fn verdict(identifier: &str) -> (&'static str, Option<u16>) {
    match scenarios::trailing_three_digits(identifier) {
        Some(0) => ("FALSE", None),
        Some(n) if n % 2 == 1 => ("PARTIAL", Some((n % 99) + 1)),
        _ => ("TRUE", None),
    }
}

/// Resolve the device identifier for a request and, when a `device` was
/// supplied, the single-identifier echo for `VerifyLocationResponse.device`.
///
/// The identifier is the first present identifier in the supplied `device`
/// (validated when it is a `phoneNumber`), else the token subject (three-legged
/// fallback). On failure returns the CAMARA error `Response` to send — 400
/// `INVALID_ARGUMENT` for a malformed `phoneNumber` or a `device` carrying no
/// identifier, 422 `MISSING_IDENTIFIER` when neither a device nor a token subject
/// is present.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<(String, Option<Value>), Response> {
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
                Ok((phone, Some(echo)))
            }
            Some(DeviceId::Ipv4(addr)) => {
                let echo = json!({ "ipv4Address": { "publicAddress": addr } });
                Ok((addr, Some(echo)))
            }
            Some(DeviceId::Ipv6(addr)) => {
                let echo = json!({ "ipv6Address": addr });
                Ok((addr.clone(), Some(echo)))
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
            Ok((subject.to_string(), None))
        }
    }
}

/// The identifier CamaraSim reads from a `Device`, kept distinct so the
/// `phoneNumber` E.164 check and the correct echo key can be applied. Precedence:
/// phoneNumber, the IPv4 `publicAddress`, then ipv6Address.
enum DeviceId {
    PhoneNumber(String),
    Ipv4(String),
    Ipv6(String),
}

/// The first present identifier of a `Device`, in precedence order, or `None`
/// when the device carries no identifier at all (`minProperties: 1` violated).
fn device_identifier(device: &Device) -> Option<DeviceId> {
    if let Some(phone) = &device.phone_number {
        return Some(DeviceId::PhoneNumber(phone.clone()));
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
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed. `OUT_OF_RANGE`
/// is the CAMARA Commonalities code for a syntactically valid value outside its
/// allowed range (declared by this API's spec alongside `INVALID_ARGUMENT`).
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
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as RFC 3339, e.g.
/// `2026-08-03T14:27:08Z`. Second precision — the CAMARA schema requires RFC
/// 3339 with a time zone but not sub-second digits. Self-contained (no date-time
/// dependency) via the civil-from-days algorithm below.
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a day count since 1970-01-01 into a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian). Month and day are
/// 1-based.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day-of-era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp = (5 * doy + 2) / 153; // month, shifted so March = 0 [0, 11]
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

    const HOST: &str = "lv.local:8080";

    // A valid CIRCLE area, radius above the regulatory minimum, as a JSON snippet.
    const OK_AREA: &str = r#""area":{"areaType":"CIRCLE","center":{"latitude":51.5,"longitude":-0.12},"radius":3000}"#;

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn verdict_is_driven_by_the_trailing_digits() {
        // …000 → FALSE, no matchRate.
        assert_eq!(verdict("+123456789000"), ("FALSE", None));
        // odd tail → PARTIAL with matchRate = (n % 99) + 1.
        assert_eq!(verdict("+123456789011"), ("PARTIAL", Some(12)));
        assert_eq!(verdict("+123456789001"), ("PARTIAL", Some(2)));
        // even non-zero tail → TRUE.
        assert_eq!(verdict("+123456789012"), ("TRUE", None));
        // no trailing digits → default happy path (TRUE).
        assert_eq!(verdict("lv-client"), ("TRUE", None));
    }

    #[test]
    fn partial_match_rate_stays_in_range() {
        // Every odd tail yields a matchRate in 1..=99.
        for tail in (1..1000).step_by(2) {
            let id = format!("+1234567{tail:03}");
            if let ("PARTIAL", Some(rate)) = verdict(&id) {
                assert!((1..=99).contains(&rate), "tail {tail} → rate {rate}");
            }
        }
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
            phone_number: Some("+123456789012".into()),
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.12".into()),
                private_address: None,
                public_port: None,
            }),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&phone), Some(DeviceId::PhoneNumber(p)) if p == "+123456789012"));

        let ipv4 = Device {
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.12".into()),
                private_address: None,
                public_port: None,
            }),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&ipv4), Some(DeviceId::Ipv4(p)) if p == "203.0.113.12"));

        let ipv6 = Device {
            ipv6_address: Some("2001:db8::11".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&ipv6), Some(DeviceId::Ipv6(p)) if p == "2001:db8::11"));

        assert!(device_identifier(&Device::default()).is_none());
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
        mint_token_with_client(scope, "lv-client").await
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

    /// POST a JSON body to `/verify` with an optional Bearer token and `x-correlator`.
    async fn post_verify(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/location-verification/v3/verify")
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

    /// Mint a scoped token and call verify with the given body.
    async fn verify_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(VERIFY_SCOPE).await;
        post_verify(Some(&token), body, None).await
    }

    /// Build a request body with the given device fragment and the OK area.
    fn body_with_device(device: &str) -> String {
        format!(r#"{{"device":{device},{OK_AREA}}}"#)
    }

    #[tokio::test]
    async fn device_inside_area_verifies_true() {
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"phoneNumber":"+123456789012"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["verificationResult"], "TRUE");
        assert!(body.get("matchRate").is_none());
        assert!(body["lastLocationTime"].as_str().unwrap().ends_with('Z'));
        // A supplied device is echoed back.
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn triple_zero_tail_verifies_false() {
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"phoneNumber":"+123456789000"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["verificationResult"], "FALSE");
        assert!(body.get("matchRate").is_none());
    }

    #[tokio::test]
    async fn odd_tail_verifies_partial_with_match_rate() {
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"phoneNumber":"+123456789011"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["verificationResult"], "PARTIAL");
        assert_eq!(body["matchRate"], 12);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"phoneNumber":"+123456789404"}"#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"phoneNumber":"+123456789422"}"#)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn ipv4_and_ipv6_identifiers_are_accepted_and_echoed() {
        // ipv4 publicAddress ending …000 → FALSE, echoed as ipv4Address.
        let (status, _, body) = verify_ok_token(&body_with_device(
            r#"{"ipv4Address":{"publicAddress":"203.0.113.000"}}"#,
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["verificationResult"], "FALSE");
        assert_eq!(body["device"]["ipv4Address"]["publicAddress"], "203.0.113.000");

        // ipv6 ending …012 → TRUE, echoed as ipv6Address.
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"ipv6Address":"2001:db8::012"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["verificationResult"], "TRUE");
        assert_eq!(body["device"]["ipv6Address"], "2001:db8::012");
    }

    #[tokio::test]
    async fn radius_below_regulatory_minimum_is_invalid_area() {
        let body = format!(
            r#"{{"device":{{"phoneNumber":"+123456789012"}},"area":{{"areaType":"CIRCLE","center":{{"latitude":51.5,"longitude":-0.12}},"radius":500}}}}"#
        );
        let (status, _, body) = verify_ok_token(&body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "LOCATION_VERIFICATION.INVALID_AREA");
    }

    #[tokio::test]
    async fn radius_below_schema_minimum_is_out_of_range() {
        let body = r#"{"device":{"phoneNumber":"+123456789012"},"area":{"areaType":"CIRCLE","center":{"latitude":51.5,"longitude":-0.12},"radius":0}}"#;
        let (status, _, body) = verify_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn center_out_of_range_is_out_of_range() {
        let body = r#"{"device":{"phoneNumber":"+123456789012"},"area":{"areaType":"CIRCLE","center":{"latitude":123.0,"longitude":-0.12},"radius":3000}}"#;
        let (status, _, body) = verify_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn non_circle_area_type_is_invalid_argument() {
        let body = r#"{"device":{"phoneNumber":"+123456789012"},"area":{"areaType":"POLYGON"}}"#;
        let (status, _, body) = verify_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_area_is_invalid_argument() {
        let body = r#"{"device":{"phoneNumber":"+123456789012"}}"#;
        let (status, _, body) = verify_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn out_of_range_max_age_is_rejected() {
        let body = format!(r#"{{"device":{{"phoneNumber":"+123456789012"}},{OK_AREA},"maxAge":-1}}"#);
        let (status, _, body) = verify_ok_token(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn valid_max_age_is_accepted() {
        let body = format!(r#"{{"device":{{"phoneNumber":"+123456789012"}},{OK_AREA},"maxAge":60}}"#);
        let (status, _, body) = verify_ok_token(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["verificationResult"], "TRUE");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"phoneNumber":"0123"}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = verify_ok_token(&body_with_device("{}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn network_access_identifier_is_not_accepted() {
        // This API does not accept networkAccessIdentifier → unknown field → 400.
        let (status, _, body) =
            verify_ok_token(&body_with_device(r#"{"networkAccessIdentifier":"user@nai"}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject() {
        // Subject is an E.164 number with an even tail → TRUE, no device echo.
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789012").await;
        let body = format!(r#"{{{OK_AREA}}}"#);
        let (status, _, resp) = post_verify(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["verificationResult"], "TRUE");
        assert!(resp.get("device").is_none());
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789503").await;
        let body = format!(r#"{{{OK_AREA}}}"#);
        let (status, _, resp) = post_verify(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn non_numeric_subject_without_device_verifies_true_by_default() {
        // Default synthetic subject "lv-client" has no digits → happy-path TRUE.
        let body = format!(r#"{{{OK_AREA}}}"#);
        let (status, _, resp) = verify_ok_token(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["verificationResult"], "TRUE");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_verify(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_verify(
            None,
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(VERIFY_SCOPE).await;
        let (status, headers, _) = post_verify(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"+123456789012"}"#),
            Some("corr-lv"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-lv")
        );

        let (status, headers, _) = post_verify(
            Some(&token),
            &body_with_device(r#"{"phoneNumber":"0123"}"#),
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
