//! In-memory **application-endpoint-list store** for the Application Endpoint
//! Registration API (docs/DESIGN.md §5 — "in-memory stores").
//!
//! Application Endpoint Registration is a **stateful, resource-oriented** API:
//! `POST /application-endpoint-lists` registers a set of application endpoints and
//! mints an opaque `applicationEndpointListId`, and later requests
//! (`GET /application-endpoint-lists/{id}`, `PUT`, `DELETE` — landing in later
//! passes) address that resource by its id. This module is the state that bridges
//! those requests. It mirrors [`crate::apis::application_profiles::store`] (opaque
//! UUID keys, single node), trimmed to what the register slice needs.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime.
//! - **Opaque ids**: [`new_list_id`] mints a UUID-shaped `applicationEndpointListId`
//!   (CAMARA `ApplicationEndpointListId` is `format: uuid`) from a monotonic
//!   counter and the clock, so ids are unique without a `uuid`/`rand` dependency.
//! - The stored value is the registration's rendered JSON in the canonical CAMARA
//!   `ApplicationEndpointList` shape (`applicationEndpointListId` +
//!   `applicationEndpointsInfo`), returned verbatim by the read leg — the created
//!   representation is the source of truth.
//!
//! The read leg (`getApplicationEndpointsById`) calls [`get`], the list leg
//! (`getAllRegisteredApplicationEndpoints`) calls [`all`], and the deregister leg
//! (`deregisterApplicationEndpoint`) calls [`remove`]; the update (`PUT`) leg
//! lands in a later pass, so the module stays `dead_code`-allowed until it wires
//! in its store operations (mirroring the first slices of the QoS Provisioning /
//! Session Insights stores).

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The process-global registration store: `applicationEndpointListId` → rendered
/// registration JSON. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `registration` (its rendered JSON) under `id`.
pub fn insert(id: String, registration: Value) {
    store()
        .lock()
        .expect("application-endpoint-registration store not poisoned")
        .insert(id, registration);
}

/// Fetch the registration stored under `id`, or `None` if no such registration
/// exists. The read leg (a later pass) uses the distinction to answer `200` vs
/// `404 NOT_FOUND`.
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("application-endpoint-registration store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the registration stored under `id`, returning it if it existed (or
/// `None` if no such registration exists). The deregister leg
/// (`deregisterApplicationEndpoint`) uses the distinction to answer `204` vs
/// `404 NOT_FOUND` (mirroring the QoD session store's `remove`).
pub fn remove(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("application-endpoint-registration store not poisoned")
        .remove(id)
}

/// Return every stored registration (the rendered `ApplicationEndpointList`
/// values), used by the list leg (`getAllRegisteredApplicationEndpoints`) →
/// `200` with an array (empty when nothing is registered — a list never 404s).
///
/// The `HashMap` iteration order is unspecified, so the results are sorted by
/// `applicationEndpointListId` to give a deterministic response (mirroring the
/// Carrier Billing payment list). The lock is held only for the clone-and-sort,
/// never across an `.await`.
pub fn all() -> Vec<Value> {
    let mut lists: Vec<Value> = store()
        .lock()
        .expect("application-endpoint-registration store not poisoned")
        .values()
        .cloned()
        .collect();
    lists.sort_by(|a, b| {
        a.get("applicationEndpointListId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(
                b.get("applicationEndpointListId")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            )
    });
    lists
}

/// Mint a fresh, opaque, UUID-shaped `applicationEndpointListId`. See [`mint_uuid`].
pub fn new_list_id() -> String {
    mint_uuid()
}

/// Mint a fresh, opaque, UUID-v4-shaped identifier.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID, matching CAMARA's `format: uuid`.
/// No `uuid`/`rand` dependency (mirrors the Application Profiles / QoD stores).
fn mint_uuid() -> String {
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
    fn list_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_list_id();
        let b = new_list_id();
        assert_ne!(a, b, "each list id must be unique");
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
    fn stored_registration_can_be_read_back_and_unknown_is_none() {
        let id = new_list_id();
        assert!(get(&id).is_none(), "not stored yet");
        let reg = json!({ "applicationEndpointListId": id, "applicationProviderName": "Acme" });
        insert(id.clone(), reg.clone());
        assert_eq!(get(&id), Some(reg));
        assert!(get("no-such-list").is_none());
    }

    #[test]
    fn all_includes_inserted_lists_and_is_sorted_by_id() {
        // The store is process-global (shared across the test binary), so assert
        // containment rather than an exact count: a freshly inserted id appears
        // in `all()`, and the results are sorted by applicationEndpointListId.
        let id = new_list_id();
        let reg = json!({ "applicationEndpointListId": id, "applicationEndpointsInfo": {} });
        insert(id.clone(), reg.clone());
        let lists = all();
        assert!(lists.contains(&reg), "all() must include the inserted list");
        let ids: Vec<&str> = lists
            .iter()
            .filter_map(|v| v.get("applicationEndpointListId").and_then(Value::as_str))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "all() must be sorted by applicationEndpointListId");
    }
}
