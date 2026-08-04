//! In-memory Blockchain Public Address **binding store** (docs/DESIGN.md §4, §5 —
//! "in-memory stores", single node).
//!
//! Blockchain Public Address becomes **stateful** the moment a caller can *bind*
//! an on-chain address to their line: `POST /blockchain-public-addresses`
//! (`bindBlockchainPublicAddress`) mints a binding `id` and remembers the
//! relationship so a later `DELETE /blockchain-public-addresses/{id}`
//! (`deleteBlockchainPublicAddress`, a later slice) can address it. This module
//! is that state. It mirrors [`crate::apis::carrier_billing::store`]: a
//! process-global `HashMap` guarded by a `std::sync::Mutex`, the lock held only
//! for the map read/write and never across an `.await`, so it never blocks the
//! async runtime.
//!
//! The stored value is the binding's rendered `BlockchainPublicAddressResponse`
//! JSON (`id` / `blockchainPublicAddress` / `blockchainNetworkId` / `currency`),
//! keyed by its `id`. The `id` is minted deterministically from the
//! `(phoneNumber, blockchainNetworkId, blockchainPublicAddress)` triple
//! ([`crate::apis::blockchain_public_address::v0_3::binding_id`]), so re-binding
//! the *same* address to the same line hits the *same* id — which is exactly how
//! [`insert`] detects a duplicate for the CAMARA `409 ALREADY_EXISTS` case.
//!
//! The stateless `retrieveBlockchainPublicAddress` read op does **not** consult
//! this store — it still answers from the deterministic synthetic model
//! (docs/DESIGN.md §7). Reconciling the two (having `retrieve` surface bound
//! records) is a deliberately deferred, separate concern.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global binding store: binding `id` → rendered
/// `BlockchainPublicAddressResponse` JSON. In-memory only (single node, per
/// DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `binding` (its rendered JSON) under `id`, unless a binding already
/// exists under that id. Because `id` is derived from the
/// `(phoneNumber, network, address)` triple, an existing id means the *same*
/// address is already bound to the *same* line.
///
/// Returns `true` when the binding was newly inserted, `false` when one already
/// existed — `bindBlockchainPublicAddress` maps the latter to `409
/// ALREADY_EXISTS`. The whole check-and-insert runs under a single lock hold
/// (never across an `.await`), so two concurrent binds of the same triple can't
/// both win.
pub fn insert(id: String, binding: Value) -> bool {
    let mut map = store()
        .lock()
        .expect("blockchain-public-address binding store not poisoned");
    if map.contains_key(&id) {
        return false;
    }
    map.insert(id, binding);
    true
}

/// Fetch the binding stored under `id`, or `None` if no such binding exists.
/// Used by the tests to assert persistence; `deleteBlockchainPublicAddress` (a
/// later slice) will use the same distinction to answer `204` vs `404
/// NOT_FOUND`, so it is retained now even though the request path does not yet
/// call it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("blockchain-public-address binding store not poisoned")
        .get(id)
        .cloned()
}
