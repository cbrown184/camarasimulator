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
//! - **Call Forwarding Signal v0.4** — `/call-forwarding-signal/v0.4/…`.
//! - **Number Recycling v0.2** — `/number-recycling/v0.2/…`.
//! - **KYC Age Verification v0.1** — `/kyc-age-verification/v0.1/…`.
//! - **Device Swap v1** — `/device-swap/v1/…`.
//! - **KYC Fill-in v0.3** — `/kyc-fill-in/v0.3/…`.
//! - **Home Devices QoD v0.4** — `/home-devices-qod/v0.4/…`.
//! - **QoS Profiles v1** — `/qos-profiles/v1/…`.
//! - **KYC Tenure v0.2** — `/kyc-tenure/v0.2/…`.
//! - **Blockchain Public Address v0.3** — `/blockchain-public-address/v0.3/…`.
//! - **Simple Edge Discovery v2** — `/simple-edge-discovery/v2/…`.
//! - **Customer Insights v0.2** — `/customer-insights/v0.2/…`.
//! - **Connected Network Type v0.2** — `/connected-network-type/v0.2/…`.
//! - **Connectivity Insights v0.6** — `/connectivity-insights/v0.6/…`.
//! - **Region Device Count v0.2** — `/region-device-count/v0.2/…`.
//! - **Device Visit Location vwip** — `/device-visit-location/vwip/…`.
//!
//! Plus [`openapi`], which serves each mounted API's vendored OpenAPI spec at
//! `/{api}/v{n}/openapi.yaml` (docs/DESIGN.md §9).

pub mod blockchain_public_address;
pub mod call_forwarding_signal;
pub mod carrier_billing;
pub mod connected_network_type;
pub mod connectivity_insights;
pub mod customer_insights;
pub mod device_identifier;
pub mod device_swap;
pub mod device_visit_location;
pub mod device_reachability_status;
pub mod device_roaming_status;
pub mod geofencing_subscriptions;
pub mod home_devices_qod;
pub mod kyc_age_verification;
pub mod kyc_fill_in;
pub mod kyc_match;
pub mod kyc_tenure;
pub mod location_retrieval;
pub mod location_verification;
pub mod number_recycling;
pub mod number_verification;
pub mod one_time_password_sms;
pub mod openapi;
pub mod qos_profiles;
pub mod quality_on_demand;
pub mod region_device_count;
pub mod sim_swap;
pub mod simple_edge_discovery;

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
        .merge(call_forwarding_signal::routes())
        .merge(number_recycling::routes())
        .merge(kyc_age_verification::routes())
        .merge(device_swap::routes())
        .merge(kyc_fill_in::routes())
        .merge(home_devices_qod::routes())
        .merge(qos_profiles::routes())
        .merge(kyc_tenure::routes())
        .merge(blockchain_public_address::routes())
        .merge(simple_edge_discovery::routes())
        .merge(customer_insights::routes())
        .merge(connected_network_type::routes())
        .merge(connectivity_insights::routes())
        .merge(region_device_count::routes())
        .merge(device_visit_location::routes())
        .merge(openapi::routes())
}
