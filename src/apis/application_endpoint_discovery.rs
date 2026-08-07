//! CAMARA **Application Endpoint Discovery** API.
//!
//! Application Endpoint Discovery returns the optimal **application endpoint(s)**
//! an end-user's device can connect to — not just the closest edge cloud zone
//! (that is Simple / Optimal Edge Discovery), but the concrete `port` + IP/FQDN
//! of the application instance(s) with the shortest network path to the device.
//! The caller identifies the application (`appId` or `applicationEndpointsId`)
//! and either a `device` object (two-legged / CIBA) or relies on the device
//! identity a three-legged token carries.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/application-endpoint-discovery/vwip`. Application
//!   Endpoint Discovery has no released version yet (its upstream `main` spec is
//!   versioned `wip`), so — like Optimal Edge Discovery / Device Data Volume —
//!   its canonical base path is literally `vwip`.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Application Endpoint Discovery.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
