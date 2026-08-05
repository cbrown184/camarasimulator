//! CAMARA **QoS Provisioning** API.
//!
//! QoS Provisioning lets an application ask the network to apply a QoS profile
//! to a device's traffic **indefinitely** — until it is explicitly revoked —
//! rather than for a bounded duration. It is the *provisioning* counterpart of
//! Quality on Demand (bounded QoS sessions): both live in the CAMARA
//! QualityOnDemand repository, but where QoD mints time-boxed `sessions`, QoS
//! Provisioning mints open-ended `qos-assignments`. Like QoD it is a
//! **stateful, resource-oriented** API: `POST /qos-assignments` creates an
//! assignment and returns an `assignmentId`, and later requests address that
//! assignment by id.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_3`] — mounted at `/qos-provisioning/v0.3` (CAMARA qos-provisioning
//!   0.3.0, release r3.2 — the latest published version, mounted at its real
//!   sub-1.0 base path like KYC Match v0.3 / Location Retrieval v0.4, DESIGN §9).
//!   First slice: `POST /qos-assignments` + `GET /qos-assignments/{id}`.

pub mod store;
pub mod v0_3;

use axum::Router;

/// Every mounted major version of QoS Provisioning.
pub fn routes() -> Router {
    Router::new().merge(v0_3::routes())
}
