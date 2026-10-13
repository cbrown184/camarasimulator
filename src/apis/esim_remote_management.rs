//! CAMARA **eSIM Remote Management** API.
//!
//! eSIM Remote Management lets device manufacturers (OEMs) and ecosystem
//! partners remotely manage the eSIM profiles installed on a device's eUICC —
//! downloading, activating, de-activating, deleting, and querying them. Every
//! operation wraps its payload in a base "CMP" request/response envelope
//! (`timestamp` / `sequenceNum` / `clientId` / `data`, answered with a
//! `resultCode` / `resultDesc` / `data`), the upstream API's own convention.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/esim-remote-management/vwip` (CAMARA eSIM Remote
//!   Management, work-in-progress — no released version yet, so mounted at its
//!   canonical `vwip` base path like the other unreleased CAMARA APIs).
//!
//! This module models the complete eSIM Remote Management vwip surface — all
//! four upstream legs: the stateless profile-inventory query
//! (`POST /profile/downloaded-list`, operationId `profileList`), the stateless
//! result query (`POST /profile/result/query`, `profileResultQuery`), and the
//! two command legs modelled as synchronous acknowledgements
//! (`POST /profile/oper`, `profileOperation`; `POST /profile/download`,
//! `profileDownload`). The actual eUICC state changes and `sink` callback
//! delivery are documented cuts (no live eUICC engine).

pub mod vwip;

use axum::Router;

/// Every mounted major version of eSIM Remote Management.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
