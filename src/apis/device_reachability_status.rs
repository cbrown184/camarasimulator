//! CAMARA **Device Reachability Status** API.
//!
//! Device Reachability Status lets a client ask the operator whether a device is
//! currently reachable on the network and, if so, over which bearers (data / SMS).
//! It is the "reachability" half of what used to be the combined CAMARA *Device
//! Status* API; in the Spring25 meta-release that API was split into separate
//! **Device Reachability Status** and **Device Roaming Status** APIs, so — like
//! KYC Match's `v0.3` — CamaraSim mounts this under its real published name and
//! version rather than a loose "device-status v1".
//!
//! It is identifier-keyed: the caller either submits a `device` (phoneNumber,
//! network access identifier, or IP address) in the request body (two-legged /
//! CIBA) or relies on the device identity carried by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/device-reachability-status/v1`.

pub mod v1;

use axum::Router;

/// Every mounted major version of Device Reachability Status.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
