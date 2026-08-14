//! eSIM Remote Management **vwip** (CAMARA eSIM Remote Management, wip).
//!
//! One endpoint so far:
//! - `POST /esim-remote-management/vwip/profile/downloaded-list` — list the eSIM
//!   profiles installed on a device's eUICC (operationId `profileList`).
//!
//! ## What it does
//!
//! The caller submits a base "CMP" request envelope (`timestamp` / `sequenceNum`
//! / `clientId` / `data`) whose `data.eId` names the device eUICC. The operator
//! answers with a base response envelope (`resultCode` / `resultDesc` /
//! `sequenceNum` / `data`) whose `data` carries the device `imei`, the echoed
//! `eId`, and the list of installed `profiles` — each an `iccid` plus an
//! `enableStatus` (`0` disabled / `1` enabled).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `esim-remote-management:downloadedlist` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The device `eId` is the control plane. Its trailing three **decimal** digits
//! (hex letters skipped, mirroring Traffic Influence's hex `appId`) drive the
//! case:
//!
//! - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!   status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!   `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]). The
//!   canonical eSIM spec declares only 400/401/403/404/500/503 for this
//!   operation; CamaraSim exposes the full shared set so every reserved case is
//!   reachable (409/422/429 are CamaraSim extensions, sanctioned by the upstream
//!   spec's own "error list is not exhaustive" note).
//! - **Profile inventory** — otherwise the trailing three digits `d` set the
//!   number of installed profiles (`d % 4`: `…000` / no digits → an empty eUICC,
//!   `…001` → 1, `…002` → 2, `…003` → 3). At most one profile is enabled at a
//!   time (the eUICC rule): the first is enabled, the rest disabled. Each
//!   profile's `iccid` and the device `imei` are derived deterministically from
//!   the `eId` (a stable FNV hash + a Luhn check digit), so the same `eId`
//!   always returns the same inventory.
//!
//! A `data.eId` that is absent or not 32 hex characters → `400 INVALID_ARGUMENT`
//! (CamaraSim requires the `eId` for the query — a documented tightening of the
//! upstream optional field). A malformed `sequenceNum` is likewise
//! `400 INVALID_ARGUMENT`.

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

/// The OAuth2 scope the `profileList` endpoint requires (CAMARA eSIM Remote
/// Management).
const LIST_SCOPE: &str = "esim-remote-management:downloadedlist";

/// The success `resultCode` of the base CMP response envelope (`B100000`).
const RESULT_OK: &str = "B100000";

/// Routes for eSIM Remote Management vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/esim-remote-management/vwip/profile/downloaded-list",
        post(profile_list),
    )
}

/// `POST /profile/downloaded-list` request body — the base CMP envelope
/// (CAMARA `BaseCmpReqEidProfileListReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileListRequest {
    #[allow(dead_code)]
    timestamp: Option<String>,
    #[serde(rename = "sequenceNum")]
    sequence_num: Option<String>,
    #[serde(rename = "clientId")]
    #[allow(dead_code)]
    client_id: Option<String>,
    data: Option<ProfileListData>,
}

/// The `data` payload (CAMARA `EidProfileListReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileListData {
    #[serde(rename = "eId")]
    e_id: Option<String>,
}

/// `POST /esim-remote-management/vwip/profile/downloaded-list`.
async fn profile_list(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(LIST_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required and must parse.
    let req: ProfileListRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid BaseCmpReqEidProfileListReq.",
                &correlator,
            )
        }
    };

    // Syntactic validation (400) before the identifier-driven scenario plane.
    if let Some(seq) = &req.sequence_num {
        if !is_valid_sequence_num(seq) {
            return invalid_argument(
                "`sequenceNum` must match ^[a-zA-Z0-9_-]{1,64}$.",
                &correlator,
            );
        }
    }

    // CamaraSim requires `data.eId` for the query (a documented tightening of the
    // upstream optional field) and validates its 32-hex EID shape.
    let e_id = match req.data.and_then(|d| d.e_id) {
        Some(id) if is_hex32(&id) => id,
        _ => {
            return invalid_argument(
                "`data.eId` is required and must be 32 hexadecimal characters.",
                &correlator,
            )
        }
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&e_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: the eId's trailing three digits set the number of installed
    // profiles; at most one is enabled (the eUICC single-active-profile rule).
    let count = scenarios::trailing_three_digits(&e_id).unwrap_or(0) as usize % 4;
    let profiles: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "iccid": iccid(&e_id, i),
                "enableStatus": if i == 0 { 1 } else { 0 },
            })
        })
        .collect();

    let mut out = json!({
        "resultCode": RESULT_OK,
        "resultDesc": "Success",
        "data": {
            "imei": imei(&e_id),
            "profiles": profiles,
            "eId": e_id,
        },
    });
    // Echo the request's `sequenceNum` when one was supplied.
    if let Some(seq) = &req.sequence_num {
        out["sequenceNum"] = json!(seq);
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// Whether `s` is exactly 32 hexadecimal characters (the CAMARA `eId` pattern
/// `^[A-Fa-f0-9]{32}$`).
fn is_hex32(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether `s` matches the CAMARA `sequenceNum` pattern `^[a-zA-Z0-9_-]{1,64}$`.
fn is_valid_sequence_num(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The device IMEI for `eid` — 14 digits derived from a stable hash plus a Luhn
/// check digit (15 digits total; matches `^[0-9]{15}$`). Deterministic, so the
/// same eId always reports the same IMEI. No date/crypto dependency.
fn imei(eid: &str) -> String {
    let h = fnv1a_64(&format!("imei:{eid}"));
    let base = format!("{:014}", h % 100_000_000_000_000); // 14 digits
    let check = luhn_check_digit(&base);
    format!("{base}{check}")
}

/// The ICCID of the `index`-th profile on `eid` — the telecom prefix `89` plus
/// 17 digits derived from a stable hash plus a Luhn check digit (20 digits total;
/// matches `^[0-9]{19,20}$`). Deterministic per (eId, index).
fn iccid(eid: &str, index: usize) -> String {
    let h = fnv1a_64(&format!("iccid:{eid}:{index}"));
    let body = format!("89{:017}", h % 100_000_000_000_000_000); // "89" + 17 = 19 digits
    let check = luhn_check_digit(&body);
    format!("{body}{check}")
}

/// The Luhn check digit that makes `payload` (followed by the digit) pass the
/// Luhn checksum. `payload` must be ASCII digits.
fn luhn_check_digit(payload: &str) -> u8 {
    let mut sum = 0u32;
    // The rightmost payload digit sits at position 2 from the right of the full
    // number (the check digit is position 1), so it is doubled.
    for (i, b) in payload.bytes().rev().enumerate() {
        let mut d = u32::from(b - b'0');
        if i % 2 == 0 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    ((10 - (sum % 10)) % 10) as u8
}

/// FNV-1a 64-bit hash of `s`. Self-contained (no dependency), used only to derive
/// stable, deterministic synthetic identifiers — never for security.
fn fnv1a_64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
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

    const HOST: &str = "esim.local:8080";
    // A 32-hex EID whose trailing decimal digits are `001` → one enabled profile.
    const EID_ONE: &str = "A1B2C3D4E5F600000000000000000001";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn hex32_validation_is_exact() {
        assert!(is_hex32(EID_ONE));
        assert!(is_hex32("abcdef0123456789ABCDEF0123456789"));
        assert!(!is_hex32("A1B2C3")); // too short
        assert!(!is_hex32("A1B2C3D4E5F60000000000000000000")); // 31 chars
        assert!(!is_hex32("A1B2C3D4E5F6000000000000000000012")); // 33 chars
        assert!(!is_hex32("A1B2C3D4E5F600000000000000000G01")); // non-hex `G`
    }

    #[test]
    fn sequence_num_validation_follows_the_pattern() {
        assert!(is_valid_sequence_num("seq-0001"));
        assert!(is_valid_sequence_num("A_b-9"));
        assert!(!is_valid_sequence_num("")); // empty
        assert!(!is_valid_sequence_num("has space"));
        assert!(!is_valid_sequence_num(&"x".repeat(65))); // too long
    }

    #[test]
    fn imei_is_15_digits_and_luhn_valid() {
        let m = imei(EID_ONE);
        assert_eq!(m.len(), 15);
        assert!(m.bytes().all(|b| b.is_ascii_digit()));
        assert!(luhn_valid(&m), "IMEI {m} should pass Luhn");
        // Deterministic.
        assert_eq!(imei(EID_ONE), imei(EID_ONE));
    }

    #[test]
    fn iccid_is_19_or_20_digits_luhn_valid_and_prefixed() {
        let c = iccid(EID_ONE, 0);
        assert_eq!(c.len(), 20);
        assert!(c.starts_with("89"));
        assert!(c.bytes().all(|b| b.is_ascii_digit()));
        assert!(luhn_valid(&c), "ICCID {c} should pass Luhn");
        // Different indices give different ICCIDs.
        assert_ne!(iccid(EID_ONE, 0), iccid(EID_ONE, 1));
    }

    /// Whether a full digit string (payload + check digit) passes Luhn.
    fn luhn_valid(full: &str) -> bool {
        let mut sum = 0u32;
        for (i, b) in full.bytes().rev().enumerate() {
            let mut d = u32::from(b - b'0');
            if i % 2 == 1 {
                d *= 2;
                if d > 9 {
                    d -= 9;
                }
            }
            sum += d;
        }
        sum % 10 == 0
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=esim-client&scope={scope}");
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

    /// POST to the endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_list(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/esim-remote-management/vwip/profile/downloaded-list")
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

    /// Mint the scoped token and call the endpoint.
    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(LIST_SCOPE).await;
        post_list(Some(&token), body, None).await
    }

    fn body_for(eid: &str) -> String {
        format!(r#"{{"data":{{"eId":"{eid}"}}}}"#)
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn returns_one_enabled_profile_for_a_001_tail() {
        let (status, _, body) = call_ok(&body_for(EID_ONE)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resultCode"], "B100000");
        assert_eq!(body["data"]["eId"], EID_ONE);
        let profiles = body["data"]["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["enableStatus"], 1);
        // IMEI/ICCID shapes.
        assert_eq!(body["data"]["imei"].as_str().unwrap().len(), 15);
        assert_eq!(profiles[0]["iccid"].as_str().unwrap().len(), 20);
    }

    #[tokio::test]
    async fn empty_euicc_for_a_000_tail() {
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000000")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["data"]["profiles"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn three_profiles_with_only_the_first_enabled_for_a_003_tail() {
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000003")).await;
        assert_eq!(status, StatusCode::OK);
        let profiles = body["data"]["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0]["enableStatus"], 1);
        assert_eq!(profiles[1]["enableStatus"], 0);
        assert_eq!(profiles[2]["enableStatus"], 0);
    }

    #[tokio::test]
    async fn sequence_num_is_echoed_when_supplied() {
        let token = mint_token(LIST_SCOPE).await;
        let body = format!(r#"{{"sequenceNum":"seq-42","data":{{"eId":"{EID_ONE}"}}}}"#);
        let (status, _, out) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["sequenceNum"], "seq-42");
    }

    // --- Reserved-error control plane -------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        // …404 → 404 NOT_FOUND.
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …429 → 429 (a CamaraSim extension beyond the canonical eSIM error set).
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000429")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
        // …503 → 503.
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000503")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn missing_eid_is_rejected() {
        let (status, _, body) = call_ok(r#"{"data":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Also a wholly absent `data`.
        let (status, _, _) = call_ok(r#"{"sequenceNum":"s"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn non_hex_eid_is_rejected() {
        let (status, _, body) = call_ok(&body_for("not-a-valid-eid")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_sequence_num_is_rejected() {
        let token = mint_token(LIST_SCOPE).await;
        let body = format!(r#"{{"sequenceNum":"has space","data":{{"eId":"{EID_ONE}"}}}}"#);
        let (status, _, out) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(r#"{"data":{"eId":"A1B2C3D4E5F600000000000000000001"},"x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
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
        let (status, _, body) = post_list(Some(&token), &body_for(EID_ONE), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_list(None, &body_for(EID_ONE), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- Correlator --------------------------------------------------------

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(LIST_SCOPE).await;
        let (status, headers, _) =
            post_list(Some(&token), &body_for(EID_ONE), Some("corr-esim")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-esim")
        );
        // Business error.
        let (status, headers, _) = post_list(
            Some(&token),
            &body_for("A1B2C3D4E5F600000000000000000404"),
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
