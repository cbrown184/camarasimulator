//! CAMARA **Network Access Devices** API (NetworkAccessManagement, wip).
//!
//! Network Access Devices manages network-operator-supplied access equipment
//! (gateways, routers, access points) — operator-managed infrastructure, not
//! end-user devices. CamaraSim implements the stateless listing endpoints
//! (`GET /network-access-devices`, `GET /network-access-devices/{id}`) and has
//! begun the **stateful reboot-request resource lifecycle**: `POST
//! /reboot-requests` creates a reboot request for the subscriber's devices and
//! persists it in an in-memory store ([`store`]). The read/patch/delete legs are
//! later slices.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/network-access-devices/vwip` (no released CAMARA
//!   version yet, so mounted at its canonical `vwip` base path).
//! - [`store`] — the in-memory reboot-request store shared across versions.

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted version of Network Access Devices.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
