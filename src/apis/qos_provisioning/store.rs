//! In-memory QoS-Provisioning **assignment store** shared by the QoS
//! Provisioning endpoints (docs/DESIGN.md §5 — "in-memory stores").
//!
//! QoS Provisioning is a **stateful, resource-oriented** API:
//! `POST /qos-assignments` provisions a QoS profile for a device and mints an
//! `assignmentId`, and later requests (`GET /qos-assignments/{assignmentId}`,
//! and — in later passes — `DELETE` / retrieve-by-device) address that resource
//! by its id. This module is the state that bridges those requests. It mirrors
//! [`crate::apis::quality_on_demand::store`], the store for QoS Provisioning's
//! sibling API.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime.
//! - **Opaque ids**: [`new_assignment_id`] mints a UUID-shaped `assignmentId`
//!   (CAMARA `AssignmentInfo.assignmentId` is `format: uuid`) from a monotonic
//!   counter and the clock, so ids are unique without a `uuid`/`rand` dependency.
//! - The stored value is the assignment's rendered `AssignmentInfo` JSON,
//!   returned verbatim by `GET` — the created representation is the source of
//!   truth for this slice.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The process-global assignment store: `assignmentId` → rendered
/// `AssignmentInfo`. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `assignment` (its rendered `AssignmentInfo` JSON) under `id`.
pub fn insert(id: String, assignment: Value) {
    store()
        .lock()
        .expect("qos-provisioning store not poisoned")
        .insert(id, assignment);
}

/// Fetch the `AssignmentInfo` stored under `id`, or `None` if no such assignment
/// exists (never created, or created in a different process). `getQosAssignmentById`
/// uses the distinction to answer `200` vs `404`.
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("qos-provisioning store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the assignment stored under `id`, returning its `AssignmentInfo` if one
/// was present, or `None` if no such assignment existed. `revokeQosAssignment`
/// uses the distinction to answer `204` (an assignment was revoked) vs `404`
/// (unknown / already-revoked id). Mirrors [`crate::apis::quality_on_demand::store::remove`].
pub fn remove(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("qos-provisioning store not poisoned")
        .remove(id)
}

/// Return the first stored `AssignmentInfo` whose echoed `device` equals
/// `device`, or `None` if the device has no assignment. `getQosAssignmentByDevice`
/// uses the distinction to answer `200` (the device's assignment) vs `404` (the
/// device has none). QoS Provisioning models at most one provisioning per device,
/// so a single match is returned — unlike QoD's `find_by_device`, which lists a
/// device's sessions as an array. The lock is held only for the scan + clone
/// (never across an `.await`), and the returned value is an independent copy.
pub fn find_by_device(device: &Value) -> Option<Value> {
    store()
        .lock()
        .expect("qos-provisioning store not poisoned")
        .values()
        .find(|info| info.get("device") == Some(device))
        .cloned()
}

/// Mint a fresh, opaque, UUID-shaped `assignmentId`. See [`mint_uuid`] for the shape.
pub fn new_assignment_id() -> String {
    mint_uuid()
}

/// Mint a fresh, opaque, UUID-shaped CloudEvent `id`. CloudEvents requires the
/// `id` to be unique within a `source`; reusing [`mint_uuid`] gives that without a
/// `uuid`/`rand` dependency (mirrors QoD's `new_event_id`).
pub fn new_event_id() -> String {
    mint_uuid()
}

/// The process-global **sink-credential side-store**: `assignmentId` → the derived
/// `Authorization` header value (e.g. `"Bearer <token>"`). Kept apart from the
/// `AssignmentInfo` map so the secret is never echoed by `GET`/retrieve-by-device
/// (mirrors QoD's credential side-store). In-memory only (single node, DESIGN §4).
fn credentials() -> &'static Mutex<HashMap<String, String>> {
    static CREDS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    CREDS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Remember the `Authorization` header value a notification callback for `id`
/// should carry (derived from the assignment's `sinkCredential` at creation).
pub fn insert_credential(id: String, auth: String) {
    credentials()
        .lock()
        .expect("qos-provisioning credential store not poisoned")
        .insert(id, auth);
}

/// Take (single-use) the stored `Authorization` header value for `id`, removing it
/// so the secret drops from memory once the notification has been sent. `None` when
/// the assignment carried no ACCESSTOKEN `sinkCredential`.
pub fn take_credential(id: &str) -> Option<String> {
    credentials()
        .lock()
        .expect("qos-provisioning credential store not poisoned")
        .remove(id)
}

/// Read (clone, non-destructive) the stored `Authorization` header value for `id`,
/// leaving it in place. Used for a **non-terminal** callback — the
/// `AVAILABLE`-on-provisioning event — so the credential survives for the eventual
/// terminal event (revoke / network-termination), which [`take_credential`]s it
/// single-use. `None` when the assignment carried no ACCESSTOKEN `sinkCredential`.
pub fn peek_credential(id: &str) -> Option<String> {
    credentials()
        .lock()
        .expect("qos-provisioning credential store not poisoned")
        .get(id)
        .cloned()
}

/// Mint a fresh, opaque, UUID-v4-shaped identifier.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID, matching CAMARA's `format: uuid`.
/// No `uuid`/`rand` dependency (mirrors the QoD store).
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
    fn assignment_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_assignment_id();
        let b = new_assignment_id();
        assert_ne!(a, b, "each assignment id must be unique");
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
    fn stored_assignment_can_be_read_back_and_unknown_is_none() {
        let id = new_assignment_id();
        assert!(get(&id).is_none(), "not stored yet");
        let info = json!({ "assignmentId": id, "status": "AVAILABLE" });
        insert(id.clone(), info.clone());
        assert_eq!(get(&id), Some(info));
        assert!(get("no-such-assignment").is_none());
    }

    #[test]
    fn find_by_device_matches_only_the_assignment_with_that_device_echo() {
        let device = json!({ "phoneNumber": "+199990000001" });
        // Nothing stored for this device yet.
        assert!(find_by_device(&device).is_none());

        let id = new_assignment_id();
        let info = json!({ "assignmentId": id, "status": "AVAILABLE", "device": device });
        insert(id, info.clone());

        assert_eq!(find_by_device(&device), Some(info));
        // A different device echo matches nothing.
        assert!(find_by_device(&json!({ "phoneNumber": "+199990000099" })).is_none());
    }

    #[test]
    fn event_ids_are_unique_and_uuid_shaped() {
        let a = new_event_id();
        let b = new_event_id();
        assert_ne!(a, b, "each event id must be unique");
        assert_eq!(a.split('-').count(), 5);
    }

    #[test]
    fn credential_side_store_is_single_use() {
        let id = new_assignment_id();
        assert!(take_credential(&id).is_none(), "not stored yet → None");
        insert_credential(id.clone(), "Bearer sekret".to_string());
        // First take returns it; a second finds nothing (dropped from memory).
        assert_eq!(take_credential(&id), Some("Bearer sekret".to_string()));
        assert!(take_credential(&id).is_none(), "single-use → gone");
    }

    #[test]
    fn peek_credential_is_non_destructive_and_take_still_consumes() {
        let id = new_assignment_id();
        assert!(peek_credential(&id).is_none(), "not stored yet → None");
        insert_credential(id.clone(), "Bearer keep".to_string());
        // Peek clones without removing — a second peek still sees it…
        assert_eq!(peek_credential(&id), Some("Bearer keep".to_string()));
        assert_eq!(peek_credential(&id), Some("Bearer keep".to_string()));
        // …and a subsequent take still consumes it single-use.
        assert_eq!(take_credential(&id), Some("Bearer keep".to_string()));
        assert!(peek_credential(&id).is_none(), "gone after take");
    }

    #[test]
    fn remove_evicts_once_and_reports_presence() {
        let id = new_assignment_id();
        assert!(remove(&id).is_none(), "not stored yet → nothing to remove");
        let info = json!({ "assignmentId": id, "status": "AVAILABLE" });
        insert(id.clone(), info.clone());
        // First remove returns the stored value (single-use); a second is a no-op.
        assert_eq!(remove(&id), Some(info));
        assert!(remove(&id).is_none(), "already removed → None");
        assert!(get(&id).is_none(), "gone from the store after remove");
    }
}
