//! CAMARA **Connected Network Type** API.
//!
//! Connected Network Type lets a client learn which mobile communication
//! technology (`2G`/`3G`/`4G`/`5G`, or `UNKNOWN`) a device is currently attached
//! to — a coarse radio-access signal, never the device's location. It is
//! device-keyed: the caller either submits a `device` object directly (two-legged
//! / CIBA) or relies on the device identity carried by a three-legged access
//! token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_2`] — mounted at `/connected-network-type/v0.2` (CAMARA Connected
//!   Network Type 0.2.0, release r1.2 — the latest published version, so mounted
//!   at its real sub-1.0 base path like KYC Match / Device Identifier /
//!   Location Retrieval).
//!
//! Only the base `POST /retrieve` operation is modelled; the API's separate
//! event-subscription surface (`connected-network-type-subscriptions`) is not
//! in scope for this API module.

pub mod v0_2;

use axum::Router;

/// Every mounted major version of Connected Network Type.
pub fn routes() -> Router {
    Router::new().merge(v0_2::routes())
}
