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

Purpose-scope validation is now in place (`src/auth/purpose.rs`): a requested scope's
`dpv:<Purpose>#<action>` tokens must be well-formed, checked at every scope-request entry
point (token `client_credentials`, `/oauth2/authorize`, `/bc-authorize`) → OAuth2
`invalid_scope`; non-`dpv:` technical scopes pass untouched. Endpoint-level gating of the
required purpose scope is already handled by `verify::Claims::require_scope`.

Phase 1 has begun. **Number Verification v1** `POST /verify` is live at
`/number-verification/v1/verify`: the first CAMARA business endpoint, protected by the
`Claims` extractor (scope `number-verification:verify`). It accepts exactly one of
`phoneNumber` (E.164) / `hashedPhoneNumber`, and drives its result from the submitted
`phoneNumber` per DESIGN §7 — reserved error suffix → canonical CAMARA error (shared
`scenarios::reserved_error`), trailing `000` → `devicePhoneNumberVerified:false`, else
`true`; a `hashedPhoneNumber` can't be reversed so always verifies `true` (documented).
`x-correlator` is echoed on every response. New `src/apis/` tree wires business APIs into
the router and the `/` catalog now lists mounted APIs.

Number Verification v1 is now complete: `POST /verify` **and**
`GET /device-phone-number` (`number-verification:device-phone-number:read`).
The latter has no request body, so its functional cases key off the token
**subject** — a reserved error suffix on `sub` → canonical CAMARA error, an
E.164 `sub` → that number, anything else → the simulator's default line.

**Next up:** SIM Swap v2.

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
- [x] Purpose/scope enforcement — validate CAMARA `dpv:` purpose-scope grammar at the
  token / authorize / bc-authorize scope-request entry points → `invalid_scope` (DESIGN §7).
  (Endpoint-level *gating* of the required purpose scope is already handled by
  `verify::Claims::require_scope`; Phase 1 APIs wire the specific scope per endpoint.)

### Phase 1 — Stateless, non-spatial
- [x] Number Verification v1 — [x] `POST /verify` · [x] `GET /device-phone-number`
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

- 2026-08-02 — Phase 1: Number Verification v1 — `GET /device-phone-number` (CAMARA 1.0.0),
  completing the API. New GET route at `/number-verification/v1/device-phone-number`, protected by
  the `Claims` extractor (scope `number-verification:device-phone-number:read`; confirmed against
  the r1.3/v1.0.0 upstream spec, along with operationId `phoneNumberShare` and the
  `NumberVerificationShareResponse{devicePhoneNumber}` body). No request body, so per DESIGN §7 the
  control plane is the token **subject** (`sub`): `scenarios::reserved_error(sub)` → canonical CAMARA
  error (…404/…429/etc., reusing the shared convention); an E.164 `sub` is echoed as the device's own
  number (line-authenticated three-legged token); any other subject (e.g. synthetic `camarasim-user`)
  → the simulator's default line `+123456789012`. `x-correlator` echoed on every response. No new deps
  (reuses existing helpers `with_correlator`/`is_valid_e164`). Spec: added the `/device-phone-number`
  path + `NumberVerificationShareResponse` schema to the vendored openapi with `x-camarasim-scenarios`
  documenting the subject-keyed cases (full shared error set exposed, noted as a CamaraSim extension
  over canonical 1.0.0). 124 tests green (was 118; +6: default/E.164-echo/reserved-error/wrong-scope/
  no-token/x-correlator). Future: bind `login_hint`→`sub` in authorize/CIBA so three-legged flows can
  also drive per-device cases. — binary: 956K (978496 B; +3560 B)
- 2026-08-02 — Phase 1: Number Verification v1 — `POST /verify` (CAMARA 1.0.0). First CAMARA
  business endpoint. New `src/apis/` tree (`apis.rs` → `number_verification.rs` → `v1.rs`),
  mounted at `/number-verification/v1/verify` and merged into the app router; `/` catalog now
  lists mounted APIs. Handler is protected by the `Claims` extractor (scope
  `number-verification:verify`), extracts the body as `Bytes` and parses with
  `deny_unknown_fields` for precise CAMARA `INVALID_ARGUMENT` (400) on malformed / unknown-field
  / neither-or-both-identifiers / bad-E.164 input. Functional cases (§7) driven by the submitted
  `phoneNumber`: `scenarios::reserved_error` → canonical CAMARA error (…404/…429/etc.), trailing
  `000` → `devicePhoneNumberVerified:false`, else `true`; `hashedPhoneNumber` accepted but always
  `true` (can't reverse a hash to pick a case — documented). E.164 validated inline against
  `^\+[1-9][0-9]{4,14}$` (no regex dep). `x-correlator` echoed on all responses. Made
  `scenarios::last_three_digits` reusable via `pub trailing_three_digits`. No new deps. Spec:
  new `specs/number-verification/v1/openapi.yaml` — vendored CAMARA 1.0.0 `POST /verify`,
  `$ref`-ing shared `errors.yaml` responses + auth `camaraOAuth` scheme, with
  `x-camarasim-scenarios` documenting the cases (device-phone-number deferred to next pass, so
  the served spec matches the code). 118 tests green (was 105; +13: E.164 unit + 12 integration
  covering happy/no-match/reserved-error/hashed/all-400-shapes/scope/auth/x-correlator).
  — binary: 956K (974936 B; +54744 B — first business API + serde derive)
- 2026-08-02 — Phase 0 (complete): purpose/scope enforcement — validate CAMARA `dpv:`
  purpose-scope grammar. New `src/auth/purpose.rs`: pure `validate_scope(&str) -> Result<(),String>`
  checking each space-delimited token; a `dpv:`-prefixed token must be `dpv:<Purpose>#<action>`
  (exactly one `#`, non-empty alphanumeric purpose, non-empty action in `[A-Za-z0-9._:-]`),
  non-`dpv:` technical scopes pass untouched, empty scope valid, first offending token reported.
  Wired into all three client-facing scope-request entry points → OAuth2 `invalid_scope`
  (RFC 6749 §5.2/§4.1.2.1): token `client_credentials` (400), `/oauth2/authorize` (redirectable
  error, honours state), `/bc-authorize` (400); shared `token::invalid_scope` helper made
  `pub(super)`. `authorization_code`/CIBA token grants take scope from the stored code/request,
  so they're validated at authorize/bc-authorize time. Gating of the required scope stays with
  `verify::Claims::require_scope` (Phase 1 wires the per-endpoint scope). No new deps. Spec:
  documented the purpose-scope grammar + `invalid_scope` functional case on all three endpoints
  (`invalid_scope` was already in the `OAuthError` enum — now exercised). 105 tests green (was 95;
  +10: 5 grammar unit + 5 endpoint integration incl. happy-path + malformed for each entry point).
  — binary: 900K (920192 B; +1768 B)
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
