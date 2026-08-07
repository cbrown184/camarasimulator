//! In-memory **reboot-request store** for the Network Access Devices API
//! (docs/DESIGN.md §5 — "in-memory stores").
//!
//! The reboot-request resource lifecycle is **stateful**: `POST /reboot-requests`
//! creates a reboot request targeting one or more of the subscriber's Network
//! Access Devices and mints a `rebootRequestId`; later slices read/patch/delete it
//! by that id. This module is the state that bridges those requests. It mirrors
//! [`crate::apis::qos_provisioning::store`].
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime.
//! - **Opaque ids**: [`new_reboot_request_id`] mints a UUID-v4-shaped id from a
//!   monotonic counter and the clock, so ids are unique without a `uuid`/`rand`
//!   dependency.
//! - The stored value is the reboot request's rendered `RebootRequest` JSON,
//!   returned verbatim by a later `GET` — the created representation is the source
//!   of truth for this slice.
//!
//! `insert` + `new_reboot_request_id` (create), `get` (`getRebootRequest` read),
//! and `remove` (`deleteRebootRequest`) are all live.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The process-global reboot-request store: `rebootRequestId` → rendered
/// `RebootRequest`. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `request` (its rendered `RebootRequest` JSON) under `id`.
pub fn insert(id: String, request: Value) {
    store()
        .lock()
        .expect("network-access-devices reboot-request store not poisoned")
        .insert(id, request);
}

/// Fetch the `RebootRequest` stored under `id`, or `None` if no such request
/// exists. A later `getRebootRequest` uses the distinction to answer `200` vs
/// `404`.
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("network-access-devices reboot-request store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the reboot request stored under `id`, returning its `RebootRequest` if
/// one was present, or `None` if no such request existed. A later
/// `deleteRebootRequest` uses the distinction to answer `204` vs `404`.
pub fn remove(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("network-access-devices reboot-request store not poisoned")
        .remove(id)
}

/// Mint a fresh, opaque, UUID-v4-shaped `rebootRequestId`.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID, matching CAMARA's `format: uuid`.
/// No `uuid`/`rand` dependency (mirrors the QoS-Provisioning store).
pub fn new_reboot_request_id() -> String {
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
    fn reboot_request_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_reboot_request_id();
        let b = new_reboot_request_id();
        assert_ne!(a, b, "each reboot-request id must be unique");
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
    fn stored_request_reads_back_and_remove_is_single_use() {
        let id = new_reboot_request_id();
        assert!(get(&id).is_none(), "not stored yet → None");
        let req = json!({ "id": id, "devices": [] });
        insert(id.clone(), req.clone());
        assert_eq!(get(&id), Some(req.clone()));
        assert!(get("no-such-request").is_none());

        // First remove returns the stored value (single-use); a second is a no-op.
        assert_eq!(remove(&id), Some(req));
        assert!(remove(&id).is_none(), "already removed → None");
        assert!(get(&id).is_none(), "gone from the store after remove");
    }
}
