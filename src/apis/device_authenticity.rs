//! CAMARA **Device Authenticity** API.
//!
//! Device Authenticity answers whether a device — identified by its **IMEI** — is
//! flagged in the register/anti-fraud lists operators maintain: is it `allowed`,
//! or has it been reported `lost`, `stolen`, `blacklisted`, `blocked`, `fraud`,
//! `non-payment`, or `regulatory` (or is the status `unknown`)? It is an
//! operational / anti-fraud signal: before activating a line on a handset, or
//! trusting a device in a transaction, a service can ask the operator whether the
//! IMEI is in good standing.
//!
//! It is **IMEI-keyed**: the device is identified by the `imei` in the request
//! body directly (a 15-digit string). Unlike the phone-number-keyed APIs there is
//! no two-legged / three-legged token-subject fallback — the IMEI is always
//! supplied, so the identifier is unambiguous.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/device-authenticity/vwip` (CAMARA DeviceAuthenticity,
//!   work-in-progress — no released version yet, so mounted at its canonical
//!   `vwip` base path, mirroring the other pre-1.0 wip APIs).

pub mod vwip;

use axum::Router;

/// Every mounted version of Device Authenticity.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
