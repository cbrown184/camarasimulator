//! CAMARA **Predictive Connectivity Data** API.
//!
//! Predictive Connectivity Data forecasts the network connectivity an operator
//! expects to be able to sustain across a geographic area over a future (or
//! recent) time window — returned per grid cell as a stack of vertical
//! **layers** (altitude bands), each rated good / marginal / none / no-data. It
//! is an aggregate, privacy-preserving planning signal (used e.g. for drone-BVLOS
//! flight planning, connected-car routing, or fixed-wireless siting); it never
//! reveals an individual device's location.
//!
//! It is **area-keyed** (no phone/device identifier): the caller supplies the
//! area to forecast, either as a `POLYGON` boundary or as a `GEOHASHLIST` of
//! geohash cells, plus a target `serviceLevel` and the `startTime`/`endTime`
//! window.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/predictive-connectivity-data/vwip`. Predictive
//!   Connectivity Data has no released version yet (its upstream `main` spec is
//!   versioned `wip`), so — like Population Density Data — its canonical base path
//!   is literally `vwip`, tracking the CAMARA work-in-progress spec. When the API
//!   cuts a release this module will gain the numbered version.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Predictive Connectivity Data.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
