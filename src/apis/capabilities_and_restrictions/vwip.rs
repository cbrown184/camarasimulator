//! Capabilities and Restrictions **vwip** (CAMARA
//! CapabilitiesAndRuntimeRestrictions, `wip`).
//!
//! One endpoint:
//! - `POST /capabilities-and-restrictions/vwip/retrieve` — query the tailored
//!   service capabilities (and their active/inactive state) for a context.
//!
//! ## What it does
//!
//! The caller submits one or more `queries`. Each query names the API
//! definitions it `overlayExtends` (a non-empty list of definition URLs) and,
//! optionally, the `resourceScopes` it is asking about (e.g. a `phoneNumber`).
//! The operator returns a `CapabilityInfo` whose `details` holds one capability
//! set per query: a fixed catalogue of runtime `bitmapCapabilities` (each a set
//! of overlay restrictions the consumer must enforce) plus a
//! `camaraCapabilitiesBitmap` integer whose bits say which of those capabilities
//! are currently **active** for the queried context.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `camara-capability:read` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The identifier is the **first query's first `resourceScope` `phoneNumber`**
//! (when one is supplied):
//!
//! - **Reserved error suffix (identifier).** If that phone number's trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with that
//!   canonical CAMARA error (shared [`crate::scenarios`]); `…404` is the API's
//!   "No Capability API found" case.
//! - **Happy path.** Otherwise `201` with a `CapabilityInfo`. For each query the
//!   `camaraCapabilitiesBitmap` is derived from the context — the queried phone
//!   number's trailing three digits (or, absent a phone number, a stable hash of
//!   the query's `overlayExtends`) modulo the catalogue size — so the *active*
//!   capability set is a genuine, input-driven second control plane. The
//!   `overlayExtends`/`resourceScopes` of each query are echoed back on its
//!   detail.
//!
//! Validation: a malformed body, an empty/absent `overlayExtends`, an
//! `overlayExtends` entry that is not a URI, or a non-E.164 `phoneNumber` in a
//! `resourceScope` → `400 INVALID_ARGUMENT`; more than 100 `queries` or more
//! than 20 `overlayExtends` in a query → `400 OUT_OF_RANGE`. `x-correlator` is
//! echoed on every response.
//!
//! ## Documented cuts
//!
//! CamaraSim implements the synchronous `POST /retrieve` happy path plus the
//! standard error set. Out of scope for this slice (documented, mirroring the
//! other first-landing API skeletons):
//! - the `subscriptionRequest` change-notification callback (accepted but not
//!   applied — no CloudEvents are delivered);
//! - the `CapabilitySetFootprint` branch of `CapabilityDetail` (CamaraSim always
//!   answers with the `CapabilitySetBitmap` + `CapabilityBitmap` branch);
//! - `ETag`/`If-None-Match`/`If-Modified-Since` caching and the `304 Not
//!   Modified` response;
//! - resolving `overlayExtends` against real, published API definitions (the
//!   restriction catalogue is synthetic).

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Capabilities).
const READ_SCOPE: &str = "camara-capability:read";

/// Upper bounds from the vendored schema (`queries` `maxItems`, per-query
/// `overlayExtends` `maxItems`).
const MAX_QUERIES: usize = 100;
const MAX_OVERLAY_EXTENDS: usize = 20;

/// The number of runtime capabilities in the (synthetic) catalogue. Each query's
/// answer enumerates all `CATALOGUE_SIZE` of them in `bitmapCapabilities` (keyed
/// by bit position `"0"`..) and the `camaraCapabilitiesBitmap` integer marks
/// which are active — so the integer ranges over `0..2^CATALOGUE_SIZE`.
const CATALOGUE_SIZE: usize = 3;

/// Routes for Capabilities and Restrictions vwip, mounted at their canonical URL.
pub fn routes() -> Router {
    Router::new().route(
        "/capabilities-and-restrictions/vwip/retrieve",
        post(retrieve),
    )
}

/// One entry of a query's `resourceScopes`. The upstream schema allows several
/// identifier kinds; CamaraSim only reads `phoneNumber` (the error/derivation
/// plane) and ignores any other keys.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceScope {
    #[serde(default)]
    phone_number: Option<String>,
}

/// A single `CamaraCapabilityQuery`: the API definitions it extends and,
/// optionally, the resource scopes it asks about.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    #[serde(default)]
    resource_scopes: Option<Vec<ResourceScope>>,
    #[serde(default)]
    overlay_extends: Option<Vec<String>>,
}

/// `POST /retrieve` request body (`CamaraCapabilityQueryRequest`). Unknown
/// fields (including `subscriptionRequest`, accepted-not-applied) are ignored per
/// the schema's `additionalProperties: true`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryRequest {
    queries: Vec<Query>,
}

/// `POST /capabilities-and-restrictions/vwip/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // A malformed body / missing `queries` surfaces here.
    let req: QueryRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid CamaraCapabilityQueryRequest.",
                &correlator,
            )
        }
    };

    // `queries` — at least one, at most `MAX_QUERIES` (schema `minItems`/`maxItems`).
    if req.queries.is_empty() {
        return invalid_argument("`queries` must contain at least one query.", &correlator);
    }
    if req.queries.len() > MAX_QUERIES {
        return out_of_range(
            &format!("`queries` must contain at most {MAX_QUERIES} entries."),
            &correlator,
        );
    }

    // Per-query validation (`overlayExtends` required + bounded + URIs;
    // `resourceScopes` phone numbers E.164).
    for query in &req.queries {
        let overlays = query.overlay_extends.as_deref().unwrap_or(&[]);
        if overlays.is_empty() {
            return invalid_argument(
                "Each query requires a non-empty `overlayExtends`.",
                &correlator,
            );
        }
        if overlays.len() > MAX_OVERLAY_EXTENDS {
            return out_of_range(
                &format!("`overlayExtends` must contain at most {MAX_OVERLAY_EXTENDS} entries."),
                &correlator,
            );
        }
        if !overlays.iter().all(|u| is_uri(u)) {
            return invalid_argument(
                "Each `overlayExtends` entry must be a URI.",
                &correlator,
            );
        }
        if let Some(scopes) = &query.resource_scopes {
            for scope in scopes {
                if let Some(pn) = &scope.phone_number {
                    if !is_valid_e164(pn) {
                        return invalid_argument(
                            "A `resourceScopes` `phoneNumber` must be in E.164 format.",
                            &correlator,
                        );
                    }
                }
            }
        }
    }

    // Error plane (docs/DESIGN.md §7): the first query's first resource-scope
    // phone number selects a canonical CAMARA error when its trailing three
    // digits name a reserved status (…404 = "No Capability API found").
    if let Some(pn) = first_phone_number(&req) {
        if let Some(err) = scenarios::reserved_error(pn) {
            return with_correlator(err.into_response(), &correlator);
        }
    }

    // Happy path: one capability detail per query.
    let details: Vec<Value> = req.queries.iter().map(capability_detail).collect();
    with_correlator(
        (StatusCode::CREATED, Json(json!({ "details": details }))).into_response(),
        &correlator,
    )
}

/// The first `phoneNumber` across the request's first query's resource scopes,
/// if any — the reserved-error identifier.
fn first_phone_number(req: &QueryRequest) -> Option<&str> {
    req.queries
        .first()?
        .resource_scopes
        .as_ref()?
        .iter()
        .find_map(|s| s.phone_number.as_deref())
}

/// Build a conformant `CapabilityDetail` (the `CapabilitySetBitmap` +
/// `CapabilityBitmap` branch) for one query. The active-capabilities bitmap is
/// derived from the queried context so it is input-driven.
fn capability_detail(query: &Query) -> Value {
    let overlays = query.overlay_extends.as_deref().unwrap_or(&[]);
    let primary_overlay = overlays.first().map(String::as_str).unwrap_or("");

    // Seed the active bitmap from the context: the queried phone number's
    // trailing three digits, else a stable hash of the query's overlayExtends.
    let seed = query
        .resource_scopes
        .as_ref()
        .and_then(|s| s.iter().find_map(|s| s.phone_number.as_deref()))
        .and_then(scenarios::trailing_three_digits)
        .map(u64::from)
        .unwrap_or_else(|| hash_seed(overlays));
    let bitmap = seed % (1u64 << CATALOGUE_SIZE); // 0..2^CATALOGUE_SIZE

    // Enumerate the whole catalogue in `bitmapCapabilities` (bit position → set);
    // the integer marks which are active.
    let mut bitmap_capabilities = Map::new();
    for i in 0..CATALOGUE_SIZE {
        bitmap_capabilities.insert(i.to_string(), restriction_set(i, primary_overlay));
    }

    let now = rfc3339_utc(now_unix_secs());
    let mut detail = json!({
        "name": "camarasim-capability-set",
        "version": "1.0.0",
        "createdAt": now,
        "lastModifiedAt": now,
        "mappingVersion": "1.0.0",
        "camaraCapabilitiesBitmap": bitmap,
        "bitmapCapabilities": Value::Object(bitmap_capabilities),
        "overlayExtends": overlays,
    });
    // Echo the queried resource scopes when supplied (optional in the schema).
    if let Some(scopes) = &query.resource_scopes {
        let echoed: Vec<Value> = scopes
            .iter()
            .filter_map(|s| s.phone_number.as_ref())
            .map(|pn| json!({ "phoneNumber": pn }))
            .collect();
        if !echoed.is_empty() {
            detail["resourceScopes"] = Value::Array(echoed);
        }
    }
    detail
}

/// One catalogue entry: a `SchemaRestrictionsSet` naming a single overlay
/// restriction the consumer must enforce. Deterministic per position.
fn restriction_set(position: usize, extends: &str) -> Value {
    // A small, fixed set of representative CAMARA runtime restrictions.
    const RESTRICTIONS: [(&str, &str); CATALOGUE_SIZE] = [
        (
            "rr-accepted-device-identifiers",
            "$.components.schemas['Device'].properties['ipv4Address']",
        ),
        ("rr-x-correlator-required", "$.components.parameters['x-correlator']"),
        (
            "rr-phone-number-only",
            "$.components.schemas['Device'].properties['networkAccessIdentifier']",
        ),
    ];
    let (name, target) = RESTRICTIONS[position];
    json!({
        "name": name,
        "version": "1.0.0",
        "restrictions": [{
            "name": format!("org.camaraproject.capability.v0.object-instance-restriction.{name}"),
            "version": "0.0.1",
            "extends": extends,
            "actions": [{ "target": target, "remove": true }],
        }],
    })
}

/// A stable, order-sensitive seed derived from a query's `overlayExtends`, used
/// when no queried phone number pins the active bitmap. FNV-1a over the joined
/// list; no dependency, and deterministic across runs (mirrors Consent Info's
/// hash-derived token).
fn hash_seed(overlays: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a offset basis
    for overlay in overlays {
        for byte in overlay.bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
        }
        h ^= u64::from(b'\n');
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Whether `s` looks like a URI: a non-empty string carrying a scheme separator
/// (`scheme://…`). Deliberately permissive — the simulator does not resolve the
/// definitions, only checks the shape.
fn is_uri(s: &str) -> bool {
    !s.is_empty() && s.contains("://")
}

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
}

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset. Self-contained (no date/time dependency; mirrors `short_message_service`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian).
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

/// A 400 `INVALID_ARGUMENT` CAMARA error, correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
}

/// A 400 `OUT_OF_RANGE` CAMARA error, correlator echoed.
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

    const HOST: &str = "capabilities.local:8080";
    const OVERLAY: &str =
        "https://github.com/camaraproject/QualityOnDemand/blob/r2.2/code/API_definitions/quality-on-demand.yaml";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uri_shape_check() {
        assert!(is_uri("https://example.com/a.yaml"));
        assert!(is_uri("http://x"));
        assert!(!is_uri(""));
        assert!(!is_uri("not-a-uri"));
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
    }

    #[test]
    fn hash_seed_is_order_sensitive_and_deterministic() {
        let a = hash_seed(&["a".into(), "b".into()]);
        let b = hash_seed(&["a".into(), "b".into()]);
        let c = hash_seed(&["b".into(), "a".into()]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn detail_bitmap_tracks_the_phone_number_and_enumerates_the_catalogue() {
        // …005 → trailing digits 5 → bitmap 5 % 8 = 5.
        let q = Query {
            resource_scopes: Some(vec![ResourceScope {
                phone_number: Some("+123456789005".into()),
            }]),
            overlay_extends: Some(vec![OVERLAY.into()]),
        };
        let d = capability_detail(&q);
        assert_eq!(d["camaraCapabilitiesBitmap"], json!(5));
        let caps = d["bitmapCapabilities"].as_object().unwrap();
        assert_eq!(caps.len(), CATALOGUE_SIZE);
        for i in 0..CATALOGUE_SIZE {
            assert!(caps.contains_key(&i.to_string()), "bit position {i} present");
        }
        // overlayExtends and resourceScopes are echoed.
        assert_eq!(d["overlayExtends"][0], json!(OVERLAY));
        assert_eq!(d["resourceScopes"][0]["phoneNumber"], json!("+123456789005"));
    }

    #[test]
    fn detail_without_a_phone_number_still_derives_a_bitmap_and_omits_scopes() {
        let q = Query {
            resource_scopes: None,
            overlay_extends: Some(vec![OVERLAY.into()]),
        };
        let d = capability_detail(&q);
        assert!(d["camaraCapabilitiesBitmap"].as_u64().unwrap() < (1 << CATALOGUE_SIZE));
        assert!(d.get("resourceScopes").is_none(), "no scopes echoed");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=cap-client&scope={scope}");
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
            .uri("/capabilities-and-restrictions/vwip/retrieve")
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

    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    fn query_body(phone: Option<&str>) -> String {
        match phone {
            Some(pn) => format!(
                r#"{{"queries":[{{"resourceScopes":[{{"phoneNumber":"{pn}"}}],"overlayExtends":["{OVERLAY}"]}}]}}"#
            ),
            None => format!(r#"{{"queries":[{{"overlayExtends":["{OVERLAY}"]}}]}}"#),
        }
    }

    // --- Happy path --------------------------------------------------------

    #[tokio::test]
    async fn retrieve_returns_201_with_a_capability_detail_per_query() {
        let (status, _, body) = call_ok(&query_body(Some("+123456789012"))).await;
        assert_eq!(status, StatusCode::CREATED);
        let details = body["details"].as_array().unwrap();
        assert_eq!(details.len(), 1);
        let d = &details[0];
        assert!(d["mappingVersion"].is_string());
        assert!(d["camaraCapabilitiesBitmap"].is_u64());
        assert_eq!(
            d["bitmapCapabilities"].as_object().unwrap().len(),
            CATALOGUE_SIZE
        );
        // The primary overlay flows into each restriction's `extends`.
        assert_eq!(d["bitmapCapabilities"]["0"]["restrictions"][0]["extends"], json!(OVERLAY));
    }

    #[tokio::test]
    async fn active_bitmap_is_input_driven() {
        // …001 → bitmap 1, …003 → bitmap 3: the queried number picks the active set.
        let (_, _, one) = call_ok(&query_body(Some("+123456789001"))).await;
        let (_, _, three) = call_ok(&query_body(Some("+123456789003"))).await;
        assert_eq!(one["details"][0]["camaraCapabilitiesBitmap"], json!(1));
        assert_eq!(three["details"][0]["camaraCapabilitiesBitmap"], json!(3));
    }

    #[tokio::test]
    async fn multiple_queries_yield_multiple_details() {
        let body = format!(
            r#"{{"queries":[{{"overlayExtends":["{OVERLAY}"]}},{{"overlayExtends":["{OVERLAY}"]}}]}}"#
        );
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(out["details"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn subscription_request_is_accepted_not_applied() {
        // An unknown-to-us `subscriptionRequest` is ignored (additionalProperties).
        let body = format!(
            r#"{{"queries":[{{"overlayExtends":["{OVERLAY}"]}}],"subscriptionRequest":{{"sink":"https://x/y","protocol":"HTTP","types":["org.camaraproject.capability.v0.capability-info-changed"]}}}}"#
        );
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(out["details"].as_array().unwrap().len(), 1);
    }

    // --- Reserved-error convention (first query's first phoneNumber) -------

    #[tokio::test]
    async fn reserved_suffix_on_the_first_phone_number_selects_a_canonical_error() {
        let (status, _, body) = call_ok(&query_body(Some("+123456789404"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = call_ok(&query_body(Some("+123456789429"))).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn no_phone_number_never_triggers_the_error_plane() {
        let (status, _, _) = call_ok(&query_body(None)).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn empty_queries_is_invalid_argument() {
        let (status, _, body) = call_ok(r#"{"queries":[]}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_queries_is_invalid_argument() {
        let (status, _, body) = call_ok(r#"{}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_overlay_extends_is_invalid_argument() {
        let (status, _, body) = call_ok(r#"{"queries":[{}]}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_overlay_extends_is_invalid_argument() {
        let (status, _, body) = call_ok(r#"{"queries":[{"overlayExtends":[]}]}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_uri_overlay_extends_is_invalid_argument() {
        let (status, _, body) = call_ok(r#"{"queries":[{"overlayExtends":["not-a-uri"]}]}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_e164_scope_phone_number_is_invalid_argument() {
        let body = format!(
            r#"{{"queries":[{{"resourceScopes":[{{"phoneNumber":"0123"}}],"overlayExtends":["{OVERLAY}"]}}]}}"#
        );
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn too_many_overlay_extends_is_out_of_range() {
        let overlays = (0..21)
            .map(|i| format!(r#""https://x/{i}.yaml""#))
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(r#"{{"queries":[{{"overlayExtends":[{overlays}]}}]}}"#);
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn too_many_queries_is_out_of_range() {
        let one = format!(r#"{{"overlayExtends":["{OVERLAY}"]}}"#);
        let queries = std::iter::repeat(one).take(101).collect::<Vec<_>>().join(",");
        let body = format!(r#"{{"queries":[{queries}]}}"#);
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = call_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_retrieve(Some(&token), &query_body(None), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_retrieve(None, &query_body(None), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_201_and_error() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            post_retrieve(Some(&token), &query_body(Some("+123456789012")), Some("corr-201")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-201")
        );
        let (status, headers, _) =
            post_retrieve(Some(&token), &query_body(Some("+123456789404")), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
