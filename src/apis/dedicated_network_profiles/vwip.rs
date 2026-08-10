//! Dedicated Network — Network Profiles **vwip** (CAMARA DedicatedNetworks,
//! work-in-progress).
//!
//! One endpoint (this slice):
//! - `GET /dedicated-network-profiles/vwip/profiles/{profileId}` — look up a
//!   single network profile by its id (operationId `readNetworkProfile`).
//!
//! ## What it does
//!
//! A **network profile** is a named, operator-published template for a dedicated
//! network: `maxNumberOfDevices`, the aggregated up/down throughput
//! (`aggregatedUlThroughput` / `aggregatedDlThroughput`), the set of
//! Quality-on-Demand profiles it permits (`qosProfiles`) and the `defaultQosProfile`.
//! CamaraSim serves a **fixed catalog** (there is no upstream network to query —
//! DESIGN §7); the single-profile lookup returns the profile the requested
//! `profileId` maps to, echoing that id back in the body's `id`.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `dedicated-network-profiles:profiles:read` scope. It is a two-legged
//! (`client_credentials`) catalog query — the profiles exist independently of any
//! subscriber, so there is no device/line identifier and no three-legged dance.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The `profileId` path parameter is the sole control plane, in three layers:
//!
//! 1. **Malformed id.** A `profileId` that is not UUID-shaped (the schema's
//!    `format: uuid`, `maxLength: 36`) → `400 INVALID_ARGUMENT`.
//! 2. **Reserved error suffix (identifier).** A UUID contains digits, so the
//!    shared reserved-error convention applies unchanged ([`crate::scenarios`]):
//!    if the id's trailing three digits name a reserved CAMARA status (`…400`,
//!    `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`) the
//!    endpoint answers with that canonical CAMARA error — e.g.
//!    `…-000000000404` → `404 NOT_FOUND` (no such profile).
//! 3. **Profile selection.** Otherwise the id's trailing three digits `d`
//!    (`000`–`999`, or `0` when the id has fewer than three digits) select one of
//!    a fixed set of profile templates (`d % N`), so the returned profile is a
//!    genuine second plane. The returned `id` echoes the requested `profileId`.
//!
//! Example: `…-000000000001` → the `iot-massive` template; `…-000000000002` →
//! `public-safety`; `…-000000000404` → `404 NOT_FOUND`; `not-a-uuid` → `400`.

use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `readNetworkProfile` requires (CAMARA Dedicated Network —
/// Network Profiles).
const READ_SCOPE: &str = "dedicated-network-profiles:profiles:read";

/// Routes for Dedicated Network — Network Profiles vwip, mounted at their
/// canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/dedicated-network-profiles/vwip/profiles/:profileId",
        get(read_network_profile),
    )
}

/// `GET /dedicated-network-profiles/vwip/profiles/{profileId}` — the
/// single-profile lookup (`readNetworkProfile`).
async fn read_network_profile(
    claims: Claims,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Layer 1: the path segment must be a UUID (schema `format: uuid`).
    if !is_uuid_shaped(&profile_id) {
        return invalid_argument("`profileId` must be a UUID.", &correlator);
    }

    // Layer 2: a reserved trailing-digit suffix selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&profile_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Layer 3: the trailing three digits pick a fixed profile template; the
    // requested id is echoed as the returned `id`.
    let digits = scenarios::trailing_three_digits(&profile_id).unwrap_or(0);
    let profile = network_profile(&profile_id, digits);

    with_correlator((StatusCode::OK, Json(profile)).into_response(), &correlator)
}

/// The fixed catalog of network-profile templates CamaraSim offers, as
/// `(name, maxNumberOfDevices, ulThroughput, dlThroughput, qosProfiles, default)`.
/// Static data — a real operator would source these from its dedicated-network
/// policy configuration. Chosen to span a spread of scales (a large IoT fleet, a
/// small high-throughput production network, …) so the `profileId` selection is a
/// meaningful control plane. `BitRate.value` stays within the schema's `1..=1024`.
const TEMPLATES: &[NetworkProfileTemplate] = &[
    NetworkProfileTemplate {
        name: "enterprise-campus",
        max_number_of_devices: 5_000,
        ul: BitRate { value: 200, unit: "Mbps" },
        dl: BitRate { value: 500, unit: "Mbps" },
        qos_profiles: &["voice", "video", "low-latency"],
        default_qos_profile: "video",
    },
    NetworkProfileTemplate {
        name: "iot-massive",
        max_number_of_devices: 100_000,
        ul: BitRate { value: 10, unit: "Mbps" },
        dl: BitRate { value: 10, unit: "Mbps" },
        qos_profiles: &["standard", "low-latency"],
        default_qos_profile: "standard",
    },
    NetworkProfileTemplate {
        name: "public-safety",
        max_number_of_devices: 2_000,
        ul: BitRate { value: 100, unit: "Mbps" },
        dl: BitRate { value: 100, unit: "Mbps" },
        qos_profiles: &["voice", "low-latency"],
        default_qos_profile: "voice",
    },
    NetworkProfileTemplate {
        name: "media-production",
        max_number_of_devices: 500,
        ul: BitRate { value: 1, unit: "Gbps" },
        dl: BitRate { value: 1, unit: "Gbps" },
        qos_profiles: &["video", "low-latency"],
        default_qos_profile: "low-latency",
    },
];

/// A static network-profile template. Rendered to a CAMARA `NetworkProfile` by
/// [`network_profile`], which stamps in the caller-supplied `id`.
struct NetworkProfileTemplate {
    name: &'static str,
    max_number_of_devices: i64,
    ul: BitRate,
    dl: BitRate,
    qos_profiles: &'static [&'static str],
    default_qos_profile: &'static str,
}

/// A CAMARA `BitRate` (`{ value: 1..=1024, unit }`).
struct BitRate {
    value: i32,
    unit: &'static str,
}

impl BitRate {
    fn to_value(&self) -> Value {
        json!({ "value": self.value, "unit": self.unit })
    }
}

/// Render the `NetworkProfile` a `profileId` maps to: the `digits % N` template,
/// with the requested `id` echoed back (the schema requires `id` be the profile's
/// own identifier).
fn network_profile(profile_id: &str, digits: u16) -> Value {
    let t = &TEMPLATES[(digits as usize) % TEMPLATES.len()];
    json!({
        "id": profile_id,
        "name": t.name,
        "maxNumberOfDevices": t.max_number_of_devices,
        "aggregatedUlThroughput": t.ul.to_value(),
        "aggregatedDlThroughput": t.dl.to_value(),
        "qosProfiles": t.qos_profiles,
        "defaultQosProfile": t.default_qos_profile,
    })
}

/// Whether `s` is UUID-shaped: five hyphen-separated hex groups of lengths
/// `8-4-4-4-12` (the schema's `format: uuid`). Mirrors the other UUID-keyed APIs.
fn is_uuid_shaped(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
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
    // aim at a specific reserved suffix or template.
    fn uuid_ending(d: u16) -> String {
        format!("00000000-0000-4000-8000-000000000{d:03}")
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_shape_matches_the_schema() {
        assert!(is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid_shaped("3FA85F64-5717-4562-B3FC-2C963F66AFA6"));
        assert!(is_uuid_shaped(&uuid_ending(1)));
        assert!(!is_uuid_shaped("not-a-uuid"));
        assert!(!is_uuid_shaped("3fa85f64-5717-4562-b3fc-2c963f66afa")); // 11 in last group
        assert!(!is_uuid_shaped("zzzzzzzz-5717-4562-b3fc-2c963f66afa6")); // non-hex
    }

    #[test]
    fn every_template_is_a_valid_network_profile() {
        // Each rendered template carries every required field, with in-range
        // BitRate values and a `defaultQosProfile` drawn from `qosProfiles`.
        for (i, _) in TEMPLATES.iter().enumerate() {
            let id = uuid_ending(i as u16);
            let p = network_profile(&id, i as u16);
            assert_eq!(p["id"], id, "id echoes the request");
            assert!(p["name"].is_string());
            assert!(p["maxNumberOfDevices"].as_i64().unwrap() >= 1);
            for k in ["aggregatedUlThroughput", "aggregatedDlThroughput"] {
                let v = p[k]["value"].as_i64().unwrap();
                assert!((1..=1024).contains(&v), "{k} value {v} in range");
                assert!(p[k]["unit"].is_string());
            }
            let qos = p["qosProfiles"].as_array().unwrap();
            assert!(!qos.is_empty(), "at least one qosProfile");
            let default = p["defaultQosProfile"].as_str().unwrap();
            assert!(
                qos.iter().any(|q| q == default),
                "defaultQosProfile is one of qosProfiles"
            );
        }
    }

    #[test]
    fn digit_selection_covers_every_template_and_wraps() {
        // Distinct low digits pick distinct templates; the index wraps mod N.
        let n = TEMPLATES.len() as u16;
        for d in 0..n {
            let p = network_profile(&uuid_ending(d), d);
            assert_eq!(p["name"], TEMPLATES[d as usize].name);
        }
        // d == N wraps back to template 0.
        let p = network_profile(&uuid_ending(n), n);
        assert_eq!(p["name"], TEMPLATES[0].name);
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

    async fn get_profile(
        token: Option<&str>,
        profile_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!(
                "/dedicated-network-profiles/vwip/profiles/{profile_id}"
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

    async fn get_ok(profile_id: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        get_profile(Some(&token), profile_id, None).await
    }

    #[tokio::test]
    async fn happy_path_returns_the_selected_profile() {
        let id = uuid_ending(1);
        let (status, _, body) = get_ok(&id).await;
        assert_eq!(status, StatusCode::OK);
        // A single object, echoing the requested id, with every required field.
        assert!(body.is_object());
        assert_eq!(body["id"], id);
        assert_eq!(body["name"], "iot-massive");
        assert!(body["maxNumberOfDevices"].is_number());
        assert!(body["aggregatedUlThroughput"]["value"].is_number());
        assert!(body["aggregatedDlThroughput"]["unit"].is_string());
        assert!(body["qosProfiles"].as_array().unwrap().contains(&json!("standard")));
        assert_eq!(body["defaultQosProfile"], "standard");
    }

    #[tokio::test]
    async fn different_ids_select_different_profiles() {
        let (_, _, a) = get_ok(&uuid_ending(1)).await; // iot-massive
        let (_, _, b) = get_ok(&uuid_ending(2)).await; // public-safety
        assert_ne!(a["name"], b["name"]);
        assert_eq!(a["name"], "iot-massive");
        assert_eq!(b["name"], "public-safety");
    }

    #[tokio::test]
    async fn no_significant_digits_selects_the_first_template() {
        // A UUID whose only digits are 0/4/8 in fixed positions → tail 000 → t0.
        let id = "abcdefab-0000-4000-8000-abcdefabcdef";
        let (status, _, body) = get_ok(id).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], TEMPLATES[0].name);
        assert_eq!(body["id"], id);
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = get_ok(&uuid_ending(404)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = get_ok(&uuid_ending(429)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");

        let (status, _, body) = get_ok(&uuid_ending(422)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn malformed_profile_id_is_400() {
        let (status, _, body) = get_ok("not-a-uuid").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_profile(Some(&token), &uuid_ending(1), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_profile(None, &uuid_ending(1), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            get_profile(Some(&token), &uuid_ending(1), Some("corr-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        let (status, headers, _) =
            get_profile(Some(&token), &uuid_ending(404), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
