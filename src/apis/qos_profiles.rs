//! CAMARA **QoS Profiles** API.
//!
//! QoS Profiles lets a client discover the Quality-on-Demand profiles an
//! operator offers — the named service levels (`voice`, `video`, …) that a
//! caller then requests when creating a Quality-on-Demand session. It is the
//! read-only companion to the already-mounted **Quality on Demand** API: QoD
//! *applies* a `qosProfile`; QoS Profiles *lists* the ones available.
//!
//! The catalog is a fixed, in-memory set (no upstream backend to query); a
//! caller narrows it with the optional `name`/`status` filters, and — for
//! availability scoping — an optional `device`.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/qos-profiles/v1` (CAMARA QoS Profiles 1.1.0, r3.2).

pub mod v1;

use axum::Router;

/// Every mounted major version of QoS Profiles.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
