//! CAMARA **Carrier Billing** API.
//!
//! Carrier Billing lets a merchant charge a purchase to an end user's mobile
//! account (the operator bills it on the phone bill / deducts from prepaid
//! credit). CamaraSim mounts the real published version — Carrier Billing
//! `0.5.0` (CAMARA release r3.2) — under its canonical sub-1.0 URL
//! `/carrier-billing/v0.5`, mirroring KYC Match `v0.3` / Location Retrieval
//! `v0.4` (docs/DESIGN.md §9).
//!
//! The API is CamaraSim's first **payments** API and opens Phase 5 (DESIGN §12).
//! It is identifier-keyed: the account to charge is the submitted
//! `amountTransaction.phoneNumber` (two-legged / CIBA) or the identity a
//! three-legged access token authenticated (in which case `phoneNumber` is
//! omitted).
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_5`] — mounted at `/carrier-billing/v0.5`. `POST /payments`
//!   (`createPayment`, the one-step charge), `GET /payments/{paymentId}`
//!   (`retrievePayment`, read-back), `GET /payments` (`retrievePayments`, list),
//!   `POST /payments/prepare` (`preparePayment`, reserve), and
//!   `POST /payments/{paymentId}/validate` (`validatePayment`, OTP validation)
//!   so far; the two-step confirm / cancel operations are later slices.
//!
//! Reading a payment back makes Carrier Billing **stateful**, so a shared
//! in-memory [`store`] holds created payments keyed by `paymentId`.

pub mod store;
pub mod v0_5;

use axum::Router;

/// Every mounted major version of Carrier Billing.
pub fn routes() -> Router {
    Router::new().merge(v0_5::routes())
}
