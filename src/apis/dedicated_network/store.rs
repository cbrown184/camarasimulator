//! In-memory **Dedicated Network resource store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Dedicated Network — Networks is **resource-oriented**: `POST /networks`
//! creates a dedicated-network resource and mints its `id`, and the upstream API
//! also exposes `GET /networks/{networkId}` (read-back), `GET /networks` (list)
//! and `DELETE /networks/{networkId}`. This module is the state that bridges
//! create to those later reads. It mirrors
//! [`crate::apis::quality_on_demand::store`] /
//! [`crate::apis::traffic_influence::store`]: a process-global `HashMap` guarded
//! by a `std::sync::Mutex`, the lock held only for the map read/write and never
//! across an `.await`, so it never blocks the async runtime.
//!
//! The stored value is the network's rendered `NetworkInfo` JSON, returned
//! verbatim by a future `GET`. `createNetwork` writes; the read/list/delete legs
//! (later passes) read.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// The process-global network store: `id` → rendered `NetworkInfo` JSON.
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store the rendered `NetworkInfo` `network` under `id`. Called by
/// `createNetwork` once the resource is created, so it can be read back.
pub fn insert(id: String, network: Value) {
    store()
        .lock()
        .expect("dedicated-network store not poisoned")
        .insert(id, network);
}

/// Fetch the `NetworkInfo` stored under `id`, or `None` if no such network
/// exists (never created, or created in a different process). `readNetwork`
/// uses the distinction to answer `200` vs `404`; the delete leg (a later pass)
/// will too.
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("dedicated-network store not poisoned")
        .get(id)
        .cloned()
}

/// Mint a fresh, opaque, UUID-v4-shaped network `id` (CAMARA `NetworkId` is
/// `format: uuid`).
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID. No `uuid`/`rand` dependency
/// (mirrors [`crate::apis::quality_on_demand::store::new_session_id`]).
pub fn new_network_id() -> String {
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
    fn network_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_network_id();
        let b = new_network_id();
        assert_ne!(a, b, "each network id must be unique");
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
    fn stored_network_can_be_read_back_and_unknown_is_none() {
        let id = new_network_id();
        assert!(get(&id).is_none(), "not stored yet");
        let info = json!({ "id": id, "status": "REQUESTED" });
        insert(id.clone(), info.clone());
        assert_eq!(get(&id), Some(info));
        assert!(get("no-such-network").is_none());
    }
}
