//! Home Devices QoD **v0.4** (CAMARA Home Devices QoD 0.4.0).
//!
//! Endpoint:
//! - `PUT /home-devices-qod/v0.4/qos` — set the desired QoS behaviour for one
//!   device on the subscriber's home network.
//!
//! ## What it does
//!
//! The caller submits a `serviceClass` (the desired treatment) and the target
//! device's internal `ipAddress` (its LAN address behind the home router). The
//! operator applies that treatment on the router and answers `204 No Content`.
//! QoS treatment is downstream-only, WiFi-only, and lives entirely inside the
//! home network — there is no session to read back, so the operation is stateless
//! (no store): a later `PUT` simply re-sets the treatment, and the special
//! `standard` class restores the router default.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the scope
//! `home-devices-qod:qos:write`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! There is no phone number here; the device is identified by its LAN
//! `ipAddress`, which is also the control plane. The **last octet** of the
//! `ipAddress` selects the outcome, so every declared functional case is
//! reachable from the request alone:
//!
//! - **Happy path.** Any address whose last octet is not reserved below →
//!   `204 No Content` (the treatment is applied).
//! - **Reserved last octets** name a specific home-network condition and return
//!   that API's canonical error (all `409` codes carry the `HOME_DEVICES_QOD.`
//!   prefix, per CAMARA 0.4.0):
//!   - `…240` → `404 DEVICE_NOT_FOUND`
//!   - `…241` → `409 HOME_DEVICES_QOD.TOO_MANY_DEVICES`
//!   - `…242` → `409 HOME_DEVICES_QOD.RSSI_BELOW_THRESHOLD`
//!   - `…243` → `409 HOME_DEVICES_QOD.QOS_TOO_HIGH`
//!   - `…244` → `409 HOME_DEVICES_QOD.OCCUPANCY_ABOVE_THRESHOLD`
//!   - `…245` → `409 HOME_DEVICES_QOD.NOT_CONNECTED_TO_REQUIRED_INTERFACE`
//!   - `…246` → `409 HOME_DEVICES_QOD.NOT_SUPPORTED_REQUIRED_INTERFACE`
//!   - `…247` → `409 HOME_DEVICES_QOD.QOS_ALREADY_SET_TO_DEFAULT` **only when**
//!     `serviceClass` is `standard` (restoring an already-default device);
//!     otherwise this address is a happy path (`204`). This makes `serviceClass`
//!     a genuine second control plane.
//!   - `…248` → `503 HOME_DEVICES_QOD.ROUTER_OFFLINE`
//!   - `…249` → `504 TIMEOUT`
//!   - `…250` → `500 INTERNAL`
//!   - `…251` → `409 CONFLICT` (the generic base code)
//!   - `…252` → `404 NOT_FOUND` (the generic base code)
//!   - `…253` → `503 UNAVAILABLE` (the generic base code)
//!
//! Validation runs first: an unknown/malformed body, a `serviceClass` outside the
//! enum, or an `ipAddress` that is not a valid IPv4 address → `400
//! INVALID_ARGUMENT`.

use std::net::Ipv4Addr;

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::put;
use axum::Router;
use serde::Deserialize;

use crate::auth::verify::Claims;
use crate::errors::CamaraError;

/// The OAuth2 scope `PUT /qos` requires (CAMARA Home Devices QoD 0.4.0).
const QOS_SCOPE: &str = "home-devices-qod:qos:write";

/// The `serviceClass` enum (CAMARA Home Devices QoD 0.4.0). `standard` restores
/// the router's default treatment; the rest raise QoS for a traffic profile.
const SERVICE_CLASSES: &[&str] = &[
    "real_time_interactive",
    "multimedia_streaming",
    "broadcast_video",
    "low_latency_data",
    "high_throughput_data",
    "low_priority_data",
    "standard",
];

/// Routes for Home Devices QoD v0.4, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/home-devices-qod/v0.4/qos", put(set_qos))
}

/// `PUT /qos` request body (CAMARA `QosOnDemandUpdate`): the desired
/// `serviceClass` and the target device's internal LAN `ipAddress`. Both are
/// required; unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QosOnDemandUpdate {
    #[serde(rename = "serviceClass")]
    service_class: String,
    #[serde(rename = "ipAddress")]
    ip_address: String,
}

/// `PUT /home-devices-qod/v0.4/qos`.
async fn set_qos(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(QOS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: QosOnDemandUpdate = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid QosOnDemandUpdate.",
                &correlator,
            )
        }
    };

    // `serviceClass` must be one of the known values.
    if !SERVICE_CLASSES.contains(&req.service_class.as_str()) {
        return invalid_argument(
            "`serviceClass` must be one of the CAMARA Home Devices QoD service classes.",
            &correlator,
        );
    }

    // `ipAddress` must be a valid IPv4 address (the internal LAN address).
    let ip: Ipv4Addr = match req.ip_address.parse() {
        Ok(ip) => ip,
        Err(_) => {
            return invalid_argument("`ipAddress` must be a valid IPv4 address.", &correlator)
        }
    };

    // The device/router condition is driven by the address's last octet
    // (docs/DESIGN.md §7). A reserved octet names a specific condition; anything
    // else applies the QoS (204). `serviceClass` is a second control plane for the
    // "already default" case.
    let is_standard = req.service_class == "standard";
    if let Some(err) = reserved_octet_error(ip.octets()[3], is_standard) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: the QoS treatment is applied. 204 carries no body.
    with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
}

/// Map an `ipAddress` last octet to a home-network error condition, or `None`
/// when the address is a happy path (`204`). See the module docs for the table.
///
/// `is_standard` is whether the request's `serviceClass` is `standard`: the
/// `…247` "already default" case only applies when restoring the default, so a
/// non-`standard` request to that address is a happy path.
fn reserved_octet_error(last_octet: u8, is_standard: bool) -> Option<CamaraError> {
    // A `409 HOME_DEVICES_QOD.<code>` error with a standard message.
    fn hdq_conflict(code: &str, message: &str) -> CamaraError {
        CamaraError::new(StatusCode::CONFLICT, format!("HOME_DEVICES_QOD.{code}"), message)
    }

    Some(match last_octet {
        240 => CamaraError::new(
            StatusCode::NOT_FOUND,
            "DEVICE_NOT_FOUND",
            "No device with the given ipAddress was found on the home network.",
        ),
        241 => hdq_conflict(
            "TOO_MANY_DEVICES",
            "Too many devices already have a QoS treatment set on the home network.",
        ),
        242 => hdq_conflict(
            "RSSI_BELOW_THRESHOLD",
            "The device's WiFi signal strength is too low to guarantee the QoS.",
        ),
        243 => hdq_conflict(
            "QOS_TOO_HIGH",
            "The requested service class exceeds what the home network can provide.",
        ),
        244 => hdq_conflict(
            "OCCUPANCY_ABOVE_THRESHOLD",
            "The home network is too busy to guarantee the requested QoS.",
        ),
        245 => hdq_conflict(
            "NOT_CONNECTED_TO_REQUIRED_INTERFACE",
            "The device is not connected to the interface required for QoS (WiFi).",
        ),
        246 => hdq_conflict(
            "NOT_SUPPORTED_REQUIRED_INTERFACE",
            "The device's connection interface does not support QoS treatment.",
        ),
        // Only a conflict when restoring an already-default device; a non-standard
        // request to this address is a happy path (serviceClass is a control plane).
        247 if is_standard => hdq_conflict(
            "QOS_ALREADY_SET_TO_DEFAULT",
            "The device is already using the default QoS treatment.",
        ),
        248 => CamaraError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "HOME_DEVICES_QOD.ROUTER_OFFLINE",
            "The home router is offline and cannot apply the QoS.",
        ),
        249 => CamaraError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "TIMEOUT",
            "The home router did not respond in time.",
        ),
        250 => CamaraError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            "Unknown server error. Typically a server bug.",
        ),
        251 => CamaraError::new(
            StatusCode::CONFLICT,
            "CONFLICT",
            "A conflict prevented the QoS from being applied.",
        ),
        252 => CamaraError::new(
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
            "The specified resource is not found.",
        ),
        253 => CamaraError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "UNAVAILABLE",
            "Service unavailable.",
        ),
        _ => return None,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "hdq.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn reserved_octets_map_to_the_declared_error_set() {
        // (octet, is_standard) -> (status, code)
        let cases = [
            (240u8, false, StatusCode::NOT_FOUND, "DEVICE_NOT_FOUND"),
            (241, false, StatusCode::CONFLICT, "HOME_DEVICES_QOD.TOO_MANY_DEVICES"),
            (242, false, StatusCode::CONFLICT, "HOME_DEVICES_QOD.RSSI_BELOW_THRESHOLD"),
            (243, false, StatusCode::CONFLICT, "HOME_DEVICES_QOD.QOS_TOO_HIGH"),
            (244, false, StatusCode::CONFLICT, "HOME_DEVICES_QOD.OCCUPANCY_ABOVE_THRESHOLD"),
            (245, false, StatusCode::CONFLICT, "HOME_DEVICES_QOD.NOT_CONNECTED_TO_REQUIRED_INTERFACE"),
            (246, false, StatusCode::CONFLICT, "HOME_DEVICES_QOD.NOT_SUPPORTED_REQUIRED_INTERFACE"),
            (247, true, StatusCode::CONFLICT, "HOME_DEVICES_QOD.QOS_ALREADY_SET_TO_DEFAULT"),
            (248, false, StatusCode::SERVICE_UNAVAILABLE, "HOME_DEVICES_QOD.ROUTER_OFFLINE"),
            (249, false, StatusCode::GATEWAY_TIMEOUT, "TIMEOUT"),
            (250, false, StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
            (251, false, StatusCode::CONFLICT, "CONFLICT"),
            (252, false, StatusCode::NOT_FOUND, "NOT_FOUND"),
            (253, false, StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE"),
        ];
        for (octet, is_standard, status, code) in cases {
            let err = reserved_octet_error(octet, is_standard)
                .unwrap_or_else(|| panic!("octet {octet} should be reserved"));
            assert_eq!(err.status, status, "octet {octet} status");
            assert_eq!(err.code, code, "octet {octet} code");
        }
    }

    #[test]
    fn unreserved_octet_is_a_happy_path() {
        assert!(reserved_octet_error(1, false).is_none());
        assert!(reserved_octet_error(100, true).is_none());
        assert!(reserved_octet_error(254, false).is_none());
    }

    #[test]
    fn already_default_is_only_a_conflict_for_the_standard_class() {
        // …247 with `standard` → conflict; with any other class → happy path.
        assert!(reserved_octet_error(247, true).is_some());
        assert!(reserved_octet_error(247, false).is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials` with the given scope.
    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=hdq-client&scope={scope}");
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

    /// PUT to `/qos` with an optional Bearer token and optional `x-correlator`.
    /// Returns (status, headers, json-or-null).
    async fn put_qos(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("PUT")
            .uri("/home-devices-qod/v0.4/qos")
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

    /// Mint a `home-devices-qod:qos:write` token and call the endpoint.
    async fn qos_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(QOS_SCOPE).await;
        put_qos(Some(&token), body, None).await
    }

    // --- Happy path --------------------------------------------------------

    #[tokio::test]
    async fn setting_qos_on_a_normal_device_is_204() {
        let (status, _, body) =
            qos_ok(r#"{"serviceClass":"real_time_interactive","ipAddress":"192.168.1.42"}"#).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_null(), "204 carries no body");
    }

    #[tokio::test]
    async fn restoring_default_on_a_normal_device_is_204() {
        let (status, _, _) =
            qos_ok(r#"{"serviceClass":"standard","ipAddress":"192.168.1.42"}"#).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn every_service_class_is_accepted() {
        for class in SERVICE_CLASSES {
            let body = format!(r#"{{"serviceClass":"{class}","ipAddress":"10.0.0.5"}}"#);
            let (status, _, _) = qos_ok(&body).await;
            assert_eq!(status, StatusCode::NO_CONTENT, "class {class}");
        }
    }

    // --- Reserved-octet control plane --------------------------------------

    #[tokio::test]
    async fn reserved_octet_returns_the_device_specific_error() {
        let (status, _, body) =
            qos_ok(r#"{"serviceClass":"low_latency_data","ipAddress":"192.168.1.241"}"#).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "HOME_DEVICES_QOD.TOO_MANY_DEVICES");
        assert_eq!(body["status"], 409);
    }

    #[tokio::test]
    async fn reserved_octet_device_not_found_is_404() {
        let (status, _, body) =
            qos_ok(r#"{"serviceClass":"standard","ipAddress":"192.168.1.240"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "DEVICE_NOT_FOUND");
    }

    #[tokio::test]
    async fn router_offline_and_timeout_and_internal() {
        for (octet, status, code) in [
            (248u16, StatusCode::SERVICE_UNAVAILABLE, "HOME_DEVICES_QOD.ROUTER_OFFLINE"),
            (249, StatusCode::GATEWAY_TIMEOUT, "TIMEOUT"),
            (250, StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        ] {
            let body =
                format!(r#"{{"serviceClass":"broadcast_video","ipAddress":"192.168.1.{octet}"}}"#);
            let (got, _, json) = qos_ok(&body).await;
            assert_eq!(got, status, "octet {octet}");
            assert_eq!(json["code"], code, "octet {octet}");
        }
    }

    #[tokio::test]
    async fn already_default_depends_on_the_service_class() {
        // …247 with `standard` → 409 already-default.
        let (status, _, body) =
            qos_ok(r#"{"serviceClass":"standard","ipAddress":"192.168.1.247"}"#).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "HOME_DEVICES_QOD.QOS_ALREADY_SET_TO_DEFAULT");

        // …247 with any raising class → happy path (serviceClass is a control plane).
        let (status, _, _) =
            qos_ok(r#"{"serviceClass":"multimedia_streaming","ipAddress":"192.168.1.247"}"#).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn unknown_service_class_is_rejected() {
        let (status, _, body) =
            qos_ok(r#"{"serviceClass":"turbo","ipAddress":"192.168.1.42"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_ipv4_address_is_rejected() {
        for bad in ["not-an-ip", "2001:db8::1", "192.168.1.999", "192.168.1"] {
            let body = format!(r#"{{"serviceClass":"standard","ipAddress":"{bad}"}}"#);
            let (status, _, json) = qos_ok(&body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "ip {bad}");
            assert_eq!(json["code"], "INVALID_ARGUMENT", "ip {bad}");
        }
    }

    #[tokio::test]
    async fn missing_required_field_is_rejected() {
        let (status, _, body) = qos_ok(r#"{"serviceClass":"standard"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            qos_ok(r#"{"serviceClass":"standard","ipAddress":"10.0.0.1","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = qos_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = put_qos(
            Some(&token),
            r#"{"serviceClass":"standard","ipAddress":"10.0.0.1"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = put_qos(
            None,
            r#"{"serviceClass":"standard","ipAddress":"10.0.0.1"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- x-correlator ------------------------------------------------------

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(QOS_SCOPE).await;
        // Success (204).
        let (status, headers, _) = put_qos(
            Some(&token),
            r#"{"serviceClass":"standard","ipAddress":"10.0.0.1"}"#,
            Some("corr-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );
        // Business error.
        let (status, headers, _) = put_qos(
            Some(&token),
            r#"{"serviceClass":"standard","ipAddress":"192.168.1.240"}"#,
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
