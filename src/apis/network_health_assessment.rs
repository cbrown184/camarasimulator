//! CAMARA **Network Health Assessment** API (part of the NetworkInsights suite).
//!
//! Network Health Assessment reports an *aggregate, network-level* health score
//! for a specified network (e.g. a private network or slice) and its component
//! modules — the whole network, or just its wireless / transport / core segment.
//! The score is a `0..100` value the operator's assessment platform derives from
//! the module's performance and fault indicators (higher is healthier); it is
//! exposed for monitoring, optimisation, and early-warning use.
//!
//! It is deliberately **not** device-keyed: the API exposes only aggregated
//! network-module data, never any individual device / user / terminal. It is a
//! two-legged (`client_credentials`) service-to-service query — no end-user
//! personal data is processed, so no three-legged token or consent is required.
//! The network is identified by the required `networkId` query parameter (a
//! UUID); the requested module by the required `netType` query parameter.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/network-health-assessment/vwip` (CAMARA Network
//!   Health Assessment, work-in-progress — no released version yet, so mounted
//!   at its canonical `vwip` base path, mirroring the other pre-1.0 wip APIs).

pub mod vwip;

use axum::Router;

/// Every mounted version of Network Health Assessment.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
