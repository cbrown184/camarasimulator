//! CAMARA **Dedicated Network — Networks** API.
//!
//! Part of the CAMARA DedicatedNetworks family (the resource sibling of the
//! already-mounted read-only **Network Profiles** catalog). A *dedicated
//! network* is a customer-provisioned private network: the client chooses a
//! connectivity profile (a `networkProfileId` from the Network Profiles catalog,
//! or a bare `qosProfileName`), a `serviceTime` window and a `serviceAreaId`,
//! and the operator provisions a network that moves through a lifecycle
//! (`REQUESTED` → `RESERVED` → `ACTIVATED` → `TERMINATED`).
//!
//! CamaraSim keeps created networks in an in-memory store ([`vwip::routes`] wires
//! the endpoints; [`store`] is the state); it is a **stateful, resource-oriented**
//! API. The full Networks CRUD is mounted — create, read-back, list, and delete
//! (docs/DESIGN.md §7 drives each result from the request / store state).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/dedicated-network/vwip` (CAMARA DedicatedNetworks /
//!   Networks, work-in-progress — no released version, mounted at its canonical
//!   `vwip` base path).

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of Dedicated Network — Networks.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
