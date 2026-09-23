//! Region Device Count **v0.2** (CAMARA Region Device Count 0.2.0, r2.2).
//!
//! One endpoint:
//! - `POST /region-device-count/v0.2/count` — how many devices are in a region
//!   (a `CIRCLE` or a `POLYGON`) during an optional time interval?
//!   (operationId `count`).
//!
//! ## What it does
//!
//! The caller submits an `area` and the operator answers
//! `{ "count"?: …, "status": … }` — an aggregate device count plus a coverage
//! `status` from the CAMARA `RegionDeviceCountResponse` enum. Unlike the rest of
//! CamaraSim's APIs this one is **not** identifier-keyed (there is no phone /
//! device id); it is protected by a two-legged access token
//! ([`crate::auth::verify::Claims`]) carrying the `region-device-count:count`
//! scope, and its functional cases are driven by the **area geometry** and the
//! optional `filter` (docs/DESIGN.md §7).
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two control planes key off the region's **characteristic radius** `r`
//! (a `CIRCLE`'s `radius` in metres; a `POLYGON`'s `sqrt(area / π)`), plus the
//! optional `filter`:
//!
//! - **Region size** — a region too large to answer:
//!   - `r > 1 000 000` m → `400 REGION_DEVICE_COUNT.UNSUPPPORTED_REQUEST` (too
//!     large for any processing; the triple-`P` code is verbatim from CAMARA).
//!   - `500 000 < r ≤ 1 000 000` m → the region can only be answered
//!     asynchronously: with no `sink` → `400
//!     REGION_DEVICE_COUNT.UNSUPPORTED_SYNC_RESPONSE`; with a `sink` it is
//!     accepted and answered **synchronously** (async delivery is not modelled —
//!     a documented cut).
//! - **Status / count** — otherwise the trailing three digits `d` of `round(r)`
//!   pick the coverage case (shared reading of the trailing digits,
//!   [`crate::scenarios::trailing_three_digits`]):
//!   - `d == 429` → `429 TOO_MANY_REQUESTS` (a rate limit reachable from input).
//!   - `d == 1` → `PART_OF_AREA_NOT_SUPPORTED` (a `count` for the covered ~50%).
//!   - `d == 2` → `AREA_NOT_SUPPORTED` (no `count`).
//!   - `d == 3` → `DENSITY_BELOW_PRIVACY_THRESHOLD` (no `count`).
//!   - `d == 4` → `TIME_INTERVAL_NO_DATA_FOUND` (no `count`).
//!   - any other `d` (incl. `0` / fewer than three digits) → `SUPPORTED_AREA`
//!     (with a `count`).
//! - **Filter** — when `SUPPORTED_AREA` / `PART_OF_AREA_NOT_SUPPORTED` carry a
//!   `count`, the count is proportional to the region's area (a fixed density)
//!   and **narrowed by `filter`**: each selected `roamingStatus` / `deviceType`
//!   category contributes its share, so `filter` is a genuine second control
//!   plane (the same region returns a smaller count when filtered).
//!
//! ## Validation (before the control planes)
//!
//! - `CIRCLE` missing `center` or `radius` → `400
//!   REGION_DEVICE_COUNT.INVALID_CIRCLE_AREA`; `radius < 1` or a `center` out of
//!   range → `400 INVALID_ARGUMENT`.
//! - `POLYGON` `boundary` not 3–15 valid points (or a degenerate zero-area
//!   shape) → `400 REGION_DEVICE_COUNT.INVALID_POLYGON_AREA`.
//! - `starttime`/`endtime` supplied one-without-the-other → `400
//!   REGION_DEVICE_COUNT.TIME_INVALID_ARGUMENT`; `endtime` before `starttime` →
//!   `400 REGION_DEVICE_COUNT.INVALID_END_DATE`; a malformed timestamp → `400
//!   INVALID_ARGUMENT`.
//! - `filter` present but empty, or an out-of-enum value → `400 INVALID_ARGUMENT`.
//! - `sinkCredential.credentialType` other than `ACCESSTOKEN` → `400
//!   INVALID_CREDENTIAL`; a non-`bearer` `accessTokenType` → `400 INVALID_TOKEN`.
//!
//! `x-correlator` is echoed on every response (CAMARA Commonalities).

use std::f64::consts::PI;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /count` endpoint requires (CAMARA Region Device
/// Count 0.2.0).
const COUNT_SCOPE: &str = "region-device-count:count";

/// Devices per km² the simulator assumes when turning a region's area into a
/// count. A fixed, documented density so the count is deterministic from the
/// geometry (docs/DESIGN.md §7).
const DENSITY_PER_KM2: f64 = 500.0;

/// Characteristic radius (m) above which a region is too large for **any**
/// processing → `UNSUPPPORTED_REQUEST`.
const MAX_PROCESSABLE_RADIUS_M: f64 = 1_000_000.0;

/// Characteristic radius (m) above which a region can only be answered
/// asynchronously (→ `UNSUPPORTED_SYNC_RESPONSE` unless a `sink` is supplied).
const MAX_SYNC_RADIUS_M: f64 = 500_000.0;

/// Routes for Region Device Count v0.2, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/region-device-count/v0.2/count", post(count))
}

/// `POST /count` request body (CAMARA `RegionDeviceCountRequestBody`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountRequest {
    area: AreaRaw,
    starttime: Option<String>,
    endtime: Option<String>,
    filter: Option<Filter>,
    #[allow(dead_code)]
    sink: Option<String>,
    #[serde(rename = "sinkCredential")]
    sink_credential: Option<SinkCredential>,
}

/// The CAMARA `Area` discriminated union, read leniently so CamaraSim can emit
/// the specific `INVALID_CIRCLE_AREA` / `INVALID_POLYGON_AREA` codes rather than
/// a generic parse failure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AreaRaw {
    #[serde(rename = "areaType")]
    area_type: String,
    center: Option<Point>,
    radius: Option<f64>,
    boundary: Option<Vec<Point>>,
}

/// The CAMARA `Point` (latitude/longitude, degrees).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    latitude: f64,
    longitude: f64,
}

/// The CAMARA `Filter`: at least one of the two criteria must be present, each a
/// non-empty array of enum values.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Filter {
    #[serde(rename = "roamingStatus")]
    roaming_status: Option<Vec<String>>,
    #[serde(rename = "deviceType")]
    device_type: Option<Vec<String>>,
}

/// The CAMARA `SinkCredential`, read leniently (all shapes' fields optional) so a
/// non-`ACCESSTOKEN` credential parses and is then rejected by its
/// `credentialType`.
#[derive(Debug, Deserialize)]
struct SinkCredential {
    #[serde(rename = "credentialType")]
    credential_type: Option<String>,
    #[serde(rename = "accessTokenType")]
    access_token_type: Option<String>,
}

/// A validated region: its area (m²) and characteristic radius (m).
struct Region {
    area_m2: f64,
    radius_m: f64,
}

/// `POST /region-device-count/v0.2/count`.
async fn count(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(COUNT_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: CountRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid RegionDeviceCountRequestBody.",
                &correlator,
            )
        }
    };

    // Validate the area and reduce it to (area, characteristic radius).
    let region = match validate_area(&req.area, &correlator) {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    // Validate the optional time interval.
    if let Err(resp) = validate_times(req.starttime.as_deref(), req.endtime.as_deref(), &correlator)
    {
        return resp;
    }

    // Validate the optional filter.
    if let Some(filter) = &req.filter {
        if let Err(resp) = validate_filter(filter, &correlator) {
            return resp;
        }
    }

    // Validate the optional sink credential.
    if let Some(cred) = &req.sink_credential {
        if let Err(resp) = validate_sink_credential(cred, &correlator) {
            return resp;
        }
    }

    // Control plane 1 — region size (docs/DESIGN.md §7).
    if region.radius_m > MAX_PROCESSABLE_RADIUS_M {
        return camara_400(
            "REGION_DEVICE_COUNT.UNSUPPPORTED_REQUEST",
            "The requested area/time is too large to be processed.",
            &correlator,
        );
    }
    if region.radius_m > MAX_SYNC_RADIUS_M && req.sink.is_none() {
        return camara_400(
            "REGION_DEVICE_COUNT.UNSUPPORTED_SYNC_RESPONSE",
            "The requested area/time is too large for a synchronous response; supply a `sink`.",
            &correlator,
        );
    }

    // Control plane 2 — the trailing three digits of the characteristic radius.
    let d = scenarios::trailing_three_digits(&format!("{}", region.radius_m.round() as i64))
        .unwrap_or(0);

    let out = match d {
        429 => {
            return with_correlator(
                CamaraError::too_many_requests("Rate limit reached.").into_response(),
                &correlator,
            )
        }
        1 => {
            // Partial coverage — a count for the covered portion (~50%).
            let c = device_count(&region, req.filter.as_ref()) / 2;
            json!({ "count": c, "status": "PART_OF_AREA_NOT_SUPPORTED" })
        }
        2 => json!({ "status": "AREA_NOT_SUPPORTED" }),
        3 => json!({ "status": "DENSITY_BELOW_PRIVACY_THRESHOLD" }),
        4 => json!({ "status": "TIME_INTERVAL_NO_DATA_FOUND" }),
        _ => {
            let c = device_count(&region, req.filter.as_ref());
            json!({ "count": c, "status": "SUPPORTED_AREA" })
        }
    };

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// The device count for a region, proportional to its area at a fixed density
/// and narrowed by the optional `filter` (a genuine control plane, DESIGN §7).
fn device_count(region: &Region, filter: Option<&Filter>) -> i64 {
    let area_km2 = region.area_m2 / 1_000_000.0;
    let base = DENSITY_PER_KM2 * area_km2;
    let fraction = filter.map_or(1.0, filter_fraction);
    (base * fraction).round() as i64
}

/// The fraction of devices selected by a `filter`. Each dimension (roaming,
/// device type) contributes the sum of its selected categories' shares; an
/// omitted dimension is `1.0` (unrestricted). The category shares are fixed and
/// documented so a filtered count is deterministic.
fn filter_fraction(filter: &Filter) -> f64 {
    fn roaming_share(v: &str) -> f64 {
        match v {
            "non-roaming" => 0.85,
            "roaming" => 0.15,
            _ => 0.0,
        }
    }
    fn device_share(v: &str) -> f64 {
        match v {
            "human device" => 0.70,
            "IoT device" => 0.25,
            "other" => 0.05,
            _ => 0.0,
        }
    }
    let rf = filter
        .roaming_status
        .as_ref()
        .map_or(1.0, |xs| xs.iter().map(|v| roaming_share(v)).sum());
    let df = filter
        .device_type
        .as_ref()
        .map_or(1.0, |xs| xs.iter().map(|v| device_share(v)).sum());
    rf * df
}

/// Validate the `area` and reduce it to a [`Region`] (area m² + characteristic
/// radius m). Emits the CAMARA area-specific 400 codes.
fn validate_area(area: &AreaRaw, correlator: &Option<HeaderValue>) -> Result<Region, Response> {
    match area.area_type.as_str() {
        "CIRCLE" => {
            let (Some(center), Some(radius)) = (area.center, area.radius) else {
                return Err(camara_400(
                    "REGION_DEVICE_COUNT.INVALID_CIRCLE_AREA",
                    "A CIRCLE area requires both `center` and `radius`.",
                    correlator,
                ));
            };
            if !radius.is_finite() || radius < 1.0 {
                return Err(invalid_argument_resp(
                    "`radius` must be a number of at least 1 metre.",
                    correlator,
                ));
            }
            if !point_in_range(&center) {
                return Err(invalid_argument_resp(
                    "`center` latitude/longitude is out of range.",
                    correlator,
                ));
            }
            Ok(Region {
                area_m2: PI * radius * radius,
                radius_m: radius,
            })
        }
        "POLYGON" => {
            let boundary = area.boundary.as_deref().unwrap_or(&[]);
            if !(3..=15).contains(&boundary.len()) || !boundary.iter().all(point_in_range) {
                return Err(camara_400(
                    "REGION_DEVICE_COUNT.INVALID_POLYGON_AREA",
                    "A POLYGON `boundary` must be 3–15 valid points.",
                    correlator,
                ));
            }
            let area_m2 = polygon_area_m2(boundary);
            if area_m2 <= 0.0 {
                return Err(camara_400(
                    "REGION_DEVICE_COUNT.INVALID_POLYGON_AREA",
                    "The POLYGON is degenerate (zero area).",
                    correlator,
                ));
            }
            Ok(Region {
                area_m2,
                radius_m: (area_m2 / PI).sqrt(),
            })
        }
        _ => Err(invalid_argument_resp(
            "`areaType` must be `CIRCLE` or `POLYGON`.",
            correlator,
        )),
    }
}

/// Whether a point's latitude/longitude are within valid ranges.
fn point_in_range(p: &Point) -> bool {
    (-90.0..=90.0).contains(&p.latitude) && (-180.0..=180.0).contains(&p.longitude)
}

/// Approximate planar area (m²) of a lat/long polygon via the shoelace formula on
/// an equirectangular projection about the first vertex. Good enough for the
/// simulator's deterministic count (no geodesy dependency).
fn polygon_area_m2(points: &[Point]) -> f64 {
    const M_PER_DEG_LAT: f64 = 110_540.0;
    const M_PER_DEG_LON: f64 = 111_320.0;
    let lat0 = points[0].latitude.to_radians();
    let cos_lat0 = lat0.cos();
    let project = |p: &Point| -> (f64, f64) {
        let x = (p.longitude - points[0].longitude) * M_PER_DEG_LON * cos_lat0;
        let y = (p.latitude - points[0].latitude) * M_PER_DEG_LAT;
        (x, y)
    };
    let mut sum = 0.0;
    for i in 0..points.len() {
        let (x0, y0) = project(&points[i]);
        let (x1, y1) = project(&points[(i + 1) % points.len()]);
        sum += x0 * y1 - x1 * y0;
    }
    (sum / 2.0).abs()
}

/// Validate the optional `starttime`/`endtime` interval (both-or-neither;
/// end ≥ start; well-formed RFC 3339).
fn validate_times(
    start: Option<&str>,
    end: Option<&str>,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    match (start, end) {
        (None, None) => Ok(()),
        (Some(_), None) | (None, Some(_)) => Err(camara_400(
            "REGION_DEVICE_COUNT.TIME_INVALID_ARGUMENT",
            "`starttime` and `endtime` must be supplied together.",
            correlator,
        )),
        (Some(s), Some(e)) => {
            let (Some(s_epoch), Some(e_epoch)) = (parse_rfc3339(s), parse_rfc3339(e)) else {
                return Err(invalid_argument_resp(
                    "`starttime`/`endtime` must be RFC 3339 date-times.",
                    correlator,
                ));
            };
            if e_epoch < s_epoch {
                return Err(camara_400(
                    "REGION_DEVICE_COUNT.INVALID_END_DATE",
                    "`endtime` must not be earlier than `starttime`.",
                    correlator,
                ));
            }
            Ok(())
        }
    }
}

/// Validate a `filter`: at least one criterion present, each a non-empty array of
/// known enum values.
fn validate_filter(filter: &Filter, correlator: &Option<HeaderValue>) -> Result<(), Response> {
    let roaming_ok = filter
        .roaming_status
        .as_ref()
        .map(|xs| !xs.is_empty() && xs.iter().all(|v| matches!(v.as_str(), "roaming" | "non-roaming")));
    let device_ok = filter.device_type.as_ref().map(|xs| {
        !xs.is_empty()
            && xs
                .iter()
                .all(|v| matches!(v.as_str(), "human device" | "IoT device" | "other"))
    });
    // At least one criterion must be present.
    if roaming_ok.is_none() && device_ok.is_none() {
        return Err(invalid_argument_resp(
            "`filter` must contain at least one of `roamingStatus`/`deviceType`.",
            correlator,
        ));
    }
    // Any present criterion must be a non-empty array of valid enum values.
    if roaming_ok == Some(false) || device_ok == Some(false) {
        return Err(invalid_argument_resp(
            "`filter` values must be non-empty and from the allowed enums.",
            correlator,
        ));
    }
    Ok(())
}

/// Validate a `sinkCredential`: only `ACCESSTOKEN` with a `bearer`
/// `accessTokenType` is supported.
fn validate_sink_credential(
    cred: &SinkCredential,
    correlator: &Option<HeaderValue>,
) -> Result<(), Response> {
    if cred.credential_type.as_deref() != Some("ACCESSTOKEN") {
        return Err(camara_400(
            "INVALID_CREDENTIAL",
            "Only an ACCESSTOKEN `sinkCredential` is supported.",
            correlator,
        ));
    }
    if cred.access_token_type.as_deref() != Some("bearer") {
        return Err(camara_400(
            "INVALID_TOKEN",
            "`accessTokenType` must be `bearer`.",
            correlator,
        ));
    }
    Ok(())
}

/// A 400 CAMARA error with a caller-chosen `code`, correlator echoed.
fn camara_400(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::BAD_REQUEST, code, message).into_response(),
        correlator,
    )
}

/// A 400 `INVALID_ARGUMENT` CAMARA error response, correlator echoed.
fn invalid_argument_resp(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
}

/// A 400 `INVALID_ARGUMENT` for a malformed body (before a correlator borrow).
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    invalid_argument_resp(message, correlator)
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
/// needs no date/time dependency.
fn parse_rfc3339(s: &str) -> Option<i64> {
    // Split "<date>T<time>".
    let (date, rest) = s.split_once(['T', 't'])?;
    let mut d = date.splitn(3, '-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: u32 = d.next()?.parse().ok()?;
    let day: u32 = d.next()?.parse().ok()?;
    if d.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Separate the offset (Z / ±HH:MM) from the time-of-day.
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

    // Drop a fractional second if present.
    let time = time.split_once('.').map_or(time, |(hms, _)| hms);
    let mut hms = time.splitn(3, ':');
    let hh: i64 = hms.next()?.parse().ok()?;
    let mm: i64 = hms.next()?.parse().ok()?;
    let ss: i64 = hms.next().unwrap_or("0").parse().ok()?;
    if hh_invalid(hh, mm, ss) {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss - offset_secs)
}

/// Basic time-of-day range check (leap second `ss == 60` tolerated).
fn hh_invalid(hh: i64, mm: i64, ss: i64) -> bool {
    !(0..=23).contains(&hh) || !(0..=59).contains(&mm) || !(0..=60).contains(&ss)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's `days_from_civil`,
/// proleptic Gregorian). Self-contained, no date dependency.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64; // [0, 399]
    let m = m as i64;
    let d = d as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// Seconds since the Unix epoch, UTC. Unused directly but kept for parity with
/// sibling modules that stamp `now`; the simulator treats time as always fresh.
#[allow(dead_code)]
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "rdc.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn circle_area_and_radius() {
        let region = Region {
            area_m2: PI * 2000.0 * 2000.0,
            radius_m: 2000.0,
        };
        // 500 dev/km² over π·2² km² ≈ 6283 devices.
        assert_eq!(device_count(&region, None), 6283);
    }

    #[test]
    fn filter_narrows_the_count() {
        let region = Region {
            area_m2: PI * 2000.0 * 2000.0,
            radius_m: 2000.0,
        };
        let full = device_count(&region, None);
        // non-roaming (0.85) × human device (0.70) = 0.595 of the total.
        let filter = Filter {
            roaming_status: Some(vec!["non-roaming".into()]),
            device_type: Some(vec!["human device".into()]),
        };
        let narrowed = device_count(&region, Some(&filter));
        assert!(narrowed < full);
        assert_eq!(narrowed, (full as f64 * 0.595).round() as i64);
    }

    #[test]
    fn polygon_area_is_positive_for_a_square() {
        // A ~0.01°×0.01° square near the equator.
        let square = [
            Point { latitude: 0.0, longitude: 0.0 },
            Point { latitude: 0.0, longitude: 0.01 },
            Point { latitude: 0.01, longitude: 0.01 },
            Point { latitude: 0.01, longitude: 0.0 },
        ];
        let area = polygon_area_m2(&square);
        // ~1113m × ~1105m ≈ 1.23e6 m².
        assert!(area > 1.0e6 && area < 1.4e6, "area was {area}");
    }

    #[test]
    fn degenerate_polygon_has_zero_area() {
        let line = [
            Point { latitude: 0.0, longitude: 0.0 },
            Point { latitude: 0.0, longitude: 0.01 },
            Point { latitude: 0.0, longitude: 0.02 },
        ];
        assert!(polygon_area_m2(&line) < 1.0);
    }

    #[test]
    fn rfc3339_parsing() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2024-01-01T00:00:00Z"), Some(1_704_067_200));
        // Fractional seconds ignored.
        assert_eq!(
            parse_rfc3339("2024-01-01T00:00:00.312Z"),
            Some(1_704_067_200)
        );
        // +02:00 offset → two hours earlier in UTC.
        assert_eq!(
            parse_rfc3339("2024-01-01T02:00:00+02:00"),
            Some(1_704_067_200)
        );
        // No offset → assumed UTC.
        assert_eq!(parse_rfc3339("2024-01-01T00:00:00"), Some(1_704_067_200));
        // Malformed.
        assert_eq!(parse_rfc3339("not-a-date"), None);
        assert_eq!(parse_rfc3339("2024-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339("2024-01-01T25:00:00Z"), None);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=rdc-client&scope={scope}");
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

    async fn post_count(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/region-device-count/v0.2/count")
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

    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(COUNT_SCOPE).await;
        post_count(Some(&token), body, None).await
    }

    fn circle(radius: &str) -> String {
        format!(
            r#"{{"area":{{"areaType":"CIRCLE","center":{{"latitude":0.0,"longitude":0.0}},"radius":{radius}}}}}"#
        )
    }

    // --- Success / status-plane cases -------------------------------------

    #[tokio::test]
    async fn supported_area_returns_a_count() {
        // radius 2000 → …000 → SUPPORTED_AREA with a count.
        let (status, _, body) = call_ok(&circle("2000")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "SUPPORTED_AREA");
        assert_eq!(body["count"], 6283);
    }

    #[tokio::test]
    async fn part_of_area_returns_half_the_count() {
        // radius 2001 → …001 → PART_OF_AREA_NOT_SUPPORTED with ~half the count.
        let (status, _, body) = call_ok(&circle("2001")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "PART_OF_AREA_NOT_SUPPORTED");
        assert!(body["count"].is_number());
    }

    #[tokio::test]
    async fn area_not_supported_has_no_count() {
        // radius 2002 → …002 → AREA_NOT_SUPPORTED, no count.
        let (status, _, body) = call_ok(&circle("2002")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "AREA_NOT_SUPPORTED");
        assert!(body.get("count").is_none());
    }

    #[tokio::test]
    async fn density_below_threshold_has_no_count() {
        let (status, _, body) = call_ok(&circle("2003")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "DENSITY_BELOW_PRIVACY_THRESHOLD");
        assert!(body.get("count").is_none());
    }

    #[tokio::test]
    async fn no_data_for_interval_has_no_count() {
        let (status, _, body) = call_ok(&circle("2004")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "TIME_INTERVAL_NO_DATA_FOUND");
        assert!(body.get("count").is_none());
    }

    #[tokio::test]
    async fn reserved_429_tail_is_too_many_requests() {
        // radius 2429 → …429 → 429.
        let (status, _, body) = call_ok(&circle("2429")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn filter_narrows_the_count_over_the_wire() {
        let full = call_ok(&circle("2000")).await.2["count"].as_i64().unwrap();
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"filter":{"deviceType":["IoT device"]}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::OK);
        let narrowed = resp["count"].as_i64().unwrap();
        assert!(narrowed < full, "{narrowed} !< {full}");
    }

    #[tokio::test]
    async fn polygon_area_is_supported() {
        let body = r#"{"area":{"areaType":"POLYGON","boundary":[{"latitude":0.0,"longitude":0.0},{"latitude":0.0,"longitude":0.02},{"latitude":0.02,"longitude":0.0}]}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::OK);
        assert!(resp["status"].is_string());
    }

    // --- Region-size plane -------------------------------------------------

    #[tokio::test]
    async fn oversized_region_is_unprocessable() {
        // radius > 1e6 → UNSUPPPORTED_REQUEST (triple-P, verbatim).
        let (status, _, body) = call_ok(&circle("1000001")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "REGION_DEVICE_COUNT.UNSUPPPORTED_REQUEST");
    }

    #[tokio::test]
    async fn large_region_without_sink_needs_async() {
        // 5e5 < radius <= 1e6 and no sink → UNSUPPORTED_SYNC_RESPONSE.
        let (status, _, body) = call_ok(&circle("600000")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "REGION_DEVICE_COUNT.UNSUPPORTED_SYNC_RESPONSE");
    }

    #[tokio::test]
    async fn large_region_with_sink_is_answered_sync() {
        // Same size, but a sink is supplied → accepted, answered synchronously.
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":600000},"sink":"http://example.test/cb"}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["status"], "SUPPORTED_AREA");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn circle_missing_radius_is_invalid_circle_area() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0}}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "REGION_DEVICE_COUNT.INVALID_CIRCLE_AREA");
    }

    #[tokio::test]
    async fn circle_radius_below_one_is_invalid_argument() {
        let (status, _, resp) = call_ok(&circle("0")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn out_of_range_center_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":99.0,"longitude":0.0},"radius":2000}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn polygon_with_too_few_points_is_invalid_polygon_area() {
        let body = r#"{"area":{"areaType":"POLYGON","boundary":[{"latitude":0.0,"longitude":0.0},{"latitude":0.0,"longitude":0.01}]}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "REGION_DEVICE_COUNT.INVALID_POLYGON_AREA");
    }

    #[tokio::test]
    async fn unknown_area_type_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"TRIANGLE","radius":2000}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn one_sided_time_is_time_invalid_argument() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"starttime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "REGION_DEVICE_COUNT.TIME_INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn end_before_start_is_invalid_end_date() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"starttime":"2024-01-02T00:00:00Z","endtime":"2024-01-01T00:00:00Z"}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "REGION_DEVICE_COUNT.INVALID_END_DATE");
    }

    #[tokio::test]
    async fn valid_time_interval_is_accepted() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"starttime":"2024-01-01T00:00:00Z","endtime":"2024-01-02T00:00:00Z"}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["status"], "SUPPORTED_AREA");
    }

    #[tokio::test]
    async fn empty_filter_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"filter":{}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bad_filter_enum_is_invalid_argument() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"filter":{"roamingStatus":["sometimes"]}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_accesstoken_sink_credential_is_invalid_credential() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"sink":"http://x.test/cb","sinkCredential":{"credentialType":"PLAIN","identifier":"u","secret":"p"}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_CREDENTIAL");
    }

    #[tokio::test]
    async fn non_bearer_access_token_type_is_invalid_token() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"sink":"http://x.test/cb","sinkCredential":{"credentialType":"ACCESSTOKEN","accessToken":"t","accessTokenExpiresUtc":"2024-01-01T00:00:00Z","accessTokenType":"mac"}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_TOKEN");
    }

    #[tokio::test]
    async fn accesstoken_sink_credential_is_accepted() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"sink":"http://x.test/cb","sinkCredential":{"credentialType":"ACCESSTOKEN","accessToken":"t","accessTokenExpiresUtc":"2024-01-01T00:00:00Z","accessTokenType":"bearer"}}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["status"], "SUPPORTED_AREA");
    }

    #[tokio::test]
    async fn malformed_json_is_invalid_argument() {
        let (status, _, resp) = call_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let body = r#"{"area":{"areaType":"CIRCLE","center":{"latitude":0.0,"longitude":0.0},"radius":2000},"x":1}"#;
        let (status, _, resp) = call_ok(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, resp) = post_count(Some(&token), &circle("2000"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(resp["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, resp) = post_count(None, &circle("2000"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(resp["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(COUNT_SCOPE).await;
        let (status, headers, _) = post_count(Some(&token), &circle("2000"), Some("corr-rdc")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-rdc")
        );
        let (status, headers, _) = post_count(Some(&token), "not json", Some("corr-err")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
