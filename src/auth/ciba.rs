//! CIBA — Client-Initiated Backchannel Authentication.
//!
//! The third CAMARA grant (docs/DESIGN.md §6). Unlike `authorization_code`, the
//! client never redirects the user agent: it starts the flow **out of band** by
//! POSTing to `/bc-authorize` with a `login_hint` identifying the end user, gets
//! back an `auth_req_id`, and then **polls** `POST /oauth2/token`
//! (`grant_type=urn:openid:params:grant-type:ciba`) until the user authorizes.
//! (OpenID Connect CIBA Core 1.0; the CAMARA profile uses `poll` delivery.)
//!
//! This module is the two halves of the backchannel that live outside the token
//! endpoint: the `/bc-authorize` handler ([`handler`]) and the in-memory request
//! store it shares with the token endpoint's CIBA grant ([`poll`]).
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`, held only for a map operation — never across an
//!   `.await` — so it does not block the async runtime. Mirrors [`super::codes`].
//! - **Single use on success**: an approved request is *removed* when the token is
//!   issued, so the same `auth_req_id` cannot mint two tokens.
//! - **Short-lived**: a request expires [`EXPIRES_IN`] seconds after `/bc-authorize`;
//!   polling an expired (but not-yet-evicted) request yields `expired_token`.
//!
//! ## Simulator functional cases (docs/DESIGN.md §7)
//!
//! CamaraSim is headless — there is no device to prompt — so the **`login_hint`
//! is the control plane** and selects the authorization outcome deterministically:
//!
//! - default → **approved** immediately (the first poll returns a token);
//! - a `login_hint` containing `pending` → stays **`authorization_pending`**
//!   (models an end user who has not yet acted — every poll returns pending);
//! - a `login_hint` containing `denied` → **`access_denied`** (the end user
//!   rejected the request).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{base_url, codes, token};

/// Lifetime of a backchannel authentication request, in seconds. Returned as
/// `expires_in`; a poll after this window yields `expired_token`.
pub const EXPIRES_IN: u64 = 120;

/// Minimum seconds a client must wait between polls (`interval`, advisory).
pub const INTERVAL: u64 = 5;

/// The authorization outcome bound to an `auth_req_id`, selected from the
/// `login_hint` at `/bc-authorize` (see the module-level functional cases).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The end user has authorized: the next poll issues a token.
    Approved,
    /// The end user has not acted yet: polls return `authorization_pending`.
    Pending,
    /// The end user rejected the request: polls return `access_denied`.
    Denied,
}

/// A backchannel authentication request, created at `/bc-authorize` and redeemed
/// by the token endpoint's CIBA grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CibaRequest {
    /// The client the request was issued to; the poll's `client_id` must match.
    pub client_id: String,
    /// The requested scope, copied into the issued token verbatim.
    pub scope: String,
    /// The resource-server identifier stamped as the token `aud` (issuer base URL).
    pub audience: String,
    /// The authorization outcome the poll resolves to.
    pub state: State,
    /// Unix expiry; polling at or after this instant yields `expired_token`.
    pub expires_at: u64,
}

/// The result of polling the token endpoint for an `auth_req_id`. The token
/// endpoint ([`super::token`]) maps each variant to the CIBA token response or
/// error code (OpenID CIBA Core §10.2 / §11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    /// Authorized: issue a token with this scope and audience (request consumed).
    Approved { scope: String, audience: String },
    /// `authorization_pending` — the end user has not authorized yet.
    Pending,
    /// `access_denied` — the end user rejected the request.
    Denied,
    /// `expired_token` — the `auth_req_id` has expired (request evicted).
    Expired,
    /// `invalid_grant` — no such `auth_req_id`.
    Unknown,
    /// `invalid_grant` — the `auth_req_id` was issued to a different client.
    WrongClient,
}

/// The process-global backchannel-request store. In-memory only (DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, CibaRequest>> {
    static STORE: OnceLock<Mutex<HashMap<String, CibaRequest>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Select the authorization outcome from the `login_hint` (functional cases §7).
fn state_for_login_hint(login_hint: &str) -> State {
    let hint = login_hint.to_ascii_lowercase();
    if hint.contains("denied") {
        State::Denied
    } else if hint.contains("pending") {
        State::Pending
    } else {
        State::Approved
    }
}

/// Mint an opaque `auth_req_id` for `req`, store it, and return it. The value is
/// `base64url(SHA-256(counter ‖ now))` — opaque and unique, no `uuid`/`rand` dep
/// (mirrors [`super::codes::issue`]).
pub fn issue(req: CibaRequest) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(codes::unix_now().to_be_bytes());
    let id = URL_SAFE_NO_PAD.encode(hasher.finalize());
    store()
        .lock()
        .expect("ciba store not poisoned")
        .insert(id.clone(), req);
    id
}

/// Poll the store for `auth_req_id` on behalf of `client_id`, returning the
/// outcome. An [`Poll::Approved`] request is **removed** (single use); an expired
/// request is evicted. The lock is never held across an `.await`.
pub fn poll(auth_req_id: &str, client_id: &str) -> Poll {
    let mut guard = store().lock().expect("ciba store not poisoned");
    let (client_matches, expired, state) = match guard.get(auth_req_id) {
        None => return Poll::Unknown,
        Some(req) => (
            req.client_id == client_id,
            codes::unix_now() >= req.expires_at,
            req.state,
        ),
    };
    if !client_matches {
        return Poll::WrongClient;
    }
    if expired {
        guard.remove(auth_req_id);
        return Poll::Expired;
    }
    match state {
        State::Pending => Poll::Pending,
        State::Denied => Poll::Denied,
        State::Approved => {
            let req = guard.remove(auth_req_id).expect("entry present under lock");
            Poll::Approved {
                scope: req.scope,
                audience: req.audience,
            }
        }
    }
}

/// Parsed `application/x-www-form-urlencoded` `/bc-authorize` request. All fields
/// optional so validation (and the resulting OAuth2 error) is explicit.
#[derive(Debug, Default, Deserialize)]
struct BcForm {
    scope: Option<String>,
    client_id: Option<String>,
    /// The end-user identifier hint (CAMARA uses `login_hint`, e.g. `tel:+34...`).
    /// Also selects the simulator's functional case (see the module docs).
    login_hint: Option<String>,
}

/// `POST /bc-authorize` — CIBA backchannel authentication endpoint.
///
/// Authenticates the client (as the token endpoint does), requires a `login_hint`
/// and `scope`, mints an `auth_req_id`, and returns `{ auth_req_id, expires_in,
/// interval }`. The `login_hint` also picks the authorization outcome the later
/// poll resolves to (functional cases, module docs).
pub async fn handler(headers: HeaderMap, body: String) -> Response {
    let form: BcForm = match serde_urlencoded::from_str(&body) {
        Ok(form) => form,
        Err(_) => {
            return token::oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request body must be application/x-www-form-urlencoded",
            )
        }
    };

    // Client authentication (client_secret_basic or client_secret_post).
    let client_id = match token::client_id_from_basic(&headers).or(form.client_id) {
        Some(id) => id,
        None => {
            return token::oauth_error(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "client authentication required (client_secret_basic or client_secret_post)",
            )
        }
    };
    // A user-identifier hint is mandatory in CIBA (OpenID CIBA Core §7.1).
    let login_hint = match form.login_hint.filter(|h| !h.is_empty()) {
        Some(h) => h,
        None => {
            return token::oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the 'login_hint' parameter is required",
            )
        }
    };
    let scope = match form.scope.filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            return token::oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the 'scope' parameter is required",
            )
        }
    };

    let auth_req_id = issue(CibaRequest {
        client_id,
        scope,
        audience: base_url(&headers),
        state: state_for_login_hint(&login_hint),
        expires_at: codes::unix_now() + EXPIRES_IN,
    });

    let body = json!({
        "auth_req_id": auth_req_id,
        "expires_in": EXPIRES_IN,
        "interval": INTERVAL,
    });
    (StatusCode::OK, token::no_store(), Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(client: &str, state: State) -> CibaRequest {
        CibaRequest {
            client_id: client.to_string(),
            scope: "openid".to_string(),
            audience: "http://sim.local:8080".to_string(),
            state,
            expires_at: codes::unix_now() + EXPIRES_IN,
        }
    }

    #[test]
    fn issued_ids_are_unique_and_opaque() {
        let a = issue(sample("c1", State::Approved));
        let b = issue(sample("c1", State::Approved));
        assert_ne!(a, b, "each issue must be unique");
        // base64url(SHA-256) is 43 chars, no padding, url-safe alphabet.
        assert_eq!(a.len(), 43);
        assert!(a
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
    }

    #[test]
    fn approved_request_polls_once_then_is_consumed() {
        let id = issue(sample("c2", State::Approved));
        match poll(&id, "c2") {
            Poll::Approved { scope, audience } => {
                assert_eq!(scope, "openid");
                assert_eq!(audience, "http://sim.local:8080");
            }
            other => panic!("expected Approved, got {other:?}"),
        }
        // Single use: a second poll finds nothing.
        assert_eq!(poll(&id, "c2"), Poll::Unknown);
    }

    #[test]
    fn pending_request_stays_pending_across_polls() {
        let id = issue(sample("c3", State::Pending));
        assert_eq!(poll(&id, "c3"), Poll::Pending);
        assert_eq!(poll(&id, "c3"), Poll::Pending, "pending is not consumed");
    }

    #[test]
    fn denied_request_reports_access_denied() {
        let id = issue(sample("c4", State::Denied));
        assert_eq!(poll(&id, "c4"), Poll::Denied);
    }

    #[test]
    fn poll_rejects_a_mismatched_client() {
        let id = issue(sample("c5", State::Approved));
        assert_eq!(poll(&id, "someone-else"), Poll::WrongClient);
        // The request survives a wrong-client poll (not consumed).
        assert!(matches!(poll(&id, "c5"), Poll::Approved { .. }));
    }

    #[test]
    fn poll_reports_expiry_and_evicts() {
        let mut req = sample("c6", State::Approved);
        req.expires_at = codes::unix_now().saturating_sub(1); // already expired
        let id = issue(req);
        assert_eq!(poll(&id, "c6"), Poll::Expired);
        assert_eq!(poll(&id, "c6"), Poll::Unknown, "expired request is evicted");
    }

    #[test]
    fn poll_unknown_id_is_unknown() {
        assert_eq!(poll("never-issued", "c7"), Poll::Unknown);
    }

    #[test]
    fn login_hint_selects_the_functional_case() {
        assert_eq!(state_for_login_hint("tel:+34600000001"), State::Approved);
        assert_eq!(state_for_login_hint("tel:+34600000000-pending"), State::Pending);
        assert_eq!(state_for_login_hint("tel:+34600000000-DENIED"), State::Denied);
        // `denied` wins if both markers are present (explicit rejection).
        assert_eq!(state_for_login_hint("pending-and-denied"), State::Denied);
    }
}
