//! In-memory **Traffic Influence resource store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Traffic Influence is **resource-oriented**: `POST /traffic-influences`
//! creates a `TrafficInfluence` resource and mints its `trafficInfluenceID`, and
//! the upstream API also exposes `GET /traffic-influences/{trafficInfluenceID}`
//! (read-back) plus PATCH/DELETE. This module is the state that bridges create to
//! those later reads. It mirrors [`crate::apis::carrier_billing::store`]: a
//! process-global `HashMap` guarded by a `std::sync::Mutex`, the lock held only
//! for the map read/write and never across an `.await`, so it never blocks the
//! async runtime.
//!
//! Like Carrier Billing (and unlike the small typed Sponsored Data record), the
//! store keeps the **rendered** `TrafficInfluence` JSON so `getTrafficInfluence`
//! can return it verbatim. `postTrafficInfluence` writes; `getTrafficInfluence`
//! reads (see `vwip.rs`).

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The process-global resource store: `trafficInfluenceID` → rendered
/// `TrafficInfluence` JSON. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store the rendered `TrafficInfluence` `resource` under `id`. Called by
/// `postTrafficInfluence` once the resource is created, so it can be read back.
pub fn insert(id: String, resource: Value) {
    store()
        .lock()
        .expect("traffic-influence store not poisoned")
        .insert(id, resource);
}

/// Fetch the resource stored under `id`, or `None` if no such resource exists
/// (never created, or created in a different process). `getTrafficInfluence`
/// uses the distinction to answer `200` (found) vs `404 NOT_FOUND` (unknown id).
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("traffic-influence store not poisoned")
        .get(id)
        .cloned()
}

/// Apply `f` to the resource stored under `id` in place, returning the updated
/// resource (cloned) if one was present, or `None` if there was no such resource.
/// The get-modify-write happens under a single lock hold — never across an
/// `.await` — so a concurrent `patchTrafficInfluence` / `deleteTrafficInfluence`
/// on the same id can never interleave. `patchTrafficInfluence` uses the
/// distinction to answer `200` (updated resource) vs `404 NOT_FOUND` (unknown id).
pub fn update_with<F: FnOnce(&mut Value)>(id: &str, f: F) -> Option<Value> {
    let mut guard = store()
        .lock()
        .expect("traffic-influence store not poisoned");
    match guard.get_mut(id) {
        Some(resource) => {
            f(resource);
            Some(resource.clone())
        }
        None => None,
    }
}

/// Evict the resource stored under `id`, returning `true` if one was present
/// (and is now removed) or `false` if there was no such resource. The
/// check-and-remove happens under a single lock hold, so a concurrent
/// `deleteTrafficInfluence` on the same id can succeed at most once.
/// `deleteTrafficInfluence` uses the distinction to answer `202` (deletion
/// accepted for a resource that existed) vs `404 NOT_FOUND` (unknown/already
/// deleted id).
pub fn remove(id: &str) -> bool {
    store()
        .lock()
        .expect("traffic-influence store not poisoned")
        .remove(id)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stored_resource_can_be_read_back_and_unknown_is_none() {
        // Uniquely-keyed so this shares the process-global store with nothing else.
        let id = "ti-store-unit-0001".to_string();
        assert!(get(&id).is_none(), "not stored yet → None");
        insert(
            id.clone(),
            json!({ "trafficInfluenceID": id, "state": "ordered" }),
        );
        let got = get(&id).expect("stored → Some");
        assert_eq!(got["trafficInfluenceID"], id);
        assert_eq!(got["state"], "ordered");
        assert!(get("ti-store-unit-no-such").is_none());
    }

    #[test]
    fn remove_evicts_once_then_reports_absent() {
        // Uniquely-keyed so this shares the process-global store with nothing else.
        let id = "ti-store-unit-remove-0001".to_string();
        assert!(!remove(&id), "nothing stored yet → false");
        insert(id.clone(), json!({ "trafficInfluenceID": id, "state": "active" }));
        assert!(get(&id).is_some(), "stored → present");
        assert!(remove(&id), "first remove evicts → true");
        assert!(get(&id).is_none(), "gone after remove");
        assert!(!remove(&id), "second remove → false (single-use)");
    }

    #[test]
    fn update_with_mutates_in_place_and_unknown_is_none() {
        // Uniquely-keyed so this shares the process-global store with nothing else.
        let id = "ti-store-unit-update-0001".to_string();
        assert!(
            update_with(&id, |_| unreachable!("closure must not run for an absent id")).is_none(),
            "nothing stored yet → None"
        );
        insert(
            id.clone(),
            json!({ "trafficInfluenceID": id, "state": "active", "edgeCloudRegion": "eu-west-1" }),
        );
        let updated = update_with(&id, |v| {
            let obj = v.as_object_mut().unwrap();
            obj.insert("edgeCloudRegion".into(), json!("us-east-1"));
        })
        .expect("stored → Some");
        // The returned clone reflects the mutation …
        assert_eq!(updated["edgeCloudRegion"], "us-east-1");
        // … and so does a fresh read-back (the mutation persisted).
        assert_eq!(get(&id).unwrap()["edgeCloudRegion"], "us-east-1");
        assert_eq!(get(&id).unwrap()["state"], "active", "unrelated fields untouched");
    }
}
