//! Sponsored Data **vwip** (CAMARA Sponsored Data, work-in-progress).
//!
//! One endpoint:
//! - `POST /sponsored-data/vwip/sponsorship` — start a data-sponsorship session
//!   for a subscriber in a campaign (operationId `startSponsorship`).
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
//! Session persistence (so a later `session-status`/`revoke` can read the grant)
//! and the end-of-session `webhookUrl` callback are deferred to later passes: the
//! `201` response is fully determined by the request, so it needs no store yet.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// Scope required to start a sponsorship (CamaraSim-assigned; see the module docs
/// — the upstream `wip` contract declares no `securitySchemes`).
const CREATE_SCOPE: &str = "sponsored-data:sponsorship:create";

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

/// Routes for Sponsored Data vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/sponsored-data/vwip/sponsorship",
        post(start_sponsorship),
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
    match req.webhook_url.as_deref() {
        Some(u) if !u.is_empty() => {}
        Some(_) => return invalid_argument("`webhookUrl` must not be empty.", &correlator),
        None => return invalid_argument("`webhookUrl` is required.", &correlator),
    }
    match req.callback_token.as_deref() {
        Some(t) if is_uuid_v4(t) => {}
        Some(_) => {
            return invalid_argument(
                "`callbackToken` must be a version-4 UUID.",
                &correlator,
            )
        }
        None => return invalid_argument("`callbackToken` is required.", &correlator),
    }

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

    // Mint the session and render the grant. No persistence yet (see module docs).
    let session_id = mint_session_id();
    let body = build_response(
        &sponsor_id,
        &campaign_id,
        &session_id,
        unix_now(),
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
}
