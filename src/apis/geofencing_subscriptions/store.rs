//! In-memory **subscription store** for Geofencing Subscriptions
//! (docs/DESIGN.md §5 — "in-memory stores").
//!
//! Geofencing Subscriptions is an **event-subscription** API: `POST
//! /subscriptions` creates a geofencing subscription and mints a subscription
//! `id`, and later requests (`GET /subscriptions/{id}`, and — in later passes —
//! `DELETE`) address that resource by its id. This module is the state that
//! bridges those requests.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it does not block the async runtime (mirrors
//!   [`crate::apis::quality_on_demand::store`]).
//! - **Opaque ids**: [`new_subscription_id`] mints a UUID-shaped `id` (CAMARA
//!   `SubscriptionInfo.id` is `format: uuid`) from a monotonic counter and the
//!   clock, so ids are unique without a `uuid`/`rand` dependency.
//! - The stored value is the subscription's rendered `SubscriptionInfo` JSON,
//!   returned verbatim by `GET` — the created representation is the source of
//!   truth for this slice (event delivery / expiry arrive in later passes).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// The process-global subscription store: subscription `id` → rendered
/// `SubscriptionInfo`. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The process-global **event budget** side-store: subscription `id` → remaining
/// number of domain events (`area-entered` / `area-left`) still allowed before the
/// subscription ends. Populated only for a subscription created with a
/// `config.subscriptionMaxEvents`; a subscription with no entry is unbounded. Kept
/// apart from the `SubscriptionInfo` map (like QoD's credential side-store) so the
/// running count is never echoed by `GET` / `retrieveSubscriptionList`. In-memory
/// only (single node, per DESIGN §4).
fn budgets() -> &'static Mutex<HashMap<String, u64>> {
    static BUDGETS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    BUDGETS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `subscription` (its rendered `SubscriptionInfo` JSON) under `id`.
pub fn insert(id: String, subscription: Value) {
    store()
        .lock()
        .expect("geofencing subscription store not poisoned")
        .insert(id, subscription);
}

/// Fetch the `SubscriptionInfo` stored under `id`, or `None` if no such
/// subscription exists (never created, or created in a different process).
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("geofencing subscription store not poisoned")
        .get(id)
        .cloned()
}

/// Remove the subscription stored under `id`, returning its `SubscriptionInfo`
/// if one was present, or `None` if no such subscription existed.
/// `deleteSubscription` uses the distinction to answer `204` (a subscription was
/// deleted) vs `404` (unknown id). Any `subscriptionMaxEvents` budget for the id is
/// dropped in the same call (the two mutexes are locked sequentially, never across
/// an `.await`), so a subscription that a delete/expiry ended leaves no stale
/// budget behind.
pub fn remove(id: &str) -> Option<Value> {
    let removed = store()
        .lock()
        .expect("geofencing subscription store not poisoned")
        .remove(id);
    budgets()
        .lock()
        .expect("geofencing subscription budget store not poisoned")
        .remove(id);
    removed
}

/// Register a `subscriptionMaxEvents` budget for a subscription: the maximum number
/// of domain events (`area-entered` / `area-left`) to deliver before the
/// subscription ends. Called once at creation for a subscription that set
/// `config.subscriptionMaxEvents` (which is validated `>= 1`).
pub fn set_event_budget(id: String, max_events: u64) {
    budgets()
        .lock()
        .expect("geofencing subscription budget store not poisoned")
        .insert(id, max_events);
}

/// The outcome of consuming one domain event from a subscription's
/// `subscriptionMaxEvents` budget (see [`consume_event`]).
#[derive(Debug, PartialEq, Eq)]
pub enum EventBudget {
    /// No `subscriptionMaxEvents` was set: deliver the event; the subscription is
    /// never ended by the count.
    Unbounded,
    /// The event was consumed and budget still remains: deliver it, do not end.
    Allowed,
    /// The event consumed the final unit of budget: deliver it, then end the
    /// subscription (`terminationReason: MAX_EVENTS_REACHED`).
    Last,
    /// The budget was already spent: suppress the event (defensive — the
    /// subscription is normally evicted the moment the budget reaches zero).
    Exhausted,
}

/// Atomically consume one domain event from the subscription's
/// `subscriptionMaxEvents` budget and report the outcome ([`EventBudget`]).
///
/// A subscription with no budget entry is [`EventBudget::Unbounded`]. Otherwise the
/// remaining count is decremented: it becomes [`EventBudget::Allowed`] while budget
/// remains, [`EventBudget::Last`] on the event that exhausts it (the entry is then
/// dropped), or [`EventBudget::Exhausted`] if it was already zero. The lock is held
/// only for the map access — never across an `.await`.
pub fn consume_event(id: &str) -> EventBudget {
    let mut budgets = budgets()
        .lock()
        .expect("geofencing subscription budget store not poisoned");
    let Some(remaining) = budgets.get(id).copied() else {
        return EventBudget::Unbounded;
    };
    if remaining == 0 {
        budgets.remove(id);
        return EventBudget::Exhausted;
    }
    let left = remaining - 1;
    if left == 0 {
        budgets.remove(id);
        EventBudget::Last
    } else {
        budgets.insert(id.to_string(), left);
        EventBudget::Allowed
    }
}

/// Return a snapshot of every stored `SubscriptionInfo`.
/// `retrieveSubscriptionList` uses this to list subscriptions. The lock is held
/// only for the clone (never across an `.await`), and the returned `Vec` is an
/// independent copy. CamaraSim does not scope subscriptions per client, so this
/// returns every subscription in the store — a documented simplification.
pub fn all() -> Vec<Value> {
    store()
        .lock()
        .expect("geofencing subscription store not poisoned")
        .values()
        .cloned()
        .collect()
}

/// Mint a fresh, opaque, UUID-shaped subscription `id`. See [`mint_uuid`].
pub fn new_subscription_id() -> String {
    mint_uuid()
}

/// Mint a fresh, opaque, UUID-shaped CloudEvent `id` (CloudEvents requires `id`
/// to be unique within its `source`). Shares [`mint_uuid`]'s monotonic counter
/// with subscription ids, so the two never collide.
pub fn new_event_id() -> String {
    mint_uuid()
}

/// Mint a fresh, opaque, UUID-v4-shaped identifier.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID, matching CAMARA's `format: uuid`.
/// No `uuid`/`rand` dependency (mirrors [`crate::apis::quality_on_demand::store`]).
fn mint_uuid() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(unix_now().to_be_bytes());
    let d = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Current Unix time in seconds (server runtime clock; not on any hot loop).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn subscription_ids_are_unique_and_uuid_v4_shaped() {
        let a = new_subscription_id();
        let b = new_subscription_id();
        assert_ne!(a, b, "each subscription id must be unique");
        // 8-4-4-4-12 lowercase hex.
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'));
        // Version nibble is 4; variant nibble is one of 8/9/a/b.
        assert_eq!(parts[2].as_bytes()[0], b'4', "version 4");
        assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn event_ids_are_unique_and_never_collide_with_subscription_ids() {
        let e1 = new_event_id();
        let e2 = new_event_id();
        assert_ne!(e1, e2, "each event id must be unique");
        // Shares the counter with subscription ids, so the two never collide.
        assert_ne!(e1, new_subscription_id());
    }

    #[test]
    fn stored_subscription_can_be_read_back_and_unknown_is_none() {
        let id = new_subscription_id();
        assert!(get(&id).is_none(), "not stored yet");
        let info = json!({ "id": id, "status": "ACTIVE" });
        insert(id.clone(), info.clone());
        assert_eq!(get(&id), Some(info));
        assert!(get("no-such-subscription").is_none());
    }

    #[test]
    fn remove_returns_the_subscription_once_then_none() {
        let id = new_subscription_id();
        insert(id.clone(), json!({ "id": id, "status": "ACTIVE" }));
        assert!(remove(&id).is_some(), "first remove evicts and returns it");
        assert!(remove(&id).is_none(), "second remove finds nothing");
        assert!(get(&id).is_none(), "and it is gone from the store");
    }

    #[test]
    fn event_budget_counts_down_and_ends_on_the_last_event() {
        let id = new_subscription_id();
        // No budget registered → unbounded (never ends by count).
        assert_eq!(consume_event(&id), EventBudget::Unbounded);

        // A budget of 2: first event allowed, second is the last, then exhausted.
        set_event_budget(id.clone(), 2);
        assert_eq!(consume_event(&id), EventBudget::Allowed);
        assert_eq!(consume_event(&id), EventBudget::Last);
        // The entry was dropped on Last, so the id is unbounded again.
        assert_eq!(consume_event(&id), EventBudget::Unbounded);
    }

    #[test]
    fn a_budget_of_one_ends_on_the_first_event() {
        let id = new_subscription_id();
        set_event_budget(id.clone(), 1);
        assert_eq!(consume_event(&id), EventBudget::Last);
    }

    #[test]
    fn remove_drops_the_event_budget_too() {
        let id = new_subscription_id();
        insert(id.clone(), json!({ "id": id, "status": "ACTIVE" }));
        set_event_budget(id.clone(), 5);
        assert!(remove(&id).is_some());
        // The budget is gone with the subscription: the id reads as unbounded.
        assert_eq!(consume_event(&id), EventBudget::Unbounded);
    }

    #[test]
    fn all_includes_a_stored_subscription() {
        let id = new_subscription_id();
        insert(id.clone(), json!({ "id": id, "status": "ACTIVE" }));
        assert!(
            all().iter().any(|s| s.get("id").and_then(Value::as_str) == Some(id.as_str())),
            "the stored subscription appears in the snapshot"
        );
    }
}
