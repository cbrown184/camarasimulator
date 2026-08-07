//! Network Access Devices **vwip** (CAMARA NetworkAccessManagement / Network
//! Access Devices, wip).
//!
//! Two endpoints:
//! - `GET /network-access-devices/vwip/network-access-devices` — list the
//!   operator-supplied network access devices associated with the subscriber.
//! - `GET /network-access-devices/vwip/network-access-devices/{networkAccessDeviceId}`
//!   — read one of those devices by id. The device set is deterministic from the
//!   token subject, so the read regenerates it and looks the id up — no store.
//!
//! ## What it does
//!
//! Returns the operator-managed access equipment (gateways/routers/access
//! points) associated with the subscriber the access token authenticated, as a
//! `NetworkAccessDeviceList`. Each device carries an `id` (UUID), a
//! `deviceStatus` (`connected`/`disconnected`/`unavailable`), a `name`, a
//! `description`, and an EUI-48 `hardwareAddress`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `network-access-devices:reboot`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The listing has no request body, so — like Number Verification's
//! `GET /device-phone-number` — its functional cases are driven by the token
//! **subject** (`sub`):
//!
//! - **Reserved error suffix** — if the subject's trailing three digits name a
//!   reserved CAMARA status (shared convention, [`crate::scenarios`]), the
//!   endpoint answers with that canonical CAMARA error instead of a list.
//! - **Device set** — otherwise the subject's trailing three digits `d` (or `0`
//!   when the subject has no digits) drive two facets of the returned list:
//!   the **count** (`d == 0` → one device, else `((d - 1) % 3) + 1`, i.e. 1–3)
//!   and each device's **status** (device `i` reports
//!   `[connected, disconnected, unavailable][(d + i) % 3]`). Each device's `id`
//!   and `hardwareAddress` are deterministic from the subject and index
//!   (SHA-256).
//!
//! `serviceSite` and the inherited Commonalities `Device` end-user identifier
//! fields are omitted for these operator devices (a documented, schema-valid
//! cut — only `id` is required).

use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `GET /network-access-devices` requires (CAMARA NAM wip).
const LIST_SCOPE: &str = "network-access-devices:reboot";

/// The three device statuses, in the CAMARA enum order the digit plane indexes.
const STATUSES: [&str; 3] = ["connected", "disconnected", "unavailable"];

/// Routes for Network Access Devices vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/network-access-devices/vwip/network-access-devices",
            get(list_devices),
        )
        .route(
            "/network-access-devices/vwip/network-access-devices/:network_access_device_id",
            get(get_device),
        )
}

/// `GET /network-access-devices/vwip/network-access-devices`.
async fn list_devices(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(LIST_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the control plane (docs/DESIGN.md §7).
    let subject = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(subject) {
        return with_correlator(err.into_response(), &correlator);
    }

    let devices = device_list(subject);
    with_correlator(
        (StatusCode::OK, Json(Value::Array(devices))).into_response(),
        &correlator,
    )
}

/// `GET /network-access-devices/vwip/network-access-devices/{networkAccessDeviceId}`
/// — read one of the subscriber's Network Access Devices by id (operationId
/// `getNetworkAccessDevice`).
///
/// The subscriber's device set is fully deterministic from the token subject (see
/// [`device_list`]), so — unlike CAMARA's stateful resource read — this endpoint
/// needs no store: it regenerates the subject's set and returns the device whose
/// `id` matches the path parameter. Two control planes (docs/DESIGN.md §7):
///
/// - **Reserved error suffix (subject)** — an account-level plane, mirroring the
///   listing: if the subject's trailing three digits name a reserved CAMARA
///   status, the endpoint answers that canonical error regardless of the id.
/// - **The id vs the subject's set** — a `networkAccessDeviceId` that belongs to
///   the subject's deterministic set → `200` with that device; any other id
///   (unknown, belonging to a different subscriber, or malformed) → `404
///   NOT_FOUND`. The id is opaque to the caller (UUID-shaped), so it is not itself
///   a scenario plane.
///
/// Requires a token carrying the `network-access-devices:reboot` scope.
async fn get_device(claims: Claims, headers: HeaderMap, Path(id): Path<String>) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(LIST_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the account-level control plane (docs/DESIGN.md §7):
    // a reserved suffix takes the whole account into a canonical error, matching
    // the listing endpoint.
    let subject = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(subject) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Regenerate the subject's deterministic set and look the id up. An id that is
    // not one of this subscriber's devices — unknown, another subscriber's, or
    // malformed — is a `404 NOT_FOUND` (there is no store to distinguish them).
    match device_list(subject)
        .into_iter()
        .find(|device| device["id"] == json!(id))
    {
        Some(device) => {
            with_correlator((StatusCode::OK, Json(device)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No Network Access Device found for the provided id.")
                .into_response(),
            &correlator,
        ),
    }
}

/// The subscriber's device list, deterministic from the token subject.
///
/// The subject's trailing three digits `d` (or `0` when it has no digits) fix
/// the count (`d == 0` → 1, else `((d - 1) % 3) + 1`) and each device's status
/// (device `i` → `STATUSES[(d + i) % 3]`).
fn device_list(subject: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(subject).unwrap_or(0) as usize;
    let count = if d == 0 { 1 } else { ((d - 1) % 3) + 1 };
    (0..count).map(|i| device(subject, i, (d + i) % 3)).collect()
}

/// One `NetworkAccessDevice`. `id` and `hardwareAddress` are derived from the
/// subject and device index via SHA-256 (deterministic, no new dependency).
fn device(subject: &str, index: usize, status_idx: usize) -> Value {
    let digest = Sha256::digest(format!("nad-device:{subject}:{index}").as_bytes());
    json!({
        "id": uuid_shaped(&digest),
        "deviceStatus": STATUSES[status_idx],
        "name": format!("Gateway-{}", index + 1),
        "description": "Operator-supplied network access gateway",
        "hardwareAddress": {
            "hardwareAddressType": "EUI-48",
            "value": mac_address(&digest),
        },
    })
}

/// A stable, UUID-shaped identifier from the first 16 bytes of a SHA-256 digest.
/// UUID-*shaped* (not a real v4/v5 UUID) — a documented cut mirroring Simple
/// Edge Discovery's `edgeCloudZoneId` and the Blockchain Public Address ids.
fn uuid_shaped(h: &[u8]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// An EUI-48 hardware address (`XX:XX:XX:XX:XX:XX`) from six digest bytes.
fn mac_address(h: &[u8]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        h[16], h[17], h[18], h[19], h[20], h[21]
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "sim.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn count_and_status_follow_the_subject_digit_plane() {
        // No digits / …000 → a single connected device.
        let no_digits = device_list("nad-client");
        assert_eq!(no_digits.len(), 1);
        assert_eq!(no_digits[0]["deviceStatus"], "connected");

        let zero = device_list("+123456789000");
        assert_eq!(zero.len(), 1);
        assert_eq!(zero[0]["deviceStatus"], "connected");

        // …001 → one disconnected device.
        let one = device_list("+123456789001");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0]["deviceStatus"], "disconnected");

        // …002 → two devices [unavailable, connected].
        let two = device_list("+123456789002");
        assert_eq!(two.len(), 2);
        assert_eq!(two[0]["deviceStatus"], "unavailable");
        assert_eq!(two[1]["deviceStatus"], "connected");

        // …003 → three devices [connected, disconnected, unavailable].
        let three = device_list("+123456789003");
        assert_eq!(three.len(), 3);
        assert_eq!(three[0]["deviceStatus"], "connected");
        assert_eq!(three[1]["deviceStatus"], "disconnected");
        assert_eq!(three[2]["deviceStatus"], "unavailable");

        // …004 wraps the count back to 1.
        assert_eq!(device_list("+123456789004").len(), 1);
    }

    #[test]
    fn ids_and_macs_are_deterministic_and_well_shaped() {
        let a = device_list("+123456789002");
        let b = device_list("+123456789002");
        // Deterministic per (subject, index).
        assert_eq!(a, b);
        // Distinct devices get distinct ids.
        assert_ne!(a[0]["id"], a[1]["id"]);

        let id = a[0]["id"].as_str().unwrap();
        // UUID-shaped: 8-4-4-4-12 lowercase hex.
        let groups: Vec<&str> = id.split('-').collect();
        assert_eq!(groups.iter().map(|g| g.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));

        let mac = a[0]["hardwareAddress"]["value"].as_str().unwrap();
        let octets: Vec<&str> = mac.split(':').collect();
        assert_eq!(octets.len(), 6);
        assert!(octets.iter().all(|o| o.len() == 2 && o.chars().all(|c| c.is_ascii_hexdigit())));
        assert_eq!(a[0]["hardwareAddress"]["hardwareAddressType"], "EUI-48");
    }

    // --- Integration through the real router -------------------------------

    /// App with the auth routes (token endpoint) and the Network Access Devices
    /// routes, so a real token can be minted and presented.
    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials` with a caller-chosen
    /// `client_id` (which becomes the token `sub`), host-pinned so its `aud`
    /// matches the route's audience. `+` is percent-encoded so an E.164 client
    /// id survives the urlencoded body.
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

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "nad-client").await
    }

    /// GET the listing with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_devices(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-devices/vwip/network-access-devices")
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::empty()).unwrap())
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

    /// GET one device by id with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_device_by_id(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/network-access-devices/vwip/network-access-devices/{id}"
            ))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::empty()).unwrap())
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

    #[tokio::test]
    async fn happy_path_lists_a_single_connected_device() {
        // sub = client_id "nad-client" (no digits) → one connected device.
        let token = mint_token(LIST_SCOPE).await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["deviceStatus"], "connected");
        assert_eq!(arr[0]["name"], "Gateway-1");
        assert!(arr[0]["id"].is_string());
    }

    #[tokio::test]
    async fn subject_digits_drive_the_device_count_and_status() {
        // …002 → two devices [unavailable, connected].
        let token = mint_token_with_client(LIST_SCOPE, "+123456789002").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["deviceStatus"], "unavailable");
        assert_eq!(arr[1]["deviceStatus"], "connected");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        // …404 → 404 NOT_FOUND.
        let token = mint_token_with_client(LIST_SCOPE, "+123456789404").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // user-503 → 503 UNAVAILABLE (subject need not be a phone number).
        let token = mint_token_with_client(LIST_SCOPE, "user-503").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_devices(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_devices(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        // Success.
        let token = mint_token(LIST_SCOPE).await;
        let (status, headers, _) = get_devices(Some(&token), Some("corr-nad")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nad")
        );

        // Business error.
        let token = mint_token_with_client(LIST_SCOPE, "+123456789404").await;
        let (status, headers, _) = get_devices(Some(&token), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- getNetworkAccessDevice (read by id) -------------------------------

    #[tokio::test]
    async fn read_by_id_returns_the_device_from_the_subjects_set() {
        // …002 → two devices; read each id back and confirm it round-trips.
        let token = mint_token_with_client(LIST_SCOPE, "+123456789002").await;
        let (_, _, list) = get_devices(Some(&token), None).await;
        let arr = list.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        for expected in arr {
            let id = expected["id"].as_str().unwrap();
            let (status, _, body) = get_device_by_id(Some(&token), id, None).await;
            assert_eq!(status, StatusCode::OK);
            // The read returns exactly the listed device (not an array).
            assert_eq!(&body, expected);
        }
    }

    #[tokio::test]
    async fn read_by_unknown_id_is_not_found() {
        let token = mint_token(LIST_SCOPE).await;
        // An id that is not part of this subject's deterministic set.
        let (status, _, body) =
            get_device_by_id(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // A malformed (non-UUID) id is likewise simply not found — no 400.
        let (status, _, body) = get_device_by_id(Some(&token), "not-a-real-id", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_by_id_belonging_to_another_subscriber_is_not_found() {
        // Mint the id from one subject, then try to read it as a different one.
        let owner = "+123456789002";
        let owner_list = device_list(owner);
        let foreign_id = owner_list[0]["id"].as_str().unwrap().to_string();

        // A different subject (no digits → its own single-device set).
        let token = mint_token_with_client(LIST_SCOPE, "other-subscriber").await;
        let (status, _, body) = get_device_by_id(Some(&token), &foreign_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn read_by_id_reserved_subject_suffix_selects_a_canonical_error() {
        // A reserved-suffix subject takes the whole account into a canonical
        // error, regardless of the id (mirrors the listing).
        let token = mint_token_with_client(LIST_SCOPE, "+123456789503").await;
        let (status, _, body) =
            get_device_by_id(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn read_by_id_requires_the_scope_and_a_token() {
        // Wrong scope → 403.
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            get_device_by_id(Some(&token), "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");

        // No token → 401.
        let (status, _, body) =
            get_device_by_id(None, "00000000-0000-0000-0000-000000000000", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn read_by_id_echoes_x_correlator_on_success_and_error() {
        let token = mint_token(LIST_SCOPE).await;
        let (_, _, list) = get_devices(Some(&token), None).await;
        let id = list.as_array().unwrap()[0]["id"].as_str().unwrap().to_string();

        // Success.
        let (status, headers, _) = get_device_by_id(Some(&token), &id, Some("corr-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        // Not-found error.
        let (status, headers, _) =
            get_device_by_id(Some(&token), "not-a-real-id", Some("corr-nf")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nf")
        );
    }
}
