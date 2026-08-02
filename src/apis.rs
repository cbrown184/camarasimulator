//! CAMARA business APIs.
//!
//! Each CAMARA API lives in its own module, one submodule per major version
//! (docs/DESIGN.md §5), mounted under its canonical URL `/{api}/v{MAJOR}/…`
//! (§9). [`routes`] composes every mounted API into a single router the
//! top-level app merges.
//!
//! So far:
//! - **Number Verification v1** — `POST /number-verification/v1/verify`.

pub mod number_verification;

use axum::Router;

/// Every mounted CAMARA API's routes, merged into one router.
pub fn routes() -> Router {
    Router::new().merge(number_verification::routes())
}
