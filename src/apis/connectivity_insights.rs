//! CAMARA **Connectivity Insights** API.
//!
//! Connectivity Insights lets an application ask the network whether it can meet
//! that application's quality requirements right now (or at a future instant)
//! for a given device and application server — a set of per-KPI *policy
//! fulfilment* verdicts (`meets` / `unable to meet the application
//! requirements`), never raw measurements. It is device-keyed: the caller
//! either submits a `device` object directly (two-legged / CIBA) or relies on
//! the device identity carried by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_6`] — mounted at `/connectivity-insights/v0.6` (CAMARA Connectivity
//!   Insights 0.6.0, release r3.2 — the latest published version, so mounted at
//!   its real sub-1.0 base path like KYC Match / Device Identifier / Connected
//!   Network Type).
//!
//! Only the stateless `POST /check-network-quality` operation is modelled; the
//! Connectivity Insights subproject's stateful companions (Application Profiles
//! CRUD and the `connectivity-insights-subscriptions` event API) are separate
//! APIs, out of scope for this module.

pub mod v0_6;

use axum::Router;

/// Every mounted major version of Connectivity Insights.
pub fn routes() -> Router {
    Router::new().merge(v0_6::routes())
}
