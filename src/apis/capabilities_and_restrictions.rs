//! CAMARA **Capabilities and Restrictions** API.
//!
//! A consumer queries the operator for the CAMARA service capabilities that
//! apply to a given context (a set of `resourceScopes`, e.g. a phone number)
//! against one or more API definitions it `overlayExtends`. The operator answers
//! with a tailored `CapabilityInfo`: for each query, a capability set naming the
//! runtime restrictions the consumer must enforce, plus a bitmap conveying which
//! of those restrictions are currently active.
//!
//! One submodule per version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/capabilities-and-restrictions/vwip` (CAMARA
//!   CapabilitiesAndRuntimeRestrictions, `wip` — no released version yet, so
//!   CamaraSim mounts it at its canonical `vwip` base path, mirroring the other
//!   work-in-progress APIs).

pub mod vwip;

use axum::Router;

/// Every mounted version of Capabilities and Restrictions.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
