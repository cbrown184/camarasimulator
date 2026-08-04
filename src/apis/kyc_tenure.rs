//! CAMARA **KYC Tenure** API.
//!
//! KYC Tenure lets a client establish a level of trust for a network
//! subscription by confirming, against a caller-supplied date, whether the
//! current end user has held the phone number (their "tenure") since at least
//! that date — a low-friction anti-fraud / onboarding signal that never reveals
//! the actual tenure length. It is identifier-keyed: the caller either submits
//! the `phoneNumber` directly (two-legged / CIBA) or relies on the line identity
//! carried by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_2`] — mounted at `/kyc-tenure/v0.2` (CAMARA KYC Tenure 0.2.0, r2.2 —
//!   the latest published version, so mounted at its real sub-1.0 base path like
//!   KYC Match / KYC Fill-in / Number Recycling).

pub mod v0_2;

use axum::Router;

/// Every mounted major version of KYC Tenure.
pub fn routes() -> Router {
    Router::new().merge(v0_2::routes())
}
