//! CAMARA **Consent Info** API.
//!
//! Consent Info reports the *current consent status* an end user has granted for
//! a given set of data-processing `scopes` under a declared `purpose`
//! (`dpv:<Purpose>`). Before a service processes a subscriber's data it can ask
//! the operator whether the necessary consent is currently valid for processing,
//! and — when it is not — obtain a `captureUrl` where the end user can be
//! redirected to grant it.
//!
//! It is phone-number-keyed: the subscriber is identified by the submitted
//! `phoneNumber` (two-legged) or by the identity a three-legged access token
//! authenticated, exactly like Subscription Status and Number Recycling.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/consent-info/vwip` (CAMARA ConsentInfo,
//!   work-in-progress — no released version yet, so mounted at its canonical
//!   `vwip` base path, mirroring the other pre-1.0 wip APIs).

pub mod vwip;

use axum::Router;

/// Every mounted version of Consent Info.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
