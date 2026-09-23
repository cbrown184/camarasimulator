//! CAMARA **Population Density Data** API.
//!
//! Population Density Data estimates how many people are in a geographic area
//! over a time window, returned as a per-cell people-per-km² figure (with a
//! min/max confidence band) — an aggregate, privacy-preserving signal used for
//! things like crowd-safety, retail footfall, or drone-flight risk assessment.
//! It never reveals an individual device's location.
//!
//! It is **area-keyed** (no phone/device identifier): the caller supplies the
//! area to estimate, either as a `POLYGON` boundary or as a `GEOHASHLIST` of
//! geohash cells, plus the `startTime`/`endTime` window.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/population-density-data/vwip`. Population Density
//!   Data has no released version yet (its upstream `main` spec is versioned
//!   `wip`), so — like Device Visit Location — its canonical base path is
//!   literally `vwip`, tracking the CAMARA work-in-progress spec. When the API
//!   cuts a release this module will gain the numbered version.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Population Density Data.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
