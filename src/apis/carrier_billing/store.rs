//! In-memory Carrier Billing **payment store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Carrier Billing became **stateful** the moment a caller can read a payment
//! back: `POST /payments` creates a payment and mints a `paymentId`, and
//! `GET /payments/{paymentId}` (`retrievePayment`) addresses that payment by its
//! id. This module is the state that bridges those requests. It mirrors
//! [`crate::apis::quality_on_demand::store`]: a process-global `HashMap` guarded
//! by a `std::sync::Mutex`, the lock held only for the map read/write and never
//! across an `.await`, so it never blocks the async runtime.
//!
//! The stored value is the payment's rendered `PaymentCreated`/`Payment` JSON —
//! the created representation is the source of truth (`retrievePayment` returns
//! it verbatim). Ids come from `carrier_billing::v0_5::mint_uuid`, so this module
//! carries no id minting of its own.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global payment store: `paymentId` → rendered payment JSON.
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `payment` (its rendered JSON) under `id`. Called by `createPayment`
/// once the one-step charge has succeeded, so the payment can be read back.
pub fn insert(id: String, payment: Value) {
    store()
        .lock()
        .expect("carrier-billing payment store not poisoned")
        .insert(id, payment);
}

/// Fetch the payment stored under `id`, or `None` if no such payment exists
/// (never created, or created in a different process). `retrievePayment` uses
/// the distinction to answer `200` (found) vs `404 NOT_FOUND` (unknown id).
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("carrier-billing payment store not poisoned")
        .get(id)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stored_payment_can_be_read_back_and_unknown_is_none() {
        // Uniquely-keyed so this shares the process-global store with nothing else.
        let id = "cb-store-unit-0001".to_string();
        assert!(get(&id).is_none(), "not stored yet → None");
        let payment = json!({ "paymentId": id, "paymentStatus": "succeeded" });
        insert(id.clone(), payment.clone());
        assert_eq!(get(&id), Some(payment));
        assert!(get("cb-store-unit-no-such").is_none());
    }
}
