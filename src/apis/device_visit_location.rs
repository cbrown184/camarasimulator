//! CAMARA **Device Visit Location** API.
//!
//! Device Visit Location lets a client learn *where a device has been* over a
//! caller-supplied time window — returned as a coarse list of geographic codes
//! (postal codes with their country), never precise coordinates. It is an
//! anti-fraud / identity-assurance signal: a bank can check whether a device was
//! recently in a region consistent with a claimed transaction.
//!
//! It is device-keyed: the caller either submits a `device` object directly
//! (two-legged / CIBA) or relies on the device identity carried by a three-legged
//! access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/device-visit-location/vwip`. Device Visit Location
//!   has no released version yet (its `main` spec is versioned `wip`), so — unlike
//!   the published sub-1.0 APIs (KYC Match v0.3, Location Retrieval v0.4) — its
//!   canonical base path is literally `vwip`, tracking the CAMARA work-in-progress
//!   spec. When the API cuts a release this module will gain the numbered version.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Device Visit Location.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
