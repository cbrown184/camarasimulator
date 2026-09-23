//! Network Traffic Analysis **vwip** (CAMARA NetworkInsights, work-in-progress).
//!
//! One endpoint:
//! - `GET /network-traffic-analysis/vwip/traffic-analysis` — aggregated,
//!   service-level traffic statistics for a network over a time window.
//!
//! ## What it does
//!
//! The caller names a network (`networkId`), an analysis window
//! (`startDate`/`endDate`, RFC 3339) and a granularity (`frequency` = `DAY` or
//! `HOUR`), and the operator answers with per-application traffic records —
//! one per application per time slot — plus a pagination envelope:
//!
//! ```json
//! {
//!   "records": [
//!     { "app": "whatsapp", "accessCount": 151, "accessUpFlow": 154624,
//!       "accessDownFlow": 309248, "accessFlow": 463872,
//!       "startDate": "2024-06-01T00:00:00Z", "endDate": "2024-06-02T00:00:00Z",
//!       "accessDate": "2024-06-01", "ipv4Address": "202.112.17.101",
//!       "description": "…" }
//!   ],
//!   "pagination": { "page": 1, "perPage": 20, "totalCount": 1, "totalPages": 1 }
//! }
//! ```
//!
//! `accessFlow` is always `accessUpFlow + accessDownFlow` (the spec's "total").
//! The endpoint requires a valid access token ([`crate::auth::verify::Claims`])
//! carrying the `network-traffic-analysis:traffic-analysis:read` scope. It is a
//! two-legged (`client_credentials`) service query — the data is aggregated
//! network-level only, so there is no device/line identifier and no three-legged
//! dance.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Three independent control planes drive the answer:
//!
//! - **Reserved error suffix (`networkId`).** If the `networkId` UUID's trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with that
//!   canonical CAMARA error (shared [`crate::scenarios`]). A UUID contains
//!   digits, so the convention applies unchanged — e.g. `…-000000000404` selects
//!   `404 NOT_FOUND` (the spec's "networkId does not exist").
//! - **Applications (`networkId` digits).** Otherwise the `networkId`'s trailing
//!   three digits `d` (`000`–`999`) fix how many applications appear:
//!   - `d == 000` (or a `networkId` with fewer than three digits) → **no data**:
//!     an empty `records` array (the spec's no-data `200`, not a `404`).
//!   - otherwise `((d % 5) + 1)` applications (1–5) are drawn from a fixed DPI
//!     catalog, and `d` also scales each application's traffic counters — so the
//!     identifier is a genuine plane over both the app set and the volumes.
//! - **Window × frequency (`startDate`/`endDate` × `frequency`).** The number of
//!   time slots is the count of whole `frequency` units in `[startDate, endDate)`
//!   (at least one, capped at [`MAX_SLOTS`]). So the same window yields far more
//!   records at `HOUR` than at `DAY` granularity — `frequency` is a real plane.
//!
//! The optional `app` filter narrows the records to a single application (an
//! unknown app → an empty page), and `page`/`perPage` window the result. Every
//! record always carries its `app` field regardless of the filter (per the spec).

use axum::extract::RawQuery;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `GET /traffic-analysis` endpoint requires (CAMARA
/// Network Traffic Analysis).
const READ_SCOPE: &str = "network-traffic-analysis:traffic-analysis:read";

/// The default page size (CAMARA `PerPage` default `20`).
const DEFAULT_PER_PAGE: i64 = 20;
/// The maximum page size the spec permits (`perPage` `maximum: 100`).
const MAX_PER_PAGE: i64 = 100;

/// The most time slots CamaraSim materialises for one query, keeping the
/// response bounded regardless of how long a window is requested. A window
/// longer than this many `frequency` units is truncated to this many slots (a
/// documented cut — see the vendored spec).
const MAX_SLOTS: i64 = 100;

/// A fixed catalog of DPI-detected applications: `(app, ipv4Address,
/// description)`. Names are normalised (lowercase, no special characters) per the
/// spec's "Application identification" section. The `networkId` digits pick how
/// many of these appear.
const APPS: [(&str, &str, &str); 5] = [
    (
        "whatsapp",
        "202.112.17.101",
        "A free cross-platform app for messaging, calls, and media sharing.",
    ),
    (
        "wechat",
        "202.112.17.102",
        "A multi-purpose messaging, social media and mobile payment app.",
    ),
    (
        "netflix",
        "202.112.17.103",
        "A subscription-based video streaming service.",
    ),
    (
        "youtube",
        "202.112.17.104",
        "A video-sharing and streaming platform.",
    ),
    (
        "spotify",
        "202.112.17.105",
        "A digital music, podcast, and video streaming service.",
    ),
];

/// Routes for Network Traffic Analysis vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/network-traffic-analysis/vwip/traffic-analysis",
        get(traffic_analysis),
    )
}

/// The analysis granularity (`frequency` query parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frequency {
    Day,
    Hour,
}

impl Frequency {
    /// The length of one aggregation slot, in seconds.
    fn unit_secs(self) -> i64 {
        match self {
            Frequency::Day => 86_400,
            Frequency::Hour => 3_600,
        }
    }
}

/// The validated query parameters of a `getTrafficAnalysis` request.
struct Params {
    network_id: String,
    start: i64,
    end: i64,
    frequency: Frequency,
    app: Option<String>,
    page: i64,
    per_page: i64,
}

/// `GET /network-traffic-analysis/vwip/traffic-analysis`.
async fn traffic_analysis(claims: Claims, headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Validate the query parameters (400 INVALID_ARGUMENT / OUT_OF_RANGE).
    let params = match parse_query(query.as_deref(), &correlator) {
        Ok(parsed) => parsed,
        Err(response) => return response,
    };

    // The `networkId` is the identifier and a control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&params.network_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Build the (possibly empty) record set and page it.
    let records = build_records(&params);
    let body = paginate(records, params.page, params.per_page);
    with_correlator((StatusCode::OK, Json(body)).into_response(), &correlator)
}

/// Parse and validate the query string into [`Params`]. Unknown query parameters
/// are ignored (serde skips fields it does not know), mirroring the other
/// query-driven endpoints.
fn parse_query(query: Option<&str>, correlator: &Option<HeaderValue>) -> Result<Params, Response> {
    #[derive(serde::Deserialize, Default)]
    struct Raw {
        #[serde(rename = "networkId")]
        network_id: Option<String>,
        #[serde(rename = "startDate")]
        start_date: Option<String>,
        #[serde(rename = "endDate")]
        end_date: Option<String>,
        frequency: Option<String>,
        app: Option<String>,
        page: Option<String>,
        #[serde(rename = "perPage")]
        per_page: Option<String>,
    }

    let raw: Raw = serde_urlencoded::from_str(query.unwrap_or("")).map_err(|_| {
        invalid_argument(
            "The query string is not valid application/x-www-form-urlencoded.",
            correlator,
        )
    })?;

    let network_id = match raw.network_id {
        Some(id) if is_uuid(&id) => id,
        Some(_) => {
            return Err(invalid_argument(
                "`networkId` must be a UUID (e.g. 123e4567-e89b-12d3-a456-426614174000).",
                correlator,
            ))
        }
        None => {
            return Err(invalid_argument(
                "`networkId` is a required query parameter.",
                correlator,
            ))
        }
    };

    let frequency = match raw.frequency.as_deref() {
        Some("DAY") => Frequency::Day,
        Some("HOUR") => Frequency::Hour,
        Some(_) => {
            return Err(invalid_argument(
                "`frequency` must be one of: DAY, HOUR.",
                correlator,
            ))
        }
        None => {
            return Err(invalid_argument(
                "`frequency` is a required query parameter.",
                correlator,
            ))
        }
    };

    let start = parse_required_instant(raw.start_date.as_deref(), "startDate", correlator)?;
    let end = parse_required_instant(raw.end_date.as_deref(), "endDate", correlator)?;
    if end <= start {
        return Err(out_of_range(
            "`endDate` must be strictly after `startDate`.",
            correlator,
        ));
    }

    let app = match raw.app {
        Some(a) if a.is_empty() => {
            return Err(invalid_argument(
                "`app` must not be empty when supplied.",
                correlator,
            ))
        }
        Some(a) if a.len() > 128 => {
            return Err(invalid_argument(
                "`app` must be at most 128 characters.",
                correlator,
            ))
        }
        other => other,
    };

    let page = parse_positive_int(raw.page.as_deref(), "page", 1, correlator)?;
    let per_page =
        parse_positive_int(raw.per_page.as_deref(), "perPage", DEFAULT_PER_PAGE, correlator)?;
    if per_page > MAX_PER_PAGE {
        return Err(out_of_range(
            "`perPage` must be at most 100.",
            correlator,
        ));
    }

    Ok(Params {
        network_id,
        start,
        end,
        frequency,
        app,
        page,
        per_page,
    })
}

/// Parse a required RFC 3339 date-time query parameter into Unix seconds (UTC):
/// absent → 400 `INVALID_ARGUMENT`; malformed → 400 `INVALID_ARGUMENT`.
fn parse_required_instant(
    value: Option<&str>,
    name: &str,
    correlator: &Option<HeaderValue>,
) -> Result<i64, Response> {
    match value {
        Some(s) => parse_rfc3339(s).ok_or_else(|| {
            invalid_argument(
                &format!("`{name}` must be an RFC 3339 date-time (e.g. 2024-06-01T00:00:00Z)."),
                correlator,
            )
        }),
        None => Err(invalid_argument(
            &format!("`{name}` is a required query parameter."),
            correlator,
        )),
    }
}

/// Parse an optional integer query parameter with a schema `minimum: 1`: absent →
/// `default`; a non-integer → 400 `INVALID_ARGUMENT`; a value `< 1` → 400
/// `OUT_OF_RANGE` (mirrors `carrier_billing::v0_5`).
fn parse_positive_int(
    value: Option<&str>,
    name: &str,
    default: i64,
    correlator: &Option<HeaderValue>,
) -> Result<i64, Response> {
    match value {
        None => Ok(default),
        Some(raw) => {
            let parsed: i64 = raw
                .parse()
                .map_err(|_| invalid_argument(&format!("`{name}` must be an integer."), correlator))?;
            if parsed < 1 {
                Err(out_of_range(
                    &format!("`{name}` must be greater than or equal to 1."),
                    correlator,
                ))
            } else {
                Ok(parsed)
            }
        }
    }
}

/// Build the full (unpaged) record set for a request. Pure over its input so it
/// can be unit-tested directly. Returns an empty vector for the no-data case
/// (`…000` tail or a `networkId` with fewer than three digits).
fn build_records(params: &Params) -> Vec<Value> {
    let d = match scenarios::trailing_three_digits(&params.network_id) {
        None | Some(0) => return Vec::new(),
        Some(d) => d as i64,
    };
    let n_apps = ((d % 5) + 1) as usize;
    let unit = params.frequency.unit_secs();
    // Whole `frequency` units in [start, end); at least one, capped for bounding.
    let slots = ((params.end - params.start) / unit).clamp(1, MAX_SLOTS);

    let mut records = Vec::new();
    for s in 0..slots {
        let slot_start = params.start + s * unit;
        let slot_end = slot_start + unit;
        for (a, &(app, ipv4, description)) in APPS.iter().enumerate().take(n_apps) {
            if let Some(filter) = params.app.as_deref() {
                if filter != app {
                    continue;
                }
            }
            // Deterministic, positive traffic counters from (d, app, slot).
            let access_count = (d % 500 + 1) * (a as i64 + 1) + s;
            let up = access_count * 1024;
            let down = access_count * 2048;
            records.push(json!({
                "app": app,
                "ipv4Address": ipv4,
                "description": description,
                "accessCount": access_count,
                "accessUpFlow": up,
                "accessDownFlow": down,
                "accessFlow": up + down,
                "startDate": rfc3339_utc(slot_start),
                "endDate": rfc3339_utc(slot_end),
                "accessDate": date_utc(slot_start),
            }));
        }
    }
    records
}

/// Window a record set into a `TrafficAnalysisResponse` — the `page`-th slice of
/// `per_page` items plus the pagination envelope. An out-of-range `page` yields
/// an empty `records` array (CAMARA lists never `404` on an empty result). Pure
/// over its input.
fn paginate(records: Vec<Value>, page: i64, per_page: i64) -> Value {
    let total = records.len() as i64;
    let total_pages = if total == 0 {
        0
    } else {
        (total + per_page - 1) / per_page
    };
    // `page` and `per_page` are both `>= 1`; saturating math guards a huge `page`.
    let start = (page - 1).saturating_mul(per_page).min(total);
    let end = start.saturating_add(per_page).min(total);
    let page_items = records[start as usize..end as usize].to_vec();

    json!({
        "records": page_items,
        "pagination": {
            "page": page,
            "perPage": per_page,
            "totalCount": total,
            "totalPages": total_pages,
        },
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

/// Whether `s` is a canonical UUID string (8-4-4-4-12 hex with hyphens), matching
/// the CAMARA `format: uuid` / `maxLength: 36` schema (mirrors
/// `network_health_assessment::vwip`).
fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, &b) in bytes.iter().enumerate() {
        match i {
            8 | 13 | 18 | 23 => {
                if b != b'-' {
                    return false;
                }
            }
            _ => {
                if !b.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// Parse an RFC 3339 date-time into Unix seconds (UTC), honouring a `Z` or
/// numeric offset. Returns `None` for anything malformed. Self-contained so
/// CamaraSim needs no date/time dependency (mirrors `device_visit_location`).
fn parse_rfc3339(s: &str) -> Option<i64> {
    let (date, rest) = s.split_once(['T', 't'])?;
    let mut d = date.splitn(3, '-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let (time, offset_secs) = if let Some(t) = rest.strip_suffix(['Z', 'z']) {
        (t, 0i64)
    } else if let Some(idx) = rest.rfind(['+', '-']) {
        let (t, off) = rest.split_at(idx);
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let (oh, om) = off[1..].split_once(':')?;
        let oh: i64 = oh.parse().ok()?;
        let om: i64 = om.parse().ok()?;
        if oh > 14 || om >= 60 {
            return None;
        }
        (t, sign * (oh * 3600 + om * 60))
    } else {
        (rest, 0i64)
    };

    let time = time.split_once('.').map_or(time, |(hms, _)| hms);
    let mut hms = time.splitn(3, ':');
    let hh: i64 = hms.next()?.parse().ok()?;
    let mm: i64 = hms.next()?.parse().ok()?;
    let ss: i64 = hms.next().unwrap_or("0").parse().ok()?;
    if !(0..=23).contains(&hh) || !(0..=59).contains(&mm) || !(0..=60).contains(&ss) {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss - offset_secs)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's `days_from_civil`,
/// proleptic Gregorian). Self-contained, no date dependency.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = m as i64;
    let d = d as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset, e.g. `2024-06-01T00:00:00Z` (mirrors `network_health_assessment`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// Format a Unix timestamp (seconds, UTC) as a calendar date `YYYY-MM-DD`.
fn date_utc(unix_secs: i64) -> String {
    let (y, m, d) = civil_from_days(unix_secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
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
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "nta.local:8080";
    const BASE: &str = "/network-traffic-analysis/vwip/traffic-analysis";
    // A valid UUID whose trailing three digits are `tail`, so tests pick the
    // app-count / error case from the identifier alone.
    fn nid(tail: &str) -> String {
        format!("00000000-0000-0000-0000-000000000{tail}")
    }
    // A one-day window (frequency=DAY → one slot).
    const ONE_DAY: &str = "startDate=2024-06-01T00:00:00Z&endDate=2024-06-02T00:00:00Z&frequency=DAY";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn uuid_validation_follows_the_camara_format() {
        assert!(is_uuid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000404"));
        assert!(!is_uuid("123e4567e89b12d3a456426614174000")); // no hyphens
        assert!(!is_uuid("nta-client")); // client-credentials subject
    }

    #[test]
    fn rfc3339_round_trips_a_z_instant() {
        let secs = parse_rfc3339("2024-06-01T00:00:00Z").unwrap();
        assert_eq!(rfc3339_utc(secs), "2024-06-01T00:00:00Z");
        assert_eq!(date_utc(secs), "2024-06-01");
        // A numeric offset is normalised to UTC.
        let plus = parse_rfc3339("2024-06-01T01:00:00+01:00").unwrap();
        assert_eq!(secs, plus);
    }

    #[test]
    fn access_flow_is_up_plus_down_and_records_scale_with_the_identifier() {
        let params = Params {
            network_id: nid("012"),
            start: parse_rfc3339("2024-06-01T00:00:00Z").unwrap(),
            end: parse_rfc3339("2024-06-02T00:00:00Z").unwrap(),
            frequency: Frequency::Day,
            app: None,
            page: 1,
            per_page: 20,
        };
        let records = build_records(&params);
        // d = 12 → (12 % 5) + 1 = 3 apps, one slot → 3 records.
        assert_eq!(records.len(), 3);
        for r in &records {
            let up = r["accessUpFlow"].as_i64().unwrap();
            let down = r["accessDownFlow"].as_i64().unwrap();
            assert_eq!(r["accessFlow"].as_i64().unwrap(), up + down);
            assert!(r["accessCount"].as_i64().unwrap() > 0);
            assert_eq!(r["startDate"], "2024-06-01T00:00:00Z");
            assert_eq!(r["endDate"], "2024-06-02T00:00:00Z");
            assert_eq!(r["accessDate"], "2024-06-01");
        }
    }

    #[test]
    fn frequency_controls_the_slot_count() {
        let day = Params {
            network_id: nid("001"),
            start: parse_rfc3339("2024-06-01T00:00:00Z").unwrap(),
            end: parse_rfc3339("2024-06-03T00:00:00Z").unwrap(),
            frequency: Frequency::Day,
            app: None,
            page: 1,
            per_page: 100,
        };
        // d = 1 → 2 apps. Two-day window at DAY → 2 slots → 4 records.
        assert_eq!(build_records(&day).len(), 4);
        // Same window at HOUR → 48 slots → 96 records.
        let hour = Params {
            frequency: Frequency::Hour,
            ..day
        };
        assert_eq!(build_records(&hour).len(), 96);
    }

    #[test]
    fn slot_count_is_capped() {
        // A ten-year HOUR window would be ~87600 slots; capped at MAX_SLOTS.
        let params = Params {
            network_id: nid("004"), // d=4 → 5 apps
            start: parse_rfc3339("2000-01-01T00:00:00Z").unwrap(),
            end: parse_rfc3339("2010-01-01T00:00:00Z").unwrap(),
            frequency: Frequency::Hour,
            app: None,
            page: 1,
            per_page: 100,
        };
        // MAX_SLOTS slots × 5 apps.
        assert_eq!(build_records(&params).len(), (MAX_SLOTS as usize) * 5);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=nta-client&scope={scope}");
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

    async fn get_ta(
        token: Option<&str>,
        query: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("{BASE}?{query}"))
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
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    async fn ta_ok(query: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(READ_SCOPE).await;
        get_ta(Some(&token), query, None).await
    }

    // --- Happy path --------------------------------------------------------

    #[tokio::test]
    async fn returns_records_and_pagination() {
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}", nid("012"))).await;
        assert_eq!(status, StatusCode::OK);
        // d=12 → 3 apps, one slot → 3 records.
        assert_eq!(body["records"].as_array().unwrap().len(), 3);
        assert_eq!(body["pagination"]["page"], 1);
        assert_eq!(body["pagination"]["perPage"], 20);
        assert_eq!(body["pagination"]["totalCount"], 3);
        assert_eq!(body["pagination"]["totalPages"], 1);
        // Every record carries its app + the up+down invariant.
        let first = &body["records"][0];
        assert!(first["app"].is_string());
        assert_eq!(
            first["accessFlow"].as_i64().unwrap(),
            first["accessUpFlow"].as_i64().unwrap() + first["accessDownFlow"].as_i64().unwrap()
        );
    }

    #[tokio::test]
    async fn zero_tail_is_the_no_data_case() {
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}", nid("000"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["records"].as_array().unwrap().len(), 0);
        assert_eq!(body["pagination"]["totalCount"], 0);
        assert_eq!(body["pagination"]["totalPages"], 0);
    }

    #[tokio::test]
    async fn app_filter_narrows_to_one_application() {
        // d=4 → 5 apps; filter to whatsapp → one record for the one-day window.
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}&app=whatsapp", nid("004"))).await;
        assert_eq!(status, StatusCode::OK);
        let records = body["records"].as_array().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["app"], "whatsapp");
        assert_eq!(body["pagination"]["totalCount"], 1);

        // An unknown app → an empty page (not a 404).
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}&app=no-such-app", nid("004"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["records"].as_array().unwrap().len(), 0);
        assert_eq!(body["pagination"]["totalCount"], 0);
    }

    #[tokio::test]
    async fn pagination_windows_the_results() {
        // d=4 → 5 apps; a two-day DAY window → 10 records. perPage=4 → 3 pages.
        let win = "startDate=2024-06-01T00:00:00Z&endDate=2024-06-03T00:00:00Z&frequency=DAY";
        let (_, _, p1) = ta_ok(&format!("networkId={}&{win}&perPage=4&page=1", nid("004"))).await;
        assert_eq!(p1["records"].as_array().unwrap().len(), 4);
        assert_eq!(p1["pagination"]["totalCount"], 10);
        assert_eq!(p1["pagination"]["totalPages"], 3);
        // Last page has the remainder.
        let (_, _, p3) = ta_ok(&format!("networkId={}&{win}&perPage=4&page=3", nid("004"))).await;
        assert_eq!(p3["records"].as_array().unwrap().len(), 2);
        // A page past the end → empty, still 200.
        let (status, _, p9) =
            ta_ok(&format!("networkId={}&{win}&perPage=4&page=9", nid("004"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(p9["records"].as_array().unwrap().len(), 0);
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}", nid("404"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}", nid("429"))).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_or_bad_required_params_are_rejected() {
        // Missing networkId.
        let (status, _, body) = ta_ok(ONE_DAY).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Non-UUID networkId.
        let (status, _, _) = ta_ok(&format!("networkId=not-a-uuid&{ONE_DAY}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Missing frequency.
        let (status, _, _) = ta_ok(&format!(
            "networkId={}&startDate=2024-06-01T00:00:00Z&endDate=2024-06-02T00:00:00Z",
            nid("012")
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Unknown frequency.
        let (status, _, _) = ta_ok(&format!(
            "networkId={}&startDate=2024-06-01T00:00:00Z&endDate=2024-06-02T00:00:00Z&frequency=WEEK",
            nid("012")
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Malformed startDate.
        let (status, _, _) = ta_ok(&format!(
            "networkId={}&startDate=nope&endDate=2024-06-02T00:00:00Z&frequency=DAY",
            nid("012")
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn end_before_start_is_out_of_range() {
        let (status, _, body) = ta_ok(&format!(
            "networkId={}&startDate=2024-06-02T00:00:00Z&endDate=2024-06-01T00:00:00Z&frequency=DAY",
            nid("012")
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn per_page_over_the_maximum_is_out_of_range() {
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}&perPage=101", nid("012"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
        // A non-integer page → INVALID_ARGUMENT.
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}&page=abc", nid("012"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // A zero page → OUT_OF_RANGE.
        let (status, _, body) =
            ta_ok(&format!("networkId={}&{ONE_DAY}&page=0", nid("012"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            get_ta(Some(&token), &format!("networkId={}&{ONE_DAY}", nid("012")), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            get_ta(None, &format!("networkId={}&{ONE_DAY}", nid("012")), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(READ_SCOPE).await;
        let (status, headers, _) = get_ta(
            Some(&token),
            &format!("networkId={}&{ONE_DAY}", nid("012")),
            Some("corr-nta"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nta")
        );
        let (status, headers, _) = get_ta(
            Some(&token),
            &format!("networkId={}&{ONE_DAY}", nid("404")),
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
