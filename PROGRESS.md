# CamaraSim — Progress & Backlog

Single source of truth for **what's done, what's next, and what was tried**. The hourly
agent reads this first (with `docs/AGENT.md`) and updates it every pass. Keep it terse.

Status keys: `[ ]` todo · `[~]` in-progress (claimed) · `[x]` done · `[!]` blocked

---

## Current status

Bootstrap skeleton only: non-blocking axum server with `/health` and `/` catalog, size-tuned
release profile, one passing test. No CAMARA APIs or auth implemented yet.

**Next up:** Phase 0 — Auth foundation (start with discovery + JWKS + `client_credentials`).

## In progress (claimed this pass)

_None._  <!-- agent: put the claimed item + run timestamp here, clear it when done -->

## Backlog (work top-down; respect phases — see docs/DESIGN.md §12)

### Phase 0 — Auth foundation
- [x] `GET /.well-known/openid-configuration` (discovery) + served metadata
- [ ] `GET /oauth2/jwks` (JWKS) + signing key management
- [ ] `POST /oauth2/token` — `client_credentials` grant (signed JWT, scopes, expiry)
- [ ] Token verification middleware for protected routes (audience/scope/expiry)
- [ ] `GET /oauth2/authorize` + `POST /oauth2/token` — `authorization_code` + PKCE (auto-consent)
- [ ] `POST /bc-authorize` + CIBA token polling
- [ ] Purpose/scope enforcement + shared reserved-identifier scenario convention (DESIGN §7)

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
- [ ] `errors.rs`: CAMARA error model + per-version catalogs (DESIGN §8)
- [ ] `registry.rs`: canonical URL versioning + `/` catalog wiring (DESIGN §9)
- [ ] `specs/…`: vendor + annotate OpenAPI per API/version, serve at `/{api}/v{n}/openapi.yaml`
- [ ] Contract-test harness (validate responses against vendored spec)

---

## Scan journal

Newest first. One line per pass: `YYYY-MM-DD HH:MMZ — <what happened> — binary: <size>`

- 2026-08-02 — Phase 0: implemented `GET /.well-known/openid-configuration` (OIDC discovery).
  New `src/auth/` module; base URL from `CAMARASIM_ISSUER` env or `X-Forwarded-Proto`+`Host`.
  Advertises all 3 CAMARA grants (client_credentials/authorization_code/CIBA), RS256, S256 PKCE,
  CIBA poll. Authored `specs/auth/openapi.yaml`. Catalog now links the discovery doc. 10 tests
  green, no new deps. — binary: 704K (720856 B)
- 2026-08-02 — Repo bootstrapped: docs (DESIGN/AGENT), PROGRESS, axum skeleton (`/health`, `/`),
  size-tuned release profile, first test. Ready for Phase 0.
