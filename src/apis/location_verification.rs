//! CAMARA **Device Location Verification** API.
//!
//! Location Verification lets a client ask the operator whether a device is
//! currently located **within a requested area** (a circle around a point),
//! without revealing the device's exact position — the answer is a verdict
//! (`TRUE` / `FALSE` / `PARTIAL`), not a coordinate. It is the first of the
//! CamaraSim *spatial* APIs (docs/DESIGN.md §12, Phase 4).
//!
//! It is identifier-keyed: the caller either submits a `device` (phoneNumber or
//! IP address) in the request body (two-legged / CIBA) or relies on the device
//! identity carried by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v3`] — mounted at `/location-verification/v3` (CAMARA 3.0.0, release r3.2;
//!   the latest published stable version).

pub mod v3;

use axum::Router;

/// Every mounted major version of Location Verification.
pub fn routes() -> Router {
    Router::new().merge(v3::routes())
}
