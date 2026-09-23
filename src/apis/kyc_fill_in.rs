//! CAMARA **KYC Fill-in** API.
//!
//! KYC Fill-in lets a client retrieve the operator-verified identity attributes
//! for a line (name, address, birthdate, e-mail, gender, …) so a sign-up or KYC
//! form can be pre-filled. It is identifier-keyed: the caller either submits the
//! `phoneNumber` directly (two-legged / CIBA) or relies on the line identity
//! carried by a three-legged access token. Unlike KYC Match (which only *confirms*
//! attributes the caller already holds) this API *returns* them, filtered to the
//! attributes the token's scopes authorise.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_3`] — mounted at `/kyc-fill-in/v0.3` (CAMARA KYC Fill-in 0.3.0).

pub mod v0_3;

use axum::Router;

/// Every mounted major version of KYC Fill-in.
pub fn routes() -> Router {
    Router::new().merge(v0_3::routes())
}
