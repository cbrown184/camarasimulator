//! CAMARA **Number Verification** API.
//!
//! Number Verification lets a client confirm that the phone number it holds for
//! a user is the same number the user's device is currently connected with — a
//! silent, network-based check with no OTP. It is a three-legged API: the caller
//! presents an access token obtained via `authorization_code`/CIBA whose context
//! identifies the device, and the operator compares the submitted number against
//! the device's own number.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v1`] — mounted at `/number-verification/v1`.

pub mod v1;

use axum::Router;

/// Every mounted major version of Number Verification.
pub fn routes() -> Router {
    Router::new().merge(v1::routes())
}
