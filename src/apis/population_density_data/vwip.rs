//! Population Density Data **vwip** (CAMARA population-density-data,
//! work-in-progress spec — no released version yet).
//!
//! One endpoint:
//! - `POST /population-density-data/vwip/retrieve` — estimate the population
//!   density of an area over a time window (operationId
//!   `retrievePopulationDensity`).
//!
//! ## What it does
//!
//! The caller supplies an **area** and a `startTime`/`endTime` window; the
//! operator answers with, per grid cell, an estimated people-per-km² figure and
//! a min/max confidence band (or a `NO_DATA` / `LOW_DENSITY` marker), plus an
//! overall area-support `status`. It is an aggregate, privacy-preserving signal —
//! never an individual device's location.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `population-density-data:read`
//! scope.
//!
//! ## Scope of this simulator build (documented cuts)
//!
//! CamaraSim implements the **synchronous** `GEOHASHLIST` path. The following are
//! accepted for schema fidelity but deliberately not implemented, so they answer
//! with the CAMARA-canonical "unsupported" codes rather than silently degrading:
//!
//! - **`POLYGON` areaType** → `422 POPULATION_DENSITY_DATA.UNSUPPORTED_AREA_TYPE`
//!   (the sim supports `GEOHASHLIST` areas only). Consequently `precision` (which
//!   is `POLYGON`-only) is never usable: supplying it with a `GEOHASHLIST` →
//!   `400 INVALID_ARGUMENT`, matching the CAMARA rule.
//! - **Asynchronous delivery** (`sink` / `sinkCredential` / the `202` +
//!   CloudEvents callback flow) — accepted but never acted on; a request too large
//!   to answer synchronously (> 100 geohashes) →
//!   `422 POPULATION_DENSITY_DATA.UNSUPPORTED_SYNC_RESPONSE`.
//! - The `±3-month` **absolute** start-time window checks
//!   (`MIN_/MAX_STARTTIME_EXCEEDED`, `INVALID_TIME_PERIOD`) — omitted so behaviour
//!   stays independent of the wall clock; the deterministic window checks
//!   (`INVALID_END_TIME`, `MAX_TIME_PERIOD_EXCEEDED`) are enforced.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! There is no device identifier; the **geometry drives the answer** (mirrors
//! Region Device Count). The request's **first geohash** is the area's control
//! identifier for the shared reserved-error convention, and every geohash's own
//! characters drive its cell:
//!
//! 1. **The first geohash** — if its trailing three digits name a reserved CAMARA
//!    status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!    `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]). Geohash
//!    characters include the digits `0-9`, so this convention applies unchanged.
//!    Checked after structural validation but before the time window, so it
//!    dominates the window and data planes.
//!
//! 2. **Each geohash** deterministically fixes its cell (a stable FNV-1a hash of
//!    the geohash string): `h % 7 == 0` → `NO_DATA`; `h % 7 == 1` → `LOW_DENSITY`;
//!    otherwise `DENSITY_ESTIMATION` with `pplDensity = (h % 20000) + 1`
//!    people/km² and a ±10% `min`/`max` band. The overall `status` is a function
//!    of the cells: **all** `NO_DATA` → `AREA_NOT_SUPPORTED`; **some** `NO_DATA` →
//!    `PART_OF_AREA_NOT_SUPPORTED`; otherwise `SUPPORTED_AREA`.
//!
//! 3. **Structural / capability planes:** an unknown `areaType` → `400
//!    INVALID_ARGUMENT`; a geohash violating the pattern
//!    `^[0-9bcdefghjkmnpqrstuvwxyz]{1,12}$`, an empty list, or a list over the
//!    schema's 1000-item max → `400 INVALID_ARGUMENT`; a geohash more precise than
//!    the sim supports (length > 9) → `422 UNSUPPORTED_PRECISION`; a list over 100
//!    geohashes → `422 UNSUPPORTED_SYNC_RESPONSE`.
//!
//! 4. **The time window** (`startTime`/`endTime`, both required, RFC 3339): a
//!    malformed timestamp → `400 INVALID_ARGUMENT`; `endTime` before `startTime` →
//!    `400 POPULATION_DENSITY_DATA.INVALID_END_TIME`; a window longer than 7 days →
//!    `400 POPULATION_DENSITY_DATA.MAX_TIME_PERIOD_EXCEEDED`.
//!
//! Examples: a `GEOHASHLIST` of ordinary geohashes over a valid ≤7-day window →
//! `200` with one cell per geohash; a first geohash `"u4pruy429"` → `429
//! TOO_MANY_REQUESTS`; a `POLYGON` area → `422 UNSUPPORTED_AREA_TYPE`; a 10-char
//! geohash → `422 UNSUPPORTED_PRECISION`; `endTime < startTime` → `400
//! POPULATION_DENSITY_DATA.INVALID_END_TIME`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Population
/// Density Data).
const RETRIEVE_SCOPE: &str = "population-density-data:read";

/// The geohash (base32) alphabet — digits `0-9` and lowercase letters excluding
/// `a`, `i`, `l`, `o`. A geohash must be 1–12 of these characters.
const GEOHASH_ALPHABET: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// The most precise geohash length the simulator supports. A longer (more
/// precise) geohash is reported `UNSUPPORTED_PRECISION` (a documented capability
/// limit, a genuine control plane).
const MAX_SUPPORTED_PRECISION: usize = 9;

/// The largest geohash count the simulator answers synchronously. A larger list
/// would require the async (`sink`) flow, which is not implemented, so it is
/// reported `UNSUPPORTED_SYNC_RESPONSE`.
const MAX_SYNC_GEOHASHES: usize = 100;

/// The schema's hard ceiling on the geohash list length.
const MAX_GEOHASHES: usize = 1000;

/// A time window longer than this (7 days, in seconds) is rejected with
/// `MAX_TIME_PERIOD_EXCEEDED` (the CAMARA `endTime - startTime ≤ 7 days` rule).
const MAX_WINDOW_SECS: i64 = 7 * 86_400;

/// Routes for Population Density Data vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/population-density-data/vwip/retrieve",
        post(retrieve),
    )
}

/// `POST /retrieve` request body (CAMARA `PopulationDensityRequest`).
/// `area`/`startTime`/`endTime` are required; `precision`/`sink`/`sinkCredential`
/// are optional (and, in this build, not acted on beyond validation).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    area: Area,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "endTime")]
    end_time: String,
    /// `POLYGON`-only cell precision. Supplying it with a `GEOHASHLIST` is a
    /// `400` per CAMARA; the sim never processes `POLYGON`, so it is otherwise
    /// unused.
    precision: Option<i64>,
    /// Async callback sink (accepted, not acted on — async delivery is a cut).
    #[allow(dead_code)]
    sink: Option<String>,
    /// Async callback credential (accepted, not acted on).
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
    sink_credential: Option<Value>,
}

/// The CAMARA `Area`: a `POLYGON` (with a `boundary`) or a `GEOHASHLIST` (with
/// `geohashes`). CamaraSim implements `GEOHASHLIST`; `boundary` is accepted for
/// schema fidelity but a `POLYGON` request is answered `UNSUPPORTED_AREA_TYPE`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Area {
    #[serde(rename = "areaType")]
    area_type: String,
    geohashes: Option<Vec<String>>,
    #[allow(dead_code)]
    boundary: Option<Vec<Point>>,
}

/// A `POLYGON` boundary vertex (accepted for fidelity; `POLYGON` is a cut).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    #[allow(dead_code)]
    latitude: f64,
    #[allow(dead_code)]
    longitude: f64,
}

/// `POST /population-density-data/vwip/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required (area/startTime/endTime are mandatory).
    let req: RetrieveRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid PopulationDensityRequest (area, startTime and endTime are required).",
                &correlator,
            )
        }
    };

    // Area type: the sim implements GEOHASHLIST only.
    match req.area.area_type.as_str() {
        "GEOHASHLIST" => {}
        "POLYGON" => {
            return unprocessable(
                "POPULATION_DENSITY_DATA.UNSUPPORTED_AREA_TYPE",
                "This simulator supports only `GEOHASHLIST` areas; `POLYGON` is not supported.",
                &correlator,
            )
        }
        _ => {
            return invalid_argument(
                "`area.areaType` must be `GEOHASHLIST` or `POLYGON`.",
                &correlator,
            )
        }
    }

    // `precision` is POLYGON-only; supplying it with a GEOHASHLIST is a 400.
    if req.precision.is_some() {
        return invalid_argument(
            "`precision` applies only to a `POLYGON` area; it must not be set for `GEOHASHLIST`.",
            &correlator,
        );
    }

    // The geohash list: required, non-empty, within the schema's 1000-item max.
    let Some(geohashes) = req.area.geohashes.as_ref() else {
        return invalid_argument(
            "A `GEOHASHLIST` area requires a non-empty `geohashes` list.",
            &correlator,
        );
    };
    if geohashes.is_empty() {
        return invalid_argument("`geohashes` must contain at least one geohash.", &correlator);
    }
    if geohashes.len() > MAX_GEOHASHES {
        return invalid_argument(
            "`geohashes` must contain at most 1000 geohashes.",
            &correlator,
        );
    }

    // Each geohash must be well-formed; and not more precise than the sim supports.
    for g in geohashes {
        if !is_valid_geohash(g) {
            return invalid_argument(
                "Each geohash must match `^[0-9bcdefghjkmnpqrstuvwxyz]{1,12}$`.",
                &correlator,
            );
        }
        if g.len() > MAX_SUPPORTED_PRECISION {
            return unprocessable(
                "POPULATION_DENSITY_DATA.UNSUPPORTED_PRECISION",
                "The requested geohash precision (length) is not supported.",
                &correlator,
            );
        }
    }

    // A list too large to answer synchronously needs the async (sink) flow, which
    // this build does not implement.
    if geohashes.len() > MAX_SYNC_GEOHASHES {
        return unprocessable(
            "POPULATION_DENSITY_DATA.UNSUPPORTED_SYNC_RESPONSE",
            "The request is too large for a synchronous response; a `sink` (async) request is required, which is not supported.",
            &correlator,
        );
    }

    // Control plane 1: a reserved suffix on the first geohash wins over the
    // remaining (time-window and data) planes.
    if let Some(err) = scenarios::reserved_error(&geohashes[0]) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Control plane: the time window must be well-formed, ordered, and ≤ 7 days.
    let (Some(start), Some(end)) = (parse_rfc3339(&req.start_time), parse_rfc3339(&req.end_time))
    else {
        return invalid_argument(
            "`startTime` and `endTime` must be valid RFC 3339 date-times.",
            &correlator,
        );
    };
    if end < start {
        return bad_request(
            "POPULATION_DENSITY_DATA.INVALID_END_TIME",
            "`endTime` must not be earlier than `startTime`.",
            &correlator,
        );
    }
    if end - start > MAX_WINDOW_SECS {
        return bad_request(
            "POPULATION_DENSITY_DATA.MAX_TIME_PERIOD_EXCEEDED",
            "The requested time window must not exceed 7 days.",
            &correlator,
        );
    }

    // Control plane 2: one cell per geohash, deterministic from its characters.
    let cells: Vec<Value> = geohashes.iter().map(|g| cell_for(g)).collect();
    let status = area_status(geohashes);
    let out = json!({
        "timedPopulationDensityData": [
            {
                // A single time slice covering the whole requested window (this
                // build does not subdivide the window into hourly slices).
                "startTime": req.start_time,
                "endTime": req.end_time,
                "cellPopulationDensityData": cells,
            }
        ],
        "status": status,
    });
    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// Whether `g` is a well-formed geohash: 1–12 characters, all from the geohash
/// base32 alphabet.
fn is_valid_geohash(g: &str) -> bool {
    let len = g.len();
    (1..=12).contains(&len) && g.bytes().all(|b| GEOHASH_ALPHABET.contains(&b))
}

/// A stable FNV-1a hash of a geohash string. Used to derive the cell's data type
/// and density deterministically, so a given geohash always yields the same cell.
fn geohash_hash(g: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in g.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The cell data-type bucket for a geohash: `0` → `NO_DATA`, `1` → `LOW_DENSITY`,
/// anything else → `DENSITY_ESTIMATION`. Deterministic from the geohash.
fn cell_kind(g: &str) -> u8 {
    (geohash_hash(g) % 7) as u8
}

/// The `CellPopulationDensityData` JSON object for one geohash, deterministic
/// from its characters (docs/DESIGN.md §7). A `DENSITY_ESTIMATION` cell carries
/// `pplDensity` and a ±10% `min`/`max` band; the other kinds carry no numbers.
fn cell_for(g: &str) -> Value {
    match cell_kind(g) {
        0 => json!({ "geohash": g, "dataType": "NO_DATA" }),
        1 => json!({ "geohash": g, "dataType": "LOW_DENSITY" }),
        _ => {
            let ppl = (geohash_hash(g) % 20_000) + 1; // 1..=20000 people/km²
            let min = ppl - ppl / 10;
            let max = ppl + ppl / 10;
            json!({
                "geohash": g,
                "dataType": "DENSITY_ESTIMATION",
                "pplDensity": ppl,
                "minPplDensity": min,
                "maxPplDensity": max,
            })
        }
    }
}

/// The overall area `status`, a function of the per-cell data types: every cell
/// `NO_DATA` → `AREA_NOT_SUPPORTED`; some but not all `NO_DATA` →
/// `PART_OF_AREA_NOT_SUPPORTED`; otherwise `SUPPORTED_AREA`.
fn area_status(geohashes: &[String]) -> &'static str {
    let no_data = geohashes.iter().filter(|g| cell_kind(g) == 0).count();
    if no_data == geohashes.len() {
        "AREA_NOT_SUPPORTED"
    } else if no_data > 0 {
        "PART_OF_AREA_NOT_SUPPORTED"
    } else {
        "SUPPORTED_AREA"
    }
}

/// A 400 CAMARA error with a caller-chosen `code`, correlator echoed.
fn bad_request(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, code, message).into_response(),
        correlator,
    )
}

/// A 422 CAMARA error with a caller-chosen `code`, correlator echoed.
fn unprocessable(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message).into_response(),
        correlator,
    )
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
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

/// Parse an RFC 3339 / ISO 8601 date-time to epoch seconds (UTC). Accepts a `Z`
/// or `±HH:MM` offset (or none — assumed UTC) and an optional fractional second
/// (ignored). Returns `None` on any malformed input. Self-contained so CamaraSim
/// needs no date/time dependency (mirrors `device_visit_location::vwip`).
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "pdd.local:8080";
    const WINDOW: &str = r#""startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-08T00:00:00Z""#;

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn geohash_validation_follows_the_pattern() {
        assert!(is_valid_geohash("u4pruy"));
        assert!(is_valid_geohash("0"));
        assert!(is_valid_geohash("gbsuv7ztqzwv")); // 12 chars, all legal
        assert!(!is_valid_geohash("")); // empty
        assert!(!is_valid_geohash("abc")); // 'a' not in the alphabet
        assert!(!is_valid_geohash("u4p il")); // space / 'i' / 'l'
        assert!(!is_valid_geohash("gbsuv7ztqzwvx")); // 13 chars
    }

    #[test]
    fn hash_and_cell_are_deterministic() {
        assert_eq!(geohash_hash("u4pruy"), geohash_hash("u4pruy"));
        assert_eq!(cell_for("u4pruy"), cell_for("u4pruy"));
    }

    #[test]
    fn density_cells_carry_an_ordered_band() {
        // Every DENSITY_ESTIMATION cell must satisfy min ≤ ppl ≤ max, all ≥ 0.
        const LETTERS: &[u8] = b"bcdefghjkmnpqrstuvwxyz";
        for a in LETTERS {
            for b in LETTERS {
                let g = String::from_utf8(vec![*a, *b, b'2']).unwrap();
                let cell = cell_for(&g);
                if cell["dataType"] == "DENSITY_ESTIMATION" {
                    let ppl = cell["pplDensity"].as_u64().unwrap();
                    let min = cell["minPplDensity"].as_u64().unwrap();
                    let max = cell["maxPplDensity"].as_u64().unwrap();
                    assert!(min <= ppl && ppl <= max, "band ordered for {g}");
                    assert!(ppl >= 1, "density is at least 1 for {g}");
                } else {
                    assert!(cell.get("pplDensity").is_none(), "no numbers for {g}");
                }
            }
        }
    }

    #[test]
    fn all_three_cell_kinds_are_reachable() {
        let mut seen = [false; 3];
        for kind in 0..3 {
            let _ = geohash_of_kind(kind); // panics if none found
            seen[kind as usize] = true;
        }
        assert_eq!(seen, [true, true, true]);
    }

    #[test]
    fn status_summarises_the_cells() {
        let density = geohash_of_kind(2);
        let no_data = geohash_of_kind(0);
        assert_eq!(area_status(&[density.clone()]), "SUPPORTED_AREA");
        assert_eq!(area_status(&[no_data.clone()]), "AREA_NOT_SUPPORTED");
        assert_eq!(
            area_status(&[density, no_data]),
            "PART_OF_AREA_NOT_SUPPORTED"
        );
    }

    #[test]
    fn rfc3339_parses_and_orders() {
        let s = parse_rfc3339("2024-01-01T00:00:00Z").unwrap();
        let e = parse_rfc3339("2024-01-08T00:00:00Z").unwrap();
        assert_eq!(e - s, MAX_WINDOW_SECS);
        assert!(parse_rfc3339("nope").is_none());
    }

    /// Find a letters-only geohash (never a reserved-error tail, no digits) whose
    /// cell has the requested kind. Used to build deterministic status fixtures.
    fn geohash_of_kind(kind: u8) -> String {
        const LETTERS: &[u8] = b"bcdefghjkmnpqrstuvwxyz";
        for a in LETTERS {
            for b in LETTERS {
                for c in LETTERS {
                    for d in LETTERS {
                        let g = String::from_utf8(vec![*a, *b, *c, *d]).unwrap();
                        if cell_kind(&g) == kind {
                            return g;
                        }
                    }
                }
            }
        }
        panic!("no geohash found for kind {kind}");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=pdd-client&scope={scope}");
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

    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/population-density-data/vwip/retrieve")
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
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    /// Mint a scoped token and call the endpoint.
    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    /// A GEOHASHLIST request body from a comma-separated list of quoted geohashes.
    fn body_with(geohashes_json: &str) -> String {
        format!(r#"{{"area":{{"areaType":"GEOHASHLIST","geohashes":[{geohashes_json}]}},{WINDOW}}}"#)
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn returns_one_cell_per_geohash() {
        let g0 = geohash_of_kind(2); // density
        let g1 = geohash_of_kind(0); // no data
        let body = body_with(&format!("\"{g0}\",\"{g1}\""));
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);

        let slices = out["timedPopulationDensityData"].as_array().unwrap();
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0]["startTime"], "2024-01-01T00:00:00Z");
        assert_eq!(slices[0]["endTime"], "2024-01-08T00:00:00Z");

        let cells = slices[0]["cellPopulationDensityData"].as_array().unwrap();
        assert_eq!(cells.len(), 2);
        // Each cell matches the deterministic mapping.
        assert_eq!(cells[0], cell_for(&g0));
        assert_eq!(cells[1], cell_for(&g1));
        // Mixed cells → the whole area is partly unsupported.
        assert_eq!(out["status"], "PART_OF_AREA_NOT_SUPPORTED");
    }

    #[tokio::test]
    async fn a_density_cell_carries_ordered_numbers() {
        let g = geohash_of_kind(2);
        let (status, _, out) = call_ok(&body_with(&format!("\"{g}\""))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["status"], "SUPPORTED_AREA");
        let cell = &out["timedPopulationDensityData"][0]["cellPopulationDensityData"][0];
        assert_eq!(cell["dataType"], "DENSITY_ESTIMATION");
        let ppl = cell["pplDensity"].as_u64().unwrap();
        let min = cell["minPplDensity"].as_u64().unwrap();
        let max = cell["maxPplDensity"].as_u64().unwrap();
        assert!(min <= ppl && ppl <= max);
    }

    #[tokio::test]
    async fn all_no_data_cells_make_the_area_unsupported() {
        let g = geohash_of_kind(0);
        let (status, _, out) = call_ok(&body_with(&format!("\"{g}\""))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["status"], "AREA_NOT_SUPPORTED");
        assert_eq!(
            out["timedPopulationDensityData"][0]["cellPopulationDensityData"][0]["dataType"],
            "NO_DATA"
        );
    }

    // --- Control plane: reserved errors ------------------------------------

    #[tokio::test]
    async fn reserved_suffix_on_the_first_geohash_selects_a_canonical_error() {
        // "u404" → trailing three digits 404 → NOT_FOUND.
        let (status, _, out) = call_ok(&body_with(r#""u404","gbsuv""#)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(out["code"], "NOT_FOUND");

        // "gb429" → 429.
        let (status, _, out) = call_ok(&body_with(r#""gb429""#)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(out["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn reserved_suffix_wins_over_a_bad_window() {
        // …404 first geohash with an inverted window still returns the reserved 404.
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["u404"]},"startTime":"2024-01-08T00:00:00Z","endTime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(out["code"], "NOT_FOUND");
    }

    // --- Control plane: area type / precision / size -----------------------

    #[tokio::test]
    async fn polygon_area_is_unsupported() {
        let body = r#"{"area":{"areaType":"POLYGON","boundary":[{"latitude":1.0,"longitude":2.0},{"latitude":3.0,"longitude":4.0},{"latitude":5.0,"longitude":6.0}]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "POPULATION_DENSITY_DATA.UNSUPPORTED_AREA_TYPE");
    }

    #[tokio::test]
    async fn unknown_area_type_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"CIRCLE"},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn precision_with_geohashlist_is_rejected() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["u4pruy"]},"precision":7,"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn a_too_precise_geohash_is_unsupported_precision() {
        // 10 chars > MAX_SUPPORTED_PRECISION (9).
        let (status, _, out) = call_ok(&body_with(r#""u4pruydqqv""#)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "POPULATION_DENSITY_DATA.UNSUPPORTED_PRECISION");
    }

    #[tokio::test]
    async fn a_malformed_geohash_is_invalid_argument() {
        // 'a' is not in the geohash alphabet.
        let (status, _, out) = call_ok(&body_with(r#""abc""#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn an_empty_geohash_list_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":[]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn a_list_too_large_for_sync_is_rejected() {
        // 101 valid, non-reserved geohashes (letters only) → UNSUPPORTED_SYNC_RESPONSE.
        let list = (0..101)
            .map(|_| "\"bcde\"".to_string())
            .collect::<Vec<_>>()
            .join(",");
        let (status, _, out) = call_ok(&body_with(&list)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "POPULATION_DENSITY_DATA.UNSUPPORTED_SYNC_RESPONSE");
    }

    // --- Control plane: the time window ------------------------------------

    #[tokio::test]
    async fn end_before_start_is_invalid_end_time() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-08T00:00:00Z","endTime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "POPULATION_DENSITY_DATA.INVALID_END_TIME");
    }

    #[tokio::test]
    async fn a_window_over_seven_days_is_rejected() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-09T00:00:01Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "POPULATION_DENSITY_DATA.MAX_TIME_PERIOD_EXCEEDED");
    }

    #[tokio::test]
    async fn malformed_time_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"nope","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z","x":1}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_body_is_rejected() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, _, out) = post_retrieve(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, out) = post_retrieve(Some(&token), &body_with(r#""bcde""#), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(out["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, out) = post_retrieve(None, &body_with(r#""bcde""#), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(out["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        let (status, headers, _) =
            post_retrieve(Some(&token), &body_with(r#""bcde""#), Some("corr-pdd")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-pdd")
        );

        let (status, headers, _) =
            post_retrieve(Some(&token), &body_with(r#""u404""#), Some("corr-err")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
