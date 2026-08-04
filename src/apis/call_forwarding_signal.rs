//! CAMARA **Call Forwarding Signal** API.
//!
//! Call Forwarding Signal lets a client ask the operator whether a phone number
//! has call forwarding configured — a signal used in anti-fraud checks (an
//! attacker who has hijacked a line often turns on unconditional forwarding to
//! intercept calls/OTPs). It is identifier-keyed: in two-legged / CIBA the
//! caller submits the `phoneNumber` directly; in a three-legged token the line
//! is already identified by the token subject and the number must *not* be
//! resubmitted.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_4`] — mounted at `/call-forwarding-signal/v0.4` (CAMARA 0.4.0, r3.3).

pub mod v0_4;

use axum::Router;

/// Every mounted major version of Call Forwarding Signal.
pub fn routes() -> Router {
    Router::new().merge(v0_4::routes())
}
