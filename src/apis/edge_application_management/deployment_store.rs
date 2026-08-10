//! In-memory Edge Application Management **app-deployment store** (docs/DESIGN.md
//! §4, §5 — "in-memory stores", single node).
//!
//! `POST /deployments` (`createAppDeployment`) deploys an onboarded application
//! across one or more edge cloud zones: it mints an `appDeploymentId` and
//! remembers the rendered `AppDeploymentInfo` so `getAppDeployment` can read it
//! back (and the later list/delete/patch legs — `getAppDeployments` /
//! `deleteAppDeployment` / `updateAppDeployment`, future passes — can address
//! it). This module is that state, kept apart from the *app*
//! store ([`super::store`]) and the *app-instance* store
//! ([`super::instance_store`]) so the three resources don't share a keyspace.
//!
//! It mirrors [`super::instance_store`]: a process-global `HashMap` guarded by a
//! `std::sync::Mutex`, the lock held only for the map read/write and never across
//! an `.await`, so it never blocks the async runtime.
//!
//! The stored value is the rendered `AppDeploymentInfo` JSON, keyed by its minted
//! `appDeploymentId`. That id is derived deterministically from the deployment's
//! identity — the `(appId, appDeploymentName, edgeCloudZones)` triple
//! ([`super::vwip::deployment_id`]) — so deploying the *same* application under
//! the *same* name across the *same* zones hits the *same* id, which is exactly
//! how [`insert`] detects the CAMARA `409` "Deployment already exists" case.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global app-deployment store: `appDeploymentId` →
/// `AppDeploymentInfo` JSON. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `info` under `deployment_id`, unless a deployment already exists there.
/// Because `deployment_id` is derived from the deployment's identity, an existing
/// id means the *same* deployment was already created.
///
/// Returns `true` when the deployment was newly inserted, `false` when one
/// already existed — `createAppDeployment` maps the latter to `409
/// ALREADY_EXISTS`. The whole check-and-insert runs under a single lock hold
/// (never across an `.await`), so two concurrent creations of the same
/// deployment can't both win.
pub fn insert(deployment_id: String, info: Value) -> bool {
    let mut map = store()
        .lock()
        .expect("edge-application-management deployment store not poisoned");
    if map.contains_key(&deployment_id) {
        return false;
    }
    map.insert(deployment_id, info);
    true
}

/// Fetch the `AppDeploymentInfo` stored under `deployment_id`, or `None` if no
/// such deployment exists. Used on the request path by the `getAppDeployment`
/// read leg, and by the persistence assertions in the tests.
pub fn get(deployment_id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("edge-application-management deployment store not poisoned")
        .get(deployment_id)
        .cloned()
}

/// Snapshot every stored app deployment as its rendered `AppDeploymentInfo`.
/// Backs the `getAppDeployments` list leg. The lock is held only for the clone
/// (never across an `.await`); iteration order is unspecified (a `HashMap`),
/// which is fine — each `AppDeploymentInfo` carries its own `appDeploymentId`, so
/// callers key off that, not order.
pub fn all() -> Vec<Value> {
    store()
        .lock()
        .expect("edge-application-management deployment store not poisoned")
        .values()
        .cloned()
        .collect()
}

/// Evict the app deployment stored under `deployment_id`, returning its
/// `AppDeploymentInfo` when one was present (and `None` when nothing was stored
/// there). Backs the `deleteAppDeployment` leg: a `Some` means the deployment
/// existed and is now gone → `204 No Content`; a `None` means an
/// unknown/already-deleted id → `404 NOT_FOUND`. The remove-and-report runs under
/// a single lock hold (never across an `.await`), so a `deleteAppDeployment` is
/// single-use — two concurrent deletes of the same deployment can't both see it
/// present.
pub fn remove(deployment_id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("edge-application-management deployment store not poisoned")
        .remove(deployment_id)
}
