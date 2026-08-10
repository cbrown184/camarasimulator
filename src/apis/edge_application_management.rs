//! CAMARA **Edge Application Management** API.
//!
//! Edge Application Management lets an application provider manage the lifecycle
//! of applications deployed onto an operator's edge cloud — registering apps,
//! instantiating them, and requesting deployments onto specific edge cloud
//! zones. It is the management counterpart to the discovery APIs already
//! mounted (Simple / Optimal Edge Discovery, Application Endpoint Discovery):
//! discovery *finds* the right edge zone for a device; Edge Application
//! Management *places workloads* onto those zones.
//!
//! CamaraSim implements the read-only zone-catalog leg first — the
//! `edge-cloud-zones` a provider deploys onto — reusing the fixed EdgeCloud zone
//! table shape shared across the EdgeCloud family (docs/DESIGN.md §7). The
//! stateful app / app-instance / deployment resources are later passes.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/edge-application-management/vwip` (CAMARA
//!   EdgeApplicationManagement `wip`, mounted at its canonical `vwip` base path
//!   like the other unreleased EdgeCloud APIs).

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of Edge Application Management.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
