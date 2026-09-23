//! CAMARA **One Time Password SMS** API.
//!
//! One Time Password SMS lets a client send a one-time password to a phone
//! number over SMS and later validate a code the user read back — the classic
//! phone-ownership check behind sign-up and step-up authentication. It is
//! CamaraSim's first **stateful** API: the `authenticationId` returned by
//! `send-code` is redeemed by `validate-code`, with the pending code held in an
//! in-memory store ([`v1::routes`] via [`store`]).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/one-time-password-sms/v1` (CAMARA 1.1.1).

pub mod store;
pub mod v1;

use axum::Router;

/// Every mounted major version of One Time Password SMS.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
