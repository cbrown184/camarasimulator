//! CAMARA **SIM Swap** API.
//!
//! SIM Swap lets a client ask the operator whether the SIM card bound to a phone
//! number has been swapped recently — a common signal in anti-fraud and account-
//! recovery flows. It is identifier-keyed: the caller either submits the
//! `phoneNumber` directly (two-legged / CIBA) or relies on the device identity
//! carried by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v2`] — mounted at `/sim-swap/v2`.

pub mod v2;

use axum::Router;

/// Every mounted major version of SIM Swap.
pub fn routes() -> Router {
    Router::new().merge(v2::routes())
}
