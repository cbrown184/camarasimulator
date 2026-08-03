//! Carrier Billing **v0.5** (CAMARA Carrier Billing 0.5.0, release r3.2).
//!
//! Endpoints so far:
//! - `POST /carrier-billing/v0.5/payments` — create (and, in the one-step flow,
//!   immediately charge) a payment against an end user's mobile account
//!   (operationId `createPayment`).
//! - `GET /carrier-billing/v0.5/payments/{paymentId}` — read a created payment
//!   back by its id (operationId `retrievePayment`, scope
//!   `carrier-billing:payments:read`). Reading a payment back makes Carrier
//!   Billing **stateful**: `createPayment` now persists the payment it charges
//!   in the shared in-memory [`super::store`], and `retrievePayment` returns it
//!   verbatim (`200`) or `404 NOT_FOUND` for an unknown id. The `paymentId` is
//!   opaque (UUID-shaped), so the store state is `retrievePayment`'s only
//!   control plane (known → `200`, unknown → `404`), mirroring QoD `getSession`.
//! - `GET /carrier-billing/v0.5/payments` — list created payments as a
//!   `PaymentArray` (operationId `retrievePayments`, scope
//!   `carrier-billing:payments:read`). Returns every stored payment as a JSON
//!   array (`200`, empty array when none). The store state is its only control
//!   plane; query-parameter pagination/filtering is a documented cut (below).
//! - `POST /carrier-billing/v0.5/payments/prepare` — **reserve** (not yet
//!   charge) a payment (operationId `preparePayment`, scope
//!   `carrier-billing:payments:create`), the first step of the two-step
//!   reserve → validate → confirm / cancel flow. A happy path returns `201` with
//!   `paymentStatus: "reserved"` (no `paymentDate` — nothing is charged yet) and
//!   persists the reservation so it can be read back / later confirmed. It shares
//!   `createPayment`'s identifier + amount control planes (below).
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
//! - The one-step `createPayment`, read-back `retrievePayment`, list
//!   `retrievePayments`, and the two-step flow's reserve `preparePayment` +
//!   validate `validatePayment` steps are implemented; the remaining two-step
//!   `confirmPayment` / `cancelPayment` operations are later slices.
//!   `preparePayment` lands a `…888`-tail reservation in `pending_validation`
//!   (with `validationInfo`) for `validatePayment` to clear, and `reserved`
//!   otherwise; the 409 `ALREADY_EXISTS` duplicate-session case on
//!   `preparePayment` (a `clientCorrelator` already in flight) is not modelled.
//! - `retrievePayments` returns the full list unpaginated and unfiltered: its
//!   `page`/`perPage`, `paymentCreationDate.gte`/`.lte`, `paymentStatus`,
//!   `merchantIdentifier`, and `order` query parameters are accepted but not
//!   applied (a later slice), and payments are not scoped per client.
//! - `sink` / `sinkCredential` are accepted for schema fidelity but not acted on
//!   (Carrier Billing charging notifications are a later slice). Because they are
//!   never applied, the persisted payment carries no `sink` — the CAMARA
//!   `Payment` schema's `sink` is optional, so a `retrievePayment` response
//!   simply omits it.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the `POST /payments` endpoint requires (CAMARA Carrier
/// Billing 0.5.0).
const CREATE_SCOPE: &str = "carrier-billing:payments:create";

/// The OAuth2 scope the `GET /payments/{paymentId}` read-back endpoint requires
/// (CAMARA Carrier Billing 0.5.0).
const READ_SCOPE: &str = "carrier-billing:payments:read";

/// The OAuth2 scope the two-step write operations require (CAMARA Carrier
/// Billing 0.5.0). `validatePayment` (and, in later slices, confirm / cancel)
/// carry it.
const WRITE_SCOPE: &str = "carrier-billing:payments:write";

/// A reservation whose resolved phone number ends in this tail requires OTP
/// **validation** before it can be confirmed: `preparePayment` lands it in
/// `pending_validation` (with an `authorizationId`) instead of `reserved`, and a
/// `validatePayment` must clear it. A deterministic functional case
/// (docs/DESIGN.md §7); `888` is not a reserved-error suffix, so it never
/// collides with the identifier error plane. E.g. `+123456789888`.
const VALIDATION_REQUIRED_TAIL: &str = "888";

/// How many OTP attempts a pending validation allows before the reservation is
/// denied (mirrors One Time Password SMS's 3-attempt budget).
const VALIDATION_ATTEMPTS: u32 = 3;

/// The largest single `amount` CamaraSim treats as authorised. A request above
/// it is refused with `422 CARRIER_BILLING.UNAUTHORIZED_AMOUNT` — a distinctive,
/// deterministic Carrier Billing functional case (docs/DESIGN.md §7). Currency
/// is not modelled, so the ceiling is a bare numeric value.
const UNAUTHORIZED_AMOUNT_THRESHOLD: f64 = 1000.0;

/// The CAMARA schema minimum for a charge `amount` (`minimum: 0.001`).
const MIN_AMOUNT: f64 = 0.001;

/// Routes for Carrier Billing v0.5, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/carrier-billing/v0.5/payments",
            post(create_payment).get(retrieve_payments),
        )
        // Static `/payments/prepare` coexists with the `/payments/:payment_id`
        // param route below — axum's router (matchit) prioritises the static
        // segment, so a `POST /payments/prepare` never collides with the
        // parameterised read.
        .route(
            "/carrier-billing/v0.5/payments/prepare",
            post(prepare_payment),
        )
        .route(
            "/carrier-billing/v0.5/payments/:payment_id",
            get(retrieve_payment),
        )
        // The two-step OTP validation step. `matchit` disambiguates it from the
        // `/payments/:payment_id` read (different path length) and from the
        // static `/payments/prepare`.
        .route(
            "/carrier-billing/v0.5/payments/:payment_id/validate",
            post(validate_payment),
        )
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

/// `POST /payments/{paymentId}/validate` request body (CAMARA `ValidatePayment`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatePayment {
    /// The `authorizationId` echoed to the caller in the reservation's
    /// `validationInfo`.
    #[serde(rename = "authorizationId")]
    authorization_id: String,
    /// The OTP `code` received "via SMS" to validate the payment.
    code: String,
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
    if let Err(resp) = check_amount(amount, &correlator) {
        return resp;
    }

    // Happy path — one-step charge succeeds immediately.
    let now = rfc3339_utc(now_unix_secs());
    let amount_tx = build_amount_tx(&req.amount_transaction, &phone);
    let payment_id = mint_uuid();
    let created = json!({
        "paymentId": payment_id,
        "amountTransaction": amount_tx,
        "paymentStatus": "succeeded",
        "paymentCreationDate": now,
        "paymentDate": now,
    });

    // Persist so `retrievePayment` (GET /payments/{paymentId}) can read it back.
    store::insert(payment_id, created.clone());

    with_correlator((StatusCode::CREATED, Json(created)).into_response(), &correlator)
}

/// `POST /carrier-billing/v0.5/payments/prepare` — **reserve** (do not yet
/// charge) a payment (operationId `preparePayment`), the first step of the
/// two-step reserve → validate → confirm / cancel flow.
///
/// Unlike `createPayment`, which charges in a single call, `preparePayment` only
/// *reserves* the amount against the account: on the happy path it mints a
/// `paymentId`, records a payment with `paymentStatus: "reserved"` (no
/// `paymentDate` — nothing has been charged) and returns `201`. The reserved
/// payment is persisted in the shared in-memory [`super::store`], so it can be
/// read back by `retrievePayment` and — in later slices — driven to `succeeded`
/// by `confirmPayment` or to `cancelled` by `cancelPayment`.
///
/// It shares `createPayment`'s two control planes (docs/DESIGN.md §7): the
/// reserved phone number (submitted `amountTransaction.phoneNumber`, else the
/// token subject) — reserved suffix → canonical CAMARA error, malformed → 400
/// `INVALID_ARGUMENT`, unidentifiable → 422 `MISSING_IDENTIFIER` — and the
/// requested `amount` (`< 0.001` → 400 `INVALID_ARGUMENT`; above the authorised
/// ceiling → 422 `CARRIER_BILLING.UNAUTHORIZED_AMOUNT`). Requires the
/// `carrier-billing:payments:create` scope.
///
/// **Documented cuts (this slice):** the reserved payment always lands in the
/// `reserved` state — the `pending_validation` / `validationInfo` (OTP) path and
/// the 409 `ALREADY_EXISTS` duplicate-session case are later slices, delivered
/// with `validatePayment`. `sink` / `sinkCredential` are accepted but not acted
/// on (mirroring `createPayment`).
async fn prepare_payment(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's create scope.
    if let Err(e) = claims.require_scope(CREATE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required and must parse as a ReservePayment (structurally the
    // same input CamaraSim accepts for `createPayment`).
    let req: CreatePayment = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid ReservePayment.", &correlator)
        }
    };

    // Control plane 1 — the reserved phone number (submitted, else token subject).
    let phone = match resolve_phone_number(&req.amount_transaction, &claims, &correlator) {
        Ok(phone) => phone,
        Err(resp) => return resp,
    };
    if let Some(err) = scenarios::reserved_error(&phone) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Control plane 2 — the requested amount.
    let amount = req.amount_transaction.payment_amount.charging_information.amount;
    if let Err(resp) = check_amount(amount, &correlator) {
        return resp;
    }

    // Happy path — the amount is reserved, not charged: no `paymentDate`
    // (nothing has been billed yet). Either it is `reserved` immediately, or —
    // when the resolved phone number requires OTP validation — it lands in
    // `pending_validation` until a `validatePayment` clears it.
    let now = rfc3339_utc(now_unix_secs());
    let amount_tx = build_amount_tx(&req.amount_transaction, &phone);
    let payment_id = mint_uuid();

    if phone.ends_with(VALIDATION_REQUIRED_TAIL) {
        // OTP validation required first. Mint an `authorizationId` (echoed in
        // `validationInfo`) and remember the expected code — deterministic from
        // the phone number so a headless caller can compute it.
        let validation = store::PendingValidation {
            authorization_id: mint_uuid(),
            code: otp_code(&phone),
            attempts_left: VALIDATION_ATTEMPTS,
        };
        let pending = json!({
            "paymentId": payment_id,
            "amountTransaction": amount_tx,
            "paymentStatus": "pending_validation",
            "paymentCreationDate": now,
            "validationInfo": {
                "action": "validate",
                "authorizationId": validation.authorization_id,
            },
        });
        store::insert(payment_id.clone(), pending.clone());
        store::insert_pending(payment_id, validation);
        return with_correlator(
            (StatusCode::CREATED, Json(pending)).into_response(),
            &correlator,
        );
    }

    let reserved = json!({
        "paymentId": payment_id,
        "amountTransaction": amount_tx,
        "paymentStatus": "reserved",
        "paymentCreationDate": now,
    });

    // Persist so the reservation can be read back and later confirmed/cancelled.
    store::insert(payment_id, reserved.clone());

    with_correlator((StatusCode::CREATED, Json(reserved)).into_response(), &correlator)
}

/// `POST /carrier-billing/v0.5/payments/{paymentId}/validate` — validate a
/// two-step reservation's OTP (operationId `validatePayment`), the second step
/// of the reserve → validate → confirm / cancel flow.
///
/// A `preparePayment` for a phone number ending in
/// [`VALIDATION_REQUIRED_TAIL`] lands in `pending_validation` carrying an
/// `authorizationId`. This endpoint clears that validation: the caller submits
/// the `authorizationId` and the OTP `code`, and on success the reservation
/// moves to `reserved` (`204 No Content`). Requires the
/// `carrier-billing:payments:write` scope.
///
/// ## Functional cases — the store state + submitted code are the control plane
///
/// Keyed only on the in-memory store (no reserved-identifier plane — the
/// `paymentId` is opaque):
///
/// - correct `authorizationId` + `code` → `204` (reservation → `reserved`);
/// - wrong `authorizationId` → `400 CARRIER_BILLING.INVALID_AUTHORIZATION_ID`;
/// - correct `authorizationId`, wrong `code` → `400 CARRIER_BILLING.INVALID_CODE`
///   (an attempt is consumed) until the [`VALIDATION_ATTEMPTS`]-attempt budget is
///   spent, when the last wrong code denies the reservation →
///   `400 CARRIER_BILLING.VALIDATION_FAILED` (reservation → `denied`);
/// - a payment that is not awaiting validation (already reserved / succeeded /
///   denied) → `409 ALREADY_EXISTS`;
/// - an unknown `paymentId` → `404 NOT_FOUND`.
///
/// The OTP `code` is deterministic from the reserved phone number — its last six
/// digits, zero-padded ([`otp_code`]) — so a headless caller can compute what to
/// submit (there is no real SMS; mirrors One Time Password SMS).
async fn validate_payment(
    claims: Claims,
    headers: HeaderMap,
    Path(payment_id): Path<String>,
    body: Bytes,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's write scope.
    if let Err(e) = claims.require_scope(WRITE_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The body is required and must parse as a ValidatePayment.
    let req: ValidatePayment = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(_) => {
            return invalid_argument("Request body is not a valid ValidatePayment.", &correlator)
        }
    };

    let resp = match store::validate_pending(&payment_id, &req.authorization_id, &req.code) {
        store::ValidateOutcome::Validated => StatusCode::NO_CONTENT.into_response(),
        store::ValidateOutcome::InvalidAuthId => CamaraError::new(
            StatusCode::BAD_REQUEST,
            "CARRIER_BILLING.INVALID_AUTHORIZATION_ID",
            "Invalid authorizationId.",
        )
        .into_response(),
        store::ValidateOutcome::InvalidCode => CamaraError::new(
            StatusCode::BAD_REQUEST,
            "CARRIER_BILLING.INVALID_CODE",
            "Invalid code.",
        )
        .into_response(),
        store::ValidateOutcome::ValidationFailed => CamaraError::new(
            StatusCode::BAD_REQUEST,
            "CARRIER_BILLING.VALIDATION_FAILED",
            "The maximum number of validation attempts has been consumed.",
        )
        .into_response(),
        store::ValidateOutcome::NotPending => CamaraError::new(
            StatusCode::CONFLICT,
            "ALREADY_EXISTS",
            "The payment is not awaiting validation.",
        )
        .into_response(),
        store::ValidateOutcome::Unknown => {
            CamaraError::not_found("No payment found for the provided paymentId.").into_response()
        }
    };

    with_correlator(resp, &correlator)
}

/// The OTP `code` CamaraSim "sends" for a pending-validation reservation: the
/// resolved phone number's last six digits, zero-padded to six. Deterministic so
/// a headless caller can compute what to submit to `validatePayment` (there is
/// no real SMS; mirrors One Time Password SMS). E.g. `+123456789888` → `789888`.
fn otp_code(phone: &str) -> String {
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    let start = digits.len().saturating_sub(6);
    format!("{:0>6}", &digits[start..])
}

/// Build the echoed `amountTransaction` JSON for a resolved `phone`: the charge
/// (`paymentAmount`), `referenceCode`, and — when supplied — `clientCorrelator`.
/// Shared by `createPayment` and `preparePayment`.
fn build_amount_tx(tx: &AmountTransactionInput, phone: &str) -> Value {
    let mut amount_tx = json!({
        "phoneNumber": phone,
        "paymentAmount": serde_json::to_value(&tx.payment_amount).unwrap_or(Value::Null),
        "referenceCode": tx.reference_code,
    });
    if let Some(cc) = &tx.client_correlator {
        amount_tx["clientCorrelator"] = json!(cc);
    }
    amount_tx
}

/// Validate the requested charge `amount` against the two amount control-plane
/// rules shared by `createPayment` and `preparePayment`: below the schema
/// minimum `0.001` (or NaN) → `400 INVALID_ARGUMENT`; above the authorised
/// ceiling → `422 CARRIER_BILLING.UNAUTHORIZED_AMOUNT`. On failure returns the
/// CAMARA error `Response` (correlator echoed).
fn check_amount(amount: f64, correlator: &Option<HeaderValue>) -> Result<(), Response> {
    if !(amount >= MIN_AMOUNT) {
        // `!(>=)` also rejects NaN.
        return Err(invalid_argument("`amount` must be at least 0.001.", correlator));
    }
    if amount > UNAUTHORIZED_AMOUNT_THRESHOLD {
        return Err(with_correlator(
            CamaraError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CARRIER_BILLING.UNAUTHORIZED_AMOUNT",
                "Unauthorized amount requested.",
            )
            .into_response(),
            correlator,
        ));
    }
    Ok(())
}

/// `GET /carrier-billing/v0.5/payments/{paymentId}` — read a created payment
/// back by its id (operationId `retrievePayment`).
///
/// Keyed only on the in-memory store: a stored payment → `200` with its
/// `PaymentCreated`/`Payment` representation (returned verbatim), an
/// unknown/never-created id → `404 NOT_FOUND`. The `paymentId` is an opaque
/// UUID-shaped token, so — unlike `createPayment` — there is no
/// reserved-identifier control plane here (mirrors QoD `getSession`). Requires a
/// token carrying `carrier-billing:payments:read`.
async fn retrieve_payment(claims: Claims, headers: HeaderMap, Path(payment_id): Path<String>) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's read scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    match store::get(&payment_id) {
        Some(payment) => {
            with_correlator((StatusCode::OK, Json(payment)).into_response(), &correlator)
        }
        None => with_correlator(
            CamaraError::not_found("No payment found for the provided paymentId.").into_response(),
            &correlator,
        ),
    }
}

/// `GET /carrier-billing/v0.5/payments` — list created payments as a
/// `PaymentArray` (operationId `retrievePayments`).
///
/// Returns every payment held in the shared in-memory [`super::store`] as a JSON
/// array of `PaymentCreated`/`Payment` objects — `200` with an empty array when
/// none exist (CAMARA lists never `404` on an empty result). Requires a token
/// carrying `carrier-billing:payments:read` (the same read scope as
/// `retrievePayment`).
///
/// **Documented cut (this slice):** the real operation's query parameters —
/// `page`/`perPage` pagination, `paymentCreationDate.gte`/`.lte` and
/// `paymentStatus`/`merchantIdentifier` filtering, and `order` — are accepted
/// (unknown query parameters are ignored) but **not applied**: the full list is
/// returned unpaginated and unfiltered. CamaraSim also does not scope payments
/// per client (a documented simplification, mirroring the Geofencing
/// Subscriptions list). Pagination/filtering is a later slice.
async fn retrieve_payments(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's read scope.
    if let Err(e) = claims.require_scope(READ_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    let payments = store::all();
    with_correlator((StatusCode::OK, Json(payments)).into_response(), &correlator)
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

    /// POST a JSON body to `/payments/prepare` with an optional Bearer token and correlator.
    async fn post_prepare(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/carrier-billing/v0.5/payments/prepare")
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

    /// Mint a create-scoped token and prepare (reserve) a payment with the given body.
    async fn prepare_ok_token(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(CREATE_SCOPE).await;
        post_prepare(Some(&token), body, None).await
    }

    /// GET `/payments/{paymentId}` with an optional Bearer token and correlator.
    async fn get_payment(
        token: Option<&str>,
        payment_id: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/carrier-billing/v0.5/payments/{payment_id}"))
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

    /// GET `/payments` (list) with an optional Bearer token and correlator.
    async fn list_payments(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/carrier-billing/v0.5/payments")
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

    // --- retrievePayment (GET /payments/{paymentId}) -----------------------

    #[tokio::test]
    async fn created_payment_can_be_retrieved_by_id() {
        // Create a payment, then read it back with a read-scoped token.
        let (status, _, created) = create_ok_token(&payment_body("+123456789012", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["paymentId"].as_str().unwrap().to_string();

        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_payment(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        // The read-back representation matches what was created, verbatim.
        assert_eq!(fetched, created);
        assert_eq!(fetched["paymentId"], id.as_str());
        assert_eq!(fetched["paymentStatus"], "succeeded");
        assert_eq!(fetched["amountTransaction"]["phoneNumber"], "+123456789012");
    }

    #[tokio::test]
    async fn retrieve_unknown_payment_is_not_found() {
        let read = mint_token(READ_SCOPE).await;
        let (status, _, body) = get_payment(
            Some(&read),
            "11111111-1111-4111-8111-111111111111",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn retrieve_without_the_read_scope_is_forbidden() {
        // A payment created with the create scope cannot be read with a token
        // carrying only the create scope — read requires the read scope.
        let (_, _, created) = create_ok_token(&payment_body("+123456789012", 9.99)).await;
        let id = created["paymentId"].as_str().unwrap().to_string();

        let create_only = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = get_payment(Some(&create_only), &id, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn retrieve_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            get_payment(None, "11111111-1111-4111-8111-111111111111", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    // --- retrievePayments (GET /payments, list) ----------------------------

    #[tokio::test]
    async fn list_returns_a_json_array_containing_a_created_payment() {
        // Create a payment, then confirm it appears in the list. The store is
        // process-global, so assert containment (not an exact list) — other
        // tests may have created payments too.
        let (status, _, created) = create_ok_token(&payment_body("+123456789012", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["paymentId"].as_str().unwrap().to_string();

        let read = mint_token(READ_SCOPE).await;
        let (status, _, list) = list_payments(Some(&read), None).await;
        assert_eq!(status, StatusCode::OK);
        let items = list.as_array().expect("response is a JSON array");
        assert!(
            items.iter().any(|p| p["paymentId"] == id.as_str()),
            "the created payment appears in the list"
        );
        // The listed representation is the verbatim created payment.
        let found = items
            .iter()
            .find(|p| p["paymentId"] == id.as_str())
            .unwrap();
        assert_eq!(found, &created);
    }

    #[tokio::test]
    async fn list_without_the_read_scope_is_forbidden() {
        let create_only = mint_token(CREATE_SCOPE).await;
        let (status, _, body) = list_payments(Some(&create_only), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn list_without_a_token_is_unauthenticated() {
        let (status, _, body) = list_payments(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn list_echoes_x_correlator() {
        let read = mint_token(READ_SCOPE).await;
        let (status, headers, _) = list_payments(Some(&read), Some("corr-list")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-list")
        );
    }

    #[tokio::test]
    async fn retrieve_echoes_x_correlator_on_success_and_error() {
        let (_, _, created) = create_ok_token(&payment_body("+123456789012", 9.99)).await;
        let id = created["paymentId"].as_str().unwrap().to_string();
        let read = mint_token(READ_SCOPE).await;

        let (status, headers, _) = get_payment(Some(&read), &id, Some("corr-get-ok")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-ok")
        );

        let (status, headers, _) = get_payment(
            Some(&read),
            "11111111-1111-4111-8111-111111111111",
            Some("corr-get-404"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-get-404")
        );
    }

    // --- preparePayment (POST /payments/prepare) ---------------------------

    #[tokio::test]
    async fn prepare_reserves_without_charging() {
        let (status, _, body) = prepare_ok_token(&payment_body("+123456789012", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        // Reserved, not charged: `reserved` status and NO `paymentDate`.
        assert_eq!(body["paymentStatus"], "reserved");
        assert!(body["paymentDate"].is_null(), "nothing is charged yet");
        assert!(body["paymentId"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(body["amountTransaction"]["phoneNumber"], "+123456789012");
        assert_eq!(body["amountTransaction"]["referenceCode"], "ref-001");
        assert_eq!(
            body["amountTransaction"]["paymentAmount"]["chargingInformation"]["amount"],
            9.99
        );
        assert!(body["paymentCreationDate"].as_str().unwrap().ends_with('Z'));
    }

    #[tokio::test]
    async fn prepare_reserved_suffix_selects_a_camara_error() {
        let (status, _, body) = prepare_ok_token(&payment_body("+123456789404", 9.99)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn prepare_amount_above_threshold_is_unauthorized() {
        let (status, _, body) = prepare_ok_token(&payment_body("+123456789012", 5000.0)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "CARRIER_BILLING.UNAUTHORIZED_AMOUNT");
    }

    #[tokio::test]
    async fn prepare_amount_below_minimum_is_invalid_argument() {
        let (status, _, body) = prepare_ok_token(&payment_body("+123456789012", 0.0)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn prepare_no_phone_and_non_e164_subject_is_missing_identifier() {
        let body = r#"{"amountTransaction":{"paymentAmount":{"chargingInformation":{"amount":2.5,"currency":"EUR","description":"x"}},"referenceCode":"ref-sub"}}"#;
        let (status, _, resp) = prepare_ok_token(body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(resp["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn prepared_payment_can_be_retrieved_verbatim() {
        // A reservation is persisted, so it reads back through `retrievePayment`.
        let (status, _, reserved) = prepare_ok_token(&payment_body("+123456789012", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = reserved["paymentId"].as_str().unwrap().to_string();

        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_payment(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched, reserved);
        assert_eq!(fetched["paymentStatus"], "reserved");
    }

    #[tokio::test]
    async fn prepare_without_the_create_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_prepare(Some(&token), &payment_body("+123456789012", 9.99), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn prepare_without_a_token_is_unauthenticated() {
        let (status, _, body) =
            post_prepare(None, &payment_body("+123456789012", 9.99), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn prepare_echoes_x_correlator_on_success_and_error() {
        let token = mint_token(CREATE_SCOPE).await;
        let (status, headers, _) = post_prepare(
            Some(&token),
            &payment_body("+123456789012", 9.99),
            Some("corr-prep-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-prep-ok")
        );

        let (status, headers, _) = post_prepare(
            Some(&token),
            &payment_body("+123456789404", 9.99),
            Some("corr-prep-err"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-prep-err")
        );
    }

    // --- validatePayment (POST /payments/{paymentId}/validate) -------------

    #[test]
    fn otp_code_is_the_last_six_digits_zero_padded() {
        assert_eq!(otp_code("+123456789888"), "789888");
        assert_eq!(otp_code("+12888"), "012888"); // fewer than six digits → left-padded
    }

    /// POST a JSON body to `/payments/{paymentId}/validate`.
    async fn post_validate(
        token: Option<&str>,
        payment_id: &str,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("/carrier-billing/v0.5/payments/{payment_id}/validate"))
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

    /// A ValidatePayment request body.
    fn validate_body(auth_id: &str, code: &str) -> String {
        format!(r#"{{"authorizationId":"{auth_id}","code":"{code}"}}"#)
    }

    /// Reserve a payment for a `…888` phone number so it lands in
    /// `pending_validation`, returning its `(paymentId, authorizationId)`.
    async fn prepare_pending(phone: &str) -> (String, String) {
        let (status, _, body) = prepare_ok_token(&payment_body(phone, 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["paymentStatus"], "pending_validation");
        assert_eq!(body["validationInfo"]["action"], "validate");
        let id = body["paymentId"].as_str().unwrap().to_string();
        let auth = body["validationInfo"]["authorizationId"]
            .as_str()
            .unwrap()
            .to_string();
        (id, auth)
    }

    #[tokio::test]
    async fn prepare_with_validation_tail_lands_in_pending_validation() {
        let (status, _, body) = prepare_ok_token(&payment_body("+123456789888", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["paymentStatus"], "pending_validation");
        assert!(body["paymentDate"].is_null());
        // The reservation carries a `validate`-action validationInfo with an id.
        assert_eq!(body["validationInfo"]["action"], "validate");
        assert!(body["validationInfo"]["authorizationId"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
    }

    #[tokio::test]
    async fn validate_correct_code_moves_reservation_to_reserved() {
        let (id, auth) = prepare_pending("+123456789888").await;
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, _) =
            post_validate(Some(&write), &id, &validate_body(&auth, "789888"), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // The reservation is now `reserved` and no longer carries validationInfo.
        let read = mint_token(READ_SCOPE).await;
        let (status, _, fetched) = get_payment(Some(&read), &id, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["paymentStatus"], "reserved");
        assert!(fetched["validationInfo"].is_null());
    }

    #[tokio::test]
    async fn validate_wrong_authorization_id_is_rejected() {
        let (id, _auth) = prepare_pending("+123456789888").await;
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) =
            post_validate(Some(&write), &id, &validate_body("not-the-id", "789888"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "CARRIER_BILLING.INVALID_AUTHORIZATION_ID");
    }

    #[tokio::test]
    async fn validate_wrong_code_is_invalid_then_fails_after_budget() {
        let (id, auth) = prepare_pending("+123456789888").await;
        let write = mint_token(WRITE_SCOPE).await;

        // Two wrong attempts (budget 3) → INVALID_CODE each.
        for _ in 0..2 {
            let (status, _, body) =
                post_validate(Some(&write), &id, &validate_body(&auth, "000000"), None).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(body["code"], "CARRIER_BILLING.INVALID_CODE");
        }
        // Third wrong attempt spends the budget → VALIDATION_FAILED, denied.
        let (status, _, body) =
            post_validate(Some(&write), &id, &validate_body(&auth, "000000"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "CARRIER_BILLING.VALIDATION_FAILED");

        // The reservation is now `denied`.
        let read = mint_token(READ_SCOPE).await;
        let (_, _, fetched) = get_payment(Some(&read), &id, None).await;
        assert_eq!(fetched["paymentStatus"], "denied");

        // A correct code after denial no longer validates — nothing is pending.
        let (status, _, body) =
            post_validate(Some(&write), &id, &validate_body(&auth, "789888"), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn validate_a_payment_not_awaiting_validation_is_conflict() {
        // A plain reservation (no `…888` tail) is `reserved`, not pending.
        let (status, _, reserved) = prepare_ok_token(&payment_body("+123456789012", 9.99)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(reserved["paymentStatus"], "reserved");
        let id = reserved["paymentId"].as_str().unwrap().to_string();

        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) =
            post_validate(Some(&write), &id, &validate_body("anything", "789012"), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn validate_already_validated_payment_is_conflict() {
        let (id, auth) = prepare_pending("+123456789888").await;
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, _) =
            post_validate(Some(&write), &id, &validate_body(&auth, "789888"), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // A second validate on the now-`reserved` payment → 409.
        let (status, _, body) =
            post_validate(Some(&write), &id, &validate_body(&auth, "789888"), None).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "ALREADY_EXISTS");
    }

    #[tokio::test]
    async fn validate_unknown_payment_is_not_found() {
        let write = mint_token(WRITE_SCOPE).await;
        let (status, _, body) = post_validate(
            Some(&write),
            "11111111-1111-4111-8111-111111111111",
            &validate_body("x", "000000"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn validate_without_the_write_scope_is_forbidden() {
        let (id, auth) = prepare_pending("+123456789888").await;
        // A create-scoped token (used to prepare) cannot validate.
        let create_only = mint_token(CREATE_SCOPE).await;
        let (status, _, body) =
            post_validate(Some(&create_only), &id, &validate_body(&auth, "789888"), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn validate_without_a_token_is_unauthenticated() {
        let (status, _, body) = post_validate(
            None,
            "11111111-1111-4111-8111-111111111111",
            &validate_body("x", "000000"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn validate_echoes_x_correlator_on_success_and_error() {
        let (id, auth) = prepare_pending("+123456789888").await;
        let write = mint_token(WRITE_SCOPE).await;

        // Error case first (wrong id), so the pending validation survives for the
        // success case below.
        let (status, headers, _) = post_validate(
            Some(&write),
            &id,
            &validate_body("wrong", "789888"),
            Some("corr-val-err"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-val-err")
        );

        let (status, headers, _) = post_validate(
            Some(&write),
            &id,
            &validate_body(&auth, "789888"),
            Some("corr-val-ok"),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-val-ok")
        );
    }
}
