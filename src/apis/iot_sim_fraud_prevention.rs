//! CAMARA **IoT SIM Fraud Prevention** API.
//!
//! IoT SIM Fraud Prevention lets an operator bind an IoT SIM card to a specific
//! device IMEI (or restrict it to a geographic area) so that a stolen or cloned
//! SIM is blocked when it appears on a different device. The upstream CAMARA API
//! exposes `POST /bind`, `POST /unbind`, and `POST /query`.
//!
//! CamaraSim mounts the full round-trip for **both** CAMARA facets over a shared
//! in-memory [`store`]. The non-spatial **`IMEIBIND`** facet — `POST /bind`,
//! `POST /unbind`, `POST /query` — records a device's SIM↔IMEI association,
//! reports it, and removes it. The spatial **`AREALIMIT`** facet — the same three
//! operations with `bindType`/`unBindType`/`queryType: AREALIMIT` — marks a SIM
//! restricted to its network-provisioned area, reports the restriction (with a
//! deterministic `Circle`), and clears it. The upstream bind carries no geometry,
//! so the allowed area is synthesised from the identifier at query time.
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
