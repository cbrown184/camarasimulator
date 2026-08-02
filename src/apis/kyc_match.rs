//! CAMARA **Know Your Customer (KYC) Match** API.
//!
//! KYC Match lets a client submit customer identity attributes (name, address,
//! date of birth, e-mail, …) and asks the operator whether each attribute
//! matches the operator's own records for that line — a common signal in
//! onboarding, fraud-prevention, and regulatory-compliance flows. It is
//! attribute-keyed: the caller sends the attributes to check, optionally scoped
//! to a `phoneNumber` (two-legged / CIBA) or to the identity carried by a
//! three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_3`] — mounted at `/kyc-match/v0.3` (CAMARA KYC Match 0.3.0).

pub mod v0_3;

use axum::Router;

/// Every mounted version of KYC Match.
pub fn routes() -> Router {
    Router::new().merge(v0_3::routes())
}
