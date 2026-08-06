//! CAMARA **IoT SIM Fraud Prevention** API.
//!
//! IoT SIM Fraud Prevention lets an operator bind an IoT SIM card to a specific
//! device IMEI (or restrict it to a geographic area) so that a stolen or cloned
//! SIM is blocked when it appears on a different device. The upstream CAMARA API
//! exposes `POST /bind`, `POST /unbind`, and `POST /query`.
//!
//! CamaraSim currently mounts **`POST /query` for `queryType: IMEIBIND` only** — a
//! stateless, non-spatial, device-identifier-keyed query. The bind/unbind
//! operations are stateful mutations and the `AREALIMIT` query type is spatial;
//! both are deferred (see `PROGRESS.md`).
//!
//! One submodule per published version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/iot-sim-fraud-prevention/vwip` (CAMARA `wip`).

pub mod vwip;

use axum::Router;

/// Every mounted version of IoT SIM Fraud Prevention.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
