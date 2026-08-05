//! Device Data Volume **vwip** (CAMARA Device Data Volume, work-in-progress).
//!
//! Two endpoints:
//! - `POST /device-data-volume/vwip/retrieve` — which coarse data-volume category
//!   (`<200MiB`/`<1GiB`/`<5GiB`/`>=5GiB`) has the device consumed?
//!   (operationId `retrieveDataVolume`).
//! - `POST /device-data-volume/vwip/check` — does the device's **remaining** data
//!   volume exceed a caller-supplied threshold? (operationId `checkDataVolume`).
//!
//! Both endpoints share the identifier-resolution rules, the reserved-error
//! convention, and the `device-data-volume:read` scope. `retrieve` reports the
//! device's *consumed* bucket; `check` answers a boolean against its *remaining*
//! volume (see [`check`] and [`remaining_data_volume_mib`]).
//!
//! ## What it does
//!
//! The caller asks about a device, identified either by a `device` object in the
//! request body (two-legged / CIBA) or by the identity a three-legged access
//! token authenticated (in which case `device` is omitted). The operator answers
//! `{ "dataVolumeCategory": …, "lastStatusTime": …, "device"?: … }` — a bucketed
//! usage figure, never a precise byte count.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `device-data-volume:read` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA (mirrors Connected Network Type): the `device` in the body
//! is only meaningful in two-legged auth. In a three-legged token the device is
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
//! - **Data-volume category** — otherwise the identifier's trailing three digits
//!   index the fixed [`CATEGORIES`] table (`digits % 4`; `…000` / no digits → the
//!   first entry, `<200MiB`), so the reported bucket is a genuine control plane
//!   (the same input always returns the same category). The order is
//!   lowest-first, so the happy-path default is a lightly-used device.
//!
//! `lastStatusTime` is the current instant (the simulator always treats usage as
//! freshly measured). The CAMARA schema marks it nullable — "no known
//! measurement" — but CamaraSim always has a fresh figure, so it is never `null`.
//!
//! The response echoes the device back only when the identifier is a
//! `phoneNumber` (the CAMARA `DeviceResponse` carries only `phoneNumber`); for an
//! IP-/NAI-keyed request the `device` field is omitted.
//!
//! Examples: `+123456789000` → `<200MiB` (index 0); `+123456789002` → `<5GiB`
//! (`2 % 4 == 2`); `+123456789003` → `>=5GiB`; `+123456789404` → `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Device Data
/// Volume).
const RETRIEVE_SCOPE: &str = "device-data-volume:read";

/// The OAuth2 scope the `POST /check` endpoint requires — the same read scope as
/// `retrieve` (CAMARA Device Data Volume declares one scope for the API).
const CHECK_SCOPE: &str = "device-data-volume:read";

/// The data-volume categories (CAMARA `dataVolumeCategory` enum), lowest-first.
/// The identifier's trailing three digits pick one (`digits % CATEGORIES.len()`),
/// so the reported bucket is deterministic from the device — a genuine second
/// control plane (docs/DESIGN.md §7). Index 0 (`…000` / no digits) is the
/// happy-path default, a lightly-used device.
const CATEGORIES: [&str; 4] = ["<200MiB", "<1GiB", "<5GiB", ">=5GiB"];

/// Routes for Device Data Volume vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/device-data-volume/vwip/retrieve", post(retrieve))
        .route("/device-data-volume/vwip/check", post(check))
}

/// `POST /retrieve` request body (CAMARA `RetrieveDataVolumeRequest`). `device`
/// is optional — omit it when a three-legged token identifies the device.
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

/// `POST /device-data-volume/vwip/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
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
                    "Request body is not a valid RetrieveDataVolumeRequest.",
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

    let category = data_volume_category(&resolved.identifier);
    // The simulator always has a fresh measurement (never null).
    let mut out = json!({
        "dataVolumeCategory": category,
        "lastStatusTime": rfc3339_utc(now_unix_secs()),
    });
    // The CAMARA DeviceResponse carries only `phoneNumber`, so echo the device
    // only for a phone-number-keyed request.
    if let Some(phone) = resolved.phone_number {
        out["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// The data-volume category for `identifier`, chosen from the fixed
/// [`CATEGORIES`] table by the identifier's trailing three digits
/// (`digits % CATEGORIES.len()`; no digits → the first entry, `<200MiB`).
fn data_volume_category(identifier: &str) -> &'static str {
    let idx =
        scenarios::trailing_three_digits(identifier).unwrap_or(0) as usize % CATEGORIES.len();
    CATEGORIES[idx]
}

/// `POST /check` request body (CAMARA `CheckDataVolumeRequest`). `device` is
/// optional (omit it under a three-legged token); `volumeToCheck` is **required**.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRequest {
    device: Option<Device>,
    #[serde(rename = "volumeToCheck")]
    volume_to_check: VolumeToCheck,
}

/// The threshold to compare the device's remaining volume against (CAMARA
/// `volumeToCheck`). `value` is an int32 in `0..=1024`; `unit` is `MiB`/`GiB`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VolumeToCheck {
    value: i64,
    unit: VolumeUnit,
}

/// The CAMARA `VolumeUnitEnum`: `MiB` (2^20 bytes) or `GiB` (2^30 bytes). An
/// unknown unit fails deserialization → `400 INVALID_ARGUMENT`.
#[derive(Debug, Clone, Copy, Deserialize)]
enum VolumeUnit {
    MiB,
    GiB,
}

/// The `value`/`unit` range CamaraSim accepts for `volumeToCheck.value`
/// (CAMARA schema `minimum: 0`, `maximum: 1024`).
const VOLUME_VALUE_MAX: i64 = 1024;

/// `POST /device-data-volume/vwip/check`.
///
/// Answers `{ "thresholdExceeded": bool, "lastStatusTime": …, "device"?: … }` —
/// whether the device's **remaining** data volume exceeds the caller-supplied
/// `volumeToCheck`. `volumeToCheck` is required (an empty/malformed body or an
/// out-of-range `value` is rejected before the identifier is resolved), then the
/// same identifier resolution and reserved-error convention as `retrieve` apply.
async fn check(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CHECK_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `volumeToCheck` is required, so the body must be present and well-formed.
    let req: CheckRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CheckDataVolumeRequest (`volumeToCheck` is required).",
                &correlator,
            )
        }
    };

    // Validate the threshold's range first (mirrors KYC Age Verification's
    // `ageThreshold` plane): a `value` outside `0..=1024` → 400 OUT_OF_RANGE.
    if !(0..=VOLUME_VALUE_MAX).contains(&req.volume_to_check.value) {
        return out_of_range(
            "`volumeToCheck.value` must be in the range 0..=1024.",
            &correlator,
        );
    }

    // Resolve the device identifier, honouring the two-legged / three-legged rule.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the first control plane (docs/DESIGN.md §7), checked first.
    if let Some(err) = scenarios::reserved_error(&resolved.identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Second control plane: the device's remaining volume vs the threshold.
    let remaining_mib = remaining_data_volume_mib(&resolved.identifier);
    let threshold_mib = to_mib(req.volume_to_check.value, req.volume_to_check.unit);
    let threshold_exceeded = remaining_mib > threshold_mib;

    let mut out = json!({
        "thresholdExceeded": threshold_exceeded,
        "lastStatusTime": rfc3339_utc(now_unix_secs()),
    });
    // The CAMARA DeviceResponse carries only `phoneNumber` (echo phone-keyed only).
    if let Some(phone) = resolved.phone_number {
        out["device"] = json!({ "phoneNumber": phone });
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// The device's **remaining** data volume, in MiB, deterministic from
/// `identifier` (docs/DESIGN.md §7). The trailing three digits `d` (`0..=999`;
/// no digits → 0) scale to `d * 10` MiB (`0..=9990` MiB ≈ 9.75 GiB), so the same
/// device always reports the same remaining volume. This is a distinct quantity
/// from `retrieve`'s *consumed* bucket. `…000` / a no-digit identifier → `0` MiB
/// remaining, so it never exceeds a positive threshold.
fn remaining_data_volume_mib(identifier: &str) -> i64 {
    scenarios::trailing_three_digits(identifier).unwrap_or(0) as i64 * 10
}

/// Normalise a `volumeToCheck` `(value, unit)` to MiB (`GiB` → `value * 1024`),
/// so the threshold can be compared against [`remaining_data_volume_mib`]. Cannot
/// overflow: `value <= 1024`, so the largest product is `1024 * 1024` MiB.
fn to_mib(value: i64, unit: VolumeUnit) -> i64 {
    match unit {
        VolumeUnit::MiB => value,
        VolumeUnit::GiB => value * 1024,
    }
}

/// A resolved device identifier plus, when it is a phone number, that number
/// (for echoing back in the `DeviceResponse`).
struct Resolved {
    identifier: String,
    phone_number: Option<String>,
}

/// Resolve the device identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Connected Network Type). See the module docs for the cases.
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

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed (used for a
/// `volumeToCheck.value` outside its `0..=1024` schema range).
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
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 / ISO 8601 instant with
/// a `Z` offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs
/// no date/time dependency (mirrors `connected_network_type::v0_2`).
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

    const HOST: &str = "ddv.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn category_is_indexed_by_the_trailing_digits() {
        // …000 → 0 → <200MiB (default happy path).
        assert_eq!(data_volume_category("+123456789000"), "<200MiB");
        // No trailing digits → default (<200MiB).
        assert_eq!(data_volume_category("ddv-client"), "<200MiB");
        // …001 → 1 % 4 == 1 → <1GiB.
        assert_eq!(data_volume_category("+123456789001"), "<1GiB");
        // …002 → 2 % 4 == 2 → <5GiB.
        assert_eq!(data_volume_category("+123456789002"), "<5GiB");
        // …003 → 3 % 4 == 3 → >=5GiB.
        assert_eq!(data_volume_category("+123456789003"), ">=5GiB");
        // …013 → 13 % 4 == 1 → <1GiB.
        assert_eq!(data_volume_category("+123456789013"), "<1GiB");
    }

    #[test]
    fn every_category_is_a_valid_camara_enum_value() {
        for c in CATEGORIES {
            assert!(
                ["<200MiB", "<1GiB", "<5GiB", ">=5GiB"].contains(&c),
                "{c} not in the enum"
            );
        }
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
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("+1234567890123456")); // too long
        assert!(!is_valid_e164("ddv-client"));
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
        mint_token_with_client(scope, "ddv-client").await
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

    /// POST to `/retrieve` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/device-data-volume/vwip/retrieve")
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
    async fn returns_the_category_with_required_fields() {
        // …000 → <200MiB, with a fresh lastStatusTime and (phone-keyed) device echo.
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789000"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dataVolumeCategory"], "<200MiB");
        assert!(body["lastStatusTime"].as_str().unwrap().ends_with('Z'));
        // Phone-keyed → device echoed.
        assert_eq!(body["device"]["phoneNumber"], "+123456789000");
    }

    #[tokio::test]
    async fn different_tails_select_different_categories() {
        // …002 → <5GiB.
        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789002"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dataVolumeCategory"], "<5GiB");
        // …003 → >=5GiB.
        let (_, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789003"}}"#).await;
        assert_eq!(body["dataVolumeCategory"], ">=5GiB");
    }

    #[tokio::test]
    async fn non_phone_identifier_omits_the_device_echo() {
        // ipv4 publicAddress ending …002 → <5GiB, no device echo
        // (DeviceResponse carries only phoneNumber).
        let (status, _, body) =
            call_ok(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.002"}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dataVolumeCategory"], "<5GiB");
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

        let (status, _, body) = call_ok(r#"{"device":{"phoneNumber":"+123456789422"}}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    // --- Identifier resolution --------------------------------------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …002 → <5GiB, device echoed.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789002").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dataVolumeCategory"], "<5GiB");
        assert_eq!(body["device"]["phoneNumber"], "+123456789002");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
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
        // Two-legged token (sub = ddv-client) with no device → can't identify.
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
            r#"{"device":{"phoneNumber":"+123456789000"}}"#,
            Some("corr-ddv"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ddv")
        );
        // Business error.
        let (status, headers, _) = post_retrieve(Some(&token), "{}", Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // ======================================================================
    // POST /check (checkDataVolume)
    // ======================================================================

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn remaining_volume_scales_with_the_trailing_digits() {
        // …000 / no digits → 0 MiB remaining (never exceeds a positive threshold).
        assert_eq!(remaining_data_volume_mib("+123456789000"), 0);
        assert_eq!(remaining_data_volume_mib("ddv-client"), 0);
        // d * 10 MiB.
        assert_eq!(remaining_data_volume_mib("+123456789050"), 500);
        assert_eq!(remaining_data_volume_mib("+123456789500"), 5000);
        assert_eq!(remaining_data_volume_mib("+123456789999"), 9990);
    }

    #[test]
    fn to_mib_normalises_the_unit() {
        assert_eq!(to_mib(500, VolumeUnit::MiB), 500);
        assert_eq!(to_mib(1, VolumeUnit::GiB), 1024);
        assert_eq!(to_mib(1024, VolumeUnit::GiB), 1024 * 1024);
        assert_eq!(to_mib(0, VolumeUnit::MiB), 0);
    }

    // --- Integration through the real router -------------------------------

    /// POST to `/check` with an optional Bearer token and optional correlator.
    async fn post_check(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/device-data-volume/vwip/check")
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

    /// Mint a scoped (two-legged, non-line subject) token and call `/check`.
    async fn check_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CHECK_SCOPE).await;
        post_check(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn threshold_is_a_second_control_plane_for_a_fixed_device() {
        // …450 → 4500 MiB remaining (450 is not a reserved error suffix). The same
        // device flips true↔false as the threshold moves — a genuine second
        // control plane (docs/DESIGN.md §7).
        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789450"},"volumeToCheck":{"value":4,"unit":"GiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // 4500 MiB > 4096 MiB → exceeds.
        assert_eq!(body["thresholdExceeded"], true);
        assert!(body["lastStatusTime"].as_str().unwrap().ends_with('Z'));
        assert_eq!(body["device"]["phoneNumber"], "+123456789450");

        let (_, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789450"},"volumeToCheck":{"value":5,"unit":"GiB"}}"#,
        )
        .await;
        // 4500 MiB > 5120 MiB → does not exceed.
        assert_eq!(body["thresholdExceeded"], false);
    }

    #[tokio::test]
    async fn device_is_a_control_plane_for_a_fixed_threshold() {
        // Threshold 1 GiB (1024 MiB). Remaining scales with the device tail.
        let (_, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":1,"unit":"GiB"}}"#,
        )
        .await;
        // …050 → 500 MiB remaining < 1024 → false.
        assert_eq!(body["thresholdExceeded"], false);

        let (_, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789200"},"volumeToCheck":{"value":1,"unit":"GiB"}}"#,
        )
        .await;
        // …200 → 2000 MiB remaining > 1024 → true.
        assert_eq!(body["thresholdExceeded"], true);
    }

    #[tokio::test]
    async fn zero_remaining_never_exceeds() {
        // …000 → 0 MiB remaining → no positive threshold is exceeded.
        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789000"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["thresholdExceeded"], false);
    }

    #[tokio::test]
    async fn mib_unit_is_honoured() {
        // …050 → 500 MiB remaining; threshold 300 MiB → 500 > 300 → true.
        let (_, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":300,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(body["thresholdExceeded"], true);
        // Threshold 700 MiB → 500 > 700 → false.
        let (_, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":700,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(body["thresholdExceeded"], false);
    }

    #[tokio::test]
    async fn check_non_phone_identifier_omits_the_device_echo() {
        let (status, _, body) = check_ok(
            r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.200"}},"volumeToCheck":{"value":1,"unit":"GiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // …200 → 2000 MiB > 1024 → true; no device echo (DeviceResponse = phone).
        assert_eq!(body["thresholdExceeded"], true);
        assert!(body.get("device").is_none());
    }

    #[tokio::test]
    async fn check_reserved_suffix_selects_a_canonical_camara_error() {
        // Reserved identifier suffix wins over the threshold plane.
        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789404"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789429"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn check_three_legged_keys_off_the_subject() {
        // No device; subject is an E.164 line …200 → 2000 MiB remaining.
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789200").await;
        let (status, _, body) =
            post_check(Some(&token), r#"{"volumeToCheck":{"value":1,"unit":"GiB"}}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["thresholdExceeded"], true);
        assert_eq!(body["device"]["phoneNumber"], "+123456789200");
    }

    #[tokio::test]
    async fn check_resubmitting_the_device_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(CHECK_SCOPE, "+123456789012").await;
        let (status, _, body) = post_check(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn check_no_device_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = check_ok(r#"{"volumeToCheck":{"value":1,"unit":"MiB"}}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn check_value_out_of_range_is_rejected() {
        // > 1024.
        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":1025,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");

        // < 0.
        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":-1,"unit":"MiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn check_range_is_validated_before_the_identifier() {
        // A bad `value` is rejected even with no resolvable identifier (a
        // two-legged token, no device) — range is checked first.
        let (status, _, body) = check_ok(r#"{"volumeToCheck":{"value":9999,"unit":"MiB"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn check_missing_volume_to_check_is_rejected() {
        // `volumeToCheck` is required.
        let (status, _, body) =
            check_ok(r#"{"device":{"phoneNumber":"+123456789050"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");

        // Empty body → also rejected (unlike /retrieve, /check requires a body).
        let token = mint_token(CHECK_SCOPE).await;
        let (status, _, body) = post_check(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn check_unknown_unit_is_rejected() {
        let (status, _, body) = check_ok(
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":1,"unit":"TiB"}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn check_empty_device_object_is_rejected() {
        let (status, _, body) =
            check_ok(r#"{"device":{},"volumeToCheck":{"value":1,"unit":"MiB"}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn check_token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_check(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn check_missing_token_is_unauthenticated() {
        let (status, _, body) = post_check(
            None,
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn check_x_correlator_is_echoed() {
        let token = mint_token(CHECK_SCOPE).await;
        let (status, headers, _) = post_check(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789050"},"volumeToCheck":{"value":1,"unit":"MiB"}}"#,
            Some("corr-chk"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-chk")
        );
    }
}
