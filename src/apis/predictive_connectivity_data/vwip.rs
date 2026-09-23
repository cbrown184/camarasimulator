//! Predictive Connectivity Data **vwip** (CAMARA predictive-connectivity-data,
//! work-in-progress spec — no released version yet).
//!
//! One endpoint:
//! - `POST /predictive-connectivity-data/vwip/retrieve` — forecast the
//!   connectivity of an area over a time window (operationId
//!   `retrieveConnectivity`).
//!
//! ## What it does
//!
//! The caller supplies an **area**, a target `serviceLevel`, and a
//! `startTime`/`endTime` window; the operator answers with, per grid cell, a
//! stack of vertical **layers** (altitude bands `layerThickness` metres tall),
//! each rated `GC` (good) / `MC` (marginal) / `NC` (none) / `ND` (no data), plus
//! an overall area-support `status`. It is an aggregate, privacy-preserving
//! planning signal — never an individual device's location.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `predictive-connectivity-data:read` scope.
//!
//! ## Scope of this simulator build (documented cuts)
//!
//! CamaraSim implements the **synchronous** `GEOHASHLIST` path with a single time
//! slice. The following are accepted for schema fidelity but deliberately not
//! implemented, so they answer with the CAMARA-canonical "unsupported" codes
//! rather than silently degrading:
//!
//! - **`POLYGON` areaType** →
//!   `422 PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_AREA_TYPE` (the sim supports
//!   `GEOHASHLIST` areas only). Consequently `precision` (which is `POLYGON`-only)
//!   is never usable: supplying it with a `GEOHASHLIST` → `400 INVALID_ARGUMENT`,
//!   matching the CAMARA rule.
//! - **Asynchronous delivery** (`sink` / `sinkCredential` / the `202` +
//!   CloudEvents callback flow) — accepted but never acted on; a request too large
//!   to answer synchronously (> 100 geohashes) →
//!   `422 PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_SYNC_RESPONSE`.
//! - **Hourly time-slicing** — the response carries a single
//!   `TimedConnectivityData` covering the whole requested window rather than one
//!   slice per hour.
//! - The **absolute** start-time window checks (`MIN_/MAX_STARTTIME_EXCEEDED`,
//!   `INVALID_TIME_PERIOD`) — omitted so behaviour stays independent of the wall
//!   clock; the deterministic window checks (`INVALID_END_TIME`,
//!   `MAX_TIME_PERIOD_EXCEEDED`) are enforced.
//! - All three `serviceLevel`s (`C2` / `STREAM_4K` / `BEST_EFFORT`) are supported,
//!   so `UNSUPPORTED_SERVICE_LEVEL` is documented but never returned.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! There is no device identifier; the **geometry drives the answer** (mirrors
//! Population Density Data). The request's **first geohash** is the area's control
//! identifier for the shared reserved-error convention, and each geohash's own
//! characters — together with the target `serviceLevel` and requested `height` —
//! drive its cell:
//!
//! 1. **The first geohash** — if its trailing three digits name a reserved CAMARA
//!    status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!    `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]). Geohash
//!    characters include the digits `0-9`, so this convention applies unchanged.
//!    Checked after structural validation but before the time window, so it
//!    dominates the window and data planes.
//!
//! 2. **Each geohash** deterministically fixes its cell (a stable FNV-1a hash of
//!    the geohash string): `h % 7 == 0` → a `NO_DATA` cell (every layer `ND`);
//!    otherwise the cell's ground-level signal score `s = h % 100` degrades with
//!    altitude (each layer up loses 15 points), and the target `serviceLevel`'s
//!    thresholds map the per-layer score to `GC`/`MC`/`NC`. The overall `status`
//!    is a function of the cells: **all** `NO_DATA` → `AREA_NOT_SUPPORTED`;
//!    **some** `NO_DATA` → `PART_OF_AREA_NOT_SUPPORTED`; otherwise
//!    `SUPPORTED_AREA`.
//!
//! 3. **The target `serviceLevel`** is a genuine second control plane: a stricter
//!    level (`C2`) needs a higher score for the same rating than a lax one
//!    (`BEST_EFFORT`), so the same geohash yields fewer good/more marginal layers
//!    under `C2`.
//!
//! 4. **The requested `height`** shapes the vertical stack: the number of layers
//!    returned is `height / layerThickness + 1` (default 4 when `height` is
//!    omitted). `includeSignalStrength` toggles the per-layer `layerSignalStrengths`
//!    (dBm) band.
//!
//! 5. **Structural / capability planes:** an unknown `areaType`, `serviceLevel`,
//!    or `networkType` → `400 INVALID_ARGUMENT`; a geohash violating the pattern
//!    `^[0-9bcdefghjkmnpqrstuvwxyz]{1,12}$`, an empty list, or a list over the
//!    schema's 1000-item max → `400 INVALID_ARGUMENT`; `height` outside `0..=250`
//!    or `precision` with a `GEOHASHLIST` → `400 INVALID_ARGUMENT`; a geohash more
//!    precise than the sim supports (length > 9) → `422 UNSUPPORTED_PRECISION`; a
//!    list over 100 geohashes → `422 UNSUPPORTED_SYNC_RESPONSE`; a `POLYGON` area →
//!    `422 UNSUPPORTED_AREA_TYPE`.
//!
//! 6. **The time window** (`startTime`/`endTime`, both required, RFC 3339): a
//!    malformed timestamp → `400 INVALID_ARGUMENT`; `endTime` before `startTime` →
//!    `400 PREDICTIVE_CONNECTIVITY_DATA.INVALID_END_TIME`; a window longer than 7
//!    days (168 hours) → `400 PREDICTIVE_CONNECTIVITY_DATA.MAX_TIME_PERIOD_EXCEEDED`.

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

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA Predictive
/// Connectivity Data).
const RETRIEVE_SCOPE: &str = "predictive-connectivity-data:read";

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

/// A time window longer than this (7 days = 168 hours, in seconds) is rejected
/// with `MAX_TIME_PERIOD_EXCEEDED` (the `timedConnectivityData` array caps at 168
/// hourly items, so 7 days is the natural window ceiling).
const MAX_WINDOW_SECS: i64 = 7 * 86_400;

/// The vertical thickness (metres) of one connectivity layer.
const LAYER_THICKNESS_M: i64 = 30;

/// The number of layers returned when the caller does not request a `height`.
const DEFAULT_LAYERS: usize = 4;

/// The largest `height` (metres above ground) the schema allows.
const MAX_HEIGHT_M: i64 = 250;

/// Routes for Predictive Connectivity Data vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/predictive-connectivity-data/vwip/retrieve",
        post(retrieve),
    )
}

/// `POST /retrieve` request body (CAMARA `RetrieveConnectivityRequest`).
/// `serviceLevel`/`area`/`startTime`/`endTime` are required; the rest are
/// optional (and, in this build, either act as control planes or are validated
/// but not acted on).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveRequest {
    #[serde(rename = "serviceLevel")]
    service_level: String,
    area: Area,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "endTime")]
    end_time: String,
    /// Radio access to forecast (`4G`/`5G`; omit for both). Validated only.
    #[serde(rename = "networkType")]
    network_type: Option<String>,
    /// `POLYGON`-only cell precision. Supplying it with a `GEOHASHLIST` is a
    /// `400` per CAMARA; the sim never processes `POLYGON`, so it is otherwise
    /// unused.
    precision: Option<i64>,
    /// Metres above ground to forecast, shaping the number of vertical layers.
    height: Option<i64>,
    /// Whether to include the per-layer `layerSignalStrengths` (dBm) band.
    #[serde(rename = "includeSignalStrength")]
    include_signal_strength: Option<bool>,
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

/// The target service level a caller wants the network to sustain. Drives how
/// strict the per-layer connectivity rating is (a genuine control plane).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceLevel {
    /// Command-and-control (drone BVLOS): the strictest — needs a high score.
    C2,
    /// 4K video streaming: moderate.
    Stream4k,
    /// Best-effort connectivity: the most lenient.
    BestEffort,
}

impl ServiceLevel {
    /// Parse the CAMARA `ServiceLevel` enum string. Returns `None` for an unknown
    /// value (→ `400 INVALID_ARGUMENT`).
    fn parse(s: &str) -> Option<Self> {
        match s {
            "C2" => Some(Self::C2),
            "STREAM_4K" => Some(Self::Stream4k),
            "BEST_EFFORT" => Some(Self::BestEffort),
            _ => None,
        }
    }

    /// `(good_min, marginal_min)`: a per-layer score `>= good_min` rates `GC`,
    /// `>= marginal_min` rates `MC`, otherwise `NC`. A stricter service level
    /// demands a higher score for the same rating.
    fn thresholds(self) -> (u32, u32) {
        match self {
            Self::C2 => (70, 40),
            Self::Stream4k => (50, 25),
            Self::BestEffort => (30, 10),
        }
    }
}

/// `POST /predictive-connectivity-data/vwip/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required (serviceLevel/area/startTime/endTime mandatory).
    let req: RetrieveRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid RetrieveConnectivityRequest (serviceLevel, area, startTime and endTime are required).",
                &correlator,
            )
        }
    };

    // serviceLevel: must be a known CAMARA ServiceLevel (all three are supported).
    let Some(service_level) = ServiceLevel::parse(&req.service_level) else {
        return invalid_argument(
            "`serviceLevel` must be one of `C2`, `STREAM_4K`, `BEST_EFFORT`.",
            &correlator,
        );
    };

    // networkType (optional): if present, must be 4G or 5G.
    if let Some(nt) = req.network_type.as_deref() {
        if nt != "4G" && nt != "5G" {
            return invalid_argument("`networkType`, if present, must be `4G` or `5G`.", &correlator);
        }
    }

    // Area type: the sim implements GEOHASHLIST only.
    match req.area.area_type.as_str() {
        "GEOHASHLIST" => {}
        "POLYGON" => {
            return unprocessable(
                "PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_AREA_TYPE",
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

    // `height` (optional): metres above ground, 0..=250.
    if let Some(h) = req.height {
        if !(0..=MAX_HEIGHT_M).contains(&h) {
            return invalid_argument("`height` must be between 0 and 250 metres.", &correlator);
        }
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
                "PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_PRECISION",
                "The requested geohash precision (length) is not supported.",
                &correlator,
            );
        }
    }

    // A list too large to answer synchronously needs the async (sink) flow, which
    // this build does not implement.
    if geohashes.len() > MAX_SYNC_GEOHASHES {
        return unprocessable(
            "PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_SYNC_RESPONSE",
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
            "PREDICTIVE_CONNECTIVITY_DATA.INVALID_END_TIME",
            "`endTime` must not be earlier than `startTime`.",
            &correlator,
        );
    }
    if end - start > MAX_WINDOW_SECS {
        return bad_request(
            "PREDICTIVE_CONNECTIVITY_DATA.MAX_TIME_PERIOD_EXCEEDED",
            "The requested time window must not exceed 7 days.",
            &correlator,
        );
    }

    // Control plane 2/3/4: one cell per geohash, deterministic from its characters,
    // the target service level, and the requested height.
    let layers = layer_count(req.height);
    let include_ss = req.include_signal_strength.unwrap_or(false);
    let cells: Vec<Value> = geohashes
        .iter()
        .map(|g| cell_for(g, service_level, layers, include_ss))
        .collect();
    let status = area_status(geohashes);

    let mut out = json!({
        "layerThickness": LAYER_THICKNESS_M,
        "timedConnectivityData": [
            {
                // A single time slice covering the whole requested window (this
                // build does not subdivide the window into hourly slices).
                "startTime": req.start_time,
                "endTime": req.end_time,
                "cellConnectivityData": cells,
            }
        ],
        "status": status,
    });
    // Echo the requested height when the caller supplied one.
    if let Some(h) = req.height {
        out["requestedHeight"] = json!(h);
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// Whether `g` is a well-formed geohash: 1–12 characters, all from the geohash
/// base32 alphabet.
fn is_valid_geohash(g: &str) -> bool {
    let len = g.len();
    (1..=12).contains(&len) && g.bytes().all(|b| GEOHASH_ALPHABET.contains(&b))
}

/// A stable FNV-1a hash of a geohash string. Used to derive the cell's data type
/// and ground-level score deterministically, so a given geohash always yields the
/// same cell.
fn geohash_hash(g: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in g.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The number of vertical layers to return: `height / layerThickness + 1`,
/// clamped to `1..=100` (the schema's per-cell layer cap). When `height` is
/// omitted, a fixed default of [`DEFAULT_LAYERS`] layers is returned.
fn layer_count(height: Option<i64>) -> usize {
    match height {
        Some(h) => ((h / LAYER_THICKNESS_M) + 1).clamp(1, 100) as usize,
        None => DEFAULT_LAYERS,
    }
}

/// Whether a geohash's cell is a `NO_DATA` cell (no forecast for the cell).
/// Deterministic from the geohash (mirrors Population Density Data's `NO_DATA`).
fn is_no_data(g: &str) -> bool {
    geohash_hash(g) % 7 == 0
}

/// The per-layer connectivity rating for a score under a service level:
/// `>= good_min` → `GC`, `>= marginal_min` → `MC`, otherwise `NC`.
fn rate(score: u32, level: ServiceLevel) -> &'static str {
    let (good_min, marginal_min) = level.thresholds();
    if score >= good_min {
        "GC"
    } else if score >= marginal_min {
        "MC"
    } else {
        "NC"
    }
}

/// The per-layer signal strength (dBm) for a score: a stronger score maps to a
/// stronger (closer to 0) dBm reading, in the range `-120..=-21`.
fn signal_dbm(score: u32) -> i64 {
    -120 + score as i64
}

/// The `CellConnectivityData` JSON object for one geohash, deterministic from its
/// characters, the target `serviceLevel`, and the layer count (docs/DESIGN.md
/// §7). A `NO_DATA` cell rates every layer `ND` (and `null` signal strengths);
/// otherwise the ground score `s = hash % 100` degrades by 15 points per layer up,
/// and the service level's thresholds map each layer's score to `GC`/`MC`/`NC`.
fn cell_for(g: &str, level: ServiceLevel, layers: usize, include_ss: bool) -> Value {
    if is_no_data(g) {
        let connectivities: Vec<Value> = (0..layers).map(|_| json!("ND")).collect();
        let mut cell = json!({
            "geohash": g,
            "layerConnectivities": connectivities,
        });
        if include_ss {
            let strengths: Vec<Value> = (0..layers).map(|_| Value::Null).collect();
            cell["layerSignalStrengths"] = json!(strengths);
        }
        return cell;
    }

    let ground = geohash_hash(g) % 100; // 0..=99 ground-level score
    let mut connectivities: Vec<Value> = Vec::with_capacity(layers);
    let mut strengths: Vec<Value> = Vec::with_capacity(layers);
    for l in 0..layers {
        // Signal degrades with altitude: each layer up loses 15 points.
        let score = ground.saturating_sub(l as u32 * 15);
        connectivities.push(json!(rate(score, level)));
        strengths.push(json!(signal_dbm(score)));
    }
    let mut cell = json!({
        "geohash": g,
        "layerConnectivities": connectivities,
    });
    if include_ss {
        cell["layerSignalStrengths"] = json!(strengths);
    }
    cell
}

/// The overall area `status`, a function of the per-cell data types: every cell
/// `NO_DATA` → `AREA_NOT_SUPPORTED`; some but not all `NO_DATA` →
/// `PART_OF_AREA_NOT_SUPPORTED`; otherwise `SUPPORTED_AREA`.
fn area_status(geohashes: &[String]) -> &'static str {
    let no_data = geohashes.iter().filter(|g| is_no_data(g)).count();
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
/// needs no date/time dependency (mirrors `population_density_data::vwip`).
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

    const HOST: &str = "pcd.local:8080";
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
    fn service_level_parses_the_enum() {
        assert_eq!(ServiceLevel::parse("C2"), Some(ServiceLevel::C2));
        assert_eq!(ServiceLevel::parse("STREAM_4K"), Some(ServiceLevel::Stream4k));
        assert_eq!(ServiceLevel::parse("BEST_EFFORT"), Some(ServiceLevel::BestEffort));
        assert_eq!(ServiceLevel::parse("nope"), None);
    }

    #[test]
    fn cell_is_deterministic() {
        assert_eq!(
            cell_for("u4pruy", ServiceLevel::C2, 4, true),
            cell_for("u4pruy", ServiceLevel::C2, 4, true)
        );
    }

    #[test]
    fn layer_count_follows_height() {
        assert_eq!(layer_count(None), DEFAULT_LAYERS);
        assert_eq!(layer_count(Some(0)), 1);
        assert_eq!(layer_count(Some(30)), 2);
        assert_eq!(layer_count(Some(250)), 9);
    }

    #[test]
    fn a_stricter_service_level_gives_no_better_ratings() {
        // For every ground score, a stricter level never rates a layer *better*
        // than a laxer one — C2 ≤ STREAM_4K ≤ BEST_EFFORT in quality.
        let order = |r: &str| match r {
            "GC" => 2,
            "MC" => 1,
            _ => 0,
        };
        for s in 0u32..=99 {
            let c2 = order(rate(s, ServiceLevel::C2));
            let s4 = order(rate(s, ServiceLevel::Stream4k));
            let be = order(rate(s, ServiceLevel::BestEffort));
            assert!(c2 <= s4 && s4 <= be, "monotonic at score {s}");
        }
    }

    #[test]
    fn signal_degrades_with_altitude() {
        // A data cell's first layer is at least as good as any higher layer.
        let g = geohash_of_data(); // a non-NO_DATA geohash
        let cell = cell_for(&g, ServiceLevel::BestEffort, 5, true);
        let ss = cell["layerSignalStrengths"].as_array().unwrap();
        for w in ss.windows(2) {
            let a = w[0].as_i64().unwrap();
            let b = w[1].as_i64().unwrap();
            assert!(a >= b, "signal non-increasing with altitude");
        }
    }

    #[test]
    fn status_summarises_the_cells() {
        let data = geohash_of_data();
        let no_data = geohash_of_no_data();
        assert_eq!(area_status(&[data.clone()]), "SUPPORTED_AREA");
        assert_eq!(area_status(&[no_data.clone()]), "AREA_NOT_SUPPORTED");
        assert_eq!(
            area_status(&[data, no_data]),
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
    /// cell is a data (non-`NO_DATA`) cell.
    fn geohash_of_data() -> String {
        find_geohash(false)
    }

    /// Find a letters-only geohash whose cell is a `NO_DATA` cell.
    fn geohash_of_no_data() -> String {
        find_geohash(true)
    }

    fn find_geohash(want_no_data: bool) -> String {
        const LETTERS: &[u8] = b"bcdefghjkmnpqrstuvwxyz";
        for a in LETTERS {
            for b in LETTERS {
                for c in LETTERS {
                    for d in LETTERS {
                        let g = String::from_utf8(vec![*a, *b, *c, *d]).unwrap();
                        if is_no_data(&g) == want_no_data {
                            return g;
                        }
                    }
                }
            }
        }
        panic!("no geohash found (want_no_data={want_no_data})");
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=pcd-client&scope={scope}");
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
            .uri("/predictive-connectivity-data/vwip/retrieve")
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

    /// A GEOHASHLIST + serviceLevel request body from a comma-separated list of
    /// quoted geohashes.
    fn body_with(geohashes_json: &str) -> String {
        format!(
            r#"{{"serviceLevel":"C2","area":{{"areaType":"GEOHASHLIST","geohashes":[{geohashes_json}]}},{WINDOW}}}"#
        )
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn returns_one_cell_per_geohash() {
        let g0 = geohash_of_data();
        let g1 = geohash_of_no_data();
        let body = body_with(&format!("\"{g0}\",\"{g1}\""));
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["layerThickness"], LAYER_THICKNESS_M);

        let slices = out["timedConnectivityData"].as_array().unwrap();
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0]["startTime"], "2024-01-01T00:00:00Z");
        assert_eq!(slices[0]["endTime"], "2024-01-08T00:00:00Z");

        let cells = slices[0]["cellConnectivityData"].as_array().unwrap();
        assert_eq!(cells.len(), 2);
        // Each cell matches the deterministic mapping (default 4 layers, C2, no SS).
        assert_eq!(cells[0], cell_for(&g0, ServiceLevel::C2, DEFAULT_LAYERS, false));
        assert_eq!(cells[1], cell_for(&g1, ServiceLevel::C2, DEFAULT_LAYERS, false));
        // Mixed cells → the whole area is partly unsupported.
        assert_eq!(out["status"], "PART_OF_AREA_NOT_SUPPORTED");
    }

    #[tokio::test]
    async fn a_data_cell_carries_default_layers_without_signal_strength() {
        let g = geohash_of_data();
        let (status, _, out) = call_ok(&body_with(&format!("\"{g}\""))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["status"], "SUPPORTED_AREA");
        let cell = &out["timedConnectivityData"][0]["cellConnectivityData"][0];
        let conn = cell["layerConnectivities"].as_array().unwrap();
        assert_eq!(conn.len(), DEFAULT_LAYERS);
        // includeSignalStrength defaults to false → no strengths band.
        assert!(cell.get("layerSignalStrengths").is_none());
        // requestedHeight is omitted when no height was requested.
        assert!(out.get("requestedHeight").is_none());
    }

    #[tokio::test]
    async fn all_no_data_cells_make_the_area_unsupported() {
        let g = geohash_of_no_data();
        let (status, _, out) = call_ok(&body_with(&format!("\"{g}\""))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["status"], "AREA_NOT_SUPPORTED");
        let cell = &out["timedConnectivityData"][0]["cellConnectivityData"][0];
        for v in cell["layerConnectivities"].as_array().unwrap() {
            assert_eq!(v, "ND");
        }
    }

    #[tokio::test]
    async fn height_sets_layer_count_and_is_echoed() {
        let g = geohash_of_data();
        let body = format!(
            r#"{{"serviceLevel":"C2","height":90,"area":{{"areaType":"GEOHASHLIST","geohashes":["{g}"]}},{WINDOW}}}"#
        );
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["requestedHeight"], 90);
        let conn = out["timedConnectivityData"][0]["cellConnectivityData"][0]["layerConnectivities"]
            .as_array()
            .unwrap();
        assert_eq!(conn.len(), layer_count(Some(90))); // 90/30 + 1 = 4
    }

    #[tokio::test]
    async fn include_signal_strength_adds_the_band() {
        let g = geohash_of_data();
        let body = format!(
            r#"{{"serviceLevel":"BEST_EFFORT","includeSignalStrength":true,"area":{{"areaType":"GEOHASHLIST","geohashes":["{g}"]}},{WINDOW}}}"#
        );
        let (status, _, out) = call_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let cell = &out["timedConnectivityData"][0]["cellConnectivityData"][0];
        let ss = cell["layerSignalStrengths"].as_array().unwrap();
        let conn = cell["layerConnectivities"].as_array().unwrap();
        assert_eq!(ss.len(), conn.len());
    }

    #[tokio::test]
    async fn service_level_changes_the_ratings() {
        // A geohash whose ground layer differs by service level, same everything else.
        let g = geohash_with_ground_score(55); // C2 → MC, BEST_EFFORT → GC
        let c2 = call_ok(&format!(
            r#"{{"serviceLevel":"C2","area":{{"areaType":"GEOHASHLIST","geohashes":["{g}"]}},{WINDOW}}}"#
        ))
        .await
        .2;
        let be = call_ok(&format!(
            r#"{{"serviceLevel":"BEST_EFFORT","area":{{"areaType":"GEOHASHLIST","geohashes":["{g}"]}},{WINDOW}}}"#
        ))
        .await
        .2;
        let l0 = |v: &Value| {
            v["timedConnectivityData"][0]["cellConnectivityData"][0]["layerConnectivities"][0]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(l0(&c2), "MC");
        assert_eq!(l0(&be), "GC");
    }

    /// A letters-only, non-NO_DATA geohash whose ground score is exactly `want`.
    fn geohash_with_ground_score(want: u32) -> String {
        const LETTERS: &[u8] = b"bcdefghjkmnpqrstuvwxyz";
        for a in LETTERS {
            for b in LETTERS {
                for c in LETTERS {
                    for d in LETTERS {
                        let g = String::from_utf8(vec![*a, *b, *c, *d]).unwrap();
                        if !is_no_data(&g) && geohash_hash(&g) % 100 == want {
                            return g;
                        }
                    }
                }
            }
        }
        panic!("no geohash found with ground score {want}");
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
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"GEOHASHLIST","geohashes":["u404"]},"startTime":"2024-01-08T00:00:00Z","endTime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(out["code"], "NOT_FOUND");
    }

    // --- Control plane: service level / area type / precision / size -------

    #[tokio::test]
    async fn unknown_service_level_is_invalid_argument() {
        let body = r#"{"serviceLevel":"ULTRA","area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_network_type_is_invalid_argument() {
        let body = r#"{"serviceLevel":"C2","networkType":"6G","area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn polygon_area_is_unsupported() {
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"POLYGON","boundary":[{"latitude":1.0,"longitude":2.0},{"latitude":3.0,"longitude":4.0},{"latitude":5.0,"longitude":6.0}]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_AREA_TYPE");
    }

    #[tokio::test]
    async fn unknown_area_type_is_invalid_argument() {
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"CIRCLE"},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn precision_with_geohashlist_is_rejected() {
        let body = r#"{"serviceLevel":"C2","precision":7,"area":{"areaType":"GEOHASHLIST","geohashes":["u4pruy"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn out_of_range_height_is_invalid_argument() {
        let body = r#"{"serviceLevel":"C2","height":300,"area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn a_too_precise_geohash_is_unsupported_precision() {
        // 10 chars > MAX_SUPPORTED_PRECISION (9).
        let (status, _, out) = call_ok(&body_with(r#""u4pruydqqv""#)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(out["code"], "PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_PRECISION");
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
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"GEOHASHLIST","geohashes":[]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
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
        assert_eq!(out["code"], "PREDICTIVE_CONNECTIVITY_DATA.UNSUPPORTED_SYNC_RESPONSE");
    }

    // --- Control plane: the time window ------------------------------------

    #[tokio::test]
    async fn end_before_start_is_invalid_end_time() {
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-08T00:00:00Z","endTime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "PREDICTIVE_CONNECTIVITY_DATA.INVALID_END_TIME");
    }

    #[tokio::test]
    async fn a_window_over_seven_days_is_rejected() {
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-09T00:00:01Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "PREDICTIVE_CONNECTIVITY_DATA.MAX_TIME_PERIOD_EXCEEDED");
    }

    #[tokio::test]
    async fn malformed_time_is_invalid_argument() {
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"nope","endTime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let body = r#"{"serviceLevel":"C2","area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z","x":1}"#;
        let (status, _, out) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_service_level_is_rejected() {
        let body = r#"{"area":{"areaType":"GEOHASHLIST","geohashes":["bcde"]},"startTime":"2024-01-01T00:00:00Z","endTime":"2024-01-02T00:00:00Z"}"#;
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
            post_retrieve(Some(&token), &body_with(r#""bcde""#), Some("corr-pcd")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-pcd")
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
