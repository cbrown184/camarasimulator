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

Device Identifier v0.3 is now complete: `POST /retrieve-ppid`
(`device-identifier:retrieve-ppid`, operationId `retrievePPID`) is live too.
Same identifier resolution and reserved-error convention as the other two
operations; on a happy path it returns a stable, **pseudonymous** `ppid` —
`SHA-256(identifier)` rendered as a UUID-shaped opaque token (irreversible, so
it never leaks the real IMEI, yet deterministic per device). Unlike the
type/identity operations the model tail is deliberately **not** a control plane
here, so a PPID reveals no device type. **This completes Phase 2.**

**Phase 3 has begun.** **One Time Password SMS v1** is CamaraSim's first
**stateful** API, mounted at `/one-time-password-sms/v1` (CAMARA
one-time-password-sms 1.1.1, release r3.2). Both `POST /send-code` and
`POST /validate-code` are live under the single scope
`one-time-password-sms:send-validate`. `send-code` mints an opaque
`authenticationId`, remembers the code it "sent" in a process-global in-memory
store (`src/apis/one_time_password_sms/store.rs`; `Mutex<HashMap>`, lock never
held across await, 300 s TTL, 3-attempt budget), and returns the id; there is no
real SMS, so the code is **deterministic from the phone number** — the last six
digits, zero-padded (`+123456789012` → `789012`) — so a headless caller can
compute what to validate (documented). Control planes (DESIGN §7): `send-code`
keys off the submitted `phoneNumber` (reserved suffix → canonical CAMARA error;
else issue an OTP); `validate-code` keys off the live store state — matching code
→ `204` (single-use consume); wrong code → `ONE_TIME_PASSWORD_SMS.INVALID_OTP`
until the attempt budget exhausts, then `…VERIFICATION_FAILED`; unknown/consumed/
expired id → `…VERIFICATION_EXPIRED` (all 400). `x-correlator` echoed on every
response, including the `204`.

**Quality on Demand v1** has begun (CamaraSim's first **resource-oriented**
stateful API). `POST /sessions` is live at `/quality-on-demand/v1/sessions`
(scope `quality-on-demand:sessions:create`, `createSession`): it mints an
opaque, UUID-shaped `sessionId` (`src/apis/quality_on_demand/store.rs`;
`Mutex<HashMap>`, lock never held across await, no uuid/rand dep), renders the
`SessionInfo`, remembers it, and returns 201. `GET /sessions/{sessionId}`
(`quality-on-demand:sessions:read`, `getSession`) reads the stored `SessionInfo`
back (200) or 404 `NOT_FOUND`. Three control planes (DESIGN §7): the identifier
(submitted `device` id, else token subject) — reserved suffix → canonical CAMARA
error (so `…409` drives the QoD 409 CONFLICT), else tail `…000`/no-digits →
`qosStatus:REQUESTED` (no times) and any other tail → `qosStatus:AVAILABLE`
(startedAt=now, expiresAt=now+duration); `duration` — `<1`→400 INVALID_ARGUMENT,
`>86400`→400 `QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE`; `qosProfile` — a name
containing `unavailable`→422 `QUALITY_ON_DEMAND.QOS_PROFILE_NOT_APPLICABLE`.
`x-correlator` echoed on every response. `DELETE /sessions/{sessionId}`
(`quality-on-demand:sessions:delete`, `deleteSession`) is now live too: keyed
only on the store state, it evicts an existing session → `204 No Content`
(single-use) or returns `404 NOT_FOUND` for an unknown/already-deleted id; no
`DELETE_REQUESTED` CloudEvent is emitted (notifications deferred).
`POST /sessions/{sessionId}/extend` (`quality-on-demand:sessions:update`,
`extendQosSession`) is now live: it adds `requestedAdditionalDuration` seconds
to a stored session's `duration` in place (via a new atomic `store::update`)
and — for an `AVAILABLE` session — pushes `expiresAt` out by the same amount,
returning the updated `SessionInfo` (`200`); the change persists (a later
`getSession` sees it). Two control planes (DESIGN §7): the stored session
(unknown id → `404 NOT_FOUND`) and the requested seconds (`<1` → 400
`INVALID_ARGUMENT`; `>86400`, **or** a resulting total duration `>86400`, → 400
`QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE` — so the ceiling case is
state-dependent).

`POST /retrieve-sessions` (`quality-on-demand:sessions:retrieve-by-device`,
`retrieveSessionsByDevice`) is now live too: it lists a device's active sessions
as an array of `SessionInfo` (`200`; an empty array when there are none — CAMARA
never 404s on an empty result). The device is the submitted `device` id, else
the token subject. Two control planes (DESIGN §7): the identifier — a reserved
error suffix → canonical CAMARA error (`…404` → 404 device-not-found) — and, on
the happy path, the in-memory store, scanned by each session's echoed `device`
(new `store::find_by_device`). A resolved identifier with no `device` echo (a
non-E.164 subject, no submitted device) matches nothing → `200 []`.

CloudEvents notifications on `sink` have begun (`src/apis/quality_on_demand/
notifications.rs`): deleting a session that was created with a `sink` delivers a
`qos-status-changed` CloudEvent (`qosStatus: UNAVAILABLE`, `statusInfo:
DELETE_REQUESTED`) to it — a best-effort, fire-and-forget HTTP POST written over a
raw `tokio` TCP stream (no HTTP-client dependency; off the request path). Only
`http://` sinks are delivered to (no TLS client) and delivery is unauthenticated
(`sinkCredential` unused). The **`DURATION_EXPIRED`** transition is now in place
too: creating an `AVAILABLE`, sink-bearing session spawns a fire-and-forget async
timer (`v1::spawn_expiry`, off the request path) that waits until the session's
`expiresAt`, re-reading it on each wake so an `extend` that pushed the expiry out is
honoured, then evicts the session and delivers a `UNAVAILABLE`/`DURATION_EXPIRED`
`qos-status-changed` CloudEvent to the sink (a concurrent `deleteSession` wins the
eviction, so exactly one event fires). The **`NETWORK_TERMINATED`** transition is
now in place too: a `…001` identifier tail marks an `AVAILABLE`, sink-bearing
session for early network drop — a short fire-and-forget timer (`v1::spawn_network_
termination`, off the request path) evicts it a fixed grace after creation,
independent of its (longer) `expiresAt`, and delivers a `UNAVAILABLE`/
`NETWORK_TERMINATED` event (again exactly-once vs a concurrent delete).
**`sinkCredential` (ACCESSTOKEN) auth** is now in place: a session created with a
`credentialType: ACCESSTOKEN` `sinkCredential` has its bearer token applied to
every callback as an `Authorization: Bearer <accessToken>` header (RFC 6750). The
credential is derived at creation (`notifications::sink_authorization`) and held
in a process-global in-memory side-store keyed by `sessionId`
(`store::insert_credential`/`take_credential`), kept apart from the `SessionInfo`
map so the secret is never echoed by `GET`/`retrieve-sessions`; it is taken
(single-use) at delivery time, so it drops from memory as the session ends. The
`PLAIN`/`REFRESHTOKEN` credential types are accepted but not applied (documented
cut). Only TLS (`https://` sink) delivery remains to complete QoD (deferred — it needs
a rustls TLS client whose crypto backend pulls a C/cmake toolchain and a large binary
regression, a dependency trade-off that deserves a deliberate decision, not an
automated pass).

**Phase 4 (spatial) has begun.** **Device Location Verification v3** `POST /verify` is
live at `/location-verification/v3/verify` (scope `location-verification:verify`,
operationId `verifyLocation`; CAMARA Location Verification 3.0.0, release r3.2 — the
latest published stable, so mounted at `/v3`). The caller submits an `area` (currently
only a `CIRCLE` — `center` lat/long + `radius` metres) and optionally a `device`
(`phoneNumber`, else IPv4 `publicAddress`, else `ipv6Address`; no
`networkAccessIdentifier` — CAMARA disallows it here), else the token subject. The
endpoint answers a **verdict, never a coordinate**:
`{verificationResult: TRUE|FALSE|PARTIAL, lastLocationTime}`. Two control planes
(DESIGN §7): the identifier's trailing three digits — reserved suffix → canonical CAMARA
error; `…000` → `FALSE`; odd → `PARTIAL` with `matchRate = (digits % 99) + 1` (1–99);
else → `TRUE` — and the circle `radius`, where CamaraSim enforces a documented regulatory
minimum of 2000 m (radius `[1, 2000)` → 422 `LOCATION_VERIFICATION.INVALID_AREA`; radius
`<1` or `center` lat/long out of range → 400 `OUT_OF_RANGE`; non-`CIRCLE` areaType → 400
`INVALID_ARGUMENT`). `maxAge` is validated for range (0..=int32) but otherwise ignored
(location data always treated as fresh). A supplied `device` is echoed back
(`VerifyLocationResponse.device`). `x-correlator` echoed on every response.

**Device Location Retrieval v0.4** `POST /retrieve` is now live at
`/location-retrieval/v0.4/retrieve` (scope `location-retrieval:read`, operationId
`retrieveLocation`; CAMARA Location Retrieval 0.4.0, release r3.2 — the latest
published version, mounted at its real `v0.4` like KYC Match / Device Identifier).
The companion to Location Verification: where Verification answers a verdict
against a supplied area, Retrieval **returns the device's position** as a CIRCLE
`area` (`{lastLocationTime, area:{areaType:CIRCLE, center, radius}}`). Same
DeviceLocation-family identifier resolution as Verification (submitted `device`
id — phoneNumber E.164, else IPv4 publicAddress, else ipv6Address, no NAI — else
token subject). Two control planes (DESIGN §7): the identifier — reserved suffix →
canonical CAMARA error; otherwise the trailing three digits fix the returned
circle deterministically (`center` = base point offset by `digits*0.001°`, `radius`
= `((digits%10)+1)*100` m, 100–1000 m accuracy) — and `maxAge` (validated
`[60, int32]`; out-of-range → 400 OUT_OF_RANGE, else ignored as location is always
fresh). `x-correlator` echoed on every response.

**Geofencing Subscriptions v0.4** has begun — CamaraSim's first
**event-subscription** API (CAMARA geofencing-subscriptions 0.4.0, r3.2, mounted
at its real sub-1.0 version `/geofencing-subscriptions/v0.4` like KYC Match /
Device Identifier / Location Retrieval). `POST /subscriptions`
(`geofencing-subscriptions:subscriptions:create`, `createSubscription`) registers
a subscription (a `device`, a `CIRCLE` `area`, and the event `types`
`area-entered`/`area-left`, plus an HTTP `sink`), mints a UUID-shaped `id`
(`src/apis/geofencing_subscriptions/store.rs`; `Mutex<HashMap>`, no uuid/rand
dep), stores the rendered `SubscriptionInfo`, and returns 201.
`GET /subscriptions/{subscriptionId}` (`…:subscriptions:read`,
`retrieveSubscription`) reads it back (200) or 404 `NOT_FOUND`. Two control planes
(DESIGN §7): the identifier (submitted `config.subscriptionDetail.device` id —
phoneNumber, else NAI, else IPv4 publicAddress, else ipv6Address — else token
subject) → reserved suffix → canonical CAMARA error; else tail `…000`/no-digits →
`status:ACTIVATION_REQUESTED`, any other tail → `status:ACTIVE`; and the circle
`radius` (2000–200000 m, else 400 OUT_OF_RANGE). `protocol` must be `HTTP` and
`types` must be known geofencing events, else 400 INVALID_ARGUMENT.
`sinkCredential` is accepted but never applied/echoed. `x-correlator` echoed on
every response. The subscription CRUD is now complete: `GET /subscriptions`
(`retrieveSubscriptionList`, `…:subscriptions:read`) lists the stored
subscriptions as an array of `SubscriptionInfo` (`200`, empty array when none;
CamaraSim does not scope subscriptions per client — a documented simplification),
and `DELETE /subscriptions/{subscriptionId}` (`deleteSubscription`,
`…:subscriptions:delete`) evicts a stored subscription → `204 No Content`
(single-use) or `404 NOT_FOUND` for an unknown/already-deleted id; deletion is
synchronous with no `subscription-ended` CloudEvent (204, not the template's
async 202 — a documented cut). CloudEvents delivery has now begun:
`createSubscription` delivers the **initial event** (`config.initialEvent: true`)
— a single `area-entered`/`area-left` CloudEvent reporting the device's current
in/out state at subscription time, POSTed to an `http://` `sink` fire-and-forget
over raw TCP (`src/apis/geofencing_subscriptions/notifications.rs`; no HTTP-client
dep, mirroring QoD). The position is deterministic from the identifier's trailing
three digits (even → inside → `area-entered`; odd → outside → `area-left`), fired
only for an `ACTIVE` subscription and filtered to the subscribed `types`.
**`sinkCredential` (ACCESSTOKEN bearer) auth** is now applied: a subscription
created with a `credentialType: ACCESSTOKEN` `sinkCredential` has its bearer token
applied to the initial-event callback as an `Authorization: Bearer <accessToken>`
header (RFC 6750), derived at creation-time (`notifications::sink_authorization`,
mirroring QoD) and never echoed in the `SubscriptionInfo` (it is a secret);
PLAIN/REFRESHTOKEN are accepted but not applied (a documented cut).
**Subscription expiry** is now enforced: a subscription created with a
`config.subscriptionExpireTime` (UTC `…Z`) arms a fire-and-forget async timer
(`v0_4::spawn_expiry`, off the request path, mirroring QoD's `spawn_expiry`) that
waits until that instant, then evicts the subscription and delivers a
`subscription-ended` CloudEvent (`terminationReason: SUBSCRIPTION_EXPIRED`) to the
`sink` — with the ACCESSTOKEN `sinkCredential` bearer applied. A concurrent
`deleteSubscription` wins the eviction (exactly-once); a non-`Z` offset time is
echoed but arms no timer (documented cut). **Movement-triggered events** are now
delivered too: a `…001` identifier tail on an `ACTIVE`, sink-bearing subscription
simulates the device *entering* the area (`area-entered`) and a `…002` tail
simulates it *leaving* (`area-left`), delivered by a short (1 s) fire-and-forget
timer (`v0_4::spawn_movement`, off the request path, mirroring QoD's `…001`
NETWORK_TERMINATED), filtered to the subscribed `types`, with the ACCESSTOKEN
`sinkCredential` bearer applied; the subscription stays `ACTIVE` and a crossing is
suppressed if a delete/expiry already ended it.

**`subscriptionMaxEvents` enforcement** is now in place, **completing Geofencing
Subscriptions v0.4 and Phase 4**. A subscription created with
`config.subscriptionMaxEvents` (integer `>= 1`, else 400 `OUT_OF_RANGE`) carries an
in-memory event budget (`store::consume_event`, a side-store kept apart from the
echoed `SubscriptionInfo`). Each delivered domain event (`area-entered`/`area-left`
— initial and movement) consumes one unit; the event that spends the last unit is
delivered and then the subscription **ends** — evicted, with a `subscription-ended`
CloudEvent (`terminationReason: MAX_EVENTS_REACHED`) POSTed in order right after the
triggering event (new `notifications::spawn_delivery_seq`), the ACCESSTOKEN
`sinkCredential` bearer applied. Eviction is synchronous, so a still-pending
movement/expiry timer becomes a no-op (exactly one terminal outcome). An unset
`subscriptionMaxEvents` is unbounded.

**Phase 5 (payments) has begun.** **Carrier Billing v0.5** `POST /payments` is
live at `/carrier-billing/v0.5/payments` (scope `carrier-billing:payments:create`,
operationId `createPayment`; CAMARA Carrier Billing 0.5.0, release r3.2 — the API's
first public release, mounted at its real sub-1.0 version like KYC Match / Location
Retrieval). It runs the **one-step** flow (the only flow 0.5.0 covers): a payment
is created and charged in a single call, so a happy path returns `201` with
`paymentStatus: "succeeded"` (opaque UUID-shaped `paymentId`, `paymentCreationDate`
= `paymentDate` = now; no `uuid`/`rand` dep). Two control planes (DESIGN §7): the
charged phone number (submitted `amountTransaction.phoneNumber`, else the token
subject) — reserved error suffix → canonical CAMARA error, malformed `phoneNumber`
→ 400 `INVALID_ARGUMENT`, no number + non-E.164 subject → 422 `MISSING_IDENTIFIER`
— and the requested `amount`, where `< 0.001` (schema minimum) → 400
`INVALID_ARGUMENT` and `> 1000` (authorised ceiling) → 422
`CARRIER_BILLING.UNAUTHORIZED_AMOUNT`; else the charge succeeds. The identifier
plane is checked first (a reserved-error number wins over an out-of-range amount).
Stateless for now — nothing reads a payment back yet, so it is not persisted;
`sink`/`sinkCredential` are accepted but not acted on (both documented cuts).
`x-correlator` echoed on every response.

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
- [x] Device Identifier v0.3 (`/device-identifier/v0.3`; CAMARA 0.3.0, release r2.2):
  - [x] `POST /retrieve-type` (`device-identifier:retrieve-type`)
  - [x] `POST /retrieve-identifier` (`device-identifier:retrieve-identifier`)
  - [x] `POST /retrieve-ppid` (`device-identifier:retrieve-ppid`)

### Phase 3 — Stateful, non-spatial
- [x] One Time Password SMS v1 — [x] `POST /send-code` · [x] `POST /validate-code`
  (`/one-time-password-sms/v1`; CAMARA 1.1.1, r3.2; in-memory OTP store)
- [~] Quality on Demand (QoD) v1 — session lifecycle + CloudEvents notifications
  (`/quality-on-demand/v1`; CAMARA 1.1.0, r3.2; in-memory session store):
  - [x] `POST /sessions` (`quality-on-demand:sessions:create`, `createSession`)
  - [x] `GET /sessions/{sessionId}` (`quality-on-demand:sessions:read`, `getSession`)
  - [x] `DELETE /sessions/{sessionId}` (`quality-on-demand:sessions:delete`, `deleteSession`)
  - [x] `POST /sessions/{sessionId}/extend` (`quality-on-demand:sessions:update`, `extendQosSession`)
  - [x] `POST /retrieve-sessions` (`quality-on-demand:sessions:retrieve-by-device`, `retrieveSessionsByDevice`)
  - [~] CloudEvents notifications on `sink` (qosStatus changes / expiry):
    - [x] `DELETE_REQUESTED` `qos-status-changed` on `deleteSession` (http sink,
      best-effort fire-and-forget over raw TCP; no HTTP-client dep)
    - [x] `DURATION_EXPIRED` transition (async timer at creation for an AVAILABLE,
      sink-bearing session; re-checks `expiresAt` so `extend` is honoured)
    - [x] `NETWORK_TERMINATED` transition (`…001` identifier tail: an AVAILABLE,
      sink-bearing session is dropped early by the simulated network)
    - [x] `sinkCredential` (ACCESSTOKEN bearer) auth — `Authorization: Bearer`
      header on the callbacks (in-memory credential; PLAIN/REFRESHTOKEN deferred)
    - [ ] TLS (`https://` sink) delivery (needs a rustls TLS client)

### Phase 4 — Spatial
- [x] Device Location Verification v3 — `POST /verify` (`/location-verification/v3`;
  CAMARA 3.0.0, r3.2; verdict TRUE/FALSE/PARTIAL, identifier + circle-radius control planes)
- [x] Device Location Retrieval v0.4 — `POST /retrieve` (`/location-retrieval/v0.4`;
  CAMARA 0.4.0, r3.2; returns a CIRCLE area, identifier + maxAge control planes)
- [x] Geofencing Subscriptions v0.4 (`/geofencing-subscriptions/v0.4`; CAMARA
  0.4.0, r3.2; in-memory subscription store):
  - [x] `POST /subscriptions` (`geofencing-subscriptions:subscriptions:create`, `createSubscription`)
  - [x] `GET /subscriptions/{subscriptionId}` (`geofencing-subscriptions:subscriptions:read`, `retrieveSubscription`)
  - [x] `GET /subscriptions` (list, `retrieveSubscriptionList`) + `DELETE /subscriptions/{subscriptionId}` (`…:subscriptions:delete`, `deleteSubscription`)
  - [x] CloudEvents delivery on `sink` (`area-entered` / `area-left`) + expiry/maxEvents:
    - [x] initial event (`config.initialEvent`) — an `area-entered`/`area-left`
      CloudEvent for the device's current in/out state at creation of an ACTIVE
      subscription (http sink, fire-and-forget over raw TCP; no HTTP-client dep)
    - [x] movement-triggered `area-entered`/`area-left` events (`…001` tail →
      enter, `…002` tail → leave; simulated crossing via a 1 s fire-and-forget timer)
    - [x] `sinkCredential` auth on the callback (ACCESSTOKEN bearer on the
      initial-event callback; PLAIN/REFRESHTOKEN deferred)
    - [x] expiry (`subscriptionExpireTime`) → `subscription-ended`
      (`SUBSCRIPTION_EXPIRED`) CloudEvent + eviction (async timer at creation)
    - [x] `subscriptionMaxEvents` enforcement (count delivered domain events;
      the Nth event ends the subscription → `subscription-ended`
      `MAX_EVENTS_REACHED` + eviction; `<1` → 400 OUT_OF_RANGE)

### Phase 5 — Remaining
- [~] Carrier Billing v0.5 (`/carrier-billing/v0.5`; CAMARA 0.5.0, release r3.2):
  - [x] `POST /payments` (`carrier-billing:payments:create`, `createPayment`) —
    one-step charge; identifier + amount control planes (DESIGN §7). Stateless
    (nothing reads a payment back yet, so not persisted).
  - [ ] `GET /payments` (list, `retrievePayments`) · `GET /payments/{paymentId}`
    (`retrievePayment`) — `carrier-billing:payments:read` (needs a payment store)
  - [ ] two-step flow: `POST /payments/prepare` (`preparePayment`) ·
    `.../{paymentId}/validate` · `.../confirm` · `.../cancel`
    (`carrier-billing:payments:write`)
  - [ ] charging notifications on `sink` (accepted-but-not-applied for now)
- [ ] Other CAMARA APIs as capacity allows

## Cross-cutting (do alongside the item that needs it)
- [~] `errors.rs`: base CAMARA error model done (`src/errors.rs`, `specs/shared/errors.yaml`); per-version catalogs still TODO (DESIGN §8)
- [ ] `registry.rs`: canonical URL versioning + `/` catalog wiring (DESIGN §9)
- [~] `specs/…`: vendor + annotate OpenAPI per API/version, serve at `/{api}/v{n}/openapi.yaml`
  — **serving done** (`src/apis/openapi.rs`: every mounted API's spec at
  `/{api}/v{n}/openapi.yaml`, plus `/auth/openapi.yaml` + `/shared/errors.yaml` so
  `$ref`s resolve; catalog advertises each `spec_url`). Per-API *vendoring/annotation*
  continues alongside each new API.
- [ ] Contract-test harness (validate responses against vendored spec)

---

## Scan journal

Newest first. One line per pass: `YYYY-MM-DD HH:MMZ — <what happened> — binary: <size>`

- 2026-08-03 — Phase 5 (payments) **begun** — **Carrier Billing v0.5**
  `POST /payments` (`createPayment`), CamaraSim's first payments API. Phases 0–4
  are complete (only the deliberately-deferred QoD `https://` TLS sink remains, an
  intentional dependency decision), so this pass opens Phase 5. Fetched the real
  CAMARA spec: the `main` branch is `wip`, but release **r3.2** ships Carrier
  Billing **0.5.0** (first public release), so — like KYC Match v0.3 / Location
  Retrieval v0.4 — it is mounted at its real sub-1.0 URL `/carrier-billing/v0.5`.
  Scoped to the single one-step `createPayment` endpoint (the only flow 0.5.0
  covers): a happy path returns `201 { paymentStatus: "succeeded" }` with an opaque
  UUID-shaped `paymentId` and `paymentCreationDate = paymentDate = now` (local
  `mint_uuid` off a SHA-256(counter‖now) + atomic counter, and a self-contained
  `rfc3339_utc` — both mirroring `quality_on_demand`; **no new dep**, sha2 already
  present). Two control planes (DESIGN §7): the charged phone number (submitted
  `amountTransaction.phoneNumber`, else the token subject) — reserved suffix →
  canonical CAMARA error, malformed `phoneNumber` → 400 INVALID_ARGUMENT, no number
  + non-E.164 subject → 422 MISSING_IDENTIFIER — and the requested `amount`
  (`< 0.001` schema min → 400 INVALID_ARGUMENT; `> 1000` ceiling → 422
  `CARRIER_BILLING.UNAUTHORIZED_AMOUNT`); identifier plane checked first. Stateless
  (no read-back yet → not persisted); `sink`/`sinkCredential` accepted but not
  applied (documented cuts). New `src/apis/carrier_billing{,.rs}/v0_5.rs` merged
  into the app router; `/` catalog + openapi server now list carrier-billing v0.5.
  Spec: new `specs/carrier-billing/v0.5/openapi.yaml` (vendored + annotated —
  `createPayment` with `x-camarasim-scenarios`, inline 400/422 responses carrying
  the carrier-billing codes, the full CreatePayment/PaymentCreated schema tree,
  reserved floor `$ref`ing shared/errors.yaml). 462 tests green (was 444; +18: 3
  units [E.164 validation · paymentId unique/uuid-v4-shaped · rfc3339 known epoch]
  + 15 integration [happy 201 succeeded · clientCorrelator echoed · reserved suffix
  → error · amount>1000 → UNAUTHORIZED_AMOUNT · amount<0.001 → INVALID_ARGUMENT ·
  reserved-id wins over amount · malformed phone → 400 · missing required field →
  400 · unknown field → 400 · no-phone falls back to E.164 subject · no-phone +
  non-E.164 subject → MISSING_IDENTIFIER · subject reserved suffix → error · no
  scope → 403 · no token → 401 · x-correlator echoed on 201 + error]). — binary:
  1538192 B (+42536 B; the new module + ~14 KB of vendored spec text embedded via
  include_str!)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** —
  **`subscriptionMaxEvents` enforcement**, the API's last open item — **completes
  Geofencing Subscriptions v0.4 and Phase 4**. A subscription created with
  `config.subscriptionMaxEvents` (validated integer `>= 1`, else 400 `OUT_OF_RANGE`)
  now bounds the `area-entered`/`area-left` events it delivers. New store side-store
  (`budgets()`, kept apart from the echoed `SubscriptionInfo`, mirroring QoD's
  credential side-store) + `set_event_budget`/`consume_event` returning a new
  `EventBudget{Unbounded,Allowed,Last,Exhausted}`; `store::remove` now also drops the
  budget so delete/expiry leave nothing stale. New `v0_4::deliver_counted` wraps every
  domain-event delivery (initial + movement): while budget remains it delivers; the
  event that spends the last unit is delivered and then the subscription ends —
  evicted synchronously (so a pending movement/expiry timer becomes a no-op, exactly
  one terminal outcome) and a `subscription-ended` (`MAX_EVENTS_REACHED`) CloudEvent
  POSTed in order right after it via the new `notifications::spawn_delivery_seq`
  (ordered multi-event delivery; a single `spawn_delivery` per event races). The
  terminal event isn't counted; unset maxEvents is unbounded; ACCESSTOKEN
  `sinkCredential` bearer applied to the terminal callback too. **No new deps.** Spec:
  updated `specs/geofencing-subscriptions/v0.4/openapi.yaml` — new "Max events"
  section + header/movement/expiry prose, `subscriptionMaxEvents` property (now
  enforced), the `MAX_EVENTS_REACHED` `terminationReason` enum value, and the
  `createSubscription` `x-camarasim-scenarios` (2 max-events cases + refreshed cuts) /
  `callbacks` prose. 444 tests green (was 436; +8: 3 store units [budget counts
  down→Last→Unbounded · budget-of-1→Last · remove drops budget] + 1 notifications
  unit [subscription-ended MAX_EVENTS_REACHED shape] + 4 v0_4 integration [maxEvents<1
  → 400 OUT_OF_RANGE · maxEvents=1 initial event → event+subscription-ended, GET 404 ·
  maxEvents=1 movement event → event+ended, GET 404 · maxEvents=2 spans initial +
  movement then ends]). — binary: 1495656 B (+10456 B; mostly the new vendored spec
  text embedded via include_str!)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** —
  **movement-triggered `area-entered`/`area-left` events** (simulated boundary
  crossing). A headless simulator has no real device motion, so — mirroring QoD's
  `…001` `NETWORK_TERMINATED` — two reserved identifier tails now instruct the
  simulator to report a crossing: `…001` → the device *enters* (`area-entered`),
  `…002` → it *leaves* (`area-left`). New pure `notifications::movement_event_type`
  (fires only for an ACTIVE subscription whose trailing-three-digits are exactly
  `001`/`002` and whose crossing type is among the subscribed `types`); new
  `v0_4::spawn_movement`, a short (1 s, `MOVEMENT_GRACE_SECS`) fire-and-forget timer
  (off the request path, mirroring QoD's `spawn_network_termination`) that — if the
  subscription is still live (a `store::get` guard suppresses the crossing when a
  delete/expiry already ended it) — builds the `geofencing_event` and delivers it,
  with the ACCESSTOKEN `sinkCredential` bearer applied. The marker is independent of
  the initial-event even/odd position, so both events can fire for one subscription;
  the subscription stays `ACTIVE` (bounding the count via `subscriptionMaxEvents` is
  the only remaining API item). **No new deps** (reuses `geofencing_event` /
  `spawn_delivery` / `sink_authorization`). Spec: updated
  `specs/geofencing-subscriptions/v0.4/openapi.yaml` — new "Movement events" section,
  refreshed header/initial-event prose, the `createSubscription`
  `x-camarasim-scenarios` (3 movement cases + refreshed cuts) and `callbacks`
  description (now three event kinds). 436 tests green (was 431; +5: 2 notifications
  units [`…001`→entered/`…002`→leave, ACTIVE-only, other-tail/None → none · type
  filtering] + 3 v0_4 integration [`…001` → area-entered to loopback sink w/
  subscriptionId+device+area · `…002` → area-left · ACCESSTOKEN → `Authorization:
  Bearer` on the movement callback]). — binary: 1485200 B (+8328 B; mostly the new
  vendored spec text embedded via include_str!)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** — **subscription
  expiry** (`config.subscriptionExpireTime` → `subscription-ended`). A subscription
  created with an RFC 3339 UTC (`…Z`) expire time now arms a fire-and-forget async
  timer (`v0_4::spawn_expiry`, off the request path, mirroring QoD's `spawn_expiry`)
  that waits until that instant — a past time fires immediately — then evicts the
  subscription (`store::remove`) and delivers a `subscription-ended` CloudEvent
  (`terminationReason: SUBSCRIPTION_EXPIRED`) to the `sink`, with the ACCESSTOKEN
  `sinkCredential` bearer applied (captured at creation, no side-store — geofencing
  has no separate delete request to defer for, unlike QoD). Exactly-once vs a
  concurrent `deleteSubscription` (whoever's `remove` returns `Some` sends the
  event; DELETE itself stays event-less). New `notifications::subscription_ended_
  event` (pure CloudEvents-1.0 builder) + `EVENT_TYPE_SUBSCRIPTION_ENDED` const;
  new local `parse_rfc3339_utc`/`days_from_civil` (inverse of the existing
  `rfc3339_utc`, mirroring QoD — no date-time dep). Documented cut: only the `…Z`
  form drives the timer (a numeric offset is echoed but arms no timer);
  `subscriptionMaxEvents` still deferred. **No new deps.** Spec: updated
  `specs/geofencing-subscriptions/v0.4/openapi.yaml` — new expiry section + header
  prose, `subscriptionExpireTime` description, `createSubscription`
  `x-camarasim-scenarios` (2 expiry cases + cuts) and `callbacks` prose, and the
  `CloudEvent` schema (added the `subscription-ended` type, a `terminationReason`
  enum, and made `data.area` optional so both event shapes validate). 431 tests
  green (was 426; +5: 1 notifications unit [subscription_ended shape, no area] +
  1 v0_4 unit [parse_rfc3339_utc round-trip / rejects offset] + 3 v0_4 integration
  [past expireTime → subscription-ended to loopback sink + eviction 404 · ACCESSTOKEN
  → `Authorization: Bearer` on the expiry callback · non-`Z` offset echoed but no
  timer → still readable]). — binary: 1476872 B (+10080 B; mostly the new vendored
  spec text embedded via include_str!)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** — `sinkCredential`
  (ACCESSTOKEN bearer) **auth on the initial-event callback**. A subscription created
  with a `credentialType: ACCESSTOKEN` `sinkCredential` now has its bearer token applied
  to the fire-and-forget initial-event CloudEvent as an `Authorization: Bearer
  <accessToken>` header (RFC 6750), matching the resource-server scheme and mirroring
  QoD. Because geofencing delivers the initial event synchronously at creation, the
  credential is derived right there (`notifications::sink_authorization`, a new pure fn
  identical to QoD's) and passed straight into `spawn_delivery` — no side-store needed
  (unlike QoD's deferred deletion path). The secret is never echoed in the
  `SubscriptionInfo`. PLAIN/REFRESHTOKEN accepted but not applied (documented cut);
  `sinkCredential` field lost its `#[allow(dead_code)]`. `spawn_delivery`/`deliver`
  gained an `auth: Option<…>` arg (emits the `Authorization` header when Some), mirroring
  QoD's signature. Only movement-triggered events + expiry/maxEvents now remain for the
  API. **No new deps.** Spec: updated `specs/geofencing-subscriptions/v0.4/openapi.yaml`
  — refreshed the header prose, the `createSubscription` `x-camarasim-scenarios`
  (ACCESSTOKEN-applied case + PLAIN/REFRESHTOKEN cut), the `callbacks.notifications`
  description, and the `sinkCredential` schema description. 426 tests green (was 422; +4:
  2 notifications units [sink_authorization: ACCESSTOKEN→Bearer, empty/other types→None ·
  deliver emits the Authorization header when Some] + 2 v0_4 integration [ACCESSTOKEN
  credential → `Authorization: Bearer` on the callback, secret not echoed · no credential
  → callback unauthenticated]; the existing unauth deliver test now also asserts no
  Authorization header). — binary: 1466792 B (−768 B)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** — CloudEvents
  delivery begun: `createSubscription` now delivers the **initial event**
  (`config.initialEvent: true`). In a headless simulator there is no real device
  movement, so the initial event is the only request-triggered geofencing
  notification (CAMARA's own mechanism for reporting the device's current in/out
  state at subscription time) — the natural first delivery slice, mirroring QoD's
  request-triggered `DELETE_REQUESTED`. New
  `src/apis/geofencing_subscriptions/notifications.rs`: a pure
  `initial_event_type(initialEvent, status, digits, types)` decision (fires only
  when initialEvent=true AND status ACTIVE AND the computed position's event type
  is among the subscribed `types`; even trailing-three-digits → inside →
  `area-entered`, odd → outside → `area-left`), a pure `geofencing_event(...)`
  CloudEvents-1.0 builder (`data`: subscriptionId/device/area), and a
  fire-and-forget `spawn_delivery`/`deliver` raw-TCP POST over `tokio`
  (`application/cloudevents+json`, `http://` sinks only — no TLS client, a
  documented cut) — no HTTP-client dep, structurally mirroring
  `quality_on_demand::notifications`. New `store::new_event_id()` mints
  UUID-shaped CloudEvent ids off the shared counter. `create_subscription` spawns
  the delivery after storing the SubscriptionInfo (off the request path; 201
  returns immediately). `sinkCredential` still accepted-but-not-applied → the
  callback is unauthenticated (a documented cut; ACCESSTOKEN auth deferred).
  Spec: updated `specs/geofencing-subscriptions/v0.4/openapi.yaml` — added the
  `createSubscription` `callbacks.notifications` block + a `CloudEvent` schema,
  refreshed the `initialEvent` property + header/info prose + the
  `x-camarasim-scenarios` (two new initial-event cases + delivery/cuts). 422 tests
  green (was 412; +10: 1 store unit [event-ids unique / never collide with
  subscription ids] + 7 notifications units [initial_event_type: true/false/absent
  · ACTIVE-vs-ACTIVATION_REQUESTED · even→entered/odd→left · no-digits→none ·
  types-filtering; event-shape; device-omitted-when-none; parse_http_sink;
  deliver-posts-a-cloudevent; non-http-noop] + 2 v0_4 integration [even-tail →
  area-entered to loopback sink w/ subscriptionId+device+area · odd-tail →
  area-left]). **No new deps.** — binary: 1467560 B (+14520 B; the delta over the
  code-only growth is the ~6 KB of new vendored spec text embedded via include_str!)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** — completed the
  subscription CRUD with `GET /subscriptions` (list, operationId
  `retrieveSubscriptionList`, scope `…:subscriptions:read`) and `DELETE
  /subscriptions/{subscriptionId}` (`deleteSubscription`, new scope
  `geofencing-subscriptions:subscriptions:delete`). New `store::all()` (snapshot of
  every stored `SubscriptionInfo`) and `store::remove()` (evict, returning the prior
  value so delete can answer 204 vs 404), both holding the `Mutex` only for the
  map access (never across `.await`); the `/subscriptions` route gained `.get(list_
  subscriptions)` and `/subscriptions/{id}` gained `.delete(delete_subscription)`.
  list returns `200` with a JSON array (empty when none) — CamaraSim does not scope
  subscriptions per client, so it returns every stored subscription, a documented
  simplification. delete evicts an existing subscription → `204 No Content`
  (single-use) or `404 NOT_FOUND`; deletion is synchronous with **no**
  `subscription-ended` CloudEvent (so `204`, not the CAMARA subscription-template's
  async `202` — a documented cut). `x-correlator` echoed on every response including
  the `204`. Only notification delivery (`area-entered`/`area-left` CloudEvents) +
  expiry/maxEvents now remain for the API. **No new deps** (pure axum routing +
  serde_json + shared errors). Spec: updated
  `specs/geofencing-subscriptions/v0.4/openapi.yaml` — added the `retrieveSubscription
  List` (200 array) and `deleteSubscription` (204/404) operations with their
  `x-camarasim-scenarios`, refreshed the header/info prose (CRUD now complete; only
  the sink-delivery cut remains) and scope docs. 412 tests green (was 402; +10:
  2 store units [remove-once-then-none / all-includes-a-stored-subscription] +
  8 integration covering list-includes-created / list-requires-read-scope /
  list-no-token-401 / delete-then-get-404 / delete-unknown-404 / delete-requires-
  delete-scope / delete-no-token-401 / x-correlator-on-delete-204-and-404). — binary: 1453040 B (+10816 B)

- 2026-08-03 — Phase 4 (spatial): **Geofencing Subscriptions v0.4** begun —
  CamaraSim's first **event-subscription** API (CAMARA geofencing-subscriptions
  0.4.0, release r3.2). The only remaining Phase 3 item (QoD TLS `https://` sink)
  stays deferred pending a deliberate rustls-TLS dependency decision, so this pass
  advances Phase 4. Mounted at `/geofencing-subscriptions/v0.4` (real sub-1.0
  published version, mirroring KYC Match / Device Identifier / Location Retrieval).
  New `src/apis/geofencing_subscriptions/{,store,v0_4}.rs` merged into the app
  router; `/` catalog + openapi server now list geofencing-subscriptions v0.4.
  Two endpoints this slice: `POST /subscriptions` (operationId `createSubscription`,
  scope `geofencing-subscriptions:subscriptions:create`) and
  `GET /subscriptions/{subscriptionId}` (`retrieveSubscription`,
  `…:subscriptions:read`). Body `SubscriptionRequest{protocol,sink,sinkCredential?,
  types,config{subscriptionDetail{device?,area},subscriptionExpireTime?,
  subscriptionMaxEvents?,initialEvent?}}` parsed with `deny_unknown_fields`.
  createSubscription mints a UUID-shaped `id` (store.rs; `Mutex<HashMap>`, lock
  never across await, SHA-256(counter‖now) uuid mint, no uuid/rand dep), renders
  `SubscriptionInfo` (request echoed minus the secret + id/startsAt/status), stores
  it, returns 201; retrieveSubscription reads it back (200) or 404 NOT_FOUND. Two
  control planes (§7): (1) the identifier (config.subscriptionDetail.device id —
  phoneNumber E.164, else NAI, else IPv4 publicAddress, else ipv6Address — else
  token subject → 422 MISSING_IDENTIFIER) — reserved suffix → canonical CAMARA
  error, `…000`/no-digits → status ACTIVATION_REQUESTED, any other tail → status
  ACTIVE; (2) the circle `radius` — 2000–200000 m (CAMARA geofencing bounds), else
  400 OUT_OF_RANGE; center out of lat/long range → 400 OUT_OF_RANGE. Envelope
  validation: `protocol` must be `HTTP` (other protocols → 400, a documented cut),
  `types` non-empty and only the two known geofencing events (else 400
  INVALID_ARGUMENT), non-CIRCLE areaType / missing area/center/radius → 400
  INVALID_ARGUMENT. Precedence: envelope 400 → area 400 → identifier (400/422) +
  reserved error → created subscription. `sinkCredential` accepted but not applied
  and never echoed. `x-correlator` echoed on every response. Documented cuts:
  notification delivery (`area-entered`/`area-left` CloudEvents), listing
  (`GET /subscriptions`), `DELETE`, and expiry/maxEvents enforcement deferred to
  later passes; scopes collapsed from CAMARA's per-event-type form to a single
  resource-action scope (a subscription can carry several event types). **No new
  deps** (serde_json + sha2 uuid mint + shared scenarios/errors + local
  E.164/rfc3339). Spec: new `specs/geofencing-subscriptions/v0.4/openapi.yaml` —
  vendored 0.4.0 `POST /subscriptions` + `GET /subscriptions/{id}` with
  SubscriptionRequest/Config/SubscriptionDetail/Device/DeviceIpv4Addr/Area(CIRCLE)/
  Point/SubscriptionInfo schemas, `$ref`-ing shared `errors.yaml` + auth
  `camaraOAuth`, Generic400 (INVALID_ARGUMENT|OUT_OF_RANGE) + Generic422
  (MISSING_IDENTIFIER), `x-camarasim-scenarios` documenting both control planes +
  precedence + cuts. 402 tests green (was 378; +24: 2 store units [uuid-shape /
  round-trip] + 2 v0_4 units [E.164 / device-precedence] + 20 integration covering
  happy-ACTIVE+device-echo / …000-ACTIVATION_REQUESTED / reserved-404+429 /
  create-then-read-back / unknown-id-404 / NAI+ipv6-echo / radius-below-2000-400 /
  radius-above-200000-400 / center-out-of-range-400 / non-CIRCLE-400 / bad-protocol-
  400 / unknown-event-type-400 / missing-types-400 / empty-device-400 / subject-
  fallback / subject-reserved-503 / non-numeric-subject-ACTIVATION_REQUESTED /
  no-scope-403 / no-token-401 / x-correlator; also extended the catalog + openapi-
  server tests). — binary: 1442224 B (+53520 B)

- 2026-08-03 — Phase 4 (spatial): **Device Location Retrieval v0.4** — `POST
  /retrieve` (CAMARA Location Retrieval 0.4.0, release r3.2 — the latest published
  version; the API has one endpoint, so this completes it). CamaraSim's second
  spatial API and the companion to Location Verification: Verification answers a
  verdict against a supplied area, Retrieval **returns the device's position** as a
  CIRCLE area. New `src/apis/location_retrieval/{,v0_4}.rs` merged into the app
  router; `/` catalog + openapi server now list location-retrieval v0.4. Mounted at
  `/location-retrieval/v0.4/retrieve` (real published sub-1.0 version, mirroring KYC
  Match / Device Identifier), scope `location-retrieval:read`, operationId
  `retrieveLocation`. Body `RetrievalLocationRequest{device?, maxAge?}` parsed with
  `deny_unknown_fields`; empty body accepted as `{}` (both fields optional).
  Response `Location{lastLocationTime, area:{areaType:CIRCLE, center{latitude,
  longitude}, radius}}`. Same DeviceLocation-family identifier resolution as
  Verification (submitted `device` id [phoneNumber E.164, else IPv4 publicAddress,
  else ipv6Address — no NAI, disallowed here], else token subject → 422
  MISSING_IDENTIFIER). Two control planes (§7): (1) the identifier — reserved suffix
  → canonical CAMARA error; otherwise the trailing three digits fix the circle
  deterministically (`center` = base point 51.5/-0.12 offset by digits*0.001°,
  `radius` = ((digits%10)+1)*100 m in 100–1000 m, the location accuracy), so the
  reported position is reproducible from the input; (2) `maxAge` — validated
  [60, 2147483647] (else 400 OUT_OF_RANGE), otherwise ignored (location always fresh,
  lastLocationTime=now via the self-contained rfc3339/civil_from_days formatter
  reused from location_verification — no date-time dep). Precedence: body 400 →
  maxAge OUT_OF_RANGE → identifier resolution + reserved error → location. Documented
  cuts: always returns a CIRCLE; `maxAge` never triggers an "unable to fulfil"
  case; CamaraSim doesn't distinguish 2- vs 3-legged tokens; 409/500 are CamaraSim
  extensions so every reserved suffix is reachable. `x-correlator` echoed on every
  response. **No new deps** (serde_json + shared scenarios/errors + local
  E.164/rfc3339). Spec: new `specs/location-retrieval/v0.4/openapi.yaml` — vendored
  0.4.0 `POST /retrieve` with RetrievalLocationRequest/Device/DeviceIpv4Addr/
  Location/Area(CIRCLE)/Point schemas, `$ref`-ing shared `errors.yaml` + auth
  `camaraOAuth`, Generic400 (INVALID_ARGUMENT|OUT_OF_RANGE) + Generic422
  (MISSING_IDENTIFIER), `x-camarasim-scenarios` documenting both control planes +
  precedence + cuts. 378 tests green (was 360; +18: 4 units [location determinism /
  radius-100-1000-band / E.164 / device-precedence] + 14 integration covering
  happy-path-deterministic-circle / different-ids-different-locations / reserved-
  404+429 / ipv4+ipv6 / empty-body-subject-fallback / subject-reserved-503 / maxAge-
  below-min-400 / valid-maxAge-200 / bad-phone-400 / empty-device-400 / NAI-rejected-
  400 / no-scope-403 / no-token-401 / x-correlator; also extended the catalog +
  openapi-server tests). — binary: 1388704 B (+23088 B)

- 2026-08-03 — Phase 4 (spatial) begun: **Device Location Verification v3** — `POST
  /verify` (CAMARA Location Verification 3.0.0, release r3.2 — the latest published
  stable; the API has one endpoint, so this completes it). CamaraSim's first spatial
  API: it answers a verdict (device inside/outside/partly-inside a requested circle),
  never a coordinate. New `src/apis/location_verification/{,v3}.rs` merged into the app
  router; `/` catalog + openapi server now list location-verification v3. Mounted at
  `/location-verification/v3/verify`, scope `location-verification:verify`, operationId
  `verifyLocation` (confirmed against the r3.2 upstream spec: `VerifyLocationRequest`
  {device?, area(required), maxAge?}, `VerifyLocationResponse` {verificationResult
  TRUE|FALSE|PARTIAL (req), lastLocationTime (req), matchRate 1–99 (PARTIAL only),
  device?}). Body parsed with `deny_unknown_fields`. Two control planes (§7): (1) the
  identifier (submitted `device` id [phoneNumber E.164, else IPv4 publicAddress, else
  ipv6Address — no NAI, disallowed here], else token subject → 422 MISSING_IDENTIFIER) —
  reserved suffix → canonical CAMARA error, `…000` → FALSE, odd tail → PARTIAL with
  matchRate=(digits%99)+1, else → TRUE; (2) the circle `radius` — regulatory minimum
  2000 m enforced (radius [1,2000) → 422 LOCATION_VERIFICATION.INVALID_AREA), radius <1
  or center lat/long out of [-90,90]/[-180,180] → 400 OUT_OF_RANGE, non-CIRCLE areaType
  or missing area/center/radius → 400 INVALID_ARGUMENT. Precedence: syntactic area/body
  400s → identifier resolution + reserved error → regulatory INVALID_AREA 422 → verdict.
  `maxAge` validated (0..=int32; else 400 OUT_OF_RANGE) but otherwise ignored (location
  data always fresh, lastLocationTime=now via the self-contained rfc3339/civil_from_days
  formatter reused from sim_swap — no date-time dep). A supplied `device` is echoed as
  the single-identifier `VerifyLocationResponse.device`. Documented cuts: the 422 codes
  AREA_NOT_COVERED / UNABLE_TO_FULFILL_MAX_AGE / UNABLE_TO_LOCATE / UNSUPPORTED_IDENTIFIER
  / UNNECESSARY_IDENTIFIER declared-not-selected (CamaraSim doesn't distinguish 2- vs
  3-legged tokens); 409/500 are CamaraSim extensions so every reserved suffix is
  reachable. `x-correlator` echoed on every response. **No new deps** (reuses
  serde_json/shared scenarios+errors, local E.164/rfc3339). Spec: new
  `specs/location-verification/v3/openapi.yaml` — vendored 3.0.0 `POST /verify` with
  VerifyLocationRequest/Device/DeviceIpv4Addr/Area(CIRCLE)/Point/VerifyLocationResponse/
  DeviceResponse schemas, `$ref`-ing shared `errors.yaml` (CamaraError) + auth
  `camaraOAuth`, Generic400 (INVALID_ARGUMENT|OUT_OF_RANGE) + Generic422
  (INVALID_AREA|MISSING_IDENTIFIER) responses, `x-camarasim-scenarios` documenting both
  control planes + precedence + cuts. 360 tests green (was 335; +25: 4 units
  [verdict/matchRate-range/E.164/device-precedence] + 21 integration covering
  inside-TRUE+device-echo / …000-FALSE / odd-PARTIAL+matchRate / reserved-404+422 /
  ipv4+ipv6-echo / radius-regulatory-422 / radius-schema-400 / center-out-of-range-400 /
  non-CIRCLE-400 / missing-area-400 / maxAge-out-of-range-400 / valid-maxAge-200 /
  bad-phone-400 / empty-device-400 / NAI-rejected-400 / subject-fallback / subject-
  reserved / non-numeric-subject-TRUE / no-scope-403 / no-token-401 / x-correlator;
  also extended the catalog + openapi-server tests). — binary: 1365616 B (+37288 B)

- 2026-08-03 — Cross-cutting: **serve the vendored OpenAPI specs over HTTP**
  (DESIGN §9). New `src/apis/openapi.rs` merged into the app router: a `GET` route
  per mounted API at `/{api}/v{n}/openapi.yaml` (all 8 — number-verification/v1,
  sim-swap/v2, kyc-match/v0.3, device-reachability-status/v1, device-roaming-
  status/v1, device-identifier/v0.3, one-time-password-sms/v1, quality-on-demand/v1),
  each returning its spec as `Content-Type: application/yaml`. Also serves
  `/auth/openapi.yaml` and `/shared/errors.yaml` — every API spec `$ref`s them by the
  relative paths `../../auth/openapi.yaml` / `../../shared/errors.yaml`, which resolve
  against a `…/v{n}/openapi.yaml` URL exactly onto those two URLs, so a client that
  follows the `$ref`s finds them and every served spec is fully resolvable. Bodies are
  embedded at compile time with `include_str!` → in-memory `&'static str` (no runtime
  filesystem read on the request path, non-blocking; binary stays self-contained). `/`
  catalog now advertises each API's `spec_url`. These are simulator meta-endpoints (they
  publish the contracts), not a CAMARA business API, so there is no upstream CAMARA spec
  to vendor for them and no served-spec change was needed. **No new deps** (pure axum
  routing + static body). Binary grew +180360 B — this is exactly the ~176 KB of vendored
  spec text now embedded so the binary can serve its own contracts (a deliberate
  self-contained-binary trade-off, not code bloat). 335 tests green (was 330; +5:
  4 openapi units [serves an API spec byte-for-byte as application/yaml / serves every
  mounted API spec / serves the shared+auth `$ref` targets / unknown path → 404] + 1
  main integration [a spec is reachable as application/yaml through the full `app()`];
  also extended the catalog test to assert every entry carries a `spec_url`). NOTE: the
  sole remaining QoD item — TLS (`https://` sink) CloudEvents delivery — was deferred
  this pass: it needs a rustls TLS client whose crypto backend (ring / aws-lc-rs) pulls a
  C/cmake build toolchain (a real green-build risk in this headless container) and a large
  binary regression, so that dependency trade-off deserves a deliberate decision rather
  than an automated pass. — binary: 1328328 B (+180360 B)
- 2026-08-03 — Phase 3: Quality on Demand v1 — CloudEvents **`sinkCredential`
  (ACCESSTOKEN) auth** (CAMARA quality-on-demand 1.1.0, r3.2). A session created
  with a `credentialType: ACCESSTOKEN` `sinkCredential` now has its bearer token
  applied to every notification callback as `Authorization: Bearer <accessToken>`
  (RFC 6750 — same scheme as the resource server). New pure
  `notifications::sink_authorization(&Value) -> Option<String>` derives the header
  (ACCESSTOKEN + non-empty accessToken → `Bearer …`; PLAIN/REFRESHTOKEN/empty →
  None, a documented cut); `deliver`/`spawn_delivery` gained an `Option<auth>`
  param that writes the `Authorization` header line. The credential is kept in a
  process-global in-memory **side-store** keyed by sessionId
  (`store::insert_credential`/`take_credential`), separate from the `SessionInfo`
  map so the secret is never echoed by `GET`/`retrieve-sessions`; stored at
  creation (before the timers spawn) and **taken single-use** at delivery time by
  all three transitions (delete / expiry / network-termination), so it drops from
  memory as the session ends. `create_session` now uses the formerly-dead
  `sink_credential` field. Non-blocking + in-memory preserved (TCP write still
  async, off the request path). **No new deps** (raw-TCP POST + serde_json only;
  TLS still deferred). Spec: updated `specs/quality-on-demand/v1/openapi.yaml` —
  header prose, `createSession` description + `x-camarasim-scenarios` (new
  ACCESSTOKEN case), the `notifications` callback description, and expanded the
  `sinkCredential` schema (credentialType/accessToken/accessTokenType) to document
  the bearer auth and the PLAIN/REFRESHTOKEN cut. 330 tests green (was 326; +4:
  1 notifications unit [sink_authorization: ACCESSTOKEN→Bearer, empty/PLAIN/
  REFRESHTOKEN→None] + 1 notifications unit [deliver sends the Authorization
  header] + 1 store unit [credential stored then taken-once] + 1 v1 integration
  [create-with-sink+ACCESSTOKEN-cred → delete → the loopback sink receives the
  callback carrying `Authorization: Bearer <token>`, and the SessionInfo never
  echoes the secret]). — binary: 1147968 B (+3168 B)
- 2026-08-03 — Phase 3: Quality on Demand v1 — CloudEvents **`NETWORK_TERMINATED`**
  transition (CAMARA quality-on-demand 1.1.0, r3.2). New `v1::spawn_network_termination`:
  when `createSession` stores an `AVAILABLE` session whose identifier tail is `…001`
  (`NETWORK_TERMINATION_TAIL`) and that recorded a `sink`, it spawns a fire-and-forget
  timer (`tokio::spawn` + `tokio::time::sleep`, off the request path) that waits a short
  fixed grace (`NETWORK_TERMINATION_GRACE_SECS` = 1 s), then `store::remove`s the session
  and — if still present (a concurrent `deleteSession` loses, so exactly one event fires)
  — delivers a `UNAVAILABLE`/`NETWORK_TERMINATED` `qos-status-changed` CloudEvent to the
  sink via the existing `notifications::{qos_status_changed_event,spawn_delivery}`. `…001`
  is otherwise an ordinary AVAILABLE tail (distinct from `…000` REQUESTED and the reserved
  error suffixes); the grace is independent of the (typically much longer) `duration`, so
  the transition is provably *not* DURATION_EXPIRED. create_session now branches: `…001`
  AVAILABLE+sink → network-termination timer, any other AVAILABLE+sink → expiry timer.
  Non-blocking + in-memory preserved. **No new deps** (`tokio::time` already compiled).
  Spec: updated `specs/quality-on-demand/v1/openapi.yaml` — header prose, info description,
  `createSession` description + `x-camarasim-scenarios` (now three transitions + a new
  `…001` case), the `notifications` callback description, the `sink` field doc, and the
  `EventQosStatusChanged.statusInfo` description now all document NETWORK_TERMINATED fires
  for a `…001` AVAILABLE sink-bearing session (was "declared but not yet emitted"); only
  TLS/sinkCredential remain deferred. 326 tests green (was 325; +1 integration: create an
  AVAILABLE `…001` session with a sink and a long 86400 s duration → the loopback sink
  receives the NETWORK_TERMINATED CloudEvent within the timeout, well before expiry, and a
  later GET is 404, i.e. the session was evicted early). — binary: 1144800 B (+2840 B)
- 2026-08-03 — Phase 3: Quality on Demand v1 — CloudEvents **`DURATION_EXPIRED`**
  transition (CAMARA quality-on-demand 1.1.0, r3.2). New `v1::spawn_expiry`: after
  `createSession` stores an `AVAILABLE` session that recorded a `sink`, it spawns a
  fire-and-forget async timer (`tokio::spawn` + `tokio::time::sleep`, off the request
  path — non-blocking preserved) that waits until the session's `expiresAt`. On each
  wake it re-reads the live `SessionInfo`: gone or no longer `AVAILABLE` → stop (a
  `deleteSession` already fired `DELETE_REQUESTED`); `expiresAt` still in the future
  (e.g. after an `extend` pushed it out) → sleep again to the new instant; reached →
  `store::remove` the session and, if it was still present (concurrent delete loses),
  deliver a `UNAVAILABLE`/`DURATION_EXPIRED` `qos-status-changed` CloudEvent to the
  sink via the existing `notifications::{qos_status_changed_event, spawn_delivery}`.
  Insert-before-spawn so the timer always sees the stored session. Reused
  `parse_rfc3339_utc`/`rfc3339_utc`/`now_unix_secs`/`store::new_event_id`; **no new
  deps** (`tokio::time` already compiled; the `Duration` import was the only add). Also
  fixed a stale `delete_session` doc comment that still claimed no notification was
  emitted. Spec: updated `specs/quality-on-demand/v1/openapi.yaml` — header prose,
  `createSession` description + `x-camarasim-scenarios` (new expiry case, delivery on
  two transitions), the `notifications` callback description, the `sink` field doc, and
  the `EventQosStatusChanged.statusInfo` description now document `DURATION_EXPIRED`
  fires at expiry (NETWORK_TERMINATED still declared-not-emitted; TLS/sinkCredential
  still deferred). 325 tests green (was 324; +1 integration: create AVAILABLE session
  with sink + duration 1 s → the loopback sink receives the DURATION_EXPIRED CloudEvent
  within the timeout and a later GET is 404, i.e. the session was evicted). — binary:
  1141960 B (+5224 B)
- 2026-08-03 — Phase 3: Quality on Demand v1 — CloudEvents notifications (begun):
  the **`DELETE_REQUESTED`** `qos-status-changed` event on `deleteSession` (CAMARA
  quality-on-demand 1.1.0, r3.2; event `type`
  `org.camaraproject.quality-on-demand.v1.qos-status-changed`, confirmed against
  the r3.2 upstream `callbacks.notifications` + `CloudEvent` schema). New
  `src/apis/quality_on_demand/notifications.rs`: a pure `qos_status_changed_event`
  builder (CloudEvents 1.0 envelope — id/source/type/specversion/datacontenttype/
  time + `data{sessionId,qosStatus,statusInfo}`; statusInfo omitted when None) and
  an async `deliver` that POSTs it as `application/cloudevents+json` over a raw
  `tokio` TCP stream (no HTTP-client dep — DESIGN §11), fired via `spawn_delivery`
  (fire-and-forget, so a slow/unreachable sink never delays the `204`). Wired into
  `v1::delete_session`: on eviction, if the stored `SessionInfo` recorded a `sink`,
  spawn a `UNAVAILABLE`/`DELETE_REQUESTED` event to it. Added `store::new_event_id`
  (shares the UUID minter with `new_session_id`). Documented cuts: `http://` sinks
  only (no TLS client → `https://` parsed but not delivered to); unauthenticated
  (`sinkCredential` unused); only DELETE_REQUESTED emitted so far
  (DURATION_EXPIRED/NETWORK_TERMINATED still deferred). Non-blocking + in-memory
  preserved (TCP write is async and off the request path). New dep feature:
  tokio `io-util` (AsyncWriteExt/AsyncReadExt) — already compiled transitively by
  axum, ~0 added size. Spec: added the `notifications` callback (POST to
  `{$request.body#/sink}`, `application/cloudevents+json`, 204) to `createSession`
  + `CloudEvent`/`EventQosStatusChanged` schemas to
  `specs/quality-on-demand/v1/openapi.yaml`; updated create/delete descriptions +
  `x-camarasim-scenarios` (delete now fires DELETE_REQUESTED for sink-bearing
  sessions) and the `sink`/`sinkCredential` field docs; header prose no longer says
  notifications are wholly deferred. 324 tests green (was 317; +7: 1 store unit
  [event-id unique/uuid-shaped/distinct-from-session-ids] + 5 notifications units
  [event shape / statusInfo-omitted-when-None / parse_http_sink host+port+path &
  rejects https/junk / deliver POSTs a CloudEvent to an http listener / deliver to
  non-http is a no-op Ok] + 1 v1 integration [create-with-sink → delete → the
  loopback sink receives the DELETE_REQUESTED CloudEvent, 204 to the caller]). —
  binary: 1136736 B (+11992 B)
- 2026-08-03 — Phase 3: Quality on Demand v1 — `POST /retrieve-sessions`
  (CAMARA quality-on-demand 1.1.0, r3.2), operationId `retrieveSessionsByDevice`,
  scope `quality-on-demand:sessions:retrieve-by-device` (confirmed against the
  r3.2 upstream spec: `device` optional in `RetrieveSessionsInput`, 200 returns an
  array of `SessionInfo`, empty array — not 404 — when no sessions found). Added
  the route + `retrieve_sessions` handler to `src/apis/quality_on_demand/v1.rs`
  and a `store::find_by_device(&Value) -> Vec<Value>` to `store.rs` (lock held
  only for the scan + clone, never across await). Body `RetrieveSessionsInput`
  parsed with `deny_unknown_fields`; empty body accepted as `{}` (device optional,
  three-legged fallback). Two control planes (§7): the identifier (submitted
  `device` id [phoneNumber E.164, else NAI, else IPv4 publicAddress, else
  ipv6Address], else token subject → 422 MISSING_IDENTIFIER) — reserved suffix →
  canonical CAMARA error (`…404` → 404 device-not-found) — and the in-memory store,
  matched by each session's echoed `device`; happy path → 200 with the array
  (empty `[]` when none). Documented cut: a resolved identifier with no `device`
  echo (non-E.164 subject, no submitted device) matches nothing → `200 []`.
  Reused the module's `resolve_identifier`/E.164/`with_correlator` helpers; no new
  deps. `x-correlator` echoed on every response. Spec: added the
  `/retrieve-sessions` path (operationId `retrieveSessionsByDevice`) +
  `RetrieveSessionsInput` schema to `specs/quality-on-demand/v1/openapi.yaml` — 200
  (array of SessionInfo, some/none examples) + shared 400/401/403/404/422/429/500/
  503, `$ref`-ing auth `camaraOAuth`, `requestBody` required:false (documented
  leniency), `x-camarasim-scenarios` documenting the two control planes + the
  no-device-echo cut; header prose updated (retrieve-sessions no longer deferred,
  only CloudEvents remain). 317 tests green (was 307; +10: 1 store unit
  [find_by_device matches only that device's echo] + 9 integration covering
  returns-only-requested-device's-sessions [multi + filtering] / empty-array-when-
  none / reserved-suffix-404+429 / subject-fallback-matches / non-E.164-subject-
  empty / bad-body-unknown-field+bad-phone / retrieve-scope-isolation / no-token-
  401 / x-correlator). — binary: 1124744 B (+8088 B)
- 2026-08-03 — Phase 3: Quality on Demand v1 — `POST /sessions/{sessionId}/extend`
  (CAMARA quality-on-demand 1.1.0, r3.2), operationId `extendQosSession`, scope
  `quality-on-demand:sessions:update` (confirmed against the r3.2 upstream spec).
  Added the route + `extend_session` handler to `src/apis/quality_on_demand/v1.rs`
  and an atomic `store::update(id, f) -> Option<Value>` to `store.rs` (lock held
  only for the map access + pure closure, never across await). Body
  `ExtendSessionDuration{requestedAdditionalDuration}` parsed with
  `deny_unknown_fields` → precise 400 INVALID_ARGUMENT (missing/`<1`/unknown
  field). Two control planes (§7): the stored session (unknown/deleted id → 404
  NOT_FOUND) and the requested seconds — `>86400`, **or** current duration +
  requested `>86400`, → 400 `QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE` (the ceiling
  case is state-dependent on the stored session's current duration). On success it
  bumps `duration` in place and, for an `AVAILABLE` session, pushes `expiresAt` out
  by the same amount (new `parse_rfc3339_utc`/`days_from_civil`, the inverse of the
  existing `rfc3339_utc`/`civil_from_days` — no date/time dep), persists it, and
  returns the updated `SessionInfo` (200); a later `getSession` reflects the
  change. Factored the shared `duration_out_of_range`/`session_not_found` error
  helpers. `x-correlator` echoed on every response. No new deps. Spec: added the
  `/sessions/{sessionId}/extend` path (operationId `extendQosSession`) +
  `ExtendSessionDuration` schema to `specs/quality-on-demand/v1/openapi.yaml` — 200
  (SessionInfo) + 400 (both INVALID_ARGUMENT and DURATION_OUT_OF_RANGE) + shared
  401/403/404/429/500/503, `$ref`-ing auth `camaraOAuth`, `x-camarasim-scenarios`
  documenting the two control planes (incl. the state-dependent ceiling); header/
  description prose updated (extend no longer deferred). 307 tests green (was 295;
  +12: 1 store unit [update-in-place-then-none] + 1 handler unit [parse↔format
  roundtrip + rejects] + 10 integration covering available-grows-duration+expiry+
  persists / requested-grows-duration-only / unknown-404 / below-1-400 / above-max-
  out-of-range / ceiling-state-dependent-out-of-range / missing+unknown-field-400 /
  update-scope-isolation / no-token-401 / x-correlator-on-200+404). — binary:
  1116656 B (+8224 B)
- 2026-08-02 — Phase 3: Quality on Demand v1 — `DELETE /sessions/{sessionId}`
  (CAMARA quality-on-demand 1.1.0, r3.2), operationId `deleteSession`, scope
  `quality-on-demand:sessions:delete` (confirmed against the r3.2 upstream spec).
  Added the route (`get(get_session).delete(delete_session)` on the existing
  `/sessions/:session_id`) + `delete_session` handler to
  `src/apis/quality_on_demand/v1.rs`, and a `store::remove(id) -> Option<Value>`
  to `store.rs`. Keyed only on the in-memory store state (no identifier control
  plane — the path param is an opaque UUID): a session that exists is evicted →
  `204 No Content` (empty body); an unknown or already-deleted id → `404
  NOT_FOUND`; delete is single-use (second delete of the same id → 404).
  `x-correlator` echoed on both the 204 and the 404. No CloudEvents
  `DELETE_REQUESTED` notification (notifications still deferred). No new deps.
  Spec: added the `delete` operation (operationId `deleteSession`) to the
  `/sessions/{sessionId}` path in `specs/quality-on-demand/v1/openapi.yaml` —
  204 (no body) + the shared 401/403/404/429/500/503 responses, `$ref`-ing
  auth `camaraOAuth`, with `x-camarasim-scenarios` documenting the store-keyed
  cases; header/description prose updated (DELETE no longer listed as deferred).
  295 tests green (was 288; +7: 1 store unit [remove-once-then-none] + 6
  integration covering create→delete→204+gone / single-use-second-404 /
  unknown-404 / scope-isolation [read↮delete, delete↮read/create] / no-token-401
  / x-correlator-on-204+404). — binary: 1108432 B (+3544 B)
- 2026-08-02 — Phase 3: Quality on Demand v1 (begun) — `POST /sessions` **and**
  `GET /sessions/{sessionId}` (CAMARA quality-on-demand 1.1.0, release r3.2 — the
  latest stable; major v1, so mounted at `/quality-on-demand/v1`, confirmed against
  the r3.2 upstream spec: `createSession`/`getSession`, scopes
  `quality-on-demand:sessions:create`/`:read`). CamaraSim's first **resource-oriented**
  stateful API (a created `sessionId` is addressed by later requests). New
  `src/apis/quality_on_demand/{,store,v1}.rs` merged into the app router; `/` catalog now
  lists quality-on-demand v1. New in-memory store `store.rs` (process-global
  `Mutex<HashMap<String,Value>>`, lock never held across await, mirrors otp/store.rs):
  `new_session_id()` mints an opaque UUID-**v4-shaped** `sessionId` (`SHA-256(counter‖now)`
  first 16 bytes with version/variant nibbles set — matches CAMARA `format:uuid`, no
  uuid/rand dep), `insert`/`get` hold the rendered `SessionInfo`. `create_session`: body
  `CreateSession{device?,applicationServer,qosProfile,duration,devicePorts?,
  applicationServerPorts?,sink?,sinkCredential?}` parsed with `deny_unknown_fields` →
  precise 400 INVALID_ARGUMENT (missing/empty applicationServer, bad qosProfile pattern
  `^[a-zA-Z0-9_.-]+$` len 3–256, missing duration, bad E.164, unknown field). Three
  control planes (§7): (1) identifier (submitted `device` id [phoneNumber E.164-valid,
  else NAI, else IPv4 publicAddress, else ipv6Address], else token subject → 422
  MISSING_IDENTIFIER) — reserved suffix → canonical CAMARA error (so `…409`→409 CONFLICT,
  the QoD duplicate-session case), else tail `…000`/no-digits → `qosStatus:REQUESTED`
  (no startedAt/expiresAt) and any other tail → `qosStatus:AVAILABLE` (startedAt=now,
  expiresAt=now+duration, self-contained rfc3339 like sim_swap/device_identifier);
  (2) `duration` — `<1`→400 INVALID_ARGUMENT, `>86400`(24h fixed profile ceiling)→400
  `QUALITY_ON_DEMAND.DURATION_OUT_OF_RANGE`; (3) `qosProfile` — name containing
  `unavailable` (ci)→422 `QUALITY_ON_DEMAND.QOS_PROFILE_NOT_APPLICABLE`. Response echoes
  device (single id)/applicationServer/ports/sink; `sinkCredential` accepted but never
  echoed (secret) and unused — notifications deferred. `get_session`: `Path(sessionId)`
  → stored SessionInfo (200) or 404 NOT_FOUND. `x-correlator` echoed on every response.
  No new deps (reuses sha2/serde/shared scenarios+errors, local E.164/qosProfile/rfc3339).
  Spec: new `specs/quality-on-demand/v1/openapi.yaml` — vendored 1.1.0 `POST /sessions` +
  `GET /sessions/{sessionId}` with `CreateSession`/`SessionInfo`/`Device`/`DeviceIpv4Addr`/
  `ApplicationServer`/`PortsSpec` schemas, `$ref`-ing shared `errors.yaml` + auth
  `camaraOAuth`, `x-camarasim-scenarios` documenting the three control planes; noted the
  documented cuts (UNNECESSARY/UNSUPPORTED_IDENTIFIER + INVALID_SINK declared-not-selected;
  500/503 CamaraSim extension so every reserved suffix is reachable; DELETE/extend/
  retrieve-sessions/CloudEvents deferred so served spec matches code). 288 tests green
  (was 268; +20: 2 store units [unique-uuid-v4-shape/insert-get-unknown] + 3 handler units
  [qosProfile/E.164/rfc3339] + 15 integration covering available/requested-…000/
  create-then-get/unknown-404/reserved-…409+…404/duration-<1/duration->max/unavailable-
  profile/missing-fields+empty-appserver+unknown-field/bad-phone/subject-fallback/
  subject-reserved/create↔read-scope-isolation/auth/x-correlator + catalog). — binary:
  1080K (1104888 B; +32400 B)
- 2026-08-02 21:46Z — Phase 3 (begun): One Time Password SMS v1 — `POST /send-code` **and**
  `POST /validate-code` (CAMARA one-time-password-sms 1.1.1, release r3.2 — the latest stable;
  major v1, so mounted at `/one-time-password-sms/v1`, scope `one-time-password-sms:send-validate`
  for both ops, confirmed against the r3.2 upstream spec). CamaraSim's **first stateful API**. New
  `src/apis/one_time_password_sms/{,store,v1}.rs` merged into the app router; `/` catalog now lists
  one-time-password-sms v1. New in-memory store `store.rs` (process-global `Mutex<HashMap>`, lock
  never held across await, mirrors auth/codes.rs): `issue(code)` mints an opaque `authenticationId`
  (`base64url(SHA-256(counter‖now))`, no uuid/rand dep) with a 300 s TTL and a 3-wrong-attempt
  budget; `validate(id, code) -> Verdict{Ok|InvalidOtp|Failed|Expired}` — matching code consumes
  the entry (single use), an exhausting wrong attempt or an expiry evicts it, an unknown/consumed
  id is Expired. `send-code`: body `SendCodeRequest{phoneNumber,message}` (both required) parsed
  with `deny_unknown_fields` → precise 400 INVALID_ARGUMENT (missing/bad-E.164 phone, message
  missing `{{code}}` or >160 chars, unknown field); reserved suffix on `phoneNumber` → canonical
  CAMARA error (§7); else "sends" the deterministic code `otp_code(phone)` = last 6 digits
  zero-padded (so a headless caller can validate without a real SMS — documented) and returns
  `{authenticationId}`. `validate-code`: body `ValidateCodeRequest{authenticationId,code}` (both
  required, code ≤10 chars) → maps the store Verdict onto 204 / `ONE_TIME_PASSWORD_SMS.INVALID_OTP`
  / `…VERIFICATION_FAILED` / `…VERIFICATION_EXPIRED` (all 400). `x-correlator` echoed on all
  responses incl. the 204. No new deps (reuses sha2/base64/serde, local E.164 validator). Spec:
  new `specs/one-time-password-sms/v1/openapi.yaml` — vendored 1.1.1 `POST /send-code` +
  `/validate-code` with `SendCodeRequest`/`SendCodeResponse`/`ValidateCodeRequest` schemas,
  `$ref`-ing shared `errors.yaml` + auth `camaraOAuth`, `x-camarasim-scenarios` documenting the
  state-driven cases and the OTP-code derivation; noted the documented cut — a `…403` suffix
  yields generic PERMISSION_DENIED, so the three API-specific 403 codes (MAX_OTP_CODES_EXCEEDED/
  PHONE_NUMBER_NOT_ALLOWED/PHONE_NUMBER_BLOCKED) are declared but not input-selected. 268 tests
  green (was 249; +19: 4 store units [unique-id/correct-consume/unknown-expired/wrong→invalid→
  failed] + 2 handler units [otp_code/E.164] + 13 integration covering send/full-happy-path/
  single-use/invalid→failed/unknown-expired/reserved-error/bad-phone/message-placeholder+length/
  unknown-field/validate-missing-fields+long-code/scope/auth/x-correlator]). — binary: 1048K
  (1072488 B; +17216 B)
- 2026-08-02 — Phase 2 (Device Identifier complete → Phase 2 complete): Device Identifier v0.3
  — `POST /retrieve-ppid` (CAMARA Device Identifier 0.3.0, r2.2), operationId `retrievePPID`
  (confirmed against the r2.2 upstream spec), scope `device-identifier:retrieve-ppid`. Added the
  route + `retrieve_ppid` handler to `src/apis/device_identifier/v0_3.rs`, reusing the module's
  existing `RequestBody`/`Device`, `resolve_identifier`, correlator/E.164/rfc3339 helpers — no new
  files, no new deps (sha2 already a dependency). Same identifier resolution and reserved-error
  convention as retrieve-type/-identifier (§7): identifier = first present `device` id [phoneNumber
  E.164-validated, else NAI, else IPv4 publicAddress, else ipv6Address], else token subject → 422
  MISSING_IDENTIFIER; empty `device{}`/bad phone/unknown field → 400; reserved suffix → canonical
  CAMARA error. Happy path returns a stable, **pseudonymous** `ppid` = `SHA-256(identifier)` first
  16 bytes rendered as a UUID-shaped opaque token (irreversible one-way hash — never leaks the real
  IMEI — yet deterministic per device); `lastChecked` current; echoes the `device` used. Unlike the
  type/identity ops the model tail is deliberately NOT a control plane here (a PPID must not reveal
  the device type). Spec: added `/retrieve-ppid` path (operationId `retrievePPID`) + `PpidResponse`
  schema to `specs/device-identifier/v0.3/openapi.yaml`, `$ref`-ing shared `errors.yaml` + auth
  `camaraOAuth`, `x-camarasim-scenarios` documenting the cases (full shared error set exposed, noted
  vs canonical 400/401/403/404/422/429; header now says all three ops served). 249 tests green (was
  237; +12: 1 unit [ppid deterministic/UUID-shaped/pseudonymous] + 11 integration covering
  ppid+echo/different-identifier-differs/reserved-error/non-phone-ids/bad-phone+empty-device/subject-
  fallback/subject-reserved-error/non-numeric-subject/scope [retrieve-type token rejected]/auth/
  x-correlator). — binary: 1030K (1055272 B; +4216 B)
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
