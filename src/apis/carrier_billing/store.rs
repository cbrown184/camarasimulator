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

/// Snapshot every stored payment (its rendered JSON), in unspecified order.
/// `retrievePayments` (GET /payments) uses this to render the `PaymentArray`
/// list. The lock is held only for the clone of the values, never across an
/// `.await`, so it never blocks the async runtime.
pub fn all() -> Vec<Value> {
    store()
        .lock()
        .expect("carrier-billing payment store not poisoned")
        .values()
        .cloned()
        .collect()
}

/// The outcome of a [`confirm`] attempt on a payment.
pub enum ConfirmOutcome {
    /// The reservation was `reserved` and is now `succeeded` (charged).
    Confirmed,
    /// The payment was already `succeeded` (charged) — `PAYMENT_CONFIRMED`.
    AlreadyConfirmed,
    /// The payment was already `cancelled` — `PAYMENT_CANCELLED`.
    AlreadyCancelled,
    /// The payment exists but is not in a confirmable (`reserved`) state — e.g.
    /// `pending_validation` (its OTP has not been cleared) or `denied`.
    NotConfirmable,
    /// No such payment.
    Unknown,
}

/// Confirm (charge) the reserved payment `id`, moving it `reserved → succeeded`
/// and stamping `paymentDate` with `payment_date`. Driven atomically under the
/// store lock (never held across an `.await`):
///
/// - `reserved` → `succeeded` (with `paymentDate`) → [`Confirmed`];
/// - already `succeeded` → [`AlreadyConfirmed`] (409 `PAYMENT_CONFIRMED`);
/// - already `cancelled` → [`AlreadyCancelled`] (409 `PAYMENT_CANCELLED`);
/// - any other existing state (`pending_validation`, `denied`, …) →
///   [`NotConfirmable`] (409 `CONFLICT`);
/// - no such payment → [`Unknown`] (404).
///
/// [`Confirmed`]: ConfirmOutcome::Confirmed
/// [`AlreadyConfirmed`]: ConfirmOutcome::AlreadyConfirmed
/// [`AlreadyCancelled`]: ConfirmOutcome::AlreadyCancelled
/// [`NotConfirmable`]: ConfirmOutcome::NotConfirmable
/// [`Unknown`]: ConfirmOutcome::Unknown
pub fn confirm(id: &str, payment_date: &str) -> ConfirmOutcome {
    let mut map = store()
        .lock()
        .expect("carrier-billing payment store not poisoned");
    match map.get_mut(id) {
        None => ConfirmOutcome::Unknown,
        Some(payment) => match payment["paymentStatus"].as_str() {
            Some("reserved") => {
                payment["paymentStatus"] = Value::String("succeeded".to_string());
                payment["paymentDate"] = Value::String(payment_date.to_string());
                ConfirmOutcome::Confirmed
            }
            Some("succeeded") => ConfirmOutcome::AlreadyConfirmed,
            Some("cancelled") => ConfirmOutcome::AlreadyCancelled,
            _ => ConfirmOutcome::NotConfirmable,
        },
    }
}

/// The outcome of a [`cancel`] attempt on a payment.
pub enum CancelOutcome {
    /// The reservation was `reserved` and is now `cancelled` (released).
    Cancelled,
    /// The payment was already `cancelled` — `PAYMENT_CANCELLED`.
    AlreadyCancelled,
    /// The payment was already `succeeded` (charged) — `PAYMENT_CONFIRMED`. A
    /// charged payment can no longer be cancelled.
    AlreadyConfirmed,
    /// The payment exists but is not in a cancellable (`reserved`) state — e.g.
    /// `pending_validation` (its OTP has not been cleared) or `denied`.
    NotCancellable,
    /// No such payment.
    Unknown,
}

/// Cancel (release) the reserved payment `id`, moving it `reserved → cancelled`.
/// Nothing is charged, so — unlike [`confirm`] — no `paymentDate` is stamped.
/// Driven atomically under the store lock (never held across an `.await`):
///
/// - `reserved` → `cancelled` → [`Cancelled`];
/// - already `cancelled` → [`AlreadyCancelled`] (409 `PAYMENT_CANCELLED`);
/// - already `succeeded` → [`AlreadyConfirmed`] (409 `PAYMENT_CONFIRMED`);
/// - any other existing state (`pending_validation`, `denied`, …) →
///   [`NotCancellable`] (409 `CONFLICT`);
/// - no such payment → [`Unknown`] (404).
///
/// [`Cancelled`]: CancelOutcome::Cancelled
/// [`AlreadyCancelled`]: CancelOutcome::AlreadyCancelled
/// [`AlreadyConfirmed`]: CancelOutcome::AlreadyConfirmed
/// [`NotCancellable`]: CancelOutcome::NotCancellable
/// [`Unknown`]: CancelOutcome::Unknown
pub fn cancel(id: &str) -> CancelOutcome {
    let mut map = store()
        .lock()
        .expect("carrier-billing payment store not poisoned");
    match map.get_mut(id) {
        None => CancelOutcome::Unknown,
        Some(payment) => match payment["paymentStatus"].as_str() {
            Some("reserved") => {
                payment["paymentStatus"] = Value::String("cancelled".to_string());
                CancelOutcome::Cancelled
            }
            Some("cancelled") => CancelOutcome::AlreadyCancelled,
            Some("succeeded") => CancelOutcome::AlreadyConfirmed,
            _ => CancelOutcome::NotCancellable,
        },
    }
}

// --- Pending-validation side-store (two-step OTP flow) --------------------
//
// A reservation created by `preparePayment` for a phone number that requires
// OTP validation lands in `pending_validation` and carries an `authorizationId`
// (echoed in its `validationInfo`). The expected OTP `code` is a **secret** and
// so is held here, apart from the echoed payment JSON — the map is keyed by
// `paymentId` and never rendered to a client. `validatePayment` consumes it.

/// A pending OTP validation for a two-step reservation awaiting `validatePayment`.
pub struct PendingValidation {
    /// The `authorizationId` echoed to the caller in `validationInfo`. A
    /// `validatePayment` must present it (else `INVALID_AUTHORIZATION_ID`).
    pub authorization_id: String,
    /// The expected OTP `code` (secret; never echoed). Deterministic from the
    /// phone number so a headless caller can compute it.
    pub code: String,
    /// Remaining validation attempts before the reservation is denied.
    pub attempts_left: u32,
}

/// The outcome of a [`validate_pending`] attempt.
pub enum ValidateOutcome {
    /// Correct `authorizationId` + `code`: the reservation moved to `reserved`.
    Validated,
    /// The presented `authorizationId` does not match the pending validation.
    InvalidAuthId,
    /// Correct `authorizationId` but wrong `code`; an attempt was consumed.
    InvalidCode,
    /// The final attempt was consumed with a wrong `code`: the reservation was
    /// denied (moved to `denied`).
    ValidationFailed,
    /// The payment exists but is not awaiting validation (already settled).
    NotPending,
    /// No such payment.
    Unknown,
}

/// The process-global pending-validation side-store: `paymentId` →
/// [`PendingValidation`]. Kept apart from the payment JSON map so the OTP `code`
/// is never rendered to a client. In-memory only (single node, per DESIGN §4).
fn pending() -> &'static Mutex<HashMap<String, PendingValidation>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingValidation>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record that the reservation `id` is awaiting OTP validation. Called by
/// `preparePayment` when the resolved phone number requires validation.
pub fn insert_pending(id: String, validation: PendingValidation) {
    pending()
        .lock()
        .expect("carrier-billing pending-validation store not poisoned")
        .insert(id, validation);
}

/// Attempt to validate the reservation `id` with the presented `authorization_id`
/// and `code`, driving its `paymentStatus` transition atomically:
///
/// - correct id + code → the reservation becomes `reserved` → [`Validated`];
/// - wrong `authorization_id` → [`InvalidAuthId`] (no attempt consumed);
/// - correct id, wrong `code` → an attempt is consumed → [`InvalidCode`], or —
///   when it was the last attempt — the reservation becomes `denied` →
///   [`ValidationFailed`];
/// - the payment exists but has no pending validation → [`NotPending`];
/// - no such payment → [`Unknown`].
///
/// Both maps are locked in a fixed order (pending → payments) so nothing
/// deadlocks against the single-lock accessors above; neither lock is held
/// across an `.await`.
///
/// [`Validated`]: ValidateOutcome::Validated
/// [`InvalidAuthId`]: ValidateOutcome::InvalidAuthId
/// [`InvalidCode`]: ValidateOutcome::InvalidCode
/// [`ValidationFailed`]: ValidateOutcome::ValidationFailed
/// [`NotPending`]: ValidateOutcome::NotPending
/// [`Unknown`]: ValidateOutcome::Unknown
pub fn validate_pending(id: &str, authorization_id: &str, code: &str) -> ValidateOutcome {
    let mut pend = pending()
        .lock()
        .expect("carrier-billing pending-validation store not poisoned");
    match pend.get_mut(id) {
        None => {
            // No pending validation: distinguish an unknown id from a payment
            // that exists but is already settled (reserved/succeeded/denied/…).
            if store()
                .lock()
                .expect("carrier-billing payment store not poisoned")
                .contains_key(id)
            {
                ValidateOutcome::NotPending
            } else {
                ValidateOutcome::Unknown
            }
        }
        Some(validation) => {
            if validation.authorization_id != authorization_id {
                return ValidateOutcome::InvalidAuthId;
            }
            if validation.code == code {
                pend.remove(id);
                settle(id, "reserved");
                ValidateOutcome::Validated
            } else {
                validation.attempts_left = validation.attempts_left.saturating_sub(1);
                if validation.attempts_left == 0 {
                    pend.remove(id);
                    settle(id, "denied");
                    ValidateOutcome::ValidationFailed
                } else {
                    ValidateOutcome::InvalidCode
                }
            }
        }
    }
}

/// Settle a pending reservation into a terminal-for-validation `status`
/// (`reserved` or `denied`): set its `paymentStatus` and drop the now-stale
/// `validationInfo`. A no-op if the payment has since been evicted.
fn settle(id: &str, status: &str) {
    if let Some(payment) = store()
        .lock()
        .expect("carrier-billing payment store not poisoned")
        .get_mut(id)
    {
        payment["paymentStatus"] = Value::String(status.to_string());
        if let Some(obj) = payment.as_object_mut() {
            obj.remove("validationInfo");
        }
    }
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

    #[test]
    fn all_includes_every_inserted_payment() {
        // Uniquely-keyed ids so the assertion is robust against whatever else
        // the process-global store holds (the test binary shares one store).
        let id_a = "cb-store-all-unit-000a".to_string();
        let id_b = "cb-store-all-unit-000b".to_string();
        let pay_a = json!({ "paymentId": id_a, "paymentStatus": "succeeded" });
        let pay_b = json!({ "paymentId": id_b, "paymentStatus": "succeeded" });
        insert(id_a.clone(), pay_a.clone());
        insert(id_b.clone(), pay_b.clone());

        let listed = all();
        assert!(listed.contains(&pay_a), "all() includes the first payment");
        assert!(listed.contains(&pay_b), "all() includes the second payment");
    }

    #[test]
    fn confirm_drives_every_state_branch() {
        // A reserved payment → succeeded (with paymentDate), then a second
        // confirm sees `succeeded` → AlreadyConfirmed.
        let id = "cb-store-confirm-unit-res".to_string();
        insert(
            id.clone(),
            json!({ "paymentId": id, "paymentStatus": "reserved" }),
        );
        assert!(matches!(
            confirm(&id, "2024-01-01T00:00:00Z"),
            ConfirmOutcome::Confirmed
        ));
        let charged = get(&id).unwrap();
        assert_eq!(charged["paymentStatus"], "succeeded");
        assert_eq!(charged["paymentDate"], "2024-01-01T00:00:00Z");
        assert!(matches!(
            confirm(&id, "2024-01-01T00:00:00Z"),
            ConfirmOutcome::AlreadyConfirmed
        ));

        // A cancelled payment → AlreadyCancelled.
        let cid = "cb-store-confirm-unit-can".to_string();
        insert(
            cid.clone(),
            json!({ "paymentId": cid, "paymentStatus": "cancelled" }),
        );
        assert!(matches!(
            confirm(&cid, "2024-01-01T00:00:00Z"),
            ConfirmOutcome::AlreadyCancelled
        ));

        // A denied (or pending_validation) payment is not confirmable.
        let did = "cb-store-confirm-unit-den".to_string();
        insert(
            did.clone(),
            json!({ "paymentId": did, "paymentStatus": "denied" }),
        );
        assert!(matches!(
            confirm(&did, "2024-01-01T00:00:00Z"),
            ConfirmOutcome::NotConfirmable
        ));

        // An unknown id → Unknown.
        assert!(matches!(
            confirm("cb-store-confirm-unit-no-such", "2024-01-01T00:00:00Z"),
            ConfirmOutcome::Unknown
        ));
    }

    #[test]
    fn cancel_drives_every_state_branch() {
        // A reserved payment → cancelled (no paymentDate), then a second cancel
        // sees `cancelled` → AlreadyCancelled.
        let id = "cb-store-cancel-unit-res".to_string();
        insert(
            id.clone(),
            json!({ "paymentId": id, "paymentStatus": "reserved" }),
        );
        assert!(matches!(cancel(&id), CancelOutcome::Cancelled));
        let released = get(&id).unwrap();
        assert_eq!(released["paymentStatus"], "cancelled");
        assert!(
            released.get("paymentDate").is_none(),
            "a cancelled payment is never charged, so gains no paymentDate"
        );
        assert!(matches!(cancel(&id), CancelOutcome::AlreadyCancelled));

        // A succeeded (charged) payment can no longer be cancelled.
        let sid = "cb-store-cancel-unit-suc".to_string();
        insert(
            sid.clone(),
            json!({ "paymentId": sid, "paymentStatus": "succeeded" }),
        );
        assert!(matches!(cancel(&sid), CancelOutcome::AlreadyConfirmed));

        // A denied (or pending_validation) payment is not cancellable.
        let did = "cb-store-cancel-unit-den".to_string();
        insert(
            did.clone(),
            json!({ "paymentId": did, "paymentStatus": "denied" }),
        );
        assert!(matches!(cancel(&did), CancelOutcome::NotCancellable));

        // An unknown id → Unknown.
        assert!(matches!(
            cancel("cb-store-cancel-unit-no-such"),
            CancelOutcome::Unknown
        ));
    }
}
