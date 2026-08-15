//! Sponsored Data **vwip** (CAMARA Sponsored Data, work-in-progress).
//!
//! Six endpoints:
//! - `POST /sponsored-data/vwip/sponsorship` — start a data-sponsorship session
//!   for a subscriber in a campaign (operationId `startSponsorship`).
//! - `GET /sponsored-data/vwip/sponsorship/{sponsorId}/{campaignId}/{sessionId}/session-status`
//!   — read a started session's live status (operationId `getSessionStatus`).
//! - `DELETE /sponsored-data/vwip/sponsorship/{sponsorId}/{campaignId}/{sessionId}/revoke`
//!   — revoke (evict) a started session (operationId `revokeSponsorship`).
//! - `GET /sponsored-data/vwip/campaign/{sponsorId}/{campaignId}/campaign-status`
//!   — report a campaign's operational state and data balance (operationId
//!   `getCampaignStatus`), derived statelessly from the campaignId.
//! - `GET /sponsored-data/vwip/campaign/{sponsorId}/{campaignId}/active-sponsorships`
//!   — list a campaign's currently-active sessions (operationId
//!   `getActiveSponsorships`).
//! - `POST /sponsored-data/vwip/campaign/{sponsorId}/{campaignId}/alert-subscription`
//!   — subscribe a campaign's webhook to alert notifications (operationId
//!   `configureAlerts`), acknowledged statelessly.
//!
//! ## What it does
//!
//! A sponsoring company names itself (`sponsorId`), a campaign (`campaignId`) and
//! the subscriber to sponsor (`phoneNumber`), optionally bounding the grant with a
//! `dataVolume` (MB) and a `duration` (minutes); the operator answers `201` with a
//! freshly minted, opaque `sessionId` and the granted window:
//!
//! ```json
//! {
//!   "sponsorId": "acme@sponsor.example.com",
//!   "campaignId": "123e4567-e89b-12d3-a456-426614174000@sponsor.example.com",
//!   "sessionId": "…-uuid-…",
//!   "startTime": "2024-06-01T00:00:00Z",
//!   "endTime": "2024-06-01T00:10:00Z",
//!   "sponsoredDataVolume": 50
//! }
//! ```
//!
//! The endpoint requires a valid access token ([`crate::auth::verify::Claims`])
//! carrying the `sponsored-data:sponsorship:create` scope. The upstream `wip`
//! contract declares no `securitySchemes`, so CamaraSim assigns this canonical
//! `<api>:<resource>:<action>` scope (documented in the vendored spec).
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Three independent control planes drive the answer:
//!
//! - **Reserved error suffix (`phoneNumber`).** The sponsored subscriber's
//!   `phoneNumber` is the identifier: if its trailing three digits name a reserved
//!   CAMARA status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`,
//!   `…500`, `…503`) the endpoint answers with that canonical CAMARA error (shared
//!   [`crate::scenarios`]) — e.g. `…422` exercises "the subscriber is not eligible
//!   for sponsorship", `…409` a duplicate active session.
//! - **`dataVolume` (MB).** Optional; when present it must be `1..=1000` (else
//!   `400 OUT_OF_RANGE`) and is echoed as `sponsoredDataVolume`. When omitted the
//!   campaign's onboarding default ([`DEFAULT_DATA_VOLUME_MB`]) applies.
//! - **`duration` (minutes).** Optional; when present it must be `1..=1440` (else
//!   `400 OUT_OF_RANGE`) and sets `endTime = startTime + duration`. When omitted
//!   the onboarding default ([`DEFAULT_DURATION_MIN`]) applies.
//!
//! ## Reading a session back — `getSessionStatus`
//!
//! `startSponsorship` now persists the granted session in the shared in-memory
//! [`super::store`] keyed by the minted `sessionId`, so
//! `GET …/{sponsorId}/{campaignId}/{sessionId}/session-status` can read it back.
//! The status view is **derived at read time** from the stored grant (docs/DESIGN
//! §7 — the input is the control plane):
//!
//! - **Store state (`sessionId`).** An unknown `sessionId` → `404 NOT_FOUND`; and
//!   a session found but whose stored `sponsorId`/`campaignId` don't match the
//!   path segments → `404 NOT_FOUND` (the session isn't addressable under that
//!   sponsor/campaign).
//! - **`phoneNumber` tail → data consumption.** The stored subscriber's phone
//!   number is a second control plane: its trailing three digits `d` fix
//!   `dataVolumeConsumed = d % (grant + 1)` (`0..=grant`) and
//!   `dataVolumeAvailable = grant − consumed`. So `+123456789012` on the 50 MB
//!   default consumes 12 MB (38 available), while a tail that lands on the grant
//!   consumes it all (0 available → `data_exhausted`, below).
//! - **The granted window → session status.** `now ≥ endTime` →
//!   `sessionStatus:"inactive"` with `endReason:"validity_expired"`; else a
//!   fully-consumed grant → `"inactive"` / `"data_exhausted"`; else `"active"`
//!   (no `endReason`).
//!
//! ## Revoking a session — `revokeSponsorship`
//!
//! `DELETE …/{sponsorId}/{campaignId}/{sessionId}/revoke` (scope
//! `sponsored-data:sponsorship:delete`) **evicts** the addressed session from
//! the store and answers `200` with the revoked window and
//! `requestResult:"successful_revocation"`. Like `getSessionStatus` the opaque
//! `sessionId` is the only control plane (docs/DESIGN.md §7): an unknown id, or a
//! `sponsorId`/`campaignId` not matching the stored session, → `404 NOT_FOUND`
//! (and a mismatch leaves the session in place). Revoke is single-use — a second
//! revoke of the same session `404`s.
//!
//! On a successful revoke the operator fires the **end-of-session webhook**
//! ([`super::notifications`]): a `SessionEndedNotification` carrying
//! `endReason:"session_revoked"` is POSTed to the consumer's recorded
//! `webhookUrl`, authenticated with its `callbackToken` (`Authorization: Bearer`).
//! Delivery is fire-and-forget off the request path (`http://` only — an `https://`
//! webhook is a no-op cut, no TLS client), so the `200` is never delayed. Because
//! revoke evicts the session, `session_revoked` remains unreachable through a
//! *status read* (a revoked session's status `404`s) — the webhook is the only
//! place it surfaces. `not_available` stays documented-but-unreached, and the
//! natural-end webhooks (`validity_expired`/`data_exhausted`) stay deferred (no
//! background expiry worker).
//!
//! ## Campaign status (`getCampaignStatus`)
//!
//! `GET …/campaign/{sponsorId}/{campaignId}/campaign-status` reports a whole
//! campaign's operational state — distinct from a single session. There is no
//! campaign store (the upstream `manageCampaign` CRUD is not modelled), so the
//! status is derived **statelessly** from the `campaignId`'s embedded UUID
//! (docs/DESIGN.md §7): a reserved trailing-digit suffix selects a canonical
//! CAMARA error (`…404` → 404 campaign-not-found), else the trailing three
//! digits `d` fix `campaignType` (`d` even → prepaid, odd → postpaid) and
//! `status` (`(d/2) mod 3` → active / paused / completed) with the matching
//! `completionReason` and data-volume balance (see [`campaign_status_body`]).
//!
//! ## Active sponsorships (`getActiveSponsorships`)
//!
//! `GET …/campaign/{sponsorId}/{campaignId}/active-sponsorships` (scope
//! [`CAMPAIGN_READ_SCOPE`]) lists the campaign's **currently-active** sponsorship
//! sessions as `{ sessionId, phoneNumber }` pairs plus their `totalCount`. Two
//! control planes (docs/DESIGN.md §7): a reserved trailing-digit suffix on the
//! campaignId's embedded UUID selects a canonical CAMARA error (as with
//! `getCampaignStatus`); otherwise the in-memory store is scanned for this
//! `(sponsorId, campaignId)` and filtered to the [`is_active`] sessions (inside
//! their window with data remaining — the same condition `getSessionStatus`
//! reports as `active`). An empty result is `200` with an empty array (a CAMARA
//! list never `404`s).
//!
//! ## Configuring campaign alerts (`configureAlerts`)
//!
//! `POST …/campaign/{sponsorId}/{campaignId}/alert-subscription` (scope
//! [`CAMPAIGN_ALERTS_SCOPE`]) subscribes a sponsor's `webhookUrl` to a campaign's
//! alert notifications — data-volume-threshold, campaign-expiry and
//! data-exhausted events (each an opt-in boolean flag). There is no campaign store
//! and no background worker to fire the alerts, so the subscription is **not
//! persisted**: the operation is a stateless synchronous acknowledgement (like the
//! In-Home `performDeviceAction` / eSIM `profileOperation` legs) that validates the
//! request and answers `200` with `{ sponsorId, campaignId, requestResult }`. The
//! natural-end alert callbacks themselves stay a documented cut, consistent with
//! the deferred `validity_expired`/`data_exhausted` session webhooks above. Two
//! control planes (docs/DESIGN.md §7): a reserved trailing-digit suffix on the
//! campaignId's embedded UUID selects a canonical CAMARA error (as with
//! `getCampaignStatus`); otherwise the request body is validated (a required,
//! non-empty `webhookUrl`; an optional non-empty `callbackToken`; optional boolean
//! alert flags) → `400 INVALID_ARGUMENT` on a malformed request, else `200`. The
//! remaining campaign operation (`manageCampaign`) stays deferred to a later pass.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::notifications;
use super::store::{self, SponsorshipRecord};
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to start a sponsorship (CamaraSim-assigned; see the module docs
/// — the upstream `wip` contract declares no `securitySchemes`).
const CREATE_SCOPE: &str = "sponsored-data:sponsorship:create";
/// Scope required to read a sponsorship session's status (CamaraSim-assigned; the
/// upstream `wip` contract declares no `securitySchemes`).
const READ_SCOPE: &str = "sponsored-data:sponsorship:read";
/// Scope required to revoke a sponsorship session (CamaraSim-assigned; the
/// upstream `wip` contract declares no `securitySchemes`).
const DELETE_SCOPE: &str = "sponsored-data:sponsorship:delete";
/// Scope required to read a campaign's status (CamaraSim-assigned; the upstream
/// `wip` contract declares no `securitySchemes`).
const CAMPAIGN_READ_SCOPE: &str = "sponsored-data:campaign:read";
/// Scope required to subscribe a campaign's alert notifications (CamaraSim-assigned;
/// the upstream `wip` contract declares no `securitySchemes`). A write-shaped
/// operation, so it carries its own `…:campaign:alerts` scope distinct from the
/// read scope above.
const CAMPAIGN_ALERTS_SCOPE: &str = "sponsored-data:campaign:alerts";

/// The sponsored data volume (MB) granted when the request omits `dataVolume` —
/// the campaign's onboarding default (the spec's `50 MB` example).
const DEFAULT_DATA_VOLUME_MB: i64 = 50;
/// The smallest / largest data volume the schema permits (`dataVolume` is
/// `minimum: 1`, `maximum: 1000`).
const MIN_DATA_VOLUME_MB: i64 = 1;
const MAX_DATA_VOLUME_MB: i64 = 1000;

/// The sponsorship duration (minutes) granted when the request omits `duration` —
/// the campaign's onboarding default (the spec's `10 minutes` example).
const DEFAULT_DURATION_MIN: i64 = 10;
/// The smallest / largest duration the schema permits (`duration` is `minimum: 1`,
/// `maximum: 1440`).
const MIN_DURATION_MIN: i64 = 1;
const MAX_DURATION_MIN: i64 = 1440;

/// The contracted data allotment (MB) reported for a **prepaid** campaign — a
/// fixed onboarding figure the used/remaining balance is computed against.
const CAMPAIGN_CONTRACTED_MB: i64 = 1000;
/// How long ago a campaign is reported to have started (before `now`).
const CAMPAIGN_STARTED_AGO_SECS: i64 = 24 * 3600;
/// How far in the future an ongoing (`active`/`paused`) campaign's `endTime` sits.
const CAMPAIGN_ENDS_IN_SECS: i64 = 24 * 3600;
/// How far in the past a `completed` campaign's `endTime` sits.
const CAMPAIGN_ENDED_AGO_SECS: i64 = 3600;

/// Routes for Sponsored Data vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/sponsored-data/vwip/sponsorship", post(start_sponsorship))
        .route(
            "/sponsored-data/vwip/sponsorship/:sponsor_id/:campaign_id/:session_id/session-status",
            get(get_session_status),
        )
        .route(
            "/sponsored-data/vwip/sponsorship/:sponsor_id/:campaign_id/:session_id/revoke",
            delete(revoke_sponsorship),
        )
        .route(
            "/sponsored-data/vwip/campaign/:sponsor_id/:campaign_id/campaign-status",
            get(get_campaign_status),
        )
        .route(
            "/sponsored-data/vwip/campaign/:sponsor_id/:campaign_id/active-sponsorships",
            get(get_active_sponsorships),
        )
        .route(
            "/sponsored-data/vwip/campaign/:sponsor_id/:campaign_id/alert-subscription",
            post(configure_alerts),
        )
}

/// The `startSponsorship` request body.
#[derive(Deserialize)]
struct StartSponsorship {
    #[serde(rename = "sponsorId")]
    sponsor_id: Option<String>,
    #[serde(rename = "campaignId")]
    campaign_id: Option<String>,
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "dataVolume")]
    data_volume: Option<i64>,
    duration: Option<i64>,
    #[serde(rename = "webhookUrl")]
    webhook_url: Option<String>,
    #[serde(rename = "callbackToken")]
    callback_token: Option<String>,
}

/// `POST /sponsored-data/vwip/sponsorship`.
async fn start_sponsorship(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (five required fields); parse strictly.
    let req: StartSponsorship = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid StartSponsorship.", &correlator)
        }
    };

    // Required fields, each with its schema pattern.
    let sponsor_id = match req.sponsor_id.as_deref() {
        Some(s) if is_sponsor_id(s) => s.to_string(),
        Some(_) => {
            return invalid_argument(
                "`sponsorId` must be `local@domain.tld` (e.g. acme@sponsor.example.com).",
                &correlator,
            )
        }
        None => return invalid_argument("`sponsorId` is required.", &correlator),
    };
    let campaign_id = match req.campaign_id.as_deref() {
        Some(c) if is_campaign_id(c) => c.to_string(),
        Some(_) => {
            return invalid_argument(
                "`campaignId` must be `UUID@domain.tld`.",
                &correlator,
            )
        }
        None => return invalid_argument("`campaignId` is required.", &correlator),
    };
    let phone_number = match req.phone_number.as_deref() {
        Some(p) if is_e164(p) => p.to_string(),
        Some(_) => {
            return invalid_argument(
                "`phoneNumber` must be an E.164 number (e.g. +123456789012).",
                &correlator,
            )
        }
        None => return invalid_argument("`phoneNumber` is required.", &correlator),
    };
    let webhook_url = match req.webhook_url.as_deref() {
        Some(u) if !u.is_empty() => u.to_string(),
        Some(_) => return invalid_argument("`webhookUrl` must not be empty.", &correlator),
        None => return invalid_argument("`webhookUrl` is required.", &correlator),
    };
    let callback_token = match req.callback_token.as_deref() {
        Some(t) if is_uuid_v4(t) => t.to_string(),
        Some(_) => {
            return invalid_argument(
                "`callbackToken` must be a version-4 UUID.",
                &correlator,
            )
        }
        None => return invalid_argument("`callbackToken` is required.", &correlator),
    };

    // Optional bounded controls: absent → onboarding default; present → range-checked.
    let data_volume = match req.data_volume {
        None => DEFAULT_DATA_VOLUME_MB,
        Some(v) if (MIN_DATA_VOLUME_MB..=MAX_DATA_VOLUME_MB).contains(&v) => v,
        Some(_) => {
            return out_of_range(
                "`dataVolume` must be between 1 and 1000 MB.",
                &correlator,
            )
        }
    };
    let duration = match req.duration {
        None => DEFAULT_DURATION_MIN,
        Some(d) if (MIN_DURATION_MIN..=MAX_DURATION_MIN).contains(&d) => d,
        Some(_) => {
            return out_of_range(
                "`duration` must be between 1 and 1440 minutes.",
                &correlator,
            )
        }
    };

    // The `phoneNumber` is the identifier and a control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&phone_number) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Mint the session, persist the granted window, and render the grant. The
    // clock is read once so the stored record and the rendered `201` agree.
    let session_id = mint_session_id();
    let now = unix_now();
    let end = now + duration * 60;
    store::insert(
        session_id.clone(),
        SponsorshipRecord {
            sponsor_id: sponsor_id.clone(),
            campaign_id: campaign_id.clone(),
            phone_number: phone_number.clone(),
            start_time: now,
            end_time: end,
            data_volume_mb: data_volume,
            webhook_url,
            callback_token,
        },
    );
    let body = build_response(
        &sponsor_id,
        &campaign_id,
        &session_id,
        now,
        duration,
        data_volume,
    );
    with_correlator((StatusCode::CREATED, Json(body)).into_response(), &correlator)
}

/// Build the `201` sponsorship-session representation. Pure over its inputs (the
/// clock is passed in as `now`) so the granted window is unit-testable exactly.
/// `endTime = now + duration` minutes.
fn build_response(
    sponsor_id: &str,
    campaign_id: &str,
    session_id: &str,
    now: i64,
    duration_min: i64,
    data_volume_mb: i64,
) -> Value {
    json!({
        "sponsorId": sponsor_id,
        "campaignId": campaign_id,
        "sessionId": session_id,
        "startTime": rfc3339_utc(now),
        "endTime": rfc3339_utc(now + duration_min * 60),
        "sponsoredDataVolume": data_volume_mb,
    })
}

/// `GET /sponsored-data/vwip/sponsorship/{sponsorId}/{campaignId}/{sessionId}/session-status`
/// (operationId `getSessionStatus`).
///
/// Reads a started session back from the shared in-memory [`super::store`] and
/// renders its **live** status. Requires a token carrying [`READ_SCOPE`]. The
/// result is driven by the stored grant (docs/DESIGN.md §7 — see the module
/// docs): an unknown `sessionId`, or one whose stored `sponsorId`/`campaignId`
/// don't match the path, → `404 NOT_FOUND`; otherwise `200` with the derived
/// [`session_status_body`]. `x-correlator` is echoed on every response.
async fn get_session_status(
    claims: Claims,
    headers: HeaderMap,
    Path((sponsor_id, campaign_id, session_id)): Path<(String, String, String)>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the read scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the control plane: unknown id, or a mismatched
    // sponsor/campaign, is not addressable → 404 (no reserved-identifier plane on
    // the opaque sessionId; a reserved `phoneNumber` never reached the store).
    let record = match store::get(&session_id) {
        Some(r) if r.sponsor_id == sponsor_id && r.campaign_id == campaign_id => r,
        _ => {
            return with_correlator(
                CamaraError::not_found(
                    "No sponsorship session found for the provided sponsorId, campaignId and sessionId.",
                )
                .into_response(),
                &correlator,
            )
        }
    };

    let body = session_status_body(&session_id, &record, unix_now());
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// The data consumed / available (MB) for a stored session, derived from the
/// grant and the stored `phoneNumber`'s trailing three digits `d` (docs/DESIGN.md
/// §7): `consumed = d % (grant + 1)` (so `0..=grant`), `available = grant −
/// consumed`. Shared by the `session-status` read and the active-sponsorships
/// list so the two derivations never drift. Pure over the record.
fn consumption(record: &SponsorshipRecord) -> (i64, i64) {
    let grant = record.data_volume_mb;
    // `grant` is always `>= 1` (validated at start), so `grant + 1 >= 2`.
    let tail = scenarios::trailing_three_digits(&record.phone_number).unwrap_or(0) as i64;
    let consumed = tail % (grant + 1);
    (consumed, grant - consumed)
}

/// Whether a stored session is still **active** at `now`: inside its granted
/// window and with data remaining. This is exactly the condition under which
/// [`session_status_body`] reports `sessionStatus:"active"`, reused so
/// `getActiveSponsorships` and `getSessionStatus` agree on what "active" means.
fn is_active(record: &SponsorshipRecord, now: i64) -> bool {
    let (_, available) = consumption(record);
    now < record.end_time && available > 0
}

/// Render a started session's live `session-status` view from the stored grant.
/// Pure over its inputs (the clock is passed in as `now`) so every derived figure
/// is unit-testable exactly.
///
/// Two control planes shape the answer (docs/DESIGN.md §7):
/// - the stored `phoneNumber`'s trailing three digits `d` fix
///   `dataVolumeConsumed = d % (grant + 1)` (so `0..=grant`) and
///   `dataVolumeAvailable = grant − consumed`;
/// - the granted window vs `now`, and whether the grant is fully consumed, fix
///   `sessionStatus`: `now ≥ endTime` → `"inactive"` / `validity_expired`; else a
///   fully-consumed grant → `"inactive"` / `data_exhausted`; else `"active"`
///   (no `endReason`).
fn session_status_body(session_id: &str, record: &SponsorshipRecord, now: i64) -> Value {
    let (consumed, available) = consumption(record);

    let (status, end_reason) = if now >= record.end_time {
        ("inactive", Some("validity_expired"))
    } else if available == 0 {
        ("inactive", Some("data_exhausted"))
    } else {
        ("active", None)
    };

    let mut body = json!({
        "sponsorId": record.sponsor_id,
        "campaignId": record.campaign_id,
        "sessionId": session_id,
        "phoneNumber": record.phone_number,
        "startTime": rfc3339_utc(record.start_time),
        "endTime": rfc3339_utc(record.end_time),
        "sessionStatus": status,
        "dataVolumeConsumed": consumed,
        "dataVolumeAvailable": available,
    });
    if let Some(reason) = end_reason {
        body["endReason"] = Value::String(reason.to_string());
    }
    body
}

/// `DELETE /sponsored-data/vwip/sponsorship/{sponsorId}/{campaignId}/{sessionId}/revoke`
/// (operationId `revokeSponsorship`).
///
/// Revokes an active sponsorship, preventing further use of sponsored data for
/// the subscriber. Requires a token carrying [`DELETE_SCOPE`]. The session is
/// addressed by the opaque `sessionId`, so — exactly like `getSessionStatus` —
/// the store state is the only control plane (docs/DESIGN.md §7): an unknown
/// `sessionId`, or one whose stored `sponsorId`/`campaignId` don't match the
/// path, → `404 NOT_FOUND` (and a mismatch leaves the session in place). A match
/// **evicts** the session (single-use: a second revoke then `404`s) and returns
/// `200` with the revoked window and `requestResult:"successful_revocation"`.
/// `x-correlator` is echoed on every response.
async fn revoke_sponsorship(
    claims: Claims,
    headers: HeaderMap,
    Path((sponsor_id, campaign_id, session_id)): Path<(String, String, String)>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the delete scope.
    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Store state is the control plane: evict an addressable session (single-use);
    // an unknown id, or a sponsor/campaign not matching the stored session, → 404
    // (and a mismatch is left in place — see `store::remove_matching`).
    let record = match store::remove_matching(&session_id, &sponsor_id, &campaign_id) {
        Some(r) => r,
        None => {
            return with_correlator(
                CamaraError::not_found(
                    "No sponsorship session found for the provided sponsorId, campaignId and sessionId.",
                )
                .into_response(),
                &correlator,
            )
        }
    };

    // Fire the end-of-session webhook (fire-and-forget, off the request path): the
    // consumer's recorded `webhookUrl` is notified that the session ended with
    // `endReason: "session_revoked"`, authenticated with its `callbackToken`
    // (`Authorization: Bearer …`). `http://` only (an `https://` webhook is a
    // documented no-op cut — no TLS client). See `super::notifications`.
    if !record.webhook_url.is_empty() {
        let notification = notifications::session_ended_notification(
            &record.sponsor_id,
            &record.campaign_id,
            &session_id,
            &record.phone_number,
            "session_revoked",
            rfc3339_utc(unix_now()),
        );
        let auth = notifications::callback_authorization(&record.callback_token);
        notifications::spawn_delivery(record.webhook_url.clone(), notification, auth);
    }

    let body = revoke_response(&session_id, &record);
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// Build the `200` revocation representation from the evicted grant. Pure over
/// its inputs so the shape is unit-testable exactly. `requestResult` is the
/// upstream's fixed success token `successful_revocation`.
fn revoke_response(session_id: &str, record: &SponsorshipRecord) -> Value {
    json!({
        "sponsorId": record.sponsor_id,
        "campaignId": record.campaign_id,
        "sessionId": session_id,
        "phoneNumber": record.phone_number,
        "startTime": rfc3339_utc(record.start_time),
        "endTime": rfc3339_utc(record.end_time),
        "requestResult": "successful_revocation",
    })
}

/// `GET /sponsored-data/vwip/campaign/{sponsorId}/{campaignId}/campaign-status`
/// (operationId `getCampaignStatus`).
///
/// Reports the operational state of a **campaign** (not a single session):
/// whether it is `active`, `paused` or `completed`, its window, its
/// prepaid/postpaid billing type, and its data-volume balance. Requires a token
/// carrying [`CAMPAIGN_READ_SCOPE`].
///
/// Campaign lifecycle management (the upstream `manageCampaign` operation) is not
/// modelled, so there is no campaign store; the status is derived **statelessly**
/// from the `campaignId`'s embedded UUID (docs/DESIGN.md §7). Two control planes:
/// a reserved trailing-digit suffix on that UUID selects a canonical CAMARA error
/// (e.g. `…404` → `404 NOT_FOUND`, campaign not found); otherwise its trailing
/// three digits derive the campaign state (see [`campaign_status_body`]).
/// Malformed path identifiers → `400 INVALID_ARGUMENT`. `x-correlator` is echoed
/// on every response.
async fn get_campaign_status(
    claims: Claims,
    headers: HeaderMap,
    Path((sponsor_id, campaign_id)): Path<(String, String)>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the campaign read scope.
    if let Err(e) = claims.require_scope(CAMPAIGN_READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path identifiers are the request's only input; both must be well-formed.
    if !is_sponsor_id(&sponsor_id) {
        return invalid_argument(
            "`sponsorId` must be `local@domain.tld` (e.g. acme@sponsor.example.com).",
            &correlator,
        );
    }
    if !is_campaign_id(&campaign_id) {
        return invalid_argument("`campaignId` must be `UUID@domain.tld`.", &correlator);
    }

    // The campaignId's embedded UUID (its `local` part) is the identifier and
    // control plane (docs/DESIGN.md §7): a reserved trailing-digit suffix selects
    // a canonical CAMARA error; otherwise its trailing three digits derive the
    // campaign state. The UUID part is used (not the whole string) so a sponsor
    // domain that happens to carry digits never perturbs the case.
    let uuid_part = campaign_id.split('@').next().unwrap_or(campaign_id.as_str());
    if let Some(err) = scenarios::reserved_error(uuid_part) {
        return with_correlator(err.into_response(), &correlator);
    }
    let d = scenarios::trailing_three_digits(uuid_part).unwrap_or(0) as i64;

    let body = campaign_status_body(&sponsor_id, &campaign_id, d, unix_now());
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// Render a campaign's `campaign-status` view. Pure over its inputs (the clock is
/// passed as `now`, the control digit as `d`) so every derived figure is exactly
/// unit-testable.
///
/// The campaignId's trailing three digits `d` drive two facets (docs/DESIGN.md
/// §7):
/// - **`campaignType`** — `d` even → `prepaid`, odd → `postpaid`.
/// - **`status`** — `(d / 2) mod 3` → `active` / `paused` / `completed`.
///
/// A `completed` campaign carries a `completionReason` (`time_expired`, or
/// `data_exhausted` when `(d / 6)` is odd); an `active`/`paused` one reports
/// `not_available`. Data volumes are in MB: a **prepaid** campaign has a fixed
/// `contractedDataVolume`, an `usedDataVolume`, and the `remainingDataVolume`
/// balance; a **postpaid** campaign reports only `usedDataVolume` (no contracted
/// ceiling). A `data_exhausted` completion has consumed the whole contracted
/// allotment; otherwise `d` MB have been used. The window is anchored to `now`:
/// the campaign started a day ago and — while ongoing — ends a day out, while a
/// `completed` campaign's `endTime` sits an hour in the past.
fn campaign_status_body(sponsor_id: &str, campaign_id: &str, d: i64, now: i64) -> Value {
    let prepaid = d % 2 == 0;
    let status = ["active", "paused", "completed"][((d / 2) % 3) as usize];
    let completion_reason = if status == "completed" {
        if (d / 6) % 2 == 0 {
            "time_expired"
        } else {
            "data_exhausted"
        }
    } else {
        "not_available"
    };

    // A data_exhausted completion has spent the whole contracted allotment;
    // otherwise `d` MB (0..=999, never a reserved suffix) have been used.
    let used = if status == "completed" && completion_reason == "data_exhausted" {
        CAMPAIGN_CONTRACTED_MB
    } else {
        d
    };

    let start = now - CAMPAIGN_STARTED_AGO_SECS;
    let end = if status == "completed" {
        now - CAMPAIGN_ENDED_AGO_SECS
    } else {
        now + CAMPAIGN_ENDS_IN_SECS
    };

    let mut body = json!({
        "sponsorId": sponsor_id,
        "campaignId": campaign_id,
        "status": status,
        "startTime": rfc3339_utc(start),
        "endTime": rfc3339_utc(end),
        "campaignType": if prepaid { "prepaid" } else { "postpaid" },
        "usedDataVolume": used,
        "completionReason": completion_reason,
    });
    // `contractedDataVolume`/`remainingDataVolume` are required only for prepaid.
    if prepaid {
        body["contractedDataVolume"] = json!(CAMPAIGN_CONTRACTED_MB);
        body["remainingDataVolume"] = json!(CAMPAIGN_CONTRACTED_MB - used);
    }
    body
}

/// `GET /sponsored-data/vwip/campaign/{sponsorId}/{campaignId}/active-sponsorships`
/// (operationId `getActiveSponsorships`).
///
/// Lists the sponsorship sessions of the addressed campaign that are **currently
/// active**, as `{ sessionId, phoneNumber }` pairs, plus their `totalCount`.
/// Requires a token carrying [`CAMPAIGN_READ_SCOPE`] (this operation lives under
/// the `/campaign/…` collection, alongside `getCampaignStatus`).
///
/// Two control planes (docs/DESIGN.md §7):
/// - **`campaignId` reserved-error suffix.** As with `getCampaignStatus`, a
///   reserved trailing-digit suffix on the campaignId's embedded UUID selects a
///   canonical CAMARA error (e.g. `…404` → `404 NOT_FOUND`, campaign not found),
///   so the error set stays reachable from this endpoint too.
/// - **Store state (filtered to active).** Otherwise the in-memory store is
///   scanned for sessions under this `(sponsorId, campaignId)` and filtered to the
///   [`is_active`] ones (inside their window with data remaining — the same
///   condition `getSessionStatus` reports as `active`). A campaign with no active
///   sessions returns `200` with an empty array and `totalCount:0` — a CAMARA list
///   never `404`s on an empty result.
///
/// Malformed path identifiers → `400 INVALID_ARGUMENT`. `x-correlator` is echoed
/// on every response.
async fn get_active_sponsorships(
    claims: Claims,
    headers: HeaderMap,
    Path((sponsor_id, campaign_id)): Path<(String, String)>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the campaign read scope.
    if let Err(e) = claims.require_scope(CAMPAIGN_READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path identifiers are the request's only input; both must be well-formed.
    if !is_sponsor_id(&sponsor_id) {
        return invalid_argument(
            "`sponsorId` must be `local@domain.tld` (e.g. acme@sponsor.example.com).",
            &correlator,
        );
    }
    if !is_campaign_id(&campaign_id) {
        return invalid_argument("`campaignId` must be `UUID@domain.tld`.", &correlator);
    }

    // Reserved-error plane on the campaignId's embedded UUID (mirrors
    // getCampaignStatus): the UUID part is used, not the whole string, so a
    // sponsor domain that happens to carry digits never perturbs the case.
    let uuid_part = campaign_id.split('@').next().unwrap_or(campaign_id.as_str());
    if let Some(err) = scenarios::reserved_error(uuid_part) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Scan the store for this campaign's sessions and keep the active ones.
    let now = unix_now();
    let active: Vec<(String, String)> = store::all_matching(&sponsor_id, &campaign_id)
        .into_iter()
        .filter(|(_, r)| is_active(r, now))
        .map(|(id, r)| (id, r.phone_number))
        .collect();

    let body = active_sponsorships_body(&sponsor_id, &campaign_id, active);
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// Build the `200` `getActiveSponsorships` representation from the active
/// `(sessionId, phoneNumber)` pairs. Pure over its inputs so the shape is exactly
/// unit-testable. The pairs are sorted by `sessionId` for a stable response (the
/// store is unordered), and `totalCount` is the number of active sessions.
fn active_sponsorships_body(
    sponsor_id: &str,
    campaign_id: &str,
    mut active: Vec<(String, String)>,
) -> Value {
    active.sort_by(|a, b| a.0.cmp(&b.0));
    let total = active.len();
    let items: Vec<Value> = active
        .into_iter()
        .map(|(session_id, phone_number)| {
            json!({ "sessionId": session_id, "phoneNumber": phone_number })
        })
        .collect();
    json!({
        "sponsorId": sponsor_id,
        "campaignId": campaign_id,
        "activeSponsorships": items,
        "totalCount": total,
    })
}

/// The `configureAlerts` request body — a webhook subscription to a campaign's
/// alert notifications. The upstream `wip` schema marks the request body required
/// but lists no per-field `required` array; CamaraSim tightens `webhookUrl` to
/// required (a subscription needs a destination, mirroring `startSponsorship`) and
/// leaves the rest optional. `callbackToken` authenticates the callbacks (any
/// non-empty string; looser than `startSponsorship`'s v4-UUID pattern, matching
/// this operation's looser upstream schema). The three flags are opt-in booleans.
#[derive(Deserialize)]
struct ConfigureAlerts {
    #[serde(rename = "webhookUrl")]
    webhook_url: Option<String>,
    #[serde(rename = "callbackToken")]
    callback_token: Option<String>,
    #[serde(rename = "alertDataVolumeThresholds")]
    #[allow(dead_code)] // parsed for validation; not persisted (no alert worker).
    alert_data_volume_thresholds: Option<bool>,
    #[serde(rename = "campaignExpiryNotification")]
    #[allow(dead_code)]
    campaign_expiry_notification: Option<bool>,
    #[serde(rename = "dataVolumeExhausted")]
    #[allow(dead_code)]
    data_volume_exhausted: Option<bool>,
}

/// `POST /sponsored-data/vwip/campaign/{sponsorId}/{campaignId}/alert-subscription`
/// (operationId `configureAlerts`).
///
/// Subscribes a campaign's `webhookUrl` to alert notifications. There is no
/// campaign store and no background worker to fire alerts, so nothing is persisted
/// — the operation is a **stateless synchronous acknowledgement** (mirroring the
/// In-Home `performDeviceAction` / eSIM `profileOperation` legs). Requires a token
/// carrying [`CAMPAIGN_ALERTS_SCOPE`].
///
/// Two control planes (docs/DESIGN.md §7):
/// - **`campaignId` reserved-error suffix.** As with `getCampaignStatus`, a
///   reserved trailing-digit suffix on the campaignId's embedded UUID selects a
///   canonical CAMARA error (e.g. `…404` → `404 NOT_FOUND`, campaign not found).
/// - **Request body.** A required, non-empty `webhookUrl`; an optional, non-empty
///   `callbackToken`; optional boolean alert flags. A malformed body or field →
///   `400 INVALID_ARGUMENT`; otherwise `200` with the subscription acknowledgement.
///
/// Body validation runs before the reserved-error plane (a malformed request is a
/// `400` even on a `…404` campaign, mirroring `startSponsorship`). Malformed path
/// identifiers → `400 INVALID_ARGUMENT`. `x-correlator` is echoed on every
/// response.
async fn configure_alerts(
    claims: Claims,
    headers: HeaderMap,
    Path((sponsor_id, campaign_id)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the campaign alerts scope.
    if let Err(e) = claims.require_scope(CAMPAIGN_ALERTS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The path identifiers must be well-formed (mirrors getCampaignStatus).
    if !is_sponsor_id(&sponsor_id) {
        return invalid_argument(
            "`sponsorId` must be `local@domain.tld` (e.g. acme@sponsor.example.com).",
            &correlator,
        );
    }
    if !is_campaign_id(&campaign_id) {
        return invalid_argument("`campaignId` must be `UUID@domain.tld`.", &correlator);
    }

    // Body is mandatory; parse strictly (a non-boolean flag or bad shape → 400).
    let req: ConfigureAlerts = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid alert subscription.",
                &correlator,
            )
        }
    };

    // `webhookUrl` is required and non-empty (a subscription needs a destination).
    match req.webhook_url.as_deref() {
        Some(u) if !u.is_empty() => {}
        Some(_) => return invalid_argument("`webhookUrl` must not be empty.", &correlator),
        None => return invalid_argument("`webhookUrl` is required.", &correlator),
    }
    // `callbackToken` is optional but, when present, must be non-empty.
    if matches!(req.callback_token.as_deref(), Some("")) {
        return invalid_argument("`callbackToken` must not be empty.", &correlator);
    }

    // Reserved-error plane on the campaignId's embedded UUID (mirrors
    // getCampaignStatus): the UUID part is used, not the whole string, so a sponsor
    // domain that happens to carry digits never perturbs the case.
    let uuid_part = campaign_id.split('@').next().unwrap_or(campaign_id.as_str());
    if let Some(err) = scenarios::reserved_error(uuid_part) {
        return with_correlator(err.into_response(), &correlator);
    }

    let body = configure_alerts_body(&sponsor_id, &campaign_id);
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// Build the `200` `configureAlerts` acknowledgement. The canonical response
/// carries only the echoed `sponsorId`/`campaignId` and a fixed `requestResult`
/// success string; pure over its inputs so the shape is exactly unit-testable.
fn configure_alerts_body(sponsor_id: &str, campaign_id: &str) -> Value {
    json!({
        "sponsorId": sponsor_id,
        "campaignId": campaign_id,
        "requestResult": "Campaign notifications subscription - SUCCESS",
    })
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed.
fn out_of_range(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, "OUT_OF_RANGE", message).into_response(),
        correlator,
    )
}

/// Echo the request's `x-correlator` onto a response, if one was supplied.
fn with_correlator(mut response: Response, correlator: &Option<HeaderValue>) -> Response {
    if let Some(value) = correlator {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-correlator"), value.clone());
    }
    response
}

// ---- Field validators (no regex dependency) ---------------------------------

/// E.164: `+`, a leading non-zero digit, then 4–14 more digits (5–15 digits total).
fn is_e164(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = rest.as_bytes();
    if !(5..=15).contains(&bytes.len()) {
        return false;
    }
    if bytes[0] == b'0' {
        return false;
    }
    bytes.iter().all(u8::is_ascii_digit)
}

/// `sponsorId` is `local@domain.tld`: a non-empty local part of `[A-Za-z0-9._%+-]`
/// and a valid domain (see [`is_domain`]).
fn is_sponsor_id(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if local.is_empty()
        || !local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-'))
    {
        return false;
    }
    is_domain(domain)
}

/// `campaignId` is `UUID@domain.tld`: any-version UUID left part, valid domain right.
fn is_campaign_id(s: &str) -> bool {
    let Some((uuid, domain)) = s.split_once('@') else {
        return false;
    };
    is_uuid_any(uuid) && is_domain(domain)
}

/// A DNS-ish domain: only `[A-Za-z0-9.-]`, not bordered by `.`/`-`, containing at
/// least one dot, whose final label is ≥ 2 ASCII letters (the TLD).
fn is_domain(s: &str) -> bool {
    if s.is_empty()
        || s.starts_with(['.', '-'])
        || s.ends_with(['.', '-'])
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
    {
        return false;
    }
    match s.rsplit_once('.') {
        Some((_, tld)) => tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()),
        None => false,
    }
}

/// Whether `s` is a canonical UUID string (8-4-4-4-12 hex with hyphens), any
/// version. Mirrors `network_traffic_analysis::vwip::is_uuid`.
fn is_uuid_any(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(i, &b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_hexdigit(),
    })
}

/// Whether `s` is a version-4 UUID: an [`is_uuid_any`] shape whose version nibble
/// is `4` and whose variant nibble is one of `8`/`9`/`a`/`b` (the `callbackToken`
/// pattern).
fn is_uuid_v4(s: &str) -> bool {
    if !is_uuid_any(s) {
        return false;
    }
    let bytes = s.as_bytes();
    bytes[14] == b'4' && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

// ---- Time + id helpers (self-contained, no date/uuid/rand dependency) --------

/// Mint a fresh, opaque, UUID-v4-shaped `sessionId`.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set (mirrors `quality_on_demand::store::mint_uuid`; no `uuid`/`rand` dep).
fn mint_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update((unix_now() as u64).to_be_bytes());
    let d = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Current Unix time in seconds (server runtime clock; not on any hot loop).
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset, e.g. `2024-06-01T00:00:00Z` (mirrors the sibling NetworkInsights
/// modules; self-contained, no date dependency).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a count of days since 1970-01-01 to a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian, valid for any date).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "sponsored.local:8080";
    const BASE: &str = "/sponsored-data/vwip/sponsorship";
    const SPONSOR: &str = "acme@sponsor.example.com";
    const CAMPAIGN: &str = "123e4567-e89b-12d3-a456-426614174000@sponsor.example.com";
    const CB_TOKEN: &str = "550e8400-e29b-41d4-a716-446655440000"; // valid v4 UUID
    const WEBHOOK: &str = "https://sponsor.example.com/webhook";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn e164_validation() {
        assert!(is_e164("+123456789012"));
        assert!(is_e164("+12345")); // 5 digits (minimum)
        assert!(!is_e164("123456789012")); // no +
        assert!(!is_e164("+0123456789")); // leading zero
        assert!(!is_e164("+1234")); // 4 digits (too short)
        assert!(!is_e164("+12345678901234567")); // 17 digits (too long)
        assert!(!is_e164("+1234abc890")); // non-digit
    }

    #[test]
    fn sponsor_and_campaign_id_validation() {
        assert!(is_sponsor_id("acme@sponsor.example.com"));
        assert!(is_sponsor_id("ID_1%2+3@a.io"));
        assert!(!is_sponsor_id("no-at-sign.com"));
        assert!(!is_sponsor_id("@sponsor.example.com")); // empty local
        assert!(!is_sponsor_id("a@localhost")); // no TLD dot

        assert!(is_campaign_id(CAMPAIGN));
        assert!(!is_campaign_id("not-a-uuid@sponsor.example.com"));
        assert!(!is_campaign_id("123e4567-e89b-12d3-a456-426614174000")); // no domain
    }

    #[test]
    fn uuid_v4_validation() {
        assert!(is_uuid_v4(CB_TOKEN));
        assert!(is_uuid_v4("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_uuid_v4("550e8400-e29b-11d4-a716-446655440000")); // version 1
        assert!(!is_uuid_v4("550e8400-e29b-41d4-c716-446655440000")); // bad variant
        assert!(!is_uuid_v4("550e8400e29b41d4a716446655440000")); // no hyphens
        // Any-version accepts the campaign UUID (version 1); v4 does not.
        assert!(is_uuid_any("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!is_uuid_v4("123e4567-e89b-12d3-a456-426614174000"));
    }

    #[test]
    fn build_response_sets_the_window_from_duration() {
        // 2024-06-01T00:00:00Z is 1717200000 unix seconds.
        let now = 1_717_200_000;
        let body = build_response(SPONSOR, CAMPAIGN, "sid-1", now, 10, 50);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["sessionId"], "sid-1");
        assert_eq!(body["startTime"], "2024-06-01T00:00:00Z");
        assert_eq!(body["endTime"], "2024-06-01T00:10:00Z"); // +10 min
        assert_eq!(body["sponsoredDataVolume"], 50);
        // A different duration moves only endTime.
        let longer = build_response(SPONSOR, CAMPAIGN, "sid-2", now, 1440, 1000);
        assert_eq!(longer["startTime"], "2024-06-01T00:00:00Z");
        assert_eq!(longer["endTime"], "2024-06-02T00:00:00Z"); // +24 h
    }

    #[test]
    fn session_ids_are_unique_and_uuid_v4_shaped() {
        let a = mint_session_id();
        let b = mint_session_id();
        assert_ne!(a, b);
        assert!(is_uuid_v4(&a), "{a} should be a v4 UUID");
        assert!(is_uuid_v4(&b));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials`, host-pinned so its `aud`
    /// matches the route's audience. Scope granted verbatim.
    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=sd-client&scope={scope}");
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/oauth2/token")
                    .header("host", HOST)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        json["access_token"].as_str().unwrap().to_string()
    }

    /// A well-formed StartSponsorship body for `phone`, with optional volume/duration.
    fn body_for(phone: &str, data_volume: Option<i64>, duration: Option<i64>) -> String {
        let mut b = json!({
            "sponsorId": SPONSOR,
            "campaignId": CAMPAIGN,
            "phoneNumber": phone,
            "webhookUrl": WEBHOOK,
            "callbackToken": CB_TOKEN,
        });
        if let Some(v) = data_volume {
            b["dataVolume"] = json!(v);
        }
        if let Some(d) = duration {
            b["duration"] = json!(d);
        }
        b.to_string()
    }

    async fn post(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(BASE)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Build the `session-status` URL, percent-encoding the `@` in the
    /// `sponsorId`/`campaignId` path segments (the only reserved char they carry).
    fn status_url(sponsor: &str, campaign: &str, session: &str) -> String {
        format!(
            "/sponsored-data/vwip/sponsorship/{}/{}/{}/session-status",
            sponsor.replace('@', "%40"),
            campaign.replace('@', "%40"),
            session,
        )
    }

    /// GET a `session-status` URL, returning the status, headers, and JSON body.
    async fn get_status(
        token: Option<&str>,
        url: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder().method("GET").uri(url).header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Start a session for `phone` and return its minted `sessionId`.
    async fn start_session(token: &str, phone: &str) -> String {
        let (status, _, body) = post(Some(token), &body_for(phone, None, None), None).await;
        assert_eq!(status, StatusCode::CREATED, "start should 201");
        body["sessionId"].as_str().unwrap().to_string()
    }

    /// Build the `revoke` URL, percent-encoding the `@` in the sponsor/campaign
    /// path segments (mirrors `status_url`).
    fn revoke_url(sponsor: &str, campaign: &str, session: &str) -> String {
        format!(
            "/sponsored-data/vwip/sponsorship/{}/{}/{}/revoke",
            sponsor.replace('@', "%40"),
            campaign.replace('@', "%40"),
            session,
        )
    }

    /// DELETE a `revoke` URL, returning the status, headers, and JSON body.
    async fn delete_revoke(
        token: Option<&str>,
        url: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(url)
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn happy_path_returns_201_with_defaults_and_echoes() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = post(Some(&token), &body_for("+123456789012", None, None), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        // Onboarding defaults when volume/duration are omitted.
        assert_eq!(body["sponsoredDataVolume"], DEFAULT_DATA_VOLUME_MB);
        // UUID-shaped sessionId; RFC 3339 Z instants; endTime after startTime.
        assert!(is_uuid_v4(body["sessionId"].as_str().unwrap()));
        assert!(body["startTime"].as_str().unwrap().ends_with('Z'));
        assert!(body["endTime"].as_str().unwrap().ends_with('Z'));
        assert_ne!(body["startTime"], body["endTime"]);
    }

    #[tokio::test]
    async fn explicit_data_volume_is_reflected() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post(Some(&token), &body_for("+123456789012", Some(250), Some(30)), None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["sponsoredDataVolume"], 250);
    }

    #[tokio::test]
    async fn reserved_phone_suffix_selects_a_canonical_camara_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // …404 → NOT_FOUND
        let (status, _, body) = post(Some(&token), &body_for("+123456789404", None, None), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …409 → CONFLICT (duplicate active session)
        let (status, _, body) = post(Some(&token), &body_for("+123456789409", None, None), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "CONFLICT");
        // …422 → SERVICE_NOT_APPLICABLE (subscriber not eligible)
        let (status, _, body) = post(Some(&token), &body_for("+123456789422", None, None), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    #[tokio::test]
    async fn data_volume_out_of_range_is_rejected() {
        let token = mint_token(CREATE_SCOPE).await;
        for v in [0, 1001] {
            let (status, _, body) =
                post(Some(&token), &body_for("+123456789012", Some(v), None), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "dataVolume {v}");
            assert_eq!(body["code"], "OUT_OF_RANGE", "dataVolume {v}");
        }
    }

    #[tokio::test]
    async fn duration_out_of_range_is_rejected() {
        let token = mint_token(CREATE_SCOPE).await;
        for d in [0, 1441] {
            let (status, _, body) =
                post(Some(&token), &body_for("+123456789012", None, Some(d)), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "duration {d}");
            assert_eq!(body["code"], "OUT_OF_RANGE", "duration {d}");
        }
    }

    #[tokio::test]
    async fn missing_required_fields_are_rejected() {
        let token = mint_token(CREATE_SCOPE).await;
        // Each variant drops exactly one required field.
        let full = json!({
            "sponsorId": SPONSOR,
            "campaignId": CAMPAIGN,
            "phoneNumber": "+123456789012",
            "webhookUrl": WEBHOOK,
            "callbackToken": CB_TOKEN,
        });
        for missing in ["sponsorId", "campaignId", "phoneNumber", "webhookUrl", "callbackToken"] {
            let mut b = full.clone();
            b.as_object_mut().unwrap().remove(missing);
            let (status, _, body) = post(Some(&token), &b.to_string(), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "missing {missing}");
            assert_eq!(body["code"], "INVALID_ARGUMENT", "missing {missing}");
        }
    }

    #[tokio::test]
    async fn malformed_fields_are_rejected() {
        let token = mint_token(CREATE_SCOPE).await;
        let cases = [
            ("sponsorId", json!("not-an-email")),
            ("campaignId", json!("not-a-uuid@sponsor.example.com")),
            ("phoneNumber", json!("0123")),
            ("callbackToken", json!("not-a-uuid")),
        ];
        for (field, bad) in cases {
            let mut b = json!({
                "sponsorId": SPONSOR,
                "campaignId": CAMPAIGN,
                "phoneNumber": "+123456789012",
                "webhookUrl": WEBHOOK,
                "callbackToken": CB_TOKEN,
            });
            b[field] = bad;
            let (status, _, body) = post(Some(&token), &b.to_string(), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "bad {field}");
            assert_eq!(body["code"], "INVALID_ARGUMENT", "bad {field}");
        }
    }

    #[tokio::test]
    async fn bad_json_body_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = post(Some(&token), "{not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post(None, &body_for("+123456789012", None, None), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn wrong_scope_is_permission_denied() {
        let token = mint_token("sponsored-data:something-else").await;
        let (status, _, body) = post(Some(&token), &body_for("+123456789012", None, None), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, _) =
            post(Some(&token), &body_for("+123456789012", None, None), Some("corr-ok")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );
        let (status, headers, _) =
            post(Some(&token), &body_for("+123456789404", None, None), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- getSessionStatus --------------------------------------------------

    /// `session_status_body` derives consumption from the phone tail and status
    /// from the window (a pure unit — the clock is an argument).
    #[test]
    fn session_status_body_derives_consumption_and_status() {
        let base = SponsorshipRecord {
            sponsor_id: SPONSOR.to_string(),
            campaign_id: CAMPAIGN.to_string(),
            phone_number: "+123456789012".to_string(), // tail 012 → 12
            start_time: 1_717_200_000,
            end_time: 1_717_200_600, // +10 min
            data_volume_mb: 50,
            webhook_url: "http://127.0.0.1:1/webhook".to_string(),
            callback_token: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        };

        // Active: now inside the window, tail 12 % 51 = 12 consumed of 50.
        let active = session_status_body("sid-1", &base, 1_717_200_100);
        assert_eq!(active["sessionId"], "sid-1");
        assert_eq!(active["phoneNumber"], "+123456789012");
        assert_eq!(active["startTime"], "2024-06-01T00:00:00Z");
        assert_eq!(active["endTime"], "2024-06-01T00:10:00Z");
        assert_eq!(active["sessionStatus"], "active");
        assert_eq!(active["dataVolumeConsumed"], 12);
        assert_eq!(active["dataVolumeAvailable"], 38);
        assert!(active.get("endReason").is_none(), "active has no endReason");

        // Expired: now past endTime → inactive / validity_expired.
        let expired = session_status_body("sid-1", &base, base.end_time + 1);
        assert_eq!(expired["sessionStatus"], "inactive");
        assert_eq!(expired["endReason"], "validity_expired");

        // Data-exhausted: a phone tail that lands on the grant (50 % 51 = 50) →
        // 0 available, inactive / data_exhausted (while still inside the window).
        let exhausted_rec = SponsorshipRecord {
            phone_number: "+123456789050".to_string(), // tail 050 → 50
            ..base.clone()
        };
        let exhausted = session_status_body("sid-2", &exhausted_rec, 1_717_200_100);
        assert_eq!(exhausted["dataVolumeConsumed"], 50);
        assert_eq!(exhausted["dataVolumeAvailable"], 0);
        assert_eq!(exhausted["sessionStatus"], "inactive");
        assert_eq!(exhausted["endReason"], "data_exhausted");
    }

    #[tokio::test]
    async fn started_session_can_be_read_back() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(READ_SCOPE).await;
        let session = start_session(&create, "+123456789012").await;

        let (status, _, body) =
            get_status(Some(&read), &status_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["sessionId"], session);
        assert_eq!(body["phoneNumber"], "+123456789012");
        // Freshly started (10-min default window) → active, defaults consumed.
        assert_eq!(body["sessionStatus"], "active");
        assert_eq!(body["dataVolumeConsumed"], 12);
        assert_eq!(body["dataVolumeAvailable"], 38);
        assert!(body["startTime"].as_str().unwrap().ends_with('Z'));
        assert!(body["endTime"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn data_exhausted_session_reports_inactive() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(READ_SCOPE).await;
        // tail 050 → 50 consumed of the 50 MB default → 0 available.
        let session = start_session(&create, "+123456789050").await;

        let (status, _, body) =
            get_status(Some(&read), &status_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dataVolumeAvailable"], 0);
        assert_eq!(body["sessionStatus"], "inactive");
        assert_eq!(body["endReason"], "data_exhausted");
    }

    #[tokio::test]
    async fn unknown_session_is_not_found() {
        let read = mint_token(READ_SCOPE).await;
        let url = status_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, _, body) = get_status(Some(&read), &url, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn mismatched_sponsor_or_campaign_is_not_found() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(READ_SCOPE).await;
        let session = start_session(&create, "+123456789012").await;

        // Right session, wrong sponsor → not addressable under that sponsor.
        let wrong_sponsor = status_url("other@sponsor.example.com", CAMPAIGN, &session);
        let (status, _, body) = get_status(Some(&read), &wrong_sponsor, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        // Right session, wrong campaign → likewise 404.
        let other_campaign = "00000000-0000-1000-8000-000000000000@sponsor.example.com";
        let wrong_campaign = status_url(SPONSOR, other_campaign, &session);
        let (status, _, _) = get_status(Some(&read), &wrong_campaign, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn session_status_missing_token_is_unauthenticated() {
        let url = status_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, _, body) = get_status(None, &url, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn session_status_wrong_scope_is_permission_denied() {
        // The create scope does not grant the read operation.
        let token = mint_token(CREATE_SCOPE).await;
        let url = status_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, _, body) = get_status(Some(&token), &url, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn session_status_echoes_x_correlator() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(READ_SCOPE).await;
        let session = start_session(&create, "+123456789012").await;

        // Success path echoes.
        let (status, headers, _) = get_status(
            Some(&read),
            &status_url(SPONSOR, CAMPAIGN, &session),
            Some("corr-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ok")
        );

        // Error (404) path echoes too.
        let url = status_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, headers, _) = get_status(Some(&read), &url, Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- revokeSponsorship -------------------------------------------------

    #[test]
    fn revoke_response_carries_the_window_and_success_result() {
        let record = SponsorshipRecord {
            sponsor_id: SPONSOR.to_string(),
            campaign_id: CAMPAIGN.to_string(),
            phone_number: "+123456789012".to_string(),
            start_time: 1_717_200_000, // 2024-06-01T00:00:00Z
            end_time: 1_717_200_600,   // +10 min
            data_volume_mb: 50,
            webhook_url: "http://127.0.0.1:1/webhook".to_string(),
            callback_token: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        };
        let body = revoke_response("sid-9", &record);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["sessionId"], "sid-9");
        assert_eq!(body["phoneNumber"], "+123456789012");
        assert_eq!(body["startTime"], "2024-06-01T00:00:00Z");
        assert_eq!(body["endTime"], "2024-06-01T00:10:00Z");
        assert_eq!(body["requestResult"], "successful_revocation");
    }

    #[tokio::test]
    async fn revoke_evicts_the_session_and_is_single_use() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(READ_SCOPE).await;
        let del = mint_token(DELETE_SCOPE).await;
        let session = start_session(&create, "+123456789012").await;

        // The session is readable before revoke.
        let (status, _, _) =
            get_status(Some(&read), &status_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK);

        // Revoke → 200 with the revoked window and the success result.
        let (status, _, body) =
            delete_revoke(Some(&del), &revoke_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sessionId"], session);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["phoneNumber"], "+123456789012");
        assert_eq!(body["requestResult"], "successful_revocation");

        // The session is gone: a subsequent status read 404s...
        let (status, _, _) =
            get_status(Some(&read), &status_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "revoked session should be evicted");

        // ...and a second revoke of the same session 404s (single-use).
        let (status, _, _) =
            delete_revoke(Some(&del), &revoke_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn revoke_unknown_session_is_not_found() {
        let del = mint_token(DELETE_SCOPE).await;
        let url = revoke_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, _, body) = delete_revoke(Some(&del), &url, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn revoke_mismatched_sponsor_is_not_found_and_keeps_the_session() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(READ_SCOPE).await;
        let del = mint_token(DELETE_SCOPE).await;
        let session = start_session(&create, "+123456789012").await;

        // A revoke addressed under the wrong sponsor is not found...
        let url = revoke_url("someone-else@sponsor.example.com", CAMPAIGN, &session);
        let (status, _, _) = delete_revoke(Some(&del), &url, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // ...and must NOT have evicted the session — it still reads back.
        let (status, _, _) =
            get_status(Some(&read), &status_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK, "a mismatch must not evict the session");
    }

    #[tokio::test]
    async fn revoke_missing_token_is_unauthenticated() {
        let url = revoke_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, _, _) = delete_revoke(None, &url, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn revoke_wrong_scope_is_permission_denied() {
        // A read-scoped token may not revoke.
        let read = mint_token(READ_SCOPE).await;
        let url = revoke_url(SPONSOR, CAMPAIGN, "8f14e45f-ceea-4e0a-9d1f-2e3c4b5a6d70");
        let (status, _, _) = delete_revoke(Some(&read), &url, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn revoke_echoes_x_correlator() {
        let create = mint_token(CREATE_SCOPE).await;
        let del = mint_token(DELETE_SCOPE).await;
        let session = start_session(&create, "+123456789012").await;

        // Success path echoes.
        let (status, headers, _) = delete_revoke(
            Some(&del),
            &revoke_url(SPONSOR, CAMPAIGN, &session),
            Some("corr-rev-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-rev-ok")
        );

        // Error (404) path echoes too (the session is already revoked).
        let (status, headers, _) = delete_revoke(
            Some(&del),
            &revoke_url(SPONSOR, CAMPAIGN, &session),
            Some("corr-rev-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-rev-err")
        );
    }

    /// Revoking a session started with an `http://` `webhookUrl` fires the
    /// end-of-session webhook to that URL: a POST carrying
    /// `endReason: "session_revoked"`, authenticated with the session's
    /// `callbackToken` (`Authorization: Bearer …`). End-to-end through the router:
    /// start → store → revoke → spawned delivery.
    #[tokio::test]
    async fn revoke_fires_the_end_of_session_webhook() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let create = mint_token(CREATE_SCOPE).await;
        let del = mint_token(DELETE_SCOPE).await;

        // A loopback receiver for the webhook.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let webhook = format!("http://{addr}/wh");

        // Start a session whose webhookUrl points at the receiver.
        let body = json!({
            "sponsorId": SPONSOR,
            "campaignId": CAMPAIGN,
            "phoneNumber": "+123456789012",
            "webhookUrl": webhook,
            "callbackToken": CB_TOKEN,
        })
        .to_string();
        let (status, _, created) = post(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let session = created["sessionId"].as_str().unwrap().to_string();

        // Accept the webhook delivery off to the side (fire-and-forget from revoke).
        let accept = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            sock.read_to_end(&mut buf).await.unwrap();
            String::from_utf8(buf).unwrap()
        });

        // Revoke → 200; the webhook is delivered from the spawned task.
        let (status, _, _) =
            delete_revoke(Some(&del), &revoke_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK);

        let raw = accept.await.unwrap();
        let (head, payload) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(head.starts_with("POST /wh HTTP/1.1\r\n"), "request line: {head}");
        assert!(
            head.contains(&format!("Authorization: Bearer {CB_TOKEN}\r\n")),
            "bearer callbackToken present: {head}"
        );
        let parsed: Value = serde_json::from_str(payload).expect("body is JSON");
        assert_eq!(parsed["sessionId"], session);
        assert_eq!(parsed["sessionStatus"], "inactive");
        assert_eq!(parsed["endReason"], "session_revoked");
        assert_eq!(parsed["phoneNumber"], "+123456789012");
    }

    /// A session started with an `https://` `webhookUrl` still revokes `200` — the
    /// webhook is a no-op cut (no TLS client), so nothing is delivered and the
    /// caller is unaffected.
    #[tokio::test]
    async fn revoke_with_an_https_webhook_still_succeeds() {
        let create = mint_token(CREATE_SCOPE).await;
        let del = mint_token(DELETE_SCOPE).await;
        // The default WEBHOOK const is https:// → the delivery no-ops.
        let session = start_session(&create, "+123456789012").await;
        let (status, _, body) =
            delete_revoke(Some(&del), &revoke_url(SPONSOR, CAMPAIGN, &session), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["requestResult"], "successful_revocation");
    }

    // --- getCampaignStatus -------------------------------------------------

    /// A `campaignId` whose embedded UUID ends in the three (hex) digits `tail`,
    /// so the campaign-status control plane resolves to `tail`. The base is the
    /// shared `CAMPAIGN` UUID `123e4567-…-426614174000`.
    fn campaign_tail(tail: &str) -> String {
        format!("123e4567-e89b-12d3-a456-426614174{tail}@sponsor.example.com")
    }

    /// Build the `campaign-status` URL, percent-encoding the `@` in the
    /// sponsor/campaign path segments (mirrors `status_url`).
    fn campaign_status_url(sponsor: &str, campaign: &str) -> String {
        format!(
            "/sponsored-data/vwip/campaign/{}/{}/campaign-status",
            sponsor.replace('@', "%40"),
            campaign.replace('@', "%40"),
        )
    }

    #[test]
    fn campaign_status_body_derives_facets_from_the_control_digit() {
        // 2024-06-01T00:00:00Z; started 24 h earlier, ongoing ends 24 h later.
        let now = 1_717_200_000;
        let start = "2024-05-31T00:00:00Z";
        let ongoing_end = "2024-06-02T00:00:00Z";
        let completed_end = "2024-05-31T23:00:00Z"; // now - 1 h

        // d = 0: default — an active, prepaid campaign, nothing used.
        let b = campaign_status_body(SPONSOR, CAMPAIGN, 0, now);
        assert_eq!(b["sponsorId"], SPONSOR);
        assert_eq!(b["campaignId"], CAMPAIGN);
        assert_eq!(b["status"], "active");
        assert_eq!(b["campaignType"], "prepaid");
        assert_eq!(b["completionReason"], "not_available");
        assert_eq!(b["usedDataVolume"], 0);
        assert_eq!(b["contractedDataVolume"], 1000);
        assert_eq!(b["remainingDataVolume"], 1000);
        assert_eq!(b["startTime"], start);
        assert_eq!(b["endTime"], ongoing_end);

        // d = 2: prepaid, (2/2)%3 = 1 → paused; 2 MB used, 998 remaining.
        let b = campaign_status_body(SPONSOR, CAMPAIGN, 2, now);
        assert_eq!(b["status"], "paused");
        assert_eq!(b["campaignType"], "prepaid");
        assert_eq!(b["completionReason"], "not_available");
        assert_eq!(b["usedDataVolume"], 2);
        assert_eq!(b["remainingDataVolume"], 998);
        assert_eq!(b["endTime"], ongoing_end);

        // d = 4: prepaid, completed, (4/6) even → time_expired; partial usage.
        let b = campaign_status_body(SPONSOR, CAMPAIGN, 4, now);
        assert_eq!(b["status"], "completed");
        assert_eq!(b["completionReason"], "time_expired");
        assert_eq!(b["usedDataVolume"], 4);
        assert_eq!(b["remainingDataVolume"], 996);
        assert_eq!(b["endTime"], completed_end);

        // d = 10: prepaid, completed, (10/6) odd → data_exhausted; grant spent.
        let b = campaign_status_body(SPONSOR, CAMPAIGN, 10, now);
        assert_eq!(b["status"], "completed");
        assert_eq!(b["completionReason"], "data_exhausted");
        assert_eq!(b["usedDataVolume"], 1000);
        assert_eq!(b["remainingDataVolume"], 0);

        // d = 3: postpaid (odd) — no contracted/remaining fields, only used.
        let b = campaign_status_body(SPONSOR, CAMPAIGN, 3, now);
        assert_eq!(b["status"], "paused");
        assert_eq!(b["campaignType"], "postpaid");
        assert_eq!(b["usedDataVolume"], 3);
        assert!(b.get("contractedDataVolume").is_none());
        assert!(b.get("remainingDataVolume").is_none());
    }

    #[tokio::test]
    async fn campaign_status_default_is_active_prepaid() {
        let token = mint_token(CAMPAIGN_READ_SCOPE).await;
        let (status, _, body) =
            get_status(Some(&token), &campaign_status_url(SPONSOR, CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["status"], "active");
        assert_eq!(body["campaignType"], "prepaid");
        assert_eq!(body["remainingDataVolume"], 1000);
        assert!(body["startTime"].as_str().unwrap().ends_with('Z'));
        assert!(body["endTime"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn campaign_status_type_and_status_are_controllable() {
        let token = mint_token(CAMPAIGN_READ_SCOPE).await;

        // …002 → paused, prepaid.
        let (status, _, body) =
            get_status(Some(&token), &campaign_status_url(SPONSOR, &campaign_tail("002")), None)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "paused");
        assert_eq!(body["campaignType"], "prepaid");

        // …010 → completed, data_exhausted, remaining 0.
        let (status, _, body) =
            get_status(Some(&token), &campaign_status_url(SPONSOR, &campaign_tail("010")), None)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "completed");
        assert_eq!(body["completionReason"], "data_exhausted");
        assert_eq!(body["usedDataVolume"], 1000);
        assert_eq!(body["remainingDataVolume"], 0);

        // …003 → postpaid: no contracted ceiling reported.
        let (status, _, body) =
            get_status(Some(&token), &campaign_status_url(SPONSOR, &campaign_tail("003")), None)
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["campaignType"], "postpaid");
        assert!(body.get("contractedDataVolume").is_none());
    }

    #[tokio::test]
    async fn campaign_status_reserved_suffix_selects_a_canonical_camara_error() {
        let token = mint_token(CAMPAIGN_READ_SCOPE).await;
        for (tail, code, http) in [
            ("404", "NOT_FOUND", StatusCode::NOT_FOUND),
            ("409", "CONFLICT", StatusCode::CONFLICT),
            ("422", "SERVICE_NOT_APPLICABLE", StatusCode::UNPROCESSABLE_ENTITY),
        ] {
            let (status, _, body) = get_status(
                Some(&token),
                &campaign_status_url(SPONSOR, &campaign_tail(tail)),
                None,
            )
            .await;
            assert_eq!(status, http, "tail {tail}");
            assert_eq!(body["code"], code, "tail {tail}");
        }
    }

    #[tokio::test]
    async fn campaign_status_malformed_ids_are_400() {
        let token = mint_token(CAMPAIGN_READ_SCOPE).await;

        // A malformed sponsorId (no domain) → 400.
        let (status, _, body) =
            get_status(Some(&token), &campaign_status_url("not-an-id", CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");

        // A malformed campaignId (not UUID@domain) → 400.
        let bad_campaign = "not-a-uuid@sponsor.example.com";
        let (status, _, body) =
            get_status(Some(&token), &campaign_status_url(SPONSOR, bad_campaign), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn campaign_status_auth_is_enforced() {
        // No token → 401.
        let (status, _, _) =
            get_status(None, &campaign_status_url(SPONSOR, CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        // A sponsorship-read token does not carry the campaign scope → 403.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) =
            get_status(Some(&read), &campaign_status_url(SPONSOR, CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn campaign_status_echoes_x_correlator() {
        let token = mint_token(CAMPAIGN_READ_SCOPE).await;
        // Success path echoes.
        let (status, headers, _) = get_status(
            Some(&token),
            &campaign_status_url(SPONSOR, CAMPAIGN),
            Some("corr-camp-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-camp-ok")
        );

        // Error (404) path echoes too.
        let (status, headers, _) = get_status(
            Some(&token),
            &campaign_status_url(SPONSOR, &campaign_tail("404")),
            Some("corr-camp-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-camp-err")
        );
    }

    // --- getActiveSponsorships --------------------------------------------

    /// Build the `active-sponsorships` URL, percent-encoding the `@` in the
    /// sponsor/campaign path segments (mirrors `campaign_status_url`).
    fn active_url(sponsor: &str, campaign: &str) -> String {
        format!(
            "/sponsored-data/vwip/campaign/{}/{}/active-sponsorships",
            sponsor.replace('@', "%40"),
            campaign.replace('@', "%40"),
        )
    }

    /// Start a session under an explicit sponsor/campaign/phone, returning its id.
    async fn start_session_for(
        token: &str,
        sponsor: &str,
        campaign: &str,
        phone: &str,
    ) -> String {
        let body = json!({
            "sponsorId": sponsor,
            "campaignId": campaign,
            "phoneNumber": phone,
            "webhookUrl": WEBHOOK,
            "callbackToken": CB_TOKEN,
        })
        .to_string();
        let (status, _, resp) = post(Some(token), &body, None).await;
        assert_eq!(status, StatusCode::CREATED, "start should 201 for {phone}");
        resp["sessionId"].as_str().unwrap().to_string()
    }

    /// `is_active` follows the window and the remaining grant, and `consumption`
    /// derives the MB figures from the phone tail — a pure unit (clock passed in).
    #[test]
    fn is_active_reflects_window_and_remaining_data() {
        let now = 1_717_200_000;
        let mut rec = SponsorshipRecord {
            sponsor_id: SPONSOR.to_string(),
            campaign_id: CAMPAIGN.to_string(),
            phone_number: "+123456789012".to_string(), // tail 12 → 38 available
            start_time: now - 60,
            end_time: now + 600,
            data_volume_mb: 50,
            webhook_url: "http://127.0.0.1:1/webhook".to_string(),
            callback_token: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        };
        assert!(is_active(&rec, now), "inside window with data → active");
        assert!(!is_active(&rec, now + 601), "past endTime → inactive");
        // Fully consumed (tail 050 on a 50 MB grant) → inactive even in-window.
        rec.phone_number = "+123456789050".to_string();
        assert!(!is_active(&rec, now));
        let (consumed, available) = consumption(&rec);
        assert_eq!(consumed, 50);
        assert_eq!(available, 0);
    }

    /// `active_sponsorships_body` sorts by `sessionId`, counts, and renders the
    /// `{ sessionId, phoneNumber }` pairs — a pure unit.
    #[test]
    fn active_sponsorships_body_sorts_and_counts() {
        let active = vec![
            ("sid-b".to_string(), "+123456789013".to_string()),
            ("sid-a".to_string(), "+123456789012".to_string()),
        ];
        let body = active_sponsorships_body(SPONSOR, CAMPAIGN, active);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["totalCount"], 2);
        let items = body["activeSponsorships"].as_array().unwrap();
        assert_eq!(items[0]["sessionId"], "sid-a"); // sorted
        assert_eq!(items[0]["phoneNumber"], "+123456789012");
        assert_eq!(items[1]["sessionId"], "sid-b");
        // Empty input → empty array, totalCount 0 (a list never 404s).
        let empty = active_sponsorships_body(SPONSOR, CAMPAIGN, vec![]);
        assert_eq!(empty["totalCount"], 0);
        assert!(empty["activeSponsorships"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn active_sponsorships_lists_only_active_sessions() {
        let create = mint_token(CREATE_SCOPE).await;
        let read = mint_token(CAMPAIGN_READ_SCOPE).await;
        // A sponsor unused by any other test → the store scan isolates cleanly.
        let sponsor = "active-list@sponsor.example.com";
        let campaign = campaign_tail("012"); // uuid tail 012 → not reserved

        let a1 = start_session_for(&create, sponsor, &campaign, "+123456789012").await;
        let a2 = start_session_for(&create, sponsor, &campaign, "+123456789013").await;
        // Tail 050 on the 50 MB default grant is fully consumed → inactive.
        let _exhausted = start_session_for(&create, sponsor, &campaign, "+123456789050").await;

        let (status, _, body) = get_status(Some(&read), &active_url(sponsor, &campaign), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sponsorId"], sponsor);
        assert_eq!(body["campaignId"], campaign);
        assert_eq!(body["totalCount"], 2, "only the two active sessions");
        let items = body["activeSponsorships"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let ids: Vec<&str> = items
            .iter()
            .map(|i| i["sessionId"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&a1.as_str()));
        assert!(ids.contains(&a2.as_str()));
        let phones: Vec<&str> = items
            .iter()
            .map(|i| i["phoneNumber"].as_str().unwrap())
            .collect();
        assert!(phones.contains(&"+123456789012"));
        assert!(phones.contains(&"+123456789013"));
        assert!(!phones.contains(&"+123456789050"), "exhausted session excluded");
    }

    #[tokio::test]
    async fn active_sponsorships_empty_campaign_is_200_empty_array() {
        let read = mint_token(CAMPAIGN_READ_SCOPE).await;
        let sponsor = "empty-list@sponsor.example.com";
        let campaign = campaign_tail("013");
        let (status, _, body) = get_status(Some(&read), &active_url(sponsor, &campaign), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["totalCount"], 0);
        assert!(body["activeSponsorships"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn active_sponsorships_reserved_suffix_selects_a_canonical_camara_error() {
        let read = mint_token(CAMPAIGN_READ_SCOPE).await;
        for (tail, code, http) in [
            ("404", "NOT_FOUND", StatusCode::NOT_FOUND),
            ("429", "TOO_MANY_REQUESTS", StatusCode::TOO_MANY_REQUESTS),
        ] {
            let (status, _, body) =
                get_status(Some(&read), &active_url(SPONSOR, &campaign_tail(tail)), None).await;
            assert_eq!(status, http, "tail {tail}");
            assert_eq!(body["code"], code, "tail {tail}");
        }
    }

    #[tokio::test]
    async fn active_sponsorships_malformed_ids_are_400() {
        let read = mint_token(CAMPAIGN_READ_SCOPE).await;
        let (status, _, body) = get_status(Some(&read), &active_url("not-an-id", CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let (status, _, body) = get_status(
            Some(&read),
            &active_url(SPONSOR, "not-a-uuid@sponsor.example.com"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn active_sponsorships_auth_is_enforced() {
        // No token → 401.
        let (status, _, _) = get_status(None, &active_url(SPONSOR, CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // A sponsorship-read token lacks the campaign scope → 403.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_status(Some(&read), &active_url(SPONSOR, CAMPAIGN), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn active_sponsorships_echoes_x_correlator() {
        let read = mint_token(CAMPAIGN_READ_SCOPE).await;
        // Success path echoes.
        let (status, headers, _) = get_status(
            Some(&read),
            &active_url("corr-list@sponsor.example.com", &campaign_tail("013")),
            Some("corr-active-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-active-ok")
        );
        // Error (404) path echoes too.
        let (status, headers, _) = get_status(
            Some(&read),
            &active_url(SPONSOR, &campaign_tail("404")),
            Some("corr-active-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-active-err")
        );
    }

    // --- configureAlerts ---------------------------------------------------

    /// Build the `alert-subscription` URL, percent-encoding the `@` in the
    /// sponsor/campaign path segments (mirrors `campaign_status_url`).
    fn alerts_url(sponsor: &str, campaign: &str) -> String {
        format!(
            "/sponsored-data/vwip/campaign/{}/{}/alert-subscription",
            sponsor.replace('@', "%40"),
            campaign.replace('@', "%40"),
        )
    }

    /// POST an arbitrary URL with a JSON body, returning status/headers/JSON (the
    /// module's `post` helper is pinned to the sponsorship BASE path).
    async fn post_url(
        token: Option<&str>,
        url: &str,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(url)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let request = builder.body(Body::from(body.to_string())).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// The `configureAlerts` acknowledgement echoes the ids and a fixed result — a
    /// pure unit.
    #[test]
    fn configure_alerts_body_echoes_ids_and_result() {
        let body = configure_alerts_body(SPONSOR, CAMPAIGN);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["requestResult"], "Campaign notifications subscription - SUCCESS");
    }

    #[tokio::test]
    async fn configure_alerts_happy_path_is_200() {
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let req = json!({ "webhookUrl": WEBHOOK }).to_string();
        let (status, _, body) = post_url(Some(&token), &alerts_url(SPONSOR, CAMPAIGN), &req, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["sponsorId"], SPONSOR);
        assert_eq!(body["campaignId"], CAMPAIGN);
        assert_eq!(body["requestResult"], "Campaign notifications subscription - SUCCESS");
    }

    #[tokio::test]
    async fn configure_alerts_accepts_token_and_optional_flags() {
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let req = json!({
            "webhookUrl": WEBHOOK,
            "callbackToken": CB_TOKEN,
            "alertDataVolumeThresholds": true,
            "campaignExpiryNotification": false,
            "dataVolumeExhausted": true,
        })
        .to_string();
        let (status, _, _) = post_url(Some(&token), &alerts_url(SPONSOR, CAMPAIGN), &req, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn configure_alerts_reserved_suffix_selects_a_canonical_camara_error() {
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let req = json!({ "webhookUrl": WEBHOOK }).to_string();
        for (tail, code, http) in [
            ("404", "NOT_FOUND", StatusCode::NOT_FOUND),
            ("429", "TOO_MANY_REQUESTS", StatusCode::TOO_MANY_REQUESTS),
        ] {
            let (status, _, body) =
                post_url(Some(&token), &alerts_url(SPONSOR, &campaign_tail(tail)), &req, None).await;
            assert_eq!(status, http, "tail {tail}");
            assert_eq!(body["code"], code, "tail {tail}");
        }
    }

    #[tokio::test]
    async fn configure_alerts_bad_body_is_400() {
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let url = alerts_url(SPONSOR, CAMPAIGN);

        // Missing webhookUrl.
        let (status, _, body) = post_url(Some(&token), &url, &json!({}).to_string(), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");

        // Empty webhookUrl.
        let (status, _, _) =
            post_url(Some(&token), &url, &json!({ "webhookUrl": "" }).to_string(), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // Empty callbackToken.
        let (status, _, _) = post_url(
            Some(&token),
            &url,
            &json!({ "webhookUrl": WEBHOOK, "callbackToken": "" }).to_string(),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        // A non-boolean alert flag fails strict parsing.
        let (status, _, _) = post_url(
            Some(&token),
            &url,
            &json!({ "webhookUrl": WEBHOOK, "dataVolumeExhausted": "yes" }).to_string(),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn configure_alerts_bad_body_beats_reserved_suffix() {
        // A malformed body on a …404 campaign is a 400, not the reserved 404.
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let (status, _, body) = post_url(
            Some(&token),
            &alerts_url(SPONSOR, &campaign_tail("404")),
            &json!({}).to_string(),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn configure_alerts_malformed_ids_are_400() {
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let req = json!({ "webhookUrl": WEBHOOK }).to_string();
        let (status, _, body) =
            post_url(Some(&token), &alerts_url("not-an-id", CAMPAIGN), &req, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let (status, _, body) = post_url(
            Some(&token),
            &alerts_url(SPONSOR, "not-a-uuid@sponsor.example.com"),
            &req,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn configure_alerts_auth_is_enforced() {
        let req = json!({ "webhookUrl": WEBHOOK }).to_string();
        // No token → 401.
        let (status, _, _) = post_url(None, &alerts_url(SPONSOR, CAMPAIGN), &req, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        // The campaign *read* scope does not carry the alerts scope → 403.
        let read = mint_token(CAMPAIGN_READ_SCOPE).await;
        let (status, _, _) = post_url(Some(&read), &alerts_url(SPONSOR, CAMPAIGN), &req, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn configure_alerts_echoes_x_correlator() {
        let token = mint_token(CAMPAIGN_ALERTS_SCOPE).await;
        let req = json!({ "webhookUrl": WEBHOOK }).to_string();
        // Success path echoes.
        let (status, headers, _) = post_url(
            Some(&token),
            &alerts_url(SPONSOR, CAMPAIGN),
            &req,
            Some("corr-alerts-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-alerts-ok")
        );
        // Error (404) path echoes too.
        let (status, headers, _) = post_url(
            Some(&token),
            &alerts_url(SPONSOR, &campaign_tail("404")),
            &req,
            Some("corr-alerts-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-alerts-err")
        );
    }
}
