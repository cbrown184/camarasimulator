//! Customer Insights **v0.2** (CAMARA Customer Insights 0.2.0, r2.2).
//!
//! One endpoint:
//! - `POST /customer-insights/v0.2/scoring/retrieve` — return a risk/trust
//!   score for the line behind a phone number.
//!
//! ## What it does
//!
//! The caller submits a `scoringType` (which scale to score on) and, optionally,
//! a `phoneNumber` (two-legged auth only) and/or an `idDocument`. The operator
//! answers `{ "scoringType": …, "scoringValue": … }` — a single numeric score on
//! the requested scale, never the underlying data:
//!
//! - `gaugeMetric` — a credit-style score `300` (highest risk) … `850` (lowest
//!   risk).
//! - `veritasIndex` — a compact index `0` (lowest risk) … `19` (highest risk).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `customer-insights:scoring:read`
//! scope.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA, the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by the
//! token **subject**, so resubmitting it is an error (mirrors KYC Tenure / Number
//! Recycling / Call Forwarding Signal):
//!
//! - `phoneNumber` present **and** the subject is itself an E.164 number (a
//!   line-authenticated three-legged token) → `422 UNNECESSARY_IDENTIFIER`.
//! - `phoneNumber` present, subject not a line → the submitted number is the
//!   identifier (two-legged).
//! - `phoneNumber` absent, subject is an E.164 number → the subject is the
//!   identifier (three-legged).
//! - `phoneNumber` absent **and** the subject is not a line → the line cannot be
//!   identified from a number. If an `idDocument` was supplied, CamaraSim cannot
//!   score by document alone → `422 CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED`;
//!   otherwise → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the answer:
//!
//! - **Reserved error suffix (identifier).** If the resolved identifier's
//!   trailing three digits name a reserved CAMARA status (`…400`, `…401`, `…403`,
//!   `…404`, `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with
//!   that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Score (identifier digits × `scoringType`).** Otherwise the identifier's
//!   trailing three digits `d` (`0`–`999`) fix the score on the requested scale:
//!   `gaugeMetric` → `300 + (d % 551)` (300–850), `veritasIndex` → `d % 20`
//!   (0–19). This makes `scoringType` a genuine second control plane: the *same*
//!   number reads a different value on each scale. An identifier with no digits
//!   scores at the scale's floor (`d = 0`).
//!
//! `scoringType` is required and must be one of the two known scales; a missing
//! or unknown value → `400 INVALID_ARGUMENT` (schema). `idDocument`, when
//! supplied, must be a non-empty string ≤ 30 chars, else `400 INVALID_ARGUMENT`.
//!
//! Example: `+123456789740` on `gaugeMetric` → `scoringValue: 1040 % 551`… i.e.
//! `300 + (740 % 551) = 489`; the same number on `veritasIndex` → `740 % 20 = 0`;
//! `+123456789404` → `404 NOT_FOUND`.

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

/// The OAuth2 scope the `POST /scoring/retrieve` endpoint requires (CAMARA
/// Customer Insights 0.2.0).
const SCORING_SCOPE: &str = "customer-insights:scoring:read";

/// The maximum length of an `idDocument` (CAMARA Customer Insights 0.2.0).
const ID_DOCUMENT_MAX_LEN: usize = 30;

/// Routes for Customer Insights v0.2, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route(
        "/customer-insights/v0.2/scoring/retrieve",
        post(retrieve_scoring),
    )
}

/// The scoring scale a caller asks for (CAMARA `ScoringType`). Unknown values are
/// rejected by serde (`deny_unknown_fields` at the request level plus this closed
/// enum) → `400 INVALID_ARGUMENT`.
#[derive(Debug, Clone, Copy, Deserialize)]
enum ScoringType {
    #[serde(rename = "gaugeMetric")]
    GaugeMetric,
    #[serde(rename = "veritasIndex")]
    VeritasIndex,
}

impl ScoringType {
    /// The wire string for this scale (echoed back in the response).
    fn as_str(self) -> &'static str {
        match self {
            ScoringType::GaugeMetric => "gaugeMetric",
            ScoringType::VeritasIndex => "veritasIndex",
        }
    }

    /// The score for an identifier's trailing three digits on this scale.
    /// `gaugeMetric` → `300 + (d % 551)` (300–850); `veritasIndex` → `d % 20`
    /// (0–19). Deterministic from the input alone (docs/DESIGN.md §7).
    fn score(self, digits: u16) -> u16 {
        match self {
            ScoringType::GaugeMetric => 300 + (digits % 551),
            ScoringType::VeritasIndex => digits % 20,
        }
    }
}

/// `POST /scoring/retrieve` request body (CAMARA `ScoringRequest`): a required
/// `scoringType`, an optional `phoneNumber` (two-legged auth only), and an
/// optional `idDocument`. Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoringRequest {
    #[serde(rename = "idDocument")]
    id_document: Option<String>,
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "scoringType")]
    scoring_type: ScoringType,
}

/// `POST /customer-insights/v0.2/scoring/retrieve`.
async fn retrieve_scoring(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SCORING_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let req: ScoringRequest = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument(
                "Request body is not a valid ScoringRequest (a known `scoringType` is required).",
                &correlator,
            )
        }
    };

    // `idDocument`, when supplied, must be a non-empty string within the schema
    // length bound.
    if let Some(doc) = &req.id_document {
        if doc.is_empty() || doc.chars().count() > ID_DOCUMENT_MAX_LEN {
            return invalid_argument(
                "`idDocument` must be a non-empty string of at most 30 characters.",
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

    // Score the resolved identifier on the requested scale.
    let digits = scenarios::trailing_three_digits(&identifier).unwrap_or(0);
    let scoring_value = req.scoring_type.score(digits);

    with_correlator(
        (
            StatusCode::OK,
            Json(json!({
                "scoringType": req.scoring_type.as_str(),
                "scoringValue": scoring_value,
            })),
        )
            .into_response(),
        &correlator,
    )
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors KYC
/// Tenure). See the module docs for the cases. A phone-less, non-line request
/// carrying an `idDocument` yields `ID_DOCUMENT_NOT_SUPPORTED` (CamaraSim scores
/// by phone number only); with neither, `MISSING_IDENTIFIER`.
fn resolve_identifier(
    req: &ScoringRequest,
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
            } else if req.id_document.is_some() {
                Err(unprocessable(
                    "CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED",
                    "Scoring by `idDocument` alone is not supported; supply a `phoneNumber` or use a token that identifies a line.",
                    correlator,
                ))
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

    const HOST: &str = "ci.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn gauge_metric_maps_digits_into_the_300_850_band() {
        assert_eq!(ScoringType::GaugeMetric.score(0), 300);
        assert_eq!(ScoringType::GaugeMetric.score(550), 850);
        assert_eq!(ScoringType::GaugeMetric.score(551), 300); // wraps
        assert_eq!(ScoringType::GaugeMetric.score(740), 300 + (740 % 551));
        // The whole scale stays within [300, 850] for every possible input.
        for d in 0u16..=999 {
            let v = ScoringType::GaugeMetric.score(d);
            assert!((300..=850).contains(&v), "gaugeMetric({d}) = {v} out of band");
        }
    }

    #[test]
    fn veritas_index_maps_digits_into_the_0_19_band() {
        assert_eq!(ScoringType::VeritasIndex.score(0), 0);
        assert_eq!(ScoringType::VeritasIndex.score(19), 19);
        assert_eq!(ScoringType::VeritasIndex.score(20), 0); // wraps
        for d in 0u16..=999 {
            let v = ScoringType::VeritasIndex.score(d);
            assert!(v <= 19, "veritasIndex({d}) = {v} out of band");
        }
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("ci-client")); // client-credentials subject
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

    /// POST to `/scoring/retrieve` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_scoring(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/customer-insights/v0.2/scoring/retrieve")
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
    async fn scoring_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(SCORING_SCOPE).await;
        post_scoring(Some(&token), body, None).await
    }

    // --- Score: identifier digits × scoringType ----------------------------

    #[tokio::test]
    async fn gauge_metric_scores_from_the_identifier_digits() {
        let (status, _, out) =
            scoring_ok(r#"{"phoneNumber":"+123456789740","scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["scoringType"], "gaugeMetric");
        assert_eq!(out["scoringValue"], 300 + (740 % 551));
    }

    #[tokio::test]
    async fn veritas_index_scores_from_the_identifier_digits() {
        let (status, _, out) =
            scoring_ok(r#"{"phoneNumber":"+123456789755","scoringType":"veritasIndex"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["scoringType"], "veritasIndex");
        assert_eq!(out["scoringValue"], 755 % 20);
    }

    #[tokio::test]
    async fn scoring_type_is_a_real_second_control_plane() {
        // The same number yields a different value on each scale.
        let (_, _, gauge) =
            scoring_ok(r#"{"phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#).await;
        let (_, _, veritas) =
            scoring_ok(r#"{"phoneNumber":"+123456789123","scoringType":"veritasIndex"}"#).await;
        assert_eq!(gauge["scoringValue"], 300 + (123 % 551));
        assert_eq!(veritas["scoringValue"], 123 % 20);
        assert_ne!(gauge["scoringValue"], veritas["scoringValue"]);
    }

    #[tokio::test]
    async fn no_digit_identifier_scores_at_the_floor() {
        // A two-legged token whose subject has no digits and no phoneNumber can't
        // happen (that's MISSING_IDENTIFIER); use a phoneNumber whose tail is 000.
        let (status, _, out) =
            scoring_ok(r#"{"phoneNumber":"+123456789000","scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["scoringValue"], 300);
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) =
            scoring_ok(r#"{"phoneNumber":"+123456789404","scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) =
            scoring_ok(r#"{"phoneNumber":"+123456789429","scoringType":"veritasIndex"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");

        let (status, _, body) =
            scoring_ok(r#"{"phoneNumber":"+123456789422","scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "SERVICE_NOT_APPLICABLE");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        let token = mint_token_with_client(SCORING_SCOPE, "+123456789123").await;
        let (status, _, out) =
            post_scoring(Some(&token), r#"{"scoringType":"gaugeMetric"}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["scoringValue"], 300 + (123 % 551));
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(SCORING_SCOPE, "+123456789503").await;
        let (status, _, body) =
            post_scoring(Some(&token), r#"{"scoringType":"gaugeMetric"}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(SCORING_SCOPE, "+123456789123").await;
        let (status, _, body) = post_scoring(
            Some(&token),
            r#"{"phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = scoring_ok(r#"{"scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn id_document_alone_is_not_supported() {
        // No phoneNumber, a non-line subject, but an idDocument supplied.
        let (status, _, body) =
            scoring_ok(r#"{"idDocument":"AB1234567","scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED");
    }

    #[tokio::test]
    async fn id_document_is_ignored_when_a_phone_number_identifies_the_line() {
        // A valid phoneNumber wins; the idDocument is accepted and ignored.
        let (status, _, out) = scoring_ok(
            r#"{"idDocument":"AB1234567","phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(out["scoringValue"], 300 + (123 % 551));
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn missing_scoring_type_is_invalid_argument() {
        let (status, _, body) = scoring_ok(r#"{"phoneNumber":"+123456789123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_scoring_type_is_invalid_argument() {
        let (status, _, body) =
            scoring_ok(r#"{"phoneNumber":"+123456789123","scoringType":"mysteryScale"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn empty_id_document_is_invalid_argument() {
        let (status, _, body) =
            scoring_ok(r#"{"idDocument":"","phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#)
                .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn overlong_id_document_is_invalid_argument() {
        let long = "X".repeat(31);
        let body = format!(
            r#"{{"idDocument":"{long}","phoneNumber":"+123456789123","scoringType":"gaugeMetric"}}"#
        );
        let (status, _, out) = scoring_ok(&body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(out["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) =
            scoring_ok(r#"{"phoneNumber":"0123","scoringType":"gaugeMetric"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = scoring_ok(
            r#"{"phoneNumber":"+123456789123","scoringType":"gaugeMetric","x":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = scoring_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = post_scoring(
            Some(&token),
            r#"{"phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = post_scoring(
            None,
            r#"{"phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(SCORING_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_scoring(
            Some(&token),
            r#"{"phoneNumber":"+123456789123","scoringType":"gaugeMetric"}"#,
            Some("corr-ci"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-ci")
        );
        // Business error.
        let (status, headers, _) =
            post_scoring(Some(&token), r#"{"scoringType":"gaugeMetric"}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
