//! CAMARA **Dedicated Network — Network Profiles** API.
//!
//! Part of the CAMARA DedicatedNetworks family. A *network profile* is a named,
//! operator-published template describing the connectivity a dedicated network
//! offers: how many devices it admits, its aggregated up/down throughput, and
//! which Quality-on-Demand profiles it permits (with a default). It is the
//! read-only catalog a client consults before booking a dedicated network — the
//! Dedicated-Networks analogue of the already-mounted **QoS Profiles** API.
//!
//! CamaraSim serves a **fixed catalog** of network profiles (there is no upstream
//! backend to query); the single-profile lookup keys its answer off the requested
//! `profileId` (docs/DESIGN.md §7).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/dedicated-network-profiles/vwip` (CAMARA
//!   DedicatedNetworks / Network Profiles, work-in-progress — no released
//!   version, mounted at its canonical `vwip` base path).

pub mod vwip;

use axum::Router;

/// Every mounted major version of Dedicated Network — Network Profiles.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
