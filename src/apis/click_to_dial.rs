//! CAMARA **Click to Dial** API.
//!
//! Click to Dial lets an application place a voice call between two phone
//! numbers on behalf of a user: the network dials the `caller`, then bridges to
//! the `callee`, so a "click" in an app (a contact card, a "call me back"
//! button) becomes a real carrier-originated call without the app ever handling
//! media. The call's progress (`initiating` → `connected` → `disconnected`) is
//! reported back to the application, and — when enabled — the call can be
//! recorded.
//!
//! Like Verified Caller, it is a two-legged, business-facing API: both the
//! `caller` and the `callee` are carried in the request body (there is no line
//! subject to fall back to), so there is no two-legged/three-legged identifier
//! resolution — the two participants are always supplied explicitly.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/click-to-dial/vwip` (CAMARA Click to Dial, `wip` —
//!   no released version yet, so mounted at its canonical `vwip` base path,
//!   mirroring Verified Caller / Sponsored Data). Serves the `createCall`
//!   endpoint; the stateful read/terminate/recording operations are a later
//!   slice.

pub mod vwip;

use axum::Router;

/// Every mounted version of Click to Dial.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
