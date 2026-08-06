//! CAMARA **IoT SIM Fraud Prevention** API.
//!
//! IoT SIM Fraud Prevention lets an operator bind an IoT SIM card to a specific
//! device IMEI (or restrict it to a geographic area) so that a stolen or cloned
//! SIM is blocked when it appears on a different device. The upstream CAMARA API
//! exposes `POST /bind`, `POST /unbind`, and `POST /query`.
//!
//! CamaraSim mounts the full **`IMEIBIND`** round-trip — `POST /bind`,
//! `POST /unbind`, and `POST /query` — over a shared in-memory binding [`store`]:
//! a bind records the device's SIM↔IMEI association, `query` reports it, and an
//! unbind removes it. Only the `AREALIMIT` query/bind type is deferred (it is
//! spatial — a geographic `Circle` restriction; see `PROGRESS.md`), so every
//! `*Type` enum in the vendored spec is trimmed to `[IMEIBIND]`.
//!
//! One submodule per published version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/iot-sim-fraud-prevention/vwip` (CAMARA `wip`).

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted version of IoT SIM Fraud Prevention.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
