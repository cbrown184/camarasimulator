//! CAMARA business APIs.
//!
//! Each CAMARA API lives in its own module, one submodule per major version
//! (docs/DESIGN.md §5), mounted under its canonical URL `/{api}/v{MAJOR}/…`
//! (§9). [`routes`] composes every mounted API into a single router the
//! top-level app merges.
//!
//! So far:
//! - **Number Verification v1** — `/number-verification/v1/…`.
//! - **SIM Swap v2** — `/sim-swap/v2/…`.
//! - **KYC Match v0.3** — `/kyc-match/v0.3/…`.
//! - **Device Reachability Status v1** — `/device-reachability-status/v1/…`.
//! - **Device Roaming Status v1** — `/device-roaming-status/v1/…`.

pub mod device_reachability_status;
pub mod device_roaming_status;
pub mod kyc_match;
pub mod number_verification;
pub mod sim_swap;

use axum::Router;

/// Every mounted CAMARA API's routes, merged into one router.
pub fn routes() -> Router {
    Router::new()
        .merge(number_verification::routes())
        .merge(sim_swap::routes())
        .merge(kyc_match::routes())
        .merge(device_reachability_status::routes())
        .merge(device_roaming_status::routes())
}
