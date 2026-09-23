//! In-memory one-time-password store shared by the OTP `send-code` and
//! `validate-code` endpoints (docs/DESIGN.md §5 — "in-memory stores").
//!
//! One Time Password SMS is CamaraSim's first **stateful** API: `POST /send-code`
//! mints an `authenticationId` and remembers the code it "sent", and a later
//! `POST /validate-code` redeems that `authenticationId` against a submitted
//! `code`. This module is the state that bridges the two requests.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime.
//! - **Single use**: a successful [`validate`] *removes* the entry, so a code
//!   cannot be replayed. A redemption of an unknown (never-issued or already
//!   consumed) `authenticationId` reports [`Verdict::Expired`].
//! - **Bounded lifetime**: an entry expires [`OTP_TTL_SECS`] seconds after issue;
//!   validating an expired (but not-yet-evicted) entry reports [`Verdict::Expired`]
//!   and evicts it.
//! - **Bounded attempts**: each entry allows [`MAX_ATTEMPTS`] wrong codes; the
//!   attempt that exhausts them reports [`Verdict::Failed`] and evicts the entry.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

/// How long an issued OTP remains valid, in seconds (5 minutes). A real SMS OTP
/// is short-lived; validating an entry at or after its expiry reports
/// [`Verdict::Expired`].
pub const OTP_TTL_SECS: u64 = 300;

/// How many wrong codes an `authenticationId` tolerates before it is burned. The
/// wrong attempt that reaches this limit reports [`Verdict::Failed`].
pub const MAX_ATTEMPTS: u8 = 3;

/// A pending one-time password, keyed in the store by its `authenticationId`.
struct Otp {
    /// The code the simulator "sent". Deterministic from the phone number so a
    /// headless caller can compute what to validate (see the API description).
    code: String,
    /// Unix expiry; a validation at or after this instant reports `Expired`.
    expires_at: u64,
    /// Wrong-code attempts still allowed before the entry is burned.
    attempts_left: u8,
}

/// The outcome of a [`validate`] call, mapped by the handler onto the CAMARA OTP
/// error codes (all 400) or a 204.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The submitted code matched: success (204). The entry is consumed.
    Ok,
    /// The code did not match and attempts remain → `ONE_TIME_PASSWORD_SMS.INVALID_OTP`.
    InvalidOtp,
    /// The maximum number of attempts has been reached → `…VERIFICATION_FAILED`.
    /// The entry is burned.
    Failed,
    /// The `authenticationId` is unknown, already consumed, or expired →
    /// `…VERIFICATION_EXPIRED`.
    Expired,
}

/// The process-global OTP store. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Otp>> {
    static STORE: OnceLock<Mutex<HashMap<String, Otp>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `code` for [`OTP_TTL_SECS`] seconds under a fresh, opaque
/// `authenticationId`, and return that id.
///
/// The id is the base64url of `SHA-256(counter ‖ now)`: opaque and unique (the
/// monotonic counter alone guarantees uniqueness), mirroring
/// [`crate::auth::codes`] — no `uuid`/`rand` dependency.
pub fn issue(code: String) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(unix_now().to_be_bytes());
    let auth_id = URL_SAFE_NO_PAD.encode(hasher.finalize());
    store().lock().expect("otp store not poisoned").insert(
        auth_id.clone(),
        Otp {
            code,
            expires_at: unix_now() + OTP_TTL_SECS,
            attempts_left: MAX_ATTEMPTS,
        },
    );
    auth_id
}

/// Redeem `auth_id` against `code`, returning the [`Verdict`]. A matching code
/// consumes the entry (single use); an exhausting wrong attempt or an expired
/// entry evicts it. An unknown `auth_id` (never issued or already consumed) is
/// `Expired`.
pub fn validate(auth_id: &str, code: &str) -> Verdict {
    let mut map = store().lock().expect("otp store not poisoned");
    // Decide the verdict and whether to evict, with the entry borrow scoped to
    // this match so the eviction below can borrow the map again.
    let (verdict, evict) = match map.get_mut(auth_id) {
        None => (Verdict::Expired, false),
        Some(entry) => {
            if unix_now() >= entry.expires_at {
                (Verdict::Expired, true)
            } else if entry.attempts_left == 0 {
                (Verdict::Failed, true)
            } else if constant_time_eq(entry.code.as_bytes(), code.as_bytes()) {
                (Verdict::Ok, true)
            } else {
                entry.attempts_left -= 1;
                if entry.attempts_left == 0 {
                    (Verdict::Failed, true)
                } else {
                    (Verdict::InvalidOtp, false)
                }
            }
        }
    };
    if evict {
        map.remove(auth_id);
    }
    verdict
}

/// Constant-time-ish equality over two byte slices (no early return on mismatch).
/// The OTP is not a long-lived secret, but avoiding a data-dependent early exit
/// costs nothing and keeps the comparison honest (mirrors [`crate::auth::codes`]).
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

    #[test]
    fn issued_ids_are_unique_and_opaque() {
        let a = issue("111111".to_string());
        let b = issue("111111".to_string());
        assert_ne!(a, b, "each issue must be unique");
        // base64url(SHA-256) is 43 chars, no padding, url-safe alphabet.
        assert_eq!(a.len(), 43);
        assert!(a
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
    }

    #[test]
    fn correct_code_validates_once_then_is_consumed() {
        let id = issue("424242".to_string());
        assert_eq!(validate(&id, "424242"), Verdict::Ok);
        // Single use: the id is gone, so a replay reports Expired.
        assert_eq!(validate(&id, "424242"), Verdict::Expired);
    }

    #[test]
    fn unknown_id_is_expired() {
        assert_eq!(validate("no-such-authentication-id", "000000"), Verdict::Expired);
    }

    #[test]
    fn wrong_code_is_invalid_until_attempts_exhaust_then_failed() {
        let id = issue("123456".to_string());
        // MAX_ATTEMPTS wrong tries: the first MAX_ATTEMPTS-1 are InvalidOtp, the
        // last one exhausts the budget and burns the entry as Failed.
        for _ in 0..(MAX_ATTEMPTS - 1) {
            assert_eq!(validate(&id, "000000"), Verdict::InvalidOtp);
        }
        assert_eq!(validate(&id, "000000"), Verdict::Failed);
        // The entry is burned, so even the correct code now reports Expired.
        assert_eq!(validate(&id, "123456"), Verdict::Expired);
    }
}
