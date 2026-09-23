//! CAMARA **Verified Caller** API.
//!
//! Verified Caller lets a calling party (typically a business / call-centre)
//! *pre-announce* an outbound call to the service platform so the network can
//! prove the call's authenticity to the called party — via an out-of-band SMS
//! or a branded caller display — before it rings. It is an anti-scam / caller-
//! trust signal: the recipient sees a verified brand instead of an unknown or
//! spoofable number, defeating impersonation ("APP") fraud.
//!
//! It is a two-legged, business-facing API: both the `callingParticipant` and
//! the `calledParticipant` are carried in the request body (there is no line
//! subject to fall back to), so — unlike the phone-number-keyed anti-fraud APIs
//! — it has no two-legged/three-legged identifier dance.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`vwip`] — mounted at `/verified-caller/vwip` (CAMARA Verified Caller,
//!   `wip` — no released version yet, so mounted at its canonical `vwip` base
//!   path, mirroring Device Data Volume / Media Streaming Rate / Optimal Edge
//!   Discovery).

pub mod vwip;

use axum::Router;

/// Every mounted version of Verified Caller.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
