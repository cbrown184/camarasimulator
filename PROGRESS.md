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
- 🔧 device-data-volume — verified 2026-10-12: both operations
  (`retrieveDataVolume`, `checkDataVolume`) implemented, tested, and documented in
  `specs/device-data-volume/vwip/openapi.yaml`; stale "`/check` is a later slice"
  module doc corrected.
- 🔧 device-visit-location — verified 2026-10-12: the one modelled operation
  (`retrieveDeviceVisitLocation`, `POST /device-visit-location/vwip/retrieve`) is
  implemented, tested (~25 test fns), and documented in
  `specs/device-visit-location/vwip/openapi.yaml`; spec ↔ code ↔ module-doc
  consistent (success example, reserved-suffix mapping, and identifier-resolution
  422s all match), no drift found, no change needed.
- 🔧 population-density-data — verified 2026-10-12: the one modelled operation
  (`retrievePopulationDensity`, `POST /population-density-data/vwip/retrieve`) is
  implemented and well-tested. **Spec↔code drift found and fixed:** the vendored
  `200` success example claimed `u4pruy` → `pplDensity 4213` (band 3792/4634) and
  `gbsuv` → `NO_DATA` with status `PART_OF_AREA_NOT_SUPPORTED`, but the code's
  deterministic FNV-1a mapping actually returns `u4pruy` → 2837 (2554/3120) and
  `gbsuv` → `LOW_DENSITY` (so status `SUPPORTED_AREA`). Corrected the example to
  the real values (`u4pruy` 2837, and `dr5bb` → `NO_DATA` to keep the illustration
  partly-supported) and added a `vendored_success_example_matches_the_mapping` test
  that pins it so it can't drift again. All error-case examples were already
  byte-accurate.
- 🔧 qos-booking — verified 2026-10-12: all four modelled operations
  (`createBooking`, `getBooking`, `deleteBooking`, `retrieveBookingByDevice`)
  are implemented, tested, and documented in `specs/qos-booking/vwip/openapi.yaml`;
  spec ↔ code consistent (create `…012`/`…013`/`…000` examples match the
  ACTIVATED/SCHEDULED/REQUESTED derivation, which dedicated tests already pin;
  all 400/409/422 error examples byte-match the handler). **Stale parent
  module doc fixed:** `src/apis/qos_booking.rs` claimed "First slice:
  `createBooking`" though all four operations ship — corrected to list them
  (same trap as device-data-volume's "later slice" doc).
- 🔧 media-streaming-rate — verified 2026-10-12: the one modelled operation
  (`retrieveMaximumDownstreamMediaRate`,
  `POST /media-streaming-rate/vwip/retrieve-maximum-downstream-media-rate`) is
  implemented and well-tested. **Spec↔code drift found and fixed in two vendored
  error examples** (`specs/media-streaming-rate/vwip/openapi.yaml`): the `404`
  example claimed `message: "The specified resource was not found."` but
  `CamaraError::for_status(404)` emits "…is not found." (was→is), and the `422`
  `missingIdentifier` example claimed the truncated "The device cannot be
  identified." but the handler emits "The device cannot be identified: supply a
  `device` or use a token that identifies a device." Corrected both examples to
  the real messages and added a `vendored_error_example_messages_match_the_handler`
  test pinning the exact 400/404/422 message bytes so they can't drift again. The
  success examples (`…000`→0 bps, `…002`→2 Mbps) and the 400/UNNECESSARY_IDENTIFIER
  examples already matched. Spec + test change (no behaviour change — the handler
  was already correct).
- 🔧 network-health-assessment — verified 2026-10-12: the one modelled operation
  (`getHealthScores`, `GET /network-health-assessment/vwip/health-scores`) is
  implemented, tested, and documented in
  `specs/network-health-assessment/vwip/openapi.yaml`; spec ↔ code ↔ module-docs
  consistent (both success examples `…950`/NET→95.0 and `…950`/NET_CORE→92.0 and
  the `…000`→null no-data example match the `score = clamp(d/10 − moduleIndex, 0,
  100)` mapping, already pinned by tests; the reserved-suffix set the spec lists
  — 400/401/403/404/409/422/429/500/503 — matches `scenarios::RESERVED_ERROR_SUFFIXES`
  exactly). No drift found. Added one test pinning the vendored 400
  `invalidArgument` example message bytes (the one spec example no test guarded)
  so it can't drift from the handler.
- 🔧 network-traffic-analysis — verified 2026-10-12: the one modelled operation
  (`getTrafficAnalysis`, `GET /network-traffic-analysis/vwip/traffic-analysis`) is
  implemented and well-tested. **Spec↔code drift found and fixed:** the vendored
  `200` `success` example claimed a `…012` tail yields **two** applications
  (whatsapp + wechat, `totalCount: 2`), but the code's deterministic mapping
  `n_apps = ((d % 5) + 1)` gives `d=12 → 3` apps — the example was missing the
  third record (netflix, `accessCount 39`, flows 39936/79872/119808) and had the
  wrong `totalCount`. Corrected the example to the real three-record output and
  added a `vendored_success_example_matches_the_mapping` test pinning the exact
  documented records + `totalCount: 3` so it can't drift again. All 400 error
  examples already matched the handler.
- 🔧 optimal-edge-discovery — verified 2026-10-12: both modelled operations
  (`discoverOptimalEdge`, `POST …/retrieve-optimal-edge-cloud-zones`; `getRegions`,
  `GET …/regions`) implemented, routed, and well-tested. **Spec↔code drift found
  and fixed in three vendored examples** (`specs/optimal-edge-discovery/vwip/openapi.yaml`):
  (1) the 200 `single`/`ranked` examples carried placeholder `edgeCloudZoneId`s
  (`00000000-…`, `11111111-…`) though the handler emits deterministic SHA-256-derived
  ids (`d0581b5b-…`, `e3a79eed-…`, `a03b2bf0-…`); (2) the 404 example said "…was not
  found." but `CamaraError::for_status(404)` emits "…is not found." (same drift as
  media-streaming-rate); (3) the 422 `missingIdentifier` example carried the truncated
  "The device cannot be identified." but the handler emits the full "…: supply a
  `device` or use a token…". Corrected all three and added
  `vendored_examples_match_the_handler` pinning the exact bytes. The success schemas,
  the `getRegions` catalog example, and the 400/`unnecessaryIdentifier` examples
  already matched.
- 🔧 verified-caller — verified 2026-10-12: the one modelled operation
  (`createPreAnnouncement`, `POST /verified-caller/vwip/pre-announce`) is
  implemented, tested, and documented in `specs/verified-caller/vwip/openapi.yaml`.
  **Spec↔code drift found and fixed in the vendored `201` success example:** the
  `created` example carried the Swagger placeholder `preAnnouncementId`
  `3fa85f64-5717-4562-b3fc-2c963f66afa6`, but the handler derives the id
  deterministically (SHA-256 over callingParticipant + calledParticipant +
  strategy) — the documented `BRAND_DISPLAY` request `+123456789111` →
  `+123456789012` actually yields `e2640459-fc76-4e90-8296-216995d9e7f4` (same
  placeholder-UUID drift class as optimal-edge-discovery). Bound the example to that
  concrete request, corrected the id, and clarified that `expiresAt` is illustrative
  (now + timeToLive, wall-clock dependent; the shown `14:29:08Z` = the module's
  `rfc3339_utc` test epoch `14:27:08Z` + default 120 s). Added a self-validating
  `vendored_success_example_matches_the_handler` test pinning the id both through
  the router and against `pre_announcement_id` directly. The `400`
  invalidArgument/outOfRange example messages, the reserved-suffix set
  (400/401/403/404/409/422/429/500/503 = `scenarios::RESERVED_ERROR_SUFFIXES`), and
  the request/response schemas already matched the Rust types.
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

> **⚠ DECISION POINT — 2026-09-27 — productive one-pass backlog is exhausted; do
> NOT reflexively pick an item below. READ THIS BEFORE CLAIMING WORK.**
>
> A grounded re-audit this pass (see today's "Backlog reality check" scan-journal
> entry) found the simulator is **feature-complete**: **61 CAMARA APIs mounted,
> ~1,100 test fns, `src/registry.rs` is 83,426 lines** of contract-test coverage,
> and every inventory item in Phases 0–5 is ✅. None of the three items below is a
> genuine, safe, *one-pass* unit any more:
> - **Item 1 (Phase 0 scope→purpose per endpoint)** is **not a real gap.** By
>   design (`src/auth/purpose.rs` module docs; DESIGN §6/§7) the simulator is
>   **purpose-agnostic**: it validates DPV *grammar* only and keeps no fixed
>   scope→purpose map, so integrators can exercise any purpose. Pinning a canonical
>   purpose per endpoint would contradict that design, not close a gap. The
>   grant→resource-server seam is already proven end-to-end on two three-legged
>   APIs; a third is marginal repetition.
> - **Item 2 (audit Phase 1–3 / add contract tests)** is the **treadmill** the
>   83k-line `registry.rs` already embodies — dozens of format/pattern/error-catalog
>   conformance families. Marginal value per added member; not fresh work.
> - **Item 3 (deepen `vwip`)** needs the deferred **stateful / provisioning-worker /
>   notification infrastructure** — a **multi-pass, human-gated** build, not a
>   one-pass increment.
>
> **This decision point has recurred:** a prior lineage reached the same conclusion
> across several passes and escalated the owner; that honest journal was later
> force-reset off `main` (remote reset to 162c3a3), which is why this reconstructed
> file re-presents the items below as if fresh. Restoring this note is the correct,
> safe (docs-only) action so the next automated pass does not repeat marginal Phase 0
> work.
>
> **Human decision needed** (any one unblocks real work): (i) authorise the
> provisioning-worker/state layer for the `vwip` APIs; (ii) name a specific
> API/increment to build; or (iii) pause the hourly routine. Until then, an idle
> pass should confirm nothing changed and **stop without committing** — do not add a
> contract-test member, do not repeat the seam check, and do not append a duplicate
> idle note (this one stands for the whole idle period).

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

- 2026-10-12 — Phase 5 vwip verification pass (verified-caller). Audited the
  ninth vwip API in mount order. The single modelled operation
  `createPreAnnouncement` (`POST /verified-caller/vwip/pre-announce`) is fully
  implemented (`src/apis/verified_caller/vwip.rs`), routed, and well-tested (20
  test fns before this pass), and both module docs agree with the code.
  **Spec↔code drift found and fixed in the vendored `201` `created` success
  example** (`specs/verified-caller/vwip/openapi.yaml`): the example carried the
  canonical Swagger placeholder `preAnnouncementId`
  `3fa85f64-5717-4562-b3fc-2c963f66afa6`, but the handler derives the id
  deterministically (SHA-256 over `callingParticipant` + `calledParticipant` +
  `strategy`, version/variant nibbles forced to a v4 shape), and the schema +
  API description both *promise* it is "deterministic … derived from the
  participants and strategy". A consumer could not reproduce the documented id
  from any input — the same consumer-visible placeholder-UUID defect the
  optimal-edge-discovery pass caught two cursors back ("the nil-UUID placeholder
  is especially misleading"). Bound the example to a concrete documented request
  — `BRAND_DISPLAY` from `+123456789111` to `+123456789012`, default
  `timeToLive` 120 s — whose real id is `e2640459-fc76-4e90-8296-216995d9e7f4`
  (verified independently and by the handler), and clarified in the example
  summary that `expiresAt` is illustrative: it is `now + timeToLive` and so
  wall-clock dependent (the shown `2024-01-01T14:29:08Z` is the module's
  `rfc3339_utc` test epoch `14:27:08Z` + the default 120 s, so the illustration
  is internally consistent — same accepted "illustrative timestamp" posture as
  qos-booking's `startedAt`). Added `vendored_success_example_matches_the_handler`,
  which drives the documented request through the router and pins the exact id,
  and also asserts that literal equals `pre_announcement_id(&req)` built directly
  (self-validating, so a future change to the derivation fails the test rather
  than silently blessing a new value). Spot-checked the rest of the spec and
  found no other drift: the `400` `invalidArgument`/`outOfRange` example messages
  byte-match the handler, the documented reserved-suffix set
  (400/401/403/404/409/422/429/500/503) equals `scenarios::RESERVED_ERROR_SUFFIXES`
  exactly, and the request/response schemas (`CreatePreAnnouncementRequest`,
  `AnnouncementInfo`, `VerifiedCallerError`, `PhoneNumber`) agree with the Rust
  types. This advances the vwip audit cursor to the next API (application-profiles).
  Spec + test change (no behaviour/response change — the handler's output was
  already correct; only the spec example was wrong); no new dependency.
  `cargo test` green: 3061 passed, 0 failed (+1, the new pinning test).
  `cargo build --release` green; binary = 5,430,424 bytes (unchanged vs. the
  prior eight passes on rustc 1.97.0 — the fix touches only the spec and
  `#[cfg(test)]` code, not the release path). *Backlog banner still stands:* the
  net-new feature backlog remains exhausted and the human decision (authorise
  stateful infra, name an increment, or pause the routine) is still open — this
  is the sanctioned per-API vwip verification (backlog item 3), and this pass it
  caught a genuine documentation defect. **Integration note:** the container
  started on a detached HEAD at `ea67dca` (the optimal-edge-discovery tip) while
  the local `origin/main` remote-tracking ref was a stale `93a23ca`; a
  `git fetch` confirmed the remote had in fact already advanced to `ea67dca`, so
  the eight prior vwip-verification passes (device-data-volume …
  optimal-edge-discovery) were pushed and are on the remote — only this
  container's cached ref lagged. This pass builds on that chain and pushes a
  single new commit to `origin/main`.
- 2026-10-12 — Phase 5 vwip verification pass (optimal-edge-discovery). Audited the
  eighth vwip API in mount order. Both modelled operations — `discoverOptimalEdge`
  (`POST /optimal-edge-discovery/vwip/retrieve-optimal-edge-cloud-zones`) and the
  read-only `getRegions` (`GET /optimal-edge-discovery/vwip/regions`) — are fully
  implemented (`src/apis/optimal_edge_discovery/vwip.rs`), routed, and tested, and
  both module docs agree with the code. **Spec↔code drift found and fixed in three
  vendored examples** (`specs/optimal-edge-discovery/vwip/openapi.yaml`): (1) the 200
  `single` and `ranked` success examples carried placeholder `edgeCloudZoneId`s — the
  nil-ish `00000000-0000-0000-0000-000000000000` and `11111111-…` — but the handler's
  `zone_id` derives a deterministic SHA-256-shaped id per zone, so a consumer running
  the documented `…000`/`…013` requests gets `d0581b5b-fa7d-50f5-bfbe-7f11c70e6e15`
  (eu-west-1), `e3a79eed-dfd9-9955-f1b0-61eb9503e139` (eu-central-1) and
  `a03b2bf0-fb61-e587-91e3-a750a7affa41` (us-east-1) — a real, consumer-visible defect
  (the nil-UUID placeholder is especially misleading); (2) the 404 `notFound` example
  said `message: "The specified resource was not found."` but the shared
  `CamaraError::for_status(404)` emits "…**is** not found." (the identical was→is drift
  the media-streaming-rate pass caught two APIs back); (3) the 422 `missingIdentifier`
  example carried the truncated "The device cannot be identified." but
  `resolve_identifier` emits the full "The device cannot be identified: supply a
  `device` or use a token that identifies a device." Corrected all three examples to
  the real bytes and added a `vendored_examples_match_the_handler` test that drives the
  documented `single`/`ranked`/404/422 requests through the router and pins the exact
  zone ids and messages (the 200 ids are asserted both as literals and against
  `zone_id(EDGE_ZONES[i])`, so the literal is self-validating) so the examples can never
  silently drift again. Spot-checked the rest of the spec and found no other drift: the
  400 `invalidArgument` and 422 `unnecessaryIdentifier` example messages already matched
  the handler, the `getRegions` region-catalog example matches `distinct_regions`
  byte-for-byte (already pinned by `distinct_regions_are_the_tables_regions_in_order`),
  and the request/response schemas (`OptimalEdgeDiscoveryInfo`, `Device`,
  `EdgeDiscoveryResponse`, `EdgeCloudZone`, `GetEdgeCloudRegionsResponse`, the reserved
  error set) agree with the Rust types. This advances the vwip audit cursor to the next
  API (verified-caller). Spec + test change (no behaviour/response change — the
  handler's output was already correct; only the spec examples were wrong); no new
  dependency. `cargo test` green: 3060 passed, 0 failed (+1, the new pinning test).
  `cargo build --release` green; binary = 5,430,424 bytes (unchanged vs. the prior seven
  passes on rustc 1.97.0 — the fix touches only the spec and `#[cfg(test)]` code, not
  the release path). *Backlog banner still stands:* the net-new feature backlog remains
  exhausted and the human decision (authorise stateful infra, name an increment, or
  pause the routine) is still open — this is the sanctioned per-API vwip verification
  (backlog item 3), and this pass it caught a genuine documentation defect.
- 2026-10-12 — Phase 5 vwip verification pass (network-traffic-analysis). Audited
  the seventh vwip API in mount order. The single modelled operation
  `getTrafficAnalysis` (`GET /network-traffic-analysis/vwip/traffic-analysis`) is
  fully implemented (`src/apis/network_traffic_analysis/vwip.rs`), routed, and
  tested, and both module docs agree with the code. **Spec↔code drift found and
  fixed in the vendored `200` `success` example**
  (`specs/network-traffic-analysis/vwip/openapi.yaml`): the example was summarised
  "Two applications, one DAY slot (`…012` tail)" and listed only whatsapp + wechat
  with `pagination.totalCount: 2`. But the handler's deterministic app-count plane
  is `n_apps = ((d % 5) + 1)` where `d` is the `networkId`'s trailing three digits,
  so a `…012` tail → `d=12` → `(12 % 5) + 1 = 3` applications. A consumer running
  the documented request would get THREE records (whatsapp `accessCount 13`, wechat
  `26`, **netflix `39`** / flows 39936/79872/119808), not two — a real,
  consumer-visible doc defect (same class as the population-density-data success
  example two cursors back). The code is correct and already pinned by
  `returns_records_and_pagination` and
  `access_flow_is_up_plus_down_and_records_scale_with_the_identifier` (both assert
  3 records for `…012`); only the spec example had drifted. Added the missing
  netflix record, corrected `totalCount` to 3 and the summary to "Three
  applications", and added a `vendored_success_example_matches_the_mapping` test
  that drives the documented request and pins the exact three records (app,
  ipv4Address, description, accessCount, the three flow fields, dates) plus the
  pagination envelope, so the example can never silently drift from the mapping
  again. Spot-checked the rest of the spec and found no other drift: the `noData`
  (`…000`) example, the 400 `invalidArgument`/`outOfRange` example messages, and
  the request/response schemas (`TrafficRecord`, `Pagination`,
  `TrafficAnalysisResponse`, `NetworkId`, `Frequency`, the reserved-error set) all
  match the Rust types and handler. This advances the vwip audit cursor to the next
  API (optimal-edge-discovery). Spec + test change (no behaviour/response change —
  the handler's output was already correct; only the spec example was wrong); no
  new dependency. `cargo test` green: 3059 passed, 0 failed (+1, the new pinning
  test). `cargo build --release` green; binary = 5,430,424 bytes (unchanged vs. the
  prior six passes on rustc 1.97.0 — the fix touches only the spec and
  `#[cfg(test)]` code, not the release path). *Backlog banner still stands:* the
  net-new feature backlog remains exhausted and the human decision (authorise
  stateful infra, name an increment, or pause the routine) is still open — this is
  the sanctioned per-API vwip verification (backlog item 3), and this pass it
  caught a genuine documentation defect.
- 2026-10-12 — Phase 5 vwip verification pass (network-health-assessment).
  Audited the sixth vwip API in mount order. The single modelled operation
  `getHealthScores` (`GET /network-health-assessment/vwip/health-scores`) is
  fully implemented (`src/apis/network_health_assessment/vwip.rs`), routed, and
  tested, and both module docs (`network_health_assessment.rs` parent + `vwip.rs`)
  agree with the code. **No spec↔code↔doc drift found** (like device-visit-location
  two passes earlier, this one is clean): the vendored spec's three 200 success
  examples — `…950`/NET → `score 95.0`, `…950`/NET_CORE → `92.0` (module shift),
  `…000` → `score null`/`scoringTime null` — all match the deterministic mapping
  `score = clamp(d/10 − moduleIndex, 0, 100)` (moduleIndex NET=0/WIRELESS=1/
  TRANSPORT=2/CORE=3) and are already pinned byte-for-byte by
  `trailing_digits_set_the_score_band`, `net_type_shifts_the_score_per_module`,
  and `zero_tail_is_the_no_data_case`. Confirmed the spec's declared
  reserved-error suffix set (400/401/403/404/409/422/429/500/503) matches the
  shared `scenarios::RESERVED_ERROR_SUFFIXES` exactly, and that
  `trailing_three_digits` reads a UUID's trailing three decimal digits as the
  control plane as documented. **Small tested increment:** the one vendored spec
  example that no test guarded was the 400 `invalidArgument` message; added
  `vendored_invalid_argument_example_matches_the_handler`, which drives a non-UUID
  `networkId` and pins the exact `status`/`code`/`message` bytes against the
  handler, closing the same drift class the media-streaming-rate pass caught
  retroactively. This advances the vwip audit cursor to the next API
  (network-traffic-analysis). Test + docs change (no behaviour/response/spec
  change — the handler and spec were already correct and mutually consistent); no
  new dependency. `cargo test` green: 3058 passed, 0 failed (+1, the new pinning
  test). `cargo build --release` green; binary = 5,430,424 bytes (unchanged vs.
  the prior four passes on rustc 1.97.0 — the new test is `#[cfg(test)]`, outside
  the release path). *Backlog banner still stands:* the net-new feature backlog
  remains exhausted and the human decision (authorise stateful infra, name an
  increment, or pause the routine) is still open — this is the sanctioned per-API
  vwip verification (backlog item 3).
- 2026-10-12 — Phase 5 vwip verification pass (media-streaming-rate). Audited the
  fifth vwip API in mount order. The single modelled operation
  `retrieveMaximumDownstreamMediaRate`
  (`POST /media-streaming-rate/vwip/retrieve-maximum-downstream-media-rate`) is
  fully implemented (`src/apis/media_streaming_rate/vwip.rs`), routed, and tested,
  and the module doc agrees with the code. **Spec↔code drift found and fixed in two
  vendored error-example messages** (`specs/media-streaming-rate/vwip/openapi.yaml`):
  (1) the `404` example claimed `message: "The specified resource was not found."`
  but the shared `CamaraError::for_status(404)` (src/errors.rs) emits "The specified
  resource **is** not found." (was→is); (2) the `422` `missingIdentifier` example
  claimed the truncated "The device cannot be identified." but the handler
  (`resolve_identifier`) emits the fuller "The device cannot be identified: supply a
  `device` or use a token that identifies a device." A consumer reading the spec
  would have expected different `message` bytes than the endpoint returns — a real,
  consumer-visible doc defect, not cosmetic (same class as the
  population-density-data success-example drift two passes back). Corrected both
  examples to the true messages and added a
  `vendored_error_example_messages_match_the_handler` test that drives the endpoint
  and pins the exact 400/404/422 message bytes so the examples can never silently
  drift again. Spot-checked the rest of the spec and found no other drift: the two
  200 success examples (`…000`→`0 bps` floor with device echoed, `…002`→`2 Mbps`)
  match `max_downstream_rate`, the 400 `INVALID_ARGUMENT` and 422
  `UNNECESSARY_IDENTIFIER` example messages already byte-matched the handler, and
  the request/response schemas (`MediaStreamingRateRequest`/`Response`, `Device`,
  `BitRateUnitEnum`, `DeviceResponse`) agree with the Rust types. This advances the
  vwip audit cursor to the next API (network-health-assessment). Spec + test change
  (no behaviour/response change — the handler's output was already correct; only the
  spec examples were wrong); no new dependency. `cargo test` green: 3057 passed, 0
  failed (+1, the new pinning test). `cargo build --release` green; binary =
  5,430,424 bytes (unchanged vs. the prior four passes on rustc 1.97.0 — the fix
  touches only the spec and `#[cfg(test)]` code, not the release path). *Backlog
  banner still stands:* the net-new feature backlog remains exhausted and the human
  decision (authorise stateful infra, name an increment, or pause the routine) is
  still open — this is the sanctioned per-API vwip verification (backlog item 3),
  and this pass it caught a genuine documentation defect.
- 2026-10-12 — Phase 5 vwip verification pass (qos-booking). Audited the fourth
  vwip API in mount order. The vendored spec (`specs/qos-booking/vwip/openapi.yaml`,
  1128 lines) and the code (`src/apis/qos_booking/vwip.rs`) are **consistent**: all
  four modelled operations — `createBooking`, `getBooking`, `deleteBooking`,
  `retrieveBookingByDevice` — are fully implemented, routed, and tested, and the
  spec's examples match the handlers. Spot-checked the deterministic planes and found
  no drift: the three `createBooking` 201 examples encode the identifier-tail
  derivation (phoneNumber `…012` → ACTIVATED with `startedAt`, `…013` → SCHEDULED,
  `…000` → REQUESTED) that dedicated tests (`create_activated_and_echoes_the_request`,
  `odd_identifier_tail_is_scheduled_without_start_time`,
  `triple_zero_identifier_is_requested_without_start_time`) already pin; the
  `activated` example's `startedAt` is an illustrative timestamp (the handler stamps
  `now`), not deterministic drift; and every 400/409/422 error example
  (`INVALID_ARGUMENT`/`OUT_OF_RANGE`/`QOS_BOOKING.DURATION_OUT_OF_RANGE`/`INVALID_SINK`,
  `CONFLICT`, the 422 catalog) byte-matches the `create_booking` handler. **One
  genuine defect found and fixed:** the top-level module doc
  (`src/apis/qos_booking.rs`) still said the vwip module's "First slice" was only
  `POST /device-qos-bookings` (`createBooking`), though `getBooking`,
  `deleteBooking`, and `retrieveBookingByDevice` are all implemented and tested —
  the identical stale-doc trap caught earlier on device-data-volume ("`/check` is a
  later slice"), which had already misdirected a pass toward re-implementing an
  existing endpoint. Corrected the parent doc to list all four operations (the
  `vwip.rs` module doc was already accurate). Docs-only correction (one module doc +
  this journal + the inventory bullet); no behaviour change, no spec change, no new
  dependency; qos-booking vwip verified complete for all four modelled operations.
  This advances the vwip audit cursor to the next API (media-streaming-rate).
  `cargo test` green: 3056 passed, 0 failed (unchanged — nothing to test in a doc
  comment; the derivation the examples encode is already tested). `cargo build
  --release` green; binary = 5,430,424 bytes (unchanged vs. the prior three passes on
  rustc 1.97.0 — a doc-only change does not alter the release path). *Backlog banner
  still stands:* the net-new feature backlog remains exhausted and the human decision
  (authorise stateful infra, name an increment, or pause the routine) is still open —
  this is the sanctioned per-API vwip verification (backlog item 3), and this pass it
  caught a genuine documentation defect.
- 2026-10-12 — Phase 5 vwip verification pass (population-density-data). Audited the
  third vwip API in mount order and found **spec↔code drift in the vendored `200`
  success example** (`specs/population-density-data/vwip/openapi.yaml`). The example
  claimed `u4pruy` → `DENSITY_ESTIMATION` `pplDensity 4213` (band 3792/4634), `gbsuv`
  → `NO_DATA`, overall `status PART_OF_AREA_NOT_SUPPORTED`. But the code's
  deterministic FNV-1a mapping (`cell_for`/`area_status`,
  `src/apis/population_density_data/vwip.rs`) actually yields `u4pruy` → 2837
  (2554/3120) and `gbsuv` → `LOW_DENSITY` — i.e. a consumer running the documented
  request would get different numbers, a different `dataType`, and `SUPPORTED_AREA`
  (not partly-unsupported). A real, consumer-visible defect, not cosmetic. Fixed the
  example to the true values and swapped the second cell to `dr5bb` (a genuine
  `NO_DATA` geohash) so the "partlySupported" illustration stays valid, and added a
  `vendored_success_example_matches_the_mapping` unit test that pins the exact
  documented `u4pruy`/`dr5bb` cells and the `PART_OF_AREA_NOT_SUPPORTED` status so
  the example can never silently drift again. Spot-checked the rest of the spec: all
  `400`/`422` error-case example messages already match the handler byte-for-byte, and
  the request/response schemas agree with the Rust types — only the success example
  had drifted. This advances the vwip audit cursor to the next API (qos-booking).
  Spec + test change (no behaviour/response change — the handler's output was already
  correct; the spec was wrong); no new dependency. `cargo test` green: 3056 passed, 0
  failed (+1, the new pinning test). `cargo build --release` green; binary =
  5,430,424 bytes (unchanged vs. the prior two passes on rustc 1.97.0 — the fix
  touches only the spec and test code, not the release path). *Backlog banner still
  stands:* net-new feature backlog remains exhausted and the human decision
  (authorise stateful infra, name an increment, or pause the routine) is still open —
  this is the sanctioned per-API vwip verification (backlog item 3), and this pass it
  caught a genuine documentation defect.
- 2026-10-12 — Phase 5 vwip verification pass (device-visit-location). Audited the
  second vwip API in mount order. Unlike device-data-volume (prior pass, stale doc
  found), this one is **clean**: the single modelled operation
  `retrieveDeviceVisitLocation` (`POST /device-visit-location/vwip/retrieve`) is
  fully implemented (`src/apis/device_visit_location/vwip.rs`), the module doc and
  the vendored spec (`specs/device-visit-location/vwip/openapi.yaml`) agree with the
  code, and ~25 tests cover every control plane (success geoCodeList, `…000`
  DATA_NOT_FOUND, reserved-suffix errors, INVALID_END_DATE, malformed/missing time,
  the two-legged/three-legged identifier-resolution 422s, auth, x-correlator echo).
  Spot-checked for drift and found none: the spec's `…012` success example
  (US/GB/DE postal codes 01651/02622/03593) matches `visited_geocodes`/`postal_code`
  byte-for-byte, and the reserved-suffix mapping the spec cites (422 →
  SERVICE_NOT_APPLICABLE, etc.) matches `CamaraError::for_status`. No code/spec/doc
  change needed — this advances the vwip audit cursor to the next API
  (population-density-data) so a future pass does not re-audit this one. Docs-only
  (PROGRESS.md inventory bullet + this entry). `cargo test` green: 3055 passed, 0
  failed (unchanged — nothing to test in a verification with no behaviour change).
  `cargo build --release` green; binary = 5,430,424 bytes (unchanged vs. the prior
  pass on rustc 1.97.0; no code changed). *Backlog banner still stands:* the
  net-new feature backlog remains exhausted and the human decision (authorise
  stateful infra, name an increment, or pause the routine) is still open — this
  pass is the sanctioned per-API vwip verification (backlog item 3), not net-new
  feature work.
- 2026-10-12 — Phase 5 vwip verification pass (device-data-volume). Audited the
  first vwip API in mount order and found the top-level module doc
  (`src/apis/device_data_volume.rs`) **stale**: it claimed only `POST /retrieve`
  (`retrieveDataVolume`) was modelled and that `POST /check` (`checkDataVolume`)
  was "a later slice". In fact `check` is fully implemented and tested in
  `src/apis/device_data_volume/vwip.rs` (handler, routes, request schema, ~15
  dedicated tests) and both operations are documented in the vendored spec
  (`specs/device-data-volume/vwip/openapi.yaml`, operationIds
  `retrieveDataVolume` / `checkDataVolume`) — the doc was the only drift.
  Corrected the comment to state both operations are modelled. **This removes a
  trap:** the stale "later slice" wording had already misdirected this pass toward
  re-implementing an existing endpoint and would mislead future automated passes
  the same way. Docs-only correction (one module comment); no behaviour change, no
  spec change, no new dependency; device-data-volume vwip verified complete for
  both modelled operations. `cargo test` green: 3055 passed, 0 failed (unchanged —
  no test added, nothing to test in a comment). `cargo build --release` green;
  binary = 5,430,424 bytes (built on rustc 1.97.0; not comparable to the prior
  5,330,648 B recorded on an earlier toolchain — the doc-only change does not alter
  the binary). *Note on the backlog banner:* this was **not** an idle pass — a
  genuine factual defect was found and fixed — but the banner's broader point
  stands: the productive net-new feature backlog remains essentially exhausted and
  a human decision is still needed (authorise stateful infra, name an increment, or
  pause the routine).
- 2026-09-27 — Backlog reality check (no code change; docs-only). Started from a
  detached HEAD at 162c3a3 while `origin/main` had just been **force-reset** from
  bf4899a back to 162c3a3, discarding a prior lineage's honest journal (its subjects:
  "flag exhausted backlog", "no small stateless net-new API remains", "error catalogs
  already covered; escalate to owner", "4th/5th consecutive idle pass"). Independently
  re-audited the *current* tree and confirmed that conclusion on its own evidence: 61
  APIs mounted, ~1,100 test fns, `src/registry.rs` = 83,426 lines of contract-test
  coverage, all Phase 0–5 inventory ✅. Assessed the three backlog items on their
  merits — item 1 (scope→purpose per endpoint) is not a real gap (the simulator is
  purpose-agnostic by design; see `src/auth/purpose.rs`); item 2 is the existing
  contract-test treadmill; item 3 needs multi-pass, human-gated stateful infra — so no
  genuine, safe, one-pass increment remains. Per AGENT.md's stop condition, made **no
  code/spec change**; the only edit is this note plus a DECISION-POINT banner atop the
  Backlog section, restoring the honest record the force-reset erased so the next pass
  does not repeat marginal Phase 0 work. `cargo build --release` re-confirmed green;
  binary = 5,330,648 bytes (unchanged). Did not re-run the full suite: no code changed.
  **Human decision needed** (see the banner): authorise the stateful/provisioning-worker
  layer, name a specific increment, or pause the routine.
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
