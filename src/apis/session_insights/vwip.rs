//! Session Insights **vwip** (CAMARA SessionInsights, work-in-progress).
//!
//! Five endpoints in this slice — the create/read/delete trio of the session
//! resource, list-by-device, and application-observed metrics submission:
//! - `POST   /session-insights/vwip/sessions` — create a session (`createSession`).
//! - `GET    /session-insights/vwip/sessions/{sessionId}` — read it (`getSession`).
//! - `DELETE /session-insights/vwip/sessions/{sessionId}` — delete it
//!   (`deleteSession`); fires the terminal `session-ended` CloudEvent
//!   (`terminationReason: SESSION_DELETED`) on the session's `sink`.
//! - `POST   /session-insights/vwip/retrieve-sessions` — list a device's sessions
//!   (`retrieveSessionsByDevice`).
//! - `POST   /session-insights/vwip/sessions/{sessionId}/metrics` — submit the
//!   application-observed session metrics (`sendSessionMetrics`).
//!
//! ## `sendSessionMetrics`
//!
//! `POST /sessions/{sessionId}/metrics` lets the application report the network
//! quality it actually observed (a `MetricsPayload` — `packetDelay`, `jitter`,
//! `packetLossErrorRate`, and the optional `upstreamRate`/`downstreamRate`).
//! CAMARA acknowledges receipt with `204 No Content`; the resulting quality score
//! is delivered later via a notification on the session's `sink`, **not** in this
//! response — so CamaraSim validates the payload, confirms the session exists, and
//! answers `204` (it does not persist the metrics — notifications are deferred).
//! Requires the `session-insights:sessions:write` scope. Keyed only on the
//! in-memory store (docs/DESIGN.md §7): an unknown `sessionId` → `404 NOT_FOUND`;
//! a malformed/out-of-range `MetricsPayload` → `400`
//! (`INVALID_ARGUMENT`/`OUT_OF_RANGE`). The spec's `410 Gone` (metrics for an
//! expired session) is a documented cut — no retained expired state exists in this
//! slice (a deleted session is evicted → `404`; expiry transitions arrive with the
//! deferred notifications).
//!
//! (`deleteSession` also delivers the terminal `session-ended` CloudEvent to the
//! session's `sink`; a time-bounded session likewise fires it — with
//! `terminationReason: SESSION_EXPIRED` — when it reaches its `expiresAt`.)
//!
//! ## What it does
//!
//! `POST /sessions` registers interest in the network quality of an application
//! session — a device talking to a named application server — and mints an
//! opaque `id`:
//!
//! ```json
//! {
//!   "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
//!   "applicationSessionId": "game-42",
//!   "device": { "phoneNumber": "+123456789012" },
//!   "applicationServer": { "ipv4Address": "198.51.100.1" },
//!   "sink": "https://callback.example/notify",
//!   "startsAt": "2026-08-05T10:41:38Z",
//!   "expiresAt": "2026-08-06T10:41:38Z",
//!   "status": "ACTIVE"
//! }
//! ```
//!
//! - `status` — one of `ACTIVE` | `EXPIRED` | `DELETED`. A freshly created session
//!   is always `ACTIVE` (the adverse states arrive via the lifecycle endpoints /
//!   notifications, which are deferred).
//! - `startsAt` — always present, the moment of creation (RFC 3339 UTC).
//! - `expiresAt` — present unless the session is open-ended (see the control
//!   planes below).
//!
//! `GET /sessions/{sessionId}` returns the stored representation verbatim (`200`),
//! or `404 NOT_FOUND` for an unknown id.
//!
//! Both endpoints are protected: `POST` requires the
//! `session-insights:sessions:create` scope, `GET` the
//! `session-insights:sessions:read` scope
//! ([`crate::auth::verify::Claims`]).
//!
//! ## Identifier
//!
//! The session is keyed by the CAMARA `Device` — the submitted `device`
//! identifier (phoneNumber, else networkAccessIdentifier, else the IPv4
//! `publicAddress`, else ipv6Address), or, when no `device` is supplied, the
//! access-token subject (three-legged fallback), mirroring Quality on Demand.
//! Neither present → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! - **Reserved error suffix (identifier).** If the resolved identifier's trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`), `POST` answers with that canonical
//!   CAMARA error (shared [`crate::scenarios`]). In particular `…409` drives the
//!   `createSession` `409 CONFLICT` (duplicate session) case and `…422` the
//!   `422 SERVICE_NOT_APPLICABLE`.
//! - **Open-ended vs time-bounded (identifier digits).** Otherwise the session is
//!   `ACTIVE`; its `expiresAt` is present (`startsAt + 24 h`) unless the
//!   identifier's trailing three digits are `…000` (or it has no digits), which
//!   marks an **open-ended** session (no `expiresAt`).
//! - **Network-initiated termination (`…001` identifier).** A session whose
//!   identifier's trailing three digits are `…001` is created `ACTIVE` as usual,
//!   but the (simulated) network drops it early: after a short fixed grace it is
//!   evicted and the **terminal** `session-ended` CloudEvent
//!   (`terminationReason: NETWORK_TERMINATED`) is delivered to the session's
//!   `sink` (fire-and-forget; see [`spawn_network_termination`]).
//! - **Session expiry (time-bounded session).** A session with an `expiresAt`
//!   (any non-`…000`, non-`…001` tail) runs to that instant, then an async expiry
//!   timer ([`spawn_session_expiry`]) evicts it and delivers the **terminal**
//!   `session-ended` CloudEvent (`terminationReason: SESSION_EXPIRED`) to its
//!   `sink`. The three terminal transitions (delete / network-termination /
//!   expiry) are mutually exclusive — exactly one fires per session.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

use super::notifications;
use super::store;

/// Scope required to create a session (CAMARA SessionInsights).
const CREATE_SCOPE: &str = "session-insights:sessions:create";
/// Scope required to read a session (CAMARA SessionInsights).
const READ_SCOPE: &str = "session-insights:sessions:read";
/// Scope required to delete a session (CAMARA SessionInsights).
const DELETE_SCOPE: &str = "session-insights:sessions:delete";
/// Scope required to submit session metrics (CAMARA SessionInsights).
const WRITE_SCOPE: &str = "session-insights:sessions:write";

/// A created session's lifetime when it is time-bounded (24 hours), added to
/// `startsAt` to compute `expiresAt`.
const SESSION_LIFETIME_SECS: i64 = 86_400;

/// The trailing-three-digit identifier tail (`…001`) that selects the
/// network-initiated termination scenario: the session is created `ACTIVE` as
/// usual, but the (simulated) network drops it early (see
/// [`spawn_network_termination`]). Mirrors QoD / QoS Provisioning's `…001`
/// `NETWORK_TERMINATED` convention.
const NETWORK_TERMINATION_TAIL: u16 = 1;

/// How long a `…001` session survives before the (simulated) network drops it —
/// a short, fixed grace, independent of the session's (24 h) lifetime, so the
/// `NETWORK_TERMINATED` transition is distinct from — and fires well before — a
/// `SESSION_EXPIRED` expiry would.
const NETWORK_TERMINATION_GRACE_SECS: u64 = 1;

/// Routes for Session Insights vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route("/session-insights/vwip/sessions", post(create_session))
        .route(
            "/session-insights/vwip/sessions/:session_id",
            get(get_session).delete(delete_session),
        )
        .route(
            "/session-insights/vwip/retrieve-sessions",
            post(retrieve_sessions),
        )
        .route(
            "/session-insights/vwip/sessions/:session_id/metrics",
            post(send_session_metrics),
        )
}

/// `POST /sessions` request body (CAMARA `CreateSession`). `applicationProfileId`,
/// `applicationServer` and `sink` are required; `device` is required only for a
/// two-legged token. Unknown fields are tolerated (the CAMARA request body does
/// not set `additionalProperties: false`).
#[derive(Debug, Deserialize)]
struct CreateSession {
    #[serde(rename = "applicationProfileId")]
    application_profile_id: Option<String>,
    device: Option<Device>,
    /// Accepted as an opaque object (echoed verbatim); validated for at least one
    /// endpoint field below.
    #[serde(rename = "applicationServer")]
    application_server: Option<Value>,
    #[serde(rename = "applicationSessionId")]
    application_session_id: Option<String>,
    sink: Option<String>,
    /// The consumer's callback credential, applied to the session's callbacks'
    /// `Authorization` header: an `ACCESSTOKEN` credential → RFC 6750
    /// `Bearer <accessToken>`, and a `PLAIN` credential → RFC 7617
    /// `Basic base64(identifier:secret)`. The derived header is stashed in a
    /// credential side-store keyed by session id and is **never** echoed in the
    /// response (it carries a secret). `REFRESHTOKEN` is a documented cut.
    #[serde(rename = "sinkCredential")]
    sink_credential: Option<Value>,
}

/// The CAMARA `Device` object: at least one identifier must be present
/// (`minProperties: 1`). CamaraSim keys its functional cases off the first
/// present identifier, in the precedence order below.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Device {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "networkAccessIdentifier")]
    network_access_identifier: Option<String>,
    #[serde(rename = "ipv4Address")]
    ipv4_address: Option<DeviceIpv4Addr>,
    #[serde(rename = "ipv6Address")]
    ipv6_address: Option<String>,
}

/// The CAMARA `DeviceIpv4Addr` object. CamaraSim reads the `publicAddress` as the
/// identifier; the other fields are accepted for schema fidelity.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceIpv4Addr {
    #[serde(rename = "publicAddress")]
    public_address: Option<String>,
    #[serde(rename = "privateAddress")]
    #[allow(dead_code)]
    private_address: Option<String>,
    #[serde(rename = "publicPort")]
    #[allow(dead_code)]
    public_port: Option<i64>,
}

/// `POST /session-insights/vwip/sessions`.
async fn create_session(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry the create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (three required fields); parse.
    let req: CreateSession = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid CreateSession.", &correlator)
        }
    };

    // Required fields.
    match req.application_profile_id.as_deref() {
        Some(id) if !id.is_empty() => {}
        _ => return invalid_argument("`applicationProfileId` is required.", &correlator),
    }
    let application_server = match &req.application_server {
        Some(a) if application_server_has_endpoint(a) => a.clone(),
        Some(_) => {
            return invalid_argument(
                "`applicationServer` must contain `domainName`, `ipv4Address`, and/or `ipv6Address`.",
                &correlator,
            )
        }
        None => return invalid_argument("`applicationServer` is required.", &correlator),
    };
    let sink = match req.application_session_id.as_deref() {
        Some(s) if s.chars().count() > 256 => {
            return invalid_argument(
                "`applicationSessionId` must be at most 256 characters.",
                &correlator,
            )
        }
        _ => match req.sink.as_deref() {
            Some(s) if is_http_uri(s) => s.to_string(),
            Some(_) => {
                return invalid_argument("`sink` must be an http(s) URI.", &correlator)
            }
            None => return invalid_argument("`sink` is required.", &correlator),
        },
    };

    // Resolve the identifier: the submitted device identifier, else the token
    // subject (three-legged fallback). Missing both → 422 MISSING_IDENTIFIER.
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (incl. `…409` → the createSession 409 CONFLICT duplicate-session case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Build the SessionInfo, remember it, and return 201. A session is `ACTIVE`
    // with `startsAt = now`; it is time-bounded (`expiresAt = now + 24 h`) unless
    // the identifier tail is `…000`/no-digits, which marks it open-ended.
    let id = store::new_session_id();
    let now = now_unix_secs();
    let mut info = json!({
        "id": id,
        "applicationServer": application_server,
        "sink": sink.clone(),
        "startsAt": rfc3339_utc(now),
        "status": "ACTIVE",
    });
    if let Some(echo) = resolved.echo {
        info["device"] = echo;
    }
    if let Some(app_session_id) = req.application_session_id {
        info["applicationSessionId"] = json!(app_session_id);
    }
    let tail = scenarios::trailing_three_digits(&resolved.id);
    let open_ended = matches!(tail, None | Some(0));
    let expires_at = now + SESSION_LIFETIME_SECS;
    if !open_ended {
        info["expiresAt"] = json!(rfc3339_utc(expires_at));
    }

    // If the session records an ACCESSTOKEN/PLAIN `sinkCredential`, stash the derived
    // `Authorization` header (Bearer / Basic) in the credential side-store (keyed by
    // session id) so the later `network-quality-score` callback authenticates. Kept
    // apart from the stored `SessionInfo` so `GET` never echoes the secret.
    if let Some(auth) = req
        .sink_credential
        .as_ref()
        .and_then(notifications::sink_authorization)
    {
        store::insert_credential(id.clone(), auth);
    }

    store::insert(id.clone(), info.clone());

    // Schedule the session's terminal `session-ended` transition. Spawned *after*
    // the insert so the timer always sees the stored session; every created session
    // has a `sink` (required above), so there is always somewhere to notify. The
    // three cases are mutually exclusive (exactly one terminal event ever fires):
    // - `…001` → network-initiated termination: the (simulated) network drops the
    //   session early ([`spawn_network_termination`], short fixed grace) →
    //   `terminationReason: NETWORK_TERMINATED`.
    // - any other **time-bounded** tail → the session runs to its `expiresAt`, then
    //   the expiry timer ([`spawn_session_expiry`]) evicts it →
    //   `terminationReason: SESSION_EXPIRED`.
    // - **open-ended** (`…000`/no-digits) → no `expiresAt`, so no timer.
    if tail == Some(NETWORK_TERMINATION_TAIL) {
        spawn_network_termination(id, sink);
    } else if !open_ended {
        spawn_session_expiry(id, sink, expires_at);
    }

    with_correlator((StatusCode::CREATED, Json(info)).into_response(), &correlator)
}

/// Schedule the `NETWORK_TERMINATED` `session-ended` transition for a `…001`
/// session.
///
/// Spawns a fire-and-forget async timer (never on the request path,
/// docs/DESIGN.md §11) that waits [`NETWORK_TERMINATION_GRACE_SECS`] — a short,
/// fixed grace, independent of the session's (24 h) lifetime — then, if the
/// session still exists, evicts it and delivers the **terminal** `session-ended`
/// CloudEvent (`terminationReason: NETWORK_TERMINATED`) to `sink`. This models
/// the network dropping a session *early*, distinct from a `SESSION_EXPIRED`
/// expiry.
///
/// A `deleteSession` that removed the session first makes this a no-op (the
/// concurrent delete already fired `SESSION_DELETED`; `store::remove` is then
/// `None`, so exactly one terminal event fires). The stashed ACCESSTOKEN
/// `sinkCredential` bearer authenticates the callback and is *taken* single-use
/// (the event is terminal). The sleep is async, so the (single-node, in-memory)
/// runtime is never blocked.
fn spawn_network_termination(session_id: String, sink: String) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(NETWORK_TERMINATION_GRACE_SECS)).await;
        // Evict it; if a concurrent delete beat us, `remove` is None and we send
        // nothing (that delete already notified SESSION_DELETED).
        if store::remove(&session_id).is_some() {
            let event = notifications::session_ended_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                &session_id,
                "NETWORK_TERMINATED",
            );
            notifications::spawn_delivery(sink, event, store::take_credential(&session_id));
        }
    });
}

/// Schedule the `SESSION_EXPIRED` `session-ended` transition for a time-bounded
/// session.
///
/// Spawns a fire-and-forget async timer (never on the request path,
/// docs/DESIGN.md §11) that waits until `expires_at` (the session's `expiresAt`,
/// Unix seconds), then — if the session still exists — evicts it and delivers the
/// **terminal** `session-ended` CloudEvent (`terminationReason: SESSION_EXPIRED`)
/// to `sink`. This models the session simply reaching the end of its lifetime,
/// distinct from an early `NETWORK_TERMINATED` drop or a consumer `SESSION_DELETED`.
///
/// A `deleteSession` (or a concurrent `spawn_network_termination`) that removed the
/// session first makes this a no-op (`store::remove` is then `None`, so exactly one
/// terminal event fires). The stashed ACCESSTOKEN `sinkCredential` bearer
/// authenticates the callback and is *taken* single-use (the event is terminal).
/// The sleep is async, so the (single-node, in-memory) runtime is never blocked; a
/// non-positive remaining time (an already-past `expires_at`) fires immediately.
fn spawn_session_expiry(session_id: String, sink: String, expires_at: i64) {
    tokio::spawn(async move {
        let now = now_unix_secs();
        if now < expires_at {
            tokio::time::sleep(std::time::Duration::from_secs((expires_at - now) as u64)).await;
        }
        // Expired. Evict it; if a concurrent delete/network-termination beat us,
        // `remove` is None and we send nothing (that path already fired its terminal
        // event).
        if store::remove(&session_id).is_some() {
            let event = notifications::session_ended_event(
                store::new_event_id(),
                rfc3339_utc(now_unix_secs()),
                &session_id,
                "SESSION_EXPIRED",
            );
            notifications::spawn_delivery(sink, event, store::take_credential(&session_id));
        }
    });
}

/// `GET /session-insights/vwip/sessions/{sessionId}`.
async fn get_session(claims: Claims, headers: HeaderMap, Path(session_id): Path<String>) -> Response {
    let correlator = headers.get("x-correlator").cloned();
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }
    match store::get(&session_id) {
        Some(info) => with_correlator((StatusCode::OK, Json(info)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No session found for the provided sessionId.").into_response(),
            &correlator,
        ),
    }
}

/// `DELETE /session-insights/vwip/sessions/{sessionId}`.
///
/// Deletes the session resource. Keyed only on the stored state (docs/DESIGN.md
/// §7): a session that exists is evicted → `204 No Content` (single-use); an
/// unknown (or already-deleted) id → `404 NOT_FOUND`. If the deleted session
/// recorded a `sink`, the **terminal** `session-ended` CloudEvent
/// (`terminationReason: SESSION_DELETED`) is delivered to it — best-effort,
/// fire-and-forget, so a slow or unreachable sink never delays this response (see
/// [`super::notifications`]). The network-initiated `session-ended` leg
/// (`NETWORK_TERMINATED`) is scheduled from `createSession` for a `…001`
/// identifier ([`spawn_network_termination`]); the session **expiry**
/// (`SESSION_EXPIRED`) leg is scheduled from `createSession` for a time-bounded
/// session ([`spawn_session_expiry`]).
async fn delete_session(
    claims: Claims,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();
    if let Err(e) = claims.require_scope(DELETE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }
    match store::remove(&session_id) {
        Some(info) => {
            // Notify the session's `sink` (if any) that it has ended because the
            // consumer deleted it — the terminal `session-ended` CloudEvent
            // (`terminationReason: SESSION_DELETED`). Fire-and-forget so a slow or
            // unreachable sink never delays this response. The stashed ACCESSTOKEN
            // `sinkCredential` bearer authenticates the callback and is *taken*
            // single-use: `session-ended` is terminal, so no later callback needs
            // it and the secret does not linger in memory.
            match info.get("sink").and_then(Value::as_str) {
                Some(sink) => {
                    let event = notifications::session_ended_event(
                        store::new_event_id(),
                        rfc3339_utc(now_unix_secs()),
                        &session_id,
                        "SESSION_DELETED",
                    );
                    notifications::spawn_delivery(
                        sink.to_string(),
                        event,
                        store::take_credential(&session_id),
                    );
                }
                // No sink to notify; still drop any stashed credential (defensive —
                // createSession always requires a sink).
                None => {
                    let _ = store::take_credential(&session_id);
                }
            }
            with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No session found for the provided sessionId.").into_response(),
            &correlator,
        ),
    }
}

/// `POST /retrieve-sessions` request body (CAMARA `RetrieveSessionsInput`). The
/// `device` is optional — omit it on a three-legged token to select the token
/// subject. Unknown fields are tolerated (the request body does not set
/// `additionalProperties: false`).
#[derive(Debug, Deserialize)]
struct RetrieveSessionsInput {
    device: Option<Device>,
}

/// `POST /session-insights/vwip/retrieve-sessions`.
///
/// Lists a device's session-insights resources as an array of `SessionInfo`
/// (`200`; an empty array when the device has none — CAMARA never 404s on an
/// empty result). The device is the submitted `device` identifier, else the token
/// subject (three-legged fallback; neither present → `422 MISSING_IDENTIFIER`).
/// Requires the `session-insights:sessions:read` scope (a read, like
/// `getSession`). Two control planes (docs/DESIGN.md §7): the identifier — a
/// reserved error suffix selects a canonical CAMARA error (so `…404` → `404
/// NOT_FOUND` for an unknown device) — and, on the happy path, the in-memory
/// store, matched by each session's echoed `device`. A resolved identifier with no
/// `device` echo (a non-E.164 token subject and no submitted device) matches
/// nothing → `200 []`.
async fn retrieve_sessions(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // `device` is optional, so an empty body is accepted as `{}` (three-legged:
    // the device comes from the token subject). A non-empty body must be valid.
    let req: RetrieveSessionsInput = if body.is_empty() {
        RetrieveSessionsInput { device: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid RetrieveSessionsInput.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the identifier (submitted device, else token subject).
    let resolved = match resolve_identifier(req.device, &claims, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Reserved error suffix on the identifier selects a canonical CAMARA error
    // (…404 → 404 NOT_FOUND, the device-identifier-not-found case).
    if let Some(err) = scenarios::reserved_error(&resolved.id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Match stored sessions by their echoed `device`. A resolved identifier with
    // no device echo (a non-E.164 subject, no submitted device) matches nothing.
    let sessions = match resolved.echo {
        Some(echo) => store::find_by_device(&echo),
        None => Vec::new(),
    };

    with_correlator((StatusCode::OK, Json(sessions)).into_response(), &correlator)
}

/// `POST /sessions/{sessionId}/metrics` request body (CAMARA `MetricsPayload`).
/// The three network-quality figures are required; the two throughput figures are
/// optional. Unknown fields are tolerated (the CAMARA schema does not set
/// `additionalProperties: false`).
#[derive(Debug, Deserialize)]
struct MetricsPayload {
    #[serde(rename = "packetDelay")]
    packet_delay: Option<Duration>,
    jitter: Option<Duration>,
    #[serde(rename = "packetLossErrorRate")]
    packet_loss_error_rate: Option<i64>,
    #[serde(rename = "upstreamRate")]
    upstream_rate: Option<Rate>,
    #[serde(rename = "downstreamRate")]
    downstream_rate: Option<Rate>,
}

/// The CAMARA `Duration` object (`packetDelay` / `jitter`): a `value` (1..=500) in
/// a `unit` from [`TIME_UNITS`]. Both are optional in the schema; when present they
/// are range/enum-validated below.
#[derive(Debug, Deserialize)]
struct Duration {
    value: Option<i64>,
    unit: Option<String>,
}

/// The CAMARA `Rate` object (`upstreamRate` / `downstreamRate`): a `value`
/// (0..=1024) in a `unit` from [`RATE_UNITS`].
#[derive(Debug, Deserialize)]
struct Rate {
    value: Option<i64>,
    unit: Option<String>,
}

/// Valid `TimeUnitEnum` values (CAMARA `Duration.unit`).
const TIME_UNITS: [&str; 7] = [
    "Days",
    "Hours",
    "Minutes",
    "Seconds",
    "Milliseconds",
    "Microseconds",
    "Nanoseconds",
];

/// Valid `RateUnitEnum` values (CAMARA `Rate.unit`).
const RATE_UNITS: [&str; 5] = ["Bps", "Kbps", "Mbps", "Gbps", "Tbps"];

/// `POST /session-insights/vwip/sessions/{sessionId}/metrics`.
///
/// Submit the application-observed metrics for a session. CAMARA acknowledges
/// receipt with `204 No Content` — the resulting quality score is delivered later
/// via a notification on the session's `sink`, not in this response — so CamaraSim
/// validates the payload, confirms the session exists, and answers `204` without
/// persisting the metrics (notification delivery is deferred). Keyed only on the
/// in-memory store (docs/DESIGN.md §7): a known `sessionId` + a valid payload →
/// `204`; an unknown/deleted id → `404 NOT_FOUND`; a malformed or out-of-range
/// `MetricsPayload` → `400` (`INVALID_ARGUMENT` / `OUT_OF_RANGE`).
async fn send_session_metrics(
    claims: Claims,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    body: Bytes,
) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    if let Err(e) = claims.require_scope(WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Body is mandatory (three required figures); parse then validate.
    let payload: MetricsPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => {
            return invalid_argument("Request body is not a valid MetricsPayload.", &correlator)
        }
    };
    if let Err(resp) = validate_metrics(&payload, &correlator) {
        return resp;
    }

    // Keyed only on the store state: the session must exist to accept metrics.
    match store::get(&session_id) {
        Some(info) => {
            // CAMARA returns `204` and delivers the resulting quality score
            // *later* on the session's `sink`. Fire-and-forget a synthetic
            // `network-quality-score` CloudEvent — deterministically derived from
            // the submitted metrics (docs/DESIGN.md §7) — so it never sits on the
            // request path. The session's ACCESSTOKEN `sinkCredential` bearer (if
            // any) authenticates the callback.
            deliver_quality_score(&info, &session_id, &payload);
            with_correlator(StatusCode::NO_CONTENT.into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No session found for the provided sessionId.").into_response(),
            &correlator,
        ),
    }
}

/// Fire-and-forget a `network-quality-score` CloudEvent to the session's stored
/// `sink`, derived from the submitted `payload`. A no-op when the stored session
/// records no `sink` (defensive — `createSession` always requires one). Delivery
/// itself is best-effort and non-blocking ([`notifications::spawn_delivery`]);
/// `http://` sinks only (an `https://` sink is a documented no-op cut — no TLS
/// client). The session's ACCESSTOKEN `sinkCredential` bearer (stashed at creation)
/// is *peeked* — non-destructively, since metrics may be submitted repeatedly — and
/// applied to the callback as an `Authorization: Bearer` header; a session with no
/// ACCESSTOKEN credential is delivered unauthenticated.
fn deliver_quality_score(info: &Value, session_id: &str, payload: &MetricsPayload) {
    let Some(sink) = info.get("sink").and_then(Value::as_str) else {
        return;
    };
    let loss = payload.packet_loss_error_rate.unwrap_or(0);
    let delay = payload.packet_delay.as_ref().and_then(|d| d.value).unwrap_or(0);
    let jitter = payload.jitter.as_ref().and_then(|d| d.value).unwrap_or(0);
    let score = notifications::quality_score(loss, delay, jitter);
    let event = notifications::quality_score_event(
        store::new_event_id(),
        rfc3339_utc(now_unix_secs()),
        session_id,
        score,
    );
    let auth = store::peek_credential(session_id);
    notifications::spawn_delivery(sink.to_string(), event, auth);
}

/// Validate a `MetricsPayload`: the three required figures must be present, and
/// every supplied `value`/`unit` must respect the CAMARA range/enum. A structural
/// problem (missing field, bad `unit`) → `400 INVALID_ARGUMENT`; a numeric field
/// outside its allowed range → `400 OUT_OF_RANGE`.
fn validate_metrics(
    payload: &MetricsPayload,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    // Required figures.
    let packet_delay = payload
        .packet_delay
        .as_ref()
        .ok_or_else(|| invalid_argument("`packetDelay` is required.", correlator))?;
    let jitter = payload
        .jitter
        .as_ref()
        .ok_or_else(|| invalid_argument("`jitter` is required.", correlator))?;
    let loss = payload
        .packet_loss_error_rate
        .ok_or_else(|| invalid_argument("`packetLossErrorRate` is required.", correlator))?;

    // packetLossErrorRate is an exponent of 10 in 1..=10.
    if !(1..=10).contains(&loss) {
        return Err(out_of_range(
            "`packetLossErrorRate` must be between 1 and 10.",
            correlator,
        ));
    }

    // The two required Durations, then the two optional Rates.
    validate_duration(packet_delay, "packetDelay", correlator)?;
    validate_duration(jitter, "jitter", correlator)?;
    if let Some(up) = &payload.upstream_rate {
        validate_rate(up, "upstreamRate", correlator)?;
    }
    if let Some(down) = &payload.downstream_rate {
        validate_rate(down, "downstreamRate", correlator)?;
    }
    Ok(())
}

/// Validate a CAMARA `Duration`: `value` (when present) in 1..=500 → else
/// `OUT_OF_RANGE`; `unit` (when present) a valid [`TIME_UNITS`] → else
/// `INVALID_ARGUMENT`.
fn validate_duration(
    d: &Duration,
    field: &str,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    if let Some(v) = d.value {
        if !(1..=500).contains(&v) {
            return Err(out_of_range(
                &format!("`{field}.value` must be between 1 and 500."),
                correlator,
            ));
        }
    }
    if let Some(unit) = &d.unit {
        if !TIME_UNITS.contains(&unit.as_str()) {
            return Err(invalid_argument(
                &format!("`{field}.unit` must be a valid TimeUnitEnum value."),
                correlator,
            ));
        }
    }
    Ok(())
}

/// Validate a CAMARA `Rate`: `value` (when present) in 0..=1024 → else
/// `OUT_OF_RANGE`; `unit` (when present) a valid [`RATE_UNITS`] → else
/// `INVALID_ARGUMENT`.
fn validate_rate(r: &Rate, field: &str, correlator: &Option<HeaderValue>) -> Result<(), Response> {
    if let Some(v) = r.value {
        if !(0..=1024).contains(&v) {
            return Err(out_of_range(
                &format!("`{field}.value` must be between 0 and 1024."),
                correlator,
            ));
        }
    }
    if let Some(unit) = &r.unit {
        if !RATE_UNITS.contains(&unit.as_str()) {
            return Err(invalid_argument(
                &format!("`{field}.unit` must be a valid RateUnitEnum value."),
                correlator,
            ));
        }
    }
    Ok(())
}

/// The resolved device identifier: the string CamaraSim keys its functional cases
/// off, plus the single-property `device` object to echo in the response (`None`
/// when the identifier came from a token subject that is not a phone number).
struct Resolved {
    id: String,
    echo: Option<Value>,
}

/// Resolve the device identifier: the first present identifier in the supplied
/// `device` (validated when it is a `phoneNumber`), else the token subject
/// (three-legged fallback). On failure returns the CAMARA error `Response` to
/// send — 400 `INVALID_ARGUMENT` for a malformed `phoneNumber` or a `device`
/// carrying no identifier, 422 `MISSING_IDENTIFIER` when neither a device nor a
/// token subject is present.
fn resolve_identifier(
    device: Option<Device>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<Resolved, Response> {
    match device {
        Some(device) => match device_identifier(&device) {
            Some(DeviceId::PhoneNumber(phone)) => {
                if !is_valid_e164(&phone) {
                    return Err(invalid_argument(
                        "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                        correlator,
                    ));
                }
                let echo = json!({ "phoneNumber": phone });
                Ok(Resolved { id: phone, echo: Some(echo) })
            }
            Some(DeviceId::Nai(id)) => {
                let echo = json!({ "networkAccessIdentifier": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv4(id)) => {
                let echo = json!({ "ipv4Address": { "publicAddress": id } });
                Ok(Resolved { id, echo: Some(echo) })
            }
            Some(DeviceId::Ipv6(id)) => {
                let echo = json!({ "ipv6Address": id });
                Ok(Resolved { id, echo: Some(echo) })
            }
            None => Err(invalid_argument(
                "`device` must contain at least one identifier.",
                correlator,
            )),
        },
        None => {
            let subject = claims.subject().unwrap_or("");
            if subject.is_empty() {
                return Err(with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "No `device` supplied and the access token identifies no device.",
                    )
                    .into_response(),
                    correlator,
                ));
            }
            let echo = is_valid_e164(subject).then(|| json!({ "phoneNumber": subject }));
            Ok(Resolved { id: subject.to_string(), echo })
        }
    }
}

/// The identifier CamaraSim reads from a `Device`, tagged by kind so the response
/// can echo the right `DeviceResponse` field. Precedence: phoneNumber,
/// networkAccessIdentifier, the IPv4 `publicAddress`, then ipv6Address.
enum DeviceId {
    PhoneNumber(String),
    Nai(String),
    Ipv4(String),
    Ipv6(String),
}

/// The first present identifier of a `Device`, in precedence order, or `None`
/// when the device carries no identifier at all (`minProperties: 1` violated).
fn device_identifier(device: &Device) -> Option<DeviceId> {
    if let Some(phone) = &device.phone_number {
        return Some(DeviceId::PhoneNumber(phone.clone()));
    }
    if let Some(nai) = &device.network_access_identifier {
        return Some(DeviceId::Nai(nai.clone()));
    }
    if let Some(ipv4) = &device.ipv4_address {
        if let Some(addr) = &ipv4.public_address {
            return Some(DeviceId::Ipv4(addr.clone()));
        }
    }
    if let Some(ipv6) = &device.ipv6_address {
        return Some(DeviceId::Ipv6(ipv6.clone()));
    }
    None
}

/// Whether an `applicationServer` object carries at least one endpoint field
/// (`domainName`, `ipv4Address`, or `ipv6Address`), so the server is reachable.
fn application_server_has_endpoint(server: &Value) -> bool {
    server
        .as_object()
        .map(|o| {
            o.contains_key("domainName")
                || o.contains_key("ipv4Address")
                || o.contains_key("ipv6Address")
        })
        .unwrap_or(false)
}

/// Whether `s` is an http(s) URI — a minimal `sink` sanity check.
fn is_http_uri(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Whether `s` is a valid CAMARA `phoneNumber`: E.164 (`+` then 5–15 digits, the
/// first `1`–`9`).
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
}

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed. Used for a
/// `MetricsPayload` numeric field that is present but outside its allowed range.
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

/// Seconds since the Unix epoch, UTC. `SystemTime` never blocks; a clock before
/// the epoch (impossible in practice) falls back to `0`.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as RFC 3339, e.g.
/// `2026-08-05T14:27:08Z`. Second precision. Self-contained (no date-time
/// dependency) via the civil-from-days algorithm below.
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Convert a day count since 1970-01-01 into a `(year, month, day)` civil date
/// (Howard Hinnant's `civil_from_days`, proleptic Gregorian). Month and day are
/// 1-based.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day-of-era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day-of-year [0, 365]
    let mp = (5 * doy + 2) / 153; // month, shifted so March = 0 [0, 11]
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

    const HOST: &str = "si.local:8080";
    const SESSIONS: &str = "/session-insights/vwip/sessions";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn rfc3339_utc_formats_a_known_epoch() {
        // 2026-04-05T10:41:38Z = 1_775_385_698 seconds since the epoch.
        assert_eq!(rfc3339_utc(1_775_385_698), "2026-04-05T10:41:38Z");
    }

    #[test]
    fn e164_validation() {
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789012")); // no +
        assert!(!is_valid_e164("+0123456789")); // leading 0
        assert!(!is_valid_e164("+12")); // too short
    }

    #[test]
    fn http_uri_check() {
        assert!(is_http_uri("https://callback.example/notify"));
        assert!(is_http_uri("http://callback.example/notify"));
        assert!(!is_http_uri("ftp://callback.example"));
        assert!(!is_http_uri("callback.example"));
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=si-client&scope={scope}");
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

    async fn post_session(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(SESSIONS)
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    async fn get_session_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{SESSIONS}/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    async fn delete_session_req(
        token: Option<&str>,
        id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("DELETE")
            .uri(format!("{SESSIONS}/{id}"))
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::empty()).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    async fn retrieve_sessions_req(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/session-insights/vwip/retrieve-sessions")
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    /// A retrieve-sessions body naming `phone` as the device.
    fn retrieve_body(phone: &str) -> String {
        format!(r#"{{"device":{{"phoneNumber":"{phone}"}}}}"#)
    }

    async fn post_metrics(
        token: Option<&str>,
        session_id: &str,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("{SESSIONS}/{session_id}/metrics"))
            .header("host", HOST)
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let response = app()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        collect(response).await
    }

    /// A well-formed `MetricsPayload` body.
    const VALID_METRICS: &str = r#"{"packetDelay":{"value":12,"unit":"Milliseconds"},
             "jitter":{"value":3,"unit":"Milliseconds"},
             "packetLossErrorRate":3,
             "upstreamRate":{"value":10,"unit":"Mbps"},
             "downstreamRate":{"value":50,"unit":"Mbps"}}"#;

    /// Create a session (as `+123456789012`) and return its `id`, for the metrics
    /// tests that need a live session to submit against.
    async fn create_session_id() -> String {
        let (status, _, created) = create_ok("+123456789012").await;
        assert_eq!(status, StatusCode::CREATED);
        created["id"].as_str().unwrap().to_string()
    }

    async fn collect(response: Response) -> (StatusCode, HeaderMap, Value) {
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// A well-formed create body for the given `phoneNumber`.
    fn body_for(phone: &str) -> String {
        format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"{phone}"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "applicationSessionId":"game-42",
                 "sink":"https://callback.example/notify"}}"#
        )
    }

    async fn create_ok(phone: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_session(Some(&token), &body_for(phone), None).await
    }

    // --- Happy path & control planes ---------------------------------------

    #[tokio::test]
    async fn create_returns_201_active_time_bounded_session_echoing_inputs() {
        let (status, _, body) = create_ok("+123456789012").await;
        assert_eq!(status, StatusCode::CREATED);
        // Opaque id, minted.
        assert!(body["id"].as_str().unwrap().contains('-'));
        assert_eq!(body["status"], "ACTIVE");
        // startsAt always present; a non-…000 tail is time-bounded.
        assert!(body["startsAt"].as_str().unwrap().ends_with('Z'));
        assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
        // Inputs echoed.
        assert_eq!(body["device"]["phoneNumber"], "+123456789012");
        assert_eq!(body["applicationServer"]["ipv4Address"], "198.51.100.1");
        assert_eq!(body["applicationSessionId"], "game-42");
        assert_eq!(body["sink"], "https://callback.example/notify");
    }

    #[tokio::test]
    async fn a_zero_tail_identifier_is_open_ended_without_an_expiry() {
        let (status, _, body) = create_ok("+123456789000").await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVE");
        assert!(body["startsAt"].as_str().unwrap().ends_with('Z'));
        assert!(body.get("expiresAt").is_none(), "open-ended → no expiresAt");
    }

    #[tokio::test]
    async fn created_session_can_be_read_back() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_session(Some(&token), &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();

        let read_token = mint_token(READ_SCOPE).await;
        let (status, _, read) = get_session_req(Some(&read_token), id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read, created, "GET returns the stored representation verbatim");
    }

    #[tokio::test]
    async fn get_unknown_session_is_not_found() {
        let token = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            get_session_req(Some(&token), "no-such-session-id", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn no_device_falls_back_to_the_token_subject_open_ended() {
        // A client_credentials subject (`si-client`) has no digits → open-ended,
        // and is not E.164 → no `device` echo.
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "applicationServer":{"domainName":"app.example.com"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "ACTIVE");
        assert!(body.get("expiresAt").is_none());
        assert!(body.get("device").is_none());
    }

    // --- Delete -------------------------------------------------------------

    #[tokio::test]
    async fn create_then_delete_returns_204_and_the_session_is_gone() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, created) =
            post_session(Some(&token), &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // Delete it → 204 No Content, with an empty body.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null, "204 carries no body");

        // …and it can no longer be read back.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_session_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_is_single_use_second_delete_is_not_found() {
        let token = mint_token(CREATE_SCOPE).await;
        let (_, _, created) = post_session(Some(&token), &body_for("+123456789012"), None).await;
        let id = created["id"].as_str().unwrap().to_string();

        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // A second delete of the same id finds nothing.
        let (status, _, body) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_unknown_session_is_not_found() {
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, body) =
            delete_session_req(Some(&del), "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_without_the_scope_is_forbidden() {
        // A read token must not satisfy the delete scope.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = delete_session_req(Some(&read), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn delete_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            delete_session_req(None, "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_delete_204_and_404() {
        // Echoed on the 204 (created-then-deleted)…
        let token = mint_token(CREATE_SCOPE).await;
        let (_, _, created) = post_session(Some(&token), &body_for("+123456789012"), None).await;
        let id = created["id"].as_str().unwrap().to_string();
        let del = mint_token(DELETE_SCOPE).await;
        let (status, headers, _) = delete_session_req(Some(&del), &id, Some("corr-del")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del")
        );
        // …and on the 404 (unknown id).
        let (status, headers, _) =
            delete_session_req(Some(&del), "no-such-id", Some("corr-del-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-del-404")
        );
    }

    #[tokio::test]
    async fn deleting_a_session_with_a_sink_fires_a_session_ended_cloudevent() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        // A loopback receiver stands in for the consumer's `sink`.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/si-notify");

        // Create a session that records the http sink…
        let create = mint_token(CREATE_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"+123456789012"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "sink":"{sink}"}}"#
        );
        let (status, _, created) = post_session(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let session_id = created["id"].as_str().unwrap().to_string();

        // …then delete it: 204 to the caller, and a session-ended CloudEvent to sink.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &session_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // Receive the fire-and-forget notification the handler spawned.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /si-notify HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(
            event["type"],
            "org.camaraproject.session-insights.v0.session-ended"
        );
        assert_eq!(event["specversion"], "1.0");
        assert_eq!(event["datacontenttype"], "application/json");
        assert!(event["id"].is_string() && event["time"].is_string());
        assert_eq!(event["data"]["sessionId"], json!(session_id));
        assert_eq!(event["data"]["terminationReason"], "SESSION_DELETED");
    }

    #[tokio::test]
    async fn a_session_ended_notification_carries_the_sink_credential_bearer() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/si-notify");

        // Create a session with an ACCESSTOKEN sinkCredential…
        let create = mint_token(CREATE_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"+123456789012"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "sink":"{sink}",
                 "sinkCredential":{{"credentialType":"ACCESSTOKEN","accessToken":"cb-secret","accessTokenType":"bearer"}}}}"#
        );
        let (status, _, created) = post_session(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed in the returned representation.
        assert!(
            !serde_json::to_string(&created).unwrap().contains("cb-secret"),
            "sinkCredential secret must never be echoed"
        );
        let session_id = created["id"].as_str().unwrap().to_string();

        // …delete it and confirm the callback carries the bearer.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &session_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        assert!(
            raw.contains("Authorization: Bearer cb-secret\r\n"),
            "session-ended callback carries the sinkCredential bearer: {raw}"
        );
    }

    #[tokio::test]
    async fn a_plain_sink_credential_authenticates_the_callback_as_basic() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/si-notify");

        // Create a session with a PLAIN sinkCredential (identifier/secret)…
        // base64("cbid:cbsecret") == "Y2JpZDpjYnNlY3JldA==".
        let create = mint_token(CREATE_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"+123456789012"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "sink":"{sink}",
                 "sinkCredential":{{"credentialType":"PLAIN","identifier":"cbid","secret":"cbsecret"}}}}"#
        );
        let (status, _, created) = post_session(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        // The secret is never echoed in the returned representation.
        assert!(
            !serde_json::to_string(&created).unwrap().contains("cbsecret"),
            "sinkCredential secret must never be echoed"
        );
        let session_id = created["id"].as_str().unwrap().to_string();

        // …delete it and confirm the callback carries RFC 7617 Basic auth.
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &session_id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        assert!(
            raw.contains("Authorization: Basic Y2JpZDpjYnNlY3JldA==\r\n"),
            "session-ended callback carries the PLAIN sinkCredential as Basic: {raw}"
        );
    }

    #[tokio::test]
    async fn a_001_session_is_network_terminated_early_with_a_session_ended_cloudevent() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        // A loopback receiver stands in for the consumer's `sink`.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/si-notify");

        // Create a session whose identifier ends in `…001` (network-termination
        // scenario) with an ACCESSTOKEN sinkCredential so the callback authenticates.
        let create = mint_token(CREATE_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"+123456789001"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "sink":"{sink}",
                 "sinkCredential":{{"credentialType":"ACCESSTOKEN","accessToken":"nt-secret","accessTokenType":"bearer"}}}}"#
        );
        let (status, _, created) = post_session(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED, "created ACTIVE like any session");
        assert_eq!(created["status"], "ACTIVE");
        let session_id = created["id"].as_str().unwrap().to_string();

        // The (simulated) network drops it early: receive the fire-and-forget
        // terminal `session-ended` CloudEvent the create handler spawned.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /si-notify HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        // The stashed ACCESSTOKEN sinkCredential bearer authenticates the callback.
        assert!(
            head.contains("Authorization: Bearer nt-secret\r\n"),
            "callback carries the sinkCredential bearer: {head}"
        );

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(
            event["type"],
            "org.camaraproject.session-insights.v0.session-ended"
        );
        assert_eq!(event["data"]["sessionId"], json!(session_id));
        assert_eq!(event["data"]["terminationReason"], "NETWORK_TERMINATED");

        // The session was evicted: it is now gone (404 on read).
        let read = mint_token(READ_SCOPE).await;
        let (status, _, _) = get_session_req(Some(&read), &session_id, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "network-terminated → evicted");
    }

    #[tokio::test]
    async fn an_expired_session_fires_session_ended_session_expired_and_is_evicted() {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

        // A loopback receiver stands in for the consumer's `sink`.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let sink = format!("http://{addr}/si-notify");

        // Insert a time-bounded session directly, with an already-past `expiresAt`
        // and an ACCESSTOKEN sinkCredential, then drive the very timer
        // `createSession` schedules ([`spawn_session_expiry`]). A past expiry means
        // it fires immediately, so the fixed 24 h lifetime need not elapse in the
        // test — the same code path a live 24 h session takes at its `expiresAt`.
        let id = store::new_session_id();
        let now = now_unix_secs();
        let info = json!({
            "id": id,
            "status": "ACTIVE",
            "device": { "phoneNumber": "+123456789012" },
            "sink": sink,
            "startsAt": rfc3339_utc(now - 100),
            "expiresAt": rfc3339_utc(now - 10),
        });
        store::insert(id.clone(), info);
        store::insert_credential(id.clone(), "Bearer ex-secret".to_string());

        spawn_session_expiry(id.clone(), sink.clone(), now - 10);

        // Receive the fire-and-forget terminal `session-ended` CloudEvent.
        let (mut sock, _) = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            listener.accept(),
        )
        .await
        .expect("expiry callback arrives")
        .unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, event_body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.starts_with("POST /si-notify HTTP/1.1\r\n"),
            "request line: {head}"
        );
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        // The stashed ACCESSTOKEN sinkCredential bearer authenticates the callback.
        assert!(
            head.contains("Authorization: Bearer ex-secret\r\n"),
            "callback carries the sinkCredential bearer: {head}"
        );

        let event: Value = serde_json::from_str(event_body).expect("body is JSON");
        assert_eq!(
            event["type"],
            "org.camaraproject.session-insights.v0.session-ended"
        );
        assert_eq!(event["data"]["sessionId"], json!(id));
        assert_eq!(event["data"]["terminationReason"], "SESSION_EXPIRED");

        // The session was evicted at expiry: it is now gone.
        assert!(store::get(&id).is_none(), "expired → evicted from the store");
    }

    // --- Retrieve sessions by device ---------------------------------------

    #[tokio::test]
    async fn retrieve_sessions_returns_only_the_requested_devices_sessions() {
        // Two distinct devices; create one session for each.
        let dev_a = "+15550170101";
        let dev_b = "+15550170202";
        let create = mint_token(CREATE_SCOPE).await;
        let (status, _, made_a) = post_session(Some(&create), &body_for(dev_a), None).await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, _, _made_b) = post_session(Some(&create), &body_for(dev_b), None).await;
        assert_eq!(status, StatusCode::CREATED);

        // Retrieving device A's sessions returns only A's.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) = retrieve_sessions_req(Some(&read), &retrieve_body(dev_a), None).await;
        assert_eq!(status, StatusCode::OK);
        let arr = list.as_array().expect("array");
        assert!(!arr.is_empty(), "device A has at least one session");
        assert!(
            arr.iter().all(|s| s["device"]["phoneNumber"] == dev_a),
            "only device A's sessions are listed"
        );
        // The created session is present.
        let created_id = made_a["id"].as_str().unwrap();
        assert!(arr.iter().any(|s| s["id"] == created_id));
    }

    #[tokio::test]
    async fn retrieve_sessions_for_a_device_with_none_is_empty_array() {
        // A device with no created sessions → 200 [] (never 404).
        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550179988"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn retrieve_sessions_reserved_suffix_selects_a_camara_error() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550170404"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550170429"), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn retrieve_sessions_with_a_non_e164_subject_and_no_device_is_empty() {
        // An empty body → device from the token subject; a client_credentials
        // subject (`si-client`) is not E.164, so it has no device echo → 200 [].
        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) = retrieve_sessions_req(Some(&read), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list, json!([]));
    }

    #[tokio::test]
    async fn retrieve_sessions_rejects_a_bad_body() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = retrieve_sessions_req(Some(&read), "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn retrieve_sessions_requires_the_read_scope() {
        let other = mint_token("some:other-scope").await;
        let (status, _, body) =
            retrieve_sessions_req(Some(&other), &retrieve_body("+15550170101"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_sessions_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            retrieve_sessions_req(None, &retrieve_body("+15550170101"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_retrieve_sessions() {
        let read = mint_token(READ_SCOPE).await;
        let (status, headers, _) =
            retrieve_sessions_req(Some(&read), &retrieve_body("+15550179988"), Some("corr-ret"))
                .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ret")
        );
    }

    // --- Send session metrics ----------------------------------------------

    #[tokio::test]
    async fn metrics_for_a_live_session_returns_204_no_content() {
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) = post_metrics(Some(&write), &id, VALID_METRICS, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(body, Value::Null, "204 carries no body");
    }

    #[tokio::test]
    async fn metrics_with_only_the_required_figures_is_accepted() {
        // upstreamRate / downstreamRate are optional.
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        let body = r#"{"packetDelay":{"value":1,"unit":"Seconds"},
                       "jitter":{"value":500},
                       "packetLossErrorRate":1}"#;
        let (status, _, _) = post_metrics(Some(&write), &id, body, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn metrics_deliver_a_quality_score_cloudevent_to_the_session_sink() {
        use tokio::io::AsyncReadExt;

        // A live loopback receiver stands in for the consumer's `sink`.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Create a session whose sink points at the receiver (http:// loopback).
        let create = mint_token(CREATE_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"+123456789012"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "sink":"http://{addr}/notify"}}"#
        );
        let (status, _, created) = post_session(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();

        // Submit metrics; CAMARA answers 204, the score arrives on the sink.
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, _) = post_metrics(Some(&write), &id, VALID_METRICS, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // The fire-and-forget delivery reaches the receiver.
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(head.starts_with("POST /notify HTTP/1.1\r\n"), "request line: {head}");
        assert!(head.contains("Content-Type: application/cloudevents+json"));
        // No sinkCredential on this session → the callback is unauthenticated.
        assert!(!head.contains("Authorization:"), "unauthenticated: {head}");
        let event: Value = serde_json::from_str(body).expect("body is JSON");
        assert_eq!(event["type"], notifications::EVENT_TYPE);
        assert_eq!(event["data"]["sessionId"], id);
        // Deterministic: packetLossErrorRate 3 → 100 − (10−3)*8 = 44.
        assert_eq!(event["data"]["qualityScore"], 44);
    }

    #[tokio::test]
    async fn metrics_callback_carries_the_sink_credential_bearer_and_get_never_echoes_it() {
        use tokio::io::AsyncReadExt;

        // A live loopback receiver stands in for the consumer's `sink`.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Create a session with an http:// sink AND an ACCESSTOKEN sinkCredential.
        let create = mint_token(CREATE_SCOPE).await;
        let body = format!(
            r#"{{"applicationProfileId":"3fa85f64-5717-4562-b3fc-2c963f66afa6",
                 "device":{{"phoneNumber":"+123456789012"}},
                 "applicationServer":{{"ipv4Address":"198.51.100.1"}},
                 "sink":"http://{addr}/notify",
                 "sinkCredential":{{"credentialType":"ACCESSTOKEN",
                                    "accessToken":"si-sink-secret-42",
                                    "accessTokenType":"bearer"}}}}"#
        );
        let (status, _, created) = post_session(Some(&create), &body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap().to_string();
        // The secret is never echoed in the created representation.
        assert!(created.get("sinkCredential").is_none(), "secret not echoed");

        // …nor by a subsequent GET.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, got) = get_session_req(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(got.get("sinkCredential").is_none(), "GET never echoes the secret");

        // Submit metrics; the score callback carries the bearer.
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, _) = post_metrics(Some(&write), &id, VALID_METRICS, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        sock.read_to_end(&mut buf).await.unwrap();
        let raw = String::from_utf8(buf).unwrap();
        let (head, _) = raw.split_once("\r\n\r\n").expect("headers then body");
        assert!(
            head.contains("Authorization: Bearer si-sink-secret-42\r\n"),
            "callback carries the sinkCredential bearer: {head}"
        );
    }

    #[tokio::test]
    async fn metrics_for_an_unknown_session_is_not_found() {
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) = post_metrics(
            Some(&write),
            "11111111-1111-4111-8111-111111111111",
            VALID_METRICS,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn metrics_for_a_deleted_session_is_not_found() {
        // A deleted session is evicted → its metrics endpoint 404s (no retained
        // expired state; the spec's 410 Gone is a documented cut).
        let create = mint_token(CREATE_SCOPE).await;
        let (_, _, created) = post_session(Some(&create), &body_for("+123456789012"), None).await;
        let id = created["id"].as_str().unwrap().to_string();
        let del = mint_token(DELETE_SCOPE).await;
        let (status, _, _) = delete_session_req(Some(&del), &id, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) = post_metrics(Some(&write), &id, VALID_METRICS, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn metrics_missing_a_required_figure_is_invalid_argument() {
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        // No packetLossErrorRate.
        let body = r#"{"packetDelay":{"value":12,"unit":"Milliseconds"},
                       "jitter":{"value":3,"unit":"Milliseconds"}}"#;
        let (status, _, body) = post_metrics(Some(&write), &id, body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn metrics_with_an_out_of_range_value_is_out_of_range() {
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        // packetLossErrorRate 11 (> 10).
        let loss = r#"{"packetDelay":{"value":12},"jitter":{"value":3},
                       "packetLossErrorRate":11}"#;
        let (status, _, b) = post_metrics(Some(&write), &id, loss, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(b["code"], "OUT_OF_RANGE");
        // packetDelay.value 501 (> 500).
        let delay = r#"{"packetDelay":{"value":501},"jitter":{"value":3},
                        "packetLossErrorRate":3}"#;
        let (status, _, b) = post_metrics(Some(&write), &id, delay, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(b["code"], "OUT_OF_RANGE");
        // downstreamRate.value 2000 (> 1024).
        let rate = r#"{"packetDelay":{"value":12},"jitter":{"value":3},
                       "packetLossErrorRate":3,"downstreamRate":{"value":2000,"unit":"Mbps"}}"#;
        let (status, _, b) = post_metrics(Some(&write), &id, rate, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(b["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn metrics_with_a_bad_unit_is_invalid_argument() {
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        let body = r#"{"packetDelay":{"value":12,"unit":"Fortnights"},
                       "jitter":{"value":3},"packetLossErrorRate":3}"#;
        let (status, _, body) = post_metrics(Some(&write), &id, body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn metrics_with_a_malformed_body_is_invalid_argument() {
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) = post_metrics(Some(&write), &id, "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn metrics_bad_body_is_reported_before_session_lookup() {
        // An invalid payload against an unknown session → 400 (validated first),
        // not 404.
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) =
            post_metrics(Some(&write), "no-such-session", "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn metrics_without_the_write_scope_is_forbidden() {
        // A read token must not satisfy the write scope.
        let id = create_session_id().await;
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = post_metrics(Some(&read), &id, VALID_METRICS, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn metrics_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            post_metrics(None, "any-session-id", VALID_METRICS, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_metrics_204_and_404() {
        let id = create_session_id().await;
        let write = mint_token(WRITE_SCOPE).await;
        // Echoed on the 204.
        let (status, headers, _) =
            post_metrics(Some(&write), &id, VALID_METRICS, Some("corr-m")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-m")
        );
        // …and on the 404 (unknown id).
        let (status, headers, _) =
            post_metrics(Some(&write), "no-such-id", VALID_METRICS, Some("corr-m-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-m-404")
        );
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = create_ok("+123456789404").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = create_ok("+123456789409").await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "CONFLICT");

        let (status, _, body) = create_ok("+123456789422").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");

        let (status, _, body) = create_ok("+123456789429").await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn missing_application_profile_id_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationServer":{"ipv4Address":"198.51.100.1"},
                       "device":{"phoneNumber":"+123456789012"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_application_server_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn application_server_without_an_endpoint_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "applicationServer":{"port":443},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_or_malformed_sink_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        // Missing.
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "applicationServer":{"ipv4Address":"198.51.100.1"}}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Malformed (not http(s)).
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"+123456789012"},
                       "applicationServer":{"ipv4Address":"198.51.100.1"},
                       "sink":"ftp://nope"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_phone_number_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let body = r#"{"applicationProfileId":"p-1",
                       "device":{"phoneNumber":"12345"},
                       "applicationServer":{"ipv4Address":"198.51.100.1"},
                       "sink":"https://callback.example/notify"}"#;
        let (status, _, body) = post_session(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_body_is_invalid_argument() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = post_session(Some(&token), "not json", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn create_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_session(Some(&token), &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn read_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_session_req(Some(&token), "any-id", None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_session(None, &body_for("+123456789012"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        // Success.
        let (status, headers, _) =
            post_session(Some(&token), &body_for("+123456789012"), Some("corr-si")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-si")
        );
        // Business error.
        let (status, headers, _) =
            post_session(Some(&token), &body_for("+123456789404"), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
