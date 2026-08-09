//! CAMARA **Short Message Service** (SMS) API.
//!
//! The SMS API lets a business/application send an SMS to one or more recipients
//! and receive a handle (`msgId`) plus the send `timestamp`. It is a two-legged,
//! business-facing API: both the `from` sender and the `to` recipients are
//! carried in the request body (there is no line subject to fall back to), so —
//! like Verified Caller / Click to Dial — it has no two-legged/three-legged
//! identifier dance.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0alpha1`] — mounted at `/sms/v0alpha1` (CAMARA Short Message Service,
//!   `0.1.0-alpha.1`; the alpha spec's canonical base path is `sms/v0alpha1`, so
//!   CamaraSim honours the version in the URL there).

pub mod v0alpha1;

use axum::Router;

/// Every mounted version of Short Message Service.
pub fn routes() -> Router {
    Router::new().merge(v0alpha1::routes())
}
