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
`sink`/`sinkCredential` are accepted but not acted on (both documented cuts).
`x-correlator` echoed on every response.

Carrier Billing is now **stateful**: `GET /payments/{paymentId}`
(`retrievePayment`, scope `carrier-billing:payments:read`) reads a created
payment back. `createPayment` persists the charged payment in a new in-memory
store (`src/apis/carrier_billing/store.rs`; `Mutex<HashMap>`, lock never held
across await, mirroring QoD's session store), and `retrievePayment` returns it
verbatim (`200`) or `404 NOT_FOUND` for an unknown/never-created id. The
`paymentId` is opaque (UUID-shaped), so — unlike `createPayment` — there is no
reserved-identifier control plane here; the store state is the only one.

Carrier Billing now also **lists**: `GET /payments` (`retrievePayments`, scope
`carrier-billing:payments:read`) returns a `PaymentArray` of every payment in
the store (new `store::all()` scan) — `200` with an empty array when none
(CAMARA lists never `404`). The store state is the only control plane. The real
op's `page`/`perPage`/`order`/`paymentCreationDate.gte|lte`/`paymentStatus`/
`merchantIdentifier` query parameters are accepted but not applied, and payments
aren't scoped per client (documented cuts, mirroring the Geofencing list).

Carrier Billing's **two-step flow has begun** (contrary to an earlier note, 0.5.0
*does* cover it): `POST /payments/prepare` (`preparePayment`, scope
`carrier-billing:payments:create`) is now live — the **reserve** step. Unlike the
one-step `createPayment`, it reserves (does not charge) the amount: a happy path
returns `201` with `paymentStatus: "reserved"` and **no** `paymentDate`, and the
reservation is persisted in the same in-memory store so `retrievePayment` reads it
back and — in later slices — `confirmPayment` (→ `succeeded`) / `cancelPayment`
(→ `cancelled`) can act on it. It shares `createPayment`'s two control planes
(DESIGN §7): the reserved phone number (submitted `amountTransaction.phoneNumber`,
else token subject — reserved suffix → canonical CAMARA error, malformed → 400,
unidentifiable → 422 `MISSING_IDENTIFIER`) and the requested `amount` (`< 0.001` →
400, `> 1000` → 422 `CARRIER_BILLING.UNAUTHORIZED_AMOUNT`). The
`pending_validation`/`validationInfo` (OTP) path and the 409 `ALREADY_EXISTS`
duplicate-session case are deferred to the `validatePayment` slice. `x-correlator`
echoed on every response.

The two-step flow's **validate** step is now live: `POST /payments/{paymentId}/
validate` (`validatePayment`, scope `carrier-billing:payments:write`). A
`preparePayment` for a phone number ending in `888` now lands in
`pending_validation`, carrying a `validationInfo` (`action: "validate"`, an
`authorizationId`); the expected OTP `code` (the reserved number's last six
digits, zero-padded — deterministic, mirroring OTP SMS) is held in a secret
in-memory side-store (`store::PendingValidation`) apart from the echoed payment.
`validatePayment` clears it atomically (`store::validate_pending`): correct
`authorizationId` + `code` → `204` (reservation → `reserved`, `validationInfo`
dropped); wrong `authorizationId` → 400 `CARRIER_BILLING.INVALID_AUTHORIZATION_ID`;
wrong `code` → 400 `CARRIER_BILLING.INVALID_CODE` until the 3-attempt budget is
spent → 400 `CARRIER_BILLING.VALIDATION_FAILED` (reservation → `denied`); a
settled payment → 409 `ALREADY_EXISTS`; unknown id → 404.

The two-step flow's **confirm** step is now live too: `POST /payments/{paymentId}/
confirm` (`confirmPayment`, `carrier-billing:payments:write`) charges a `reserved`
payment → `succeeded` (stamping `paymentDate`) and answers `202 Accepted` (no
body). Keyed only on the store state (opaque `paymentId`, no reserved-identifier
plane): `reserved` → 202; already `succeeded` → 409
`CARRIER_BILLING.PAYMENT_CONFIRMED`; already `cancelled` → 409
`CARRIER_BILLING.PAYMENT_CANCELLED`; other non-`reserved`
(`pending_validation`/`denied`) → 409 `CONFLICT`; unknown → 404. The transition
runs atomically in a new `store::confirm`; the optional `phoneNumber` body is
accepted-not-applied.

The two-step flow's **cancel** step is now live too, **completing the two-step
flow**: `POST /payments/{paymentId}/cancel` (`cancelPayment`,
`carrier-billing:payments:write`) releases a `reserved` payment → `cancelled`
(no `paymentDate`, since nothing is charged) and answers `202 Accepted` (no
body). Keyed only on the store state (opaque `paymentId`, no reserved-identifier
plane): `reserved` → 202; already `cancelled` → 409
`CARRIER_BILLING.PAYMENT_CANCELLED`; already `succeeded` → 409
`CARRIER_BILLING.PAYMENT_CONFIRMED` (a charged payment can no longer be
cancelled); other non-`reserved` (`pending_validation`/`denied`) → 409
`CONFLICT`; unknown → 404. The transition runs atomically in a new
`store::cancel` (mirroring `store::confirm`); the optional `phoneNumber` body is
accepted-not-applied. Only charging notifications on `sink` remain before Carrier
Billing v0.5 is complete.

**Charging notifications on `sink` have begun** (`src/apis/carrier_billing/
notifications.rs`): a successful one-step `createPayment` charge now delivers a
`payment-completed` CloudEvent (`data.status: succeeded`, `paymentId`,
`description`, `paymentDate`) to the request's `sink` — best-effort,
fire-and-forget over a raw `tokio` TCP stream (no HTTP-client dep, `http://`
sinks only, mirroring QoD/Geofencing), off the request path so a slow sink never
delays the `201`. An `ACCESSTOKEN` `sinkCredential`'s bearer token is applied as
an `Authorization: Bearer` header (RFC 6750); the `sink` is used only to notify
and never persisted with the payment (so `retrievePayment` still omits it).
**`preparePayment` → `payment-reserved`** is now in place too: a successful
two-step `preparePayment` that lands in the `reserved` state delivers a
`payment-reserved` CloudEvent (`data.status: succeeded`, `paymentId`,
`description`; no `paymentDate` — nothing is charged) to its `sink`, over the
same fire-and-forget raw-TCP transport with the same ACCESSTOKEN `sinkCredential`
bearer handling. **`preparePayment` → `payment-pending-validation`** is now in
place too: a `…888` reservation that lands in `pending_validation` delivers a
`payment-pending-validation` CloudEvent (`data.status: succeeded`, no
`paymentDate` and — per the CAMARA schema — no `validationInfo`) to its `sink`,
over the same fire-and-forget raw-TCP transport with the same ACCESSTOKEN
`sinkCredential` bearer handling; it is mutually exclusive with
`payment-reserved`. The first **two-step terminal** event is now in place too:
a successful `confirmPayment` delivers a `payment-completed` CloudEvent
(`data.status: succeeded`, with `paymentDate`) to the `sink` recorded at
`preparePayment`. Because the confirm body carries no `sink`, the sink and any
`ACCESSTOKEN` `sinkCredential` bearer are stashed at prepare-time in a
`paymentId`-keyed in-memory side-store (`store::insert_notify`/`take_notify`,
mirroring QoD's credential side-store) and taken **single-use** when the charge
goes through (so exactly one terminal event fires; the secret is never echoed by
`retrievePayment`). The second **two-step terminal** event is now in place too:
a successful `cancelPayment` delivers a `payment-cancelled` CloudEvent
(`data.status: failed`, no `paymentDate` — nothing is charged) to the `sink`
recorded at `preparePayment`, taken **single-use** from the same notify
side-store (so a reservation fires exactly one terminal event — confirm *or*
cancel).

The final terminal event, **`payment-denied`**, is now in place too: a
`validatePayment` that exhausts its OTP-attempt budget denies the reservation
(→ `denied`) and delivers a `payment-denied` CloudEvent (`data.status: failed`,
no `paymentDate` — nothing is charged) to the `sink` recorded at
`preparePayment`, taken **single-use** from the same notify side-store (so a
`…888` reservation fires exactly one terminal event — confirm, cancel, *or*
deny). ACCESSTOKEN `sinkCredential` bearer applied. **This completes Carrier
Billing v0.5 charging notifications** — only TLS (`https://`) sink delivery
remains deferred across the stateful APIs.

**Phase 5 (other CAMARA APIs) — Call Forwarding Signal v0.4** has begun, a new
stateless, non-spatial, phone-number-keyed anti-fraud API mounted at its real
published version `/call-forwarding-signal/v0.4` (CAMARA 0.4.0, release r3.3 —
the latest published, like KYC Match / Location Retrieval). `POST
/unconditional-call-forwardings` is live (scope
`call-forwarding-signal:unconditional-call-forwardings:read`, operationId
`retrieveUnconditionalCallForwarding`): it answers whether *unconditional* call
forwarding is active for a line — `{active: boolean}`. Faithful to CAMARA's
two-legged/three-legged identifier rule (the `phoneNumber` body is valid only in
two-legged auth): a submitted `phoneNumber` on a three-legged **line** token
(E.164 `sub`) → 422 `UNNECESSARY_IDENTIFIER`; no number + a non-line subject →
422 `MISSING_IDENTIFIER`; otherwise the identifier is the submitted number
(two-legged) or the E.164 subject (three-legged). Two control planes (DESIGN §7):
the identifier's reserved error suffix → canonical CAMARA error, else its
trailing three digits' parity — **odd → `active:true`** (forwarding on), **even
(incl. `…000`) → `active:false`** (the common case). `x-correlator` echoed on
every response. The companion `POST /call-forwardings` (`retrieveCallForwarding`,
scope `call-forwarding-signal:call-forwardings:read`) is now live too, reporting
the CAMARA `CallForwardingSignal` — the **set** of active forwarding types
(`inactive`/`unconditional`/`conditional_busy`/`conditional_not_reachable`/
`conditional_no_answer`). Same identifier resolution + reserved-error convention;
the second control plane is the identifier's trailing three digits taken as a
4-bit mask (`digits % 16`) over the four active types (bit 0 = `unconditional`, so
an odd tail lines up with the unconditional endpoint), zero mask → `["inactive"]`.
**This completes Call Forwarding Signal v0.4.**

**Phase 5 (other CAMARA APIs) — Number Recycling v0.2** is now live, a new
stateless, non-spatial, phone-number-keyed account-integrity API mounted at its
real published version `/number-recycling/v0.2` (CAMARA 0.2.0, release r2.2 —
the latest published, like KYC Match / Call Forwarding Signal). `POST /check`
(scope `number-recycling:check`, operationId `checkNumberRecycling`) answers
whether the **subscriber** behind a phone number changed after a caller-supplied
`specifiedDate` (an anti-fraud signal) — `{ "phoneNumberRecycled": boolean }`.
Faithful to CAMARA's two-legged/three-legged identifier rule (the `phoneNumber`
body is valid only in two-legged auth): a submitted `phoneNumber` on a
three-legged **line** token (E.164 `sub`) → 422 `UNNECESSARY_IDENTIFIER`; no
number + a non-line subject → 422 `MISSING_IDENTIFIER`. **Two** control planes
(DESIGN §7): the identifier's reserved error suffix → canonical CAMARA error;
else its trailing three digits read as **days since the subscriber last changed**
compared against `specifiedDate` — recycled iff the change is strictly after the
reference date (`(today − specifiedDate) > digits`), so `specifiedDate` is a
genuine second control plane (the *same* number flips true↔false as the date
moves). `specifiedDate` is validated: malformed/impossible → 400
`INVALID_ARGUMENT`; future → 400 `OUT_OF_RANGE`. A self-contained civil-date
parser/formatter (Howard Hinnant `days_from_civil`/`civil_from_days`, no new
dependency) does the date maths. `x-correlator` echoed on every response.

**Phase 5 (other CAMARA APIs) — KYC Age Verification v0.1** is now live, a new
stateless, non-spatial, phone-number-keyed identity API mounted at its real
published version `/kyc-age-verification/v0.1` (CAMARA 0.1.0, release r2.2 —
the latest published, like KYC Match / Number Recycling). `POST /verify` (scope
`kyc-age-verification:verify`, operationId `verifyAge`) answers whether the
line's holder is **at or above** a caller-supplied `ageThreshold` — a
privacy-preserving `{ ageCheck: "true"|"false"|"not_available" }`, never a
birthdate. Faithful to CAMARA's two-legged/three-legged identifier rule (a
`phoneNumber` on a line token → 422 `UNNECESSARY_IDENTIFIER`; no number + a
non-line subject → 422 `MISSING_IDENTIFIER`). **Three** control planes (DESIGN
§7): `ageThreshold` range (`0..=120`, else 400 `OUT_OF_RANGE`); the identifier's
reserved error suffix → canonical CAMARA error; and the identifier's trailing
three digits read as the **held age** (`d % 100`) compared to `ageThreshold`
(`held ≥ threshold` → `"true"`, else `"false"`; `…000` → `"not_available"`), so
`ageThreshold` is a genuine second plane (the same number flips true↔false as
the threshold moves). The body's identity attributes add optional response
fields: any of `idDocument`/`name`/`givenName`/…/`email` → `identityMatchScore`
(fixed 90, no real backend); `idDocument` → `verifiedStatus: true`; and the
`includeContentLock`/`includeParentalControl` toggles opt into
`contentLock`/`parentalControl` (a minor `< 18` → `"true"`, adult → `"false"`,
unknown age → `"not_available"`). `x-correlator` echoed on every response.

**Phase 5 (other CAMARA APIs) — Device Swap v1** has begun, a new stateless,
non-spatial, phone-number-keyed anti-fraud API — the **device** counterpart of
SIM Swap — mounted at its real published version `/device-swap/v1` (CAMARA
1.0.0, release r3.2; `main` is `wip`). `POST /check` (scope `device-swap:check`,
operationId `checkDeviceSwap`) answers whether the device bound to a line was
swapped within the last `maxAge` hours — `{ "swapped": boolean }`. Faithful to
CAMARA's two-legged/three-legged identifier rule (the `phoneNumber` body is valid
only in two-legged auth): a submitted `phoneNumber` on a three-legged **line**
token (E.164 `sub`) → 422 `UNNECESSARY_IDENTIFIER`; no number + a non-line
subject → 422 `MISSING_IDENTIFIER`. Two control planes (DESIGN §7): the
identifier's reserved error suffix → canonical CAMARA error; else its trailing
three digits are **hours since the last device swap** and `swapped = hoursAgo <
maxAge`, making `maxAge` (1–2400, default 240) a genuine second control plane
(out-of-range `maxAge` → 400 `OUT_OF_RANGE`; an identifier with no digits →
never swapped). `x-correlator` echoed on every response. Device Swap v1 is now
**complete**: `POST /retrieve-date` (`device-swap:retrieve-date`,
`retrieveDeviceSwapDate`) is live too — the companion to `check`, reporting *when*
the device was last swapped as `{ latestDeviceChange: <RFC 3339 UTC | null>,
monitoredPeriod: <days> }`. Same identifier resolution and reserved-error
convention as `check`; the identifier's trailing three digits are hours-since-swap,
so `latestDeviceChange` = *now − hoursAgo h* when that swap is inside the fixed
monitored period (240 h = 10 days, aligned to `check`'s default `maxAge` so the two
operations agree), else `null`. `monitoredPeriod` (10 days) is always returned. A
self-contained RFC 3339 UTC formatter (no new dependency, mirroring SIM Swap's
`retrieve-date`) renders the timestamp.

**Phase 5 (other CAMARA APIs) — KYC Tenure v0.2** is now live, a new stateless,
non-spatial, phone-number-keyed identity/anti-fraud API (part of Know Your
Customer) mounted at its real published version `/kyc-tenure/v0.2` (CAMARA
0.2.0, release r2.2; `main` is `wip`). `POST /check-tenure` (scope
`kyc-tenure:check-tenure`, operationId `checkTenure`) establishes a level of
trust by answering whether the current end user has held the line **since at
least** a caller-supplied `tenureDate` — a privacy-preserving
`{ tenureDateCheck: boolean, contractType: PAYG|PAYM|Business }`, never the
actual tenure length. Faithful to CAMARA's two-legged/three-legged identifier
rule (a `phoneNumber` on a line token → 422 `UNNECESSARY_IDENTIFIER`; no number
+ a non-line subject → 422 `MISSING_IDENTIFIER`). Two control planes (DESIGN
§7): the identifier's reserved error suffix → canonical CAMARA error; else its
trailing three digits read as **days of tenure** — `tenureDateCheck` is true iff
the tenure started on or before `tenureDate` (`(today − tenureDate) <= digits`),
so `tenureDate` is a genuine second plane (the same number flips true↔false as
the date moves). `tenureDate` validated: malformed/impossible → 400
`INVALID_ARGUMENT`; future → 400 `OUT_OF_RANGE`. `contractType` is derived
deterministically from the same trailing digits (`digits % 3`). A self-contained
civil-date parser (no new dependency, mirroring Number Recycling) does the date
maths. `x-correlator` echoed on every response.

**Phase 5 (other CAMARA APIs) — Blockchain Public Address v0.3** has begun, a new
stateless, non-spatial, phone-number-keyed Web3-onboarding API mounted at its real
published version `/blockchain-public-address/v0.3` (CAMARA 0.3.0, release r2.2 —
the latest published, like KYC Match / KYC Tenure). `POST
/blockchain-public-addresses/retrieve-blockchains` (scope
`blockchain-public-address:read`, operationId `retrieveBlockchainPublicAddress`)
is live: it returns the array of `BlockchainPublicAddressResponse` records
(`id` / `blockchainPublicAddress` / `blockchainNetworkId` / `currency`) the
subscriber behind a phone number has bound to their line. Faithful to the 0.3.0
schema, `phoneNumber` is **required** (no three-legged fallback; missing/malformed
→ 400 `INVALID_ARGUMENT`). Two control planes (DESIGN §7): the number's reserved
error suffix → canonical CAMARA error; else its trailing three digits `d` decide
the address set — `d == 0` (`…000`/no digits) → `200 []` (nothing bound, a list
never 404s), else `((d - 1) % 3) + 1` addresses (1–3), the `i`-th on
`NETWORKS[(d + i) % 6]` from a fixed 6-entry CAIP-2 EVM table (so the chain is a
genuine second plane). Each `0x…` address and UUID-shaped `id` is deterministic
(SHA-256, no new dep); addresses are lowercase (not EIP-55 checksummed — a
documented cut). `x-correlator` echoed on every response. The API is now
**stateful**: `POST /blockchain-public-addresses` (`bindBlockchainPublicAddress`,
scope `blockchain-public-address:create`) binds an on-chain address to a phone
number, persisting it in a new in-memory store
(`src/apis/blockchain_public_address/store.rs`; `Mutex<HashMap>`, the binding
`id` derived from the `(phoneNumber, network, address)` triple so a re-bind
collides), returning `201 {id}`. Three control planes (DESIGN §7): the
phoneNumber reserved-error suffix → canonical error; request validation (bad
`blockchainNetworkId` → 400 `…INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER`, bad EVM
address → 400 `INVALID_ARGUMENT`, lone `nonce`/`signature` → 400
`…BOTH_NONCE_SIGNATURE_REQUIRED`, both → 422 `…UNSUPPORTED_ENHANCED_VALIDATION`
since the simulator has no chain for enhanced ownership validation); and the
store (re-binding the same triple → 409 `ALREADY_EXISTS`). The `retrieve` read
op still answers from the deterministic-synthetic model (it does not read the
store — a deferred reconciliation). `DELETE /blockchain-public-addresses/{id}`
(`deleteBlockchainPublicAddress`, scope `blockchain-public-address:delete`) is
now live too — it unbinds the stored binding named by the opaque `id` from the
same store (new `store::remove`): present → `204 No Content` (single-use),
absent → `404 NOT_FOUND`. Keyed only on store state (the `id` is opaque, so no
reserved-identifier plane, mirroring QoD `deleteSession` / Carrier Billing);
bindings aren't scoped per subscriber, so the spec's `sub`-ownership check isn't
enforced (a documented cut). `x-correlator` echoed on every response. **This
completes Blockchain Public Address v0.3.**

**Phase 5 (other CAMARA APIs) — Simple Edge Discovery v2** is now live, a new
stateless, non-spatial, **device-keyed** edge/MEC-discovery API mounted at its
real published version `/simple-edge-discovery/v2` (CAMARA 2.0.1, meta-release
r2.3 — the latest published, like KYC Match / Blockchain Public Address). `POST
/retrieve-closest-edge-cloud-zone` (scope `simple-edge-discovery:read`,
operationId `readClosestEdgeCloudZone`) returns the `EdgeCloudZone`
(`{edgeCloudZoneId, edgeCloudZoneName, edgeCloudProvider}`) with the lowest
network latency to a device — a zone identity, never the device's location. The
device is resolved from the submitted `device` object (phoneNumber → NAI → IPv4
`publicAddress` → ipv6Address; mirroring the device-status family), else the
token subject, honouring the CAMARA two-legged/three-legged rule (a `device`
submitted on a three-legged **device** token → 422 `UNNECESSARY_IDENTIFIER`; no
`device` + a non-device subject → 422 `MISSING_IDENTIFIER`; a `device` carrying
no identifier → 400 `INVALID_ARGUMENT`). Two control planes (DESIGN §7): the
identifier's reserved error suffix → canonical CAMARA error; else its trailing
three digits index a fixed 6-entry edge-zone table (`% 6`; `…000`/no-digits →
entry 0), so the reported zone is a genuine second plane. The `edgeCloudZoneId`
is UUID-shaped and derived from the zone via SHA-256 (deterministic and **stable
per zone** — a zone has one id regardless of which device resolves to it; no new
dependency). The `device` is echoed back only for a phoneNumber-keyed request
(the CAMARA `DeviceResponse` carries only `phoneNumber`). `x-correlator` echoed
on every response. **This completes Simple Edge Discovery v2.**

**Phase 5 (other CAMARA APIs) — Customer Insights v0.2** is now live, a new
stateless, non-spatial, phone-number-keyed identity/anti-fraud API mounted at its
real published version `/customer-insights/v0.2` (CAMARA 0.2.0, release r2.2 —
the latest published, like KYC Match / KYC Tenure / Number Recycling). `POST
/scoring/retrieve` (scope `customer-insights:scoring:read`, operationId
`retrieveScoring`) returns a privacy-preserving risk/trust **score** for a line —
`{ scoringType, scoringValue }`, a single number on a caller-chosen scale, never
the underlying data: `gaugeMetric` (a 300 highest-risk … 850 lowest-risk band) or
`veritasIndex` (a 0 lowest-risk … 19 highest-risk index). Faithful to CAMARA's
two-legged/three-legged identifier rule (a `phoneNumber` on a line token → 422
`UNNECESSARY_IDENTIFIER`; no number + a non-line subject → 422
`MISSING_IDENTIFIER`, or — when only an `idDocument` is supplied — 422
`CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED`, since the sim scores by phone
number only). Two control planes (DESIGN §7): the identifier's reserved error
suffix → canonical CAMARA error; else its trailing three digits `d` fix the score
on the requested scale (`gaugeMetric` → `300 + (d % 551)`, `veritasIndex` →
`d % 20`), so `scoringType` is a genuine second plane (the same number reads a
different value on each scale). `scoringType` is required (missing/unknown → 400
`INVALID_ARGUMENT`); `idDocument`, when supplied, must be a non-empty string ≤ 30
chars. No new dependency. `x-correlator` echoed on every response. **This
completes Customer Insights v0.2.**

**Phase 5 (other CAMARA APIs) — Connected Network Type v0.2** is now live, a new
stateless, non-spatial, device-keyed radio-access API mounted at its real
published version `/connected-network-type/v0.2` (CAMARA 0.2.0, release r1.2 —
the latest published, like Simple Edge Discovery / KYC Match). `POST /retrieve`
(scope `connected-network-type:read`, operationId `getConnectedNetworkType`)
answers which mobile technology a device is attached to —
`{ connectedNetworkType: 2G|3G|4G|5G|UNKNOWN, lastStatusTime, device? }`, never
its location. Device-object identifier resolution + the CAMARA two-legged /
three-legged rule (mirrors Simple Edge Discovery): a submitted `device` on a
three-legged **line** token (E.164 `sub`) → 422 `UNNECESSARY_IDENTIFIER`; no
`device` + a non-line subject → 422 `MISSING_IDENTIFIER`; an empty `device`
object → 400 `INVALID_ARGUMENT`. Two control planes (DESIGN §7): the
identifier's reserved error suffix → canonical CAMARA error; else its trailing
three digits index a fixed newest-first table (`digits % 5` over
`[5G,4G,3G,2G,UNKNOWN]`; `…000`/no digits → `5G`), so the reported technology is
a genuine second plane. `lastStatusTime` is the current instant (RFC 3339 UTC,
self-contained formatter, no new dep) — `null` for an `UNKNOWN` attachment. The
`device` is echoed only for a phoneNumber-keyed request (CAMARA `DeviceResponse`
carries only phoneNumber). Only the base `POST /retrieve` is modelled; the
API's separate event-subscription surface is out of scope. `x-correlator`
echoed on every response. **This completes Connected Network Type v0.2.**

**Phase 5 (other CAMARA APIs) — Connectivity Insights v0.6** is now live, a new
stateless, non-spatial, device-keyed API mounted at its real published version
`/connectivity-insights/v0.6` (CAMARA 0.6.0, release r3.2 — the latest published
Fall25 version). `POST /check-network-quality` (scope
`connectivity-insights:check`, operationId `checkNetworkQuality`) answers whether
the network can meet an application's quality requirements for a device —
per-KPI *policy-fulfilment* verdicts (`packetDelayBudget` /
`targetMinDownstreamRate` / `targetMinUpstreamRate` / `packetlossErrorRate` /
`jitter` → `"meets the application requirements"` / `"unable to meet…"`) plus
coarse `additionalKPIs` (`signalStrength` / `connectivityType`), never raw
measurements or a location. Device-object identifier resolution mirrors Connected
Network Type / Simple Edge Discovery (submitted `device` id, else token subject);
faithful to 0.6.0 there is **no** `UNNECESSARY_IDENTIFIER` (a device on a line
token is simply used), and no device + a non-line subject → 422
`MISSING_IDENTIFIER`. Required `applicationProfileId` is validated as a UUID but
not resolved against a profile store (a documented cut — the sim has no
Application Profiles store); required `applicationServer` needs ≥1 address;
optional `applicationServerPorts` are range-checked (`0..=65535`, else 400
`OUT_OF_RANGE`) and `monitoringTimeStamp` lightly validated. Two control planes
(DESIGN §7): the identifier's reserved error suffix → canonical CAMARA error;
else the identifier's trailing three digits' low five bits form an *unmet mask*
(one bit per KPI, in the fixed order above) — `…000` → all KPIs met (a healthy
default, `excellent` / `5G-SA`), `…031` → all unmet (`no signal` / `3G`) — with
`additionalKPIs` degrading coherently with the count met. `device` echoed only
for a phoneNumber request. No new dependency. `x-correlator` echoed on every
response.

**Phase 5 (other CAMARA APIs) — Region Device Count v0.2** is now live, a new
**area-keyed** (not identifier-keyed), stateless aggregate-count API mounted at
its real published version `/region-device-count/v0.2` (CAMARA 0.2.0, release
r2.2 — the latest published, like Customer Insights / Connectivity Insights).
`POST /count` (scope `region-device-count:count`, operationId `count`) answers
how many devices are in a geographic region (a `CIRCLE` or a `POLYGON`) during an
optional time interval — a privacy-preserving `{ count?, status }`, never a
per-device location. Because there is no phone/device identifier, its control
planes (DESIGN §7) key off the **area geometry**: the region's characteristic
radius `r` (a circle's `radius`; a polygon's `sqrt(area/π)`, area by a
self-contained equirectangular shoelace, no new dep) drives both a **size plane**
(`r > 1 000 000 m` → 400 `REGION_DEVICE_COUNT.UNSUPPPORTED_REQUEST`;
`500 000 < r ≤ 1 000 000 m` with no `sink` → 400
`REGION_DEVICE_COUNT.UNSUPPORTED_SYNC_RESPONSE`, with a `sink` answered sync) and
a **status/count plane** keyed on `round(r)`'s trailing three digits (`…429` →
429; `…001` → `PART_OF_AREA_NOT_SUPPORTED`; `…002` → `AREA_NOT_SUPPORTED`; `…003`
→ `DENSITY_BELOW_PRIVACY_THRESHOLD`; `…004` → `TIME_INTERVAL_NO_DATA_FOUND`; else
→ `SUPPORTED_AREA`). When a `count` is returned it is proportional to the area at
a fixed 500 devices/km² and **narrowed by `filter`** (a genuine second plane —
each `roamingStatus`/`deviceType` category contributes its fixed share). Full
CAMARA validation: `INVALID_CIRCLE_AREA`/`INVALID_POLYGON_AREA`, the both-or-
neither time rule (`TIME_INVALID_ARGUMENT`) + `INVALID_END_DATE` (self-contained
RFC 3339 parser, no new dep), empty/bad `filter` → `INVALID_ARGUMENT`, and
`sinkCredential` → `INVALID_CREDENTIAL`/`INVALID_TOKEN`. The asynchronous
`sink`/CloudEvents delivery surface (and its 410 GONE) is a documented cut. No
new dependency. `x-correlator` echoed on every response.

**Phase 5 (other CAMARA APIs) — Device Visit Location vwip** is now live, a new
stateless, device-keyed anti-fraud / identity-assurance API mounted at its
canonical **`vwip`** base path `/device-visit-location/vwip` (CAMARA
device-visit-location — the API has **no released version yet**, its upstream
`main` spec is versioned `wip`, so — unlike the published sub-1.0 APIs like KYC
Match v0.3 / Location Retrieval v0.4 — the canonical path segment is literally
`vwip`; a numbered version will be added when CAMARA cuts a release). `POST
/retrieve` (scope `device-visit-location:retrieve`, operationId
`retrieveDeviceVisitLocation`) answers **where a device has been** over a
caller-supplied time window — a coarse `geoCodeList` of `{ countryCode,
codeType: "PostalCode", codeValue }` geographic codes, never precise
coordinates. Device-object identifier resolution mirrors Connected Network Type
/ Simple Edge Discovery (submitted `device` id — phoneNumber → NAI → IPv4
publicAddress → ipv6Address — else token subject) with the two-legged /
three-legged rule (a `device` on a line token → 422 `UNNECESSARY_IDENTIFIER`; no
device + a non-line subject → 422 `MISSING_IDENTIFIER`; empty `device` → 400
`INVALID_ARGUMENT`). **Two control planes** (DESIGN §7): (1) the identifier —
reserved error suffix → canonical CAMARA error (checked first, so it dominates);
else a `…000` / no-digit tail → 404 `DEVICE_VISIT_LOCATION.DATA_NOT_FOUND` (no
recorded visit; "no data" is a 404 rather than an empty list because
`geoCodeList` is `minItems: 1`); else the trailing three digits `d` yield
`((d - 1) % 3) + 1` codes (1–3), the `i`-th on `COUNTRIES[(d + i) % 6]` over a
fixed `[US, GB, DE, FR, ES, IT]` table with a deterministic 5-digit postal code,
so both the number of places and the countries are a genuine second plane; and
(2) the required RFC 3339 `startTime`/`endTime` window — malformed → 400
`INVALID_ARGUMENT`, `endTime < startTime` → 400
`DEVICE_VISIT_LOCATION.INVALID_END_DATE` (self-contained RFC 3339 parser, no new
dep, mirroring Region Device Count). `x-correlator` echoed on every response. No
new dependency.

**QoS Provisioning v0.3** has begun (`/qos-provisioning/v0.3`; CAMARA
qos-provisioning 0.3.0, release r3.2 — part of the QualityOnDemand repo). It is
the **provisioning** (open-ended) counterpart of Quality on Demand's bounded
sessions: a stateful, resource-oriented API that mints indefinite QoS
`qos-assignments`. `POST /qos-assignments` (`createQosAssignment`, scope
`qos-provisioning:qos-assignments:create`) provisions a `qosProfile` for a
device, mints an opaque UUID-shaped `assignmentId` (new in-memory store
`src/apis/qos_provisioning/store.rs`, mirroring QoD's store), persists the
rendered `AssignmentInfo`, and returns 201. `GET /qos-assignments/{assignmentId}`
(`getQosAssignmentById`, scope `…:read`) reads it back (200) or 404 `NOT_FOUND`.
Device-object identifier resolution + the two-legged/three-legged rule (device on
a line token → 422 `UNNECESSARY_IDENTIFIER`; no device + non-line subject → 422
`MISSING_IDENTIFIER`). Control planes (DESIGN §7): the identifier's reserved-error
suffix → canonical CAMARA error (`…409` → 409 `CONFLICT`, the "existing
provisioning for the same device" case); else its trailing three digits fix the
grant — `…000`/no-digits → `status:REQUESTED` (no `startedAt`), any other tail →
`status:AVAILABLE` (`startedAt`=now; no `expiresAt` — provisioning is open-ended);
a `qosProfile` name containing `unavailable` → 422
`QOS_PROVISIONING.QOS_PROFILE_NOT_APPLICABLE`; an optional `sink` must be a valid
`https://` URL → else 400 `INVALID_SINK`. `sinkCredential` is accepted but not
applied; `DELETE`/retrieve-by-device/notifications are deferred. `x-correlator`
echoed on every response.

**CloudEvents notifications on `sink`** have begun for QoS Provisioning
(`src/apis/qos_provisioning/notifications.rs`, event type
`org.camaraproject.qos-provisioning.v0.status-changed`, mirroring QoD). The first
transition is **`DELETE_REQUESTED` on `revokeQosAssignment`**: a revoked
assignment that recorded a `sink` receives a `status: UNAVAILABLE` /
`statusInfo: DELETE_REQUESTED` CloudEvent, POSTed fire-and-forget over a raw
`tokio` TCP stream (no HTTP-client dep), off the request path so it never delays
the `204`. To make callbacks observable, the create `sink` now accepts `http://`
as well as `https://` (delivering only to `http://` — no TLS client, so an
`https://` sink is a documented no-op cut, aligning with QoD/Geofencing/Carrier
Billing); a non-http(s) scheme → 400 `INVALID_SINK`. An `ACCESSTOKEN`
`sinkCredential`'s bearer token is applied to the callback as
`Authorization: Bearer …` (RFC 6750), derived at creation and held in a
`assignmentId`-keyed in-memory side-store (`store::insert_credential`/
`take_credential`, taken single-use at delivery) so the secret is never echoed;
PLAIN/REFRESHTOKEN are a documented cut. The **`AVAILABLE`-on-provisioning** event
and the **`NETWORK_TERMINATED`** transition are now in place too: creating an
`AVAILABLE`, sink-bearing assignment delivers a `status: AVAILABLE` `status-changed`
CloudEvent (no `statusInfo`) fire-and-forget; a `…001` identifier tail instead
schedules `v0_3::spawn_network_termination` (a short 1 s grace, mirroring QoD's
`NETWORK_TERMINATION_TAIL`) that evicts the assignment and delivers a
`status: UNAVAILABLE` / `statusInfo: NETWORK_TERMINATED` event (exactly-once vs a
concurrent revoke); a `REQUESTED` assignment is not yet active, so it notifies
nothing. The ACCESSTOKEN `sinkCredential` bearer is applied to every callback —
the non-terminal AVAILABLE event *peeks* it (new `store::peek_credential`, a
non-destructive read) so a later terminal event (revoke / network-drop) still
`take_credential`s it single-use. Only TLS (`https://`) delivery remains deferred.

**Phase 5 (other CAMARA APIs) — Device Data Volume vwip** has begun, a new
stateless, non-spatial, device-keyed data-usage query API mounted at its
canonical work-in-progress base path `/device-data-volume/vwip` (CAMARA
device-data-volume, `wip` — no released version yet, like Device Visit Location
/ Population Density Data). `POST /retrieve` (scope `device-data-volume:read`,
operationId `retrieveDataVolume`) answers a coarse, bucketed data-usage figure —
`{ dataVolumeCategory: "<200MiB"|"<1GiB"|"<5GiB"|">=5GiB", lastStatusTime,
device? }` — never a precise byte count. Device-object identifier resolution +
the two-legged/three-legged rule (mirrors Connected Network Type): a submitted
`device` on a three-legged **line** token (E.164 `sub`) → 422
`UNNECESSARY_IDENTIFIER`; no `device` + a non-line subject → 422
`MISSING_IDENTIFIER`; an empty `device` → 400 `INVALID_ARGUMENT`. Two control
planes (DESIGN §7): the identifier's reserved error suffix → canonical CAMARA
error; else its trailing three digits index a fixed lowest-first table
`[<200MiB, <1GiB, <5GiB, >=5GiB]` (`% 4`; `…000`/no digits → `<200MiB`), so the
reported bucket is a genuine second plane. `lastStatusTime` = now (RFC 3339 UTC;
schema-nullable for "no known measurement" but the sim always reports fresh).
`device` echoed only for a phoneNumber request. The companion `POST /check`
(`checkDataVolume`, `{ thresholdExceeded }` for a `volumeToCheck`) is a later
slice. `x-correlator` echoed on every response.

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
    one-step charge; identifier + amount control planes (DESIGN §7). Now
    persists the charged payment (see `retrievePayment` below).
  - [x] `GET /payments/{paymentId}` (`retrievePayment`) —
    `carrier-billing:payments:read`. In-memory payment store
    (`src/apis/carrier_billing/store.rs`); `createPayment` now persists the
    charged payment so it can be read back (`200`) or `404 NOT_FOUND` for an
    unknown id. Opaque `paymentId` → store state is the only control plane.
  - [x] `GET /payments` (list, `retrievePayments`) —
    `carrier-billing:payments:read`. Store `all()` scan → a `PaymentArray` of
    every stored payment (`200`, empty array when none; store state the only
    control plane). Query-param pagination/filtering (`page`/`perPage`/`order`/
    date+status filters) accepted but not applied — documented cut, later slice.
  - [x] two-step flow: reserve → validate → confirm / cancel
    - [x] `POST /payments/prepare` (`preparePayment`,
      `carrier-billing:payments:create`) — reserve step; happy path →
      `201 { paymentStatus: "reserved" }` (no `paymentDate`), persisted so it
      reads back via `retrievePayment`. Shares `createPayment`'s identifier +
      amount control planes. `pending_validation`/`validationInfo` (OTP) + 409
      `ALREADY_EXISTS` deferred to `validatePayment`.
    - [x] `POST /payments/{paymentId}/validate` (`validatePayment`,
      `carrier-billing:payments:write`) — the OTP-validation step. A
      `preparePayment` for a phone number ending in `888` now lands in
      `pending_validation` (with `validationInfo`: `action` + `authorizationId`);
      `validatePayment` clears it. Correct `authorizationId` + `code` → `204`
      (reservation → `reserved`); wrong `authorizationId` → 400
      `CARRIER_BILLING.INVALID_AUTHORIZATION_ID`; wrong `code` → 400
      `CARRIER_BILLING.INVALID_CODE` until the 3-attempt budget is spent → 400
      `CARRIER_BILLING.VALIDATION_FAILED` (reservation → `denied`); a payment not
      awaiting validation → 409 `ALREADY_EXISTS`; unknown id → 404. OTP `code` is
      the reserved phone number's last six digits, zero-padded (deterministic;
      mirrors OTP SMS). The 409 duplicate-session case on `preparePayment` is not
      modelled (documented cut).
    - [x] `POST /payments/{paymentId}/confirm` (`confirmPayment`,
      `carrier-billing:payments:write`) — charges a `reserved` payment
      → `succeeded` (gains `paymentDate`), `202 Accepted` (no body). Store
      state the only control plane: `reserved` → 202; already `succeeded` →
      409 `CARRIER_BILLING.PAYMENT_CONFIRMED`; already `cancelled` → 409
      `CARRIER_BILLING.PAYMENT_CANCELLED`; other non-`reserved`
      (`pending_validation`/`denied`) → 409 `CONFLICT`; unknown → 404. Optional
      `phoneNumber` body accepted-not-applied.
    - [x] `POST /payments/{paymentId}/cancel` (`cancelPayment`,
      `carrier-billing:payments:write`) — releases a `reserved` payment
      → `cancelled` (no `paymentDate`), `202 Accepted` (no body). Store state
      the only control plane: `reserved` → 202; already `cancelled` → 409
      `CARRIER_BILLING.PAYMENT_CANCELLED`; already `succeeded` → 409
      `CARRIER_BILLING.PAYMENT_CONFIRMED` (a charged payment can't be cancelled);
      other non-`reserved` (`pending_validation`/`denied`) → 409 `CONFLICT`;
      unknown → 404. New `store::cancel`; optional `phoneNumber` body
      accepted-not-applied. **Completes the two-step flow.**
  - [~] charging notifications on `sink`:
    - [x] `createPayment` → `payment-completed` CloudEvent on a successful
      one-step charge (http sink, fire-and-forget over raw TCP, no HTTP-client
      dep; ACCESSTOKEN `sinkCredential` bearer applied; PLAIN/REFRESHTOKEN cut).
    - [x] `preparePayment` → `payment-reserved` CloudEvent on a successful
      `reserved` reservation (http sink, fire-and-forget over raw TCP, no
      HTTP-client dep; ACCESSTOKEN `sinkCredential` bearer applied,
      PLAIN/REFRESHTOKEN cut; a `…888` pending_validation reservation fires no
      payment-reserved — that is the separate payment-pending-validation event)
    - [x] `payment-pending-validation` on `preparePayment` — a `…888`
      reservation → `pending_validation` delivers a `payment-pending-validation`
      CloudEvent to the request `sink` (fire-and-forget over raw TCP, no
      HTTP-client dep; ACCESSTOKEN `sinkCredential` bearer applied,
      PLAIN/REFRESHTOKEN cut). Mutually exclusive with `payment-reserved`; per the
      CAMARA schema the event carries only paymentId/status/description (no
      paymentDate, no validationInfo).
    - [x] `payment-completed` on `confirmPayment` — a reservation prepared with a
      `sink` now delivers the terminal `payment-completed` CloudEvent
      (`data.status: succeeded`, with `paymentDate`) when charged. The
      `sink`/derived credential are stashed at `preparePayment` in a `paymentId`
      side-store (`store::insert_notify`/`take_notify`, mirroring QoD's credential
      side-store) since the confirm body carries no `sink`; taken single-use.
    - [x] `payment-cancelled` on `cancelPayment` — a reservation prepared with a
      `sink` delivers the terminal `payment-cancelled` CloudEvent
      (`data.status: failed`, no `paymentDate`) when released. Reuses the
      `preparePayment` notify side-store (`store::take_notify`, single-use — so a
      reservation fires exactly one terminal event, confirm *or* cancel); the
      cancel body carries no `sink`. ACCESSTOKEN `sinkCredential` bearer applied.
    - [x] `payment-denied` — on a `validatePayment` that exhausts its OTP attempts
      (reservation → `denied`); reuses the same `preparePayment` notify side-store.
      **Completes Carrier Billing v0.5 charging notifications.**
- [x] Call Forwarding Signal v0.4 (`/call-forwarding-signal/v0.4`; CAMARA 0.4.0,
  release r3.3; stateless, non-spatial, phone-number-keyed):
  - [x] `POST /unconditional-call-forwardings`
    (`call-forwarding-signal:unconditional-call-forwardings:read`,
    `retrieveUnconditionalCallForwarding`) — `{active: boolean}`; two-legged
    (submitted `phoneNumber`) / three-legged (E.164 `sub`) identifier rule with
    422 `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER`; identifier reserved-error
    + trailing-digit-parity control planes (DESIGN §7).
  - [x] `POST /call-forwardings` (`retrieveCallForwarding`,
    `call-forwarding-signal:call-forwardings:read`) — the forwarding-type **set**
    (`CallForwardingSignal`: `inactive`/`unconditional`/`conditional_busy`/
    `conditional_not_reachable`/`conditional_no_answer`). Same identifier
    resolution + reserved-error convention; second control plane — the trailing
    three digits as a 4-bit mask (`digits % 16`) over the four active types (bit 0
    = `unconditional`, so an odd tail lines up with the unconditional endpoint);
    zero mask → `["inactive"]`. **Completes Call Forwarding Signal v0.4.**
- [x] Number Recycling v0.2 (`/number-recycling/v0.2`; CAMARA 0.2.0, release
  r2.2; stateless, non-spatial, phone-number-keyed):
  - [x] `POST /check` (`number-recycling:check`, `checkNumberRecycling`) —
    `{ phoneNumberRecycled: boolean }`; two-legged (submitted `phoneNumber`) /
    three-legged (E.164 `sub`) identifier rule with 422 `UNNECESSARY_IDENTIFIER`
    / `MISSING_IDENTIFIER`; two control planes (DESIGN §7): identifier
    reserved-error suffix, and identifier trailing digits (days-since-change) vs
    `specifiedDate` (recycled iff change strictly after the date). `specifiedDate`
    validated: malformed → 400 `INVALID_ARGUMENT`, future → 400 `OUT_OF_RANGE`.
- [x] KYC Age Verification v0.1 (`/kyc-age-verification/v0.1`; CAMARA 0.1.0,
  release r2.2; stateless, non-spatial, phone-number-keyed):
  - [x] `POST /verify` (`kyc-age-verification:verify`, `verifyAge`) —
    `{ ageCheck: "true"|"false"|"not_available" }`; two-legged (submitted
    `phoneNumber`) / three-legged (E.164 `sub`) identifier rule with 422
    `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER`; three control planes
    (DESIGN §7): `ageThreshold` range (0..=120, else 400 `OUT_OF_RANGE`),
    identifier reserved-error suffix, and identifier trailing digits (held age =
    `d % 100`) vs `ageThreshold` (`…000` → `not_available`). Optional response
    fields driven by the body: `identityMatchScore`=90 (any identity attribute),
    `verifiedStatus`=true (`idDocument`), `contentLock`/`parentalControl` (opt-in
    via `include*`, minor `< 18` → `"true"`).
- [x] Device Swap v1 (`/device-swap/v1`; CAMARA 1.0.0, release r3.2; stateless,
  non-spatial, phone-number-keyed; the device counterpart of SIM Swap):
  - [x] `POST /check` (`device-swap:check`, `checkDeviceSwap`) —
    `{ swapped: boolean }`; two-legged (submitted `phoneNumber`) / three-legged
    (E.164 `sub`) identifier rule with 422 `UNNECESSARY_IDENTIFIER` /
    `MISSING_IDENTIFIER`; two control planes (DESIGN §7): identifier reserved-error
    suffix, and identifier trailing digits (hours-since-last-swap) vs `maxAge`
    (`swapped = hoursAgo < maxAge`; 1–2400, default 240, else 400 `OUT_OF_RANGE`;
    no digits → never swapped).
  - [x] `POST /retrieve-date` (`device-swap:retrieve-date`, `retrieveDeviceSwapDate`)
    — `{ latestDeviceChange: <RFC 3339 UTC | null>, monitoredPeriod: <days> }`;
    same two-legged / three-legged identifier rule (422 `UNNECESSARY_IDENTIFIER`
    / `MISSING_IDENTIFIER`) and reserved-error convention as `check`. Identifier
    trailing digits = hours-since-swap → `latestDeviceChange` = now − hoursAgo h
    when inside the fixed monitored period (240 h = 10 days, aligned to `check`'s
    default `maxAge`), else `null`; `monitoredPeriod` (10 days) always reported.
    Self-contained RFC 3339 formatter (no new dep, mirroring SIM Swap).
    **Completes Device Swap v1.**
- [x] KYC Fill-in v0.3 (`/kyc-fill-in/v0.3`; CAMARA 0.3.0; stateless,
  non-spatial, phone-number-keyed identity API — the *return-attributes*
  counterpart of KYC Match):
  - [x] `POST /fill-in` (`KYC_Fill-in`, scope `kyc-fill-in:set-all` **or** any
    per-attribute `kyc-fill-in:<attribute>`) — returns the operator-verified
    identity attributes so a caller can pre-fill a form. Two-legged (submitted
    `phoneNumber`) / three-legged (E.164 `sub`) identifier rule with 422
    `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER`; a token with no
    `kyc-fill-in:*` scope → 403 `PERMISSION_DENIED`. Control planes (DESIGN §7):
    identifier reserved-error suffix → canonical CAMARA error; identifier
    trailing three digits pick one of 3 fixed personas (`% 3`, covering the 3
    `gender` values); and the **granted scope set** shapes the response —
    `set-all` → all 20 attributes, else only the per-attribute-scoped ones (a
    genuine second control plane over the body). Synthetic data only.
    **Completes KYC Fill-in v0.3.**
- [x] Home Devices QoD v0.4 (`/home-devices-qod/v0.4`; CAMARA 0.4.0; stateless,
  non-spatial, **ipAddress-keyed** home-LAN QoS — distinct from network-side QoD):
  - [x] `PUT /qos` (`home-devices-qod:qos:write`, `setQos`) — apply a
    `serviceClass` to the home device at `ipAddress` → `204 No Content`. Stateless
    (no store). Control planes (DESIGN §7): the `ipAddress` **last octet** (happy
    path unless a reserved `…240`–`…253` octet names a specific CAMARA 0.4.0
    condition: `404 DEVICE_NOT_FOUND`, the `409 HOME_DEVICES_QOD.*` conflict set,
    `503 ROUTER_OFFLINE`, `504 TIMEOUT`, `500 INTERNAL`, plus generic 409/404/503),
    and `serviceClass` (second plane: `…247` conflicts as QOS_ALREADY_SET_TO_DEFAULT
    only when restoring `standard`). Bad body / unknown `serviceClass` / non-IPv4
    `ipAddress` → 400 INVALID_ARGUMENT. **Completes Home Devices QoD v0.4.**
- [x] QoS Profiles v1 (`/qos-profiles/v1`; CAMARA 1.1.0, r3.2; stateless catalog —
  the read-only companion to Quality on Demand):
  - [x] `POST /retrieve-qos-profiles` (`qos-profiles:read`, `retrieveQoSProfiles`)
    — lists the operator's fixed in-memory QoS-profile catalog as a JSON array
    (`200`), narrowed by the optional `name`/`status` filters (a real control
    plane; an unknown `name` → empty array, a list never 404s). `device` is an
    optional *error plane* only (DESIGN §7): its first present identifier's
    reserved suffix → canonical CAMARA error, and a `device` on a three-legged
    line token (E.164 `sub`) → 422 `UNNECESSARY_IDENTIFIER`. Bad `name` pattern /
    unknown `status` enum / empty `device` / malformed `phoneNumber` → 400
    `INVALID_ARGUMENT`. `x-correlator` echoed. Catalog covers all three
    `QosProfileStatusEnum` states + a spread of service classes / L4S queue types.
  - [x] `GET /qos-profiles/{name}` (`qos-profiles:read`, `getQosProfile`) —
    single-profile lookup over the same fixed catalog. The `name` path parameter
    is the sole control plane (DESIGN §7; no body/`device`, so no error plane): a
    known name → `200` that `QosProfile`; a well-formed unknown name → `404
    NOT_FOUND` (unlike the list, which returns `[]`); a name violating the
    `QosProfileName` schema (`^[a-zA-Z0-9_.-]+$`, len 3–256) → `400
    INVALID_ARGUMENT`. `x-correlator` echoed. **Completes QoS Profiles v1.**
- [x] KYC Tenure v0.2 (`/kyc-tenure/v0.2`; CAMARA 0.2.0, release r2.2; stateless,
  non-spatial, phone-number-keyed identity/anti-fraud API — part of Know Your
  Customer):
  - [x] `POST /check-tenure` (`kyc-tenure:check-tenure`, `checkTenure`) —
    `{ tenureDateCheck: boolean, contractType: PAYG|PAYM|Business }`; two-legged
    (submitted `phoneNumber`) / three-legged (E.164 `sub`) identifier rule with
    422 `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER`; two control planes
    (DESIGN §7): identifier reserved-error suffix, and identifier trailing digits
    (days-of-tenure) vs `tenureDate` — `tenureDateCheck` true iff the tenure
    started on or before the date (`(today − tenureDate) <= digits`), so
    `tenureDate` is a genuine second control plane. `tenureDate` validated:
    malformed → 400 `INVALID_ARGUMENT`, future → 400 `OUT_OF_RANGE`.
    `contractType` derived deterministically (`digits % 3`). Self-contained
    civil-date parser (no new dep, mirroring Number Recycling).
    **Completes KYC Tenure v0.2.**
- [~] Blockchain Public Address v0.3 (`/blockchain-public-address/v0.3`; CAMARA
  0.3.0, release r2.2; phone-number-keyed Web3 onboarding — link a line to
  on-chain addresses):
  - [x] `POST /blockchain-public-addresses/retrieve-blockchains`
    (`blockchain-public-address:read`, `retrieveBlockchainPublicAddress`) —
    stateless read; returns the array of `BlockchainPublicAddressResponse`
    (`id`, `blockchainPublicAddress`, `blockchainNetworkId`, `currency`) bound
    to the required `phoneNumber` (no three-legged fallback — the 0.3.0 schema
    marks `phoneNumber` required; missing/malformed → 400 INVALID_ARGUMENT). Two
    control planes (DESIGN §7): identifier reserved-error suffix → canonical
    CAMARA error; else trailing three digits `d` → `[]` when `d == 0`, else
    `((d - 1) % 3) + 1` addresses (1–3), the `i`-th on `NETWORKS[(d + i) % 6]`
    (fixed CAIP-2 EVM table → the chain is a second plane). Deterministic
    `0x…`/UUID via SHA-256 (no new dep); addresses lowercase (not EIP-55) —
    documented cut. `x-correlator` echoed.
  - [x] stateful `POST /blockchain-public-addresses` (`bindBlockchainPublicAddress`,
    scope `blockchain-public-address:create`) — binds an on-chain address to a
    phone number, persisting it in a new in-memory store
    (`src/apis/blockchain_public_address/store.rs`; `Mutex<HashMap>`, id derived
    from the triple so a duplicate collides), `201 {id}`. Three control planes
    (DESIGN §7): identifier reserved-error suffix → canonical error; request
    validation (bad `blockchainNetworkId` → 400
    `…INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER`; bad EVM address → 400
    INVALID_ARGUMENT; lone `nonce`/`signature` → 400
    `…BOTH_NONCE_SIGNATURE_REQUIRED`; both → 422
    `…UNSUPPORTED_ENHANCED_VALIDATION`); store state (re-bind same triple → 409
    ALREADY_EXISTS). `x-correlator` echoed. (No new dep — reuses `sha2`.)
  - [x] stateful `DELETE /blockchain-public-addresses/{id}`
    (`deleteBlockchainPublicAddress`, `blockchain-public-address:delete`) —
    unbinds the stored binding named by the opaque `id` from the same in-memory
    store (new `store::remove`): present → `204 No Content` (single-use), absent
    → `404 NOT_FOUND`. Keyed only on store state — the `id` is opaque, so no
    reserved-identifier plane. Bindings aren't scoped per subscriber, so the
    spec's `sub`-ownership check isn't enforced (documented cut). `x-correlator`
    echoed. **This completes Blockchain Public Address v0.3.**
- [x] Simple Edge Discovery v2 (`/simple-edge-discovery/v2`; CAMARA 2.0.1,
  release r2.3; stateless, non-spatial, device-keyed edge/MEC discovery):
  - [x] `POST /retrieve-closest-edge-cloud-zone` (`simple-edge-discovery:read`,
    `readClosestEdgeCloudZone`) — returns the closest `EdgeCloudZone`
    (`{edgeCloudZoneId, edgeCloudZoneName, edgeCloudProvider}`), never the
    device's location. Device-object identifier resolution (phoneNumber → NAI →
    IPv4 publicAddress → ipv6Address, else token subject) with the CAMARA
    two-legged / three-legged rule (422 `UNNECESSARY_IDENTIFIER` /
    `MISSING_IDENTIFIER`; empty `device` → 400 INVALID_ARGUMENT). Two control
    planes (DESIGN §7): identifier reserved-error suffix → canonical CAMARA
    error; else trailing three digits index a fixed 6-entry edge-zone table
    (`% 6`; `…000`/no-digits → entry 0), so the zone is a genuine second plane.
    `edgeCloudZoneId` is UUID-shaped, deterministic and stable per zone
    (SHA-256, no new dep). The `device` is echoed only for a phoneNumber-keyed
    request (CAMARA `DeviceResponse` carries only phoneNumber). `x-correlator`
    echoed. **Completes Simple Edge Discovery v2.**
- [x] Customer Insights v0.2 (`/customer-insights/v0.2`; CAMARA 0.2.0, release
  r2.2; stateless, non-spatial, phone-number-keyed identity/anti-fraud API):
  - [x] `POST /scoring/retrieve` (`customer-insights:scoring:read`,
    `retrieveScoring`) — `{ scoringType, scoringValue }`, a privacy-preserving
    risk/trust score on a caller-chosen scale (`gaugeMetric` 300–850 or
    `veritasIndex` 0–19), never the underlying data. Two-legged (submitted
    `phoneNumber`) / three-legged (E.164 `sub`) identifier rule with 422
    `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER`, plus 422
    `CUSTOMER_INSIGHTS.ID_DOCUMENT_NOT_SUPPORTED` when only an `idDocument`
    identifies the caller (the sim scores by phone number only). Two control
    planes (DESIGN §7): identifier reserved-error suffix → canonical CAMARA
    error; and identifier trailing digits `d` × `scoringType` fix the score
    (`gaugeMetric` → `300 + d%551`, `veritasIndex` → `d%20`), so `scoringType`
    is a genuine second plane. `scoringType` required (missing/unknown → 400
    `INVALID_ARGUMENT`); `idDocument` non-empty ≤ 30 chars. No new dep.
    **Completes Customer Insights v0.2.**
- [x] Connected Network Type v0.2 (`/connected-network-type/v0.2`; CAMARA 0.2.0,
  release r1.2; stateless, non-spatial, device-keyed radio-access API):
  - [x] `POST /retrieve` (`connected-network-type:read`, `getConnectedNetworkType`)
    — `{ connectedNetworkType: 2G|3G|4G|5G|UNKNOWN, lastStatusTime, device? }`.
    Device-object identifier resolution + the two-legged/three-legged rule
    (mirrors Simple Edge Discovery): submitted `device` on a line token → 422
    `UNNECESSARY_IDENTIFIER`; no `device` + non-line subject → 422
    `MISSING_IDENTIFIER`; empty `device` → 400 `INVALID_ARGUMENT`. Two control
    planes (DESIGN §7): identifier reserved-error suffix → canonical CAMARA
    error; else trailing three digits index `[5G,4G,3G,2G,UNKNOWN]` (`% 5`;
    `…000`/no digits → `5G` — type is a 2nd plane). `lastStatusTime` = now
    (RFC 3339 UTC), `null` for `UNKNOWN`. `device` echoed only for a phoneNumber
    request. No new dep. The event-subscription surface is out of scope.
    **Completes Connected Network Type v0.2.**
- [x] Connectivity Insights v0.6 (`/connectivity-insights/v0.6`; CAMARA 0.6.0,
  release r3.2; stateless, non-spatial, device-keyed network-quality insight):
  - [x] `POST /check-network-quality` (`connectivity-insights:check`,
    `checkNetworkQuality`) — per-KPI policy-fulfilment verdicts
    (`packetDelayBudget`/`targetMinDownstreamRate`/`targetMinUpstreamRate`/
    `packetlossErrorRate`/`jitter` → `meets`/`unable to meet…`) + coarse
    `additionalKPIs` (`signalStrength`/`connectivityType`), never raw
    measurements. Device-object identifier resolution (phoneNumber → NAI → IPv4
    publicAddress → ipv6Address, else token subject); 0.6.0 has **no**
    `UNNECESSARY_IDENTIFIER`, so a device on a line token is simply used; no
    device + non-line subject → 422 `MISSING_IDENTIFIER`. Required
    `applicationProfileId` (UUID; not resolved against a profile store — a cut)
    and `applicationServer` (≥1 addr); optional `applicationServerPorts`
    (0..=65535, else 400 OUT_OF_RANGE) / `monitoringTimeStamp`. Control planes
    (DESIGN §7): identifier reserved-error suffix → canonical CAMARA error; else
    the identifier's trailing three digits' low five bits are an *unmet mask*
    (one bit per KPI; `…000` → all met, healthy default → excellent/5G-SA;
    `…031` → all unmet → no signal/3G), and `additionalKPIs` degrade coherently
    with the count met. `device` echoed only for a phoneNumber request. No new
    dep. **Completes Connectivity Insights v0.6.**
- [x] Region Device Count v0.2 (`/region-device-count/v0.2`; CAMARA 0.2.0,
  release r2.2; stateless, **area-keyed** aggregate count):
  - [x] `POST /count` (`region-device-count:count`, `count`) — `{ count?,
    status }` device count for a `CIRCLE`/`POLYGON` region over an optional time
    interval. No identifier: control planes (DESIGN §7) key off the geometry —
    the characteristic radius `r` drives a size plane (`r > 1e6 m` → 400
    `REGION_DEVICE_COUNT.UNSUPPPORTED_REQUEST`; `5e5 < r ≤ 1e6 m`, no `sink` → 400
    `REGION_DEVICE_COUNT.UNSUPPORTED_SYNC_RESPONSE`, with `sink` → sync) and a
    status/count plane on `round(r)`'s trailing digits (`…429`→429; `…001`→
    PART_OF_AREA_NOT_SUPPORTED; `…002`→AREA_NOT_SUPPORTED; `…003`→DENSITY_BELOW_
    PRIVACY_THRESHOLD; `…004`→TIME_INTERVAL_NO_DATA_FOUND; else→SUPPORTED_AREA).
    `count` = area × 500 dev/km², narrowed by `filter` (second plane). Validation:
    INVALID_CIRCLE_AREA/INVALID_POLYGON_AREA, TIME_INVALID_ARGUMENT/INVALID_END_
    DATE (self-contained RFC 3339 parser), filter → INVALID_ARGUMENT,
    sinkCredential → INVALID_CREDENTIAL/INVALID_TOKEN. Shoelace area, no new dep.
    Async `sink`/CloudEvents (+ 410 GONE) a documented cut. **Completes Region
    Device Count v0.2.**
- [x] Device Visit Location vwip (`/device-visit-location/vwip`; CAMARA
  device-visit-location, wip — no released version yet, so mounted at its
  canonical `vwip` base path; stateless, device-keyed anti-fraud / identity):
  - [x] `POST /retrieve` (`device-visit-location:retrieve`,
    `retrieveDeviceVisitLocation`) — `{ geoCodeList: [{ countryCode, codeType:
    "PostalCode", codeValue }] }`, the coarse places a device visited over a
    required RFC 3339 `startTime`/`endTime` window (never coordinates).
    Device-object identifier resolution + the two-legged/three-legged rule
    (mirrors Connected Network Type): `device` on a line token → 422
    `UNNECESSARY_IDENTIFIER`; no `device` + non-line subject → 422
    `MISSING_IDENTIFIER`; empty `device` → 400 `INVALID_ARGUMENT`. Two control
    planes (DESIGN §7): the identifier — reserved error suffix → canonical CAMARA
    error (checked first); else `…000`/no-digits → 404
    `DEVICE_VISIT_LOCATION.DATA_NOT_FOUND` (non-empty `geoCodeList`, so no-data is
    a 404); else trailing digits `d` → `((d-1)%3)+1` codes, `i`-th on
    `COUNTRIES[(d+i)%6]` of `[US,GB,DE,FR,ES,IT]`, deterministic postal codes —
    and the `startTime`/`endTime` window (malformed → 400 `INVALID_ARGUMENT`;
    `endTime < startTime` → 400 `DEVICE_VISIT_LOCATION.INVALID_END_DATE`;
    self-contained RFC 3339 parser, no new dep). `x-correlator` echoed.
    **Completes Device Visit Location vwip.**
- [x] Population Density Data vwip (`/population-density-data/vwip`; CAMARA
  population-density-data, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateless, **area-keyed** aggregate density estimate):
  - [x] `POST /retrieve` (`population-density-data:read`,
    `retrievePopulationDensity`) — per grid cell, an estimated people-per-km²
    figure with a min/max band (or `NO_DATA`/`LOW_DENSITY`), plus an overall
    area-support `status`. No device identifier — the **geometry is the control
    plane** (mirrors Region Device Count). CamaraSim implements the **synchronous
    `GEOHASHLIST`** path (one cell per input geohash, a single time slice over the
    window). Control planes (DESIGN §7): the **first geohash**'s reserved error
    suffix → canonical CAMARA error (geohashes contain digits, so the shared
    convention applies unchanged; checked before the window); **each geohash**'s
    stable hash fixes its cell (`h%7==0`→NO_DATA, `==1`→LOW_DENSITY, else
    DENSITY_ESTIMATION `pplDensity=(h%20000)+1` ±10%), and the cell mix fixes
    `status` (all NO_DATA→`AREA_NOT_SUPPORTED`, some→`PART_OF_AREA_NOT_SUPPORTED`,
    else `SUPPORTED_AREA`). Capability/structural planes: `POLYGON` areaType → 422
    `…UNSUPPORTED_AREA_TYPE`; a geohash longer than 9 chars → 422
    `…UNSUPPORTED_PRECISION`; > 100 geohashes → 422 `…UNSUPPORTED_SYNC_RESPONSE`;
    unknown areaType / bad geohash / empty or >1000 list / `precision` with a
    GEOHASHLIST → 400 `INVALID_ARGUMENT`. Time window: malformed → 400
    `INVALID_ARGUMENT`; `endTime<startTime` → 400 `…INVALID_END_TIME`; > 7 days →
    400 `…MAX_TIME_PERIOD_EXCEEDED` (self-contained RFC 3339 parser). Documented
    cuts: `POLYGON`, async `sink`/CloudEvents (202 flow), hourly time-slicing, and
    the ±3-month absolute start-time checks. `x-correlator` echoed. No new dep.
    **Completes Population Density Data vwip.**
- [~] QoS Provisioning v0.3 (`/qos-provisioning/v0.3`; CAMARA qos-provisioning
  0.3.0, release r3.2 — part of the QualityOnDemand repo; the *provisioning*
  (open-ended) counterpart of Quality on Demand's bounded sessions; stateful,
  resource-oriented, in-memory assignment store):
  - [x] `POST /qos-assignments` (`qos-provisioning:qos-assignments:create`,
    `createQosAssignment`) — provisions a `qosProfile` for a device, mints an
    opaque UUID-shaped `assignmentId` (`src/apis/qos_provisioning/store.rs`;
    `Mutex<HashMap>`, no uuid/rand dep, mirroring QoD's store), persists the
    rendered `AssignmentInfo`, `201`. Device-object identifier resolution + the
    two-legged/three-legged rule (device on a line token → 422
    `UNNECESSARY_IDENTIFIER`; no device + non-line subject → 422
    `MISSING_IDENTIFIER`; empty `device` → 400 INVALID_ARGUMENT). Control planes
    (DESIGN §7): identifier reserved-error suffix → canonical CAMARA error
    (`…409` → 409 CONFLICT "existing provisioning"); else `…000`/no-digits →
    `status:REQUESTED` (no `startedAt`), any other tail → `status:AVAILABLE`
    (`startedAt`=now; no `expiresAt` — provisioning is open-ended); `qosProfile`
    name containing `unavailable` → 422
    `QOS_PROVISIONING.QOS_PROFILE_NOT_APPLICABLE`; optional `sink` must be a
    valid `https://` URL → else 400 `INVALID_SINK`. `sinkCredential`
    accepted-not-applied; notifications a later slice. `x-correlator` echoed.
  - [x] `GET /qos-assignments/{assignmentId}` (`qos-provisioning:qos-assignments:read`,
    `getQosAssignmentById`) — reads a stored assignment back (`200`) or `404
    NOT_FOUND`. Opaque `assignmentId` → store state the only control plane.
  - [x] `DELETE /qos-assignments/{assignmentId}` (`revokeQosAssignment`,
    `qos-provisioning:qos-assignments:delete`) — revokes a stored assignment,
    evicting it from the store (new `store::remove`): present → `204 No Content`
    (single-use), unknown/already-revoked → `404 NOT_FOUND`. Keyed only on store
    state (opaque `assignmentId`, no reserved-identifier plane). CAMARA's async
    `202 Accepted`/`DELETE_REQUESTED` form is deferred with `sink` notifications —
    synchronous `204` only (mirrors QoD's `deleteSession`). `x-correlator` echoed.
  - [x] `POST /retrieve-qos-assignment` (`getQosAssignmentByDevice`,
    `qos-provisioning:qos-assignments:read-by-device`) — returns the device's
    provisioned `AssignmentInfo` (`200`) or `404 NOT_FOUND` when it has none.
    Same two-legged/three-legged identifier rule as create (422
    `UNNECESSARY_IDENTIFIER`/`MISSING_IDENTIFIER`). Two control planes (DESIGN
    §7): resolved identifier reserved-error suffix → canonical CAMARA error
    (checked first, mirroring QoD's retrieve-by-device); else the in-memory store
    scanned by device echo (new `store::find_by_device`). One provisioning per
    device → a single `AssignmentInfo`.
  - [~] CloudEvents notifications on `sink` (status transitions)
    (`src/apis/qos_provisioning/notifications.rs`; event type
    `org.camaraproject.qos-provisioning.v0.status-changed`, mirroring QoD):
    - [x] `DELETE_REQUESTED` `status-changed` on `revokeQosAssignment` — a revoked
      assignment that recorded a `sink` receives a `status: UNAVAILABLE` /
      `statusInfo: DELETE_REQUESTED` CloudEvent, fire-and-forget over raw TCP (no
      HTTP-client dep), still `204`. `sink` now accepts `http://` (for a loopback
      receiver) as well as `https://`, delivering only to `http://` (no TLS
      client — `https://` a documented no-op cut, mirroring QoD); non-http(s)
      scheme → 400 `INVALID_SINK`. ACCESSTOKEN `sinkCredential` bearer applied
      (single-use side-store `store::insert_credential`/`take_credential`),
      PLAIN/REFRESHTOKEN a documented cut; the secret is never echoed.
    - [x] `NETWORK_TERMINATED` transition + an `AVAILABLE`-on-provisioning event
      — creating an `AVAILABLE`, sink-bearing assignment delivers a
      `status: AVAILABLE` `status-changed` CloudEvent (no `statusInfo`,
      fire-and-forget); a `…001` `AVAILABLE` assignment instead schedules an early
      network drop (`v0_3::spawn_network_termination`, 1 s grace) → evict +
      `status: UNAVAILABLE`/`statusInfo: NETWORK_TERMINATED` (exactly-once vs a
      concurrent revoke). A `REQUESTED` assignment fires nothing. The ACCESSTOKEN
      `sinkCredential` bearer is applied to every callback — *peeked*
      (`store::peek_credential`) on the non-terminal AVAILABLE event so a later
      terminal event still authenticates, *taken* single-use on the terminal one.
    - [ ] TLS (`https://` sink) delivery (needs a rustls TLS client)
- [~] Device Data Volume vwip (`/device-data-volume/vwip`; CAMARA
  device-data-volume, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateless, non-spatial, device-keyed data-usage query):
  - [x] `POST /retrieve` (`device-data-volume:read`, `retrieveDataVolume`) —
    `{ dataVolumeCategory: "<200MiB"|"<1GiB"|"<5GiB"|">=5GiB", lastStatusTime,
    device? }`, a coarse bucketed usage figure (never a byte count).
    Device-object identifier resolution + the two-legged/three-legged rule
    (mirrors Connected Network Type): `device` on a line token → 422
    `UNNECESSARY_IDENTIFIER`; no `device` + non-line subject → 422
    `MISSING_IDENTIFIER`; empty `device` → 400 `INVALID_ARGUMENT`. Two control
    planes (DESIGN §7): identifier reserved-error suffix → canonical CAMARA
    error; else trailing three digits index `[<200MiB,<1GiB,<5GiB,>=5GiB]`
    (`% 4`; `…000`/no digits → `<200MiB` — category is a 2nd plane).
    `lastStatusTime` = now (RFC 3339 UTC; schema-nullable but always fresh).
    `device` echoed only for a phoneNumber request. No new dep.
  - [ ] `POST /check` (`device-data-volume:read`, `checkDataVolume`) —
    `{ thresholdExceeded }` for a caller-supplied `volumeToCheck`
    (`{value:0..1024, unit:MiB|GiB}`). Later slice.
- [ ] Other CAMARA APIs as capacity allows

## Cross-cutting (do alongside the item that needs it)
- [~] `errors.rs`: base CAMARA error model done (`src/errors.rs`, `specs/shared/errors.yaml`); per-version catalogs still TODO (DESIGN §8)
- [ ] `registry.rs`: canonical URL versioning + `/` catalog wiring (DESIGN §9)
- [~] `specs/…`: vendor + annotate OpenAPI per API/version, serve at `/{api}/v{n}/openapi.yaml`
  — **serving done** (`src/apis/openapi.rs`: every mounted API's spec at
  `/{api}/v{n}/openapi.yaml`, plus `/auth/openapi.yaml` + `/shared/errors.yaml` so
  `$ref`s resolve; catalog advertises each `spec_url`). Per-API *vendoring/annotation*
  continues alongside each new API.
- [~] Contract-test harness (validate responses against vendored spec) — first
  slice landed: a registry/wiring contract test (`src/main.rs`
  `catalog_spec_urls_match_served_specs_and_resolve`) asserts the `/` catalog's
  `spec_url` set exactly equals the served API-spec set (new
  `apis::openapi::api_spec_urls`, single source of truth) and that every
  catalogued `spec_url` resolves as `application/yaml` through the full app, so
  catalog↔spec drift (a newly mounted API missing from the catalog, or a
  `spec_url` that 404s) fails CI. Full response-vs-schema validation still TODO
  (would need a YAML/JSON-Schema validator — a dependency trade-off, deferred).

---

## Scan journal

Newest first. One line per pass: `YYYY-MM-DD HH:MMZ — <what happened> — binary: <size>`

- 2026-08-05 — Phase 5 (other CAMARA APIs): **Device Data Volume vwip** — new
  stateless, non-spatial, device-keyed data-usage query API. Verified the
  authoritative CAMARA spec via WebFetch (device-data-volume, version `wip`: base
  `/device-data-volume/vwip`, `POST /retrieve` = `retrieveDataVolume` scope
  `device-data-volume:read`; request `RetrieveDataVolumeRequest {device?}`; 200 →
  `RetrieveDataVolumeResponse {device?, lastStatusTime(nullable date-time,req),
  dataVolumeCategory enum ["<200MiB","<1GiB","<5GiB",">=5GiB"](req)}`; errors
  400/401/403/404/422/429). First slice = `POST /retrieve` (the companion
  `POST /check` = `checkDataVolume` deferred to a later slice). New
  `src/apis/device_data_volume/{,vwip}.rs` (mirrors Connected Network Type:
  device-object identifier resolution + two-legged/three-legged rule — device on
  a line token → 422 UNNECESSARY_IDENTIFIER, no device + non-line subject → 422
  MISSING_IDENTIFIER, empty device → 400; self-contained RFC 3339 formatter, no
  new dep), wired into apis/openapi/main catalog. Control planes (DESIGN §7):
  reserved-error suffix → canonical error; else trailing three digits index
  `[<200MiB,<1GiB,<5GiB,>=5GiB]` (`% 4`, `…000` → `<200MiB`), so the bucket is a
  2nd plane; `lastStatusTime` always fresh (schema-nullable, never null). `device`
  echoed only for a phoneNumber request. Vendored/annotated spec at
  `specs/device-data-volume/vwip/openapi.yaml`, served + catalogued. 20 new tests
  (6 unit + 14 integration through the real router: category-by-tail, required
  fields + phone echo, non-phone omits echo, reserved-suffix errors, three-legged
  subject + reserved subject, UNNECESSARY/MISSING_IDENTIFIER, empty/invalid/
  unknown-field/malformed body 400s, scope-forbidden, missing-token, x-correlator
  echo) + the catalog contract test extended. `cargo test` green (1028, was 1008),
  `cargo build --release` clean (no warnings). No new dependency. — binary:
  2,261,616 bytes (2.2M)
- 2026-08-05 — Phase 5: **QoS Provisioning v0.3** — CloudEvents notifications:
  added the **`AVAILABLE`-on-provisioning** event and the **`NETWORK_TERMINATED`**
  transition, so only TLS (`https://`) delivery remains cut. `createQosAssignment`
  now delivers, fire-and-forget, a `status: AVAILABLE` `status-changed` CloudEvent
  (no `statusInfo`) for an `AVAILABLE`, sink-bearing assignment; a `…001` tail
  instead schedules `v0_3::spawn_network_termination` (1 s grace, mirroring QoD's
  `NETWORK_TERMINATION_TAIL`) → evict + `status: UNAVAILABLE`/`statusInfo:
  NETWORK_TERMINATED`, exactly-once vs a concurrent revoke; a `REQUESTED`
  assignment notifies nothing. The ACCESSTOKEN `sinkCredential` bearer is applied
  to every callback: new `store::peek_credential` reads it non-destructively for
  the non-terminal AVAILABLE event so a later terminal event (revoke /
  network-drop) still `take_credential`s it single-use. Store insert now happens
  before the timer is spawned so the eviction always sees the assignment. Spec
  updated: header note, create `sink` description + `x-camarasim-scenarios` cases
  (AVAILABLE / …001 NETWORK_TERMINATED / REQUESTED-no-event), `callbacks` +
  `StatusChangedEvent`/`statusInfo` docs. Tests: 4 new integration tests (AVAILABLE
  event shape + unauthenticated; `…001` → NETWORK_TERMINATED + eviction (GET 404);
  AVAILABLE event authenticated AND a later revoke still authenticated (proves
  peek); REQUESTED fires no event via an accept-timeout) + a `peek_credential`
  store test; isolated the two existing DELETE_REQUESTED tests to a `…000`
  identifier. `cargo test` green (1008 tests), `cargo build --release` green. No
  new dependency. — binary: 2.2M (2,238,648 bytes)
- 2026-08-05 — Phase 5: **QoS Provisioning v0.3** — CloudEvents notifications on
  `sink` have begun: the **`DELETE_REQUESTED` `status-changed`** transition on
  `revokeQosAssignment`. Verified the authoritative CAMARA spec via WebFetch
  (qos-provisioning 0.3.0, r3.2): CloudEvent type
  `org.camaraproject.qos-provisioning.v0.status-changed`, `data`
  `{assignmentId, status(AVAILABLE|UNAVAILABLE), statusInfo?(NETWORK_TERMINATED|
  DELETE_REQUESTED)}`, sink pattern `^https:\/\/.+$`. Added
  `src/apis/qos_provisioning/notifications.rs` (event builder + `sink_authorization`
  + raw-TCP `deliver`/`spawn_delivery`, mirroring QoD; no HTTP-client/uuid/rand
  dep) and store helpers `new_event_id`/`insert_credential`/`take_credential`
  (single-use bearer side-store). `revokeQosAssignment` now delivers the
  UNAVAILABLE/DELETE_REQUESTED event to a recorded `sink` fire-and-forget (still
  `204`); create stashes an ACCESSTOKEN `sinkCredential` bearer. Relaxed the
  create `sink` to accept `http://` (for a loopback receiver) as well as
  `https://` — delivering only to `http://` (no TLS client, `https://` a
  documented no-op cut, aligning with QoD); non-http(s) scheme → 400
  `INVALID_SINK`. Spec updated: `callbacks.statusChanged` on `createQosAssignment`
  + new `StatusChangedEvent` schema, sink pattern `^https?:\/\/.+$`, statusInfo /
  SinkCredential docs, revoke description + both ops' `x-camarasim-scenarios`.
  Tests: notifications unit tests (event shape, sink-auth, http delivery + auth
  header, non-http no-op), store credential/event-id tests, and two end-to-end
  integration tests (revoke fires DELETE_REQUESTED to a loopback http sink; an
  ACCESSTOKEN sinkCredential authenticates the callback and is never echoed);
  adjusted the sink-validation tests for the new http-accepted behaviour. `cargo
  test` green (1003 tests), `cargo build --release` green. No new dependency. —
  binary: 2.2M
- 2026-08-05 — Phase 5: **QoS Provisioning v0.3** — `POST /retrieve-qos-assignment`
  (`getQosAssignmentByDevice`, scope `qos-provisioning:qos-assignments:read-by-device`)
  now live. Verified the authoritative CAMARA spec via WebFetch (qos-provisioning
  0.3.0, r3.2): POST-not-GET (device may carry PII), body `RetrieveAssignmentByDevice
  {device?}`, 2-legged → device required / 3-legged → device omitted, 200 →
  `AssignmentInfo`, error set 400/401/403/404/422/429. Implemented: same
  two-legged/three-legged identifier resolution as create (reuses
  `resolve_identifier`; 422 UNNECESSARY/MISSING_IDENTIFIER); two control planes —
  resolved identifier reserved-error suffix → canonical error (checked first,
  mirroring QoD's retrieveSessionsByDevice), else new `store::find_by_device`
  scans the store by echoed `device` → the device's `AssignmentInfo` (200) or 404
  NOT_FOUND. One provisioning per device → a single record (Option, not QoD's
  array). Empty body accepted as `{}` (3-legged). `x-correlator` echoed. Spec
  updated (`/retrieve-qos-assignment` path + `RetrieveAssignmentByDevice` schema +
  `x-camarasim-scenarios`; header comment refreshed). No new dependency (reuses the
  existing store `Mutex<HashMap>`). 9 new handler tests + 1 store `find_by_device`
  test; `cargo test` 991 passed (was 982); `cargo build --release` clean, no
  warnings. — binary: 2,220,760 bytes (2.2M)

- 2026-08-05 — Phase 5: **QoS Provisioning v0.3** — `DELETE /qos-assignments/
  {assignmentId}` (`revokeQosAssignment`, scope
  `qos-provisioning:qos-assignments:delete`) now live. Verified the authoritative
  CAMARA spec via WebFetch (qos-provisioning 0.3.0, r3.2): revoke has a
  synchronous `204 No Content` form and an async `202 Accepted` + `AssignmentInfo`
  (`status:AVAILABLE`, `statusInfo:DELETE_REQUESTED`) form driven by a `sink`
  callback; error set 400/401/403/404/429. Implemented the synchronous `204` path
  (the async/202 form is deferred with `sink` notifications — a documented cut,
  mirroring QoD's `deleteSession`): new `store::remove` evicts the assignment,
  present → 204 (single-use), unknown/already-revoked → 404 NOT_FOUND; opaque
  `assignmentId` → store state the only control plane. `x-correlator` echoed on the
  204. Spec updated (`specs/qos-provisioning/v0.3/openapi.yaml`: `delete` op added
  under `/qos-assignments/{assignmentId}` with the 204 + async-cut note and
  `x-camarasim-scenarios`; header comment refreshed). No new dependency. 6 new
  tests (5 handler + 1 store `remove`); `cargo test` 982 passed (was 976);
  `cargo build --release` clean, no warnings. — binary: 2,208,656 bytes (2.2M)

- 2026-08-05 — Phase 5 (other CAMARA APIs): **QoS Provisioning v0.3** — new
  stateful, resource-oriented API, the *provisioning* (open-ended) counterpart of
  Quality on Demand. Verified the authoritative CAMARA spec via WebFetch
  (qos-provisioning 0.3.0 lives in the QualityOnDemand repo, meta-release r3.2:
  base `/qos-provisioning/v0.3`, `POST /qos-assignments` = `createQosAssignment`
  scope `qos-provisioning:qos-assignments:create`, `GET /qos-assignments/{id}` =
  `getQosAssignmentById` scope `…:read`; request `CreateAssignment {device?,
  qosProfile(req), sink?, sinkCredential?}`; 201 → `AssignmentInfo {assignmentId,
  qosProfile, device?, status(REQUESTED/AVAILABLE/UNAVAILABLE), statusInfo?,
  startedAt?, sink?}`; errors 400 [INVALID_ARGUMENT/OUT_OF_RANGE/INVALID_CREDENTIAL/
  INVALID_TOKEN/INVALID_SINK]/401/403/404/409 CONFLICT/422 [MISSING/UNSUPPORTED/
  UNNECESSARY_IDENTIFIER/SERVICE_NOT_APPLICABLE/QOS_PROVISIONING.QOS_PROFILE_NOT_
  APPLICABLE]/429). First slice = the create + read-by-id pair (mirroring QoD's
  first slice). New `src/apis/qos_provisioning/{,store,v0_3}.rs` + wired into
  apis/openapi/main catalog; vendored/annotated spec at
  `specs/qos-provisioning/v0.3/openapi.yaml`, served + catalogued. In-memory
  assignment store (`Mutex<HashMap>`, UUID-shaped id via SHA-256, no uuid/rand
  dep, mirroring QoD's store). Device-object identifier resolution + two-legged/
  three-legged rule (device on line token → 422 UNNECESSARY_IDENTIFIER; no device
  + non-line subject → 422 MISSING_IDENTIFIER; empty device → 400). Control planes
  (DESIGN §7): reserved suffix → canonical error (`…409` → 409 CONFLICT "existing
  provisioning"); else `…000`/no-digits → REQUESTED (no startedAt), else AVAILABLE
  (startedAt=now, no expiresAt — open-ended); qosProfile name `unavailable` → 422
  QOS_PROFILE_NOT_APPLICABLE; non-https `sink` → 400 INVALID_SINK. sinkCredential
  accepted-not-applied; DELETE/retrieve-by-device/notifications deferred. No new
  dependency (serde/serde_json/axum/sha2 + self-contained RFC 3339 formatter). 20
  new tests; `cargo test` 976 passed (was 956); `cargo build --release` clean, no
  warnings. — binary: 2,203,016 bytes (2.2M)
- 2026-08-05 — Cross-cutting (contract-test harness, first slice): added a
  registry/wiring contract test guarding catalog↔spec drift. New
  `apis::openapi::api_spec_urls()` is the single source of truth for served API
  spec URLs; `main.rs` test `catalog_spec_urls_match_served_specs_and_resolve`
  asserts the `/` catalog's `spec_url` set equals it and that every catalogued
  `spec_url` resolves as `application/yaml` through the full app. Refactored
  `openapi.rs::serves_every_mounted_api_spec` to iterate that source of truth
  instead of a duplicated 28-path list. Stateless/spatial API backlog is fully
  implemented; remaining new APIs (e.g. QoD Provisioning, Scam Signal) either
  need their canonical spec fetched (proxy returned 404s this run) or are
  on-demand-only, so I did a safe, verifiable, test-only increment rather than
  implement from memory. Test-only change → binary unchanged. `cargo test`: 956
  passed (2 new/updated); `cargo build --release`: clean, no warnings. — binary: 2,163,760 bytes (2.1M)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Population Density Data vwip** —
  new stateless, **area-keyed** aggregate API `POST
  /population-density-data/vwip/retrieve` (`retrievePopulationDensity`, scope
  `population-density-data:read`). Mounted at its canonical `vwip` path (no
  released version yet). Verified the authoritative CAMARA spec (info.version
  `wip`) via WebFetch. Implements the **synchronous GEOHASHLIST** path: one
  `CellPopulationDensityData` per input geohash in a single time slice, plus an
  overall `status`. Control planes (DESIGN §7): the first geohash's reserved
  error suffix → canonical CAMARA error (geohashes carry digits, shared
  convention unchanged); each geohash's FNV-1a hash fixes its cell
  (NO_DATA/LOW_DENSITY/DENSITY_ESTIMATION with a ±10% band); the cell mix fixes
  `status` (SUPPORTED/PART/AREA_NOT_SUPPORTED). API-specific 422s wired to real
  cases: POLYGON→UNSUPPORTED_AREA_TYPE, >9-char geohash→UNSUPPORTED_PRECISION,
  >100 geohashes→UNSUPPORTED_SYNC_RESPONSE; window checks →
  INVALID_END_TIME/MAX_TIME_PERIOD_EXCEEDED (self-contained RFC 3339 parser).
  Documented cuts: POLYGON, async sink/CloudEvents, hourly slicing, ±3-month
  start-time checks. Vendored + annotated spec at
  `specs/population-density-data/vwip/openapi.yaml`, served + catalogued. No new
  dependency. Added 26 tests; 955 tests green (was 929). — binary: 2.1M (2163760 B)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Device Visit Location vwip** — new
  stateless, device-keyed API `POST /device-visit-location/vwip/retrieve`
  (`retrieveDeviceVisitLocation`, scope `device-visit-location:retrieve`).
  Mounted at its canonical `vwip` path (the API has no released version yet).
  Returns a `geoCodeList` of `{countryCode, codeType:"PostalCode", codeValue}`
  places a device visited over a required RFC 3339 `startTime`/`endTime` window.
  Two control planes (DESIGN §7): the identifier (device id → token subject,
  two/three-legged rule; reserved suffix → canonical error checked first;
  `…000`/no-digits → 404 `DEVICE_VISIT_LOCATION.DATA_NOT_FOUND`; else
  `((d-1)%3)+1` codes over a fixed `[US,GB,DE,FR,ES,IT]` table w/ deterministic
  postal codes) and the time window (malformed → 400 `INVALID_ARGUMENT`;
  `endTime < startTime` → 400 `DEVICE_VISIT_LOCATION.INVALID_END_DATE`;
  self-contained RFC 3339 parser). Vendored + annotated spec at
  `specs/device-visit-location/vwip/openapi.yaml`, served + catalogued. No new
  dependency. Added 23 tests; 929 tests green (was 906). — binary: 2.1M (2122808 B)
- 2026-08-04 23:45Z — Phase 5 (other CAMARA APIs): **Region Device Count v0.2**
  — new stateless, **area-keyed** (not identifier-keyed) aggregate-count API,
  **completes the API in one pass** (single endpoint). The published stateless
  non-spatial CAMARA set is now essentially exhausted; RDC is spatial-input but
  stateless and single-endpoint (Phase 4 spatial work already complete). Verified
  the authoritative CAMARA RegionDeviceCount spec at tag **r2.2** via WebFetch
  (`info.version` "0.2.0", base `/region-device-count/v0.2`, `POST /count` =
  `count`, scope `region-device-count:count`; request `RegionDeviceCountRequestBody
  {area(CIRCLE center+radius / POLYGON boundary), starttime?, endtime?, filter?,
  sink?, sinkCredential?}`; 200 → `{count?, status}` with status enum SUPPORTED_AREA
  / PART_OF_AREA_NOT_SUPPORTED / AREA_NOT_SUPPORTED / DENSITY_BELOW_PRIVACY_THRESHOLD
  / TIME_INTERVAL_NO_DATA_FOUND; errors 400 [INVALID_ARGUMENT + RDC-specific
  INVALID_CIRCLE_AREA/INVALID_POLYGON_AREA/TIME_INVALID_ARGUMENT/INVALID_END_DATE/
  UNSUPPORTED_SYNC_RESPONSE/UNSUPPPORTED_REQUEST(triple-P)/INVALID_CREDENTIAL/
  INVALID_TOKEN]/401/403/410(async)/429). New `src/apis/region_device_count/{,v0_2}.rs`
  + vendored/annotated `specs/region-device-count/v0.2/openapi.yaml`; wired into
  apis/openapi/main catalog. No phone/device identifier, so control planes
  (DESIGN §7) key off the area: characteristic radius `r` (circle radius; polygon
  `sqrt(area/π)` via a self-contained equirectangular shoelace) drives a size plane
  (`>1e6 m` → UNSUPPPORTED_REQUEST; `5e5–1e6 m` no sink → UNSUPPORTED_SYNC_RESPONSE,
  with sink → sync) and a status/count plane on `round(r)`'s trailing three digits
  (…429→429; …001..004 → the four non-count statuses; else SUPPORTED_AREA); count =
  area×500 dev/km² narrowed by `filter` (2nd plane). Self-contained RFC 3339 parser
  for the time-interval validation; async `sink`/CloudEvents (+410) a documented cut.
  No new dependency (serde/serde_json/axum + std f64 maths). 34 new tests; `cargo test`
  906 passed (was 872); `cargo build --release` clean, no warnings.
  — binary: 2,093,848 bytes (~2.0M)

- 2026-08-04 22:45Z — Phase 5 (other CAMARA APIs): **Connectivity Insights v0.6**
  — new stateless, non-spatial, device-keyed network-quality API, **completes the
  API in one pass** (single endpoint). Verified the authoritative CAMARA
  ConnectivityInsights spec at tag **r3.2** via WebFetch (`info.version` "0.6.0",
  base `/connectivity-insights/v0.6`, `POST /check-network-quality` =
  `checkNetworkQuality`, scope `connectivity-insights:check`, request
  `NetworkQualityInsightRequest {applicationProfileId(uuid,req), device?,
  applicationServer(req), applicationServerPorts?, monitoringTimeStamp?}`, 200 →
  `NetworkQualityInsightResponse` {five PolicyFulfilmentConfidence KPIs +
  additionalKPIs(signalStrength/connectivityType) + device?}, errors
  400(INVALID_ARGUMENT/OUT_OF_RANGE)/401/403(PERMISSION_DENIED/INVALID_TOKEN_
  CONTEXT)/404(NOT_FOUND/IDENTIFIER_NOT_FOUND)/422(SERVICE_NOT_APPLICABLE/
  MISSING_IDENTIFIER)/429). New `src/apis/connectivity_insights/{,v0_6}.rs` +
  vendored/annotated `specs/connectivity-insights/v0.6/openapi.yaml`; wired into
  apis/openapi/main catalog. Device-object identifier resolution mirrors Connected
  Network Type; **no** UNNECESSARY_IDENTIFIER in 0.6.0 (device on a line token is
  used); no device + non-line subject → 422 MISSING_IDENTIFIER. applicationProfileId
  UUID-validated but not resolved against a profile store (documented cut — no
  Application Profiles store); applicationServer needs ≥1 addr; ports range-checked
  (0..=65535 → else 400 OUT_OF_RANGE); monitoringTimeStamp lightly validated. Two
  control planes (DESIGN §7): reserved suffix → canonical error; else the tail's
  low five bits are an unmet-mask over the five KPIs (`…000` → all met →
  excellent/5G-SA; `…031` → all unmet → no signal/3G), additionalKPIs degrade
  coherently with the count met. Device echoed only for phoneNumber requests. No
  new dependency (reuses serde/serde_json/axum). 27 new tests; `cargo test` 872
  passed (was 845); `cargo build --release` clean, no warnings.
  — binary: 2,046,280 bytes (~2.0M)

- 2026-08-04 21:45Z — Phase 5 (other CAMARA APIs): **Connected Network Type v0.2**
  — new stateless, non-spatial, device-keyed radio-access API, **completes the API
  in one pass** (single endpoint). Verified the authoritative CAMARA
  ConnectedNetworkType spec at tag **r1.2** via WebFetch (`info.version` "0.2.0",
  base `/connected-network-type/v0.2`, `POST /retrieve` = `getConnectedNetworkType`,
  scope `connected-network-type:read`, request `{device?}`, 200 →
  `{connectedNetworkType: 2G|3G|4G|5G|UNKNOWN, lastStatusTime(nullable), device?}`,
  errors 400/401/403/404/422 (MISSING/UNNECESSARY/UNSUPPORTED_IDENTIFIER)/429).
  New `src/apis/connected_network_type/{,v0_2}.rs` + vendored/annotated
  `specs/connected-network-type/v0.2/openapi.yaml`; wired into apis/openapi/main
  catalog. Device-object identifier resolution + two-legged/three-legged rule
  mirror Simple Edge Discovery (device on line token → 422 UNNECESSARY_IDENTIFIER;
  no device + non-line subject → 422 MISSING_IDENTIFIER; empty device → 400). Two
  control planes (DESIGN §7): reserved suffix → canonical error; else trailing
  digits index `[5G,4G,3G,2G,UNKNOWN]` (`% 5`, `…000`→5G — type is a 2nd plane);
  `lastStatusTime` now/`null` for UNKNOWN (self-contained RFC 3339 formatter, no
  new dep). Device echoed only for phoneNumber requests. Subscription surface out
  of scope. 21 new tests; `cargo test` 845 passed; `cargo build --release` ok.
  — binary: 2,006,072 bytes (~1.9M)

- 2026-08-04 20:45Z — Phase 5 (other CAMARA APIs): **Customer Insights v0.2** —
  new stateless, non-spatial, phone-number-keyed risk/trust-scoring API,
  **completes the API in one pass** (single endpoint). Verified the authoritative
  CAMARA CustomerInsights spec at tag **r2.2** via WebFetch: `info.version`
  "0.2.0", base `/customer-insights/v0.2`, `POST /scoring/retrieve`
  (`retrieveScoring`, scope `customer-insights:scoring:read`), request
  `{ idDocument?, phoneNumber?, scoringType }` (scoringType required:
  gaugeMetric|veritasIndex), 200 → `{ scoringType, scoringValue }` (gaugeMetric
  300–850, veritasIndex 0–19), errors 400/401/403/404/422 (MISSING/UNNECESSARY_
  IDENTIFIER, CUSTOMER_INSIGHTS.*)/429. New `src/apis/customer_insights/{,v0_2}.rs`
  + vendored/annotated `specs/customer-insights/v0.2/openapi.yaml`; wired into
  apis/openapi/main catalog. Identifier resolution + two-legged/three-legged rule
  mirror KYC Tenure; id-document-alone → 422 ID_DOCUMENT_NOT_SUPPORTED. Two
  control planes (DESIGN §7): reserved suffix → canonical error; else trailing
  digits × scoringType fix the score (`gaugeMetric` 300+d%551, `veritasIndex`
  d%20 — scoringType is a real second plane). No new dependency. 24 new tests;
  `cargo test` 824 passed; `cargo build --release` ok. — binary: 1,983,776 bytes (~1.9M)
- 2026-08-04 19:45Z — Phase 5 (other CAMARA APIs): **Simple Edge Discovery v2** —
  new stateless, non-spatial, device-keyed edge/MEC-discovery API, **completes the
  API in one pass** (single endpoint). Verified the authoritative CAMARA
  SimpleEdgeDiscovery spec at tag **r2.3** via WebFetch: `info.version` "2.0.1",
  base `/simple-edge-discovery/v2`, `POST /retrieve-closest-edge-cloud-zone`
  (`readClosestEdgeCloudZone`, scope `simple-edge-discovery:read`), request
  `{device?: {phoneNumber?/networkAccessIdentifier?/ipv4Address?/ipv6Address?}}`,
  200 → `EdgeCloudZone {edgeCloudZoneId(uuid), edgeCloudZoneName, edgeCloudProvider,
  device?}`, errors 400/401/403/404/422 (MISSING/UNNECESSARY_IDENTIFIER)/429. New
  `src/apis/simple_edge_discovery/{,v2}.rs`. Device-object identifier resolution
  (mirrors device-reachability) + the two-legged/three-legged rule (mirrors
  call-forwarding): device id + line subject → 422 UNNECESSARY_IDENTIFIER; no
  device + non-line subject → 422 MISSING_IDENTIFIER; empty device → 400. Two
  control planes (DESIGN §7): reserved suffix → canonical error; else trailing
  digits index a fixed 6-entry edge-zone table (`% 6`, `…000`→entry 0 — zone is a
  2nd plane). `edgeCloudZoneId` UUID-shaped, deterministic + stable per zone
  (SHA-256; **no new dep** — reuses sha2). Device echoed only for phoneNumber
  requests. Wired into `apis.rs`, `openapi.rs` (served at
  `/simple-edge-discovery/v2/openapi.yaml`) and the `/` catalog. Spec vendored +
  annotated (`specs/simple-edge-discovery/v2/openapi.yaml`) with functional cases,
  `x-camarasim-scenarios`, examples, full schemas. Tests: +19 (units: zone index
  by digits, deterministic stable-per-zone UUID, E.164, device precedence;
  integration: closest-zone fields, distinct zones per tail, no device echo for
  IP, reserved→404/429, three-legged subject + reserved subject,
  UNNECESSARY/MISSING_IDENTIFIER, empty/invalid/unknown-field/malformed body → 400,
  no-scope→403, no-token→401, x-correlator echo). `cargo test` 800 green (was 781);
  `cargo build --release` clean. — binary: 1.9M (1959280 B)
- 2026-08-04 — Phase 5: **Blockchain Public Address v0.3 — stateful unbind**
  (`DELETE /blockchain-public-addresses/{id}`, `deleteBlockchainPublicAddress`,
  scope `blockchain-public-address:delete`), **completing Blockchain Public
  Address v0.3**. Verified the op against the authoritative CAMARA r2.2 spec via
  WebFetch: `DELETE …/{id}` (path param `id`: string), scope
  `…:delete`, responses `204`/400/401/403/404/429. Keyed only on the in-memory
  store (new `store::remove` — single lock hold, `.remove().is_some()`): present
  → `204 No Content` (single-use), absent/unknown → `404 NOT_FOUND`. The `id` is
  opaque, so no reserved-identifier plane (mirrors QoD `deleteSession` / Carrier
  Billing). Bindings aren't scoped per subscriber, so the spec's `sub`-ownership
  check is a documented cut. Spec updated: new `DELETE …/{id}` path, `BindingId`
  path param, `x-camarasim-scenarios`; header comment now says v0.3 complete. No
  new dep. Tests: +6 (204 evict + store-gone; single-use → 404; unknown id →
  404; wrong scope → 403 + binding survives; no token → 401; x-correlator echoed
  on 204 and 404). `cargo test`: 781 pass. binary (release): 1.9M (1,936,520 B).
- 2026-08-04 — Phase 5: **Blockchain Public Address v0.3 — stateful bind**
  (`POST /blockchain-public-addresses`, `bindBlockchainPublicAddress`, scope
  `blockchain-public-address:create`). Verified the op against the authoritative
  CAMARA r2.2 spec via WebFetch: request `BindBlockchainPublicAddressRequest
  {phoneNumber, blockchainPublicAddress, blockchainNetworkId (all required),
  currency?, nonce?, signature?}`, 201 → `BindBlockchainPublicAddressResponse
  {id}`, errors incl. 400 `…INVALID_BLOCKCHAIN_NETWORK_IDENTIFIER`/
  `…BOTH_NONCE_SIGNATURE_REQUIRED`, 409 `ALREADY_EXISTS`, 422
  `…UNSUPPORTED_ENHANCED_VALIDATION`. Makes the API **stateful**: new in-memory
  binding store (`src/apis/blockchain_public_address/store.rs`; `Mutex<HashMap>`,
  lock never held across await, id derived from the triple so a duplicate
  collides → 409). Three control planes (DESIGN §7): phoneNumber reserved-error
  suffix → canonical error; request validation (network-id format, EVM-address
  form, nonce/signature pairing → 400s, both → 422 unsupported-enhanced); store
  duplicate → 409. `nonce+signature` (enhanced ownership validation) is
  unsupported (no chain) → 422 — a faithful, documented behaviour. `retrieve`
  read op left deterministic-synthetic (does not read the store — a deferred
  reconciliation); `DELETE …/{id}` deferred (will reuse `store::get`). **No new
  dep** (reuses `sha2`). Spec updated: new POST path, `x-camarasim-scenarios`,
  `BindBlockchainPublicAddress{Request,Response}` schemas; `DELETE` still omitted
  so the contract advertises no unimplemented path. Tests: +16 (units: network-id
  + EVM-address validation, deterministic UUID-shaped binding id; integration:
  persist+mint id, 409 re-bind, different-address-same-line ok, reserved→404,
  malformed phone→400, bad network-id→400, bad address→400, lone nonce→400, both
  →422, unknown field→400, no create-scope→403, no token→401, x-correlator echo).
  `cargo test` 775 green (was 759); `cargo build --release` clean (no warnings).
  — binary: 1.9M (1930656 B)
- 2026-08-04 — Phase 5 (other CAMARA APIs): **Blockchain Public Address v0.3** —
  new stateless, non-spatial, phone-number-keyed Web3-onboarding API. Verified the
  authoritative CAMARA BlockchainPublicAddress r2.2 spec (`blockchain-public-address`
  0.3.0) via WebFetch: base `/blockchain-public-address/v0.3`, `POST
  /blockchain-public-addresses/retrieve-blockchains` (`retrieveBlockchainPublicAddress`,
  scope `blockchain-public-address:read`), request `PhoneNumber {phoneNumber (required)}`,
  200 → array of `BlockchainPublicAddressResponse {id, blockchainPublicAddress,
  blockchainNetworkId, currency?}`, errors 400/401/403/404/429. Scoped this pass to
  the stateless read op; the stateful `bind`(POST)/`delete`(DELETE) ops deferred to a
  later slice (need an in-memory store, mirroring QoD/Carrier Billing slicing). New
  `src/apis/blockchain_public_address/{,v0_3}.rs` (**no new dep** — reuses `sha2`).
  `phoneNumber` required (no three-legged fallback — the 0.3.0 schema marks it
  required; missing/malformed → 400 INVALID_ARGUMENT). Two control planes (DESIGN
  §7): reserved suffix → canonical error; else trailing digits `d` → `[]` when
  `d==0`, else `((d-1)%3)+1` addresses (1–3), `i`-th on `NETWORKS[(d+i)%6]` (fixed
  CAIP-2 EVM table → chain is a 2nd plane). Deterministic `0x…`/UUID via SHA-256;
  addresses lowercase (not EIP-55) — documented cut. `x-correlator` echoed. Wired
  into `apis.rs`, `openapi.rs` (served at `/blockchain-public-address/v0.3/openapi.yaml`)
  and the `/` catalog. Spec vendored + annotated
  (`specs/blockchain-public-address/v0.3/openapi.yaml`) with functional cases,
  `x-camarasim-scenarios`, examples, full `PhoneNumber`/`BlockchainPublicAddressResponse`
  schemas (read op only; bind/delete intentionally omitted until implemented). Tests:
  +16 (units: empty-for-zero-tail, count-from-digits, deterministic well-shaped
  records, network 2nd plane, E.164; integration: bound addresses, empty list,
  count tracks digits, reserved suffix→404/429, missing/malformed phone→400, unknown
  field, malformed JSON, no-scope→403, no-token→401, x-correlator echo on 200+error).
  `cargo test` 759 green (was 743); `cargo build --release` ok. — binary: 1.9M (1911600 B)
- 2026-08-04 — Phase 5 (other CAMARA APIs): **KYC Tenure v0.2** — new stateless,
  non-spatial, phone-number-keyed identity/anti-fraud API (part of Know Your
  Customer), **completes the API in one pass** (single endpoint). Verified the
  authoritative CAMARA Tenure r2.2 spec (`kyc-tenure` 0.2.0) via WebFetch: base
  `/kyc-tenure/v0.2`, `POST /check-tenure` (`checkTenure`, scope
  `kyc-tenure:check-tenure`), request `Tenure {phoneNumber?, tenureDate}`
  (tenureDate required, `date`), 200 `TenureInfo {tenureDateCheck, contractType?
  enum PAYG/PAYM/Business}`, errors 400 INVALID_ARGUMENT/OUT_OF_RANGE, 404, 422
  MISSING/UNNECESSARY_IDENTIFIER. New `src/apis/kyc_tenure/{,v0_2}.rs` (**no new
  dep**): reuses the two-legged/three-legged identifier rule (422
  UNNECESSARY_/MISSING_IDENTIFIER) and shared reserved-error convention. Two
  control planes (DESIGN §7): identifier reserved suffix → canonical error; else
  trailing digits = days-of-tenure vs `tenureDate` → tenureDateCheck true iff
  tenure started on/before the date (`(today − tenureDate) <= digits`), so
  `tenureDate` is a real second plane. `contractType` derived (`digits % 3`).
  tenureDate validated (malformed → 400 INVALID_ARGUMENT, future → 400
  OUT_OF_RANGE). Self-contained civil-date parser (mirrors Number Recycling).
  `x-correlator` echoed. Wired into `apis.rs`, `openapi.rs` (served at
  `/kyc-tenure/v0.2/openapi.yaml`) and the `/` catalog. Spec vendored + annotated
  (`specs/kyc-tenure/v0.2/openapi.yaml`) with functional cases,
  `x-camarasim-scenarios`, examples, full `Tenure`/`TenureInfo` schemas. Tests:
  +23 (units: date round-trip, parse validation, contractType, E.164; integration:
  tenure verdict recent/old/boundary, tenureDate second-plane, contractType,
  reserved suffix, three-legged subject/reserved/unnecessary, missing-identifier,
  future/malformed/missing tenureDate, bad phone, unknown field, malformed JSON,
  no-scope→403, no-token→401, x-correlator echo). `cargo test` 743 green;
  `cargo build --release` ok. — binary: 1.9M (1892592 B)
- 2026-08-04 — Phase 5: **QoS Profiles v1 completed** — added the single-profile
  lookup `GET /qos-profiles/{name}` (`getQosProfile`, scope `qos-profiles:read`),
  mounted at `/qos-profiles/v1/qos-profiles/:name` (axum `matchit`; distinct first
  segment from `/retrieve-qos-profiles`, no route collision). Reads the same fixed
  catalog as the list; the `name` path parameter is the sole control plane (DESIGN
  §7, no body/`device` → no error plane): known name → `200` that `QosProfile`
  (single object, not an array); well-formed unknown name → `404 NOT_FOUND` (the
  list returns `[]` — the deliberate list/lookup contrast); malformed name (fails
  `QosProfileName` `^[a-zA-Z0-9_.-]+$` / len 3–256) → `400 INVALID_ARGUMENT`.
  Reuses `is_valid_profile_name`/`catalog`/`with_correlator` + `CamaraError::
  not_found`; `x-correlator` echoed on success and 404; non-blocking, stateless
  (**no new dep**). Spec updated (`specs/qos-profiles/v1/openapi.yaml`): added the
  `/qos-profiles/{name}` GET path (params, 200 `QosProfile` example, 400/401/403/
  404/429/500/503 error set, `x-camarasim-scenarios`) + header note. Tests: +7
  integration (known name→object, every catalog name→200, unknown→404, malformed→
  400, no-scope→403, no-token→401, x-correlator echo on 200+404). `cargo test` 720
  green; `cargo build --release` ok. — binary: 1.8M (1869632 B)
- 2026-08-04 — Phase 5 (other CAMARA APIs): **QoS Profiles v1** — new stateless
  catalog API, the read-only companion to Quality on Demand (QoD *applies* a
  `qosProfile`; QoS Profiles *lists* them). Verified the authoritative CAMARA
  QualityOnDemand r3.2 spec via WebFetch: `qos-profiles` 1.1.0, base
  `/qos-profiles/v1`, `POST /retrieve-qos-profiles` (`retrieveQoSProfiles`, scope
  `qos-profiles:read`), request `QosProfileDeviceRequest {device?,name?,status?}`,
  200 → array of `QosProfile`, error set incl. 422 SERVICE_NOT_APPLICABLE /
  UNSUPPORTED_IDENTIFIER / UNNECESSARY_IDENTIFIER. Scoped this pass to the list
  endpoint; `GET /qos-profiles/{name}` (`getQosProfile`) deferred. New
  `src/apis/qos_profiles/{,v1}.rs` (**no new dep**): serves a fixed in-memory
  catalog (6 profiles covering all 3 `QosProfileStatusEnum` states + a spread of
  service classes / L4S queue types), filtered by `name`/`status` (genuine control
  planes; unknown `name` → `[]`, a list never 404s). `device` is an optional error
  plane only (DESIGN §7): first present identifier's reserved suffix → canonical
  CAMARA error; `device` on a three-legged line token (E.164 `sub`) → 422
  UNNECESSARY_IDENTIFIER. Bad `name` pattern / unknown `status` enum / empty
  `device` / malformed `phoneNumber` → 400 INVALID_ARGUMENT. `x-correlator` echoed.
  Non-blocking, stateless (no store). Wired into `apis.rs`, `openapi.rs` (served at
  `/qos-profiles/v1/openapi.yaml`) and the `/` catalog. Spec vendored + annotated
  (`specs/qos-profiles/v1/openapi.yaml`) with functional cases, `x-camarasim-
  scenarios`, examples, full `QosProfile`/`Rate`/`Duration` schemas. Tests: +21 (2
  units [catalog covers every status + unique names, name-pattern validation] + 19
  integration: empty-body/`{}`→full catalog, name filter→1, unknown name→`[]`,
  status filter, name+status AND, device reserved suffix (404/422), device happy
  path→catalog, non-phone device id, three-legged UNNECESSARY, no-device line
  token→catalog, empty device→400, bad phone→400, bad name→400, unknown status→400,
  unknown field→400, no-scope→403, no-token→401, x-correlator echo). `cargo test`
  713 green; `cargo build --release` ok. — binary: 1.8M (1862592 B)
- 2026-08-04 — Phase 5 (other CAMARA APIs): **Home Devices QoD v0.4** — new
  stateless, non-spatial, **ipAddress-keyed** API (raise/restore per-device QoS on
  the subscriber's home LAN, distinct from network-side QoD). `PUT /qos` (`setQos`,
  scope `home-devices-qod:qos:write`) live at `/home-devices-qod/v0.4/qos`; applies
  `serviceClass` to the device at `ipAddress` → `204 No Content`. Control planes
  (DESIGN §7): the device is the internal LAN `ipAddress` and its **last octet** is
  the control plane — happy path (`204`) unless a reserved octet (`…240`–`…253`)
  names a specific home-network condition mapping to the CAMARA 0.4.0 error set
  (`404 DEVICE_NOT_FOUND`; the `409 HOME_DEVICES_QOD.*` conflicts TOO_MANY_DEVICES/
  RSSI_BELOW_THRESHOLD/QOS_TOO_HIGH/OCCUPANCY_ABOVE_THRESHOLD/NOT_CONNECTED_/
  NOT_SUPPORTED_REQUIRED_INTERFACE/QOS_ALREADY_SET_TO_DEFAULT; `503 ROUTER_OFFLINE`;
  `504 TIMEOUT`; `500 INTERNAL`; plus generic 409/404/503). `serviceClass` is a
  genuine second control plane: `…247` only conflicts (QOS_ALREADY_SET_TO_DEFAULT)
  when restoring `standard`, else it's a happy path. Validation first: bad body /
  unknown `serviceClass` / non-IPv4 `ipAddress` → 400 INVALID_ARGUMENT. Stateless
  (no store — no session to read back); non-blocking; std `Ipv4Addr` parse, no new
  dep. `x-correlator` echoed on every response incl. `204`. Spec vendored +
  annotated (`specs/home-devices-qod/v0.4/openapi.yaml`) with functional cases +
  served at `/home-devices-qod/v0.4/openapi.yaml`; catalog + openapi wiring updated.
  18 new tests (692 total, all green). — binary: 1.8M (1824520 B)
- 2026-08-04 — Phase 5 (other CAMARA APIs): **KYC Fill-in v0.3** — new stateless,
  non-spatial, phone-number-keyed identity API (the *return-attributes* counterpart
  of KYC Match), **completes the API in one pass** (single endpoint). Verified the
  authoritative CAMARA KnowYourCustomer spec (`kyc-fill-in` 0.3.0) via WebFetch: op
  `KYC_Fill-in`, base `/kyc-fill-in/v0.3`, `POST /fill-in`, scopes
  `kyc-fill-in:set-all` + per-attribute `kyc-fill-in:<attr>`, request
  `KYC_FillinRequest {phoneNumber?}`, 200 `KYC_FillinResponse` (20 optional identity
  attributes incl. Kana names, address parts, birthdate, email, gender enum), 422
  identifier codes. New `src/apis/kyc_fill_in/{,v0_3}.rs` (**no new dep**): reuses the
  two-legged/three-legged identifier rule (422 `UNNECESSARY_/MISSING_IDENTIFIER`) and
  shared reserved-error convention. Three control planes (DESIGN §7): identifier
  reserved suffix → canonical error; identifier trailing three digits pick 1 of 3
  fixed synthetic personas (`% 3`, one per gender); and the **granted scope set**
  filters the response (`set-all` → all 20 fields, else only per-attribute-scoped
  ones — a real second plane over the body). No `kyc-fill-in:*` scope → 403
  PERMISSION_DENIED. `x-correlator` echoed. Wired into `apis.rs`, `openapi.rs` (served
  at `/kyc-fill-in/v0.3/openapi.yaml`) and the `/` catalog. Spec: vendored + annotated
  `specs/kyc-fill-in/v0.3/openapi.yaml` (full functional-case description,
  `x-camarasim-scenarios`, examples, `KYC_FillinError` code enum). Tests: +20 (3 units
  [persona selection mod 3, all-persona-fields-non-empty, E.164] + 17 integration:
  set-all→20 fields, persona-by-digits + mod-3 collision, per-attribute scope
  narrowing, phoneNumber-needs-own-scope, set-all-alone, reserved suffix, three-legged
  subject/reserved-subject, UNNECESSARY_/MISSING_IDENTIFIER, empty-body subject
  fallback, invalid phone, unknown field, malformed json, no-scope→403, no-token→401,
  x-correlator). `cargo test` 674 green (was 654); `cargo build --release` ok —
  binary: 1801240 bytes (1.80M, +28392 B).

- 2026-08-04 — Phase 5 (other CAMARA APIs): **Device Swap v1 `POST /retrieve-date`**
  — **completes Device Swap v1**. Verified the authoritative CAMARA DeviceSwap r3.2
  spec for `retrieveDeviceSwapDate`: scope `device-swap:retrieve-date`, base
  `/device-swap/v1`, body `CreateDeviceSwapDate {phoneNumber?}` (no `maxAge`), 200
  `DeviceSwapInfo {latestDeviceChange: string date-time nullable (required),
  monitoredPeriod?: integer days}`, plus the 422 identifier codes. Added `POST
  /retrieve-date` to `src/apis/device_swap/v1.rs` (mirrors SIM Swap v2's
  `retrieve-date`; **no new dep**): same two-legged/three-legged identifier rule as
  `check` (refactored `resolve_identifier` to take `Option<&str>` so both endpoints
  share it — 422 `UNNECESSARY_IDENTIFIER`/`MISSING_IDENTIFIER`), reserved-error
  suffix → canonical CAMARA error; the identifier's trailing three digits =
  hours-since-swap, `latestDeviceChange` = now − hoursAgo h when inside the fixed
  240 h (10-day) monitored period (aligned to `check`'s default `maxAge`), else
  `null`; `monitoredPeriod` (10 days) always reported. Self-contained RFC 3339 UTC
  formatter (Hinnant `civil_from_days`, no new dep). `x-correlator` echoed. Spec:
  vendored + annotated `specs/device-swap/v1/openapi.yaml` (new `/retrieve-date`
  path, `CreateDeviceSwapDate`/`DeviceSwapInfo` schemas, `x-camarasim-scenarios`,
  examples). Tests: +15 (4 units [rfc3339 known epochs, civil_from_days leap
  boundaries, last_swap_timestamp null-outside-period, now−hoursAgo monotonic] + 11
  integration: recent→timestamp+monitoredPeriod, old→null, reserved suffix,
  rejects-maxAge, invalid phone, three-legged empty-body subject fallback,
  UNNECESSARY_/MISSING_IDENTIFIER, wrong-scope→403, no-token→401, x-correlator).
  `cargo test` 654 green (was 639); `cargo build --release` ok — binary: 1772848
  bytes (1.77M, +13656 B).

- 2026-08-04 — Phase 5 (other CAMARA APIs): **Device Swap v1** — new stateless,
  non-spatial, phone-number-keyed anti-fraud API, the **device** counterpart of
  SIM Swap. Verified the latest published CAMARA DeviceSwap spec (release **r3.2 →
  device-swap 1.0.0**; `main` is `wip`): op `checkDeviceSwap`, scope
  `device-swap:check`, base `/device-swap/v1`, body `CreateCheckDeviceSwap
  {phoneNumber?, maxAge?(1–2400, default 240)}`, 200 `CheckDeviceSwapInfo
  {swapped: boolean}`, and the 422 identifier codes `MISSING_IDENTIFIER` /
  `UNNECESSARY_IDENTIFIER` / `SERVICE_NOT_APPLICABLE`. New
  `src/apis/device_swap/{,v1}.rs` (combines Number Recycling's two/three-legged
  identifier rule with SIM Swap's `maxAge`/hours-ago logic; **no new dep**): `POST
  /check`. Identifier = submitted `phoneNumber` (two-legged) else E.164 `sub`
  (three-legged); a number on a line token → 422 `UNNECESSARY_IDENTIFIER`, no
  number + non-line subject → 422 `MISSING_IDENTIFIER`. Two control planes
  (DESIGN §7): reserved error suffix → canonical CAMARA error; else the
  identifier's trailing three digits are hours-since-last-swap, `swapped = hoursAgo
  < maxAge` (so `maxAge` is a real second plane — same number flips true↔false as
  the window moves), no digits → never swapped; out-of-range `maxAge` → 400
  `OUT_OF_RANGE`. `x-correlator` echoed. Wired into `apis.rs`, `openapi.rs` (served
  at `/device-swap/v1/openapi.yaml`) and the `/` catalog. Spec: vendored +
  annotated `specs/device-swap/v1/openapi.yaml` (full functional-case description,
  `x-camarasim-scenarios`, examples, local `DeviceSwapError` code enum incl.
  `OUT_OF_RANGE` + the 422 identifier codes; full shared reserved-error set
  exposed). Tests: +19 (2 units [is_swapped recency × maxAge incl. strict
  boundary, e164] + 17 integration: recently/not-recently swapped,
  maxAge-widens/narrows [control plane], maxAge out-of-range/at-bounds, reserved
  suffix, three-legged subject-keyed + reserved subject, UNNECESSARY_/
  MISSING_IDENTIFIER, invalid phone, unknown-field, malformed json, no-scope→403,
  no-token→401, x-correlator echo) + catalog + openapi serving assertions. `cargo
  test` 639 green (was 620); `cargo build --release` ok — binary: 1759192 bytes
  (1.76M, +19528 B). `POST /retrieve-date` (device-swap:retrieve-date) is next.

- 2026-08-04 — Phase 5 (other CAMARA APIs): **KYC Age Verification v0.1** — new
  stateless, non-spatial, phone-number-keyed identity API. Verified the latest
  published CAMARA KnowYourCustomer spec (release **r2.2 → kyc-age-verification
  v0.1.0**; `main` is `wip` 0.1.0): op `verifyAge`, scope
  `kyc-age-verification:verify`, base `/kyc-age-verification/v0.1`, body
  `{ageThreshold(0–120, required), phoneNumber?, idDocument?, name?/givenName?/
  familyName?/middleNames?/familyNameAtBirth?/birthdate?/email?,
  includeContentLock?, includeParentalControl?}`, 200
  `{ageCheck: "true"|"false"|"not_available", verifiedStatus?,
  identityMatchScore?, contentLock?, parentalControl?}`, and the 422 identifier
  codes `MISSING_IDENTIFIER`/`UNNECESSARY_IDENTIFIER`. New
  `src/apis/kyc_age_verification/{,v0_1}.rs` (mirrors Number Recycling, **no new
  dep**): `POST /verify`. Identifier = submitted `phoneNumber` (two-legged) else
  E.164 `sub` (three-legged); a number on a line token → 422
  `UNNECESSARY_IDENTIFIER`, no number + non-line subject → 422
  `MISSING_IDENTIFIER`. Three control planes (DESIGN §7): `ageThreshold` range
  (0..=120, else 400 `OUT_OF_RANGE`); reserved error suffix → canonical CAMARA
  error; else the identifier's trailing three digits are the held age (`d % 100`),
  `ageCheck = held ≥ ageThreshold ? "true" : "false"` (so `ageThreshold` is a real
  second plane), `…000` tail → `"not_available"`. Optional response fields driven
  by the body: `identityMatchScore`=90 (any identity attribute), `verifiedStatus`
  =true (`idDocument`), `contentLock`/`parentalControl` (opt-in via `include*`,
  minor `< 18` → `"true"`, adult → `"false"`, unknown age → `"not_available"`).
  `x-correlator` echoed. Wired into `apis.rs`, `openapi.rs` (served at
  `/kyc-age-verification/v0.1/openapi.yaml`) and the `/` catalog. Spec: vendored +
  annotated `specs/kyc-age-verification/v0.1/openapi.yaml` (full functional-case
  description, `x-camarasim-scenarios`, examples, local `AgeVerificationError`
  code enum incl. `OUT_OF_RANGE` + the 422 identifier codes; full shared
  reserved-error set exposed). Tests: +27 (3 units [age_check, minor_signal, e164]
  + 24 integration: above/below/boundary threshold, ageThreshold-as-control-plane
  flip, `…000`→not_available, identity attrs→score+verifiedStatus, name-only
  scores-not-verified, content-lock/parental opt-in age-driven, lock
  not_available on unknown age, toggles-off-add-nothing, reserved suffix,
  threshold out-of-range/at-bounds/missing, three-legged subject-keyed + reserved
  subject, UNNECESSARY_/MISSING_IDENTIFIER, invalid phone, unknown-field, malformed
  json, no-scope→403, no-token→401, x-correlator echo) + catalog + openapi serving
  assertions. `cargo test` 620 green (was 593); `cargo build --release` ok —
  binary: 1739664 bytes (1.74M, +31672 B).
- 2026-08-04 — Phase 5 (other CAMARA APIs): **Number Recycling v0.2** — new
  stateless, non-spatial, phone-number-keyed account-integrity API. Verified the
  latest published CAMARA NumberRecycling spec (release **r2.2 → v0.2.0**; `main`
  is `wip`): op `checkNumberRecycling`, scope `number-recycling:check`, body
  `CreateCheckNumRecycling{phoneNumber?, specifiedDate(date, required)}`, 200
  `CheckNumRecyclingInfo{phoneNumberRecycled: boolean}`, and the two-legged /
  three-legged identifier rule (422 `MISSING_IDENTIFIER` / `UNNECESSARY_IDENTIFIER`).
  New `src/apis/number_recycling/{,v0_2}.rs` (mirrors Call Forwarding Signal, no
  new dep): `POST /check` mounted at the real `/number-recycling/v0.2`. Identifier
  = submitted `phoneNumber` (two-legged) else E.164 `sub` (three-legged). Two
  control planes (DESIGN §7): reserved error suffix → canonical CAMARA error; else
  the identifier's trailing three digits are days-since-the-last-subscriber-change,
  recycled iff strictly after `specifiedDate` — so `specifiedDate` is a genuine
  second plane (same number flips true↔false as the date moves). `specifiedDate`
  validated with a self-contained civil-date parser (Howard Hinnant
  `days_from_civil`/`civil_from_days`, no dep): malformed/impossible → 400
  `INVALID_ARGUMENT`, future → 400 `OUT_OF_RANGE`. `x-correlator` echoed. Wired
  into `apis.rs`, `openapi.rs` (served at `/number-recycling/v0.2/openapi.yaml`)
  and the `/` catalog. Spec: vendored + annotated
  `specs/number-recycling/v0.2/openapi.yaml` (full functional-case description,
  `x-camarasim-scenarios`, examples, local `NumberRecyclingError` code enum incl.
  `OUT_OF_RANGE` + the 422 identifier codes; full shared reserved-error set
  exposed). Tests: +20 (3 units [civil-date round-trip, `parse_date`
  valid/malformed/impossible, e164] + 17 integration: old-date→recycled,
  today→not-recycled, same-number date-flip [specifiedDate control plane],
  reserved suffix, three-legged subject-keyed + reserved subject,
  UNNECESSARY_/MISSING_IDENTIFIER, future→OUT_OF_RANGE, malformed/missing date,
  invalid phone, unknown-field, malformed json, no-scope→403, no-token→401,
  x-correlator echo) + catalog + openapi serving assertions. `cargo test` 593
  green (was 573); `cargo build --release` ok — binary: 1707992 bytes (1.71M,
  +22328 B).
- 2026-08-04 — Phase 5 (other CAMARA APIs): **Call Forwarding Signal v0.4** —
  **`POST /call-forwardings`** (`retrieveCallForwarding`, scope
  `call-forwarding-signal:call-forwardings:read`) added, **completing the API**.
  Reports the CAMARA `CallForwardingSignal` — the **set** of active forwarding
  types (`inactive`/`unconditional`/`conditional_busy`/
  `conditional_not_reachable`/`conditional_no_answer`). Reused the existing
  `resolve_identifier` (two-legged/three-legged, 422 `UNNECESSARY_IDENTIFIER`/
  `MISSING_IDENTIFIER`) and reserved-error convention; new control plane — the
  identifier's trailing three digits as a 4-bit mask (`digits % 16`) over the four
  active types via new `forwarding_set` helper (bit 0 = `unconditional`, so an odd
  tail lines up with the unconditional endpoint), zero mask → `["inactive"]`, set
  always non-empty + canonically ordered. `x-correlator` echoed. No new dep. Spec:
  vendored spec extended — `/call-forwardings` path (full description, examples,
  `x-camarasim-scenarios`, `call-forwardings:read` security) + new
  `CallForwardingSignal` array schema (`minItems:1`, `uniqueItems`). Tests: +12
  (1 unit on `forwarding_set` mask/order + 11 integration: inactive/unconditional/
  conditional-set, reserved suffix, three-legged subject-keyed, UNNECESSARY_/
  MISSING_IDENTIFIER, invalid phone, no-scope→403, no-token→401, x-correlator).
  `cargo test` 573 green (was 561); `cargo build --release` ok — binary: 1685664
  bytes (1.69M, +11128 B). Call Forwarding Signal v0.4 is now complete.
- 2026-08-04 — Phase 5 (other CAMARA APIs): **Call Forwarding Signal v0.4** — new
  stateless, non-spatial, phone-number-keyed anti-fraud API begun. Verified the
  real CAMARA CallForwardingSignal spec at release **r3.3 → v0.4.0** (latest
  published; `main` is `wip`): op `retrieveUnconditionalCallForwarding`, scope
  `call-forwarding-signal:unconditional-call-forwardings:read`, body
  `CreateCallForwardingSignal{phoneNumber?}`, 200 `UnconditionalCallForwardingSignal
  {active:boolean}`, and the 422 identifier codes `MISSING_IDENTIFIER` /
  `UNNECESSARY_IDENTIFIER` (phoneNumber valid only in two-legged auth). New
  `src/apis/call_forwarding_signal/{,v0_4}.rs` mirrors Number Verification / Carrier
  Billing (no new dep): `POST /unconditional-call-forwardings` mounted at the real
  `/call-forwarding-signal/v0.4`. Identifier = submitted `phoneNumber` (two-legged,
  E.164-validated) else E.164 `sub` (three-legged); a number on a line token → 422
  `UNNECESSARY_IDENTIFIER`, no number + non-line subject → 422 `MISSING_IDENTIFIER`.
  Control planes: reserved error suffix → canonical CAMARA error; else trailing-
  three-digit **parity** — odd → `active:true`, even (incl. `…000`) →
  `active:false`. `x-correlator` echoed. Wired into `apis.rs`, `openapi.rs` (served
  at `/call-forwarding-signal/v0.4/openapi.yaml`) and the `/` catalog. Spec:
  vendored + annotated `specs/call-forwarding-signal/v0.4/openapi.yaml` (full
  functional-case description, `x-camarasim-scenarios`, examples, local
  `CallForwardingError` code enum; full shared reserved-error set exposed so every
  suffix is reachable). Tests: +15 (1 e164 unit + 14 integration: even/odd/zero
  tail, reserved suffix, three-legged subject-keyed + reserved subject,
  UNNECESSARY_IDENTIFIER, MISSING_IDENTIFIER, invalid/unknown-field/malformed body,
  no-scope→403, no-token→401, x-correlator echo) + the catalog assertion. `cargo
  test` 561 green (was 546); `cargo build --release` ok — binary: 1674536 bytes
  (1.67M, +17336 B). `POST /call-forwardings` (forwarding-type list) is next.
- 2026-08-04 — Phase 5 (payments): **Carrier Billing v0.5** — **`payment-denied`
  on `validatePayment`** (final two-step *terminal* charging event; **completes
  Carrier Billing v0.5 charging notifications**). Added `payment_denied_event`
  builder + `EVENT_TYPE_PAYMENT_DENIED` to `notifications.rs` (mirrors
  `payment_cancelled_event`; reuses the fire-and-forget `spawn_delivery` + the
  `paymentId`-keyed notify side-store stashed at `preparePayment`; **no new dep**).
  A `validatePayment` whose `ValidationFailed` outcome denies a `…888` reservation
  prepared with a `sink` now takes the stashed `NotifyTarget` single-use (so a
  reservation fires exactly one terminal event — confirm, cancel, *or* deny) and
  delivers a `payment-denied` CloudEvent. Modelled `data.status: failed` (the flow
  ends without a charge) and **no** `paymentDate` (nothing charged, like
  payment-cancelled) — documented in code + spec. ACCESSTOKEN `sinkCredential`
  bearer applied; secret never echoed by `retrievePayment`. Tests: +4 (deny-with-
  sink delivers payment-denied [status failed, no paymentDate]; the bearer persists
  prepare→deny; a sink-less reservation stashes no target; + a notifications unit
  test on the event shape). Spec: top-level notification note + `validatePayment`
  description/scenarios/`callbacks` + `CloudEvent.type` enum/`data` oneOf + new
  `EventPaymentDenied` schema. `cargo test` 546 green (was 542); `cargo build
  --release` ok — binary: 1657200 bytes (1.58M, +6376 B).
- 2026-08-04 — Phase 5 (payments): **Carrier Billing v0.5** — **`payment-cancelled`
  on `cancelPayment`** (second two-step *terminal* charging event). Added
  `payment_cancelled_event` builder + `EVENT_TYPE_PAYMENT_CANCELLED` to
  `notifications.rs` (mirrors `payment_reserved_event`; reuses the fire-and-forget
  `spawn_delivery` + the `paymentId`-keyed notify side-store; **no new dep**). A
  `cancelPayment` that releases a reservation prepared with a `sink` now takes the
  stashed `NotifyTarget` single-use (so a reservation fires exactly one terminal
  event — confirm *or* cancel; a prior confirm would already have taken it) and
  delivers a `payment-cancelled` CloudEvent. Modelled `data.status: failed` (the
  flow ends without a charge) and **no** `paymentDate` (nothing charged, like
  payment-reserved) — documented in code + spec. ACCESSTOKEN `sinkCredential`
  bearer applied; secret never echoed by `retrievePayment`. Tests: +4 (cancel-with-
  sink delivers payment-cancelled [status failed, no paymentDate]; the bearer
  persists prepare→cancel; a sink-less reservation stashes no target; + a
  notifications unit test on the event shape). Spec: top-level notification note +
  `cancelPayment` description/scenarios/`callbacks` + `CloudEvent.type` enum/`data`
  oneOf + new `EventPaymentCancelled` schema. `cargo test` 542 green (was 538);
  `cargo build --release` ok — binary: 1650824 bytes (1.65M, +6152 B).
- 2026-08-04 — Phase 5 (payments): **Carrier Billing v0.5** — **`payment-completed`
  on `confirmPayment`** (first two-step *terminal* charging event). Added a
  `paymentId`-keyed notify side-store to `store.rs` (`NotifyTarget{sink, auth}`,
  `insert_notify`/`take_notify`, mirroring QoD's credential side-store, **no new
  dep**): `preparePayment` stashes the request `sink` + derived ACCESSTOKEN bearer
  (both `reserved` and `pending_validation` branches) since the confirm body has
  no `sink`; `confirmPayment`'s `Confirmed` transition takes it single-use and
  fire-and-forgets a `payment-completed` CloudEvent (reuses `spawn_delivery`).
  Secret never echoed by `retrievePayment`. Tests: confirm-with-sink delivers
  payment-completed (with paymentDate), the bearer persists from prepare→confirm,
  a sink-less reservation stashes no target, + a store unit test. Spec: top-level
  notification note + `confirmPayment` description/scenarios/`callbacks` updated.
  `cargo test` 538 green; `cargo build --release` ok — binary: 1644672 bytes (1.6M).
- 2026-08-04 — Phase 5 (payments): **Carrier Billing v0.5** — **`preparePayment`
  → `payment-pending-validation` notification**. Verified the real CAMARA
  CarrierBilling r3.2 event: `payment-pending-validation` `data` =
  paymentId/status/description only (**no** paymentDate, **no** validationInfo).
  Added `payment_pending_validation_event` builder +
  `EVENT_TYPE_PAYMENT_PENDING_VALIDATION` to `notifications.rs` (reuses the
  fire-and-forget `spawn_delivery` + `sink_authorization`; **no new dep**), wired
  into `prepare_payment`'s `…888` pending_validation branch (delivers before the
  `insert_pending` move). A `…888` reservation now fires
  `payment-pending-validation` instead of nothing (previously a documented cut) —
  mutually exclusive with `payment-reserved`. Spec: `CloudEvent.type` enum + `data`
  oneOf now include `payment-pending-validation`; new
  `EventPaymentPendingValidation` schema; refreshed `preparePayment` scenarios +
  the API-description charging-notifications note. 534 tests green (was 532; +2 net:
  +1 unit [pending-validation event shape, no paymentDate/validationInfo]; the old
  "fires no payment-reserved" integration test was rewritten into 2 [payment-
  pending-validation received & sink not echoed & not payment-reserved; ACCESSTOKEN
  sinkCredential → Bearer header & secret not echoed]). — binary: 1638640 B (+3928 B)

- 2026-08-04 — Phase 5 (payments): **Carrier Billing v0.5** — **`preparePayment`
  → `payment-reserved` notification**. Verified the real CAMARA CarrierBilling
  r3.2 notification set (5 event types: payment-reserved/-completed/-cancelled/
  -denied/-pending-validation; `payment-reserved` `data` = paymentId/status/
  description, **no** paymentDate). Added `payment_reserved_event` builder +
  `EVENT_TYPE_PAYMENT_RESERVED` to `notifications.rs` (reuses the existing
  fire-and-forget `spawn_delivery` + `sink_authorization`; **no new dep**), wired
  into `prepare_payment` on the `reserved` happy path (a `…888` pending_validation
  reservation fires nothing — that is the separate payment-pending-validation
  event, deferred). Spec: `preparePayment` gains a `reserveNotifications`
  `callbacks` block + 3 scenario cases; `CloudEvent.type` enum + `data` oneOf now
  include `payment-reserved`; new `EventPaymentReserved` schema; refreshed the
  API-description charging-notifications note. 532 tests green (was 528; +4: 1
  unit [reserved event shape, no paymentDate] + 3 integration [reserve+http sink →
  payment-reserved received & sink not echoed; ACCESSTOKEN sinkCredential → Bearer
  header & secret not echoed; a …888 pending_validation reservation fires no
  payment-reserved]). — binary: 1634712 B (+4680 B)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — **charging
  notifications on `sink` begun**: a successful one-step `createPayment` charge
  now delivers a `payment-completed` CloudEvent to the request's `sink`. Verified
  the real CAMARA CarrierBillingCheckOut r3.2 notification set — event type
  `org.camaraproject.carrier-billing.v0.payment-completed`, `data` (BasicEvent)
  = required `paymentId`/`status`(`succeeded`|`failed`)/`description`/
  `paymentDate`. New `src/apis/carrier_billing/notifications.rs` mirrors QoD:
  pure `payment_completed_event` builder + `sink_authorization` + fire-and-forget
  `spawn_delivery`/`deliver` over a raw `tokio` TCP stream (**no new dep**;
  `http://` sinks only — no TLS client; ACCESSTOKEN `sinkCredential` bearer
  applied, PLAIN/REFRESHTOKEN cut). Wired into `create_payment`: on a `succeeded`
  charge with a `sink`, spawn the event off the request path (so a slow sink
  never delays the `201`); the `sink` is used only to notify and never persisted
  (retrievePayment still omits it). `preparePayment` notify + the two-step
  terminal events remain. Spec: updated `specs/carrier-billing/v0.5/openapi.yaml`
  — `createPayment` `callbacks` (`payment-completed`), two new scenario cases,
  new `CloudEvent` + `EventPaymentCompleted` schemas, `SinkCredential` gains
  `accessToken`/`accessTokenType`, refreshed `sink` description + documented-cuts
  section. 528 tests green (was 519; +9: 6 unit in notifications.rs [event shape,
  parse_http_sink host/port/path + non-http reject, deliver posts a
  cloudevents+json POST, deliver sends Authorization when present,
  sink_authorization ACCESSTOKEN-only, non-http deliver is a no-op success] + 3
  integration [createPayment+http sink → payment-completed CloudEvent received &
  sink not echoed; ACCESSTOKEN sinkCredential → Bearer header & secret never
  echoed; a …404 reserved-error charge fires no notification]). — binary:
  1630032 B (+12432 B; the new notifications module + wiring + vendored spec text)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — **two-step
  `cancelPayment`** (`POST /payments/{paymentId}/cancel`, scope
  `carrier-billing:payments:write`), the cancel step — **completes the two-step
  reserve → validate → confirm / cancel flow**. Fully symmetric with the
  verified r3.2 `confirmPayment`: op `cancelPayment`, body `CancelPayment` (the
  `PhoneNumber` shape — optional `phoneNumber`, no required fields), responses
  `202` (no body) + 400/401/403/404/409/429, 409 codes
  `CARRIER_BILLING.PAYMENT_CANCELLED` (already cancelled) /
  `CARRIER_BILLING.PAYMENT_CONFIRMED` (already charged — can't cancel), 404
  NOT_FOUND. Implemented: releases a `reserved` payment → `cancelled` (no
  `paymentDate`, nothing is charged) → `202 Accepted`; already `cancelled` →
  409 `PAYMENT_CANCELLED`; already `succeeded` → 409 `PAYMENT_CONFIRMED`; other
  non-`reserved` (`pending_validation`/`denied`) → 409 `CONFLICT`; unknown id →
  404. New `store::cancel(id)` → `CancelOutcome` drives the transition
  atomically under the store lock (never across await), mirroring
  `store::confirm`. Store state is the only control plane (opaque `paymentId`);
  the optional `phoneNumber` body is accepted-not-applied. Only charging
  notifications on `sink` remain for Carrier Billing v0.5. **No new deps.**
  Spec: updated `specs/carrier-billing/v0.5/openapi.yaml` — new
  `POST /payments/{paymentId}/cancel` operation (`x-camarasim-scenarios`, full
  202/400/401/403/404/409/429/500/503 set with the three 409 examples), new
  `CancelPayment` schema (reused the existing PAYMENT_CANCELLED/PAYMENT_CONFIRMED
  error codes), new "Cancel" description section + refreshed header/cuts. 519
  tests green (was 505; +14: 1 unit [store::cancel drives every state branch:
  reserved→cancelled/no-paymentDate, →AlreadyCancelled, succeeded→AlreadyConfirmed,
  denied→NotCancellable, unknown→Unknown] + 13 integration [reserved→202/cancelled
  · optional phoneNumber body · malformed body→400 · cancel twice→PAYMENT_CANCELLED
  · confirmed→PAYMENT_CONFIRMED · one-step payment→PAYMENT_CONFIRMED · cancel→confirm
  →PAYMENT_CANCELLED · pending_validation→CONFLICT · denied→CONFLICT · unknown→404
  · no write scope→403 · no token→401 · x-correlator on 202+404]). — binary:
  1617600 B (+11576 B; the cancel handler + store::cancel + the new vendored spec
  text embedded via include_str!)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — **two-step
  `confirmPayment`** (`POST /payments/{paymentId}/confirm`, scope
  `carrier-billing:payments:write`), the confirm step. Verified the real CAMARA
  CarrierBillingCheckOut r3.2 spec: op `confirmPayment`, body `ConfirmPayment`
  (the `PhoneNumber` shape — optional `phoneNumber`, no required fields),
  responses `202` (no body) + 400/401/403/404/409/422/429, 409 codes
  `CARRIER_BILLING.PAYMENT_CONFIRMED` (already confirmed) / `…PAYMENT_CANCELLED`
  (already cancelled), 404 NOT_FOUND. Implemented: charges a `reserved` payment
  → `succeeded` (stamps `paymentDate`) → `202 Accepted`; already `succeeded` →
  409 `PAYMENT_CONFIRMED`; already `cancelled` → 409 `PAYMENT_CANCELLED`; other
  non-`reserved` (`pending_validation`/`denied`) → 409 `CONFLICT`; unknown id →
  404. New `store::confirm(id, payment_date)` → `ConfirmOutcome` drives the
  transition atomically under the store lock (never across await). Store state
  is the only control plane (opaque `paymentId`, no reserved-identifier plane);
  the optional `phoneNumber` body is accepted-not-applied (documented cut — the
  `paymentId` identifies the reservation). Only `cancelPayment` remains to
  complete the two-step flow. **No new deps.** Spec: updated
  `specs/carrier-billing/v0.5/openapi.yaml` — new `POST /payments/{paymentId}/
  confirm` operation (`x-camarasim-scenarios`, full 202/400/401/403/404/409/429/
  500/503 set with the three 409 examples), new `ConfirmPayment` schema, two new
  `CarrierBillingError` codes + `CONFLICT`, new "Confirm" description section +
  refreshed header/cuts. 505 tests green (was 493; +12: 1 unit [store::confirm
  drives every state branch: reserved→succeeded/paymentDate, →AlreadyConfirmed,
  cancelled→AlreadyCancelled, denied→NotConfirmable, unknown→Unknown] + 11
  integration [reserved→202/succeeded · optional phoneNumber body · malformed
  body→400 · confirm twice→PAYMENT_CONFIRMED · one-step payment→PAYMENT_CONFIRMED
  · pending_validation→CONFLICT · denied→CONFLICT · unknown→404 · no write
  scope→403 · no token→401 · x-correlator on 202+404]). — binary: 1606024 B
  (+12088 B; the confirm handler + store::confirm + the new vendored spec text
  embedded via include_str!)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — **two-step
  `validatePayment`** (`POST /payments/{paymentId}/validate`, scope
  `carrier-billing:payments:write`), the OTP-validation step. Verified the real
  CAMARA CarrierBillingCheckOut r3.2 spec: op `validatePayment`, body
  `ValidatePayment` (`authorizationId` + `code`), responses
  204/400/401/403/404/409/429, 400 codes `CARRIER_BILLING.INVALID_AUTHORIZATION_ID`
  / `…INVALID_CODE` / `…VALIDATION_FAILED`, `validationInfo` with an `action`
  discriminator (`validate` → an `authorizationId`). To have something to
  validate, `preparePayment` now produces `pending_validation` for a phone number
  ending in `888` (a free tail — not a reserved-error suffix), carrying a
  `validationInfo` (`action: "validate"`, minted `authorizationId`); the expected
  OTP `code` = the reserved number's last six digits, zero-padded (deterministic,
  mirroring OTP SMS), held in a new secret side-store `store::PendingValidation`
  apart from the echoed payment. `validatePayment` clears it atomically
  (`store::validate_pending`, locking pending→payments in a fixed order, never
  across await): correct id+code → `204` (reservation → `reserved`,
  `validationInfo` dropped); wrong `authorizationId` → 400 INVALID_AUTHORIZATION_ID;
  wrong `code` → 400 INVALID_CODE until the 3-attempt budget is spent → 400
  VALIDATION_FAILED (reservation → `denied`); a settled payment → 409
  ALREADY_EXISTS; unknown id → 404. Only `confirmPayment`/`cancelPayment` remain
  to complete the two-step flow; the 409 duplicate-session case on
  `preparePayment` is not modelled (documented cut). **No new deps.** Spec:
  updated `specs/carrier-billing/v0.5/openapi.yaml` — new `validatePayment`
  operation (`x-camarasim-scenarios`, full 204/400/401/403/404/409/429/500/503
  set with the three validation examples), new `ValidatePayment` + `ValidationInfo`
  schemas, `validationInfo` added to `PaymentReserved`, new error codes in
  `CarrierBillingError`, refreshed header/prepare description + cuts. 493 tests
  green (was 482; +11: 1 unit [otp_code last-six-padded] + 10 integration
  [prepare `…888` → pending_validation · validate ok → 204/reserved · wrong authId
  → 400 · wrong code ×3 → INVALID_CODE then VALIDATION_FAILED/denied · validate a
  reserved payment → 409 · re-validate a validated payment → 409 · unknown → 404 ·
  no write scope → 403 · no token → 401 · x-correlator echoed on 204 + error]). —
  binary: 1593936 B (+26128 B; the validate handler + side-store + the new
  vendored spec text embedded via include_str!)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — **two-step flow
  begun**: `POST /payments/prepare` (`preparePayment`, scope
  `carrier-billing:payments:create`), the reserve step. Verified the real CAMARA
  CarrierBillingCheckOut r3.2 spec — 0.5.0 **does** cover the two-step flow
  (`preparePayment` · `validatePayment` · `confirmPayment` · `cancelPayment`),
  correcting an earlier journal note that called one-step "the only flow". Scoped
  this pass to `preparePayment` only: it **reserves** (does not charge) —
  happy path `201 { paymentStatus: "reserved" }` with no `paymentDate`, persisted
  in the existing `store` so `retrievePayment` reads it back and later
  confirm/cancel can act on it. Reuses `createPayment`'s two control planes
  (identifier: reserved suffix → canonical error, malformed → 400, unidentifiable
  → 422 MISSING_IDENTIFIER; amount: `<0.001` → 400, `>1000` → 422
  UNAUTHORIZED_AMOUNT) via two new shared helpers (`build_amount_tx`,
  `check_amount`) refactored out of `create_payment` (no behaviour change there).
  `pending_validation`/`validationInfo` (OTP) + 409 ALREADY_EXISTS deferred to
  `validatePayment`; `sink`/`sinkCredential` accepted-not-applied (mirrors
  create). Static `/payments/prepare` route coexists with `/payments/:id` (matchit
  prioritises the static segment). **No new deps.** Spec: updated
  `specs/carrier-billing/v0.5/openapi.yaml` — new `POST /payments/prepare`
  operation (`x-camarasim-scenarios`, full 400/401/403/404/409/422/429/500/503
  set, `reserved` example), new `ReservePayment` + `PaymentReserved` schemas, new
  "Reserve" description section + refreshed header/cuts. 482 tests green (was 473;
  +9 v0_5 integration: reserve→201 reserved/no paymentDate · reserved suffix →
  error · amount>1000 → UNAUTHORIZED_AMOUNT · amount<0.001 → INVALID_ARGUMENT ·
  no-phone+non-E.164 subject → MISSING_IDENTIFIER · reserved payment retrieved
  verbatim · no create scope → 403 · no token → 401 · x-correlator echoed on
  201 + error). — binary: 1567808 B (+12632 B; the prepare handler + ~4 KB of
  vendored spec text embedded via include_str!)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — `GET /payments`
  (`retrievePayments`, scope `carrier-billing:payments:read`), the list op.
  Confirmed the canonical operation against the real CAMARA
  CarrierBillingCheckOut r3.2 spec (op `retrievePayments`, path `/payments`,
  scope `carrier-billing:payments:read`, response `PaymentArray` = array of
  `Payment`; query params `page`/`perPage`/`order`/`paymentCreationDate.gte|lte`/
  `paymentStatus`/`merchantIdentifier`; errors 400/401/403/429). Scoped this pass
  to the **core list**: new `store::all()` snapshots every stored payment, and
  the new `retrieve_payments` handler (a `.get()` on the existing `/payments`
  route) returns them as a `PaymentArray` (`200`, empty array when none; store
  state the only control plane, mirroring QoD `retrieve-sessions` / the
  Geofencing list). Query-param pagination/filtering accepted but **not applied**
  and payments not scoped per client — documented cuts, a later slice; no
  reserved-identifier plane (opaque ids). No new deps. Spec: updated
  `specs/carrier-billing/v0.5/openapi.yaml` — new `GET /payments` operation
  (`page`/`perPage`/`order` params documented as accepted-not-applied, 200
  `PaymentArray` with one-item + empty examples, shared 400/401/403/429), new
  `PaymentArray` schema, refreshed header/description/cuts. 473 tests green (was
  468; +5: 1 store unit [`all()` includes every inserted payment] + 4 v0_5
  integration [list contains a created payment verbatim as a JSON array · list
  without read scope → 403 · list without token → 401 · x-correlator echoed]). —
  binary: 1555176 B (+7672 B; the list handler + the new vendored spec text
  embedded via include_str!)

- 2026-08-03 — Phase 5 (payments): **Carrier Billing v0.5** — `GET
  /payments/{paymentId}` (`retrievePayment`, scope `carrier-billing:payments:read`),
  making the API **stateful**. Confirmed the canonical operation against the real
  CAMARA CarrierBillingCheckOut r3.2 spec (op `retrievePayment`, path
  `/payments/{paymentId}`, scope `carrier-billing:payments:read`, 200/400/401/403/
  404/429, response schema `Payment` = `PaymentCreated` + optional `sink`). New
  in-memory payment store `src/apis/carrier_billing/store.rs` (`Mutex<HashMap>`,
  lock never held across await, mirroring QoD's session store; `insert`/`get`, no
  new dep); `createPayment` now persists the charged payment. `retrievePayment`
  returns it verbatim (`200`) or `404 NOT_FOUND` for an unknown id — the opaque
  `paymentId` means the store state is the only control plane (no reserved-suffix
  plane, mirroring QoD `getSession`); `x-correlator` echoed on 200 + 404. The list
  op (`retrievePayments`) is the next slice. Spec: updated
  `specs/carrier-billing/v0.5/openapi.yaml` — new `GET /payments/{paymentId}`
  operation (`x-camarasim-scenarios`: found → 200 / unknown → 404, `$ref`ing the
  shared 400/401/403/404/429 responses), new `PaymentId` path param, new `Payment`
  schema (sink optional, always omitted here), refreshed description (read-back
  section + updated cuts). 468 tests green (was 462; +6: 1 store unit [read-back /
  unknown → None] + 5 v0_5 integration [created payment retrieved verbatim ·
  unknown id → 404 NOT_FOUND · read without read scope → 403 · no token → 401 ·
  x-correlator echoed on 200 + 404]). — binary: 1547504 B (+9312 B; the store
  module + the new vendored spec text embedded via include_str!)

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
