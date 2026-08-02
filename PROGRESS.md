# CamaraSim — Progress & Backlog

Single source of truth for **what's done, what's next, and what was tried**. The hourly
agent reads this first (with `docs/AGENT.md`) and updates it every pass. Keep it terse.

Status keys: `[ ]` todo · `[~]` in-progress (claimed) · `[x]` done · `[!]` blocked

---

## Current status

Phase 0 auth underway: OIDC discovery + JWKS + the token endpoint's
`client_credentials` **and `authorization_code` + PKCE** grants. The simulator holds one
fixed, public, simulator-only RSA key (RS256) bundled in the binary; its public half is
served at `/oauth2/jwks`, and `POST /oauth2/token` issues real RS256 `at+jwt` JWTs signed
with that key. `GET /oauth2/authorize` now runs the three-legged front leg (auto-consent,
mandatory S256 PKCE) and mints a single-use in-memory authorization code, which the token
endpoint redeems (validating redirect_uri / client_id / PKCE). The resource-server half is
in place: the `verify::Claims` extractor validates a presented Bearer token (RS256
signature against the JWKS, `exp`, `aud`) and enforces scope, returning the CAMARA error
model (401 `UNAUTHENTICATED` / 403 `PERMISSION_DENIED`) with RFC 6750 `WWW-Authenticate`.
No CAMARA business APIs yet.

All three CAMARA grants now work: `client_credentials`, `authorization_code`+PKCE,
and CIBA. `POST /bc-authorize` mints a single-use, in-memory `auth_req_id` (headless
auto-consent; the `login_hint` selects approved/pending/denied per DESIGN §7), and the
token endpoint's `ciba` grant polls it, returning the CIBA token-error set
(`authorization_pending` / `access_denied` / `expired_token`) or an access token.

The shared **reserved-identifier scenario convention** and the **base CAMARA error
model** are now in place (`src/scenarios.rs`, `src/errors.rs`): an identifier's trailing
three digits deterministically select a canonical CAMARA error (`…404`→404 `NOT_FOUND`,
`…429`→429 `TOO_MANY_REQUESTS`, etc.) so every Phase 1 identifier-keyed API exposes its
error cases from the input alone; documented once in `specs/shared/errors.yaml`.

**Next up:** Phase 0 — Purpose/scope enforcement (validate/gate CAMARA `dpv:` purpose
scopes), then Phase 1 stateless APIs.

## In progress (claimed this pass)

_None._  <!-- agent: put the claimed item + run timestamp here, clear it when done -->

## Backlog (work top-down; respect phases — see docs/DESIGN.md §12)

### Phase 0 — Auth foundation
- [x] `GET /.well-known/openid-configuration` (discovery) + served metadata
- [x] `GET /oauth2/jwks` (JWKS) + signing key management
- [x] `POST /oauth2/token` — `client_credentials` grant (signed JWT, scopes, expiry)
- [x] Token verification middleware for protected routes (audience/scope/expiry)
- [x] `GET /oauth2/authorize` + `POST /oauth2/token` — `authorization_code` + PKCE (auto-consent)
- [x] `POST /bc-authorize` + CIBA token polling
- [x] Shared reserved-identifier scenario convention + base CAMARA error model (DESIGN §7, §8)
- [ ] Purpose/scope enforcement — validate/gate CAMARA `dpv:` purpose scopes (DESIGN §7)

### Phase 1 — Stateless, non-spatial
- [ ] Number Verification v1 — `POST /verify`, `GET /device-phone-number`
- [ ] SIM Swap v2 — `POST /check`, `POST /retrieve-date` (+ full parameter-driven cases)
- [ ] KYC Match v1 — `POST /match`

### Phase 2 — Stateless device queries
- [ ] Device Status — reachability, roaming
- [ ] Device Identifier

### Phase 3 — Stateful, non-spatial
- [ ] One-Time-Password SMS — `POST /send-code`, `POST /validate-code` (in-memory store)
- [ ] Quality on Demand (QoD) — session lifecycle + CloudEvents notifications

### Phase 4 — Spatial
- [ ] Device Location Verification
- [ ] Device Location Retrieval
- [ ] Geofencing (subscriptions/notifications)

### Phase 5 — Remaining
- [ ] Carrier Billing / Payments
- [ ] Other CAMARA APIs as capacity allows

## Cross-cutting (do alongside the item that needs it)
- [~] `errors.rs`: base CAMARA error model done (`src/errors.rs`, `specs/shared/errors.yaml`); per-version catalogs still TODO (DESIGN §8)
- [ ] `registry.rs`: canonical URL versioning + `/` catalog wiring (DESIGN §9)
- [ ] `specs/…`: vendor + annotate OpenAPI per API/version, serve at `/{api}/v{n}/openapi.yaml`
- [ ] Contract-test harness (validate responses against vendored spec)

---

## Scan journal

Newest first. One line per pass: `YYYY-MM-DD HH:MMZ — <what happened> — binary: <size>`

- 2026-08-02 — Phase 0: shared reserved-identifier scenario convention + base CAMARA error model
  (DESIGN §7, §8). New `src/errors.rs`: `CamaraError { status, code, message }` with the
  Commonalities standard code/message set and `CamaraError::for_status(u16)` (400 INVALID_ARGUMENT,
  401 UNAUTHENTICATED, 403 PERMISSION_DENIED, 404 NOT_FOUND, 409 CONFLICT, 422 SERVICE_NOT_APPLICABLE,
  429 TOO_MANY_REQUESTS, 500 INTERNAL, 503 UNAVAILABLE; `None` outside the set) + `IntoResponse`.
  New `src/scenarios.rs`: `reserved_error(id)` maps an identifier's trailing three digits (non-digit
  formatting ignored) to that canonical CAMARA error, `None` for happy path; codes verified against
  live CAMARA API Design Guide. Pure/in-memory, no request-path consumer yet (Phase 1 wires it in),
  so `#![allow(dead_code)]`. No new deps. Spec: new `specs/shared/errors.yaml` — reusable
  `CamaraError` schema + per-status responses + the reserved-suffix convention table
  (`x-camarasim-reserved-error-suffixes`), `$ref`-able by every API version. 95 tests green (was 84;
  +11: every suffix→status, tail-only matching, formatting-ignored, catalog↔convention no-drift,
  body/IntoResponse shape). — binary: 900K (918424 B; unchanged — dead code stripped by LTO)
- 2026-08-02 — Phase 0: implemented CIBA — `POST /bc-authorize` + the `ciba` token-polling
  grant. New `src/auth/ciba.rs`: process-global in-memory backchannel-request store
  (`std::sync::Mutex<HashMap>`, lock never held across await), opaque single-use `auth_req_id`
  (`base64url(SHA-256(counter‖now))`, mirrors codes.rs). `/bc-authorize` authenticates the client
  (reuses token.rs `client_id_from_basic`), requires `login_hint`+`scope`, and returns
  `{auth_req_id, expires_in:120, interval:5}`. Functional case (§7) driven by `login_hint`:
  default→approved, contains `pending`→`authorization_pending`, contains `denied`→`access_denied`.
  `token.rs`: `ciba` grant branch polls the store → maps OpenID CIBA Core §11 outcomes (approved
  issues a token via shared `issue_access_token`, aud = captured audience, sub = `camarasim-user`;
  else `authorization_pending`/`access_denied`/`expired_token`/`invalid_grant`); approved id
  consumed (single use). Made token.rs `oauth_error`/`no_store`/`client_id_from_basic` `pub(super)`.
  No new deps (reuses sha2/base64/serde_urlencoded). Spec: added `/bc-authorize` path +
  Backchannel{Request,Response} schemas, extended `/oauth2/token` (auth_req_id, CIBA error set) and
  OAuthError enum, documented functional cases. Full backchannel flow tested end-to-end
  (bc-authorize → poll) incl. approved/pending/denied/single-use/wrong-client/unknown. 84 tests
  green (was 67). — binary: 897K (918424 B)
- 2026-08-02 — Phase 0: implemented `GET /oauth2/authorize` + the `authorization_code` + PKCE
  grant at `POST /oauth2/token`. New `src/auth/codes.rs`: process-global in-memory, single-use
  authorization-code store (`std::sync::Mutex<HashMap>`, lock never held across await),
  opaque codes (`base64url(SHA-256(counter‖now))`), S256 PKCE verify (RFC 7636 §4.6, constant-
  time compare). New `src/auth/authorize.rs`: `GET /oauth2/authorize` auto-consents (headless),
  mandates S256 PKCE, binds redirect_uri/client_id/scope/audience to the code; redirectable vs
  direct errors per RFC 6749 §4.1.2.1 (302 back with error+state, or direct 400 when
  client_id/redirect_uri untrusted). `token.rs`: `authorization_code` branch redeems (single-use)
  and validates redirect_uri / client_id / PKCE → `invalid_grant` on any mismatch; token issuance
  refactored into shared `issue_access_token` (aud = authorized audience, sub = `camarasim-user`).
  No new deps (reuses sha2/base64/serde_urlencoded). Spec: added `/oauth2/authorize` path + params
  + functional cases, extended `/oauth2/token` (TokenRequest code/redirect_uri/code_verifier,
  invalid_grant, unsupported_response_type). Full 3-legged flow tested end-to-end (mint code →
  redeem), plus single-use/PKCE/redirect/client mismatch cases. 67 tests green (was 45). —
  binary: 888K (907680 B)
- 2026-08-02 — Phase 0: implemented token-verification middleware for protected routes. New
  `src/auth/verify.rs`: `Claims` axum extractor (`FromRequestParts`) that pulls
  `Authorization: Bearer`, pins header `alg` to RS256 (rejects `alg:none`/confusion), verifies
  the signature against the bundled JWKS key, requires `exp` (unexpired) and `aud` == this
  server's issuer URL; `Claims::require_scope` gates per-endpoint authorisation. Failures map
  to the CAMARA error body (`{status,code,message}`) — 401 `UNAUTHENTICATED` / 403
  `PERMISSION_DENIED` — with RFC 6750 `WWW-Authenticate`. Added `keys::verify_rs256`
  (cached `VerifyingKey`, anti-drift with signing). No new deps (reuses `rsa`/`sha2`). Spec:
  auth openapi now documents resource-server verification + reusable `camaraOAuth`
  securityScheme, `CamaraError` schema, `Unauthenticated`/`PermissionDenied` responses.
  End-to-end tests mint a real token and drive it through a protected route (accept / missing /
  garbage / wrong-scope / wrong-audience). 45 tests green. — binary: 868K (884808 B)
- 2026-08-02 — Phase 0: implemented `POST /oauth2/token` `client_credentials` grant. New
  `src/auth/token.rs`: parses the urlencoded body, requires client auth via
  `client_secret_basic` or `client_secret_post` (any secret accepted), grants the requested
  scope verbatim, and issues a real RS256 `at+jwt` JWT (iss/aud/sub/client_id/scope/iat/exp
  +3600s/jti) signed with the JWKS key. `keys::sign_rs256` added (RSASSA-PKCS1-v1_5+SHA-256),
  reusing the bundled RSA key. OAuth2 errors (RFC 6749 §5.2): invalid_request /
  unsupported_grant_type / invalid_client; `authorization_code`+CIBA advertised but return
  unsupported_grant_type for now. Deps: `sha2` (oid) + `serde_urlencoded` (already transitive
  via axum). Spec: added `/oauth2/token` path + TokenRequest/TokenResponse/OAuthError schemas
  with functional cases. 28 tests green (incl. signature-verifies-against-JWKS). — binary:
  868K (884808 B)
- 2026-08-02 — Phase 0: implemented `GET /oauth2/jwks` + signing-key management. New
  `src/auth/keys.rs`: bundled fixed 2048-bit PKCS#8 RSA key (`assets/signing_key.pem`),
  parsed once via `OnceLock`, public half published as an RSA JWK (`use:sig`, `alg:RS256`,
  stable `kid`, base64url `n`/`e`) derived from the same key (anti-drift). Deps: `rsa`
  (pure Rust, no OpenSSL) + `base64`. Spec: added `/oauth2/jwks` path + `JwkSet`/`Jwk`
  schemas. 17 tests green. — binary: 804K (819504 B)
- 2026-08-02 — Phase 0: implemented `GET /.well-known/openid-configuration` (OIDC discovery).
  New `src/auth/` module; base URL from `CAMARASIM_ISSUER` env or `X-Forwarded-Proto`+`Host`.
  Advertises all 3 CAMARA grants (client_credentials/authorization_code/CIBA), RS256, S256 PKCE,
  CIBA poll. Authored `specs/auth/openapi.yaml`. Catalog now links the discovery doc. 10 tests
  green, no new deps. — binary: 704K (720856 B)
- 2026-08-02 — Repo bootstrapped: docs (DESIGN/AGENT), PROGRESS, axum skeleton (`/health`, `/`),
  size-tuned release profile, first test. Ready for Phase 0.
