//! Blockchain Public Address **v0.3** (CAMARA Blockchain Public Address 0.3.0,
//! release r2.2).
//!
//! One endpoint so far:
//! - `POST /blockchain-public-address/v0.3/blockchain-public-addresses/retrieve-blockchains`
//!   — list the blockchain public address(es) bound to a phone number.
//!
//! ## What it does
//!
//! The caller submits a `phoneNumber` and the operator answers with the array of
//! `BlockchainPublicAddressResponse` records the subscriber has bound to that
//! line — each an on-chain `blockchainPublicAddress` on a `blockchainNetworkId`
//! (a [CAIP-2] chain identifier), an opaque record `id`, and the optional
//! `currency` list the address transacts in. A number with **no** bound address
//! answers `200 []` (a list never 404s).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `blockchain-public-address:read`
//! scope.
//!
//! ## Identifier resolution
//!
//! Faithful to the CAMARA v0.3.0 schema, `phoneNumber` is a **required** request
//! field, so it is always the identifier (no three-legged token fallback — the
//! vendored spec marks it required). A missing or malformed `phoneNumber` →
//! `400 INVALID_ARGUMENT`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer, both read off the submitted
//! `phoneNumber`:
//!
//! - **Reserved error suffix.** If the number's trailing three digits name a
//!   reserved CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`,
//!   `…429`, `…500`, `…503`), the endpoint answers that canonical CAMARA error
//!   (shared [`crate::scenarios`]). The operation's own documented error set is
//!   `400/401/403/404/429`; the remaining shared suffixes still resolve to their
//!   canonical error (the shared convention is project-wide).
//! - **Address set (trailing digits).** Otherwise the number's trailing three
//!   digits `d` decide the bound addresses:
//!   - `d == 0` (a `…000` tail, or a number with no trailing digits) → `[]`
//!     (no blockchain address is bound to this line).
//!   - `d > 0` → a deterministic list of `((d - 1) % 3) + 1` addresses (1–3), the
//!     `i`-th on network `NETWORKS[(d + i) % NETWORKS.len()]`. Each
//!     `blockchainPublicAddress` is a deterministic EVM-style `0x…` address and
//!     each `id` a deterministic UUID-shaped token, both derived (SHA-256) from
//!     the number and the network so the same input always yields the same
//!     records without any stored state.
//!
//! Example: `+123456789001` → one Ethereum-mainnet address; `+123456789002` →
//! two addresses; `+123456789000` → `[]`; `+123456789404` → `404 NOT_FOUND`.
//!
//! [CAIP-2]: https://chainagnostic.org/CAIPs/caip-2

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the retrieve endpoint requires (CAMARA Blockchain Public
/// Address 0.3.0).
const READ_SCOPE: &str = "blockchain-public-address:read";

/// The blockchain networks the simulator can report, as `(CAIP-2 id, currency)`.
/// All are EVM chains so a single `0x…` address form suffices. Indexed
/// deterministically by the identifier's trailing digits (`% len`) — a second
/// control plane, so the reported chain is reproducible from the input.
const NETWORKS: [(&str, &[&str]); 6] = [
    ("eip155:1", &["ETH"]),      // Ethereum Mainnet
    ("eip155:137", &["POL"]),    // Polygon
    ("eip155:56", &["BNB"]),     // BNB Smart Chain
    ("eip155:42161", &["ETH"]),  // Arbitrum One
    ("eip155:10", &["ETH"]),     // Optimism
    ("eip155:8453", &["ETH"]),   // Base
];

/// Routes for Blockchain Public Address v0.3, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/blockchain-public-address/v0.3/blockchain-public-addresses/retrieve-blockchains",
        post(retrieve_blockchains),
    )
}

/// `POST …/retrieve-blockchains` request body (CAMARA `PhoneNumber`): a required
/// `phoneNumber`. Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: String,
}

/// `POST /blockchain-public-address/v0.3/blockchain-public-addresses/retrieve-blockchains`.
async fn retrieve_blockchains(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: RetrieveRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body must be a JSON object with a `phoneNumber`.",
                &correlator,
            )
        }
    };

    // `phoneNumber` is required and must be E.164 (the vendored schema pattern).
    if !is_valid_e164(&req.phone_number) {
        return invalid_argument(
            "`phoneNumber` must be in E.164 format (e.g. +123456789).",
            &correlator,
        );
    }
    let identifier = req.phone_number.as_str();

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    let addresses = bound_addresses(identifier);
    with_correlator(
        (StatusCode::OK, Json(Value::Array(addresses))).into_response(),
        &correlator,
    )
}

/// The blockchain public address records bound to `identifier`, derived
/// deterministically from its trailing three digits (docs/DESIGN.md §7): `0`
/// (a `…000` tail or no digits) → no records; `d > 0` → `((d - 1) % 3) + 1`
/// records, the `i`-th on `NETWORKS[(d + i) % NETWORKS.len()]`.
fn bound_addresses(identifier: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(identifier).unwrap_or(0) as usize;
    if d == 0 {
        return Vec::new();
    }
    let count = ((d - 1) % 3) + 1; // 1..=3
    (0..count)
        .map(|i| {
            let (network_id, currency) = NETWORKS[(d + i) % NETWORKS.len()];
            json!({
                "id": record_id(identifier, network_id),
                "blockchainPublicAddress": evm_address(identifier, network_id),
                "blockchainNetworkId": network_id,
                "currency": currency,
            })
        })
        .collect()
}

/// A deterministic EVM-style public address (`0x` + 40 lowercase hex = 20 bytes)
/// for `identifier` on `network_id`, from the first 20 bytes of a domain-tagged
/// SHA-256. Lowercase (not EIP-55 checksummed) — a documented simplification.
fn evm_address(identifier: &str, network_id: &str) -> String {
    let h = Sha256::digest(format!("bpa-addr:{identifier}:{network_id}").as_bytes());
    let mut s = String::with_capacity(42);
    s.push_str("0x");
    for b in &h[..20] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// A deterministic, UUID-shaped opaque record `id` for `identifier` on
/// `network_id`, from the first 16 bytes of a domain-tagged SHA-256 (distinct
/// tag from [`evm_address`] so the two never collide). Mirrors Device
/// Identifier's PPID rendering.
fn record_id(identifier: &str, network_id: &str) -> String {
    let h = Sha256::digest(format!("bpa-id:{identifier}:{network_id}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
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
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "bpa.local:8080";
    const PATH: &str =
        "/blockchain-public-address/v0.3/blockchain-public-addresses/retrieve-blockchains";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn empty_for_zero_tail_or_no_digits() {
        assert!(bound_addresses("+123456789000").is_empty());
        assert!(bound_addresses("no-digits-here").is_empty());
        assert!(bound_addresses("+1").is_empty()); // fewer than three digits
    }

    #[test]
    fn count_is_deterministic_from_trailing_digits() {
        // count = ((d - 1) % 3) + 1  →  1,2,3 cycling.
        assert_eq!(bound_addresses("+123456789001").len(), 1); // (0 % 3)+1
        assert_eq!(bound_addresses("+123456789002").len(), 2); // (1 % 3)+1
        assert_eq!(bound_addresses("+123456789003").len(), 3); // (2 % 3)+1
        assert_eq!(bound_addresses("+123456789004").len(), 1); // (3 % 3)+1
    }

    #[test]
    fn records_are_deterministic_and_well_shaped() {
        let a = bound_addresses("+123456789007");
        let b = bound_addresses("+123456789007");
        assert_eq!(a, b, "same input → same records");
        for rec in &a {
            let addr = rec["blockchainPublicAddress"].as_str().unwrap();
            assert!(addr.starts_with("0x") && addr.len() == 42);
            assert!(addr[2..].bytes().all(|c| c.is_ascii_hexdigit()));
            let id = rec["id"].as_str().unwrap();
            assert_eq!(id.len(), 36); // UUID-shaped
            assert_eq!(id.matches('-').count(), 4);
            assert!(rec["blockchainNetworkId"].as_str().unwrap().starts_with("eip155:"));
        }
    }

    #[test]
    fn network_is_a_second_control_plane() {
        // Lead network index is d % 6; …001 → 1, …002 → 2 → different chains.
        let n1 = bound_addresses("+123456789001")[0]["blockchainNetworkId"].clone();
        let n2 = bound_addresses("+123456789002")[0]["blockchainNetworkId"].clone();
        assert_ne!(n1, n2);
        assert_eq!(n1, "eip155:137"); // NETWORKS[1]
        assert_eq!(n2, "eip155:56"); // NETWORKS[2]
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=bpa-client&scope={scope}");
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
            .uri(PATH)
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

    async fn retrieve_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn returns_the_bound_addresses_for_a_normal_number() {
        let (status, _, out) = retrieve_ok(r#"{"phoneNumber":"+123456789002"}"#).await;
        assert_eq!(status, StatusCode::OK);
        let arr = out.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        for rec in arr {
            assert!(rec["id"].is_string());
            assert!(rec["blockchainPublicAddress"].as_str().unwrap().starts_with("0x"));
            assert!(rec["blockchainNetworkId"].is_string());
            assert!(rec["currency"].is_array());
        }
    }

    #[tokio::test]
    async fn a_number_with_no_bound_address_is_an_empty_list() {
        let (status, _, out) = retrieve_ok(r#"{"phoneNumber":"+123456789000"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out, json!([]));
    }

    #[tokio::test]
    async fn address_count_tracks_the_trailing_digits() {
        for (num, want) in [("+123456789001", 1), ("+123456789002", 2), ("+123456789003", 3)] {
            let (status, _, out) = retrieve_ok(&format!(r#"{{"phoneNumber":"{num}"}}"#)).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(out.as_array().unwrap().len(), want, "for {num}");
        }
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn missing_phone_number_is_invalid_argument() {
        let (status, _, body) = retrieve_ok(r#"{}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_phone_number_is_invalid_argument() {
        let (status, _, body) = retrieve_ok(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            retrieve_ok(r#"{"phoneNumber":"+123456789002","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = retrieve_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_retrieve(Some(&token), r#"{"phoneNumber":"+123456789002"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_retrieve(None, r#"{"phoneNumber":"+123456789002"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"phoneNumber":"+123456789002"}"#,
            Some("corr-bpa"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-bpa")
        );
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"phoneNumber":"+123456789404"}"#,
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
