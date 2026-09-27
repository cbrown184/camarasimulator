# PROGRESS.md — CamaraSim backlog & scan journal

Working tracker for the build agent. See `docs/AGENT.md` for the workflow and
`docs/DESIGN.md` for requirements. Phase order (DESIGN §12) is authoritative: do
Phase 0 auth first, then stateless & non-spatial APIs before stateful/spatial.

> **Reconstruction notice (2026-09-26).** This file and `docs/AGENT.md` were
> missing from the repository and have been reconstructed from `docs/DESIGN.md`,
> the API registry (`src/registry.rs`), and the on-disk module/spec layout. The
> **mounted** inventory below is authoritative (taken from the registry); the
> **completeness** of each API's scenario set, per-version error catalog, and
> contract-test coverage was NOT re-audited in this pass and is the standing
> backlog. Future passes should verify one API at a time and correct this file.

## Status legend

- ✅ mounted at a stable/numbered version — treat as implemented; verify depth.
- 🔧 mounted at CAMARA `vwip` (work-in-progress) version — implemented at wip
  version; most likely to need scenario/error/contract deepening.
- ⬜ not yet mounted.

## Inventory (from `src/registry.rs`, 61 APIs mounted, in mount order)

### Phase 0 — Auth foundation
Auth modules present under `src/auth/` (`token`, `authorize`, `ciba`, `codes`,
`keys`, `verify`, `purpose`) covering discovery, JWKS, `client_credentials`,
`authorization_code`+PKCE, and CIBA. **Backlog:** re-verify scope/purpose
enforcement, PKCE, audience, and expiry against DESIGN §6, and confirm auth tests
cover each grant type and failure mode.

Verified sub-items:
- ✅ Client authentication methods now match discovery: discovery + `specs/auth`
  advertise `private_key_jwt` in `token_endpoint_auth_methods_supported`, and the
  token endpoint (all three grants) and `/bc-authorize` now honour it —
  `client_id_from_assertion` (`src/auth/token.rs`) reads the client id from a
  `client_assertion` JWT's `sub` (falling back to `iss`), signature unverified,
  matching the "any secret accepted" test-double posture (DESIGN §6). Precedence:
  `client_secret_basic` → `client_secret_post` → `private_key_jwt`. Spec request
  bodies (`TokenRequest`, `BackchannelAuthenticationRequest`) gained
  `client_assertion`/`client_assertion_type`. 6 new tests. No new dependency.
- ✅ Discovery advertises `token_endpoint_auth_signing_alg_values_supported`
  (`["RS256"]`). OIDC Discovery 1.0 §3 makes this metadata entry mandatory once
  `private_key_jwt` is listed in `token_endpoint_auth_methods_supported` (which it
  is), and `none` MUST NOT appear. Added to `metadata()` (`src/auth/mod.rs`) and
  the vendored `specs/auth` discovery example + `OpenIdConfiguration` schema in the
  same pass. 1 new test. No new dependency.
- ✅ Resource-server temporal validation (`src/auth/verify.rs`): `exp` enforced;
  `nbf` (not-before, RFC 7519 §4.1.5) now enforced when present → 401
  `UNAUTHENTICATED` (`AuthError::NotYetValid`), documented in `specs/auth`. Stale
  module comment claiming no business endpoint consumes `Claims` corrected (60 do).
  Still to verify: `client_credentials`/`authorization_code`+PKCE/CIBA grant paths
  and scope/purpose mapping end to end.
- ✅ Resource-server issuer validation (RFC 9068 §4): `verify_token`
  (`src/auth/verify.rs`) now requires the JWT `iss` claim to exactly match this
  deployment's issuer identifier (the request-derived base URL, the same value the
  token endpoint stamps as `iss` on every grant). A correctly signed, unexpired
  token whose `aud` matches but whose `iss` is foreign or absent is rejected 401
  `UNAUTHENTICATED` (`AuthError::WrongIssuer`) — the issuer check is orthogonal to
  the audience check (`aud` = intended recipient, `iss` = trusted minter). The
  extractor passes the base URL as both expected issuer and audience.
  `verify_token` gained an `expected_issuer` parameter. `specs/auth` middleware
  checklist, securityScheme note, and 401 `Unauthenticated` description updated in
  the same pass. 3 new tests. No new dependency.
- ✅ Resource-server explicit typing (RFC 9068 §4): `verify_token`
  (`src/auth/verify.rs`) now requires the JWT `typ` header be `at+jwt` (or
  `application/at+jwt`, matched case-insensitively) — a well-formed but wrong-type
  JWT (an ID token, a `private_key_jwt` client assertion) is rejected 401
  `UNAUTHENTICATED` (`AuthError::WrongType`), closing the RFC 8725 §3.11
  cross-JWT-confusion gap. The token endpoint already stamps `typ: at+jwt`, so no
  issued token is affected. `specs/auth` middleware checklist + 401 description
  updated in the same pass. 4 new tests. No new dependency. **Auth grant paths
  (`client_credentials`, `authorization_code`+PKCE, CIBA) reviewed this pass and
  found faithful & well-tested; scope/purpose *grammar* gating confirmed at all
  three request points.** Remaining Phase 0 verify item: end-to-end scope→purpose
  mapping documentation per endpoint.

### Phase 1 — Stateless, non-spatial (identity/number)
- ✅ number-verification v1
- ✅ sim-swap v2
- ✅ kyc-match v0.3

### Phase 2 — Stateless device queries
- ✅ device-reachability-status v1
- ✅ device-roaming-status v1
- ✅ device-identifier v0.3

### Phase 3 — Stateful, non-spatial
- ✅ one-time-password-sms v1  (in-memory OTP store)
- ✅ quality-on-demand v1  (session lifecycle + CloudEvents notifications)

### Phase 4 — Spatial
- ✅ location-verification v3
- ✅ location-retrieval v0.4
- ✅ geofencing-subscriptions v0.4

### Phase 5 — Remaining APIs (stable/numbered)
- ✅ carrier-billing v0.5
- ✅ call-forwarding-signal v0.4
- ✅ number-recycling v0.2
- ✅ kyc-age-verification v0.1
- ✅ device-swap v1
- ✅ kyc-fill-in v0.3
- ✅ home-devices-qod v0.4
- ✅ qos-profiles v1
- ✅ kyc-tenure v0.2
- ✅ blockchain-public-address v0.3
- ✅ simple-edge-discovery v2
- ✅ customer-insights v0.2
- ✅ connected-network-type v0.2
- ✅ connectivity-insights v0.6
- ✅ region-device-count v0.2
- ✅ qos-provisioning v0.3
- ✅ sms v0alpha1
- ✅ in-home-device-management v1

### Phase 5 — Remaining APIs (CAMARA `vwip` — deepen scenarios/errors/contracts)
- 🔧 device-data-volume
- 🔧 device-visit-location
- 🔧 population-density-data
- 🔧 qos-booking
- 🔧 media-streaming-rate
- 🔧 network-health-assessment
- 🔧 network-traffic-analysis
- 🔧 optimal-edge-discovery
- 🔧 verified-caller
- 🔧 application-profiles
- 🔧 subscription-status
- 🔧 device-authenticity
- 🔧 session-insights
- 🔧 consent-info
- 🔧 iot-sim-fraud-prevention
- 🔧 sponsored-data
- 🔧 click-to-dial
- 🔧 most-frequent-location
- 🔧 traffic-influence
- 🔧 application-endpoint-discovery
- 🔧 application-endpoint-registration
- 🔧 predictive-connectivity-data
- 🔧 network-access-devices
- 🔧 capabilities-and-restrictions
- 🔧 dedicated-network-profiles
- 🔧 dedicated-network
- 🔧 dedicated-network-accesses
- 🔧 dedicated-network-areas
- 🔧 edge-application-management
- 🔧 esim-remote-management
- 🔧 network-access-domains

## Backlog (unclaimed, top-first)

Respect phase order. Preferred next work:

1. **Verify Phase 0 auth** end to end (grant types, PKCE, scope/purpose, expiry,
   JWKS, discovery) and record any gaps here as concrete sub-items.
   - ✅ End-to-end grant→resource-server seam proven on number-verification v1: a
     token minted via the full `authorization_code`+PKCE flow and via CIBA is now
     tested to be accepted by a protected **business** endpoint (aud/iss/typ/scope
     carried through the whole flow), and a three-legged token lacking the API scope
     is 403 at the endpoint. Previously all 60 business modules tested with
     `client_credentials` tokens only, so the issuance↔verification seam for the two
     interactive grants was never exercised past the token endpoint. 4 new tests, no
     behaviour/spec change, no new dependency.
   - ✅ Seam check repeated on a second three-legged API, sim-swap v2
     (`src/apis/sim_swap/v2.rs`): a token minted via the full
     `authorization_code`+PKCE flow and via CIBA is accepted by `POST /check`
     (200), a three-legged token granted only `openid` is 403 `PERMISSION_DENIED`
     at the endpoint, and — with no `phoneNumber` in the body — the synthetic
     `camarasim-user` subject survives issuance → verification and drives the
     three-legged identifier fallback (no trailing digits → `swapped=false`). 4 new
     tests, no behaviour/spec change, no new dependency. *Remaining:* document
     end-to-end scope→purpose mapping per endpoint.
2. **Audit Phase 1–3 stable APIs** one at a time: confirm the full CAMARA error
   set and the parameter-driven scenario convention (DESIGN §7) are implemented
   and documented in each endpoint's vendored spec, with contract tests.
3. **Deepen `vwip` APIs** (🔧) one endpoint/scenario at a time, in mount order,
   only after the earlier phases are verified.

> The above is a reconstructed starting point. As each API is verified, replace
> its bullet with concrete, checked-off sub-steps.

## Scan journal

- 2026-09-27 — Phase 0 auth pass: repeated the end-to-end grant→resource-server
  seam check on a second three-legged API, sim-swap v2 (`src/apis/sim_swap/v2.rs`).
  The seam was previously proven only on number-verification v1; every other
  business module (incl. sim-swap) tested with `client_credentials` tokens, so the
  two interactive grants were never exercised past the token endpoint here. Added 4
  integration tests that drive both grants through the router (authorize→token with
  the RFC 7636 PKCE vector; `/bc-authorize`→CIBA token grant), then call
  `POST /sim-swap/v2/check`: a three-legged token and a CIBA token are accepted
  (200, `swapped=true` for a recent-swap tail), a three-legged token granted only
  `openid` is 403 `PERMISSION_DENIED` at the endpoint, and — with no `phoneNumber`
  in the body — the synthetic `camarasim-user` subject survives issuance →
  verification and drives sim-swap's three-legged identifier fallback (no trailing
  digits → `swapped=false`). Test-only change: no behaviour/spec change, no new
  dependency. `cargo test` green: 3055 passed, 0 failed (+4). `cargo build --release`
  green; binary = 5,330,648 bytes (unchanged). *Note:* this session began on a
  detached HEAD at the tip of prior work while local `main` lagged four commits
  behind `origin/main`; synced local `main` to `origin/main` (which already carried
  that work) before starting — no lost commits.
- 2026-09-27 — Phase 0 auth pass: proved the end-to-end grant→resource-server seam.
  The three grant paths were unit-tested at the token endpoint, and business
  endpoints were tested with `client_credentials` tokens, but nothing wired the two
  together — no test showed a token minted via the full `authorization_code`+PKCE
  flow or CIBA being accepted by a protected business endpoint (its `aud`/`iss`/`typ`
  /`scope` surviving issuance → resource-server verification). Added 4 integration
  tests to number-verification v1 (`src/apis/number_verification/v1.rs`) that drive
  both interactive grants through the router (authorize→token redemption with the
  RFC 7636 PKCE vector; `/bc-authorize`→CIBA token grant), then call `POST /verify`
  and `GET /device-phone-number`: a three-legged token and a CIBA token are accepted
  (200), a three-legged token granted only `openid` is 403 `PERMISSION_DENIED` at the
  endpoint, and the synthetic `camarasim-user` subject yields the default device
  line. Test-only change: no behaviour/spec change, no new dependency. `cargo test`
  green: 3051 passed, 0 failed (+4). `cargo build --release` green; binary =
  5,330,648 bytes (unchanged).
- 2026-09-26 — Phase 0 auth pass: enforced RFC 9068 §4 issuer validation in the
  resource-server verifier (`src/auth/verify.rs`). The verifier checked `alg`,
  `typ`, signature, `exp`, `nbf`, and `aud` but never validated `iss`, the last
  RFC 9068 §4 check missing. It now requires the JWT `iss` to exactly match the
  deployment's issuer identifier (the request-derived base URL, the same value the
  token endpoint stamps as `iss` on all three grants); a token whose `aud` matches
  but whose `iss` is foreign or absent is rejected 401 `UNAUTHENTICATED`
  (new `AuthError::WrongIssuer`). The check is orthogonal to the audience check —
  `aud` proves intended recipient, `iss` proves trusted minter. `verify_token`
  gained an `expected_issuer` parameter; the extractor passes the base URL as both
  expected issuer and audience. Updated the `specs/auth` middleware checklist, the
  securityScheme enforcement note, and the 401 `Unauthenticated` description in the
  same pass. 3 new tests. `cargo test` green: 3047 passed, 0 failed.
  `cargo build --release` green; binary = 5,330,648 bytes (−128 B vs. prior, no new
  dependency).
- 2026-09-26 — Phase 0 auth pass: enforced RFC 9068 §4 explicit typing in the
  resource-server verifier (`src/auth/verify.rs`). The verifier pinned `alg=RS256`
  but never checked the JWT `typ`; it now requires `at+jwt` (or `application/at+jwt`,
  case-insensitive) and rejects anything else 401 `UNAUTHENTICATED`
  (new `AuthError::WrongType`), so a JWT minted for another purpose (an ID token, a
  `private_key_jwt` client assertion) cannot be replayed as an access token
  (RFC 8725 §3.11 cross-JWT confusion). All issued tokens already carry
  `typ: at+jwt`, so none is affected. Updated the `specs/auth` middleware checklist,
  the securityScheme note, and the 401 `Unauthenticated` description in the same
  pass. Reviewed the three grant paths (`client_credentials`, `authorization_code`
  +PKCE, CIBA) and the purpose-scope grammar gating — found faithful and
  well-tested. 4 new tests. `cargo test` green: 3044 passed, 0 failed.
  `cargo build --release` green; binary = 5,330,776 bytes (+192 B, no new dependency).
- 2026-09-26 — Phase 0 auth pass: closed an OIDC discovery conformance gap.
  Discovery advertised `private_key_jwt` in `token_endpoint_auth_methods_supported`
  but omitted `token_endpoint_auth_signing_alg_values_supported`, which OIDC
  Discovery 1.0 §3 REQUIRES whenever `private_key_jwt`/`client_secret_jwt` is
  advertised (and where `none` MUST NOT appear). Added the field as `["RS256"]` to
  `metadata()` (`src/auth/mod.rs`) — matching the bundled RS256 signing/verification
  key — and updated the vendored `specs/auth` discovery example and the
  `OpenIdConfiguration` schema in the same pass. 1 new test. `cargo test` green:
  3040 passed, 0 failed. `cargo build --release` green; binary = 5,330,584 bytes
  (+64 B, no new dependency).
- 2026-09-26 — Bootstrap pass: `docs/AGENT.md` and `PROGRESS.md` were missing;
  reconstructed both from `docs/DESIGN.md`, `src/registry.rs`, and the on-disk
  layout. Inventoried 61 mounted APIs by phase. No code/spec behaviour changed.
  Baseline `cargo test` green: 3030 passed, 0 failed. `cargo build --release`
  green; release binary `target/release/camarasimulator` = 5,323,160 bytes
  (5.1 MB). Per-API completeness still to be audited in future passes.
- 2026-09-26 — Phase 0 auth pass: added `nbf` (not-before) validation to the
  resource-server verifier (`src/auth/verify.rs`), faithful to RFC 7519 §4.1.5 /
  RFC 9068 — a token with a future `nbf` is now rejected 401 `UNAUTHENTICATED`
  (new `AuthError::NotYetValid`); absent `nbf` unaffected. Updated `specs/auth`
  middleware checklist + 401 description in the same pass; corrected the stale
  `verify.rs` comment (60 mounted APIs consume `Claims`) and replaced the blanket
  `#![allow(dead_code)]` with a targeted allow on `client_id`. 3 new tests.
  `cargo test` green: 3033 passed, 0 failed. `cargo build --release` green;
  binary = 5,323,352 bytes (+192 B, no new dependency).
- 2026-09-26 — Phase 0 auth pass: made the advertised `private_key_jwt` token
  endpoint auth method real. Discovery + `specs/auth` listed `private_key_jwt` in
  `token_endpoint_auth_methods_supported`, but only `client_secret_basic`/`_post`
  were honoured. Added `client_id_from_assertion` (`src/auth/token.rs`) resolving
  the client id from a `client_assertion` JWT (`sub`, else `iss`; signature not
  verified — "authenticate the shape, not credentials", DESIGN §6) and wired it as
  a third client-auth fallback into all three token grants and `/bc-authorize`
  (`src/auth/ciba.rs`). Updated the vendored spec's endpoint descriptions, the 401
  example, and both request-body schemas (`client_assertion`/`client_assertion_type`)
  in the same pass. 6 new tests. `cargo test` green: 3039 passed, 0 failed.
  `cargo build --release` green; binary = 5,330,520 bytes (+7,168 B, no new dependency).
</content>
