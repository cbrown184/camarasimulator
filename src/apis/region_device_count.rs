//! CAMARA **Region Device Count** API.
//!
//! Region Device Count answers *how many devices are in a geographic region*
//! during an optional time interval — a privacy-preserving aggregate, never a
//! per-device location. It is **area-keyed** (there is no phone/device
//! identifier): the caller submits an `area` (a `CIRCLE` or a `POLYGON`) and the
//! operator returns a `count` together with a coverage `status`.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_2`] — mounted at `/region-device-count/v0.2` (CAMARA Region Device
//!   Count 0.2.0, release r2.2 — the latest published version, so mounted at its
//!   real sub-1.0 base path like KYC Match / Customer Insights / Connectivity
//!   Insights).
//!
//! Only the synchronous `POST /count` path is modelled; the API's asynchronous
//! `sink`/CloudEvents delivery surface is a documented cut (mirroring the
//! deferred TLS/notification work across the stateful APIs).

pub mod v0_2;

use axum::Router;

/// Every mounted major version of Region Device Count.
pub fn routes() -> Router {
    Router::new().merge(v0_2::routes())
}
