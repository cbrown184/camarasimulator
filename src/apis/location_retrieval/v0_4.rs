//! Location Retrieval **v0.4** (CAMARA Location Retrieval 0.4.0, release r3.2).
//!
//! One endpoint:
//! - `POST /location-retrieval/v0.4/retrieve` — where is the device now?
//!   (operationId `retrieveLocation`).
//!
//! ## What it does
//!
//! The caller optionally submits a `device` (identified by `phoneNumber`,
//! `ipv4Address`, or `ipv6Address`; when omitted the device is the one a
//! three-legged access token authenticated) and an optional `maxAge`. The
//! endpoint returns the device's current location as a circular `area`:
//! `{ "lastLocationTime": …, "area": { "areaType": "CIRCLE", "center": {…},
//! "radius": … } }`, where `radius` expresses the location's accuracy in metres.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `location-retrieval:read` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes:
//!
//! **1. The identifier** (the submitted `device` id — `phoneNumber`, else the
//! IPv4 `publicAddress`, else `ipv6Address` — or, when no `device` is supplied,
//! the access token subject `sub`). Its trailing three digits drive the result:
//!
//! - **Reserved error suffix** (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//!   `…429`, `…500`, `…503`) → the canonical CAMARA error (shared convention,
//!   [`crate::scenarios`]).
//! - **any other input** (the happy-path default) → `200` with a CIRCLE `area`
//!   whose `center` and `radius` are **deterministic from the trailing three
//!   digits**: the `center` is a fixed base point offset by those digits and the
//!   `radius` (accuracy) is `((digits % 10) + 1) * 100` metres (100–1000 m), so
//!   the reported position is reproducible from the input alone.
//!
//! **2. `maxAge`** — the maximum acceptable age (seconds) of the location data.
//! CamaraSim validates its range (`[60, 2147483647]`; a value outside → `400
//! OUT_OF_RANGE`) but always treats its location data as fresh
//! (`lastLocationTime` = now), so a valid `maxAge` never changes the result.
//!
//! Examples: `+123456789012` → a circle centred ~`(51.512, -0.108)` with a
//! `radius` of `300` m; `+123456789404` → `404 NOT_FOUND`; `maxAge: 30` →
//! `400 OUT_OF_RANGE`.

use std::time::{SystemTime, UNIX_EPOCH};

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

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Location
/// Retrieval 0.4.0).
const RETRIEVE_SCOPE: &str = "location-retrieval:read";

/// The minimum acceptable `maxAge` in seconds (CAMARA Location Retrieval schema
/// floor). A smaller value → `400 OUT_OF_RANGE`.
const MIN_MAX_AGE_SECS: i64 = 60;

/// The fixed base point (WGS-84 decimal degrees) CamaraSim reports device
/// locations around. The identifier's trailing digits offset it, so different
/// inputs yield different — but reproducible — positions.
const BASE_LATITUDE: f64 = 51.5;
const BASE_LONGITUDE: f64 = -0.12;

/// Routes for Location Retrieval v0.4, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/location-retrieval/v0.4/retrieve", post(retrieve))
}

/// `POST /retrieve` request body (CAMARA `RetrievalLocationRequest`). `device`
/// is optional (omit it when a three-legged token identifies the device);
/// `maxAge` is optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    device: Option<Device>,
    #[serde(rename = "maxAge")]
    max_age: Option<i64>,
}

/// The CAMARA `Device` object for Location Retrieval. Like its companion
/// Location Verification (and per CAMARA location guidelines) this API does
/// **not** accept `networkAccessIdentifier`, so an unknown identifier field is
/// rejected by `deny_unknown_fields`. CamaraSim keys its cases off the first
/// present identifier, in the precedence order below.
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

/// `POST /location-retrieval/v0.4/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is optional (device optional, maxAge optional): an empty body is
    // an empty request `{}`. A present body must parse as a RetrievalLocationRequest.
    let req: RetrieveRequest = if body.is_empty() {
        RetrieveRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrievalLocationRequest.",
                    &correlator,
                )
            }
        }
    };

    // maxAge, when present, must be in [60, int32::MAX] (CAMARA `maxAge`).
    if let Some(max_age) = req.max_age {
        if max_age < MIN_MAX_AGE_SECS || max_age > i64::from(i32::MAX) {
            return out_of_range(
                "`maxAge` must be between 60 and 2147483647 seconds.",
                &correlator,
            );
        }
    }

    // --- Identifier resolution + reserved-error convention ----------------
    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // --- Result (identifier is the control plane) -------------------------
    let (latitude, longitude, radius) = location_for(&identifier);
    let response = json!({
        "lastLocationTime": rfc3339_utc(now_unix_secs()),
        "area": {
            "areaType": "CIRCLE",
            "center": { "latitude": latitude, "longitude": longitude },
            "radius": radius,
        },
    });

    with_correlator((StatusCode::OK, Json(response)).into_response(), &correlator)
}

/// The simulated location for `identifier` as `(latitude, longitude, radius)`.
///
/// Driven by the identifier's trailing three digits (docs/DESIGN.md §7, `000`
/// when it has none): the `center` is the fixed base point offset north/east by
/// `digits * 0.001` degrees, and the `radius` (the location accuracy, in metres)
/// is `((digits % 10) + 1) * 100` (100–1000 m). Both are therefore deterministic
/// from the input, so a caller can predict the reported position. The largest
/// offset (`999 * 0.001 ≈ 1.0°`) keeps the point well inside the valid
/// latitude/longitude range.
fn location_for(identifier: &str) -> (f64, f64, f64) {
    let digits = scenarios::trailing_three_digits(identifier).unwrap_or(0);
    let latitude = BASE_LATITUDE + f64::from(digits) * 0.001;
    let longitude = BASE_LONGITUDE + f64::from(digits) * 0.001;
    let radius = f64::from((digits % 10) + 1) * 100.0;
    (latitude, longitude, radius)
}

/// Resolve the device identifier for a request: the first present identifier in
/// the supplied `device` (validated when it is a `phoneNumber`), else the token
/// subject (three-legged fallback).
///
/// On failure returns the CAMARA error `Response` to send — 400
/// `INVALID_ARGUMENT` for a malformed `phoneNumber` or a `device` carrying no
/// identifier, 422 `MISSING_IDENTIFIER` when neither a device nor a token subject
/// is present.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                Ok(phone)
            }
            Some(DeviceId::Ipv4(addr)) => Ok(addr),
            Some(DeviceId::Ipv6(addr)) => Ok(addr),
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
            Ok(subject.to_string())
        }
    }
}

/// The identifier CamaraSim reads from a `Device`, kept distinct so the
/// `phoneNumber` E.164 check can be applied. Precedence: phoneNumber, the IPv4
/// `publicAddress`, then ipv6Address.
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

    const HOST: &str = "lr.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn location_is_deterministic_from_the_trailing_digits() {
        // …012 → radius ((12 % 10) + 1) * 100 = 300, centre offset by 12 * 0.001.
        let (lat, lon, radius) = location_for("+123456789012");
        assert_eq!(radius, 300.0);
        assert!((lat - (BASE_LATITUDE + 0.012)).abs() < 1e-9);
        assert!((lon - (BASE_LONGITUDE + 0.012)).abs() < 1e-9);

        // …000 (and no-digit) → smallest radius 100, centre at the base point.
        assert_eq!(location_for("+123456789000"), (BASE_LATITUDE, BASE_LONGITUDE, 100.0));
        assert_eq!(location_for("lr-client"), (BASE_LATITUDE, BASE_LONGITUDE, 100.0));
    }

    #[test]
    fn radius_stays_in_the_100_to_1000_metre_band() {
        // Every input yields an accuracy radius in {100,200,…,1000}.
        for tail in 0..1000u16 {
            let id = format!("+1234567{tail:03}");
            let (lat, lon, radius) = location_for(&id);
            assert!((100.0..=1000.0).contains(&radius), "tail {tail} → radius {radius}");
            assert_eq!(radius % 100.0, 0.0, "tail {tail} → radius {radius} not a 100 m step");
            // Centre stays inside the valid WGS-84 range.
            assert!((-90.0..=90.0).contains(&lat));
            assert!((-180.0..=180.0).contains(&lon));
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
        mint_token_with_client(scope, "lr-client").await
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

    /// POST a JSON body to `/retrieve` with an optional Bearer token and `x-correlator`.
    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/location-retrieval/v0.4/retrieve")
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

    /// Mint a scoped token and call retrieve with the given body.
    async fn retrieve_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn happy_path_returns_a_deterministic_circle() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["area"]["areaType"], "CIRCLE");
        // Location is deterministic from the identifier's trailing digits.
        let (lat, lon, radius) = location_for("+123456789012");
        assert_eq!(body["area"]["radius"].as_f64().unwrap(), radius);
        assert_eq!(body["area"]["center"]["latitude"].as_f64().unwrap(), lat);
        assert_eq!(body["area"]["center"]["longitude"].as_f64().unwrap(), lon);
        assert!(body["lastLocationTime"].as_str().unwrap().ends_with('Z'));
        // Location Retrieval returns a position, not a device echo.
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn different_identifiers_yield_different_locations() {
        let (_, _, a) = retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        let (_, _, b) = retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789034"}}"#).await;
        assert_ne!(a["area"]["radius"], b["area"]["radius"]);
        assert_ne!(a["area"]["center"]["latitude"], b["area"]["center"]["latitude"]);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789404"}}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789429"}}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn ipv4_and_ipv6_identifiers_are_accepted() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.34"}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["area"]["radius"].as_f64().unwrap(), location_for("203.0.113.34").2);

        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"ipv6Address":"2001:db8::034"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["area"]["areaType"], "CIRCLE");
    }

    #[tokio::test]
    async fn empty_body_falls_back_to_the_token_subject() {
        // Subject is an E.164 number → happy-path location, no device required.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["area"]["radius"].as_f64().unwrap(), location_for("+123456789012").2);
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn max_age_below_the_minimum_is_out_of_range() {
        let (status, _, body) = retrieve_ok_token(
            r#"{"device":{"phoneNumber":"+123456789012"},"maxAge":30}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn valid_max_age_is_accepted() {
        let (status, _, body) = retrieve_ok_token(
            r#"{"device":{"phoneNumber":"+123456789012"},"maxAge":120}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["area"]["areaType"], "CIRCLE");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"0123"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = retrieve_ok_token(r#"{"device":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn network_access_identifier_is_not_accepted() {
        // This API does not accept networkAccessIdentifier → unknown field → 400.
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"networkAccessIdentifier":"user@nai"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_retrieve(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_retrieve(
            None,
            r#"{"device":{"phoneNumber":"+123456789012"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"}}"#,
            Some("corr-lr"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-lr")
        );

        let (status, headers, _) = post_retrieve(
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
}
