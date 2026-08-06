//! In-memory Click to Dial **call store** shared by the Click to Dial endpoints
//! (docs/DESIGN.md §5 — "in-memory stores").
//!
//! Click to Dial becomes **stateful** the moment a created call can be read back:
//! `POST /calls` (`createCall`) mints a `callId` and this module remembers the
//! rendered `Call`, so `GET /calls/{callId}` (`getCall`) can return it — or a
//! `404 NOT_FOUND` for an id that was never created. This module is the state
//! that bridges those two requests.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime (mirrors
//!   [`crate::apis::quality_on_demand::store`] and
//!   [`crate::apis::carrier_billing::store`]).
//! - The stored value is the call's rendered `Call` JSON, returned verbatim by
//!   `getCall`. `terminateCall` (`DELETE /calls/{callId}`) evicts it via
//!   [`remove`], so a subsequent `getCall` returns `404`. `getRecording`
//!   (`GET /calls/{callId}/recording`) reads the same stored `Call` — its
//!   `recordingEnabled` flag decides whether a recording is available — so it
//!   needs no store method of its own. Lifecycle `status` transitions arrive in
//!   a later slice.
//! - The `callId` is already a deterministic, UUID-shaped token derived from the
//!   participant pair by `createCall`, so this store mints no ids of its own.
//!   Because the id is deterministic, an id already present means the *same*
//!   caller/callee pair already has a live call — so [`insert_new`] refuses the
//!   duplicate (`createCall` maps it to `409 ALREADY_EXISTS`), rather than
//!   overwriting; the pair becomes creatable again only once `terminateCall`
//!   evicts it.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global call store: `callId` → rendered `Call`. In-memory only
/// (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `call` (its rendered `Call` JSON) under `id`, unless a call already
/// exists under that id. `createCall` calls this so the call can later be read
/// back by `getCall`. Because the `callId` is deterministic from the participant
/// pair, an id already present means the *same* caller/callee pair already has a
/// live call.
///
/// Returns `true` when the call was newly inserted, `false` when one already
/// existed — `createCall` maps the latter to `409 ALREADY_EXISTS` (rather than
/// overwriting the live call). The whole check-and-insert runs under a single
/// lock hold (never across an `.await`), so two concurrent creates of the same
/// pair can't both win.
pub fn insert_new(id: String, call: Value) -> bool {
    let mut map = store()
        .lock()
        .expect("click-to-dial call store not poisoned");
    if map.contains_key(&id) {
        return false;
    }
    map.insert(id, call);
    true
}

/// Fetch the `Call` stored under `id`, or `None` if no such call exists (never
/// created, or created in a different process). `getCall` uses the distinction to
/// answer `200` (the stored call) vs `404 NOT_FOUND` (unknown id).
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("click-to-dial call store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the `Call` stored under `id`, returning `true` if a call was present
/// (and is now gone) or `false` if no such call existed. `terminateCall` uses the
/// distinction to answer `204 No Content` (the call was terminated) vs
/// `404 NOT_FOUND` (unknown/never-created id). The check-and-remove is atomic
/// under the store lock, so two concurrent terminates never both see the call.
pub fn remove(id: &str) -> bool {
    store()
        .lock()
        .expect("click-to-dial call store not poisoned")
        .remove(id)
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stored_call_can_be_read_back_and_unknown_is_none() {
        // A uniquely-keyed id so this test shares the process-global store with
        // nothing else.
        let id = "store-unit-call-a";
        assert!(get(id).is_none(), "not stored yet → None");
        let call = json!({ "callId": id, "status": "initiating" });
        assert!(insert_new(id.to_string(), call.clone()), "first insert wins");
        assert_eq!(get(id), Some(call));
        assert!(get("store-unit-no-such-call").is_none());
    }

    #[test]
    fn remove_reports_presence_and_evicts_the_call() {
        let id = "store-unit-call-c";
        // Removing something never stored → false (nothing to terminate).
        assert!(!remove(id), "unknown id → false");
        assert!(insert_new(id.to_string(), json!({ "callId": id, "status": "initiating" })));
        // First remove sees the call and evicts it.
        assert!(remove(id), "present id → true");
        assert!(get(id).is_none(), "evicted → gone");
        // A second remove no longer sees it (single-use eviction).
        assert!(!remove(id), "already removed → false");
    }

    #[test]
    fn insert_new_refuses_a_duplicate_id_and_keeps_the_stored_call() {
        // A repeat createCall for the same pair mints the same deterministic id;
        // the second insert must be refused (→ 409 ALREADY_EXISTS) and must not
        // overwrite the live call.
        let id = "store-unit-call-b";
        let call = json!({ "callId": id, "status": "initiating" });
        assert!(insert_new(id.to_string(), call.clone()), "first insert wins");
        // A different value under the same id: refused, original preserved.
        let other = json!({ "callId": id, "status": "different" });
        assert!(!insert_new(id.to_string(), other), "duplicate id → false");
        assert_eq!(get(id), Some(call), "the live call is not overwritten");
        // Once evicted, the id is creatable again.
        assert!(remove(id));
        assert!(insert_new(id.to_string(), json!({ "callId": id })), "re-creatable after evict");
    }
}
