//! CAMARA **Device Identifier** API.
//!
//! Device Identifier lets a client obtain details about the physical device a
//! mobile subscription is currently attached to — its type (manufacturer /
//! model / TAC), its full identifier (IMEI/IMEISV), or a pseudonymous device
//! identifier — without the client needing to read anything from the device
//! itself.
//!
//! It is identifier-keyed: the caller either submits a `device` (phoneNumber,
//! network access identifier, or IP address) in the request body (two-legged /
//! CIBA) or relies on the device identity carried by a three-legged access
//! token.
//!
//! CamaraSim mounts this under the API's real published version. The latest
//! CAMARA Device Identifier public release (r2.2) carries API version `0.3.0` —
//! it has never reached 1.0.0 — so, mirroring the KYC Match `v0.3` decision
//! (docs/DESIGN.md §9), it is served at `/device-identifier/v0.3` rather than a
//! loose "v1".
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_3`] — mounted at `/device-identifier/v0.3`.

pub mod v0_3;

use axum::Router;

/// Every mounted major version of Device Identifier.
pub fn routes() -> Router {
    Router::new().merge(v0_3::routes())
}
