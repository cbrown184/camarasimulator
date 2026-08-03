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
//! - **Device Identifier v0.3** — `/device-identifier/v0.3/…`.
//! - **One Time Password SMS v1** — `/one-time-password-sms/v1/…`.
//! - **Quality on Demand v1** — `/quality-on-demand/v1/…`.
//! - **Location Verification v3** — `/location-verification/v3/…`.
//! - **Location Retrieval v0.4** — `/location-retrieval/v0.4/…`.
//! - **Geofencing Subscriptions v0.4** — `/geofencing-subscriptions/v0.4/…`.
//! - **Carrier Billing v0.5** — `/carrier-billing/v0.5/…`.
//!
//! Plus [`openapi`], which serves each mounted API's vendored OpenAPI spec at
//! `/{api}/v{n}/openapi.yaml` (docs/DESIGN.md §9).

pub mod carrier_billing;
pub mod device_identifier;
pub mod device_reachability_status;
pub mod device_roaming_status;
pub mod geofencing_subscriptions;
pub mod kyc_match;
pub mod location_retrieval;
pub mod location_verification;
pub mod number_verification;
pub mod one_time_password_sms;
pub mod openapi;
pub mod quality_on_demand;
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
        .merge(device_identifier::routes())
        .merge(one_time_password_sms::routes())
        .merge(quality_on_demand::routes())
        .merge(location_verification::routes())
        .merge(location_retrieval::routes())
        .merge(geofencing_subscriptions::routes())
        .merge(carrier_billing::routes())
        .merge(openapi::routes())
}
