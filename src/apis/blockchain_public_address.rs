//! CAMARA **Blockchain Public Address** API.
//!
//! Blockchain Public Address lets a client discover the blockchain public
//! address(es) a mobile subscriber has bound to their line — a building block
//! for Web3 / crypto onboarding, letting an operator vouch that a given
//! `phoneNumber` controls a given on-chain address. It is identifier-keyed on
//! the submitted `phoneNumber`.
//!
//! One submodule per major version (docs/DESIGN.md §5, §9). So far:
//! - [`v0_3`] — mounted at `/blockchain-public-address/v0.3` (CAMARA Blockchain
//!   Public Address 0.3.0, release r2.2 — the latest published version, so
//!   mounted at its real sub-1.0 base path like KYC Match / KYC Tenure).
//!
//! Implemented so far:
//! - the **stateless read** op `retrieveBlockchainPublicAddress`, and
//! - the **stateful bind** op `bindBlockchainPublicAddress`
//!   (`POST /blockchain-public-addresses`), which persists the binding in an
//!   in-memory [`store`] so a later `deleteBlockchainPublicAddress` slice can
//!   address it by `id`.
//!
//! The `DELETE /blockchain-public-addresses/{id}` op is deferred to a later
//! slice (it reads the same [`store`], mirroring how QoD / Carrier Billing were
//! grown endpoint-by-endpoint).

pub mod store;
pub mod v0_3;

use axum::Router;

/// Every mounted major version of Blockchain Public Address.
pub fn routes() -> Router {
    Router::new().merge(v0_3::routes())
}
