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
//! The two **mutation** legs are also implemented against the in-memory [`store`]:
//! `DELETE /devices/{deviceId}` (`deleteDevice`) tombstones a device, and
//! `PATCH /devices/{deviceId}` (`updateDevice`) records an overlay of the mutable
//! fields (`deviceName`/`blocked`/`paused`). The read legs honour both, so a
//! deleted device stops appearing and an updated one shows its new settings.
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
