//! In-memory Edge Application Management **app store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Edge Application Management becomes **stateful** the moment a provider can
//! *submit* (onboard) an application: `POST /apps` (`submitApp`) mints an `appId`
//! and remembers the submitted `AppManifest` so later legs (`getApp` /
//! `getApps` / `deleteApp`, later passes) can address it. This module is that
//! state. It mirrors [`crate::apis::blockchain_public_address::store`]: a
//! process-global `HashMap` guarded by a `std::sync::Mutex`, the lock held only
//! for the map read/write and never across an `.await`, so it never blocks the
//! async runtime.
//!
//! The stored value is the submitted `AppManifest` JSON, keyed by its minted
//! `appId`. The `appId` is derived deterministically from the app's identity —
//! the `(name, version, appProvider)` triple
//! ([`crate::apis::edge_application_management::vwip::app_id`]) — so re-submitting
//! the *same* application hits the *same* id, which is exactly how [`insert`]
//! detects a duplicate for the CAMARA `409 ALREADY_EXISTS` case.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global app store: `appId` → submitted `AppManifest` JSON.
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `manifest` under `app_id`, unless an app already exists under that id.
/// Because `app_id` is derived from the `(name, version, appProvider)` triple,
/// an existing id means the *same* application has already been submitted.
///
/// Returns `true` when the app was newly inserted, `false` when one already
/// existed — `submitApp` maps the latter to `409 ALREADY_EXISTS`. The whole
/// check-and-insert runs under a single lock hold (never across an `.await`), so
/// two concurrent submissions of the same app can't both win.
pub fn insert(app_id: String, manifest: Value) -> bool {
    let mut map = store()
        .lock()
        .expect("edge-application-management app store not poisoned");
    if map.contains_key(&app_id) {
        return false;
    }
    map.insert(app_id, manifest);
    true
}

/// Fetch the `AppManifest` stored under `app_id`, or `None` if no such app
/// exists. Used by the tests to assert persistence (and by later `getApp` legs).
#[cfg_attr(not(test), allow(dead_code))]
pub fn get(app_id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("edge-application-management app store not poisoned")
        .get(app_id)
        .cloned()
}
