//! In-memory **device-mutation (tombstone) store** for In-Home Device Management
//! v1 (docs/DESIGN.md §5 — "in-memory stores").
//!
//! The In-Home Device Management inventory is otherwise **stateless**: a
//! household's roster is regenerated deterministically from its `ssid` on every
//! read (see [`super::v1::household`]). `deleteDevice`
//! (`DELETE /devices/{deviceId}`) is the API's first mutation, and this module is
//! the state that makes it observable: it records which `(ssid, deviceId)` pairs
//! have been deleted, and the read legs (`listDevices` / `getDevice` /
//! `getDeviceNetworkHealth` / `performDeviceAction`) filter those tombstoned
//! devices out of the regenerated roster. A device stays deleted for the life of
//! the process (single node, in-memory only — docs/DESIGN.md §4); there is no
//! un-delete leg.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global
//!   `HashSet<(ssid, deviceId)>` guarded by a `std::sync::Mutex`. The lock is
//!   held only for set reads/writes — never across an `.await` — so it does not
//!   block the async runtime (mirrors the sibling API stores, e.g.
//!   [`crate::apis::click_to_dial::store`]).
//! - We store a **tombstone**, not the device: the device itself is still derived
//!   from the `ssid` (there is no persisted `Device` to hold), so the store only
//!   needs to remember that a given device of a given household is gone.
//! - [`delete`] is the single-use gate: it returns `true` only the first time a
//!   `(ssid, deviceId)` is tombstoned, so a second `deleteDevice` for the same
//!   device answers `404 NOT_FOUND` rather than a second `200`.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// The process-global tombstone set: the `(ssid, deviceId)` pairs that have been
/// deleted. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashSet<(String, String)>> {
    static STORE: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Tombstone the device `device_id` of household `ssid`.
///
/// Returns `true` when the device was newly tombstoned (the `deleteDevice`
/// happy path → `200`), and `false` when it had already been deleted — so a
/// second delete of the same device is a `404 NOT_FOUND` (single-use). The
/// whole check-and-insert runs under one lock hold (never across an `.await`),
/// so two concurrent deletes of the same device can't both win.
pub fn delete(ssid: &str, device_id: &str) -> bool {
    store()
        .lock()
        .expect("in-home device tombstone store not poisoned")
        .insert((ssid.to_string(), device_id.to_string()))
}

/// Whether the device `device_id` of household `ssid` has been deleted. The read
/// legs use this to filter deleted devices out of the regenerated roster.
pub fn is_deleted(ssid: &str, device_id: &str) -> bool {
    store()
        .lock()
        .expect("in-home device tombstone store not poisoned")
        .contains(&(ssid.to_string(), device_id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tombstone_is_single_use_and_scoped_to_the_pair() {
        // A unique ssid so this unit test never collides with the integration tests.
        let ssid = "UnitStoreHouse-unique";
        assert!(!is_deleted(ssid, "dev-x"));
        // First delete wins; a second is a no-op (→ 404 in the handler).
        assert!(delete(ssid, "dev-x"));
        assert!(!delete(ssid, "dev-x"));
        assert!(is_deleted(ssid, "dev-x"));
        // A different device of the same household is untouched.
        assert!(!is_deleted(ssid, "dev-y"));
        // The same deviceId under a different ssid is a different key.
        assert!(!is_deleted("OtherStoreHouse-unique", "dev-x"));
    }
}
