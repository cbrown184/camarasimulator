//! In-memory Edge Application Management **app-instance store** (docs/DESIGN.md
//! §4, §5 — "in-memory stores", single node).
//!
//! `POST /app-instances` (`createAppInstance`) instantiates an onboarded
//! application onto a specific edge cloud zone: it mints an `appInstanceId` and
//! remembers the rendered `AppInstanceInfo` so the later read/list/delete legs
//! (`getAppInstance` / `getAppInstances` / `deleteAppInstance`, future passes)
//! can address it. This module is that state, kept apart from the *app* store
//! ([`super::store`]) so the two resources don't share a keyspace.
//!
//! It mirrors [`super::store`]: a process-global `HashMap` guarded by a
//! `std::sync::Mutex`, the lock held only for the map read/write and never
//! across an `.await`, so it never blocks the async runtime.
//!
//! The stored value is the rendered `AppInstanceInfo` JSON, keyed by its minted
//! `appInstanceId`. The `appInstanceId` is derived deterministically from the
//! `(appId, edgeCloudZoneId)` pair
//! ([`super::vwip::instance_id`]) — instantiating the *same* application onto
//! the *same* zone hits the *same* id, which is exactly how [`insert`] detects
//! the CAMARA `409` "already instantiated in the given Edge Cloud Zone" case.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global app-instance store: `appInstanceId` → `AppInstanceInfo`
/// JSON. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `info` under `instance_id`, unless an instance already exists there.
/// Because `instance_id` is derived from the `(appId, edgeCloudZoneId)` pair, an
/// existing id means the *same* application is already instantiated on the
/// *same* zone.
///
/// Returns `true` when the instance was newly inserted, `false` when one already
/// existed — `createAppInstance` maps the latter to `409 ALREADY_EXISTS`. The
/// whole check-and-insert runs under a single lock hold (never across an
/// `.await`), so two concurrent instantiations of the same (app, zone) can't
/// both win.
pub fn insert(instance_id: String, info: Value) -> bool {
    let mut map = store()
        .lock()
        .expect("edge-application-management instance store not poisoned");
    if map.contains_key(&instance_id) {
        return false;
    }
    map.insert(instance_id, info);
    true
}

/// Fetch the `AppInstanceInfo` stored under `instance_id`, or `None` if no such
/// instance exists. Backs the `getAppInstance` read leg (and the persistence
/// assertions in the tests).
pub fn get(instance_id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("edge-application-management instance store not poisoned")
        .get(instance_id)
        .cloned()
}

/// Snapshot every stored app instance as its rendered `AppInstanceInfo`. Backs
/// the `getAppInstances` list leg. The lock is held only for the clone (never
/// across an `.await`); iteration order is unspecified (a `HashMap`), which is
/// fine — each `AppInstanceInfo` carries its own `appInstanceId`, so callers key
/// off that, not order.
pub fn all() -> Vec<Value> {
    store()
        .lock()
        .expect("edge-application-management instance store not poisoned")
        .values()
        .cloned()
        .collect()
}

/// Evict the app instance stored under `instance_id`, returning its
/// `AppInstanceInfo` when one was present (and `None` when nothing was stored
/// there). Backs the `deleteAppInstance` leg: a `Some` means the instance
/// existed and is now gone → `204 No Content`; a `None` means an
/// unknown/already-deleted id → `404 NOT_FOUND`. The remove-and-report runs
/// under a single lock hold (never across an `.await`), so a `deleteAppInstance`
/// is single-use — two concurrent deletes of the same instance can't both see it
/// present.
pub fn remove(instance_id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("edge-application-management instance store not poisoned")
        .remove(instance_id)
}
