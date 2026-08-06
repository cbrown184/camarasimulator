//! In-memory **IMEI-binding store** for IoT SIM Fraud Prevention vwip
//! (docs/DESIGN.md §5 — "in-memory stores").
//!
//! IoT SIM Fraud Prevention's `IMEIBIND` flow is **stateful**: `POST /bind`
//! associates a device's SIM with its IMEI, `POST /query` reports whether a
//! binding is in force, and `POST /unbind` removes it. This module is the state
//! that bridges those three operations — a device identifier (the resolved
//! `phoneNumber`/NAI/IP or three-legged subject) maps to the IMEI bound to its
//! SIM. It mirrors the other stores under `src/apis/**/store.rs`.
//!
//! ## Simulator constraints
//!
//! - **In-memory, single node** (docs/DESIGN.md §4): a process-global map guarded
//!   by a `std::sync::Mutex`. The lock is held only for `HashMap` reads/writes —
//!   never across an `.await` — so it never blocks the async runtime.
//! - The key is the resolved device identifier; the value is the 15-digit IMEI
//!   the SIM is bound to (deterministic from the identifier, per
//!   [`super::vwip`]). Storing the IMEI (rather than a bare flag) lets `query`
//!   report the same `bindImei` the network would have observed at bind time.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The process-global IMEI-binding store: device identifier → bound IMEI.
/// In-memory only (single node, per DESIGN §4).
fn store() -> &'static Mutex<HashMap<String, String>> {
    static STORE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Bind `identifier`'s SIM to `imei`, replacing any existing binding. Binding is
/// idempotent — re-binding the same device simply re-records the (identical,
/// deterministic) IMEI — so `bindDeviceImei` always answers `{ "bound": true }`.
pub fn bind(identifier: String, imei: String) {
    store()
        .lock()
        .expect("iot-sim bind store not poisoned")
        .insert(identifier, imei);
}

/// The IMEI currently bound to `identifier`, or `None` if the device has no
/// explicit binding. `query` uses the distinction to report a stored `BOUND`
/// binding ahead of its deterministic default.
pub fn bound_imei(identifier: &str) -> Option<String> {
    store()
        .lock()
        .expect("iot-sim bind store not poisoned")
        .get(identifier)
        .cloned()
}

/// Remove `identifier`'s binding, returning the previously bound IMEI, or `None`
/// if the device had no binding. `unBindDeviceImei` uses the distinction to
/// answer `200 { "unbound": true }` (a binding was removed) vs `422
/// UNNECESSARY_UNBIND_IMEI` (nothing was bound).
pub fn unbind(identifier: &str) -> Option<String> {
    store()
        .lock()
        .expect("iot-sim bind store not poisoned")
        .remove(identifier)
}
