//! In-memory **booking store** shared by the QoS Booking endpoints
//! (docs/DESIGN.md §5 — "in-memory stores").
//!
//! QoS Booking is a **stateful, resource-oriented** API: `POST /device-qos-bookings`
//! books a QoS profile for a device over a time window and mints a `bookingId`, and
//! later requests (`GET /device-qos-bookings/{bookingId}`, and — in later passes —
//! `DELETE` / retrieve-by-device) address that resource by its id. This module is the
//! state that bridges those requests. It mirrors
//! [`crate::apis::qos_provisioning::store`], the store for QoS Booking's sibling API.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded by a
//!   `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes — never
//!   across an `.await` — so it does not block the async runtime.
//! - **Opaque ids**: [`new_booking_id`] mints a UUID-shaped `bookingId` (CAMARA
//!   `BookingInfo.bookingId` is `format: uuid`) from a monotonic counter and the
//!   clock, so ids are unique without a `uuid`/`rand` dependency.
//! - The stored value is the booking's rendered `BookingInfo` JSON, returned verbatim
//!   by `GET` — the created representation is the source of truth for this slice.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The process-global booking store: `bookingId` → rendered `BookingInfo`.
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `booking` (its rendered `BookingInfo` JSON) under `id`. `createBooking`
/// persists the created booking here so a later read-back leg (a subsequent pass)
/// can return it.
pub fn insert(id: String, booking: Value) {
    store()
        .lock()
        .expect("qos-booking store not poisoned")
        .insert(id, booking);
}

/// Fetch the `BookingInfo` stored under `id`, or `None` if no such booking exists
/// (never created, or created in a different process). Backs the read-back leg
/// (`getBooking`, `GET /device-qos-bookings/{bookingId}`).
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("qos-booking store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the booking stored under `id`, returning its `BookingInfo` if one was
/// present, or `None` if no such booking existed. `deleteBooking` uses the
/// distinction to answer `204` (a booking was deleted) vs `404` (unknown /
/// already-deleted id). Mirrors [`crate::apis::qos_provisioning::store::remove`].
pub fn remove(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("qos-booking store not poisoned")
        .remove(id)
}

/// Mint a fresh, opaque, UUID-shaped `bookingId`. See [`mint_uuid`] for the shape.
pub fn new_booking_id() -> String {
    mint_uuid()
}

/// Mint a fresh, opaque, UUID-v4-shaped identifier.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits set
/// so it is a well-formed v4-shaped UUID, matching CAMARA's `format: uuid`. No
/// `uuid`/`rand` dependency (mirrors the QoS Provisioning store).
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
    fn booking_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_booking_id();
        let b = new_booking_id();
        assert_ne!(a, b, "each booking id must be unique");
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
    fn stored_booking_can_be_read_back_and_unknown_is_none() {
        let id = new_booking_id();
        assert!(get(&id).is_none(), "not stored yet");
        let info = json!({ "bookingId": id, "bookingStatus": "SCHEDULED" });
        insert(id.clone(), info.clone());
        assert_eq!(get(&id), Some(info));
        assert!(get("no-such-booking").is_none());
    }

    #[test]
    fn remove_evicts_the_booking_and_is_single_use() {
        let id = new_booking_id();
        let info = json!({ "bookingId": id, "bookingStatus": "ACTIVATED" });
        insert(id.clone(), info.clone());
        // First remove returns the stored booking; a second returns None.
        assert_eq!(remove(&id), Some(info));
        assert!(remove(&id).is_none(), "single-use: already removed");
        assert!(get(&id).is_none(), "booking is gone after remove");
        // Removing an id that was never stored is None.
        assert!(remove("no-such-booking").is_none());
    }
}
