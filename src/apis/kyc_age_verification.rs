//! CAMARA **KYC Age Verification** API.
//!
//! Age Verification answers whether the person behind a phone number is **at or
//! above a caller-supplied age threshold** — a privacy-preserving alternative to
//! handing over a birthdate. A service that must gate a minor (age-restricted
//! goods, adult content, a signup floor) asks "is this line's holder ≥ N?" and
//! gets back only a yes/no/unknown verdict, never the actual age.
//!
//! It is phone-number-keyed: the line is identified by the submitted
//! `phoneNumber` (two-legged) or by the identity a three-legged access token
//! authenticated, exactly like Number Recycling / Call Forwarding Signal.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_1`] — mounted at `/kyc-age-verification/v0.1` (CAMARA KYC Age
//!   Verification 0.1.0, release r2.2 — the latest published version).

pub mod v0_1;

use axum::Router;

/// Every mounted version of KYC Age Verification.
pub fn routes() -> Router {
    Router::new().merge(v0_1::routes())
}
