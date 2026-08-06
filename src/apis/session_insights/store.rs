//! In-memory Session Insights **session store** shared by the Session Insights
//! endpoints (docs/DESIGN.md §5 — "in-memory stores").
//!
//! Session Insights is a **stateful, resource-oriented** API: `POST /sessions`
//! creates a session and mints an `id`, and later requests
//! (`GET /sessions/{sessionId}`, and — in later passes — `DELETE` /
//! `retrieve-sessions` / `metrics`) address that resource by its id. This module
//! is the state that bridges those requests.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime (mirrors
//!   [`crate::apis::quality_on_demand::store`]).
//! - **Opaque ids**: [`new_session_id`] mints a UUID-shaped `id` (CAMARA
//!   `SessionInfo.id` is `format: uuid`) from a monotonic counter and the clock,
//!   so ids are unique without a `uuid`/`rand` dependency.
//! - The stored value is the session's rendered `SessionInfo` JSON, returned
//!   verbatim by `GET` — the created representation is the source of truth for
//!   this slice (status transitions / notifications arrive in later passes).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The process-global session store: `id` → rendered `SessionInfo`. In-memory
/// only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `session` (its rendered `SessionInfo` JSON) under `id`.
pub fn insert(id: String, session: Value) {
    store()
        .lock()
        .expect("session-insights store not poisoned")
        .insert(id, session);
}

/// Fetch the `SessionInfo` stored under `id`, or `None` if no such session
/// exists (never created, or created in a different process).
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("session-insights store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the session stored under `id`, returning its `SessionInfo` if one was
/// present, or `None` if no such session existed. `deleteSession` uses the
/// distinction to answer `204 No Content` (a session was deleted) vs `404`
/// (unknown/already-deleted id).
pub fn remove(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("session-insights store not poisoned")
        .remove(id)
}

/// Return a snapshot of every stored `SessionInfo` whose echoed `device` equals
/// `device`. `retrieveSessionsByDevice` uses this to list a device's sessions.
/// The lock is held only for the scan + clone (never across an `.await`), and the
/// returned `Vec` is an independent copy (mirrors
/// [`crate::apis::quality_on_demand::store::find_by_device`]).
pub fn find_by_device(device: &Value) -> Vec<Value> {
    store()
        .lock()
        .expect("session-insights store not poisoned")
        .values()
        .filter(|info| info.get("device") == Some(device))
        .cloned()
        .collect()
}

/// Mint a fresh, opaque, UUID-v4-shaped session `id`.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID, matching CAMARA's `format: uuid`.
/// No `uuid`/`rand` dependency.
pub fn new_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(unix_now().to_be_bytes());
    let d = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Mint a fresh, opaque, UUID-shaped CloudEvent `id`.
///
/// CloudEvents requires the `id` to be unique within a `source`; reusing
/// [`new_session_id`]'s minting gives that without a `uuid`/`rand` dependency
/// (mirrors [`crate::apis::qos_provisioning::store::new_event_id`]). Used by the
/// `quality-score` notification delivered on `sendSessionMetrics`.
pub fn new_event_id() -> String {
    new_session_id()
}

/// Current Unix time in seconds (server runtime clock; not on any hot loop).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn session_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_session_id();
        let b = new_session_id();
        assert_ne!(a, b, "each session id must be unique");
        // 8-4-4-4-12 lowercase hex.
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'));
        // Version nibble is 4; variant nibble is one of 8/9/a/b.
        assert_eq!(parts[2].as_bytes()[0], b'4', "version 4");
        assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn stored_session_can_be_read_back_and_unknown_is_none() {
        let id = new_session_id();
        assert!(get(&id).is_none(), "not stored yet");
        let info = json!({ "id": id, "status": "ACTIVE" });
        insert(id.clone(), info.clone());
        assert_eq!(get(&id), Some(info));
        assert!(get("no-such-session").is_none());
    }

    #[test]
    fn find_by_device_matches_only_sessions_with_that_device_echo() {
        let phone_a = json!({ "phoneNumber": "+15550009001" });
        let phone_b = json!({ "phoneNumber": "+15550009002" });
        let id1 = new_session_id();
        let id2 = new_session_id();
        let id3 = new_session_id();
        insert(id1.clone(), json!({ "id": id1, "device": phone_a, "status": "ACTIVE" }));
        insert(id2.clone(), json!({ "id": id2, "device": phone_a, "status": "ACTIVE" }));
        insert(id3.clone(), json!({ "id": id3, "device": phone_b, "status": "ACTIVE" }));

        let found = find_by_device(&phone_a);
        assert_eq!(found.len(), 2, "both device-A sessions match");
        assert!(found.iter().all(|s| s["device"] == phone_a));
        // An unrelated device matches nothing.
        assert!(find_by_device(&json!({ "phoneNumber": "+15550009099" })).is_empty());
    }

    #[test]
    fn remove_returns_the_session_once_then_none() {
        let id = new_session_id();
        assert!(remove(&id).is_none(), "not stored yet → None");
        let info = json!({ "id": id, "status": "ACTIVE" });
        insert(id.clone(), info.clone());
        // First remove yields the stored session and evicts it…
        assert_eq!(remove(&id), Some(info));
        // …a second remove finds nothing (single-use delete), and get agrees.
        assert!(remove(&id).is_none());
        assert!(get(&id).is_none());
    }
}
