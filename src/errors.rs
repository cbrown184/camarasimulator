//! Shared CAMARA error model (docs/DESIGN.md §8).
//!
//! Every CAMARA API answers a failure with the *same* body — the HTTP status
//! echoed as `status`, a machine-readable `code`, and a human-readable
//! `message`:
//!
//! ```json
//! { "status": 404, "code": "NOT_FOUND", "message": "The specified resource is not found." }
//! ```
//!
//! This module is the single, version-agnostic base catalog of that model. The
//! `code`/`message` strings are the CAMARA Commonalities standard values (see
//! the CAMARA API Design Guide, "Error responses"). Where a specific API
//! version narrows or extends the catalog, that lives with the API's own module
//! and per-version spec — this is the shared floor every version builds on.
//!
//! It has no consumers on the request path yet (the first CAMARA business API
//! lands in Phase 1), so its constructors are `dead_code`-allowed for now.
#![allow(dead_code)]

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

/// A CAMARA error, ready to be returned from any handler via [`IntoResponse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CamaraError {
    /// The HTTP status; also echoed inside the body as `status`.
    pub status: StatusCode,
    /// The machine-readable CAMARA error code (e.g. `NOT_FOUND`).
    pub code: String,
    /// The human-readable explanation.
    pub message: String,
}

impl CamaraError {
    /// Build an error from its parts.
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    // ---- Canonical constructors (CAMARA Commonalities standard codes) -------

    /// 400 — client supplied an invalid argument, body, or query parameter.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "INVALID_ARGUMENT", message)
    }

    /// 401 — request not authenticated (missing/invalid/expired credentials).
    pub fn unauthenticated(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHENTICATED", message)
    }

    /// 403 — authenticated but lacking permission for this action.
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "PERMISSION_DENIED", message)
    }

    /// 404 — the specified resource was not found.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
    }

    /// 409 — conflict with the current state of the target resource.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "CONFLICT", message)
    }

    /// 422 — the service is not applicable for the provided identifier.
    pub fn service_not_applicable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SERVICE_NOT_APPLICABLE",
            message,
        )
    }

    /// 429 — rate limit reached.
    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "TOO_MANY_REQUESTS", message)
    }

    /// 500 — unknown server error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL",
            message,
        )
    }

    /// 503 — service unavailable.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE", message)
    }

    /// The canonical CAMARA error for one of the standard HTTP statuses, using
    /// the generic Commonalities `code`/`message` for that status.
    ///
    /// Returns `None` for any status outside the standard set below — this is
    /// exactly the set the reserved-identifier scenario convention selects from
    /// (see [`crate::scenarios`]).
    pub fn for_status(status: u16) -> Option<Self> {
        Some(match status {
            400 => Self::invalid_argument(
                "Client specified an invalid argument, request body or query param.",
            ),
            401 => Self::unauthenticated(
                "Request not authenticated due to missing, invalid, or expired credentials.",
            ),
            403 => Self::permission_denied(
                "Client does not have sufficient permissions to perform this action.",
            ),
            404 => Self::not_found("The specified resource is not found."),
            409 => Self::conflict("A specified resource duplicate entry found."),
            422 => Self::service_not_applicable(
                "The service is not available for the provided identifier.",
            ),
            429 => Self::too_many_requests("Rate limit reached."),
            500 => Self::internal("Unknown server error. Typically a server bug."),
            503 => Self::unavailable("Service unavailable."),
            _ => return None,
        })
    }

    /// The CAMARA error body as JSON (`{ status, code, message }`).
    pub fn body(&self) -> Value {
        json!({
            "status": self.status.as_u16(),
            "code": self.code,
            "message": self.message,
        })
    }
}

impl IntoResponse for CamaraError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body())).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn for_status_covers_the_reserved_standard_set() {
        // (status, expected code) — the CAMARA Commonalities generic codes.
        let expected = [
            (400, "INVALID_ARGUMENT"),
            (401, "UNAUTHENTICATED"),
            (403, "PERMISSION_DENIED"),
            (404, "NOT_FOUND"),
            (409, "CONFLICT"),
            (422, "SERVICE_NOT_APPLICABLE"),
            (429, "TOO_MANY_REQUESTS"),
            (500, "INTERNAL"),
            (503, "UNAVAILABLE"),
        ];
        for (status, code) in expected {
            let err = CamaraError::for_status(status).expect("standard status");
            assert_eq!(err.status.as_u16(), status);
            assert_eq!(err.code, code);
            assert!(!err.message.is_empty());
        }
    }

    #[test]
    fn for_status_returns_none_outside_the_standard_set() {
        for status in [200, 201, 402, 405, 418, 451, 501, 502, 504] {
            assert!(CamaraError::for_status(status).is_none());
        }
    }

    #[test]
    fn body_echoes_status_code_and_message() {
        let err = CamaraError::not_found("gone");
        let body = err.body();
        assert_eq!(body["status"], 404);
        assert_eq!(body["code"], "NOT_FOUND");
        assert_eq!(body["message"], "gone");
    }

    #[tokio::test]
    async fn into_response_sets_status_and_camara_body() {
        let response = CamaraError::too_many_requests("slow down").into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["status"], 429);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
        assert_eq!(body["message"], "slow down");
    }
}
