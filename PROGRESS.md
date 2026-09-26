# PROGRESS.md — backlog, status & scan journal

Operating rules live in [`docs/AGENT.md`](docs/AGENT.md); requirements in
[`docs/DESIGN.md`](docs/DESIGN.md). Each scheduled firing does **one small, verifiable
pass** (see AGENT §1): pick the top *unclaimed* item respecting phase order, scope it to
one pass, implement + spec + test, verify `cargo test` and `cargo build --release` green,
land it on a branch and merge to `main`, then record it here.

**Status legend:** `[ ]` todo · `[~]` in-progress (claimed) · `[x]` done ·
`[·]` scaffolded, needs version-pinning / verification.

> **Note on the baseline.** The repository's initial commit already contains a broad
> implementation: **63 API modules mounted, all three OAuth grant types, and 3030 passing
> tests** (measured 2026-09-26). This backlog therefore tracks (a) pinning the many
> work-in-progress (`vwip`) APIs to their released CAMARA versions, and (b) an ongoing
> audit of the already-pinned APIs against their released specs. Treat a `[x]` here as
> "implemented, mounted, and covered by tests" — not "audited line-by-line against the
> upstream release." Re-auditing a pinned API against its latest CAMARA spec is a valid
> backlog item; when it turns up a gap, add a specific sub-item for the fix.

---

## Phase 0 — Auth foundation (DESIGN §6)

- [x] Discovery `/.well-known/openid-configuration`
- [x] JWKS `/oauth2/jwks` (RS256 signing key)
- [x] Token endpoint `/oauth2/token` — `client_credentials`
- [x] Token endpoint — `authorization_code` + PKCE (`/oauth2/authorize`)
- [x] CIBA — `/bc-authorize` + token polling (`urn:openid:params:grant-type:ciba`)
- [x] Scope / purpose enforcement + error model + shared scenario convention
      (`src/scenarios.rs`, `src/errors.rs`, `src/auth/purpose.rs`, `src/auth/verify.rs`)
- [ ] **Audit backlog:** re-verify each grant's failure modes (bad scope, expired,
      PKCE mismatch, audience) against the CAMARA Security & Interoperability Profile;
      add any missing negative tests. *(one grant per pass)*

## Phase 1 — Stateless, non-spatial: identity / number (DESIGN §12)

- [x] Number Verification v1 — `/number-verification/v1/…`
- [x] SIM Swap v2 — `/sim-swap/v2/…`
- [x] KYC Match v0.3 — `/kyc-match/v0.3/…`
- [x] Number Recycling v0.2 — `/number-recycling/v0.2/…`
- [x] KYC Age Verification v0.1 — `/kyc-age-verification/v0.1/…`
- [x] KYC Fill-in v0.3 — `/kyc-fill-in/v0.3/…`
- [x] KYC Tenure v0.2 — `/kyc-tenure/v0.2/…`
- [x] Blockchain Public Address v0.3 — `/blockchain-public-address/v0.3/…`
- [x] Customer Insights v0.2 — `/customer-insights/v0.2/…`

## Phase 2 — Stateless device queries

- [x] Device Reachability Status v1 — `/device-reachability-status/v1/…`
- [x] Device Roaming Status v1 — `/device-roaming-status/v1/…`
- [x] Device Identifier v0.3 — `/device-identifier/v0.3/…`
- [x] Device Swap v1 — `/device-swap/v1/…`
- [x] Connected Network Type v0.2 — `/connected-network-type/v0.2/…`
- [x] Connectivity Insights v0.6 — `/connectivity-insights/v0.6/…`
- [x] Simple Edge Discovery v2 — `/simple-edge-discovery/v2/…`
- [·] Device Data Volume — `vwip` → pin to released version
- [·] Device Authenticity — `vwip` → pin to released version
- [·] Verified Caller — `vwip` → pin to released version
- [·] Subscription Status — `vwip` → pin to released version
- [·] Consent Info — `vwip` → pin to released version
- [·] Capabilities and Restrictions — `vwip` → pin to released version

## Phase 3 — Stateful, non-spatial

- [x] One-Time-Password SMS v1 — `/one-time-password-sms/v1/…`
- [x] Quality on Demand v1 — `/quality-on-demand/v1/…` (session lifecycle)
- [x] QoS Profiles v1 — `/qos-profiles/v1/…`
- [x] QoS Provisioning v0.3 — `/qos-provisioning/v0.3/…`
- [x] Home Devices QoD v0.4 — `/home-devices-qod/v0.4/…`
- [x] In-Home Device Management v1 — `/in-home-device-management/v1/…`
- [x] Carrier Billing v0.5 — `/carrier-billing/v0.5/…`
- [x] Call Forwarding Signal v0.4 — `/call-forwarding-signal/v0.4/…`
- [x] Short Message Service v0alpha1 — `/sms/v0alpha1/…`
- [·] QoS Booking — `vwip` → pin to released version
- [·] Session Insights — `vwip` → pin to released version
- [·] Sponsored Data — `vwip` → pin to released version
- [·] Click to Dial — `vwip` → pin to released version
- [·] Traffic Influence — `vwip` → pin to released version
- [·] IoT SIM Fraud Prevention — `vwip` → pin to released version
- [·] eSIM Remote Management — `vwip` → pin to released version
- [·] Media Streaming Rate — `vwip` → pin to released version
- [·] Network Traffic Analysis — `vwip` → pin to released version
- [·] Network Health Assessment — `vwip` → pin to released version
- [·] Predictive Connectivity Data — `vwip` → pin to released version

## Phase 4 — Spatial

- [x] Location Verification v3 — `/location-verification/v3/…`
- [x] Location Retrieval v0.4 — `/location-retrieval/v0.4/…`
- [x] Geofencing Subscriptions v0.4 — `/geofencing-subscriptions/v0.4/…`
- [x] Region Device Count v0.2 — `/region-device-count/v0.2/…`
- [·] Device Visit Location — `vwip` → pin to released version
- [·] Population Density Data — `vwip` → pin to released version
- [·] Most Frequent Location — `vwip` → pin to released version

## Phase 5 — Edge / dedicated network / discovery (remaining)

- [·] Optimal Edge Discovery — `vwip` → pin to released version
- [·] Edge Application Management — `vwip` → pin to released version
- [·] Application Profiles — `vwip` → pin to released version
- [·] Application Endpoint Discovery — `vwip` → pin to released version
- [·] Application Endpoint Registration — `vwip` → pin to released version
- [·] Dedicated Network — `vwip` → pin to released version
- [·] Dedicated Network Accesses — `vwip` → pin to released version
- [·] Dedicated Network Areas — `vwip` → pin to released version
- [·] Dedicated Network Profiles — `vwip` → pin to released version
- [·] Network Access Devices — `vwip` → pin to released version
- [·] Network Access Domains — `vwip` → pin to released version

## Cross-cutting / standing backlog

- [ ] Contract tests: assert each mounted API's responses conform to its vendored spec
      (DESIGN §9.4), one API per pass where not yet covered.
- [ ] Verify `x-camarasim-scenarios` / endpoint `description` documents every
      parameter-driven case for each API (DESIGN §7), one API per pass.
- [ ] Track and minimise release binary size on every pass (DESIGN §11).

---

## Scan journal

One line per pass: date · branch · what landed · `cargo test` result · release binary size.

- 2026-09-26 · `agent/docs/bootstrap-harness` · Bootstrapped the operating harness:
  added `docs/AGENT.md` (pass procedure & rules) and this `PROGRESS.md` (phase-ordered
  backlog + journal) reflecting the initial-commit baseline. Docs-only, no code change ·
  `cargo test`: 3030 passed, 0 failed · release binary: 5.1 MiB (5,323,160 bytes).
</content>
