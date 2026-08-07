//! CAMARA **Traffic Influence** API (work-in-progress).
//!
//! Traffic Influence lets an *API consumer* (typically an application provider)
//! influence how the operator's network routes an application's traffic toward
//! the nearest **edge cloud** instance of that application — steering flows to a
//! chosen `edgeCloudZoneId`/`edgeCloudRegion`, optionally narrowed to specific
//! source/destination traffic filters. The upstream CAMARA API exposes a
//! `TrafficInfluence` **resource** lifecycle (`POST /traffic-influences`, then
//! `GET`/`PATCH`/`DELETE /traffic-influences/{trafficInfluenceID}`), a
//! per-device create (`POST /traffic-influence-devices`), and CloudEvents
//! change notifications.
//!
//! CamaraSim mounts the resource lifecycle + the per-device create:
//! - [`vwip`] — mounted at `/traffic-influence/vwip` (CAMARA `wip` — no released
//!   version yet, so mounted at its canonical `vwip` base path, like the other
//!   pre-1.0 wip APIs). Serves `POST /traffic-influences` (`postTrafficInfluence`),
//!   `POST /traffic-influence-devices` (`postTrafficInfluenceDevice`),
//!   `GET`/`PATCH`/`DELETE /traffic-influences/{trafficInfluenceID}`.
//!
//! Creating a resource that can later be read back makes Traffic Influence
//! **stateful**, so a shared in-memory [`store`] holds created resources keyed by
//! `trafficInfluenceID`; `getTrafficInfluence` reads them back. The per-device
//! create makes the same `TrafficInfluence` resource from the same fields plus a
//! required `device` object; for privacy the device is validated only, never
//! echoed nor persisted.
//!
//! The `subscriptionRequest` CloudEvents notifications' **initial event**
//! (`config.initialEvent: true`) is delivered on create (see [`notifications`]);
//! the ongoing state-change stream, `subscriptionExpireTime` /
//! `subscriptionMaxEvents` lifecycle, and TLS (`https://` sink) delivery are
//! deferred to a later pass (see `PROGRESS.md`).

pub mod notifications;
pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted version of Traffic Influence.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
