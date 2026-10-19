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
- ✅ carrier-billing v0.5 — spec↔code audited 2026-10-15 (Phase 5 stable tier, tenth; the
  second **stateful** one — a `store.rs`-backed payment lifecycle with a `notifications.rs`
  CloudEvent sink): all seven operations (`createPayment` `POST /payments`, `retrievePayments`
  `GET /payments`, `retrievePayment` `GET /payments/{id}`, and the complete two-step flow —
  `preparePayment` `POST /payments/prepare`, `validatePayment` `POST …/{id}/validate`,
  `confirmPayment` `POST …/{id}/confirm`, `cancelPayment` `POST …/{id}/cancel`) reviewed end to
  end against `specs/carrier-billing/v0.5/openapi.yaml`. The two control planes (identifier
  reserved-error suffix + the `…888` validation-required tail; the `amount` < 0.001 →
  INVALID_ARGUMENT / > 1000 → UNAUTHORIZED_AMOUNT plane), the OTP budget (3) → VALIDATION_FAILED
  path, and the confirm/cancel store state machine all match the handler. The minted `paymentId`
  (counter+clock `mint_uuid`) and wall-clock `paymentCreationDate`/`paymentDate` are legitimately
  illustrative (the application-profiles/session-insights posture), so the 200/201 examples are
  not pinned. **Drift found & fixed — the truncated-identifier-message class** (device-swap /
  kyc-fill-in / kyc-tenure / consent-info precedent): both the `createPayment` and `preparePayment`
  422 `missingIdentifier` examples carried the short "The phone number cannot be identified." but
  `resolve_phone_number` emits the full "…: supply `phoneNumber` or use a token that identifies a
  phone number." Corrected both. **Stale parent module doc fixed** (`src/apis/carrier_billing.rs`):
  it listed `confirmPayment`/`cancelPayment` as "later slices" though both ship and are routed —
  the same stale-slice-doc trap caught on device-data-volume / qos-booking / application-profiles /
  session-insights / sponsored-data; corrected to list all seven as implemented. Added
  `vendored_error_example_messages_match_the_handler` pinning the exact status/code/message bytes
  of all fourteen router-reachable inline error families whose message is the API's own
  (createPayment/preparePayment 422 UNAUTHORIZED_AMOUNT + MISSING_IDENTIFIER; validatePayment 400
  INVALID_AUTHORIZATION_ID/INVALID_CODE/VALIDATION_FAILED + 409 ALREADY_EXISTS; confirmPayment &
  cancelPayment 409 PAYMENT_CONFIRMED/PAYMENT_CANCELLED/CONFLICT) — existing tests guarded only
  `code`. The two createPayment/preparePayment 400 `INVALID_ARGUMENT` examples show the shared
  catalog's canonical generic message, which the handler never emits (specific per-cause messages),
  so they are intentionally left unpinned; every other 4xx/5xx `$ref`s the shared catalog. Spec +
  doc + `#[cfg(test)]` change only, binary unchanged.
- ✅ call-forwarding-signal v0.4
- ✅ number-recycling v0.2
- ✅ kyc-age-verification v0.1
- ✅ device-swap v1 — spec↔code audited 2026-10-14 (Phase 5 stable tier): fixed the
  truncated 422 `missingIdentifier` example in both operations (`check`,
  `retrieve-date`); handler already emitted the full message, added a pinning test.
- ✅ kyc-fill-in v0.3 — spec↔code audited 2026-10-15 (Phase 5 stable tier): fixed the
  truncated 422 `missingIdentifier` example (`The phone number cannot be identified.` →
  the handler's full "…: supply `phoneNumber` or use a token that identifies a line.");
  the two 200 persona-0 examples (`setAll`/`narrowScopes`) and the other inline
  error messages already byte-matched; added a test pinning all five router-reachable
  inline error families through the router.
- ✅ home-devices-qod v0.4 — spec↔code audited 2026-10-15 (Phase 5 stable tier, sixth):
  the one operation (`setQos`, `PUT /home-devices-qod/v0.4/qos`) — a stateless,
  phone-number-free API keyed on the device LAN `ipAddress` last octet — reviewed end
  to end; the happy-path/reserved-octet control plane (incl. `serviceClass=standard` as
  the second plane for the `…247` already-default case), the generic-code messages
  (matching `CamaraError::for_status`), and all seven inline error examples
  (400/404/409×3/503/504) already byte-matched the handler. **No drift found.** Added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of all seven through the router (existing error tests
  guarded only `status`/`code`); `#[cfg(test)]`-only, binary unchanged.
- ✅ qos-profiles v1 — spec↔code audited 2026-10-15 (Phase 5 stable tier, seventh):
  both operations (`retrieveQoSProfiles`, `POST /retrieve-qos-profiles`; `getQosProfile`,
  `GET /qos-profiles/{name}`) over a fixed in-memory catalog reviewed end to end. **Drift
  found & fixed — the incomplete-example class:** the `voice` 200 example (shared by both
  operations) showed only 7 of the 15 fields the handler returns for the fully
  deterministic static `voice` profile (missing `targetMinUpstreamRate`, `maxUpstreamRate`,
  `minDuration`, `maxDuration`, `jitter`, `packetErrorLossRate`, `l4sQueueType`). Since the
  catalog is reproducible, the example must match byte-for-byte; expanded both to the full
  profile and added `vendored_voice_example_matches_the_handler` pinning it as an exact
  object through the router via both the single-lookup and the `name` filter. The two 422
  inline examples (`serviceNotApplicable` = `for_status(422)`, `unnecessaryIdentifier` =
  the handler's inline message) already byte-matched; all other 4xx/5xx `$ref` the shared
  catalog.
- ✅ kyc-tenure v0.2 — spec↔code audited 2026-10-15 (Phase 5 stable tier, eighth): the one
  operation (`checkTenure`, `POST /kyc-tenure/v0.2/check-tenure`) reviewed end to end. Both
  control planes (identifier reserved-error-suffix; the trailing-three-digits held-days verdict
  compared against `tenureDate`, with `tenureDate` a genuine second control plane and
  `contractType = [PAYG,PAYM,Business][digits % 3]`) match the handler, the E.164 pattern and the
  strict `YYYY-MM-DD` date (malformed→400 `INVALID_ARGUMENT`, future→400 `OUT_OF_RANGE`) are
  genuinely enforced, and the two-legged/three-legged identifier resolution is correct. **Drift
  found & fixed — the truncated-identifier-message class** (device-swap / kyc-fill-in /
  kyc-age-verification / number-recycling / call-forwarding-signal precedent): the 422
  `missingIdentifier` example showed the short "The phone number cannot be identified." but
  `resolve_identifier` emits the full "…: supply `phoneNumber` or use a token that identifies a
  line." Corrected it; the 400 `invalidArgument`/`outOfRange`, 422
  `unnecessaryIdentifier`/`serviceNotApplicable` inline examples already byte-matched. Added
  `vendored_error_example_messages_match_the_handler` pinning the exact `status`/`code`/`message`
  bytes of all five router-reachable inline error families (existing error tests guarded only
  `status`/`code`). Spec + `#[cfg(test)]` change only, no behaviour change, no new dependency.
- ✅ blockchain-public-address v0.3 — spec↔code audited 2026-10-15 (Phase 5 stable tier,
  ninth; the first **stateful** one in the tier — a `store.rs`-backed bind/unbind): all three
  operations (`retrieveBlockchainPublicAddress` `POST …/retrieve-blockchains`,
  `bindBlockchainPublicAddress` `POST …/blockchain-public-addresses`,
  `deleteBlockchainPublicAddress` `DELETE …/{id}`) reviewed end to end. **Drift found & fixed —
  the deterministic-value / placeholder class** (verified-caller / device-identifier /
  optimal-edge-discovery precedent): both response-level 200/201 `bound` examples carried Swagger
  placeholders, and the retrieve one claimed a single address on `eip155:1` — impossible under the
  `NETWORKS[(d+i)%6]` mapping (a single-address case needs `d ≡ 1 mod 3`, but `eip155:1` =
  `NETWORKS[0]` needs `d % 6 == 0`, which always yields three). Bound the retrieve example to
  `+123456789001` (→ one Polygon `eip155:137`/`[POL]` address) and the bind example to the
  request-schema triple (`+123456789012`/`evm:1`/`0x9f8a…` → `binding_id` `62a4c79b-…`), both with
  the real SHA-256-derived values, and corrected the "Ethereum-mainnet" claim in the spec
  description + module doc to Polygon. Added `vendored_examples_match_the_handler` driving both
  through the router and pinning each body exactly (the bind id also against `binding_id`). The
  error responses all `$ref` the shared catalog (no inline error examples to drift). Spec + doc +
  `#[cfg(test)]` change only, binary unchanged.
- ✅ simple-edge-discovery v2 — spec↔code audited 2026-10-15 (Phase 5 stable tier, eleventh):
  the one operation (`readClosestEdgeCloudZone`,
  `POST /simple-edge-discovery/v2/retrieve-closest-edge-cloud-zone`) reviewed end to end. Both
  control planes match the handler (the identifier reserved-error-suffix plane; the
  trailing-three-digits `EDGE_ZONES[digits % 6]` closest-zone plane, `…000`/no digits → index 0),
  the two-legged/three-legged identifier resolution (incl. `422 UNNECESSARY_IDENTIFIER` on a
  resubmitted device over a line token, `422 MISSING_IDENTIFIER`, `400 INVALID_ARGUMENT` on an
  empty `device`) is faithful, and the E.164 pattern + `deny_unknown_fields` are genuinely
  enforced. **Drift found & fixed — the deterministic-value / placeholder class**
  (blockchain-public-address / verified-caller / device-identifier / optimal-edge-discovery
  precedent): both 200 examples (`phoneKeyed`, `ipKeyed`) and the `EdgeCloudZone` schema-level
  example carried hand-picked placeholder `edgeCloudZoneId`s (`b1e5f0a2-…`, `c2f6018b-…`) though
  `zone_id` derives the id as SHA-256(`sed-zone:<zoneName>`) rendered UUID-shaped — eu-west-1 (idx
  0) = `ddf71ac2-cdd5-9f27-40b6-e042dcf7b697`, ap-northeast-1 (idx 5) =
  `95b0c904-3bf3-d083-d64e-e0f15fce3a62`. Corrected all three to the real derived values and bound
  each 200 example's summary to the concrete request that reproduces it. The three inline 422
  example messages (`unnecessaryIdentifier` = inline; `serviceNotApplicable` = `for_status(422)`)
  already byte-matched, except **the truncated-identifier-message class** — the `missingIdentifier`
  example carried the short "The device cannot be identified." but the handler emits the full
  "…: supply a `device` or use a token that identifies a device." (device-swap / media-streaming-rate
  / optimal-edge-discovery precedent); corrected it. Added `vendored_examples_match_the_handler`
  driving both documented 200 requests through the real router and pinning each body as an exact
  object (the `edgeCloudZoneId` also against `zone_id`), plus the exact `status`/`code`/`message`
  bytes of all three router-reachable inline 422 families (prior error tests guarded only `code`).
  The 400/401/403/404/409/429/500/503 responses all `$ref` the shared catalog. Spec + `#[cfg(test)]`
  change only, binary unchanged, no new dependency.
- ✅ customer-insights v0.2 — spec↔code audited 2026-10-15 (Phase 5 stable tier, twelfth): the one
  operation (`retrieveScoring`, `POST /customer-insights/v0.2/scoring/retrieve`) reviewed end to
  end. Both control planes match the handler (the identifier reserved-error-suffix plane; the
  trailing-three-digits score plane, `gaugeMetric → 300 + d%551`, `veritasIndex → d%20`, so
  `scoringType` is a genuine second control plane), the two-legged/three-legged identifier
  resolution (incl. `422 UNNECESSARY_IDENTIFIER`, `422 CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED`,
  `422 MISSING_IDENTIFIER`) is faithful, and the E.164 pattern, `deny_unknown_fields`, the
  `idDocument` length bound and the scope are genuinely enforced. **Drift found & fixed — the
  truncated-identifier-message class** (simple-edge-discovery / carrier-billing / kyc-tenure
  precedent): the 422 `missingIdentifier` AND `idDocumentNotSupported` examples carried truncated
  messages ("The phone number cannot be identified." / "Scoring by `idDocument` alone is not
  supported.") but the handler emits the full "…: supply `phoneNumber`/a `phoneNumber` or use a
  token that identifies a line." Corrected both; the `unnecessaryIdentifier` / `serviceNotApplicable`
  (= `for_status(422)`) / 400 `invalidArgument` examples already byte-matched. Bound the two 200
  example summaries to the reproducing line `+123456789740` (gauge 489, veritas 0). Added
  `vendored_examples_match_the_handler` pinning both 200 bodies as exact objects and the exact
  `status`/`code`/`message` bytes of all five router-reachable inline error families (prior tests
  guarded only `code` / the 200 fields individually). Spec + `#[cfg(test)]` change only, binary
  unchanged (5,455,000 bytes), no new dependency.
- ✅ connected-network-type v0.2 — spec↔code audited 2026-10-15 (Phase 5 stable tier, thirteenth):
  the one operation (`getConnectedNetworkType`, `POST /connected-network-type/v0.2/retrieve`)
  reviewed end to end against `specs/connected-network-type/v0.2/openapi.yaml`. Both control planes
  match the handler (the identifier reserved-error-suffix plane; the trailing-three-digits radio
  plane — `digits % 5` over the newest-first `NETWORK_TYPES` table `[5G,4G,3G,2G,UNKNOWN]`, `…000`/no
  digits → index 0 `5G`, so the reported technology is a genuine second control plane), the
  two-legged/three-legged identifier resolution (device precedence phoneNumber → NAI → IPv4
  `publicAddress` → ipv6; a `device` resubmitted on a line-subject three-legged token → `422
  UNNECESSARY_IDENTIFIER`; no device + non-line subject → `422 MISSING_IDENTIFIER`; empty `device`
  object → `400 INVALID_ARGUMENT`) is faithful, the `DeviceResponse` is echoed only for a phone-keyed
  request, `lastStatusTime` is the current instant (`null` for `UNKNOWN`), and the E.164 pattern,
  `deny_unknown_fields`, the `connected-network-type:read` scope and the `x-correlator` echo are
  genuinely enforced. **Drift found & fixed — the truncated-identifier-message class**
  (simple-edge-discovery / carrier-billing / customer-insights precedent): the 422 `missingIdentifier`
  example carried the short "The device cannot be identified." but the handler emits the full "…:
  supply a `device` or use a token that identifies a device." Corrected it. **Also the
  illustrative-timestamp class** (sim-swap `latestSimChange` / device-identifier `lastChecked` /
  quality-on-demand precedent): the `phoneKeyed5G` and `ipKeyed3G` 200 examples presented
  `lastStatusTime` as a frozen literal (`2024-01-01T14:27:08Z`) though the handler stamps the current
  instant (wall-clock dependent, can never reproduce); marked both illustrative with a derivation note
  and bound each 200 example to the concrete reproducing request (`+…000`→5G, ipv4 `…002`→3G, `+…004`→
  UNKNOWN null). The `unnecessaryIdentifier` and `serviceNotApplicable` (= `for_status(422)`) examples
  already byte-matched; the 400/401/403/404/409/429/500/503 responses all `$ref` the shared catalog.
  Added `vendored_examples_match_the_handler` pinning all three 200 bodies as exact objects (minus the
  illustrative `lastStatusTime`, asserted present/UTC or `null`) plus the exact `status`/`code`/
  `message` bytes of the three router-reachable inline 422 families (prior tests guarded only `code`).
  Spec + `#[cfg(test)]` change only — no behaviour/response change (the handler was already correct;
  one message truncated, two timestamps illustrative), no new dependency.
- ✅ connectivity-insights v0.6 — spec↔code audited 2026-10-15 (Phase 5 stable tier, fourteenth):
  the one operation (`checkNetworkQuality`, `POST /connectivity-insights/v0.6/check-network-quality`)
  reviewed end to end against `specs/connectivity-insights/v0.6/openapi.yaml`. Both control planes match
  the handler (the identifier reserved-error-suffix plane; the trailing-three-digits plane where the low
  five bits `d & 0x1F` form an *unmet mask*, one bit per KPI in the order `packetDelayBudget`,
  `targetMinDownstreamRate`, `targetMinUpstreamRate`, `packetlossErrorRate`, `jitter`, and `additionalKPIs`
  `signalStrength`/`connectivityType` degrade coherently with `met = 5 − popcount`, so `…000`/no digits →
  all met / excellent / 5G-SA), the two-legged/three-legged identifier resolution is faithful (device
  precedence phoneNumber → NAI → IPv4 `publicAddress` → ipv6; **no** `UNNECESSARY_IDENTIFIER` — a `device`
  on a line token is simply used; no device + non-line subject → `422 MISSING_IDENTIFIER`; empty `device`
  → `400 INVALID_ARGUMENT`), and the required `applicationProfileId` UUID / required `applicationServer`
  `minProperties:1` / optional `applicationServerPorts` `0..=65535` / `monitoringTimeStamp` RFC-3339 shape,
  the `phoneNumber` E.164 pattern, `deny_unknown_fields`, the `connectivity-insights:check` scope and the
  `x-correlator` echo are genuinely enforced. The response carries **no wall-clock field** (no
  `lastStatusTime`), so the 200 examples are fully reproducible — the device is echoed only for a
  phone-keyed request. **Drift found & fixed — the truncated-identifier-message class**
  (connected-network-type / simple-edge-discovery / carrier-billing / customer-insights precedent): the 422
  `missingIdentifier` example carried the short "The device cannot be identified." but the handler emits the
  full "The device cannot be identified: supply a `device` or use a token that identifies a device."
  Corrected it. The two 200 examples (`healthy` `…000`, `degraded` `…001`), the 400 `invalidArgument`
  (`applicationProfileId` must be a UUID) / `outOfRange` (port) and the 422 `serviceNotApplicable`
  (= `for_status(422)`) examples already byte-matched; the 401/403/404/409/429/500/503 responses all `$ref`
  the shared catalog. The parent module doc was already accurate (one operation; the stateful companions
  correctly out of scope) — no stale-slice-doc drift. Added `vendored_examples_match_the_handler` driving
  both documented 200 requests through the real router and pinning each body as an **exact object** (prior
  tests asserted the KPI fields individually), plus the exact `status`/`code`/`message` bytes of all four
  router-reachable inline error families (prior error tests guarded only `code`). Spec + `#[cfg(test)]`
  change only — no behaviour/response change (the handler was already correct; one message was truncated),
  no new dependency.
- ✅ region-device-count v0.2 — spec↔code audited 2026-10-15 (Phase 5 stable tier,
  fifteenth; the area-keyed, identifier-free aggregate-count API — no phone/device
  id, the control planes key off the region's characteristic radius + optional
  `filter`): the one operation (`count`, `POST /region-device-count/v0.2/count`)
  reviewed end to end against `specs/region-device-count/v0.2/openapi.yaml`. The
  handler is faithful — the region-size plane (`r > 1e6` → `UNSUPPPORTED_REQUEST`
  (triple-P verbatim); `5e5 < r ≤ 1e6` without `sink` → `UNSUPPORTED_SYNC_RESPONSE`,
  with a `sink` answered synchronously), the trailing-three-digits coverage plane
  (`…429`→429, `…001`→PART, `…002`→AREA_NOT_SUPPORTED, `…003`→DENSITY_BELOW_PRIVACY_THRESHOLD,
  `…004`→TIME_INTERVAL_NO_DATA_FOUND, else SUPPORTED_AREA), the fixed-density count
  (500 dev/km², halved for PART, narrowed by `filter`'s fixed category shares), and
  the full validation set (CIRCLE/POLYGON area codes, both-or-neither + end≥start
  time, filter enum, sinkCredential) all match. **Drift found & fixed — the
  deterministic-value class** (population-density-data / network-traffic-analysis
  precedent): the vendored 200 `partial` example claimed `count: 3142` (a hand-derived
  half of the `supported` 6283) but the handler computes the full count for the
  concrete `…001` radius *first*, then integer-halves — radius 2001 → full 6289,
  `6289/2 = 3144`. Corrected the example to 3144 and bound all three 200 example
  summaries to their concrete reproducing radii (2000→6283 SUPPORTED, 2001→3144 PART,
  2003→DENSITY_BELOW). The three inline 400 error examples (`invalidCircleArea`,
  `unsupportedSync`, `unsupportedRequest`) already byte-matched the handler; the
  401/403/429 responses `$ref` the shared catalog. The module docs
  (`region_device_count.rs` + `v0_2.rs`) and the `x-camarasim-scenarios` block were
  already accurate (one operation, async sink a documented cut) — no stale-slice
  drift. Added `vendored_examples_match_the_handler` driving all three documented 200
  requests through the real router and pinning each body as an **exact object** (prior
  tests asserted `status`/`count` individually and allowed a stray field), plus the
  exact `status`/`code`/`message` bytes of all three inline 400 families (prior error
  tests guarded only `code`). Spec + `#[cfg(test)]` change only — no behaviour/response
  change (the handler was already correct; one example `count` was hand-derived), no
  new dependency.
- ✅ qos-provisioning v0.3 — spec↔code audited 2026-10-15 (Phase 5 stable tier, seventeenth;
  the first of the two remaining **stateful** stable APIs — a `store.rs`-backed open-ended
  provisioning lifecycle with a `notifications.rs` CloudEvent sink, the QoD sibling): all four
  operations (`createQosAssignment` `POST /qos-assignments`, `getQosAssignmentById`
  `GET /qos-assignments/{id}`, `revokeQosAssignment` `DELETE /qos-assignments/{id}`,
  `getQosAssignmentByDevice` `POST /retrieve-qos-assignment`) reviewed end to end against
  `specs/qos-provisioning/v0.3/openapi.yaml`. The control planes match the handler (identifier
  reserved-error-suffix incl. `…409`→CONFLICT; the grant plane `…000`/no-digits → REQUESTED
  without `startedAt`, else AVAILABLE with `startedAt`=now and no `expiresAt` — open-ended; a
  `qosProfile` containing `unavailable` → 422 QOS_PROFILE_NOT_APPLICABLE; the `http(s)` `sink`
  validation + the AVAILABLE/`…001` NETWORK_TERMINATED/DELETE_REQUESTED CloudEvents with
  ACCESSTOKEN→Bearer / PLAIN→Basic auth), the two-legged/three-legged identifier resolution is
  faithful, and the scopes + `x-correlator` echo are enforced. The minted `assignmentId`
  (counter+clock `mint_uuid`) and wall-clock `startedAt` are legitimately illustrative (the QoD
  posture). **Drift found & fixed — four message/example classes.** (1) The **truncated-identifier
  -message class** (connected-network-type / simple-edge-discovery / carrier-billing precedent):
  the 422 `missingIdentifier` example (in **both** createQosAssignment and retrieve) carried the
  short "The device cannot be identified." but `resolve_identifier` emits the full "…: supply a
  `device` or use a token that identifies a device." Corrected both. (2) The createQosAssignment
  400 `invalidSink` example said "…valid `https://` callback URL." but the handler (which accepts
  `http://` too) emits "…valid `http://` or `https://` callback URL." (3) The 409 `conflict`
  example claimed "There is a conflict with the current state of the resource." but `…409` →
  `for_status(409)` emits "A specified resource duplicate entry found." (4) The
  deterministic/illustrative class: the three 200/201 success examples (createQosAssignment
  `available`/`requested`, retrieve `available`) carried Swagger-placeholder `assignmentId`s
  (`3fa85f64-…`) and frozen `startedAt`s with no illustrative note — marked `assignmentId`/
  `startedAt` illustrative (the QoD `sessionId`/`startedAt` convention) with a derivation note.
  **Stale-doc drift fixed — the no-op-TLS-cut class:** three code sites (the `v0_3.rs` header, the
  create-handler sink comment, and a create test comment) claimed `https://` sink delivery was a
  "documented no-op cut (no TLS client)", contradicting `notifications.rs` (which delivers over a
  verified `rustls` TLS session) and the served spec — corrected all three. **Stale-slice-doc
  drift fixed:** the parent `qos_provisioning.rs` module doc still said "First slice: POST
  /qos-assignments + GET /…/{id}" though all four operations ship — corrected to the full surface.
  Added `vendored_examples_match_the_handler` (drives the two 201 + the retrieve 200 through the
  real router, pins each body as an **exact object** minus the illustrative `assignmentId`/
  `startedAt`, asserted UUID-shaped/UTC and ≠ the frozen literals; prior tests asserted these
  fields individually) and `vendored_error_example_messages_match_the_handler` (pins the exact
  `status`/`code`/`message` bytes of all nine router-reachable inline error families across both
  operations; prior error tests guarded only `status`/`code`). Spec + doc + `#[cfg(test)]` change
  only — no behaviour/response change (the handlers were already correct; the drift was message
  text, example placeholders, and stale docs), no new dependency. Only in-home-device-management v1
  remains in the Phase 5 stable tier.
- ✅ sms v0alpha1 — spec↔code audited 2026-10-15 (Phase 5 stable tier, sixteenth; the next
  **stateless** stable API after region-device-count, taken before the two remaining stateful
  ones — qos-provisioning v0.3, in-home-device-management v1 — per phase order). The one
  operation (`send-sms`, `POST /sms/v0alpha1/short-message`) reviewed end to end against
  `specs/sms/v0alpha1/openapi.yaml`. The handler is faithful — two-legged/business-facing (both
  `from` and `to` in the body, no subject fallback, so no identifier dance), the first-recipient
  control plane (`to[0]`'s reserved trailing suffix → the canonical shared CAMARA error; a
  reserved suffix on `from` or on a later recipient is **not** the error plane), the deterministic
  SHA-256 `msgId` (over `from` + all of `to` + `message`, v4-shaped, no `uuid`/`rand` dep), and
  the full validation set (malformed/unknown-field/unknown-category body, empty `to`, non-E.164
  `from`/recipient, empty `message` → 400 `INVALID_ARGUMENT`) all match, and the scope + `x-correlator`
  echo are genuinely enforced. **Drift found & fixed — the deterministic-value/placeholder +
  illustrative-timestamp classes** (verified-caller / device-identifier / connected-network-type
  precedent): the 200 `sent` example carried a Swagger-placeholder `msgId`
  (`3fa85f64-5717-4562-b3fc-2c963f66afa6`) bound to no request though `msgId` is fully
  reproducible, and a frozen `timestamp` (`2024-01-01T14:27:08Z`) though the handler stamps the
  current instant (wall-clock, can never reproduce). Bound the example to the concrete request
  (`"hi"` from `+123456789111` to `+123456789012`), corrected `msgId` to the real derived value
  `ceb0abb4-354b-46af-a9f3-845e4bd92fb0`, and marked `timestamp` illustrative with a derivation
  note (its presence/RFC-3339-UTC shape is the contract, not the value). The 400 `invalidArgument`
  example message already byte-matched the handler; every 401/403/404/409/422/429/500/503 response
  `$ref`s the shared catalog (no other inline examples to drift), and the parent/version module
  docs were already accurate (one operation, the delivery-notification-subscription surface a
  documented cut) — no stale-slice drift. Added `vendored_sent_example_matches_the_handler`
  (drives the bound request through the real router, pins `msgId` against both the spec literal and
  `message_id()`, pins the `timestamp` shape as UTC) and `vendored_invalid_argument_example_matches_the_handler`
  (pins the 400 example's exact `status`/`code`/`message` bytes; prior error tests guarded only
  `code`). Spec + `#[cfg(test)]` change only — no behaviour/response change (the handler was
  already correct; one example placeholder + one illustrative timestamp), no new dependency.
  Binary 5,459,096 bytes. Next in phase order: the two remaining stateful Phase 5 stable APIs —
  qos-provisioning v0.3, then in-home-device-management v1.
- ✅ in-home-device-management v1 — spec↔code audited 2026-10-15 (Phase 5 stable tier, **eighteenth
  and last**; the second of the two remaining **stateful** ones — a `store.rs`-backed
  tombstone + overlay layer over an otherwise `ssid`-derived roster): all six operations
  (`listDevices` `GET /devices`, `getDevice` `GET /devices/{id}`, `getDeviceNetworkHealth`
  `GET …/network-health`, `performDeviceAction` `POST …/actions/{actionId}`, `deleteDevice`
  `DELETE /devices/{id}`, `updateDevice` `PATCH /devices/{id}`) reviewed end to end against
  `specs/in-home-device-management/v1/openapi.yaml`; handler/store faithful (the `ssid`
  reserved-suffix error plane; the `d % 6` roster-shape plane + `connectionStatus` filter; the
  delete tombstone single-use gate; the update overlay with blocked>paused folded into
  `connectionStatus`; the connected→applied/offline→accepted action plane; read/write scope
  gates + `x-correlator` echo). **Stale-slice-doc drift found & fixed** (the device-swap /
  carrier-billing / qos-provisioning precedent): both the parent module doc
  (`src/apis/in_home_device_management.rs`) and the vendored spec header still called
  `updateDevice` "a later slice" though PATCH is fully routed, documented in the spec's own
  `patch:` block, and has ~16 tests — corrected both to describe it as the implemented second
  mutation (overlay). **Illustrative-value class** (device-identifier / sms / carrier-billing
  precedent): the 200/201 success examples presented opaque FNV-1a–derived `deviceId`/`macAddress`/
  `actionId`, hash-derived health figures (`radioFrequency`/`rssiDbm`/`wifiCompatibility`/
  `lastNetworkSpeedMbps`/`networkCongestion`) and frozen wall-clock `measuredAt`/`appliedAt` as if
  literal — none reproduce the hand-written bytes/instant — so marked them illustrative with a
  derivation note on each operation's 200/201 response `description`, keeping the reproducible
  scalars (`total`, `deviceName`/`deviceType`/`connectionStatus`/`blocked`/`paused`/`ipAddress`/
  `infraDevice`/`interfaceType`, `status`/`actionType`, the `maxPhyRateMbps` 1000 wired constant)
  as the contract. All inline 400 `INVALID_ARGUMENT` examples already byte-matched the handler;
  every 401/403/404/409/422/429/500/503 `$ref`s the shared catalog. Added two `#[cfg(test)]` pins:
  `vendored_success_examples_are_illustrative_shapes_matching_the_handler` (drives listDevices/
  getDevice/getDeviceNetworkHealth/performDeviceAction through the router, pins the reproducible
  scalars and asserts the illustrative `deviceId`/`macAddress`/`actionId`/`measuredAt`/`appliedAt`
  are derived/fresh and ≠ the frozen spec literals) and
  `vendored_invalid_argument_examples_match_the_handler` (pins the exact status/code/message bytes
  of all five operations' 400 example; prior 400 tests guarded only `code`). Spec + doc +
  `#[cfg(test)]` change only — no behaviour/response change (handlers already correct). **This
  completes the Phase 5 stable/numbered tier (all 18 audited).**

### Phase 5 — Remaining APIs (CAMARA `vwip` — deepen scenarios/errors/contracts)
- 🔧 device-data-volume — verified 2026-10-12: both operations
  (`retrieveDataVolume`, `checkDataVolume`) implemented, tested, and documented in
  `specs/device-data-volume/vwip/openapi.yaml`; stale "`/check` is a later slice"
  module doc corrected. **Re-audited 2026-10-18 for the drift classes discovered
  *after* the vwip sweep closed (2026-10-13)** — the sweep predated the
  truncated-identifier-message class first found 2026-10-14. **Drift found & fixed:**
  the 422 `missingIdentifier` example in **both** operations carried the short
  "The device cannot be identified." but `resolve_identifier` (`vwip.rs:387`) emits
  the full "…: supply a `device` or use a token that identifies a device."
  Corrected both. The other inline messages (`retrieve` 400 INVALID_ARGUMENT /
  422 UNNECESSARY_IDENTIFIER; `check` 400 OUT_OF_RANGE / 400 INVALID_ARGUMENT /
  422 UNNECESSARY_IDENTIFIER) already byte-matched; the 404 reserved-suffix / other
  4xx/5xx examples show the shared catalog's generic message. Added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of all seven router-reachable API-own inline error
  families across both operations (prior error tests guarded only `code`). Spec +
  `#[cfg(test)]` change only — no behaviour/response change, no new dependency.
- 🔧 device-visit-location — verified 2026-10-12; **vwip residual-drift re-audit
  2026-10-18 (truncated-identifier-message class — last of the three vwip specs
  flagged on 2026-10-18 by the device-data-volume pass; drift found & fixed)**: the
  one modelled operation (`retrieveDeviceVisitLocation`,
  `POST /device-visit-location/vwip/retrieve`) is implemented, tested, and
  documented in `specs/device-visit-location/vwip/openapi.yaml`. The 422
  `missingIdentifier` example carried the short "The device cannot be identified."
  but `resolve_identifier` (`vwip.rs:293`) emits the full "…: supply a `device` or
  use a token that identifies a device." (router-reachable: a two-legged
  `client_credentials` token's `sub` is a non-line client id — existing
  `no_device_and_non_line_subject_is_missing_identifier` proves the branch).
  Corrected it; the five other API-own inline examples (400 INVALID_ARGUMENT /
  DEVICE_VISIT_LOCATION.INVALID_END_DATE, 404 DEVICE_VISIT_LOCATION.DATA_NOT_FOUND,
  422 UNNECESSARY_IDENTIFIER, and the MISSING_IDENTIFIER code/status) already
  byte-matched. Added `vendored_error_example_messages_match_the_handler` pinning
  the exact `status`/`code`/`message` bytes of all five router-reachable API-own
  inline error families (prior per-case tests guarded only `status`/`code`); the
  reserved-suffix 404 `NOT_FOUND` example is `for_status(404)` (shared catalog), so
  left unpinned. **This closes the three-spec vwip residual-drift re-audit.** Spec +
  `#[cfg(test)]` change only, no behaviour change, no new dependency.
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
- 🔧 application-profiles — verified 2026-10-12: all four modelled CRUD operations
  (`createApplicationProfile`, `readApplicationProfile`, `updateApplicationProfile`,
  `deleteApplicationProfile`) are implemented (`src/apis/application_profiles/vwip.rs`
  + `store.rs`), routed, and well-tested, and the vendored
  `specs/application-profiles/vwip/openapi.yaml` is consistent (full CRUD set, scopes,
  error model, scenarios; the 201 `created` example's `applicationProfileId` is
  legitimately illustrative — ids are minted from an internal counter + clock, not
  the request, so a consumer cannot reproduce them, unlike the deterministic ids in
  verified-caller / optimal-edge-discovery). **Two stale "first slice / later passes"
  module docs fixed** (same trap class as device-data-volume / qos-booking):
  `application_profiles.rs` parent doc said "First slice: POST + GET {id}" (omitting
  PATCH + DELETE) and `store.rs` said PATCH/DELETE were "in later passes" though
  `replace`/`remove` ship in that same file. Added
  `vendored_error_example_messages_match_the_handler` pinning the four distinct 400
  example `message` bytes (no prior test guarded them, only `code`).
- 🔧 subscription-status — verified 2026-10-13: the one modelled operation
  (`retrieveSubscriptionStatus`, `POST
  /subscription-status/vwip/retrieve-subscription-status`) is implemented,
  tested, and documented in `specs/subscription-status/vwip/openapi.yaml`;
  the status-bitfield mapping (bit 0 → `voiceSmsIn`, bit 1 → `voiceSmsOut`,
  `(d>>2)%3` → `dataService`) and the two-legged/three-legged identifier
  resolution all match the handler. **Spec↔code drift found and fixed in the
  vendored 422 `missingIdentifier` example:** it carried the truncated
  `message: "The phone number cannot be identified."` but the handler emits the
  full "…: supply `phoneNumber` or use a token that identifies a line." (the
  same truncated-identifier-message drift class caught on media-streaming-rate
  and optimal-edge-discovery). Corrected the example and added
  `vendored_error_example_messages_match_the_handler` pinning the 400
  `invalidArgument`, 422 `missingIdentifier`/`unnecessaryIdentifier`, and 422
  `serviceNotApplicable` message bytes through the router. The 200 examples
  (`…000`/`…001`/`…008`) and the `serviceNotApplicable`/`unnecessaryIdentifier`
  messages already matched.
- 🔧 device-authenticity — verified 2026-10-13: the one modelled operation
  (`checkImeiStatus`, `POST /device-authenticity/vwip/check-status`) is
  implemented (`src/apis/device_authenticity/vwip.rs`), routed, and well-tested,
  and both module docs agree with the code and the vendored
  `specs/device-authenticity/vwip/openapi.yaml`: the IMEI-keyed identifier (no
  two-legged/three-legged fallback), the `d % 9` operational-status plane over the
  nine-value enum (`…000`→`allowed` no `reportedDate`, `…002`→`stolen` with
  `reportedDate = now − d h`), and the reserved-error suffix plane all match. No
  spec↔code drift found (the 200 examples' mapping is already pinned by existing
  status tests; the `IDENTIFIER_NOT_FOUND`/`QUOTA_EXCEEDED` schema enum members
  are the canonical API's own codes, intentionally superseded by the shared
  reserved-error convention — documented divergence, not drift). **Small tested
  increment:** the two *inline* error examples' `message` bytes (400
  `invalidArgument`, 422 `serviceNotApplicable`) were guarded only for `code`;
  added `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes through the router so they can't silently drift
  (same drift class the media-streaming-rate / subscription-status passes closed).
- 🔧 session-insights — verified 2026-10-13: all five modelled operations
  (`createSession`, `getSession`, `deleteSession`, `retrieveSessionsByDevice`,
  `sendSessionMetrics`) are implemented (`src/apis/session_insights/vwip.rs` +
  `store.rs` + `notifications.rs`), routed, and well-tested (87 test fns), and the
  vendored `specs/session-insights/vwip/openapi.yaml` declares the same five
  operations — spec ↔ code consistent (the 201 success examples' minted `id`s are
  legitimately illustrative, as in application-profiles, since `store::new_session_id`
  is counter/clock-derived not request-derived; time-bounded vs `…000`-tail
  open-ended `expiresAt` presence matches `create_session`; all inline error-example
  messages byte-match the handler). **Stale parent module doc fixed**
  (`src/apis/session_insights.rs`): it listed `DELETE`, `retrieve-sessions`, and
  `metrics` as "in later passes" though all three ship — the same stale-slice-doc
  trap caught on device-data-volume / qos-booking / application-profiles. Added
  `vendored_error_example_messages_match_the_handler` pinning the four
  router-reachable inline error-example families (createSession 400
  `invalidArgument`, createSession 422 `serviceNotApplicable`, sendSessionMetrics
  400 `invalidArgument`, sendSessionMetrics 400 `outOfRange`); the createSession 422
  `missingIdentifier` example is not router-reachable (its branch needs an
  empty-`sub` token the token endpoint never issues) and is documented as such.
- 🔧 consent-info — verified 2026-10-13: the one modelled operation
  (`retrieveStatus`, `POST /consent-info/vwip/retrieve`) is implemented
  (`src/apis/consent_info/vwip.rs`), routed, and well-tested, and the vendored
  `specs/consent-info/vwip/openapi.yaml` is consistent (the `d % 6` consent-state
  plane, the `requestCaptureUrl` capture-URL gate, the two reserved 403s, and the
  two-legged/three-legged identifier resolution all match the handler).
  **Spec↔code drift found and fixed in the vendored 422 `missingIdentifier`
  example:** it carried the truncated `message: "The phone number cannot be
  identified."` but `resolve_identifier` emits the full "…: supply `phoneNumber`
  or use a token that identifies a line." (the same truncated-identifier-message
  drift class caught on subscription-status / media-streaming-rate /
  optimal-edge-discovery). Corrected the example and added
  `vendored_error_example_messages_match_the_handler` pinning the six
  router-reachable inline example families whose messages are the API's own —
  400 `invalidArgument`, 403 `notAllowedScopesPurpose`, 403 `invalidCallbackUrl`,
  422 `missingIdentifier`/`unnecessaryIdentifier`/`serviceNotApplicable` — through
  the router. The 403 `permissionDenied` example intentionally shows the shared
  catalog's canonical message (the auth layer's runtime message is scope-specific),
  so it is not pinned here; the 401/404/409/429/500/503 responses are `$ref`s.
- 🔧 iot-sim-fraud-prevention — verified 2026-10-13: all three modelled operations
  over both CAMARA facets — `bindDeviceImei` (`POST /bind`), `query` (`POST /query`),
  `unBindDeviceImei` (`POST /unbind`), each carrying `IMEIBIND` and `AREALIMIT` — are
  implemented (`src/apis/iot_sim_fraud_prevention/vwip.rs` + an in-memory `store.rs`
  with a disjoint IMEI-binding map and area-restriction set), routed, and well-tested
  (46 test fns before this pass). **Spec↔code drift found and fixed, two classes.**
  (1) The 200 `IMEI_BIND_BOUND` success example carried `bindImei
  "352099000000117"` — not even a valid GSMA Luhn IMEI — but the handler's
  deterministic `synth_imei` emits `352099000000113` for the documented
  `+123456789011` (TAC `35209900` + serial `000011` + Luhn check digit `3`); the same
  deterministic-value drift class the verified-caller / optimal-edge-discovery passes
  caught. Corrected it and bound the example to its input. (2) Every inline error
  example `message` had been transcribed from the upstream CAMARA spec and never
  reconciled with this handler: the four 400 query examples ("At least one of
  phoneNumber…", "QueryType must be specified", "Invalid format: phoneNumber", "The
  value of queryType can only be…"), both 400 bind/unbind examples, and both 422
  `MISSING_IDENTIFIER` examples ("The device cannot be identified.") all differed from
  the bytes the server returns, and the 422 `UNNECESSARY_IDENTIFIER` example dropped
  the handler's trailing period. Corrected all of them to the handler's exact messages
  (collapsing the two queryType/bindType/unBindType 400 examples that share one serde
  message), and added `vendored_examples_match_the_handler`, which drives every
  documented request through the real router and pins the success `bindImei` (both as
  the literal and against `synth_imei(11)`), the `AREALIMIT` `Circle` (against
  `synth_circle(11)` and the documented −79.0/−169.0/11), and the exact
  `status`/`code`/`message` bytes of all router-reachable inline error families so they
  can never silently drift again. The `AREA_LIMIT_RESTRICTED` circle, the reserved-error
  suffix plane, and the shared-`$ref` 401/403/404/429 responses were already correct.
  This advances the vwip audit cursor to the next API (sponsored-data). Spec + test
  change (no behaviour/response change — the handlers were already correct; only the
  spec examples were wrong). No new dependency.
- 🔧 sponsored-data — verified 2026-10-13: all seven modelled operations across
  both facets — the sponsorship lifecycle (`startSponsorship`, `getSessionStatus`,
  `revokeSponsorship`) and the campaign operations (`getCampaignStatus`,
  `getActiveSponsorships`, `configureAlerts`, `manageCampaign`) — are implemented
  (`src/apis/sponsored_data/vwip.rs` + an in-memory `store.rs` and a
  `notifications.rs` firing the `session_revoked` webhook), routed, and well-tested
  (73 test fns before this pass), and the vendored
  `specs/sponsored-data/vwip/openapi.yaml` is consistent with the code: every
  derived success example byte-matches the handler — `getSessionStatus`
  (`+123456789012` → 12/38 active; `+123456789050` → 50/0 `data_exhausted`),
  `getCampaignStatus` (campaignId UUID tail `000` → active/prepaid/1000 remaining,
  `010` → completed/`data_exhausted`/0 remaining, `003` → paused/postpaid/3 used),
  and the fixed acknowledgement strings (`successful_revocation`, `CAMPAIGN PAUSED
  SUCCESSFULLY`, `Campaign notifications subscription - SUCCESS`); minted
  `sessionId`s are legitimately illustrative (counter/clock-derived, the
  application-profiles posture, not request-reproducible). **Stale parent module
  doc fixed** (`src/apis/sponsored_data.rs`): it said the campaign-management
  operations `configureAlerts`/`manageCampaign` "remain deferred to later work"
  though both ship and are routed — the same stale-"deferred/later slice" doc trap
  caught on device-data-volume / qos-booking / application-profiles /
  session-insights; corrected to list all seven operations as implemented. **Small
  tested increment:** the spec's two inline 400 error examples (`startSponsorship`
  → `invalidArgument` / `outOfRange`, the only error examples not `$ref`ing the
  shared catalog) were exercised by existing tests only for their `code`; added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of both through the router so they can't drift.
  This advances the vwip audit cursor to the next API (click-to-dial).
- 🔧 click-to-dial — verified 2026-10-13: all four modelled operations
  (`createCall` `POST /calls`, the stateful `getCall` `GET /calls/{callId}`,
  `terminateCall` `DELETE /calls/{callId}`, `getRecording`
  `GET /calls/{callId}/recording`) are implemented
  (`src/apis/click_to_dial/vwip.rs` + an in-memory `store.rs` with a disjoint
  sink side-store + a `notifications.rs` firing the create/terminate/progression
  `status-changed` CloudEvents), routed, and well-tested (~95 test fns), and the
  vendored `specs/click-to-dial/vwip/openapi.yaml` is consistent with the code
  (the full CRUD-ish set, scopes, reserved-error + availability/recording control
  planes, and the simulated-progression notification cuts all match). **Spec↔code
  drift found and fixed, placeholder-UUID class:** the `created`, `found`, and
  `recording` examples all carried the Swagger placeholder `callId`
  `3fa85f64-5717-4562-b3fc-2c963f66afa6`, but `call_id` derives the id
  deterministically (SHA-256 over `caller` + `\0` + `callee`, v4-shaped) — the
  documented `created` pair `+123456789111`/`+123456789012` actually yields
  `40c0c2be-6c32-41b9-bc58-e5cc1a25f8b3` and the `found` pair
  `+123456789111`/`+123456789222` yields `a2b3b31d-6b8e-49b0-8487-06f70764d7bd`
  (the same deterministic-id drift the verified-caller / optimal-edge-discovery /
  iot-sim-fraud-prevention passes caught). Corrected all three (tying the
  `recording` example to the `found` call's id, so it is the recording of that
  recorded call) and added `vendored_examples_match_the_handler`, which pins the
  documented pair→literal mapping directly against `call_id` and proves the router
  emits `call_id(pair)` + the fixed silent WAV on a pair unique to this test
  (the store-keyed `createCall` 409s on a shared fixed pair, so the documented
  pairs can't be re-created through the router). All inline error-example
  `message` bytes (400 `INVALID_ARGUMENT`, 409 `ALREADY_EXISTS`, 422
  `INVALID_PHONE_NUMBER`/`SAME_CALLER_CALLEE`/`CALLER_NOT_AVAILABLE`/
  `CALLEE_NOT_AVAILABLE`/`RECORDING_NOT_SUPPORTED`, 404 `NOT_FOUND`×2) and the
  recording `content` base64 were already byte-accurate against the handler. This
  advances the vwip audit cursor to the next API (most-frequent-location). Spec +
  test change (no behaviour/response change — the handler was already correct;
  only the three example ids were placeholders); no new dependency.
- 🔧 most-frequent-location — verified 2026-10-13: the one modelled operation
  (`verifyFrequentLocation`, `POST /most-frequent-location/vwip/verify`) is
  implemented (`src/apis/most_frequent_location/vwip.rs`), routed, and well-tested
  (23 test fns before this pass), and the vendored
  `specs/most-frequent-location/vwip/openapi.yaml` is consistent with the code:
  the two-plane score (`(identifier trailing three digits + area offset) % 101`,
  area offset = `round(|lat|)+round(|long|)` for a `COVERAGE_ZONE` or the postal
  code's trailing three digits), the `geoReference` validation (`COVERAGE_ZONE`
  range → 400 `OUT_OF_RANGE`; reserved `00000` → 400
  `MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID`; malformed → 400
  `INVALID_ARGUMENT`), the two-legged/three-legged identifier resolution, and the
  reserved-error suffix plane (`…422` → `SERVICE_NOT_APPLICABLE`) all match the
  handler. **Spec↔code drift found and fixed in the vendored 422
  `missingIdentifier` example:** it carried the truncated `message: "The device
  cannot be identified."` but `resolve_identifier` emits the full "…: supply a
  `device` or use a token that identifies a device." (the same
  truncated-identifier-message drift class caught on subscription-status /
  media-streaming-rate / optimal-edge-discovery / consent-info). Corrected the
  example and added `vendored_error_example_messages_match_the_handler` pinning the
  exact `status`/`code`/`message` bytes of all six router-reachable inline error
  families (400 `invalidArgument`/`outOfRange`/`postalCodeNotValid`, 422
  `missingIdentifier`/`unnecessaryIdentifier`/`serviceNotApplicable`) through the
  router. The 200 examples (score 50 / 0) and the shared-`$ref`
  401/403/404/409/429/500/503 responses already matched. This advances the vwip
  audit cursor to the next API (traffic-influence).
- 🔧 traffic-influence — verified 2026-10-13: all six modelled operations
  (`postTrafficInfluence` `POST /traffic-influences`, `getAllTrafficInfluences`
  `GET /traffic-influences` with optional `appId` filter, `postTrafficInfluenceDevice`
  `POST /traffic-influence-devices`, the stateful `getTrafficInfluence`
  `GET /traffic-influences/{id}`, `patchTrafficInfluence` `PATCH …/{id}`,
  `deleteTrafficInfluence` `DELETE …/{id}`) are implemented
  (`src/apis/traffic_influence/vwip.rs` + an in-memory `store.rs` + a
  `notifications.rs` firing the create-time initial-event CloudEvent), routed, and
  well-tested (~87 test fns before this pass). All seven inline `400` error-example
  message bytes already matched the handler, and the minted `trafficInfluenceID`s
  are legitimately illustrative (`mint_id` is counter/clock-derived, the
  application-profiles posture — not request-reproducible). **Spec↔code consistency
  defect found and fixed in the GET-list `two` example:** it showed two resources
  that *share* `appId …426614174002` but carry different states (one `active`, one
  `created`), an impossible combination — `state` is a pure function of the `appId`
  tail (`derive_state`: `d%3` 0→ordered/1→created/2→active) and PATCH does not
  re-derive it, so two resources with the same appId always share one state
  (`…002`→`active`). Corrected the second resource to `active` and clarified the
  example summary. Also **fixed the stale spec header comment** (it listed five
  operations, omitting `getAllTrafficInfluences`, though the body defines it — the
  stale-doc trap class). Added `vendored_examples_match_the_handler` pinning the
  shared-appId→state consistency (`derive_state(…002)=="active"`, guarding the
  `two`/201/get/patch examples) and the exact `status`/`code`/`message` bytes of all
  router-reachable inline `400` families (postTrafficInfluence invalidArgument/
  outOfRange, getAll invalidArgument, postDevice missingDevice/emptyDevice, patch
  invalidArgument/outOfRange). This advances the vwip audit cursor to the next API
  (application-endpoint-discovery). Spec + test change (no behaviour/response change
  — the handler was already correct; only the list example was self-inconsistent);
  no new dependency.
- 🔧 application-endpoint-discovery — verified 2026-10-13: the one modelled
  operation (`getOptimalAppEndpoints`,
  `POST /application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints`) is
  implemented (`src/apis/application_endpoint_discovery/vwip.rs`), routed, and
  well-tested (29 test fns before this pass), and the module docs agree with the
  code and the vendored `specs/application-endpoint-discovery/vwip/openapi.yaml`:
  the structural application-identifier plane (exactly one UUID `appId` /
  `applicationEndpointsId`; `…000` → 404; else `(d % 3) + 1` endpoints with a
  rotating fqdn/ipv4/ipv6 address family), the device reserved-error-suffix plane
  (checked before the app-not-found 404), and the two-legged/three-legged
  identifier resolution all match. **Spec↔code drift found and fixed — the
  deterministic-value / placeholder class** (same as verified-caller /
  optimal-edge-discovery / click-to-dial / iot-sim-fraud-prevention): the vendored
  `single` 200 example, explicitly bound to `appId …012`, carried placeholder
  values `fqdn: app-1a2b3c4d.edge.camarasim.example.com` and `edgeCloudZoneId:
  5d803d3f-…`, but the handler derives both deterministically via SHA-256 — appId
  …012 actually yields `app-d08c37d8.edge.camarasim.example.com` and (for
  `camarasim-edge-eu-west-1`) `b89bb689-d520-cfa4-6542-021d73c84510`. The
  `EdgeCloudZone` schema example paired the same zone name with the same wrong id
  (the id is a pure function of the name), so it was corrected to the real value
  too. Corrected all three and added `vendored_success_example_matches_the_handler`,
  which drives the real router for appId …012 and pins the exact `port` (1036),
  `fqdn`, single-address-family shape, `edgeCloudZone` fields (id/name/provider/
  region/status), description, echoed `appId`, `applicationServerProviderName`,
  and the absent-device case. Two further inline error examples had previously-seen
  drift, both fixed in the same pass: the 404 `notFound` example ("was"→"is" not
  found) and the 422 `missingIdentifier` example (truncated identifier message);
  added `vendored_error_example_messages_match_the_handler` pinning the 400/404/422
  inline error families through the router. The reserved-suffix plane and the
  request/response schemas already matched. This advances the vwip audit cursor to
  the next API (application-endpoint-registration).
- 🔧 application-endpoint-registration — verified 2026-10-13: all five modelled
  operations over the full registration lifecycle — `registerApplicationEndpoints`
  (`POST /application-endpoint-lists` → 200 with a bare minted
  `applicationEndpointListId`), `getAllRegisteredApplicationEndpoints`
  (`GET /application-endpoint-lists`, sorted array, never 404s),
  `getApplicationEndpointsById` (`GET …/{id}`), `updateApplicationEndpoint`
  (`PUT …/{id}` full replace → 204), and `deregisterApplicationEndpoint`
  (`DELETE …/{id}` → 204, single-use) — are implemented
  (`src/apis/application_endpoint_registration/vwip.rs` + an in-memory `store.rs`),
  routed, and well-tested (44 test fns before this pass), and the vendored
  `specs/application-endpoint-registration/vwip/openapi.yaml` is consistent with the
  code: the two request-body/`applicationProfileId` control planes
  (body validation → 400 `INVALID_ARGUMENT`/`OUT_OF_RANGE`; the shared reserved-error
  suffix on `applicationProfileId` e.g. `…404`/`…422`; the nil-UUID sentinel → 422
  `UNIDENTIFIABLE_APPLICATION_PROFILE`) and the opaque server-minted id's store state
  (non-UUID path → 400, unknown id → 404) all match the handler, and the minted
  `applicationEndpointListId` is legitimately *illustrative* (`store::new_list_id` is
  counter/clock-derived, the application-profiles / session-insights posture, not
  request-reproducible). **No spec↔code drift found — every inline error example was
  already byte-accurate.** Fixed one **stale store-module doc** (same trap class as
  device-data-volume / qos-booking / application-profiles / session-insights /
  sponsored-data): `store.rs` said the `GET {id}` / `PUT` / `DELETE` legs were
  "landing in later passes" and the store was "trimmed to what the register slice
  needs," though all four legs ship and `get`/`all`/`replace`/`remove` are all present
  — corrected to describe the full lifecycle. **Small tested increment:** every
  existing error test asserted only `status`/`code`, so no test guarded the inline
  example `message` bytes; added `vendored_error_example_messages_match_the_handler`
  pinning the exact `status`/`code`/`message` bytes of all router-reachable inline
  error families through the router — register 400 `invalidArgument`/`outOfRange`,
  register 422 `unidentifiableProfile`/`serviceNotApplicable`, getById 400, delete
  400, and update 400 (body + path) / 422 — so they can't silently drift (the same
  pinning the device-authenticity / network-health-assessment passes added). This
  advances the vwip audit cursor to the next API (predictive-connectivity-data).
  Spec is unchanged (examples were correct); doc + test change only; no new
  dependency.
- 🔧 predictive-connectivity-data — verified 2026-10-13: the one modelled operation
  (`retrieveConnectivity`, `POST /predictive-connectivity-data/vwip/retrieve`) is
  implemented (`src/apis/predictive_connectivity_data/vwip.rs`), routed, and
  well-tested (35 test fns before this pass), and the module docs agree with the
  code and the vendored `specs/predictive-connectivity-data/vwip/openapi.yaml`: the
  area-keyed geometry control plane (first-geohash reserved-suffix errors; each
  geohash's FNV-1a hash fixing its cell — `h%7==0`→NO_DATA, else ground score
  `h%100` degrading 15 points per 30 m layer; the `serviceLevel` thresholds and
  requested `height`→layer-count planes; the documented `POLYGON` / async-sink /
  precision cuts) all match the handler. **Spec↔code drift found and fixed in the
  vendored `200` `partlySupported` example — two defects of the NO_DATA-mislabel /
  deterministic-value class** (the same class caught on population-density-data):
  (1) `gbsuv` was shown as a NO_DATA cell (`[ND,ND,ND,ND]`), but
  `fnv1a("gbsuv")%7==1` → it is a *data* cell (ground 62), so two data cells would
  make the area `SUPPORTED_AREA`, not the claimed `PART_OF_AREA_NOT_SUPPORTED`; and
  (2) `u4pruy`'s `[GC,MC,NC,NC]` holds only under `BEST_EFFORT` (ground 36), but the
  example named no `serviceLevel`. Rebound the example to a concrete request
  (`BEST_EFFORT` over `["u4pruy","dr5bb"]`, the documented 7-day window) and swapped
  `gbsuv` for `dr5bb`, which genuinely hashes to a NO_DATA cell (`%7==0`), so the
  mixed area is correctly partly-unsupported. Added
  `vendored_success_example_matches_the_handler` (drives the router and pins the
  exact documented cells + status) and `vendored_error_example_messages_match_the_handler`
  (pins the six inline `400`/`422` example `message` bytes — invalidArgument,
  invalidEndTime, maxTimePeriodExceeded, unsupportedAreaType, unsupportedPrecision,
  unsupportedSyncResponse — which the existing error tests guarded only for
  `status`/`code`). All inline error-example messages were already byte-accurate.
  This advances the vwip audit cursor to the next API (network-access-devices).
  Spec + `#[cfg(test)]` change (no behaviour/response change — the handler was
  already correct; only the one success example was wrong); no new dependency.
- 🔧 network-access-devices — verified 2026-10-13: all five modelled operations
  over the full device-listing + stateful reboot-request lifecycle —
  `getNetworkAccessDevices` (`GET /network-access-devices`),
  `getNetworkAccessDevice` (`GET …/{id}`), `createRebootRequest`
  (`POST /reboot-requests`), `getRebootRequest` (`GET …/{id}`),
  `updateRebootRequest` (`PATCH …/{id}`, merge-patch gated on schedule state →
  409 `NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE`), and `deleteRebootRequest`
  (`DELETE …/{id}`) — are implemented (`src/apis/network_access_devices/vwip.rs`
  + `store.rs`), routed, and well-tested (74 test fns before this pass), and the
  vendored `specs/network-access-devices/vwip/openapi.yaml` declares the same set
  with the 409 inline example byte-matching the handler. **Spec↔code drift found
  and fixed — the deterministic-value / placeholder class** (as on
  application-endpoint-discovery / click-to-dial / verified-caller): the `200`
  `singleConnected`/`twoDevices` listing examples and the `connectedGateway`
  by-id example were bound to concrete subject tails (`…000`/`…002`) yet carried
  placeholder device `id`s (`3fa85f64-…`, `123e4567-…`) and MACs, though each
  `id` and `hardwareAddress.value` is a deterministic `SHA-256("nad-device:{sub}:{i}")`
  derivation — for `+123456789000` the one device is actually
  `ad59992d-c305-2bb1-d861-380b14eec986` / `81:F4:87:98:A7:60`, and for
  `+123456789002` the two are `20664178-…`/`C5:A5:64:05:03:37` and
  `939deb68-…`/`F7:DB:88:02:F6:9E`. The `createRebootRequest`/`getRebootRequest`
  examples' materialised `devices` entry carried the same placeholder though it
  is the subject's real derived device id. Bound all five examples to concrete
  subjects, corrected the ids/MACs to the handler's values (keeping the minted
  reboot `id` + timestamps explicitly illustrative, the application-profiles
  posture), and added `vendored_success_examples_match_the_handler` driving the
  real router for both subjects to pin the full list/by-id bodies and the
  reboot-all materialised `devices`. **Two stale module docs fixed** (the
  "later slices" trap class caught on device-data-volume / qos-booking /
  application-profiles / session-insights / sponsored-data): the parent
  `network_access_devices.rs` said "The read/patch/delete legs are later slices"
  and the `vwip.rs` header said "Three endpoints" (listing five) and "The `PATCH`
  (update) leg is a later slice" though all five legs ship and are tested —
  corrected both to describe the full lifecycle. This advances the vwip audit
  cursor to the next API (capabilities-and-restrictions). Spec + doc + test
  change (no behaviour/response change — the handlers were already correct; only
  the example ids/MACs were placeholders); no new dependency.
- 🔧 capabilities-and-restrictions — verified 2026-10-13: the one modelled
  operation (`postServiceCapability`, `POST
  /capabilities-and-restrictions/vwip/retrieve`) is implemented
  (`src/apis/capabilities_and_restrictions/vwip.rs`), routed, and well-tested
  (23 test fns before this pass), and the module docs agree with the code and
  the vendored `specs/capabilities-and-restrictions/vwip/openapi.yaml`: the
  reserved-error-suffix plane on the first query's first `phoneNumber`
  (`…404` = "No Capability API found"), the context-derived
  `camaraCapabilitiesBitmap` (queried phone number's trailing three digits, else
  an FNV-1a hash of `overlayExtends`, `% 2^CATALOGUE_SIZE`), the three-entry
  synthetic restriction catalogue, the `overlayExtends`/`resourceScopes` echo,
  and the validation plane (400 `INVALID_ARGUMENT`/`OUT_OF_RANGE`) all match.
  **Spec↔code drift found and fixed — the deterministic-value class** (as on
  network-access-devices / click-to-dial / verified-caller): the vendored `201`
  `info` success example, bound to the request example's `phoneNumber
  "+123456789012"`, claimed `camaraCapabilitiesBitmap: 5`, but the handler
  derives it from the trailing three digits — `012` = 12, `12 % 8 = 4` — so the
  value is `4` (the `5` was evidently copied from the `…005` unit-test
  scenario). Corrected the example to `4` (with a derivation comment) and added
  `vendored_success_example_matches_the_handler`, which drives the documented
  request through the real router and pins the bitmap, the echoed
  `overlayExtends`/`resourceScopes`, the full three-key `bitmapCapabilities`
  map, and key `"0"`'s exact restriction block. **Small tested increment:** the
  two inline `400` examples (the only error responses not `$ref`ing the shared
  catalog) were guarded by existing tests only for `code`; added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of the `invalidArgument` and `outOfRange`
  examples through the router. The restriction-catalogue blocks, the request
  example, and the shared-`$ref` 401/403/404/409/422/429/500/503 responses
  already matched. This advances the vwip audit cursor to the next API
  (dedicated-network-profiles). Spec + `#[cfg(test)]` change (no
  behaviour/response change — the handler was already correct; only the one
  success-example bitmap was wrong); no new dependency.
- 🔧 dedicated-network-profiles — verified 2026-10-13: both modelled operations
  (`readNetworkProfiles`, `GET /profiles`; `readNetworkProfile`,
  `GET /profiles/{profileId}`) are implemented
  (`src/apis/dedicated_network_profiles/vwip.rs`), routed, and well-tested
  (21 test fns before this pass), and the module docs agree with the code and
  the vendored `specs/dedicated-network-profiles/vwip/openapi.yaml`: the fixed
  four-template catalog (enterprise-campus/iot-massive/public-safety/
  media-production), the `profileId` three-layer control plane (malformed → 400
  `INVALID_ARGUMENT`; reserved trailing suffix → the canonical CAMARA error;
  else `d % 4` template with the requested id echoed), and the list control
  planes (exact-match `name` filter, `page`/`perPage` window, no identifier so
  no reserved plane) all match the handler. **Spec↔code drift found and fixed in
  the vendored `200` `firstPage` list example:** it showed only the *first* of
  the four catalog items while its own pagination claimed `totalCount 4` /
  `totalPages 1` / `perPage 10` — an incomplete page that can't come back from
  the router (`GET /profiles` with no params returns all four on page 1).
  Completed the `firstPage` example to the real four-item body. The by-id
  `iotMassive` example (`…001` → iot-massive) and both inline 400 error examples
  were already byte-accurate. Added `vendored_success_examples_match_the_handler`
  (drives the router for `GET /profiles` and pins the full four-item items array
  + pagination, and the `…001` by-id body) and
  `vendored_error_example_messages_match_the_handler` (pins the list `outOfRange`
  and by-id `invalidArgument` `{status,code,message}` bytes, which existing tests
  guarded only for `code`). This advances the vwip audit cursor to the next API
  (dedicated-network). Spec + `#[cfg(test)]` change (no behaviour/response change
  — the handlers were already correct; only the one list example was
  incomplete); no new dependency.
- 🔧 dedicated-network — verified 2026-10-13: all four modelled CRUD operations
  (`createNetwork`, `readNetwork`, `listNetworks`, `deleteNetwork`) are
  implemented (`src/apis/dedicated_network/vwip.rs` + an in-memory `store.rs`),
  routed, and well-tested, and the vendored
  `specs/dedicated-network/vwip/openapi.yaml` is consistent with the code (the
  request-validation + `serviceAreaId` `d % 3` lifecycle planes, the minted-id
  store legs, all success/error examples). **No spec↔code value drift** (minted
  `id`s are legitimately illustrative). Fixed the **stale "create-only / later
  pass" doc trap** in four places (parent module doc, `vwip.rs` body doc,
  `store.rs` `get` doc, and two spec strings) — all claimed read/list/delete were
  future work though they ship — keeping the genuine cut (no `sink` notification).
  Added `vendored_success_example_matches_the_handler` and
  `vendored_error_example_messages_match_the_handler` pinning the two previously
  unpinned examples. Doc + `#[cfg(test)]` change; no new dependency.
- 🔧 dedicated-network-accesses — verified 2026-10-13: all seven modelled
  operations over the full access lifecycle — `createAccess` (`POST /accesses`
  → 201 `AccessInfo`), the stateful `readAccess` (`GET /accesses/{accessId}`),
  `listAccesses` (`GET /accesses`, optional `networkId` filter, bare array),
  `deleteAccess` (`DELETE …/{accessId}` → 204, single-use), and the device
  sub-resources `listDevices` (`GET …/{accessId}/devices`, paginated
  `AccessDevicesPage` + `deviceStatus` filter), `addDevicesToAccess`
  (`POST …/devices/add`, bare `Device[]` body → 201 `AddDevicesSuccess`) and
  `removeDevicesFromAccess` (`POST …/devices/remove`, bare `Device[]` body →
  204) — are implemented (`src/apis/dedicated_network_accesses/vwip.rs` + an
  in-memory `store.rs`), routed, and well-tested (54 test fns before this pass),
  and the vendored `specs/dedicated-network-accesses/vwip/openapi.yaml` is
  consistent with the code: the three control planes (request validation → 400;
  the `networkId` reserved-suffix identifier plane; the per-device
  `GRANTED`/`DENIED` grant plane keyed on each device's first-present identifier)
  and all success examples (`createAccess` `mixed` …012 → 2/1/1 with the …404
  device DENIED; `listDevices` `onePage` → page 1/10, totalCount 2/totalPages 1;
  `addDevicesToAccess` `added` → NAI granted, …404 denied) match the handler.
  **No spec↔code value drift found:** there are no inline error examples (every
  400 `$ref`s the `AccessError` schema with no `examples`), and the minted
  `id`s are legitimately illustrative (`store::new_access_id` is counter/clock-
  derived, the application-profiles posture, not request-reproducible). **Stale
  "create-only / later passes" docs fixed in six places** (the same stale-slice-
  doc trap caught on device-data-volume / qos-booking / application-profiles /
  session-insights / sponsored-data / network-access-devices / dedicated-network):
  the parent `dedicated_network_accesses.rs` doc ("only the create leg is
  mounted … read/list/delete and the device sub-resources arrive in later
  passes"), the `vwip.rs` module header (the store-bridge sentence and the
  "this slice being create-only" documented-cuts paragraph), the
  `CreateAccessRequest.sink_credential` struct comment ("notifications are a
  later pass"), three `store.rs` doc lines ("sub-resource legs arrive in later
  passes", "later passes' read/list/delete", "the list/delete legs (later
  passes)"), and the spec's `SinkCredential` schema string ("this slice being
  create-only") — all corrected to describe the full lifecycle while keeping the
  genuine documented cuts (the CAMARA 207 multi-status / 422 NO_VALID_DEVICE /
  409 request-level device-conflict forms, `sink` notification delivery, and the
  `listAccesses` `x-device` header filter). **Small tested increment:** added
  `vendored_success_examples_match_the_handler`, which drives the three
  documented success examples through the real router and pins the derived parts
  (the `createAccess` `mixed` stats + verbatim `recentAccessDevices`, the
  `listDevices` `onePage` full page body, and the `addDevicesToAccess` `added`
  AccessDevices array) so they can't silently drift; the illustrative minted
  `id`s are deliberately not pinned. This advances the vwip audit cursor to the
  next API (dedicated-network-areas). Doc + spec-string + `#[cfg(test)]` change
  only (no behaviour/response change — the handlers were already correct; the
  drift was doc staleness, not examples); no new dependency.
- 🔧 dedicated-network-areas — verified 2026-10-13: both modelled operations
  (`readNetworkServiceArea`, `GET /areas/{areaId}`; `retrieveNetworkServiceAreas`,
  `POST /retrieve-service-areas`) are implemented
  (`src/apis/dedicated_network_areas/vwip.rs`), routed, and well-tested (36 test
  fns before this pass), and the module docs agree with the code and the vendored
  `specs/dedicated-network-areas/vwip/openapi.yaml`: the single-area three-layer
  `areaId` control plane (malformed → 400 `INVALID_ARGUMENT`; reserved trailing
  suffix → the canonical CAMARA error; else `digits % N` picks a fixed
  service-area template with the requested id echoed) and the collection query's
  AND-combined optional filters (`atLocation`/`overlappingArea`/`coveringArea`
  CIRCLE-only haversine spatial filters + `byName`/`byNetworkProfileId`/
  `byQosProfileName`, never 404s) all match the handler. **Spec↔code drift found
  and fixed in the vendored `readNetworkServiceArea` 200 `cityCentre` example —
  the deterministic-value class** (as on dedicated-network / network-access-devices
  / click-to-dial): the example was bound to `areaId …001` but showed
  `london-city`, yet the handler selects `TEMPLATES[digits % N]`, so tail `001`
  → template 1 (`manchester-metro`), not template 0 (`london-city`); the
  existing `happy_path` test already pins `…001` → manchester-metro, so the
  example id was the wrong part (its whole body is london-city). Rebound the
  example to `areaId …000` (which genuinely selects london-city) and added a
  derivation comment. Added `vendored_success_example_matches_the_handler` (drives
  `…000` through the router and pins the full london-city body, asserts `…001` is
  a different area so the id stays load-bearing, and pins the collection
  `catalog` example's `…000`/`…002` ids/bodies) and
  `vendored_error_example_messages_match_the_handler` (pins the exact
  `status`/`code`/`message` bytes of the three inline error examples —
  readNetworkServiceArea 400 `invalidArgument`, retrieve 400
  `invalidArgument`/`outOfRange` — which existing tests guarded only for `code`).
  This advances the vwip audit cursor to the next API (edge-application-management).
  Spec + `#[cfg(test)]` change (no behaviour/response change — the handler was
  already correct; only the one success-example id was wrong); no new dependency.
- 🔧 edge-application-management — verified 2026-10-13: all **fifteen** modelled
  operations across the full surface — the `edge-cloud-zones` + `clusters`
  read-only catalogs, the `apps` CRUD (`submitApp`/`getApp`/`getApps`/`deleteApp`),
  the `app-instances` CRUD (`createAppInstance`/`getAppInstance`/`getAppInstances`/
  `deleteAppInstance`), and the `deployments` lifecycle (`createAppDeployment`/
  `getAppDeployment`/`getAppDeployments`/`updateAppDeployment` (merge-patch)/
  `deleteAppDeployment`) — are implemented (`src/apis/edge_application_management/
  vwip.rs` + `store.rs`/`instance_store.rs`/`deployment_store.rs`), routed, and
  well-tested (~90 test fns before this pass). **Spec↔code drift found and fixed —
  the deterministic-value / placeholder-UUID class** (as on
  application-endpoint-discovery / network-access-devices / click-to-dial): the
  two reproducible catalog 200 examples carried Swagger placeholder ids — the
  getEdgeCloudZones `catalog` example claimed "the full zone catalog" but showed
  only 1 of 6 zones with `edgeCloudZoneId: 6ec0bd7f-…`, and the getClusters
  `catalog` example carried `clusterRef: 642f6105-…` (not even v5-shaped, so
  `cluster_ref` can never emit it) and the same placeholder zone id — though both
  ids are deterministic v5 UUIDs (`zone_id`/`cluster_ref`, SHA-256 over the
  name). Completed both to the real full catalogs (6 zones, 4 clusters) with the
  handler's derived ids (e.g. `camarasim-edge-eu-west-1` →
  `6c7adb07-5cdc-58a3-b049-918bcd741d7b`, `camarasim-cluster-eu-west-1a` →
  `48397420-b675-506a-a94f-cf6ea9f6c885`), and corrected the name-paired
  `EdgeCloudZone` schema example's id to its derived value too. Added
  `vendored_success_examples_match_the_handler`, which drives both no-filter GETs
  through the real router and pins every documented zone/cluster id both as the
  literal and against `zone_id`/`cluster_ref` so they can't drift again (the bare
  illustrative UUID schema examples — appInstanceId, appDeploymentId, Location
  headers, request-body ids — stay illustrative, the application-profiles
  posture). **Two stale "later passes" module docs fixed** (the stale-slice-doc
  trap caught on device-data-volume / qos-booking / dedicated-network-*): the
  parent `edge_application_management.rs` doc and the `vwip.rs` header both
  presented the app-instance read/list/delete legs and the deployment/cluster
  resources as future work and the header's endpoint list omitted `getClusters`,
  `getAppDeployments`, `deleteAppDeployment`, though all fifteen ship — corrected
  both to describe the complete surface. This advances the vwip audit cursor to
  the next API (esim-remote-management). Spec + doc + `#[cfg(test)]` change (no
  behaviour/response change — the handlers were already correct; only the two
  catalog examples and one schema example carried placeholders); no new
  dependency.
- 🔧 esim-remote-management — verified 2026-10-13: all four modelled operations
  over the full CMP surface — the two stateless read legs (`profileList`
  `POST /profile/downloaded-list`, `profileResultQuery`
  `POST /profile/result/query`) and the two command legs modelled as synchronous
  acknowledgements (`profileOperation` `POST /profile/oper`, `profileDownload`
  `POST /profile/download`) — are implemented
  (`src/apis/esim_remote_management/vwip.rs`), routed, and well-tested (66 test
  fns before this pass), and the module docs/spec agree with the code: the device
  `eId` / `taskId` trailing-three-decimal-digit control plane (reserved-suffix →
  canonical CAMARA error; `profileList` `d % 4` profile count with the first
  enabled; `profileResultQuery` `d % 3` `operResult`; the command legs accept and
  echo), the full envelope validation (`sequenceNum`/`eId`/`taskId` shapes,
  `optType` 1..=3 / `autoEnableType` ==1 → `OUT_OF_RANGE`, `protocol`/`sink`/
  `types`/`subscriptionMaxEvents` bounds), and the documented eUICC-engine/`sink`
  cuts all match. **Spec↔code drift found and fixed — the deterministic-value /
  placeholder class** (as on verified-caller / click-to-dial /
  network-access-devices / edge-application-management): every `200` success
  example carried the placeholder `imei "356938035643809"` and `iccid
  "8931089011234567890"`, and the two `profileResultQuery` examples additionally
  claimed a placeholder reported `eId` (`A1B2C3D4E5F6…0001`/`…0000`), but the
  handler derives all of these deterministically — `imei`/`iccid` from the device
  `eId` (FNV-1a + Luhn), and the query leg's reported `eId` from the `taskId` via
  `eid_from_task`. The real values are: for `eId …0001`, `imei
  992085662256008` + `iccid 89814418471879978689` (used by `profileList`
  `oneEnabledProfile`, `profileOperation` `enableAccepted`, and both
  `profileDownload` examples); for `eId …0000`, `imei 003080778538115`; for
  `taskId task-001`, `eId 9b056a5d5ef9f464030ffcfd8d0c89ca` + `imei
  054921625391109` + `iccid 89974964855544562789`; for `taskId task-000`, `eId
  9b056b5d5ef9f617030ffdfd8d0c8b7d` + `imei 791890645115681` + `iccid
  89642926825946493487`. Corrected all seven examples (binding each to its request
  with a derivation note) and added `vendored_success_examples_match_the_handler`,
  which drives all four endpoints through the real router and pins every documented
  body so the examples can't silently drift back to placeholders. **Two stale
  "first slice / not modelled yet" docs fixed** (the stale-slice-doc trap caught on
  device-data-volume / qos-booking / dedicated-network-*): the parent
  `esim_remote_management.rs` doc said "This first slice models only the stateless
  profile-inventory query … the three asynchronous … legs … are not in scope for
  this module yet," and the `profileList` `x-camarasim-scenarios` note said "The
  asynchronous lifecycle operations (download / operation / result query) are not
  modelled yet," though all four legs ship and are tested — corrected both to the
  complete surface. The reserved-suffix set and the request/response schemas
  already matched. This advances the vwip audit cursor to the next API
  (network-access-domains, the last unaudited vwip API). Spec + doc + `#[cfg(test)]`
  change (no behaviour/response change — the handlers were already correct; only
  the examples carried placeholders); no new dependency.
- 🔧 network-access-domains — verified 2026-10-13: all **thirteen** modelled
  operations across the full surface — the read-only `getTrustDomainCapabilities`
  (`GET /trust-domains/capabilities`); the subject-keyed `getServices`
  (`GET /services`) and `getService` (`GET /services/{serviceId}`); the stateful
  Trust Domain CRUD (`createTrustDomain`, `getTrustDomains`, `getTrustDomain`,
  `updateTrustDomain`, `deleteTrustDomain`); and the Trust Domain **Device**
  sub-resource CRUD (`createTrustDomainDevice`, `getTrustDomainDevices`,
  `getTrustDomainDevice`, `updateTrustDomainDevice`, `deleteTrustDomainDevice`) —
  are implemented (`src/apis/network_access_domains/vwip.rs` + an in-memory
  `store.rs` with disjoint Trust Domain and device maps), routed, and well-tested
  (~90 test fns before this pass), and the vendored
  `specs/network-access-domains/vwip/openapi.yaml` is consistent with the code (the
  subject reserved-suffix / request-validation / store-state / parent-existence
  control planes, the advertised-access-type gate, the write-only password/credential
  stripping, and the `(serviceId, name)` / `(trustDomainId, deviceName)` →
  deterministic-v5-id duplicate-409 derivation all match). **Spec↔code drift found
  and fixed — the deterministic-id / placeholder class, this spec's whole success-example
  set** (the same class caught on verified-caller / click-to-dial /
  network-access-devices / edge-application-management / esim-remote-management):
  *every* 200/201 success example carried Swagger placeholder ids and none was bound to
  a concrete input. The four Trust Domain examples (createTrustDomain `created`,
  getTrustDomains `one`, getTrustDomain/updateTrustDomain `trustDomain`) all showed
  `id: b1e0f6a2-…`, but `trust_domain_id("3fa85f64-…", "Primary WiFi Network")` is
  `950925b2-55e7-5330-8f22-93fef8f82784`; the four device examples showed
  `c2f1a7b3-…`/`d3a2b8c4-…`, but `trust_domain_device_id` of that Trust Domain yields
  `0cc7f47a-…` (Alice's Laptop) and `542bfe03-…` (Alice's Phone); and the getServices
  `twoServices` + getService `service` examples carried placeholder service/site ids
  (`3fa85f64-…`, `123e4567-…`) and real-city geo points (Bonn/Zurich) though all are
  SHA-256-derived from the token subject — for subject `nad-005` (trailing 005 → 2
  services) the real ids are `d161db25-…`/`8daaa955-…` (lat 67.55459 / long −110.31266)
  and `fe6a3242-…`/`eefceeba-…` (lat 70.88075 / long −115.6466). Bound each example to
  its concrete input, corrected all ids/geo to the handler's derived values (keeping
  the audit stamps `createdAt`/`modifiedAt` + subject-derived `createdBy`/`modifiedBy`
  explicitly illustrative, the network-access-devices posture), and added
  `vendored_success_examples_match_the_handler`, which drives createTrustDomain +
  getTrustDomain, createTrustDomainDevice, and getServices + getService through the real
  router (and pins `trust_domain_id`/`trust_domain_device_id` directly) so the examples
  can't drift back to placeholders. There are **no inline error examples** (every 4xx/5xx
  `$ref`s the shared catalog, as on dedicated-network-accesses), so none to pin. **Two
  stale "later slices" module docs fixed** (the stale-slice-doc trap caught on
  device-data-volume / qos-booking / dedicated-network-* / esim-remote-management): the
  parent `network_access_domains.rs` doc and the `vwip.rs` header both presented the
  Trust Domain read/update/delete legs and the whole device sub-resource as future work
  (the header listed only 5 of 13 operations) though all thirteen ship and are tested —
  corrected both to the complete surface. **This was the last unaudited vwip API: the
  per-API vwip verification sweep (backlog item 3) is now complete across all 🔧 APIs.**
  Spec + doc + `#[cfg(test)]` change (no behaviour/response change — the handlers were
  already correct; only the success examples carried placeholders); no new dependency.

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
>
> **Update (2026-10-14):** the item-2 per-API spec↔code audit has *not* been idle —
> it found genuine drift (truncated error messages, placeholder/deterministic-value
> examples, illustrative timestamps, stale slice docs) in nearly every ✅ API across
> Phases 1–4 and the entire vwip tier, and has now moved into the **Phase 5
> stable/numbered tier** (~18 APIs that were covered by neither the Phase 1–4 audit
> nor the vwip sweep; see item 2 below). Each is a real one-pass unit. This block's
> "idle pass → stop without committing" guidance applies only to a pass that finds
> no drift to fix, not to auditing a never-audited API.

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
   and documented in each endpoint's vendored spec, with contract tests. *Now the
   top open item — the vwip audit (item 3) completed 2026-10-13.*
   - ✅ number-verification v1 — verified 2026-10-14: both operations
     (`verifyPhoneNumber`, `phoneNumberShare`) implemented, tested, documented;
     spec ↔ code ↔ module-docs consistent. **No drift:** the 200 success examples
     (`true`/`false`/default line `+123456789012`) are reproducible scalars already
     pinned by existing router tests, and every 4xx/5xx `$ref`s the shared catalog
     (no inline examples to drift). Nothing to change.
   - ✅ sim-swap v2 — verified 2026-10-14: both operations (`checkSimSwap`,
     `retrieveSimSwapDate`) implemented and well-tested. **Spec↔code drift found and
     fixed — the wall-clock / illustrative-value class** (as on verified-caller's
     `expiresAt`): the `/retrieve-date` `recentSwap` 200 example presented
     `latestSimChange: "2024-01-01T14:27:08Z"` as a plain response value, but the
     handler returns *now* − hoursAgo (wall-clock dependent), so that frozen value
     (the module's RFC-3339 reference epoch) can never reproduce. Marked the example
     `latestSimChange` illustrative (its *presence* + `monitoredPeriod` are the
     contract) and added `vendored_success_examples_match_the_handler` pinning both
     `/check` booleans and both `/retrieve-date` shapes (incl. the `monitoredPeriod`
     literal 10 and the `null` case) through the router, asserting the illustrative
     timestamp is not a returned literal. Spec + `#[cfg(test)]` change, no behaviour
     change, no new dependency. *This shows item 2 is **not** a pure contract-test
     treadmill: the same deterministic-value drift the vwip sweep found also hides in
     stable-API examples.* Next in phase order: kyc-match v0.3.
   - ✅ kyc-match v0.3 — verified 2026-10-14: the one operation (`matchCustomer`,
     `POST /kyc-match/v0.3/match`) is implemented (`src/apis/kyc_match/v0_3.rs`),
     routed, and well-tested. The two control planes (top-level reserved-error
     suffix on the identifier; per-attribute value-marker verdict with a fixed
     90 partial score for the score-bearing attributes) match the handler, the
     request/response schemas cover the same 20 attributes and the same 13
     score-bearing fields, and the 200 (`allMatch`/`partialMatch`/`notAvailable`)
     and 400 (`invalidArgument`/`invalidParamCombination`) examples are byte-accurate.
     **Spec↔code inconsistency found and fixed — the documented-divergence /
     unenforced-constraint class** (as on device-authenticity's
     `IDENTIFIER_NOT_FOUND`/`QUOTA_EXCEEDED` schema enums): the `gender` request
     property declared `enum: [MALE, FEMALE, OTHER]`, implying the server rejects
     any other value, but the handler treats `gender` as a plain `Option<String>`
     and classifies it by the same uniform value-marker plane as every other
     attribute (so `"banana"`, `"nomatch"`, `"unavailable"` are all accepted and
     drive the verdict) — the enum is never enforced, which also silently
     contradicts CamaraSim's own "each attribute is classified by its value"
     functional-case plane. Kept the enum (faithful to canonical CAMARA 0.3.0) but
     documented the divergence on the property ("CamaraSim does NOT enforce it …
     documented divergence, not drift"), converting undocumented drift into an
     explicit divergence note. Added `gender_is_classified_by_value_not_enforced_against_the_enum`
     (pins MALE→true/no-score, out-of-enum `banana`→true accepted-not-400,
     `nomatch`→false/no-score boolean-only, `unavailable`→not_available) and
     `vendored_examples_match_the_handler` (pins the three 200 example bodies as
     exact objects + the two 400 `status`/`code`/`message` byte strings through the
     router, which prior tests guarded only by `code`). Spec + `#[cfg(test)]` change
     only — no behaviour/response change (the handler was already correct; the enum
     was an unenforced schema claim), no new dependency. Next in phase order:
     device-reachability-status v1 (Phase 2).
   - ✅ device-reachability-status v1 — verified 2026-10-14 (first Phase 2 API):
     the one operation (`getReachabilityStatus`,
     `POST /device-reachability-status/v1/retrieve`) is implemented
     (`src/apis/device_reachability_status/v1.rs`), routed, and well-tested. The two
     control planes (the identifier reserved-error-suffix plane; the
     trailing-three-digits reachability plane — `…000` → not reachable, odd → SMS
     only, else data+SMS) match the handler, the `phoneNumber` E.164 pattern is
     genuinely enforced (`is_valid_e164` == the spec's `^\+[1-9][0-9]{4,14}$`, so no
     unenforced-constraint divergence like kyc-match's gender enum), and the
     request/response schemas match the Rust types. **No spec↔code drift found** —
     the three 200 success examples (`reachableDataSms`/`reachableSmsOnly`/
     `notReachable`) are reproducible scalar shapes that match the handler, every
     4xx/5xx `$ref`s the shared catalog (no inline error examples to drift), and the
     documented reserved-suffix set (400/401/403/404/409/422/429/500/503, 409/500
     noted as CamaraSim extensions) equals `scenarios::RESERVED_ERROR_SUFFIXES`
     exactly. **Small tested increment:** the existing router tests asserted the 200
     examples only field-by-field (`reachable`/`connectivity` separately), so a
     stray/renamed field could slip past; added
     `vendored_success_examples_match_the_handler` pinning all three documented
     example bodies as **exact objects** through the real router (the same
     whole-object pin kyc-match added). `#[cfg(test)]`-only change — no
     behaviour/spec change (the spec was already correct), no new dependency. Next in
     phase order: device-roaming-status v1 (Phase 2).
   - ✅ device-roaming-status v1 — verified 2026-10-14 (second Phase 2 API): the
     one operation (`getRoamingStatus`,
     `POST /device-roaming-status/v1/retrieve`) is implemented
     (`src/apis/device_roaming_status/v1.rs`), routed, and well-tested. The two
     control planes (the identifier reserved-error-suffix plane; the
     trailing-three-digits roaming plane — `…000`/no-digits → not roaming, else
     roaming with the visited country `COUNTRIES[digits % 6]` over the fixed table
     `[(262,DE),(234,GB),(208,FR),(310,US),(440,JP),(505,AU)]` so the country is a
     second control plane) match the handler, the `phoneNumber` E.164 pattern is
     genuinely enforced (`is_valid_e164` == the spec's `^\+[1-9][0-9]{4,14}$`, so no
     unenforced-constraint divergence like kyc-match's gender enum), the
     reserved-suffix set (400/401/403/404/409/422/429/500/503, 409/500 noted as
     CamaraSim extensions) equals `scenarios::RESERVED_ERROR_SUFFIXES`, and the
     optional `lastStatusTime` is correctly documented as omitted. **No spec↔code
     drift found** — the two 200 examples (`roaming` → `262`/`["DE"]`, `home` →
     `{roaming:false}`) are reproducible scalar shapes matching the handler, and
     every 4xx/5xx `$ref`s the shared catalog (no inline error examples to drift),
     the device-reachability-status / number-verification posture. **Small tested
     increment:** the existing router tests asserted the 200 examples only
     field-by-field (`roaming`/`countryCode`/`countryName` separately), so a
     stray/renamed/extra field (e.g. an accidental `lastStatusTime`) could slip
     past; added `vendored_success_examples_match_the_handler` pinning both
     documented example bodies as **exact objects** through the real router (the
     whole-object pin device-reachability-status / kyc-match added).
     `#[cfg(test)]`-only change — no behaviour/spec change (the spec was already
     correct), no new dependency. Next in phase order: device-identifier v0.3
     (Phase 2).
   - ✅ device-identifier v0.3 — verified 2026-10-14 (third and last Phase 2 API):
     all three operations (`retrieveType`, `retrieveIdentifier`, `retrievePPID`,
     `POST /device-identifier/v0.3/retrieve-{type,identifier,ppid}`) are
     implemented (`src/apis/device_identifier/v0_3.rs`), routed, and well-tested.
     The two shared control planes (the identifier reserved-error-suffix plane; the
     trailing-three-digits `DEVICE_TYPES[digits % 6]` device-type plane with the
     IMEI/IMEISV synthesised from the selected TAC + zero-padded serial + Luhn
     check digit, and the PPID = SHA-256(identifier) UUID-shaped) and the
     two-legged/three-legged identifier resolution all match the handler; every
     4xx/5xx `$ref`s the shared catalog (no inline error examples to drift).
     **Spec↔code drift found and fixed — the deterministic-value / placeholder
     class** (as on verified-caller / click-to-dial / network-access-domains): the
     `retrieve-ppid` 200 `ppid` example was bound to `phoneNumber "+123456789001"`
     but carried a placeholder `ppid: "b1d5781d-1141-df9d-6d76-2f8e70edb19c"`,
     whereas `ppid_from` returns SHA-256("+123456789001") rendered UUID-shaped =
     `5dacd245-aaf6-fa2e-ce77-e48eaabc4064`; corrected it and bound the example to
     its derivation. Also marked all three examples' frozen `lastChecked`
     **illustrative** (the handler stamps the current time, the sim-swap
     `latestSimChange` convention) with a derivation note on each. Added
     `vendored_success_examples_match_the_handler` driving all three documented
     requests through the real router and pinning each body as an exact object
     (minus the illustrative `lastChecked`, asserted present/UTC but ≠ the frozen
     literal), with the ppid pinned both as the literal and against `ppid_from`.
     The `type`/`identifier` example values (TAC 35847104 / OnePlus 12, imei
     `358471040000011`, imeisv `3584710400000100`) were already byte-accurate.
     Spec + `#[cfg(test)]` change only — no behaviour/response change (the handler
     was already correct; only the one example value was a placeholder), no new
     dependency. **This completes the Phase 2 stable tier;** next in phase order is
     Phase 3 (one-time-password-sms v1 / quality-on-demand v1).
   - ✅ one-time-password-sms v1 — verified 2026-10-14 (first Phase 3 API,
     CamaraSim's first **stateful** API): both operations (`sendCode`
     `POST /one-time-password-sms/v1/send-code`, `validateCode`
     `POST /one-time-password-sms/v1/validate-code`) are implemented
     (`src/apis/one_time_password_sms/v1.rs` + an in-memory `store.rs`), routed,
     and well-tested. The two control planes match the handler/spec: `send-code`
     is keyed on the `phoneNumber` (trailing-three-digit reserved-error-suffix
     plane → canonical CAMARA error; else issue an OTP whose code is the
     zero-padded last six digits and return the `authenticationId`), with the
     E.164 pattern and the `message` maxLength-160 + `{{code}}` placeholder
     genuinely enforced; `validate-code` is keyed on the live store state
     (matching code → 204 single-use consume; wrong code → `INVALID_OTP` until
     the 3-attempt budget exhausts → `VERIFICATION_FAILED`; unknown/consumed/
     expired id → `VERIFICATION_EXPIRED`, all HTTP 400). **No spec↔code drift
     found** — all four inline `validate-code` 400 example messages
     (`invalidArgument`/`invalidOtp`/`verificationFailed`/`verificationExpired`)
     already byte-match the handler, and `send-code`'s 200 `authenticationId`
     example is a counter/clock-derived opaque token, so it stays legitimately
     illustrative (the application-profiles posture) — not the deterministic-value
     drift class. **Small tested increment:** the existing router tests guarded
     those four 400 examples only by `code`, so a reworded `message` could
     silently drift from the spec; added
     `vendored_error_example_messages_match_the_handler` pinning the exact
     `status`/`code`/`message` bytes of all four through the real router (the
     device-authenticity / network-health-assessment pinning). `#[cfg(test)]`-only
     change — no behaviour/spec change (the spec was already correct), no new
     dependency. Next in phase order: quality-on-demand v1 (Phase 3).
   - ✅ quality-on-demand v1 — verified 2026-10-14 (second Phase 3 API, the
     session-lifecycle + CloudEvents-notifications API): all **five** modelled
     operations — `createSession` (`POST /sessions`), `getSession`
     (`GET /sessions/{id}`), `deleteSession` (`DELETE /sessions/{id}`),
     `extendQosSession` (`POST /sessions/{id}/extend`), and
     `retrieveSessionsByDevice` (`POST /retrieve-sessions`) — are implemented
     (`src/apis/quality_on_demand/v1.rs` + an in-memory `store.rs` +
     `notifications.rs` firing the `qos-status-changed` CloudEvents), routed, and
     well-tested, and the vendored `specs/quality-on-demand/v1/openapi.yaml` is
     consistent with the code: the three createSession control planes (identifier
     reserved-error-suffix → canonical CAMARA error incl. `…409` CONFLICT; the
     trailing-three-digits grant plane `…000`/no-digits → REQUESTED else AVAILABLE
     with `…001` → early NETWORK_TERMINATED; `duration` `<1`→INVALID_ARGUMENT,
     `>86400`→DURATION_OUT_OF_RANGE; a `qosProfile` name containing "unavailable" →
     QOS_PROFILE_NOT_APPLICABLE), and the extend/delete/retrieve state planes, all
     match the handler, and **all inline error-example messages across all five
     operations byte-match** (create 400 `invalidArgument`/`durationOutOfRange`,
     422 `missingIdentifier`/`qosProfileNotApplicable`; extend 400
     `invalidArgument`/`durationOutOfRange`). **Spec↔code consistency defect found
     and fixed — the illustrative-timestamp class** (as on sim-swap's
     `latestSimChange` / device-identifier's `lastChecked`): the createSession
     `available` and retrieve-sessions `some` success examples presented
     `startedAt`/`expiresAt` as frozen literals (`2024-01-01T14:27:08Z` /
     `…15:27:08Z`), but `build_session_info` stamps `now` / `now + duration`
     (wall-clock dependent) — those literals can never reproduce; the `sessionId`
     is likewise minted (counter+clock), the application-profiles posture. Marked
     all three example families' `sessionId`/`startedAt`/`expiresAt` **illustrative**
     with a derivation note (keeping the frozen values to show the shape), the
     reproducible echoed fields staying contract. Added
     `vendored_success_examples_match_the_handler` (drives createSession
     available/requested + retrieve-sessions some/none through the router, pins the
     reproducible fields as exact objects minus the illustrative ones, asserts
     `expiresAt == startedAt + duration` and that the illustrative values are NOT
     the frozen literals) and `vendored_error_example_messages_match_the_handler`
     (pins the exact `status`/`code`/`message` bytes of all six router-reachable
     inline error families; the 422 `missingIdentifier` example is documented as
     not router-reachable — its branch needs an empty-`sub` token the token
     endpoint never issues, the session-insights precedent). **Stale "First slice:
     POST /sessions + GET /sessions/{id}" parent module doc fixed** (the
     stale-slice-doc trap caught on device-data-volume / session-insights /
     dedicated-network-*), plus two stale `store.rs` doc lines ("in later passes —
     DELETE / extend"; "transitions arrive with DELETE / notifications") though all
     five legs + notifications ship. Spec + doc + `#[cfg(test)]` change only — no
     behaviour/response change (the handlers were already correct; the drift was
     two illustrative-timestamp examples + doc staleness), no new dependency. **This
     completes the Phase 3 stable tier;** next in phase order is Phase 4 (spatial):
     location-verification v3.
   - ✅ location-verification v3 — verified 2026-10-14 (first Phase 4 API,
     CamaraSim's first **spatial** API): the one operation (`verifyLocation`,
     `POST /location-verification/v3/verify`) is implemented
     (`src/apis/location_verification/v3.rs`), routed, and well-tested. The two
     control planes match the handler/spec: the identifier (first present device
     id, else token `sub`) trailing-three-digits verdict plane
     (reserved-error-suffix → canonical CAMARA error; `…000` → `FALSE`; odd →
     `PARTIAL` with `matchRate = (digits % 99) + 1`; else `TRUE`), and the circle
     `radius` area plane (`< 2000 m` regulatory minimum → 422
     `LOCATION_VERIFICATION.INVALID_AREA`; `< 1 m` or center lat/long out of range
     → 400 `OUT_OF_RANGE`; `areaType ≠ CIRCLE` → 400 `INVALID_ARGUMENT`), with the
     `phoneNumber` E.164 pattern and `maxAge` range genuinely enforced. All four
     inline error-example messages (400 `invalidArgument`/`outOfRange`, 422
     `invalidArea`/`missingIdentifier`) already byte-match the handler.
     **Spec↔code consistency defect found and fixed — the illustrative-timestamp
     class** (as on sim-swap `latestSimChange` / device-identifier `lastChecked` /
     quality-on-demand `startedAt`): all three 200 success examples
     (`inside`/`partly`/`outside`) presented `lastLocationTime` as a frozen literal
     (`2026-08-03T14:27:08Z`), but the handler stamps `rfc3339_utc(now_unix_secs())`
     (wall-clock dependent), so that literal can never reproduce. Marked
     `lastLocationTime` **illustrative** on all three examples with a derivation
     note (its presence as an RFC-3339 UTC instant is the contract; the
     `verificationResult`/`matchRate`/echoed `device` are reproducible), and
     clarified the `partly`/`outside` summaries as subject-driven (no `device`
     echoed). Added `vendored_success_examples_match_the_handler` (drives
     `inside` via a device …012, `partly` via a subject …011, `outside` via a
     subject …000 through the router, pins the reproducible fields as exact
     objects minus the illustrative `lastLocationTime`, asserting it is UTC but ≠
     the frozen literal) and `vendored_error_example_messages_match_the_handler`
     (pins the exact `status`/`code`/`message` bytes of the three router-reachable
     inline error families; the 422 `missingIdentifier` example is documented not
     router-reachable — its branch needs an empty-`sub` token the endpoint never
     issues, the quality-on-demand / session-insights precedent). Spec +
     `#[cfg(test)]` change only — no behaviour/response change (the handler was
     already correct; only the three illustrative timestamps lacked their caveat),
     no new dependency. Next in phase order: location-retrieval v0.4 (Phase 4).
   - ✅ location-retrieval v0.4 — verified 2026-10-14 (second Phase 4 API): the one
     operation (`retrieveLocation`, `POST /location-retrieval/v0.4/retrieve`) is
     implemented (`src/apis/location_retrieval/v0_4.rs`), routed, and well-tested.
     The two control planes match the handler/spec: the identifier (first present
     device id, else token `sub`) trailing-three-digits location plane
     (reserved-error-suffix → canonical CAMARA error; else a CIRCLE `area` whose
     `center` = base point `(51.5, -0.12)` offset by `digits × 0.001°` and whose
     `radius` = `((digits % 10) + 1) × 100` m, 100–1000 m), and the `maxAge` range
     plane (`< 60` or `> 2147483647` → 400 `OUT_OF_RANGE`; always-fresh so a valid
     `maxAge` never changes the result), with the `phoneNumber` E.164 pattern
     genuinely enforced and `networkAccessIdentifier` correctly rejected. Both 400
     inline error-example messages (`invalidArgument`/`outOfRange`) already byte-match
     the handler, and the happy-path `center`/`radius` for `+123456789012` are
     **byte-accurate** (`51.5 + 12×0.001 == 51.512`, `-0.12 + 12×0.001 == -0.108`,
     radius `300` — the f64 arithmetic is exact), so no deterministic-value drift.
     **Spec↔code consistency defect found and fixed — the illustrative-timestamp
     class** (as on location-verification `lastLocationTime` / sim-swap
     `latestSimChange` / device-identifier `lastChecked` / quality-on-demand
     `startedAt`): the one 200 success example (`located`) presented
     `lastLocationTime` as a frozen literal (`2026-08-03T14:27:08Z`), but the handler
     stamps `rfc3339_utc(now_unix_secs())` (wall-clock dependent), so that literal can
     never reproduce. Marked `lastLocationTime` **illustrative** with a derivation
     note (its presence as an RFC-3339 UTC instant is the contract; the
     `center`/`radius` are reproducible) and bound the example to its concrete input
     `+123456789012` with the centre/radius derivation spelled out. Added
     `vendored_success_example_matches_the_handler` (drives `+123456789012` through
     the router, pins the `area` body as an exact object minus the illustrative
     `lastLocationTime`, asserting it is UTC but ≠ the frozen literal) and
     `vendored_error_example_messages_match_the_handler` (pins the exact
     `status`/`code`/`message` bytes of the two router-reachable inline 400 families
     — `invalidArgument`/`outOfRange`; the 422 `missingIdentifier` example is
     documented not router-reachable — its branch needs an empty-`sub` token the
     endpoint never issues, the location-verification / quality-on-demand precedent).
     Spec + `#[cfg(test)]` change only — no behaviour/response change (the handler
     was already correct; only the one illustrative timestamp lacked its caveat), no
     new dependency. Next in phase order: geofencing-subscriptions v0.4 (Phase 4,
     last Phase 4 API).
   - ✅ geofencing-subscriptions v0.4 — verified 2026-10-14 (third and last Phase 4
     API; CamaraSim's first **event-subscription** API): all **four** modelled CRUD
     operations (`createSubscription` `POST /subscriptions`, `retrieveSubscriptionList`
     `GET /subscriptions`, `retrieveSubscription` `GET …/{subscriptionId}`,
     `deleteSubscription` `DELETE …/{subscriptionId}`) are implemented
     (`src/apis/geofencing_subscriptions/v0_4.rs` + an in-memory `store.rs` +
     `notifications.rs` firing initial-event / movement / expiry / max-events
     CloudEvents), routed, and well-tested (72 test fns before this pass). The two
     create control planes (the identifier reserved-error-suffix plane; the
     trailing-three-digits status plane — `…000`/no-digits → `ACTIVATION_REQUESTED`
     else `ACTIVE`, plus the `…001`/`…002` movement and even/odd initial-event
     position markers) and the circle `radius` area plane (`< 2000`/`> 200000` m or
     center out of range → 400 `OUT_OF_RANGE`; non-CIRCLE / missing area → 400
     `INVALID_ARGUMENT`) match the handler, the `phoneNumber` E.164 pattern is
     genuinely enforced, and both inline 400 error-example messages
     (`invalidArgument` "Only `CIRCLE` …", `outOfRange` "…between 2000 and 200000
     metres.") and the 422 `missingIdentifier` example byte-match the handler.
     **Spec↔code consistency defect found and fixed — the illustrative-timestamp +
     minted-id class** (as on quality-on-demand `sessionId`/`startedAt` /
     location-verification `lastLocationTime` / application-profiles minted ids): the
     `active` 201 success example presented `startsAt` as a frozen literal
     (`2026-08-03T14:27:08Z`), but the handler stamps `rfc3339_utc(now_unix_secs())`
     (wall-clock dependent), and `id` is minted (counter+clock, not request-derived),
     so neither can reproduce. Marked `id`/`startsAt` **illustrative** with a
     derivation note (their presence — a UUID `id`, an RFC-3339 UTC `startsAt` — is
     the contract; the echoed `protocol`/`sink`/`types`/`config` and `status` are
     reproducible). Added `vendored_success_example_matches_the_handler` (drives the
     example's exact request through the router, pins every reproducible field as an
     exact object minus the illustrative `id`/`startsAt`, asserting `startsAt` is UTC
     but ≠ the frozen literal) and `vendored_error_example_messages_match_the_handler`
     (pins the two router-reachable inline 400 families' exact `status`/`code`/`message`
     bytes; the 422 `missingIdentifier` example is documented not router-reachable —
     its branch needs an empty-`sub` token the endpoint never issues, the
     quality-on-demand / location-verification precedent). **Stale "First slice: POST
     /subscriptions + GET …/{subscriptionId}" parent module doc fixed**
     (`src/apis/geofencing_subscriptions.rs`) though all four CRUD legs + the
     notification machinery ship — the stale-slice-doc trap caught on
     device-data-volume / session-insights / quality-on-demand / dedicated-network-*.
     Spec + doc + `#[cfg(test)]` change only — no behaviour/response change (the
     handler was already correct; only the one illustrative example lacked its
     caveat), no new dependency. **This completes the Phase 4 spatial tier, and with
     it backlog item 2's stable-API audit across every ✅ API in Phases 1–4.** The
     ~18 Phase 5 stable/numbered APIs (inventory above) were covered by neither this
     Phase 1–4 audit nor the vwip sweep, so auditing them one at a time (stateless
     before stateful, per phase order) is the now-active continuation of item 2:
   - ✅ call-forwarding-signal v0.4 — verified 2026-10-14 (first **Phase 5**
     stable/numbered API audited). Both operations
     (`retrieveUnconditionalCallForwarding` `POST /unconditional-call-forwardings`,
     `retrieveCallForwarding` `POST /call-forwardings`) are implemented
     (`src/apis/call_forwarding_signal/v0_4.rs`), routed, and well-tested. The two
     control planes match the handler/spec: the identifier reserved-error-suffix
     plane and, once resolved, the trailing-three-digits result — unconditional
     endpoint: **odd** tail → `active:true`, even (incl. `…000`) → `active:false`;
     `/call-forwardings`: `digits % 16` 4-bit mask over `[unconditional,
     conditional_busy, conditional_not_reachable, conditional_no_answer]`, zero mask
     → `["inactive"]` — plus the two-legged/three-legged identifier resolution, all
     matching. The 200 examples are byte-accurate reproducible shapes (incl. `…012` →
     mask `0b1100` → `["conditional_not_reachable","conditional_no_answer"]`, already
     pinned), and every 400/401/403/404/409/429/500/503 response `$ref`s the shared
     catalog (no inline examples to drift). **Spec↔code drift found and fixed — the
     truncated-identifier-message class** (as on subscription-status /
     media-streaming-rate / consent-info / most-frequent-location): the 422
     `missingIdentifier` example in **both** operations carried the short
     `message: "The phone number cannot be identified."`, but `resolve_identifier`
     emits the full "…: supply `phoneNumber` or use a token that identifies a line."
     Corrected both and added `vendored_error_example_messages_match_the_handler`
     pinning the exact `status`/`code`/`message` bytes of all three router-reachable
     inline 422 families — `missingIdentifier` (reachable here, unlike several vwip
     APIs: a `client_credentials` token's `sub` is the non-line client id),
     `unnecessaryIdentifier`, `serviceNotApplicable` — through the real router (the
     existing tests guarded only `code`). Spec + `#[cfg(test)]` change only — no
     behaviour/response change (the handler was already correct; only the two
     examples were truncated), no new dependency. Next in phase order:
     number-recycling v0.2 (the next stateless Phase 5 stable API; the stateful
     Phase 5 stable APIs — carrier-billing v0.5, blockchain-public-address v0.3 —
     follow the stateless ones).
   - ✅ number-recycling v0.2 — verified 2026-10-14 (second Phase 5 stable/numbered
     API). The one operation (`checkNumberRecycling`,
     `POST /number-recycling/v0.2/check`) is implemented
     (`src/apis/number_recycling/v0_2.rs`), routed, and well-tested. The two control
     planes match the handler/spec: the identifier reserved-error-suffix plane and,
     once resolved, the recycling verdict (the identifier's trailing three digits are
     days-since-the-last-subscriber-change; recycled iff that change is strictly after
     `specifiedDate`, so `specifiedDate` is a genuine second control plane), plus the
     `specifiedDate` validation (malformed/impossible → 400 `INVALID_ARGUMENT`; future
     → 400 `OUT_OF_RANGE`), the genuinely-enforced E.164 pattern, and the
     two-legged/three-legged identifier resolution. The 200 verdict shape and the 400
     `invalidArgument`/`outOfRange` and 422 `unnecessaryIdentifier`/`serviceNotApplicable`
     inline examples already byte-match the handler, and every 401/403/404/409/429/500/503
     response `$ref`s the shared catalog. **Spec↔code drift found and fixed — the
     truncated-identifier-message class** (as on call-forwarding-signal /
     subscription-status / media-streaming-rate / consent-info / most-frequent-location):
     the 422 `missingIdentifier` example carried the short `message: "The phone number
     cannot be identified."`, but `resolve_identifier` emits the full "…: supply
     `phoneNumber` or use a token that identifies a line." Corrected the example and added
     `vendored_error_example_messages_match_the_handler` pinning the exact
     `status`/`code`/`message` bytes of all five router-reachable inline error families —
     400 `invalidArgument`/`outOfRange`, 422 `missingIdentifier` (reachable here, as on
     call-forwarding-signal: a `client_credentials` token's `sub` is a non-line client
     id), `unnecessaryIdentifier`, `serviceNotApplicable` — through the real router (the
     existing tests guarded only `code`). Spec + `#[cfg(test)]` change only — no
     behaviour/response change (the handler was already correct; only the one example was
     truncated), no new dependency. Next in phase order: kyc-age-verification v0.1 (the
     next stateless Phase 5 stable API; the stateful ones — carrier-billing v0.5,
     blockchain-public-address v0.3 — follow).
   - ✅ kyc-age-verification v0.1 — verified 2026-10-14 (third Phase 5 stable/numbered
     API). The one operation (`verifyAge`, `POST /kyc-age-verification/v0.1/verify`) is
     implemented (`src/apis/kyc_age_verification/v0_1.rs`), routed, and well-tested. The
     two control planes match the handler/spec: the identifier reserved-error-suffix
     plane and, once resolved, the age verdict — the identifier's trailing three digits
     encode the held age as `d % 100`, `ageCheck = "true"` iff that age ≥ the requested
     `ageThreshold` (so `ageThreshold` is a genuine second control plane), `…000` →
     `"not_available"` — plus the optional-field planes (`identityMatchScore` fixed 90
     when any identity attribute is supplied, `verifiedStatus: true` when an `idDocument`
     is, the opt-in `contentLock`/`parentalControl` minor signals), the genuinely-enforced
     E.164 pattern + `ageThreshold` 0..=120 range, and the two-legged/three-legged
     identifier resolution. The four 200 success examples are reproducible scalar shapes
     already pinned by existing router tests (`withIdentityAndLocks` is internally
     consistent — a minor at `ageCheck:false` with both locks `"true"`), the 400
     `invalidArgument`/`outOfRange` and 422 `unnecessaryIdentifier`/`serviceNotApplicable`
     inline examples already byte-match the handler, and every 401/403/404/409/429/500/503
     response `$ref`s the shared catalog. **Spec↔code drift found and fixed — the
     truncated-identifier-message class** (as on number-recycling / call-forwarding-signal /
     subscription-status / media-streaming-rate / consent-info / most-frequent-location):
     the 422 `missingIdentifier` example carried the short `message: "The phone number
     cannot be identified."`, but `resolve_identifier` emits the full "…: supply
     `phoneNumber` or use a token that identifies a line." Corrected the example and added
     `vendored_error_example_messages_match_the_handler` pinning the exact
     `status`/`code`/`message` bytes of all five router-reachable inline error families —
     400 `invalidArgument`/`outOfRange`, 422 `missingIdentifier` (reachable here, as on
     call-forwarding-signal / number-recycling: a `client_credentials` token's `sub` is a
     non-line client id), `unnecessaryIdentifier`, `serviceNotApplicable` — through the
     real router (the existing tests guarded only `code`). Spec + `#[cfg(test)]` change
     only — no behaviour/response change (the handler was already correct; only the one
     example was truncated), no new dependency. Next in phase order: device-swap v1 (the
     next stateless Phase 5 stable API; the stateful ones — carrier-billing v0.5,
     blockchain-public-address v0.3 — follow).
3. **Deepen `vwip` APIs** (🔧) one endpoint/scenario at a time, in mount order,
   only after the earlier phases are verified. **The per-API vwip verification sweep
   is complete (2026-10-13); further deepening needs the deferred stateful /
   provisioning-worker layer (human-gated, see the decision point above).**

> The above is a reconstructed starting point. As each API is verified, replace
> its bullet with concrete, checked-off sub-steps.

## Scan journal

- 2026-10-19 — **vwip residual-drift re-audit — iot-sim-fraud-prevention 429 example,
  drift found & fixed.** Re-checked the fifteenth vwip API (audited 2026-10-13; the
  `store.rs`-backed bind/query/unbind across the `IMEIBIND`/`AREALIMIT` facets) for the
  inline-error-message drift class. Its `vendored_examples_match_the_handler` test already
  pins every API-own 400/422 message, but the shared reserved-suffix 4xx families were
  left unpinned, and the `Generic429` example still carried `message: "Rejected due to
  request rate limit overpassed."` though the reserved-error-suffix plane routes `…429`
  through `scenarios::reserved_error` → `CamaraError::for_status(429)` = `"Rate limit
  reached."` (`src/errors.rs:124`) — so a consumer reading the spec saw a 429 message the
  endpoint never returns. Corrected the example to `"Rate limit reached."` and extended
  `vendored_examples_match_the_handler` to drive the reserved-suffix `…404`/`…429` paths
  through the real router and pin their exact `status`/`code`/`message` bytes (the
  `Generic404` example `"The specified resource is not found."` already byte-matched). The
  401/403 examples intentionally show the shared-catalog canonical (the auth layer emits a
  scope-specific 403), left unpinned as on consent-info. Spec + `#[cfg(test)]` change only
  — no behaviour/response change (the handler was already correct; the example message was
  stale), no new dependency. Green: `cargo test` → **3129 passed, 0 failed** (the one
  pinning test gained three reserved-suffix assertions, no net test-count change);
  `cargo build --release` ok; binary = **5,467,288 bytes** (unchanged — the shorter spec
  string is below section-padding granularity, `#[cfg(test)]`-only Rust). Branch
  `agent/iot-sim-fraud-prevention/fix-vwip-429`.
- 2026-10-18 — **vwip residual-drift re-audit — device-data-volume illustrative-timestamp
  drift found & fixed (new signature class in the vwip tier).** The 2026-10-18 truncated-message
  re-audit closed one drift class in the vwip specs but noted the vwip sweep had closed
  **2026-10-13**, one day before *both* late-identified classes (truncated-identifier-message
  **and** illustrative-timestamp) were first found in the stable tier on **2026-10-14** — yet only
  the message class was re-checked. A targeted grep for the frozen reference-epoch literal
  `2024-01-01T14:27:08Z` across `specs/**` surfaced it in **device-data-volume/vwip** success
  examples: both operations' 200 bodies (`/retrieve` `lightUsage`/`heavyUsage`; `/check`
  `exceeds`/`belowThreshold`) carried `lastStatusTime: "2024-01-01T14:27:08Z"` as a plain value
  with **no illustrative annotation**, but the handler stamps `rfc3339_utc(now_unix_secs())`
  (`vwip.rs:184`,`:289` — the current wall-clock instant), so that frozen literal can never
  reproduce (the sim-swap `latestSimChange` / quality-on-demand `startedAt` class). Annotated all
  four example summaries and both `RetrieveDataVolumeResponse`/`CheckDataVolumeResponse`
  `lastStatusTime` property descriptions to mark the value illustrative, and added
  `vendored_success_examples_match_the_handler` driving all four documented example requests
  through the real router and pinning each body as an **exact object** minus `lastStatusTime`
  (which is asserted present + UTC but **≠** the frozen literal). The deterministic fields
  (`dataVolumeCategory`/`thresholdExceeded` + the phone-keyed `device` echo) were already
  byte-accurate. Green: `cargo test` → **3129 passed, 0 failed** (+1); `cargo build --release` ok;
  binary = **5,467,288 bytes** (+4,096 vs 5,463,192 = one page of embedded/vendored spec-text
  growth from the longer summaries/descriptions; the test is `#[cfg(test)]`-only, no code-path
  change, no new dependency). Spec + `#[cfg(test)]` + this doc change only — no behaviour/response
  change (the handler was already correct; the examples' timestamps were unannotated frozen
  literals). **Next pass:** the analogous illustrative-timestamp sweep should confirm the remaining
  vwip success-example timestamps are annotated (click-to-dial/vwip's `/retrieve` `created` example
  already carries the annotation; the other vwip timestamp hits are schema-level `example:` fields,
  not success-body examples). Branch `agent/device-data-volume/audit-vwip-illustrative-timestamp`.
- 2026-10-18 — **vwip residual-drift re-audit — device-visit-location, drift found & fixed
  (closes the three-spec sweep).** Last of the three vwip specs the 2026-10-18
  device-data-volume pass flagged as carrying the truncated-identifier-message drift (the vwip
  sweep closed 2026-10-13, one day before that drift class was first identified in the stable
  tier on 2026-10-14). Took **device-visit-location** (the one operation,
  `retrieveDeviceVisitLocation` `POST /device-visit-location/vwip/retrieve`): its 422
  `missingIdentifier` example carried the short "The device cannot be identified." though
  `resolve_identifier` (`vwip.rs:293`) emits the full "The device cannot be identified: supply a
  `device` or use a token that identifies a device." (confirmed MISSING_IDENTIFIER router-reachable —
  a two-legged `client_credentials` token's `sub` is a non-line client id; existing
  `no_device_and_non_line_subject_is_missing_identifier` proves the branch). Corrected it. Audited
  every other inline error example against the handler and found **no further drift** — the five
  other API-own messages (400 INVALID_ARGUMENT "`startTime` and `endTime` must be valid RFC 3339
  date-times." / DEVICE_VISIT_LOCATION.INVALID_END_DATE; 404 DEVICE_VISIT_LOCATION.DATA_NOT_FOUND;
  422 UNNECESSARY_IDENTIFIER) already byte-matched. Added
  `vendored_error_example_messages_match_the_handler` pinning the exact `status`/`code`/`message`
  bytes of all five router-reachable API-own inline error families across the operation (prior
  per-case tests guarded only `code`/`status`); the reserved-suffix 404 `NOT_FOUND` example is
  deliberately left unpinned — its message is the shared-catalog `for_status(404)` canonical, not an
  API-own string. Green: `cargo test` → **3128 passed, 0 failed** (+1); `cargo build --release` ok;
  binary = **5,463,192 bytes** (unchanged — embedded-spec text edit below section-padding
  granularity, `#[cfg(test)]`-only Rust, no code-path change, no new dependency). Spec +
  `#[cfg(test)]` + this doc change only — no behaviour/response change (the handler was already
  correct). **The three vwip specs flagged on 2026-10-18 (device-data-volume, qos-booking,
  device-visit-location) are now all de-truncated and pinned; the vwip residual-drift re-audit is
  complete.** Branch `agent/device-visit-location/audit-vwip-missing-identifier`.
- 2026-10-18 — **vwip residual-drift re-audit — qos-booking, drift found & fixed.**
  Second of the three vwip specs the 2026-10-18 device-data-volume pass flagged as carrying the
  truncated-identifier-message drift (the vwip sweep closed 2026-10-13, one day before that drift
  class was first identified in the stable tier). Took the next in vwip mount order,
  **qos-booking**: both operations' (`createBooking` `POST …/device-qos-bookings`,
  `retrieveBookingByDevice` `POST …/retrieve-device-qos-bookings`) 422 `missingIdentifier`
  examples carried the short "The device cannot be identified." though `resolve_identifier`
  (`vwip.rs:863`) emits the full "The device cannot be identified: supply a `device` or use a
  token that identifies a device." (confirmed MISSING_IDENTIFIER router-reachable in both legs —
  a two-legged `client_credentials` token's `sub` is a non-line client id; existing
  `no_device_and_non_line_subject_is_422_missing_identifier` /
  `retrieve_no_device_and_non_line_subject_is_422_missing_identifier` tests prove the branch).
  Corrected both examples. Audited every other inline error example against the handler and found
  **no further drift** — the ten other API-own messages (`createBooking` 400 INVALID_ARGUMENT /
  OUT_OF_RANGE / QOS_BOOKING.DURATION_OUT_OF_RANGE / INVALID_SINK, 422 UNNECESSARY_IDENTIFIER /
  QOS_BOOKING.QOS_PROFILE_NOT_APPLICABLE / INVALID_AREA / AREA_NOT_COVERED / NOT_MANAGED_AREA_TYPE;
  `retrieve` 422 UNNECESSARY_IDENTIFIER) already byte-matched. Added
  `vendored_error_example_messages_match_the_handler` pinning the exact `status`/`code`/`message`
  bytes of all twelve router-reachable API-own inline error families across both operations (prior
  per-case tests guarded only `code`/`status`); the `createBooking` 409 CONFLICT example is
  deliberately left unpinned — its message is the shared-catalog `for_status(409)` canonical
  (reserved-suffix path), not an API-own string. Green: `cargo test` → **3127 passed, 0 failed**
  (+1); `cargo build --release` ok; binary = **5,463,192 bytes** (unchanged — embedded-spec text
  edit below section-padding granularity, `#[cfg(test)]`-only Rust, no code-path change, no new
  dependency). Spec + `#[cfg(test)]` + this doc change only — no behaviour/response change (the
  handler was already correct). **`device-visit-location` carries the same truncated-message drift
  and remains for the next pass** (last of the three flagged on 2026-10-18).
  Branch `agent/qos-booking/audit-vwip-missing-identifier`.
- 2026-10-18 — **vwip residual-drift re-audit — device-data-volume, drift found & fixed.**
  The 2026-10-15 idle note concluded the per-API audit sweep was complete across every tier,
  but noticed the vwip sweep closed **2026-10-13**, one day *before* the systematic drift
  classes (truncated-identifier-message, Swagger-placeholder examples, illustrative timestamps)
  were first identified in the stable-tier audit on **2026-10-14** — so vwip specs were never
  checked for them. A targeted grep across `specs/**` for the truncated
  "…cannot be identified." signature surfaced it in three vwip specs (`device-data-volume`,
  `qos-booking`, `device-visit-location`). Took the first in vwip mount order,
  **device-data-volume**: both operations' (`/retrieve`, `/check`) 422 `missingIdentifier`
  examples carried the short message though `resolve_identifier` (`vwip.rs:387`) emits the full
  "The device cannot be identified: supply a `device` or use a token that identifies a device."
  (confirmed MISSING_IDENTIFIER is router-reachable here — a two-legged `client_credentials`
  token's `sub` is a non-line client id). Corrected both examples and added
  `vendored_error_example_messages_match_the_handler` pinning the exact `status`/`code`/`message`
  bytes of all seven router-reachable API-own inline error families across both operations. Green:
  `cargo test` → **3126 passed, 0 failed** (+1); `cargo build --release` ok; binary =
  **5,463,192 bytes** (unchanged — embedded-spec text edit below section-padding granularity,
  `#[cfg(test)]`-only Rust, no code-path change, no new dependency). Spec + `#[cfg(test)]` +
  this doc change only — no behaviour/response change (the handler was already correct).
  **`qos-booking` and `device-visit-location` carry the same truncated-message drift and remain
  for the next passes.** Branch `agent/device-data-volume/audit-vwip-missing-identifier`.
- 2026-10-15 — **Idle pass; stale-local-ref self-correction (no audit increment).** A scheduled
  pass confirmed the per-API spec↔code audit sweep (backlog item 2) is **complete across every
  tier** — Phase 0 auth, Phase 1–4 stable, Phase 5 stable (all 18, last = in-home-device-management
  v1), and the vwip sweep (done 2026-10-13); no unaudited API remains as a fresh one-pass unit, so
  there was no new audit increment. **Correction to this pass's own first (now-superseded) note:**
  on entry the container presented a **detached HEAD at `da66aa0`** with local `main` *and* the
  `origin/main` remote-tracking ref both stale at `f15d2f9` (`fix(consent-info)`), which this pass
  first misread as "46 committed-but-unpushed commits / a §8 integration failure." That diagnosis
  was **wrong.** The push proved the actual remote `main` was **already at `da66aa0`** (push range
  `da66aa0..77cfa49`), i.e. every prior pass's work *had* been pushed; only the local clone's
  tracking ref was stale (the container checks out the true tip as a detached HEAD but leaves
  `main`/`origin/main` at an older value). No commits were ever stranded and the §8 push step is
  **not** broken. The only new content pushed this pass is this journal note. Verified green before
  pushing regardless: `cargo test` → **3125 passed, 0 failed**; `cargo build --release` → ok; binary
  = **5,463,192 bytes** (unchanged — no code/spec/behaviour change this pass). Branch
  `agent/integration/push-stranded-audit-commits`. **Note for future passes:** at container start,
  `git fetch origin` before trusting `origin/main`; a detached HEAD ahead of a stale local
  `origin/main` is this environment's normal checkout shape, not an integration failure.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — in-home-device-management v1
  (eighteenth and **last** Phase 5 stable API; the second remaining **stateful** one — a
  `store.rs` tombstone + overlay layer over an otherwise `ssid`-derived roster), drift found &
  fixed. Audited all six operations (`listDevices`, `getDevice`, `getDeviceNetworkHealth`,
  `performDeviceAction`, `deleteDevice`, `updateDevice`) against
  `specs/in-home-device-management/v1/openapi.yaml`; handler/store faithful (ssid reserved-suffix
  error plane; `d % 6` roster-shape plane + `connectionStatus` filter; delete tombstone single-use
  gate; update overlay with blocked>paused folded into `connectionStatus` and admin-state clearing
  → connected; connected→applied / offline→accepted action plane; read/write scope gates +
  `x-correlator`). **Stale-slice-doc drift** (device-swap / carrier-billing / qos-provisioning
  precedent): the parent module doc (`src/apis/in_home_device_management.rs`) and the spec header
  both still called `updateDevice` "a later slice" though PATCH is fully routed, documented in the
  spec's own `patch:` block and has ~16 tests — corrected both to the implemented second mutation.
  **Illustrative-value class** (device-identifier / sms / carrier-billing precedent): the 200/201
  success examples presented opaque FNV-1a `deviceId`/`macAddress`/`actionId`, hash-derived health
  figures and frozen wall-clock `measuredAt`/`appliedAt` as if literal (none reproduce) — marked
  them illustrative with a derivation note on each operation's 200/201 response `description`,
  keeping the reproducible scalars as the contract. Inline 400 examples already byte-matched;
  every other 4xx/5xx `$ref`s the shared catalog. Added two pinning tests
  (`vendored_success_examples_are_illustrative_shapes_matching_the_handler`,
  `vendored_invalid_argument_examples_match_the_handler`). `cargo test` green: 3125 passed, 0
  failed (+2). `cargo build --release` green; binary = 5,463,192 bytes (unchanged vs
  qos-provisioning — the embedded-spec text additions did not move the measured size; tests are
  `#[cfg(test)]`, no code-path change, no new dependency). Spec + doc + `#[cfg(test)]` change only.
  **This completes the Phase 5 stable/numbered tier (all 18 audited); the only remaining
  per-API spec↔code audit tier is the vwip APIs, whose sweep completed 2026-10-13.** Branch
  `agent/in-home-device-management/audit-v1`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — qos-provisioning v0.3
  (seventeenth Phase 5 stable API; the first of the two remaining **stateful** ones — the QoD
  sibling, `store.rs` provisioning lifecycle + `notifications.rs` CloudEvent sink), drift found &
  fixed. Audited all four operations (`createQosAssignment`, `getQosAssignmentById`,
  `revokeQosAssignment`, `getQosAssignmentByDevice`) against `specs/qos-provisioning/v0.3/openapi.yaml`;
  handler/store/notifications faithful (grant plane `…000`→REQUESTED else AVAILABLE/no `expiresAt`;
  `…409`→CONFLICT; `unavailable`-profile→422; `http(s)` sink + AVAILABLE/`NETWORK_TERMINATED`(`…001`)/
  `DELETE_REQUESTED` CloudEvents with ACCESSTOKEN→Bearer/PLAIN→Basic; two-/three-legged resolution;
  scopes + `x-correlator`). `assignmentId` (counter+clock `mint_uuid`) and `startedAt` (wall-clock)
  are legitimately illustrative (QoD posture). **Message/example drift (four fixes):** (1) the 422
  `missingIdentifier` example in **both** create and retrieve was truncated ("The device cannot be
  identified.") vs the handler's full "…: supply a `device` or use a token that identifies a device."
  (truncated-identifier-message class); (2) the create 400 `invalidSink` example said "valid
  `https://` …" vs the handler's "valid `http://` or `https://` …"; (3) the create 409 `conflict`
  example said "There is a conflict with the current state of the resource." vs the real
  `for_status(409)` "A specified resource duplicate entry found."; (4) the three 200/201 success
  examples carried Swagger-placeholder `assignmentId`s + frozen `startedAt`s — marked both
  illustrative with a derivation note (QoD `sessionId`/`startedAt` convention). **Stale-doc drift:**
  three code sites (v0_3.rs header, the create-handler sink comment, a create test comment) still
  called `https://` sink delivery a "documented no-op cut (no TLS client)" though `notifications.rs`
  delivers over a verified `rustls` TLS session (and the spec says so) — corrected all three; and the
  parent `qos_provisioning.rs` module doc's "First slice: POST /qos-assignments + GET /…/{id}" (only
  2 of 4 ops) was corrected to the full surface (stale-slice-doc class). Added two pinning tests —
  `vendored_examples_match_the_handler` (two 201 + the retrieve 200 as exact objects minus the
  illustrative `assignmentId`/`startedAt`, each asserted UUID-shaped/UTC and ≠ the frozen literal;
  round-trip + not-found cases use test-unique tails …517/…619 so the process-global store can't
  cross-contaminate) and `vendored_error_example_messages_match_the_handler` (exact
  `status`/`code`/`message` bytes of all nine router-reachable inline error families across both
  operations; prior error tests guarded only `status`/`code`). `cargo test` green: 3123 passed, 0
  failed (+2). `cargo build --release` green; binary = 5,463,192 bytes (+4,096 vs sms's 5,459,096 —
  the embedded-spec YAML grew from the longer example descriptions/messages; no code-path change, no
  new dependency). Spec + doc + `#[cfg(test)]` change only — no behaviour/response change (the
  handlers were already correct; the drift was message text, example placeholders, and stale docs).
  Next in phase order: in-home-device-management v1 (the last Phase 5 stable API).
  Branch `agent/qos-provisioning/audit-v0_3`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — sms v0alpha1 (sixteenth
  Phase 5 stable API, the next **stateless** one, taken ahead of the two remaining stateful
  stable APIs per phase order), drift found & fixed. Audited the one operation (`send-sms`,
  `POST /sms/v0alpha1/short-message`) against `specs/sms/v0alpha1/openapi.yaml`; handler faithful
  (two-legged/business-facing, the `to[0]`-reserved-suffix error plane, the deterministic SHA-256
  `msgId`, the full 400 `INVALID_ARGUMENT` validation set, scope + `x-correlator` echo). **Drift —
  the deterministic-value/placeholder + illustrative-timestamp classes:** the 200 `sent` example
  carried a Swagger-placeholder `msgId` (`3fa85f64-…`) bound to no request though `msgId` is fully
  reproducible, and a frozen wall-clock `timestamp`. Bound it to the concrete request, corrected
  `msgId` to the real `ceb0abb4-354b-46af-a9f3-845e4bd92fb0`, marked `timestamp` illustrative. The
  400 `invalidArgument` example already byte-matched; all other 4xx/5xx `$ref` the shared catalog;
  module docs already accurate (no stale-slice drift). Added two pinning tests
  (`vendored_sent_example_matches_the_handler` — pins `msgId` vs the spec literal and `message_id()`
  + the `timestamp` UTC shape through the router; `vendored_invalid_argument_example_matches_the_handler`
  — pins the 400 example's exact status/code/message bytes). `cargo test` green: 3121 passed, 0 failed.
  `cargo build --release` green; binary = 5,459,096 bytes. Spec + `#[cfg(test)]` change only, no
  behaviour change, no new dependency.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — region-device-count v0.2
  (fifteenth Phase 5 stable API), drift found & fixed. Audited the one operation (`count`,
  `POST /region-device-count/v0.2/count`) against `specs/region-device-count/v0.2/openapi.yaml`.
  This API is the tier's odd one out — **area-keyed, not identifier-keyed** (no phone/device id),
  so no truncated-identifier-message class to check; its control planes key off the region's
  characteristic radius `r` (a CIRCLE's `radius`; a POLYGON's `sqrt(area/π)`) and the optional
  `filter`. The handler is faithful: the region-size plane (`r > 1e6` →
  `REGION_DEVICE_COUNT.UNSUPPPORTED_REQUEST`, triple-P verbatim; `5e5 < r ≤ 1e6` without a `sink`
  → `UNSUPPORTED_SYNC_RESPONSE`, with a `sink` accepted + answered synchronously — async delivery
  a documented cut), the trailing-three-digits coverage plane (`round(r)` → `…429`→429
  TOO_MANY_REQUESTS, `…001`→PART_OF_AREA_NOT_SUPPORTED with a count for the covered ~50%,
  `…002`→AREA_NOT_SUPPORTED, `…003`→DENSITY_BELOW_PRIVACY_THRESHOLD, `…004`→TIME_INTERVAL_NO_DATA_FOUND,
  else SUPPORTED_AREA with a count), the fixed-density count (500 dev/km² × area, integer-halved for
  PART, narrowed by `filter`'s fixed category shares — non-roaming 0.85/roaming 0.15; human 0.70/IoT
  0.25/other 0.05), and the full validation set (CIRCLE missing center/radius → INVALID_CIRCLE_AREA,
  radius<1/out-of-range center → INVALID_ARGUMENT; POLYGON 3–15 valid points or zero-area →
  INVALID_POLYGON_AREA; time both-or-neither → TIME_INVALID_ARGUMENT, end<start → INVALID_END_DATE,
  malformed → INVALID_ARGUMENT; empty/bad filter → INVALID_ARGUMENT; sinkCredential non-ACCESSTOKEN →
  INVALID_CREDENTIAL, non-bearer → INVALID_TOKEN) all match, and the `region-device-count:count` scope
  + `x-correlator` echo are genuinely enforced. **Drift found — the deterministic-value class**
  (population-density-data / network-traffic-analysis precedent, not the identifier-message class):
  the vendored 200 `partial` example claimed `count: 3142` — a hand-derived half of the `supported`
  example's 6283 — but the handler computes the **full** count for the concrete `…001` radius first,
  then integer-halves: radius 2001 → area π·2001² ≈ 12.579 km² × 500 = round(6289.5)=6289, `6289/2 =
  3144`. Corrected the example to 3144 and bound all three 200 example summaries to their concrete
  reproducing radii (2000 → 6283 SUPPORTED_AREA; 2001 → 3144 PART_OF_AREA_NOT_SUPPORTED; 2003 →
  DENSITY_BELOW_PRIVACY_THRESHOLD, no count). The three inline 400 error examples (`invalidCircleArea`
  "A CIRCLE area requires both `center` and `radius`.", `unsupportedSync` "…too large for a synchronous
  response; supply a `sink`.", `unsupportedRequest` "…too large to be processed.") already byte-matched
  the handler; the 401/403/429 responses all `$ref` the shared catalog (no other inline error examples
  to drift). The parent + version module docs and the `x-camarasim-scenarios` block were already
  accurate (one modelled operation, the async sink/CloudEvents surface a documented cut) — no
  stale-slice-doc drift. **Tested increment:** added `vendored_examples_match_the_handler` driving all
  three documented 200 requests through the real router and pinning each body as an **exact object**
  (the existing router tests asserted `status`/`count` individually and would let a stray/renamed field
  or the drifted 3142 slip past), plus the exact `status`/`code`/`message` bytes of all three inline 400
  families (the existing error tests guarded only `code`). `cargo test` 3119 green (+1);
  `cargo build --release` ok, binary 5,459,096 bytes (unchanged — the embedded-YAML edit, one count
  digit plus lengthened example summaries, is below section-padding granularity; no code-path change,
  no new dependency). Spec + `#[cfg(test)]` change only — no behaviour/response change (the handler was
  already correct; one example `count` was hand-derived). Next in phase order: the remaining Phase 5
  stable APIs (qos-provisioning v0.3, sms v0alpha1, in-home-device-management v1).
  Branch `agent/region-device-count/audit-v0_2`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — connectivity-insights v0.6
  (fourteenth Phase 5 stable API), drift found & fixed. Audited the one operation (`checkNetworkQuality`,
  `POST /connectivity-insights/v0.6/check-network-quality`) against
  `specs/connectivity-insights/v0.6/openapi.yaml`. The handler is faithful: the two control planes (the
  identifier reserved-error-suffix plane; the trailing-three-digits plane where the low five bits
  `d & 0x1F` form an *unmet mask*, one bit per KPI in the fixed order `packetDelayBudget`,
  `targetMinDownstreamRate`, `targetMinUpstreamRate`, `packetlossErrorRate`, `jitter` — set → `unable to
  meet…`, clear → `meets…` — and `additionalKPIs` `signalStrength`/`connectivityType` degrade coherently
  with `met = 5 − popcount`, so `…000`/no digits → all five met / excellent / 5G-SA, `…031` → all unmet /
  no signal / 3G) match, the two-legged/three-legged identifier resolution is correct (device precedence
  phoneNumber → NAI → IPv4 `publicAddress` → ipv6; Connectivity Insights 0.6.0 defines **no**
  `UNNECESSARY_IDENTIFIER`, so a `device` on a line token is simply used — the submitted device wins over
  the subject; no `device` + non-line subject → `422 MISSING_IDENTIFIER`; a `device` with no identifier →
  `400 INVALID_ARGUMENT`), and the required `applicationProfileId` UUID + required `applicationServer`
  (`minProperties:1`) + optional `applicationServerPorts` (`0..=65535`, `from<=to`) + `monitoringTimeStamp`
  RFC-3339 shape, the `phoneNumber` E.164 pattern, `deny_unknown_fields`, the `connectivity-insights:check`
  scope and the `x-correlator` echo are genuinely enforced. The response echoes the device only for a
  phone-keyed request and carries no wall-clock field, so both 200 examples reproduce exactly. **Drift
  found — the truncated-identifier-message class** (connected-network-type / simple-edge-discovery /
  carrier-billing / customer-insights precedent): the 422 `missingIdentifier` example carried the short
  "The device cannot be identified." but `resolve_identifier` emits the full "The device cannot be
  identified: supply a `device` or use a token that identifies a device." Corrected it. The two 200
  examples (`healthy` `…000` → all met / excellent / 5G-SA / device echoed; `degraded` `…001` →
  packetDelayBudget unmet, four met, good / 5G-NSA), the 400 `invalidArgument` ("`applicationProfileId`
  must be a UUID.") / `outOfRange` ("A port must be within 0..=65535.") and the 422 `serviceNotApplicable`
  (= `CamaraError::for_status(422)`, "The service is not available for the provided identifier.") examples
  already byte-matched; the 401/403/404/409/429/500/503 responses all `$ref` the shared catalog (no other
  inline error examples to drift). The parent module doc (`src/apis/connectivity_insights.rs`) was already
  accurate (one modelled operation; the Application Profiles CRUD + subscriptions companions correctly out
  of scope) — no stale-slice-doc drift. **Tested increment:** added `vendored_examples_match_the_handler`
  driving both documented 200 requests through the real router and pinning each body as an **exact object**
  (the existing router tests asserted the KPI/`additionalKPIs`/`device` fields individually, so a
  stray/renamed field could slip past), plus the exact `status`/`code`/`message` bytes of all four
  router-reachable inline error families (`INVALID_ARGUMENT`/`OUT_OF_RANGE`/`MISSING_IDENTIFIER`/
  `SERVICE_NOT_APPLICABLE`) — the existing error tests guarded only `code`. `cargo test` 3118 green (+1);
  `cargo build --release` ok, binary 5,459,096 bytes (unchanged — the +~60-byte lengthened
  `missingIdentifier` message is embedded via `include_str!` but below section-padding granularity; no
  code-path change, no new dependency). Spec + `#[cfg(test)]` change only — no behaviour/response change
  (the handler was already correct; one message was truncated). Next in phase order: the remaining Phase 5
  stable APIs (region-device-count v0.2, qos-provisioning v0.3, sms v0alpha1, in-home-device-management
  v1). Branch `agent/connectivity-insights/audit-v0_6`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — connected-network-type v0.2
  (thirteenth Phase 5 stable API), drift found & fixed. Audited the one operation
  (`getConnectedNetworkType`, `POST /connected-network-type/v0.2/retrieve`) against
  `specs/connected-network-type/v0.2/openapi.yaml`. The handler is faithful: the two control planes
  (the identifier reserved-error-suffix plane; the trailing-three-digits radio plane — `digits % 5`
  over the fixed newest-first `NETWORK_TYPES` table `[5G,4G,3G,2G,UNKNOWN]`, `…000`/no digits → index
  0 `5G`, so the reported technology is a genuine second control plane) match, the
  two-legged/three-legged identifier resolution is correct (device precedence phoneNumber → NAI →
  IPv4 `publicAddress` → ipv6; a `device` resubmitted on a line-subject three-legged token → `422
  UNNECESSARY_IDENTIFIER`; no device + non-line subject → `422 MISSING_IDENTIFIER`; an empty `device`
  object → `400 INVALID_ARGUMENT`), the `DeviceResponse` is echoed only for a phone-keyed request,
  `lastStatusTime` is the current instant for a known attachment and `null` for `UNKNOWN`, and the
  E.164 pattern, `deny_unknown_fields`, the `connected-network-type:read` scope and the `x-correlator`
  echo are genuinely enforced. **Drift found — the truncated-identifier-message class**
  (simple-edge-discovery / carrier-billing / customer-insights precedent): the 422 `missingIdentifier`
  example carried the short "The device cannot be identified." but the handler emits the full "The
  device cannot be identified: supply a `device` or use a token that identifies a device." Corrected
  it. **Also the illustrative-timestamp class** (sim-swap `latestSimChange` / device-identifier
  `lastChecked` / quality-on-demand precedent): the `phoneKeyed5G` and `ipKeyed3G` 200 examples
  presented `lastStatusTime` as a frozen literal (`2024-01-01T14:27:08Z`) though `retrieve` stamps the
  current instant (wall-clock dependent — the literal can never reproduce); marked both **illustrative**
  with a derivation note, and bound all three 200 examples to the concrete reproducing request
  (`phoneNumber +123456789000` → idx 0 `5G` device-echoed; `ipv4 publicAddress 203.0.113.002` → idx 2
  `3G` no-echo; `phoneNumber +123456789004` → idx 4 `UNKNOWN` null — the UNKNOWN body reproduces
  exactly, including the device echo). The `unnecessaryIdentifier` ("The device is already identified
  by the access token.") and `serviceNotApplicable` (= `CamaraError::for_status(422)`, "The service is
  not available for the provided identifier.") 422 examples already byte-matched; the
  400/401/403/404/409/429/500/503 responses all `$ref` the shared catalog (no other inline error
  examples to drift). The parent module doc (`src/apis/connected_network_type.rs`) was already accurate
  (one operation, event-subscription surface correctly out of scope) — no stale-slice-doc drift.
  **Tested increment:** added `vendored_examples_match_the_handler` driving all three documented 200
  requests through the real router and pinning each body as an **exact object** (minus the illustrative
  `lastStatusTime`, asserted present/UTC `Z` or `null`), plus the exact `status`/`code`/`message` bytes
  of the three router-reachable inline 422 families (`MISSING_IDENTIFIER`/`UNNECESSARY_IDENTIFIER`/
  `SERVICE_NOT_APPLICABLE`) — the existing error tests guarded only `code` and the 200 fields
  individually. `cargo test` 3117 green (+1); `cargo build --release` ok, binary 5,459,096 bytes.
  Note: specs are embedded in the binary via `include_str!` (`src/registry.rs`), so the vendored-YAML
  edits (the illustrative-example descriptions + the lengthened `missingIdentifier` message) are the
  +~4 KB vs the last recorded size; no code-path change, no new dependency. Spec + `#[cfg(test)]`
  change only — no behaviour/response change (the handler was already correct; one message was
  truncated, two timestamps illustrative). Next in phase order: the remaining Phase 5 stable APIs
  (connectivity-insights v0.6, region-device-count v0.2, qos-provisioning v0.3, sms v0alpha1,
  in-home-device-management v1). Branch `agent/connected-network-type/audit-v0_2`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — customer-insights v0.2
  (twelfth Phase 5 stable API), drift found & fixed. Audited the one operation (`retrieveScoring`,
  `POST /customer-insights/v0.2/scoring/retrieve`) against
  `specs/customer-insights/v0.2/openapi.yaml`. The handler is faithful: the two control planes (the
  identifier reserved-error-suffix plane; the trailing-three-digits score plane — `gaugeMetric →
  300 + d%551` [300–850], `veritasIndex → d%20` [0–19], so the same number reads a different value
  on each scale and `scoringType` is a genuine second control plane, `…000`/no digits → the scale
  floor) match, the two-legged/three-legged identifier resolution is correct (`phoneNumber` on a
  line-subject three-legged token → `422 UNNECESSARY_IDENTIFIER`; no `phoneNumber` + non-line
  subject + `idDocument` → `422 CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED`; neither → `422
  MISSING_IDENTIFIER`), and the E.164 pattern, `deny_unknown_fields`, the 1–30 char `idDocument`
  bound, the `customer-insights:scoring:read` scope and the `x-correlator` echo are genuinely
  enforced. **Drift found — the truncated-identifier-message class** (simple-edge-discovery /
  carrier-billing / kyc-tenure / consent-info precedent): both the 422 `missingIdentifier` and the
  422 `idDocumentNotSupported` examples carried truncated messages ("The phone number cannot be
  identified." and "Scoring by `idDocument` alone is not supported.") but the handler emits the full
  forms ("…: supply `phoneNumber` or use a token that identifies a line." and "…; supply a
  `phoneNumber` or use a token that identifies a line."). Corrected both. The `unnecessaryIdentifier`
  ("The phone number is already identified by the access token."), `serviceNotApplicable` (=
  `CamaraError::for_status(422)`, "The service is not available for the provided identifier.") and
  400 `invalidArgument` (the body-parse message) examples already byte-matched; every
  401/403/404/409/429/500/503 response `$ref`s the shared catalog. The two 200 examples (gauge 489,
  veritas 0) are reproducible deterministic scalars, so their summaries were bound to the concrete
  reproducing line `+123456789740` (gauge: 300 + 740%551 = 489; veritas: 740%20 = 0). **Tested
  increment:** added `vendored_examples_match_the_handler` driving both documented 200 requests
  through the real router and pinning each body as an **exact object**, plus the exact
  `status`/`code`/`message` bytes of all five router-reachable inline error families (400
  `INVALID_ARGUMENT`; 422 `MISSING_IDENTIFIER`/`UNNECESSARY_IDENTIFIER`/`ID_DOCUMENT_NOT_SUPPORTED`/
  `SERVICE_NOT_APPLICABLE`) — the existing tests guarded only `code` and the 200 fields
  individually. `cargo test` 3116 green (+1); `cargo build --release` ok, binary 5,455,000 bytes
  (5.3M, byte-identical to the simple-edge-discovery / carrier-billing passes — the YAML example
  corrections are outside the release path and the new test is `#[cfg(test)]`-only). Spec +
  `#[cfg(test)]` change only — no behaviour/response change (the handler was already correct; two
  examples were truncated), no new dependency. Next in phase order: the remaining Phase 5 stable
  APIs (connected-network-type v0.2, connectivity-insights v0.6, region-device-count v0.2,
  qos-provisioning v0.3, sms v0alpha1, in-home-device-management v1). Branch
  `agent/customer-insights/audit-v0_2`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — simple-edge-discovery v2
  (eleventh Phase 5 stable API), drift found & fixed. Audited the one operation
  (`readClosestEdgeCloudZone`, `POST /simple-edge-discovery/v2/retrieve-closest-edge-cloud-zone`)
  against `specs/simple-edge-discovery/v2/openapi.yaml`. The handler is faithful: the two control
  planes (the identifier reserved-error-suffix plane; the trailing-three-digits
  `EDGE_ZONES[digits % 6]` closest-zone plane over the fixed 6-entry table, `…000`/no digits →
  index 0, so the zone is a genuine second control plane) match, the two-legged/three-legged
  identifier resolution is correct (`device` precedence phoneNumber → NAI → IPv4 `publicAddress` →
  ipv6; a `device` resubmitted on a line-subject three-legged token → `422 UNNECESSARY_IDENTIFIER`;
  no device + non-line subject → `422 MISSING_IDENTIFIER`; an empty `device` object → `400
  INVALID_ARGUMENT`), the `DeviceResponse` is echoed only for a phone-keyed request, and the E.164
  pattern, `deny_unknown_fields`, the `simple-edge-discovery:read` scope and the `x-correlator` echo
  are genuinely enforced. **Drift found — the deterministic-value / placeholder class**
  (blockchain-public-address / verified-caller / device-identifier / optimal-edge-discovery
  precedent): the two 200 examples (`phoneKeyed`, `ipKeyed`) and the `EdgeCloudZone` schema-level
  example all carried hand-picked placeholder `edgeCloudZoneId`s (`b1e5f0a2-…` / `c2f6018b-…`)
  though `zone_id` derives the id deterministically as SHA-256(`sed-zone:<zoneName>`) rendered
  UUID-shaped — eu-west-1 (index 0) = `ddf71ac2-cdd5-9f27-40b6-e042dcf7b697`, ap-northeast-1
  (index 5, the `203.0.113.005` → `…005` → `5 % 6` IP example) =
  `95b0c904-3bf3-d083-d64e-e0f15fce3a62`. Corrected all three to the real derived values and bound
  each 200 example's summary to the concrete request that reproduces it. **Also the
  truncated-identifier-message class** (device-swap / kyc-tenure / media-streaming-rate /
  optimal-edge-discovery precedent): the 422 `missingIdentifier` example carried the short "The
  device cannot be identified." but the handler emits the full "…: supply a `device` or use a token
  that identifies a device." — corrected. The `unnecessaryIdentifier` ("The device is already
  identified by the access token.") and `serviceNotApplicable` (= `CamaraError::for_status(422)`,
  "The service is not available for the provided identifier.") 422 examples already byte-matched; the
  400/401/403/404/409/429/500/503 responses all `$ref` the shared catalog (no other inline error
  examples to drift). **Tested increment:** added `vendored_examples_match_the_handler` driving both
  documented 200 requests (`+123456789012` → idx 0 device-echoed; `publicAddress 203.0.113.005` →
  idx 5 no-echo) through the real router and pinning each body as an **exact object** (the
  `edgeCloudZoneId` also asserted against `zone_id`), plus the exact `status`/`code`/`message` bytes
  of all three router-reachable inline 422 families (the existing error tests guarded only `code`).
  `cargo test` 3115 green (+1); `cargo build --release` ok, binary 5,455,000 bytes (5.3M,
  byte-identical to the carrier-billing / blockchain-public-address passes — the YAML example
  corrections are outside the release path and the new test is `#[cfg(test)]`-only). Spec +
  `#[cfg(test)]` change only — no behaviour/response change (the handler was already correct; the
  examples were placeholders / one truncated message), no new dependency. Next in phase order: the
  remaining Phase 5 stable APIs (customer-insights v0.2, connected-network-type v0.2,
  connectivity-insights v0.6, region-device-count v0.2, qos-provisioning v0.3, sms v0alpha1,
  in-home-device-management v1). Branch `agent/simple-edge-discovery/audit-v2`.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — carrier-billing v0.5
  (tenth Phase 5 stable API, the **second stateful** one), drift found & fixed. Audited all seven
  operations against `specs/carrier-billing/v0.5/openapi.yaml`: the one-step `createPayment`
  (`POST /payments`), the read-back `retrievePayment` (`GET /payments/{id}`) and list
  `retrievePayments` (`GET /payments`, paginated/ordered over the in-memory `store.rs`), and the
  complete two-step reserve → validate → confirm/cancel flow (`preparePayment`
  `POST /payments/prepare`, `validatePayment` `POST …/{id}/validate`, `confirmPayment`
  `POST …/{id}/confirm`, `cancelPayment` `POST …/{id}/cancel`). The handler is faithful: the two
  shared control planes (the identifier reserved-error-suffix plane + the `…888`
  validation-required tail that lands a reservation in `pending_validation`; the `amount` plane —
  `< 0.001` → 400 `INVALID_ARGUMENT`, `> 1000` → 422 `CARRIER_BILLING.UNAUTHORIZED_AMOUNT`, with
  the identifier plane checked first), the deterministic OTP `code` (last six digits) and 3-attempt
  budget → `CARRIER_BILLING.VALIDATION_FAILED` (reservation → `denied`), and the confirm/cancel
  store state machine (`reserved` → `succeeded`/`cancelled`; re-confirm → `PAYMENT_CONFIRMED`,
  re-cancel → `PAYMENT_CANCELLED`, a `pending_validation`/`denied` payment → `CONFLICT`) all match.
  The `phoneNumber` E.164 pattern, `deny_unknown_fields`, the three scopes
  (`:create`/`:read`/`:write`) and `x-correlator` echo are genuinely enforced; the optional
  confirm/cancel `phoneNumber` body field is a documented cut (the reservation is addressed by its
  opaque `paymentId`). The minted `paymentId` (counter+clock `mint_uuid`, not request-derived) and
  the wall-clock `paymentCreationDate`/`paymentDate` are legitimately illustrative (the
  application-profiles / session-insights posture), so the 200/201 success examples are not pinned.
  **Drift found — the truncated-identifier-message class** (device-swap / kyc-fill-in / kyc-tenure /
  consent-info / subscription-status / media-streaming-rate precedent): both the `createPayment`
  and `preparePayment` 422 `missingIdentifier` examples carried the short "The phone number cannot
  be identified." but `resolve_phone_number` emits the full "…: supply `phoneNumber` or use a token
  that identifies a phone number." (note: carrier-billing's wording ends "a phone number", not the
  "a line" of the line-keyed APIs). Corrected both. **Stale parent module doc fixed**
  (`src/apis/carrier_billing.rs`): the submodule listing said `confirmPayment`/`cancelPayment` were
  "later slices" though both ship and are routed — the same stale-"later slice/deferred" doc trap
  caught on device-data-volume / qos-booking / application-profiles / session-insights /
  sponsored-data; corrected to list all seven operations as implemented (the `v0_5.rs` submodule
  header already said "the two-step flow is now complete", so only the parent was stale). **Tested
  increment:** added `vendored_error_example_messages_match_the_handler` driving each documented
  request through the real router and pinning the exact `status`/`code`/`message` bytes of all
  fourteen router-reachable inline error families whose message is the API's own —
  createPayment/preparePayment 422 `UNAUTHORIZED_AMOUNT`/`MISSING_IDENTIFIER`; validatePayment 400
  `INVALID_AUTHORIZATION_ID`/`INVALID_CODE`/`VALIDATION_FAILED` + 409 `ALREADY_EXISTS`;
  confirmPayment & cancelPayment 409 `PAYMENT_CONFIRMED`/`PAYMENT_CANCELLED`/`CONFLICT` — which the
  existing error tests guarded only by `code`. The two createPayment/preparePayment 400
  `INVALID_ARGUMENT` examples carry the shared catalog's canonical generic message, which the
  handler never emits (every `INVALID_ARGUMENT` path returns a specific per-cause message such as
  "`amount` must be at least 0.001."), so — like other shared-message examples — they are
  intentionally left unpinned; every other 4xx/5xx `$ref`s the shared catalog. `cargo test` 3114
  green (+1); `cargo build --release` ok, binary 5,455,000 bytes (5.3M, byte-identical to the
  blockchain-public-address pass — the YAML message correction and the module-doc fix are outside
  the release path and the new test is `#[cfg(test)]`-only). Spec + doc + `#[cfg(test)]` change
  only — no behaviour/response change (the handler was already correct; the two examples were
  truncated and the parent doc was stale), no new dependency. Next in phase order: the remaining
  Phase 5 stable APIs (simple-edge-discovery v2, customer-insights v0.2, connected-network-type
  v0.2, connectivity-insights v0.6, region-device-count v0.2, qos-provisioning v0.3, sms v0alpha1,
  in-home-device-management v1). Branch `agent/carrier-billing/audit-v0_5`. **Integration note:**
  on entry HEAD was detached at `origin/main` (`5c933d2`, blockchain-public-address); local `main`
  ref was stale (`f15d2f9`) and was reset to `origin/main` before branching.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — blockchain-public-address
  v0.3 (ninth Phase 5 stable API, the **first stateful** one in the tier), drift found & fixed.
  Audited all three operations against `specs/blockchain-public-address/v0.3/openapi.yaml`:
  `retrieveBlockchainPublicAddress` (`POST …/retrieve-blockchains`, stateless,
  deterministic-synthetic), `bindBlockchainPublicAddress` (`POST …/blockchain-public-addresses`,
  persists a binding in the in-memory `store.rs`, mints an `id`), and
  `deleteBlockchainPublicAddress` (`DELETE …/{id}`, evicts from the store). All control planes
  match the handler — the `phoneNumber` reserved-error-suffix plane; the trailing-three-digits
  address-set plane on retrieve (`d == 0` → `[]`, else `((d-1)%3)+1` records, the `i`-th on
  `NETWORKS[(d+i)%6]` so the chain is a genuine second control plane); bind's request-validation
  plane (`blockchainNetworkId` `<eco>:<sub>` → `INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER`, EVM
  `0x`+40-hex address, lone `nonce`/`signature` → `BOTH_NONCE_SIGNATURE_REQUIRED`, both →
  422 `UNSUPPORTED_ENHANCED_VALIDATION`) and store plane (duplicate triple → 409 `ALREADY_EXISTS`,
  the id being derived from the triple); and delete's store-only plane (present → 204, absent →
  404, single-use). The E.164 pattern, the network-id and EVM-address validators, and
  `deny_unknown_fields` are all genuinely enforced; the three scopes
  (`:read`/`:create`/`:delete`) gate correctly and `x-correlator` is echoed on success and error.
  **Drift found — the deterministic-value / placeholder class** (verified-caller /
  device-identifier / optimal-edge-discovery precedent, new to this Phase 5 tier — the tier's
  other drift so far was the truncated-identifier-message / incomplete-example classes): the two
  response-level `bound` examples both carried Swagger placeholder values, not reproducible
  handler output. Worse, the retrieve 200 `bound` example claimed a **single** address on
  `eip155:1` with `currency: [ETH]` — unreachable under the mapping: `count == 1` needs
  `d ≡ 1 (mod 3)`, but the single address lands on `NETWORKS[d % 6]`, and `eip155:1` =
  `NETWORKS[0]` needs `d % 6 == 0` (⇒ `d ≡ 0 mod 3`, which always gives `count == 3`), so no
  single-address response is ever on Ethereum Mainnet. Bound the retrieve example to
  `{"phoneNumber":"+123456789001"}` (`d = 1` → one address on `NETWORKS[1] = eip155:137`, Polygon)
  with the real SHA-256-derived `id` (`ee31cfad-…`) / `blockchainPublicAddress`
  (`0x92fb45f4…`) / `currency: [POL]`, and bound the bind 201 `bound` example to the
  `BindBlockchainPublicAddressRequest` example triple (`+123456789012` / `evm:1` /
  `0x9f8a7b6c…`) whose `binding_id` is `62a4c79b-fd1d-0a55-5737-424dc3563306`, so a reader can
  POST the request example and get the response example's id. Also corrected the
  "`+123456789001` → one Ethereum-mainnet address" claim in both the spec API description and the
  module doc to "one Polygon (`eip155:137`) address" (with a note that a single-address case is
  never on `eip155:1`). The error responses all `$ref` the shared catalog — no inline error
  examples to drift. **Tested increment:** added `vendored_examples_match_the_handler` driving
  `+123456789001` (retrieve) and the request-example triple (bind) through the real router and
  pinning each body as an exact object, the bind id also asserted equal to `binding_id` over the
  triple. `cargo test` 3113 green (+1); `cargo build --release` ok, binary 5,455,000 bytes (5.3M,
  byte-identical to the kyc-tenure pass — the YAML example correction and the module-doc fix are
  outside the release path and the new test is `#[cfg(test)]`-only). Spec + doc + `#[cfg(test)]`
  change only — no behaviour/response change (the handler was already correct; the examples were
  placeholders), no new dependency. Next in phase order: carrier-billing v0.5 (the next Phase 5
  stable; also stateful), then the remaining Phase 5 stable APIs (simple-edge-discovery v2,
  customer-insights v0.2, connected-network-type v0.2, connectivity-insights v0.6,
  region-device-count v0.2, qos-provisioning v0.3, sms v0alpha1, in-home-device-management v1).
  Branch `agent/blockchain-public-address/audit-v0_3`. **Integration note:** on entry HEAD was
  detached at `origin/main` (`725aba2`, kyc-tenure); local `main` ref was stale (`f15d2f9`) and
  was reset to `origin/main` before branching.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — kyc-tenure v0.2
  (eighth Phase 5 stable API), drift found & fixed. Audited the one operation (`checkTenure`,
  `POST /kyc-tenure/v0.2/check-tenure`) against `specs/kyc-tenure/v0.2/openapi.yaml`. KYC Tenure
  is a stateless, privacy-preserving trust signal: it answers only yes/no about whether the
  subscriber has held the line since at least a caller-supplied `tenureDate`. Both control planes
  match the handler — the identifier reserved-error-suffix plane, and the trailing-three-digits
  held-days verdict compared against `tenureDate` (`tenureDateCheck` iff `(today − tenureDate) <=
  held_days`), which makes `tenureDate` a genuine second control plane (the same number flips
  true→false against an older reference date); `contractType = [PAYG,PAYM,Business][digits % 3]`
  is reproducible from the identifier alone. The `phoneNumber` E.164 pattern and the strict
  self-contained `YYYY-MM-DD` date parse (impossible dates like `2026-02-30` → 400
  `INVALID_ARGUMENT` via a civil round-trip; a future date → 400 `OUT_OF_RANGE`) are genuinely
  enforced, and the two-legged/three-legged identifier resolution (number-recycling posture) is
  correct. **Drift found — the truncated-identifier-message class** (device-swap / kyc-fill-in /
  kyc-age-verification / number-recycling / call-forwarding-signal / consent-info precedent): the
  422 `missingIdentifier` example carried the short `message: "The phone number cannot be
  identified."`, but `resolve_identifier` emits the full "…: supply `phoneNumber` or use a token
  that identifies a line." Corrected the example. The 400 `invalidArgument` (`tenureDate` parse
  message) / `outOfRange`, 422 `unnecessaryIdentifier`, and the shared `serviceNotApplicable`
  (= `CamaraError::for_status(422)`) inline examples already byte-matched the handler, and every
  401/403/404/409/429/500/503 response `$ref`s the shared catalog. **Small tested increment:** the
  existing error tests guarded only `status`/`code`, leaving the five documented `message` strings
  unpinned; added `vendored_error_example_messages_match_the_handler` driving each documented
  request through the real router and pinning the exact `status`/`code`/`message` bytes — 400
  `invalidArgument`/`outOfRange`, 422 `missingIdentifier` (reachable here: a `client_credentials`
  token's `sub` is a non-line client id), `unnecessaryIdentifier`, `serviceNotApplicable`. `cargo
  test` 3112 green (+1); `cargo build --release` ok, binary 5,455,000 bytes (5.3M, byte-identical
  to the qos-profiles pass — the YAML message correction is outside the release path and the new
  test is `#[cfg(test)]`-only). Spec + `#[cfg(test)]` change only — no behaviour/response change
  (the handler was already correct; only the one example was truncated), no new dependency. Next
  in phase order: blockchain-public-address v0.3 (the next Phase 5 stable; it carries a `store.rs`
  — the first stateful one in the tier). Branch `agent/kyc-tenure/audit-v0_2`. **Integration
  note:** on entry HEAD was detached at `origin/main` (`9b2bfad`, qos-profiles); local `main` ref
  was stale (`f15d2f9`) and was fast-forwarded to `origin/main` before branching.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — qos-profiles v1
  (seventh Phase 5 stable API), drift found & fixed. Audited both operations
  (`retrieveQoSProfiles`, `POST /qos-profiles/v1/retrieve-qos-profiles`; `getQosProfile`,
  `GET /qos-profiles/v1/qos-profiles/{name}`) against `specs/qos-profiles/v1/openapi.yaml`.
  QoS Profiles is the read-only fixed-catalog companion to Quality on Demand: six static
  profiles covering all three `QosProfileStatusEnum` states, narrowed by the `name`/`status`
  filters, with an optional `device` acting only as an error plane (reserved-suffix + the
  two-legged/three-legged `UNNECESSARY_IDENTIFIER` rule). All planes match the handler; the
  `QosProfileName` pattern/length and the `status` enum are genuinely enforced (→ 400
  `INVALID_ARGUMENT`), the `qos-profiles:read` scope gate and `x-correlator` echo are
  correct, and the single-lookup 404s on an unknown name where the list returns `[]`. **Drift
  found — the incomplete-example class (new to the tier):** the `voice` 200 example, shared
  by both operations, carried only 7 of the 15 fields the handler returns for the fully
  deterministic static `voice` profile (it dropped `targetMinUpstreamRate`, `maxUpstreamRate`,
  `minDuration`, `maxDuration`, `jitter`, `packetErrorLossRate`, `l4sQueueType`). Because the
  catalog is reproducible (not counter/clock/wall-clock derived), the documented example must
  reproduce the response byte-for-byte; expanded both the list and single-lookup examples to
  the full profile and added `vendored_voice_example_matches_the_handler`, which drives the
  real router through both the `GET /qos-profiles/voice` lookup and the `{"name":"voice"}`
  filter and pins the body as an exact object (resp. `[object]`). The two 422 inline examples
  already byte-matched the handler (`serviceNotApplicable` = `CamaraError::for_status(422)`;
  `unnecessaryIdentifier` = the handler's inline string), and every other 4xx/5xx `$ref`s the
  shared catalog (no inline examples to drift). `cargo test` 3111 green (+1); `cargo build
  --release` ok, binary 5,455,000 bytes (5.3M, byte-identical to the home-devices-qod pass —
  the YAML example expansion is outside the release path and the new test is
  `#[cfg(test)]`-only). Spec + `#[cfg(test)]` change only — no behaviour/response change (the
  handler was already correct; only the example was abbreviated), no new dependency. Next in
  phase order: kyc-tenure v0.2 (next stateless Phase 5 stable; the stateful carrier-billing
  v0.5 / blockchain-public-address v0.3, which carry a `store.rs`, follow the stateless ones).
  Branch `agent/qos-profiles/audit-v1`. **Integration note:** on entry HEAD was detached at
  `origin/main` (`b3790a0`, home-devices-qod); local `main` ref was stale (`f15d2f9`) and was
  fast-forwarded to `origin/main` before branching.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — home-devices-qod
  v0.4 (sixth Phase 5 stable API), no drift; message-byte pins added. Audited the one
  operation (`setQos`, `PUT /home-devices-qod/v0.4/qos`) against
  `specs/home-devices-qod/v0.4/openapi.yaml`. This API is unusual for the tier: it is
  stateless and has **no phone number / identifier-resolution plane** — the device is
  the subscriber's home LAN device, identified by its internal `ipAddress`, whose
  **last octet** is the control plane (`…240` → 404 `DEVICE_NOT_FOUND`; `…241`–`…247` →
  the `HOME_DEVICES_QOD.*` 409 conflict codes, with `…247`
  `QOS_ALREADY_SET_TO_DEFAULT` gated on `serviceClass=standard` so `serviceClass` is a
  genuine second control plane; `…248` → 503 `ROUTER_OFFLINE`, `…249` → 504 `TIMEOUT`,
  `…250` → 500 `INTERNAL`, `…251`/`…252`/`…253` → the generic `CONFLICT`/`NOT_FOUND`/
  `UNAVAILABLE`; anything else → 204). All planes match the handler; the `serviceClass`
  enum and `ipAddress` IPv4 format are genuinely enforced (→ 400 `INVALID_ARGUMENT`),
  and the `x-correlator` echo and `home-devices-qod:qos:write` scope gate are correct.
  **No spec↔code drift found** — unlike every other Phase 5 stable API audited so far,
  there is no identifier message to truncate (no `resolve_identifier` here): all seven
  inline error examples (400 `invalidArgument`, 404 `deviceNotFound`, 409
  `tooManyDevices`/`qosTooHigh`/`alreadyDefault`, 503 `routerOffline`, 504 `timeout`)
  already byte-match the handler, the generic-code messages (`…250`/`…252`/`…253`)
  equal `CamaraError::for_status`, every status the handler can emit is declared, and
  the module docs describe the single stateless endpoint accurately (no stale-slice-doc
  trap). **Small tested increment:** the existing error tests asserted only
  `status`/`code`, leaving the seven documented `message` strings unguarded; added
  `vendored_error_example_messages_match_the_handler` driving each documented request
  through the real router and pinning the exact `status`/`code`/`message` bytes so they
  can't silently drift. `cargo test` 3110 green (+1); `cargo build --release` ok, binary
  5,455,000 bytes (5.3M, byte-identical to the kyc-fill-in pass — the new test is
  `#[cfg(test)]`-only, outside the release path). `#[cfg(test)]`-only change — no
  spec/behaviour change (the spec was already correct), no new dependency. Next in phase
  order: qos-profiles v1 (next Phase 5 stable; the stateful carrier-billing v0.5 /
  blockchain-public-address v0.3, which carry a `store.rs`, follow the stateless ones).
  Branch `agent/home-devices-qod/audit-v0_4`. **Integration note:** on entry HEAD was
  detached at `origin/main` (`00e5021`, kyc-fill-in); local `main` ref was stale
  (`f15d2f9`) and was fast-forwarded to `origin/main` before branching.
- 2026-10-15 — Phase 5 stable/numbered-tier audit (backlog item 2) — kyc-fill-in v0.3
  (fifth Phase 5 stable API), drift found & fixed. Audited the one operation
  (`KYC_Fill-in`, `POST /kyc-fill-in/v0.3/fill-in`) against
  `specs/kyc-fill-in/v0.3/openapi.yaml`: the endpoint-authorisation gate (any
  `kyc-fill-in:*` scope, else 403 `PERMISSION_DENIED`), the reserved-error-suffix
  plane on the resolved identifier, the persona plane (trailing three digits →
  `PERSONAS[digits % 3]`, so `…000`→Alice/FEMALE, `…001`→Bob/MALE, `…002`→Kenji/OTHER,
  `…003`/`…006` collide with `…000`), the granted-scope response-shaping plane
  (`set-all` → all 20 attributes, else only the per-attribute `kyc-fill-in:<attr>`
  scopes granted), the genuinely-enforced E.164 pattern + `deny_unknown_fields`, and
  the two-legged/three-legged identifier resolution all match the handler. The two
  200 examples are byte-accurate reproducible persona-0 shapes (`setAll` = the full
  Alice persona echoing `+123456789000`; `narrowScopes` = only `name`+`email`), the
  400 `invalidArgument`, 403 `permissionDenied`, 422 `unnecessaryIdentifier`, and 422
  `serviceNotApplicable` inline examples already byte-match, and every other 4xx/5xx
  `$ref`s the shared catalog. **Drift found — the truncated-identifier-message class**
  (device-swap / number-recycling / kyc-age-verification / call-forwarding-signal /
  subscription-status / media-streaming-rate / consent-info / most-frequent-location
  precedent): the 422 `missingIdentifier` example showed the short "The phone number
  cannot be identified." but `resolve_identifier` emits "…: supply `phoneNumber` or
  use a token that identifies a line." Corrected it and added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of all five router-reachable inline error families
  through the real router — 400 `invalidArgument`, 403 `permissionDenied`, 422
  `missingIdentifier` (reachable here: a `client_credentials` `sub` is a non-line
  client id), `unnecessaryIdentifier`, `serviceNotApplicable` — which the existing
  error tests guarded only by `code`. `cargo test` 3109 green (+1); `cargo build
  --release` ok, binary 5,455,000 bytes (5.3M, byte-identical to the device-swap pass
  — the YAML message correction fit within the current allocation page and the new
  test is `#[cfg(test)]`-only, outside the release path). Spec + `#[cfg(test)]` change
  only — no behaviour/response change (the handler was already correct; only the one
  example was truncated), no new dependency. Next in phase order: home-devices-qod
  v0.4 (next Phase 5 stable; carrier-billing / blockchain-public-address, which carry
  a `store.rs`, wait for the stateless APIs). Branch `agent/kyc-fill-in/audit-v0_3`.
  **Integration note:** on entry HEAD was detached at `origin/main` (`8f4738c`,
  device-swap); local `main` ref was stale (`f15d2f9`) and was fast-forwarded to
  `origin/main` before branching.
- 2026-10-14 — Phase 5 stable/numbered-tier audit (backlog item 2) — device-swap v1
  (fourth Phase 5 stable API), drift found & fixed. Audited both operations
  (`checkDeviceSwap`, `POST /device-swap/v1/check`; `retrieveDeviceSwapDate`,
  `POST /device-swap/v1/retrieve-date`) against `specs/device-swap/v1/openapi.yaml`:
  the reserved-error-suffix plane, the swap-recency verdict (identifier trailing three
  digits = hours-since-last-swap, `swapped` iff `< maxAge`, default 240 h — a genuine
  second control plane, exercised both directions by existing tests), the `maxAge`
  `[1,2400]` range plane, the `retrieve-date` monitored-period/`latestDeviceChange`
  companion mapping (same hours-ago, `null` outside 240 h), the genuinely-enforced
  E.164 pattern, and the two-legged/three-legged identifier resolution all match the
  handler; the 200 examples are illustrative scalar/timestamp shapes (`latestDeviceChange`
  is `rfc3339_utc(now − hoursAgo)`, wall-clock dependent) and the 400
  `invalidArgument`/`outOfRange`, 422 `unnecessaryIdentifier`/`serviceNotApplicable`
  inline examples already byte-match, and every other 4xx/5xx `$ref`s the shared catalog.
  **Drift found — the truncated-identifier-message class** (number-recycling /
  kyc-age-verification / call-forwarding-signal / subscription-status / media-streaming-rate
  / consent-info / most-frequent-location precedent): the 422 `missingIdentifier` example
  in **both** operations showed the short "The phone number cannot be identified." but
  `resolve_identifier` emits "…: supply `phoneNumber` or use a token that identifies a
  line." Corrected both and added `vendored_error_example_messages_match_the_handler`
  pinning the exact `status`/`code`/`message` bytes of all router-reachable inline error
  families through the real router — `/check` 400 `invalidArgument`/`outOfRange`, 422
  `missingIdentifier` (reachable: a `client_credentials` `sub` is a non-line client id) /
  `unnecessaryIdentifier`/`serviceNotApplicable`; `/retrieve-date` 400 `invalidArgument`,
  422 `missingIdentifier`/`unnecessaryIdentifier`. `cargo test` 3108 green (+1);
  `cargo build --release` ok, binary 5,455,000 bytes (5.3M, byte-identical to the
  kyc-age-verification pass — the YAML message correction fit within the current allocation
  page and the new test is `#[cfg(test)]`-only, outside the release path). Spec +
  `#[cfg(test)]` change only — no behaviour/response change (the handler was already
  correct; only the two examples were truncated), no new dependency. Next in phase order:
  kyc-fill-in v0.3 (next stateless Phase 5 stable; carrier-billing /
  blockchain-public-address, which carry a `store.rs`, wait for the stateless APIs).
  Branch `agent/device-swap/audit-v1`. **Integration note:** on entry HEAD was detached
  at `origin/main` (`2b343ab`, kyc-age-verification); local `main` ref was stale
  (`f15d2f9`) and was fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 5 stable/numbered-tier audit (backlog item 2) — kyc-age-verification
  v0.1 (third Phase 5 stable API), drift found & fixed. Audited the one operation
  (`verifyAge`, `POST /kyc-age-verification/v0.1/verify`) against
  `specs/kyc-age-verification/v0.1/openapi.yaml`: the reserved-error-suffix plane, the age
  verdict (trailing three digits = held age `d % 100`, `ageCheck:"true"` iff ≥
  `ageThreshold` — a genuine second control plane, `…000` → `"not_available"`), the
  optional-field planes (`identityMatchScore` fixed 90 on any identity attribute,
  `verifiedStatus` on `idDocument`, opt-in `contentLock`/`parentalControl` minor signals),
  the genuinely-enforced E.164 pattern + `ageThreshold` 0..=120 range, and the
  two-legged/three-legged identifier resolution all match the handler; the four 200
  success examples are reproducible scalar shapes already pinned, the 400
  `invalidArgument`/`outOfRange` and 422 `unnecessaryIdentifier`/`serviceNotApplicable`
  inline examples already byte-match, and every other 4xx/5xx `$ref`s the shared catalog.
  **Drift found — the truncated-identifier-message class** (number-recycling /
  call-forwarding-signal / subscription-status / media-streaming-rate / consent-info /
  most-frequent-location precedent): the 422 `missingIdentifier` example showed the short
  "The phone number cannot be identified." but `resolve_identifier` emits "…: supply
  `phoneNumber` or use a token that identifies a line." Corrected it and added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of all five router-reachable inline error families (400
  `invalidArgument`/`outOfRange`; 422 `missingIdentifier` — reachable, a
  `client_credentials` `sub` is a non-line client id; `unnecessaryIdentifier`;
  `serviceNotApplicable`) through the real router. `cargo test` 3107 green (+1);
  `cargo build --release` ok, binary 5,455,000 bytes (5.3M, byte-identical to the
  number-recycling pass — the YAML message correction fit within the current allocation
  page and the new test is `#[cfg(test)]`-only, outside the release path). Spec +
  `#[cfg(test)]` change only — no behaviour/response change (the handler was already
  correct; only the one example was truncated), no new dependency. Next in phase order:
  device-swap v1 (next stateless Phase 5 stable). Branch
  `agent/kyc-age-verification/audit-v0_1`. **Integration note:** on entry HEAD was detached
  at `origin/main` (`7aadf29`, number-recycling); local `main` ref was stale (`f15d2f9`)
  and was fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 5 stable/numbered-tier audit (backlog item 2) — number-recycling
  v0.2 (second Phase 5 stable API), drift found & fixed. Audited the one operation
  (`checkNumberRecycling`, `POST /number-recycling/v0.2/check`) against
  `specs/number-recycling/v0.2/openapi.yaml`: the reserved-error-suffix plane, the
  recycling verdict (trailing-three-digits = days-since-last-subscriber-change, recycled
  iff strictly after `specifiedDate` — a genuine second control plane), the
  `specifiedDate` validation (malformed → 400 `INVALID_ARGUMENT`, future → 400
  `OUT_OF_RANGE`), the genuinely-enforced E.164 pattern, and the two-legged/three-legged
  identifier resolution all match the handler; the 200 shape + the 400
  `invalidArgument`/`outOfRange` and 422 `unnecessaryIdentifier`/`serviceNotApplicable`
  inline examples already byte-match, and every other 4xx/5xx `$ref`s the shared catalog.
  **Drift found — the truncated-identifier-message class** (call-forwarding-signal /
  subscription-status / media-streaming-rate / consent-info / most-frequent-location
  precedent): the 422 `missingIdentifier` example showed the short "The phone number
  cannot be identified." but `resolve_identifier` emits "…: supply `phoneNumber` or use a
  token that identifies a line." Corrected it and added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of all five router-reachable inline error families (400
  `invalidArgument`/`outOfRange`; 422 `missingIdentifier` — reachable, a
  `client_credentials` `sub` is a non-line client id; `unnecessaryIdentifier`;
  `serviceNotApplicable`) through the real router. `cargo test` 3106 green (+1);
  `cargo build --release` ok, binary 5,455,000 bytes (5.3M, byte-identical to the
  call-forwarding-signal pass — the YAML message correction isn't compiled in and the new
  test is `#[cfg(test)]`-only, outside the release path). Spec + `#[cfg(test)]` change
  only — no behaviour/response change (the handler was already correct; only the one
  example was truncated), no new dependency. Next in phase order: kyc-age-verification
  v0.1 (next stateless Phase 5 stable). Branch `agent/number-recycling/audit-v0_2`.
  **Integration note:** on entry HEAD was detached at `origin/main` (`67dd79f`,
  call-forwarding-signal); local `main` ref was stale (`f15d2f9`) and was
  fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 5 **stable/numbered**-tier audit (backlog item 2) begins —
  call-forwarding-signal v0.4, drift found & fixed. The Phase 1–4 stable audit and
  the vwip sweep are complete, but the ~18 Phase 5 stable/numbered APIs (inventory)
  were never audited by either; moved the per-API spec↔code audit into that tier in
  phase order, stateless-first (so carrier-billing/blockchain-public-address, which
  carry a `store.rs`, wait for the stateless APIs). Audited both operations
  (`retrieveUnconditionalCallForwarding`, `retrieveCallForwarding`) against
  `specs/call-forwarding-signal/v0.4/openapi.yaml`: the reserved-error-suffix plane,
  the odd/even `active` plane + the `digits % 16` 4-bit forwarding-type mask (zero →
  `["inactive"]`), the genuinely-enforced E.164 pattern, and the
  two-legged/three-legged identifier resolution all match the handler; the 200
  examples (incl. `…012` → `["conditional_not_reachable","conditional_no_answer"]`)
  are byte-accurate and already pinned, and every 4xx/5xx `$ref`s the shared catalog
  except the inline 422s. **Drift found — the truncated-identifier-message class**
  (subscription-status / media-streaming-rate / consent-info / most-frequent-location
  precedent): the 422 `missingIdentifier` example in **both** operations showed the
  short "The phone number cannot be identified." but `resolve_identifier` emits
  "…: supply `phoneNumber` or use a token that identifies a line." Corrected both and
  added `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of all three router-reachable inline 422 families
  (`missingIdentifier` — reachable here, a `client_credentials` `sub` is a non-line
  client id; `unnecessaryIdentifier`; `serviceNotApplicable`) through the real router.
  `cargo test` 3105 green (+1); `cargo build --release` ok, binary 5,455,000 bytes
  (5.2M, byte-identical to the geofencing-subscriptions pass — the message correction
  fit within the current allocation page; the new test is `#[cfg(test)]`-only, outside
  the release path). Spec + `#[cfg(test)]` change only — no behaviour/response change
  (the handler was already correct; only the two examples were truncated), no new
  dependency. Next in phase order: number-recycling v0.2 (Phase 5 stable). Branch
  `agent/call-forwarding-signal/audit-v0_4`. **Integration note:** on entry HEAD was
  detached at `origin/main` (`865f02c`, geofencing-subscriptions); local `main` ref
  was stale (`f15d2f9`) and was fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 4 stable-API audit (backlog item 2) — geofencing-subscriptions
  v0.4 (third and last Phase 4 API; CamaraSim's first event-subscription API), drift
  found & fixed. Completed the Phase 4 (spatial) tier in phase order. Audited all four
  CRUD operations (`createSubscription`, `retrieveSubscriptionList`,
  `retrieveSubscription`, `deleteSubscription`) against
  `specs/geofencing-subscriptions/v0.4/openapi.yaml`, the in-memory `store.rs`, and
  `notifications.rs`: the create identifier plane (reserved-error-suffix → canonical
  CAMARA error; `…000`/no-digits → ACTIVATION_REQUESTED else ACTIVE; `…001`/`…002`
  movement + even/odd initial-event position markers), the circle `radius` area plane
  (`< 2000`/`> 200000` m or center out of range → 400 OUT_OF_RANGE; non-CIRCLE/missing
  area → 400 INVALID_ARGUMENT), the genuinely-enforced `phoneNumber` E.164 pattern, and
  the two-legged/three-legged identifier resolution all match the handler, and both
  inline 400 example messages + the 422 `missingIdentifier` example byte-match.
  **Defect found — the illustrative-timestamp + minted-id class** (quality-on-demand
  `sessionId`/`startedAt` / location-verification `lastLocationTime` precedent): the
  `active` 201 success example showed `startsAt` as a frozen literal
  (`2026-08-03T14:27:08Z`), but the handler stamps `rfc3339_utc(now_unix_secs())`
  (wall-clock dependent) and `id` is minted (counter+clock, not request-derived), so
  neither can reproduce. Marked `id`/`startsAt` illustrative with a derivation note
  (their presence is the contract; the echoed `protocol`/`sink`/`types`/`config` +
  `status` are reproducible). Added `vendored_success_example_matches_the_handler`
  (drives the example's exact request through the router, pins every reproducible field
  as an exact object minus the illustrative `id`/`startsAt`, asserting `startsAt` is UTC
  but ≠ the frozen literal) and `vendored_error_example_messages_match_the_handler`
  (pins the two router-reachable inline 400 families' exact status/code/message bytes;
  the 422 `missingIdentifier` example is documented not router-reachable — empty-`sub`
  token the endpoint never issues, the quality-on-demand / location-verification
  precedent). Fixed the stale "First slice: POST /subscriptions + GET …/{subscriptionId}"
  parent module doc (`src/apis/geofencing_subscriptions.rs`) though all four CRUD legs +
  the notification machinery ship (the stale-slice-doc trap). `cargo test` 3104 green
  (+2); `cargo build --release` ok, binary 5,455,000 bytes (5.2M, byte-identical to the
  location-retrieval pass — the ~0.7 KB illustrative-example note fit within the current
  allocation page; the new tests are `#[cfg(test)]`-only, outside the release path).
  Spec + doc + `#[cfg(test)]` change only — no behaviour/response change (the handler
  was already correct; only the one illustrative example lacked its caveat), no new
  dependency. **This completes the Phase 4 spatial tier, and with it backlog item 2's
  stable-API audit across every ✅ API in Phases 1–4.** Next in phase order: the vwip
  deepening (item 3) remains human-gated (see the decision point); the next idle pass
  should confirm nothing changed and stop without committing unless a human names work.
  Branch `agent/geofencing-subscriptions/audit-v0_4`. **Integration note:** on entry
  HEAD was detached at `origin/main` (`58b4ca0`, location-retrieval); local `main` ref
  was stale (`f15d2f9`) and was fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 4 stable-API audit (backlog item 2) — location-retrieval v0.4
  (second Phase 4 API), drift found & fixed. Continued the per-API spec↔code audit
  in phase order. Audited the one operation (`retrieveLocation`,
  `POST /location-retrieval/v0.4/retrieve`) against
  `specs/location-retrieval/v0.4/openapi.yaml`: the identifier
  trailing-three-digits location plane (reserved-error-suffix → canonical CAMARA
  error; else a CIRCLE `area` with `center` = base point `(51.5, -0.12)` offset by
  `digits × 0.001°` and `radius` = `((digits % 10) + 1) × 100` m), the `maxAge`
  range plane (`< 60`/`> 2147483647` → 400 `OUT_OF_RANGE`, always-fresh), the
  genuinely-enforced `phoneNumber` E.164 pattern + `networkAccessIdentifier`
  rejection, the two-legged/three-legged identifier resolution, and the
  reserved-suffix set all match the handler, and both 400 inline error-example
  messages byte-match. The happy-path `center`/`radius` for `+123456789012` are
  byte-accurate (verified the f64 arithmetic is exact: `51.5 + 12×0.001 == 51.512`,
  `-0.12 + 12×0.001 == -0.108`, radius `300`), so no deterministic-value drift.
  **Defect found — the illustrative-timestamp class** (location-verification
  `lastLocationTime` / sim-swap `latestSimChange` / device-identifier `lastChecked`
  / quality-on-demand `startedAt` precedent): the one 200 success example
  (`located`) showed `lastLocationTime` as a frozen literal (`2026-08-03T14:27:08Z`),
  but the handler stamps `rfc3339_utc(now_unix_secs())` (wall-clock dependent) so it
  can't reproduce. Marked `lastLocationTime` illustrative with a derivation note (its
  presence as an RFC-3339 UTC instant is the contract; `center`/`radius` are
  reproducible) and bound the example to its concrete input `+123456789012`. Added
  `vendored_success_example_matches_the_handler` (drives `+123456789012` through the
  router, pins the `area` body as an exact object minus the illustrative
  `lastLocationTime`, asserting it is UTC but ≠ the frozen literal) and
  `vendored_error_example_messages_match_the_handler` (pins the exact
  `status`/`code`/`message` bytes of the two router-reachable inline 400 families —
  `invalidArgument`/`outOfRange`; the 422 `missingIdentifier` example is documented
  not router-reachable — empty-`sub` token the endpoint never issues, the
  location-verification / quality-on-demand precedent). `cargo test` 3102 green (+2);
  `cargo build --release` ok, binary 5,455,000 bytes (5.2M, up 4,096 bytes / one page
  from the location-verification pass's 5,450,904 — the embedded vendored spec grew
  with the ~1.5 KB illustrative example note; the new tests are `#[cfg(test)]`-only,
  outside the release path). Spec + `#[cfg(test)]` change only — no behaviour/response
  change (the handler was already correct; only the one illustrative timestamp lacked
  its caveat), no new dependency. Next in phase order: geofencing-subscriptions v0.4
  (Phase 4, the last Phase 4 API). Branch `agent/location-retrieval/audit-v0_4`.
  **Integration note:** on entry HEAD was detached at `origin/main` (`9f64f3a`,
  location-verification); local `main` ref was stale (`f15d2f9`) and was
  fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 4 stable-API audit (backlog item 2) begins —
  location-verification v3 (first Phase 4 API, CamaraSim's first **spatial** API),
  drift found & fixed. With the Phase 3 stable tier complete, moved the per-API
  spec↔code audit into Phase 4 (spatial) in phase order. Audited the one operation
  (`verifyLocation`, `POST /location-verification/v3/verify`) against
  `specs/location-verification/v3/openapi.yaml`: the identifier
  trailing-three-digits verdict plane (reserved-error-suffix → canonical CAMARA
  error; `…000` → `FALSE`; odd → `PARTIAL` with `matchRate = (digits % 99) + 1`;
  else `TRUE`), the circle `radius` area plane (`< 2000 m` regulatory minimum →
  422 `LOCATION_VERIFICATION.INVALID_AREA`; `< 1 m` / center out of range → 400
  `OUT_OF_RANGE`; non-CIRCLE `areaType` → 400 `INVALID_ARGUMENT`), the
  genuinely-enforced `phoneNumber` E.164 pattern + `maxAge` range, the
  two-legged/three-legged identifier resolution, and the reserved-suffix set all
  match the handler, and all four inline error-example messages byte-match.
  **Defect found — the illustrative-timestamp class** (sim-swap `latestSimChange` /
  device-identifier `lastChecked` / quality-on-demand `startedAt` precedent): all
  three 200 success examples (`inside`/`partly`/`outside`) showed
  `lastLocationTime` as a frozen literal (`2026-08-03T14:27:08Z`), but the handler
  stamps `rfc3339_utc(now_unix_secs())` (wall-clock dependent) so it can't
  reproduce. Marked `lastLocationTime` illustrative on all three examples with a
  derivation note (its presence as an RFC-3339 UTC instant is the contract; the
  `verificationResult`/`matchRate`/echoed `device` are reproducible) and clarified
  the `partly`/`outside` summaries as subject-driven (no `device` echoed). Added
  `vendored_success_examples_match_the_handler` (drives `inside` via a device …012,
  `partly` via a subject …011, `outside` via a subject …000 through the router,
  pins the reproducible fields as exact objects minus the illustrative timestamp,
  asserting it is UTC but ≠ the frozen literal) and
  `vendored_error_example_messages_match_the_handler` (pins the exact
  `status`/`code`/`message` bytes of the three router-reachable inline error
  families — 400 `invalidArgument`/`outOfRange`, 422 `invalidArea`; the 422
  `missingIdentifier` example is documented not router-reachable — empty-`sub`
  token the endpoint never issues, the quality-on-demand / session-insights
  precedent). `cargo test` 3100 green (+2); `cargo build --release` ok, binary
  5,450,904 bytes (5.2M, byte-identical to the quality-on-demand pass — the ~1.5 KB
  of added example descriptions fit within the current allocation page; the new
  tests are `#[cfg(test)]`-only, outside the release path). Spec + `#[cfg(test)]`
  change only — no behaviour/response change (the handler was already correct;
  only the three illustrative timestamps lacked their caveat), no new dependency.
  Next in phase order: location-retrieval v0.4 (Phase 4). Branch
  `agent/location-verification/audit-v3`. **Integration note:** on entry HEAD was
  detached at `origin/main` (`40e5982`, quality-on-demand); local `main` ref was
  stale (`f15d2f9`) and was fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 3 stable-API audit (backlog item 2) — quality-on-demand v1
  (second Phase 3 API; the session-lifecycle + CloudEvents-notifications API),
  drift found & fixed. Audited all five operations (`createSession`, `getSession`,
  `deleteSession`, `extendQosSession`, `retrieveSessionsByDevice`) against
  `specs/quality-on-demand/v1/openapi.yaml`, the in-memory `store.rs`, and
  `notifications.rs`. All inline error-example messages byte-match the handler
  (create 400 invalidArgument/durationOutOfRange, 422 qosProfileNotApplicable;
  extend 400 invalidArgument/durationOutOfRange). **Defect found — the
  illustrative-timestamp class** (sim-swap `latestSimChange` / device-identifier
  `lastChecked` precedent): the createSession `available` and retrieve-sessions
  `some` 200 examples showed `startedAt`/`expiresAt` as frozen literals, but
  `build_session_info` stamps `now` / `now + duration` (wall-clock dependent) so
  they can't reproduce; `sessionId` is likewise minted (counter+clock, the
  application-profiles posture). Marked `sessionId`/`startedAt`/`expiresAt`
  illustrative on all three success-example families with derivation notes (the
  reproducible echoed fields stay contract). Added
  `vendored_success_examples_match_the_handler` (pins the reproducible fields as
  exact objects through the router + the `expiresAt == startedAt + duration`
  relation, asserting the illustrative values ≠ the frozen literals) and
  `vendored_error_example_messages_match_the_handler` (pins the six
  router-reachable inline error families' exact status/code/message bytes; the 422
  `missingIdentifier` example is documented not router-reachable — empty-`sub`
  token the endpoint never issues, the session-insights precedent). Fixed the stale
  "First slice: POST /sessions + GET /sessions/{id}" parent module doc and two
  stale `store.rs` "later passes — DELETE / extend / notifications" doc lines
  though all five legs + notifications ship (the stale-slice-doc trap). `cargo test`
  3098 green (+2); `cargo build --release` ok, binary 5,450,904 bytes (5.2M,
  byte-identical to the one-time-password-sms pass — the ~1.5 KB of embedded
  example descriptions fit within the current allocation page; the new tests are
  `#[cfg(test)]`-only). Spec + doc + `#[cfg(test)]` change only — no
  behaviour/response change (the handlers were already correct), no new dependency.
  **This completes the Phase 3 stable tier;** next in phase order is Phase 4
  (spatial): location-verification v3. Branch `agent/quality-on-demand/audit-v1`.
  **Integration note:** on entry HEAD was detached at `origin/main` (`34bd37c`,
  one-time-password-sms); local `main` ref was stale (`f15d2f9`) and was
  fast-forwarded to `origin/main` before branching.
- 2026-10-14 — Phase 3 stable-API audit (backlog item 2) begins —
  one-time-password-sms v1 (first Phase 3 API, CamaraSim's first stateful API),
  verified clean. With the Phase 2 tier complete, moved the per-API spec↔code
  audit into Phase 3 (stateful, non-spatial) in phase order. Audited both
  operations (`sendCode` `POST /one-time-password-sms/v1/send-code`, `validateCode`
  `POST /one-time-password-sms/v1/validate-code`) against
  `specs/one-time-password-sms/v1/openapi.yaml` and the in-memory `store.rs`: the
  `send-code` phoneNumber control plane (reserved-error-suffix → canonical CAMARA
  error; else an OTP whose code is the zero-padded last six digits, returning the
  `authenticationId`), the genuinely-enforced E.164 pattern + `message`
  maxLength-160/`{{code}}`-placeholder validation, and the `validate-code` live-state
  plane (match → 204 single-use; wrong code → `INVALID_OTP` until the 3-attempt
  budget → `VERIFICATION_FAILED`; unknown/consumed/expired id → `VERIFICATION_EXPIRED`,
  all HTTP 400) all match the handler. **No drift found** — all four inline
  `validate-code` 400 example messages already byte-match the handler, and
  `send-code`'s 200 `authenticationId` example is a counter/clock-derived opaque
  token (base64url of SHA-256(counter‖now)), so it stays legitimately illustrative
  (the application-profiles posture), not the deterministic-value drift class.
  **Small tested increment:** existing router tests guarded those four 400 examples
  only by `code`, so added `vendored_error_example_messages_match_the_handler`
  pinning the exact `status`/`code`/`message` bytes of all four
  (`invalidArgument`/`invalidOtp`/`verificationFailed`/`verificationExpired`)
  through the real router (the device-authenticity / network-health-assessment
  message-pin). `cargo test` 3096 green (+1); `cargo build --release` ok, binary
  5,450,904 bytes (5.2M, byte-identical to the device-identifier pass — the new
  test is `#[cfg(test)]`-only, outside the release path). Next in phase order:
  quality-on-demand v1 (Phase 3). Branch
  `agent/one-time-password-sms/pin-error-examples`. **Integration note:** on entry
  HEAD was detached at `origin/main` (`0dc8fa4`, device-identifier); local `main`
  ref was stale (`f15d2f9`) and was fast-forwarded to `origin/main` before
  branching.
- 2026-10-14 — Phase 2 stable-API audit (backlog item 2) — device-identifier v0.3
  (third and last Phase 2 API) — drift found & fixed. Continued the per-API
  spec↔code audit in phase order. Audited all three operations (`retrieveType`,
  `retrieveIdentifier`, `retrievePPID`) against
  `specs/device-identifier/v0.3/openapi.yaml`: the identifier reserved-error-suffix
  plane, the trailing-three-digits `DEVICE_TYPES[digits % 6]` type plane (…000 / no
  digits → the default Apple entry), the synthesised IMEI (TAC + zero-padded serial
  + Luhn check digit, verified by hand: 35847104000001 → check digit 1) / IMEISV
  (+ "00" SVN), the PPID = SHA-256(identifier) UUID-shaped, and the
  two-legged/three-legged identifier resolution all match the handler; every error
  `$ref`s the shared catalog (no inline error examples). **Drift found — the
  deterministic-value / placeholder class** (verified-caller / click-to-dial /
  network-access-domains precedent): the `retrieve-ppid` 200 `ppid` example, bound
  to `phoneNumber "+123456789001"`, carried a placeholder
  `b1d5781d-1141-df9d-6d76-2f8e70edb19c`, but `ppid_from("+123456789001")` =
  `5dacd245-aaf6-fa2e-ce77-e48eaabc4064` (verified: `sha256sum` of the identifier,
  first 16 bytes). Corrected it and bound the example to its derivation. Also marked
  all three examples' frozen `lastChecked` **illustrative** (the handler stamps the
  current time — the sim-swap `latestSimChange` convention) with a derivation note
  on each. Added `vendored_success_examples_match_the_handler` driving all three
  documented requests through the real router and pinning each body as an exact
  object (minus the illustrative `lastChecked`, asserted present/UTC but ≠ the
  frozen literal); the ppid is pinned both as the corrected literal and against
  `ppid_from`. The `type`/`identifier` example values were already byte-accurate.
  `cargo test` 3095 green (+1); `cargo build --release` ok, binary 5,450,904 bytes
  (5.2M, up one 4,096-byte page from the device-roaming-status pass's 5,446,808 —
  the embedded vendored spec grew with the corrected ppid value + the three
  illustrative example notes; the new test is `#[cfg(test)]`-only, outside the
  release path). Spec + `#[cfg(test)]` change only — no behaviour/response change
  (the handler was already correct; only the one example value was a placeholder),
  no new dependency. **This completes the Phase 2 stable tier;** next in phase order
  is Phase 3 (one-time-password-sms v1). Branch
  `agent/device-identifier/pin-success-examples`. **Integration note:** on entry
  HEAD was detached at `origin/main` (`0066dd8`, device-roaming-status); local
  `main` ref was stale (`f15d2f9`) and was fast-forwarded to `origin/main` before
  branching.
- 2026-10-14 — Phase 2 stable-API audit (backlog item 2) — device-roaming-status
  v1 (second Phase 2 API), verified clean. Continued the per-API spec↔code audit in
  phase order. Audited the one operation (`getRoamingStatus`,
  `POST /device-roaming-status/v1/retrieve`) against
  `specs/device-roaming-status/v1/openapi.yaml`: the identifier
  reserved-error-suffix plane, the trailing-three-digits roaming plane (`…000`/
  no-digits → not roaming; else roaming with visited country `COUNTRIES[digits % 6]`
  over `[(262,DE),(234,GB),(208,FR),(310,US),(440,JP),(505,AU)]`), the
  genuinely-enforced `phoneNumber` E.164 pattern, the request/response schemas, the
  reserved-suffix set (= `scenarios::RESERVED_ERROR_SUFFIXES`, 409/500 noted as
  extensions), the deliberately-omitted optional `lastStatusTime`, and the module
  docs all match the handler. **No drift found** — the two 200 examples (`roaming`
  → `262`/`["DE"]`, `home` → `{roaming:false}`) are reproducible scalar shapes and
  every error `$ref`s the shared catalog (no inline examples to drift), the
  device-reachability-status / number-verification posture (not a
  documented-divergence like sim-swap's illustrative timestamp or kyc-match's
  unenforced gender enum). **Small tested increment:** existing router tests pinned
  the 200 examples only field-by-field, so added
  `vendored_success_examples_match_the_handler` pinning both documented example
  bodies (`roaming` via a …012 tail → 262/["DE"], `home` via a …000 tail) as exact
  objects through the real router (the device-reachability-status whole-object pin).
  `cargo test` 3094 green (+1); `cargo build --release` ok, binary 5,446,808 bytes
  (5.2M, byte-identical to the device-reachability-status pass — `#[cfg(test)]`-only
  change, outside the release path). Next in phase order: device-identifier v0.3.
  Branch `agent/device-roaming-status/pin-success-examples`. **Integration note:**
  on entry HEAD was detached at `origin/main` (`364aa24`,
  device-reachability-status); local `main` ref was stale (`f15d2f9`) and was
  updated to `origin/main` before branching.
- 2026-10-14 — Phase 2 stable-API audit (backlog item 2) begins —
  device-reachability-status v1, verified clean. With Phase 1 complete, moved the
  per-API spec↔code audit into Phase 2 (stateless device queries) in phase order.
  Audited the one operation (`getReachabilityStatus`,
  `POST /device-reachability-status/v1/retrieve`) against
  `specs/device-reachability-status/v1/openapi.yaml`: the identifier
  reserved-error-suffix plane, the trailing-three-digits reachability plane (`…000`
  → not reachable / odd → SMS only / else data+SMS), the genuinely-enforced
  `phoneNumber` E.164 pattern, the request/response schemas, the reserved-suffix set
  (= `scenarios::RESERVED_ERROR_SUFFIXES`, 409/500 noted as extensions), and the
  module docs all match the handler. **No drift found** — the three 200 examples are
  reproducible scalar shapes and every error `$ref`s the shared catalog (no inline
  examples to drift), the number-verification posture, **not** a documented-divergence
  like sim-swap's illustrative timestamp or kyc-match's unenforced gender enum.
  **Small tested increment:** existing router tests pinned the 200 examples only
  field-by-field, so added `vendored_success_examples_match_the_handler` pinning all
  three documented example bodies (`reachableDataSms`/`reachableSmsOnly`/
  `notReachable`) as exact objects through the real router (the kyc-match whole-object
  pin). `cargo test` 3093 green (+1); `cargo build --release` ok, binary 5,446,808
  bytes (5.2M, byte-identical to the kyc-match pass — `#[cfg(test)]`-only change,
  outside the release path). Next in phase order: device-roaming-status v1. Branch
  `agent/device-reachability-status/pin-success-examples`. **Integration note:** on
  entry HEAD was detached at `origin/main` (`5d76bf0`, kyc-match); local `main` ref
  was stale (`f15d2f9`) and was updated to `origin/main` before branching.
- 2026-10-14 — Phase 1 stable-API audit (backlog item 2) — kyc-match v0.3
  (third and last Phase 1 API). Audited the one operation (`matchCustomer`,
  `POST /kyc-match/v0.3/match`) against `specs/kyc-match/v0.3/openapi.yaml`:
  the reserved-error-suffix + per-attribute value-marker planes, the 20-attribute
  request schema, the 13 score-bearing response fields, and the 200/400 examples
  all matched the handler. **One spec↔code inconsistency found & fixed — the
  documented-divergence / unenforced-constraint class** (device-authenticity's
  schema-enum precedent): the `gender` request property carried
  `enum: [MALE, FEMALE, OTHER]`, implying rejection of other values, but the
  handler never enforces it — `gender` is classified by the uniform value-marker
  plane like every other attribute, so out-of-enum values (and the
  `nomatch`/`unavailable` markers) are accepted and drive the verdict. Kept the
  enum (faithful to canonical CAMARA 0.3.0) and added a property note documenting
  that CamaraSim does not enforce it ("documented divergence, not drift"),
  converting undocumented drift into an explicit divergence. Added two tests:
  `gender_is_classified_by_value_not_enforced_against_the_enum` (pins the
  value-marker behaviour + out-of-enum acceptance) and
  `vendored_examples_match_the_handler` (pins the three 200 example bodies and the
  two 400 `status`/`code`/`message` byte strings through the router, previously
  guarded only by `code`). Spec + `#[cfg(test)]` change only — no behaviour/response
  change, no new dependency. `cargo test` 3092 green (+2); `cargo build --release`
  ok, binary 5,446,808 bytes (5.2M, unchanged). This completes the Phase 1 stable
  tier; next in phase order is Phase 2 (device-reachability-status v1). Branch
  `agent/kyc-match/gender-enum-divergence`.
- 2026-10-14 — Phase 1 stable-API audit (backlog item 2) begins — sim-swap v2
  drift found & fixed. With the vwip sweep (item 3) complete, extended the per-API
  spec↔code audit into the ✅ **stable** tier, in phase order. **number-verification
  v1:** verified clean — reproducible scalar success examples (`true`/`false`/
  `+123456789012`), all errors `$ref` the shared catalog, already pinned by existing
  router tests; nothing to change. **sim-swap v2:** found the `/retrieve-date`
  `recentSwap` 200 example showing a frozen `latestSimChange` (`2024-01-01T14:27:08Z`)
  though `last_swap_timestamp` returns *now* − hoursAgo — the wall-clock /
  illustrative-value drift class (verified-caller's `expiresAt`). Marked the example
  illustrative (its presence + `monitoredPeriod` are the contract) and added
  `vendored_success_examples_match_the_handler` pinning the four reproducible
  success-example parts through the router (both `/check` booleans; `/retrieve-date`
  `monitoredPeriod` literal 10, the non-null recent case, and the `null` case),
  asserting the illustrative timestamp is not a returned literal. Spec +
  `#[cfg(test)]` change only — no behaviour/response change (the handler was already
  correct; only the one example lacked its illustrative caveat), no new dependency.
  `cargo test` 3090 green (+1); `cargo build --release` ok, binary 5,446,808 bytes
  (5.2M, unchanged). This confirms backlog item 2 is real work (not a contract-test
  treadmill): the deterministic-value drift the vwip sweep caught also hides in
  stable-API examples. Branch `agent/sim-swap/retrieve-date-illustrative-example`.
  Next in phase order: kyc-match v0.3.
- 2026-10-13 — Phase 5 vwip verification pass (network-access-domains) — **the
  last unaudited vwip API; the per-API vwip sweep (backlog item 3) is now
  complete.** Audited the thirty-first and final vwip API in mount order (the
  NetworkAccessManagement Trust Domain surface). All **thirteen** modelled
  operations — `getTrustDomainCapabilities`; `getServices`/`getService`; the Trust
  Domain CRUD (`createTrustDomain`/`getTrustDomains`/`getTrustDomain`/
  `updateTrustDomain`/`deleteTrustDomain`); and the Trust Domain Device CRUD
  (`createTrustDomainDevice`/`getTrustDomainDevices`/`getTrustDomainDevice`/
  `updateTrustDomainDevice`/`deleteTrustDomainDevice`) — are implemented
  (`src/apis/network_access_domains/vwip.rs` + an in-memory `store.rs`), routed, and
  well-tested (~90 test fns before this pass). Verified the control planes match the
  handler: the token-subject reserved-suffix plane (createTrustDomain/
  createTrustDomainDevice/getServices/getService), request validation, parent Trust
  Domain existence (device legs), the store-state duplicate-409 keyed on the
  deterministic `(serviceId, name)` / `(trustDomainId, deviceName)` v5-UUID, the
  advertised-access-type gate, and the write-only WPA-password / deviceCredential
  stripping. **Spec↔code drift found and fixed — the deterministic-id / placeholder
  class, covering this spec's ENTIRE success-example set** (the same class caught on
  verified-caller / click-to-dial / network-access-devices /
  edge-application-management / esim-remote-management): every 200/201 example carried
  Swagger placeholder ids and none was bound to a concrete input. Corrected the four
  Trust Domain examples' `id` (`b1e0f6a2-…` → `trust_domain_id("3fa85f64-…","Primary
  WiFi Network")` = `950925b2-55e7-5330-8f22-93fef8f82784`), the four device examples'
  `id` (`c2f1a7b3-…`/`d3a2b8c4-…` → the real `trust_domain_device_id` values
  `0cc7f47a-…` Alice's Laptop / `542bfe03-…` Alice's Phone), and the
  getServices/getService examples' service+site ids and geo points (placeholder
  `3fa85f64-…`/`123e4567-…` + Bonn/Zurich coords → the SHA-256-derived values for the
  now-stated token subject `nad-005`: `d161db25-…`/`8daaa955-…` lat 67.55459/long
  −110.31266 and `fe6a3242-…`/`eefceeba-…` lat 70.88075/long −115.6466). Bound each
  example to its concrete input (keeping the audit stamps `createdAt`/`modifiedAt` +
  subject-derived `createdBy`/`modifiedBy` explicitly illustrative, the
  network-access-devices posture) and added `vendored_success_examples_match_the_handler`,
  which drives createTrustDomain + getTrustDomain, createTrustDomainDevice, and
  getServices + getService through the real router and pins
  `trust_domain_id`/`trust_domain_device_id` directly so the examples can't drift back
  to placeholders. No inline error examples exist (every 4xx/5xx `$ref`s the shared
  catalog, as on dedicated-network-accesses), so none to pin. **Two stale "later
  slices" module docs fixed** (the stale-slice-doc trap caught on device-data-volume /
  qos-booking / dedicated-network-* / esim-remote-management): the parent
  `network_access_domains.rs` doc and the `vwip.rs` header both presented the Trust
  Domain read/update/delete legs and the whole device sub-resource as future work (the
  header listed only 5 of 13 operations) though all thirteen ship — corrected both to
  the complete surface. `cargo test` green (3088 → 3089 tests) and `cargo build
  --release` green; release binary 5,446,808 bytes (byte-identical to the prior esim
  pass — the spec example edits were size-neutral within the same page; the module-doc
  edits are not embedded in the binary). Spec + doc + `#[cfg(test)]` change only (no
  behaviour/response change — the handlers were already correct; only the success
  examples carried placeholders); no new dependency. *Backlog banner still stands:* the
  net-new feature backlog remains exhausted and the human decision (authorise stateful
  infra, name an increment, or pause the routine) is still open. With the vwip sweep
  now complete, future idle passes have no remaining per-API verification target — an
  idle pass should confirm nothing changed and **stop without committing** per the
  decision-point banner. **Integration note:** on entry the local checkout == `origin/main`
  at `b9936f1` (esim-remote-management); this pass branches off it, rebases on main, and
  pushes one commit on top.
- 2026-10-13 — Phase 5 vwip verification pass (esim-remote-management). Audited
  the thirtieth vwip API in mount order (the CMP-envelope eSIM profile-management
  surface). All four modelled operations — the stateless read legs `profileList`
  (`POST /profile/downloaded-list`) and `profileResultQuery`
  (`POST /profile/result/query`), and the two command legs modelled as synchronous
  acknowledgements `profileOperation` (`POST /profile/oper`) and `profileDownload`
  (`POST /profile/download`) — are implemented
  (`src/apis/esim_remote_management/vwip.rs`), routed, and well-tested (66 test fns
  before this pass). Verified the control planes match the handler: the device
  `eId` / `taskId` trailing-three-decimal-digit plane (reserved-suffix → canonical
  CAMARA error; `profileList` `d % 4` profile count with only the first enabled;
  `profileResultQuery` `d % 3` `operResult`; the command legs validate-and-echo),
  the envelope validation (`sequenceNum`/`eId`/`taskId` patterns, `optType` 1..=3
  and `autoEnableType` ==1 → `OUT_OF_RANGE`, `protocol`/`sink`/`types`/
  `subscriptionMaxEvents` bounds, `config`/`subscriptionDetail` required), and the
  documented eUICC-engine / `sink`-delivery cuts. **Spec↔code drift found and fixed
  — the deterministic-value / placeholder class** (as on verified-caller /
  click-to-dial / network-access-devices / edge-application-management): all seven
  `200` success examples carried the placeholder `imei "356938035643809"` and
  `iccid "8931089011234567890"`, and the two `profileResultQuery` examples also
  claimed placeholder reported `eId`s (`A1B2C3D4E5F6…0001`/`…0000`), though the
  handler derives `imei`/`iccid` deterministically from the device `eId` (FNV-1a +
  Luhn) and the query leg's reported `eId` from the `taskId` (`eid_from_task`).
  Corrected all seven to the handler's real values (eId …0001 → imei
  992085662256008 / iccid 89814418471879978689; eId …0000 → imei 003080778538115;
  taskId task-001 → eId 9b056a5d5ef9f464030ffcfd8d0c89ca / imei 054921625391109 /
  iccid 89974964855544562789; taskId task-000 → eId 9b056b5d5ef9f617030ffdfd8d0c8b7d
  / imei 791890645115681 / iccid 89642926825946493487), bound each to its request
  with a derivation note, and added `vendored_success_examples_match_the_handler`
  driving all four endpoints through the real router to pin every documented body.
  **Two stale "first slice / not modelled yet" docs fixed** (the stale-slice-doc
  trap caught on device-data-volume / qos-booking / dedicated-network-*): the parent
  `esim_remote_management.rs` doc ("This first slice models only the stateless
  profile-inventory query … the three asynchronous … legs … are not in scope for
  this module yet") and the `profileList` `x-camarasim-scenarios` note ("The
  asynchronous lifecycle operations … are not modelled yet") — corrected both to
  the complete four-leg surface. `cargo test` green (3087 → 3088 tests) and
  `cargo build --release` green; release binary 5,446,808 bytes (up 4,096 bytes /
  one page from the prior 5,442,712 — the embedded vendored spec grew with the
  corrected example values + descriptions; no runtime-code change). This advances
  the vwip audit cursor to the next API (network-access-domains, the last unaudited
  vwip API). Spec + doc + `#[cfg(test)]` change only (no behaviour/response change —
  the handlers were already correct; only the examples carried placeholders); no new
  dependency. *Backlog banner still stands:* the net-new feature backlog remains
  exhausted and the human decision (authorise stateful infra, name an increment, or
  pause the routine) is still open — this is the sanctioned per-API vwip
  verification (backlog item 3). **Integration note:** on entry the local checkout
  was a detached HEAD at `5833e14` (edge-application-management) == `origin/main`;
  local `main` was stale (`f15d2f9`) and was fast-forwarded. This pass branches off
  `5833e14`, rebases on main, and pushes one commit on top.
- 2026-10-13 — Phase 5 vwip verification pass (edge-application-management).
  Audited the twenty-ninth vwip API in mount order (the management counterpart to
  the EdgeCloud discovery family: two read-only catalogs plus three stateful
  resource lifecycles). All **fifteen** modelled operations — `getEdgeCloudZones`,
  `getClusters`, the `apps` CRUD, the `app-instances` CRUD, and the `deployments`
  create/read/list/update(merge-patch)/delete lifecycle — are implemented
  (`src/apis/edge_application_management/vwip.rs` + `store.rs` / `instance_store.rs`
  / `deployment_store.rs`), routed, and well-tested (~90 test fns before this
  pass). Verified the control planes match the handler: the two catalogs are
  pure fixed tables filtered by query params (no device identifier → no
  reserved-error plane); the stateful legs mint deterministic v5-UUID ids from
  their identity (`app_id` from name+version+provider, `instance_id` from
  app+zone, `deployment_id` from app+name+sorted-zones) so duplicates 409, and
  the zone/cluster ids are v5 UUIDs derived from the name (`zone_id`/`cluster_ref`).
  **Spec↔code drift found and fixed — the deterministic-value / placeholder-UUID
  class** (as on application-endpoint-discovery / network-access-devices /
  click-to-dial): the two reproducible catalog 200 examples carried Swagger
  placeholders — getEdgeCloudZones `catalog` claimed "the full zone catalog" but
  showed 1 of 6 zones with `edgeCloudZoneId: 6ec0bd7f-…`, and getClusters
  `catalog` carried `clusterRef: 642f6105-…` (version-4 shaped, so `cluster_ref`,
  which forces v5, can never emit it) plus the same placeholder zone id. Completed
  both to the real full catalogs (6 zones / 4 clusters) with the handler's derived
  ids (e.g. `camarasim-edge-eu-west-1` → `6c7adb07-5cdc-58a3-b049-918bcd741d7b`;
  `camarasim-cluster-eu-west-1a` → `48397420-b675-506a-a94f-cf6ea9f6c885`), and
  corrected the name-paired `EdgeCloudZone` schema example id to its derived value.
  Added `vendored_success_examples_match_the_handler`, which drives both no-filter
  GETs through the real router and pins each documented zone/cluster id both as the
  literal and against `zone_id`/`cluster_ref` so they can't silently drift (the
  bare illustrative UUID schema examples — appInstanceId/appDeploymentId/Location/
  request-body ids — are left illustrative, the application-profiles posture).
  **Two stale "later passes" module docs fixed** (the stale-slice-doc trap caught
  on device-data-volume / qos-booking / dedicated-network-*): the parent
  `edge_application_management.rs` doc and the `vwip.rs` header both framed the
  app-instance read/list/delete legs and the deployment/cluster resources as
  future work, and the header's endpoint list omitted `getClusters`,
  `getAppDeployments`, `deleteAppDeployment` though all fifteen ship — corrected
  both to the complete surface. `cargo test` green (3086 → 3087 tests) and
  `cargo build --release` green; release binary 5,442,712 bytes (~5.19 MiB, up
  4,096 bytes / one page from the prior 5,438,616 — the embedded vendored spec
  grew with the completed catalog examples; no runtime-code change). This advances
  the vwip audit cursor to the next API (esim-remote-management). Spec + doc +
  `#[cfg(test)]` change only (no behaviour/response change — the handlers were
  already correct; only the two catalog examples and one schema example carried
  placeholders); no new dependency. *Backlog banner still stands:* the net-new
  feature backlog remains exhausted and the human decision (authorise stateful
  infra, name an increment, or pause the routine) is still open — this is the
  sanctioned per-API vwip verification (backlog item 3). **Integration note:** on
  entry the local checkout was a detached HEAD at `e421bcb`
  (dedicated-network-areas) == `origin/main`; local `main` was stale (`f15d2f9`)
  and was fast-forwarded. This pass branches off `e421bcb`, rebases on main, and
  pushes one commit on top.
- 2026-10-13 — Phase 5 vwip verification pass (dedicated-network-areas). Audited
  the twenty-eighth vwip API in mount order (the geographical catalog sibling of
  dedicated-network-profiles / -network / -network-accesses). Both modelled
  operations — `readNetworkServiceArea` (`GET /areas/{areaId}`) and
  `retrieveNetworkServiceAreas` (`POST /retrieve-service-areas`, AND-combined
  CIRCLE-only spatial + attribute filters over a fixed four-area catalog) — are
  implemented (`src/apis/dedicated_network_areas/vwip.rs`), routed, and
  well-tested (36 test fns before this pass). **Spec↔code drift found and fixed —
  the deterministic-value class:** the vendored `readNetworkServiceArea` 200
  `cityCentre` example was bound to `areaId …001` but showed `london-city`, yet
  the handler selects `TEMPLATES[digits % N]` so tail `001` → `manchester-metro`
  (template 1), not `london-city` (template 0) — the example's body is entirely
  london-city, so the id was the wrong part (the existing `happy_path` test
  already pins `…001` → manchester-metro). Rebound the example to `areaId …000`
  and added a derivation comment. Added `vendored_success_example_matches_the_handler`
  (drives `…000` through the router pinning the full london-city body, asserts
  `…001` selects a different area so the id stays load-bearing, and pins the
  collection `catalog` example's `…000`/`…002` ids/bodies) and
  `vendored_error_example_messages_match_the_handler` (pins the three inline error
  examples' exact `status`/`code`/`message` bytes). Module docs agree with the
  code; no stale-slice doc trap (both operations already documented as shipped).
  `cargo test` green (3084 → 3086 tests) and `cargo build --release` green;
  release binary 5,438,616 bytes (~5.2 MiB, unchanged from the prior pass). This
  advances the vwip audit cursor to the next API (edge-application-management).
  Spec + `#[cfg(test)]` change only (no behaviour/response change — the handler
  was already correct; only the one success-example id was wrong); no new
  dependency.
- 2026-10-13 — Phase 5 vwip verification pass (dedicated-network-accesses).
  Audited the twenty-seventh vwip API in mount order (the access sibling of the
  DedicatedNetworks family). All seven modelled operations over the full access
  lifecycle (`createAccess`, `readAccess`, `listAccesses`, `deleteAccess`,
  `listDevices`, `addDevicesToAccess`, `removeDevicesFromAccess`) are
  implemented (`src/apis/dedicated_network_accesses/vwip.rs` + an in-memory
  `store.rs`), routed, and well-tested (54 test fns before this pass). Verified
  the three control planes match the handler (request validation → 400; the
  `networkId` reserved-suffix plane; the per-device GRANTED/DENIED grant plane)
  and that all success examples match. **No spec↔code value drift:** there are
  no inline error examples (every 400 `$ref`s the `AccessError` schema), and the
  minted `id`s are legitimately illustrative (counter/clock-derived). **Fixed
  the stale "create-only / later passes" doc trap in six places** (parent module
  doc, `vwip.rs` header store-bridge sentence + documented-cuts paragraph, the
  `sink_credential` struct comment, three `store.rs` doc lines, and the spec's
  `SinkCredential` schema string) — all claimed read/list/delete and the device
  sub-resources were future work though all ship — keeping the genuine cuts
  (CAMARA 207 / 422 NO_VALID_DEVICE / 409 conflict forms, `sink` delivery,
  `x-device` filter). Added `vendored_success_examples_match_the_handler`
  pinning the `createAccess` `mixed`, `listDevices` `onePage`, and
  `addDevicesToAccess` `added` documented examples through the router. `cargo
  test` green (3083 → 3084 tests) and `cargo build --release` green; release
  binary 5,438,616 bytes (~5.2 MiB, unchanged from the prior pass). This
  advances the vwip audit cursor to the next API (dedicated-network-areas). Doc
  + spec-string + `#[cfg(test)]` change only (no behaviour/response change — the
  handlers were already correct; the drift was doc staleness, not examples); no
  new dependency.
- 2026-10-13 — Phase 5 vwip verification pass (dedicated-network). Audited the
  twenty-sixth vwip API in mount order (the resource sibling of
  dedicated-network-profiles). All four modelled CRUD operations — `createNetwork`
  (`POST /networks` → 201 `NetworkInfo`), the stateful `readNetwork`
  (`GET /networks/{networkId}`), `listNetworks` (`GET /networks`, optional `name`
  filter, bare JSON array), and `deleteNetwork` (`DELETE …/{networkId}` → 204,
  single-use) — are implemented (`src/apis/dedicated_network/vwip.rs` + an
  in-memory `store.rs`), routed, and well-tested (~40 test fns before this pass).
  Verified the two control planes match the handler: request validation (name
  length, the `networkProfileId`/`qosProfileName` `oneOf`, UUID shapes, the
  `serviceTime` RFC-3339 bounds + UTC end<start → 400 `OUT_OF_RANGE`, the
  `^https://.+$` sink) and the `serviceAreaId` identifier plane (reserved trailing
  suffix → the canonical CAMARA error; else `d % 3` → `REQUESTED`/`RESERVED`/
  `ACTIVATED`). **No spec↔code value drift found:** all success examples
  (`reserved` tail 001→RESERVED, `twoNetworks` 001/002, read `activated` 002→
  ACTIVATED) match the `status_for` mapping, and the minted `id`s are legitimately
  illustrative (`store::new_network_id` is counter/clock-derived, the
  application-profiles posture, not request-reproducible); the one inline error
  example (`createNetwork` 400 `outOfRange`) was byte-accurate. **Stale
  "create-only / later pass" docs fixed in four places** (the same stale-slice-doc
  trap caught on device-data-volume / qos-booking / application-profiles /
  session-insights / sponsored-data / network-access-devices): the parent
  `dedicated_network.rs` doc ("only the create leg is mounted … read/list/delete
  arrive in later passes"), the `vwip.rs` body doc ("this slice being create-only
  … list/delete arrive in later passes"), the `store.rs` `get` doc ("the delete
  leg (a later pass) will too"), and two spec strings (`info.description` and the
  `SinkCredential` schema, both "this slice being create-only") — corrected to
  describe the full CRUD while keeping the genuine documented cut (no `sink`
  notification is ever delivered; this API records no notification target).
  **Small tested increment:** added `vendored_success_example_matches_the_handler`
  (drives the spec's `withProfile` request through the router and pins the
  `reserved` 201 example's status + echoed fields) and
  `vendored_error_example_messages_match_the_handler` (pins the `outOfRange`
  example's exact `{status,code,message}` bytes, which existing tests guarded only
  for `code`). `cargo test` green (3081 → 3083 tests) and `cargo build --release`
  green; release binary 5,438,616 bytes (~5.2 MiB, unchanged from the prior pass).
  This advances the vwip audit cursor to the next API (dedicated-network-accesses).
  Spec + doc + `#[cfg(test)]` change only (no behaviour/response change — the
  handlers were already correct; the drift was doc staleness, not examples); no
  new dependency.
- 2026-10-13 — Phase 5 vwip verification pass (dedicated-network-profiles).
  Audited the twenty-fifth vwip API in mount order (the first of the
  DedicatedNetworks family). Both modelled operations — `readNetworkProfiles`
  (`GET /profiles`, a paginated `NetworkProfilesPage`) and `readNetworkProfile`
  (`GET /profiles/{profileId}`) — are implemented
  (`src/apis/dedicated_network_profiles/vwip.rs`), routed, and well-tested
  (21 test fns before this pass). Verified the control planes match the handler:
  the fixed four-template catalog (enterprise-campus/iot-massive/public-safety/
  media-production); the `profileId` three-layer plane (non-UUID → 400
  `INVALID_ARGUMENT`; reserved trailing suffix → the canonical CAMARA error; else
  the trailing-three-digits `d % 4` template with the requested id echoed); and
  the list planes (exact-match `name` filter → empty page on a miss, never a 404;
  the `page`/`perPage` window; no device/line identifier so no reserved plane).
  **Spec↔code drift found and fixed in the vendored `200` `firstPage` list
  example:** it carried only the *first* of the four catalog items while its own
  pagination claimed `totalCount 4` / `totalPages 1` / `perPage 10` — an
  incomplete page that can't come back from the router, which returns all four on
  page 1 for `GET /profiles` with no params (the first incomplete-list-example
  drift class, distinct from the deterministic-value and truncated-message
  classes of earlier passes). Completed the example to the real four-item body
  (each item's id = `template_id(i)`, its canonical catalog-index UUID). The by-id
  `iotMassive` example (`…001` → iot-massive, byte-accurate) and both inline 400
  error examples (list `outOfRange`, by-id `invalidArgument`) were already
  correct. Added `vendored_success_examples_match_the_handler` (drives the real
  router for `GET /profiles` and pins the full four-item items array + pagination,
  and the `…001` by-id body) and `vendored_error_example_messages_match_the_handler`
  (pins both inline 400 examples' exact `{status,code,message}` bytes, which the
  existing error tests guarded only for `code`). `cargo test` green (3079 → 3081
  tests) and `cargo build --release` green; release binary 5,438,616 bytes
  (~5.2 MiB; the only non-test change is the enlarged embedded spec example).
  This advances the vwip audit cursor to the next API
  (dedicated-network). Spec + `#[cfg(test)]` change only (no behaviour/response
  change — the handlers were already correct; only the one list example was
  incomplete); no new dependency.
- 2026-10-13 — Phase 5 vwip verification pass (capabilities-and-restrictions).
  Audited the twenty-fourth vwip API in mount order. The one modelled operation
  (`postServiceCapability`, `POST /capabilities-and-restrictions/vwip/retrieve`)
  is implemented (`src/apis/capabilities_and_restrictions/vwip.rs`), routed, and
  well-tested (23 test fns before this pass). Verified the control planes match
  the handler: the reserved-error suffix on the first query's first `phoneNumber`
  (`…404` = "No Capability API found"); the context-derived
  `camaraCapabilitiesBitmap` (trailing three digits of the queried phone number,
  else an FNV-1a hash of `overlayExtends`, `% 2^CATALOGUE_SIZE`=8); the
  three-entry synthetic restriction catalogue; the `overlayExtends`/
  `resourceScopes` echo; and the 400 `INVALID_ARGUMENT`/`OUT_OF_RANGE` validation
  plane. **Spec↔code drift found and fixed — the deterministic-value class** (as
  on network-access-devices / click-to-dial / verified-caller): the `201` `info`
  success example, bound to the request example's `phoneNumber "+123456789012"`,
  claimed `camaraCapabilitiesBitmap: 5`, but the handler derives it from the
  trailing three digits (`012` = 12, `12 % 8 = 4`), so the value is `4` (the `5`
  was evidently copied from the `…005` unit-test scenario). Corrected the example
  to `4` and added `vendored_success_example_matches_the_handler`, driving the
  documented request through the real router to pin the bitmap, the echoed
  `overlayExtends`/`resourceScopes`, the full three-key `bitmapCapabilities` map,
  and key `"0"`'s exact restriction block. Also added
  `vendored_error_example_messages_match_the_handler` pinning the exact
  `status`/`code`/`message` bytes of the two inline `400` examples
  (`invalidArgument`, `outOfRange` — the only error responses not `$ref`ing the
  shared catalog, previously guarded only for `code`). All other examples and the
  shared-`$ref` 401/403/404/409/422/429/500/503 responses already matched. Green:
  `cargo test` 3079 passed (0 failed), `cargo build --release` ok, release binary
  5,434,520 bytes (unchanged — spec-example + `#[cfg(test)]` change only, no
  runtime code touched; no new dependency). This advances the vwip audit cursor
  to the next API (dedicated-network-profiles).
- 2026-10-13 — Phase 5 vwip verification pass (network-access-devices). Audited
  the twenty-third vwip API in mount order (device listing + a stateful
  reboot-request resource lifecycle). All **five** modelled operations —
  `getNetworkAccessDevices`, `getNetworkAccessDevice`, `createRebootRequest`,
  `getRebootRequest`, `updateRebootRequest` (merge-patch, 409
  `NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE` on an immediate reboot),
  `deleteRebootRequest` — are implemented
  (`src/apis/network_access_devices/vwip.rs` + an in-memory `store.rs`), routed,
  and well-tested (74 test fns before this pass). I verified the control planes
  match the handler: the device set is deterministic from the token subject (the
  trailing three digits fix the count `d==0→1 else ((d-1)%3)+1` and each device's
  status `STATUSES[(d+i)%3]`; `id` + `hardwareAddress` are `SHA-256`-derived); a
  reserved subject suffix answers the shared canonical error on the subject-keyed
  operations; the reboot-request legs are keyed on opaque server-minted ids with
  store state (and, for PATCH, the stored request's schedule state) as the only
  control plane; the 409 inline example byte-matched the handler. **Spec↔code
  drift found and fixed — the deterministic-value / placeholder class** (the same
  class caught on application-endpoint-discovery / click-to-dial /
  verified-caller / iot-sim-fraud-prevention): the `200` `singleConnected` /
  `twoDevices` listing examples and the `connectedGateway` by-id example were
  bound to concrete subject tails (`…000` / `…002`) but carried placeholder device
  `id`s (`3fa85f64-…`, `123e4567-…`) and MACs, though every `id` and
  `hardwareAddress.value` is a deterministic `SHA-256("nad-device:{sub}:{i}")`
  derivation — for `+123456789000` the one device is actually
  `ad59992d-c305-2bb1-d861-380b14eec986` / `81:F4:87:98:A7:60`, and for
  `+123456789002` the two are `20664178-…` / `C5:A5:64:05:03:37` and `939deb68-…`
  / `F7:DB:88:02:F6:9E`; the `createRebootRequest` / `getRebootRequest` examples'
  materialised `devices` entry carried the same placeholder though it is the
  subject's real derived device id. I bound all five examples to concrete
  subjects, corrected the ids/MACs to the handler's exact values (keeping the
  minted reboot-request `id` and the `createdAt`/`modifiedAt` timestamps
  explicitly illustrative — the application-profiles / session-insights posture,
  since `store::new_reboot_request_id` is counter/clock-derived), and added
  `vendored_success_examples_match_the_handler`, which drives the real router for
  both subjects and pins the full `singleConnected` / `twoDevices` list bodies,
  the `connectedGateway` by-id body, and the reboot-all materialised `devices`
  array (the minted id/timestamps are not pinned, being non-deterministic). All
  error responses are `$ref`s to the shared `errors.yaml` catalog except the
  update `409`, whose inline example already byte-matched the handler. **Two
  stale module docs fixed** (the "later slices / later passes" stale-doc trap
  class caught on device-data-volume / qos-booking / application-profiles /
  session-insights / sponsored-data): the parent `network_access_devices.rs`
  claimed "The read/patch/delete legs are later slices" and the `vwip.rs` header
  said "Three endpoints" (then listed five) and "The `PATCH` (update) leg is a
  later slice," though all five legs ship and are fully tested — corrected both to
  describe the complete lifecycle. This advances the vwip audit cursor to the next
  API (capabilities-and-restrictions). Spec + doc + `#[cfg(test)]` change (no
  behaviour/response change — the handlers were already correct; only the example
  ids/MACs were placeholders); no new dependency. `cargo test` green: 3077 passed,
  0 failed (+1, the new pinning test). `cargo build --release` green; binary =
  5,434,520 bytes (unchanged vs. the predictive-connectivity-data pass on rustc
  1.97.0 — the change is outside the release path). *Backlog banner still stands:*
  the net-new feature backlog remains exhausted and the human decision (authorise
  stateful infra, name an increment, or pause the routine) is still open — this is
  the sanctioned per-API vwip verification (backlog item 3). **Integration note:**
  on entry the local checkout was a detached HEAD at `47274ba`
  (predictive-connectivity-data) == `origin/main`; local `main` was fast-forwarded
  to it. This pass branches off `47274ba`, rebases on main, and pushes one commit
  on top.

- 2026-10-13 — Phase 5 vwip verification pass (predictive-connectivity-data).
  Audited the twenty-second vwip API in mount order (a stateless, deterministic,
  area-keyed connectivity forecast — no stored state, no device identifier). The
  single modelled operation `retrieveConnectivity`
  (`POST /predictive-connectivity-data/vwip/retrieve`) is fully implemented
  (`src/apis/predictive_connectivity_data/vwip.rs`), routed, and well-tested (35
  test fns before this pass), and the module docs agree with the code and the
  vendored spec. I verified the geometry-as-control-plane design matches the
  handler: the first geohash's trailing three digits drive the shared
  reserved-error convention (checked after structural validation, before the time
  window); each geohash's FNV-1a hash fixes its cell (`h%7==0` → a NO_DATA cell of
  all `ND`; else ground score `s=h%100` degrading 15 points per 30 m layer, mapped
  to `GC`/`MC`/`NC` by the target `serviceLevel`'s thresholds — C2 70/40, STREAM_4K
  50/25, BEST_EFFORT 30/10); the overall `status` summarises the cell mix
  (all/some/no NO_DATA → AREA_NOT_SUPPORTED / PART_OF_AREA_NOT_SUPPORTED /
  SUPPORTED_AREA); `height` sets the layer count (`height/30 + 1`, default 4) and
  is echoed as `requestedHeight`; `includeSignalStrength` toggles the per-layer
  `layerSignalStrengths` (dBm) band; and the documented cuts (`POLYGON` → 422
  UNSUPPORTED_AREA_TYPE, `precision` with GEOHASHLIST → 400, length>9 → 422
  UNSUPPORTED_PRECISION, >100 geohashes → 422 UNSUPPORTED_SYNC_RESPONSE, async sink
  accepted-not-acted-on, single time slice, deterministic INVALID_END_TIME /
  MAX_TIME_PERIOD_EXCEEDED window checks) all hold. **Spec↔code drift found and
  fixed in the vendored `200` `partlySupported` example — the NO_DATA-mislabel /
  deterministic-value class** (the same class caught on population-density-data,
  which had the very same `gbsuv`→NO_DATA error): the example showed `gbsuv` as a
  NO_DATA cell (`[ND,ND,ND,ND]`), but `fnv1a("gbsuv")%7==1` → it is a *data* cell
  (ground 62), so an area of `u4pruy`+`gbsuv` (both data cells) would actually be
  `SUPPORTED_AREA`, contradicting the example's `PART_OF_AREA_NOT_SUPPORTED`
  status; and `u4pruy`'s `[GC,MC,NC,NC]` layers hold only under `BEST_EFFORT`
  (ground 36 under the 30/10 thresholds), though the example named no
  `serviceLevel`. I rebound the example to a concrete request — `BEST_EFFORT` over
  `["u4pruy","dr5bb"]` for the documented 7-day window — and replaced `gbsuv` with
  `dr5bb`, which genuinely hashes to a NO_DATA cell (`%7==0`), so the mixed area is
  correctly `PART_OF_AREA_NOT_SUPPORTED` and `u4pruy`'s `[GC,MC,NC,NC]` is exactly
  the handler's BEST_EFFORT output. Added
  `vendored_success_example_matches_the_handler`, which drives the real router with
  that request and pins the exact documented bytes (layerThickness 30, both slice
  times, `u4pruy`→`[GC,MC,NC,NC]` with no strengths band, `dr5bb`→`[ND×4]`, status,
  and the omitted `requestedHeight`), and `vendored_error_example_messages_match_the_handler`,
  which pins the six inline `400`/`422` error-example `message` bytes through the
  router (invalidArgument, invalidEndTime, maxTimePeriodExceeded, unsupportedAreaType,
  unsupportedPrecision, unsupportedSyncResponse) — the existing error tests guarded
  only `status`/`code`, so a message could have silently drifted (the pinning the
  media-streaming-rate / subscription-status passes added). Every inline error
  example was already byte-accurate; the `401`/`403`/`404`/`409`/`429`/`500`/`503`
  responses are `$ref`s to the shared `errors.yaml` catalog (covered by its own
  tests). This advances the vwip audit cursor to the next API
  (network-access-devices). Spec + `#[cfg(test)]` change (no behaviour/response
  change — the handler was already correct; only the one success example was
  wrong); no new dependency. `cargo test` green: 3076 passed, 0 failed (+2, the two
  new pinning tests). `cargo build --release` green; binary = 5,434,520 bytes
  (unchanged vs. the application-endpoint-registration pass on rustc 1.97.0 — the
  change is outside the release path). *Backlog banner still stands:* the net-new
  feature backlog remains exhausted and the human decision (authorise stateful
  infra, name an increment, or pause the routine) is still open — this is the
  sanctioned per-API vwip verification (backlog item 3). **Integration note:** on
  entry the local checkout was a detached HEAD at `a2e45fa`
  (application-endpoint-registration) == `origin/main`; local `main` pointer was
  stale (f15d2f9) and was fast-forwarded. This pass branches off `a2e45fa`,
  rebases on main, and pushes one commit on top. No origin divergence or
  regression observed.

- 2026-10-13 — Phase 5 vwip verification pass (application-endpoint-registration).
  Audited the twenty-first vwip API in mount order (a stateful, resource-oriented
  registration: an in-memory `applicationEndpointListId`→rendered-JSON `store`, no
  notifications). All **five** modelled operations over the full lifecycle are
  implemented (`src/apis/application_endpoint_registration/vwip.rs` + `store.rs`),
  routed, and well-tested (44 test fns before this pass):
  `registerApplicationEndpoints` (`POST /application-endpoint-lists` → `200` with a
  bare minted `applicationEndpointListId`), `getAllRegisteredApplicationEndpoints`
  (`GET /application-endpoint-lists`, store-sorted array, never `404`s),
  `getApplicationEndpointsById` (`GET …/{id}`), `updateApplicationEndpoint`
  (`PUT …/{id}`, full replace → `204`, body validated before store state so a body
  `400` wins over a `404`), and `deregisterApplicationEndpoint` (`DELETE …/{id}` →
  `204`, single-use). I verified the control planes match the handler: there is **no**
  device identifier / two-legged-three-legged rule (a registration is a free-standing
  catalog resource); request-body validation (empty/`>20` endpoints, the `anyOf`
  address form, port `1..=65535` → `OUT_OF_RANGE`, malformed IPv4/IPv6/`edgeCloudZoneId`,
  multi-line/over-length provider/description, non-UUID `applicationProfileId`) → `400`;
  the shared reserved-error suffix on `applicationProfileId` (`scenarios::reserved_error`,
  e.g. `…404`→404, `…422`→422 `SERVICE_NOT_APPLICABLE`); and the nil-UUID sentinel
  `00000000-0000-0000-0000-000000000000` → `422 UNIDENTIFIABLE_APPLICATION_PROFILE`
  (the API's own code), checked after the reserved suffix. The minted
  `applicationEndpointListId` is legitimately *illustrative* (`store::new_list_id` is a
  SHA-256 over an atomic counter + clock, not the request — the application-profiles /
  session-insights posture, not the deterministic-id posture of verified-caller /
  click-to-dial), so the vendored `200` example id is correctly not pinned.
  **No spec↔code drift found — every inline error example was already byte-accurate
  against the handler** (the `was→is`, truncated-identifier, and placeholder-UUID drift
  classes caught on earlier APIs do not occur here). **Fixed one stale store-module
  doc** (the stale-"later passes / trimmed slice" trap class caught on
  device-data-volume / qos-booking / application-profiles / session-insights /
  sponsored-data): `store.rs` described the `GET {id}` / `PUT` / `DELETE` legs as
  "landing in later passes" and the store as "trimmed to what the register slice needs,"
  though all four legs ship and `get`/`all`/`replace`/`remove` are all present and
  documented later in the same file — corrected to describe the full
  read/list/replace/remove lifecycle. **Small tested increment:** every existing error
  test asserted only `status`/`code`, so no test guarded the vendored inline
  error-example `message` bytes; added `vendored_error_example_messages_match_the_handler`,
  which drives the real router and pins the exact `status`/`code`/`message` bytes of all
  router-reachable inline error families — register `400`
  `invalidArgument`/`outOfRange`, register `422`
  `unidentifiableProfile`/`serviceNotApplicable`, getById `400`, delete `400`, and
  update `400` (body + non-UUID path) / `422` — so none can silently drift (the same
  pinning the device-authenticity / network-health-assessment passes added). The
  `401`/`403`/`404`/`409`/`429`/`500`/`503` responses are `$ref`s to the shared
  `errors.yaml` catalog (covered by its own tests). This advances the vwip audit cursor
  to the next API (predictive-connectivity-data). Spec unchanged (examples were already
  correct); doc + `#[cfg(test)]` change only; no new dependency. `cargo test` green:
  3074 passed, 0 failed (+1, the new pinning test). `cargo build --release` green;
  binary = 5,434,520 bytes (unchanged vs. the application-endpoint-discovery pass on
  rustc 1.97.0 — the change is outside the release path). *Backlog banner still stands:*
  the net-new feature backlog remains exhausted and the human decision (authorise
  stateful infra, name an increment, or pause the routine) is still open — this is the
  sanctioned per-API vwip verification (backlog item 3). **Integration note:** on entry
  the local checkout was a detached HEAD at `cc206c9` (application-endpoint-discovery),
  7 commits ahead of the *local* `main` pointer (f15d2f9); a `git fetch` confirmed
  `origin/main` was already at `cc206c9` (the
  iot-sim-fraud-prevention…application-endpoint-discovery passes had pushed cleanly — the
  local `main`/tracking refs were merely stale), so this pass fast-forwards local `main`
  to this commit and pushes one commit on top. No origin divergence or regression
  observed.

- 2026-10-13 — Phase 5 vwip verification pass (application-endpoint-discovery).
  Audited the twentieth vwip API in mount order (a stateless, deterministic edge
  endpoint discovery — no stored state). The single modelled operation
  `getOptimalAppEndpoints`
  (`POST /application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints`) is
  fully implemented (`src/apis/application_endpoint_discovery/vwip.rs`), routed,
  and well-tested (29 test fns before this pass), and the module docs agree with
  the code and the vendored spec. I verified the three genuine control planes
  match the handler: the structural application-identifier plane (exactly one
  UUID-shaped `appId` / `applicationEndpointsId` — neither/both/non-UUID → 400
  `INVALID_ARGUMENT`; a documented divergence from CAMARA's `anyOf`; the trailing
  three digits `d` then drive the set: `…000` → 404 `NOT_FOUND`, else `(d % 3) + 1`
  endpoints with a `(d + rank) % 3` rotating fqdn/single-ipv4/single-ipv6 address
  family), the device reserved-error-suffix plane (`scenarios::reserved_error`,
  checked **before** the app-not-found 404 so the device `…404`/`…429`/… cases are
  reachable), and the two-legged/three-legged identifier resolution (mirrors
  Optimal Edge Discovery: `device`+line subject → 422 `UNNECESSARY_IDENTIFIER`;
  empty `device` → 400 `INVALID_ARGUMENT`; absent `device` + non-line subject →
  422 `MISSING_IDENTIFIER`; `DeviceResponse` echoed only when the request `device`
  carried ≥2 identifiers incl. a `phoneNumber`). **Spec↔code drift found and fixed
  — the deterministic-value / placeholder class** (identical to the
  verified-caller / optimal-edge-discovery / click-to-dial /
  iot-sim-fraud-prevention passes): the vendored `single` 200 example, explicitly
  bound to `appId …012`, carried placeholder values — `fqdn:
  app-1a2b3c4d.edge.camarasim.example.com` and `edgeCloudZoneId: 5d803d3f-…` — but
  `build_endpoint`/`zone_id` derive both deterministically via SHA-256, so a
  consumer sending appId …012 actually gets `fqdn:
  app-d08c37d8.edge.camarasim.example.com` and (for the chosen zone
  `camarasim-edge-eu-west-1`) `edgeCloudZoneId:
  b89bb689-d520-cfa4-6542-021d73c84510`. The `EdgeCloudZone` schema example paired
  that same zone name with the same wrong id (and the id is a pure function of the
  name), so I corrected it to the real value as well. Corrected all three and
  added `vendored_success_example_matches_the_handler`, which drives the real
  router for appId …012 and pins the exact documented bytes — `port` 1036, the
  `fqdn`, the single-address-family shape (no ipv4/ipv6 siblings), every
  `edgeCloudZone` field (id/name/provider/region/status), the description, the
  echoed `appId`, `applicationServerProviderName`, and the absent `device` — so
  the example can never silently drift again. **Two more inline error-example
  drifts found and fixed in the same pass**, both previously-seen classes: (a) the
  404 `notFound` example said "The specified resource **was** not found." but
  `CamaraError::for_status(404)` emits "…**is** not found." (the was→is class
  caught on media-streaming-rate / optimal-edge-discovery), and (b) the 422
  `missingIdentifier` example carried the truncated "The device cannot be
  identified." but `resolve_identifier` emits the full "…: supply a `device` or
  use a token that identifies a device." (the truncated-identifier-message class
  caught on subscription-status / media-streaming-rate / optimal-edge-discovery /
  consent-info / most-frequent-location). Corrected both and added
  `vendored_error_example_messages_match_the_handler`, which drives the router and
  pins the exact `status`/`code`/`message` bytes of all four router-reachable
  inline error families (400 `invalidArgument`, 404 `notFound`, 422
  `missingIdentifier`, 422 `unnecessaryIdentifier`). The 401/403/409/429/500/503
  responses are `$ref`s to the shared catalog (covered by its own tests). This
  advances the vwip audit cursor to the next API
  (application-endpoint-registration). Spec + test change (no behaviour/response
  change — the handler's output was already correct; only the vendored examples
  carried placeholders / stale messages); no new dependency. `cargo test` green:
  3073 passed, 0 failed (+2, the two new pinning tests). `cargo build --release`
  green;
  binary = 5,434,520 bytes (unchanged vs. the traffic-influence pass on rustc
  1.97.0 — the change touches only the spec and `#[cfg(test)]` code, outside the
  release path). *Backlog banner still stands:* the net-new feature backlog
  remains exhausted and the human decision (authorise stateful infra, name an
  increment, or pause the routine) is still open — this is the sanctioned per-API
  vwip verification (backlog item 3), and this pass it caught a genuine,
  consumer-visible documentation defect.

- 2026-10-13 — Phase 5 vwip verification pass (traffic-influence). Audited the
  nineteenth vwip API in mount order (a stateful resource: an in-memory
  `trafficInfluenceID`→rendered-JSON `store`, plus a `notifications` module firing
  the create-time initial-event `traffic-influence-change` CloudEvent). All **six**
  modelled operations are fully implemented, routed, and well-tested (~87 test fns
  before this pass): `postTrafficInfluence` (`POST /traffic-influences`),
  `getAllTrafficInfluences` (`GET /traffic-influences`, optional `appId` filter,
  bare array, never 404s), `postTrafficInfluenceDevice`
  (`POST /traffic-influence-devices`, same fields + a required `device` that is
  validated-only / never echoed / never persisted), the stateful
  `getTrafficInfluence` (`GET …/{trafficInfluenceID}`), `patchTrafficInfluence`
  (`PATCH …/{id}`, `merge-patch+json`, body validated before the store so a body
  `400` wins over an unknown-id `404`, and `state` is **not** re-derived), and
  `deleteTrafficInfluence` (`DELETE …/{id}`, synchronous single-use eviction behind
  the `202` contract). I verified the two genuine `appId` control planes match the
  handler: the reserved-error suffix plane (`scenarios::reserved_error`) and
  `derive_state` (`d = appId` trailing three digits; `d%3` → 0 `ordered` / 1
  `created` / 2 `active`; <3 digits → `ordered`). The minted `trafficInfluenceID`s
  are legitimately *illustrative* — `mint_id` is a SHA-256 over an atomic counter +
  clock seconds (not the request), so pinning a concrete value would be wrong (the
  application-profiles / session-insights / sponsored-data posture, not the
  deterministic-id posture of verified-caller / click-to-dial). All **seven** inline
  `400` error-example `message` bytes already matched the handler exactly (no
  message drift — the truncated-identifier-message class caught on earlier APIs does
  not occur here). **Spec↔code consistency defect found and fixed — a new class
  (self-inconsistent success example):** the GET-list `two` example showed two
  resources that *share* `appId …426614174002` yet carry **different** states (one
  `active`, one `created`). Because `state` is a pure function of the shared appId's
  tail and PATCH never re-derives it, two resources with the same appId *always*
  share one `state` (`…002` → `active`) — the `created` entry is an impossible
  combination a consumer could never reproduce (and the existing
  `list_filtered_by_app_id_returns_exactly_the_matching_resources` test already
  creates both resources under one appId, confirming the shared-state invariant).
  Corrected the second entry to `active` and clarified the example summary that the
  `state` is derived from the shared appId tail. Also **fixed the stale spec header
  comment**: it enumerated five served operations and omitted
  `getAllTrafficInfluences` though the spec body (and the module docs) define it —
  the stale-doc trap class caught on device-data-volume / qos-booking /
  application-profiles / session-insights / sponsored-data; added the missing
  `GET /traffic-influences` line. Added `vendored_examples_match_the_handler`, which
  (a) pins the shared-appId→state consistency the fix depends on
  (`derive_state("…002") == "active"`, guarding the `two`/201/getTrafficInfluence/
  patch `active` examples) and (b) drives the real router to pin the exact
  `status`/`code`/`message` bytes of every router-reachable inline `400` family
  (postTrafficInfluence `invalidArgument` "`appId` must be a UUID." / `outOfRange`
  "`sourcePort` must be between 0 and 65535.", getAll `invalidArgument` "`appId`
  must be a valid UUID.", postDevice `missingDevice` "`device` is required." /
  `emptyDevice` "`device` must contain at least one identifier.", patch
  `invalidArgument` "`appInstanceId` must be a UUID or null." / `outOfRange`) so none
  can silently drift. The 401/403/404/409/422/429/500/503 responses are `$ref`s to
  the shared catalog (covered by its own tests). This advances the vwip audit cursor
  to the next API (application-endpoint-discovery). Spec + test change (no
  behaviour/response change — the handler's output was already correct; only the
  list example was self-inconsistent and the header comment stale); no new
  dependency. `cargo test` green: 3071 passed, 0 failed (+1, the new pinning test).
  `cargo build --release` green; binary = 5,434,520 bytes (unchanged vs. the
  most-frequent-location pass on rustc 1.97.0 — the change touches only the spec and
  `#[cfg(test)]` code, outside the release path). *Backlog banner still stands:* the
  net-new feature backlog remains exhausted and the human decision (authorise
  stateful infra, name an increment, or pause the routine) is still open — this is
  the sanctioned per-API vwip verification (backlog item 3), and this pass it caught
  a genuine, consumer-visible documentation defect.
- 2026-10-13 — Phase 5 vwip verification pass (most-frequent-location). Audited
  the eighteenth vwip API in mount order (a stateless, privacy-preserving
  presence score — no stored state). The single modelled operation
  `verifyFrequentLocation` (`POST /most-frequent-location/vwip/verify`) is fully
  implemented (`src/apis/most_frequent_location/vwip.rs`), routed, and well-tested
  (23 test fns before this pass), and the module docs agree with the code and the
  vendored `specs/most-frequent-location/vwip/openapi.yaml`: the two genuine
  control planes (`score = (identifier trailing three digits + area offset) % 101`,
  where the area offset is `round(|lat|)+round(|long|)` for a `COVERAGE_ZONE` and
  the postal code's trailing three digits for a `POSTAL_CODE`, so the same device
  flips its score as the area changes and vice-versa), the `geoReference`
  validation ordering (request-level 400 plane first — `COVERAGE_ZONE` lat/long
  range → `OUT_OF_RANGE`, reserved `00000` postal code →
  `MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID`, unknown `type`/foreign fields →
  `INVALID_ARGUMENT` — then the identifier's reserved-suffix plane, then the
  score), the two-legged/three-legged identifier resolution (mirrors Connected
  Network Type: `device`+line subject → 422 `UNNECESSARY_IDENTIFIER`;
  `device` with no identifier → 400 `INVALID_ARGUMENT`; absent `device` +
  non-line subject → 422 `MISSING_IDENTIFIER`), and the reserved-error suffix
  plane (`…422` → `SERVICE_NOT_APPLICABLE`) all match. **Spec↔code drift found and
  fixed in the vendored 422 `missingIdentifier` example:** it carried the
  truncated `message: "The device cannot be identified."` but `resolve_identifier`
  emits the full "The device cannot be identified: supply a `device` or use a
  token that identifies a device." — a consumer reading the spec would expect
  different `message` bytes than the endpoint returns (the identical
  truncated-identifier-message drift class caught on subscription-status,
  media-streaming-rate, optimal-edge-discovery, and consent-info). Corrected the
  example to the real bytes and added
  `vendored_error_example_messages_match_the_handler`, which drives the endpoint
  and pins the exact `status`/`code`/`message` bytes of all six router-reachable
  inline example families whose messages are this API's own — 400
  `invalidArgument` ("Request body is not a valid VerifyFrequentLocationRequest."),
  400 `outOfRange` ("`latitude` must be in [-90, 90] and `longitude` in
  [-180, 180]."), 400 `postalCodeNotValid` ("Requested postal code value does not
  exist or is invalid."), 422 `missingIdentifier`, 422 `unnecessaryIdentifier`
  ("The device is already identified by the access token."), and 422
  `serviceNotApplicable` ("The service is not available for the provided
  identifier.") — so none can silently drift again. The 200 success examples
  (score 50 / 0) are illustrative single integers already consistent with the
  score mapping, and the 401/403/404/409/429/500/503 responses are `$ref`s to the
  shared catalog (covered by its own tests). This advances the vwip audit cursor
  to the next API (traffic-influence). Spec + test change (no behaviour/response
  change — the handler's output was already correct; only the spec example was
  wrong); no new dependency. `cargo test` green: 3070 passed, 0 failed (+1, the
  new pinning test). `cargo build --release` green; binary = 5,434,520 bytes
  (unchanged vs. the click-to-dial pass on rustc 1.97.0 — the fix touches only the
  spec and `#[cfg(test)]` code, not the release path). *Backlog banner still
  stands:* the net-new feature backlog remains exhausted and the human decision
  (authorise stateful infra, name an increment, or pause the routine) is still
  open — this is the sanctioned per-API vwip verification (backlog item 3), and
  this pass it caught a genuine, consumer-visible documentation defect.
- 2026-10-13 — Phase 5 vwip verification pass (click-to-dial). Audited the
  seventeenth vwip API in mount order (a stateful resource: an in-memory call
  `store` with a disjoint sink side-store, plus a `notifications` module firing
  create-time / terminate / simulated-progression `status-changed` CloudEvents).
  All **four** modelled operations are fully implemented, routed, and well-tested
  (~95 test fns before this pass): `createCall` (`POST /calls`, store-keyed — a
  deterministic `callId` from the pair, so a re-create of a still-live pair is
  `409 ALREADY_EXISTS`), the stateful `getCall` (`GET /calls/{callId}`),
  `terminateCall` (`DELETE /calls/{callId}`, single-use eviction that fires a
  terminal `disconnected` event), and `getRecording`
  (`GET /calls/{callId}/recording`, gated on the stored call's `recordingEnabled`).
  **Spec↔code drift found and fixed — the placeholder-UUID class** (the same
  consumer-visible drift the verified-caller / optimal-edge-discovery /
  iot-sim-fraud-prevention passes caught): the `created`, `found`, and `recording`
  200/201 examples all carried the Swagger placeholder `callId`
  `3fa85f64-5717-4562-b3fc-2c963f66afa6`, but the schema promises (and the handler
  delivers) a `callId` *deterministically derived* from the participant pair —
  `call_id` = SHA-256 over `caller` + `\0` + `callee`, v4-shaped. The documented
  `created` pair `+123456789111`/`+123456789012` actually yields
  `40c0c2be-6c32-41b9-bc58-e5cc1a25f8b3` and the `found` pair
  `+123456789111`/`+123456789222` yields `a2b3b31d-6b8e-49b0-8487-06f70764d7bd`
  (both confirmed by replicating the derivation and then by the new router test).
  Corrected all three example ids and tied the `recording` example to the `found`
  call's id (so it reads as the recording of that recorded call), and clarified in
  each example `summary` that the id is the deterministic value for that pair while
  `createdAt`/`generatedAt` stay illustrative (wall-clock). Added
  `vendored_examples_match_the_handler`, which (a) pins each documented pair →
  documented literal *directly* against `call_id` and (b) proves the real router
  emits `call_id(pair)` as the returned id and the fixed silent WAV as the
  recording `content` — driven on a pair unique to this test, because `createCall`
  is store-keyed and the documented pairs are already used (and left live) by the
  happy-path tests, so re-creating them through the router would `409`. Verified
  the rest of the spec is byte-accurate: every inline error-example `message` (400
  `INVALID_ARGUMENT` "Request body is not a valid CreateCallRequest.", 409
  `ALREADY_EXISTS`, the five 422 cases `INVALID_PHONE_NUMBER` / `SAME_CALLER_CALLEE`
  / `CALLER_NOT_AVAILABLE` / `CALLEE_NOT_AVAILABLE` / `RECORDING_NOT_SUPPORTED`,
  and both 404 `NOT_FOUND` messages) already matched the handler, as did the
  `recording` `content` base64 (the handler's `silent_wav`: a 60-byte 8 kHz 8-bit
  mono silent RIFF/WAVE, `audio/wav`) and the 409 `CONFLICT` / reserved-suffix
  plane. This advances the vwip audit cursor to the next API
  (most-frequent-location). Spec + test change (no behaviour/response change — the
  handler was already correct; only the three example ids were placeholders); no
  new dependency. `cargo test` green: 3069 passed, 0 failed (+1, the new pinning
  test). `cargo build --release` green; binary = 5,434,520 bytes (unchanged vs. the
  sponsored-data pass on rustc 1.97.0 — the net spec-text change and the new
  `#[cfg(test)]` code did not cross a 4 KiB page boundary; `#[cfg(test)]` is
  outside the release path). *Backlog banner still stands:* the net-new feature
  backlog remains exhausted and the human decision (authorise stateful infra, name
  an increment, or pause the routine) is still open — this is the sanctioned
  per-API vwip verification (backlog item 3), and this pass it caught a genuine,
  consumer-visible documentation defect.
- 2026-10-13 — Phase 5 vwip verification pass (sponsored-data). Audited the
  sixteenth vwip API in mount order (a stateful resource: an in-memory session
  `store` plus a `notifications` module firing the end-of-session webhook). All
  **seven** modelled operations across both facets are fully implemented, routed,
  and well-tested (73 test fns before this pass): the sponsorship lifecycle —
  `startSponsorship` (`POST /sponsorship`), `getSessionStatus`
  (`GET …/{sponsorId}/{campaignId}/{sessionId}/session-status`), `revokeSponsorship`
  (`DELETE …/revoke`, single-use eviction that fires the `session_revoked`
  webhook) — plus the campaign operations `getCampaignStatus`, `getActiveSponsorships`
  (both `GET /campaign/…`, reserved-suffix-on-the-campaignId-UUID + store scan),
  `configureAlerts` (`POST …/alert-subscription`, stateless ack) and `manageCampaign`
  (`POST /campaign/management`, stateless pause/resume ack). **Spec ↔ code found
  consistent — no value drift.** I verified every derived success example
  byte-for-byte against the handler: `getSessionStatus` consumption
  (`+123456789012` tail 12 on the 50 MB default → consumed 12 / available 38,
  `active`; `+123456789050` tail 50 → 50 / 0, `inactive` / `data_exhausted`, both
  matching `consumption` = `d mod (grant+1)`); `getCampaignStatus` state from the
  campaignId UUID's trailing three digits (`…000` → d 0 → active/prepaid,
  used 0 / remaining 1000; `…010` → d 10 → completed, `(d/6)%2=1` → `data_exhausted`,
  used 1000 / remaining 0; `…003` → d 3 → paused/postpaid, used 3), and the window
  anchoring (start = now−24h; ongoing end = now+24h; completed end = now−1h); and the
  fixed acknowledgement strings (`successful_revocation`, `Campaign notifications
  subscription - SUCCESS`, `CAMPAIGN PAUSED SUCCESSFULLY` / `… RESUMED …`,
  `status` `paused`/`resumed`). The minted `sessionId`s in the 201 / status / revoke
  / active-sponsorships examples are legitimately *illustrative* — `mint_session_id`
  derives them from a counter + clock (not the request), so pinning a concrete value
  would be wrong (the application-profiles / session-insights posture, not the
  deterministic-id posture of verified-caller / optimal-edge-discovery). **One
  genuine defect found and fixed — the stale "deferred to later work" module-doc
  trap** (identical class to device-data-volume's "`/check` is a later slice",
  qos-booking's "First slice: createBooking", application-profiles' "First slice:
  POST + GET", and session-insights' "in later passes", each of which had misdirected
  a pass toward re-implementing an existing endpoint): the parent doc
  `src/apis/sponsored_data.rs` described only `startSponsorship`/`getSessionStatus`
  as mounted and claimed the campaign-management operations
  `configureAlerts`/`manageCampaign` "remain deferred to later work", though all
  seven operations ship and are routed in `vwip.rs` — corrected to list the full
  set (lifecycle + campaign) as implemented and to describe the stateful store vs
  stateless campaign derivation accurately. **Small tested increment:** the spec's
  two inline 400 error examples (`startSponsorship` → `invalidArgument` /
  `outOfRange`) are the only error examples unique to this API — every other error
  response `$ref`s the shared catalog — and existing tests exercised only their
  `code`, never the `message` bytes; added
  `vendored_error_example_messages_match_the_handler`, which drives both through the
  real router and pins the exact `status`/`code`/`message`
  ("`phoneNumber` must be an E.164 number (e.g. +123456789012)." and "`dataVolume`
  must be between 1 and 1000 MB.") so they can never silently drift from the handler
  (the same drift class the media-streaming-rate / subscription-status passes
  closed). This advances the vwip audit cursor to the next API (click-to-dial).
  Docs + test change (no behaviour/response/spec change — the handlers and spec were
  already correct and mutually consistent; one module doc was stale and one
  inline-example family was unguarded); no new dependency. `cargo test` green: 3068
  passed, 0 failed (+1, the new pinning test). `cargo build --release` green;
  binary = 5,434,520 bytes (unchanged vs. the iot-sim-fraud-prevention pass on
  rustc 1.97.0 — the change touches only a module doc and `#[cfg(test)]` code,
  outside the release path). *Backlog banner still stands:* the net-new feature
  backlog remains exhausted and the human decision (authorise stateful infra, name
  an increment, or pause the routine) is still open — this is the sanctioned per-API
  vwip verification (backlog item 3), and this pass it caught a genuine
  documentation defect.
- 2026-10-13 — Phase 5 vwip verification pass (iot-sim-fraud-prevention). Audited the
  fifteenth vwip API in mount order (a stateful resource with two disjoint in-memory
  maps). All three modelled operations across both CAMARA facets — `bindDeviceImei`
  (`POST /bind`), `query` (`POST /query`), `unBindDeviceImei` (`POST /unbind`), each
  taking `IMEIBIND` and `AREALIMIT` — are fully implemented, routed, and well-tested
  (46 test fns before this pass), and the module docs agree with the code (no
  stale-slice trap: the parent doc already lists both facets as fully supported).
  **Spec↔code drift found and fixed, two distinct classes.** (1) *Deterministic
  success value* — the 200 `IMEI_BIND_BOUND` example carried `bindImei
  "352099000000117"`, which is not even a valid GSMA Luhn IMEI (I confirmed the Luhn
  sum fails), whereas the handler's `synth_imei` deterministically yields
  `352099000000113` for the documented `+123456789011` request (fixed TAC `35209900`
  + zero-padded serial `000011` + Luhn check digit `3`) — the same consumer-visible
  deterministic-id drift the verified-caller and optimal-edge-discovery passes caught
  (a value a consumer would reproduce from the input and the schema's "deterministic"
  promise). Corrected the example and tied it to its input. (2) *Unreconciled error
  messages* — every inline error example `message` had been copied verbatim from the
  upstream CAMARA IoTSIMFraudPrevention spec and never reconciled with this handler:
  the four 400 query examples ("At least one of phoneNumber, networkAccessIdentifier,
  ipv4Address and ipv6Address must be specified", "QueryType must be specified",
  "Invalid format: phoneNumber", "The value of queryType can only be IMEIBIND or
  AREALIMIT"), both 400 bind/unbind examples ("… can only be …", "bindType must be
  specified"), and both 422 `MISSING_IDENTIFIER` examples ("The device cannot be
  identified.") differed from the exact bytes the server returns, and the 422
  `UNNECESSARY_IDENTIFIER` example dropped the handler's trailing period — the
  truncated/diverging-identifier-message drift class caught on subscription-status /
  media-streaming-rate / optimal-edge-discovery / consent-info, here across a whole
  file. Corrected all of them to the handler's exact messages: `INVALID_ARGUMENT`
  "`device` must contain at least one identifier." / "`phoneNumber` must be in E.164
  format (e.g. +123456789)." / the per-operation "Request body is not a valid
  {Query,BindDeviceImei,UnBindDeviceImei}Request (…Type must be \"IMEIBIND\" or
  \"AREALIMIT\")." (collapsing each operation's two serde-failure examples — missing
  vs invalid type — into one, since the handler returns one message for both), and
  `MISSING_IDENTIFIER` "No `device` supplied and the access token identifies no
  device." Added `vendored_examples_match_the_handler`, which drives every documented
  request through the real router and pins the success `bindImei` (asserted both as
  the literal `352099000000113` and against `synth_imei(11)`, so a future derivation
  change fails the test rather than silently blessing a new value), the `AREALIMIT`
  `Circle` (against `synth_circle(11)` and the documented center −79.0/−169.0, radius
  11), and the exact `status`/`code`/`message` bytes of all router-reachable inline
  error families (query 400×3, query 422 MISSING/UNNECESSARY, bind 400×2, unbind 400,
  unbind 422 UNNECESSARY_UNBIND_IMEI/UNNECESSARY_UNBIND_AREALIMIT/MISSING) so none can
  silently drift again. The `AREA_LIMIT_RESTRICTED` circle example and the reserved-error
  suffix plane were already correct; the 401/403/404/429 responses are inline-but-canonical
  shared-shape examples left as the catalog's standard bytes. This advances the vwip
  audit cursor to the next API (sponsored-data). Spec + test change (no
  behaviour/response change — the handlers' output was already correct; only the spec
  examples were wrong); no new dependency. `cargo test` green: 3067 passed, 0 failed
  (+1, the new pinning test). `cargo build --release` green; binary = 5,434,520 bytes
  (+4,096 — one page — vs the prior fourteen passes' 5,430,424 on rustc 1.97.0). The
  bump is attributable: the vendored specs are embedded in the binary via
  `include_str!` (`src/registry.rs`) and served at runtime, so the net-longer spec
  example text crossed one 4 KiB page boundary; no code-size regression (only spec
  text and `#[cfg(test)]` code changed, and `#[cfg(test)]` is outside the release
  path). *Backlog banner still stands:* the net-new feature backlog remains exhausted
  and the human decision (authorise stateful infra, name an increment, or pause the
  routine) is still open — this is the sanctioned per-API vwip verification (backlog
  item 3), and this pass it caught a genuine, consumer-visible documentation defect.
- 2026-10-13 — Phase 5 vwip verification pass (consent-info). Audited the
  fourteenth vwip API in mount order. The single modelled operation
  `retrieveStatus` (`POST /consent-info/vwip/retrieve`) is fully implemented
  (`src/apis/consent_info/vwip.rs`), routed, and well-tested (22 test fns before
  this pass), and both module docs agree with the code: the phone-number-keyed
  identifier with the two-legged/three-legged resolution (shared with Subscription
  Status — `phoneNumber` valid only two-legged; resubmitted on a line token → 422
  `UNNECESSARY_IDENTIFIER`; absent under a non-line subject → 422
  `MISSING_IDENTIFIER`), the deterministic `d % 6` consent-state plane (`0` →
  valid/no reason/future expiry, `1` PENDING, `2` REQUESTED, `3` DENIED, `4`
  EXPIRED/past expiry, `5` OBJECTED), the `requestCaptureUrl` capture-URL gate
  (offered only when consent is not valid AND the caller asked), the two
  request-level 403s (`NOT_ALLOWED_SCOPES_PURPOSE` for a scope containing
  `forbidden`; `INVALID_CALLBACK_URL` for a malformed `callbackUrl`), and the
  reserved-error suffix plane all match the handler and the vendored
  `specs/consent-info/vwip/openapi.yaml`. **Spec↔code drift found and fixed in the
  vendored 422 `missingIdentifier` example:** it carried the truncated
  `message: "The phone number cannot be identified."` but `resolve_identifier`
  emits the fuller "The phone number cannot be identified: supply `phoneNumber` or
  use a token that identifies a line." — a consumer reading the spec would expect
  different `message` bytes than the endpoint returns (the identical
  truncated-identifier-message drift class caught on subscription-status,
  media-streaming-rate, and optimal-edge-discovery). Corrected the example to the
  real bytes and added `vendored_error_example_messages_match_the_handler`, which
  drives the endpoint and pins the exact `status`/`code`/`message` bytes of the six
  router-reachable inline example families whose messages are this API's own —
  400 `invalidArgument` ("`purpose` must match the pattern `^dpv:[a-zA-Z0-9]+$`."),
  403 `notAllowedScopesPurpose`, 403 `invalidCallbackUrl`, 422 `missingIdentifier`,
  422 `unnecessaryIdentifier`, and the reserved-suffix 422 `serviceNotApplicable` —
  so they can never silently drift again. Documented why the 403 `permissionDenied`
  example is *not* pinned: it intentionally shows the shared catalog's canonical
  message ("Client does not have sufficient permissions to perform this action.",
  the same literal in `specs/shared/errors.yaml` and sibling specs), while the auth
  layer's `require_scope` runtime message is scope-specific ("Access token does not
  have the required scope '…'."), so that family is covered by the shared catalog's
  own tests (the 401/404/409/429/500/503 responses are `$ref`s likewise). Spot-checked
  the rest of the spec and found no other drift: the three 200 success examples
  (`valid`/`pendingWithCapture`/`expired`) illustrate the `d % 6` plane with
  wall-clock-dependent expiry/capture values that are legitimately illustrative
  (not request-reproducible), and the request/response/error schemas agree with the
  Rust types. This advances the vwip audit cursor to the next API
  (iot-sim-fraud-prevention). Spec + test change (no behaviour/response change — the
  handler's output was already correct; only the spec example was wrong); no new
  dependency. `cargo test` green: 3066 passed, 0 failed (+1, the new pinning test).
  `cargo build --release` green; binary = 5,430,424 bytes (unchanged vs. the prior
  thirteen passes on rustc 1.97.0 — the fix touches only the spec and `#[cfg(test)]`
  code, not the release path). *Backlog banner still stands:* the net-new feature
  backlog remains exhausted and the human decision (authorise stateful infra, name
  an increment, or pause the routine) is still open — this is the sanctioned per-API
  vwip verification (backlog item 3), and this pass it caught a genuine
  documentation defect.
- 2026-10-13 — Phase 5 vwip verification pass (session-insights). Audited the
  thirteenth vwip API in mount order (the second stateful vwip resource, after
  application-profiles). All **five** modelled operations — `createSession` (`POST
  /session-insights/vwip/sessions`), `getSession` (`GET …/sessions/{sessionId}`),
  `deleteSession` (`DELETE …/sessions/{sessionId}`), `retrieveSessionsByDevice`
  (`POST …/retrieve-sessions`), and `sendSessionMetrics` (`POST
  …/sessions/{sessionId}/metrics`) — are fully implemented
  (`src/apis/session_insights/vwip.rs` + an in-memory `store.rs` and a
  `notifications.rs` delivering the terminal `session-ended` CloudEvent via
  fire-and-forget async timers), routed, and well-tested (87 test fns before this
  pass), and the vendored `specs/session-insights/vwip/openapi.yaml` declares the
  same five operations. **Spec ↔ code found consistent** (no response/behaviour
  drift): the two 201 success examples' minted `id`s are legitimately *illustrative*
  — `store::new_session_id` derives them from a counter/clock, not the request body,
  so a consumer cannot reproduce them (the application-profiles posture, *not* the
  deterministic-id posture of verified-caller / optimal-edge-discovery, so pinning a
  concrete id here would be wrong); the time-bounded (`expiresAt` present) vs
  `…000`-tail open-ended (no `expiresAt`) split matches `create_session`'s
  `open_ended = tail ∈ {None, Some(0)}`; and every inline error-example `message`
  byte-matches the handler. **One genuine defect found and fixed — the stale
  "in later passes" module-doc trap** (identical to device-data-volume's "`/check`
  is a later slice", qos-booking's "First slice: createBooking", and
  application-profiles' "First slice: POST + GET", each of which had misdirected a
  pass toward re-implementing an existing endpoint): the parent doc
  `src/apis/session_insights.rs` said the resource was addressed by its id "`GET
  /sessions/{sessionId}`, and — in later passes — `DELETE`, `retrieve-sessions`,
  and `metrics`", though all three ship and are tested — corrected to list all five
  operations as implemented. **Small tested increment:** the spec's inline error
  examples (those not `$ref`ing the shared catalog) were exercised by existing tests
  only for `code`, never their `message` bytes; added
  `vendored_error_example_messages_match_the_handler` driving the four
  router-reachable families through the real router and pinning the exact
  `status`/`code`/`message` bytes — createSession 400 `invalidArgument`
  ("`sink` is required."), createSession 422 `serviceNotApplicable`, sendSessionMetrics
  400 `invalidArgument` ("`packetLossErrorRate` is required."), and sendSessionMetrics
  400 `outOfRange` ("`packetLossErrorRate` must be between 1 and 10.") — so they can
  never silently drift (the 401/403/404/409/429/500/503 responses are `$ref`s covered
  by the shared catalog's own tests). Documented that the createSession 422
  `missingIdentifier` example is *not* router-reachable: its `resolve_identifier`
  branch fires only for a token with an empty `sub`, which the token endpoint never
  issues (a minted `client_credentials` token always carries a non-empty subject),
  and `Claims` cannot be constructed from outside the auth module (`raw` is private),
  so its message literal (which matches the single source in `resolve_identifier`) is
  left to its handler branch. This advances the vwip audit cursor to the next API
  (consent-info). Docs + test change (no behaviour/response/spec change — the handlers
  and spec were already correct and mutually consistent; one module doc was stale and
  one inline-example family was unguarded); no new dependency. `cargo test` green:
  3065 passed, 0 failed (+1, the new pinning test). `cargo build --release` green;
  binary = 5,430,424 bytes (unchanged vs. the prior twelve passes on rustc 1.97.0 —
  the change touches only a module doc and `#[cfg(test)]` code, outside the release
  path). *Backlog banner still stands:* the net-new feature backlog remains exhausted
  and the human decision (authorise stateful infra, name an increment, or pause the
  routine) is still open — this is the sanctioned per-API vwip verification (backlog
  item 3).
- 2026-10-13 — Phase 5 vwip verification pass (device-authenticity). Audited the
  twelfth vwip API in mount order. The single modelled operation `checkImeiStatus`
  (`POST /device-authenticity/vwip/check-status`) is fully implemented
  (`src/apis/device_authenticity/vwip.rs`), routed, and well-tested (14 test fns
  before this pass), and both module docs (`device_authenticity.rs` parent +
  `vwip.rs`) agree with the code: the IMEI-keyed identifier (required 15-digit
  `imei`, no two-legged/three-legged token-subject fallback — the device is named
  explicitly), the deterministic operational-status plane (IMEI trailing three
  digits `d` → status by `d % 9` over the nine-value enum
  `allowed`/`lost`/`stolen`/`blacklisted`/`blocked`/`fraud`/`non-payment`/
  `regulatory`/`unknown`; `allowed` carries no `reportedDate`, an adverse status
  carries `reportedDate = now − d h`), and the reserved-error suffix plane all
  match the handler and the vendored `specs/device-authenticity/vwip/openapi.yaml`.
  **No spec↔code↔doc drift found** (like device-visit-location and
  network-health-assessment, this one is clean): the two 200 examples
  (`…000`→`allowed` no `reportedDate`; `…002`→`stolen`, `lastChecked 10:41:38Z` /
  `reportedDate 08:41:38Z` = exactly `now − 2 h`, internally consistent) match the
  mapping and are already pinned by `zero_tail_is_the_allowed_default_without_a_reported_date`,
  `trailing_digits_select_each_status_in_order`, and
  `an_adverse_status_carries_a_reported_date`; the 400 `invalidArgument` example
  message byte-matches the handler's invalid-`imei` message; the 422
  `serviceNotApplicable` example message byte-matches `CamaraError::for_status(422)`;
  the reserved-suffix set (400/401/403/404/409/422/429/500/503) equals
  `scenarios::RESERVED_ERROR_SUFFIXES`. Confirmed the `DeviceAuthenticityError`
  schema's `IDENTIFIER_NOT_FOUND`/`QUOTA_EXCEEDED` enum members are the canonical
  API's own codes, intentionally superseded in CamaraSim by the shared
  reserved-error convention (`…404`→canonical `NOT_FOUND`, `…429`→`TOO_MANY_REQUESTS`,
  as the module docs and existing `reserved_suffix_selects_a_canonical_camara_error`
  test state) — a documented divergence, not drift. **Small tested increment:** the
  two *inline* error examples whose `message` is not a `$ref` into the shared
  catalog (400 `invalidArgument`, 422 `serviceNotApplicable`) were exercised by
  existing tests only for their `code`, never their `message` bytes; added
  `vendored_error_example_messages_match_the_handler` driving both through the
  router and pinning the exact `status`/`code`/`message` bytes so the spec examples
  can never silently drift from the handler (the same drift class the
  media-streaming-rate / network-health-assessment / application-profiles /
  subscription-status passes closed). This advances the vwip audit cursor to the
  next API (session-insights). Test + docs change (no behaviour/response/spec change
  — the handler and spec were already correct and mutually consistent; one inline
  example family was simply unguarded); no new dependency. `cargo test` green: 3064
  passed, 0 failed (+1, the new pinning test). `cargo build --release` green;
  binary = 5,430,424 bytes (unchanged vs. the prior eleven passes on rustc 1.97.0 —
  the new test is `#[cfg(test)]`, outside the release path). *Backlog banner still
  stands:* the net-new feature backlog remains exhausted and the human decision
  (authorise stateful infra, name an increment, or pause the routine) is still open
  — this is the sanctioned per-API vwip verification (backlog item 3).
- 2026-10-13 — Phase 5 vwip verification pass (subscription-status). Audited the
  eleventh vwip API in mount order. The single modelled operation
  `retrieveSubscriptionStatus` (`POST
  /subscription-status/vwip/retrieve-subscription-status`) is fully implemented
  (`src/apis/subscription_status/vwip.rs`), routed, and well-tested (16 test fns
  before this pass), and both module docs agree with the code: the deterministic
  status-bitfield plane (identifier trailing digits `d` → bit 0 `voiceSmsIn`
  suspended, bit 1 `voiceSmsOut` suspended, `(d>>2)%3` `dataService`
  active/suspended/throttled), the reserved-error suffix plane, and the
  two-legged/three-legged identifier resolution (`phoneNumber` only valid
  two-legged; resubmitted on a line token → 422 `UNNECESSARY_IDENTIFIER`; absent
  under a non-line subject → 422 `MISSING_IDENTIFIER`) all match the handler.
  **Spec↔code drift found and fixed in the vendored 422 `missingIdentifier`
  example** (`specs/subscription-status/vwip/openapi.yaml`): it carried the
  truncated `message: "The phone number cannot be identified."` but
  `resolve_identifier` emits the fuller "The phone number cannot be identified:
  supply `phoneNumber` or use a token that identifies a line." — a consumer
  reading the spec would expect different `message` bytes than the endpoint
  returns (the identical truncated-identifier-message drift class caught on
  media-streaming-rate and optimal-edge-discovery). Corrected the example to the
  real bytes and added `vendored_error_example_messages_match_the_handler`, which
  drives the endpoint and pins the exact `message` bytes of all four inline
  example families unique to this spec — 400 `invalidArgument`, 422
  `missingIdentifier`, 422 `unnecessaryIdentifier`, and the reserved-suffix 422
  `serviceNotApplicable` — so they can never silently drift again (the
  401/403/404/409/429/500/503 responses are `$ref`s into the shared catalog,
  covered by its own tests). Spot-checked the rest of the spec and found no other
  drift: the three 200 success examples (`…000` all-active, `…001` inbound
  suspended, `…008` data throttled) match the bitfield mapping and are already
  pinned by `all_active_healthy_default_for_zero_tail` /
  `each_field_is_independently_controllable` / `data_service_covers_all_three_states`,
  and the request/response/error schemas agree with the Rust types. This advances
  the vwip audit cursor to the next API (device-authenticity). Spec + test change
  (no behaviour/response change — the handler's output was already correct; only
  the spec example was wrong); no new dependency. `cargo test` green: 3063 passed,
  0 failed (+1, the new pinning test). `cargo build --release` green; binary =
  5,430,424 bytes (unchanged vs. the prior ten passes on rustc 1.97.0 — the fix
  touches only the spec and `#[cfg(test)]` code, not the release path). *Backlog
  banner still stands:* the net-new feature backlog remains exhausted and the
  human decision (authorise stateful infra, name an increment, or pause the
  routine) is still open — this is the sanctioned per-API vwip verification
  (backlog item 3), and this pass it caught a genuine documentation defect.
- 2026-10-12 — Phase 5 vwip verification pass (application-profiles). Audited the
  tenth vwip API in mount order (first of the stateful `vwip` resources). All four
  modelled CRUD operations — `createApplicationProfile` (`POST
  /application-profiles/vwip/application-profiles`), `readApplicationProfile` (`GET
  …/{applicationProfileId}`), `updateApplicationProfile` (`PATCH …`, full-set
  replacement), and `deleteApplicationProfile` (`DELETE …`, single-use eviction) —
  are fully implemented (`src/apis/application_profiles/vwip.rs` + an in-memory
  `store.rs` with `insert`/`get`/`replace`/`remove`), routed, and well-tested (35
  test fns before this pass), and the vendored
  `specs/application-profiles/vwip/openapi.yaml` is consistent with the code: the
  full CRUD set, per-operation scopes (`application-profiles:{create,read,update,
  delete}`), the `anyOf`/`minProperties`/range validation driving
  `400 INVALID_ARGUMENT` / `400 OUT_OF_RANGE`, and the store-keyed `200`/`404`/`204`
  planes all match the handlers. **Checked the placeholder-UUID drift class that
  bit verified-caller and optimal-edge-discovery and found it does NOT apply here:**
  the 201 `created` example's `applicationProfileId`
  (`3fa85f64-5717-4562-b3fc-2c963f66afa6`) is legitimately *illustrative* because
  `store::mint_uuid` derives the id from an internal monotonic counter + wall clock
  (not from the request body), so no consumer can reproduce it — unlike those two
  APIs' SHA-256-over-request ids, pinning a concrete value here would be wrong.
  **Two genuine defects found and fixed, both the stale "first slice / later
  passes" module-doc trap** (identical to device-data-volume's "`/check` is a later
  slice" and qos-booking's "First slice: createBooking", which had each misdirected
  a pass toward re-implementing an existing endpoint): (1) the parent doc
  `src/apis/application_profiles.rs` said "First slice: `POST /application-profiles`
  + `GET /application-profiles/{id}`", omitting the shipped PATCH + DELETE —
  corrected to the full CRUD set; (2) the `store.rs` module doc said `PATCH` /
  `DELETE` were "in later passes" though `replace`/`remove` are defined and used in
  that very file — corrected to state read/update/delete are all served now.
  **Small tested increment:** the four distinct `message` strings in the vendored
  spec's 400 examples (create `anyOf` + `outOfRange`, read/delete malformed-id,
  patch `outOfRange`) were exercised by existing tests only for their `code`, never
  their `message` bytes; added `vendored_error_example_messages_match_the_handler`
  driving all four through the router and pinning the exact message bytes so the
  spec examples can never silently drift from the handler (the same drift class the
  media-streaming-rate / network-health-assessment passes closed retroactively).
  This advances the vwip audit cursor to the next API (subscription-status).
  Docs + test change (no behaviour/response/spec change — the handlers and spec were
  already correct and mutually consistent; only two module docs were stale and one
  spec-example family was unguarded); no new dependency. `cargo test` green: 3062
  passed, 0 failed (+1, the new pinning test). `cargo build --release` green; binary
  = 5,430,424 bytes (unchanged vs. the prior nine passes on rustc 1.97.0 — the
  change touches only module docs and `#[cfg(test)]` code, not the release path).
  *Backlog banner still stands:* the net-new feature backlog remains exhausted and
  the human decision (authorise stateful infra, name an increment, or pause the
  routine) is still open — this is the sanctioned per-API vwip verification (backlog
  item 3), and this pass it caught two genuine documentation defects.
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
