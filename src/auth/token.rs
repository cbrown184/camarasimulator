//! `POST /oauth2/token` — the OAuth2 token endpoint.
//!
//! This pass implements the **`client_credentials`** grant (RFC 6749 §4.4,
//! CAMARA *Security & Interoperability Profile* — see docs/DESIGN.md §6): a
//! two-legged flow for APIs that need no end-user context. The other advertised
//! grants (`authorization_code`, CIBA) are filled in by later passes and, until
//! then, return `unsupported_grant_type`.
//!
//! The endpoint issues a **real, signed** RS256 JWT (RFC 9068 `at+jwt`) using
//! the bundled JWKS key, so a client that fetches `/oauth2/jwks` can verify it.
//! The token-verification middleware (next pass) validates these on protected
//! routes.
//!
//! ## Simulator behaviour (functional cases)
//!
//! The simulator is a test double, so it authenticates the *shape* of the flow
//! but not real credentials:
//!
//! - **Client authentication** is required and resolved from either
//!   `client_secret_basic` (HTTP Basic `Authorization` header) or
//!   `client_secret_post` (`client_id`/`client_secret` form fields). Any secret
//!   is accepted; the `client_id` becomes the token `sub`/`client_id`. A request
//!   with no resolvable `client_id` fails with `invalid_client` (401).
//! - **Scope** is granted as requested: the space-delimited `scope` parameter is
//!   echoed into the token's `scope` claim and the response. Absent → empty.
//! - **Expiry** is fixed at [`EXPIRES_IN`] seconds.
//!
//! Errors follow the OAuth2 token-endpoint format (RFC 6749 §5.2):
//! `{ "error": "...", "error_description": "..." }` with `Cache-Control: no-store`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{base_url, keys};

/// Lifetime of an issued access token, in seconds (1 hour).
const EXPIRES_IN: u64 = 3600;

/// Parsed `application/x-www-form-urlencoded` token request. Unknown fields are
/// ignored by serde; all fields are optional so validation (and the resulting
/// OAuth2 error) is handled explicitly rather than by an extractor rejection.
#[derive(Debug, Default, Deserialize)]
struct TokenForm {
    grant_type: Option<String>,
    scope: Option<String>,
    client_id: Option<String>,
}

/// `POST /oauth2/token`.
///
/// Takes the raw body as a `String` (any content type) so malformed input yields
/// a spec-shaped OAuth2 error instead of an axum extractor rejection.
pub async fn handler(headers: HeaderMap, body: String) -> Response {
    let form: TokenForm = match serde_urlencoded::from_str(&body) {
        Ok(form) => form,
        Err(_) => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request body must be application/x-www-form-urlencoded",
            )
        }
    };

    match form.grant_type.as_deref() {
        Some("client_credentials") => client_credentials(&headers, form),
        // Advertised in discovery but not yet implemented in this build.
        Some("authorization_code") | Some("urn:openid:params:grant-type:ciba") => oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "grant type is advertised by discovery but not yet implemented in this build",
        ),
        Some(_) => oauth_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            "the authorization server does not support this grant type",
        ),
        None => oauth_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "the 'grant_type' parameter is required",
        ),
    }
}

/// Issue a token for the `client_credentials` grant.
fn client_credentials(headers: &HeaderMap, form: TokenForm) -> Response {
    // The client must authenticate (RFC 6749 §4.4.2). We accept any secret but
    // require a client identity via Basic auth or the `client_id` form field.
    let client_id = match client_id_from_basic(headers).or(form.client_id) {
        Some(id) => id,
        None => {
            return oauth_error(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "client authentication required (client_secret_basic or client_secret_post)",
            )
        }
    };

    let scope = form.scope.unwrap_or_default();
    let issuer = base_url(headers);
    let now = unix_now();

    let claims = json!({
        "iss": issuer,
        "sub": client_id,
        "aud": issuer,
        "client_id": client_id,
        "scope": scope,
        "jti": next_jti(now),
        "iat": now,
        "exp": now + EXPIRES_IN,
    });

    let mut body = json!({
        "access_token": encode_jwt(&claims),
        "token_type": "Bearer",
        "expires_in": EXPIRES_IN,
    });
    if !scope.is_empty() {
        body["scope"] = Value::String(scope);
    }

    (StatusCode::OK, no_store(), Json(body)).into_response()
}

/// Encode `claims` as a signed RS256 JWT with an `at+jwt` header (RFC 9068).
fn encode_jwt(claims: &Value) -> String {
    let header = json!({
        "alg": keys::SIGNING_ALG,
        "typ": "at+jwt",
        "kid": keys::SIGNING_KID,
    });
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header serializes"));
    let claims_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("claims serialize"));
    let signing_input = format!("{header_b64}.{claims_b64}");
    let signature = URL_SAFE_NO_PAD.encode(keys::sign_rs256(signing_input.as_bytes()));
    format!("{signing_input}.{signature}")
}

/// Extract a `client_id` from an HTTP Basic `Authorization` header
/// (`client_secret_basic`). The secret is not checked. Returns `None` if the
/// header is absent, malformed, or carries an empty client id.
fn client_id_from_basic(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = STANDARD.decode(encoded.trim()).ok()?;
    let creds = String::from_utf8(decoded).ok()?;
    let (id, _secret) = creds.split_once(':')?;
    (!id.is_empty()).then(|| id.to_string())
}

/// Current Unix time in seconds (server runtime clock; not on any hot loop).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A per-process-unique JWT id, cheap and allocation-light (no `uuid` dep).
fn next_jti(now: u64) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{now:x}-{n:x}")
}

/// Token-endpoint responses must not be cached (RFC 6749 §5.1).
fn no_store() -> [(header::HeaderName, &'static str); 2] {
    [
        (header::CACHE_CONTROL, "no-store"),
        (header::PRAGMA, "no-cache"),
    ]
}

/// An OAuth2 token-endpoint error (RFC 6749 §5.2).
fn oauth_error(status: StatusCode, code: &str, description: &str) -> Response {
    let body = json!({ "error": code, "error_description": description });
    (status, no_store(), Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    /// POST an urlencoded body to the token route and return (status, headers, json).
    async fn post_token(
        body: &str,
        auth: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/oauth2/token")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("host", "sim.local:8080");
        if let Some(a) = auth {
            builder = builder.header("authorization", a);
        }
        let response = super::super::routes()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();

        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        (status, headers, json)
    }

    /// Decode a JWT into (header, claims) JSON, panicking on a malformed token.
    fn decode_jwt(token: &str) -> (Value, Value) {
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "a JWT has three segments");
        let header = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        let claims = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        (header, claims)
    }

    #[tokio::test]
    async fn client_credentials_issues_a_bearer_token() {
        let (status, headers, body) =
            post_token("grant_type=client_credentials&client_id=client-1", None).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["token_type"], "Bearer");
        assert_eq!(body["expires_in"], EXPIRES_IN);
        assert!(body["access_token"].as_str().unwrap().contains('.'));
        // Token responses must not be cached.
        assert_eq!(
            headers.get("cache-control").unwrap().to_str().unwrap(),
            "no-store"
        );
    }

    #[tokio::test]
    async fn issued_token_has_at_jwt_header_and_kid() {
        let (_, _, body) =
            post_token("grant_type=client_credentials&client_id=client-1", None).await;
        let (header, _) = decode_jwt(body["access_token"].as_str().unwrap());

        assert_eq!(header["alg"], "RS256");
        assert_eq!(header["typ"], "at+jwt");
        assert_eq!(header["kid"], keys::SIGNING_KID);
    }

    #[tokio::test]
    async fn issued_token_carries_camara_claims() {
        let (_, _, body) = post_token(
            "grant_type=client_credentials&client_id=client-1&scope=dpv%3AFraudPreventionAndDetection%23check-sim-swap",
            None,
        )
        .await;
        let token = body["access_token"].as_str().unwrap();
        let (_, claims) = decode_jwt(token);

        assert_eq!(claims["iss"], "http://sim.local:8080");
        assert_eq!(claims["aud"], "http://sim.local:8080");
        assert_eq!(claims["sub"], "client-1");
        assert_eq!(claims["client_id"], "client-1");
        assert_eq!(
            claims["scope"],
            "dpv:FraudPreventionAndDetection#check-sim-swap"
        );
        assert!(claims["jti"].is_string());
        let iat = claims["iat"].as_u64().unwrap();
        let exp = claims["exp"].as_u64().unwrap();
        assert_eq!(exp - iat, EXPIRES_IN);

        // The granted scope is echoed in the response body too.
        assert_eq!(
            body["scope"],
            "dpv:FraudPreventionAndDetection#check-sim-swap"
        );
    }

    #[tokio::test]
    async fn issued_token_signature_verifies_against_the_jwks_key() {
        use rsa::pkcs1v15::{Signature, VerifyingKey};
        use rsa::signature::Verifier;
        use rsa::RsaPublicKey;
        use sha2::Sha256;

        let (_, _, body) =
            post_token("grant_type=client_credentials&client_id=client-1", None).await;
        let token = body["access_token"].as_str().unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature_bytes = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();

        let verifying_key = VerifyingKey::<Sha256>::new(RsaPublicKey::from(keys::signing_key()));
        let signature = Signature::try_from(signature_bytes.as_slice()).unwrap();
        verifying_key
            .verify(signing_input.as_bytes(), &signature)
            .expect("issued token verifies under the JWKS public key");
    }

    #[tokio::test]
    async fn scope_is_omitted_from_response_when_not_requested() {
        let (_, _, body) =
            post_token("grant_type=client_credentials&client_id=client-1", None).await;
        assert!(body.get("scope").is_none());
        let (_, claims) = decode_jwt(body["access_token"].as_str().unwrap());
        assert_eq!(claims["scope"], "");
    }

    #[tokio::test]
    async fn client_id_resolves_from_basic_auth() {
        // base64("client-2:any-secret") == "Y2xpZW50LTI6YW55LXNlY3JldA=="
        let (status, _, body) = post_token(
            "grant_type=client_credentials",
            Some("Basic Y2xpZW50LTI6YW55LXNlY3JldA=="),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let (_, claims) = decode_jwt(body["access_token"].as_str().unwrap());
        assert_eq!(claims["sub"], "client-2");
    }

    #[tokio::test]
    async fn missing_client_is_invalid_client() {
        let (status, _, body) = post_token("grant_type=client_credentials", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "invalid_client");
    }

    #[tokio::test]
    async fn missing_grant_type_is_invalid_request() {
        let (status, _, body) = post_token("client_id=client-1", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_request");
    }

    #[tokio::test]
    async fn unknown_grant_type_is_unsupported() {
        let (status, _, body) =
            post_token("grant_type=password&client_id=client-1", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "unsupported_grant_type");
    }

    #[tokio::test]
    async fn advertised_but_unimplemented_grant_is_unsupported() {
        let (status, _, body) =
            post_token("grant_type=authorization_code&client_id=client-1&code=x", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "unsupported_grant_type");
    }
}
