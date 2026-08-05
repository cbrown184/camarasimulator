//! CAMARA **Session Insights** API.
//!
//! Session Insights lets an application register an interest in the network
//! quality of a specific application session — a device talking to a named
//! application server — so the operator can later report quality metrics and
//! notify the application when the experience degrades or the session ends. It
//! is a **stateful, resource-oriented** API: `POST /sessions` creates a session
//! resource and mints an opaque `id`, and later requests address that resource
//! by its id (`GET /sessions/{sessionId}`, and — in later passes —
//! `DELETE`, `retrieve-sessions`, and `metrics`).
//!
//! It is **non-spatial** (no location in the model) and the session is keyed by
//! the CAMARA `Device` object — the submitted `device` identifier, else the
//! access-token subject (three-legged fallback), mirroring Quality on Demand.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/session-insights/vwip` (CAMARA SessionInsights,
//!   work-in-progress — no released version yet, so mounted at its canonical
//!   `vwip` base path, mirroring the other pre-1.0 wip APIs).

pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted version of Session Insights.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
