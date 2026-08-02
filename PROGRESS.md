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

**SIM Swap v2** has begun. `POST /check` is live at `/sim-swap/v2/check`
(scope `sim-swap:check`): the identifier is the submitted `phoneNumber` or —
when omitted — the token subject (three-legged fallback). Reserved error suffix
→ canonical CAMARA error; otherwise the identifier's trailing three digits encode
**hours since the last swap** and `swapped = hoursAgo < maxAge`, making `maxAge`
(1–2400, default 240) a real second control plane; out-of-range `maxAge` → 400
`OUT_OF_RANGE`. `x-correlator` echoed on every response.

SIM Swap v2 is now complete: `POST /check` **and** `POST /retrieve-date`
(scope `sim-swap:retrieve-date`). Retrieve-date reads the **same** swap history
as check — the identifier's trailing three digits are hours-since-last-swap — and
reports `latestSimChange` = *now − hoursAgo hours* (RFC 3339 UTC) when that swap
is inside a fixed 240 h (10-day) monitored period, else `null`; `monitoredPeriod`
(10 days) is always returned. The 240 h boundary lines up with check's default
`maxAge`, so `+123456789012` yields a date and `+123456789365` yields `null`. A
self-contained civil-date formatter (no new dependency) renders the timestamp.

**KYC Match** is now live at `/kyc-match/v0.3/match` (CAMARA KYC Match 0.3.0 —
the API's real published version; the backlog's "v1" was loose, so it is mounted
under its canonical `v0.3` URL). Scope `kyc-match:match`. The caller submits
identity attributes (name/address/birthdate/email/…) and the response carries a
`<attribute>Match` verdict per submitted attribute — the CAMARA strings
`"true"`/`"false"`/`"not_available"` — plus a `<attribute>MatchScore` (fixed 90)
for score-bearing attributes when the verdict is `"false"`. Two control planes
(DESIGN §7): a top-level reserved error suffix on the identifier (submitted
`phoneNumber`, else token subject) → canonical CAMARA error; and a per-attribute
verdict chosen from each attribute's own value (`unavailable`→not_available,
`nomatch`→false, else true). At least one non-`phoneNumber` attribute is
required → 400 `KNOW_YOUR_CUSTOMER.INVALID_PARAM_COMBINATION`.

This completes Phase 1 (stateless, non-spatial).

**Phase 2 has begun.** The former combined CAMARA *Device Status* API was split
(Spring25) into separate **Device Reachability Status** and **Device Roaming
Status** APIs, so — like KYC Match's `v0.3` — CamaraSim mounts the real published
APIs rather than a loose "device-status v1". **Device Reachability Status v1**
`POST /retrieve` is live at `/device-reachability-status/v1/retrieve` (scope
`device-reachability-status:read`, operationId `getReachabilityStatus`): the
identifier is the submitted `device` (phoneNumber, else networkAccessIdentifier,
else the IPv4 `publicAddress`, else ipv6Address) or — when omitted — the token
subject. Reserved error suffix → canonical CAMARA error; `…000` → not reachable
(empty `connectivity`); odd trailing digits → reachable over SMS only; anything
else → reachable over `["DATA","SMS"]`. `x-correlator` echoed on every response.

**Device Roaming Status v1** `POST /retrieve` is now live at
`/device-roaming-status/v1/retrieve` (scope `device-roaming-status:read`,
operationId `getRoamingStatus`) — the "roaming" half of the split. Same
identifier resolution as reachability (submitted `device` id, else token
subject). Reserved error suffix → canonical CAMARA error; a `…000` tail (and a
no-digit subject) → `{roaming:false}` (home network); any other numeric tail →
`{roaming:true, countryCode, countryName}`, where the visited country is a fixed
6-entry MCC/ISO-3166 table indexed by `digits % 6`, making the country a second
control plane. `x-correlator` echoed on every response.

**Device Identifier v0.3** has begun. `POST /retrieve-type` is live at
`/device-identifier/v0.3/retrieve-type` (scope `device-identifier:retrieve-type`,
operationId `retrieveType`). Mounted under the API's real published version
(CAMARA 0.3.0, release r2.2 — it has never reached 1.0.0), mirroring KYC Match's
`v0.3`. Same identifier resolution as the device-status APIs (submitted `device`
id, else token subject). Reserved error suffix → canonical CAMARA error;
otherwise the identifier's trailing three digits pick a device type from a fixed
6-entry `(TAC, manufacturer, model)` table (`digits % 6`; `…000`/no-digits →
default Apple entry), so the reported model is a second control plane. The
response echoes the `device` identifier used (single-property `DeviceResponse`)
and a current `lastChecked`. `x-correlator` echoed on every response.

Device Identifier v0.3 `POST /retrieve-identifier` is now live too
(`device-identifier:retrieve-identifier`, operationId `retrieveIdentifier`).
Same identifier resolution and reserved-error convention as `retrieve-type`;
on a happy path it returns the full device identity — a synthesised `imei`
(15-digit: the selected type's TAC + a 6-digit serial = zero-padded trailing
three digits + a GSMA Luhn check digit) and `imeisv` (16-digit: TAC + serial +
fixed `"00"` software version) — alongside `tac`/`manufacturer`/`model`, so both
the model and the IMEI are deterministic from the input.
**Next up:** Device Identifier `POST /retrieve-ppid`.

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
- [x] SIM Swap v2 — [x] `POST /check` · [x] `POST /retrieve-date`
- [x] KYC Match v0.3 — `POST /match` (mounted `/kyc-match/v0.3`; CAMARA 0.3.0)

### Phase 2 — Stateless device queries
- [~] Device Status — reachability, roaming (canonical split, DESIGN §9):
  - [x] Device Reachability Status v1 — `POST /retrieve` (`/device-reachability-status/v1`)
  - [x] Device Roaming Status v1 — `POST /retrieve` (`/device-roaming-status/v1`; CAMARA 1.0.0)
- [~] Device Identifier v0.3 (`/device-identifier/v0.3`; CAMARA 0.3.0, release r2.2):
  - [x] `POST /retrieve-type` (`device-identifier:retrieve-type`)
  - [x] `POST /retrieve-identifier` (`device-identifier:retrieve-identifier`)
  - [ ] `POST /retrieve-ppid` (`device-identifier:retrieve-ppid`)

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

- 2026-08-02 — Phase 2: Device Identifier v0.3 — `POST /retrieve-identifier` (CAMARA Device
  Identifier 0.3.0, r2.2), operationId `retrieveIdentifier`, scope
  `device-identifier:retrieve-identifier`. Added the route + handler to
  `src/apis/device_identifier/v0_3.rs`, reusing the module's existing request body
  (`RequestBody`/`Device`), identifier resolution (`resolve_identifier`), `device_type`
  table, correlator/E.164/rfc3339 helpers — no new files, no new deps. Same identifier
  resolution and reserved-error convention as retrieve-type (§7): identifier = first present
  `device` id [phoneNumber E.164-validated, else NAI, else IPv4 publicAddress, else
  ipv6Address], else token subject → 422 MISSING_IDENTIFIER; empty `device{}`/bad phone → 400;
  unknown field → 400; reserved suffix → canonical CAMARA error. Happy path returns the full
  device identity: `imei` (15-digit = the selected type's TAC(8) + serial(6, the zero-padded
  trailing three digits) + a GSMA **Luhn** check digit) and `imeisv` (16-digit = TAC + serial +
  fixed "00" SVN), alongside `tac`/`manufacturer`/`model` from the shared 6-entry DEVICE_TYPES
  table (`digits % 6`; …000/no-digits → default Apple), so both the model and the IMEI are a
  control plane. Echoes the `device` used (single-property `DeviceResponse`) and current
  `lastChecked`; `x-correlator` echoed on all responses. New `luhn_check_digit`/`imei_from`/
  `serial_from` helpers; Luhn verified against the canonical 490154203237518 IMEI. Spec: added
  `/retrieve-identifier` path + `IdentifierResponse` schema (imei `^[0-9]{15}$`, imeisv
  `^[0-9]{16}$`) to `specs/device-identifier/v0.3/openapi.yaml`, `$ref`-ing shared `errors.yaml`
  + auth `camaraOAuth`, with `x-camarasim-scenarios` documenting the cases (full shared error
  set exposed, noted vs canonical 400/401/403/404/422/429; optional device `name` not modelled;
  retrieve-ppid still deferred so served spec matches code). 237 tests green (was 222; +15: 4
  units [luhn-known-imei/imei-15-digit-luhn-valid/serial-default/serial-…001] + 11 integration
  covering identity+echo/different-tail/default-tail/reserved-error/non-phone-ids/bad-phone+
  empty-device/subject-fallback/subject-reserved-error/non-numeric-subject/scope [retrieve-type
  token rejected]/auth/x-correlator). — binary: 1026K (1051056 B; +4872 B)
- 2026-08-02 — Phase 2 (Device Identifier begun): Device Identifier v0.3 — `POST /retrieve-type`
  (CAMARA Device Identifier 0.3.0, the API version carried by the latest public release r2.2;
  never reached 1.0.0, so mounted under real `v0.3` like KYC Match — DESIGN §9). New
  `src/apis/device_identifier/{,v0_3}.rs` mounted at `/device-identifier/v0.3/retrieve-type`,
  merged into the app router; `/` catalog now lists device-identifier v0.3. Protected by the
  `Claims` extractor (scope `device-identifier:retrieve-type`, operationId `retrieveType`,
  confirmed against the r2.2 upstream spec). Body `RequestBody{device?}` parsed as `Bytes` with
  `deny_unknown_fields` (nested `Device`/`DeviceIpv4Addr` too) → precise `INVALID_ARGUMENT`;
  empty body allowed. Identifier resolution mirrors the device-status APIs (first present device
  id [phoneNumber E.164-validated, else NAI, else IPv4 publicAddress, else ipv6Address], else
  token subject → 422 MISSING_IDENTIFIER; empty `device{}` → 400). Functional cases (§7) from the
  identifier's trailing three digits: reserved suffix → canonical CAMARA error; otherwise the
  reported device type = `DEVICE_TYPES[digits % 6]` over a fixed 6-entry `(TAC, manufacturer,
  model)` table [(35692005,Apple,iPhone 15 Pro),(35847104,OnePlus,OnePlus 12),(35438509,Google,
  Pixel 8 Pro),(86234502,Xiaomi,Redmi Note 13),(35315106,Samsung,Galaxy S24 Ultra),(35201607,
  Motorola,Edge 50)] with `…000`/no-digits → default (Apple), so the model is a second control
  plane; every TAC is 8 digits (CAMARA `^[0-9]{8}$`). Response carries `lastChecked` (current
  time, RFC 3339 UTC via a self-contained `rfc3339_utc`/`civil_from_days` like sim_swap — no
  date/time dep), `tac`/`manufacturer`/`model`, and echoes the `device` identifier used as a
  single-property `DeviceResponse` (subject echoed as phoneNumber only when E.164). `x-correlator`
  echoed on all responses. No new deps. Spec: new `specs/device-identifier/v0.3/openapi.yaml` —
  vendored 0.3.0 `POST /retrieve-type` + `RequestBody`/`Device`/`DeviceIpv4Addr`/`DeviceResponse`/
  `TypeResponse` schemas, `$ref`-ing shared `errors.yaml` + auth `camaraOAuth`, with
  `x-camarasim-scenarios` documenting the cases (full shared error set exposed, noted vs canonical
  400/401/403/404/422/429; reserved suffixes use generic Commonalities codes; 409/500/503 are
  CamaraSim extensions; retrieve-identifier/retrieve-ppid deferred so served spec matches code).
  222 tests green (was 204; +18: 4 units [device-type/TAC-pattern/E.164/rfc3339] + 14 integration
  covering type/different-type/default-tail/reserved-error/non-phone-ids-echoed/bad-phone/
  empty-device/unknown-field/subject-fallback/subject-reserved-error/non-numeric-subject-no-echo/
  scope/auth/x-correlator). — binary: 1022K (1046184 B; +11128 B)
- 2026-08-02 — Phase 2: Device Roaming Status v1 — `POST /retrieve` (CAMARA Device Roaming
  Status 1.0.0), the "roaming" half of the Spring25 Device Status split. New
  `src/apis/device_roaming_status/{,v1}.rs` mounted at `/device-roaming-status/v1/retrieve`,
  merged into the app router; `/` catalog now lists device-roaming-status v1. Protected by the
  `Claims` extractor (scope `device-roaming-status:read`, operationId `getRoamingStatus`,
  confirmed against the 1.0.0 upstream spec). Body `RoamingStatusRequest{device?}` parsed as
  `Bytes` with `deny_unknown_fields` (nested `Device`/`DeviceIpv4Addr` too) → precise
  `INVALID_ARGUMENT`; empty body allowed. Identifier resolution mirrors reachability (first
  present device id [phoneNumber E.164-validated, else NAI, else IPv4 publicAddress, else
  ipv6Address], else token subject → 422 MISSING_IDENTIFIER; empty `device{}` → 400). Functional
  cases (§7) from the identifier's trailing three digits: reserved suffix → canonical CAMARA
  error; `…000` and a no-digit subject → `{roaming:false}` (home); any other numeric tail →
  `{roaming:true, countryCode, countryName}` with the visited country = fixed 6-entry MCC/ISO-3166
  table `[(262,DE),(234,GB),(208,FR),(310,US),(440,JP),(505,AU)]` indexed by `digits % 6`, so the
  country is a genuine second control plane (e.g. `…012`→Germany, `…011`→Australia). Optional
  `lastStatusTime` not modelled (omitted). `x-correlator` echoed on all responses. No new deps
  (reuses shared `scenarios`/`errors`, local E.164 validator). Spec: new
  `specs/device-roaming-status/v1/openapi.yaml` — vendored 1.0.0 `POST /retrieve` +
  `RoamingStatusRequest`/`Device`/`DeviceIpv4Addr`/`RoamingStatusResponse` schemas, `$ref`-ing
  shared `errors.yaml` + auth `camaraOAuth`, with `x-camarasim-scenarios` documenting the cases
  (full shared error set exposed, noted vs canonical 400/401/403/404/422/429/503; three-legged
  IDENTIFIER_MISMATCH/UNNECESSARY_IDENTIFIER not modelled). 204 tests green (was 187; +17: 3 units
  [roaming/E.164/device-precedence] + 14 integration covering roaming-country/different-country/
  not-roaming/reserved-error/non-phone-ids/bad-phone/empty-device/unknown-field/subject-fallback/
  subject-reserved-error/non-numeric-subject/scope/auth/x-correlator + catalog). — binary: 1012K
  (1035056 B; +7304 B)
- 2026-08-02 — Phase 2 (begun): Device Reachability Status v1 — `POST /retrieve` (CAMARA
  Device Reachability Status 1.0.0). The former combined *Device Status* API was split in
  Spring25 into Device Reachability Status + Device Roaming Status, so — like KYC Match's
  `v0.3` — mounted under the real published API/version rather than a loose "device-status v1"
  (DESIGN §9). New `src/apis/device_reachability_status/{,v1}.rs` mounted at
  `/device-reachability-status/v1/retrieve`, merged into the app router; `/` catalog now lists
  device-reachability-status v1. Protected by the `Claims` extractor (scope
  `device-reachability-status:read`, operationId `getReachabilityStatus`, confirmed against the
  1.0.0 upstream spec). Body `RequestReachabilityStatus{device?}` parsed as `Bytes` with
  `deny_unknown_fields` (nested `Device`/`DeviceIpv4Addr` too) → precise `INVALID_ARGUMENT` on
  malformed/unknown-field input; empty body allowed (device optional). Identifier = first present
  device id (phoneNumber [E.164-validated], else networkAccessIdentifier, else IPv4 publicAddress,
  else ipv6Address) or, when no device, the token subject (three-legged fallback → 422
  MISSING_IDENTIFIER if absent); empty `device{}` → 400. Functional cases (§7) from the
  identifier's trailing three digits: reserved suffix → canonical CAMARA error; `…000` →
  `{reachable:false, connectivity:[]}`; odd → `{reachable:true, connectivity:["SMS"]}`; else →
  `{reachable:true, connectivity:["DATA","SMS"]}` (happy-path default, incl. no-digit subject).
  `x-correlator` echoed on all responses. No new deps (reuses shared `scenarios`/`errors`, local
  E.164 validator like NV/SIM-Swap). Spec: new `specs/device-reachability-status/v1/openapi.yaml`
  — vendored 1.0.0 `POST /retrieve` + `RequestReachabilityStatus`/`Device`/`DeviceIpv4Addr`/
  `ReachabilityStatusResponse` schemas, `$ref`-ing shared `errors.yaml` + auth `camaraOAuth`, with
  `x-camarasim-scenarios` documenting the cases (full shared error set exposed, noted vs canonical
  400/401/403/404/422/429/503; three-legged IDENTIFIER_MISMATCH/UNNECESSARY_IDENTIFIER not
  modelled). 187 tests green (was 170; +17: 3 units [reachability/E.164/device-precedence] + 14
  integration covering data+SMS/SMS-only/not-reachable/reserved-error/non-phone-ids/bad-phone/
  empty-device/unknown-field/subject-fallback/subject-reserved-error/non-numeric-subject/scope/
  auth/x-correlator). — binary: 1004K (1027752 B; +14936 B)
- 2026-08-02 — Phase 1 (complete): KYC Match — `POST /match` (CAMARA Know Your Customer Match
  0.3.0). New `src/apis/kyc_match/{,v0_3}.rs` mounted at `/kyc-match/v0.3/match`, merged into the
  app router; `/` catalog now lists kyc-match v0.3. Mounted under the API's real published version
  `v0.3` (canonical URL, DESIGN §9) rather than the backlog's loose "v1" — KYC Match has never
  reached 1.0.0. Protected by the `Claims` extractor (scope `kyc-match:match`, confirmed against
  upstream). Body `KYC_MatchRequestBody` (20 optional string attributes) parsed via serde with
  `deny_unknown_fields` for precise `INVALID_ARGUMENT` on malformed/unknown-field/non-string input;
  bad `phoneNumber` E.164 → 400; empty body → 400; only `phoneNumber` (no other attribute) → 400
  `KNOW_YOUR_CUSTOMER.INVALID_PARAM_COMBINATION` (the API-specific code). Functional cases (§7),
  two control planes: (1) top-level reserved error suffix on the identifier (submitted `phoneNumber`,
  else token subject) → canonical CAMARA error (validation first, so the body must be well-formed
  for it to surface); (2) per-attribute verdict from each attribute's OWN value — `unavailable`
  →`not_available`, `nomatch`→`false` (+ fixed `MatchScore` 90 for the 13 score-bearing attributes),
  else `true`; verdicts are the CAMARA enum STRINGS `"true"/"false"/"not_available"`. Only submitted
  attributes appear in the response; `phoneNumber` scopes but has no verdict. Table-driven field
  mapping (19 match keys, 13 with scores) keeps it compact. `x-correlator` echoed on all responses.
  No new deps (reuses shared `scenarios`/`errors`, local E.164 validator like NV/SIM-Swap). Spec:
  new `specs/kyc-match/v0.3/openapi.yaml` — vendored 0.3.0 `POST /match` + `KYC_MatchRequestBody`/
  `KYC_MatchResponse`/`MatchResult`/`MatchScoreResult` schemas, `$ref`-ing shared `errors.yaml` +
  auth `camaraOAuth`, with the 400 documenting both `INVALID_ARGUMENT` and the KYC-specific
  `INVALID_PARAM_COMBINATION`, and `x-camarasim-scenarios` documenting the value-driven cases (full
  shared error set exposed, noted vs canonical 400/401/403/404/422). 170 tests green (was 153; +17:
  3 units [classify/E.164/score-emission] + 14 integration covering all-match/partial-with-score/
  not-available/phone-scoping/reserved-error-via-phone/reserved-error-via-subject/param-combination/
  empty-body/unknown-field/non-string/bad-phone/scope/auth/x-correlator). — binary: 990K (1012816 B;
  +20488 B)
- 2026-08-02 — Phase 1: SIM Swap v2 — `POST /retrieve-date` (CAMARA sim-swap 2.0.0, r2.2),
  completing the API. New route `/sim-swap/v2/retrieve-date` in `src/apis/sim_swap/v2.rs`,
  protected by the `Claims` extractor (scope `sim-swap:retrieve-date`; confirmed against the
  r2.2 upstream spec, operationId `retrieveSimSwapDate`, body `CreateSimSwapDate{phoneNumber?}`
  — no `maxAge` — response `SimSwapInfo{latestSimChange nullable, monitoredPeriod}`). Identifier
  resolution (validated `phoneNumber`, else token subject → 422 `MISSING_IDENTIFIER`) factored
  into a shared `resolve_identifier` reused by `check`. Functional cases (§7): reserved suffix →
  canonical CAMARA error; otherwise the identifier's trailing three digits = hours-since-last-swap
  (the SAME history `check` reads), and `latestSimChange` = *now − hoursAgo h* (RFC 3339 UTC) when
  `hoursAgo < 240` (the fixed monitored period = 240 h = 10 days, chosen to equal check's default
  `maxAge`), else `null`; `monitoredPeriod: 10` always returned. `x-correlator` echoed. No new deps
  — a self-contained `rfc3339_utc`/`civil_from_days` (Howard Hinnant) renders the timestamp from
  `SystemTime::now()` (non-blocking), avoiding a chrono/time dependency. Spec: added `/retrieve-date`
  path + `CreateSimSwapDate`/`SimSwapInfo` schemas to `specs/sim-swap/v2/openapi.yaml`, `$ref`-ing
  shared `errors.yaml` + auth `camaraOAuth`, with `x-camarasim-scenarios` documenting the
  identifier-driven cases (full shared error set exposed, noted vs the canonical 400/401/403/404/
  422/429 subset). 153 tests green (was 139; +14: 4 time-helper/last-swap units + 10 integration
  covering recent-date/null-outside-period/reserved-error/no-maxAge/bad-input/subject-fallback/
  non-numeric-subject/scope/auth/x-correlator). — binary: 972K (992328 B; +6184 B)
- 2026-08-02 — Phase 1: SIM Swap v2 — `POST /check` (CAMARA sim-swap 2.0.0, r2.2). New
  `src/apis/sim_swap/{,v2}.rs` mounted at `/sim-swap/v2/check`, merged into the app router;
  `/` catalog now lists sim-swap v2. Protected by the `Claims` extractor (scope
  `sim-swap:check`, confirmed against upstream). Body `CreateCheckSimSwap {phoneNumber?, maxAge?}`
  parsed as `Bytes` with `deny_unknown_fields`; empty body allowed (both fields optional per
  v2). Identifier = submitted `phoneNumber` (validated E.164) or, when omitted, the token
  subject (three-legged fallback; non-numeric/absent subject → never swapped). Functional cases
  (§7): `scenarios::reserved_error` → canonical CAMARA error (…404/…429/etc.); otherwise the
  identifier's trailing three digits encode hours-since-last-swap and `swapped =
  hoursAgo < maxAge`, so `maxAge` (1–2400, default 240) is a genuine second control plane;
  `maxAge` out of range → 400 `OUT_OF_RANGE` (canonical v2 code). `x-correlator` echoed on all
  responses. No new deps (reuses shared `scenarios`/`errors`; small local E.164 validator like
  NV). Spec: new `specs/sim-swap/v2/openapi.yaml` — vendored `POST /check` + `CreateCheckSimSwap`
  /`CheckSimSwapInfo` schemas, `$ref`-ing shared `errors.yaml` + auth `camaraOAuth`, with
  `x-camarasim-scenarios` documenting the identifier- and maxAge-driven cases (retrieve-date
  deferred, so served spec matches code; full shared error set exposed, noted vs canonical
  subset). 139 tests green (was 124; +15: 2 recency/E.164 units + 13 integration covering
  swapped/not-swapped/maxAge-window/out-of-range/reserved-error/subject-fallback/bad-input/
  scope/auth/x-correlator + catalog). — binary: 964K (986144 B; +7648 B)
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
