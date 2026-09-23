//! In-memory authorization-code store shared by the authorize and token endpoints.
//!
//! The `authorization_code` grant is a two-request flow: `GET /oauth2/authorize`
//! (see [`super::authorize`]) mints a short-lived code and hands it to the client
//! via redirect; `POST /oauth2/token` (see [`super::token`]) then redeems it for an
//! access token. This module is the state that bridges the two requests.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for a `HashMap` insert/remove —
//!   never across an `.await` — so it does not block the async runtime.
//! - **Single use**: [`redeem`] *removes* the entry, so a code cannot be replayed
//!   (OAuth2 RFC 6749 §4.1.2). A second redemption of the same code simply finds
//!   nothing and fails as `invalid_grant`.
//! - **Short-lived**: a code expires [`CODE_TTL`] seconds after issue; the token
//!   endpoint rejects an expired (but not-yet-evicted) code.
//!
//! ## PKCE (RFC 7636, mandatory in the CAMARA profile)
//!
//! The authorize request carries a `code_challenge` (the S256 transform of a
//! client-held `code_verifier`); it is stored here and checked at redemption by
//! [`pkce_s256_matches`], binding the code to the client that started the flow.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

/// Lifetime of an issued authorization code, in seconds. Codes are meant to be
/// redeemed immediately; a short window is enough and limits replay (RFC 6749
/// §4.1.2 recommends a maximum of ~10 minutes).
pub const CODE_TTL: u64 = 600;

/// An issued authorization code's bound context, captured at `GET /oauth2/authorize`
/// and validated when the code is redeemed at the token endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCode {
    /// The client the code was issued to; the redemption's `client_id` must match.
    pub client_id: String,
    /// The redirect URI the code was issued for; the redemption's `redirect_uri`
    /// must match exactly (RFC 6749 §4.1.3).
    pub redirect_uri: String,
    /// The scope granted at authorization, copied into the issued token verbatim.
    pub scope: String,
    /// The PKCE `code_challenge` (S256), checked against the redemption's
    /// `code_verifier` (RFC 7636 §4.6).
    pub code_challenge: String,
    /// The resource-server identifier stamped as the token `aud` — the issuer base
    /// URL resolved at authorization time, so the token targets this same server.
    pub audience: String,
    /// Unix expiry; a code redeemed at or after this instant is rejected.
    pub expires_at: u64,
}

/// The process-global code store. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, AuthCode>> {
    static STORE: OnceLock<Mutex<HashMap<String, AuthCode>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Mint an opaque authorization code for `entry`, store it, and return it.
///
/// The value is the base64url of `SHA-256(counter ‖ now)`: opaque and unique (the
/// monotonic counter alone guarantees uniqueness; the hash makes it unguessable
/// enough for a test double, with no `uuid`/`rand` dependency).
pub fn issue(entry: AuthCode) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(unix_now().to_be_bytes());
    let code = URL_SAFE_NO_PAD.encode(hasher.finalize());
    store().lock().expect("code store not poisoned").insert(code.clone(), entry);
    code
}

/// Redeem `code`, removing it from the store (single use) and returning its bound
/// context if present. Expiry is left to the caller so it can distinguish an
/// expired code from an unknown one for diagnostics; either way the entry is
/// consumed.
pub fn redeem(code: &str) -> Option<AuthCode> {
    store().lock().expect("code store not poisoned").remove(code)
}

/// Whether `verifier` satisfies the stored S256 `challenge`:
/// `challenge == base64url(SHA-256(verifier))` (RFC 7636 §4.6). The comparison is
/// length-then-byte and does not early-return per differing byte.
pub fn pkce_s256_matches(verifier: &str, challenge: &str) -> bool {
    let computed = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    constant_time_eq(computed.as_bytes(), challenge.as_bytes())
}

/// Constant-time-ish equality over two byte slices (no early return on mismatch).
/// PKCE values are not long-lived secrets, but avoiding a data-dependent early
/// exit costs nothing and keeps the comparison honest.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Current Unix time in seconds (server runtime clock; not on any hot loop).
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(client: &str) -> AuthCode {
        AuthCode {
            client_id: client.to_string(),
            redirect_uri: "https://app.example/cb".to_string(),
            scope: "openid".to_string(),
            code_challenge: "chal".to_string(),
            audience: "http://sim.local:8080".to_string(),
            expires_at: unix_now() + CODE_TTL,
        }
    }

    #[test]
    fn issued_codes_are_unique_and_opaque() {
        let a = issue(sample("c1"));
        let b = issue(sample("c1"));
        assert_ne!(a, b, "each issue must be unique");
        // base64url(SHA-256) is 43 chars, no padding, url-safe alphabet.
        assert_eq!(a.len(), 43);
        assert!(a
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    }

    #[test]
    fn redeem_returns_the_entry_once_then_nothing() {
        let entry = sample("c2");
        let code = issue(entry.clone());
        assert_eq!(redeem(&code), Some(entry), "first redemption returns the bound context");
        assert_eq!(redeem(&code), None, "single use: a second redemption finds nothing");
    }

    #[test]
    fn redeem_unknown_code_is_none() {
        assert_eq!(redeem("no-such-code"), None);
    }

    #[test]
    fn pkce_s256_matches_the_rfc_7636_test_vector() {
        // RFC 7636 Appendix B: verifier -> challenge.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(pkce_s256_matches(verifier, challenge));
        assert!(!pkce_s256_matches("wrong-verifier", challenge));
        assert!(!pkce_s256_matches(verifier, "wrong-challenge"));
    }
}
