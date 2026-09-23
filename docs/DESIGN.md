# CamaraSim — Design & Requirements

A self-hostable **virtual CAMARA operator** for testing telecom API integrations.
Telecom companies, aggregators, and enterprises point their CAMARA client at CamaraSim
instead of a live operator, and get spec-faithful responses, working auth flows, and
deterministic, parameter-driven scenarios — locally or in CI.

---

## 1. Goal

Provide a single, small, fast binary that faithfully implements the
[CAMARA](https://camaraproject.org) standard APIs so integrators can develop and test
against a reference operator without a live network connection.

The product's value is **fidelity and determinism**, not raw throughput: correct auth,
correct error semantics per API version, and the ability to trigger any functional case
on demand from the request input.

## 2. Non-goals

- Not a real network / not connected to any live operator.
- Not a load-testing target (v1). Performance must be *good* (non-blocking, small), but
  correctness wins every trade-off.
- Not multi-node (v1). **Exactly one instance runs**, so all state lives in memory.
- Not a general OAuth product — auth exists only to serve the CAMARA profile.

## 3. Core requirements

### Functional
- Implement CAMARA APIs, working through them **API by API, version by version**.
- Implement the CAMARA auth model, all grant types (see §6).
- Every endpoint exposes its **functional cases** (success variants + error cases) and
  lets the caller **select the case via request parameters** (see §7).
- Follow the CAMARA standard exactly, including the **error model**, and honour
  **per-version** differences (§8).
- Maintain the **OpenAPI spec for every change**, including the documentation of the
  parameter-driven functional cases (§9).

### Non-functional
- **Rust**, async, **non-blocking** end to end (tokio + axum/hyper).
- **Smallest practical binary** (release profile tuned for size — see §11).
- **In-memory state only** (single node assumption). No external DB.
- **Every change covered by tests** (§10). `main` is always green.
- Self-contained: `cargo run` starts a working operator with no external services.

## 4. Tech stack

| Concern | Choice | Why |
|---|---|---|
| Language | Rust (stable) | Small static binary, no GC, deterministic tail latency |
| Async runtime | tokio | De-facto non-blocking runtime |
| HTTP | axum (on hyper/tower) | Ergonomic, non-blocking, small, strong ecosystem |
| Serialization | serde / serde_json | Standard |
| Auth / JWT | `jsonwebtoken` (+ `rsa`/`p256` as needed) | Sign/verify tokens, JWKS |
| OpenAPI | vendored CAMARA YAML, served + contract-tested | Spec is source of truth (§9) |
| State | in-memory (`tokio::sync`/`dashmap` or `RwLock<HashMap>`) | Single-node assumption |

## 5. Architecture

Monolith, one binary, layered into small modules that can grow independently:

```
src/
  main.rs            # bootstrap: build router, bind, serve (non-blocking)
  server.rs          # router composition, middleware, error mapping
  auth/              # OIDC/OAuth: discovery, JWKS, token, authorize, CIBA
  registry.rs        # API + version registry -> canonical versioning (§?)
  scenario.rs        # parameter -> functional-case resolution (§7)
  errors.rs          # CAMARA error model, per-version catalogs (§8)
  state.rs           # in-memory stores (OTP, QoD sessions, etc.)
  apis/
    sim_swap/v2/...  # one module per API per major version
    number_verification/v1/...
    ...
specs/
  <api>/<version>/openapi.yaml   # vendored CAMARA spec, annotated (§9)
```

Each API module is self-contained: handlers, request/response types, scenario mapping,
and tests. Adding an API/version = adding a module + its vendored spec + tests + a
registry entry. Nothing else changes.

## 6. Auth model

CAMARA uses OIDC per the *Security & Interoperability Profile*. Implement all grant types:

- **`client_credentials`** — 2-legged, for APIs that don't need an end-user context.
- **`authorization_code` + PKCE** — 3-legged, user consent via the authorize endpoint.
- **CIBA** (`urn:openid:params:grant-type:ciba`) — backchannel auth (`/bc-authorize`
  then token polling / notification).
- **Purpose-based scopes** — e.g. `dpv:FraudPreventionAndDetection#check-sim-swap`.

Endpoints:
- `/.well-known/openid-configuration` — discovery
- `/oauth2/jwks` — signing keys (JWKS)
- `/oauth2/token` — token endpoint (all grants)
- `/oauth2/authorize` — authorization endpoint (auto-consent in sim mode)
- `/bc-authorize` — CIBA backchannel

Tokens are real, signed JWTs the simulator verifies on protected endpoints. In sim mode,
the authorize/consent steps auto-approve so flows can run headlessly, but the **shape and
validation of the flow are faithful** (scopes, audience, PKCE, expiry all enforced).

## 7. Functional cases — parameter-driven

Every endpoint must let the caller pick which functional case to exercise, driven by the
**input identifier** (phone number, device id, etc.), so success variants *and* every
error case are reachable deterministically with no hidden config.

Principle: **the input is the control plane.** A reserved convention maps identifier
patterns to outcomes. Defaults are "happy path"; reserved patterns trigger variants.

Example — **SIM Swap** (`phoneNumber`):
- Default number → `swapped: false` (or `lastSwapDate` far in the past).
- Number matching a "recently swapped" pattern → `swapped: true` within the requested period.
- **Reserved error suffixes** (shared convention across identifier-keyed APIs), e.g. last
  three digits select the CAMARA error: `...400`, `...401`, `...403`, `...404`, `...409`,
  `...422`, `...429`, `...500`, `...503`.

The exact convention is defined once and reused across APIs, and — critically — is
**documented in each endpoint's OpenAPI description** so users discover cases from the
spec alone. Where an API's case space is small, cover them all; where it's large, cover
the main success variants + the standard error set and note the cut in the spec.

## 8. Error model & versioning of behaviour

CAMARA error body:
```json
{ "status": 404, "code": "IDENTIFIER_NOT_FOUND", "message": "..." }
```
- Implement the full standard status/code set (400/401/403/404/409/422/429/5xx) with the
  CAMARA `code` enums.
- Error codes and response shapes are **version-dependent**. Each API version carries its
  own error catalog and response schema; behaviour must match the version in the URL.

## 9. Canonical versioning & OpenAPI policy

**URL versioning (canonical).** Every API is mounted at:
```
/{api-name}/v{MAJOR}/{resource}
```
e.g. `/sim-swap/v2/check`, `/number-verification/v1/verify`. Multiple major versions of
the same API are served concurrently, each backed by its own module + spec + error catalog.

Discovery endpoints:
- `GET /` — catalog of all mounted APIs and versions.
- `GET /{api}/v{n}/openapi.yaml` — the served spec for that version.
- `GET /{api}/v{n}/docs` — human-readable docs for that version.

**OpenAPI is the source of truth.** For each API version:
1. Vendor the official CAMARA `openapi.yaml` (from its release tag) under `specs/…`.
2. Annotate it with the parameter-driven functional cases from §7 (in endpoint
   `description` and/or `x-camarasim-scenarios`).
3. Implement handlers to conform to that spec.
4. A **contract test** validates the implementation's responses against the spec.

Any code change that alters a request/response/behaviour **must** update the vendored
spec in the same change. The spec and the server never drift.

## 10. Testing policy

- **Every change is covered by a test.** No exceptions; `main` stays green.
- Unit tests per handler/scenario mapping.
- Integration tests hit the router (in-process, non-blocking) and assert status + body.
- Contract tests assert responses conform to the vendored OpenAPI (§9).
- Auth tests cover each grant type and the failure modes (bad scope, expired, PKCE).

## 11. Build / binary size

Release profile tuned for size (see `Cargo.toml`):
```toml
[profile.release]
opt-level = "z"     # optimise for size
lto = true          # link-time optimisation
codegen-units = 1   # better cross-module optimisation
panic = "abort"     # drop unwinding tables
strip = true        # strip symbols
```
Keep the dependency tree lean (every crate adds bytes). Prefer `rustls` over OpenSSL to
stay static and small. Measure binary size and note regressions.

## 12. Roadmap — stateless & non-spatial first

Ordered so early passes are simple and self-contained, deferring state and geospatial work.

- **Phase 0 — Auth foundation.** Discovery, JWKS, `client_credentials`, then
  `authorization_code`+PKCE, then CIBA. Scope/purpose enforcement + error model + shared
  scenario convention (§7).
- **Phase 1 — Stateless, non-spatial (identity/number).**
  Number Verification → SIM Swap → KYC Match. (Best showcase of parameter-driven cases.)
- **Phase 2 — Stateless device queries.** Device Status (reachability / roaming),
  Device Identifier.
- **Phase 3 — Stateful, non-spatial.** One-Time-Password SMS (send/validate — intro to
  in-memory state), then Quality on Demand (session lifecycle + CloudEvents notifications).
- **Phase 4 — Spatial.** Device Location Verification → Retrieval → Geofencing.
- **Phase 5 — Remaining.** Carrier Billing / Payments and other APIs as capacity allows.
