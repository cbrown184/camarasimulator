//! CAMARA **Subscription Status** API.
//!
//! Subscription Status reports the *current service status* of a mobile line —
//! whether inbound and outbound Voice/SMS are usable and whether mobile data is
//! usable (and, for data, whether it is throttled). It is an operational /
//! anti-fraud signal: before trusting a number for a call, an OTP, or a data
//! session a service can ask the operator whether those services are actually
//! active on the line right now.
//!
//! It is phone-number-keyed: the line is identified by the submitted
//! `phoneNumber` (two-legged) or by the identity a three-legged access token
//! authenticated, exactly like Number Recycling and Call Forwarding Signal.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/subscription-status/vwip` (CAMARA
//!   SubscriptionStatus, work-in-progress — no released version yet, so mounted
//!   at its canonical `vwip` base path, mirroring the other pre-1.0 wip APIs).

pub mod vwip;

use axum::Router;

/// Every mounted version of Subscription Status.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
