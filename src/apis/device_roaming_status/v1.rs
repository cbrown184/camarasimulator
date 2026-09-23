//! Device Roaming Status **v1** (CAMARA Device Roaming Status 1.0.0).
//!
//! One endpoint:
//! - `POST /device-roaming-status/v1/retrieve` — is the device currently roaming
//!   and, if so, in which country?
//!
//! ## What it does
//!
//! The caller asks about a device, identified either by a `device` object in the
//! request body (`phoneNumber`, `networkAccessIdentifier`, `ipv4Address`, or
//! `ipv6Address`) or — when `device` is omitted — by the identity a three-legged
//! access token authenticated. The endpoint answers
//! `{ "roaming": true, "countryCode": 262, "countryName": ["DE"] }` (operationId
//! `getRoamingStatus`), or just `{ "roaming": false }` for a device on its home
//! network.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `device-roaming-status:read`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The identifier is the submitted device identifier (phoneNumber, else network
//! access identifier, else the IPv4 `publicAddress`, else ipv6Address) or, when no
//! `device` is supplied, the access token's subject (`sub`). Its trailing three
//! digits select the case:
//!
//! - **Reserved error suffix** — if those trailing three digits name a reserved
//!   CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`,
//!   `…500`, `…503`), the endpoint answers with that canonical CAMARA error
//!   instead of a result (shared convention, [`crate::scenarios`]).
//! - **`…000`** (and an identifier with no trailing digits, such as the synthetic
//!   `camarasim-user` subject) — the device is **on its home network**:
//!   `{ roaming: false }`, no country.
//! - **any other numeric tail** — the device is **roaming**: `{ roaming: true }`
//!   plus a `countryCode` (MCC) and `countryName` (ISO 3166-1 alpha-2), chosen
//!   deterministically from the trailing three digits so the visited country is
//!   itself a second control plane.
//!
//! Examples: `+123456789012` → roaming in Germany (`262`/`DE`); `+123456789011` →
//! roaming in Australia (`505`/`AU`); `+123456789000` → not roaming;
//! `+123456789404` → `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Device
/// Roaming Status 1.0.0).
const RETRIEVE_SCOPE: &str = "device-roaming-status:read";

/// Visited countries a roaming device can be reported in, as `(MCC, ISO 3166-1
/// alpha-2)` pairs. The device's trailing three digits index this table (`digits
/// % len`), so the visited country is deterministic from the input.
const COUNTRIES: &[(u16, &str)] = &[
    (262, "DE"), // Germany
    (234, "GB"), // United Kingdom
    (208, "FR"), // France
    (310, "US"), // United States
    (440, "JP"), // Japan
    (505, "AU"), // Australia
];

/// Routes for Device Roaming Status v1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/device-roaming-status/v1/retrieve", post(retrieve))
}

/// `POST /retrieve` request body (CAMARA `RoamingStatusRequest`). `device` is
/// optional — omit it when a three-legged token identifies the device.
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
    // Accepted for schema fidelity but not used to key scenarios.
    #[serde(rename = "privateAddress")]
    #[allow(dead_code)]
    private_address: Option<String>,
    #[serde(rename = "publicPort")]
    #[allow(dead_code)]
    public_port: Option<i64>,
}

/// `POST /device-roaming-status/v1/retrieve`.
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
                    "Request body is not a valid RoamingStatusRequest.",
                    &correlator,
                )
            }
        }
    };

    // The identifier is the submitted device identifier, else the token subject
    // (three-legged fallback). Missing both → 422 MISSING_IDENTIFIER.
    let identifier = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let response_body = match roaming_status(&identifier) {
        RoamingStatus::Home => json!({ "roaming": false }),
        RoamingStatus::Roaming { country_code, country_name } => json!({
            "roaming": true,
            "countryCode": country_code,
            "countryName": [country_name],
        }),
    };

    with_correlator(
        (StatusCode::OK, Json(response_body)).into_response(),
        &correlator,
    )
}

/// A device's roaming status: either home (not roaming) or roaming in a specific
/// visited country.
#[derive(Debug, PartialEq)]
enum RoamingStatus {
    Home,
    Roaming {
        country_code: u16,
        country_name: &'static str,
    },
}

/// The roaming status of `identifier`, driven by its trailing three digits
/// (docs/DESIGN.md §7): `…000` (and an identifier with no trailing digits) → home
/// / not roaming; any other numeric tail → roaming, with the visited country
/// chosen from [`COUNTRIES`] by `digits % len`.
fn roaming_status(identifier: &str) -> RoamingStatus {
    match scenarios::trailing_three_digits(identifier) {
        Some(0) | None => RoamingStatus::Home,
        Some(n) => {
            let (country_code, country_name) = COUNTRIES[(n as usize) % COUNTRIES.len()];
            RoamingStatus::Roaming { country_code, country_name }
        }
    }
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
            Some(DeviceId::Other(id)) => Ok(id),
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

    const HOST: &str = "drs.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn roaming_status_is_driven_by_the_trailing_digits() {
        // …000 → home / not roaming.
        assert_eq!(roaming_status("+123456789000"), RoamingStatus::Home);
        // no trailing digits → default home / not roaming.
        assert_eq!(roaming_status("camarasim-user"), RoamingStatus::Home);
        // 012 → 12 % 6 = 0 → Germany.
        assert_eq!(
            roaming_status("+123456789012"),
            RoamingStatus::Roaming { country_code: 262, country_name: "DE" }
        );
        // 011 → 11 % 6 = 5 → Australia.
        assert_eq!(
            roaming_status("+123456789011"),
            RoamingStatus::Roaming { country_code: 505, country_name: "AU" }
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
    }

    #[test]
    fn device_identifier_follows_precedence() {
        let phone = Device {
            phone_number: Some("+123456789012".into()),
            network_access_identifier: Some("nai@example.com".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&phone), Some(DeviceId::PhoneNumber(p)) if p == "+123456789012"));

        let nai = Device {
            network_access_identifier: Some("nai@example.com".into()),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&nai), Some(DeviceId::Other(p)) if p == "nai@example.com"));

        let ipv4 = Device {
            ipv4_address: Some(DeviceIpv4Addr {
                public_address: Some("203.0.113.12".into()),
                private_address: None,
                public_port: None,
            }),
            ..Device::default()
        };
        assert!(matches!(device_identifier(&ipv4), Some(DeviceId::Other(p)) if p == "203.0.113.12"));

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

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the route's audience. Scope granted verbatim.
    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "drs-client").await
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
            .uri("/device-roaming-status/v1/retrieve")
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
    async fn roaming_device_reports_country() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789012"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], true);
        assert_eq!(body["countryCode"], 262);
        assert_eq!(body["countryName"], json!(["DE"]));
    }

    #[tokio::test]
    async fn a_different_tail_roams_in_a_different_country() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789011"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], true);
        assert_eq!(body["countryCode"], 505);
        assert_eq!(body["countryName"], json!(["AU"]));
    }

    #[tokio::test]
    async fn triple_zero_tail_is_not_roaming() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789000"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], false);
        // Home device: no country information.
        assert!(body.get("countryCode").is_none());
        assert!(body.get("countryName").is_none());
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
    async fn non_phone_identifiers_are_accepted() {
        // networkAccessIdentifier ending in …012 → roaming in Germany.
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"networkAccessIdentifier":"user012@nai"}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], true);
        assert_eq!(body["countryCode"], 262);

        // ipv4 publicAddress ending in …000 → not roaming.
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"ipv4Address":{"publicAddress":"203.0.113.000"}}}"#)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], false);
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = retrieve_ok_token(r#"{"device":{"phoneNumber":"0123"}}"#).await;
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
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            retrieve_ok_token(r#"{"device":{"phoneNumber":"+123456789012"},"x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject() {
        // Subject is an E.164 number ending …012 → roaming in Germany, empty body.
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], true);
        assert_eq!(body["countryCode"], 262);
        assert_eq!(body["countryName"], json!(["DE"]));
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789503").await;
        let (status, _, body) = post_retrieve(Some(&token), "{}", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn non_numeric_subject_without_device_is_home_by_default() {
        // Default synthetic subject "drs-client" has no digits → home default.
        let (status, _, body) = retrieve_ok_token("{}").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["roaming"], false);
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
        let (status, _, body) =
            post_retrieve(None, r#"{"device":{"phoneNumber":"+123456789012"}}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"device":{"phoneNumber":"+123456789012"}}"#,
            Some("corr-drs"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-drs")
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
