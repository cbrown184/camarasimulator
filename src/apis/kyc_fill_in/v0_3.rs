//! KYC Fill-in **v0.3** (CAMARA KYC Fill-in 0.3.0).
//!
//! Endpoint:
//! - `POST /kyc-fill-in/v0.3/fill-in` — return the operator-verified identity
//!   attributes for the line, so a caller can pre-fill a sign-up / KYC form.
//!
//! ## What it does
//!
//! The caller optionally submits a `phoneNumber` (two-legged auth only) and the
//! operator answers with the identity attributes it holds for that line — name,
//! address, birthdate, e-mail, gender, etc. (`KYC_FillinResponse`). Unlike KYC
//! Match (which only *confirms* attributes the caller already has) this endpoint
//! *returns* them.
//!
//! The endpoint is protected: it requires a valid access token
//! ([`crate::auth::verify::Claims`]) carrying **at least one** KYC Fill-in scope
//! — either `kyc-fill-in:set-all` (all attributes) or one or more per-attribute
//! scopes `kyc-fill-in:<attribute>`. A token with none of these → `403
//! PERMISSION_DENIED`.
//!
//! ## Identifier resolution (two-legged vs three-legged)
//!
//! Faithful to CAMARA: the `phoneNumber` in the body is *only* valid in
//! two-legged auth. In a three-legged token the line is already identified by the
//! token **subject**, so resubmitting it is an error (mirrors Number Recycling /
//! Device Swap):
//!
//! - `phoneNumber` present **and** the subject is itself an E.164 number (a
//!   line-authenticated three-legged token) → `422 UNNECESSARY_IDENTIFIER`.
//! - `phoneNumber` present, subject not a line → the submitted number is the
//!   identifier (two-legged).
//! - `phoneNumber` absent, subject is an E.164 number → the subject is the
//!   identifier (three-legged).
//! - `phoneNumber` absent **and** the subject is not a line → the line cannot be
//!   identified → `422 MISSING_IDENTIFIER`.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! Two independent control planes drive the response:
//!
//! - **Reserved error suffix (identifier).** If the resolved identifier's
//!   trailing three digits name a reserved CAMARA status (`…400`, `…401`, `…403`,
//!   `…404`, `…409`, `…422`, `…429`, `…500`, `…503`), the endpoint answers with
//!   that canonical CAMARA error (shared [`crate::scenarios`]).
//! - **Persona (identifier digits).** Otherwise the identifier's trailing three
//!   digits select one of a fixed set of [`PERSONAS`] (`digits % PERSONAS.len()`),
//!   so the returned identity is deterministic from the number — `+1…003` and
//!   `+1…006` yield the same persona (`003 % 3 == 006 % 3`), `+1…001` a different
//!   one.
//! - **Granted scopes (response shape).** The response carries **only** the
//!   attributes the token is authorised for: `kyc-fill-in:set-all` returns every
//!   attribute; otherwise only those attributes whose per-attribute scope
//!   `kyc-fill-in:<attribute>` is granted. The granted scope set is thus a genuine
//!   second control plane over the response body — the same line yields a wider or
//!   narrower attribute set as the token's scopes change.

use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The scope granting **all** attributes (CAMARA KYC Fill-in 0.3.0).
const SET_ALL_SCOPE: &str = "kyc-fill-in:set-all";
/// Scope prefix for the per-attribute scopes `kyc-fill-in:<attribute>`.
const SCOPE_PREFIX: &str = "kyc-fill-in:";

/// A fixed identity the endpoint can return, selected by the identifier's
/// trailing three digits. Every field is non-empty so a granted per-attribute
/// scope always yields a value. Values are obviously synthetic (a simulator holds
/// no real personal data).
struct Persona {
    id_document: &'static str,
    name: &'static str,
    given_name: &'static str,
    family_name: &'static str,
    name_kana_hankaku: &'static str,
    name_kana_zenkaku: &'static str,
    middle_names: &'static str,
    family_name_at_birth: &'static str,
    address: &'static str,
    street_name: &'static str,
    street_number: &'static str,
    postal_code: &'static str,
    region: &'static str,
    locality: &'static str,
    country: &'static str,
    house_number_extension: &'static str,
    birthdate: &'static str,
    email: &'static str,
    gender: &'static str,
}

/// The fixed personas, indexed by `identifier_digits % PERSONAS.len()`. Kept
/// small so the case space stays enumerable (docs/DESIGN.md §7); the three cover
/// the three `gender` enum values.
const PERSONAS: &[Persona] = &[
    Persona {
        id_document: "GB-PP-100045",
        name: "Alice Wonderland",
        given_name: "Alice",
        family_name: "Wonderland",
        name_kana_hankaku: "ｱﾘｽ ﾜﾝﾀﾞｰﾗﾝﾄﾞ",
        name_kana_zenkaku: "アリス ワンダーランド",
        middle_names: "Marie",
        family_name_at_birth: "Liddell",
        address: "10 Rabbit Hole Lane, Oxford OX1 2JD, GB",
        street_name: "Rabbit Hole Lane",
        street_number: "10",
        postal_code: "OX1 2JD",
        region: "Oxfordshire",
        locality: "Oxford",
        country: "GB",
        house_number_extension: "A",
        birthdate: "1990-05-04",
        email: "alice.wonderland@example.com",
        gender: "FEMALE",
    },
    Persona {
        id_document: "US-DL-B420704",
        name: "Bob Builder",
        given_name: "Bob",
        family_name: "Builder",
        name_kana_hankaku: "ﾎﾞﾌﾞ ﾋﾞﾙﾀﾞｰ",
        name_kana_zenkaku: "ボブ ビルダー",
        middle_names: "Robert",
        family_name_at_birth: "Builder",
        address: "42 Construction Way, Springfield 62704, US",
        street_name: "Construction Way",
        street_number: "42",
        postal_code: "62704",
        region: "Illinois",
        locality: "Springfield",
        country: "US",
        house_number_extension: "B",
        birthdate: "1985-11-19",
        email: "bob.builder@example.com",
        gender: "MALE",
    },
    Persona {
        id_document: "JP-MN-100078",
        name: "Kenji Tanaka",
        given_name: "Kenji",
        family_name: "Tanaka",
        name_kana_hankaku: "ﾀﾅｶ ｹﾝｼﾞ",
        name_kana_zenkaku: "タナカ ケンジ",
        middle_names: "Hiroshi",
        family_name_at_birth: "Tanaka",
        address: "3-2-1 Chiyoda, Tokyo 100-0001, JP",
        street_name: "Chiyoda",
        street_number: "3-2-1",
        postal_code: "100-0001",
        region: "Tokyo",
        locality: "Chiyoda",
        country: "JP",
        house_number_extension: "C",
        birthdate: "1978-03-27",
        email: "kenji.tanaka@example.com",
        gender: "OTHER",
    },
];

/// Routes for KYC Fill-in v0.3, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new().route("/kyc-fill-in/v0.3/fill-in", post(fill_in))
}

/// `POST /fill-in` request body (CAMARA `KYC_FillinRequest`): an optional
/// `phoneNumber` (valid only in two-legged auth). Unknown fields are rejected.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FillinRequest {
    #[serde(rename = "phoneNumber")]
    phone_number: Option<String>,
}

/// `POST /kyc-fill-in/v0.3/fill-in`.
async fn fill_in(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry at least one KYC Fill-in scope
    // (`set-all` or a per-attribute scope), else 403 PERMISSION_DENIED.
    if !is_authorised(&claims) {
        return with_correlator(
            CamaraError::permission_denied(
                "Access token does not carry any 'kyc-fill-in:*' scope.",
            )
            .into_response(),
            &correlator,
        );
    }

    // An empty body is allowed (phoneNumber optional on a three-legged token).
    let req: FillinRequest = if body.is_empty() {
        FillinRequest { phone_number: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => {
                return invalid_argument(
                    "Request body is not a valid KYC_FillinRequest.",
                    &correlator,
                )
            }
        }
    };

    // Resolve the line identifier, honouring the two-legged / three-legged rule.
    let identifier = match resolve_identifier(req.phone_number.as_deref(), &claims, &correlator) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    // The identifier is the control plane (docs/DESIGN.md §7): a reserved suffix
    // selects a canonical CAMARA error.
    if let Some(err) = scenarios::reserved_error(&identifier) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Build the response filtered to the granted scopes (the second control plane).
    let attributes = fill_in_response(&identifier, &claims);

    with_correlator(
        (StatusCode::OK, Json(Value::Object(attributes))).into_response(),
        &correlator,
    )
}

/// Whether the token carries any KYC Fill-in scope (`set-all` or a per-attribute
/// scope). Authorisation is "any relevant scope"; *which* attributes come back is
/// then decided per-attribute in [`fill_in_response`].
fn is_authorised(claims: &Claims) -> bool {
    claims
        .scope()
        .split(' ')
        .any(|s| s == SET_ALL_SCOPE || s.starts_with(SCOPE_PREFIX))
}

/// The persona for `identifier`, selected by its trailing three digits
/// (`digits % PERSONAS.len()`). An identifier with no trailing digits (never the
/// case for a resolved E.164 line, but handled defensively) uses persona `0`.
fn persona_for(identifier: &str) -> &'static Persona {
    let idx = scenarios::trailing_three_digits(identifier)
        .map(|d| d as usize % PERSONAS.len())
        .unwrap_or(0);
    &PERSONAS[idx]
}

/// Build the `KYC_FillinResponse` for `identifier`, including only the attributes
/// the token is authorised for. `kyc-fill-in:set-all` includes every attribute;
/// otherwise an attribute is included iff its `kyc-fill-in:<attribute>` scope is
/// granted (docs/DESIGN.md §7 — granted scopes are a control plane).
fn fill_in_response(identifier: &str, claims: &Claims) -> Map<String, Value> {
    let set_all = claims.has_scope(SET_ALL_SCOPE);
    let p = persona_for(identifier);

    // (attribute name, value). `phoneNumber` echoes the resolved identifier; the
    // rest come from the selected persona. Order matches the CAMARA schema.
    let fields: [(&str, &str); 20] = [
        ("phoneNumber", identifier),
        ("idDocument", p.id_document),
        ("name", p.name),
        ("givenName", p.given_name),
        ("familyName", p.family_name),
        ("nameKanaHankaku", p.name_kana_hankaku),
        ("nameKanaZenkaku", p.name_kana_zenkaku),
        ("middleNames", p.middle_names),
        ("familyNameAtBirth", p.family_name_at_birth),
        ("address", p.address),
        ("streetName", p.street_name),
        ("streetNumber", p.street_number),
        ("postalCode", p.postal_code),
        ("region", p.region),
        ("locality", p.locality),
        ("country", p.country),
        ("houseNumberExtension", p.house_number_extension),
        ("birthdate", p.birthdate),
        ("email", p.email),
        ("gender", p.gender),
    ];

    let mut out = Map::new();
    for (attr, value) in fields {
        if set_all || claims.has_scope(&format!("{SCOPE_PREFIX}{attr}")) {
            out.insert(attr.to_string(), Value::String(value.to_string()));
        }
    }
    out
}

/// Resolve the line identifier from the request body and the token subject,
/// enforcing the CAMARA two-legged / three-legged identifier rules (mirrors
/// Device Swap / Number Recycling). See the module docs for the four cases.
fn resolve_identifier(
    phone_number: Option<&str>,
    claims: &Claims,
    correlator: &Option<HeaderValue>,
) -> Result<String, Response> {
    let subject = claims.subject().unwrap_or("");
    let subject_is_line = is_valid_e164(subject);

    match phone_number {
        Some(phone) => {
            if !is_valid_e164(phone) {
                return Err(invalid_argument(
                    "`phoneNumber` must be in E.164 format (e.g. +123456789).",
                    correlator,
                ));
            }
            // The line is already identified by a three-legged token; the number
            // must not be resubmitted.
            if subject_is_line {
                return Err(unprocessable(
                    "UNNECESSARY_IDENTIFIER",
                    "The phone number is already identified by the access token.",
                    correlator,
                ));
            }
            Ok(phone.to_string())
        }
        None => {
            if subject_is_line {
                Ok(subject.to_string())
            } else {
                Err(unprocessable(
                    "MISSING_IDENTIFIER",
                    "The phone number cannot be identified: supply `phoneNumber` or use a token that identifies a line.",
                    correlator,
                ))
            }
        }
    }
}

/// A 422 CAMARA error with a caller-chosen `code`, correlator echoed.
fn unprocessable(code: &str, message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::new(StatusCode::UNPROCESSABLE_ENTITY, code, message).into_response(),
        correlator,
    )
}

/// A 400 `INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(CamaraError::invalid_argument(message).into_response(), correlator)
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "kfi.local:8080";

    // --- Pure units --------------------------------------------------------

    #[test]
    fn persona_selection_is_deterministic_from_the_trailing_digits() {
        // digits % 3 selects the persona: 003 and 006 collide, 001 differs.
        assert!(std::ptr::eq(persona_for("+123456789003"), persona_for("+123456789006")));
        assert!(!std::ptr::eq(persona_for("+123456789003"), persona_for("+123456789001")));
        // …000 → persona 0.
        assert_eq!(persona_for("+123456789000").gender, "FEMALE");
        assert_eq!(persona_for("+123456789001").gender, "MALE");
        assert_eq!(persona_for("+123456789002").gender, "OTHER");
    }

    #[test]
    fn every_persona_field_is_non_empty() {
        // A granted per-attribute scope must always yield a value.
        for p in PERSONAS {
            for v in [
                p.id_document, p.name, p.given_name, p.family_name, p.name_kana_hankaku,
                p.name_kana_zenkaku, p.middle_names, p.family_name_at_birth, p.address,
                p.street_name, p.street_number, p.postal_code, p.region, p.locality,
                p.country, p.house_number_extension, p.birthdate, p.email, p.gender,
            ] {
                assert!(!v.is_empty());
            }
        }
    }

    #[test]
    fn e164_validation_follows_the_camara_pattern() {
        assert!(is_valid_e164("+12345"));
        assert!(is_valid_e164("+123456789012"));
        assert!(!is_valid_e164("123456789")); // no +
        assert!(!is_valid_e164("+0234567")); // leading zero
        assert!(!is_valid_e164("+1234")); // too short
        assert!(!is_valid_e164("kfi-client")); // client-credentials subject
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    async fn mint_token(scope: &str) -> String {
        mint_token_with_client(scope, "kfi-client").await
    }

    /// Mint an access token via `client_credentials` with a caller-chosen
    /// `client_id` (which becomes the token `sub`). `+` is percent-encoded so an
    /// E.164 client id survives the urlencoded body.
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

    /// POST to `/fill-in` with an optional Bearer token and `x-correlator`.
    async fn post_fill_in(
        token: Option<&str>,
        body: &str,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/kyc-fill-in/v0.3/fill-in")
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

    /// Mint a `set-all` (two-legged, non-line subject) token and call the endpoint.
    async fn fill_in_ok(body: &str) -> (StatusCode, HeaderMap, Value) {
        let token = mint_token(SET_ALL_SCOPE).await;
        post_fill_in(Some(&token), body, None).await
    }

    // --- Happy path: set-all returns every attribute -----------------------

    #[tokio::test]
    async fn set_all_returns_every_attribute() {
        let (status, _, body) = fill_in_ok(r#"{"phoneNumber":"+123456789000"}"#).await;
        assert_eq!(status, StatusCode::OK);
        // Persona 0 (FEMALE) with the echoed phone number.
        assert_eq!(body["phoneNumber"], "+123456789000");
        assert_eq!(body["givenName"], "Alice");
        assert_eq!(body["familyName"], "Wonderland");
        assert_eq!(body["country"], "GB");
        assert_eq!(body["gender"], "FEMALE");
        assert_eq!(body["email"], "alice.wonderland@example.com");
        // All 20 attributes present.
        assert_eq!(body.as_object().unwrap().len(), 20);
    }

    #[tokio::test]
    async fn persona_is_selected_by_the_trailing_digits() {
        let (_, _, p0) = fill_in_ok(r#"{"phoneNumber":"+123456789000"}"#).await;
        let (_, _, p1) = fill_in_ok(r#"{"phoneNumber":"+123456789001"}"#).await;
        let (_, _, p2) = fill_in_ok(r#"{"phoneNumber":"+123456789002"}"#).await;
        assert_eq!(p0["gender"], "FEMALE");
        assert_eq!(p1["gender"], "MALE");
        assert_eq!(p2["gender"], "OTHER");
        // …006 collides with …000 (mod 3) → same persona, echoed number differs.
        let (_, _, p6) = fill_in_ok(r#"{"phoneNumber":"+123456789006"}"#).await;
        assert_eq!(p6["givenName"], p0["givenName"]);
        assert_eq!(p6["phoneNumber"], "+123456789006");
    }

    // --- Scope-driven field filtering (second control plane) ----------------

    #[tokio::test]
    async fn per_attribute_scopes_narrow_the_response() {
        // Only name + email scopes → only those two attributes come back.
        let token = mint_token("kyc-fill-in:name kyc-fill-in:email").await;
        let (status, _, body) =
            post_fill_in(Some(&token), r#"{"phoneNumber":"+123456789000"}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(body["name"], "Alice Wonderland");
        assert_eq!(body["email"], "alice.wonderland@example.com");
        assert!(body.get("givenName").is_none());
        assert!(body.get("phoneNumber").is_none());
    }

    #[tokio::test]
    async fn phone_number_attribute_needs_its_own_scope() {
        // A single per-attribute scope for phoneNumber echoes only the number.
        let token = mint_token("kyc-fill-in:phoneNumber").await;
        let (status, _, body) =
            post_fill_in(Some(&token), r#"{"phoneNumber":"+123456789001"}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_object().unwrap().len(), 1);
        assert_eq!(body["phoneNumber"], "+123456789001");
    }

    #[tokio::test]
    async fn set_all_wins_over_the_absence_of_per_attribute_scopes() {
        // set-all alone still returns everything (no per-attribute scopes needed).
        let (_, _, body) = fill_in_ok(r#"{"phoneNumber":"+123456789001"}"#).await;
        assert_eq!(body.as_object().unwrap().len(), 20);
    }

    // --- Reserved-error convention -----------------------------------------

    #[tokio::test]
    async fn reserved_suffix_selects_a_canonical_camara_error() {
        let (status, _, body) = fill_in_ok(r#"{"phoneNumber":"+123456789404"}"#).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");

        let (status, _, body) = fill_in_ok(r#"{"phoneNumber":"+123456789429"}"#).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    // --- Identifier resolution (two-legged / three-legged) -----------------

    #[tokio::test]
    async fn three_legged_keys_off_the_subject() {
        // No phoneNumber; the subject is an E.164 line → persona from its digits,
        // and phoneNumber echoes the subject.
        let token = mint_token_with_client(SET_ALL_SCOPE, "+123456789002").await;
        let (status, _, body) = post_fill_in(Some(&token), r#"{}"#, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phoneNumber"], "+123456789002");
        assert_eq!(body["gender"], "OTHER");
    }

    #[tokio::test]
    async fn three_legged_reserved_subject_selects_error() {
        let token = mint_token_with_client(SET_ALL_SCOPE, "+123456789503").await;
        let (status, _, body) = post_fill_in(Some(&token), r#"{}"#, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "UNAVAILABLE");
    }

    #[tokio::test]
    async fn resubmitting_the_number_on_a_line_token_is_unnecessary() {
        let token = mint_token_with_client(SET_ALL_SCOPE, "+123456789002").await;
        let (status, _, body) =
            post_fill_in(Some(&token), r#"{"phoneNumber":"+123456789002"}"#, None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "UNNECESSARY_IDENTIFIER");
    }

    #[tokio::test]
    async fn no_number_and_non_line_subject_is_missing_identifier() {
        let (status, _, body) = fill_in_ok(r#"{}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "MISSING_IDENTIFIER");
    }

    #[tokio::test]
    async fn empty_body_falls_back_to_the_token_subject() {
        let token = mint_token_with_client(SET_ALL_SCOPE, "+123456789000").await;
        let (status, _, body) = post_fill_in(Some(&token), "", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phoneNumber"], "+123456789000");
    }

    // --- Validation & auth -------------------------------------------------

    #[tokio::test]
    async fn invalid_phone_format_is_rejected() {
        let (status, _, body) = fill_in_ok(r#"{"phoneNumber":"0123"}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn unknown_field_is_rejected() {
        let (status, _, body) = fill_in_ok(r#"{"phoneNumber":"+123456789000","x":1}"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn malformed_json_body_is_rejected() {
        let (status, _, body) = fill_in_ok("not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn token_without_any_fill_in_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) =
            post_fill_in(Some(&token), r#"{"phoneNumber":"+123456789000"}"#, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) =
            post_fill_in(None, r#"{"phoneNumber":"+123456789000"}"#, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success_and_error() {
        let token = mint_token(SET_ALL_SCOPE).await;
        // Success (two-legged).
        let (status, headers, _) = post_fill_in(
            Some(&token),
            r#"{"phoneNumber":"+123456789000"}"#,
            Some("corr-kfi"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-kfi")
        );
        // Business error.
        let (status, headers, _) = post_fill_in(Some(&token), r#"{}"#, Some("corr-err")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-err")
        );
    }
}
