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
- ✅ Resource-server temporal validation (`src/auth/verify.rs`): `exp` enforced;
  `nbf` (not-before, RFC 7519 §4.1.5) now enforced when present → 401
  `UNAUTHENTICATED` (`AuthError::NotYetValid`), documented in `specs/auth`. Stale
  module comment claiming no business endpoint consumes `Claims` corrected (60 do).
  Still to verify: `client_credentials`/`authorization_code`+PKCE/CIBA grant paths
  and scope/purpose mapping end to end.

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
2. **Audit Phase 1–3 stable APIs** one at a time: confirm the full CAMARA error
   set and the parameter-driven scenario convention (DESIGN §7) are implemented
   and documented in each endpoint's vendored spec, with contract tests.
3. **Deepen `vwip` APIs** (🔧) one endpoint/scenario at a time, in mount order,
   only after the earlier phases are verified.

> The above is a reconstructed starting point. As each API is verified, replace
> its bullet with concrete, checked-off sub-steps.

## Scan journal

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
</content>
