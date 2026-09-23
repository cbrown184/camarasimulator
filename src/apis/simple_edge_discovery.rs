//! CAMARA **Simple Edge Discovery** API.
//!
//! Simple Edge Discovery lets a client find the closest **edge cloud zone**
//! (MEC — Multi-access Edge Computing) to a device, so an application can run
//! its edge workload in the zone with the lowest network latency to that
//! device. It is device-keyed: in two-legged / CIBA the caller submits a
//! `device` object; with a three-legged token the device is identified by the
//! token subject and the `device` must *not* be resubmitted.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v2`] — mounted at `/simple-edge-discovery/v2` (CAMARA 2.0.1, r2.3).

pub mod v2;

use axum::Router;

/// Every mounted major version of Simple Edge Discovery.
pub fn routes() -> Router {
    Router::new().merge(v2::routes())
}
