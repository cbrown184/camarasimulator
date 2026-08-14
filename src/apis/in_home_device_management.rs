//! CAMARA **In-Home Device Management** API.
//!
//! In-Home Device Management lets a consumer (or an app acting for the
//! household) discover and manage the devices attached to a home fixed-line
//! network — the laptops, phones, TVs and IoT gadgets behind the router — plus
//! the infrastructure devices (modem, Wi-Fi boosters, mesh pods) that carry
//! them. The household network is named by its `ssid`.
//!
//! CamaraSim implements the inventory read legs (`GET /devices` `listDevices`,
//! `GET /devices/{deviceId}` `getDevice`, `GET /devices/{deviceId}/network-health`
//! `getDeviceNetworkHealth`) and the action leg
//! (`POST /devices/{deviceId}/actions/{actionId}` `performDeviceAction`): there is
//! no real home network to query, so the roster is derived deterministically from
//! the `ssid` (docs/DESIGN.md §7) — the input is the control plane.
//!
//! `DELETE /devices/{deviceId}` (`deleteDevice`) is the API's first **mutation**:
//! it tombstones a device in the in-memory [`store`], and the read legs honour
//! those tombstones, so a deleted device stops appearing. The remaining mutation
//! leg (`updateDevice`) is a later slice.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/in-home-device-management/v1` (CAMARA
//!   InHomeDeviceManagement 1.0.0, sandbox).

pub mod store;
pub mod v1;

use axum::Router;

/// Every mounted major version of In-Home Device Management.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
