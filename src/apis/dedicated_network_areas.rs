//! CAMARA **Dedicated Network — Areas** API.
//!
//! Part of the CAMARA DedicatedNetworks family. A *service area* is a named,
//! operator-published geographical region (an `area`, here a WGS-84 `CIRCLE`)
//! together with the network and/or QoS profiles available within it. It is the
//! read-only catalog a client consults to learn *where* — and with which
//! profiles — a dedicated network can be provisioned; the geographical sibling of
//! the already-mounted **Network Profiles** catalog.
//!
//! CamaraSim serves a **fixed catalog** of service areas (there is no upstream
//! backend to query); the single-area lookup keys its answer off the requested
//! `areaId` (docs/DESIGN.md §7).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/dedicated-network-areas/vwip` (CAMARA
//!   DedicatedNetworks / Areas, work-in-progress — no released version, mounted
//!   at its canonical `vwip` base path).

pub mod vwip;

use axum::Router;

/// Every mounted major version of Dedicated Network — Areas.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
