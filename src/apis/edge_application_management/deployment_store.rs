//! In-memory Edge Application Management **app-deployment store** (docs/DESIGN.md
//! §4, §5 — "in-memory stores", single node).
//!
//! `POST /deployments` (`createAppDeployment`) deploys an onboarded application
//! across one or more edge cloud zones: it mints an `appDeploymentId` and
//! remembers the rendered `AppDeploymentInfo` so the later read/list/delete legs
//! (`getAppDeployment` / `getAppDeployments` / `deleteAppDeployment`, future
//! passes) can address it. This module is that state, kept apart from the *app*
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
/// such deployment exists. Backs the persistence assertions in the tests; the
/// `getAppDeployment` read leg (a later pass) will use it on the request path
/// too, so it is allowed to be unused in the non-test build for now.
#[cfg_attr(not(test), allow(dead_code))]
pub fn get(deployment_id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("edge-application-management deployment store not poisoned")
        .get(deployment_id)
        .cloned()
}
