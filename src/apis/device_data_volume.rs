//! CAMARA **Device Data Volume** API.
//!
//! Device Data Volume lets a client learn, coarsely, how much data a device has
//! consumed — a bucketed category (`<200MiB`/`<1GiB`/`<5GiB`/`>=5GiB`), never a
//! precise byte count. It is device-keyed: the caller either submits a `device`
//! object directly (two-legged / CIBA) or relies on the device identity carried
//! by a three-legged access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/device-data-volume/vwip`. Device Data Volume has no
//!   released version yet (its upstream `main` spec is versioned `wip`), so — like
//!   Device Visit Location / Population Density Data — its canonical base path is
//!   literally `vwip`.
//!
//! Only the `POST /retrieve` operation (`retrieveDataVolume`) is modelled so far;
//! the companion `POST /check` (`checkDataVolume`) is a later slice.

pub mod vwip;

use axum::Router;

/// Every mounted major version of Device Data Volume.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
