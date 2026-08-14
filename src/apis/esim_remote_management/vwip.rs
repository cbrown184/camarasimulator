//! eSIM Remote Management **vwip** (CAMARA eSIM Remote Management, wip).
//!
//! Four endpoints so far:
//! - `POST /esim-remote-management/vwip/profile/downloaded-list` — list the eSIM
//!   profiles installed on a device's eUICC (operationId `profileList`).
//! - `POST /esim-remote-management/vwip/profile/result/query` — query the result
//!   of an asynchronous profile operation by its `taskId` (operationId
//!   `profileResultQuery`). Stateless: the result is deterministic from the
//!   `taskId` (its trailing three digits pick `operResult` — executing / success
//!   / fail — with the device `eId`/`imei`/`iccid` synthesised from the id).
//! - `POST /esim-remote-management/vwip/profile/oper` — perform a lifecycle
//!   operation on a profile — enable / disable / delete (operationId
//!   `profileOperation`, scope `esim-remote-management:oper`). CamaraSim models
//!   the **synchronous acknowledgement** of the command: it validates the request
//!   and answers `code: 0` with the echoed configuration. The device `eId` (in
//!   `config.subscriptionDetail`) is the control plane (reserved suffix →
//!   canonical error, else accepted) and `optType` (1 Enable / 2 Disable / 3
//!   Delete) is surfaced in the response `message`. The actual eUICC state change
//!   and any `sink` callback delivery are **documented cuts** (no live eUICC
//!   engine — mirroring Click-to-Dial's engine cut); the operation's eventual
//!   result is separately pollable via `profileResultQuery`.
//! - `POST /esim-remote-management/vwip/profile/download` — download (and
//!   optionally auto-enable) a new profile onto the device's eUICC (operationId
//!   `profileDownload`, scope `esim-remote-management:download`). Modelled like
//!   `profileOperation` as a **synchronous acknowledgement**: it validates the
//!   request and answers `code: 0` echoing the configuration, with the device
//!   `eId` (in `config.subscriptionDetail`) as the control plane (reserved
//!   suffix → canonical error, else accepted). `autoEnableType` (only `1`,
//!   download-and-enable) is surfaced in the response `message`. The actual
//!   profile download / eUICC state change and any `sink` callback delivery are
//!   documented cuts (no live eUICC engine); the eventual result is separately
//!   pollable via `profileResultQuery`.
//!
//! ## What it does
//!
//! The caller submits a base "CMP" request envelope (`timestamp` / `sequenceNum`
//! / `clientId` / `data`) whose `data.eId` names the device eUICC. The operator
//! answers with a base response envelope (`resultCode` / `resultDesc` /
//! `sequenceNum` / `data`) whose `data` carries the device `imei`, the echoed
//! `eId`, and the list of installed `profiles` — each an `iccid` plus an
//! `enableStatus` (`0` disabled / `1` enabled).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the
//! `esim-remote-management:downloadedlist` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! The device `eId` is the control plane. Its trailing three **decimal** digits
//! (hex letters skipped, mirroring Traffic Influence's hex `appId`) drive the
//! case:
//!
//! - **Reserved error suffix** — trailing three digits naming a reserved CAMARA
//!   status (`…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`,
//!   `…503`) → that canonical CAMARA error (shared [`crate::scenarios`]). The
//!   canonical eSIM spec declares only 400/401/403/404/500/503 for this
//!   operation; CamaraSim exposes the full shared set so every reserved case is
//!   reachable (409/422/429 are CamaraSim extensions, sanctioned by the upstream
//!   spec's own "error list is not exhaustive" note).
//! - **Profile inventory** — otherwise the trailing three digits `d` set the
//!   number of installed profiles (`d % 4`: `…000` / no digits → an empty eUICC,
//!   `…001` → 1, `…002` → 2, `…003` → 3). At most one profile is enabled at a
//!   time (the eUICC rule): the first is enabled, the rest disabled. Each
//!   profile's `iccid` and the device `imei` are derived deterministically from
//!   the `eId` (a stable FNV hash + a Luhn check digit), so the same `eId`
//!   always returns the same inventory.
//!
//! A `data.eId` that is absent or not 32 hex characters → `400 INVALID_ARGUMENT`
//! (CamaraSim requires the `eId` for the query — a documented tightening of the
//! upstream optional field). A malformed `sequenceNum` is likewise
//! `400 INVALID_ARGUMENT`.

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

/// The OAuth2 scope the `profileList` endpoint requires (CAMARA eSIM Remote
/// Management).
const LIST_SCOPE: &str = "esim-remote-management:downloadedlist";

/// The OAuth2 scope the `profileResultQuery` endpoint requires (CAMARA eSIM
/// Remote Management).
const QUERY_SCOPE: &str = "esim-remote-management:query";

/// The OAuth2 scope the `profileOperation` endpoint requires (CAMARA eSIM
/// Remote Management).
const OPER_SCOPE: &str = "esim-remote-management:oper";

/// The OAuth2 scope the `profileDownload` endpoint requires (CAMARA eSIM
/// Remote Management).
const DOWNLOAD_SCOPE: &str = "esim-remote-management:download";

/// The success `resultCode` of the base CMP response envelope (`B100000`).
const RESULT_OK: &str = "B100000";

/// Routes for eSIM Remote Management vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/esim-remote-management/vwip/profile/downloaded-list",
            post(profile_list),
        )
        .route(
            "/esim-remote-management/vwip/profile/result/query",
            post(profile_result_query),
        )
        .route(
            "/esim-remote-management/vwip/profile/oper",
            post(profile_operation),
        )
        .route(
            "/esim-remote-management/vwip/profile/download",
            post(profile_download),
        )
}

/// `POST /profile/downloaded-list` request body — the base CMP envelope
/// (CAMARA `BaseCmpReqEidProfileListReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileListRequest {
    #[allow(dead_code)]
    timestamp: Option<String>,
    #[serde(rename = "sequenceNum")]
    sequence_num: Option<String>,
    #[serde(rename = "clientId")]
    #[allow(dead_code)]
    client_id: Option<String>,
    data: Option<ProfileListData>,
}

/// The `data` payload (CAMARA `EidProfileListReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileListData {
    #[serde(rename = "eId")]
    e_id: Option<String>,
}

/// `POST /esim-remote-management/vwip/profile/downloaded-list`.
async fn profile_list(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(LIST_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required and must parse.
    let req: ProfileListRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid BaseCmpReqEidProfileListReq.",
                &correlator,
            )
        }
    };

    // Syntactic validation (400) before the identifier-driven scenario plane.
    if let Some(seq) = &req.sequence_num {
        if !is_valid_sequence_num(seq) {
            return invalid_argument(
                "`sequenceNum` must match ^[a-zA-Z0-9_-]{1,64}$.",
                &correlator,
            );
        }
    }

    // CamaraSim requires `data.eId` for the query (a documented tightening of the
    // upstream optional field) and validates its 32-hex EID shape.
    let e_id = match req.data.and_then(|d| d.e_id) {
        Some(id) if is_hex32(&id) => id,
        _ => {
            return invalid_argument(
                "`data.eId` is required and must be 32 hexadecimal characters.",
                &correlator,
            )
        }
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&e_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: the eId's trailing three digits set the number of installed
    // profiles; at most one is enabled (the eUICC single-active-profile rule).
    let count = scenarios::trailing_three_digits(&e_id).unwrap_or(0) as usize % 4;
    let profiles: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "iccid": iccid(&e_id, i),
                "enableStatus": if i == 0 { 1 } else { 0 },
            })
        })
        .collect();

    let mut out = json!({
        "resultCode": RESULT_OK,
        "resultDesc": "Success",
        "data": {
            "imei": imei(&e_id),
            "profiles": profiles,
            "eId": e_id,
        },
    });
    // Echo the request's `sequenceNum` when one was supplied.
    if let Some(seq) = &req.sequence_num {
        out["sequenceNum"] = json!(seq);
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// `POST /profile/result/query` request body — the base CMP envelope
/// (CAMARA `BaseCmpReqProfileResultQueryReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultQueryRequest {
    #[allow(dead_code)]
    timestamp: Option<String>,
    #[serde(rename = "sequenceNum")]
    sequence_num: Option<String>,
    #[serde(rename = "clientId")]
    #[allow(dead_code)]
    client_id: Option<String>,
    data: Option<ResultQueryData>,
}

/// The `data` payload (CAMARA `ProfileResultQueryReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultQueryData {
    #[serde(rename = "taskId")]
    task_id: Option<String>,
}

/// `POST /esim-remote-management/vwip/profile/result/query`.
///
/// Queries the result of an asynchronous profile operation (download / enable /
/// disable / delete) by the `taskId` a prior operation returned. CamaraSim is
/// stateless, so the result is deterministic from the `taskId` (docs/DESIGN.md
/// §7): its trailing three decimal digits are the control plane — a reserved
/// error suffix selects a canonical CAMARA error, otherwise `d % 3` picks the
/// task `operResult` (`0` executing / `1` success / `2` fail). The `eId`,
/// `imei`, and per-task `iccid` on the response are derived deterministically
/// from the `taskId`, so the same `taskId` always reports the same result.
async fn profile_result_query(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(QUERY_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required and must parse.
    let req: ResultQueryRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid BaseCmpReqProfileResultQueryReq.",
                &correlator,
            )
        }
    };

    // Syntactic validation (400) before the identifier-driven scenario plane.
    if let Some(seq) = &req.sequence_num {
        if !is_valid_sequence_num(seq) {
            return invalid_argument(
                "`sequenceNum` must match ^[a-zA-Z0-9_-]{1,64}$.",
                &correlator,
            );
        }
    }

    // `data.taskId` is required and follows the CAMARA `^[a-zA-Z0-9_-]{1,64}$`
    // pattern (a query with no task to look up is meaningless).
    let task_id = match req.data.and_then(|d| d.task_id) {
        Some(id) if is_valid_sequence_num(&id) => id,
        _ => {
            return invalid_argument(
                "`data.taskId` is required and must match ^[a-zA-Z0-9_-]{1,64}$.",
                &correlator,
            )
        }
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&task_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: the taskId's trailing three digits pick the task result
    // (`d % 3`: 0 executing / 1 success / 2 fail — all three reachable). The
    // device identifiers are synthesised deterministically from the taskId.
    let d = scenarios::trailing_three_digits(&task_id).unwrap_or(0);
    let oper_result = (d % 3) as i32;
    let result_msg = match oper_result {
        1 => "Operation completed successfully",
        2 => "Operation failed",
        _ => "Operation in progress",
    };
    let e_id = eid_from_task(&task_id);

    let mut out = json!({
        "resultCode": RESULT_OK,
        "resultDesc": "Success",
        "data": {
            "taskId": task_id,
            "imei": imei(&e_id),
            "iccid": iccid(&e_id, 0),
            "operResult": oper_result,
            "eId": e_id,
            "resultMsg": result_msg,
        },
    });
    // Echo the request's `sequenceNum` when one was supplied.
    if let Some(seq) = &req.sequence_num {
        out["sequenceNum"] = json!(seq);
    }

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// `POST /profile/oper` request body — the base CMP subscription-style envelope
/// (CAMARA `BaseCmpReqProfileOperReq`). Unlike the read legs (which use the
/// `timestamp`/`sequenceNum`/`clientId`/`data` CMP envelope), the command legs
/// wrap their payload in a callback-subscription envelope
/// (`protocol`/`sink`/`types`/`config`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileOperRequest {
    /// Callback protocol; the upstream `Protocol` enum admits only `HTTP`.
    protocol: Option<String>,
    /// Callback address for asynchronous notification delivery. Accepted and
    /// validated, but delivery is a documented cut (no live eUICC engine).
    sink: Option<String>,
    /// Subscribed event-type identifiers. Accepted for fidelity.
    types: Option<Vec<String>>,
    config: Option<ProfileOperConfig>,
}

/// The `config` payload (CAMARA `ProfileOperReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileOperConfig {
    #[serde(rename = "subscriptionDetail")]
    subscription_detail: Option<SubscriptionDetail>,
    #[serde(rename = "initialEvent")]
    initial_event: Option<bool>,
    #[serde(rename = "subscriptionMaxEvents")]
    subscription_max_events: Option<i64>,
    #[serde(rename = "subscriptionExpireTime")]
    subscription_expire_time: Option<String>,
}

/// The device + operation selector inside `config.subscriptionDetail`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubscriptionDetail {
    #[serde(rename = "eId")]
    e_id: Option<String>,
    imei: Option<String>,
    iccid: Option<String>,
    #[serde(rename = "optType")]
    opt_type: Option<i64>,
}

/// `POST /esim-remote-management/vwip/profile/oper`.
///
/// Performs a lifecycle operation on an eSIM profile — enable (`optType` 1),
/// disable (`2`), or delete (`3`). The upstream operation is asynchronous (the
/// eventual result is delivered to a `sink` and pollable via
/// `profileResultQuery`); CamaraSim models the **synchronous acknowledgement**:
/// it validates the request and answers `code: 0` echoing the configuration. The
/// device `eId` (`config.subscriptionDetail.eId`) is the control plane
/// (docs/DESIGN.md §7) — a reserved error suffix on its trailing three decimal
/// digits selects a canonical CAMARA error, otherwise the command is accepted.
/// The actual eUICC state change and any `sink` callback delivery are documented
/// cuts (no live eUICC engine).
async fn profile_operation(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(OPER_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required and must parse.
    let req: ProfileOperRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid BaseCmpReqProfileOperReq.",
                &correlator,
            )
        }
    };

    // Envelope-level syntactic validation (400) before the identifier plane.
    if let Some(p) = &req.protocol {
        if p != "HTTP" {
            return invalid_argument("`protocol` must be `HTTP`.", &correlator);
        }
    }
    if let Some(sink) = &req.sink {
        if !is_http_sink(sink) {
            return invalid_argument(
                "`sink` must be an http(s) URL of 1..=256 characters.",
                &correlator,
            );
        }
    }
    if let Some(types) = &req.types {
        if types.len() > 10 || !types.iter().all(|t| is_valid_sequence_num(t)) {
            return invalid_argument(
                "`types` admits at most 10 entries, each matching ^[a-zA-Z0-9_-]{1,64}$.",
                &correlator,
            );
        }
    }

    // `config` and its `subscriptionDetail` are required by CamaraSim (a
    // documented tightening — a command with no target profile is meaningless).
    let config = match req.config {
        Some(c) => c,
        None => return invalid_argument("`config` is required.", &correlator),
    };
    if let Some(max) = config.subscription_max_events {
        if !(1..=1000).contains(&max) {
            return out_of_range(
                "`config.subscriptionMaxEvents` must be within 1..=1000.",
                &correlator,
            );
        }
    }
    let detail = match config.subscription_detail {
        Some(d) => d,
        None => {
            return invalid_argument(
                "`config.subscriptionDetail` is required.",
                &correlator,
            )
        }
    };

    // The device `eId` is required and must be 32 hex characters.
    let e_id = match &detail.e_id {
        Some(id) if is_hex32(id) => id.clone(),
        _ => {
            return invalid_argument(
                "`config.subscriptionDetail.eId` is required and must be 32 hexadecimal characters.",
                &correlator,
            )
        }
    };
    // `optType` is required; the upstream enum admits 1 (Enable) / 2 (Disable) /
    // 3 (Delete). Missing → INVALID_ARGUMENT; present but out of 1..=3 →
    // OUT_OF_RANGE (mirrors the numeric-bounds convention, DESIGN §7/§8).
    let opt_type = match detail.opt_type {
        Some(t) if (1..=3).contains(&t) => t,
        Some(_) => {
            return out_of_range(
                "`config.subscriptionDetail.optType` must be 1 (Enable), 2 (Disable), or 3 (Delete).",
                &correlator,
            )
        }
        None => {
            return invalid_argument(
                "`config.subscriptionDetail.optType` is required.",
                &correlator,
            )
        }
    };
    // Optional device identifiers, validated to their CAMARA shapes when supplied.
    if let Some(imei) = &detail.imei {
        if !is_digits_len(imei, 15, 15) {
            return invalid_argument(
                "`config.subscriptionDetail.imei` must be 15 digits.",
                &correlator,
            );
        }
    }
    if let Some(iccid) = &detail.iccid {
        if !is_digits_len(iccid, 19, 20) {
            return invalid_argument(
                "`config.subscriptionDetail.iccid` must be 19 or 20 digits.",
                &correlator,
            );
        }
    }

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&e_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: acknowledge the accepted command. `optType` selects the
    // operation name surfaced in `message`; the device identity echoes any
    // supplied `imei`/`iccid` or synthesises them deterministically from the
    // `eId` (mirroring `profileList`), so the same device always reports the
    // same identifiers.
    let op = match opt_type {
        1 => "Enable",
        2 => "Disable",
        _ => "Delete",
    };
    let imei_out = detail.imei.clone().unwrap_or_else(|| imei(&e_id));
    let iccid_out = detail.iccid.clone().unwrap_or_else(|| iccid(&e_id, 0));

    let sub = json!({
        "eId": e_id,
        "imei": imei_out,
        "iccid": iccid_out,
        "optType": opt_type,
    });
    // Echo the subscription fields only when supplied.
    let mut config_out = json!({ "subscriptionDetail": sub });
    if let Some(v) = config.initial_event {
        config_out["initialEvent"] = json!(v);
    }
    if let Some(v) = config.subscription_max_events {
        config_out["subscriptionMaxEvents"] = json!(v);
    }
    if let Some(v) = &config.subscription_expire_time {
        config_out["subscriptionExpireTime"] = json!(v);
    }

    let out = json!({
        "code": 0,
        "message": format!("{op} operation accepted"),
        "config": config_out,
    });

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// `POST /profile/download` request body — the base CMP subscription-style
/// envelope (CAMARA `BaseCmpReqProfileDownloadReq`), identical in shape to the
/// `profileOperation` envelope (`protocol` / `sink` / `types` / `config`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDownloadRequest {
    /// Callback protocol; the upstream `Protocol` enum admits only `HTTP`.
    protocol: Option<String>,
    /// Callback address for asynchronous notification delivery. Accepted and
    /// validated, but delivery is a documented cut (no live eUICC engine).
    sink: Option<String>,
    /// Subscribed event-type identifiers. Accepted for fidelity.
    types: Option<Vec<String>>,
    config: Option<ProfileDownloadConfig>,
}

/// The `config` payload (CAMARA `ProfileDownloadReq`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDownloadConfig {
    #[serde(rename = "subscriptionDetail")]
    subscription_detail: Option<DownloadSubscriptionDetail>,
    #[serde(rename = "initialEvent")]
    initial_event: Option<bool>,
    #[serde(rename = "subscriptionMaxEvents")]
    subscription_max_events: Option<i64>,
    #[serde(rename = "subscriptionExpireTime")]
    subscription_expire_time: Option<String>,
}

/// The device + download selector inside `config.subscriptionDetail`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadSubscriptionDetail {
    #[serde(rename = "eId")]
    e_id: Option<String>,
    imei: Option<String>,
    iccid: Option<String>,
    #[serde(rename = "autoEnableType")]
    auto_enable_type: Option<i64>,
}

/// `POST /esim-remote-management/vwip/profile/download`.
///
/// Downloads (and optionally auto-enables) a new eSIM profile onto the device
/// named by `config.subscriptionDetail.eId`. The upstream operation is
/// asynchronous (its eventual result is delivered to a `sink` and pollable via
/// `profileResultQuery`); CamaraSim models the **synchronous acknowledgement**:
/// it validates the request and answers `code: 0` echoing the configuration. The
/// device `eId` is the control plane (docs/DESIGN.md §7) — a reserved error
/// suffix on its trailing three decimal digits selects a canonical CAMARA error,
/// otherwise the command is accepted. When `autoEnableType` is `1` the
/// acknowledgement reports a download-and-enable (the eUICC single-active rule);
/// otherwise a plain download. The actual profile download / eUICC state change
/// and any `sink` callback delivery are documented cuts (no live eUICC engine).
async fn profile_download(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(DOWNLOAD_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required and must parse.
    let req: ProfileDownloadRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid BaseCmpReqProfileDownloadReq.",
                &correlator,
            )
        }
    };

    // Envelope-level syntactic validation (400) before the identifier plane.
    if let Some(p) = &req.protocol {
        if p != "HTTP" {
            return invalid_argument("`protocol` must be `HTTP`.", &correlator);
        }
    }
    if let Some(sink) = &req.sink {
        if !is_http_sink(sink) {
            return invalid_argument(
                "`sink` must be an http(s) URL of 1..=256 characters.",
                &correlator,
            );
        }
    }
    if let Some(types) = &req.types {
        if types.len() > 10 || !types.iter().all(|t| is_valid_sequence_num(t)) {
            return invalid_argument(
                "`types` admits at most 10 entries, each matching ^[a-zA-Z0-9_-]{1,64}$.",
                &correlator,
            );
        }
    }

    // `config` and its `subscriptionDetail` are required by CamaraSim (a
    // documented tightening — a download with no target eUICC is meaningless).
    let config = match req.config {
        Some(c) => c,
        None => return invalid_argument("`config` is required.", &correlator),
    };
    if let Some(max) = config.subscription_max_events {
        if !(1..=1000).contains(&max) {
            return out_of_range(
                "`config.subscriptionMaxEvents` must be within 1..=1000.",
                &correlator,
            );
        }
    }
    let detail = match config.subscription_detail {
        Some(d) => d,
        None => {
            return invalid_argument(
                "`config.subscriptionDetail` is required.",
                &correlator,
            )
        }
    };

    // The device `eId` is required and must be 32 hex characters.
    let e_id = match &detail.e_id {
        Some(id) if is_hex32(id) => id.clone(),
        _ => {
            return invalid_argument(
                "`config.subscriptionDetail.eId` is required and must be 32 hexadecimal characters.",
                &correlator,
            )
        }
    };
    // `autoEnableType` is optional; the upstream enum admits only `1` (download
    // and enable). Present but ≠ 1 → OUT_OF_RANGE (mirrors `optType`'s bounds).
    if let Some(t) = detail.auto_enable_type {
        if t != 1 {
            return out_of_range(
                "`config.subscriptionDetail.autoEnableType` must be 1 (download and enable).",
                &correlator,
            );
        }
    }
    // Optional device identifiers, validated to their CAMARA shapes when supplied.
    if let Some(imei) = &detail.imei {
        if !is_digits_len(imei, 15, 15) {
            return invalid_argument(
                "`config.subscriptionDetail.imei` must be 15 digits.",
                &correlator,
            );
        }
    }
    if let Some(iccid) = &detail.iccid {
        if !is_digits_len(iccid, 19, 20) {
            return invalid_argument(
                "`config.subscriptionDetail.iccid` must be 19 or 20 digits.",
                &correlator,
            );
        }
    }

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&e_id) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Happy path: acknowledge the accepted download. `autoEnableType: 1` marks a
    // download-and-enable; otherwise a plain download. The device identity echoes
    // any supplied `imei`/`iccid` or synthesises them deterministically from the
    // `eId` (mirroring `profileList`/`profileOperation`).
    let auto_enable = detail.auto_enable_type == Some(1);
    let message = if auto_enable {
        "Profile download and enable accepted"
    } else {
        "Profile download accepted"
    };
    let imei_out = detail.imei.clone().unwrap_or_else(|| imei(&e_id));
    let iccid_out = detail.iccid.clone().unwrap_or_else(|| iccid(&e_id, 0));

    let mut sub = json!({
        "eId": e_id,
        "imei": imei_out,
        "iccid": iccid_out,
    });
    // Echo `autoEnableType` only when supplied (it is optional upstream).
    if let Some(t) = detail.auto_enable_type {
        sub["autoEnableType"] = json!(t);
    }
    // Echo the subscription fields only when supplied.
    let mut config_out = json!({ "subscriptionDetail": sub });
    if let Some(v) = config.initial_event {
        config_out["initialEvent"] = json!(v);
    }
    if let Some(v) = config.subscription_max_events {
        config_out["subscriptionMaxEvents"] = json!(v);
    }
    if let Some(v) = &config.subscription_expire_time {
        config_out["subscriptionExpireTime"] = json!(v);
    }

    let out = json!({
        "code": 0,
        "message": message,
        "config": config_out,
    });

    with_correlator((StatusCode::OK, Json(out)).into_response(), &correlator)
}

/// A synthetic device `eId` (32 hex, matching `^[A-Fa-f0-9]{32}$`) derived
/// deterministically from a `taskId`. The `profileResultQuery` request carries
/// only a `taskId`, so the device identity it reports is generated from that id
/// (two 64-bit FNV hashes → 128 bits → 32 hex characters). Stable per taskId.
fn eid_from_task(task_id: &str) -> String {
    let hi = fnv1a_64(&format!("eid-hi:{task_id}"));
    let lo = fnv1a_64(&format!("eid-lo:{task_id}"));
    format!("{hi:016x}{lo:016x}")
}

/// Whether `s` is exactly 32 hexadecimal characters (the CAMARA `eId` pattern
/// `^[A-Fa-f0-9]{32}$`).
fn is_hex32(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether `s` matches the CAMARA `sequenceNum` pattern `^[a-zA-Z0-9_-]{1,64}$`.
fn is_valid_sequence_num(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Whether `s` is an `http://` or `https://` URL of 1..=256 characters (the
/// upstream `sink` shape). A light structural check — CamaraSim does not deliver
/// to the sink (a documented cut), so it validates the surface form only.
fn is_http_sink(s: &str) -> bool {
    (1..=256).contains(&s.len()) && (s.starts_with("http://") || s.starts_with("https://"))
}

/// Whether `s` is all ASCII digits with a length in `min..=max`.
fn is_digits_len(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
}

/// The device IMEI for `eid` — 14 digits derived from a stable hash plus a Luhn
/// check digit (15 digits total; matches `^[0-9]{15}$`). Deterministic, so the
/// same eId always reports the same IMEI. No date/crypto dependency.
fn imei(eid: &str) -> String {
    let h = fnv1a_64(&format!("imei:{eid}"));
    let base = format!("{:014}", h % 100_000_000_000_000); // 14 digits
    let check = luhn_check_digit(&base);
    format!("{base}{check}")
}

/// The ICCID of the `index`-th profile on `eid` — the telecom prefix `89` plus
/// 17 digits derived from a stable hash plus a Luhn check digit (20 digits total;
/// matches `^[0-9]{19,20}$`). Deterministic per (eId, index).
fn iccid(eid: &str, index: usize) -> String {
    let h = fnv1a_64(&format!("iccid:{eid}:{index}"));
    let body = format!("89{:017}", h % 100_000_000_000_000_000); // "89" + 17 = 19 digits
    let check = luhn_check_digit(&body);
    format!("{body}{check}")
}

/// The Luhn check digit that makes `payload` (followed by the digit) pass the
/// Luhn checksum. `payload` must be ASCII digits.
fn luhn_check_digit(payload: &str) -> u8 {
    let mut sum = 0u32;
    // The rightmost payload digit sits at position 2 from the right of the full
    // number (the check digit is position 1), so it is doubled.
    for (i, b) in payload.bytes().rev().enumerate() {
        let mut d = u32::from(b - b'0');
        if i % 2 == 0 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    ((10 - (sum % 10)) % 10) as u8
}

/// FNV-1a 64-bit hash of `s`. Self-contained (no dependency), used only to derive
/// stable, deterministic synthetic identifiers — never for security.
fn fnv1a_64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
}

/// A 400 `OUT_OF_RANGE` CAMARA error, with the correlator echoed (used for the
/// numeric bounds on `optType` / `subscriptionMaxEvents`).
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "esim.local:8080";
    // A 32-hex EID whose trailing decimal digits are `001` → one enabled profile.
    const EID_ONE: &str = "A1B2C3D4E5F600000000000000000001";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn hex32_validation_is_exact() {
        assert!(is_hex32(EID_ONE));
        assert!(is_hex32("abcdef0123456789ABCDEF0123456789"));
        assert!(!is_hex32("A1B2C3")); // too short
        assert!(!is_hex32("A1B2C3D4E5F60000000000000000000")); // 31 chars
        assert!(!is_hex32("A1B2C3D4E5F6000000000000000000012")); // 33 chars
        assert!(!is_hex32("A1B2C3D4E5F600000000000000000G01")); // non-hex `G`
    }

    #[test]
    fn sequence_num_validation_follows_the_pattern() {
        assert!(is_valid_sequence_num("seq-0001"));
        assert!(is_valid_sequence_num("A_b-9"));
        assert!(!is_valid_sequence_num("")); // empty
        assert!(!is_valid_sequence_num("has space"));
        assert!(!is_valid_sequence_num(&"x".repeat(65))); // too long
    }

    #[test]
    fn imei_is_15_digits_and_luhn_valid() {
        let m = imei(EID_ONE);
        assert_eq!(m.len(), 15);
        assert!(m.bytes().all(|b| b.is_ascii_digit()));
        assert!(luhn_valid(&m), "IMEI {m} should pass Luhn");
        // Deterministic.
        assert_eq!(imei(EID_ONE), imei(EID_ONE));
    }

    #[test]
    fn iccid_is_19_or_20_digits_luhn_valid_and_prefixed() {
        let c = iccid(EID_ONE, 0);
        assert_eq!(c.len(), 20);
        assert!(c.starts_with("89"));
        assert!(c.bytes().all(|b| b.is_ascii_digit()));
        assert!(luhn_valid(&c), "ICCID {c} should pass Luhn");
        // Different indices give different ICCIDs.
        assert_ne!(iccid(EID_ONE, 0), iccid(EID_ONE, 1));
    }

    /// Whether a full digit string (payload + check digit) passes Luhn.
    fn luhn_valid(full: &str) -> bool {
        let mut sum = 0u32;
        for (i, b) in full.bytes().rev().enumerate() {
            let mut d = u32::from(b - b'0');
            if i % 2 == 1 {
                d *= 2;
                if d > 9 {
                    d -= 9;
                }
            }
            sum += d;
        }
        sum % 10 == 0
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        let body = format!("grant_type=client_credentials&client_id=esim-client&scope={scope}");
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

    /// POST to the endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_list(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/esim-remote-management/vwip/profile/downloaded-list")
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

    /// Mint the scoped token and call the endpoint.
    async fn call_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(LIST_SCOPE).await;
        post_list(Some(&token), body, None).await
    }

    fn body_for(eid: &str) -> String {
        format!(r#"{{"data":{{"eId":"{eid}"}}}}"#)
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn returns_one_enabled_profile_for_a_001_tail() {
        let (status, _, body) = call_ok(&body_for(EID_ONE)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resultCode"], "B100000");
        assert_eq!(body["data"]["eId"], EID_ONE);
        let profiles = body["data"]["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["enableStatus"], 1);
        // IMEI/ICCID shapes.
        assert_eq!(body["data"]["imei"].as_str().unwrap().len(), 15);
        assert_eq!(profiles[0]["iccid"].as_str().unwrap().len(), 20);
    }

    #[tokio::test]
    async fn empty_euicc_for_a_000_tail() {
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000000")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["data"]["profiles"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn three_profiles_with_only_the_first_enabled_for_a_003_tail() {
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000003")).await;
        assert_eq!(status, StatusCode::OK);
        let profiles = body["data"]["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0]["enableStatus"], 1);
        assert_eq!(profiles[1]["enableStatus"], 0);
        assert_eq!(profiles[2]["enableStatus"], 0);
    }

    #[tokio::test]
    async fn sequence_num_is_echoed_when_supplied() {
        let token = mint_token(LIST_SCOPE).await;
        let body = format!(r#"{{"sequenceNum":"seq-42","data":{{"eId":"{EID_ONE}"}}}}"#);
        let (status, _, out) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["sequenceNum"], "seq-42");
    }

    // --- Reserved-error control plane -------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        // …404 → 404 NOT_FOUND.
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …429 → 429 (a CamaraSim extension beyond the canonical eSIM error set).
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000429")).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
        // …503 → 503.
        let (status, _, body) = call_ok(&body_for("A1B2C3D4E5F600000000000000000503")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn missing_eid_is_rejected() {
        let (status, _, body) = call_ok(r#"{"data":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Also a wholly absent `data`.
        let (status, _, _) = call_ok(r#"{"sequenceNum":"s"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn non_hex_eid_is_rejected() {
        let (status, _, body) = call_ok(&body_for("not-a-valid-eid")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_sequence_num_is_rejected() {
        let token = mint_token(LIST_SCOPE).await;
        let body = format!(r#"{{"sequenceNum":"has space","data":{{"eId":"{EID_ONE}"}}}}"#);
        let (status, _, out) = post_list(Some(&token), &body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = call_ok(r#"{"data":{"eId":"A1B2C3D4E5F600000000000000000001"},"x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = call_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_list(Some(&token), &body_for(EID_ONE), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_list(None, &body_for(EID_ONE), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- Correlator --------------------------------------------------------

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(LIST_SCOPE).await;
        let (status, headers, _) =
            post_list(Some(&token), &body_for(EID_ONE), Some("corr-esim")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-esim")
        );
        // Business error.
        let (status, headers, _) = post_list(
            Some(&token),
            &body_for("A1B2C3D4E5F600000000000000000404"),
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }

    // --- profileResultQuery -------------------------------------------------

    /// POST to the result-query endpoint with an optional Bearer token and
    /// optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_query(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/esim-remote-management/vwip/profile/result/query")
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

    /// Mint the scoped token and call the query endpoint.
    async fn query_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(QUERY_SCOPE).await;
        post_query(Some(&token), body, None).await
    }

    fn query_body_for(task_id: &str) -> String {
        format!(r#"{{"data":{{"taskId":"{task_id}"}}}}"#)
    }

    // Pure scenario unit: the synthesised eId is 32 hex and stable per taskId.
    #[test]
    fn eid_from_task_is_32_hex_and_deterministic() {
        let e = eid_from_task("task-001");
        assert!(is_hex32(&e), "eId {e} should be 32 hex characters");
        assert_eq!(eid_from_task("task-001"), e);
        assert_ne!(eid_from_task("task-001"), eid_from_task("task-002"));
    }

    #[tokio::test]
    async fn executing_result_for_a_000_tail() {
        // …000 → d % 3 == 0 → executing.
        let (status, _, body) = query_ok(&query_body_for("task-000")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resultCode"], "B100000");
        assert_eq!(body["data"]["taskId"], "task-000");
        assert_eq!(body["data"]["operResult"], 0);
        // Device identity is synthesised deterministically from the taskId.
        assert_eq!(body["data"]["imei"].as_str().unwrap().len(), 15);
        assert_eq!(body["data"]["iccid"].as_str().unwrap().len(), 20);
        assert_eq!(body["data"]["eId"].as_str().unwrap().len(), 32);
    }

    #[tokio::test]
    async fn success_result_for_a_001_tail() {
        // …001 → d % 3 == 1 → success.
        let (status, _, body) = query_ok(&query_body_for("task-001")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"]["operResult"], 1);
    }

    #[tokio::test]
    async fn failed_result_for_a_002_tail() {
        // …002 → d % 3 == 2 → fail.
        let (status, _, body) = query_ok(&query_body_for("task-002")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"]["operResult"], 2);
    }

    #[tokio::test]
    async fn query_result_is_deterministic() {
        let (_, _, a) = query_ok(&query_body_for("task-777")).await;
        let (_, _, b) = query_ok(&query_body_for("task-777")).await;
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn query_sequence_num_is_echoed_when_supplied() {
        let token = mint_token(QUERY_SCOPE).await;
        let body = r#"{"sequenceNum":"seq-77","data":{"taskId":"task-001"}}"#;
        let (status, _, out) = post_query(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["sequenceNum"], "seq-77");
    }

    #[tokio::test]
    async fn query_reserved_suffix_selects_a_canonical_camara_error() {
        // taskId ending …404 → 404 NOT_FOUND (the query itself fails).
        let (status, _, body) = query_ok(&query_body_for("task-404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …503 → 503.
        let (status, _, body) = query_ok(&query_body_for("task-503")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn query_missing_task_id_is_rejected() {
        let (status, _, body) = query_ok(r#"{"data":{}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Wholly absent `data` too.
        let (status, _, _) = query_ok(r#"{"sequenceNum":"s"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn query_malformed_task_id_is_rejected() {
        // A space violates the ^[a-zA-Z0-9_-]{1,64}$ pattern.
        let (status, _, body) = query_ok(&query_body_for("has space")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn query_unknown_field_is_rejected() {
        let (status, _, body) = query_ok(r#"{"data":{"taskId":"task-001"},"x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn query_malformed_json_body_is_rejected() {
        let (status, _, body) = query_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn query_token_without_the_scope_is_forbidden() {
        // The profileList scope must not grant the query endpoint.
        let token = mint_token(LIST_SCOPE).await;
        let (status, _, body) = post_query(Some(&token), &query_body_for("task-001"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn query_missing_token_is_unauthenticated() {
        let (status, _, body) = post_query(None, &query_body_for("task-001"), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn query_x_correlator_is_echoed() {
        let token = mint_token(QUERY_SCOPE).await;
        let (status, headers, _) =
            post_query(Some(&token), &query_body_for("task-001"), Some("corr-q")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-q")
        );
    }

    // --- profileOperation --------------------------------------------------

    /// POST to the profile-operation endpoint with an optional Bearer token and
    /// optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_oper(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/esim-remote-management/vwip/profile/oper")
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

    /// Mint the scoped token and call the operation endpoint.
    async fn oper_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(OPER_SCOPE).await;
        post_oper(Some(&token), body, None).await
    }

    /// A minimal `profileOperation` body for `eId` + `optType`.
    fn oper_body(eid: &str, opt_type: i64) -> String {
        format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{eid}","optType":{opt_type}}}}}}}"#
        )
    }

    // --- Pure unit ---------------------------------------------------------

    #[test]
    fn http_sink_and_digit_validators_are_exact() {
        assert!(is_http_sink("http://a"));
        assert!(is_http_sink("https://example.com/cb"));
        assert!(!is_http_sink("ftp://x"));
        assert!(!is_http_sink("")); // too short
        assert!(!is_http_sink(&format!("http://{}", "a".repeat(300)))); // too long
        assert!(is_digits_len("123456789012345", 15, 15));
        assert!(!is_digits_len("12345", 15, 15));
        assert!(!is_digits_len("12345678901234a", 15, 15)); // non-digit
        assert!(is_digits_len("8931089011234567890", 19, 20));
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn enable_operation_is_accepted() {
        let (status, _, body) = oper_ok(&oper_body(EID_ONE, 1)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["code"], 0);
        assert_eq!(body["message"], "Enable operation accepted");
        let detail = &body["config"]["subscriptionDetail"];
        assert_eq!(detail["eId"], EID_ONE);
        assert_eq!(detail["optType"], 1);
        // Device identity is synthesised deterministically from the eId.
        assert_eq!(detail["imei"].as_str().unwrap().len(), 15);
        assert_eq!(detail["iccid"].as_str().unwrap().len(), 20);
    }

    #[tokio::test]
    async fn disable_and_delete_report_their_operation_names() {
        let (_, _, disable) = oper_ok(&oper_body(EID_ONE, 2)).await;
        assert_eq!(disable["message"], "Disable operation accepted");
        assert_eq!(disable["config"]["subscriptionDetail"]["optType"], 2);
        let (_, _, delete) = oper_ok(&oper_body(EID_ONE, 3)).await;
        assert_eq!(delete["message"], "Delete operation accepted");
        assert_eq!(delete["config"]["subscriptionDetail"]["optType"], 3);
    }

    #[tokio::test]
    async fn supplied_imei_and_iccid_are_echoed() {
        let body = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","imei":"356938035643809","iccid":"8931089011234567890","optType":1}}}}}}"#
        );
        let (status, _, out) = oper_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let detail = &out["config"]["subscriptionDetail"];
        assert_eq!(detail["imei"], "356938035643809");
        assert_eq!(detail["iccid"], "8931089011234567890");
    }

    #[tokio::test]
    async fn subscription_fields_are_echoed_when_supplied() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://cb.example/notify","types":["profile-enabled"],"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","optType":1}},"initialEvent":true,"subscriptionMaxEvents":5,"subscriptionExpireTime":"2026-08-14T12:00:00Z"}}}}"#
        );
        let (status, _, out) = oper_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["config"]["initialEvent"], true);
        assert_eq!(out["config"]["subscriptionMaxEvents"], 5);
        assert_eq!(out["config"]["subscriptionExpireTime"], "2026-08-14T12:00:00Z");
    }

    #[tokio::test]
    async fn operation_result_is_deterministic() {
        let (_, _, a) = oper_ok(&oper_body(EID_ONE, 1)).await;
        let (_, _, b) = oper_ok(&oper_body(EID_ONE, 1)).await;
        assert_eq!(a, b);
    }

    // --- Reserved-error control plane -------------------------------------

    #[tokio::test]
    async fn oper_reserved_suffix_selects_a_canonical_camara_error() {
        // eId …404 → 404 NOT_FOUND (the command is rejected).
        let (status, _, body) = oper_ok(&oper_body("A1B2C3D4E5F600000000000000000404", 1)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …503 → 503.
        let (status, _, body) = oper_ok(&oper_body("A1B2C3D4E5F600000000000000000503", 3)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn missing_config_is_rejected() {
        let (status, _, body) = oper_ok(r#"{"protocol":"HTTP"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_subscription_detail_is_rejected() {
        let (status, _, body) = oper_ok(r#"{"config":{"initialEvent":true}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_or_non_hex_eid_is_rejected() {
        // Missing eId.
        let (status, _, body) = oper_ok(r#"{"config":{"subscriptionDetail":{"optType":1}}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Non-hex eId.
        let (status, _, body) = oper_ok(&oper_body("not-a-valid-eid", 1)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_opt_type_is_invalid_argument() {
        let (status, _, body) =
            oper_ok(&format!(r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}"}}}}}}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn out_of_range_opt_type_is_out_of_range() {
        for bad in [0i64, 4, 99] {
            let (status, _, body) = oper_ok(&oper_body(EID_ONE, bad)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "optType {bad}");
            assert_eq!(body["code"], "OUT_OF_RANGE", "optType {bad}");
        }
    }

    #[tokio::test]
    async fn out_of_range_subscription_max_events_is_rejected() {
        let body = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","optType":1}},"subscriptionMaxEvents":0}}}}"#
        );
        let (status, _, out) = oper_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn malformed_device_identifiers_are_rejected() {
        // Bad imei (not 15 digits).
        let bad_imei = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","imei":"123","optType":1}}}}}}"#
        );
        let (status, _, body) = oper_ok(&bad_imei).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Bad iccid (too short).
        let bad_iccid = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","iccid":"123","optType":1}}}}}}"#
        );
        let (status, _, body) = oper_ok(&bad_iccid).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn bad_protocol_and_sink_are_rejected() {
        let bad_proto = format!(
            r#"{{"protocol":"MQTT","config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","optType":1}}}}}}"#
        );
        let (status, _, body) = oper_ok(&bad_proto).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let bad_sink = format!(
            r#"{{"sink":"ftp://x","config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","optType":1}}}}}}"#
        );
        let (status, _, body) = oper_ok(&bad_sink).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn oper_unknown_field_and_malformed_json_are_rejected() {
        let (status, _, body) = oper_ok(&format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","optType":1}}}},"x":1}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let (status, _, body) = oper_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn oper_scope_is_isolated_from_the_read_scopes() {
        // The query scope must not grant the operation endpoint.
        let token = mint_token(QUERY_SCOPE).await;
        let (status, _, body) = post_oper(Some(&token), &oper_body(EID_ONE, 1), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn oper_missing_token_is_unauthenticated() {
        let (status, _, body) = post_oper(None, &oper_body(EID_ONE, 1), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- Correlator --------------------------------------------------------

    #[tokio::test]
    async fn oper_x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(OPER_SCOPE).await;
        let (status, headers, _) =
            post_oper(Some(&token), &oper_body(EID_ONE, 1), Some("corr-op")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-op")
        );
        let (status, headers, _) = post_oper(
            Some(&token),
            &oper_body("A1B2C3D4E5F600000000000000000404", 1),
            Some("corr-e"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-e")
        );
    }

    // --- profileDownload ---------------------------------------------------

    /// POST to the profile-download endpoint with an optional Bearer token and
    /// optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_download(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/esim-remote-management/vwip/profile/download")
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

    /// Mint the scoped token and call the download endpoint.
    async fn download_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(DOWNLOAD_SCOPE).await;
        post_download(Some(&token), body, None).await
    }

    /// A minimal `profileDownload` body for `eId` (no `autoEnableType`).
    fn download_body(eid: &str) -> String {
        format!(r#"{{"config":{{"subscriptionDetail":{{"eId":"{eid}"}}}}}}"#)
    }

    // --- Success cases -----------------------------------------------------

    #[tokio::test]
    async fn plain_download_is_accepted() {
        let (status, _, body) = download_ok(&download_body(EID_ONE)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["code"], 0);
        assert_eq!(body["message"], "Profile download accepted");
        let detail = &body["config"]["subscriptionDetail"];
        assert_eq!(detail["eId"], EID_ONE);
        // No autoEnableType supplied → none echoed.
        assert!(detail["autoEnableType"].is_null());
        // Device identity is synthesised deterministically from the eId.
        assert_eq!(detail["imei"].as_str().unwrap().len(), 15);
        assert_eq!(detail["iccid"].as_str().unwrap().len(), 20);
    }

    #[tokio::test]
    async fn auto_enable_download_reports_and_echoes_the_flag() {
        let body = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","autoEnableType":1}}}}}}"#
        );
        let (status, _, out) = download_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["message"], "Profile download and enable accepted");
        assert_eq!(out["config"]["subscriptionDetail"]["autoEnableType"], 1);
    }

    #[tokio::test]
    async fn download_supplied_imei_and_iccid_are_echoed() {
        let body = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","imei":"356938035643809","iccid":"8931089011234567890"}}}}}}"#
        );
        let (status, _, out) = download_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        let detail = &out["config"]["subscriptionDetail"];
        assert_eq!(detail["imei"], "356938035643809");
        assert_eq!(detail["iccid"], "8931089011234567890");
    }

    #[tokio::test]
    async fn download_subscription_fields_are_echoed_when_supplied() {
        let body = format!(
            r#"{{"protocol":"HTTP","sink":"https://cb.example/notify","types":["profile-downloaded"],"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","autoEnableType":1}},"initialEvent":true,"subscriptionMaxEvents":5,"subscriptionExpireTime":"2026-08-14T12:00:00Z"}}}}"#
        );
        let (status, _, out) = download_ok(&body).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["config"]["initialEvent"], true);
        assert_eq!(out["config"]["subscriptionMaxEvents"], 5);
        assert_eq!(out["config"]["subscriptionExpireTime"], "2026-08-14T12:00:00Z");
    }

    #[tokio::test]
    async fn download_result_is_deterministic() {
        let (_, _, a) = download_ok(&download_body(EID_ONE)).await;
        let (_, _, b) = download_ok(&download_body(EID_ONE)).await;
        assert_eq!(a, b);
    }

    // --- Reserved-error control plane -------------------------------------

    #[tokio::test]
    async fn download_reserved_suffix_selects_a_canonical_camara_error() {
        // eId …404 → 404 NOT_FOUND (the command is rejected).
        let (status, _, body) =
            download_ok(&download_body("A1B2C3D4E5F600000000000000000404")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
        // …503 → 503.
        let (status, _, body) =
            download_ok(&download_body("A1B2C3D4E5F600000000000000000503")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    // --- Validation --------------------------------------------------------

    #[tokio::test]
    async fn download_missing_config_and_subscription_detail_are_rejected() {
        let (status, _, body) = download_ok(r#"{"protocol":"HTTP"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let (status, _, body) = download_ok(r#"{"config":{"initialEvent":true}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn download_missing_or_non_hex_eid_is_rejected() {
        // Missing eId.
        let (status, _, body) =
            download_ok(r#"{"config":{"subscriptionDetail":{"autoEnableType":1}}}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        // Non-hex eId.
        let (status, _, body) = download_ok(&download_body("not-a-valid-eid")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn download_out_of_range_auto_enable_type_is_out_of_range() {
        for bad in [0i64, 2, 99] {
            let body = format!(
                r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","autoEnableType":{bad}}}}}}}"#
            );
            let (status, _, out) = download_ok(&body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "autoEnableType {bad}");
            assert_eq!(out["code"], "OUT_OF_RANGE", "autoEnableType {bad}");
        }
    }

    #[tokio::test]
    async fn download_out_of_range_subscription_max_events_is_rejected() {
        let body = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}"}},"subscriptionMaxEvents":0}}}}"#
        );
        let (status, _, out) = download_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "OUT_OF_RANGE");
    }

    #[tokio::test]
    async fn download_malformed_device_identifiers_are_rejected() {
        let bad_imei = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","imei":"123"}}}}}}"#
        );
        let (status, _, body) = download_ok(&bad_imei).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let bad_iccid = format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}","iccid":"123"}}}}}}"#
        );
        let (status, _, body) = download_ok(&bad_iccid).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn download_bad_protocol_and_sink_are_rejected() {
        let bad_proto = format!(
            r#"{{"protocol":"MQTT","config":{{"subscriptionDetail":{{"eId":"{EID_ONE}"}}}}}}"#
        );
        let (status, _, body) = download_ok(&bad_proto).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let bad_sink = format!(
            r#"{{"sink":"ftp://x","config":{{"subscriptionDetail":{{"eId":"{EID_ONE}"}}}}}}"#
        );
        let (status, _, body) = download_ok(&bad_sink).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn download_unknown_field_and_malformed_json_are_rejected() {
        let (status, _, body) = download_ok(&format!(
            r#"{{"config":{{"subscriptionDetail":{{"eId":"{EID_ONE}"}}}},"x":1}}"#
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
        let (status, _, body) = download_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Auth --------------------------------------------------------------

    #[tokio::test]
    async fn download_scope_is_isolated_from_the_other_scopes() {
        // The operation scope must not grant the download endpoint.
        let token = mint_token(OPER_SCOPE).await;
        let (status, _, body) = post_download(Some(&token), &download_body(EID_ONE), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn download_missing_token_is_unauthenticated() {
        let (status, _, body) = post_download(None, &download_body(EID_ONE), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- Correlator --------------------------------------------------------

    #[tokio::test]
    async fn download_x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(DOWNLOAD_SCOPE).await;
        let (status, headers, _) =
            post_download(Some(&token), &download_body(EID_ONE), Some("corr-dl")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-dl")
        );
        let (status, headers, _) = post_download(
            Some(&token),
            &download_body("A1B2C3D4E5F600000000000000000404"),
            Some("corr-de"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-de")
        );
    }
}
