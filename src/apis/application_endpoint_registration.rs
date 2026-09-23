//! CAMARA **Application Endpoint Registration** API.
//!
//! Application Endpoint Registration lets an application provider register the set
//! of network-reachable **endpoints** (FQDN / IPv4 / IPv6 + port, each pinned to
//! an `edgeCloudZone`) that serve a given application, tie them to an
//! `applicationProfileId`, and get back an opaque `applicationEndpointListId`.
//! That id then identifies the registration for read / update / deregister. It is
//! the *registration* counterpart of the sibling **Application Endpoint
//! Discovery** API (which the simulator already implements): Discovery answers
//! *which* endpoints are optimal for a device; Registration is how a provider
//! *publishes* those endpoints in the first place. It is a **stateful,
//! resource-oriented** API.
//!
//! Its `edgeCloudZone` is an EdgeCloud *placement identifier* (zone id / name /
//! provider / region), not a geographic coordinate — so, like Application Endpoint
//! Discovery / Optimal Edge Discovery, this API is **non-spatial** in CamaraSim's
//! classification (no lat/long, no area geometry).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/application-endpoint-registration/vwip` (CAMARA
//!   ApplicationEndpointRegistration, work-in-progress — no released version yet,
//!   so mounted at its canonical `vwip` base path like Application Endpoint
//!   Discovery / Application Profiles, DESIGN §9). First slice:
//!   `POST /application-endpoint-lists` (`registerApplicationEndpoints`).

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted major version of Application Endpoint Registration.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
