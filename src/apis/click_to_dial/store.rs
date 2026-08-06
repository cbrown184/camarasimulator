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
//!   `getCall` — the created representation is the source of truth for this slice
//!   (lifecycle `status` transitions and `terminateCall` / `getRecording` arrive
//!   in later slices).
//! - The `callId` is already a deterministic, UUID-shaped token derived from the
//!   participant pair by `createCall`, so this store mints no ids of its own.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global call store: `callId` → rendered `Call`. In-memory only
/// (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `call` (its rendered `Call` JSON) under `id`. `createCall` calls this so
/// the call can later be read back by `getCall`. A repeat `createCall` for the
/// same participant pair mints the same deterministic `callId` and overwrites the
/// identical value — a no-op for observable behaviour (the `409 ALREADY_EXISTS`
/// duplicate-call case is a later slice).
pub fn insert(id: String, call: Value) {
    store()
        .lock()
        .expect("click-to-dial call store not poisoned")
        .insert(id, call);
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
        insert(id.to_string(), call.clone());
        assert_eq!(get(id), Some(call));
        assert!(get("store-unit-no-such-call").is_none());
    }

    #[test]
    fn insert_overwrites_the_same_id_with_an_identical_value() {
        // A repeat createCall for the same pair re-inserts the same value; the
        // store stays consistent and the read-back is unchanged.
        let id = "store-unit-call-b";
        let call = json!({ "callId": id, "status": "initiating" });
        insert(id.to_string(), call.clone());
        insert(id.to_string(), call.clone());
        assert_eq!(get(id), Some(call));
    }
}
