//! Dedicated Network — Networks **vwip** (CAMARA DedicatedNetworks,
//! work-in-progress).
//!
//! Endpoints:
//! - `POST /dedicated-network/vwip/networks` — create a dedicated network
//!   (operationId `createNetwork`).
//!
//! ## What it does
//!
//! A **dedicated network** is a customer-provisioned private network with a
//! chosen connectivity profile (`networkProfileId`, from the companion
//! **Network Profiles** catalog, *or* a bare `qosProfileName`), a `serviceTime`
//! window, and a `serviceAreaId` naming where it applies. `createNetwork`
//! accepts a `CreateNetwork` body, mints an opaque UUID `id`, records the
//! rendered `NetworkInfo` in an in-memory store ([`super::store`]) so later
//! passes' read/list/delete legs can address it, and returns `201`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `dedicated-network:networks:create` scope. It is a two-legged
//! (`client_credentials`) provisioning call — the network exists independently of
//! any single subscriber, so there is no device/line identifier and no
//! three-legged dance.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes:
//!
//! 1. **Request validation → 400.** A body that is not a valid `CreateNetwork`,
//!    a `name` outside `1..=1024` chars, neither-or-both of
//!    `networkProfileId`/`qosProfileName` (the schema `oneOf`), a non-UUID
//!    `networkProfileId`/`serviceAreaId`, a malformed/empty `qosProfileName`, a
//!    `serviceTime` missing/`malformed` `start`/`end`, or a `sink` not matching
//!    `^https://.+$` → `400 INVALID_ARGUMENT`. A `serviceTime` whose (UTC) `end`
//!    precedes its `start` → `400 OUT_OF_RANGE`.
//! 2. **`serviceAreaId` identifier plane.** The required `serviceAreaId` UUID is
//!    the identifier. Its trailing three digits drive the result (docs/DESIGN.md
//!    §7):
//!    - A reserved suffix ([`crate::scenarios`]) → the canonical CAMARA error —
//!      e.g. `…404` → `404 NOT_FOUND` (no such service area), `…422` → `422`.
//!    - Otherwise `d % 3` selects the created network's lifecycle `status`:
//!      `0` (incl. `…000`/no-digits) → `REQUESTED`, `1` → `RESERVED`, `2` →
//!      `ACTIVATED`. (`TERMINATED` is a teardown state, never assigned at
//!      creation.) So `serviceAreaId` is a genuine second plane.
//!
//! `sinkCredential` is accepted but never echoed (it is a secret) and — this
//! slice being create-only — no notification is delivered (a documented cut;
//! notifications, read/list/delete arrive in later passes). `x-correlator` is
//! echoed on every response.

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

use super::store;

/// The OAuth2 scope `createNetwork` requires (CAMARA Dedicated Network —
/// Networks).
const CREATE_SCOPE: &str = "dedicated-network:networks:create";

/// Routes for Dedicated Network — Networks vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/dedicated-network/vwip/networks", post(create_network))
}

/// A `CreateNetwork` request body (CAMARA `BaseNetworkInfo`). Every field is
/// optional at the serde layer; required-ness and the `oneOf`
/// (`networkProfileId` xor `qosProfileName`) are enforced in the handler so each
/// failure maps to the right CAMARA error.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateNetwork {
    name: Option<String>,
    #[serde(rename = "networkProfileId")]
    network_profile_id: Option<String>,
    #[serde(rename = "qosProfileName")]
    qos_profile_name: Option<String>,
    #[serde(rename = "serviceTime")]
    service_time: Option<ServiceTime>,
    #[serde(rename = "serviceAreaId")]
    service_area_id: Option<String>,
    sink: Option<String>,
    // Accepted but never applied/echoed (a secret; notifications are a later
    // pass). Kept in the struct so `deny_unknown_fields` still admits it.
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
    sink_credential: Option<Value>,
}

/// The `serviceTime` window (CAMARA `ServiceTime`): both bounds required.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceTime {
    start: Option<String>,
    end: Option<String>,
}

/// `POST /dedicated-network/vwip/networks` — create a dedicated network
/// (`createNetwork`).
async fn create_network(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory; parse strictly (`deny_unknown_fields`).
    let req: CreateNetwork = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid CreateNetwork.", &correlator)
        }
    };

    // `name` (optional): schema `minLength: 1`, `maxLength: 1024`.
    if let Some(name) = req.name.as_deref() {
        let len = name.chars().count();
        if !(1..=1024).contains(&len) {
            return invalid_argument("`name` must be 1–1024 characters.", &correlator);
        }
    }

    // oneOf: exactly one of `networkProfileId` / `qosProfileName`.
    match (req.network_profile_id.as_deref(), req.qos_profile_name.as_deref()) {
        (Some(_), Some(_)) => {
            return invalid_argument(
                "exactly one of `networkProfileId` / `qosProfileName` is required, not both.",
                &correlator,
            )
        }
        (None, None) => {
            return invalid_argument(
                "one of `networkProfileId` / `qosProfileName` is required.",
                &correlator,
            )
        }
        _ => {}
    }
    // A supplied `networkProfileId` must be UUID-shaped (schema `format: uuid`).
    if let Some(id) = req.network_profile_id.as_deref() {
        if !is_uuid_shaped(id) {
            return invalid_argument("`networkProfileId` must be a UUID.", &correlator);
        }
    }
    // A supplied `qosProfileName` must be non-empty (schema `minLength: 1`).
    if let Some(name) = req.qos_profile_name.as_deref() {
        if name.is_empty() {
            return invalid_argument("`qosProfileName` must not be empty.", &correlator);
        }
    }

    // `serviceTime` (required): both `start` and `end` present + RFC 3339.
    let service_time = match req.service_time {
        None => return invalid_argument("`serviceTime` is required.", &correlator),
        Some(st) => st,
    };
    let (start, end) = match (service_time.start.as_deref(), service_time.end.as_deref()) {
        (Some(s), Some(e)) => (s.to_string(), e.to_string()),
        _ => {
            return invalid_argument(
                "`serviceTime` must contain both `start` and `end`.",
                &correlator,
            )
        }
    };
    for (field, value) in [("start", &start), ("end", &end)] {
        if !is_rfc3339_datetime(value) {
            return invalid_argument(
                &format!("`serviceTime.{field}` must be an RFC 3339 date-time."),
                &correlator,
            );
        }
    }
    // A (UTC) window whose `end` precedes its `start` is an invalid range.
    // Compared on the fixed-width `YYYY-MM-DDTHH:MM:SS` prefix, which is
    // lexically == chronologically for the `Z`/UTC form (a documented scope: a
    // non-UTC offset is accepted without the ordering check).
    if is_utc(&start) && is_utc(&end) && end[..19] < start[..19] {
        return out_of_range("`serviceTime.end` must not precede `serviceTime.start`.", &correlator);
    }

    // `serviceAreaId` (required): must be UUID-shaped.
    let service_area_id = match req.service_area_id.as_deref() {
        Some(id) if is_uuid_shaped(id) => id.to_string(),
        Some(_) => return invalid_argument("`serviceAreaId` must be a UUID.", &correlator),
        None => return invalid_argument("`serviceAreaId` is required.", &correlator),
    };

    // `sink` (optional): schema `pattern: ^https://.+$`.
    if let Some(sink) = req.sink.as_deref() {
        if !(sink.starts_with("https://") && sink.len() > "https://".len()) {
            return invalid_argument("`sink` must be an `https://` URI.", &correlator);
        }
    }

    // Control plane: a reserved suffix on `serviceAreaId` selects a canonical
    // CAMARA error (e.g. `…404` → no such service area).
    if let Some(err) = scenarios::reserved_error(&service_area_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Otherwise the `serviceAreaId` trailing three digits pick the lifecycle
    // status.
    let status = status_for(&service_area_id);

    // Build the NetworkInfo, remember it, and return 201.
    let id = store::new_network_id();
    let info = build_network_info(
        &id,
        status,
        req.name.as_deref(),
        req.network_profile_id.as_deref(),
        req.qos_profile_name.as_deref(),
        &start,
        &end,
        &service_area_id,
        req.sink.as_deref(),
    );
    store::insert(id.clone(), info.clone());

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// The lifecycle `status` a `serviceAreaId` maps to (its trailing three digits
/// `d % 3`): `0` → `REQUESTED`, `1` → `RESERVED`, `2` → `ACTIVATED`. `…000` /
/// no-digits → `REQUESTED`. `TERMINATED` is a teardown state, never assigned at
/// creation. Pure over its input, so it is unit-tested directly.
fn status_for(service_area_id: &str) -> &'static str {
    let d = scenarios::trailing_three_digits(service_area_id).unwrap_or(0);
    match d % 3 {
        0 => "REQUESTED",
        1 => "RESERVED",
        _ => "ACTIVATED",
    }
}

/// Render a `NetworkInfo`: the minted `id` and derived `status`, plus the
/// caller's fields echoed back. `sinkCredential` is deliberately omitted (a
/// secret). Pure over its inputs, so it is unit-tested directly.
#[allow(clippy::too_many_arguments)]
fn build_network_info(
    id: &str,
    status: &str,
    name: Option<&str>,
    network_profile_id: Option<&str>,
    qos_profile_name: Option<&str>,
    start: &str,
    end: &str,
    service_area_id: &str,
    sink: Option<&str>,
) -> Value {
    let mut info = json!({
        "id": id,
        "status": status,
        "serviceTime": { "start": start, "end": end },
        "serviceAreaId": service_area_id,
    });
    let obj = info.as_object_mut().expect("info is an object");
    if let Some(name) = name {
        obj.insert("name".into(), json!(name));
    }
    if let Some(v) = network_profile_id {
        obj.insert("networkProfileId".into(), json!(v));
    }
    if let Some(v) = qos_profile_name {
        obj.insert("qosProfileName".into(), json!(v));
    }
    if let Some(v) = sink {
        obj.insert("sink".into(), json!(v));
    }
    info
}

/// Whether `s` is UUID-shaped: five hyphen-separated hex groups of lengths
/// `8-4-4-4-12` (the schema's `format: uuid`). Mirrors the other UUID-keyed APIs.
fn is_uuid_shaped(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// Whether `s` is a syntactically valid RFC 3339 date-time
/// (`YYYY-MM-DDThh:mm:ss[.fff](Z|±hh:mm)`), within the schema `maxLength: 64`.
/// A lightweight shape check — no calendar validation (a self-contained parser,
/// no dependency).
fn is_rfc3339_datetime(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > 64 || b.len() < 20 {
        return false;
    }
    let digit = |i: usize| b[i].is_ascii_digit();
    // YYYY-MM-DD
    if !(digit(0) && digit(1) && digit(2) && digit(3)) || b[4] != b'-' {
        return false;
    }
    if !(digit(5) && digit(6)) || b[7] != b'-' {
        return false;
    }
    if !(digit(8) && digit(9)) {
        return false;
    }
    // T (RFC 3339 allows a lower-case 't' too)
    if !(b[10] == b'T' || b[10] == b't') {
        return false;
    }
    // hh:mm:ss
    if !(digit(11) && digit(12)) || b[13] != b':' {
        return false;
    }
    if !(digit(14) && digit(15)) || b[16] != b':' {
        return false;
    }
    if !(digit(17) && digit(18)) {
        return false;
    }
    // Optional fractional seconds.
    let mut i = 19;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return false; // a dot with no digits
        }
    }
    // Mandatory offset: `Z`/`z` or `±hh:mm`.
    if i >= b.len() {
        return false;
    }
    match b[i] {
        b'Z' | b'z' => i + 1 == b.len(),
        b'+' | b'-' => {
            i + 6 == b.len()
                && digit(i + 1)
                && digit(i + 2)
                && b[i + 3] == b':'
                && digit(i + 4)
                && digit(i + 5)
        }
        _ => false,
    }
}

/// Whether an RFC 3339 date-time is in UTC (`Z`/`z` form). Only UTC pairs get the
/// `start`/`end` ordering check (see [`create_network`]).
fn is_utc(s: &str) -> bool {
    matches!(s.bytes().last(), Some(b'Z') | Some(b'z'))
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "dedicatednet.local:8080";

    // A UUID whose trailing three digits are `d` (zero-padded), so a test can
    // aim at a specific reserved suffix or status.
    fn uuid_ending(d: u16) -> String {
        format!("00000000-0000-4000-8000-000000000{d:03}")
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_shape_matches_the_schema() {
        assert!(is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid_shaped(&uuid_ending(1)));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa")); // 11 in last group
        assert!(!is_uuid_shaped("zzzzzzzz-5717-4562-b3fc-2c963f66afa6")); // non-hex
    }

    #[test]
    fn status_selection_covers_the_three_creation_states() {
        assert_eq!(status_for(&uuid_ending(0)), "REQUESTED");
        assert_eq!(status_for(&uuid_ending(1)), "RESERVED");
        assert_eq!(status_for(&uuid_ending(2)), "ACTIVATED");
        // Wraps mod 3.
        assert_eq!(status_for(&uuid_ending(3)), "REQUESTED");
        assert_eq!(status_for(&uuid_ending(5)), "ACTIVATED");
        // No significant digits → REQUESTED.
        assert_eq!(status_for("abcdefab-0000-4000-8000-abcdefabcdef"), "REQUESTED");
    }

    #[test]
    fn rfc3339_shape_check() {
        assert!(is_rfc3339_datetime("2024-01-01T00:00:00Z"));
        assert!(is_rfc3339_datetime("2024-06-15T12:34:56.789Z"));
        assert!(is_rfc3339_datetime("2024-06-15T12:34:56+02:00"));
        assert!(is_rfc3339_datetime("2024-06-15t12:34:56z"));
        assert!(!is_rfc3339_datetime("2024-01-01")); // date only
        assert!(!is_rfc3339_datetime("2024-01-01T00:00:00")); // no offset
        assert!(!is_rfc3339_datetime("2024-01-01T00:00:00.Z")); // empty fraction
        assert!(!is_rfc3339_datetime("not-a-date"));
        assert!(!is_rfc3339_datetime(&"9".repeat(65))); // over maxLength
    }

    #[test]
    fn build_network_info_omits_secret_and_absent_fields() {
        let info = build_network_info(
            "id-1",
            "RESERVED",
            None,
            Some("00000000-0000-4000-8000-000000000abc"),
            None,
            "2024-01-01T00:00:00Z",
            "2024-01-02T00:00:00Z",
            "area-1",
            None,
        );
        assert_eq!(info["id"], "id-1");
        assert_eq!(info["status"], "RESERVED");
        assert_eq!(info["networkProfileId"], "00000000-0000-4000-8000-000000000abc");
        assert_eq!(info["serviceTime"]["start"], "2024-01-01T00:00:00Z");
        assert_eq!(info["serviceAreaId"], "area-1");
        // Absent optionals + the secret are not present.
        assert!(info.get("name").is_none());
        assert!(info.get("qosProfileName").is_none());
        assert!(info.get("sink").is_none());
        assert!(info.get("sinkCredential").is_none());
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=dn-client&scope={scope}");
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

    async fn post_network(
        token: Option<&str>,
        body: Value,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/dedicated-network/vwip/networks")
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

    // A valid CreateNetwork body whose serviceAreaId ends in `d`.
    fn valid_body(area_tail: u16) -> Value {
        json!({
            "name": "factory-floor",
            "networkProfileId": "00000000-0000-4000-8000-000000000abc",
            "serviceTime": {
                "start": "2024-01-01T00:00:00Z",
                "end": "2024-12-31T23:59:59Z"
            },
            "serviceAreaId": uuid_ending(area_tail),
        })
    }

    async fn create_ok(body: Value) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_network(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn happy_path_creates_and_echoes_the_request() {
        let (status, _, body) = create_ok(valid_body(1)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(is_uuid_shaped(body["id"].as_str().unwrap()));
        assert_eq!(body["status"], "RESERVED"); // tail 001 → d%3 == 1
        assert_eq!(body["name"], "factory-floor");
        assert_eq!(body["networkProfileId"], "00000000-0000-4000-8000-000000000abc");
        assert_eq!(body["serviceTime"]["start"], "2024-01-01T00:00:00Z");
        assert_eq!(body["serviceAreaId"], uuid_ending(1));
        // The secret is never echoed.
        assert!(body.get("sinkCredential").is_none());
    }

    #[tokio::test]
    async fn service_area_id_drives_the_status() {
        let (_, _, req) = create_ok(valid_body(0)).await;
        assert_eq!(req["status"], "REQUESTED");
        let (_, _, res) = create_ok(valid_body(2)).await;
        assert_eq!(res["status"], "ACTIVATED");
    }

    #[tokio::test]
    async fn created_network_is_persisted() {
        let (_, _, body) = create_ok(valid_body(1)).await;
        let id = body["id"].as_str().unwrap();
        assert_eq!(store::get(id), Some(body));
    }

    #[tokio::test]
    async fn qos_profile_name_alternative_is_accepted_and_echoed() {
        let body = json!({
            "qosProfileName": "voice",
            "serviceTime": { "start": "2024-01-01T00:00:00Z", "end": "2024-06-01T00:00:00Z" },
            "serviceAreaId": uuid_ending(2),
        });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(res["qosProfileName"], "voice");
        assert!(res.get("networkProfileId").is_none());
        assert_eq!(res["status"], "ACTIVATED");
    }

    #[tokio::test]
    async fn reserved_service_area_suffix_selects_a_canonical_error() {
        let (status, _, body) = create_ok(valid_body(404)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = create_ok(valid_body(429)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");

        let (status, _, body) = create_ok(valid_body(422)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn oneof_profile_constraint_is_enforced() {
        // Both → 400.
        let body = json!({
            "networkProfileId": "00000000-0000-4000-8000-000000000abc",
            "qosProfileName": "voice",
            "serviceTime": { "start": "2024-01-01T00:00:00Z", "end": "2024-06-01T00:00:00Z" },
            "serviceAreaId": uuid_ending(1),
        });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(res["code"], "INVALID_ARGUMENT");

        // Neither → 400.
        let body = json!({
            "serviceTime": { "start": "2024-01-01T00:00:00Z", "end": "2024-06-01T00:00:00Z" },
            "serviceAreaId": uuid_ending(1),
        });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(res["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_fields_are_400() {
        // Non-UUID serviceAreaId.
        let body = json!({
            "networkProfileId": "00000000-0000-4000-8000-000000000abc",
            "serviceTime": { "start": "2024-01-01T00:00:00Z", "end": "2024-06-01T00:00:00Z" },
            "serviceAreaId": "not-a-uuid",
        });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(res["code"], "INVALID_ARGUMENT");

        // Missing serviceTime.end.
        let body = json!({
            "networkProfileId": "00000000-0000-4000-8000-000000000abc",
            "serviceTime": { "start": "2024-01-01T00:00:00Z" },
            "serviceAreaId": uuid_ending(1),
        });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(res["code"], "INVALID_ARGUMENT");

        // Non-https sink.
        let mut body = valid_body(1);
        body["sink"] = json!("http://insecure.example/cb");
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(res["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn end_before_start_is_out_of_range() {
        let body = json!({
            "networkProfileId": "00000000-0000-4000-8000-000000000abc",
            "serviceTime": { "start": "2024-12-31T00:00:00Z", "end": "2024-01-01T00:00:00Z" },
            "serviceAreaId": uuid_ending(1),
        });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(res["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn https_sink_is_accepted_and_echoed_but_credential_is_not() {
        let mut body = valid_body(2);
        body["sink"] = json!("https://callback.example/events");
        body["sinkCredential"] = json!({ "credentialType": "ACCESSTOKEN", "accessToken": "s3cr3t", "accessTokenType": "bearer" });
        let (status, _, res) = create_ok(body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(res["sink"], "https://callback.example/events");
        assert!(res.get("sinkCredential").is_none());
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_network(Some(&token), valid_body(1), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_network(None, valid_body(1), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, _) =
            post_network(Some(&token), valid_body(1), Some("corr-ok")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        let (status, headers, _) =
            post_network(Some(&token), valid_body(404), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
