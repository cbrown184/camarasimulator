//! `POST /oauth2/token` — the OAuth2 token endpoint.
//!
//! This endpoint implements two of the three CAMARA grants (docs/DESIGN.md §6):
//!
//! - **`client_credentials`** (RFC 6749 §4.4): a two-legged flow for APIs that
//!   need no end-user context.
//! - **`authorization_code` + PKCE** (RFC 6749 §4.1.3, RFC 7636): the three-legged
//!   flow, redeeming a code minted by `GET /oauth2/authorize` ([`super::authorize`]).
//!
//! - **CIBA** (`urn:openid:params:grant-type:ciba`): the backchannel flow. The
//!   client starts it at `POST /bc-authorize` ([`super::ciba`]) and then **polls**
//!   this endpoint with the returned `auth_req_id` until the end user authorizes.
//!
//! The endpoint issues a **real, signed** RS256 JWT (RFC 9068 `at+jwt`) using
//! the bundled JWKS key, so a client that fetches `/oauth2/jwks` can verify it.
//! The token-verification middleware ([`super::verify`]) validates these on
//! protected routes (signature / audience / expiry / scope).
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

use super::{base_url, ciba, codes, keys, purpose};

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
    // `authorization_code` grant (RFC 6749 §4.1.3 + PKCE RFC 7636):
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    // CIBA grant (OpenID CIBA Core §10.1): the id from `POST /bc-authorize`.
    auth_req_id: Option<String>,
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
        Some("authorization_code") => authorization_code(&headers, form),
        Some("urn:openid:params:grant-type:ciba") => ciba_grant(&headers, form),
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
    // A requested `dpv:` purpose scope must be well-formed (docs/DESIGN.md §7).
    if let Err(bad) = purpose::validate_scope(&scope) {
        return invalid_scope(&bad);
    }
    let issuer = base_url(headers);
    // Two-legged: the client is the subject, and iss == aud == this server.
    issue_access_token(&issuer, &issuer, &client_id, &client_id, &scope)
}

/// Issue a token for the **`authorization_code`** grant (RFC 6749 §4.1.3 + PKCE
/// RFC 7636). Redeems a code minted by `GET /oauth2/authorize` ([`super::authorize`]).
///
/// Validation, all yielding `invalid_grant` (HTTP 400) on failure so a client
/// cannot distinguish *why* a code was rejected:
///   - the code exists and is unexpired (it is **consumed** on lookup — single use);
///   - `redirect_uri` matches the one bound at authorization;
///   - `client_id` matches the client the code was issued to;
///   - the PKCE `code_verifier` satisfies the stored S256 `code_challenge`.
///
/// The issued token's `aud` is the audience captured at authorization (the resource
/// server the user consented against); its `sub` is the simulator's synthetic
/// resource owner (there is no real end-user login — see [`super::authorize`]).
fn authorization_code(headers: &HeaderMap, form: TokenForm) -> Response {
    // Required parameters. Each field is moved out of `form` independently.
    let code = match form.code.filter(|c| !c.is_empty()) {
        Some(c) => c,
        None => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the 'code' parameter is required",
            )
        }
    };
    // Public client + PKCE: the client still identifies itself (Basic or form).
    let client_id = match client_id_from_basic(headers).or(form.client_id) {
        Some(id) => id,
        None => {
            return oauth_error(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "client authentication required (client_id or client_secret_basic)",
            )
        }
    };
    let code_verifier = match form.code_verifier.filter(|v| !v.is_empty()) {
        Some(v) => v,
        None => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the 'code_verifier' parameter is required (PKCE)",
            )
        }
    };
    let redirect_uri = match form.redirect_uri.filter(|u| !u.is_empty()) {
        Some(u) => u,
        None => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the 'redirect_uri' parameter is required",
            )
        }
    };

    // Consume the code (single use): a replay finds nothing and fails below.
    let entry = match codes::redeem(&code) {
        Some(e) => e,
        None => return invalid_grant("the authorization code is invalid or already used"),
    };
    if codes::unix_now() >= entry.expires_at {
        return invalid_grant("the authorization code has expired");
    }
    if redirect_uri != entry.redirect_uri {
        return invalid_grant("redirect_uri does not match the authorization request");
    }
    if client_id != entry.client_id {
        return invalid_grant("client_id does not match the authorization request");
    }
    if !codes::pkce_s256_matches(&code_verifier, &entry.code_challenge) {
        return invalid_grant("PKCE code_verifier does not match the code_challenge");
    }

    // iss is this token endpoint; aud is the audience the user authorized against.
    let issuer = base_url(headers);
    issue_access_token(&issuer, &entry.audience, &client_id, "camarasim-user", &entry.scope)
}

/// Issue a token for the **CIBA** grant (`urn:openid:params:grant-type:ciba`,
/// OpenID CIBA Core §10.1). The client polls this endpoint with the `auth_req_id`
/// returned by `POST /bc-authorize` ([`super::ciba`]) until the end user authorizes.
///
/// The authenticated `client_id` must match the client the `auth_req_id` was
/// issued to. The poll outcome ([`ciba::poll`]) maps to the CIBA token-error set
/// (OpenID CIBA Core §11), all HTTP 400:
///   - not yet authorized → `authorization_pending`;
///   - end user rejected → `access_denied`;
///   - request expired → `expired_token`;
///   - unknown id, or id issued to a different client → `invalid_grant`.
///
/// On approval the token's `aud` is the audience captured at `/bc-authorize` and
/// its `sub` is the simulator's synthetic resource owner (`camarasim-user`); the
/// `auth_req_id` is consumed (single use).
fn ciba_grant(headers: &HeaderMap, form: TokenForm) -> Response {
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
    let auth_req_id = match form.auth_req_id.filter(|a| !a.is_empty()) {
        Some(a) => a,
        None => {
            return oauth_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the 'auth_req_id' parameter is required",
            )
        }
    };

    match ciba::poll(&auth_req_id, &client_id) {
        ciba::Poll::Approved { scope, audience } => {
            let issuer = base_url(headers);
            issue_access_token(&issuer, &audience, &client_id, "camarasim-user", &scope)
        }
        ciba::Poll::Pending => oauth_error(
            StatusCode::BAD_REQUEST,
            "authorization_pending",
            "the end-user authorization is pending; poll again after the interval",
        ),
        ciba::Poll::Denied => oauth_error(
            StatusCode::BAD_REQUEST,
            "access_denied",
            "the end user denied the authorization request",
        ),
        ciba::Poll::Expired => oauth_error(
            StatusCode::BAD_REQUEST,
            "expired_token",
            "the auth_req_id has expired; start a new backchannel request",
        ),
        ciba::Poll::Unknown | ciba::Poll::WrongClient => {
            invalid_grant("the auth_req_id is invalid or was not issued to this client")
        }
    }
}

/// An `invalid_grant` token-endpoint error (RFC 6749 §5.2), HTTP 400.
fn invalid_grant(description: &str) -> Response {
    oauth_error(StatusCode::BAD_REQUEST, "invalid_grant", description)
}

/// An `invalid_scope` error (RFC 6749 §5.2) naming the malformed purpose scope
/// (`bad`), HTTP 400. `pub(super)` so `/bc-authorize` ([`super::ciba`]) can raise
/// the same error for a malformed requested scope.
pub(super) fn invalid_scope(bad: &str) -> Response {
    oauth_error(
        StatusCode::BAD_REQUEST,
        "invalid_scope",
        &format!("the requested scope '{bad}' is not a valid CAMARA purpose scope"),
    )
}

/// Build and return a signed access-token response. Shared by every grant.
///
/// Stamps `iss`/`aud` (kept distinct so a grant can target a resource server other
/// than the token endpoint's own issuer), `sub`, `client_id`, the granted `scope`,
/// `iat`, `exp` (`iat` + [`EXPIRES_IN`]), and a unique `jti`. `scope` is echoed in
/// the response body only when non-empty.
fn issue_access_token(iss: &str, aud: &str, client_id: &str, subject: &str, scope: &str) -> Response {
    let now = unix_now();
    let claims = json!({
        "iss": iss,
        "sub": subject,
        "aud": aud,
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
        body["scope"] = Value::String(scope.to_string());
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
///
/// `pub(super)` so the CIBA endpoints ([`super::ciba`]) authenticate clients the
/// same way as the token grants.
pub(super) fn client_id_from_basic(headers: &HeaderMap) -> Option<String> {
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

/// Token-endpoint responses must not be cached (RFC 6749 §5.1). `pub(super)` so
/// the CIBA `/bc-authorize` response ([`super::ciba`]) carries the same header.
pub(super) fn no_store() -> [(header::HeaderName, &'static str); 2] {
    [
        (header::CACHE_CONTROL, "no-store"),
        (header::PRAGMA, "no-cache"),
    ]
}

/// An OAuth2 token-endpoint error (RFC 6749 §5.2). `pub(super)` so the CIBA
/// `/bc-authorize` endpoint ([`super::ciba`]) returns the same error shape.
pub(super) fn oauth_error(status: StatusCode, code: &str, description: &str) -> Response {
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
    async fn client_credentials_rejects_a_malformed_purpose_scope() {
        // `dpv:Foo` has no `#action`, so it is not a valid CAMARA purpose scope.
        let (status, headers, body) = post_token(
            "grant_type=client_credentials&client_id=client-1&scope=dpv%3AFoo",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_scope");
        assert!(body["error_description"]
            .as_str()
            .unwrap()
            .contains("dpv:Foo"));
        // Error responses must not be cached either.
        assert_eq!(
            headers.get("cache-control").unwrap().to_str().unwrap(),
            "no-store"
        );
    }

    #[tokio::test]
    async fn client_credentials_accepts_a_well_formed_purpose_scope() {
        let (status, _, body) = post_token(
            "grant_type=client_credentials&client_id=client-1\
             &scope=dpv%3AFraudPreventionAndDetection%23check-sim-swap",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["scope"], "dpv:FraudPreventionAndDetection#check-sim-swap");
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
    async fn ciba_grant_without_auth_req_id_is_invalid_request() {
        // CIBA is implemented; the poll still requires an auth_req_id.
        let (status, _, body) = post_token(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=client-1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_request");
    }

    #[tokio::test]
    async fn ciba_grant_without_client_is_invalid_client() {
        let (status, _, body) = post_token(
            "grant_type=urn:openid:params:grant-type:ciba&auth_req_id=whatever",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "invalid_client");
    }

    #[tokio::test]
    async fn ciba_grant_with_unknown_auth_req_id_is_invalid_grant() {
        let (status, _, body) = post_token(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=client-1&auth_req_id=never-issued",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_grant");
    }

    // --- CIBA grant end to end (POST /bc-authorize -> poll POST /oauth2/token) ---
    //
    // These drive the full backchannel flow through one router: start at
    // /bc-authorize to mint an auth_req_id, then poll the token endpoint, so the
    // shared in-memory request store is exercised across both requests.

    /// Run the backchannel leg for `login_hint`/`scope` and return the auth_req_id.
    async fn bc_authorize(login_hint: &str, scope: &str) -> Value {
        let hint_enc = serde_urlencoded::to_string([("login_hint", login_hint)]).unwrap();
        let body = format!("client_id=app-1&scope={scope}&{hint_enc}");
        let response = super::super::routes()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/bc-authorize")
                    .header("host", "sim.local:8080")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn ciba_poll_issues_a_token_for_an_approved_request() {
        let bc = bc_authorize("tel:+34600000001", "openid").await;
        let auth_req_id = bc["auth_req_id"].as_str().unwrap();
        assert_eq!(bc["expires_in"], ciba::EXPIRES_IN);
        assert_eq!(bc["interval"], ciba::INTERVAL);

        let body = format!(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=app-1&auth_req_id={auth_req_id}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["token_type"], "Bearer");
        assert_eq!(resp["scope"], "openid");
        let (_, claims) = decode_jwt(resp["access_token"].as_str().unwrap());
        // Backchannel is a three-legged flow: sub is the sim end user, not the client.
        assert_eq!(claims["sub"], "camarasim-user");
        assert_eq!(claims["client_id"], "app-1");
        assert_eq!(claims["aud"], "http://sim.local:8080");
        assert_eq!(claims["scope"], "openid");
    }

    #[tokio::test]
    async fn ciba_auth_req_id_is_single_use() {
        let bc = bc_authorize("tel:+34600000001", "openid").await;
        let auth_req_id = bc["auth_req_id"].as_str().unwrap();
        let body = format!(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=app-1&auth_req_id={auth_req_id}"
        );
        let (first, _, _) = post_token(&body, None).await;
        assert_eq!(first, StatusCode::OK);
        // A second poll after the token was issued finds nothing.
        let (second, _, resp) = post_token(&body, None).await;
        assert_eq!(second, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn ciba_pending_login_hint_reports_authorization_pending() {
        let bc = bc_authorize("tel:+34600000000-pending", "openid").await;
        let auth_req_id = bc["auth_req_id"].as_str().unwrap();
        let body = format!(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=app-1&auth_req_id={auth_req_id}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "authorization_pending");
        // Pending is not consumed: a second poll still reports pending.
        let (again, _, resp2) = post_token(&body, None).await;
        assert_eq!(again, StatusCode::BAD_REQUEST);
        assert_eq!(resp2["error"], "authorization_pending");
    }

    #[tokio::test]
    async fn ciba_denied_login_hint_reports_access_denied() {
        let bc = bc_authorize("tel:+34600000000-denied", "openid").await;
        let auth_req_id = bc["auth_req_id"].as_str().unwrap();
        let body = format!(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=app-1&auth_req_id={auth_req_id}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "access_denied");
    }

    #[tokio::test]
    async fn ciba_poll_rejects_a_mismatched_client() {
        let bc = bc_authorize("tel:+34600000001", "openid").await;
        let auth_req_id = bc["auth_req_id"].as_str().unwrap();
        // A different client polling the same auth_req_id must not get a token.
        let body = format!(
            "grant_type=urn:openid:params:grant-type:ciba&client_id=other-app&auth_req_id={auth_req_id}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn bc_authorize_requires_a_login_hint() {
        let response = super::super::routes()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/bc-authorize")
                    .header("host", "sim.local:8080")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("client_id=app-1&scope=openid"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(resp["error"], "invalid_request");
    }

    #[tokio::test]
    async fn bc_authorize_rejects_a_malformed_purpose_scope() {
        let response = super::super::routes()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/bc-authorize")
                    .header("host", "sim.local:8080")
                    .header("content-type", "application/x-www-form-urlencoded")
                    // `dpv:Foo` lacks the `#action`, so it is an invalid purpose scope.
                    .body(Body::from(
                        "client_id=app-1&scope=dpv:Foo&login_hint=tel:%2B34600000001",
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(resp["error"], "invalid_scope");
        assert!(resp["error_description"]
            .as_str()
            .unwrap()
            .contains("dpv:Foo"));
    }

    #[tokio::test]
    async fn bc_authorize_requires_client_authentication() {
        let response = super::super::routes()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/bc-authorize")
                    .header("host", "sim.local:8080")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("scope=openid&login_hint=tel:%2B34600000001"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(resp["error"], "invalid_client");
    }

    // --- authorization_code grant (redeeming a code from /oauth2/authorize) ---
    //
    // These tests drive the full three-legged flow through a single router: mint a
    // code at GET /oauth2/authorize, then redeem it at POST /oauth2/token, so the
    // shared in-memory code store and PKCE binding are exercised end to end.

    use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
    use sha2::{Digest, Sha256};

    /// A fixed PKCE pair (RFC 7636 Appendix B): verifier and its S256 challenge.
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    const REDIRECT: &str = "https://app.example/cb";

    /// Run the authorize leg and return the issued authorization code.
    async fn mint_code(scope: &str) -> String {
        let redirect_enc =
            serde_urlencoded::to_string([("redirect_uri", REDIRECT)]).unwrap();
        let query = format!(
            "response_type=code&client_id=app-1&{redirect_enc}&scope={scope}\
             &code_challenge={CHALLENGE}&code_challenge_method=S256"
        );
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
        let location = response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let q = location.split_once('?').unwrap().1;
        let pairs: Vec<(String, String)> = serde_urlencoded::from_str(q).unwrap();
        pairs.into_iter().find(|(k, _)| k == "code").unwrap().1
    }

    #[tokio::test]
    async fn authorization_code_redeems_for_a_token_carrying_the_authorized_scope() {
        let code = mint_code("openid").await;
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=app-1\
             &redirect_uri={REDIRECT}&code_verifier={VERIFIER}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["token_type"], "Bearer");
        assert_eq!(resp["scope"], "openid");
        let (_, claims) = decode_jwt(resp["access_token"].as_str().unwrap());
        // aud is the resource server authorized against; sub is the sim end user.
        assert_eq!(claims["aud"], "http://sim.local:8080");
        assert_eq!(claims["client_id"], "app-1");
        assert_eq!(claims["sub"], "camarasim-user");
        assert_eq!(claims["scope"], "openid");
    }

    #[tokio::test]
    async fn authorization_code_is_single_use() {
        let code = mint_code("openid").await;
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=app-1\
             &redirect_uri={REDIRECT}&code_verifier={VERIFIER}"
        );
        let (first, _, _) = post_token(&body, None).await;
        assert_eq!(first, StatusCode::OK);
        // Replaying the same code must be rejected as invalid_grant.
        let (second, _, resp) = post_token(&body, None).await;
        assert_eq!(second, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn authorization_code_rejects_a_wrong_pkce_verifier() {
        let code = mint_code("openid").await;
        let wrong = "wrong-verifier-that-does-not-hash-to-the-challenge-value";
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=app-1\
             &redirect_uri={REDIRECT}&code_verifier={wrong}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn authorization_code_rejects_a_mismatched_redirect_uri() {
        let code = mint_code("openid").await;
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=app-1\
             &redirect_uri=https://evil.example/cb&code_verifier={VERIFIER}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn authorization_code_rejects_a_mismatched_client_id() {
        let code = mint_code("openid").await;
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=other-app\
             &redirect_uri={REDIRECT}&code_verifier={VERIFIER}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn authorization_code_rejects_an_unknown_code() {
        let body = format!(
            "grant_type=authorization_code&code=never-issued&client_id=app-1\
             &redirect_uri={REDIRECT}&code_verifier={VERIFIER}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_grant");
    }

    #[tokio::test]
    async fn authorization_code_requires_the_code_verifier() {
        let code = mint_code("openid").await;
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=app-1&redirect_uri={REDIRECT}"
        );
        let (status, _, resp) = post_token(&body, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(resp["error"], "invalid_request");
    }

    #[tokio::test]
    async fn authorization_code_token_verifies_against_the_jwks_key() {
        use rsa::pkcs1v15::{Signature, VerifyingKey};
        use rsa::signature::Verifier;
        use rsa::RsaPublicKey;

        // Sanity: the PKCE constants are a real S256 pair.
        assert_eq!(B64.encode(Sha256::digest(VERIFIER.as_bytes())), CHALLENGE);

        let code = mint_code("openid").await;
        let body = format!(
            "grant_type=authorization_code&code={code}&client_id=app-1\
             &redirect_uri={REDIRECT}&code_verifier={VERIFIER}"
        );
        let (_, _, resp) = post_token(&body, None).await;
        let token = resp["access_token"].as_str().unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature_bytes = B64.decode(parts[2]).unwrap();

        let verifying_key = VerifyingKey::<Sha256>::new(RsaPublicKey::from(keys::signing_key()));
        let signature = Signature::try_from(signature_bytes.as_slice()).unwrap();
        verifying_key
            .verify(signing_input.as_bytes(), &signature)
            .expect("authorization_code token verifies under the JWKS public key");
    }
}
