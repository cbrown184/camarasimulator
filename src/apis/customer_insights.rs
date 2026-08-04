//! CAMARA **Customer Insights** API.
//!
//! Customer Insights lets a client retrieve a privacy-preserving risk/trust
//! **score** for the line behind a phone number — a single number on a
//! caller-chosen scale (`gaugeMetric`, a 300–850 credit-style band, or
//! `veritasIndex`, a compact 0–19 index), never the underlying data. It is
//! identifier-keyed: the caller either submits the `phoneNumber` directly
//! (two-legged / CIBA) or relies on the line identity carried by a three-legged
//! access token.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_2`] — mounted at `/customer-insights/v0.2` (CAMARA Customer Insights
//!   0.2.0, r2.2 — the latest published version, so mounted at its real sub-1.0
//!   base path like KYC Match / KYC Tenure / Number Recycling).

pub mod v0_2;

use axum::Router;

/// Every mounted major version of Customer Insights.
pub fn routes() -> Router {
    Router::new().merge(v0_2::routes())
}
