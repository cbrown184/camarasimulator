//! CAMARA **Dedicated Network — Accesses** API.
//!
//! Part of the CAMARA DedicatedNetworks family (the access sibling of the
//! already-mounted **Networks** and read-only **Network Profiles** APIs). An
//! *access* binds a set of devices (each a CAMARA `Device`) to a dedicated
//! network, optionally restricting the QoS profiles those devices may use. The
//! `POST /accesses` operation creates an access, mints its `id`, evaluates each
//! submitted device to a per-device `GRANTED`/`DENIED` status, and returns an
//! `AccessInfo` carrying the minted `id`, the associated `networkId`, aggregate
//! `stats` and the recently-processed devices.
//!
//! CamaraSim keeps created accesses in an in-memory store ([`vwip::routes`] wires
//! the endpoints; [`store`] is the state); it is a **stateful, resource-oriented**
//! API. So far only the create leg is mounted (docs/DESIGN.md §7 drives the
//! result from the request); read / list / delete and the device sub-resources
//! (`/accesses/{accessId}/devices…`) arrive in later passes.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/dedicated-network-accesses/vwip` (CAMARA
//!   DedicatedNetworks / Accesses, work-in-progress — no released version,
//!   mounted at its canonical `vwip` base path).

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of Dedicated Network — Accesses.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
