//! CAMARA **Network Traffic Analysis** API (part of the NetworkInsights suite).
//!
//! The sibling of [`crate::apis::network_health_assessment`]: where Health
//! Assessment scores a network's *health*, Traffic Analysis reports *aggregated,
//! service-level traffic statistics* for a network — how much each detected
//! application (e.g. `whatsapp`, `netflix`) sent and received over a requested
//! time window, at DAY or HOUR granularity. It is derived from DPI collection and
//! is deliberately **network-level only**: no individual device, user, or
//! terminal appears in any field, so it is a two-legged (`client_credentials`)
//! service-to-service query — no end-user personal data, no consent, no
//! three-legged token.
//!
//! The network is identified by the required `networkId` query parameter (a
//! UUID); the window by the required `startDate`/`endDate`; the granularity by
//! the required `frequency` (`DAY`/`HOUR`). Results are paged (`page`/`perPage`)
//! and can be narrowed to a single application with the optional `app` filter.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/network-traffic-analysis/vwip` (CAMARA Network
//!   Traffic Analysis, work-in-progress — no released version yet, so mounted at
//!   its canonical `vwip` base path, mirroring the other pre-1.0 wip APIs).

pub mod vwip;

use axum::Router;

/// Every mounted version of Network Traffic Analysis.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
