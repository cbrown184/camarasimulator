//! Most Frequent Location **vwip** (CAMARA MostFrequentLocation, wip).
//!
//! One endpoint:
//! - `POST /most-frequent-location/vwip/verify` — how frequently does the device
//!   reside within the supplied geographic area? (operationId
//!   `verifyFrequentLocation`).
//!
//! ## What it does
//!
//! The caller submits a `geoReference` (a `COVERAGE_ZONE` lat/long point or a
//! `POSTAL_CODE`) and a device — identified either by a `device` object in the
//! request body (two-legged / CIBA) or by the identity a three-legged access
//! token authenticated (in which case `device` is omitted). The operator answers
//! `{ "score": 0..=100 }` — how often the device is present in that area, from
//! `0` (never) to `100` (almost always), never the device's real location.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `most-frequent-location:verify`
//! scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA (mirrors Connected Network Type). The `device` in the body
//! is only meaningful in two-legged auth; in a three-legged token the device is
//! already identified by the token **subject**:
//!
//! - `device` carries an identifier **and** the token subject is an E.164 line →
//!   `422 UNNECESSARY_IDENTIFIER`.
//! - `device` carries an identifier, subject not a line → that identifier is used
//!   (two-legged). Precedence: `phoneNumber` → `networkAccessIdentifier` → the
//!   IPv4 `publicAddress` → `ipv6Address`.
//! - `device` absent, subject is an E.164 line → the subject is the identifier.
//! - `device` absent **and** subject not a line → `422 MISSING_IDENTIFIER`.
//! - `device` present but carrying **no** identifier → `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two planes drive the answer:
//!
//! - **`geoReference`** is validated first: a `COVERAGE_ZONE` `latitude` /
//!   `longitude` out of range → `400 OUT_OF_RANGE`; a `POSTAL_CODE` equal to the
//!   reserved [`RESERVED_INVALID_POSTAL_CODE`] → `400
//!   MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID`; a structurally-wrong
//!   `geoReference` (unknown `type`, missing/foreign fields) → `400
//!   INVALID_ARGUMENT`.
//! - **Reserved error suffix** — once the identifier is resolved, trailing three
//!   digits naming a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`) → that canonical CAMARA error
//!   (shared [`crate::scenarios`]). `…422` maps to `SERVICE_NOT_APPLICABLE`,
//!   the API's own service-level 422.
//! - **Score** — otherwise the score is `(identifier trailing three digits +
//!   area offset) % 101`, so **both** the device and the `geoReference` are
//!   genuine control planes: the same device flips its score as the area changes,
//!   and vice-versa. The area offset is `round(|lat|)+round(|long|)` for a
//!   `COVERAGE_ZONE` and the postal code's trailing three digits for a
//!   `POSTAL_CODE`. A `…000` (or no-digit) identifier over a zero-offset area
//!   scores `0` ("never present").
//!
//! Examples: `+123456789000` over `COVERAGE_ZONE (0,0)` → `score 0`;
//! `+123456789050` over `COVERAGE_ZONE (0,0)` → `score 50`; `+123456789404` →
//! `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /verify` endpoint requires (CAMARA
/// MostFrequentLocation).
const VERIFY_SCOPE: &str = "most-frequent-location:verify";

/// A reserved `postalCode` value that deterministically selects the API's own
/// `400 MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID` case ("does not exist or
/// is invalid"), so that error is reachable from the input alone (DESIGN §7).
const RESERVED_INVALID_POSTAL_CODE: &str = "00000";

/// The largest score the endpoint returns (CAMARA `Score`: `0..=100`).
const MAX_SCORE: u16 = 100;

/// Routes for Most Frequent Location vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/most-frequent-location/vwip/verify", post(verify))
}

/// `POST /verify` request body (CAMARA `VerifyFrequentLocationRequest`).
/// `device` is optional (omit it when a three-legged token identifies the
/// device); `geoReference` is required.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyRequest {
    device: Option<Device>,
    #[serde(rename = "geoReference")]
    geo_reference: GeoReferenceRaw,
}

/// The CAMARA `GeoReference` discriminated object, read flat so CamaraSim can
/// give precise validation errors. `type` is the discriminator; the remaining
/// fields belong to exactly one variant (`COVERAGE_ZONE` → `latitude`/
/// `longitude`; `POSTAL_CODE` → `postalCode`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeoReferenceRaw {
    #[serde(rename = "type")]
    ref_type: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    #[serde(rename = "postalCode")]
    postal_code: Option<String>,
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

/// A validated geographic reference (one of the two `GeoReference` variants).
enum GeoArea {
    CoverageZone { latitude: f64, longitude: f64 },
    PostalCode { postal_code: String },
}

impl GeoArea {
    /// The area's contribution to the score: `round(|lat|)+round(|long|)` for a
    /// coverage zone, or the postal code's trailing three digits. Bounded well
    /// below `u16::MAX` (lat/long ≤ 270, postal ≤ 999), so no overflow.
    fn offset(&self) -> u16 {
        match self {
            GeoArea::CoverageZone { latitude, longitude } => {
                (latitude.round().abs() as u32 + longitude.round().abs() as u32) as u16
            }
            GeoArea::PostalCode { postal_code } => {
                scenarios::trailing_three_digits(postal_code).unwrap_or(0)
            }
        }
    }
}

/// `POST /most-frequent-location/vwip/verify`.
async fn verify(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(VERIFY_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required (`geoReference` is mandatory), so it must parse.
    let req: VerifyRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid VerifyFrequentLocationRequest.",
                &correlator,
            )
        }
    };

    // Validate the geographic reference (request-level input validation first).
    let area = match validate_geo(&req.geo_reference, &correlator) {
        Ok(area) => area,
        Err(resp) => return resp,
    };

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let score = frequent_location_score(&identifier, &area);
    with_correlator(
        (StatusCode::OK, Json(json!({ "score": score }))).into_response(),
        &correlator,
    )
}

/// The presence score for `identifier` over `area`: `(identifier trailing three
/// digits + area offset) % 101`, in `0..=100`. Both inputs are genuine control
/// planes (DESIGN §7) — changing either moves the score.
fn frequent_location_score(identifier: &str, area: &GeoArea) -> u16 {
    let id = scenarios::trailing_three_digits(identifier).unwrap_or(0);
    (id + area.offset()) % (MAX_SCORE + 1)
}

/// Validate a raw `GeoReference` into a [`GeoArea`], or an error response.
///
/// `COVERAGE_ZONE` requires `latitude`/`longitude` in range (else 400
/// OUT_OF_RANGE) and forbids `postalCode`; `POSTAL_CODE` requires a non-empty
/// `postalCode` (the reserved `00000` → 400 POSTAL_CODE_NOT_VALID) and forbids
/// `latitude`/`longitude`. Any other `type` or shape → 400 INVALID_ARGUMENT.
fn validate_geo(raw: &GeoReferenceRaw, correlator: &Option<HeaderValue>) -> Result<GeoArea, Response> {
    match raw.ref_type.as_str() {
        "COVERAGE_ZONE" => {
            if raw.postal_code.is_some() {
                return Err(invalid_argument(
                    "`postalCode` is not valid for a COVERAGE_ZONE geoReference.",
                    correlator,
                ));
            }
            let (Some(lat), Some(long)) = (raw.latitude, raw.longitude) else {
                return Err(invalid_argument(
                    "A COVERAGE_ZONE geoReference requires `latitude` and `longitude`.",
                    correlator,
                ));
            };
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&long) {
                return Err(out_of_range(
                    "`latitude` must be in [-90, 90] and `longitude` in [-180, 180].",
                    correlator,
                ));
            }
            Ok(GeoArea::CoverageZone { latitude: lat, longitude: long })
        }
        "POSTAL_CODE" => {
            if raw.latitude.is_some() || raw.longitude.is_some() {
                return Err(invalid_argument(
                    "`latitude`/`longitude` are not valid for a POSTAL_CODE geoReference.",
                    correlator,
                ));
            }
            let Some(code) = raw.postal_code.clone() else {
                return Err(invalid_argument(
                    "A POSTAL_CODE geoReference requires `postalCode`.",
                    correlator,
                ));
            };
            if code.trim().is_empty() {
                return Err(invalid_argument("`postalCode` must not be empty.", correlator));
            }
            if code == RESERVED_INVALID_POSTAL_CODE {
                return Err(with_correlator(
                    CamaraError::new(
                        StatusCode::BAD_REQUEST,
                        "MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID",
                        "Requested postal code value does not exist or is invalid.",
                    )
                    .into_response(),
                    correlator,
                ));
            }
            Ok(GeoArea::PostalCode { postal_code: code })
        }
        other => Err(invalid_argument(
            &format!("Unknown geoReference type `{other}` (expected COVERAGE_ZONE or POSTAL_CODE)."),
            correlator,
        )),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "mfl.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn score_combines_identifier_and_area() {
        let zero = GeoArea::CoverageZone { latitude: 0.0, longitude: 0.0 };
        // …000 over a zero-offset area → 0 (never present).
        assert_eq!(frequent_location_score("+123456789000", &zero), 0);
        // …050 over a zero-offset area → 50.
        assert_eq!(frequent_location_score("+123456789050", &zero), 50);
        // …100 over a zero-offset area → 100 (max).
        assert_eq!(frequent_location_score("+123456789100", &zero), 100);
        // Wrap past 100: …999 → 999 % 101 == 90.
        assert_eq!(frequent_location_score("+123456789999", &zero), 90);
        // No trailing digits → 0 contribution.
        assert_eq!(frequent_location_score("mfl-client", &zero), 0);
    }

    #[test]
    fn area_is_a_genuine_second_plane() {
        // The SAME device scores differently as the area changes.
        let a = GeoArea::CoverageZone { latitude: 10.0, longitude: 5.0 }; // offset 15
        let b = GeoArea::PostalCode { postal_code: "SW1A 020".into() }; // offset 20
        assert_eq!(frequent_location_score("+123456789000", &a), 15);
        assert_eq!(frequent_location_score("+123456789000", &b), 20);
        assert_eq!(frequent_location_score("+123456789010", &a), 25);
    }

    #[test]
    fn every_score_is_within_the_camara_range() {
        let area = GeoArea::CoverageZone { latitude: 90.0, longitude: 180.0 };
        for tail in 0..=999u16 {
            let id = format!("+12345678{tail:04}");
            assert!(frequent_location_score(&id, &area) <= MAX_SCORE);
        }
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("mfl-client"));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "mfl-client").await
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

    /// POST to `/verify` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_verify(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/most-frequent-location/vwip/verify")
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
        let token = mint_token(VERIFY_SCOPE).await;
        post_verify(Some(&token), body, None).await
    }

    // --- Two-legged (submitted device) success cases ----------------------

    #[tokio::test]
    async fn returns_a_score_for_a_coverage_zone() {
        // …050 over (0,0) → score 50.
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["score"], 50);
    }

    #[tokio::test]
    async fn returns_a_score_for_a_postal_code() {
        // …000 identifier + postalCode trailing digits 020 → score 20.
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789000"},"geoReference":{"type":"POSTAL_CODE","postalCode":"SW1A 020"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["score"], 20);
    }

    #[tokio::test]
    async fn the_area_moves_the_score() {
        // Same device, two different coverage zones → different scores.
        let (_, _, a) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789000"},"geoReference":{"type":"COVERAGE_ZONE","latitude":10.0,"longitude":5.0}}"#,
        )
        .await;
        assert_eq!(a["score"], 15);
        let (_, _, b) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789000"},"geoReference":{"type":"COVERAGE_ZONE","latitude":40.0,"longitude":2.0}}"#,
        )
        .await;
        assert_eq!(b["score"], 42);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let base = r#""geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}"#;
        let (status, _, body) =
            call_ok(&format!(r#"{{"device":{{"phoneNumber":"+123456789404"}},{base}}}"#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // …422 → the API's own SERVICE_NOT_APPLICABLE (shared convention).
        let (status, _, body) =
            call_ok(&format!(r#"{{"device":{{"phoneNumber":"+123456789422"}},{base}}}"#)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");

        let (status, _, body) =
            call_ok(&format!(r#"{{"device":{{"phoneNumber":"+123456789429"}},{base}}}"#)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …050 over (0,0) → score 50.
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789050").await;
        let (status, _, body) = post_verify(
            Some(&token),
            r#"{"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["score"], 50);
    }

    #[tokio::test]
    async fn resubmitting_the_device_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789012").await;
        let (status, _, body) = post_verify(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_device_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) =
            call_ok(r#"{"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#)
                .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn empty_device_object_is_rejected() {
        let (status, _, body) = call_ok(
            r#"{"device":{},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- geoReference validation ------------------------------------------

    #[tokio::test]
    async fn latitude_out_of_range_is_out_of_range() {
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"COVERAGE_ZONE","latitude":100.0,"longitude":0.0}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn reserved_postal_code_is_not_valid() {
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"POSTAL_CODE","postalCode":"00000"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID");
    }

    #[tokio::test]
    async fn coverage_zone_missing_coordinates_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_geo_reference_type_is_invalid_argument() {
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"POLYGON"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_geo_reference_is_invalid_argument() {
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn postal_code_with_foreign_fields_is_invalid_argument() {
        // latitude belongs to COVERAGE_ZONE, not POSTAL_CODE.
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"POSTAL_CODE","postalCode":"SW1A","latitude":0.0}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0},"x":1}"#,
        )
        .await;
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
        let (status, _, body) = post_verify(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
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
            r#"{"device":{"phoneNumber":"+123456789012"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(VERIFY_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_verify(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789000"},"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
            Some("corr-mfl"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-mfl")
        );
        // Business error (unresolved identifier).
        let (status, headers, _) = post_verify(
            Some(&token),
            r#"{"geoReference":{"type":"COVERAGE_ZONE","latitude":0.0,"longitude":0.0}}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
