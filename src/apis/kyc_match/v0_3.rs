//! KYC Match **v0.3** (CAMARA Know Your Customer Match 0.3.0).
//!
//! One endpoint:
//! - `POST /kyc-match/v0.3/match` — does each submitted customer attribute
//!   match the operator's records?
//!
//! ## What it does
//!
//! The caller submits a set of identity attributes (`name`, `address`,
//! `birthdate`, `email`, …), optionally scoped to a `phoneNumber` in the body or
//! the identity a three-legged access token authenticated. For **each attribute
//! present in the request** the response carries a `<attribute>Match` verdict —
//! one of the CAMARA strings `"true"`, `"false"`, or `"not_available"` — and,
//! for the score-bearing attributes, a `<attribute>MatchScore` (0–100) whenever
//! the verdict is `"false"` (a partial match). Attributes the caller did not
//! send are simply absent from the response.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `kyc-match:match` scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the response:
//!
//! - **Reserved error suffix (top-level).** The identifier is the submitted
//!   `phoneNumber` when present, otherwise the access token's subject (`sub`). If
//!   its trailing three digits name a reserved CAMARA status (`…400`, `…401`,
//!   `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint
//!   answers with that canonical CAMARA error instead of a match result (shared
//!   convention, [`crate::scenarios`]). Validation runs first, so the request
//!   must still be well-formed (include at least one non-`phoneNumber`
//!   attribute) for the reserved error to surface.
//! - **Per-attribute verdict (from the value).** Otherwise each attribute's
//!   verdict is chosen from its **own submitted value**: a value containing
//!   `"unavailable"` → `"not_available"`; a value containing `"nomatch"` →
//!   `"false"` (plus a [`PARTIAL_MATCH_SCORE`] for score-bearing attributes);
//!   any other value → `"true"`. Comparison is case-insensitive.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope `POST /match` requires (CAMARA KYC Match 0.3.0).
const MATCH_SCOPE: &str = "kyc-match:match";

/// The score reported for a score-bearing attribute whose verdict is `"false"`
/// — a fixed, deterministic "near miss" (a real operator would return a
/// similarity metric; CamaraSim reports a constant partial score so the case is
/// reproducible). Only emitted alongside a `"false"` verdict.
const PARTIAL_MATCH_SCORE: i32 = 90;

/// Routes for KYC Match v0.3, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/kyc-match/v0.3/match", post(match_customer))
}

/// `POST /match` request body (CAMARA `KYC_MatchRequestBody`). Every field is
/// optional at the schema level, but at least one attribute other than
/// `phoneNumber` must be supplied. All values are strings; unknown fields are
/// rejected.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MatchRequest {
    phone_number: Option<String>,
    id_document: Option<String>,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
    name_kana_hankaku: Option<String>,
    name_kana_zenkaku: Option<String>,
    middle_names: Option<String>,
    family_name_at_birth: Option<String>,
    address: Option<String>,
    street_name: Option<String>,
    street_number: Option<String>,
    postal_code: Option<String>,
    region: Option<String>,
    locality: Option<String>,
    country: Option<String>,
    house_number_extension: Option<String>,
    birthdate: Option<String>,
    email: Option<String>,
    gender: Option<String>,
}

/// `POST /kyc-match/v0.3/match`.
async fn match_customer(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(MATCH_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required and must parse as a KYC_MatchRequestBody.
    if body.is_empty() {
        return invalid_argument("A request body is required.", &correlator);
    }
    let req: MatchRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid KYC_MatchRequestBody.", &correlator)
        }
    };

    // `phoneNumber`, when supplied, must be valid E.164.
    if let Some(ref phone) = req.phone_number {
        if !is_valid_e164(phone) {
            return invalid_argument(
                "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                &correlator,
            );
        }
    }

    // At least one attribute other than `phoneNumber` must be present
    // (KYC Match: nothing to match against otherwise).
    let fields = req.match_fields();
    if fields.is_empty() {
        return with_correlator(
            CamaraError::new(
                StatusCode::BAD_REQUEST,
                "KNOW_YOUR_CUSTOMER.INVALID_PARAM_COMBINATION",
                "At least one attribute other than `phoneNumber` must be provided.",
            )
            .into_response(),
            &correlator,
        );
    }

    // The identifier for the shared reserved-error convention is the submitted
    // `phoneNumber`, else the token subject.
    let identifier = req
        .phone_number
        .as_deref()
        .or_else(|| claims.subject())
        .unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Build the response: one verdict per submitted attribute, chosen from its
    // own value (docs/DESIGN.md §7).
    let mut out = Map::new();
    for field in fields {
        field.evaluate(&mut out);
    }

    with_correlator((StatusCode::OK, Json(Value::Object(out))).into_response(), &correlator)
}

/// A submitted attribute paired with the response keys it drives.
struct MatchField<'a> {
    /// The submitted value, used to pick the verdict.
    value: &'a str,
    /// The response key for the verdict, e.g. `nameMatch`.
    match_key: &'static str,
    /// The response key for the partial-match score, e.g. `nameMatchScore`, for
    /// score-bearing attributes; `None` for boolean-only attributes.
    score_key: Option<&'static str>,
}

impl MatchField<'_> {
    /// Insert this attribute's verdict (and, when `"false"`, its score) into the
    /// response object.
    fn evaluate(&self, out: &mut Map<String, Value>) {
        let verdict = classify(self.value);
        out.insert(self.match_key.to_string(), Value::String(verdict.to_string()));
        if verdict == "false" {
            if let Some(score_key) = self.score_key {
                out.insert(score_key.to_string(), Value::from(PARTIAL_MATCH_SCORE));
            }
        }
    }
}

impl MatchRequest {
    /// The submitted matchable attributes (everything except `phoneNumber`),
    /// paired with their response keys, in a stable order.
    fn match_fields(&self) -> Vec<MatchField<'_>> {
        // (submitted value, match key, score key). `score_key: None` marks a
        // boolean-only attribute (no similarity score in CAMARA 0.3.0).
        let table: [(&Option<String>, &'static str, Option<&'static str>); 19] = [
            (&self.id_document, "idDocumentMatch", None),
            (&self.name, "nameMatch", Some("nameMatchScore")),
            (&self.given_name, "givenNameMatch", Some("givenNameMatchScore")),
            (&self.family_name, "familyNameMatch", Some("familyNameMatchScore")),
            (&self.name_kana_hankaku, "nameKanaHankakuMatch", Some("nameKanaHankakuMatchScore")),
            (&self.name_kana_zenkaku, "nameKanaZenkakuMatch", Some("nameKanaZenkakuMatchScore")),
            (&self.middle_names, "middleNamesMatch", Some("middleNamesMatchScore")),
            (&self.family_name_at_birth, "familyNameAtBirthMatch", Some("familyNameAtBirthMatchScore")),
            (&self.address, "addressMatch", Some("addressMatchScore")),
            (&self.street_name, "streetNameMatch", Some("streetNameMatchScore")),
            (&self.street_number, "streetNumberMatch", Some("streetNumberMatchScore")),
            (&self.postal_code, "postalCodeMatch", None),
            (&self.region, "regionMatch", Some("regionMatchScore")),
            (&self.locality, "localityMatch", Some("localityMatchScore")),
            (&self.country, "countryMatch", None),
            (&self.house_number_extension, "houseNumberExtensionMatch", None),
            (&self.birthdate, "birthdateMatch", None),
            (&self.email, "emailMatch", Some("emailMatchScore")),
            (&self.gender, "genderMatch", None),
        ];
        table
            .into_iter()
            .filter_map(|(value, match_key, score_key)| {
                value.as_deref().map(|value| MatchField { value, match_key, score_key })
            })
            .collect()
    }
}

/// Choose an attribute's verdict from its submitted value (docs/DESIGN.md §7).
/// Case-insensitive: `"unavailable"` → `"not_available"`; `"nomatch"` →
/// `"false"`; anything else → `"true"`.
fn classify(value: &str) -> &'static str {
    let lower = value.to_ascii_lowercase();
    if lower.contains("unavailable") {
        "not_available"
    } else if lower.contains("nomatch") {
        "false"
    } else {
        "true"
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "kyc.local:8080";

    // --- Pure scenario units ----------------------------------------------

    #[test]
    fn classify_reads_the_value_case_insensitively() {
        assert_eq!(classify("John Smith"), "true");
        assert_eq!(classify("nomatch"), "false");
        assert_eq!(classify("Jane NoMatch"), "false");
        assert_eq!(classify("unavailable"), "not_available");
        assert_eq!(classify("DATA-UNAVAILABLE"), "not_available");
        // "unavailable" wins if both markers appear.
        assert_eq!(classify("nomatch-unavailable"), "not_available");
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(!is_valid_e164("0123")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
    }

    #[test]
    fn score_is_only_emitted_for_false_score_bearing_fields() {
        let mut out = Map::new();
        // Score-bearing + false → score present.
        MatchField { value: "nomatch", match_key: "nameMatch", score_key: Some("nameMatchScore") }
            .evaluate(&mut out);
        assert_eq!(out["nameMatch"], "false");
        assert_eq!(out["nameMatchScore"], PARTIAL_MATCH_SCORE);
        // Score-bearing + true → no score.
        MatchField { value: "ok", match_key: "emailMatch", score_key: Some("emailMatchScore") }
            .evaluate(&mut out);
        assert_eq!(out["emailMatch"], "true");
        assert!(!out.contains_key("emailMatchScore"));
        // Boolean-only + false → still no score.
        MatchField { value: "nomatch", match_key: "birthdateMatch", score_key: None }
            .evaluate(&mut out);
        assert_eq!(out["birthdateMatch"], "false");
        assert!(!out.contains_key("birthdateMatchScore"));
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
        mint_token_with_client(scope, "kyc-client").await
    }

    /// As [`mint_token`], but with a caller-chosen `client_id` — which becomes
    /// the token `sub`, driving the subject-keyed reserved-error case.
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

    /// POST a JSON body to `/match` with an optional Bearer token and `x-correlator`.
    async fn post_match(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/kyc-match/v0.3/match")
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

    /// Mint a scoped token and call `/match` with the given body.
    async fn match_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(MATCH_SCOPE).await;
        post_match(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn all_matching_attributes_are_true_with_no_scores() {
        let (status, _, body) = match_ok_token(
            r#"{"name":"John Smith","email":"j@example.com","country":"GB"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["nameMatch"], "true");
        assert_eq!(body["emailMatch"], "true");
        assert_eq!(body["countryMatch"], "true");
        // No scores when everything matches.
        assert!(body.get("nameMatchScore").is_none());
        assert!(body.get("emailMatchScore").is_none());
        // Attributes not submitted are absent.
        assert!(body.get("birthdateMatch").is_none());
    }

    #[tokio::test]
    async fn false_verdict_carries_a_score_only_for_score_bearing_fields() {
        let (status, _, body) = match_ok_token(
            r#"{"name":"nomatch","postalCode":"nomatch"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        // Score-bearing name → false + score.
        assert_eq!(body["nameMatch"], "false");
        assert_eq!(body["nameMatchScore"], PARTIAL_MATCH_SCORE);
        // Boolean-only postalCode → false, no score.
        assert_eq!(body["postalCodeMatch"], "false");
        assert!(body.get("postalCodeMatchScore").is_none());
    }

    #[tokio::test]
    async fn unavailable_marker_yields_not_available() {
        let (status, _, body) = match_ok_token(r#"{"address":"unavailable"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["addressMatch"], "not_available");
        assert!(body.get("addressMatchScore").is_none());
    }

    #[tokio::test]
    async fn phone_number_scopes_but_is_not_a_match_field() {
        let (status, _, body) =
            match_ok_token(r#"{"phoneNumber":"+123456789012","name":"Jane"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["nameMatch"], "true");
        // phoneNumber has no verdict of its own.
        assert!(body.get("phoneNumberMatch").is_none());
    }

    #[tokio::test]
    async fn reserved_suffix_on_phone_number_selects_a_canonical_error() {
        // A well-formed request (has `name`) whose phoneNumber suffix is reserved.
        let (status, _, body) =
            match_ok_token(r#"{"phoneNumber":"+123456789404","name":"Jane"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            match_ok_token(r#"{"phoneNumber":"+123456789429","name":"Jane"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn reserved_suffix_on_token_subject_selects_a_canonical_error() {
        // No phoneNumber → identifier is the token subject.
        let token = mint_token_with_client(MATCH_SCOPE, "+123456789503").await;
        let (status, _, body) = post_match(Some(&token), r#"{"name":"Jane"}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn only_phone_number_is_invalid_param_combination() {
        let (status, _, body) = match_ok_token(r#"{"phoneNumber":"+123456789012"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "KNOW_YOUR_CUSTOMER.INVALID_PARAM_COMBINATION");
    }

    #[tokio::test]
    async fn empty_body_is_rejected() {
        let token = mint_token(MATCH_SCOPE).await;
        let (status, _, body) = post_match(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = match_ok_token(r#"{"name":"Jane","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn non_string_value_is_rejected() {
        let (status, _, body) = match_ok_token(r#"{"name":123}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = match_ok_token(r#"{"phoneNumber":"0123","name":"Jane"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_match(Some(&token), r#"{"name":"Jane"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_match(None, r#"{"name":"Jane"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(MATCH_SCOPE).await;
        let (status, headers, _) =
            post_match(Some(&token), r#"{"name":"Jane"}"#, Some("corr-kyc")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-kyc")
        );

        let (status, headers, _) =
            post_match(Some(&token), r#"{"phoneNumber":"0123","name":"Jane"}"#, Some("corr-err"))
                .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
