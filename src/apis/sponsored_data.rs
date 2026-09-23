//! CAMARA **Sponsored Data** API (work-in-progress).
//!
//! Sponsored Data lets a *sponsoring company* fund an end-user's mobile data —
//! the operator grants a bounded data volume for a bounded time and bills the
//! sponsor, not the user. The upstream CAMARA API exposes a sponsorship
//! lifecycle (`POST /sponsorship`, then session-status / revoke queries) plus
//! campaign-management operations.
//!
//! CamaraSim mounts the sponsorship-session lifecycle first:
//! - [`vwip`] — mounted at `/sponsored-data/vwip` (CAMARA `wip` — no released
//!   version yet, so mounted at its canonical `vwip` base path, like the other
//!   pre-1.0 wip APIs). Serves `POST /sponsorship` (`startSponsorship`) and
//!   `GET /sponsorship/{sponsorId}/{campaignId}/{sessionId}/session-status`
//!   (`getSessionStatus`).
//!
//! Reading a session back makes Sponsored Data **stateful**, so a shared
//! in-memory [`store`] holds started sessions keyed by `sessionId`.
//!
//! `revokeSponsorship` now fires the end-of-session `webhookUrl` callback
//! ([`notifications`]): when a session is revoked, a `SessionEndedNotification`
//! (`endReason: session_revoked`) is POSTed to the consumer's recorded
//! `webhookUrl`, authenticated with its `callbackToken`. The campaign-management
//! operations (`configureAlerts`, `manageCampaign`) remain deferred to later
//! work.

pub mod notifications;
pub mod store;
pub mod vwip;

use axum::Router;

/// Every mounted version of Sponsored Data.
pub fn routes() -> Router {
    Router::new().merge(vwip::routes())
}
