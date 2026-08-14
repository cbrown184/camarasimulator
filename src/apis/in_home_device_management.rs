//! CAMARA **In-Home Device Management** API.
//!
//! In-Home Device Management lets a consumer (or an app acting for the
//! household) discover and manage the devices attached to a home fixed-line
//! network — the laptops, phones, TVs and IoT gadgets behind the router — plus
//! the infrastructure devices (modem, Wi-Fi boosters, mesh pods) that carry
//! them. The household network is named by its `ssid`.
//!
//! CamaraSim implements the read-only inventory leg (`GET /devices`,
//! `listDevices`): a **stateless**, `ssid`-keyed query that returns the
//! household's attached devices. There is no real home network to query, so the
//! roster is derived deterministically from the `ssid` (docs/DESIGN.md §7) — the
//! input is the control plane. The device-mutation legs (`getDevice`,
//! `updateDevice`, `deleteDevice`, `performDeviceAction`, network-health) are a
//! later slice.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/in-home-device-management/v1` (CAMARA
//!   InHomeDeviceManagement 1.0.0, sandbox).

pub mod v1;

use axum::Router;

/// Every mounted major version of In-Home Device Management.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
