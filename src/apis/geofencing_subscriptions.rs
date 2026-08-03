//! CAMARA **Geofencing Subscriptions** API.
//!
//! Geofencing Subscriptions lets an application be *notified* when a device
//! enters or leaves a geographic area, instead of polling its location. The
//! caller registers a subscription describing a device, a circular area, and the
//! event types of interest (`area-entered` / `area-left`) plus a `sink` callback
//! URL; the network later POSTs a CloudEvent to that sink on each matching
//! transition. It is an **event-subscription** API: `POST /subscriptions` creates
//! a subscription and returns an `id`, and later requests address that
//! subscription by id.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_4`] — mounted at `/geofencing-subscriptions/v0.4` (CAMARA
//!   geofencing-subscriptions 0.4.0, release r3.2 — the latest published version,
//!   mounted at its real sub-1.0 version like KYC Match / Device Identifier /
//!   Location Retrieval). First slice: `POST /subscriptions` +
//!   `GET /subscriptions/{subscriptionId}`.

pub mod store;
pub mod v0_4;

use axum::Router;

/// Every mounted major version of Geofencing Subscriptions.
pub fn routes() -> Router {
    Router::new().merge(v0_4::routes())
}
