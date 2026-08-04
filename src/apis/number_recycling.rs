//! CAMARA **Number Recycling** API.
//!
//! Number Recycling answers whether the *subscriber* behind a phone number has
//! changed since a caller-supplied reference date — i.e. whether the number has
//! been reassigned ("recycled") to a new person. It is an anti-fraud / account-
//! integrity signal: a service that saw a number bound to a user last year can
//! ask whether that binding still holds before trusting it for recovery or KYC.
//!
//! It is phone-number-keyed: the line is identified by the submitted
//! `phoneNumber` (two-legged) or by the identity a three-legged access token
//! authenticated, exactly like Call Forwarding Signal.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_2`] — mounted at `/number-recycling/v0.2` (CAMARA Number Recycling
//!   0.2.0, release r2.2 — the latest published version).

pub mod v0_2;

use axum::Router;

/// Every mounted version of Number Recycling.
pub fn routes() -> Router {
    Router::new().merge(v0_2::routes())
}
