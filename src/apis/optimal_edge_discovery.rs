//! CAMARA **Optimal Edge Discovery** API.
//!
//! Optimal Edge Discovery lets a client learn the edge cloud zones (MEC regions)
//! best suited to run an application's workload for a device — a **ranked** list
//! (1–20) of `EdgeCloudZone`s, never the device's coordinates. It is the ranked
//! successor to Simple Edge Discovery (which answers a single closest zone). The
//! caller supplies an `applicationProfileId` and either a `device` object
//! (two-legged / CIBA) or relies on the device identity a three-legged token
//! carries, and may narrow the search to one `edgeCloudRegion`.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/optimal-edge-discovery/vwip`. Optimal Edge Discovery
//!   has no released version yet (its upstream `main` spec is versioned `wip`), so
//!   — like Device Data Volume / Media Streaming Rate — its canonical base path is
//!   literally `vwip`.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Optimal Edge Discovery.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
