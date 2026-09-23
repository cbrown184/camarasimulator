//! CAMARA **Media Streaming Rate** (Device Media Streaming Rate) API.
//!
//! Media Streaming Rate lets a client learn the maximum **downstream** media bit
//! rate an operator's network can currently sustain for a device — a figure an
//! application can use to pick a streaming quality. It answers a magnitude plus a
//! unit (`{ maxDownstreamMediaBitRateSupported, unit }`), never raw per-flow
//! telemetry. It is device-keyed: the caller either submits a `device` object
//! directly (two-legged / CIBA) or relies on the device identity carried by a
//! three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/media-streaming-rate/vwip`. Media Streaming Rate has
//!   no released version yet (its upstream `main` spec is versioned `wip`), so —
//!   like Device Data Volume / Device Visit Location — its canonical base path is
//!   literally `vwip`.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Media Streaming Rate.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
