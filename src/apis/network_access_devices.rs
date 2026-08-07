//! CAMARA **Network Access Devices** API (NetworkAccessManagement, wip).
//!
//! Network Access Devices manages network-operator-supplied access equipment
//! (gateways, routers, access points) — operator-managed infrastructure, not
//! end-user devices. CamaraSim implements the stateless listing endpoint
//! `GET /network-access-devices`, which enumerates the equipment associated with
//! the subscriber the access token authenticated. The reboot-request resource
//! lifecycle is a stateful later slice (see `PROGRESS.md`).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/network-access-devices/vwip` (no released CAMARA
//!   version yet, so mounted at its canonical `vwip` base path).

pub mod vwip;

use axum::Router;

/// Every mounted version of Network Access Devices.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
