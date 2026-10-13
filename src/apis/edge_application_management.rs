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
//! CamaraSim serves the full modelled surface: the read-only `edge-cloud-zones`
//! and `clusters` catalogs (reusing the fixed EdgeCloud zone table shape shared
//! across the EdgeCloud family — docs/DESIGN.md §7), the stateful `apps` CRUD
//! (onboard/read/list/delete), the `app-instances` CRUD (create/read/list/delete,
//! in-memory [`instance_store`]), and the `deployments` lifecycle
//! (create/read/list/update via JSON Merge Patch/delete, in-memory
//! [`deployment_store`]). Documented cuts remain per leg (e.g. the live
//! `componentEndpointInfo`, the async `202`/`DELETE_REQUESTED` delete form, and
//! the `subscriptionRequest` status-change callbacks) — see the [`vwip`] module
//! docs and the vendored spec's `x-camarasim-scenarios`.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/edge-application-management/vwip` (CAMARA
//!   EdgeApplicationManagement `wip`, mounted at its canonical `vwip` base path
//!   like the other unreleased EdgeCloud APIs).

pub mod deployment_store;
pub mod instance_store;
pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of Edge Application Management.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
