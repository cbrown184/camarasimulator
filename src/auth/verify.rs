//! Token-verification middleware for protected routes.
//!
//! CAMARA business APIs are protected: a caller presents the access token issued
//! by [`super::token`] as an HTTP Bearer credential (RFC 6750), and the resource
//! server verifies it before serving the request (docs/DESIGN.md §6). This module
//! provides that verification as a reusable axum extractor, [`Claims`], which a
//! protected handler names in its signature to require (and read) a valid token.
//!
//! ## What is verified
//!
//! For a presented `Authorization: Bearer <jwt>`:
//!
//! 1. **Signature** — the JWT is `RS256`-signed by the simulator's key. The
//!    header `alg` must be exactly `RS256` (an `alg: none` or HMAC token is
//!    rejected outright), and the signature must verify under the public half of
//!    the bundled key published at `/oauth2/jwks` ([`keys::verify_rs256`]).
//! 2. **Expiry** — `exp` is required (RFC 9068 `at+jwt`) and must be in the
//!    future relative to the server clock.
//! 3. **Not-before** — `nbf` is optional, but if present the token must not be
//!    accepted before it (RFC 7519 §4.1.5): the server clock must be at or after
//!    `nbf`.
//! 4. **Audience** — `aud` must contain this resource server's identifier, i.e.
//!    the issuer base URL derived from the request (the same value the token
//!    endpoint stamps as `aud`), so a token minted for one host is not accepted
//!    by another.
//!
//! **Scope** is enforced per-endpoint by the handler, not by the extractor: a
//! protected handler calls [`Claims::require_scope`] with the scope its endpoint
//! demands (a CAMARA purpose scope such as
//! `dpv:FraudPreventionAndDetection#check-sim-swap`).
//!
//! ## Error model
//!
//! Failures map to the CAMARA error body `{ status, code, message }` with an
//! RFC 6750 `WWW-Authenticate` challenge:
//!
//! - missing / malformed / bad-signature / expired / not-yet-valid /
//!   wrong-audience token → **401 `UNAUTHENTICATED`**;
//! - valid token lacking the required scope → **403 `PERMISSION_DENIED`**.

// This is the resource-server surface: the mounted CAMARA business endpoints name
// `Claims` in their handler signatures to require a verified token and call
// `Claims::require_scope` for endpoint authorisation, so the whole surface is
// reached from product routes as well as the auth tests.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde_json::{json, Value};

use super::{base_url, keys};

/// The validated claims of a verified access token.
///
/// Holds the decoded JWT payload; construction (via the [`FromRequestParts`]
/// extractor or [`verify_token`]) already proves the token was well-formed,
/// correctly signed, unexpired, and audience-matched, so a `Claims` in hand is
/// an *authenticated* caller. Authorisation (scope) is a separate, per-endpoint
/// check — see [`Claims::require_scope`].
#[derive(Debug, Clone, PartialEq)]
pub struct Claims {
    raw: Value,
}

impl Claims {
    /// The granted scope string (`scope` claim, space-delimited); empty if absent.
    pub fn scope(&self) -> &str {
        self.raw.get("scope").and_then(Value::as_str).unwrap_or("")
    }

    /// Whether the token was granted `needed` as one of its space-delimited scopes.
    pub fn has_scope(&self, needed: &str) -> bool {
        self.scope().split(' ').any(|s| s == needed)
    }

    /// Require `needed` as a granted scope, else a `PERMISSION_DENIED` error a
    /// handler can return directly. This is the endpoint-level authorisation gate.
    pub fn require_scope(&self, needed: &str) -> Result<(), AuthError> {
        if self.has_scope(needed) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope(needed.to_string()))
        }
    }

    /// The authenticated client identifier (`client_id` claim), if present.
    ///
    /// Part of the `Claims` accessor surface; currently read only by tests and the
    /// token-introspection helper, so allow it to be otherwise unused.
    #[allow(dead_code)]
    pub fn client_id(&self) -> Option<&str> {
        self.raw.get("client_id").and_then(Value::as_str)
    }

    /// The token subject (`sub` claim), if present.
    pub fn subject(&self) -> Option<&str> {
        self.raw.get("sub").and_then(Value::as_str)
    }

    /// Expiry (`exp`) as a Unix timestamp, if present and numeric.
    fn expires_at(&self) -> Option<u64> {
        self.raw.get("exp").and_then(Value::as_u64)
    }

    /// Not-before (`nbf`) as a Unix timestamp, if present and numeric.
    fn not_before(&self) -> Option<u64> {
        self.raw.get("nbf").and_then(Value::as_u64)
    }

    /// Audiences (`aud`), which may be a single string or an array of strings
    /// (RFC 7519 §4.1.3).
    fn audiences(&self) -> Vec<&str> {
        match self.raw.get("aud") {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        }
    }
}

/// A token-verification failure, mapped to the CAMARA error model on response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// No usable `Authorization: Bearer` credential was presented.
    Missing,
    /// The credential is not a well-formed RS256 JWT (bad structure, non-RS256
    /// `alg`, undecodable segment, or a missing required claim).
    Malformed,
    /// The signature did not verify under the JWKS key.
    BadSignature,
    /// The token has expired (`exp` in the past).
    Expired,
    /// The token is not yet valid (`nbf` in the future).
    NotYetValid,
    /// The token's `aud` does not include this resource server.
    WrongAudience,
    /// The token is valid but lacks the scope the endpoint requires.
    InsufficientScope(String),
}

impl AuthError {
    /// The RFC 6750 `WWW-Authenticate` challenge value for this failure.
    fn challenge(&self) -> String {
        match self {
            // A missing credential is not an *error* per RFC 6750 — just a bare
            // challenge inviting the client to authenticate.
            AuthError::Missing => "Bearer".to_string(),
            AuthError::Malformed => {
                r#"Bearer error="invalid_token", error_description="malformed access token""#
                    .to_string()
            }
            AuthError::BadSignature => {
                r#"Bearer error="invalid_token", error_description="signature verification failed""#
                    .to_string()
            }
            AuthError::Expired => {
                r#"Bearer error="invalid_token", error_description="the access token expired""#
                    .to_string()
            }
            AuthError::NotYetValid => {
                r#"Bearer error="invalid_token", error_description="the access token is not yet valid""#
                    .to_string()
            }
            AuthError::WrongAudience => {
                r#"Bearer error="invalid_token", error_description="token audience does not match this resource server""#
                    .to_string()
            }
            AuthError::InsufficientScope(scope) => {
                format!(r#"Bearer error="insufficient_scope", scope="{scope}""#)
            }
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let challenge = self.challenge();
        let (status, code, message): (StatusCode, &str, String) = match &self {
            AuthError::Missing => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Request not authenticated: a Bearer access token is required.".to_string(),
            ),
            AuthError::Malformed => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Request not authenticated: the access token is malformed.".to_string(),
            ),
            AuthError::BadSignature => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Request not authenticated: the access token signature is invalid.".to_string(),
            ),
            AuthError::Expired => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Request not authenticated: the access token has expired.".to_string(),
            ),
            AuthError::NotYetValid => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Request not authenticated: the access token is not yet valid.".to_string(),
            ),
            AuthError::WrongAudience => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "Request not authenticated: the access token audience does not match this server."
                    .to_string(),
            ),
            AuthError::InsufficientScope(scope) => (
                StatusCode::FORBIDDEN,
                "PERMISSION_DENIED",
                format!("Access token does not have the required scope '{scope}'."),
            ),
        };

        let body = json!({
            "status": status.as_u16(),
            "code": code,
            "message": message,
        });
        (
            status,
            [(header::WWW_AUTHENTICATE, challenge)],
            Json(body),
        )
            .into_response()
    }
}

/// Extract the raw bearer token from an `Authorization` header, if present and
/// non-empty. The scheme match is case-insensitive per RFC 7235.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

/// Decode a base64url (no-pad) JWT segment as JSON.
fn decode_segment(segment: &str) -> Option<Value> {
    let bytes = URL_SAFE_NO_PAD.decode(segment).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Verify a bearer token against `expected_audience` at time `now`.
///
/// This is the pure core of the middleware: no I/O, no clock — the caller
/// supplies `now` so the checks are deterministic under test. The extractor
/// wraps this with the request headers and the server clock.
pub fn verify_token(token: &str, expected_audience: &str, now: u64) -> Result<Claims, AuthError> {
    // A JWS compact token is exactly three dot-separated segments.
    let mut segments = token.split('.');
    let (header_b64, claims_b64, sig_b64) =
        match (segments.next(), segments.next(), segments.next(), segments.next()) {
            (Some(h), Some(c), Some(s), None) => (h, c, s),
            _ => return Err(AuthError::Malformed),
        };

    // Header: the algorithm must be exactly RS256. Pinning the alg is what makes
    // `alg: none` and algorithm-confusion (e.g. HS256 over the public key)
    // impossible — we never trust the token's choice of algorithm.
    let header = decode_segment(header_b64).ok_or(AuthError::Malformed)?;
    if header.get("alg").and_then(Value::as_str) != Some(keys::SIGNING_ALG) {
        return Err(AuthError::Malformed);
    }

    // Signature over the exact `header.claims` bytes.
    let signing_input = format!("{header_b64}.{claims_b64}");
    let signature = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|_| AuthError::Malformed)?;
    if !keys::verify_rs256(signing_input.as_bytes(), &signature) {
        return Err(AuthError::BadSignature);
    }

    let claims = Claims {
        raw: decode_segment(claims_b64).ok_or(AuthError::Malformed)?,
    };

    // Expiry — required for an `at+jwt` access token (RFC 9068 §2.2).
    match claims.expires_at() {
        Some(exp) if now < exp => {}
        Some(_) => return Err(AuthError::Expired),
        None => return Err(AuthError::Malformed),
    }

    // Not-before — optional for an `at+jwt` access token, but if present the token
    // MUST NOT be accepted before it (RFC 7519 §4.1.5): valid only when the clock
    // is at or after `nbf`.
    if let Some(nbf) = claims.not_before() {
        if now < nbf {
            return Err(AuthError::NotYetValid);
        }
    }

    // Audience — the token must be intended for this resource server.
    if !claims.audiences().iter().any(|a| *a == expected_audience) {
        return Err(AuthError::WrongAudience);
    }

    Ok(claims)
}

/// Current Unix time in seconds (server runtime clock; not on any hot loop).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// [`Claims`] is an axum extractor: a protected handler names it to require a
/// verified access token. The expected audience is the request's own base URL
/// (the resource server's identifier), matching what the token endpoint stamps.
#[axum::async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers).ok_or(AuthError::Missing)?;
        let audience = base_url(&parts.headers);
        verify_token(token, &audience, unix_now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUD: &str = "http://sim.local:8080";

    /// Mint a token signed with the bundled key, exactly as the token endpoint
    /// does (RS256 / `at+jwt`), so verification exercises the real signature path.
    fn mint(claims: Value) -> String {
        let header = json!({ "alg": "RS256", "typ": "at+jwt", "kid": keys::SIGNING_KID });
        let h = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).unwrap());
        let c = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let signing_input = format!("{h}.{c}");
        let sig = URL_SAFE_NO_PAD.encode(keys::sign_rs256(signing_input.as_bytes()));
        format!("{signing_input}.{sig}")
    }

    fn claims(exp: u64, aud: Value, scope: &str) -> Value {
        json!({
            "iss": AUD,
            "aud": aud,
            "sub": "client-1",
            "client_id": "client-1",
            "scope": scope,
            "iat": exp.saturating_sub(3600),
            "exp": exp,
        })
    }

    #[test]
    fn valid_token_verifies_and_exposes_claims() {
        let token = mint(claims(1_000, json!(AUD), "s1 s2"));
        let c = verify_token(&token, AUD, 500).expect("valid token verifies");
        assert_eq!(c.client_id(), Some("client-1"));
        assert_eq!(c.subject(), Some("client-1"));
        assert!(c.has_scope("s1"));
        assert!(c.has_scope("s2"));
        assert!(!c.has_scope("s3"));
    }

    #[test]
    fn audience_may_be_an_array_containing_this_server() {
        let token = mint(claims(1_000, json!(["other", AUD]), ""));
        assert!(verify_token(&token, AUD, 500).is_ok());
    }

    #[test]
    fn wrong_audience_is_rejected() {
        let token = mint(claims(1_000, json!("http://elsewhere"), ""));
        assert_eq!(verify_token(&token, AUD, 500), Err(AuthError::WrongAudience));
    }

    #[test]
    fn expired_token_is_rejected() {
        let token = mint(claims(1_000, json!(AUD), ""));
        assert_eq!(verify_token(&token, AUD, 1_000), Err(AuthError::Expired));
        assert_eq!(verify_token(&token, AUD, 2_000), Err(AuthError::Expired));
    }

    #[test]
    fn not_before_in_the_future_is_rejected() {
        // Token valid until exp=2000 but not before nbf=1000; at now=500 it is
        // correctly signed and unexpired yet must be rejected as not-yet-valid.
        let mut c = claims(2_000, json!(AUD), "s1");
        c["nbf"] = json!(1_000);
        let token = mint(c);
        assert_eq!(verify_token(&token, AUD, 500), Err(AuthError::NotYetValid));
        assert_eq!(verify_token(&token, AUD, 999), Err(AuthError::NotYetValid));
    }

    #[test]
    fn not_before_at_or_after_now_is_accepted() {
        let mut c = claims(2_000, json!(AUD), "s1");
        c["nbf"] = json!(1_000);
        let token = mint(c);
        // Valid exactly at nbf (the boundary is inclusive) and after it.
        assert!(verify_token(&token, AUD, 1_000).is_ok());
        assert!(verify_token(&token, AUD, 1_500).is_ok());
    }

    #[test]
    fn absent_nbf_is_accepted() {
        // The base `claims` helper sets no `nbf`; verification must not require it.
        let token = mint(claims(1_000, json!(AUD), "s1"));
        assert!(verify_token(&token, AUD, 500).is_ok());
    }

    #[test]
    fn missing_exp_is_malformed() {
        let token = mint(json!({ "aud": AUD, "scope": "" }));
        assert_eq!(verify_token(&token, AUD, 500), Err(AuthError::Malformed));
    }

    #[test]
    fn tampered_payload_fails_signature() {
        let token = mint(claims(1_000, json!(AUD), "s1"));
        // Swap the payload segment for a re-encoded, elevated-scope one while
        // keeping the original signature: verification must reject it.
        let mut parts: Vec<String> = token.split('.').map(String::from).collect();
        let forged = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&claims(1_000, json!(AUD), "admin")).unwrap(),
        );
        parts[1] = forged;
        let tampered = parts.join(".");
        assert_eq!(verify_token(&tampered, AUD, 500), Err(AuthError::BadSignature));
    }

    #[test]
    fn alg_none_is_rejected() {
        // A classic downgrade attack: unsigned token claiming `alg: none`.
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"at+jwt"}"#);
        let payload =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims(1_000, json!(AUD), "")).unwrap());
        let token = format!("{header}.{payload}.");
        assert_eq!(verify_token(&token, AUD, 500), Err(AuthError::Malformed));
    }

    #[test]
    fn structurally_invalid_tokens_are_malformed() {
        assert_eq!(verify_token("not-a-jwt", AUD, 500), Err(AuthError::Malformed));
        assert_eq!(verify_token("a.b", AUD, 500), Err(AuthError::Malformed));
        assert_eq!(verify_token("a.b.c.d", AUD, 500), Err(AuthError::Malformed));
        assert_eq!(verify_token("$.$.$", AUD, 500), Err(AuthError::Malformed));
    }

    #[test]
    fn require_scope_gates_authorisation() {
        let c = Claims {
            raw: claims(1_000, json!(AUD), "read write"),
        };
        assert!(c.require_scope("read").is_ok());
        assert_eq!(
            c.require_scope("delete"),
            Err(AuthError::InsufficientScope("delete".to_string()))
        );
    }

    #[test]
    fn bearer_token_parses_case_insensitively() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "bearer abc.def.ghi".parse().unwrap());
        assert_eq!(bearer_token(&headers), Some("abc.def.ghi"));

        headers.insert(header::AUTHORIZATION, "Basic xyz".parse().unwrap());
        assert_eq!(bearer_token(&headers), None);

        headers.insert(header::AUTHORIZATION, "Bearer   ".parse().unwrap());
        assert_eq!(bearer_token(&headers), None);
    }

    #[test]
    fn error_responses_carry_www_authenticate_challenge() {
        assert_eq!(AuthError::Missing.challenge(), "Bearer");
        assert!(AuthError::Expired.challenge().contains("invalid_token"));
        assert!(AuthError::NotYetValid.challenge().contains("invalid_token"));
        assert!(AuthError::NotYetValid.challenge().contains("not yet valid"));
        assert!(AuthError::InsufficientScope("s".into())
            .challenge()
            .contains(r#"scope="s""#));
    }
}
