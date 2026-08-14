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
//!
//! ## Device overlays (`updateDevice`)
//!
//! `updateDevice` (`PATCH /devices/{deviceId}`) is the API's second mutation. Like
//! `deleteDevice` it cannot rewrite a persisted row (there is none — the roster is
//! derived from the `ssid`); instead it records a small **overlay** of the mutable
//! fields (`deviceName` / `blocked` / `paused`) per `(ssid, deviceId)`, and the read
//! legs apply that overlay on top of the regenerated device (see
//! [`super::v1::live_household`]). The overlay is merged on each PATCH (partial
//! update — an absent field leaves the prior value in place) and survives for the
//! life of the process (single node, in-memory only — docs/DESIGN.md §4).

use std::collections::{HashMap, HashSet};
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

/// A partial update of a device's mutable fields (`updateDevice`). Each field is
/// `Some` only when the caller set it; `None` leaves the prior value untouched, so
/// overlays merge with PATCH semantics.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Overlay {
    /// A new human-friendly name for the device.
    pub device_name: Option<String>,
    /// Whether the device is blocked from the network.
    pub blocked: Option<bool>,
    /// Whether the device's internet access is paused.
    pub paused: Option<bool>,
}

impl Overlay {
    /// Whether this overlay carries no changes (an empty PATCH body).
    pub fn is_empty(&self) -> bool {
        self.device_name.is_none() && self.blocked.is_none() && self.paused.is_none()
    }
}

/// The process-global overlay map: the mutable-field overrides recorded for a
/// `(ssid, deviceId)` by `updateDevice`. In-memory only (single node, per DESIGN §4).
fn overlays() -> &'static Mutex<HashMap<(String, String), Overlay>> {
    static OVERLAYS: OnceLock<Mutex<HashMap<(String, String), Overlay>>> = OnceLock::new();
    OVERLAYS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Merge `patch` into the overlay for device `device_id` of household `ssid` and
/// persist it, returning the resulting merged overlay. PATCH semantics: only the
/// `Some` fields of `patch` overwrite; absent fields keep whatever a prior update
/// set. The whole read-merge-write runs under one lock hold (never across an
/// `.await`), so concurrent PATCHes of the same device don't interleave.
pub fn merge_overlay(ssid: &str, device_id: &str, patch: Overlay) -> Overlay {
    let mut map = overlays()
        .lock()
        .expect("in-home device overlay store not poisoned");
    let entry = map
        .entry((ssid.to_string(), device_id.to_string()))
        .or_default();
    if patch.device_name.is_some() {
        entry.device_name = patch.device_name;
    }
    if patch.blocked.is_some() {
        entry.blocked = patch.blocked;
    }
    if patch.paused.is_some() {
        entry.paused = patch.paused;
    }
    entry.clone()
}

/// The overlay recorded for device `device_id` of household `ssid`, if any. The
/// read legs apply it on top of the regenerated device.
pub fn overlay(ssid: &str, device_id: &str) -> Option<Overlay> {
    overlays()
        .lock()
        .expect("in-home device overlay store not poisoned")
        .get(&(ssid.to_string(), device_id.to_string()))
        .cloned()
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

    #[test]
    fn overlay_merges_with_patch_semantics_and_is_scoped_to_the_pair() {
        let ssid = "UnitOverlayHouse-unique";
        // No overlay to start.
        assert_eq!(overlay(ssid, "dev-o"), None);

        // First patch sets a name and blocks the device.
        let merged = merge_overlay(
            ssid,
            "dev-o",
            Overlay {
                device_name: Some("Kids Tablet".into()),
                blocked: Some(true),
                paused: None,
            },
        );
        assert_eq!(merged.device_name.as_deref(), Some("Kids Tablet"));
        assert_eq!(merged.blocked, Some(true));
        assert_eq!(merged.paused, None);

        // A second patch that only pauses leaves the earlier name/blocked in place.
        let merged = merge_overlay(
            ssid,
            "dev-o",
            Overlay {
                device_name: None,
                blocked: None,
                paused: Some(true),
            },
        );
        assert_eq!(merged.device_name.as_deref(), Some("Kids Tablet"));
        assert_eq!(merged.blocked, Some(true));
        assert_eq!(merged.paused, Some(true));
        assert_eq!(overlay(ssid, "dev-o"), Some(merged));

        // A different device of the same household has no overlay.
        assert_eq!(overlay(ssid, "dev-other"), None);
        // An empty overlay reports empty.
        assert!(Overlay::default().is_empty());
    }
}
