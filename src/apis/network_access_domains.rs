//! CAMARA **Network Access Domains** API (part of NetworkAccessManagement).
//!
//! Network Access Domains configures **Trust Domains** — groups of devices on the
//! subscriber's home network that share a common access policy (Wi-Fi or Thread).
//! It is the sibling of the operator-facing Network Access Devices API in the same
//! CAMARA NetworkAccessManagement repo.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/network-access-domains/vwip` (CAMARA
//!   NetworkAccessManagement / Network Access Domains `wip` — no released version
//!   yet, so mounted at its canonical `vwip` base path like Network Access Devices
//!   and the other pre-1.0 wip APIs).
//!
//! Implemented so far (all read-only): the provider-level
//! `GET /trust-domains/capabilities` leg and the `GET /services` catalog
//! (`getServices`). The single-service `GET /services/{serviceId}` read and the
//! stateful Trust Domain / device CRUD resources are later slices.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Network Access Domains.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
