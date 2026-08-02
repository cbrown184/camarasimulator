//! CAMARA **Quality on Demand** (QoD) API.
//!
//! Quality on Demand lets an application ask the network for a specific quality
//! profile (latency / throughput) for the traffic between a device and an
//! application server, for a bounded duration — the classic "boost this flow"
//! request behind cloud gaming, live video, and real-time control. It is a
//! **stateful, resource-oriented** API: `POST /sessions` creates a QoS session
//! and returns a `sessionId`, and later requests address that session by id.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/quality-on-demand/v1` (CAMARA quality-on-demand
//!   1.1.0, release r3.2). First slice: `POST /sessions` + `GET /sessions/{id}`.

pub mod store;
pub mod v1;

use axum::Router;

/// Every mounted major version of Quality on Demand.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
