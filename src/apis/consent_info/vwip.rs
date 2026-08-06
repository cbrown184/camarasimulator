//! Consent Info **vwip** (CAMARA ConsentInfo, work-in-progress).
//!
//! One endpoint:
//! - `POST /consent-info/vwip/retrieve` — the current consent status a subscriber
//!   has granted for a set of `scopes` under a declared `purpose`.
//!
//! ## What it does
//!
//! The caller declares the `scopes` it needs and the `purpose` (a
//! `dpv:<Purpose>` token from the W3C Data Privacy Vocabulary) it needs them for,
//! identifies a line, and the operator answers whether consent is currently valid
//! for processing:
//!
//! ```json
//! {
//!   "statusInfo": [
//!     { "scopes": ["kyc-match:match"], "purpose": "dpv:FraudPreventionAndDetection",
//!       "statusValidForProcessing": true, "expirationDate": "2027-08-06T12:00:00Z" }
//!   ]
//! }
//! ```
//!
//! When consent is *not* valid, `statusValidForProcessing` is `false` and
//! `statusReason` gives the reason (`PENDING` / `REQUESTED` / `DENIED` /
//! `EXPIRED` / `OBJECTED`). If the caller set `requestCaptureUrl: true`, the
//! response also carries a top-level `captureUrl` where the end user can be
//! redirected to grant the missing consent.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `consent-info:retrieve` scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by the
//! token **subject**, so resubmitting it is an error (mirrors Subscription
//! Status):
//!
//! - `phoneNumber` present **and** the subject is itself an E.164 number (a
//!   line-authenticated three-legged token) → `422 UNNECESSARY_IDENTIFIER`.
//! - `phoneNumber` present, subject not a line → the submitted number is the
//!   identifier (two-legged).
//! - `phoneNumber` absent, subject is an E.164 number → the subject is the
//!   identifier (three-legged).
//! - `phoneNumber` absent **and** the subject is not a line → the line cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Three independent control planes drive the answer:
//!
//! - **Reserved error suffix (identifier).** If the resolved identifier's trailing
//!   three digits name a reserved CAMARA status (`…400`, `…401`, `…403`, `…404`,
//!   `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with that
//!   canonical CAMARA error (shared [`crate::scenarios`]). `…404` gives the
//!   canonical `NOT_FOUND` (the API's own 404 code is `IDENTIFIER_NOT_FOUND`) and
//!   `…422` gives `SERVICE_NOT_APPLICABLE` (the API's own service-level 422 code).
//! - **Consent status (identifier digits).** Otherwise the identifier's trailing
//!   three digits `d` (`000`–`999`) pick the consent state via `d % 6`:
//!   `0` → valid for processing (no `statusReason`), `1` → `PENDING`,
//!   `2` → `REQUESTED`, `3` → `DENIED`, `4` → `EXPIRED`, `5` → `OBJECTED`. So
//!   `…000` (or an identifier with no digits) is the granted-and-valid default.
//! - **`requestCaptureUrl` (request body).** When consent is *not* valid **and**
//!   the caller set `requestCaptureUrl: true`, the response carries a deterministic
//!   top-level `captureUrl`; otherwise it is omitted (a valid consent needs no
//!   capture, and a caller that did not ask gets none).
//!
//! Two request-level planes select the API's own 403s:
//! - a requested scope containing `forbidden` → `403 NOT_ALLOWED_SCOPES_PURPOSE`
//!   (the scope/purpose combination is not permitted);
//! - a `callbackUrl` that is not a valid `http(s)` URL → `403 INVALID_CALLBACK_URL`.
//!
//! The stateful `CAPTURE_FREQUENCY_EXCEEDED` 403 (rate limit on capture requests)
//! is a documented cut — the simulator is stateless and holds no per-caller rate.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /retrieve` endpoint requires (CAMARA ConsentInfo).
const RETRIEVE_SCOPE: &str = "consent-info:retrieve";

/// Routes for Consent Info vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/consent-info/vwip/retrieve", post(retrieve))
}

/// `POST /retrieve` request body (CAMARA `RetrieveStatusRequestBody`).
///
/// `scopes`, `purpose`, and `requestCaptureUrl` are required; `phoneNumber`
/// (valid only in two-legged auth) and `callbackUrl` are optional. Unknown
/// fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetrieveStatusRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    scopes: Vec<String>,
    purpose: String,
    #[serde(rename = "requestCaptureUrl")]
    request_capture_url: bool,
    #[serde(rename = "callbackUrl")]
    callback_url: Option<String>,
}

/// `POST /consent-info/vwip/retrieve`.
async fn retrieve(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(RETRIEVE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The request body is required (scopes / purpose / requestCaptureUrl are
    // mandatory), so it must be present and parse.
    let req: RetrieveStatusRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid RetrieveStatusRequestBody.",
                &correlator,
            )
        }
    };

    // Request-level structural validation.
    if req.scopes.is_empty() {
        return invalid_argument("`scopes` must contain at least one scope.", &correlator);
    }
    if !is_valid_purpose(&req.purpose) {
        return invalid_argument(
            "`purpose` must match the pattern `^dpv:[a-zA-Z0-9]+$`.",
            &correlator,
        );
    }

    // The scope/purpose combination may be disallowed (API's own 403).
    if req.scopes.iter().any(|s| s.to_ascii_lowercase().contains("forbidden")) {
        return with_correlator(
            CamaraError::new(
                StatusCode::FORBIDDEN,
                "NOT_ALLOWED_SCOPES_PURPOSE",
                "The requested combination of scopes and purpose is not allowed.",
            )
            .into_response(),
            &correlator,
        );
    }

    // A supplied callback URL must be a valid http(s) URL (API's own 403).
    if let Some(cb) = &req.callback_url {
        if !is_valid_http_url(cb) {
            return with_correlator(
                CamaraError::new(
                    StatusCode::FORBIDDEN,
                    "INVALID_CALLBACK_URL",
                    "`callbackUrl` must be a valid http(s) URL.",
                )
                .into_response(),
                &correlator,
            );
        }
    }

    // Resolve the line identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(&req, &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7).
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // The trailing three digits pick the consent state via `d % 6`.
    let d = scenarios::trailing_three_digits(&identifier).unwrap_or(0);
    let reason = consent_reason(d);
    let valid = reason.is_none();

    // Build the single grouped statusInfo entry echoing the requested scopes and
    // purpose. CamaraSim returns one entry covering the requested scope set for
    // the purpose (documented modelling choice; the schema allows 1..N entries).
    let mut status_info = json!({
        "scopes": req.scopes,
        "purpose": req.purpose,
        "statusValidForProcessing": valid,
    });
    if let Some(reason) = reason {
        status_info["statusReason"] = json!(reason);
    }
    // A valid consent carries a future expiry; an EXPIRED one a past expiry (now −
    // d hours). The other not-yet-granted reasons have no expiry to report.
    let now = now_unix_secs();
    if valid {
        status_info["expirationDate"] = json!(rfc3339_utc(now + 365 * 86_400));
    } else if reason == Some("EXPIRED") {
        status_info["expirationDate"] = json!(rfc3339_utc(now - d as i64 * 3600));
    }

    let mut response = json!({ "statusInfo": [status_info] });
    // A capture URL is offered only when consent is not valid and the caller asked.
    if !valid && req.request_capture_url {
        response["captureUrl"] = json!(capture_url(&identifier, &req.purpose));
    }

    with_correlator((StatusCode::OK, Json(response)).into_response(), &correlator)
}

/// The consent `statusReason` for an identifier's trailing three digits, or
/// `None` when consent is valid for processing (`d % 6 == 0`).
fn consent_reason(d: u16) -> Option<&'static str> {
    match d % 6 {
        0 => None,
        1 => Some("PENDING"),
        2 => Some("REQUESTED"),
        3 => Some("DENIED"),
        4 => Some("EXPIRED"),
        _ => Some("OBJECTED"),
    }
}

/// A deterministic, opaque consent-capture URL for an identifier + purpose. The
/// token is an FNV-1a hash (self-contained, no dependency) so the same request
/// always yields the same URL without leaking the identifier.
fn capture_url(identifier: &str, purpose: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in identifier.bytes().chain(b"|".iter().copied()).chain(purpose.bytes()) {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("https://consent.camarasim.example/capture/{hash:016x}")
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Subscription Status). See the module docs for the four cases.
fn resolve_identifier(
    req: &RetrieveStatusRequest,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match &req.phone_number {
        Some(phone) => {
            if !is_valid_e164(phone) {
                return Err(invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    correlator,
                ));
            }
            if subject_is_line {
                return Err(unprocessable(
                    "UNNECESSARY_IDENTIFIER",
                    "The phone number is already identified by the access token.",
                    correlator,
                ));
            }
            Ok(phone.clone())
        }
        None => {
            if subject_is_line {
                Ok(subject.to_string())
            } else {
                Err(unprocessable(
                    "MISSING_IDENTIFIER",
                    "The phone number cannot be identified: supply `phoneNumber` or use a token that identifies a line.",
                    correlator,
                ))
            }
        }
    }
}

/// Whether `s` matches the CAMARA `purpose` pattern `^dpv:[a-zA-Z0-9]+$`: the
/// literal `dpv:` prefix followed by one or more ASCII alphanumerics.
fn is_valid_purpose(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("dpv:") else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A permissive check that `s` is an `http://` or `https://` URL with a non-empty
/// host. Enough to distinguish a plausible callback URL from a malformed one
/// without a URL-parsing dependency.
fn is_valid_http_url(s: &str) -> bool {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"));
    match rest {
        Some(host_and_path) => {
            let host = host_and_path.split(['/', '?', '#']).next().unwrap_or("");
            !host.is_empty() && !host.contains(char::is_whitespace)
        }
        None => false,
    }
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
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
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

/// Whether `s` matches the CAMARA `phoneNumber` pattern `^\+[1-9][0-9]{4,14}$`:
/// a leading `+`, then 5–15 digits, the first of which is non-zero.
fn is_valid_e164(s: &str) -> bool {
    let Some(digits) = s.strip_prefix('+') else {
        return false;
    };
    let bytes = digits.as_bytes();
    (5..=15).contains(&bytes.len())
        && matches!(bytes[0], b'1'..=b'9')
        && bytes.iter().all(u8::is_ascii_digit)
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
/// `2026-08-06T14:27:08Z`. Second precision (the CAMARA schema requires RFC 3339
/// with a time zone but not sub-second digits). Self-contained (no date-time
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
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "ci.local:8080";
    const PATH: &str = "/consent-info/vwip/retrieve";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn purpose_validation_follows_the_camara_pattern() {
        assert!(is_valid_purpose("dpv:FraudPreventionAndDetection"));
        assert!(is_valid_purpose("dpv:Marketing123"));
        assert!(!is_valid_purpose("dpv:")); // no purpose name
        assert!(!is_valid_purpose("FraudPrevention")); // no prefix
        assert!(!is_valid_purpose("dpv:Fraud_Prevention")); // underscore not allowed
        assert!(!is_valid_purpose("dpv:Fraud Prevention")); // space not allowed
    }

    #[test]
    fn http_url_validation_accepts_only_plausible_callbacks() {
        assert!(is_valid_http_url("https://example.com/cb"));
        assert!(is_valid_http_url("http://host:9090/notify"));
        assert!(!is_valid_http_url("ftp://example.com"));
        assert!(!is_valid_http_url("https://"));
        assert!(!is_valid_http_url("not a url"));
    }

    #[test]
    fn consent_reason_cycles_through_the_enum() {
        assert_eq!(consent_reason(0), None);
        assert_eq!(consent_reason(1), Some("PENDING"));
        assert_eq!(consent_reason(4), Some("EXPIRED"));
        assert_eq!(consent_reason(5), Some("OBJECTED"));
        assert_eq!(consent_reason(6), None); // wraps
    }

    #[test]
    fn capture_url_is_deterministic_and_opaque() {
        let a = capture_url("+123456789001", "dpv:Marketing");
        let b = capture_url("+123456789001", "dpv:Marketing");
        let c = capture_url("+123456789002", "dpv:Marketing");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("https://consent.camarasim.example/capture/"));
        assert!(!a.contains("123456789001")); // does not leak the identifier
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "ci-client").await
    }

    /// Mint an access token via `client_credentials` with a caller-chosen
    /// `client_id` (which becomes the token `sub`). `+` is percent-encoded so an
    /// E.164 client id survives the urlencoded body.
    async fn mint_token_with_client(scope: &str, client_id: &str) -> String {
        let enc = client_id.replace('+', "%2B");
        let body = format!("grant_type=client_credentials&client_id={enc}&scope={scope}");
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
    async fn post_retrieve(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(PATH)
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

    /// Mint a scoped (two-legged, non-line subject) token and call the endpoint.
    async fn retrieve_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(RETRIEVE_SCOPE).await;
        post_retrieve(Some(&token), body, None).await
    }

    // --- Consent-status control plane --------------------------------------

    #[tokio::test]
    async fn valid_consent_for_zero_tail_is_the_default() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789000","scopes":["kyc-match:match"],"purpose":"dpv:FraudPreventionAndDetection","requestCaptureUrl":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let info = &body["statusInfo"][0];
        assert_eq!(info["statusValidForProcessing"], true);
        assert!(info.get("statusReason").is_none());
        assert_eq!(info["scopes"][0], "kyc-match:match");
        assert_eq!(info["purpose"], "dpv:FraudPreventionAndDetection");
        // A valid consent carries a future expiry and never a capture URL.
        assert!(info["expirationDate"].as_str().unwrap().ends_with('Z'));
        assert!(body.get("captureUrl").is_none());
    }

    #[tokio::test]
    async fn each_not_granted_reason_is_reachable() {
        // …001 → PENDING, …002 → REQUESTED, …003 → DENIED, …005 → OBJECTED.
        for (tail, reason) in [("001", "PENDING"), ("002", "REQUESTED"), ("003", "DENIED"), ("005", "OBJECTED")] {
            let body = format!(
                r#"{{"phoneNumber":"+123456789{tail}","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}}"#
            );
            let (status, _, resp) = retrieve_ok(&body).await;
            assert_eq!(status, StatusCode::OK, "tail {tail}");
            let info = &resp["statusInfo"][0];
            assert_eq!(info["statusValidForProcessing"], false, "tail {tail}");
            assert_eq!(info["statusReason"], reason, "tail {tail}");
            assert!(info.get("expirationDate").is_none(), "tail {tail} has no expiry");
        }
    }

    #[tokio::test]
    async fn expired_consent_carries_a_past_expiry() {
        // …004 → EXPIRED, which reports an expirationDate in the past.
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789004","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let info = &body["statusInfo"][0];
        assert_eq!(info["statusReason"], "EXPIRED");
        assert!(info["expirationDate"].as_str().unwrap().ends_with('Z'));
    }

    // --- requestCaptureUrl control plane -----------------------------------

    #[tokio::test]
    async fn capture_url_is_offered_only_when_asked_and_not_valid() {
        // Not valid (…001) + requestCaptureUrl:true → captureUrl present.
        let (_, _, with) = retrieve_ok(
            r#"{"phoneNumber":"+123456789001","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":true}"#,
        )
        .await;
        assert!(with["captureUrl"].as_str().unwrap().starts_with("https://"));

        // Not valid (…001) + requestCaptureUrl:false → no captureUrl.
        let (_, _, without) = retrieve_ok(
            r#"{"phoneNumber":"+123456789001","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
        )
        .await;
        assert!(without.get("captureUrl").is_none());
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789404","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789422","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; subject is an E.164 line whose tail is …003 (DENIED).
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789003").await;
        let (status, _, body) = post_retrieve(
            Some(&token),
            r#"{"scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["statusInfo"][0]["statusReason"], "DENIED");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(RETRIEVE_SCOPE, "+123456789012").await;
        let (status, _, body) = post_retrieve(
            Some(&token),
            r#"{"phoneNumber":"+123456789012","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) =
            retrieve_ok(r#"{"scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Request-level 403s ------------------------------------------------

    #[tokio::test]
    async fn a_forbidden_scope_is_not_allowed_for_the_purpose() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789000","scopes":["forbidden:data"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "NOT_ALLOWED_SCOPES_PURPOSE");
    }

    #[tokio::test]
    async fn a_malformed_callback_url_is_rejected() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789000","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false,"callbackUrl":"not-a-url"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "INVALID_CALLBACK_URL");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn empty_scopes_is_rejected() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789000","scopes":[],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_purpose_is_rejected() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789000","scopes":["s"],"purpose":"Marketing","requestCaptureUrl":false}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_required_field_is_rejected() {
        // No requestCaptureUrl → body fails to parse → 400.
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"+123456789000","scopes":["s"],"purpose":"dpv:Marketing"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = retrieve_ok(
            r#"{"scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false,"x":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = retrieve_ok(
            r#"{"phoneNumber":"0123","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_retrieve(
            Some(&token),
            r#"{"phoneNumber":"+123456789000","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_retrieve(
            None,
            r#"{"phoneNumber":"+123456789000","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(RETRIEVE_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"phoneNumber":"+123456789000","scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
            Some("corr-ci"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ci")
        );
        // Business error.
        let (status, headers, _) = post_retrieve(
            Some(&token),
            r#"{"scopes":["s"],"purpose":"dpv:Marketing","requestCaptureUrl":false}"#,
            Some("corr-err"),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
