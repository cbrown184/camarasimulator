//! KYC Age Verification **v0.1** (CAMARA KYC Age Verification 0.1.0, r2.2).
//!
//! One endpoint:
//! - `POST /kyc-age-verification/v0.1/verify` — is the line's holder at or above
//!   a caller-supplied `ageThreshold`?
//!
//! ## What it does
//!
//! The caller submits an `ageThreshold` (0–120) and, optionally, a `phoneNumber`
//! (two-legged auth only) plus identity attributes, and the operator answers a
//! privacy-preserving verdict `{ "ageCheck": "true" | "false" | "not_available" }`:
//! `"true"` when the holder is at or above the threshold, `"false"` when below,
//! `"not_available"` when the operator holds no age for the line. No birthdate is
//! ever returned.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `kyc-age-verification:verify`
//! scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by the
//! token **subject**, so resubmitting it is an error (mirrors Number Recycling):
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
//! - **`ageThreshold` range.** Outside `0..=120` → `400 OUT_OF_RANGE`.
//! - **Reserved error suffix (identifier).** If the resolved identifier's
//!   trailing three digits name a reserved CAMARA status (`…400`, `…401`, `…403`,
//!   `…404`, `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with
//!   that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Age verdict (identifier digits × `ageThreshold`).** Otherwise the
//!   identifier's trailing three digits `d` encode the holder's **held age** as
//!   `d % 100` (0–99 years). `ageCheck = "true"` iff that age is at or above the
//!   requested `ageThreshold`, else `"false"`. This makes `ageThreshold` a genuine
//!   second control plane: the *same* number reads `"true"` against a low
//!   threshold and `"false"` against a high one. A `…000` tail (the operator holds
//!   no birthdate) → `"not_available"`.
//! - **Identity attributes (optional response fields).**
//!   - `identityMatchScore` (fixed 90) is returned when any identity attribute
//!     (`idDocument`/`name`/`givenName`/`familyName`/`middleNames`/
//!     `familyNameAtBirth`/`birthdate`/`email`) is supplied — the caller gave
//!     something to match against.
//!   - `verifiedStatus: true` is returned when an `idDocument` is supplied (the
//!     answer was checked against an official document).
//!   - `contentLock` is returned only when `includeContentLock: true`, and
//!     `parentalControl` only when `includeParentalControl: true`; each is
//!     `"true"` when the held age is a minor (`< 18`), `"false"` for an adult, and
//!     `"not_available"` when the age is unknown (`…000`).
//!
//! Example: `+123456789025` (held age 25) with `ageThreshold: 18` → `"true"`; the
//! same number with `ageThreshold: 30` → `"false"`; `+123456789404` →
//! `404 NOT_FOUND`; `+123456789000` → `"not_available"`.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /verify` endpoint requires (CAMARA KYC Age
/// Verification 0.1.0).
const VERIFY_SCOPE: &str = "kyc-age-verification:verify";

/// The fixed identity-match score CamaraSim reports when identity attributes are
/// supplied (mirrors KYC Match's fixed score — no real matching backend).
const FIXED_MATCH_SCORE: u8 = 90;

/// Legal-adulthood boundary used for the `contentLock` / `parentalControl`
/// signals (a minor is `< 18`).
const ADULTHOOD_AGE: u16 = 18;

/// Routes for KYC Age Verification v0.1, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/kyc-age-verification/v0.1/verify", post(verify))
}

/// `POST /verify` request body (CAMARA `AgeVerificationRequest`): a required
/// `ageThreshold`, an optional `phoneNumber` (valid only in two-legged auth),
/// optional identity attributes, and the two `include*` output toggles. Unknown
/// fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgeVerificationRequest {
    #[serde(rename = "ageThreshold")]
    age_threshold: i64,
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "idDocument")]
    id_document: Option<String>,
    name: Option<String>,
    #[serde(rename = "givenName")]
    given_name: Option<String>,
    #[serde(rename = "familyName")]
    family_name: Option<String>,
    #[serde(rename = "middleNames")]
    middle_names: Option<String>,
    #[serde(rename = "familyNameAtBirth")]
    family_name_at_birth: Option<String>,
    birthdate: Option<String>,
    email: Option<String>,
    #[serde(rename = "includeContentLock", default)]
    include_content_lock: bool,
    #[serde(rename = "includeParentalControl", default)]
    include_parental_control: bool,
}

impl AgeVerificationRequest {
    /// Whether the caller supplied any identity attribute to match against.
    fn has_identity_attribute(&self) -> bool {
        self.id_document.is_some()
            || self.name.is_some()
            || self.given_name.is_some()
            || self.family_name.is_some()
            || self.middle_names.is_some()
            || self.family_name_at_birth.is_some()
            || self.birthdate.is_some()
            || self.email.is_some()
    }
}

/// `POST /kyc-age-verification/v0.1/verify`.
async fn verify(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(VERIFY_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: AgeVerificationRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid AgeVerificationRequest.",
                &correlator,
            )
        }
    };

    // `ageThreshold` must be within the CAMARA range (0–120).
    if !(0..=120).contains(&req.age_threshold) {
        return out_of_range("`ageThreshold` must be between 0 and 120.", &correlator);
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

    // The trailing three digits encode the holder's held age (`d % 100`); a
    // `…000` tail means the operator holds no birthdate for the line.
    let tail = scenarios::trailing_three_digits(&identifier).unwrap_or(0);
    let held_age: Option<u16> = if tail == 0 { None } else { Some(tail % 100) };

    // Build the verdict body, adding the optional fields the request asked for.
    let mut out = Map::new();
    out.insert("ageCheck".into(), Value::String(age_check(held_age, req.age_threshold)));

    if req.has_identity_attribute() {
        out.insert("identityMatchScore".into(), json!(FIXED_MATCH_SCORE));
    }
    if req.id_document.is_some() {
        out.insert("verifiedStatus".into(), Value::Bool(true));
    }
    if req.include_content_lock {
        out.insert("contentLock".into(), Value::String(minor_signal(held_age)));
    }
    if req.include_parental_control {
        out.insert("parentalControl".into(), Value::String(minor_signal(held_age)));
    }

    with_correlator((StatusCode::OK, Json(Value::Object(out))).into_response(), &correlator)
}

/// The `ageCheck` verdict string: `"not_available"` when the held age is unknown,
/// else `"true"`/`"false"` for held-age ≥/< the requested threshold.
fn age_check(held_age: Option<u16>, threshold: i64) -> String {
    match held_age {
        None => "not_available",
        Some(age) => {
            if i64::from(age) >= threshold {
                "true"
            } else {
                "false"
            }
        }
    }
    .to_string()
}

/// A minor-vs-adult signal (`contentLock` / `parentalControl`): `"true"` for a
/// minor (`< 18`), `"false"` for an adult, `"not_available"` when age is unknown.
fn minor_signal(held_age: Option<u16>) -> String {
    match held_age {
        None => "not_available",
        Some(age) if age < ADULTHOOD_AGE => "true",
        Some(_) => "false",
    }
    .to_string()
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Number Recycling). See the module docs for the four cases.
fn resolve_identifier(
    req: &AgeVerificationRequest,
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
            // The line is already identified by a three-legged token; the number
            // must not be resubmitted.
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "av.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn age_check_compares_held_age_to_threshold() {
        assert_eq!(age_check(Some(25), 18), "true");
        assert_eq!(age_check(Some(25), 25), "true"); // boundary: >= is inclusive
        assert_eq!(age_check(Some(25), 30), "false");
        assert_eq!(age_check(None, 18), "not_available");
    }

    #[test]
    fn minor_signal_tracks_adulthood() {
        assert_eq!(minor_signal(Some(12)), "true"); // minor
        assert_eq!(minor_signal(Some(18)), "false"); // adult (boundary)
        assert_eq!(minor_signal(Some(40)), "false");
        assert_eq!(minor_signal(None), "not_available");
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("av-client")); // client-credentials subject
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "av-client").await
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

    /// POST to `/verify` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_verify(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/kyc-age-verification/v0.1/verify")
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
    async fn verify_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(VERIFY_SCOPE).await;
        post_verify(Some(&token), body, None).await
    }

    // --- Age verdict: identifier digits × ageThreshold ---------------------

    #[tokio::test]
    async fn holder_at_or_above_threshold_is_true() {
        // Held age 25 (…025); threshold 18 → true.
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ageCheck"], "true");
    }

    #[tokio::test]
    async fn holder_below_threshold_is_false() {
        // Held age 12 (…012); threshold 18 → false.
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789012","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ageCheck"], "false");
    }

    #[tokio::test]
    async fn age_threshold_is_a_real_second_control_plane() {
        // Same number (held age 25); the threshold alone flips the verdict.
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ageCheck"], "true");

        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":30}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ageCheck"], "false");
    }

    #[tokio::test]
    async fn zero_tail_has_no_age_on_record() {
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789000","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ageCheck"], "not_available");
    }

    // --- Optional response fields ------------------------------------------

    #[tokio::test]
    async fn identity_attributes_add_match_score_and_verified_status() {
        let (status, _, body) = verify_ok(
            r#"{"phoneNumber":"+123456789025","ageThreshold":18,"name":"Ada Lovelace","idDocument":"P1234567"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["identityMatchScore"], 90);
        assert_eq!(body["verifiedStatus"], true);
    }

    #[tokio::test]
    async fn no_identity_attributes_means_no_score_or_verified_status() {
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("identityMatchScore").is_none());
        assert!(body.get("verifiedStatus").is_none());
    }

    #[tokio::test]
    async fn name_without_id_document_scores_but_is_not_verified() {
        let (status, _, body) = verify_ok(
            r#"{"phoneNumber":"+123456789025","ageThreshold":18,"givenName":"Ada"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["identityMatchScore"], 90);
        assert!(body.get("verifiedStatus").is_none());
    }

    #[tokio::test]
    async fn content_lock_and_parental_control_are_opt_in_and_age_driven() {
        // Minor (held age 12) with both toggles on → both locked.
        let (status, _, body) = verify_ok(
            r#"{"phoneNumber":"+123456789012","ageThreshold":18,"includeContentLock":true,"includeParentalControl":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["contentLock"], "true");
        assert_eq!(body["parentalControl"], "true");

        // Adult (held age 40), only contentLock requested → false, and no
        // parentalControl field at all.
        let (status, _, body) = verify_ok(
            r#"{"phoneNumber":"+123456789040","ageThreshold":18,"includeContentLock":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["contentLock"], "false");
        assert!(body.get("parentalControl").is_none());
    }

    #[tokio::test]
    async fn content_lock_is_not_available_when_age_unknown() {
        let (status, _, body) = verify_ok(
            r#"{"phoneNumber":"+123456789000","ageThreshold":18,"includeContentLock":true}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["contentLock"], "not_available");
    }

    #[tokio::test]
    async fn toggles_off_by_default_add_no_fields() {
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789012","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("contentLock").is_none());
        assert!(body.get("parentalControl").is_none());
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789404","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789429","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- ageThreshold validation -------------------------------------------

    #[tokio::test]
    async fn threshold_out_of_range_is_rejected() {
        for bad in ["-1", "121", "999"] {
            let (status, _, body) = verify_ok(&format!(
                r#"{{"phoneNumber":"+123456789025","ageThreshold":{bad}}}"#
            ))
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "threshold {bad}");
            assert_eq!(body["code"], "OUT_OF_RANGE", "threshold {bad}");
        }
    }

    #[tokio::test]
    async fn threshold_at_the_bounds_is_accepted() {
        // 0 and 120 are inclusive bounds.
        let (status, _, _) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":0}"#).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":120}"#).await;
        assert_eq!(status, StatusCode::OK);
        // Held age 25 < 120 → false.
        assert_eq!(body["ageCheck"], "false");
    }

    #[tokio::test]
    async fn missing_threshold_is_invalid_argument() {
        let (status, _, body) = verify_ok(r#"{"phoneNumber":"+123456789025"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; subject is an E.164 line with held age 25.
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789025").await;
        let (status, _, body) =
            post_verify(Some(&token), r#"{"ageThreshold":18}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ageCheck"], "true");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789503").await;
        let (status, _, body) =
            post_verify(Some(&token), r#"{"ageThreshold":18}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(VERIFY_SCOPE, "+123456789025").await;
        let (status, _, body) = post_verify(
            Some(&token),
            r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = verify_ok(r#"{"ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"0123","ageThreshold":18}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) =
            verify_ok(r#"{"phoneNumber":"+123456789025","ageThreshold":18,"x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = verify_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_verify(
            Some(&token),
            r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_verify(
            None,
            r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(VERIFY_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_verify(
            Some(&token),
            r#"{"phoneNumber":"+123456789025","ageThreshold":18}"#,
            Some("corr-av"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-av")
        );
        // Business error.
        let (status, headers, _) =
            post_verify(Some(&token), r#"{"ageThreshold":18}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
