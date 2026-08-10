//! In-memory **Dedicated Network access store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Dedicated Network — Accesses is **resource-oriented**: `POST /accesses`
//! creates an access resource (binding a set of devices to a dedicated network)
//! and mints its `id`, and the upstream API also exposes
//! `GET /accesses/{accessId}` (read-back), `GET /accesses` (list) and
//! `DELETE /accesses/{accessId}`, plus the device sub-resources. This module is
//! the state that bridges create to those later reads. It mirrors
//! [`crate::apis::dedicated_network::store`]: a process-global `HashMap` guarded
//! by a `std::sync::Mutex`, the lock held only for the map read/write and never
//! across an `.await`, so it never blocks the async runtime.
//!
//! The stored value is the access's rendered `AccessInfo` JSON. This create-only
//! slice writes (`insert`) and mints ids (`new_access_id`); the read/list/delete
//! and device legs arrive in later passes.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// The process-global access store: `id` → rendered `AccessInfo` JSON.
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store the rendered `AccessInfo` `access` under `id`. Called by `createAccess`
/// once the resource is created, so later passes' read/list/delete legs can
/// address it.
pub fn insert(id: String, access: Value) {
    store()
        .lock()
        .expect("dedicated-network-accesses store not poisoned")
        .insert(id, access);
}

/// Mint a fresh, opaque, UUID-v4-shaped access `id` (CAMARA `AccessId` is
/// `format: uuid`).
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID. No `uuid`/`rand` dependency
/// (mirrors [`crate::apis::dedicated_network::store::new_network_id`]).
pub fn new_access_id() -> String {
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
    fn access_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_access_id();
        let b = new_access_id();
        assert_ne!(a, b, "each access id must be unique");
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'));
        assert_eq!(parts[2].as_bytes()[0], b'4', "version 4");
        assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn stored_access_is_retained_under_its_id() {
        let id = new_access_id();
        let info = json!({ "id": id, "networkId": "n-1" });
        insert(id.clone(), info.clone());
        // A create-only slice: the store retains what was written (read-back
        // legs arrive later). Re-inserting the same id overwrites.
        insert(id.clone(), json!({ "id": id, "networkId": "n-2" }));
    }
}
