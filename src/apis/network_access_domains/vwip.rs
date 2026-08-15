//! Network Access Domains **vwip** (CAMARA NetworkAccessManagement / Network
//! Access Domains, wip).
//!
//! Endpoints so far:
//! - `GET /network-access-domains/vwip/trust-domains/capabilities` — the
//!   provider-level Trust Domain capabilities document (operationId
//!   `getTrustDomainCapabilities`).
//! - `GET /network-access-domains/vwip/services` — the caller's **Services**
//!   catalog (operationId `getServices`): the logical commercial subscriptions /
//!   account relationships the authenticated identity holds.
//! - `GET /network-access-domains/vwip/services/{serviceId}` — a single
//!   **Service** from that catalog by id (operationId `getService`).
//! - `POST /network-access-domains/vwip/trust-domains` — the first **stateful**
//!   leg: create a Trust Domain (operationId `createTrustDomain`), persisted in
//!   the in-memory [`store`].
//!
//! ## What they do
//!
//! `getTrustDomainCapabilities` returns the set of Trust Domain configuration
//! capabilities this API provider supports: which access types (Wi-Fi
//! WPA-Personal/Enterprise, Thread) and their properties, plus the policy limits
//! (max devices, per-domain up/downstream bandwidth bands, egress allow-list
//! constraints) a caller may configure when creating a Trust Domain. It requires
//! the `network-access-domains:trust-domains` scope.
//!
//! `getServices` returns the caller's `ServiceList` — the services (each a
//! logical commercial subscription tied to a `serviceSite`) associated with the
//! authenticated identity. It requires the `network-access-domains:services:read`
//! scope. Each `serviceSite` carries a deterministic `location.geographicPoint`
//! (a WGS-84 point stable per identity/slot); the canonical `propertyAddress`
//! (civic address) remains a documented cut. The point is a fixed, renderable
//! coordinate, not a queryable spatial field, so it is not a control plane.
//!
//! Both are protected: they require a valid access token
//! ([`crate::auth::verify::Claims`]) carrying the endpoint's scope.
//!
//! ## Functional cases — the input is the control plane (docs/DESIGN.md §7)
//!
//! **`getTrustDomainCapabilities`** — provider capabilities are fixed operator
//! configuration, not keyed to any device or subscriber, so this operation has
//! **no** parameter-driven control plane: given a scoped token it always returns
//! the same deterministic document (`200`).
//!
//! **`getServices`** has no request body either, but it *is* keyed to the
//! authenticated identity, so its control plane is the **token subject** (DESIGN
//! §7, mirroring the other subject-keyed reads): a reserved error suffix on the
//! subject → the canonical CAMARA error; otherwise the subject's trailing three
//! digits `d` fix the catalog deterministically — `d == 0` (`…000` / no digits) →
//! `200 []` (nothing associated; a list never 404s), else `((d - 1) % 3) + 1`
//! services (1–3), each with a deterministic UUID-shaped `id` and `serviceSite`.
//!
//! **`getService`** (`GET /services/{serviceId}`) reads a single `Service` from
//! that same identity-keyed catalog. It regenerates the subject's catalog (no
//! store) and matches the `serviceId` path parameter. Two control planes (DESIGN
//! §7): the subject's reserved error suffix → the canonical CAMARA error (an
//! account-level plane, checked first, mirroring `getServices`); otherwise the
//! `serviceId` vs the catalog — an id the identity holds → `200` that `Service`,
//! any other id (unknown / another identity's / malformed) → `404 NOT_FOUND`
//! (the opaque id is not itself a plane, so malformed folds into the `404`).
//!
//! **`createTrustDomain`** (`POST /trust-domains`) creates a Trust Domain from a
//! `TrustDomainCreate` body and persists it. Three control planes (DESIGN §7):
//! (1) the subject's reserved error suffix → the canonical CAMARA error (an
//! account-level plane, checked first, mirroring the reads); (2) request
//! validation → `400 INVALID_ARGUMENT` (missing/blank/oversized `name`, missing
//! `enabled`, missing/malformed `serviceId`, an `accessDetails` that is empty /
//! longer than 4, or an entry whose `accessType` is unknown, is not advertised,
//! or lacks its variant's required keys); (3) store state → `409` when a Trust
//! Domain with the *same* `name` already exists for the *same* `serviceId` (the
//! `trustDomainId` is derived from that pair, so a duplicate collides).
//!
//! For all of them, a missing/invalid token → `401 UNAUTHENTICATED`; a token
//! without the endpoint's scope → `403 PERMISSION_DENIED` (both from the shared
//! resource-server layer). `x-correlator` is echoed on every response.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::store;
use crate::auth::verify::Claims;
use crate::errors::CamaraError;
use crate::scenarios;

/// The OAuth2 scope the Trust Domain legs require — both the read-only
/// `GET /trust-domains/capabilities` and the stateful `POST /trust-domains`
/// (`createTrustDomain`) carry the single `network-access-domains:trust-domains`
/// scope (CAMARA NetworkAccessManagement / Network Access Domains).
const TRUST_DOMAINS_SCOPE: &str = "network-access-domains:trust-domains";

/// The OAuth2 scope the `GET /services` endpoint requires.
const SERVICES_SCOPE: &str = "network-access-domains:services:read";

/// The advertised Trust Domain access types (the discriminator values this
/// provider supports, from [`trust_domain_capabilities`]). `createTrustDomain`
/// admits only these — a well-formed `accessType` outside this set (e.g. the
/// canonical `"Thread:TLV"`, which this provider does not advertise) → `400`.
const ADVERTISED_ACCESS_TYPES: [&str; 3] = [
    "Wi-Fi:WPA_PERSONAL",
    "Wi-Fi:WPA_ENTERPRISE",
    "Thread:STRUCTURED",
];

/// Routes for Network Access Domains vwip, mounted at their canonical URLs.
pub fn routes() -> Router {
    Router::new()
        .route(
            "/network-access-domains/vwip/trust-domains/capabilities",
            get(get_capabilities),
        )
        .route(
            "/network-access-domains/vwip/trust-domains",
            post(create_trust_domain),
        )
        .route(
            "/network-access-domains/vwip/services",
            get(get_services),
        )
        .route(
            "/network-access-domains/vwip/services/:service_id",
            get(get_service),
        )
}

/// `GET /network-access-domains/vwip/trust-domains/capabilities`.
async fn get_capabilities(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    with_correlator(
        (StatusCode::OK, Json(trust_domain_capabilities())).into_response(),
        &correlator,
    )
}

/// `POST /network-access-domains/vwip/trust-domains` (`createTrustDomain`).
///
/// Creates a Trust Domain: the caller sends a `TrustDomainCreate` (a `serviceId`,
/// a human-readable `name`, an `enabled` flag, and 1–4 `accessDetails`), the
/// simulator validates it, mints a `trustDomainId`, renders the full
/// `TrustDomain` (adding the read-only `id` + audit stamps), persists it in the
/// in-memory [`store`], and returns `201` with that `TrustDomain`.
///
/// Three control planes (docs/DESIGN.md §7):
///
/// 1. **Reserved error suffix (token subject)** — an account-level plane checked
///    first, mirroring `getServices`/`getService`: if the subject's trailing
///    three digits name a reserved CAMARA status, the create answers that
///    canonical error regardless of the body.
/// 2. **Request validation** — a missing/blank/oversized `name`, a missing
///    `enabled`, a missing/malformed `serviceId` (must be a UUID), an
///    `accessDetails` that is absent / empty / longer than 4, or an
///    `accessDetail` whose `accessType` is unknown, is not one this provider
///    advertises (see [`ADVERTISED_ACCESS_TYPES`]), or is missing its variant's
///    required fields → `400 INVALID_ARGUMENT`.
/// 3. **Store state** — the `trustDomainId` is derived deterministically from the
///    `(serviceId, name)` identity ([`trust_domain_id`]), so creating a Trust
///    Domain with the *same* name for the *same* service collides → `409`
///    (duplicate name for service).
///
/// The nested `accessDetails` field *values* (SSID pattern, hex lengths, Thread
/// channel range, …) and the optional `policies` object are validated only for
/// presence/shape of the discriminator's required keys; the full per-field
/// pattern/range validation is a documented cut. The write-only WPA password is
/// stripped from the echoed `TrustDomain` (it never appears in a response).
/// `x-correlator` is echoed on every response.
async fn create_trust_domain(claims: Claims, headers: HeaderMap, body: Bytes) -> Response {
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's Trust Domain scope.
    if let Err(e) = claims.require_scope(TRUST_DOMAINS_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // Control plane 1 — account-level reserved-error suffix on the token subject
    // (checked first, mirroring the read legs).
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Parse the TrustDomainCreate body (malformed JSON / non-object → 400).
    let req: Value = match serde_json::from_slice::<Value>(&body) {
        Ok(v) if v.is_object() => v,
        _ => {
            return invalid_argument(
                "the request body is not a valid TrustDomainCreate JSON object",
                &correlator,
            )
        }
    };

    // Control plane 2 — required-field / shape validation.
    if let Err(message) = validate_trust_domain_create(&req) {
        return invalid_argument(&message, &correlator);
    }

    // Identity fields are guaranteed present + well-typed by `validate` above.
    let service_id = req["serviceId"].as_str().unwrap_or_default();
    let name = req["name"].as_str().unwrap_or_default();
    let id = trust_domain_id(service_id, name);

    // Render the full TrustDomain (adds the read-only id + audit stamps; strips
    // the write-only WPA password from the echoed accessDetails).
    let now = rfc3339_utc(now_unix_secs());
    let actor = deterministic_uuid_v5("nad-td-actor", identity);
    let trust_domain = render_trust_domain(&id, &req, &now, &actor);

    // Control plane 3 — store state (same (serviceId, name) → duplicate → 409).
    if !store::insert(id, trust_domain.clone()) {
        return with_correlator(
            CamaraError::conflict("A Trust Domain with this name already exists for the service.")
                .into_response(),
            &correlator,
        );
    }

    with_correlator(
        (StatusCode::CREATED, Json(trust_domain)).into_response(),
        &correlator,
    )
}

/// `GET /network-access-domains/vwip/services`.
///
/// Returns the caller's `ServiceList` — the services associated with the
/// authenticated identity. Keyed on the token subject (DESIGN §7): a reserved
/// error suffix → the canonical CAMARA error; otherwise the subject's trailing
/// three digits fix the catalog deterministically.
async fn get_services(claims: Claims, headers: HeaderMap) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SERVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The authenticated identity is the token subject (client_credentials sets
    // `sub` = client_id; the three-legged grants set a user subject). Reserved
    // error suffix on that identity → the canonical CAMARA error (DESIGN §7).
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    let list = Value::Array(services_for(identity));
    with_correlator((StatusCode::OK, Json(list)).into_response(), &correlator)
}

/// `GET /network-access-domains/vwip/services/{serviceId}`.
///
/// Reads a single `Service` from the caller's catalog by id (operationId
/// `getService`). The caller's `ServiceList` is fully deterministic from the
/// token subject (see [`services_for`]), so — mirroring the sibling
/// `getNetworkAccessDevice` — this endpoint needs no store: it regenerates the
/// subject's catalog and returns the service whose `id` matches the path
/// parameter. Two control planes (docs/DESIGN.md §7):
///
/// - **Reserved error suffix (subject)** — an account-level plane, mirroring
///   `getServices`: if the subject's trailing three digits name a reserved CAMARA
///   status, the endpoint answers that canonical error regardless of the id.
/// - **The `serviceId` vs the subject's catalog** — an id drawn from the
///   subject's deterministic catalog → `200` with that `Service`; any other id
///   (unknown, belonging to a different identity, or malformed) → `404
///   NOT_FOUND`. The id is opaque to the caller (SHA-256-derived UUID), so it is
///   not itself a scenario plane (there is no store to distinguish an unknown id
///   from a malformed one — both fold into the `404`, mirroring
///   `getNetworkAccessDevice`).
///
/// Requires a token carrying the `network-access-domains:services:read` scope.
async fn get_service(
    claims: Claims,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Response {
    // Optional correlation header, echoed on every response (CAMARA Commonalities).
    let correlator = headers.get("x-correlator").cloned();

    // Endpoint authorisation: the token must carry this API's scope.
    if let Err(e) = claims.require_scope(SERVICES_SCOPE) {
        return with_correlator(e.into_response(), &correlator);
    }

    // The token subject is the account-level control plane (docs/DESIGN.md §7):
    // a reserved suffix takes the whole account into a canonical error, matching
    // the listing endpoint.
    let identity = claims.subject().unwrap_or("");
    if let Some(err) = scenarios::reserved_error(identity) {
        return with_correlator(err.into_response(), &correlator);
    }

    // Regenerate the subject's deterministic catalog and look the id up. An id
    // that is not one of this identity's services — unknown, another identity's,
    // or malformed — is a `404 NOT_FOUND` (there is no store to distinguish them).
    match services_for(identity)
        .into_iter()
        .find(|svc| svc["id"] == json!(service_id))
    {
        Some(svc) => with_correlator((StatusCode::OK, Json(svc)).into_response(), &correlator),
        None => with_correlator(
            CamaraError::not_found("No service found for the provided id.").into_response(),
            &correlator,
        ),
    }
}

/// Fixed service templates `(name, description, siteName, siteDescription)`, one
/// per catalog slot. The count and per-slot selection are chosen deterministically
/// from the identity (below), so the catalog is reproducible per caller.
const SERVICE_TEMPLATES: [(&str, &str, &str, &str); 3] = [
    (
        "Main Home Internet",
        "Internet subscription for primary residence",
        "Primary Residence",
        "123 Main Street",
    ),
    (
        "Vacation Home Internet",
        "Internet subscription for vacation home",
        "Vacation Home",
        "45 Lakeside Road",
    ),
    (
        "Small Business Fibre",
        "Fibre subscription for a small business site",
        "Downtown Office",
        "500 Market Avenue",
    ),
];

/// The `ServiceList` (0..=3 `Service` records) for an authenticated `identity`.
///
/// Deterministic from the identity's trailing three digits `d` (DESIGN §7):
/// `d == 0` (or no digits) → an empty list (nothing associated; a list never
/// 404s), else `((d - 1) % 3) + 1` services (1–3).
fn services_for(identity: &str) -> Vec<Value> {
    let d = scenarios::trailing_three_digits(identity).unwrap_or(0);
    if d == 0 {
        return Vec::new();
    }
    let count = ((d - 1) % 3) + 1; // 1..=3
    (0..count as usize).map(|i| service(identity, i)).collect()
}

/// A single deterministic `Service` for `identity` at catalog slot `i`.
fn service(identity: &str, i: usize) -> Value {
    let (name, description, site_name, site_description) =
        SERVICE_TEMPLATES[i % SERVICE_TEMPLATES.len()];
    let (latitude, longitude) = deterministic_point(identity, i);
    json!({
        "id": deterministic_uuid("nad-service", identity, i),
        "name": name,
        "description": description,
        "serviceSite": {
            "id": deterministic_uuid("nad-service-site", identity, i),
            "name": site_name,
            "description": site_description,
            // The site's physical location (docs/DESIGN.md §7). Only the WGS-84
            // `geographicPoint` is emitted; the canonical `propertyAddress` (a
            // 20-field civic address) remains a documented cut. This is a fixed,
            // representative coordinate the caller can render, not a queryable
            // spatial field, so it is not a scenario control plane.
            "location": {
                "geographicPoint": { "latitude": latitude, "longitude": longitude }
            }
        }
    })
}

/// A deterministic WGS-84 point (decimal degrees) for `identity`'s service site
/// at catalog slot `index`.
///
/// Derived from a domain-tagged SHA-256 (a tag distinct from the id tags, so the
/// coordinate never collides with an id) mapped onto the valid latitude
/// (`[-90, 90]`) and longitude (`[-180, 180]`) ranges, rounded to five decimal
/// places (~1 m). Stable per `(identity, slot)` yet unrelated to the ids. No
/// `rand`/geo dependency (reuses sha2).
fn deterministic_point(identity: &str, index: usize) -> (f64, f64) {
    let h = Sha256::digest(format!("nad-service-site-geo:{identity}:{index}").as_bytes());
    // Two independent unit fractions in `[0, 1]` from disjoint hash bytes.
    let lat_unit = u16::from_be_bytes([h[0], h[1]]) as f64 / u16::MAX as f64;
    let lon_unit = u16::from_be_bytes([h[2], h[3]]) as f64 / u16::MAX as f64;
    let round5 = |x: f64| (x * 1e5).round() / 1e5;
    (round5(lat_unit * 180.0 - 90.0), round5(lon_unit * 360.0 - 180.0))
}

/// A deterministic, UUID-shaped id from the first 16 bytes of a domain-tagged
/// SHA-256 over `(tag, identity, index)` (distinct tags never collide). Mirrors
/// Blockchain Public Address's record-id rendering; no `uuid`/`rand` dependency.
fn deterministic_uuid(tag: &str, identity: &str, index: usize) -> String {
    let h = Sha256::digest(format!("{tag}:{identity}:{index}").as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// The fixed `TrustDomainCapabilities` document this provider advertises.
///
/// A representative, schema-valid set covering the three access-type families
/// (Wi-Fi WPA-Personal, Wi-Fi WPA-Enterprise, Thread STRUCTURED) and all four
/// policy capabilities. Deterministic — capabilities are provider configuration,
/// not keyed to any request input (DESIGN §7).
fn trust_domain_capabilities() -> Value {
    json!({
        "supportedAccessTypes": [
            {
                "accessType": "Wi-Fi:WPA_PERSONAL",
                "wifiProperties": {
                    "ssidRequired": true,
                    "supportedSecurityModes": [
                        "WPA2-Personal",
                        "WPA3-Personal",
                        "WPA2-WPA3-Personal"
                    ],
                    "passwordConstraints": {
                        "minLength": 8,
                        "maxLength": 63,
                        "requireSpecialCharacters": false
                    },
                    "authServerRequired": false
                }
            },
            {
                "accessType": "Wi-Fi:WPA_ENTERPRISE",
                "wifiProperties": {
                    "ssidRequired": true,
                    "supportedSecurityModes": [
                        "WPA2-Enterprise",
                        "WPA3-Enterprise"
                    ],
                    "authServerRequired": true
                }
            },
            {
                "accessType": "Thread:STRUCTURED",
                "threadProperties": {
                    "supportedChannels": { "min": 11, "max": 26 },
                    "networkKeyFormat": "32-hex-digits",
                    "networkNameConstraints": { "minLength": 1, "maxLength": 16 },
                    "maxDatasetLength": 254
                }
            }
        ],
        "supportedPolicies": {
            "maxDevices": { "minValue": 1, "maxValue": 100 },
            "maxDomainDownstreamRate": { "minValue": 1_000_000, "maxValue": 1_000_000_000 },
            "maxDomainUpstreamRate": { "minValue": 1_000_000, "maxValue": 1_000_000_000 },
            "egressAllowedList": {
                "maxEntries": 50,
                "supportedDestinationTypes": ["IP", "FQDN", "CIDR"]
            }
        }
    })
}

/// Validate a `TrustDomainCreate` body. Returns `Err(message)` on the first
/// violation (mapped by the caller to `400 INVALID_ARGUMENT`).
///
/// Checks the required top-level fields (`name`, `enabled`, `serviceId`,
/// `accessDetails`) and, for each access detail, that its `accessType` is one
/// this provider advertises ([`ADVERTISED_ACCESS_TYPES`]) and that the
/// discriminated variant carries its required keys. The nested field *values*
/// (SSID/key patterns, Thread channel range, …) and the optional `policies`
/// object are a documented cut — only presence/shape of the required keys is
/// enforced here.
fn validate_trust_domain_create(req: &Value) -> Result<(), String> {
    // name — required, non-blank, ≤ 64 characters.
    match req.get("name").and_then(Value::as_str) {
        None => return Err("`name` is required and must be a string".into()),
        Some(n) if n.trim().is_empty() => return Err("`name` must not be blank".into()),
        Some(n) if n.chars().count() > 64 => {
            return Err("`name` must be at most 64 characters".into())
        }
        Some(_) => {}
    }

    // enabled — required boolean.
    if !req.get("enabled").map(Value::is_boolean).unwrap_or(false) {
        return Err("`enabled` is required and must be a boolean".into());
    }

    // serviceId — required, a UUID (the caller obtains it from `getServices`;
    // validated as a lowercase-hex UUID, matching the ServiceId `format: uuid`,
    // not the stricter version/variant Uuid pattern the response id carries).
    match req.get("serviceId").and_then(Value::as_str) {
        None => return Err("`serviceId` is required and must be a string".into()),
        Some(s) if !is_uuid(s) => return Err("`serviceId` must be a valid UUID".into()),
        Some(_) => {}
    }

    // accessDetails — required array of 1..=4 access-detail objects.
    let details = match req.get("accessDetails").and_then(Value::as_array) {
        None => return Err("`accessDetails` is required and must be an array".into()),
        Some(d) => d,
    };
    if details.is_empty() {
        return Err("`accessDetails` must contain at least one entry".into());
    }
    if details.len() > 4 {
        return Err("`accessDetails` must contain at most 4 entries".into());
    }
    for detail in details {
        validate_access_detail(detail)?;
    }

    // description — optional, but if present a string ≤ 255 characters.
    if let Some(desc) = req.get("description").filter(|v| !v.is_null()) {
        match desc.as_str() {
            Some(s) if s.chars().count() <= 255 => {}
            Some(_) => return Err("`description` must be at most 255 characters".into()),
            None => return Err("`description` must be a string".into()),
        }
    }

    // expiration — optional, but if present a string (RFC 3339 date-time).
    if let Some(exp) = req.get("expiration").filter(|v| !v.is_null()) {
        if !exp.is_string() {
            return Err("`expiration` must be an RFC 3339 date-time string".into());
        }
    }

    // policies — optional, but if present an object (contents not validated).
    if let Some(pol) = req.get("policies").filter(|v| !v.is_null()) {
        if !pol.is_object() {
            return Err("`policies` must be an object".into());
        }
    }

    Ok(())
}

/// Validate one `AccessDetail` item — its `accessType` must be advertised and the
/// discriminated variant must carry its required keys.
fn validate_access_detail(detail: &Value) -> Result<(), String> {
    let obj = detail
        .as_object()
        .ok_or("each `accessDetails` entry must be an object")?;
    let access_type = obj
        .get("accessType")
        .and_then(Value::as_str)
        .ok_or("each `accessDetails` entry requires an `accessType`")?;
    if !ADVERTISED_ACCESS_TYPES.contains(&access_type) {
        return Err(format!(
            "`accessType` \"{access_type}\" is not supported by this provider (see GET /trust-domains/capabilities)"
        ));
    }

    match access_type {
        "Wi-Fi:WPA_PERSONAL" => {
            let mode = obj
                .get("securityMode")
                .and_then(Value::as_object)
                .ok_or("a Wi-Fi:WPA_PERSONAL access detail requires a `securityMode` object")?;
            if !mode.get("password").map(Value::is_string).unwrap_or(false) {
                return Err(
                    "a Wi-Fi:WPA_PERSONAL `securityMode` requires a string `password`".into(),
                );
            }
        }
        "Wi-Fi:WPA_ENTERPRISE" => {
            let mode = obj
                .get("securityMode")
                .and_then(Value::as_object)
                .ok_or("a Wi-Fi:WPA_ENTERPRISE access detail requires a `securityMode` object")?;
            if !mode
                .get("securityModeType")
                .map(Value::is_string)
                .unwrap_or(false)
            {
                return Err(
                    "a Wi-Fi:WPA_ENTERPRISE `securityMode` requires a string `securityModeType`"
                        .into(),
                );
            }
        }
        "Thread:STRUCTURED" => {
            for key in ["extendedPanId", "networkKey", "networkName", "panId"] {
                if !obj.get(key).map(Value::is_string).unwrap_or(false) {
                    return Err(format!(
                        "a Thread:STRUCTURED access detail requires a string `{key}`"
                    ));
                }
            }
            if !obj.get("channel").map(Value::is_number).unwrap_or(false) {
                return Err(
                    "a Thread:STRUCTURED access detail requires a numeric `channel`".into(),
                );
            }
        }
        _ => unreachable!("accessType already checked against the advertised set"),
    }
    Ok(())
}

/// Render the full `TrustDomain` response from a validated `TrustDomainCreate`
/// `req`: the minted read-only `id`, the echoed create fields, and the audit
/// stamps (`createdAt`/`modifiedAt` = `now`, `createdBy`/`modifiedBy` = `actor`).
/// The write-only WPA `password` is stripped from the echoed access details (it
/// never appears in a response).
fn render_trust_domain(id: &str, req: &Value, now: &str, actor: &str) -> Value {
    let mut td = serde_json::Map::new();
    td.insert("id".into(), json!(id));
    td.insert("serviceId".into(), req["serviceId"].clone());
    td.insert("name".into(), req["name"].clone());
    td.insert("enabled".into(), req["enabled"].clone());
    for key in ["description", "expiration", "policies"] {
        if let Some(v) = req.get(key).filter(|v| !v.is_null()) {
            td.insert(key.into(), v.clone());
        }
    }
    td.insert(
        "accessDetails".into(),
        sanitised_access_details(&req["accessDetails"]),
    );
    td.insert("createdAt".into(), json!(now));
    td.insert("createdBy".into(), json!(actor));
    td.insert("modifiedAt".into(), json!(now));
    td.insert("modifiedBy".into(), json!(actor));
    Value::Object(td)
}

/// Strip the write-only WPA `password` from each `accessDetails` entry so it is
/// never echoed in the `TrustDomain` response (CAMARA marks it `writeOnly`).
fn sanitised_access_details(details: &Value) -> Value {
    let items = details.as_array().cloned().unwrap_or_default();
    let cleaned = items.into_iter().map(|mut detail| {
        if let Some(mode) = detail
            .get_mut("securityMode")
            .and_then(Value::as_object_mut)
        {
            mode.remove("password");
        }
        detail
    });
    Value::Array(cleaned.collect())
}

/// A deterministic, server-assigned `trustDomainId` for a Trust Domain, derived
/// from its `(serviceId, name)` identity. Deriving it from the identity is what
/// makes a duplicate (same name for the same service) collide → the CAMARA `409`.
/// It is a strict RFC 4122 (version 5, name-based) UUID so it satisfies the
/// `TrustDomain.id` `Uuid` pattern the CAMARA schema requires.
pub fn trust_domain_id(service_id: &str, name: &str) -> String {
    deterministic_uuid_v5("nad-trust-domain", &format!("{service_id}\u{1f}{name}"))
}

/// A stable, strict RFC 4122 (version 5, name-based) UUID from a domain-tagged
/// SHA-256 over `key`, with the version (`5`) and variant nibbles forced so it
/// satisfies the strict `Uuid` pattern (`[1-5]` version, `[89ab]` variant) the
/// CAMARA `TrustDomain.id`/audit fields require. No `uuid`/`rand` dependency
/// (mirrors `edge_application_management::vwip::app_id`).
fn deterministic_uuid_v5(tag: &str, key: &str) -> String {
    let mut h = Sha256::digest(format!("{tag}:{key}").as_bytes());
    h[6] = (h[6] & 0x0f) | 0x50; // version 5
    h[8] = (h[8] & 0x3f) | 0x80; // variant (10xx)
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
    )
}

/// Whether `s` is a lowercase-hex, hyphenated 8-4-4-4-12 UUID string — the loose
/// `format: uuid` check applied to the caller-supplied `serviceId` (a strict
/// version/variant match is not required of it; the ServiceId schema is just
/// `format: uuid`).
fn is_uuid(s: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    let is_lower_hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    parts.len() == groups.len()
        && parts
            .iter()
            .zip(groups)
            .all(|(p, n)| p.len() == n && p.bytes().all(is_lower_hex))
}

/// A `400 INVALID_ARGUMENT` CAMARA error, with the correlator echoed.
fn invalid_argument(message: &str, correlator: &Option<HeaderValue>) -> Response {
    with_correlator(
        CamaraError::invalid_argument(message).into_response(),
        correlator,
    )
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
/// date/time dependency (mirrors `qos_provisioning::v0_3`).
fn rfc3339_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
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

/// Echo the request's `x-correlator` onto a response, if one was supplied.
fn with_correlator(mut response: Response, correlator: &Option<HeaderValue>) -> Response {
    if let Some(value) = correlator {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-correlator"), value.clone());
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt; // for `oneshot`

    const HOST: &str = "nad.local:8080";

    // --- Pure unit: the fixed capabilities document -----------------------

    #[test]
    fn capabilities_document_is_schema_shaped() {
        let caps = trust_domain_capabilities();

        // `supportedAccessTypes` — required, 1..=4, each carries its discriminator.
        let access = caps["supportedAccessTypes"].as_array().unwrap();
        assert!((1..=4).contains(&access.len()));
        for entry in access {
            let ty = entry["accessType"].as_str().unwrap();
            assert!(matches!(
                ty,
                "Wi-Fi:WPA_PERSONAL"
                    | "Wi-Fi:WPA_ENTERPRISE"
                    | "Thread:STRUCTURED"
                    | "Thread:TLV"
            ));
            // A Wi-Fi variant carries wifiProperties; a Thread variant threadProperties.
            if ty.starts_with("Wi-Fi") {
                assert!(entry["wifiProperties"].is_object());
            } else {
                assert!(entry["threadProperties"].is_object());
            }
        }

        // All four policy capabilities are advertised, in range.
        let pol = &caps["supportedPolicies"];
        assert_eq!(pol["maxDevices"]["minValue"], 1);
        assert_eq!(pol["maxDevices"]["maxValue"], 100);
        assert!(pol["maxDomainDownstreamRate"]["maxValue"].as_i64().unwrap() <= 10_000_000_000);
        assert!(pol["maxDomainUpstreamRate"]["minValue"].as_i64().unwrap() >= 0);
        let egress = &pol["egressAllowedList"];
        assert_eq!(egress["maxEntries"], 50);
        assert_eq!(
            egress["supportedDestinationTypes"].as_array().unwrap().len(),
            3
        );
    }

    #[test]
    fn wifi_password_constraints_are_within_the_camara_bounds() {
        let caps = trust_domain_capabilities();
        let personal = &caps["supportedAccessTypes"][0]["wifiProperties"]["passwordConstraints"];
        let min = personal["minLength"].as_i64().unwrap();
        let max = personal["maxLength"].as_i64().unwrap();
        assert!((8..=63).contains(&min));
        assert!((8..=63).contains(&max));
        assert!(min <= max);
    }

    // --- Integration through the real router -------------------------------

    fn app() -> Router {
        Router::new()
            .merge(crate::auth::routes())
            .merge(crate::apis::routes())
    }

    /// Mint an access token via `client_credentials` with the given scope.
    async fn mint_token(scope: &str) -> String {
        mint_token_as("nad-client", scope).await
    }

    /// Mint a token whose subject (`sub` = `client_id`) is `client_id` — lets a
    /// test choose the identity the subject-keyed `getServices` reads.
    async fn mint_token_as(client_id: &str, scope: &str) -> String {
        let body =
            format!("grant_type=client_credentials&client_id={client_id}&scope={scope}");
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

    /// GET the capabilities endpoint with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_caps(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-domains/vwip/trust-domains/capabilities")
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

    #[tokio::test]
    async fn returns_the_capabilities_document() {
        let token = mint_token(TRUST_DOMAINS_SCOPE).await;
        let (status, _, body) = get_caps(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["supportedAccessTypes"].as_array().unwrap().len() >= 1);
        assert!(body["supportedPolicies"]["maxDevices"].is_object());
    }

    #[tokio::test]
    async fn token_without_the_scope_is_forbidden() {
        let token = mint_token("some:other-scope").await;
        let (status, _, body) = get_caps(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn missing_token_is_unauthenticated() {
        let (status, _, body) = get_caps(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn x_correlator_is_echoed_on_success() {
        let token = mint_token(TRUST_DOMAINS_SCOPE).await;
        let (status, headers, _) = get_caps(Some(&token), Some("corr-nad")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-nad")
        );
    }

    // === getServices ======================================================

    // --- Pure unit: the deterministic catalog ------------------------------

    #[test]
    fn services_count_is_deterministic_from_trailing_digits() {
        // `…000` / no digits → empty list.
        assert!(services_for("nad-000").is_empty());
        assert!(services_for("no-digits").is_empty());
        // d>0 → ((d-1) % 3) + 1 services (1..=3).
        assert_eq!(services_for("nad-001").len(), 1); // (0%3)+1 = 1
        assert_eq!(services_for("nad-005").len(), 2); // (4%3)+1 = 2
        assert_eq!(services_for("nad-003").len(), 3); // (2%3)+1 = 3
    }

    #[test]
    fn services_are_well_shaped_and_deterministic() {
        let a = services_for("nad-005");
        let b = services_for("nad-005");
        assert_eq!(a, b, "same identity yields the same catalog");

        for svc in &a {
            // `id` is required and UUID-shaped; `serviceSite.id` likewise.
            let id = svc["id"].as_str().unwrap();
            assert!(is_uuid_shaped(id), "service id {id}");
            assert!(svc["name"].is_string());
            let site = &svc["serviceSite"];
            let site_id = site["id"].as_str().unwrap();
            assert!(is_uuid_shaped(site_id), "site id {site_id}");
            // A service id and its site id are drawn from distinct SHA-256 tags.
            assert_ne!(id, site_id);
            assert!(site["name"].is_string());
        }

        // A different identity yields a different first id.
        assert_ne!(a[0]["id"], services_for("nad-006")[0]["id"]);
    }

    #[test]
    fn service_site_location_point_is_valid_and_deterministic() {
        let catalog = services_for("nad-003"); // 3 services → 3 sites
        assert_eq!(catalog.len(), 3);
        for svc in &catalog {
            let point = &svc["serviceSite"]["location"]["geographicPoint"];
            let lat = point["latitude"].as_f64().unwrap();
            let lon = point["longitude"].as_f64().unwrap();
            // Within the WGS-84 valid ranges.
            assert!((-90.0..=90.0).contains(&lat), "latitude {lat} out of range");
            assert!((-180.0..=180.0).contains(&lon), "longitude {lon} out of range");
            // Rounded to at most five decimal places.
            assert_eq!((lat * 1e5).round() / 1e5, lat);
            assert_eq!((lon * 1e5).round() / 1e5, lon);
        }
        // Deterministic per identity+slot, and distinct across slots (the geo tag
        // is seeded by the slot index, so sites don't share one coordinate).
        assert_eq!(deterministic_point("nad-003", 0), deterministic_point("nad-003", 0));
        assert_ne!(deterministic_point("nad-003", 0), deterministic_point("nad-003", 1));
        // A different identity yields a different point for the same slot.
        assert_ne!(deterministic_point("nad-003", 0), deterministic_point("nad-004", 0));
    }

    /// Whether `s` is a lowercase-hex, hyphenated 8-4-4-4-12 UUID string.
    fn is_uuid_shaped(s: &str) -> bool {
        let parts: Vec<&str> = s.split('-').collect();
        parts.len() == 5
            && [8, 4, 4, 4, 12] == [
                parts[0].len(),
                parts[1].len(),
                parts[2].len(),
                parts[3].len(),
                parts[4].len(),
            ]
            && s.chars().all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    }

    // --- Integration through the real router -------------------------------

    /// GET `/services` with an optional Bearer token and optional `x-correlator`.
    async fn get_services_req(
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri("/network-access-domains/vwip/services")
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

    #[tokio::test]
    async fn services_returns_the_catalog_for_the_identity() {
        // sub = client_id = "nad-005" → digits 005 → 2 services.
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        let list = body.as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0]["id"].is_string());
        assert!(list[0]["serviceSite"]["id"].is_string());
        // The serviceSite carries a WGS-84 geographicPoint through the router.
        let point = &list[0]["serviceSite"]["location"]["geographicPoint"];
        assert!((-90.0..=90.0).contains(&point["latitude"].as_f64().unwrap()));
        assert!((-180.0..=180.0).contains(&point["longitude"].as_f64().unwrap()));
    }

    #[tokio::test]
    async fn services_is_empty_for_a_zero_tail_identity() {
        // sub = "nad-000" → digits 000 → empty list (a list never 404s).
        let token = mint_token_as("nad-000", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn services_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404 NOT_FOUND.
        let token = mint_token_as("nad-404", SERVICES_SCOPE).await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn services_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, body) = get_services_req(Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn services_missing_token_is_unauthenticated() {
        let (status, _, body) = get_services_req(None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn services_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, headers, _) = get_services_req(Some(&token), Some("corr-svc")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-svc")
        );
    }

    // === getService (single-service read) =================================

    /// GET `/services/{serviceId}` with an optional Bearer token and optional
    /// `x-correlator`. Returns (status, headers, json-or-null).
    async fn get_service_req(
        service_id: &str,
        token: Option<&str>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("GET")
            .uri(format!("/network-access-domains/vwip/services/{service_id}"))
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

    #[tokio::test]
    async fn service_read_returns_a_member_of_the_identitys_catalog() {
        // sub = "nad-005" → 2 services; read the first back by its id.
        let catalog = services_for("nad-005");
        let wanted = catalog[0].clone();
        let id = wanted["id"].as_str().unwrap();

        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req(id, Some(&token), None).await;
        assert_eq!(status, StatusCode::OK);
        // The read returns exactly the same Service the catalog listing holds.
        assert_eq!(body, wanted);
        assert_eq!(body["id"].as_str().unwrap(), id);
        assert!(body["serviceSite"]["id"].is_string());
    }

    #[tokio::test]
    async fn service_read_unknown_id_is_not_found() {
        // A well-formed but unowned id → 404 (a list read of the catalog never
        // holds this id).
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) =
            get_service_req("00000000-0000-0000-0000-000000000000", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_of_another_identitys_service_is_not_found() {
        // An id that belongs to a *different* identity's catalog is not this
        // caller's → 404 (the catalog is per-identity).
        let other_id = services_for("nad-006")[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req(&other_id, Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_malformed_id_is_not_found() {
        // A malformed (non-UUID) id folds into the same 404 (the opaque id is not
        // a plane; mirrors getNetworkAccessDevice).
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req("not-a-uuid", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_empty_catalog_identity_is_not_found() {
        // sub = "nad-000" → empty catalog, so any id → 404.
        let token = mint_token_as("nad-000", SERVICES_SCOPE).await;
        let (status, _, body) =
            get_service_req("3fa85f64-5717-4562-b3fc-2c963f66afa6", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404, regardless of the id.
        let token = mint_token_as("nad-404", SERVICES_SCOPE).await;
        let (status, _, body) =
            get_service_req("3fa85f64-5717-4562-b3fc-2c963f66afa6", Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn service_read_reserved_429_suffix_beats_a_valid_id() {
        // sub = "nad-429" → reserved suffix → 429, even for an otherwise-valid id.
        // (The …429 catalog would hold a service, but the account-level plane wins.)
        let catalog = services_for("nad-429");
        assert!(!catalog.is_empty(), "…429 tail yields a non-empty catalog");
        let id = catalog[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-429", SERVICES_SCOPE).await;
        let (status, _, body) = get_service_req(&id, Some(&token), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn service_read_token_without_the_scope_is_forbidden() {
        let id = services_for("nad-005")[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let (status, _, body) = get_service_req(&id, Some(&token), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn service_read_missing_token_is_unauthenticated() {
        let (status, _, body) =
            get_service_req("3fa85f64-5717-4562-b3fc-2c963f66afa6", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn service_read_x_correlator_is_echoed() {
        let id = services_for("nad-005")[0]["id"].as_str().unwrap().to_string();
        let token = mint_token_as("nad-005", SERVICES_SCOPE).await;
        let (status, headers, _) = get_service_req(&id, Some(&token), Some("corr-svc-1")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-svc-1")
        );
    }

    // === createTrustDomain (POST /trust-domains) ==========================

    /// A representative, valid `TrustDomainCreate` body (one Wi-Fi WPA-Personal
    /// access detail). `service_id` / `name` are the (serviceId, name) identity
    /// pair that keys the minted `trustDomainId` and the `409` duplicate case.
    fn td_body(service_id: &str, name: &str) -> Value {
        json!({
            "serviceId": service_id,
            "name": name,
            "enabled": true,
            "description": "Primary home Wi-Fi",
            "accessDetails": [
                {
                    "accessType": "Wi-Fi:WPA_PERSONAL",
                    "ssid": "my-ssid",
                    "securityMode": {
                        "password": "s3cr3t-pass",
                        "securityModeType": "WPA3-Personal"
                    }
                }
            ]
        })
    }

    // --- Pure units --------------------------------------------------------

    #[test]
    fn trust_domain_id_is_strict_uuid_and_identity_keyed() {
        let id = trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        // 8-4-4-4-12, lowercase hex, version nibble 5, variant nibble in 8..=b.
        assert!(is_uuid_shaped(&id), "id {id}");
        let raw: String = id.chars().filter(|c| *c != '-').collect();
        assert_eq!(raw.as_bytes()[12], b'5', "version nibble is 5");
        assert!(matches!(raw.as_bytes()[16], b'8' | b'9' | b'a' | b'b'), "variant nibble");
        // Deterministic per (serviceId, name); differs when either changes.
        assert_eq!(id, trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home"));
        assert_ne!(id, trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Office"));
        assert_ne!(id, trust_domain_id("123e4567-e89b-12d3-a456-426614174000", "Home"));
    }

    #[test]
    fn is_uuid_accepts_lowercase_and_rejects_others() {
        assert!(is_uuid("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
        assert!(is_uuid("00000000-0000-0000-0000-000000000000"));
        assert!(!is_uuid("3FA85F64-5717-4562-B3FC-2C963F66AFA6")); // uppercase
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("3fa85f64-5717-4562-b3fc-2c963f66afa")); // last group short
        assert!(!is_uuid("3fa85f64571745 62b3fc2c963f66afa6")); // wrong shape
    }

    #[test]
    fn validate_accepts_a_well_formed_body() {
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        assert!(validate_trust_domain_create(&body).is_ok());
    }

    #[test]
    fn validate_rejects_missing_and_malformed_fields() {
        let base = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");

        // name missing / blank / too long.
        let mut b = base.clone();
        b.as_object_mut().unwrap().remove("name");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["name"] = json!("   ");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["name"] = json!("x".repeat(65));
        assert!(validate_trust_domain_create(&b).is_err());

        // enabled missing / wrong type.
        let mut b = base.clone();
        b.as_object_mut().unwrap().remove("enabled");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["enabled"] = json!("yes");
        assert!(validate_trust_domain_create(&b).is_err());

        // serviceId missing / not a UUID.
        let mut b = base.clone();
        b.as_object_mut().unwrap().remove("serviceId");
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        b["serviceId"] = json!("not-a-uuid");
        assert!(validate_trust_domain_create(&b).is_err());

        // accessDetails empty / too many.
        let mut b = base.clone();
        b["accessDetails"] = json!([]);
        assert!(validate_trust_domain_create(&b).is_err());
        let mut b = base.clone();
        let one = base["accessDetails"][0].clone();
        b["accessDetails"] = json!([one, one.clone(), one.clone(), one.clone(), one.clone()]);
        assert!(validate_trust_domain_create(&b).is_err());
    }

    #[test]
    fn validate_rejects_unadvertised_and_incomplete_access_types() {
        let base = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");

        // Thread:TLV is a canonical enum but NOT advertised by this provider → 400.
        let mut b = base.clone();
        b["accessDetails"] = json!([{
            "accessType": "Thread:TLV",
            "operationalDataset": "0e08000000000000010010"
        }]);
        assert!(validate_trust_domain_create(&b).is_err());

        // An entirely unknown accessType → 400.
        let mut b = base.clone();
        b["accessDetails"] = json!([{ "accessType": "Bluetooth:LE" }]);
        assert!(validate_trust_domain_create(&b).is_err());

        // Advertised but missing the variant's required key (no securityMode) → 400.
        let mut b = base.clone();
        b["accessDetails"] = json!([{ "accessType": "Wi-Fi:WPA_PERSONAL", "ssid": "x" }]);
        assert!(validate_trust_domain_create(&b).is_err());

        // Thread:STRUCTURED missing a required field (panId) → 400.
        let mut b = base;
        b["accessDetails"] = json!([{
            "accessType": "Thread:STRUCTURED",
            "channel": 13,
            "extendedPanId": "d63e8e3e495ebbc3",
            "networkKey": "dfd34f0f05cad978ec4e32b0413038ff",
            "networkName": "Spec-Thread"
        }]);
        assert!(validate_trust_domain_create(&b).is_err());
    }

    #[test]
    fn render_strips_password_and_stamps_audit_fields() {
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        let id = trust_domain_id("3fa85f64-5717-4562-b3fc-2c963f66afa6", "Home");
        let td = render_trust_domain(&id, &body, "2024-01-01T00:00:00Z", "actor-uuid");

        assert_eq!(td["id"], json!(id));
        assert_eq!(td["name"], json!("Home"));
        assert_eq!(td["enabled"], json!(true));
        assert_eq!(td["description"], json!("Primary home Wi-Fi"));
        assert_eq!(td["createdAt"], json!("2024-01-01T00:00:00Z"));
        assert_eq!(td["modifiedAt"], json!("2024-01-01T00:00:00Z"));
        assert_eq!(td["createdBy"], json!("actor-uuid"));
        assert_eq!(td["modifiedBy"], json!("actor-uuid"));
        // The write-only password is stripped; the rest of securityMode remains.
        let mode = &td["accessDetails"][0]["securityMode"];
        assert!(mode.get("password").is_none(), "password must not be echoed");
        assert_eq!(mode["securityModeType"], json!("WPA3-Personal"));
    }

    // --- Integration through the real router -------------------------------

    /// POST `/trust-domains` with an optional Bearer token, optional JSON body,
    /// and optional `x-correlator`. Returns (status, headers, json-or-null).
    async fn post_trust_domain(
        token: Option<&str>,
        body: Option<&Value>,
        correlator: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/network-access-domains/vwip/trust-domains")
            .header("host", HOST);
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        if let Some(c) = correlator {
            builder = builder.header("x-correlator", c);
        }
        let req_body = match body {
            Some(v) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(v).unwrap())
            }
            None => Body::empty(),
        };
        let response = app().oneshot(builder.body(req_body).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    #[tokio::test]
    async fn create_trust_domain_persists_and_returns_the_resource() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD create ok");
        let (status, _, td) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::CREATED);

        // A minted, strict-UUID id + echoed create fields + audit stamps.
        let id = td["id"].as_str().unwrap();
        assert!(is_uuid_shaped(id), "id {id}");
        assert_eq!(td["serviceId"], body["serviceId"]);
        assert_eq!(td["name"], json!("TD create ok"));
        assert_eq!(td["enabled"], json!(true));
        assert!(td["createdAt"].as_str().unwrap().ends_with('Z'));
        assert!(td["createdBy"].is_string());
        assert!(td["modifiedAt"].as_str().unwrap().ends_with('Z'));
        // The write-only password never appears in the response.
        assert!(td["accessDetails"][0]["securityMode"].get("password").is_none());

        // It is persisted under its minted id (backs the later getTrustDomain leg).
        let stored = store::get(id).expect("the created Trust Domain is stored");
        assert_eq!(stored["name"], json!("TD create ok"));
    }

    #[tokio::test]
    async fn create_duplicate_name_for_service_conflicts() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("123e4567-e89b-12d3-a456-426614174000", "TD dup name");

        let (first, _, _) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(first, StatusCode::CREATED);
        // Same (serviceId, name) → same minted id → duplicate → 409.
        let (second, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(second, StatusCode::CONFLICT);
        assert_eq!(err["code"], "CONFLICT");

        // A different name for the same service is a distinct Trust Domain → 201.
        let other = td_body("123e4567-e89b-12d3-a456-426614174000", "TD dup name other");
        let (third, _, _) = post_trust_domain(Some(&token), Some(&other), None).await;
        assert_eq!(third, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_reserved_error_suffix_on_the_subject() {
        // sub = "nad-404" → reserved suffix → canonical 404, regardless of the body.
        let token = mint_token_as("nad-404", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD reserved 404");
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(err["code"], "NOT_FOUND");
    }

    #[tokio::test]
    async fn create_reserved_429_suffix_beats_a_valid_body() {
        // sub = "nad-429" → reserved suffix → 429, even for an otherwise-valid body.
        let token = mint_token_as("nad-429", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD reserved 429");
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(err["code"], "TOO_MANY_REQUESTS");
    }

    #[tokio::test]
    async fn create_invalid_body_is_bad_request() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        // Missing accessDetails → 400 INVALID_ARGUMENT.
        let body = json!({
            "serviceId": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
            "name": "TD invalid",
            "enabled": true
        });
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_unadvertised_access_type_is_bad_request() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let mut body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD tlv");
        body["accessDetails"] = json!([{
            "accessType": "Thread:TLV",
            "operationalDataset": "0e08000000000000010010"
        }]);
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(err["code"], "INVALID_ARGUMENT");
    }

    #[tokio::test]
    async fn create_token_without_the_scope_is_forbidden() {
        let token = mint_token_as("nad-005", "some:other-scope").await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD forbidden");
        let (status, _, err) = post_trust_domain(Some(&token), Some(&body), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(err["code"], "PERMISSION_DENIED");
    }

    #[tokio::test]
    async fn create_missing_token_is_unauthenticated() {
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD noauth");
        let (status, _, err) = post_trust_domain(None, Some(&body), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(err["code"], "UNAUTHENTICATED");
    }

    #[tokio::test]
    async fn create_x_correlator_is_echoed() {
        let token = mint_token_as("nad-005", TRUST_DOMAINS_SCOPE).await;
        let body = td_body("3fa85f64-5717-4562-b3fc-2c963f66afa6", "TD correlator");
        let (status, headers, _) =
            post_trust_domain(Some(&token), Some(&body), Some("corr-td-1")).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            headers.get("x-correlator").and_then(|v| v.to_str().ok()),
            Some("corr-td-1")
        );
    }
}
