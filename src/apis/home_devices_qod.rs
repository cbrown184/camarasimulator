//! CAMARA **Home Devices QoD** API.
//!
//! Home Devices QoD lets a client raise (or restore) the Quality-of-Service
//! treatment a specific device on the subscriber's **home** network receives from
//! the home router — e.g. prioritise a games console for `real_time_interactive`
//! traffic. Unlike the network-side Quality on Demand API it is scoped to the LAN
//! between the router and a WiFi device, identified by its internal `ipAddress`.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_4`] — mounted at `/home-devices-qod/v0.4` (CAMARA Home Devices QoD
//!   0.4.0, the latest published version, so mounted at its real base path).

pub mod v0_4;

use axum::Router;

/// Every mounted major version of Home Devices QoD.
pub fn routes() -> Router {
    Router::new().merge(v0_4::routes())
}
