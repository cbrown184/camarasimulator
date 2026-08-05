//! CAMARA **Application Profiles** API.
//!
//! Application Profiles lets an application register a named set of *quality
//! requirements* — network-quality thresholds (latency, throughput, loss,
//! jitter) and/or compute-resource thresholds (CPU, GPU, memory, storage) — and
//! get back an opaque `applicationProfileId`. That id is then referenced by the
//! sibling **Connectivity Insights** API (its `applicationProfileId` request
//! field), which answers whether the network can meet a profile's requirements
//! for a given device right now. It is a **stateful, resource-oriented** API:
//! `POST /application-profiles` creates a profile and returns its id, and later
//! requests address the profile by that id.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/application-profiles/vwip` (CAMARA
//!   application-profiles, work-in-progress — no released version yet, so mounted
//!   at its canonical `vwip` base path like Device Visit Location / Media
//!   Streaming Rate / Optimal Edge Discovery / Verified Caller, DESIGN §9).
//!   First slice: `POST /application-profiles` + `GET /application-profiles/{id}`.

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of Application Profiles.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
