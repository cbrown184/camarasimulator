//! Blockchain Public Address **v0.3** (CAMARA Blockchain Public Address 0.3.0,
//! release r2.2).
//!
//! Endpoints:
//! - `POST …/blockchain-public-addresses/retrieve-blockchains`
//!   (`retrieveBlockchainPublicAddress`) — list the blockchain public address(es)
//!   bound to a phone number (stateless, deterministic-synthetic).
//! - `POST …/blockchain-public-addresses` (`bindBlockchainPublicAddress`) — bind
//!   an on-chain address to a phone number, persisting it in [`super::store`] and
//!   returning a `201` with the minted binding `id`.
//!
//! (`DELETE …/blockchain-public-addresses/{id}` is a later slice.)
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

/// The OAuth2 scope the bind endpoint requires (CAMARA Blockchain Public
/// Address 0.3.0).
const CREATE_SCOPE: &str = "blockchain-public-address:create";

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
    Router::new()
        .route(
            "/blockchain-public-address/v0.3/blockchain-public-addresses/retrieve-blockchains",
            post(retrieve_blockchains),
        )
        .route(
            "/blockchain-public-address/v0.3/blockchain-public-addresses",
            post(bind_blockchain_public_address),
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

/// `POST …/blockchain-public-addresses` request body
/// (`BindBlockchainPublicAddressRequest`): the required `phoneNumber`,
/// `blockchainPublicAddress` and `blockchainNetworkId`, an optional `currency`
/// list, and the optional enhanced-ownership-validation pair `nonce`/`signature`.
/// Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: String,
    #[serde(rename = "blockchainPublicAddress")]
    blockchain_public_address: String,
    #[serde(rename = "blockchainNetworkId")]
    blockchain_network_id: String,
    #[serde(default)]
    currency: Option<Vec<String>>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    signature: Option<String>,
}

/// `POST /blockchain-public-address/v0.3/blockchain-public-addresses`
/// (`bindBlockchainPublicAddress`).
///
/// Binds an on-chain `blockchainPublicAddress` (on `blockchainNetworkId`) to the
/// subscriber's `phoneNumber` and persists the relationship in the in-memory
/// [`super::store`], minting an opaque binding `id`. Happy path → `201` with
/// `{ "id": … }` (`BindBlockchainPublicAddressResponse`).
///
/// Control planes (docs/DESIGN.md §7):
/// - **`phoneNumber` reserved error suffix** → canonical CAMARA error (shared
///   [`crate::scenarios`]); a malformed number → `400 INVALID_ARGUMENT`.
/// - **Request validation** (second plane): a `blockchainNetworkId` that is not
///   `<ecosystem>:<sub_id>` → `400
///   BLOCKCHAIN_PUBLIC_ADDRESS.INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER`; a
///   `blockchainPublicAddress` that is not an EVM `0x…` (40-hex) address → `400
///   INVALID_ARGUMENT`; supplying exactly one of `nonce`/`signature` → `400
///   BLOCKCHAIN_PUBLIC_ADDRESS.BOTH_NONCE_SIGNATURE_REQUIRED`; supplying **both**
///   requests enhanced on-chain ownership validation, which the simulator has no
///   chain to perform → `422
///   BLOCKCHAIN_PUBLIC_ADDRESS.UNSUPPORTED_ENHANCED_VALIDATION` (a documented
///   behaviour).
/// - **Store state** (third plane): re-binding the *same*
///   `(phoneNumber, blockchainNetworkId, blockchainPublicAddress)` triple → `409
///   ALREADY_EXISTS` (the id is derived from the triple, so a duplicate collides).
async fn bind_blockchain_public_address(
    claims: Claims,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: BindRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body must be a JSON object with `phoneNumber`, \
                 `blockchainPublicAddress` and `blockchainNetworkId`.",
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

    // First control plane: the identifier's reserved error suffix (docs/DESIGN §7).
    if let Some(err) = scenarios::reserved_error(&req.phone_number) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Second control plane: request validation.
    if !is_valid_network_id(&req.blockchain_network_id) {
        return with_correlator(
            invalid_blockchain_network_identifier().into_response(),
            &correlator,
        );
    }
    if !is_valid_evm_address(&req.blockchain_public_address) {
        return invalid_argument(
            "`blockchainPublicAddress` must be an EVM address (`0x` + 40 hex digits).",
            &correlator,
        );
    }
    match (req.nonce.is_some(), req.signature.is_some()) {
        // Exactly one supplied → the pair is required together.
        (true, false) | (false, true) => {
            return with_correlator(
                both_nonce_signature_required().into_response(),
                &correlator,
            )
        }
        // Both supplied → enhanced ownership validation, which the simulator
        // (no chain to verify against) cannot perform.
        (true, true) => {
            return with_correlator(
                unsupported_enhanced_validation().into_response(),
                &correlator,
            )
        }
        // Neither → a basic binding.
        (false, false) => {}
    }

    // Third control plane: the store. A duplicate triple → 409 ALREADY_EXISTS.
    let id = binding_id(
        &req.phone_number,
        &req.blockchain_network_id,
        &req.blockchain_public_address,
    );
    let mut record = json!({
        "id": id,
        "blockchainPublicAddress": req.blockchain_public_address,
        "blockchainNetworkId": req.blockchain_network_id,
    });
    if let Some(currency) = &req.currency {
        record["currency"] = json!(currency);
    }
    if !super::store::insert(id.clone(), record) {
        return with_correlator(already_exists().into_response(), &correlator);
    }

    with_correlator(
        (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        &correlator,
    )
}

/// A deterministic, UUID-shaped binding `id` for the
/// `(phone_number, network_id, address)` triple, from the first 16 bytes of a
/// domain-tagged SHA-256. Because it is a pure function of the triple, re-binding
/// the same address to the same line yields the same id — which
/// [`super::store::insert`] uses to detect a `409 ALREADY_EXISTS` duplicate.
pub fn binding_id(phone_number: &str, network_id: &str, address: &str) -> String {
    let h = Sha256::digest(format!("bpa-binding:{phone_number}:{network_id}:{address}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// Whether `s` is a well-formed `blockchainNetworkId` — `<ecosystem>:<sub_id>`
/// (CAMARA e.g. `evm:1`): exactly one `:`, both sides non-empty and made of
/// `[A-Za-z0-9_-]`.
fn is_valid_network_id(s: &str) -> bool {
    let mut parts = s.split(':');
    let (Some(eco), Some(sub), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |p: &str| {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    };
    ok(eco) && ok(sub)
}

/// Whether `s` is an EVM public address: `0x` followed by exactly 40 hex digits.
fn is_valid_evm_address(s: &str) -> bool {
    let Some(hex) = s.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `400 BLOCKCHAIN_PUBLIC_ADDRESS.INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER`.
fn invalid_blockchain_network_identifier() -> CamaraError {
    CamaraError::new(
        StatusCode::BAD_REQUEST,
        "BLOCKCHAIN_PUBLIC_ADDRESS.INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER",
        "`blockchainNetworkId` must be `<ecosystem>:<sub_id>` (e.g. `evm:1`).",
    )
}

/// `400 BLOCKCHAIN_PUBLIC_ADDRESS.BOTH_NONCE_SIGNATURE_REQUIRED`.
fn both_nonce_signature_required() -> CamaraError {
    CamaraError::new(
        StatusCode::BAD_REQUEST,
        "BLOCKCHAIN_PUBLIC_ADDRESS.BOTH_NONCE_SIGNATURE_REQUIRED",
        "`nonce` and `signature` must be supplied together.",
    )
}

/// `422 BLOCKCHAIN_PUBLIC_ADDRESS.UNSUPPORTED_ENHANCED_VALIDATION` — the
/// simulator has no chain to run enhanced on-chain ownership validation against.
fn unsupported_enhanced_validation() -> CamaraError {
    CamaraError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "BLOCKCHAIN_PUBLIC_ADDRESS.UNSUPPORTED_ENHANCED_VALIDATION",
        "Enhanced ownership validation (`nonce`/`signature`) is not supported.",
    )
}

/// `409 ALREADY_EXISTS` — this address is already bound to this line.
fn already_exists() -> CamaraError {
    CamaraError::new(
        StatusCode::CONFLICT,
        "ALREADY_EXISTS",
        "This blockchain public address is already bound to this phone number.",
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

    #[test]
    fn network_id_validation() {
        assert!(is_valid_network_id("evm:1"));
        assert!(is_valid_network_id("eip155:42161"));
        assert!(is_valid_network_id("cosmos:cosmoshub-4"));
        assert!(!is_valid_network_id("evm")); // no sub_id
        assert!(!is_valid_network_id(":1")); // empty ecosystem
        assert!(!is_valid_network_id("evm:")); // empty sub_id
        assert!(!is_valid_network_id("evm:1:2")); // too many parts
        assert!(!is_valid_network_id("evm:1 2")); // space
    }

    #[test]
    fn evm_address_validation() {
        assert!(is_valid_evm_address(
            "0x0000000000000000000000000000000000000000"
        ));
        assert!(is_valid_evm_address(
            "0xabcdefABCDEF0123456789abcdefABCDEF012345"
        ));
        assert!(!is_valid_evm_address("0x1234")); // too short
        assert!(!is_valid_evm_address(
            "0x000000000000000000000000000000000000000g" // non-hex
        ));
        assert!(!is_valid_evm_address(
            "0000000000000000000000000000000000000000" // no 0x
        ));
    }

    #[test]
    fn binding_id_is_deterministic_and_uuid_shaped() {
        let a = binding_id("+123456789111", "evm:1", "0xabc");
        let b = binding_id("+123456789111", "evm:1", "0xabc");
        assert_eq!(a, b, "same triple → same id");
        assert_eq!(a.len(), 36);
        assert_eq!(a.matches('-').count(), 4);
        // A different triple → a different id.
        assert_ne!(a, binding_id("+123456789111", "evm:137", "0xabc"));
        assert_ne!(a, binding_id("+123456789222", "evm:1", "0xabc"));
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

    // --- bindBlockchainPublicAddress (POST /blockchain-public-addresses) ----

    const BIND_PATH: &str = "/blockchain-public-address/v0.3/blockchain-public-addresses";

    /// A valid EVM address literal for tests (40 hex digits).
    const ADDR: &str = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd";

    async fn post_bind(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(BIND_PATH)
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

    async fn bind_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_bind(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn bind_persists_and_returns_a_minted_id() {
        // A unique triple so the process-global store isn't touched by others.
        let body = format!(
            r#"{{"phoneNumber":"+123456780001","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1","currency":["ETH"]}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = out["id"].as_str().expect("id in response");
        assert_eq!(id.len(), 36);
        // Only `id` is returned (BindBlockchainPublicAddressResponse).
        assert_eq!(out.as_object().unwrap().len(), 1);
        // The full record is persisted in the store under that id.
        let stored = super::super::store::get(id).expect("binding persisted");
        assert_eq!(stored["blockchainPublicAddress"], ADDR);
        assert_eq!(stored["blockchainNetworkId"], "evm:1");
        assert_eq!(stored["currency"], json!(["ETH"]));
    }

    #[tokio::test]
    async fn re_binding_the_same_triple_is_already_exists() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780010","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let (status, _, _) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(out["code"], "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn binding_a_different_address_to_the_same_line_is_allowed() {
        let phone = "+123456780020";
        let a = format!(
            r#"{{"phoneNumber":"{phone}","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let other = "0x1111111111111111111111111111111111111111";
        let b = format!(
            r#"{{"phoneNumber":"{phone}","blockchainPublicAddress":"{other}","blockchainNetworkId":"evm:1"}}"#
        );
        assert_eq!(bind_ok(&a).await.0, StatusCode::CREATED);
        assert_eq!(bind_ok(&b).await.0, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn bind_reserved_suffix_selects_a_canonical_camara_error() {
        let body = format!(
            r#"{{"phoneNumber":"+123456789404","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(out["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn bind_malformed_phone_number_is_invalid_argument() {
        let body = format!(
            r#"{{"phoneNumber":"0123","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bind_bad_network_id_is_a_specific_error() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780030","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"not-a-caip"}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            out["code"],
            "BLOCKCHAIN_PUBLIC_ADDRESS.INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER"
        );
    }

    #[tokio::test]
    async fn bind_bad_public_address_is_invalid_argument() {
        let body = r#"{"phoneNumber":"+123456780031","blockchainPublicAddress":"0xnothex","blockchainNetworkId":"evm:1"}"#;
        let (status, _, out) = bind_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bind_lone_nonce_or_signature_is_both_required() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780040","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1","nonce":"abc"}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            out["code"],
            "BLOCKCHAIN_PUBLIC_ADDRESS.BOTH_NONCE_SIGNATURE_REQUIRED"
        );
    }

    #[tokio::test]
    async fn bind_both_nonce_and_signature_is_unsupported_enhanced_validation() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780041","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1","nonce":"abc","signature":"0xsig"}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            out["code"],
            "BLOCKCHAIN_PUBLIC_ADDRESS.UNSUPPORTED_ENHANCED_VALIDATION"
        );
    }

    #[tokio::test]
    async fn bind_unknown_field_is_rejected() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780050","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1","x":1}}"#
        );
        let (status, _, out) = bind_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bind_without_the_create_scope_is_forbidden() {
        // A read-scoped token cannot bind.
        let token = mint_token(READ_SCOPE).await;
        let body = format!(
            r#"{{"phoneNumber":"+123456780060","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let (status, _, out) = post_bind(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(out["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn bind_missing_token_is_unauthenticated() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780061","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let (status, _, out) = post_bind(None, &body, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(out["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn bind_echoes_x_correlator() {
        let body = format!(
            r#"{{"phoneNumber":"+123456780070","blockchainPublicAddress":"{ADDR}","blockchainNetworkId":"evm:1"}}"#
        );
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, _) = post_bind(Some(&token), &body, Some("corr-bind")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-bind")
        );
    }
}
