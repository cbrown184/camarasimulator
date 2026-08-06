//! CAMARA **Most Frequent Location** API.
//!
//! Most Frequent Location answers *how frequently* a device resides within a
//! caller-supplied geographic area, as a privacy-preserving score from `0`
//! (never present) to `100` (almost always present) — never the device's
//! actual location. It is device-keyed: the caller either submits a `device`
//! object directly (two-legged / CIBA) or relies on the device identity carried
//! by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/most-frequent-location/vwip` (CAMARA
//!   MostFrequentLocation `wip` — no released version yet, so mounted at its
//!   canonical `vwip` base path like Device Visit Location / Session Insights).

pub mod vwip;

use axum::Router;

/// Every mounted major version of Most Frequent Location.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
