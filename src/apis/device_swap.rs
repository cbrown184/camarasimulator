//! CAMARA **Device Swap** API.
//!
//! Device Swap lets a client ask the operator whether the **device** bound to a
//! phone number has changed recently — a common anti-fraud / account-recovery
//! signal, the device counterpart of SIM Swap. It is identifier-keyed: the caller
//! either submits the `phoneNumber` directly (two-legged / CIBA) or relies on the
//! device identity carried by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/device-swap/v1` (CAMARA Device Swap 1.0.0, release r3.2).

pub mod v1;

use axum::Router;

/// Every mounted major version of Device Swap.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
