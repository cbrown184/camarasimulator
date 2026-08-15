//! In-memory Network Access Domains **Trust Domain store** (docs/DESIGN.md §4,
//! §5 — "in-memory stores", single node).
//!
//! Network Access Domains becomes **stateful** the moment a caller can *create* a
//! Trust Domain: `POST /trust-domains` (`createTrustDomain`) mints a
//! `trustDomainId` and remembers the rendered `TrustDomain` so the later read /
//! update / delete legs can address it. This module is that state. It mirrors
//! [`crate::apis::edge_application_management::store`]: a process-global
//! `HashMap` guarded by a `std::sync::Mutex`, the lock held only for the map
//! read/write and never across an `.await`, so it never blocks the async runtime.
//!
//! The stored value is the rendered `TrustDomain` JSON, keyed by its minted
//! `trustDomainId`. The id is derived deterministically from the Trust Domain's
//! identity — the `(serviceId, name)` pair
//! ([`crate::apis::network_access_domains::vwip::trust_domain_id`]) — so
//! re-creating a Trust Domain with the *same* name for the *same* service hits
//! the *same* id, which is exactly how [`insert`] detects the duplicate the
//! CAMARA `409` (duplicate name for service) case reports.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// The process-global Trust Domain store: `trustDomainId` → rendered
/// `TrustDomain` JSON. In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, Value>> {
    static STORE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `trust_domain` under `id`, unless one already exists under that id.
/// Because `id` is derived from the `(serviceId, name)` pair, an existing id
/// means a Trust Domain with the *same* name has already been created for the
/// *same* service.
///
/// Returns `true` when the Trust Domain was newly inserted, `false` when one
/// already existed — `createTrustDomain` maps the latter to the CAMARA `409`
/// (duplicate name for service). The whole check-and-insert runs under a single
/// lock hold (never across an `.await`), so two concurrent creates of the same
/// `(serviceId, name)` can't both win.
pub fn insert(id: String, trust_domain: Value) -> bool {
    let mut map = store()
        .lock()
        .expect("network-access-domains trust-domain store not poisoned");
    if map.contains_key(&id) {
        return false;
    }
    map.insert(id, trust_domain);
    true
}

/// Fetch the `TrustDomain` stored under `id`, or `None` if no such Trust Domain
/// exists. Backs the `getTrustDomain` read leg (`GET /trust-domains/{id}`) — a
/// hit renders `200`, a miss the CAMARA `404` — and the tests that assert a
/// created Trust Domain persists.
pub fn get(id: &str) -> Option<Value> {
    store()
        .lock()
        .expect("network-access-domains trust-domain store not poisoned")
        .get(id)
        .cloned()
}

/// Atomically update the `TrustDomain` stored under `id` by applying `apply` to a
/// mutable reference to it, returning the updated `TrustDomain` (a clone taken
/// after the mutation) or `None` if no Trust Domain exists under `id`. Backs the
/// `updateTrustDomain` leg (`PATCH /trust-domains/{id}`): a hit renders `200` with
/// the patched resource, a miss the CAMARA `404`. The whole get-modify-write runs
/// under a single lock hold (never across an `.await`), so a concurrent update /
/// delete of the same id can't observe a torn state.
pub fn update<F>(id: &str, apply: F) -> Option<Value>
where
    F: FnOnce(&mut Value),
{
    let mut map = store()
        .lock()
        .expect("network-access-domains trust-domain store not poisoned");
    match map.get_mut(id) {
        Some(td) => {
            apply(td);
            Some(td.clone())
        }
        None => None,
    }
}

/// Evict the `TrustDomain` stored under `id`, reporting whether one existed.
/// Backs the `deleteTrustDomain` leg (`DELETE /trust-domains/{id}`): a present
/// id is removed and the leg answers `204 No Content` (single-use — a second
/// delete of the same id finds nothing), a missing id yields the CAMARA `404`.
/// The whole check-and-remove runs under a single lock hold (never across an
/// `.await`), so two concurrent deletes of the same id can't both report success.
pub fn remove(id: &str) -> bool {
    store()
        .lock()
        .expect("network-access-domains trust-domain store not poisoned")
        .remove(id)
        .is_some()
}

/// The process-global **Trust Domain Device** store: the composite
/// `(trustDomainId, deviceId)` → rendered `TrustDomainDevice` JSON. A device
/// lives inside a Trust Domain, so it is keyed by the pair — the later read /
/// update / delete legs will address a device by its owning Trust Domain and its
/// own id, and two Trust Domains may hold devices with the same minted id shape
/// without colliding. In-memory only (single node, per DESIGN §4).
fn device_store() -> &'static Mutex<HashMap<(String, String), Value>> {
    static STORE: OnceLock<Mutex<HashMap<(String, String), Value>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Store `device` under `(trust_domain_id, device_id)`, unless one already exists
/// under that key. Because `device_id` is derived from the
/// `(trustDomainId, deviceName)` pair
/// ([`crate::apis::network_access_domains::vwip::trust_domain_device_id`]), an
/// existing key means a device with the *same* `deviceName` has already been
/// created in the *same* Trust Domain.
///
/// Returns `true` when the device was newly inserted, `false` when one already
/// existed — `createTrustDomainDevice` maps the latter to the CAMARA `409`
/// (duplicate device name in the Trust Domain). The whole check-and-insert runs
/// under a single lock hold (never across an `.await`), so two concurrent creates
/// of the same `(trustDomainId, deviceName)` can't both win.
pub fn insert_device(trust_domain_id: &str, device_id: &str, device: Value) -> bool {
    let key = (trust_domain_id.to_owned(), device_id.to_owned());
    let mut map = device_store()
        .lock()
        .expect("network-access-domains trust-domain-device store not poisoned");
    if map.contains_key(&key) {
        return false;
    }
    map.insert(key, device);
    true
}

/// Fetch the `TrustDomainDevice` stored under `(trust_domain_id, device_id)`, or
/// `None` if no such device exists. Backs the `getTrustDomainDevice` read leg
/// (`GET /trust-domains/{trustDomainId}/devices/{deviceId}`) — a hit renders `200`
/// with the persisted device, a miss (unknown Trust Domain *or* unknown device)
/// the CAMARA `404` — and the tests that assert a created device persists. Because
/// the key is the full `(trustDomainId, deviceId)` pair, a device is only found via
/// its owning Trust Domain: an unknown parent id yields the same `None` as an
/// unknown device id.
pub fn get_device(trust_domain_id: &str, device_id: &str) -> Option<Value> {
    let key = (trust_domain_id.to_owned(), device_id.to_owned());
    device_store()
        .lock()
        .expect("network-access-domains trust-domain-device store not poisoned")
        .get(&key)
        .cloned()
}
