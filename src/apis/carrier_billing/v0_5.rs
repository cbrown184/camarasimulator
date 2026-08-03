//! Carrier Billing **v0.5** (CAMARA Carrier Billing 0.5.0, release r3.2).
//!
//! One endpoint so far:
//! - `POST /carrier-billing/v0.5/payments` — create (and, in the one-step flow,
//!   immediately charge) a payment against an end user's mobile account
//!   (operationId `createPayment`).
//!
//! ## What it does
//!
//! The caller submits an `amountTransaction` describing what to charge (an
//! `amount`/`currency`/`description`, a `referenceCode`, and — for a two-legged
//! or CIBA token — the `phoneNumber` of the account). CamaraSim runs the
//! **one-step** flow (the only flow Carrier Billing 0.5.0 covers): the payment is
//! created and charged in a single call, so a happy path returns `201` with
//! `paymentStatus: "succeeded"`. There is no real billing system; the outcome is
//! driven entirely by the input (below).
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the `carrier-billing:payments:create`
//! scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes:
//!
//! 1. **The charged phone number** (the submitted `amountTransaction.phoneNumber`,
//!    else the access token subject `sub`). Its trailing three digits select a
//!    **reserved CAMARA error** via the shared convention ([`crate::scenarios`]):
//!    `…400`, `…401`, `…403`, `…404`, `…409`, `…422`, `…429`, `…500`, `…503`.
//!    A malformed `phoneNumber` → `400 INVALID_ARGUMENT`; no `phoneNumber` and a
//!    non-E.164 token subject (so the account cannot be identified) →
//!    `422 MISSING_IDENTIFIER`.
//! 2. **The requested `amount`** (`amountTransaction.paymentAmount.
//!    chargingInformation.amount`). Below the schema minimum `0.001` →
//!    `400 INVALID_ARGUMENT`; above the simulator's authorised ceiling
//!    ([`UNAUTHORIZED_AMOUNT_THRESHOLD`]) → `422
//!    CARRIER_BILLING.UNAUTHORIZED_AMOUNT`; otherwise the charge **succeeds**.
//!
//! The identifier plane is checked before the amount plane, so a reserved-error
//! phone number wins even when the amount is also out of range.
//!
//! Examples: `+123456789012` charging `9.99` → `201 succeeded`; `+123456789404`
//! → `404 NOT_FOUND`; `+123456789012` charging `5000` → `422
//! CARRIER_BILLING.UNAUTHORIZED_AMOUNT`.
//!
//! ## Documented cuts (this slice)
//!
//! - Only `POST /payments` (the one-step `createPayment`) is implemented; the
//!   `retrievePayments` / `retrievePayment` / `preparePayment` / `validatePayment`
//!   / `confirmPayment` / `cancelPayment` operations are later slices. Because
//!   nothing reads a payment back yet, the created payment is **not** persisted.
//! - `sink` / `sinkCredential` are accepted for schema fidelity but not acted on
//!   (Carrier Billing charging notifications are a later slice).

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /payments` endpoint requires (CAMARA Carrier
/// Billing 0.5.0).
const CREATE_SCOPE: &str = "carrier-billing:payments:create";

/// The largest single `amount` CamaraSim treats as authorised. A request above
/// it is refused with `422 CARRIER_BILLING.UNAUTHORIZED_AMOUNT` — a distinctive,
/// deterministic Carrier Billing functional case (docs/DESIGN.md §7). Currency
/// is not modelled, so the ceiling is a bare numeric value.
const UNAUTHORIZED_AMOUNT_THRESHOLD: f64 = 1000.0;

/// The CAMARA schema minimum for a charge `amount` (`minimum: 0.001`).
const MIN_AMOUNT: f64 = 0.001;

/// Routes for Carrier Billing v0.5, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/carrier-billing/v0.5/payments", post(create_payment))
}

/// `POST /payments` request body (CAMARA `CreatePayment`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePayment {
    #[serde(rename = "amountTransaction")]
    amount_transaction: AmountTransactionInput,
    /// Accepted for schema fidelity; charging notifications are a later slice.
    #[allow(dead_code)]
    sink: Option<String>,
    /// Accepted for schema fidelity; not applied (notifications deferred).
    #[serde(rename = "sinkCredential")]
    #[allow(dead_code)]
    sink_credential: Option<Value>,
}

/// The CAMARA `AmountTransactionInput`: what to charge and to whom.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AmountTransactionInput {
    /// The mobile account to charge (E.164). Optional: omit it when a
    /// three-legged token identifies the account.
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
    #[serde(rename = "clientCorrelator")]
    client_correlator: Option<String>,
    #[serde(rename = "paymentAmount")]
    payment_amount: PaymentAmountForCharge,
    #[serde(rename = "referenceCode")]
    reference_code: String,
}

/// The CAMARA `PaymentAmountForCharge`. `chargingMetaData` / `paymentDetails` are
/// echoed verbatim (opaque here); the charge is driven by `chargingInformation`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PaymentAmountForCharge {
    #[serde(rename = "chargingInformation")]
    charging_information: ChargingInformation,
    #[serde(rename = "chargingMetaData", skip_serializing_if = "Option::is_none")]
    charging_meta_data: Option<Value>,
    #[serde(rename = "paymentDetails", skip_serializing_if = "Option::is_none")]
    payment_details: Option<Value>,
}

/// The CAMARA `ChargingInformation`: the money to charge.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChargingInformation {
    amount: f64,
    currency: String,
    description: String,
    #[serde(rename = "isTaxIncluded", skip_serializing_if = "Option::is_none")]
    is_tax_included: Option<bool>,
    #[serde(rename = "taxAmount", skip_serializing_if = "Option::is_none")]
    tax_amount: Option<f64>,
}

/// `POST /carrier-billing/v0.5/payments`.
async fn create_payment(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required and must parse as a CreatePayment.
    let req: CreatePayment = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid CreatePayment.", &correlator)
        }
    };

    // Control plane 1 — the charged phone number (submitted, else token subject).
    let phone = match resolve_phone_number(&req.amount_transaction, &claims, &correlator) {
        Ok(phone) => phone,
        Err(resp) => return resp,
    };
    if let Some(err) = scenarios::reserved_error(&phone) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Control plane 2 — the requested amount.
    let amount = req.amount_transaction.payment_amount.charging_information.amount;
    if !(amount >= MIN_AMOUNT) {
        // `!(>=)` also rejects NaN.
        return invalid_argument(
            "`amount` must be at least 0.001.",
            &correlator,
        );
    }
    if amount > UNAUTHORIZED_AMOUNT_THRESHOLD {
        return with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CARRIER_BILLING.UNAUTHORIZED_AMOUNT",
                "Unauthorized amount requested.",
            )
            .into_response(),
            &correlator,
        );
    }

    // Happy path — one-step charge succeeds immediately.
    let now = rfc3339_utc(now_unix_secs());
    let mut amount_tx = json!({
        "phoneNumber": phone,
        "paymentAmount": serde_json::to_value(&req.amount_transaction.payment_amount)
            .unwrap_or(Value::Null),
        "referenceCode": req.amount_transaction.reference_code,
    });
    if let Some(cc) = &req.amount_transaction.client_correlator {
        amount_tx["clientCorrelator"] = json!(cc);
    }
    let created = json!({
        "paymentId": mint_uuid(),
        "amountTransaction": amount_tx,
        "paymentStatus": "succeeded",
        "paymentCreationDate": now,
        "paymentDate": now,
    });

    with_correlator((StatusCode::CREATED, Json(created)).into_response(), &correlator)
}

/// Resolve the mobile account to charge: the submitted `phoneNumber` (validated
/// E.164), else the access token subject when it is itself an E.164 number
/// (three-legged fallback). On failure returns the CAMARA error `Response`:
/// `400 INVALID_ARGUMENT` for a malformed submitted `phoneNumber`, or
/// `422 MISSING_IDENTIFIER` when no `phoneNumber` was supplied and the token
/// subject is not an identifiable phone number.
fn resolve_phone_number(
    tx: &AmountTransactionInput,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    match &tx.phone_number {
        Some(phone) => {
            if !is_valid_e164(phone) {
                return Err(invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    correlator,
                ));
            }
            Ok(phone.clone())
        }
        None => {
            let subject = claims.subject().unwrap_or("");
            if is_valid_e164(subject) {
                Ok(subject.to_string())
            } else {
                Err(with_correlator(
                    CamaraError::new(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "MISSING_IDENTIFIER",
                        "The phone number cannot be identified: supply `phoneNumber` or use a token that identifies a phone number.",
                    )
                    .into_response(),
                    correlator,
                ))
            }
        }
    }
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
/// the epoch (impossible in practice) clamps to 0.
fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format a Unix timestamp (seconds, UTC) as an RFC 3339 instant with a `Z`
/// offset, e.g. `2024-01-01T14:27:08Z`. Self-contained so CamaraSim needs no
/// date/time dependency (mirrors `quality_on_demand::v1` / `sim_swap::v2`).
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

/// Mint a fresh, opaque, UUID-v4-shaped `paymentId`.
///
/// The 16 bytes come from `SHA-256(counter ‖ now)` — the monotonic counter alone
/// guarantees uniqueness — with the RFC 4122 version (4) and variant (`10`) bits
/// set so it is a well-formed v4-shaped UUID. No `uuid`/`rand` dependency
/// (mirrors `quality_on_demand::store::mint_uuid`).
fn mint_uuid() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Sha256::new();
    hasher.update(n.to_be_bytes());
    hasher.update(now_unix_secs().to_be_bytes());
    let digest = hasher.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "cb.local:8080";

    // --- Pure unit tests ---------------------------------------------------

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012345"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("cb-client")); // synthetic subject
        assert!(!is_valid_e164("")); // empty
    }

    #[test]
    fn payment_ids_are_unique_and_uuid_v4_shaped() {
        let a = mint_uuid();
        let b = mint_uuid();
        assert_ne!(a, b);
        for id in [&a, &b] {
            assert_eq!(id.len(), 36);
            let parts: Vec<&str> = id.split('-').collect();
            assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
            assert!(id.as_bytes()[14] == b'4', "version nibble is 4"); // 3rd group starts with 4
        }
    }

    #[test]
    fn rfc3339_utc_formats_a_known_epoch() {
        // 2021-01-01T00:00:00Z = 1_609_459_200.
        assert_eq!(rfc3339_utc(1_609_459_200), "2021-01-01T00:00:00Z");
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
        mint_token_with_client(scope, "cb-client").await
    }

    /// As [`mint_token`], but with a caller-chosen `client_id` — which becomes
    /// the token `sub`. Used to drive the subject-keyed (no-phoneNumber) cases.
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

    /// POST a JSON body to `/payments` with an optional Bearer token and correlator.
    async fn post_payment(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/carrier-billing/v0.5/payments")
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

    /// A minimal valid CreatePayment body for the given phone number and amount.
    fn payment_body(phone: &str, amount: f64) -> String {
        format!(
            r#"{{"amountTransaction":{{"phoneNumber":"{phone}","paymentAmount":{{"chargingInformation":{{"amount":{amount},"currency":"EUR","description":"A digital good"}}}},"referenceCode":"ref-001"}}}}"#
        )
    }

    /// Mint a scoped token and create a payment with the given body.
    async fn create_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_payment(Some(&token), body, None).await
    }

    #[tokio::test]
    async fn happy_path_charges_and_succeeds() {
        let (status, _, body) = create_ok_token(&payment_body("+123456789012", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["paymentStatus"], "succeeded");
        assert!(body["paymentId"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(body["amountTransaction"]["phoneNumber"], "+123456789012");
        assert_eq!(body["amountTransaction"]["referenceCode"], "ref-001");
        assert_eq!(
            body["amountTransaction"]["paymentAmount"]["chargingInformation"]["amount"],
            9.99
        );
        assert!(body["paymentCreationDate"].as_str().unwrap().ends_with('Z'));
        assert_eq!(body["paymentDate"], body["paymentCreationDate"]);
    }

    #[tokio::test]
    async fn client_correlator_is_echoed_when_present() {
        let body = r#"{"amountTransaction":{"phoneNumber":"+123456789012","clientCorrelator":"cc-42","paymentAmount":{"chargingInformation":{"amount":1.0,"currency":"USD","description":"x"}},"referenceCode":"ref-001"}}"#;
        let (status, _, resp) = create_ok_token(body).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp["amountTransaction"]["clientCorrelator"], "cc-42");
    }

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = create_ok_token(&payment_body("+123456789404", 9.99)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = create_ok_token(&payment_body("+123456789429", 9.99)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn amount_above_threshold_is_unauthorized() {
        let (status, _, body) = create_ok_token(&payment_body("+123456789012", 5000.0)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "CARRIER_BILLING.UNAUTHORIZED_AMOUNT");
    }

    #[tokio::test]
    async fn amount_below_minimum_is_invalid_argument() {
        let (status, _, body) = create_ok_token(&payment_body("+123456789012", 0.0)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn reserved_identifier_wins_over_amount() {
        // …404 reserved suffix AND an over-threshold amount → the identifier plane wins.
        let (status, _, body) = create_ok_token(&payment_body("+123456789404", 5000.0)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = create_ok_token(&payment_body("0123", 9.99)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn missing_required_field_is_rejected() {
        // No referenceCode.
        let body = r#"{"amountTransaction":{"phoneNumber":"+123456789012","paymentAmount":{"chargingInformation":{"amount":1.0,"currency":"EUR","description":"x"}}}}"#;
        let (status, _, resp) = create_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let body = r#"{"amountTransaction":{"phoneNumber":"+123456789012","paymentAmount":{"chargingInformation":{"amount":1.0,"currency":"EUR","description":"x"}},"referenceCode":"r"},"x":1}"#;
        let (status, _, resp) = create_ok_token(body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn no_phone_number_falls_back_to_an_e164_token_subject() {
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789012").await;
        let body = r#"{"amountTransaction":{"paymentAmount":{"chargingInformation":{"amount":2.5,"currency":"EUR","description":"x"}},"referenceCode":"ref-sub"}}"#;
        let (status, _, resp) = post_payment(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(resp["paymentStatus"], "succeeded");
        assert_eq!(resp["amountTransaction"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn no_phone_number_and_non_e164_subject_is_missing_identifier() {
        // Default synthetic subject "cb-client" is not a phone number.
        let body = r#"{"amountTransaction":{"paymentAmount":{"chargingInformation":{"amount":2.5,"currency":"EUR","description":"x"}},"referenceCode":"ref-sub"}}"#;
        let (status, _, resp) = create_ok_token(body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(resp["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn subject_reserved_suffix_selects_a_camara_error() {
        let token = mint_token_with_client(CREATE_SCOPE, "+123456789503").await;
        let body = r#"{"amountTransaction":{"paymentAmount":{"chargingInformation":{"amount":2.5,"currency":"EUR","description":"x"}},"referenceCode":"ref-sub"}}"#;
        let (status, _, resp) = post_payment(Some(&token), body, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_payment(Some(&token), &payment_body("+123456789012", 9.99), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_payment(None, &payment_body("+123456789012", 9.99), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, _) = post_payment(
            Some(&token),
            &payment_body("+123456789012", 9.99),
            Some("corr-cb"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-cb")
        );

        let (status, headers, _) = post_payment(
            Some(&token),
            &payment_body("+123456789404", 9.99),
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
