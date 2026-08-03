//! CAMARA **Device Location Retrieval** API.
//!
//! Location Retrieval lets a client ask the operator for a device's *current
//! location*, returned as an area (a circle around a `center` point with a
//! `radius` expressing the location's accuracy / uncertainty). Unlike its
//! companion [Location Verification](super::location_verification) — which
//! answers a yes/no verdict against a supplied area — Retrieval hands back the
//! device's estimated position itself. It is the second of the CamaraSim
//! *spatial* APIs (docs/DESIGN.md §12, Phase 4).
//!
//! It is identifier-keyed: the caller either submits a `device` (phoneNumber or
//! IP address) in the request body (two-legged / CIBA) or relies on the device
//! identity carried by a three-legged access token.
//!
//! CamaraSim mounts this under the API's real published version. The latest
//! CAMARA Location Retrieval public release (r3.2) carries API version `0.4.0` —
//! it has never reached 1.0.0 — so, mirroring the KYC Match / Device Identifier
//! `v0.x` decision (docs/DESIGN.md §9), it is served at `/location-retrieval/v0.4`
//! rather than a loose "v1".
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_4`] — mounted at `/location-retrieval/v0.4` (CAMARA 0.4.0, release r3.2).

pub mod v0_4;

use axum::Router;

/// Every mounted major version of Location Retrieval.
pub fn routes() -> Router {
    Router::new().merge(v0_4::routes())
}
