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
//!   [`remove`], which returns the removed `Call` so `terminateCall` can read
//!   its participants for the terminal `status-changed` notification; a
//!   subsequent `getCall` then returns `404`. `getRecording`
//!   (`GET /calls/{callId}/recording`) reads the same stored `Call` — its
//!   `recordingEnabled` flag decides whether a recording is available — so it
//!   needs no store method of its own.
//! - A **sink side-store** (keyed by `callId`) holds the `sink` URL and the
//!   derived callback `Authorization` header for a call that was created with a
//!   `sink`, kept apart from the `Call` map so the callback secret is never
//!   echoed by `getCall`/`getRecording`. `createCall` records it via
//!   [`insert_sink`]; `terminateCall` takes it (single-use) via [`take_sink`] to
//!   deliver the terminal `status-changed` CloudEvent, so the secret drops from
//!   memory as the call ends (mirrors QoD's credential side-store). Ongoing
//!   lifecycle `status` transitions (a live call engine) remain a later slice.
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

/// Remove the `Call` stored under `id`, returning the removed `Call` (`Some`) if
/// one was present, or `None` if no such call existed. `terminateCall` uses the
/// distinction to answer `204 No Content` (the call was terminated) vs
/// `404 NOT_FOUND` (unknown/never-created id), and reads the returned `Call`'s
/// participants for the terminal `status-changed` notification. The
/// check-and-remove is atomic under the store lock, so two concurrent terminates
/// never both see the call — exactly one gets `Some`, so the terminal event fires
/// exactly once.
pub fn remove(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("click-to-dial call store not poisoned")
        .remove(id)
}

/// The process-global **sink side-store**: `callId` → (`sink` URL, optional
/// callback `Authorization` header). Populated by `createCall` for a call created
/// with a `sink`, and drained by `terminateCall` to deliver the terminal
/// `status-changed` event. Kept apart from the `Call` map (above) so the callback
/// secret is never returned by `getCall`/`getRecording`.
fn sink_store() -> &'static Mutex<HashMap<String, (String, Option<String>)>> {
    static STORE: OnceLock<Mutex<HashMap<String, (String, Option<String>)>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Remember, under `id`, the `sink` URL and the derived callback `auth` header
/// (`Some("Bearer …")` for an ACCESSTOKEN `sinkCredential`, else `None`) so
/// `terminateCall` can deliver the terminal `status-changed` CloudEvent. Called by
/// `createCall` only when a call is newly created with a `sink`. An existing entry
/// under the same id is overwritten (a re-create of an evicted pair re-arms it).
pub fn insert_sink(id: String, sink: String, auth: Option<String>) {
    sink_store()
        .lock()
        .expect("click-to-dial sink store not poisoned")
        .insert(id, (sink, auth));
}

/// Take (remove) the sink entry for `id`, returning `(sink, auth)` if the call was
/// created with a `sink`, else `None`. Single-use: `terminateCall` calls this once
/// per terminated call, so the callback secret drops from memory as the call ends.
/// A call created without a `sink` has no entry → `None` (no terminal event).
pub fn take_sink(id: &str) -> Option<(String, Option<String>)> {
    sink_store()
        .lock()
        .expect("click-to-dial sink store not poisoned")
        .remove(id)
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
        // Removing something never stored → None (nothing to terminate).
        assert!(remove(id).is_none(), "unknown id → None");
        let call = json!({ "callId": id, "status": "initiating" });
        assert!(insert_new(id.to_string(), call.clone()));
        // First remove sees the call and evicts it, returning the removed value.
        assert_eq!(remove(id), Some(call), "present id → the removed Call");
        assert!(get(id).is_none(), "evicted → gone");
        // A second remove no longer sees it (single-use eviction).
        assert!(remove(id).is_none(), "already removed → None");
    }

    #[test]
    fn sink_side_store_is_single_use_and_absent_when_never_inserted() {
        let id = "store-unit-call-sink";
        // No sink recorded → None (a call created without a `sink`).
        assert!(take_sink(id).is_none(), "no entry → None");
        insert_sink(id.to_string(), "http://cb.test/notify".to_string(), Some("Bearer s".to_string()));
        // Taken once, returns the recorded (sink, auth) pair.
        assert_eq!(
            take_sink(id),
            Some(("http://cb.test/notify".to_string(), Some("Bearer s".to_string())))
        );
        // Single-use: a second take finds nothing.
        assert!(take_sink(id).is_none(), "already taken → None");
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
        assert!(remove(id).is_some());
        assert!(insert_new(id.to_string(), json!({ "callId": id })), "re-creatable after evict");
    }
}
