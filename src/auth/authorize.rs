//! `GET /oauth2/authorize` — the OAuth2 authorization endpoint (RFC 6749 §3.1).
//!
//! This is the front leg of the **`authorization_code` + PKCE** grant (the CAMARA
//! three-legged flow, docs/DESIGN.md §6). A client redirects the user here; the
//! server authenticates the user, obtains consent, and redirects back to the
//! client's `redirect_uri` with a short-lived authorization `code`. The client
//! then redeems that code at `POST /oauth2/token` (see [`super::token`]).
//!
//! ## Simulator behaviour (auto-consent)
//!
//! CamaraSim is headless — there is no human to log in or click "allow" — so the
//! authorize step **auto-consents**: a well-formed request is immediately granted
//! and redirected back with a code. The *shape* of the flow is faithful
//! nonetheless (docs/DESIGN.md §6): PKCE is mandatory and its `S256` challenge is
//! bound to the code, the requested scope is carried through to the token, and the
//! `redirect_uri`/`client_id` are pinned to the code and re-checked at redemption.
//!
//! ## Error handling (RFC 6749 §4.1.2.1)
//!
//! - If `client_id` or `redirect_uri` is missing or invalid, the server **cannot**
//!   safely redirect (it would be an open redirector), so it responds directly
//!   with an OAuth2 error body (`{ error, error_description }`, HTTP 400).
//! - Otherwise the error is delivered **by redirect** to `redirect_uri` as
//!   `error`/`error_description` (plus `state` if supplied):
//!     - `response_type` other than `code` → `unsupported_response_type`
//!     - missing `code_challenge`, or `code_challenge_method` not `S256` →
//!       `invalid_request` (PKCE with S256 is mandatory in the CAMARA profile).

use axum::extract::RawQuery;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use super::base_url;
use super::codes::{self, AuthCode};
use super::purpose;

/// Parsed `GET /oauth2/authorize` query parameters. All optional so validation
/// (and the resulting redirect-or-direct error) is handled explicitly rather than
/// by an extractor rejection; unknown parameters are ignored.
#[derive(Debug, Default, Deserialize)]
struct AuthorizeParams {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
}

/// `GET /oauth2/authorize`.
pub async fn handler(headers: HeaderMap, RawQuery(query): RawQuery) -> Response {
    let params: AuthorizeParams = match serde_urlencoded::from_str(query.as_deref().unwrap_or("")) {
        Ok(p) => p,
        Err(_) => {
            return direct_error(
                "invalid_request",
                "the query string is not valid application/x-www-form-urlencoded",
            )
        }
    };

    // `client_id` and `redirect_uri` gate whether we may redirect at all: an error
    // in either means we must respond directly, never bounce to an unvalidated URI.
    let client_id = match non_empty(params.client_id) {
        Some(c) => c,
        None => return direct_error("invalid_request", "the 'client_id' parameter is required"),
    };
    let redirect_uri = match non_empty(params.redirect_uri) {
        Some(u) if is_absolute_uri(&u) => u,
        Some(_) => {
            return direct_error(
                "invalid_request",
                "the 'redirect_uri' parameter must be an absolute URI",
            )
        }
        None => return direct_error("invalid_request", "the 'redirect_uri' parameter is required"),
    };

    // Past this point, errors are delivered back to the (validated) redirect_uri.
    let state = params.state.as_deref();

    if params.response_type.as_deref() != Some("code") {
        return redirect_error(
            &redirect_uri,
            "unsupported_response_type",
            "only 'response_type=code' is supported",
            state,
        );
    }

    // PKCE is mandatory in the CAMARA profile, and only S256 is supported (the
    // RFC 7636 'plain' default is rejected).
    let code_challenge = match non_empty(params.code_challenge) {
        Some(c) => c,
        None => {
            return redirect_error(
                &redirect_uri,
                "invalid_request",
                "PKCE is required: 'code_challenge' is missing",
                state,
            )
        }
    };
    if params.code_challenge_method.as_deref() != Some("S256") {
        return redirect_error(
            &redirect_uri,
            "invalid_request",
            "'code_challenge_method' must be S256",
            state,
        );
    }

    // A requested `dpv:` purpose scope must be well-formed (docs/DESIGN.md §7);
    // a malformed one is a redirectable `invalid_scope` (RFC 6749 §4.1.2.1).
    if let Some(scope) = params.scope.as_deref() {
        if let Err(bad) = purpose::validate_scope(scope) {
            return redirect_error(
                &redirect_uri,
                "invalid_scope",
                &format!("the requested scope '{bad}' is not a valid CAMARA purpose scope"),
                state,
            );
        }
    }

    // Auto-consent: mint a code bound to this client / redirect_uri / scope /
    // challenge / audience and redirect the user agent back to the client.
    let code = codes::issue(AuthCode {
        client_id,
        redirect_uri: redirect_uri.clone(),
        scope: params.scope.unwrap_or_default(),
        code_challenge,
        audience: base_url(&headers),
        expires_at: codes::unix_now() + codes::CODE_TTL,
    });

    let mut pairs: Vec<(&str, &str)> = vec![("code", &code)];
    if let Some(s) = state {
        pairs.push(("state", s));
    }
    redirect(append_query(&redirect_uri, &pairs))
}

/// `Some(s)` if `s` is present and non-empty after trimming, else `None`.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.trim().is_empty())
}

/// A permissive absolute-URI check: a non-empty scheme of URI-scheme characters
/// followed by `://` and a non-empty remainder. Enough to reject a relative or
/// empty `redirect_uri` (which would make the redirect an open redirector) without
/// pretending to be a full URI parser — the simulator registers no client URIs.
fn is_absolute_uri(uri: &str) -> bool {
    match uri.split_once("://") {
        Some((scheme, rest)) => {
            !scheme.is_empty()
                && !rest.is_empty()
                && scheme
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        }
        None => false,
    }
}

/// Append `pairs` to `uri`'s query string, percent-encoding values and choosing
/// `?` or `&` depending on whether `uri` already carries a query.
fn append_query(uri: &str, pairs: &[(&str, &str)]) -> String {
    let qs = serde_urlencoded::to_string(pairs).unwrap_or_default();
    let sep = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{sep}{qs}")
}

/// A 302 redirect to `location`, marked `no-store` (it carries a fresh code).
fn redirect(location: String) -> Response {
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, location),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
    )
        .into_response()
}

/// Deliver an OAuth2 error back to the client by redirect (RFC 6749 §4.1.2.1).
fn redirect_error(
    redirect_uri: &str,
    error: &str,
    description: &str,
    state: Option<&str>,
) -> Response {
    let mut pairs: Vec<(&str, &str)> = vec![("error", error), ("error_description", description)];
    if let Some(s) = state {
        pairs.push(("state", s));
    }
    redirect(append_query(redirect_uri, &pairs))
}

/// An OAuth2 error returned directly (used when we cannot trust `redirect_uri`).
fn direct_error(error: &str, description: &str) -> Response {
    let body = json!({ "error": error, "error_description": description });
    (
        StatusCode::BAD_REQUEST,
        [(header::CACHE_CONTROL, "no-store")],
        Json(body),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    /// Drive `GET /oauth2/authorize?<query>` through the auth router and return
    /// (status, Location header if any, parsed JSON body if any).
    async fn authorize(query: &str) -> (StatusCode, Option<String>, Value) {
        let response = super::super::routes()
            .oneshot(
                Request::builder()
                    .uri(format!("/oauth2/authorize?{query}"))
                    .header("host", "sim.local:8080")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, location, json)
    }

    /// The query part of a `Location` URL as `(key, value)` pairs.
    fn location_query(location: &str) -> Vec<(String, String)> {
        let q = location.split_once('?').map(|(_, q)| q).unwrap_or("");
        serde_urlencoded::from_str(q).unwrap()
    }

    const VALID: &str = "response_type=code&client_id=app-1\
        &redirect_uri=https%3A%2F%2Fapp.example%2Fcb\
        &scope=openid&state=xyz\
        &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM\
        &code_challenge_method=S256";

    #[tokio::test]
    async fn valid_request_redirects_with_code_and_state() {
        let (status, location, _) = authorize(VALID).await;
        assert_eq!(status, StatusCode::FOUND);
        let location = location.expect("Location header");
        assert!(location.starts_with("https://app.example/cb?"));
        let q = location_query(&location);
        assert!(q.iter().any(|(k, v)| k == "code" && !v.is_empty()));
        assert!(q.iter().any(|(k, v)| k == "state" && v == "xyz"));
        // No error is delivered on the happy path.
        assert!(!q.iter().any(|(k, _)| k == "error"));
    }

    #[tokio::test]
    async fn state_is_omitted_when_not_supplied() {
        let q = VALID.replace("&state=xyz", "");
        let (_, location, _) = authorize(&q).await;
        let location = location.unwrap();
        assert!(!location_query(&location).iter().any(|(k, _)| k == "state"));
    }

    #[tokio::test]
    async fn missing_client_id_is_a_direct_error() {
        let q = VALID.replace("client_id=app-1", "");
        let (status, location, body) = authorize(&q).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(location.is_none(), "must not redirect without a client");
        assert_eq!(body["error"], "invalid_request");
    }

    #[tokio::test]
    async fn missing_redirect_uri_is_a_direct_error() {
        let q = VALID.replace("&redirect_uri=https%3A%2F%2Fapp.example%2Fcb", "");
        let (status, location, body) = authorize(&q).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(location.is_none());
        assert_eq!(body["error"], "invalid_request");
    }

    #[tokio::test]
    async fn relative_redirect_uri_is_a_direct_error() {
        let q = VALID.replace(
            "redirect_uri=https%3A%2F%2Fapp.example%2Fcb",
            "redirect_uri=%2Fnot-absolute",
        );
        let (status, location, body) = authorize(&q).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(location.is_none());
        assert_eq!(body["error"], "invalid_request");
    }

    #[tokio::test]
    async fn wrong_response_type_redirects_with_error() {
        let q = VALID.replace("response_type=code", "response_type=token");
        let (status, location, _) = authorize(&q).await;
        assert_eq!(status, StatusCode::FOUND);
        let q = location_query(&location.unwrap());
        assert!(q
            .iter()
            .any(|(k, v)| k == "error" && v == "unsupported_response_type"));
        // The client's state is echoed on error redirects too.
        assert!(q.iter().any(|(k, v)| k == "state" && v == "xyz"));
    }

    #[tokio::test]
    async fn missing_pkce_challenge_redirects_with_invalid_request() {
        let q = VALID.replace(
            "&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            "",
        );
        let (status, location, _) = authorize(&q).await;
        assert_eq!(status, StatusCode::FOUND);
        assert!(location_query(&location.unwrap())
            .iter()
            .any(|(k, v)| k == "error" && v == "invalid_request"));
    }

    #[tokio::test]
    async fn malformed_purpose_scope_redirects_with_invalid_scope() {
        // `dpv:Foo` is not a valid purpose scope (no `#action`). Because
        // client_id/redirect_uri are valid, the error is delivered by redirect.
        let q = VALID.replace("scope=openid", "scope=dpv:Foo");
        let (status, location, _) = authorize(&q).await;
        assert_eq!(status, StatusCode::FOUND);
        let q = location_query(&location.unwrap());
        assert!(q
            .iter()
            .any(|(k, v)| k == "error" && v == "invalid_scope"));
        // The client's state is echoed on the error redirect.
        assert!(q.iter().any(|(k, v)| k == "state" && v == "xyz"));
    }

    #[tokio::test]
    async fn well_formed_purpose_scope_is_accepted() {
        let q = VALID.replace(
            "scope=openid",
            "scope=dpv:FraudPreventionAndDetection%23check-sim-swap",
        );
        let (status, location, _) = authorize(&q).await;
        assert_eq!(status, StatusCode::FOUND);
        let q = location_query(&location.unwrap());
        // Happy path: a code is issued, no error delivered.
        assert!(q.iter().any(|(k, v)| k == "code" && !v.is_empty()));
        assert!(!q.iter().any(|(k, _)| k == "error"));
    }

    #[tokio::test]
    async fn plain_pkce_method_is_rejected() {
        let q = VALID.replace("code_challenge_method=S256", "code_challenge_method=plain");
        let (status, location, _) = authorize(&q).await;
        assert_eq!(status, StatusCode::FOUND);
        assert!(location_query(&location.unwrap())
            .iter()
            .any(|(k, v)| k == "error" && v == "invalid_request"));
    }

    #[test]
    fn is_absolute_uri_accepts_and_rejects() {
        assert!(is_absolute_uri("https://app.example/cb"));
        assert!(is_absolute_uri("com.example.app://cb"));
        assert!(!is_absolute_uri("/relative"));
        assert!(!is_absolute_uri("app.example/cb"));
        assert!(!is_absolute_uri("https://"));
    }

    #[test]
    fn append_query_picks_the_right_separator() {
        assert_eq!(
            append_query("https://a/cb", &[("code", "x")]),
            "https://a/cb?code=x"
        );
        assert_eq!(
            append_query("https://a/cb?foo=1", &[("code", "x")]),
            "https://a/cb?foo=1&code=x"
        );
    }
}
