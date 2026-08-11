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
Billing); a non-http(s) scheme → 400 `INVALID_SINK`. A `sinkCredential` is applied
to the callback's `Authorization` header — an `ACCESSTOKEN` credential as
`Authorization: Bearer …` (RFC 6750), a `PLAIN` credential as
`Authorization: Basic base64(identifier:secret)` (RFC 7617) — derived at creation
and held in an `assignmentId`-keyed in-memory side-store (`store::insert_credential`/
`take_credential`, taken single-use at delivery) so the secret is never echoed;
REFRESHTOKEN is a documented cut. The **`AVAILABLE`-on-provisioning** event
and the **`NETWORK_TERMINATED`** transition are now in place too: creating an
`AVAILABLE`, sink-bearing assignment delivers a `status: AVAILABLE` `status-changed`
CloudEvent (no `statusInfo`) fire-and-forget; a `…001` identifier tail instead
schedules `v0_3::spawn_network_termination` (a short 1 s grace, mirroring QoD's
`NETWORK_TERMINATION_TAIL`) that evicts the assignment and delivers a
`status: UNAVAILABLE` / `statusInfo: NETWORK_TERMINATED` event (exactly-once vs a
concurrent revoke); a `REQUESTED` assignment is not yet active, so it notifies
nothing. The derived `sinkCredential` `Authorization` (ACCESSTOKEN → Bearer, PLAIN
→ Basic) is applied to every callback — the non-terminal AVAILABLE event *peeks* it
(new `store::peek_credential`, a non-destructive read) so a later terminal event
(revoke / network-drop) still `take_credential`s it single-use. Only TLS
(`https://`) delivery remains deferred.

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

**Phase 5 (other CAMARA APIs) — Media Streaming Rate vwip** is now live, a new
stateless, non-spatial, device-keyed network-quality query mounted at its
canonical `vwip` base path (CAMARA Device Media Streaming Rate, work-in-progress
— no released version yet, mirroring Device Data Volume / Device Visit Location).
`POST /retrieve-maximum-downstream-media-rate` (scope
`media-streaming-rate:retrieve-maximum-downstream-media-rate`, operationId
`retrieveMaximumDownstreamMediaRate`) answers the maximum **downstream** media
bit rate the network can currently sustain for a device —
`{ maxDownstreamMediaBitRateSupported: 0..1024, unit: bps|kbps|Mbps|Gbps|Tbps,
device? }`, a magnitude an application can use to size its streaming, never raw
per-flow telemetry. Device-object identifier resolution + the two-legged/
three-legged rule (mirrors Device Data Volume): a submitted `device` on a
three-legged **line** token (E.164 `sub`) → 422 `UNNECESSARY_IDENTIFIER`; no
`device` + a non-line subject → 422 `MISSING_IDENTIFIER`; an empty `device` → 400
`INVALID_ARGUMENT`. Two control planes (DESIGN §7): the identifier's reserved
error suffix → canonical CAMARA error (checked first); else its trailing three
digits `d` (`0..=999`; `…000`/no digits → 0) are the magnitude (always within the
schema's `0..=1024`) and `d % 5` picks the `unit` from the lowest-first enum
`[bps,kbps,Mbps,Gbps,Tbps]`, so both axes climb from the `…000` floor (`0 bps`) —
a genuine control plane. `device` echoed only for a phoneNumber request. No new
dependency. `x-correlator` echoed on every response.

**Device Authenticity vwip** is now live at `/device-authenticity/vwip/check-status`
(`device-authenticity:check-status`, operationId `checkImeiStatus`) — a stateless,
non-spatial, **IMEI-keyed** anti-fraud query. The caller submits an `imei` (15
digits, required — no two/three-legged fallback, the device is named explicitly)
and the operator answers its register status: `{ imei, operationalStatus,
lastChecked, reportedDate? }`. Two control planes (DESIGN §7): the IMEI's
reserved-error suffix → canonical CAMARA error (`…404` → NOT_FOUND over the API's
own `IDENTIFIER_NOT_FOUND`, `…422` → SERVICE_NOT_APPLICABLE); else its trailing
three digits `d` pick `operationalStatus` by `d % 9` over the nine-value enum in
order (`…000` → `allowed` default, `…001` → `lost`, `…002` → `stolen`, …, `…008`
→ `unknown`), and a non-`allowed` status carries a deterministic `reportedDate` of
`now − d hours`. `lastChecked` is a fresh RFC 3339 UTC timestamp (self-contained
formatter, no new dependency). `x-correlator` echoed on every response.

**Network Health Assessment vwip** is now live at
`/network-health-assessment/vwip/health-scores` (`getHealthScores`, scope
`network-health-assessment:health-scores:read`) — CamaraSim's first
**network-keyed** API and its first from the CAMARA NetworkInsights suite. It is a
two-legged (`client_credentials`) service query returning an *aggregate,
network-level* health score, never any device/user data. The caller names a
network (`networkId`, a UUID query param) and a module (`netType` query param:
`NET`/`NET_WIRELESS`/`NET_TRANSPORT`/`NET_CORE`) and gets `{ networkId, netType,
score, scoringTime }`. Two control planes (DESIGN §7): the `networkId`
reserved-error suffix → canonical CAMARA error (a UUID carries digits, so the
shared convention applies unchanged, e.g. `…-000000000404` → 404 NOT_FOUND); else
its trailing three digits `d` set the score (`d/10`) and `netType` shifts it per
module (`score = clamp(d/10 − moduleIndex, 0, 100)`; `NET`=0…`NET_CORE`=3), with
`…000`/no-digits → the spec's no-data `{ score: null, scoringTime: null }`.
`scoringTime` is a fresh RFC 3339 UTC timestamp (self-contained formatter, no new
dependency). `x-correlator` echoed on every response. The sibling Network Traffic
Analysis API of the suite is now live too (see below).

**Consent Info vwip** is now live at `/consent-info/vwip/retrieve`
(`retrieveStatus`, scope `consent-info:retrieve`) — a stateless, non-spatial,
phone-number-keyed consent-status query. The caller declares the `scopes` it
needs and the `purpose` (`dpv:<Purpose>`) it needs them for, identifies a line
(two-legged submitted `phoneNumber` / three-legged E.164 `sub`, with the usual
422 `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER` rule), and gets back whether
consent is valid for processing (`{ statusInfo: [{ scopes, purpose,
statusValidForProcessing, statusReason?, expirationDate? }], captureUrl? }`).
Three control planes (DESIGN §7): the identifier reserved-error suffix → canonical
CAMARA error; the identifier's trailing three digits pick the consent state
(`d % 6`: valid / PENDING / REQUESTED / DENIED / EXPIRED / OBJECTED), so a valid
consent carries a future `expirationDate` and an EXPIRED one a past date; and
`requestCaptureUrl` gates a deterministic top-level `captureUrl` (FNV-1a token,
no dep) offered only when consent is not valid. Two request-level 403 planes —
a `forbidden` scope → `NOT_ALLOWED_SCOPES_PURPOSE`, a malformed `callbackUrl` →
`INVALID_CALLBACK_URL`. `x-correlator` echoed on every response. This is
CamaraSim's first Identity-and-Consent-Management-adjacent consent API.

**Network Traffic Analysis vwip** is now live at
`/network-traffic-analysis/vwip/traffic-analysis` (`getTrafficAnalysis`, scope
`network-traffic-analysis:traffic-analysis:read`) — the traffic counterpart of
Network Health Assessment in the NetworkInsights suite, and stateless,
non-spatial, network-keyed like it. The caller names a network (`networkId`
UUID), a window (`startDate`/`endDate`, RFC 3339) and a granularity (`frequency`
= `DAY`/`HOUR`), and gets aggregated, per-application (DPI-detected) traffic
records — `{ app, accessCount, accessUpFlow, accessDownFlow, accessFlow,
startDate, endDate, accessDate, ipv4Address?, description? }` (`accessFlow` always
`up + down`) — plus a `pagination` envelope. Three control planes (DESIGN §7):
the `networkId` reserved-error suffix → canonical CAMARA error (UUIDs carry
digits, so `…404` → 404 "networkId not found"); its trailing three digits `d` set
the application count (`(d % 5) + 1`, from a fixed 5-entry DPI catalog) and scale
the traffic counters, with `…000`/no-digits → the spec's no-data `200` (empty
`records`); and the window × `frequency` set the number of time slots (whole
`DAY`/`HOUR` units in `[startDate, endDate)`, min 1, capped at 100 to bound the
response). Optional `app` narrows to one application (unknown → empty page; the
`app` field is still present per record); `page`/`perPage` window the result
(`perPage` ≤ 100). Validation mirrors the sibling: bad/missing params → 400
`INVALID_ARGUMENT`, `endDate <= startDate` / out-of-range paging → 400
`OUT_OF_RANGE`. Self-contained RFC 3339 parser + formatter (no new dependency).
`x-correlator` echoed on every response. **This begins the NetworkInsights
suite's second API and completes the suite's two published stateless surfaces.**

**Sponsored Data vwip** `POST /sponsorship` is now live at
`/sponsored-data/vwip/sponsorship` (`startSponsorship`, scope
`sponsored-data:sponsorship:create`): a sponsoring company (`sponsorId`) funds a
subscriber's (`phoneNumber`) mobile data within a campaign (`campaignId`), and
the operator returns `201` with a minted opaque `sessionId` and the granted
window. Three control planes (DESIGN §7): the `phoneNumber` reserved-error suffix
→ canonical CAMARA error (`…422` → not eligible, `…409` → duplicate session);
`dataVolume` (1–1000 MB, default 50) echoed as `sponsoredDataVolume`; and
`duration` (1–1440 min, default 10) sets `endTime = startTime + duration` — both
out-of-range → 400 `OUT_OF_RANGE`. Required-field patterns (`sponsorId`
`local@domain`, `campaignId` `UUID@domain`, E.164 `phoneNumber`, v4-UUID
`callbackToken`) → 400 `INVALID_ARGUMENT`. Self-contained validators + RFC 3339
formatter + UUID-v4 `sessionId` minter (no new dependency). Persistence (for
`session-status`/`revoke`), the `webhookUrl` callback, and campaign management
are deferred to later passes; the scope is CamaraSim-assigned (the wip contract
declares no securitySchemes). `x-correlator` echoed on every response.

**Traffic Influence vwip** has begun (the EdgeCloud traffic-steering API — the
last released-spec CAMARA API not yet mounted; the simpler stateless/non-spatial
space is exhausted). `POST /traffic-influences` is live at
`/traffic-influence/vwip/traffic-influences` (scope
`traffic-influence:traffic-influences:write`, `postTrafficInfluence`): an API
consumer (`apiConsumerId`) names an application (`appId`) to steer toward an
edge-cloud placement (`appInstanceId`/`edgeCloudRegion`/`edgeCloudZoneId`,
optional source/destination traffic filters), and the operator answers `201`
with a minted `trafficInfluenceID`, the placement echoed, a lifecycle `state`,
and a `Location` header. Creating a readable resource makes it stateful, so a
new in-memory store (`src/apis/traffic_influence/store.rs`; `Mutex<HashMap>`, no
new dep) persists the rendered resource for a future `getTrafficInfluenceById`.
Two control planes (DESIGN §7) keyed on `appId` (a hex UUID, so its trailing
digits are caller-controlled): a reserved error suffix → canonical CAMARA error;
else the trailing three digits `d` select the state — `d%3==0`→`ordered`,
`==1`→`created`, `==2`→`active`. Field validation → 400 `INVALID_ARGUMENT`
(missing/malformed ids or non-JSON body) / `OUT_OF_RANGE` (a port outside
`0..=65535`). The read/update/delete ops, the per-device create, and the
`subscriptionRequest` CloudEvents notifications are deferred. `x-correlator`
echoed on every response.

**Application Endpoint Discovery vwip** is now live at
`/application-endpoint-discovery/vwip/retrieve-optimal-app-endpoints`
(`getOptimalAppEndpoints`, scope
`application-endpoint-discovery:app-endpoints:read`) — a new stateless,
non-spatial, device-keyed EdgeCloud API (found by re-checking the CAMARA org: the
`ApplicationEndpointDiscovery` repo was not yet mounted). Where Simple / Optimal
Edge Discovery answer with an edge cloud **zone**, this returns the concrete
application **endpoints** (`port` + a single `fqdn`/`ipv4Addresses`/`ipv6Addresses`,
with an `edgeCloudZone`) of the instance(s) closest to the device. The caller
names the application with exactly one of `appId` / `applicationEndpointsId` (both
UUID; neither/both/non-UUID → 400 INVALID_ARGUMENT — a documented tightening of
the upstream `anyOf`), and identifies the device via the same two-legged /
three-legged rule as Optimal Edge Discovery (422 `UNNECESSARY_IDENTIFIER` /
`MISSING_IDENTIFIER`). Two control planes (DESIGN §7): the application
identifier's trailing three digits `d` (`…000` → 404 NOT_FOUND — not registered;
else `(d % 3) + 1` endpoints, address family rotating by rank so a 3-endpoint
answer shows all three), and the resolved device identifier's reserved-error
suffix → canonical CAMARA error (checked first, so the device-not-found 404 is
reachable distinctly from the app-not-found 404). The `DeviceResponse`
(phoneNumber only) is echoed only when the request `device` carried multiple
identifiers, faithful to the CAMARA rule. Deterministic endpoints/zone-ids via
SHA-256 (no new dep). This completes Application Endpoint Discovery vwip's single
operation.

**Predictive Connectivity Data vwip** is now live at
`/predictive-connectivity-data/vwip/retrieve` (`retrieveConnectivity`, scope
`predictive-connectivity-data:read`) — a new **area-keyed** (no device
identifier) CAMARA API forecasting the connectivity an operator expects to
sustain across an area over a time window, per grid cell as a stack of vertical
**layers** (altitude bands), each rated `GC`/`MC`/`NC`/`ND`. It mirrors
Population Density Data's synchronous `GEOHASHLIST` path: the geometry is the
control plane (DESIGN §7) — the first geohash's reserved suffix → canonical
CAMARA error; each geohash's stable hash fixes its cell (`h % 7 == 0` → NO_DATA;
else ground score `h % 100` degrading 15 pts/layer up); the target `serviceLevel`
(`C2`/`STREAM_4K`/`BEST_EFFORT`) sets how strict the per-layer rating is (a
genuine 2nd plane); `height` sets the layer count (`height/30 + 1`, default 4);
`includeSignalStrength` toggles the per-layer dBm band. Capability/structural
planes: `POLYGON` → 422 `…UNSUPPORTED_AREA_TYPE`; geohash length > 9 → 422
`…UNSUPPORTED_PRECISION`; > 100 geohashes → 422 `…UNSUPPORTED_SYNC_RESPONSE`;
window checks `…INVALID_END_TIME` / `…MAX_TIME_PERIOD_EXCEEDED` (self-contained
RFC 3339 parser). Documented cuts: `POLYGON`, async `sink`/CloudEvents, hourly
time-slicing (single slice), absolute start-time checks, and
`UNSUPPORTED_SERVICE_LEVEL` (all three service levels supported). No new dep.

**Network Access Devices vwip** has begun (`/network-access-devices/vwip`; CAMARA
NetworkAccessManagement, wip). `GET /network-access-devices`
(`network-access-devices:reboot`, `getNetworkAccessDevices`) lists the
operator-supplied access equipment (gateways/routers/access points — operator
infrastructure, not end-user devices) associated with the subscriber. No request
body, so — like Number Verification's `GET /device-phone-number` — the token
**subject** is the sole control plane (DESIGN §7): reserved error suffix →
canonical CAMARA error; else the subject's trailing three digits `d` (or `0` when
none) drive both the device **count** (`d == 0` → 1, else `((d-1) % 3) + 1`, i.e.
1–3) and each device's **status** (device `i` → `[connected, disconnected,
unavailable][(d + i) % 3]`), so `…000` is one connected gateway, `…002` two
devices `[unavailable, connected]`, `…003` three. Each device's UUID-shaped `id`
and EUI-48 `hardwareAddress` are deterministic from the subject + index (SHA-256,
no new dep). `serviceSite` + the inherited Commonalities `Device` end-user
identifier fields are omitted for operator devices (schema-valid cut — only `id`
is required); the reboot-request resource lifecycle is a stateful later slice.
`x-correlator` echoed.

The Network Access Devices reboot-request lifecycle now has its **read** leg
alongside create: `GET /network-access-devices/vwip/reboot-requests/{rebootRequestId}`
(`getRebootRequest`, `network-access-devices:reboot`) returns the `RebootRequest`
persisted by `createRebootRequest`, verbatim. The id is opaque and server-minted,
so store state is the sole control plane (DESIGN §7 — no reserved-identifier
plane): a stored id → `200`, any other (never created / already deleted) → `404
NOT_FOUND`. Reboot requests aren't scoped per subscriber, so the `sub`-ownership
check isn't enforced (documented cut). `x-correlator` echoed; no new dep.

The reboot-request lifecycle now also has its **delete** leg:
`DELETE /network-access-devices/vwip/reboot-requests/{rebootRequestId}`
(`deleteRebootRequest`, `network-access-devices:reboot`) evicts the stored
`RebootRequest` from the shared store (`store::remove`, single-use): a stored id →
`204 No Content`, any other (never created / already deleted) → `404 NOT_FOUND`.
Store state is the sole control plane (opaque server-minted id — no
reserved-identifier plane, mirroring QoD `deleteSession`); `sub`-ownership not
enforced (documented cut, mirroring the read). `x-correlator` echoed on `204` and
`404`; no new dep. Only `PATCH /reboot-requests…` (update leg) remains.

**Short Message Service v0alpha1** is now live: `POST /sms/v0alpha1/short-message`
(scope `send-sms:short-message`, operationId `send-sms`) — the first send-SMS
API. Two-legged / business-facing (like Verified Caller / Click to Dial): the
`from` sender and the `to` recipients are both in the body, so no
two-/three-legged dance. Accepts `to` (≥1 E.164 MSISDN), `from` (E.164),
`message` (non-empty), and an optional `category`
(`PROMOTION`/`SERVICE`/`TRANSACTION`). Control plane (DESIGN §7): the **first
recipient** `to[0]`'s reserved error suffix → canonical CAMARA error (`…404` =
recipient not found, `…503`/`…500` the network-unavailable/internal cases the
upstream API declares); otherwise `200 { msgId, timestamp }` with a
deterministic, UUID-shaped `msgId` (SHA-256 of `from` + all `to` + `message`,
no uuid/rand dep) and a fresh RFC 3339 UTC `timestamp`. A reserved suffix on
`from` or on a non-first recipient is **not** the plane. Validation: empty `to`,
non-E.164 `from`/recipient, empty `message`, unknown `category`/field, bad body →
400 INVALID_ARGUMENT. `x-correlator` echoed. The upstream API's
delivery-notification subscription surface is out of scope. (Adding this entry
grew the `/` catalog `json!` literal past the default macro recursion limit, so a
crate-level `#![recursion_limit = "256"]` was added to `src/main.rs`.)

**Capabilities and Restrictions vwip** is now live at
`/capabilities-and-restrictions/vwip/retrieve` (`postServiceCapability`, scope
`camara-capability:read`) — a new stateless, non-spatial consumer-context
capability-discovery API (CAMARA CapabilitiesAndRuntimeRestrictions `wip`). A
consumer submits `queries` (each with a required `overlayExtends` list and
optional `resourceScopes`); the operator answers `201 CapabilityInfo` with one
`CapabilityDetail` per query — a fixed 3-entry `bitmapCapabilities` catalogue of
overlay `SchemaRestrictionsSet`s plus a context-derived `camaraCapabilitiesBitmap`
whose bits mark the active restrictions. Two control planes (DESIGN §7): the
first query's first `resourceScopes.phoneNumber` reserved-error suffix →
canonical CAMARA error (`…404` = no capability API found); and the active bitmap
derives from that phoneNumber's trailing three digits (else an FNV hash of
`overlayExtends`) `% 8`, so the active set is a genuine second plane. The
`subscriptionRequest` change-notification callback, the `CapabilitySetFootprint`
branch, ETag/`If-None-Match`/304 caching, and overlay resolution are documented
cuts.

**Dedicated Network — Networks vwip** now has its read leg too:
`GET /dedicated-network/vwip/networks/{networkId}` (`readNetwork`, scope
`dedicated-network:networks:read`) reads the stored `NetworkInfo` back (`200`)
or `404 NOT_FOUND` for an unknown id. Like QoD `getSession`, the `networkId` is
a server-minted opaque UUID, so the only control plane is the in-memory store
state (no reserved-suffix plane on a minted id). `x-correlator` echoed. The
list/delete legs and the sibling Dedicated-Networks APIs remain later passes.

The Networks CRUD is now complete, and the first **sibling** Dedicated-Networks
API has begun: **Dedicated Network — Accesses vwip** is mounted at
`/dedicated-network-accesses/vwip` with its create leg `POST /accesses`
(`createAccess`, scope `dedicated-network-accesses:accesses:create`). An access
binds a set of devices (CAMARA `Device`s) to a dedicated `networkId`; `createAccess`
mints an opaque UUID `id`, resolves each device to `GRANTED`/`DENIED` from its own
identifier (a reserved-suffix identifier → `DENIED`), records the rendered
`AccessInfo` (with aggregate `stats` + `recentAccessDevices`) in a new in-memory
store, and returns `201`. The `networkId` reserved suffix is the top-level error
plane (`…404` → no such network); request validation → 400. `sinkCredential` is
accepted but never echoed. This picks the non-spatial sibling (`-accesses`) ahead
of the spatial `-areas`, honouring the phase order. The read leg
`GET /accesses/{accessId}` (`readAccess`,
`dedicated-network-accesses:accesses:read`) is now live too — keyed only on the
in-memory store state (opaque minted UUID, no reserved-suffix plane) it reads the
stored `AccessInfo` back (`200`) or `404 NOT_FOUND` for an unknown/malformed id,
mirroring the sibling `readNetwork`. The `207` multi-status form, the list/delete
legs and the `/accesses/{accessId}/devices…` sub-resources remain documented cuts
for later passes.

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
    - [x] `sinkCredential` auth — ACCESSTOKEN → `Authorization: Bearer` and PLAIN
      → `Authorization: Basic base64(identifier:secret)` (RFC 7617) on the callbacks
      (in-memory credential; REFRESHTOKEN deferred — needs a token-exchange round trip)
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
    - [x] `sinkCredential` auth on the callbacks (ACCESSTOKEN → `Authorization:
      Bearer`; PLAIN → `Authorization: Basic base64(identifier:secret)`, RFC 7617,
      on every callback; REFRESHTOKEN deferred)
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
    `carrier-billing:payments:read`. Store `all()` scan → a `PaymentArray`
    (`200`, empty array when none). Now a second control plane: the spec's
    `page`/`perPage`/`order` query params are **applied** — payments sorted by
    `paymentCreationDate` (`paymentId` tiebreaker) in `order` (asc/desc, default
    desc), then the `page`-th window of `perPage` (defaults 1/10). Validation:
    non-integer `page`/`perPage` → 400 INVALID_ARGUMENT, `<1` → 400 OUT_OF_RANGE,
    unknown `order` → 400 INVALID_ARGUMENT; unknown query params ignored. No new
    dep (`RawQuery` + `serde_urlencoded`). Per-client scoping and
    `paymentCreationDate`/`paymentStatus`/`merchantIdentifier` filters (not in
    the 0.5.0 spec) remain out of scope.
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
      dep; ACCESSTOKEN `sinkCredential` bearer + PLAIN Basic applied; REFRESHTOKEN cut).
    - [x] `preparePayment` → `payment-reserved` CloudEvent on a successful
      `reserved` reservation (http sink, fire-and-forget over raw TCP, no
      HTTP-client dep; ACCESSTOKEN `sinkCredential` bearer + PLAIN Basic applied,
      REFRESHTOKEN cut; a `…888` pending_validation reservation fires no
      payment-reserved — that is the separate payment-pending-validation event)
    - [x] `payment-pending-validation` on `preparePayment` — a `…888`
      reservation → `pending_validation` delivers a `payment-pending-validation`
      CloudEvent to the request `sink` (fire-and-forget over raw TCP, no
      HTTP-client dep; ACCESSTOKEN `sinkCredential` bearer + PLAIN Basic applied,
      REFRESHTOKEN cut). Mutually exclusive with `payment-reserved`; per the
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
    - [x] `sinkCredential` **PLAIN** (RFC 7617 Basic) now applied on every
      carrier-billing callback (`sink_authorization` extended from ACCESSTOKEN-only
      to also `PLAIN` → `Authorization: Basic base64(identifier:secret)`), so the
      credential recorded at create/prepare authenticates all one-step and terminal
      events; REFRESHTOKEN still cut (needs a token-exchange round trip).
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
      scheme → 400 `INVALID_SINK`. `sinkCredential` applied to every callback's
      `Authorization` header — ACCESSTOKEN → `Bearer <token>` (RFC 6750), PLAIN →
      `Basic base64(identifier:secret)` (RFC 7617) — via the single-use side-store
      `store::insert_credential`/`take_credential`; REFRESHTOKEN a documented cut; the
      secret is never echoed.
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
- [~] QoS Booking vwip (`/qos-booking/vwip`; CAMARA qos-booking, wip — part of the
  ConnectivityQualityManagement subproject; the *time-boxed booking* sibling of
  Quality on Demand (immediate sessions) and QoS Provisioning (open-ended); stateful,
  resource-oriented, in-memory booking store):
  - [x] `POST /device-qos-bookings` (`qos-booking:device-qos-bookings:create`,
    `createBooking`) — books a `qosProfile` for a device over a `startTime` /
    `duration` window in a `serviceArea`, mints an opaque UUID-shaped `bookingId`
    (`src/apis/qos_booking/store.rs`; `Mutex<HashMap>`, no uuid/rand dep, mirroring
    QoS Provisioning), persists the rendered `BookingInfo`, `201`. Device-object
    identifier resolution + the two-legged/three-legged rule (device on a line token
    → 422 `UNNECESSARY_IDENTIFIER`; no device + non-line subject → 422
    `MISSING_IDENTIFIER`; empty `device` → 400 INVALID_ARGUMENT). Control planes
    (DESIGN §7): identifier reserved-error suffix → canonical CAMARA error (`…409` →
    409 CONFLICT); else the trailing three digits fix `bookingStatus` — `…000`/no
    digits → `REQUESTED` (no `startedAt`), odd tail → `SCHEDULED` (no `startedAt`),
    other tail → `ACTIVATED` (`startedAt`=now); `duration` (`<1` → 400 OUT_OF_RANGE,
    `>31622400` → 400 `QOS_BOOKING.DURATION_OUT_OF_RANGE`); `serviceArea` — CamaraSim
    manages `CIRCLE` (center out of range → 400 OUT_OF_RANGE, radius `<1` → 422
    `QOS_BOOKING.INVALID_AREA`) + `AREANAME` (`uncovered` → 422
    `QOS_BOOKING.AREA_NOT_COVERED`), `POLYGON` → 422 `QOS_BOOKING.NOT_MANAGED_AREA_TYPE`
    (documented cut), unknown/missing areaType → 400 INVALID_ARGUMENT; `qosProfile`
    name containing `unavailable` → 422 `QOS_BOOKING.QOS_PROFILE_NOT_APPLICABLE`;
    `sink` must be http(s) → else 400 `INVALID_SINK`. `startTime` validated for shape
    (RFC 3339) but not used to compute status (documented cut). `x-correlator` echoed.
  - [x] `GET /device-qos-bookings/{bookingId}` (`getBooking`,
    `qos-booking:device-qos-bookings:read`) — reads a created booking back from the
    in-memory store by its opaque, server-minted `bookingId` → `200` `BookingInfo`
    verbatim / `404 NOT_FOUND`. Store state the only control plane (opaque id → no
    reserved-identifier plane; mirrors QoS Provisioning `getQosAssignmentById` / QoD
    `getSession`). `x-correlator` echoed. No new dep.
  - [x] `DELETE /device-qos-bookings/{bookingId}` (`deleteBooking`,
    `qos-booking:device-qos-bookings:delete`) — deletes a stored booking, evicting
    it from the in-memory store (`store::remove`): present → `204 No Content`
    (single-use), unknown/already-deleted → `404 NOT_FOUND`. Keyed only on store
    state (opaque `bookingId`, no reserved-identifier plane). CAMARA's async `202
    Accepted` (returning `BookingInfo`) form is deferred with `sink` notifications —
    synchronous `204` only (mirrors QoS Provisioning `revokeQosAssignment` / QoD
    `deleteSession`). `x-correlator` echoed. No new dep.
  - [x] `POST /retrieve-device-qos-bookings` (`retrieveBookingByDevice`,
    `qos-booking:device-qos-bookings:retrieve-by-device`) — the canonical
    device-keyed collection query: lists a device's bookings as an array of
    `BookingInfo` (`200`, empty array when none — CAMARA never 404s on an empty
    result). Device is the submitted `device` id, else the token subject
    (two-legged/three-legged rule: `device` on a line token → 422
    `UNNECESSARY_IDENTIFIER`; no device + non-line subject → 422
    `MISSING_IDENTIFIER`). Two control planes (DESIGN §7): identifier
    reserved-error suffix → canonical CAMARA error; else the in-memory store,
    scanned by each booking's echoed `device` (new `store::find_by_device`,
    mirroring QoD's `retrieveSessionsByDevice`). Not scoped per client
    (documented cut). `x-correlator` echoed. No new dep.
  - [~] CloudEvents notifications on `sink` (status transitions)
    (`src/apis/qos_booking/notifications.rs`; event type
    `org.camaraproject.qos-booking.v0.status-changed`, mirroring QoS Provisioning):
    - [x] `DELETE_REQUESTED` `status-changed` on `deleteBooking` — a deleted
      booking that recorded a `sink` receives a `bookingStatus: TERMINATED` /
      `statusInfo: DELETE_REQUESTED` CloudEvent, fire-and-forget over raw TCP (no
      HTTP-client dep), still `204`. `sinkCredential` applied to the callback's
      `Authorization` header — ACCESSTOKEN → `Bearer <token>` (RFC 6750), PLAIN →
      `Basic base64(identifier:secret)` (RFC 7617) — via the single-use side-store
      `store::insert_credential`/`take_credential`; REFRESHTOKEN a documented cut;
      the secret is never echoed. `https://` sink not delivered to (no TLS client).
    - [x] `NETWORK_TERMINATED` `status-changed` — a `…002` (even, non-zero →
      `ACTIVATED`) booking that recorded a `sink` is dropped early by the simulated
      network: a short-grace (`1 s`) fire-and-forget async timer evicts it (a later
      `GET` is `404`) and delivers a `bookingStatus: TERMINATED` / `statusInfo:
      NETWORK_TERMINATED` CloudEvent (raw TCP, no HTTP-client dep), with the
      single-use `sinkCredential` applied. Mirrors QoD's `…001` case.
    - [x] window-expiry `DURATION_EXPIRED` — any other `ACTIVATED`, sink-bearing
      booking (not the `…002` network-drop tail) runs its window to completion: an
      async timer at creation waits the booking's `duration` (from `startedAt`), then
      evicts it (a later `GET` is `404`) and delivers `bookingStatus: TERMINATED` /
      `statusInfo: DURATION_EXPIRED` (raw TCP, no HTTP-client dep), single-use
      `sinkCredential` applied. Mutually exclusive with `NETWORK_TERMINATED` (by
      tail), so exactly one terminal event fires. Mirrors QoD's `DURATION_EXPIRED`.
    - [x] `SCHEDULED`→`ACTIVATED`-at-window-start transition — a `SCHEDULED`
      (odd-tail), sink-bearing booking arms an async timer (`v0_4::spawn_activation`,
      off the request path) that sleeps until its window start (`startTime`,
      already-past → fires at once), flips the stored booking `SCHEDULED`→`ACTIVATED`
      in place (new atomic `store::activate`, stamping `startedAt`), and delivers a
      **non-terminal** `status-changed` CloudEvent (`bookingStatus: ACTIVATED`, no
      `statusInfo`); the credential is *peeked* (`store::peek_credential`), so a later
      terminal event still authenticates, and the now-`ACTIVATED` booking chains
      `spawn_window_expiry` → `DURATION_EXPIRED`. A concurrent `deleteBooking` makes
      activation a no-op (`store::activate` → `None`; exactly-once). Unlike the
      initial status, this transition keys off `startTime` (self-contained RFC 3339
      parser `unix_secs_from_rfc3339` + `days_from_civil`, no new dep). A no-`sink`
      SCHEDULED booking stays SCHEDULED (nothing to notify).
    - [ ] TLS (`https://` sink) delivery (needs a rustls TLS client)
- [x] Device Data Volume vwip (`/device-data-volume/vwip`; CAMARA
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
  - [x] `POST /check` (`device-data-volume:read`, `checkDataVolume`) —
    `{ thresholdExceeded }` for a caller-supplied `volumeToCheck`
    (`{value:0..1024, unit:MiB|GiB}`). Same identifier resolution + two-legged/
    three-legged rule and reserved-error convention as `retrieve`. Two control
    planes (DESIGN §7): identifier reserved-error suffix (checked first, after the
    `value` range); else the identifier's trailing three digits `d` fix the
    device's **remaining** volume at `d*10` MiB (`…000`/no digits → 0 MiB) and
    `thresholdExceeded = remaining_MiB > volumeToCheck` (GiB → value*1024), so
    `volumeToCheck` is a genuine second plane. `volumeToCheck` required (missing/
    unknown `unit` → 400 INVALID_ARGUMENT; `value` ∉ 0..=1024 → 400 OUT_OF_RANGE).
    `device` echoed only for a phoneNumber request. No new dep.
    **Completes Device Data Volume vwip.**
- [x] Media Streaming Rate vwip (`/media-streaming-rate/vwip`; CAMARA Device
  Media Streaming Rate, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateless, non-spatial, device-keyed network-quality query):
  - [x] `POST /retrieve-maximum-downstream-media-rate`
    (`media-streaming-rate:retrieve-maximum-downstream-media-rate`,
    `retrieveMaximumDownstreamMediaRate`) —
    `{ maxDownstreamMediaBitRateSupported: 0..1024, unit:
    bps|kbps|Mbps|Gbps|Tbps, device? }`, the max downstream media bit rate the
    network can sustain (never raw telemetry). Device-object identifier
    resolution + the two-legged/three-legged rule (mirrors Device Data Volume):
    `device` on a line token → 422 `UNNECESSARY_IDENTIFIER`; no `device` +
    non-line subject → 422 `MISSING_IDENTIFIER`; empty `device` → 400
    `INVALID_ARGUMENT`. Two control planes (DESIGN §7): identifier reserved-error
    suffix → canonical CAMARA error (checked first); else the trailing three
    digits `d` (`0..=999`; `…000`/no digits → 0) are the magnitude and `d % 5`
    picks the `unit` from `[bps,kbps,Mbps,Gbps,Tbps]`, so both axes climb from the
    `…000` floor (`0 bps`) — a genuine plane. `device` echoed only for a
    phoneNumber request. No new dep. **Completes Media Streaming Rate vwip.**
- [x] Optimal Edge Discovery vwip (`/optimal-edge-discovery/vwip`; CAMARA
  optimal-edge-discovery, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateless, device-keyed edge/MEC discovery — the *ranked*
  successor to Simple Edge Discovery):
  - [x] `POST /retrieve-optimal-edge-cloud-zones` (`discoverOptimalEdge`, scope
    `optimal-edge-discovery:edge-zones:read`) — returns a ranked list (1–20) of
    `EdgeCloudZone`s optimal for the device, never coordinates. Device-object
    identifier resolution + the two-legged/three-legged rule (mirrors Simple Edge
    Discovery: device on a line token → 422 UNNECESSARY_IDENTIFIER; no device +
    non-line subject → 422 MISSING_IDENTIFIER; empty device → 400). Control planes
    (DESIGN §7): required `applicationProfileId` (UUID, else 400 INVALID_ARGUMENT);
    identifier reserved-error suffix → canonical CAMARA error; the identifier's
    trailing three digits pick the optimal zone (start index `% 6`) and the count
    (`(d % 3) + 1`, 1–3 zones); and the optional `edgeCloudRegion` filters the
    candidate zones (unknown region → 404 NOT_FOUND) — a genuine second plane.
  - [x] `GET /regions` (`getRegions`, scope `optimal-edge-discovery:regions:read`)
    — the read-only helper listing the edge cloud **regions** where zones are
    available. No request body / identifier, so no parameter-driven functional
    cases (only the auth error set): any authorised call returns the **distinct**
    `edgeCloudRegion` values of the fixed 6-entry zone table, in table order
    (≤20 per the schema). `x-correlator` echoed. **Completes Optimal Edge
    Discovery vwip.**
- [x] Verified Caller vwip (`/verified-caller/vwip`; CAMARA Verified Caller, wip
  — no released version yet, mounted at its canonical `vwip` base path;
  stateless, non-spatial, two-legged / business-facing anti-scam caller-trust
  API):
  - [x] `POST /pre-announce` (`verified-caller:create`, `createPreAnnouncement`)
    — pre-announce an outbound call so the network can verify the calling party
    to the called party (SMS or branded display) before it rings. Two-legged
    only: both `callingParticipant` and `calledParticipant` are required in the
    body (no line subject → no two-legged/three-legged dance). Control planes
    (DESIGN §7): the `calledParticipant` is the identifier — a reserved error
    suffix → canonical CAMARA error (`…404` = participant not found); else
    `strategy` selects the response shape (`BRAND_DISPLAY` → `201
    {preAnnouncementId, expiresAt}`, `SMS`/omitted default → `204 No Content`);
    and `timeToLive` (default 120 s) sets `expiresAt` = now + ttl on the `201`
    path. `preAnnouncementId` is a deterministic UUID-shaped SHA-256 token (no
    uuid/rand dep). Validation: bad body / non-E.164 participant / unknown
    `strategy` / over-length `registrationId`/`dynamicDisplayName`/`callReason`
    → 400 INVALID_ARGUMENT; `timeToLive` ∉ 1..=86400 → 400 OUT_OF_RANGE.
    Stateless (no read-back → no store). `x-correlator` echoed on every response
    incl. the `204`. **Completes Verified Caller vwip.**
- [x] Application Profiles vwip (`/application-profiles/vwip`; CAMARA
  ApplicationProfiles, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateful, resource-oriented; the *quality-requirements*
  registry the sibling Connectivity Insights API references by
  `applicationProfileId`):
  - [x] `POST /application-profiles` (`createApplicationProfile`, scope
    `application-profiles:create`) + `GET /application-profiles/{applicationProfileId}`
    (`readApplicationProfile`, scope `application-profiles:read`). In-memory
    profile store (`src/apis/application_profiles/store.rs`; `Mutex<HashMap>`,
    opaque UUID id, no uuid/rand dep, mirroring QoS Provisioning). No identifier /
    no `device` — the id is server-minted, so no reserved-identifier plane and no
    two/three-legged rule. Create is driven entirely by the request body (DESIGN
    §7): `anyOf` (≥1 of `networkQualityThresholds`/`computeResources`, else 400
    INVALID_ARGUMENT); each supplied object's `minProperties: 1` (else 400
    INVALID_ARGUMENT); numeric ranges → 400 OUT_OF_RANGE (Duration value ≥ 1;
    Rate/Compute value 0..=1024; packetLossErrorRate 1..=10); unknown unit/
    gpuVendorType enum / unknown field / wrong type → 400 INVALID_ARGUMENT at
    parse. Created profile echoes the validated thresholds + minted
    `applicationProfileId`; read → 200 (store hit) / 404 NOT_FOUND (well-formed
    unknown) / 400 INVALID_ARGUMENT (non-UUID path). `targetMinCPU`/`targetMinGPU`/
    `gpuVendorType`/`gpuModelName` accepted with schema-level validation only
    (no numeric range in the spec) — documented cut. `x-correlator` echoed. No new
    dep (reuses `sha2`).
  - [x] `PATCH /application-profiles/{applicationProfileId}` (`updateApplicationProfile`,
    `application-profiles:update`) — full-set **replacement** of a stored
    profile's thresholds (CAMARA op: "update the complete set … with the new set
    of thresholds"), keeping the same `applicationProfileId` → `200`
    `ApplicationProfile`. Body is an `ApplicationProfileRequest` validated by the
    same shared `parse_and_validate` as create (`anyOf` / `minProperties` /
    numeric ranges / strict parse → 400 INVALID_ARGUMENT / OUT_OF_RANGE), checked
    **before** the store (a body 400 wins over the unknown-id 404). Path plane
    mirrors read/delete: non-UUID → 400 INVALID_ARGUMENT, well-formed unknown →
    404 NOT_FOUND. Atomic check-and-swap `store::replace`; `x-correlator` echoed.
    No new dep. **Completes Application Profiles vwip.**
  - [x] `DELETE /application-profiles/{applicationProfileId}` (`deleteApplicationProfile`,
    `application-profiles:delete`) — evicts a stored profile from the in-memory
    store (`store::remove`): stored id → `204 No Content` (single-use); well-formed
    unknown/already-deleted id → `404 NOT_FOUND`; non-UUID path → `400
    INVALID_ARGUMENT` (mirrors `readApplicationProfile`). Store state the only
    control plane (opaque server-minted id → no reserved-identifier plane).
    `x-correlator` echoed.
- [x] Subscription Status vwip (`/subscription-status/vwip`; CAMARA
  SubscriptionStatus, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateless, non-spatial, phone-number-keyed line-status query):
  - [x] `POST /retrieve-subscription-status` (`subscription-status:retrieve-subscription-status`,
    `retrieveSubscriptionStatus`) — the live service status of a line
    (`{ voiceSmsIn: active|suspended, voiceSmsOut: active|suspended,
    dataService: active|suspended|throttled }`). Same two-legged (submitted
    `phoneNumber`) / three-legged (E.164 `sub`) identifier rule as Number
    Recycling with 422 `UNNECESSARY_IDENTIFIER` / `MISSING_IDENTIFIER`; empty
    body accepted (three-legged). Two control planes (DESIGN §7): identifier
    reserved-error suffix → canonical CAMARA error (`…404` → NOT_FOUND, `…422`
    → SERVICE_NOT_APPLICABLE — the API's own service-level 422); else the
    identifier's trailing three digits `d` are a status bitfield — bit 0 →
    `voiceSmsIn` suspended, bit 1 → `voiceSmsOut` suspended, `(d>>2)%3` →
    `dataService` active/suspended/throttled — so `…000`/no-digits is the
    all-active default and each field is independently controllable. No new dep.
    **Completes Subscription Status vwip.**
- [x] Device Authenticity vwip (`/device-authenticity/vwip`; CAMARA
  DeviceAuthenticity, wip — no released version yet, mounted at its canonical
  `vwip` base path; stateless, non-spatial, **IMEI-keyed** anti-fraud query):
  - [x] `POST /check-status` (`device-authenticity:check-status`,
    `checkImeiStatus`) — `{ imei, operationalStatus, lastChecked, reportedDate? }`,
    the register / operational status of a device by its IMEI. The IMEI is the
    required identifier (no two-legged / three-legged fallback; missing or
    non-15-digit → 400 INVALID_ARGUMENT). Two control planes (DESIGN §7):
    identifier reserved-error suffix → canonical CAMARA error (`…404` → NOT_FOUND
    over the API's own IDENTIFIER_NOT_FOUND, `…422` → SERVICE_NOT_APPLICABLE);
    else the IMEI's trailing three digits `d` pick the `operationalStatus` by
    `d % 9` over the nine-value enum in order (`…000` → `allowed` default, `…001`
    → `lost`, `…002` → `stolen`, …, `…008` → `unknown`); a non-`allowed` status
    carries a deterministic `reportedDate` of `now − d hours`. `lastChecked` = now
    (self-contained RFC 3339 formatter, no new dep). `x-correlator` echoed.
    **Completes Device Authenticity vwip.**
- [~] Session Insights vwip (`/session-insights/vwip`; CAMARA SessionInsights,
  wip — no released version yet, mounted at its canonical `vwip` base path;
  **stateful, resource-oriented**, non-spatial application-session resource):
  - [x] `POST /sessions` (`session-insights:sessions:create`, `createSession`)
    + `GET /sessions/{sessionId}` (`session-insights:sessions:read`, `getSession`)
    — the create/read pair. `POST` mints an opaque UUID-shaped `id`, stores the
    rendered `SessionInfo`, returns 201; `GET` reads it back (200) or 404
    NOT_FOUND. Identifier = submitted `device` id else token subject (two/three
    -legged, 422 MISSING_IDENTIFIER when neither). Control planes (DESIGN §7):
    identifier reserved-error suffix → canonical CAMARA error (`…409` → the
    createSession 409 CONFLICT); else `status:ACTIVE`, `startsAt:now`, and
    `expiresAt` present (now+24h) unless the tail is `…000`/no-digits (open-ended).
  - [x] `DELETE /sessions/{sessionId}` (`deleteSession`,
    `session-insights:sessions:delete`) — evicts the session from the in-memory
    store → `204 No Content` (single-use); an unknown/already-deleted id → `404
    NOT_FOUND` (new `store::remove`). Keyed only on store state; no `session-ended`
    CloudEvent emitted (notifications still deferred). `x-correlator` echoed.
  - [x] `POST /retrieve-sessions` (`retrieveSessionsByDevice`,
    `session-insights:sessions:read`) — lists a device's sessions as an array of
    `SessionInfo` (`200`; empty array when none — never 404s). Device = submitted
    `device` id, else token subject (two/three-legged, 422 MISSING_IDENTIFIER when
    neither). Two control planes (DESIGN §7): identifier reserved-error suffix →
    canonical CAMARA error (checked first, mirroring QoD's retrieve-by-device);
    else the in-memory store scanned by device echo (new `store::find_by_device`).
    A resolved identifier with no `device` echo (non-E.164 subject, no submitted
    device) matches nothing → `200 []`.
  - [x] `POST /sessions/{sessionId}/metrics` (`sendSessionMetrics`,
    `session-insights:sessions:write`) — submit the application-observed
    `MetricsPayload` (`packetDelay`/`jitter` Durations, `packetLossErrorRate`
    exponent, optional `upstreamRate`/`downstreamRate`). CAMARA acknowledges with
    `204` (the quality score arrives later via a `sink` notification, deferred), so
    the sim validates + confirms the session exists + `204`, without persisting the
    metrics. Keyed only on store state (opaque `sessionId`, like delete): known id
    + valid payload → `204`; unknown/deleted id → `404 NOT_FOUND`; malformed/
    missing-figure/bad-`unit` → `400 INVALID_ARGUMENT`, value out of range → `400
    OUT_OF_RANGE`. Spec's `410 Gone` (expired session) a documented cut (no
    retained expired state — a deleted session evicts → 404; expiry arrives with
    the deferred notifications). No new dep.
  - [~] CloudEvents notifications on `sink`
    (`src/apis/session_insights/notifications.rs`; event type
    `org.camaraproject.session-insights.v0.network-quality-score`, mirroring QoD /
    QoS Provisioning):
    - [x] `network-quality-score` on `sendSessionMetrics` — a `204` for a session
      that recorded a `sink` now fires a `network-quality-score` CloudEvent to that
      sink (fire-and-forget over raw TCP, `http://` only — no HTTP-client dep,
      `https://` a documented no-op cut; still `204`). `data.qualityScore` (0–100)
      is deterministic from the submitted `MetricsPayload` (score = clamp(100 −
      (10−packetLossErrorRate)·8 − packetDelay.value/20 − jitter.value/20, 0, 100)),
      so the metrics are a genuine control plane. No new dep.
    - [x] `sinkCredential` (ACCESSTOKEN bearer / PLAIN Basic) auth on the callback —
      a session created with a `sinkCredential` now authenticates every callback
      (`network-quality-score` and the `session-ended` legs): ACCESSTOKEN →
      `Authorization: Bearer <token>` (RFC 6750), PLAIN →
      `Authorization: Basic base64(identifier:secret)` (RFC 7617). The derived header
      is stashed in a `sessionId`-keyed credential side-store at `createSession`
      (never echoed by `GET`/`retrieve-sessions`), *peeked* non-destructively on each
      `sendSessionMetrics` delivery (metrics may repeat), and taken single-use on the
      terminal legs. `REFRESHTOKEN` a documented cut. No new dep (mirrors QoS
      Provisioning / Carrier Billing).
    - [~] `session-ended` CloudEvent:
      - [x] `SESSION_DELETED` on `deleteSession` — a deleted session that recorded a
        `sink` receives the terminal
        `org.camaraproject.session-insights.v0.session-ended` CloudEvent
        (`data.terminationReason: SESSION_DELETED`), fire-and-forget over raw TCP
        (`http://` only; no HTTP-client dep); ACCESSTOKEN `sinkCredential` bearer
        applied and taken single-use (the event is terminal). Still `204` to the caller.
      - [x] `NETWORK_TERMINATED` leg — a `…001` identifier creates an ordinary
        `ACTIVE` session, but `createSession` schedules an early network drop
        (`spawn_network_termination`, 1 s fixed grace, mirroring QoD/QoS
        Provisioning): the session is evicted and the terminal `session-ended`
        CloudEvent (`terminationReason: NETWORK_TERMINATED`) is delivered to the
        `sink` (fire-and-forget over raw TCP, `http://` only; ACCESSTOKEN
        `sinkCredential` bearer applied, taken single-use; exactly-once vs a
        concurrent delete). No new dep.
      - [x] `SESSION_EXPIRED` (expiry timer) leg — `createSession` schedules an
        async expiry timer (`spawn_session_expiry`) for a time-bounded, sink-bearing
        session; at `expiresAt` it evicts the session and delivers the terminal
        `session-ended` CloudEvent (`terminationReason: SESSION_EXPIRED`) over raw
        TCP (`http://` only; ACCESSTOKEN `sinkCredential` bearer applied, taken
        single-use). Mutually exclusive with `SESSION_DELETED`/`NETWORK_TERMINATED`
        (exactly-once via `store::remove`). The fixed 24 h lifetime is not waited on
        in a test — the timer is exercised end-to-end by driving it with an
        already-past `expiresAt` (fires immediately), the same code path a live
        session takes at expiry. No new dep.
    - [ ] TLS (`https://` sink) delivery (needs a rustls TLS client)
- [x] Network Health Assessment vwip (`/network-health-assessment/vwip`; CAMARA
  NetworkInsights, wip — no released version yet, mounted at its canonical `vwip`
  base path; stateless, non-spatial, **network-keyed** aggregate health score —
  never device-level data, so two-legged only):
  - [x] `GET /health-scores` (`network-health-assessment:health-scores:read`,
    `getHealthScores`) — the latest aggregate health score of a network module
    (`{ networkId, netType, score, scoringTime }`). Two required query params:
    `networkId` (UUID; missing/non-UUID → 400 INVALID_ARGUMENT) and `netType`
    (`NET`/`NET_WIRELESS`/`NET_TRANSPORT`/`NET_CORE`; missing/unknown → 400
    INVALID_ARGUMENT). Two control planes (DESIGN §7): the `networkId`
    reserved-error suffix → canonical CAMARA error (UUIDs contain digits, so the
    shared convention applies unchanged, e.g. `…-000000000404` → 404 NOT_FOUND);
    else the `networkId`'s trailing three digits `d` set the score band and
    `netType` shifts it per module — `d == 000`/no-digits → no data
    (`score:null`, `scoringTime:null`), else `score = clamp(d/10 − moduleIndex,
    0, 100)` (`NET`=0…`NET_CORE`=3). `scoringTime` = now (self-contained RFC 3339
    formatter, no new dep). `x-correlator` echoed. The sibling Network Traffic
    Analysis API of the NetworkInsights suite is now implemented (see below).
    **Completes Network Health Assessment vwip.**
- [x] Network Traffic Analysis vwip (`/network-traffic-analysis/vwip`; CAMARA
  NetworkInsights, wip — no released version yet, mounted at its canonical `vwip`
  base path; stateless, non-spatial, **network-keyed** aggregate traffic query —
  the traffic counterpart of Network Health Assessment, so two-legged only):
  - [x] `GET /traffic-analysis` (`network-traffic-analysis:traffic-analysis:read`,
    `getTrafficAnalysis`) — aggregated, per-application DPI traffic records for a
    network over a window (`{ records: [{ app, accessCount, accessUpFlow,
    accessDownFlow, accessFlow (=up+down), startDate, endDate, accessDate,
    ipv4Address?, description? }], pagination }`). Required query params:
    `networkId` (UUID), `startDate`/`endDate` (RFC 3339), `frequency`
    (`DAY`/`HOUR`); optional `app` filter (≤128), `page` (default 1), `perPage`
    (default 20, max 100). Three control planes (DESIGN §7): the `networkId`
    reserved-error suffix → canonical CAMARA error (UUIDs carry digits, so `…404`
    → 404 NOT_FOUND); its trailing three digits `d` → application count
    (`(d % 5) + 1` from a fixed 5-entry DPI catalog) + traffic scale, `…000`/
    no-digits → the spec's no-data `200` (empty `records`); and window ×
    `frequency` → number of time slots (whole units in `[startDate, endDate)`,
    min 1, **capped at 100** to bound the response — a documented cut). `app`
    filters to one application (unknown → empty page; `app` still present per
    record); `page`/`perPage` window the result. Validation: bad/missing param →
    400 INVALID_ARGUMENT; `endDate <= startDate` / paging `<1` / `perPage > 100`
    → 400 OUT_OF_RANGE. Self-contained RFC 3339 parser + formatter, no new dep.
    `x-correlator` echoed. **Completes Network Traffic Analysis vwip.** (The
    NetworkInsights suite's async/subscription surfaces remain out of scope.)
- [x] Consent Info vwip (`/consent-info/vwip`; CAMARA ConsentInfo, wip — no
  released version yet, mounted at its canonical `vwip` base path; stateless,
  non-spatial, phone-number-keyed consent-status query):
  - [x] `POST /retrieve` (`consent-info:retrieve`, `retrieveStatus`) —
    `{ statusInfo: [{ scopes, purpose, statusValidForProcessing, statusReason?,
    expirationDate? }], captureUrl? }`, whether the consent for a set of `scopes`
    under a declared `purpose` (`dpv:<Purpose>`) is currently valid for
    processing. Same two-legged (submitted `phoneNumber`) / three-legged (E.164
    `sub`) identifier rule as Subscription Status with 422 `UNNECESSARY_IDENTIFIER`
    / `MISSING_IDENTIFIER`. Three control planes (DESIGN §7): identifier
    reserved-error suffix → canonical CAMARA error (`…404` → NOT_FOUND over the
    API's own IDENTIFIER_NOT_FOUND, `…422` → SERVICE_NOT_APPLICABLE); the
    identifier's trailing three digits `d` pick the consent state (`d % 6`: 0 →
    valid + future expiry, 1 → PENDING, 2 → REQUESTED, 3 → DENIED, 4 → EXPIRED +
    past expiry, 5 → OBJECTED); and `requestCaptureUrl` gates a deterministic
    top-level `captureUrl` (FNV-1a token, no dep) offered only when consent is
    not valid. Two request-level 403 planes: a scope containing `forbidden` →
    `CONSENT_INFO.NOT_ALLOWED_SCOPES_PURPOSE`; a bad `callbackUrl` →
    `CONSENT_INFO.INVALID_CALLBACK_URL`. Validation: missing/unknown field, empty
    `scopes`, bad `purpose` pattern, non-E.164 `phoneNumber` → 400 INVALID_ARGUMENT.
    Cuts (documented): single grouped `statusInfo` entry, stateful
    `CAPTURE_FREQUENCY_EXCEEDED`, async `callbackUrl` delivery. No new dep.
    **Completes Consent Info vwip.**
- [x] IoT SIM Fraud Prevention vwip (`/iot-sim-fraud-prevention/vwip`; CAMARA
  IoTSIMFraudPrevention `wip`; device-identifier-keyed; both the `IMEIBIND` and
  `AREALIMIT` flows are **stateful** over a shared in-memory store):
  - [x] `POST /query` (`query`, `iot-sim-fraud-prevention:query`) for
    `queryType: IMEIBIND` — `{ imeiBind: { bindStatus, bindImei? } }`. Device
    (phoneNumber/nai/ipv4/ipv6) or three-legged-token identifier with the CAMARA
    two-/three-legged rule (422 `UNNECESSARY_IDENTIFIER`/`MISSING_IDENTIFIER`).
    Control planes (DESIGN §7): identifier reserved-error suffix; **a stored
    binding wins** (BOUND with the stored IMEI); else identifier trailing-digit
    parity → BOUND (odd, with a synthesised Luhn-valid 15-digit IMEI = fixed TAC
    `35209900` + zero-padded serial + check digit) vs UNBOUND
    (even/`…000`/no-digits). Vendored spec's `QueryType` enum trimmed to
    `[IMEIBIND]`, so an `AREALIMIT` request → 400 `INVALID_ARGUMENT` (documented cut).
  - [x] `POST /bind` (`bindDeviceImei`, `iot-sim-fraud-prevention:bind`) — binds a
    device's SIM to its IMEI in a new in-memory store
    (`src/apis/iot_sim_fraud_prevention/store.rs`; `Mutex<HashMap>`, no new dep),
    `200 { bound: true }` (idempotent). Same identifier resolution + two-/three
    -legged rule; reserved-error suffix → canonical CAMARA error. `bindType` enum
    trimmed to `[IMEIBIND]` → `AREALIMIT` bind → 400 INVALID_ARGUMENT. A later
    `query` sees the binding.
  - [x] `POST /unbind` (`unBindDeviceImei`, `iot-sim-fraud-prevention:unbind`) —
    removes the binding: present → `200 { unbound: true }` (query returns to its
    stateless default); no binding → `422 UNNECESSARY_UNBIND_IMEI`. Same
    identifier/reserved-error planes; `unBindType` trimmed to `[IMEIBIND]` →
    `AREALIMIT` unbind → 400 INVALID_ARGUMENT. **Completes the IMEIBIND round-trip.**
  - [x] `queryType: AREALIMIT` on `POST /query` — reports the device's
    area-restriction status `{ areaLimit: { areaLimitStatus:
    RESTRICTED|UNRESTRICTED, limitArea?: <Circle> } }`. Same identifier
    resolution / two-/three-legged rule / reserved-error plane as the IMEIBIND
    query; stateless default keyed off the identifier's trailing-digit parity
    (odd → RESTRICTED with a deterministic schema-valid `Circle`; else →
    UNRESTRICTED, no `limitArea`), mirroring IMEIBIND's odd→active pattern. The
    `QueryType` enum now carries `AREALIMIT`; an unknown value → 400
    INVALID_ARGUMENT. Spec: `QueryType`/`QueryFraudPreventionResponse` +
    `AreaLimit`/`AreaLimitStatus`/`Area`/`AreaType`/`Circle`/`Point` schemas +
    functional cases/examples. No new dep.
  - [x] `bindType: AREALIMIT` / `unBindType: AREALIMIT` — the *set*/*clear* of a
    device's area restriction over a second in-memory set in the shared store.
    The upstream bind carries **no** geometry (the allowed area is
    network-provisioned), so the store only records membership; the `Circle`
    `limitArea` is synthesised deterministically from the identifier at query
    time (shared `synth_circle`). A stored restriction wins on an `AREALIMIT`
    query (`RESTRICTED`, even for an even-tail device that defaults
    `UNRESTRICTED`), mirroring IMEIBIND's "stored binding wins"; an `AREALIMIT`
    unbind clears it → `200 { unbound: true }`, or `422
    UNNECESSARY_UNBIND_AREALIMIT` when none is in force. The two facets are
    independent (an IMEIBIND unbind leaves an AREALIMIT restriction in force).
    `BindType`/`UnBindType`/`QueryType` enums now all carry both `IMEIBIND` and
    `AREALIMIT`. **Completes the IoT SIM Fraud Prevention API.**
- [~] Sponsored Data vwip (`/sponsored-data/vwip`; CAMARA SponsoredData `wip`;
  phone-number-keyed sponsorship lifecycle):
  - [x] `POST /sponsorship` (`startSponsorship`, `sponsored-data:sponsorship:create`)
    — start a sponsorship session → `201` with a minted `sessionId` and the
    granted window. Identifier = submitted `phoneNumber`; reserved suffix →
    canonical CAMARA error; `dataVolume` (1–1000 MB, default 50) and `duration`
    (1–1440 min, default 10) are two more control planes (out-of-range → 400
    OUT_OF_RANGE), `endTime = startTime + duration`. Now **persists** the granted
    session in a new in-memory store (`src/apis/sponsored_data/store.rs`;
    `Mutex<HashMap>`, no new dep) so `getSessionStatus` can read it back; scope
    CamaraSim-assigned (the wip contract declares no securitySchemes).
    `x-correlator` echoed.
  - [x] `GET /sponsorship/{sponsorId}/{campaignId}/{sessionId}/session-status`
    (`getSessionStatus`, `sponsored-data:sponsorship:read`) — reads a started
    session back and derives its **live** status. Opaque `sessionId` → store state
    is the control plane (unknown id, or a `sponsorId`/`campaignId` not matching
    the stored session → 404 NOT_FOUND). Two derived planes (DESIGN §7): the
    stored `phoneNumber`'s trailing three digits `d` → `dataVolumeConsumed =
    d % (grant+1)`, `dataVolumeAvailable = grant − consumed`; and the granted
    window → `sessionStatus` (`now ≥ endTime` → inactive/`validity_expired`; else
    a fully-consumed grant → inactive/`data_exhausted`; else `active`, no
    `endReason`). `endReason` `session_revoked`/`not_available` documented but not
    yet reachable. `x-correlator` echoed.
  - [x] `DELETE /sponsorship/{sponsorId}/{campaignId}/{sessionId}/revoke`
    (`revokeSponsorship`, `sponsored-data:sponsorship:delete`) — evicts the
    addressed session from the shared store (single-use) and returns `200` with
    the revoked window + `requestResult:"successful_revocation"`. Opaque
    `sessionId` → store state is the only control plane (like `getSessionStatus`):
    unknown id, or a `sponsorId`/`campaignId` not matching the stored session, →
    `404 NOT_FOUND` (a mismatch leaves the session in place); a second revoke →
    `404`. Because revoke evicts, `getSessionStatus`'s `endReason` `session_revoked`
    stays documented-but-unreached. The end-of-session `webhookUrl` callback is
    still deferred. New `store::remove_matching` (atomic check-and-remove).
  - [~] campaign operations (`/campaign/…`):
    - [x] `GET /campaign/{sponsorId}/{campaignId}/campaign-status`
      (`getCampaignStatus`, `sponsored-data:campaign:read`) — reports a whole
      campaign's operational state, distinct from a single session. No campaign
      store (the upstream `manageCampaign` CRUD is unmodelled), so the status is
      derived **statelessly** from the `campaignId`'s embedded UUID (DESIGN §7):
      malformed `sponsorId`/`campaignId` path → 400 INVALID_ARGUMENT; the UUID's
      trailing three digits `d` — reserved suffix → canonical CAMARA error
      (`…404` → 404 campaign-not-found); else `d` even → `prepaid` (carries
      `contractedDataVolume`/`remainingDataVolume`) / odd → `postpaid`
      (`usedDataVolume` only), and `(d/2)%3` → `status` active/paused/completed
      with the matching `completionReason` (`completed` + `(d/6)` odd →
      `data_exhausted` spending the whole allotment, else `time_expired`). Window
      anchored to now. `x-correlator` echoed. No new dep.
    - [x] `GET /campaign/{sponsorId}/{campaignId}/active-sponsorships`
      (`getActiveSponsorships`, scope `sponsored-data:campaign:read`) — lists the
      campaign's **currently-active** sessions as `{sessionId, phoneNumber}` pairs
      + `totalCount`. Two control planes (DESIGN §7): a reserved trailing-digit
      suffix on the campaignId UUID → canonical CAMARA error (mirrors
      `getCampaignStatus`); else the shared session store is scanned for the
      `(sponsorId, campaignId)` pair (new `store::all_matching`) and filtered to
      the active sessions (shared `is_active`/`consumption`, extracted from
      `getSessionStatus` so the two derivations don't drift). Empty campaign → 200
      empty array, `totalCount:0` (a list never 404s). Malformed path ids → 400
      INVALID_ARGUMENT. `x-correlator` echoed. No new dep.
    - [ ] `configureAlerts` / `manageCampaign` — deferred.
- [~] Click to Dial vwip (`/click-to-dial/vwip`; CAMARA ClickToDial `wip`;
  two-legged, business-facing call origination):
  - [x] `POST /calls` (`createCall`, `click-to-dial:calls:create`) — create a
    call between `caller` and `callee` → `201` `Call { status: initiating }`.
    Two-legged (both participants in the body, like Verified Caller — no
    identifier dance). The `callee` is the identifier: reserved suffix →
    canonical CAMARA error; `…000` → 422 CALLEE_NOT_AVAILABLE; a `…000` caller →
    422 CALLER_NOT_AVAILABLE (checked first); `recordingEnabled` + a `…777`
    callee → 422 RECORDING_NOT_SUPPORTED; equal numbers → 422 SAME_CALLER_CALLEE;
    a non-E.164 number → 422 INVALID_PHONE_NUMBER; missing/unknown field/bad body
    → 400 INVALID_ARGUMENT. `callId` deterministic UUID-shaped from the pair (no
    new dep). Stateless create (the `201` is fully determined by the request);
    `x-correlator` echoed.
  - [x] stateful `GET /calls/{callId}` (`getCall`, `click-to-dial:calls:read`) —
    `createCall` now **persists** the created call in a new in-memory store
    (`src/apis/click_to_dial/store.rs`; `Mutex<HashMap>`, no new dep) so `getCall`
    reads it back verbatim (`200`) or `404 NOT_FOUND` for an unknown/never-created
    id. Opaque `callId` → store state is the only control plane (no
    reserved-identifier plane; mirrors QoD `getSession` / Carrier Billing
    `retrievePayment`). `x-correlator` echoed.
  - [x] stateful `DELETE /calls/{callId}` (`terminateCall`,
    `click-to-dial:calls:delete`) — evicts the call from the shared store
    (`store::remove`, atomic check-and-remove) → `204 No Content`; unknown/
    already-terminated id → `404 NOT_FOUND` (single-use eviction, so a later
    `getCall`/`terminateCall` is a 404). Opaque `callId` → store state is the
    only control plane (mirrors `getCall`). No `sink` signalling (deferred).
    `x-correlator` echoed. No new dep.
  - [x] `GET /calls/{callId}/recording` (`getRecording`,
    `click-to-dial:recordings:read` — a dedicated recordings scope) — returns the
    call's `RecordingResource` (`callId`, base64 `content`, `contentType`,
    `generatedAt`). Two control planes (DESIGN §7): the opaque `callId` → store
    state (mirrors `getCall`), plus the stored call's `recordingEnabled` flag — a
    call with `recordingEnabled:true` → `200`, a call created without recording →
    `404 NOT_FOUND` (none generated), unknown id → `404 NOT_FOUND` (both 404s use
    the canonical `NOT_FOUND` code, per CAMARA, differing only in message). The
    `content` is a fixed, deterministic silent WAV (`audio/wav`) — the sim has no
    real media; the "session completed" precondition is a documented cut. No new
    dep (reuses `base64`). `x-correlator` echoed.
  - [x] `409 ALREADY_EXISTS` duplicate-call case on `createCall` — makes
    `createCall` **stateful**: the `callId` is deterministic from the pair, so a
    re-create of a still-live call for the same `caller`/`callee` pair → `409
    ALREADY_EXISTS` (store-keyed control plane) rather than an overwrite. New
    atomic `store::insert_new` (check-and-insert under one lock hold, no new dep);
    the pair is creatable again once `terminateCall` evicts it. A `callee` reserved
    suffix `…409` still yields the canonical `CONFLICT` (distinct code).
  - [~] `status-changed` CloudEvents on `sink`
    (`src/apis/click_to_dial/notifications.rs`; event type
    `org.camaraproject.click-to-dial.v0.status-changed`, mirroring QoD /
    Session Insights):
    - [x] create-time event — a `createCall` that supplies a `sink` delivers one
      `status-changed` CloudEvent reflecting the call's initial `initiating`
      state (fire-and-forget over raw TCP, `http://` only — no HTTP-client dep,
      `https://` a documented no-op cut; `sinkCredential` applied on the callback —
      ACCESSTOKEN → `Authorization: Bearer` (RFC 6750), PLAIN → `Authorization: Basic
      base64(identifier:secret)` (RFC 7617), REFRESHTOKEN a cut; the credential is
      derived from the create body, so no side-store needed). Documented by the
      `createCall` callback in the vendored spec.
    - [x] terminate-time event — a `terminateCall` of a call created with a
      `sink` delivers a terminal `status-changed` CloudEvent (`state:
      disconnected`, with a `reason`), mirroring QoD's `deleteSession` →
      `DELETE_REQUESTED`. The sink + derived credential (ACCESSTOKEN Bearer / PLAIN
      Basic) are recorded at create time in a `store` sink side-store (kept apart
      from the `Call`, so the secret is never echoed) and taken single-use at
      terminate; `store::remove`
      now returns the removed `Call` so the participants can be read for the event.
      `http://` only (`https://`/no-sink → 204 with no event); exactly-once vs a
      concurrent terminate (atomic `remove`). No new dep.
    - [ ] ongoing *intermediate* lifecycle transitions
      (`callingCaller`/`callingCallee`/`connected`, a spontaneous `failed`, with
      `callDuration`/`recordingResult`) — deferred (no live call engine).
    - [ ] TLS (`https://` sink) delivery (needs a rustls TLS client).
- [x] Most Frequent Location vwip (`/most-frequent-location/vwip`; CAMARA
  MostFrequentLocation `wip` — no released version, mounted at its canonical
  `vwip` base path like Device Visit Location / Session Insights; stateless,
  device-keyed area-residency score):
  - [x] `POST /verify` (`most-frequent-location:verify`, `verifyFrequentLocation`)
    — how frequently the device resides within a supplied `geoReference`, as a
    privacy-preserving `{ score: 0..=100 }` (0 never present … 100 almost
    always), never the device's location. Device-object identifier resolution +
    the two-legged/three-legged rule (device on a line token → 422
    `UNNECESSARY_IDENTIFIER`; no device + non-line subject → 422
    `MISSING_IDENTIFIER`; empty `device` → 400 INVALID_ARGUMENT). Two control
    planes (DESIGN §7): the `geoReference` (validated first — `COVERAGE_ZONE`
    lat/long out of range → 400 OUT_OF_RANGE; `POSTAL_CODE` `00000` → 400
    `MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID`; unknown `type` / missing or
    foreign fields → 400 INVALID_ARGUMENT) and, once the identifier is resolved,
    its reserved error suffix → canonical CAMARA error (`…422` →
    SERVICE_NOT_APPLICABLE); else `score = (identifier trailing three digits +
    area offset) % 101`, so **both** the device and the area are genuine planes
    (area offset = `round(|lat|)+round(|long|)` for a zone, postal trailing three
    digits for a code). `x-correlator` echoed. No new dep. The upstream API's
    specific 404 IDENTIFIER_NOT_FOUND / 404 INFORMATION_NOT_AVAILABLE / 422
    UNSUPPORTED_IDENTIFIER sub-cases are represented via the shared reserved-suffix
    canonical codes (documented cut). **Completes Most Frequent Location vwip.**
- [~] Traffic Influence vwip (`/traffic-influence/vwip`; CAMARA TrafficInfluence
  `wip`; EdgeCloud traffic-steering, resource-oriented over an in-memory store):
  - [x] `POST /traffic-influences` (`postTrafficInfluence`,
    `traffic-influence:traffic-influences:write`) — create a `TrafficInfluence`
    resource steering an app's traffic toward an edge-cloud placement → `201`
    with a minted `trafficInfluenceID`, the placement echoed, a lifecycle
    `state`, and a `Location` header. Persisted in a new in-memory store
    (`src/apis/traffic_influence/store.rs`; `Mutex<HashMap>`, no new dep) for a
    future read-back. Two control planes (DESIGN §7) on `appId` (a hex UUID):
    reserved error suffix → canonical CAMARA error; else trailing three digits
    `d` → state (`d%3`: 0→ordered, 1→created, 2→active). 400 INVALID_ARGUMENT
    (missing/malformed `apiConsumerId`/`appId`/`appInstanceId`/`edgeCloudRegion`/
    `edgeCloudZoneId`, or non-JSON body) / OUT_OF_RANGE (a `sourcePort`/
    `destinationPort` outside `0..=65535`). `x-correlator` echoed.
  - [x] `GET /traffic-influences/{trafficInfluenceID}` (`getTrafficInfluence`,
    `traffic-influence:traffic-influences:read`) — reads a created resource back
    from the shared in-memory store (`store::get`), returned verbatim (`200`) or
    `404 NOT_FOUND` for an unknown/never-created id. Opaque operator-minted
    `trafficInfluenceID` → store state is the only control plane (no
    reserved-identifier plane; mirrors QoD `getSession` / Click to Dial `getCall`).
    `x-correlator` echoed. No new dep.
  - [x] `DELETE /traffic-influences/{trafficInfluenceID}` (`deleteTrafficInfluence`,
    `traffic-influence:traffic-influences:delete`) — evicts the stored resource
    from the shared in-memory store (new `store::remove`, atomic check-and-remove):
    present → `202 Accepted` (async-deletion contract; evicted synchronously,
    single-use), unknown/already-deleted id → `404 NOT_FOUND`. Opaque id → store
    state is the only control plane (no reserved-identifier plane; mirrors Click to
    Dial `terminateCall`). `x-correlator` echoed. No new dep.
  - [x] `PATCH /traffic-influences/{trafficInfluenceID}` (`patchTrafficInfluence`,
    `traffic-influence:traffic-influences:write`) — merge-patch update of the
    mutable placement/filter fields in place → `200` updated resource / `404
    NOT_FOUND`. Body is `merge-patch+json`: a supplied field replaces, an explicit
    `null` clears an optional field, and identity/read-only fields
    (`trafficInfluenceID`/`appId`/`state`) are ignored (no provisioning worker, so
    `state` is not re-derived — a documented cut). Two control planes (DESIGN §7):
    the request body (validated first — malformed field → 400 INVALID_ARGUMENT,
    port ∉ 0..=65535 → 400 OUT_OF_RANGE) and the opaque id's store state (unknown →
    404), so a body 400 wins over a 404. New atomic `store::update_with`
    (get-modify-write under one lock hold, no new dep).
  - [x] `POST /traffic-influence-devices` (`postTrafficInfluenceDevice`,
    `traffic-influence:traffic-influence-devices:write`) — the per-device create
    variant. Same fields + validation as `postTrafficInfluence` (shared
    `validate_base`/`finalize`) plus a required `device` object (`minProperties:
    1`; phoneNumber E.164 / networkAccessIdentifier / ipv4Address / ipv6Address,
    `publicPort` range-checked). For privacy the device is validated only — never
    echoed nor persisted; `appId` stays the sole control plane. Creates the same
    `TrafficInfluence` resource → `201` (device absent from the body), `Location`
    under `/traffic-influences`, readable back via `getTrafficInfluence`.
  - [~] `subscriptionRequest` CloudEvents change notifications
    (`src/apis/traffic_influence/notifications.rs`; event type
    `org.camaraproject.traffic-influence.v1.traffic-influence-change`, mirroring
    QoD / QoS Provisioning):
    - [x] create-time **initial event** (`config.initialEvent: true`) — a valid
      `subscriptionRequest` on `postTrafficInfluence` / `postTrafficInfluenceDevice`
      delivers one `traffic-influence-change` CloudEvent to the `sink` on create,
      reflecting the new resource's `state` (its `data` is the created
      `TrafficInfluence`). Fire-and-forget over raw TCP (`http://` only, no
      HTTP-client dep); `protocol: HTTP` + the single change-event `types` required;
      ACCESSTOKEN `sinkCredential` bearer applied (never echoed), PLAIN/REFRESHTOKEN
      a cut; a malformed `subscriptionRequest` → 400 INVALID_ARGUMENT.
    - [ ] ongoing state-change stream + `subscriptionExpireTime` /
      `subscriptionMaxEvents` lifecycle + the `TrafficInfluenceNotification` extras
      (`selected_appInstanceId` / `deviceResponse`) — deferred (no provisioning worker).
    - [ ] TLS (`https://` sink) delivery (needs a rustls TLS client)
- [x] Application Endpoint Discovery vwip (`/application-endpoint-discovery/vwip`;
  CAMARA ApplicationEndpointDiscovery `wip`; stateless, non-spatial, device-keyed
  EdgeCloud discovery — the endpoint-level successor to Simple/Optimal Edge
  Discovery):
  - [x] `POST /retrieve-optimal-app-endpoints` (`getOptimalAppEndpoints`, scope
    `application-endpoint-discovery:app-endpoints:read`) — returns the optimal
    application `ApplicationEndpoint`s (`port` + one of `fqdn`/`ipv4Addresses`/
    `ipv6Addresses`, plus an `edgeCloudZone`) for the identified device. Exactly
    one of `appId` / `applicationEndpointsId` required (both UUID; neither/both/
    non-UUID → 400 INVALID_ARGUMENT — a documented tightening of the upstream
    `anyOf`). Device-object two-legged / three-legged identifier rule (mirrors
    Optimal Edge Discovery: 422 `UNNECESSARY_IDENTIFIER`/`MISSING_IDENTIFIER`,
    empty device → 400). Two control planes (DESIGN §7): the application
    identifier's trailing three digits `d` — `…000` → 404 NOT_FOUND (not
    registered), else `(d % 3) + 1` endpoints (address family rotates by rank);
    and the resolved device identifier's reserved-error suffix → canonical CAMARA
    error (checked first, so device-not-found 404 is distinct from app-not-found
    404). `DeviceResponse` (phoneNumber only) echoed only when the request
    `device` carried multiple identifiers. Deterministic endpoints/zone-ids via
    SHA-256; no new dep. **Completes Application Endpoint Discovery vwip.**
- [x] Predictive Connectivity Data vwip (`/predictive-connectivity-data/vwip`;
  CAMARA PredictiveConnectivityData `wip` — no released version yet, mounted at
  its canonical `vwip` base path; stateless, **area-keyed** connectivity forecast
  — the connectivity counterpart of Population Density Data):
  - [x] `POST /retrieve` (`retrieveConnectivity`, `predictive-connectivity-data:read`)
    — per grid cell, a stack of vertical **layers** (altitude bands
    `layerThickness`=30 m tall) each rated `GC`/`MC`/`NC`/`ND`, plus an overall
    area-support `status`. No device identifier — the **geometry is the control
    plane** (mirrors Population Density Data). CamaraSim implements the
    **synchronous `GEOHASHLIST`** path with a single time slice. Control planes
    (DESIGN §7): the **first geohash**'s reserved error suffix → canonical CAMARA
    error (checked before the window); **each geohash**'s stable hash fixes its
    cell (`h % 7 == 0` → NO_DATA/all-`ND`, else ground score `h % 100` degrading
    15 pts per layer up); the target **`serviceLevel`** (`C2`/`STREAM_4K`/
    `BEST_EFFORT`) sets how strict the per-layer rating is (a genuine 2nd plane);
    **`height`** sets the layer count (`height/30 + 1`, default 4);
    `includeSignalStrength` toggles the per-layer dBm band; the cell mix fixes
    `status` (all NO_DATA→`AREA_NOT_SUPPORTED`, some→`PART_OF_AREA_NOT_SUPPORTED`,
    else `SUPPORTED_AREA`). Capability/structural planes: `POLYGON` areaType → 422
    `…UNSUPPORTED_AREA_TYPE`; geohash length > 9 → 422 `…UNSUPPORTED_PRECISION`;
    > 100 geohashes → 422 `…UNSUPPORTED_SYNC_RESPONSE`; unknown areaType/
    `serviceLevel`/`networkType`, bad geohash, empty or >1000 list, `precision`
    with a GEOHASHLIST, `height` ∉ 0..=250 → 400 `INVALID_ARGUMENT`. Time window:
    malformed → 400 `INVALID_ARGUMENT`; `endTime<startTime` → 400
    `…INVALID_END_TIME`; > 7 days → 400 `…MAX_TIME_PERIOD_EXCEEDED`
    (self-contained RFC 3339 parser). Documented cuts: `POLYGON`, async `sink`/
    CloudEvents (202 flow), hourly time-slicing, absolute start-time checks, and
    `UNSUPPORTED_SERVICE_LEVEL` (all three service levels supported).
    `x-correlator` echoed. No new dep. **Completes Predictive Connectivity Data
    vwip.**
- [x] Network Access Devices vwip (`/network-access-devices/vwip`; CAMARA
  NetworkAccessManagement, wip — no released version yet, mounted at its
  canonical `vwip` base path; operator-managed access equipment — gateways/
  routers/access points, not end-user devices):
  - [x] `GET /network-access-devices` (`network-access-devices:reboot`,
    `getNetworkAccessDevices`) — lists the subscriber's operator-supplied
    devices as a `NetworkAccessDeviceList` (`200`). No request body, so the token
    **subject** is the sole control plane (DESIGN §7; mirrors Number
    Verification's `GET /device-phone-number`): reserved error suffix → canonical
    CAMARA error; else the subject's trailing three digits `d` (or `0` when none)
    drive the device **count** (`d == 0` → 1, else `((d-1) % 3) + 1`, 1–3) and
    each device's **status** (device `i` → `[connected, disconnected,
    unavailable][(d + i) % 3]`), so both facets are controllable. UUID-shaped
    `id` + EUI-48 `hardwareAddress` deterministic from subject + index (SHA-256,
    no new dep). `serviceSite` + inherited `Device` end-user identifier fields
    omitted for operator devices (schema-valid cut — only `id` required).
    `x-correlator` echoed.
  - [x] `GET /network-access-devices/{networkAccessDeviceId}`
    (`getNetworkAccessDevice`, `network-access-devices:reboot`) — per-device read,
    implemented **statelessly** (no store): the subscriber's device set is
    deterministic from the token subject, so the read regenerates that set and
    returns the device whose `id` matches. Two control planes (DESIGN §7): the
    subject's reserved-error suffix → canonical CAMARA error (account-level,
    mirroring the list); else the id vs the subject's set — a matching id → `200`
    with that `NetworkAccessDevice`, any other id (unknown / another subscriber's
    / malformed) → `404 NOT_FOUND`. `x-correlator` echoed. No new dep (reuses
    `sha2`). A documented simplification of CAMARA's stateful resource read.
  - [x] Reboot Requests resource lifecycle (`POST/GET/PATCH/DELETE
    /reboot-requests…`, `network-access-devices:reboot`) — stateful, in-memory
    store (`src/apis/network_access_devices/store.rs`; `Mutex<HashMap>`, no new dep):
    - [x] `POST /reboot-requests` (`createRebootRequest`) — creates a reboot
      request for the subscriber's devices, mints an opaque UUID-shaped `id`,
      persists the rendered `RebootRequest` (`id` + resolved `devices` + optional
      `message`/`atTime` + `createdAt`/`modifiedAt`), `201` + `Location`. Two
      control planes (DESIGN §7): reserved subject suffix → canonical CAMARA error
      (account-level, mirroring list/read); and `devices` vs the subject's
      deterministic device set — an explicit list must hold UUIDs (else 400
      INVALID_ARGUMENT) the subscriber owns (else 404 NOT_FOUND), an omitted/empty
      list reboots all of them (the schema requires `devices`, so the inferred set
      is materialised). `message` ≤255 / `atTime` RFC 3339 validated + echoed;
      malformed body/field → 400. Audit `createdBy`/`modifiedBy` (uuid) omitted
      (subject isn't a UUID — documented cut). `x-correlator` echoed.
    - [x] `GET /reboot-requests/{rebootRequestId}` (`getRebootRequest`) — reads a
      created request back from the store by its opaque, server-minted id. Store
      state is the sole control plane (DESIGN §7; the id is opaque, so no
      reserved-identifier plane): a stored id → `200` with the persisted
      `RebootRequest` verbatim, any other id (never created / already deleted) →
      `404 NOT_FOUND`. Reboot requests aren't scoped per subscriber, so the
      `sub`-ownership check on the read isn't enforced (documented cut).
      `x-correlator` echoed. No new dep.
    - [x] `DELETE /reboot-requests/{rebootRequestId}` (`deleteRebootRequest`) —
      evicts the stored `RebootRequest` from the shared store (`store::remove`,
      single-use): present → `204 No Content`, unknown/already-deleted → `404
      NOT_FOUND`. Opaque, server-minted id → store state is the sole control plane
      (no reserved-identifier plane; mirrors QoD `deleteSession` / Traffic
      Influence `deleteTrafficInfluence`). `sub`-ownership not enforced (documented
      cut, mirroring the read). `x-correlator` echoed on `204` and `404`. No new dep.
    - [x] `PATCH /reboot-requests/{rebootRequestId}` (`updateRebootRequest`) —
      merge-patch update of the mutable `atTime`/`message` in place → `200`
      updated `RebootRequest` (`modifiedAt` bumped; persists). Two control planes
      (DESIGN §7): the request body (validated first — malformed `atTime`/over-long
      `message` → 400 INVALID_ARGUMENT) and the opaque store-state id **plus** the
      stored request's schedule state — a pending **scheduled** reboot (has
      `atTime`) is modifiable → 200, an **immediate** reboot (no `atTime`, already
      fired) → 409 `NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE` (store unchanged),
      unknown id → 404. Identity/target/audit fields (`id`/`devices`/`createdAt`/
      `modifiedAt`) + unknown keys ignored; `devices` not re-targeted and "already
      fired" modelled by the missing `atTime` (documented cuts — no reboot engine).
      New atomic `store::update_with` (get-modify-write, decline-aware; no new dep).
      **Completes the Reboot Requests lifecycle and Network Access Devices vwip.**
- [x] Application Endpoint Registration vwip (`/application-endpoint-registration/vwip`;
  CAMARA ApplicationEndpointRegistration, wip — no released version yet, mounted at its
  canonical `vwip` base path like Application Endpoint Discovery / Application Profiles;
  stateful, resource-oriented, **non-spatial** — the *registration* counterpart of
  Application Endpoint Discovery; `edgeCloudZone` is a placement identifier, not a
  coordinate):
  - [x] `POST /application-endpoint-lists` (`registerApplicationEndpoints`, scope
    `application-endpoint-registration:application-endpoints:write`) — registers a set of
    application endpoints (FQDN/IPv4/IPv6 + `port`, each with an optional `edgeCloudZone`),
    mints an opaque UUID-shaped `applicationEndpointListId`
    (`src/apis/application_endpoint_registration/store.rs`; `Mutex<HashMap>`, no uuid/rand
    dep, mirroring Application Profiles), persists the rendered registration, and returns
    `200` with the id (CAMARA `ApplicationEndpointListId`, a bare string). No device
    identifier → the request body is the control plane (DESIGN §7): body validation → 400
    (`applicationEndpoints` 1..=20, per-endpoint `anyOf` one address / `port` 1..=65535 →
    `OUT_OF_RANGE`, malformed IPv4/IPv6/`edgeCloudZoneId`, multi-line/over-length
    provider/description, non-UUID `applicationProfileId`, unknown field/type); the
    `applicationProfileId` reserved-error suffix → canonical CAMARA error (UUIDs carry
    digits, mirroring Network Health Assessment's `networkId`); and the nil UUID
    (`00000000-0000-0000-0000-000000000000`) → 422 `UNIDENTIFIABLE_APPLICATION_PROFILE`.
    `x-correlator` echoed.
  - [x] `GET /application-endpoint-lists` (`getAllRegisteredApplicationEndpoints`,
    scope `application-endpoint-registration:application-endpoints:read`) — list.
    Returns every registered list as an array of the canonical
    `ApplicationEndpointList` (`200`; empty array when none — a list never 404s),
    ordered by `applicationEndpointListId` for a deterministic response (new
    `store::all()`). Store state is the only control plane (no request body, no
    device identifier); registrations aren't scoped per client (documented
    simplification, mirroring the Carrier Billing / Geofencing lists).
    `x-correlator` echoed. No new dep. Same pass corrected the stored/returned
    shape to canonical **nested** `ApplicationEndpointList`
    (`applicationEndpointListId` + `applicationEndpointsInfo`) so the read-back and
    list legs agree with CAMARA (the earlier flat `ApplicationEndpointsInfoResponse`
    is replaced).
  - [x] `GET /application-endpoint-lists/{id}` (`getApplicationEndpointsById`,
    scope `application-endpoint-registration:application-endpoints:read`) —
    read-back. Returns the registration stored at `registerApplicationEndpoints`
    as the canonical **nested** `ApplicationEndpointList` (the submitted
    `ApplicationEndpointsInfo` under `applicationEndpointsInfo` + minted
    `applicationEndpointListId`) verbatim (`200`) or `404 NOT_FOUND` for an
    unknown id; a non-UUID path value → `400 INVALID_ARGUMENT` (mirrors
    Application Profiles' `getApplicationProfile`). The opaque server-minted id is
    the only control plane (no reserved-identifier suffix — it was never
    caller-chosen). `x-correlator` echoed. Reuses `store::get`; no new dep.
  - [x] `PUT /application-endpoint-lists/{id}` (`updateApplicationEndpoint`,
    scope `…:application-endpoints:update`) — full-replace update. Replaces the
    endpoints registered under an existing id with a fresh `ApplicationEndpointsInfo`
    body (a full replace, not a merge) → `204 No Content`; a later `GET` reads back
    the replacement. Shares `register`'s body-validation + `applicationProfileId`
    control planes (bad body/field → 400 INVALID_ARGUMENT / OUT_OF_RANGE; reserved
    suffix → canonical CAMARA error; nil UUID → 422 UNIDENTIFIABLE_APPLICATION_PROFILE),
    then the opaque store-state id (unknown → 404 NOT_FOUND, non-UUID path → 400).
    Body validated before store state, so a body 400 wins over a 404 (mirrors the
    Traffic Influence / Network Access Devices PATCH convention). New atomic
    `store::replace` (existence-check-and-swap under one lock hold, no new dep).
    **Completes the Application Endpoint Registration vwip resource lifecycle
    (register / read / list / update / deregister).**
  - [x] `DELETE /application-endpoint-lists/{id}` (`deregisterApplicationEndpoint`,
    scope `…:application-endpoints:delete`) — removes the stored registration →
    `204 No Content` (single-use), `404 NOT_FOUND` for an unknown/already-deregistered
    id, `400 INVALID_ARGUMENT` for a non-UUID path value. Store state the only
    control plane (no reserved-identifier suffix — the id is server-minted). New
    `store::remove`; no new dep. Verified scope/response codes against the upstream
    CAMARA spec.
- [x] Short Message Service v0alpha1 (`/sms/v0alpha1`; CAMARA ShortMessageService
  0.1.0-alpha.1; stateless, non-spatial, two-legged / business-facing send-SMS):
  - [x] `POST /short-message` (`send-sms`, scope `send-sms:short-message`) —
    `{ msgId, timestamp }` for a send from `from` to `to` (≥1 E.164 MSISDN) with a
    non-empty `message` and optional `category`
    (`PROMOTION`/`SERVICE`/`TRANSACTION`). Two-legged (both parties in the body,
    like Verified Caller). One control plane (DESIGN §7): the first recipient
    `to[0]`'s reserved error suffix → canonical CAMARA error (`…404` = recipient
    not found; `…503`/`…500` = the network-unavailable/internal cases); else
    `200` with a deterministic UUID-shaped `msgId` (SHA-256 of from+to+message, no
    uuid/rand dep) + fresh RFC 3339 UTC `timestamp`. A reserved suffix on `from`
    or a non-first recipient is not the plane. Empty `to` / non-E.164
    `from`/recipient / empty `message` / unknown `category`/field / bad body → 400
    INVALID_ARGUMENT. `x-correlator` echoed. Upstream delivery-notification
    subscription surface out of scope. **Completes Short Message Service v0alpha1.**
- [~] Capabilities and Restrictions vwip (`/capabilities-and-restrictions/vwip`;
  CAMARA CapabilitiesAndRuntimeRestrictions `wip` — no released version, mounted
  at its canonical `vwip` base path; stateless, non-spatial, consumer-context
  capability discovery):
  - [x] `POST /retrieve` (`postServiceCapability`, `camara-capability:read`) —
    query the tailored service capabilities (and their active/inactive state) for
    a context. Request `CamaraCapabilityQueryRequest`: `queries` (1..=100), each
    with a required `overlayExtends` (1..=20 definition URIs) and optional
    `resourceScopes`. Returns `201 CapabilityInfo` with one `CapabilityDetail`
    (the `CapabilitySetBitmap`+`CapabilityBitmap` branch) per query — a fixed
    3-entry `bitmapCapabilities` catalogue (each a `SchemaRestrictionsSet` of
    `CamaraOverlay` restrictions) plus a `camaraCapabilitiesBitmap` marking the
    active bits. Two control planes (DESIGN §7): the first query's first
    `resourceScopes` `phoneNumber` reserved-error suffix → canonical CAMARA error
    (`…404` = no capability API found); and the active bitmap is derived from the
    queried context (that phoneNumber's trailing three digits, else a stable FNV
    hash of `overlayExtends`, `% 8`), so the active set is a genuine second plane.
    Validation: malformed body / empty-absent `overlayExtends` / non-URI overlay /
    non-E.164 scope `phoneNumber` → 400 INVALID_ARGUMENT; >100 queries or >20
    overlays → 400 OUT_OF_RANGE. `x-correlator` echoed. No new dep. Documented
    cuts: `subscriptionRequest` change-notification callback (accepted-not-
    applied), the `CapabilitySetFootprint` branch, ETag/If-None-Match/304 caching,
    and resolving `overlayExtends` against real API definitions.
- [~] Dedicated Network — Network Profiles vwip (`/dedicated-network-profiles/vwip`;
  CAMARA DedicatedNetworks `wip` — no released version, mounted at its canonical
  `vwip` base path; stateless, non-spatial, two-legged catalog — the
  Dedicated-Networks analogue of QoS Profiles):
  - [x] `GET /profiles/{profileId}` (`readNetworkProfile`,
    `dedicated-network-profiles:profiles:read`) — single-profile lookup from a
    fixed catalog (no upstream backend). The `profileId` path param is the sole
    control plane, in three layers (DESIGN §7): not UUID-shaped → 400
    INVALID_ARGUMENT; reserved trailing-digit suffix → canonical CAMARA error
    (`…404` → 404 no-such-profile); else the trailing three digits `d` select one
    of a fixed 4-entry `NetworkProfile` template table (`d % 4`; `…000`/no-digits
    → template 0), so the profile is a genuine second plane. The returned `id`
    echoes the requested `profileId`; each profile carries `name`,
    `maxNumberOfDevices`, `aggregatedUl/DlThroughput` (`BitRate`), `qosProfiles`
    (reusing the QoS Profiles catalog names) and `defaultQosProfile`.
    `x-correlator` echoed. No new dep.
  - [x] `GET /profiles` (`readNetworkProfiles`) — the paginated
    `NetworkProfilesPage` list (`{ items, pagination }`) over the fixed catalog.
    Two control planes (DESIGN §7): the optional exact-match `name` filter
    (unknown name → empty page; a list never 404s) and the `page`/`perPage`
    window (defaults 1/10; non-integer → 400 INVALID_ARGUMENT, `<1` → 400
    OUT_OF_RANGE; `RawQuery` + `serde_urlencoded`, no new dep). Each item carries
    a stable canonical `id` (UUID tail = catalog index), so `GET /profiles/{id}`
    returns the same profile the list holds. No device identifier → no
    reserved-error plane.
- [~] Dedicated Network — Networks vwip (`/dedicated-network/vwip`; CAMARA
  DedicatedNetworks `wip` — no released version, mounted at its canonical `vwip`
  base path; **stateful**, resource-oriented, two-legged — the resource sibling
  of Network Profiles; in-memory network store
  `src/apis/dedicated_network/store.rs`):
  - [x] `POST /networks` (`createNetwork`, `dedicated-network:networks:create`)
    — creates a dedicated network from a `CreateNetwork` body (`name`,
    `networkProfileId` **xor** `qosProfileName`, `serviceTime`, `serviceAreaId`,
    optional `sink`/`sinkCredential`), mints an opaque UUID `id`, persists the
    rendered `NetworkInfo` in the store, and returns `201`. Two control planes
    (DESIGN §7): request validation (bad body/`name` length/the profile `oneOf`/
    non-UUID `networkProfileId`|`serviceAreaId`/empty `qosProfileName`/missing or
    malformed `serviceTime`/non-`https` `sink` → 400 INVALID_ARGUMENT; a UTC
    `serviceTime.end` before `start` → 400 OUT_OF_RANGE); and the required
    `serviceAreaId` UUID identifier — reserved trailing-digit suffix → canonical
    CAMARA error (`…404` → no such service area), else `d % 3` picks the created
    `status` (`0`/`…000`→REQUESTED, `1`→RESERVED, `2`→ACTIVATED; TERMINATED is a
    teardown-only state). `sinkCredential` accepted but never echoed (a secret);
    no notification delivered (create-only slice, documented cut). Self-contained
    RFC 3339 date-time shape check (no new dep). `x-correlator` echoed.
  - [x] `GET /networks/{networkId}` (`readNetwork`,
    `dedicated-network:networks:read`) — reads the stored `NetworkInfo` back
    (`200`) or `404 NOT_FOUND` for an unknown id. Keyed only on the store state
    (the minted opaque `networkId` has no reserved-suffix plane), mirroring QoD
    `getSession`. `x-correlator` echoed.
  - [x] `GET /networks` (`listNetworks`, `dedicated-network:networks:read`) —
    lists the stored `NetworkInfo`s as a bare JSON array (CAMARA has no list
    wrapper); optional `name` query param filters by network name (a second
    control plane), a `name` > 1024 chars → 400 INVALID_ARGUMENT. `store::all()`
    + self-contained form-query decode (no new dep).
  - [x] `DELETE /networks/{networkId}` (`deleteNetwork`,
    `dedicated-network:networks:delete`) — evicts the stored network named by the
    opaque `networkId` from the same in-memory store (new `store::remove`):
    present → `204 No Content` (single-use), unknown/already-deleted → `404
    NOT_FOUND`. Keyed only on store state (opaque `networkId`, no
    reserved-identifier plane, mirroring QoD `deleteSession`); synchronous `204`
    (no async `202`/`DELETE_REQUESTED`), no `sink` notification (documented cut).
    `x-correlator` echoed. **Completes the Networks CRUD.**
  - [~] the other sibling Dedicated-Networks APIs (`dedicated-network-accesses`,
    `dedicated-network-areas`) — later, as capacity allows (the `-areas` API is
    spatial).
    - [~] **Dedicated Network — Accesses vwip** (`/dedicated-network-accesses/vwip`;
      stateful, non-spatial — the access sibling of Networks; in-memory access
      store `src/apis/dedicated_network_accesses/store.rs`):
      - [x] `POST /accesses` (`createAccess`,
        `dedicated-network-accesses:accesses:create`) — creates an access from a
        `CreateAccessRequest` (`networkId` (UUID, required), optional `devices`
        (1..=100 CAMARA `Device`s), `qosProfiles` (1..=32), `defaultQosProfile`,
        `sink`/`sinkCredential`), mints an opaque UUID `id`, persists the rendered
        `AccessInfo`, returns `201`. Three control planes (DESIGN §7): request
        validation (bad body/non-UUID `networkId`/`devices` bounds/device with no
        identifier/non-E.164 `phoneNumber`/`qosProfiles` bounds or empty name/
        non-`https` `sink` → 400 INVALID_ARGUMENT); the `networkId` reserved
        trailing-digit suffix → canonical CAMARA error (`…404` → no such network);
        and each device's own identifier → per-device `GRANTED`/`DENIED`, driving
        `stats` + `recentAccessDevices` (a genuine second plane). `sinkCredential`
        accepted but never echoed (a secret). Cuts: the `207` multi-status
        `ResultForDevice[]` form (partial denials are data inside the `201`), the
        `409`/`422` request-level cases, `sink` notification, and the read/list/
        delete + `/devices…` legs. `x-correlator` echoed. No new dep.
      - [x] `GET /accesses/{accessId}` (`readAccess`,
        `dedicated-network-accesses:accesses:read`) — reads the stored
        `AccessInfo` back (`200`) or `404 NOT_FOUND` for an unknown/malformed id;
        store state is the only control plane (opaque minted UUID, no
        reserved-suffix plane), mirroring the sibling `readNetwork`.
      - [x] list/delete legs (`listAccesses`, `deleteAccess`) — `GET /accesses`
        returns a bare `AccessInfo[]` (`200`, empty when none), narrowed by the
        optional `networkId` query filter (a second control plane; non-UUID
        `networkId` → 400 INVALID_ARGUMENT, unknown-but-valid → `[]`); scope
        `…:accesses:read`. `DELETE /accesses/{accessId}` evicts a stored access
        → `204` (single-use) or `404 NOT_FOUND`, keyed only on store state (opaque
        minted id, no reserved-suffix plane, mirroring `deleteNetwork`); scope
        `…:accesses:delete`. New `store::all`/`store::remove`; self-contained
        query decode (no new dep). `x-correlator` echoed. **Completes the
        Accesses CRUD** (`/devices…` sub-resources remain).
      - [x] the `/accesses/{accessId}/devices…` sub-resources:
        - [x] `GET /accesses/{accessId}/devices` (`listDevices`,
          `dedicated-network-accesses:devices:read`) — reads the access's recorded
          device roster (its `recentAccessDevices`, each an `AccessDevice`) back as
          a paginated `AccessDevicesPage` (`{items, pagination}`). Three control
          planes (DESIGN §7): opaque `accessId` → store state (unknown → 404, no
          reserved-suffix plane, mirroring `readAccess`); optional `deviceStatus`
          filter (`REQUESTED`/`GRANTED`/`DENIED`, unknown → 400 INVALID_ARGUMENT) —
          a genuine second plane; and the `page`/`perPage` window (non-integer →
          400 INVALID_ARGUMENT, `<1` → 400 OUT_OF_RANGE), validated before the
          store so a bad query wins over a 404. House pagination envelope
          (`page`/`perPage`/`totalCount`/`totalPages`, mirroring Network Profiles).
          `x-correlator` echoed. No new dep.
        - [x] `POST /accesses/{accessId}/devices/add` (`addDevicesToAccess`,
          `…:devices:add`) — appends a bare `AddDevicesRequest` (JSON array of
          1..=100 Devices) to the access's `recentAccessDevices` roster (atomic
          `store::update`), recomputes `stats`, returns the added `AccessDevices`
          (`201 AddDevicesSuccess`). Body validation → 400 before store → 404;
          per-device GRANTED/DENIED plane; 207/422 folded into AccessDevice.status.
        - [x] `POST /accesses/{accessId}/devices/remove`
          (`removeDevicesFromAccess`, `…:devices:remove`) — evicts the roster
          entries matching a bare `RemoveDevicesRequest` (JSON array of 1..=100
          Devices, mirroring `add`) from the access's `recentAccessDevices`
          (atomic `store::update`), recomputes `stats`, returns `204 No Content`.
          Matching is by primary identifier (`device_identifier`); a device absent
          from the roster is an idempotent per-device no-op folded into the `204`
          (the `207` partial-success form is a documented cut, mirroring `add`).
          Body validation → 400 before store → 404. **Completes the
          `/devices…` sub-resources** (`dedicated-network-areas` remains). No new
          dep. `x-correlator` echoed.
    - [x] **Dedicated Network — Areas vwip** (`/dedicated-network-areas/vwip`;
      read-only, two-legged **service-area catalog** — the geographical sibling
      of Network Profiles; the `area` is a fixed CIRCLE, so the "spatial" surface
      is trivial data, not computed geometry):
      - [x] `GET /areas/{areaId}` (`readNetworkServiceArea`,
        `dedicated-network-areas:areas:read`) — single service-area lookup over a
        fixed 4-entry catalog (no upstream backend), mirroring
        `readNetworkProfile`. The `areaId` path param is the sole control plane in
        three layers (DESIGN §7): not UUID-shaped → 400 INVALID_ARGUMENT; reserved
        trailing-digit suffix → canonical CAMARA error (`…404` → 404 no-such-area);
        else trailing three digits `d` select a template (`d % 4`; `…000`/no-digits
        → template 0), so the returned area is a genuine second plane. Each
        `ServiceArea` carries a CIRCLE `area` (center + radius) and **either**
        `qosProfiles` **or** `networkProfiles` (the either/or constraint); the
        requested id is echoed as `id`. `x-correlator` echoed. No new dep.
      - [x] `POST /retrieve-service-areas` (`retrieveNetworkServiceAreas`) — the
        collection query. Lists the fixed catalog narrowed by the request body's
        optional filters, ANDed (DESIGN §7): spatial CIRCLE-only geometry —
        `atLocation` (point in area), `overlappingArea` (circles intersect),
        `coveringArea` (area contains circle), computed with a self-contained
        haversine (no geo dep) — and attribute (`byName`/`byNetworkProfileId`/
        `byQosProfileName`, exact match). No identifier → no reserved-error plane;
        a list never 404s (over-narrow filter → `[]`). Each returned area carries
        its stable canonical `id` (round-trips to `readNetworkServiceArea`). Bad
        body / non-CIRCLE query area / schema-violating filter → 400
        INVALID_ARGUMENT; out-of-range coordinate or radius < 1 → 400 OUT_OF_RANGE.
        POLYGON query areas a documented cut. **Completes Dedicated Network — Areas
        vwip.**
- [~] Other CAMARA APIs as capacity allows
  - [~] Edge Application Management vwip (`/edge-application-management/vwip`;
    CAMARA EdgeApplicationManagement `wip` — no released version, mounted at its
    canonical `vwip` base path like the other unreleased EdgeCloud APIs; the
    management counterpart to the EdgeCloud discovery APIs):
    - [x] `GET /edge-cloud-zones` (`getEdgeCloudZones`,
      `edge-application-management:edge-cloud-zones:read`) — the read-only
      zone-catalog leg. Serves a fixed, in-memory 6-entry `EdgeCloudZone` catalog
      (reusing the EdgeCloud family's zone naming), each a strict RFC 4122 v5
      `edgeCloudZoneId` (SHA-256 of the zone name, version/variant nibbles forced
      to satisfy the schema pattern; no new dep). Two control planes (DESIGN §7),
      both query filters (no device identifier → no reserved-error plane, a pure
      catalog like QoS Profiles): the exact-match `region` filter (unknown region
      → empty array, a list never 404s) and the `status` filter (`active`/
      `inactive`/`unknown`; unknown value → 400 INVALID_ARGUMENT). Both combine
      (AND); neither → the whole catalog. `x-correlator` echoed. The canonical
      `EdgeCloudZones` `minItems: 1` is relaxed so a filter narrowing to zero
      returns `[]` (documented deviation).
    - [x] the stateful `apps` resource (CRUD complete: submit/get/list/delete):
      - [x] `POST /apps` (`submitApp`,
        `edge-application-management:apps:write`) — the first **stateful** leg.
        Onboards an application from a submitted `AppManifest`, mints a
        UUID-shaped `appId` and persists the manifest in a new in-memory store
        (`src/apis/edge_application_management/store.rs`; `Mutex<HashMap>`, lock
        never held across await, mirroring the blockchain/QoD stores), `201
        {appId}` (`SubmittedApp`). Two control planes (DESIGN §7): request
        validation (missing/blank required field — name/version/appProvider/
        packageType/appRepo/requiredResources/componentSpec — bad `name` pattern
        `^[A-Za-z][A-Za-z0-9_]{1,63}$`, unknown `packageType`, empty
        `componentSpec`, or malformed JSON → 400 INVALID_ARGUMENT); and store
        state — the `appId` is derived deterministically from the app identity
        (`name`+`version`+`appProvider`, RFC 4122 v5 UUID via SHA-256, no new
        dep), so re-submitting the same app collides → 409 `ALREADY_EXISTS`.
        Nested `requiredResources` `oneOf` / `appRepo` / `componentSpec` item
        shapes validated for presence/non-emptiness only (documented cut).
        `x-correlator` echoed.
      - [x] `GET /apps/{appId}` (`getApp`,
        `edge-application-management:apps:read`) — reads an onboarded app back
        as the CAMARA `AppManifestInfo` (the stored `AppManifest` + the minted
        `appId`). Keyed only on store state (opaque UUID `appId`, no
        reserved-suffix plane): known id → `200 AppManifestInfo`; unknown *or
        malformed* id → `404 NOT_FOUND` (400 malformed-path folded into 404,
        mirroring `readAccess`). `x-correlator` echoed.
      - [x] `GET /apps` (`getApps`, `edge-application-management:apps:read`) —
        lists every onboarded app as an array of `AppManifestInfo` (the store
        snapshot; empty array when none — a list never 404s). Store the only
        control plane; mirrors the sibling list legs (`listAccesses`,
        `retrievePayments`) by returning the same shape as `getApp`.
        `x-correlator` echoed.
      - [x] `DELETE /apps/{appId}` (`deleteApp`,
        `edge-application-management:apps:delete`) — de-boards an onboarded app,
        evicting its stored `AppManifest` (new `store::remove`): present → `204
        No Content` (single-use), unknown/already-deleted/malformed id → `404
        NOT_FOUND`. Keyed only on store state (opaque minted `appId`, no
        reserved-suffix plane, mirroring `getApp`/`deleteNetwork`/`deleteAccess`);
        synchronous `204` (async `202`/`DELETE_REQUESTED` + `sink` notification a
        documented cut). **Completes the `apps` CRUD** (`app-instances`/
        `deployments` remain). `x-correlator` echoed. No new dep.
    - [~] the stateful `app-instances` resource:
      - [x] `POST /app-instances` (`createAppInstance`,
        `edge-application-management:instances:write`) — instantiates an
        onboarded app onto an edge cloud zone, mints a UUID `appInstanceId`,
        persists the rendered `AppInstanceInfo` in a new in-memory store
        (`src/apis/edge_application_management/instance_store.rs`; `Mutex<HashMap>`,
        no new dep), and returns `202 Accepted` + a `Location` header (CAMARA
        models instantiation as async). Three control planes (DESIGN §7): request
        validation (missing/blank/invalid `name`, missing or non-UUID `appId`/
        `edgeCloudZoneId`/`kubernetesClusterRef`, or non-JSON body → 400
        INVALID_ARGUMENT); a cross-reference against the two in-memory stores —
        the `appId` must be an onboarded app (its `appProvider` is echoed) and the
        `edgeCloudZoneId` must name a catalog zone, else → 404 NOT_FOUND; and store
        state — the `appInstanceId` is derived deterministically from the
        `(appId, edgeCloudZoneId)` pair, so re-instantiating the same app on the
        same zone collides → 409 ALREADY_EXISTS. Second plane: `status` follows the
        target zone's own catalog `edgeCloudZoneStatus` (active→`ready`,
        inactive→`failed`, unknown→`instantiating`). Cuts: `componentEndpointInfo`
        (no live workload), the `terminating`/`unknown` instance states (unreachable
        on create), and the `subscriptionRequest` callback (accepted-not-applied).
        `x-correlator` echoed. No new dep.
      - [x] `getAppInstance` (`GET /app-instances/{appInstanceId}`,
        `edge-application-management:instances:read`) / `getAppInstances`
        (`GET /app-instances`, same scope) / `deleteAppInstance`
        (`DELETE /app-instances/{appInstanceId}`,
        `edge-application-management:instances:delete`) — the read/list/delete
        legs of the app-instance resource, mirroring the `apps` CRUD. Keyed only
        on the in-memory instance store (opaque minted `appInstanceId`, no
        reserved-suffix plane): `getAppInstance` → `200` the stored
        `AppInstanceInfo` verbatim / `404 NOT_FOUND` (malformed folded into 404);
        `getAppInstances` → `200` array (empty when none, a list never 404s);
        `deleteAppInstance` → `204` single-use / `404`. Synchronous delete (async
        `202`/`DELETE_REQUESTED` + `sink` a documented cut). `x-correlator`
        echoed. New `instance_store::all`/`remove` (+ un-gated `get`); no new dep.
        **Completes the `app-instances` CRUD** (`deployments`/`clusters` remain).
    - [x] `GET /clusters` (`getClusters`,
      `edge-application-management:clusters:read`) — read-only Kubernetes-cluster
      catalog leg (fixed 4-entry in-memory catalog, mirroring `getEdgeCloudZones`).
      Two control planes (DESIGN §7; no device identifier → no reserved-error
      plane): the `region`/`clusterRef`/`edgeCloudZoneId` query filters narrow the
      catalog (AND; no match → `[]`), and each cluster is hosted in one of the
      fixed `edge-cloud-zones` so its `edgeCloudZoneId` cross-references the zone
      catalog and `edgeCloudRegion` is inherited. `clusterRef`/`edgeCloudZoneId`
      are strict UUIDs (malformed → 400 INVALID_ARGUMENT); `region` free text
      (unknown → `[]`); unknown query key → 400. Each `ClusterInfo` carries a
      fixed representative `nodePools` entry (no live orchestrator — documented
      cut). `clusterRef` is a stable RFC 4122 v5 UUID (SHA-256, no new dep).
      `x-correlator` echoed.
    - [~] the stateful `deployments` resource:
      - [x] `POST /deployments` (`createAppDeployment`,
        `edge-application-management:deployments:write`) — deploys an onboarded
        app across one or more edge cloud zones, mints a UUID `appDeploymentId`,
        persists the rendered `AppDeploymentInfo` in a new in-memory store
        (`src/apis/edge_application_management/deployment_store.rs`;
        `Mutex<HashMap>`, no new dep, mirroring the app/instance stores), returns
        `202 Accepted` + `{appDeploymentId}` + `Location`. Three control planes
        (DESIGN §7): request validation (missing/invalid `appDeploymentName`,
        missing/non-UUID `appId`, missing/empty/>100/non-UUID `edgeCloudZones`,
        non-UUID `kubernetesClusterRefs`, or non-JSON body → 400 INVALID_ARGUMENT);
        a cross-reference against the app + zone stores (unknown `appId` or any
        non-catalog `edgeCloudZones` entry → 404 NOT_FOUND); and store state — the
        `appDeploymentId` is derived from the `(appId, appDeploymentName, sorted
        edgeCloudZones)` identity (RFC 4122 v5, SHA-256), so re-deploying the same
        app+name across the same zone *set* (any order) collides → 409
        ALREADY_EXISTS. The persisted `AppDeploymentInfo` lists one `appInstances`
        id per zone (same derivation as `createAppInstance`); the individual
        `AppInstanceInfo` resources are not separately materialised (documented
        cut), and `subscriptionRequest` is accepted-not-applied. `x-correlator`
        echoed.
      - [x] `GET /deployments/{appDeploymentId}` (`getAppDeployment`,
        `edge-application-management:deployments:read`) — reads a created
        deployment back from `deployment_store`. The `appDeploymentId` is an
        opaque, server-minted UUID → store state is the sole control plane
        (DESIGN §7): a stored id → `200` with the persisted `AppDeploymentInfo`
        verbatim, any other id (never created / already deleted / malformed) →
        `404 NOT_FOUND` (the canonical 400 malformed-path case folded into 404,
        mirroring `getApp`/`getAppInstance`). `x-correlator` echoed. No new dep.
      - [x] `GET /deployments` (`getAppDeployments`) + `DELETE
        /deployments/{appDeploymentId}` (`deleteAppDeployment`) — the list/delete
        legs, keyed only on store state (opaque server-minted `appDeploymentId`, no
        reserved-suffix plane, mirroring the `apps`/`app-instances` list/delete
        legs). List → `200` array of `AppDeploymentInfo` (empty when none — a list
        never 404s), the same shape as the single read; scope
        `edge-application-management:deployments:read`. Delete → `204 No Content`
        (single-use) / `404 NOT_FOUND` (malformed path folded into 404); scope
        `edge-application-management:deployments:delete`; synchronous delete (async
        `202`/`DELETE_REQUESTED` + `sink` a documented cut). New
        `deployment_store::all`/`remove`; `x-correlator` echoed. No new dep.
      - [x] `PATCH /deployments/{appDeploymentId}` (`updateAppDeployment`,
        `edge-application-management:deployments:update`) — in-place update via
        JSON Merge Patch (RFC 7396, `application/merge-patch+json`): only present
        fields change, arrays replaced wholesale, `null` = no-op, `appId`
        immutable. Four control planes (DESIGN §7): request validation → 400
        INVALID_ARGUMENT; unknown/malformed id → 404 (malformed path folded);
        effective `edgeCloudZones` cross-ref against the catalog → 404; and the
        patched `(appId, name, sorted zones)` identity colliding with a *different*
        stored deployment → 409 ALREADY_EXISTS (mirrors `createAppDeployment`). On
        success → 200 `AppDeploymentInfo` (appDeploymentId unchanged, appInstances
        re-derived per effective zone). New `deployment_store::update` (atomic
        check-and-replace). No new dep. **Completes the deployments resource.**

## Cross-cutting (do alongside the item that needs it)
- [~] `errors.rs`: base CAMARA error model done (`src/errors.rs`, `specs/shared/errors.yaml`); per-version catalogs still TODO (DESIGN §8)
- [x] `registry.rs`: canonical URL versioning + `/` catalog wiring (DESIGN §9).
  All three §9 discovery endpoints served: `GET /` (catalog), `GET
  /{api}/v{n}/openapi.yaml` (spec), `GET /{api}/v{n}/docs` (human-readable Redoc
  page per spec). **The two hand-maintained parallel lists are now one**:
  `src/registry.rs` holds `APIS` (name/version/embedded spec body) as the single
  source of truth, and both the `/` catalog (`main::catalog`) and the spec-serving
  routes (`apis::openapi`) are derived from it, so they cannot drift. Adding an API
  is now one registry entry (+ its `routes()` merge) instead of two lockstep edits.
- [~] `specs/…`: vendor + annotate OpenAPI per API/version, serve at `/{api}/v{n}/openapi.yaml`
  — **serving done** (`src/apis/openapi.rs`: every mounted API's spec at
  `/{api}/v{n}/openapi.yaml`, plus `/auth/openapi.yaml` + `/shared/errors.yaml` so
  `$ref`s resolve; catalog advertises each `spec_url`). Per-API *vendoring/annotation*
  continues alongside each new API.
- [~] Contract-test harness (validate responses against vendored spec) — slices landed:
  - a registry/wiring contract test (`src/main.rs`
    `catalog_spec_urls_match_served_specs_and_resolve`) asserts the `/` catalog's
    `spec_url` set exactly equals the served API-spec set (new
    `apis::openapi::api_spec_urls`, single source of truth) and that every
    catalogued `spec_url` resolves as `application/yaml` through the full app, so
    catalog↔spec drift (a newly mounted API missing from the catalog, or a
    `spec_url` that 404s) fails CI.
  - a spec↔mount-path contract test (`src/registry.rs`
    `spec_server_url_matches_mounted_base_path`) asserts every embedded spec
    declares its base path in `servers[].url` as `{apiRoot}{base_path()}`, so a
    newly vendored spec that kept the CAMARA template's original server url — or an
    entry mounted at a version its own spec doesn't declare — fails CI (the served
    contract must name the path it is served at). Complements the catalog↔served
    test above, which checks *which* specs serve, not that each names its own path.
  - a spec-version↔URL-version contract test (`src/registry.rs`
    `spec_info_version_matches_mounted_url_version`) asserts every embedded spec's
    declared `info.version` agrees with the version segment it is mounted at
    (`vwip`↔`wip`; `v0.N`↔`0.N.*`; `vN`↔`N.*`; a pre-release `…alpha…`/`…rc…`
    segment↔a pre-release version), so a spec vendored/copy-pasted with a stale
    `info.version` — or bumped upstream to a new major while still mounted at the
    old `v{n}` — fails CI. Complements the `servers[].url` test: that proves a
    spec *names* its mount path, this proves its declared version *is* that
    version. Two pure helpers (`info_version` extractor, `url_version_agrees`) are
    unit-covered so the contract test can't pass vacuously.
  - a specs↔registry parity contract test (`src/registry.rs`
    `every_vendored_spec_on_disk_is_registered`) walks the on-disk `specs/` tree
    and asserts the set of vendored `specs/<name>/<version>/openapi.yaml` files
    exactly equals the set of registered `APIS` spec paths (excluding the shared
    `shared/`+`auth/` `$ref` building blocks). Closes the drift the compile-time
    `include_str!` can't: that fails the build only for a *registered* API whose
    file is missing (registry→file); a spec vendored on disk but never added to
    `APIS` compiles fine and is silently never mounted or served. Now both
    directions fail CI.
  - an operationId-uniqueness contract test (`src/registry.rs`
    `operation_ids_are_unique_within_each_spec`) asserts every mounted spec
    declares ≥1 `operationId` and none repeats *within* that document — the
    OpenAPI structural rule that an operation's canonical name is unique per
    doc. A new endpoint's spec is usually drafted by copy-pasting an operation
    from a sibling API, so a pasted `operationId` left unrenamed yields two
    operations sharing an id — an invalid document the mount-path/version/parity
    tests can't see (they check a spec's identity, never that its operation
    *names* are well-formed). Uniqueness is scoped per spec (the same id, e.g.
    `createSubscription`, legitimately recurs across APIs). A pure `operation_ids`
    extractor (no YAML dep; scans for the `operationId:` key, skips prose that
    merely mentions it) is unit-covered so the contract can't pass vacuously.
  - a shared-security-scheme contract test (`src/registry.rs`
    `every_spec_refs_the_shared_camara_oauth_scheme`) asserts every mounted spec
    defines its `openId` security scheme by `$ref`-ing the single shared
    `auth/openapi.yaml#/components/securitySchemes/camaraOAuth` definition, not
    inline. The OAuth/OIDC scheme (its `openIdConnectUrl` → this server's discovery
    doc) is maintained once; a spec that copies it inline is a drift-prone
    duplicate whose type/url/description can diverge from the source of truth and
    from what the resource server enforces — a drift the mount-path/version/parity/
    operationId tests can't see (they check a spec's identity, never how it wires
    auth). Converged the one remaining inline spec (`iot-sim-fraud-prevention/vwip`)
    onto the shared `$ref` in the same pass so the invariant holds across all specs.
  - a functional-cases contract test (`src/registry.rs`
    `every_spec_documents_functional_cases`) asserts every mounted spec declares
    ≥1 `x-camarasim-scenarios` block, so a newly vendored spec drafted from a
    CAMARA template can't ship with its parameter-driven behaviour documented in
    prose only (DESIGN §7, §9) — a drift the identity/wiring tests can't see. A
    pure `scenario_blocks` counter (no YAML dep; matches the key at any indent,
    skips prose mentions) is unit-covered so the contract can't pass vacuously.
    Closed the one gap the test surfaced in the same pass: `iot-sim-fraud-prevention/
    vwip` (its `query`/`bindDeviceImei`/`unBindDeviceImei` operations) now carry
    structured scenarios blocks matching their 56 siblings.
  - a canonical-shared-ref contract test (`src/registry.rs`
    `shared_fragment_refs_use_the_canonical_relative_path`) asserts every
    cross-file `$ref` a mounted spec makes to the two shared fragments — the
    error model (`shared/errors.yaml`) and the auth scheme (`auth/openapi.yaml`)
    — uses the canonical relative path `../../shared/errors.yaml` /
    `../../auth/openapi.yaml`. That is the only form that resolves once the spec
    is served: a spec at `/{name}/{version}/openapi.yaml` resolves its `$ref`s
    relative to that URL, and the server serves the fragments only at
    `/shared/errors.yaml` and `/auth/openapi.yaml`. A bare `errors.yaml#…`
    (a CAMARA-template copy-paste, where the error file sits beside the spec)
    resolves to `/{name}/{version}/errors.yaml` → a 404 for any Redoc/Swagger/
    codegen client that follows the ref, leaving the served spec unresolvable —
    a drift no existing test saw (the camaraOAuth-scheme test checks only the
    `openId` *securityScheme* `$ref`, and the identity/behaviour tests never
    check that cross-file `$ref`s point at a served path). Local intra-document
    refs (`#/components/…`) carry neither fragment name, so a spec that inlines
    its own error responses is unaffected. Fixed the one real drift the survey
    found in the same pass: `number-verification/v1` (the first business API
    vendored) referenced the error model by a bare `errors.yaml#…` in all 18 of
    its response `$ref`s — converged onto `../../shared/errors.yaml#…` like its
    54 siblings. A pure `ref_targets` extractor (no YAML dep; handles both
    `$ref:` mapping keys and `- $ref:` sequence items, skips prose) is
    unit-covered so the contract can't pass vacuously.
  - a shared-error-ref-target contract test (`src/registry.rs`
    `shared_error_refs_resolve_to_defined_components`) asserts every cross-file
    `$ref` a mounted spec makes into the shared error model
    (`../../shared/errors.yaml#/components/…`) points at a component that fragment
    actually **defines**. The canonical-shared-ref test proves such a ref uses the
    one path that reaches the served fragment (the *file* half); this proves the
    JSON-pointer *into* it names a real component (the *fragment* half), so a
    client dereferencing it gets the response/schema, not a dangling pointer. The
    break it catches: a spec drafted by copy-pasting a sibling's error block picks
    a name the shared model never defines — a typo (`InvalidArguments`), a CAMARA-
    template name never adopted (`Generic404`), or a name renamed in the shared
    file after the copy — so both `$ref` halves look right yet resolve to nothing.
    The allowed set is extracted from the embedded shared fragment itself (a pure
    `component_pointers` helper, no YAML dep — scans `components:` → 2-space
    section → exact-4-space component keys, ignoring nested property/content
    lines), so adding a shared response widens it automatically and the test never
    needs editing. Verified the invariant already holds (all 10 pointers the 57
    specs use — the `CamaraError` schema + the 9 canonical responses — are
    defined). The helper is unit-covered (`component_pointer_extraction_rules`) so
    the contract can't pass vacuously.
  - a security-requirement↔definition contract test (`src/registry.rs`
    `every_security_requirement_references_a_defined_scheme`) asserts every
    `security` requirement an operation declares names a scheme the spec
    **defines** under `components.securitySchemes` (in this sim, `openId`), and
    that every spec both defines `openId` and carries ≥1 requirement (every
    business op is OAuth-protected). Complements the shared-security-scheme test,
    which checks only how `openId` is *defined* (the shared `$ref`) — this checks
    that operations *reference* a defined scheme, catching a copy-pasted CAMARA-
    template requirement that kept a scheme name the spec never declares
    (`oAuth2ClientCredentials`, `three_legged`, a typo'd `openID`) → a dangling,
    unresolvable requirement invisible to the identity/wiring tests. Two pure
    helpers — `defined_security_schemes` (reuses `component_pointers`, filtered to
    the `securitySchemes` section) and `security_requirement_schemes` (scans each
    indentation-tracked `security:` block, taking sequence items that are mapping
    keys `- openId:` and skipping scope scalars `- api:scope` by the colon-suffix
    rule) — are unit-covered (`security_scheme_extraction_rules`) so the contract
    can't pass vacuously. Verified true across all 57 mounted specs (143
    requirements, all `openId`) before asserting.
  - an openapi-root-version contract test (`src/registry.rs`
    `every_spec_declares_a_valid_openapi_3_version`) asserts every mounted spec
    declares, at its root, a valid `openapi:` version of the OpenAPI 3 family
    (`3.MINOR.PATCH`, all numeric) — the single REQUIRED root field of an OpenAPI
    document, which every Redoc/Swagger/codegen client reads first to decide how
    to interpret the rest (3.0↔3.1 differ in `nullable`/`type`). Hardens the weak
    `bodies_are_non_empty_openapi_docs` smoke check, which only asserts the body
    *contains* the substring `openapi:` anywhere — satisfied by a prose mention, a
    stale Swagger `2.0` header, or a truncated `openapi: 3.0`, none of which is a
    valid served document. A pure `openapi_version` extractor (top-level-key
    scoped, so an indented `openapi:` mention in a description isn't matched) +
    `is_openapi_3_version` validator (no YAML dep) are unit-covered so the contract
    can't pass vacuously. Verified true (all `3.0.3`) across all mounted specs.
  - a local-component-ref-resolution contract test (`src/registry.rs`
    `local_component_refs_resolve_within_their_own_spec`) asserts every
    *intra-document* `$ref` a mounted spec makes — a local pointer
    `#/components/<section>/<Name>` (empty file half) — points at a component that
    same document **defines**. The complementary half of the shared-error-ref test:
    that dereferences a spec's *cross-file* pointers into the shared error fragment;
    this dereferences a spec's *own* local pointers against its own `components:`.
    Catches the copy-paste drift where a pasted `$ref: '#/components/schemas/Foo'`
    names a schema/response/parameter/header this document never declares (renamed
    after the copy, or only ever in the sibling) → a dangling pointer that leaves
    the served spec unresolvable — invisible to the shared-error-ref test (it skips
    local refs) and the identity/wiring tests. Reuses the unit-covered `ref_targets`
    + `component_pointers` helpers; restricted to exact 2-segment component
    pointers (the granularity `component_pointers` resolves; every local ref these
    specs make has that shape). Verified true (0 dangling local refs across all 57
    mounted specs) before asserting.
  - a path-templating contract test (`src/registry.rs`
    `path_template_params_match_declared_path_parameters`) asserts, both ways, that
    every `{name}` a spec puts in a `paths:` key is declared as an `in: path`
    parameter, and every `in: path` parameter it declares appears in some path
    template. Both are OpenAPI structural rules — an undeclared path variable, or a
    path parameter that templates nothing, is an invalid document — and a live
    copy-paste drift: a path block pasted from a sibling can keep its `{sessionId}`
    template while the operation declares a `paymentId` path param (or a path is
    renamed but its parameter isn't), a mismatch the identity/wiring/`$ref` tests
    never see. Two pure helpers — `path_template_params` (scans `paths:` keys,
    pulls `{…}` spans; a `{…}` in prose is excluded) and
    `declared_path_parameter_names` (credits each `in: path` its own object's
    `name`, handling name-first/in-first order, dash-sequence and bare
    `components.parameters` mapping forms, bounded so an adjacent sibling's name is
    never miscredited) — are unit-covered
    (`path_parameter_extraction_rules`) so the contract can't pass vacuously.
    Verified true across all 19 path-templating specs (every variable declared,
    every path param used) before asserting.
  - a required-`responses` contract test (`src/registry.rs`
    `every_operation_declares_a_responses_object`) asserts every operation a
    mounted spec declares carries a `responses` object — the **single REQUIRED
    field** of an OpenAPI Operation Object (summary/operationId/parameters are all
    optional), so an operation without one is an invalid document (a Redoc/Swagger/
    codegen client is handed an operation with no declared outcome). Catches the
    copy-paste drift where a pasted/edited operation block loses or dedents its
    `responses:` — a break the identity/wiring/path-templating/`$ref` tests never
    see (they check a spec's identity, wiring, or path variables, never that each
    operation declares its responses). A pure `operations_without_responses`
    extractor (no YAML dep; scopes method keys to under a `paths:` path item so an
    HTTP verb used as a schema property name isn't mistaken for an operation, and
    credits a `responses:` only to the operation whose indented block it sits in)
    is unit-covered (`operations_without_responses_extraction_rules`, incl. a
    non-vacuous floor over all specs) so the contract can't pass vacuously.
    Verified true (142 operations across all mounted specs, none missing) before
    asserting.
  - an every-operation-has-an-`operationId` contract test (`src/registry.rs`
    `every_operation_declares_an_operation_id`) asserts every operation a mounted
    spec declares carries an `operationId`. OpenAPI marks it optional, but **CAMARA
    mandates** one on every operation — it is the operation's canonical name, the
    method a codegen client derives, and the key each handler/scope narrative is
    written against. Closes the gap the sibling
    `operation_ids_are_unique_within_each_spec` leaves: that pins the *other* half
    of the operationId contract (≥1 per spec, none repeated **within** a document)
    but a spec with two operations sharing an id and a third with none still passes
    it. Catches the copy-paste drift where a pasted/edited operation block loses its
    `operationId:` line, leaving an anonymous operation codegen names arbitrarily —
    invisible to the required-`responses` test (checks the one REQUIRED field) and
    the identity/wiring/path-templating/`$ref` tests. A pure
    `operations_without_operation_id` extractor (no YAML dep; mirrors
    `operations_without_responses`' path-item/method scoping, but matches the
    `operationId` **key name** before its inline-value colon rather than a whole
    trimmed line) is unit-covered (`operations_without_operation_id_extraction_rules`,
    incl. a non-vacuous floor over all specs) so the contract can't pass vacuously.
    Verified true (every operation across all 57 mounted specs carries an
    operationId) before asserting.
  - a slash-prefixed-path-items contract test (`src/registry.rs`
    `every_paths_object_declares_slash_prefixed_path_items`) asserts every mounted
    spec declares **≥1 path item** and that **every `paths:` key begins with `/`**
    — a `paths` object maps URL path *templates* (resolved relative to the server
    url) to Path Item Objects, so a non-slash key is an invalid document a
    client/codegen tool can't bind. Closes two vacuous-pass gaps: every
    operation-scoped test (`operations_without_responses`,
    `operations_without_operation_id`, `path_template_params_…`) treats only a
    2-space key that *already* starts with `/` as a path item, so a path key that
    lost its leading slash (copy-paste/edit) contributes zero operations and every
    one of them passes it silently; and no test asserted a spec declares any path
    at all (an empty `paths:` block described nothing yet sailed through). A pure
    `path_item_keys` extractor (no YAML dep; 2-space direct children of the
    top-level `paths:` block, unquoting a `"/foo":` key and excluding `x-`
    Paths-Object extensions) is unit-covered (`path_item_key_extraction_rules`,
    incl. a non-vacuous floor over all specs) so the contract can't pass vacuously.
    Verified true (114 path items across all 57 mounted specs, all slash-prefixed)
    before asserting.
  Full response-vs-schema validation still TODO (would need a YAML/JSON-Schema
  validator — a dependency trade-off, deferred).

---

## Scan journal

Newest first. One line per pass: `YYYY-MM-DD HH:MMZ — <what happened> — binary: <size>`

- 2026-08-11 — contract-harness: added a **slash-prefixed-path-items** contract
  test (`src/registry.rs` `every_paths_object_declares_slash_prefixed_path_items`)
  asserting every mounted spec declares ≥1 path item and that every `paths:` key
  begins with `/` — a `paths` object maps URL path *templates* (resolved relative
  to the server url) to Path Item Objects, so a key without a leading slash is an
  invalid document a Redoc/Swagger/codegen client can't bind. Feature-API backlog
  stays effectively exhausted (remaining `[ ]` leaves are `https://` sink-TLS cases
  needing a multi-MB rustls client vs the small-binary directive, and open-ended
  state streams with no live worker), so this advanced the cross-cutting
  **contract-test harness** item. Closes two vacuous-pass gaps no existing test
  sees: every operation-scoped test (`operations_without_responses`,
  `operations_without_operation_id`, `path_template_params_…`) treats only a
  2-space key that *already* starts with `/` as a path item, so a path key that
  lost its leading slash in a copy-paste/edit contributes zero operations and each
  of those tests passes it silently (no ops found → nothing missing); and nothing
  asserted a spec declares any path at all, so an empty `paths:` block describing
  nothing would sail through the whole harness. One pure helper `path_item_keys`
  (no YAML dep; 2-space direct children of the top-level `paths:` block, unquotes a
  `"/foo":` key, excludes `x-` Paths-Object extensions, and — mirroring
  `path_template_params`' scoping — never mistakes a deeper method key or a
  `/`-looking schema property for a path item) is unit-covered
  (`path_item_key_extraction_rules`, incl. a non-vacuous floor over all specs) so
  the contract can't pass vacuously. Verified true (114 path items across all 57
  mounted specs, all slash-prefixed, each spec ≥1) before asserting. Tests: +2
  registry (1 contract + 1 helper unit). `cargo test` 2148 green (was 2146),
  `cargo build --release` warning-clean. No new dep; binary unchanged
  (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added an **every-operation-has-an-`operationId`**
  contract test (`src/registry.rs` `every_operation_declares_an_operation_id`)
  asserting every operation a mounted spec declares carries an `operationId`.
  OpenAPI marks it optional but CAMARA mandates one on every operation (the
  operation's canonical name — the codegen method name, and the key each simulator
  handler/scope narrative is written against). Closes the gap the sibling
  `operation_ids_are_unique_within_each_spec` leaves: that pins ≥1 per spec + no
  duplicate *within* a doc, but a spec with two ops sharing an id and a third with
  none passes it (two distinct ids, no dup). Feature-API backlog stays effectively
  exhausted (remaining `[ ]` leaves are `https://` sink-TLS cases needing a
  multi-MB rustls client vs the small-binary directive, and open-ended state
  streams with no live worker), so this advanced the cross-cutting **contract-test
  harness** item. Closes a live copy-paste drift no existing test sees: an operation
  block pasted from a sibling can lose its `operationId:` line (an anonymous op
  codegen names arbitrarily) — the required-`responses`/path-templating/version/
  parity/`$ref` tests check the one REQUIRED field, path variables, identity, or
  wiring, never that every op is named. One pure helper
  `operations_without_operation_id` (no YAML dep; mirrors
  `operations_without_responses`' path-item/method scoping, matching the
  `operationId` key name before its inline-value colon) is unit-covered
  (`operations_without_operation_id_extraction_rules`, incl. a non-vacuous floor
  over all specs) so the contract can't pass vacuously. Verified true (every
  operation across all 57 mounted specs carries an operationId) before asserting.
  Tests: +2 registry (1 contract + 1 helper unit). `cargo test` 2146 green (was
  2144), `cargo build --release` warning-clean. No new dep; binary unchanged
  (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **required-`responses`** contract test
  (`src/registry.rs` `every_operation_declares_a_responses_object`) asserting every
  operation a mounted spec declares carries a `responses` object — the single
  REQUIRED field of an OpenAPI Operation Object (summary/operationId/parameters are
  all optional), so an operation without one is an invalid document a Redoc/Swagger/
  codegen client can't render (no declared outcome to bind). Feature-API backlog
  stays effectively exhausted (remaining `[ ]` leaves are the `https://` sink-TLS
  cases needing a multi-MB rustls client vs the small-binary directive, and
  open-ended state streams with no live worker), so this advanced the cross-cutting
  **contract-test harness** item. Closes a drift no existing test sees: a new
  endpoint's spec is drafted by copy-pasting an operation from a sibling, so a
  pasted/edited operation block can lose or dedent its `responses:` — the
  mount-path/version/parity/operationId/path-templating/`$ref` tests all check a
  spec's identity, wiring, or path variables, never that each operation declares its
  responses. One pure helper `operations_without_responses` (no YAML dep; scopes
  4-space method keys to under a `paths:` path item so an HTTP verb used as a schema
  property name isn't mistaken for an operation, and credits a 6-space `responses:`
  only to the operation whose indented block it sits in) is unit-covered
  (`operations_without_responses_extraction_rules`, incl. a non-vacuous floor over
  all specs) so the contract can't pass vacuously. Verified true (142 operations
  across all mounted specs, none missing) before asserting. Tests: +2 registry
  (1 contract + 1 helper unit). `cargo test` 2144 green (was 2142), `cargo build
  --release` warning-clean. No new dep; binary unchanged (`#[cfg(test)]`-only). —
  binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **required-`info.title`** contract test
  (`src/registry.rs` `every_spec_declares_a_non_empty_info_title`) asserting every
  mounted vendored spec declares a non-empty `info.title`. With `info.version`,
  `title` is one of the two REQUIRED fields of the OpenAPI `info` object — the human
  name every Redoc/Swagger/codegen client renders as the document heading (a spec
  without it renders "Untitled") and the label the `/` catalog / per-spec docs pages
  show. This **completes the required-`info`-field coverage**: an existing test pins
  `info.version` (`spec_info_version_matches_mounted_url_version`) and another pins the
  root `openapi:` field (`every_spec_declares_a_valid_openapi_3_version`), but nothing
  asserted the required `info.title`. Feature-API backlog stays effectively exhausted
  (remaining `[ ]` leaves are the `https://` sink-TLS cases needing a multi-MB rustls
  client vs the small-binary directive, and open-ended state streams with no live
  worker), so this advanced the cross-cutting **contract-test harness** item. Closes a
  drift no existing test sees: a spec drafted from a CAMARA template can drop or blank
  its `title:` (dropped in an edit, or left an empty scalar) — the identity/wiring
  tests (mount-path/version/parity/operationId/oauth/scenarios) all trust the document
  is a structurally complete OpenAPI doc. One pure helper `info_title` (mirrors
  `info_version`: scoped to the top-level `info:` block, 2-space direct child, unquotes;
  a JSON-Schema `title:` inside a component schema is not mistaken for it; blank scalar →
  `Some("")` distinct from a missing line → `None`) is unit-covered
  (`info_title_extraction_rules`, incl. a non-vacuous floor over all specs) so the
  contract can't pass vacuously. Verified true (all 58 specs carry a non-empty title)
  before asserting. Tests: +2 registry (1 contract + 1 helper unit). `cargo test` 2142
  green (was 2140), `cargo build --release` warning-clean. No new dep; binary unchanged
  (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **path-templating** contract test
  (`src/registry.rs` `path_template_params_match_declared_path_parameters`)
  asserting, both ways, that every `{name}` a mounted spec puts in a `paths:` key
  is declared as an `in: path` parameter and every `in: path` parameter it declares
  appears in some path template — the OpenAPI path-templating structural rules, and
  a live copy-paste drift (a pasted path block keeping a sibling's `{sessionId}`
  template while its operation declares a `paymentId` path param, or a path renamed
  without its parameter) that no existing test sees — the mount-path/version/parity/
  operationId/`$ref` tests all check a spec's identity or wiring, never that its path
  *variables* line up with its path *parameters*. Feature-API backlog stays
  effectively exhausted (remaining `[ ]` leaves are `https://` sink-TLS needing a
  multi-MB rustls client vs the small-binary directive, and open-ended state streams
  with no live worker), so this advanced the cross-cutting contract-test harness. Two
  pure helpers — `path_template_params` (scans `paths:` keys, pulls `{…}` spans,
  excludes a `{…}` in prose) and `declared_path_parameter_names` (credits each
  `in: path` its own object's `name`; handles name-first/in-first order, dash-sequence
  and bare `components.parameters` mapping forms; bounded so an adjacent sibling's name
  is never miscredited) — are unit-covered (`path_parameter_extraction_rules`, incl. a
  non-vacuous floor over all specs) so the contract can't pass vacuously. Verified true
  across all 19 path-templating specs before asserting. Tests: +2 registry (1 contract
  + 1 helper unit). `cargo test` 2140 green (was 2138), `cargo build --release`
  warning-clean. No new dep; binary unchanged (`#[cfg(test)]`-only). — binary: 3.7M
  (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **local-component-ref-resolution** contract
  test (`src/registry.rs` `local_component_refs_resolve_within_their_own_spec`)
  asserting every intra-document `$ref` a mounted spec makes (a local pointer
  `#/components/<section>/<Name>`, empty file half) points at a component that same
  document defines. Complements `shared_error_refs_resolve_to_defined_components`,
  which dereferences a spec's *cross-file* pointers into the shared error fragment;
  this dereferences a spec's *own* local pointers against its own `components:`
  block. The feature-API backlog stays effectively exhausted (remaining `[ ]` leaves
  are the `https://` sink-TLS cases needing a multi-MB rustls client vs the
  small-binary directive, and open-ended state streams with no live worker), so this
  advanced the cross-cutting contract-test harness. Closes a drift no existing test
  sees: a new endpoint's spec is drafted by copy-pasting an operation (with its
  `$ref`s) from a sibling, so a pasted `$ref: '#/components/schemas/Foo'` can name a
  schema/response/parameter/header this document never declares (renamed after the
  copy, or only ever in the sibling) → both `$ref` halves look right yet resolve to
  nothing, leaving the served spec unresolvable for any Redoc/Swagger/codegen client
  that follows it. The shared-error-ref test skips local refs (empty file half); the
  mount-path/version/parity/operationId/security tests check a spec's identity or
  wiring, never that its own local pointers resolve. Reuses the unit-covered
  `ref_targets` + `component_pointers` helpers (no new helper, no YAML dep);
  restricted to exact 2-segment component pointers — the granularity
  `component_pointers` resolves, and every local ref these specs make has that shape
  (verified: all local targets are `section/name`, no deeper pointers). Verified the
  invariant already holds (0 dangling local refs across all 57 mounted specs) before
  asserting. Tests: +1 registry contract. `cargo test` 2138 green (was 2137),
  `cargo build --release` warning-clean. No new dep; binary unchanged
  (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added an **openapi-root-version** contract test
  (`src/registry.rs` `every_spec_declares_a_valid_openapi_3_version`) asserting every
  mounted vendored spec declares, at its root, a valid `openapi:` version of the
  OpenAPI 3 family (`3.MINOR.PATCH`, all numeric). `openapi` is the single REQUIRED
  root field of an OpenAPI document — the first thing every Redoc/Swagger/codegen
  client reads to decide how to interpret the rest (3.0↔3.1 differ in `nullable`/
  `type` handling) — so a document without it, or one declaring a Swagger `2.x`
  version, is not a spec this simulator serves. The feature-API backlog stays
  effectively exhausted (remaining `[ ]` leaves are the `https://` sink-TLS cases
  needing a multi-MB rustls client vs the small-binary directive, and open-ended state
  streams with no live worker), so this advanced the cross-cutting **contract-test
  harness** item. Hardens the weak `bodies_are_non_empty_openapi_docs` smoke check,
  which only asserts the body *contains* the substring `openapi:` anywhere —
  satisfied by a prose mention inside a description, a stale Swagger `2.0` header, or
  a malformed/truncated `openapi: 3.0`, none of which is a valid served document. A
  newly vendored spec drafted from a CAMARA template can lose or mangle its root
  `openapi:` line (dropped in an edit, indented into a block, or copied from a 2.x
  source) — a drift the identity/wiring tests (mount-path/version/parity/operationId/
  oauth/scenarios) never look for, since they all trust the document is structurally
  an OpenAPI 3 doc to begin with. Verified true (all `3.0.3`) across every mounted
  spec before asserting. Two pure helpers — `openapi_version` (top-level-key scoped,
  so an indented `openapi:` in prose isn't matched; unquotes the scalar) and
  `is_openapi_3_version` (exactly `3.MINOR.PATCH`, all numeric; rejects Swagger 2.x, a
  truncated two-part `3.0`, an over-long `3.0.3.1`, non-numeric values) — are
  unit-covered (`openapi_version_extraction_and_validation_rules`) so the contract
  can't pass vacuously. Tests: +2 registry (1 contract + 1 helper unit). `cargo test`
  2137 green (was 2135), `cargo build --release` warning-clean. No new dep; binary
  unchanged (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **security-requirement↔definition** contract
  test (`src/registry.rs` `every_security_requirement_references_a_defined_scheme`)
  asserting every `security` requirement a mounted spec's operations declare names a
  security scheme the spec **defines** under `components.securitySchemes` — in this
  simulator the shared `openId` scheme — and that every spec both defines `openId` and
  carries ≥1 requirement (every CAMARA business op is OAuth-protected). The feature-API
  backlog stays effectively exhausted (remaining `[ ]` leaves are the `https://`
  sink-TLS cases needing a multi-MB rustls client vs the small-binary directive, and
  open-ended state streams with no live worker), so this advanced the cross-cutting
  **contract-test harness** item. Closes a drift no existing test sees: the
  shared-security-scheme test checks only how `openId` is *defined* (that its
  definition is the shared `$ref`), never that operations *reference* a defined
  scheme; the mount-path/version/parity/operationId/functional-cases tests all check a
  spec's identity or behaviour. A new endpoint's spec is usually drafted by
  copy-pasting an operation from a CAMARA template or sibling, so a pasted `security`
  requirement can keep a scheme name the spec never declares
  (`oAuth2ClientCredentials`, `three_legged`, a typo'd `openID`) → a dangling,
  unresolvable requirement (an operation demands a scheme its own `securitySchemes`
  omits, so a client can't tell what auth it needs and codegen breaks). Verified the
  invariant already holds across all 57 mounted specs (143 requirements, all `openId`;
  every spec defines exactly `openId`) before asserting. Two pure helpers —
  `defined_security_schemes` (reuses the unit-covered `component_pointers`, filtered to
  the `securitySchemes` section, so it needs no new parser) and
  `security_requirement_schemes` (scans each indentation-tracked `security:` block —
  so a sibling `parameters:` list's `- name:`/`- in:` items are never mistaken for
  scheme refs — and within it takes sequence items that are mapping keys `- openId:`
  while skipping scope scalars `- api:scope` by the colon-suffix rule) — are
  unit-covered (`security_scheme_extraction_rules`) so the contract can't pass
  vacuously. Tests: +2 registry (1 contract + 1 helper unit). `cargo test` 2135 green
  (was 2133), `cargo build --release` warning-clean. No new dep; binary unchanged
  (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **shared-error-ref-target** contract test
  (`src/registry.rs` `shared_error_refs_resolve_to_defined_components`) asserting
  every cross-file `$ref` a mounted spec makes into the shared error model
  (`../../shared/errors.yaml#/components/…`) points at a component that fragment
  actually **defines**. Complements the sibling canonical-shared-ref test, which
  proves such a ref uses the one relative path that reaches the served fragment
  (the *file* half); this proves the JSON-pointer *into* it names a real component
  (the *fragment* half), so a Redoc/Swagger/codegen client dereferencing it gets
  the response/schema instead of a dangling pointer. Catches the drift a
  copy-pasted error block introduces: a response name the shared model never
  defines — a typo (`InvalidArguments`), a CAMARA-template name never adopted
  (`Generic404`), or one renamed in the shared file after the copy — where both
  `$ref` halves look right yet resolve to nothing (invisible to the
  canonical-path test, which checks only the file half, and to the identity/wiring
  tests, which never dereference cross-file pointers). The feature-API backlog
  stays effectively exhausted (remaining `[ ]` leaves are the `https://` sink-TLS
  cases needing a multi-MB rustls client vs the small-binary directive, and
  open-ended state streams with no live worker), so this advanced the cross-cutting
  **contract-test harness** item. Verified the invariant already holds before
  asserting: all 10 pointers the 57 specs reference (the `CamaraError` schema + the
  9 canonical responses) are defined in `shared/errors.yaml`. The allowed set is
  extracted from the embedded fragment itself via a pure `component_pointers`
  helper (no YAML dep; scans `components:` → 2-space section → exact-4-space
  component keys, ignoring nested property/content lines), so adding a shared
  response widens it automatically and the test never needs editing; the helper is
  unit-covered (`component_pointer_extraction_rules`) so the contract can't pass
  vacuously. Tests: +2 registry (1 contract + 1 helper unit). `cargo test` 2133
  green (was 2131), `cargo build --release` warning-clean. No new dep; binary
  unchanged (`#[cfg(test)]`-only). — binary: 3.7M (3851880 B, +0 B)
- 2026-08-11 — contract-harness: added a **canonical-shared-ref** contract test
  (`src/registry.rs` `shared_fragment_refs_use_the_canonical_relative_path`)
  asserting every cross-file `$ref` a mounted spec makes to the two shared
  fragments (`shared/errors.yaml`, `auth/openapi.yaml`) uses the canonical
  relative path (`../../shared/errors.yaml` / `../../auth/openapi.yaml`) — the
  only form that resolves once the spec is served (a spec at
  `/{api}/v{n}/openapi.yaml` resolves `$ref`s relative to that URL, and the
  fragments are served only at `/shared/errors.yaml` + `/auth/openapi.yaml`).
  The feature-API backlog stays effectively exhausted (remaining `[ ]` leaves
  are the `https://` sink-TLS cases needing a multi-MB rustls client vs the
  small-binary directive, and open-ended state streams with no live worker), so
  this advanced the cross-cutting **contract-test harness** item. Surfaced and
  fixed a **real served-spec bug** in the same pass: `number-verification/v1`
  (the first business API vendored) referenced the error model by a bare
  `errors.yaml#…` in all 18 of its response `$ref`s — resolving to
  `/number-verification/v1/errors.yaml`, which the server never serves, so a
  Redoc/Swagger/codegen client following the refs got a 404 and the served spec
  was unresolvable (contradicting `apis::openapi`'s documented "every served
  spec is fully resolvable" invariant). Converged all 18 onto
  `../../shared/errors.yaml#…` like its 54 siblings; the served bytes now
  resolve. This drift was invisible to every prior contract test (the
  camaraOAuth-scheme test checks only the `openId` securityScheme `$ref`; the
  mount-path/version/parity/operationId/functional-cases tests check a spec's
  identity or behaviour, never that its cross-file `$ref`s point at a served
  path). A pure `ref_targets` extractor (no YAML dep; handles both `$ref:`
  mapping keys and `- $ref:` sequence items, skips prose mentions) is
  unit-covered so the contract can't pass vacuously. Tests: +2 registry (1
  contract + 1 helper unit). `cargo test` 2131 green (was 2129), `cargo build
  --release` warning-clean. No new dep; binary grew by the widened embedded
  `$ref` path bytes only. — binary: 3.7M (3851880 B, +256 B)
- 2026-08-11 — contract-harness: added a **functional-cases** contract test
  (`src/registry.rs` `every_spec_documents_functional_cases`) asserting every
  mounted vendored spec declares ≥1 structured `x-camarasim-scenarios` block —
  the machine-readable record of each API's parameter-driven behaviour (DESIGN
  §7, §9), a drift the identity/wiring tests (mount-path/version/parity/
  operationId/oauth) can't see. The feature-API backlog stays effectively
  exhausted (remaining `[ ]` leaves are the `https://` sink-TLS cases needing a
  multi-MB rustls client vs the small-binary directive, and open-ended state
  streams with no live worker), so this advanced the cross-cutting **contract-test
  harness** item. Closed the one gap it surfaced in the same pass: 56/57 mounted
  specs carried the block, but `iot-sim-fraud-prevention/vwip` documented its
  functional cases in prose only. Added structured scenarios to all three of its
  operations (`query`, `bindDeviceImei`, `unBindDeviceImei`), transcribed from the
  verified handler behaviour (identifier resolution + reserved-error + store +
  trailing-digit-parity planes across the IMEIBIND/AREALIMIT facets). Fixed a
  stale module-header doc-comment in `src/apis/iot_sim_fraud_prevention/vwip.rs`
  that still claimed `AREALIMIT` bind/unbind was deferred/rejected 400 while the
  code (and per-symbol docs + spec) fully implement both facets — no behaviour
  change. A pure `scenario_blocks` counter is unit-covered so the contract can't
  pass vacuously. Tests: +2 (1 contract, 1 unit). `cargo test` 2129 green (was
  2127), `cargo build --release` warning-clean. No new dep; binary grew by the
  embedded scenario-block bytes only. — binary: 3.7M (3851624 B, +5952 B)
- 2026-08-11 — contract-harness: added a **shared-security-scheme** contract test
  (`src/registry.rs` `every_spec_refs_the_shared_camara_oauth_scheme`) asserting
  every mounted vendored spec defines its `openId` security scheme by `$ref`-ing the
  single shared `auth/openapi.yaml#/components/securitySchemes/camaraOAuth`, not
  inline. The feature-API backlog stays effectively exhausted (remaining `[ ]`
  leaves are the `https://` sink-TLS cases needing a multi-MB rustls client vs the
  small-binary directive, and open-ended state streams with no live worker), so this
  advanced the cross-cutting **contract-test harness** item. Found and fixed a real
  drift while surveying: 56/57 mounted specs reference the shared `camaraOAuth`
  scheme, but `iot-sim-fraud-prevention/vwip` defined `openId` inline — a drift-prone
  duplicate whose `type`/`openIdConnectUrl`/description can diverge from the single
  source of truth (and from what the resource server enforces), invisible to the
  mount-path/version/parity/operationId tests (they check a spec's identity, never
  how it wires auth). Converged that spec onto the shared `$ref` (same relative depth
  as its 56 siblings) in the same pass, so the invariant holds across all specs. No
  behaviour change — the security *requirement* per operation is unchanged; only the
  scheme *definition* moved from a local copy to the shared reference. Tests: +1
  registry contract test. `cargo test` 2127 green (was 2126), `cargo build --release`
  warning-clean. No new dep; binary shrank slightly (the inline block bytes dropped
  from the embedded spec). — binary: 3.7M (3845672 B, −64 B)
- 2026-08-11 — contract-harness: added an **operationId-uniqueness** contract test
  (`src/registry.rs` `operation_ids_are_unique_within_each_spec`) asserting every
  mounted vendored spec declares ≥1 `operationId` and none repeats within that
  document — the OpenAPI rule that an operation's canonical name is unique per
  doc, which the simulator keys handlers/scope narrative to. The feature-API
  backlog stays effectively exhausted (remaining `[ ]` leaves are the `https://`
  sink-TLS cases needing a multi-MB rustls client vs the small-binary directive,
  and open-ended state streams with no live worker), so this advanced the
  cross-cutting **contract-test harness** item. Closes a real drift the existing
  tests can't see: the mount-path / info.version / specs↔registry-parity tests
  check a spec's *identity*, never that its operation *names* are well-formed —
  and a new endpoint's spec is usually drafted by copy-pasting an operation from a
  sibling API, so a pasted `operationId` left unrenamed yields two operations
  sharing an id, an invalid document that all pass today. Uniqueness is scoped
  per spec on purpose (the same id, e.g. `createSubscription`, legitimately
  recurs across different APIs). Verified the invariant already holds across all
  57 mounted specs (142 operationIds, 0 within-spec dups) before asserting it. A
  pure `operation_ids` extractor (test-only; no YAML dep — scans for the
  `operationId:` key, strips quotes, skips prose that merely mentions the word) is
  unit-covered (`operation_id_extraction_rules`) so the contract can't pass
  vacuously. Tests: +2 registry (1 contract + 1 helper unit). `cargo test` 2126
  green (was 2124), `cargo build --release` warning-clean. No new dep; binary
  unchanged (`#[cfg(test)]`-only). — binary: 3.7M (3845736 B, +0 B)
- 2026-08-11 — contract-harness: added a **specs↔registry parity** contract test
  (`src/registry.rs` `every_vendored_spec_on_disk_is_registered`) that walks the
  on-disk `specs/` tree and asserts the set of vendored
  `specs/<name>/<version>/openapi.yaml` files exactly equals the set of registered
  `APIS` spec paths (excluding the shared `shared/`+`auth/` `$ref` building
  blocks). The feature-API backlog stays effectively exhausted (remaining `[ ]`
  leaves are the `https://` sink-TLS cases needing a multi-MB rustls client vs the
  small-binary directive, and open-ended state streams with no live worker), so
  this advanced the cross-cutting **contract-test harness** item. Closes the one
  drift direction the existing tests can't see: the compile-time `include_str!`
  fails the build only for a *registered* API whose spec file is missing
  (registry→file), but a spec vendored on disk yet never added to `APIS` compiles
  fine and is silently never mounted or served (file→registry). Reconciled the two
  sets before asserting (57 on disk = 57 registered, exact match). Test-only walk
  via `std::fs` + `env!("CARGO_MANIFEST_DIR")` (no request-path I/O, no new dep);
  the set-difference assertion is non-vacuous (it builds `on_disk` from a real
  directory walk). Tests: +1 registry contract. `cargo test` 2124 green (was
  2123), `cargo build --release` warning-clean. No new dep; binary unchanged
  (`#[cfg(test)]`-only). — binary: 3.7M (3845736 B, +0 B)
- 2026-08-11 — contract-harness: added a **spec-version↔URL-version** contract
  test (`src/registry.rs` `spec_info_version_matches_mounted_url_version`)
  asserting every embedded vendored spec's declared `info.version` agrees with the
  version segment the API is mounted at (`vwip`↔`wip`; `v0.N`↔`0.N.*`; `vN`↔`N.*`;
  a pre-release `…alpha…`/`…rc…` segment↔a pre-release version). The feature-API
  backlog stays effectively exhausted (remaining `[ ]` leaves are the `https://`
  sink-TLS cases needing a multi-MB rustls client vs the small-binary directive,
  and open-ended state streams with no live worker), so this advanced the
  cross-cutting **contract-test harness** item. Closes a real drift the existing
  tests can't see: the catalog↔served test checks *which* specs serve, the
  `servers[].url` test checks a spec *names* its mount path — neither checks the
  spec's declared semantic version *is* the version it is mounted at, so a spec
  vendored/copy-pasted with a stale `info.version`, or bumped upstream to a new
  major while still mounted at the old `v{n}`, would pass today. Verified the
  invariant already holds across all 57 mounted specs before asserting it. Two
  pure helpers (`info_version` extractor — scoped to the `info:` block, no YAML
  dep — and `url_version_agrees`) are unit-covered so the contract test can't pass
  vacuously (test-only; no vendored spec or server-code change, so no
  behaviour/spec drift). Tests: +2 registry (1 contract + 1 helper unit).
  `cargo test` 2123 green (was 2121), `cargo build --release` warning-clean. No
  new dep; binary unchanged (`#[cfg(test)]`-only). — binary: 3.7M (3845736 B, +0 B)
- 2026-08-11 — contract-harness: added a **spec↔mount-path** contract test
  (`src/registry.rs` `spec_server_url_matches_mounted_base_path`) asserting every
  embedded vendored spec advertises its base path in `servers[].url` as
  `{apiRoot}{base_path()}` — the path the router actually mounts it at. The
  feature-API backlog stays effectively exhausted (remaining `[ ]` leaves are the
  `https://` sink-TLS cases needing a multi-MB rustls client vs the small-binary
  directive, and open-ended state streams with no live worker), so this advanced
  the cross-cutting **contract-test harness** item, closing a real drift the
  existing catalog↔served-spec tests can't see: those check *which* specs are
  served, not that a spec names the path it is served at, so a spec copy-pasted
  with the CAMARA template's original `servers` url — or mounted at a version its
  own spec doesn't declare — would pass today. Verified the invariant already
  holds across all 60 registry entries before asserting it (test-only; no vendored
  spec or server-code change, so no behaviour/spec drift). Tests: +1 registry unit
  test. `cargo test` 2121 green (was 2120), `cargo build --release` warning-clean.
  No new dep; binary unchanged (`#[cfg(test)]`-only). — binary: 3.7M (3845736 B, +0 B)
- 2026-08-11 — registry: collapsed the **two hand-maintained parallel API lists**
  into one source of truth, completing the cross-cutting `registry.rs` §9 item.
  New `src/registry.rs` holds `APIS: &[ApiSpec{name, version, body}]` (each `body`
  an `include_str!` of the vendored spec); `base_path`/`spec_url` are derived. Both
  consumers now derive from it: `main::catalog` builds its `apis` array by iterating
  `registry::APIS` (the ~340-line `json!` catalog literal is gone), and
  `apis::openapi` builds its spec/`docs` routes + `api_spec_urls()` from the same
  registry (the ~230-line `SPECS` const shrinks to a 2-entry `FRAGMENTS` const for
  the `auth`/`shared` `$ref` targets only). So a newly mounted API is one registry
  entry instead of two lockstep edits to `main.rs` **and** `openapi.rs`. Feature-API
  backlog remains effectively exhausted (only the `https://` sink-TLS cases — a
  multi-MB rustls client vs the small-binary directive — and open-ended state
  streams with no live worker are left `[ ]`), so this advanced the cross-cutting
  registry item. Pure refactor — the served surface (catalog JSON + every spec/docs
  byte) is unchanged, proven by the existing catalog↔served contract tests
  (`catalog_lists_mounted_apis`, `catalog_spec_urls_match_served_specs_and_resolve`,
  `serves_every_mounted_api_spec`, `serves_docs_for_every_mounted_api`) — so no
  vendored CAMARA spec changes. Tests: +4 registry unit tests (non-empty, no
  duplicate base_path, derived-path consistency, bodies are OpenAPI docs). `cargo
  test` 2120 green (was 2116), `cargo build --release` warning-clean. No new dep;
  binary **shrank** (dropped the big catalog literal). — binary: 3.7M (3845736 B,
  −37144 B)
- 2026-08-11 — openapi/docs: implemented DESIGN §9's third (and last unbuilt)
  discovery endpoint, `GET /{api}/v{n}/docs` — a human-readable docs page per
  served spec. The feature-API backlog is effectively exhausted (remaining `[ ]`
  leaves are the `https://` sink-TLS cases needing a multi-MB rustls client vs
  the small-binary directive, and open-ended state streams with no live worker),
  so this pass advanced the cross-cutting §9 catalog-wiring item instead. Each
  `…/docs` is a tiny static HTML shell (built at startup, in-memory `String`, no
  request-path I/O) that renders the sibling `…/openapi.yaml` with Redoc (CDN in
  the viewer's browser) and carries a `<noscript>` fallback linking the raw spec;
  no Rust dependency. Routes are generated from the existing `SPECS` table (one
  `/docs` per full `…/openapi.yaml`, incl. `/auth/docs`; the `/shared/errors.yaml`
  `$ref` fragment gets none). These are simulator meta-endpoints (they publish the
  contracts), so — like the `openapi.yaml`-serving routes — they have no vendored
  CAMARA spec to update. Tests: +7 (docs page is HTML + points Redoc at its spec
  + `<noscript>` fallback; every API spec has a resolvable `/docs` driven off the
  `api_spec_urls` source of truth; auth docs; `/shared/docs` 404; unknown `/docs`
  404; reachable through the full app; and a `catalog_apis_serve_html_docs`
  wiring test tying each catalogued `base_path`'s `/docs` to the served set).
  `cargo test` 2116 green (was 2109), `cargo build --release` warning-clean. No
  new dep. — binary: 3.8M (3882880 B, +3232 B)
- 2026-08-11 — dedicated-network-areas: added the **collection query**,
  `POST /dedicated-network-areas/vwip/retrieve-service-areas`
  (`retrieveNetworkServiceAreas`, scope `dedicated-network-areas:areas:read`) —
  the topmost unclaimed *actionable* `[ ]` leaf and the natural continuation of
  last pass's single-area read, **completing Dedicated Network — Areas vwip**.
  (The strictly-topmost `[ ]` items remain the `https://` sink-TLS cases needing
  a multi-MB rustls client vs the small-binary directive, and open-ended state
  streams with no live worker.) Verified against the authoritative CAMARA
  DedicatedNetworks `dedicated-network-areas.yaml` (`wip`): optional filters
  `atLocation`(Point)/`overlappingArea`(Area)/`coveringArea`(Area)/`byName`/
  `byNetworkProfileId`/`byQosProfileName`, 200 array of `ServiceArea`, 400/401/403.
  Lists the fixed 4-entry catalog narrowed by the request body's optional filters,
  ANDed (DESIGN §7): spatial geometry (CIRCLE-only) — point-in-circle, circle
  intersection, circle containment — computed with a **self-contained haversine**
  (no geospatial dep), plus exact-match attribute filters. No identifier → no
  reserved-error plane; a list never 404s (over-narrow filter → `[]`). Each
  returned area now carries its **stable canonical id** (`template_id` un-gated
  from `#[cfg(test)]`), so ids round-trip back through `readNetworkServiceArea`.
  Validation: bad body / non-CIRCLE query area / schema-violating filter → 400
  INVALID_ARGUMENT; out-of-range coordinate or radius < 1 → 400 OUT_OF_RANGE.
  POLYGON query areas a documented cut. Refactored `service_area` to share a
  `render_area` helper. Spec: authored the `/retrieve-service-areas` POST op +
  `RetrieveServiceAreasRequest` schema + description/`x-camarasim-scenarios`;
  header comment updated. 23 new tests (whole-catalog, each filter incl. AND
  combination + empty-result, id round-trip, 400/OUT_OF_RANGE cases, scope 403,
  missing-token 401, x-correlator, + pure haversine/qos-name units). `cargo test`
  2109 green (was 2086), `cargo build --release` warning-clean. No new dep. —
  binary: 3.8M (3879648 B, +25104 B)
- 2026-08-11 — dedicated-network-areas: mounted a new CAMARA API,
  `GET /dedicated-network-areas/vwip/areas/{areaId}` (`readNetworkServiceArea`,
  scope `dedicated-network-areas:areas:read`) — the last unstarted sibling of the
  DedicatedNetworks family (Networks/Accesses/Profiles were done; the topmost
  actionable `[ ]` leaf was `dedicated-network-areas`, all other remaining leaves
  being the `https://` sink-TLS cases needing a multi-MB rustls client vs the
  small-binary directive, or open-ended state streams with no live worker).
  Verified against the **authoritative** CAMARA DedicatedNetworks
  `dedicated-network-areas.yaml` (`wip`): it has two read ops
  (`readNetworkServiceArea` GET `/areas/{areaId}`, `retrieveNetworkServiceAreas`
  POST `/retrieve-service-areas`); scoped this pass to the **single-area read
  leg** only (the collection query, with its spatial `atLocation`/`overlappingArea`
  /`coveringArea` filters, left `[ ]` for later). A read-only, two-legged
  service-area **catalog** — the geographical sibling of Network Profiles, mirroring
  `readNetworkProfile` almost exactly. Serves a fixed 4-entry catalog (no upstream
  backend); the `areaId` is the sole control plane in three layers (DESIGN §7): not
  UUID-shaped → 400 INVALID_ARGUMENT; reserved trailing-digit suffix → canonical
  CAMARA error (`…404` → 404 no-such-area); else trailing three digits `d` select a
  template (`d % 4`, `…000`/no-digits → t0), the returned area a genuine second
  plane. Each `ServiceArea` carries a fixed CIRCLE `area` (center + radius) and
  **either** `qosProfiles` **or** `networkProfiles` (the schema either/or); the
  requested id is echoed as `id`. New `src/apis/dedicated_network_areas{,.rs}`
  (entry + `vwip.rs`), router + catalog + openapi-serving wiring. Spec: authored
  `specs/dedicated-network-areas/vwip/openapi.yaml` (the GET op + ServiceArea/Area/
  Point/error schemas + `x-camarasim-scenarios`, inlining the CIRCLE branch of the
  common `Area`). 13 new tests (happy path + both profile branches, distinct-id
  selection, no-digits → t0, reserved-suffix errors, malformed 400, scope 403,
  missing-token 401, x-correlator on 200+404, and pure units incl. every-template-
  valid + id round-trip). `cargo test` 2086 green (was 2073), `cargo build
  --release` warning-clean (`template_id` gated `#[cfg(test)]`). No new dep. —
  binary: 3.7M (3854544 B, +19560 B)
- 2026-08-11 — edge-application-management: added the deployment **patch leg**,
  `PATCH /edge-application-management/vwip/deployments/{appDeploymentId}`
  (`updateAppDeployment`, scope `…:deployments:update`) — the topmost unclaimed
  *actionable* `[ ]` leaf (verified against the authoritative CAMARA
  `EdgeApplicationManagement` `wip` spec, now in its own repo: PATCH,
  `application/merge-patch+json`, body `{appDeploymentName?, edgeCloudZones?,
  kubernetesClusterRefs?}`, 200 `AppDeploymentInfo`, 400/401/403/404/409/500/503).
  The strictly-topmost `[ ]` items remain the `https://` sink-TLS cases (need a
  multi-MB rustls client vs the small-binary directive), spatial
  `dedicated-network-areas`, and open-ended state streams with no live worker.
  In-place update via JSON Merge Patch (RFC 7396): only present fields change,
  arrays replaced wholesale, `null` = no-op, `appId` immutable, `appInstances`
  re-derived per effective zone. Four control planes (DESIGN §7): validation →
  400, unknown/malformed id → 404 (malformed folded), effective-zone catalog
  cross-ref → 404, patched-identity collision with a *different* stored
  deployment → 409 ALREADY_EXISTS. New atomic `deployment_store::update`
  (check-and-replace under one lock). Spec: PATCH op + `UpdateAppDeploymentRequest`
  schema + description/scenarios; header comment updated. 8 new tests (full
  patch, partial/no-op patch, unknown/malformed 404, validation 400, unknown-zone
  404, identity-collision 409, auth+scope, x-correlator). `cargo test` 2073 green,
  `cargo build --release` ok. No new dep. Completes the deployments resource
  (`createAppDeployment`/get/list/delete/patch). — binary: 3.7M (3834984 B)
- 2026-08-10 — edge-application-management: added the deployment **list + delete
  legs** — `GET /edge-application-management/vwip/deployments` (`getAppDeployments`,
  scope `…:deployments:read`) and `DELETE …/deployments/{appDeploymentId}`
  (`deleteAppDeployment`, scope `…:deployments:delete`) — the topmost unclaimed
  actionable `[ ]` leaf (natural continuation of last pass's `getAppDeployment`;
  every item above is done, remaining `[ ]` leaves are the `https://` sink-TLS
  cases needing a multi-MB rustls client vs the small-binary directive, `dedicated-
  network-areas` (spatial), or open-ended state streams with no live worker). Both
  keyed only on store state (opaque server-minted id, no reserved-suffix plane),
  mirroring the `apps`/`app-instances` list/delete legs: list → `200` array of
  `AppDeploymentInfo` (empty when none, a list never 404s); delete → `204`
  single-use / `404` (malformed path folded into 404). New `deployment_store::all`/
  `remove`; spec updated (both operations + description + scenarios). 10 new tests
  (list present/scope/auth/x-correlator; delete evict+re-404/unknown/malformed/
  scope-untouched/auth/x-correlator). `cargo test` 2065 green, `cargo build
  --release` ok. patch leg (`updateAppDeployment`) left for a later pass. No new
  dep. — binary: 3.7M (3816592 B)
- 2026-08-10 22:47Z — edge-application-management: added the **`getAppDeployment`
  leg**, `GET /edge-application-management/vwip/deployments/{appDeploymentId}`
  (scope `edge-application-management:deployments:read`) — the topmost unclaimed
  actionable `[ ]` leaf (the natural read-back slice after last pass's
  `createAppDeployment`; every item above it is done, and the remaining `[~]`
  leaves across the backlog are still the `https://` sink-TLS cases that need a
  multi-MB rustls client fighting the small-binary directive, or open-ended
  state streams with no live worker). Verified against the **authoritative**
  CAMARA `EdgeApplicationManagement` `wip` spec: `getAppDeployment` →
  `GET /deployments/{appDeploymentId}`, scope `…:deployments:read`, `200`
  `AppDeploymentInfo`, errors 400/401/403/404/500/503. `createAppDeployment`
  already persists the rendered `AppDeploymentInfo` (which carries its own
  `appDeploymentId`/`appInstances`/identity), so the read is a thin store lookup:
  the opaque, server-minted id → store state is the sole control plane (DESIGN
  §7), a stored id → `200` verbatim, any other (never-created / already-deleted /
  malformed) → `404 NOT_FOUND` (canonical 400 malformed-path folded into 404,
  mirroring `getApp`/`getAppInstance`). `x-correlator` echoed. Code: new
  `DEPLOYMENTS_READ_SCOPE` const + `get_app_deployment` handler (mirrors
  `get_app_instance`), route chained onto `/deployments/:app_deployment_id`,
  module doc refreshed; dropped the now-stale `#[cfg_attr(not(test),
  allow(dead_code))]` on `deployment_store::get` (now live on the request path).
  spec: new `/deployments/{appDeploymentId}` GET op (200 `AppDeploymentInfo` +
  401/403/404/500/503 + `x-camarasim-scenarios`), `AppDeploymentId` path param,
  header op-list + `getAppDeployment` narrative added. tests: +6 integration
  (verbatim read-back incl. store-equality + per-zone `appInstances`; unknown-id
  404; malformed-id → 404; write-scope-can't-read 403; missing-token 401;
  x-correlator on 200+404) — 2055 pass (was 2049). No new dependency. —
  binary: 3.63M (3,804,352 bytes, +6,920 B)
- 2026-08-10 21:47Z — edge-application-management: added the **`createAppDeployment`
  leg**, `POST /edge-application-management/vwip/deployments` (scope
  `edge-application-management:deployments:write`) — the topmost unclaimed
  actionable `[ ]` leaf (the remaining `[~]` items across the backlog are all
  either `https://` sink-TLS, which needs a multi-MB rustls client that fights the
  small-binary directive, or "no live worker/engine" open-ended state streams).
  Verified against the **authoritative** CAMARA `EdgeApplicationManagement` `wip`
  spec first: it confirms a real `/deployments` resource
  (createAppDeployment/getAppDeployment/getAppDeployments/deleteAppDeployment/
  updateAppDeployment). Scoped to the **create leg** only (read/list/delete/patch
  left `[ ]` for later passes). Deploys an onboarded app across one or more edge
  cloud zones: mints a deterministic RFC 4122 v5 `appDeploymentId` from the
  `(appId, appDeploymentName, sorted edgeCloudZones)` identity (SHA-256, order-
  independent), persists the rendered `AppDeploymentInfo` in a new in-memory store
  (`deployment_store.rs`; `Mutex<HashMap>`, lock never across await), returns `202
  Accepted` + `{appDeploymentId}` + `Location`. Three control planes (DESIGN §7):
  request validation → 400 INVALID_ARGUMENT; a cross-reference against the app +
  zone stores (unknown `appId` or any non-catalog zone → 404 NOT_FOUND); and store
  state (same identity → 409 ALREADY_EXISTS "Deployment already exists"). The
  persisted info lists one `appInstances` id per zone (same `instance_id`
  derivation as `createAppInstance`, so ids line up with the app-instance
  keyspace); the individual `AppInstanceInfo` resources are not separately
  materialised (documented cut), and `subscriptionRequest` is accepted-not-applied.
  Code: new `deployment_store` module (wired in `edge_application_management.rs`),
  `create_app_deployment` handler + `CreateAppDeployment` body + `deployment_id`
  helper, route wired. spec: added the `/deployments` POST op (202 `{appDeploymentId}`
  + 400/401/403/404/409/500/503) with `x-camarasim-scenarios` + `AppDeploymentName`/
  `AppDeploymentId`/`CreateAppDeploymentRequest`/`AppDeploymentInfo` schemas; header
  op-list + narrative updated. tests: 9 new (1 unit: deployment_id stable/order-
  independent/uuid-shaped/distinct-per-identity; 8 integration: happy path mints
  uuid + persists AppDeploymentInfo w/ per-zone appInstances + Location; duplicate
  identity (reordered zones) → 409; distinct name → new deployment; unknown app →
  404; non-catalog zone → 404; full 400 validation table + bad JSON; 401/403 auth;
  x-correlator on success + error) — 2049 pass (was 2040). No new dependency. —
  binary: 3.7M (3,797,432 bytes)
- 2026-08-10 — edge-application-management: added the **`getClusters` leg**,
  `GET /edge-application-management/vwip/clusters` (`getClusters`, scope
  `edge-application-management:clusters:read`) — the topmost unclaimed actionable
  `[ ]` leaf. Verified against the **authoritative** CAMARA spec first: the
  EdgeCloud repo was split (Apr 2026) and Edge Application Management now lives in
  `camaraproject/EdgeApplicationManagement`; its `wip` spec confirms `/clusters`
  (getClusters) + `/deployments` are real (the previous "deployments + clusters"
  backlog note was correct). Scoped to the read-only cluster catalog only
  (`deployments` left `[ ]` for a later pass — it is stateful CRUD). Serves a
  fixed 4-entry in-memory `ClusterInfo` catalog, mirroring `getEdgeCloudZones`:
  no device identifier → no reserved-error plane (a pure catalog). Two control
  planes (DESIGN §7): the `region`/`clusterRef`/`edgeCloudZoneId` query filters
  (AND; no match → `[]`, a list never 404s), and cross-reference — each cluster
  is hosted in one of the fixed `edge-cloud-zones`, so its `edgeCloudZoneId`
  (`zone_id`) and `edgeCloudRegion` come from the zone catalog. Validation:
  `clusterRef`/`edgeCloudZoneId` are strict UUIDs (malformed → 400
  INVALID_ARGUMENT, via existing `is_uuid`); `region` free text (unknown → `[]`);
  unknown query key → 400 (`deny_unknown_fields`). `clusterRef` is a stable RFC
  4122 v5 UUID (new `cluster_ref` helper, SHA-256, no new dep); each `ClusterInfo`
  carries a fixed representative `nodePools` entry (no live orchestrator to size
  it — documented cut). `x-correlator` echoed. Code: new `CLUSTERS` catalog,
  `get_clusters` handler, `ClusterQuery` (deny_unknown_fields), `cluster_info`,
  `zone_by_name`, `cluster_ref`; route wired. spec: added the `/clusters` GET op
  (200 array of ClusterInfo + 400/401/403/429/500/503) with `x-camarasim-scenarios`
  + `ClusterInfo`/`KubernetesClusterRef`/`KubernetesNodePool`/`EdgeCloudZoneId`/
  `EdgeCloudRegion` schemas (reuses `AppProvider`); header op-list + narrative
  updated. tests: 11 new (2 unit: clusterRef UUID validity/stability, every
  cluster names a real zone; 9 integration: full catalog as ClusterInfo w/
  nodePool; zoneId cross-reference; region filter + unknown→[]; clusterRef selects
  one; edgeCloudZoneId filter + AND combo; malformed clusterRef/zoneId→400; unknown
  query→400; 401 no token + 403 wrong scope; x-correlator echoed) — 2040 pass. No
  new dependency. — binary: 3.6M (3,770,424 bytes)
- 2026-08-10 — edge-application-management: completed the **app-instances CRUD** —
  added the read/list/delete legs `GET /app-instances/{appInstanceId}`
  (`getAppInstance`), `GET /app-instances` (`getAppInstances`) and
  `DELETE /app-instances/{appInstanceId}` (`deleteAppInstance`), the natural
  continuation of last pass's `createAppInstance`. All keyed only on the
  in-memory instance store (opaque minted `appInstanceId`, no reserved-suffix
  plane, mirroring the `apps` CRUD): read → 200 the stored `AppInstanceInfo`
  verbatim / 404 (malformed folded into 404); list → 200 array (empty when none);
  delete → 204 single-use / 404; scopes `instances:read` / `instances:delete`;
  synchronous delete (async 202/`DELETE_REQUESTED` + `sink` a documented cut);
  `x-correlator` echoed. Store gained `all`/`remove` (+ un-gated `get`); spec
  updated (2 new operations under `/app-instances`, new `/app-instances/{id}`
  path, `AppInstanceId` param, narrative + x-camarasim-scenarios). 22 new tests.
  No new dependency. `cargo test` 2029 green; `cargo build --release` green.
  binary: 3.6M (3,752,080 bytes). Remaining EAM cuts: `deployments`/`clusters`
  (later passes); every `https://` sink-TLS item still needs a rustls client
  (multi-MB dep vs the small-binary directive; test receivers run on `http://`
  loopback).
- 2026-08-10 — edge-application-management: added the **createAppInstance leg**,
  `POST /edge-application-management/vwip/app-instances` (`createAppInstance`,
  scope `edge-application-management:instances:write`) — the topmost genuinely
  actionable increment. (The earlier `[ ]` leaves remain out of reach for a
  clean, weightless single pass: every `https://` sink-TLS item needs a rustls
  TLS client — a multi-MB dependency that fights the prime directive "keep the
  binary small / justify every dependency", especially as the design docs note
  test receivers run on `http://` loopback; the remaining lifecycle-stream items
  are "no live engine/worker" open-ended state machines; `dedicated-network-areas`
  is spatial-later.) Instantiates an onboarded app onto an edge cloud zone: mints
  a deterministic UUID `appInstanceId` from the `(appId, edgeCloudZoneId)` pair,
  persists the rendered `AppInstanceInfo` in a new in-memory store
  (`instance_store.rs`; `Mutex<HashMap>`, lock never held across await), returns
  `202 Accepted` + `Location`. Three control planes (DESIGN §7): request
  validation → 400 INVALID_ARGUMENT; a cross-reference against both in-memory
  stores (unknown `appId` or non-catalog `edgeCloudZoneId` → 404 NOT_FOUND; the
  app's `appProvider` is echoed); and store state (same app+zone → 409
  ALREADY_EXISTS, matching CAMARA's "already instantiated in the given Edge Cloud
  Zone"). Second plane: `status` follows the target zone's catalog status
  (active→ready / inactive→failed / unknown→instantiating). Cuts:
  `componentEndpointInfo`, `terminating`/`unknown` states, `subscriptionRequest`
  callback. Code: new `instance_store` (insert = 409 detector; test-gated get),
  `create_app_instance` handler + `instance_id`/`instance_status`/`zone_by_id`/
  `is_uuid` helpers (promoted `is_uuid` out of the test module). spec: added the
  `/app-instances` POST op (202 + 400/401/403/404/409/500/503) with
  `x-camarasim-scenarios` + `AppInstanceInfo`/`CreateAppInstanceRequest`/
  `AppInstanceName`/`AppInstanceStatus` schemas; header/description updated.
  tests: 13 new (mint+persist+Location+provider echo; k8s ref echo; zone-status→
  instance-status; same-app-same-zone 409 vs new-zone new instance; unknown app
  404; unknown zone 404; 7 validation/malformed-JSON 400s; 403 without scope; 401
  no token; x-correlator on 202 and 409) — 2012 pass. No new dependency. —
  binary: 3.6M (3,733,656 bytes)

- 2026-08-10 — edge-application-management: added the **app delete leg**,
  `DELETE /edge-application-management/vwip/apps/{appId}` (`deleteApp`, scope
  `edge-application-management:apps:delete`) — the topmost unclaimed actionable
  `[ ]` leaf (last pass added `getApp`/`getApps`; the remaining earlier `[ ]`
  leaves stay the deferred `https://` sink-TLS infra, spatial
  `dedicated-network-areas`, and no-live-engine lifecycle streams). De-boards an
  onboarded app, evicting its stored `AppManifest`: present → `204 No Content`
  (single-use), unknown/already-deleted/malformed id → `404 NOT_FOUND`. Keyed
  only on store state (opaque, simulator-minted `appId`, so **no reserved-error
  plane** — DESIGN §7), mirroring `getApp`/`deleteNetwork`/`deleteAccess`;
  synchronous `204` (async `202`/`DELETE_REQUESTED` + `sink` notification a
  documented cut, as with every CamaraSim delete leg). **Completes the `apps`
  CRUD** (`app-instances`/`deployments`/`clusters` remain). `x-correlator`
  echoed. Code: new `store::remove` evict-and-report fn (single lock hold, never
  across await); `get(get_app).delete(delete_app)` on `/apps/{appId}`. spec:
  added the `/apps/{appId}` DELETE op (`deleteApp`, 204 + 401/403/404/500/503) +
  `x-camarasim-scenarios`; updated the header op list. tests: 7 new (removes a
  submitted app then a read 404s; single-use double-delete; unknown id 404;
  malformed id folds to 404; 403 without the delete scope + app survives; 401 no
  token; x-correlator echoed on 204 and 404) — 1998 pass. No new dependency. —
  binary: 3.6M (3,710,864 bytes)

- 2026-08-10 — edge-application-management: added the **app list leg**,
  `GET /edge-application-management/vwip/apps` (`getApps`, scope
  `edge-application-management:apps:read`) — the topmost unclaimed actionable
  `[ ]` leaf (last pass added `getApp`; the remaining earlier `[ ]` leaves stay
  the deferred `https://` sink-TLS infra, spatial `dedicated-network-areas`, and
  no-live-engine lifecycle streams). Scoped to the list op only; the paired
  `deleteApp` is left `[ ]` for a later pass. Returns the store snapshot as an
  array of `AppManifestInfo` (each `AppManifest` `submitApp` persisted + its
  minted `appId`) — the same shape `getApp` returns for one, mirroring the
  sibling list legs `listAccesses`/`retrievePayments`. Apps are opaque,
  simulator-minted UUIDs, so — like `getApp` — there is **no reserved-error
  plane**: the in-memory store is the only control plane (DESIGN §7); empty
  array when nothing onboarded (a list never 404s). `x-correlator` echoed. Code:
  new `store::all()` snapshot fn; `post(submit_app).get(get_apps)` on `/apps`;
  factored a shared `app_manifest_info` helper (getApp now reuses it). spec:
  added the `/apps` GET op (`getApps`, 200 array of `AppManifestInfo`,
  401/403/500/503) + `x-camarasim-scenarios`; updated the header op list. tests:
  4 new (lists a submitted app as AppManifestInfo; 403 without read scope; 401
  no token; x-correlator echoed) — 1991 pass. No new dependency. — binary: 3.6M
  (3,705,112 bytes)
- 2026-08-10 — edge-application-management: added the **app read leg**,
  `GET /edge-application-management/vwip/apps/{appId}` (`getApp`, scope
  `edge-application-management:apps:read`) — the topmost unclaimed actionable
  `[ ]` leaf (last pass added `submitApp`; the remaining earlier `[ ]` leaves are
  the deferred `https://` sink-TLS infra, spatial `dedicated-network-areas`, and
  no-live-engine lifecycle streams). Confirmed the contract from the
  authoritative upstream `edge-application-management.yaml` (`main`, WebFetch):
  `getApp` → scope `apps:read`, `appId` path param = strict UUID, `200`
  `AppManifestInfo` (`allOf` `AppManifest` + required `appId`), errors
  400/401/403/404. In CamaraSim it reads the `AppManifest` `submitApp` persisted
  and merges the minted `appId` in (reuses `store::get`; no new store fn). The
  `appId` is an opaque, simulator-minted UUID, so — like `readAccess`/
  `readNetwork` — there is **no reserved-error plane**: the in-memory store state
  is the only control plane (DESIGN §7): known id → `200 AppManifestInfo`;
  unknown *or malformed* id → `404 NOT_FOUND` (the canonical 400 malformed-path
  case folded into 404, a documented deviation mirroring the sibling read legs).
  `x-correlator` echoed on 200/404. New route + `get_app` handler in `vwip.rs`;
  dropped the now-stale `#[cfg_attr(not(test), allow(dead_code))]` on
  `store::get` (it now backs production). spec: added the `/apps/{appId}` GET op
  (getApp) + reusable `AppId` path parameter + `AppManifestInfo` schema
  (`allOf` AppManifest + appId) + `x-camarasim-scenarios` to
  `specs/edge-application-management/vwip/openapi.yaml` (404 → shared `NotFound`).
  tests: +6 integration (submit→read-back returns manifest+appId, unknown-UUID→404
  NOT_FOUND, malformed-id→404, write-scope-can't-read→403, no-token→401,
  x-correlator echoed on 200+404). No new dep. `cargo test` 1987 pass (was 1981);
  `cargo build --release` ok. binary (release): 3,698,848 bytes (~3.6M; +7,032 B).
- 2026-08-10 — edge-application-management: added the **first stateful leg**,
  `POST /edge-application-management/vwip/apps` (`submitApp`, scope
  `edge-application-management:apps:write`) — the topmost unclaimed actionable
  `[ ]` leaf (last pass began this API with the read-only `getEdgeCloudZones`;
  its stateful `apps`/`app-instances`/`deployments` resources were the next
  thing). Confirmed the contract from the authoritative upstream (now its own
  repo) `camaraproject/EdgeApplicationManagement` `edge-application-management.yaml`
  (`wip`, WebFetch): `POST /apps` = request body `AppManifest` (required
  name/version/appProvider/packageType/appRepo/requiredResources/componentSpec),
  `201 SubmittedApp` ({appId: uuid}), errors 400/401/403/409 `ALREADY_EXISTS`/
  500/503. In CamaraSim it onboards the app, mints a UUID-shaped `appId` and
  persists the manifest in a new in-memory store
  (`src/apis/edge_application_management/store.rs`; `Mutex<HashMap>`, lock never
  across await, mirroring the blockchain store). Two control planes (DESIGN §7):
  request validation (missing/blank required field, bad `name` pattern
  `^[A-Za-z][A-Za-z0-9_]{1,63}$`, unknown `packageType`, empty `componentSpec`,
  malformed JSON → 400 INVALID_ARGUMENT — validated by hand, no `regex` dep) and
  store state — the `appId` is derived deterministically from the app identity
  (`name`+`version`+`appProvider`, RFC 4122 v5 UUID via SHA-256, no new dep), so
  re-submitting the same app collides → 409 `ALREADY_EXISTS`. Nested
  `requiredResources` `oneOf` / `appRepo` / `componentSpec` item shapes validated
  for presence/non-emptiness only (documented cut). New `store.rs`
  (`insert`/`get`) + `pub mod store` in `edge_application_management.rs`; new
  route + `submit_app` handler + `AppManifest` struct + `is_valid_app_name` /
  `app_id` helpers in `vwip.rs`; `x-correlator` echoed on 201/400/409. spec:
  added the `/apps` POST op (submitApp) + `AppManifest`/`AppProvider`/`AppRepo`/
  `SubmittedApp` schemas + `x-camarasim-scenarios` + description to
  `specs/edge-application-management/vwip/openapi.yaml` (409 → shared
  `CamaraError`). tests: +13 (2 pure: name-pattern enforcement, app_id
  deterministic/uuid/identity-keyed; 11 integration: submit→201+persists,
  resubmit→409 ALREADY_EXISTS, different version→distinct id, missing required
  field→400, bad name→400, unknown packageType→400, empty componentSpec→400,
  malformed JSON→400, wrong scope→403, no token→401, x-correlator echoed). No new
  dep. `cargo test` 1981 pass (was 1968); `cargo build --release` ok. binary
  (release): 3,691,816 bytes (~3.6M).
- 2026-08-10 — edge-application-management: **new API begun** —
  `GET /edge-application-management/vwip/edge-cloud-zones` (`getEdgeCloudZones`,
  scope `edge-application-management:edge-cloud-zones:read`). All prior-phase
  actionable `[ ]` leaves were exhausted (the remaining ones are the deferred
  `https://` sink-TLS infra, the spatial `dedicated-network-areas`, and the
  no-live-engine lifecycle streams), so this pass opens a new CAMARA API under
  "Other CAMARA APIs as capacity allows", respecting phase order with a
  **stateless, non-spatial** read-only leg. Confirmed the contract from the
  authoritative upstream `edge-application-management.yaml` (`wip`, WebFetch):
  `getEdgeCloudZones` → 200 array of `EdgeCloudZone`
  ({edgeCloudZoneId(uuid), edgeCloudZoneName, edgeCloudZoneStatus(active/
  inactive/unknown), edgeCloudProvider, edgeCloudRegion}) with optional
  `region`/`status` query filters, responses 400/401/403/500/503. CamaraSim
  serves a fixed 6-entry catalog (EdgeCloud-family zone naming; a strict RFC 4122
  v5 `edgeCloudZoneId` = SHA-256(name) with version/variant nibbles forced, no
  new dep). Two control planes (DESIGN §7), both query filters (no device →
  no reserved-error plane, a pure catalog like QoS Profiles): exact-match
  `region` (unknown → `[]`, a list never 404s) and `status` (unknown value →
  400 INVALID_ARGUMENT); both combine (AND). Relaxed the canonical
  `EdgeCloudZones` `minItems: 1` so a filter narrowing to zero returns `[]`
  (documented deviation). `x-correlator` echoed on 200/400. New module
  `src/apis/edge_application_management{,.rs}` (+ `vwip.rs`); wired into
  `apis::routes()`, the `/` catalog (main.rs), and the served-spec table
  (openapi.rs). spec: authored `specs/edge-application-management/vwip/openapi.yaml`
  (getEdgeCloudZones + `EdgeCloudZone`/`EdgeCloudZoneStatus` schemas + query
  params + `x-camarasim-scenarios`). tests: +12 (2 pure: catalog covers every
  status / ids are valid UUIDs, zone_id stable+distinct; 10 integration:
  no-filter→full catalog, region filter, unknown-region→`[]`, status filter,
  region+status AND, unknown-status→400, unknown-query-param→400, wrong-scope→403,
  no-token→401, x-correlator echoed on success+error). No new dep. `cargo test`
  1968 pass; `cargo build --release` ok. binary (release): 3,672,072 bytes (~3.6M).
- 2026-08-10 — dedicated-network-accesses: added the **device-remove leg**,
  `POST /dedicated-network-accesses/vwip/accesses/{accessId}/devices/remove`
  (`removeDevicesFromAccess`, scope `dedicated-network-accesses:devices:remove`) —
  the topmost unclaimed actionable `[ ]` leaf (the remaining earlier `[ ]` leaves
  are the deferred `https://` sink-TLS infra and the spatial `-areas` API). This
  **completes the `/accesses/{accessId}/devices…` sub-resources** (listDevices /
  add / remove all done). The leg evicts the roster entries matching a bare
  `RemoveDevicesRequest` (a **bare** `Devices` array, minItems 1 / maxItems 100,
  mirroring `add`) from the access's `recentAccessDevices` and recomputes `stats`
  **atomically** (reusing `store::update`, closure under the map lock, never across
  await; `Vec::retain` by primary identifier), returning `204 No Content`. Matching
  is by `device_identifier` (first present of phoneNumber / NAI / ipv6 / IPv4
  publicAddress) so a submitted `Device` need not be byte-identical to the stored
  one; a device absent from the roster is an idempotent per-device no-op folded
  into the `204` (the `207` partial-success form is a documented cut, mirroring
  `add`). Two control planes (DESIGN §7): request validation (non-array body /
  array outside 1..=100 / device with no identifier / non-E.164 phoneNumber → 400
  INVALID_ARGUMENT, validated **before** the store so a bad body wins over a 404)
  and the opaque `accessId` → store state (unknown → 404, no reserved-suffix
  plane, mirroring `readAccess`). New route + `remove_devices` handler in
  `vwip.rs`; `x-correlator` echoed on 204/400/404. No new store method needed.
  spec: added the `/accesses/{accessId}/devices/remove` POST op
  (removeDevicesFromAccess) + `RemoveDevicesRequest` schema +
  `x-camarasim-scenarios` to `specs/dedicated-network-accesses/vwip/openapi.yaml`;
  header + description + cuts updated. tests: +6 integration (create→remove evicts
  matching entries + recomputes stats; absent-device is idempotent no-op;
  malformed-body-400-before-store (non-array/empty/>100/no-id/non-E164);
  unknown-access→404 with correlator; wrong-scope→403; no-token→401). No new dep.
  `cargo test` 1956 pass; `cargo build --release` ok. binary (release): 3,655,648
  bytes (~3.5M).
- 2026-08-10 — dedicated-network-accesses: added the **device-add leg**,
  `POST /dedicated-network-accesses/vwip/accesses/{accessId}/devices/add`
  (`addDevicesToAccess`, scope `dedicated-network-accesses:devices:add`) — the
  topmost unclaimed actionable `[ ]` leaf (the remaining earlier `[ ]` leaves are
  the deferred `https://` sink-TLS infra). Confirmed the contract from the
  authoritative upstream `dedicated-network-accesses.yaml` (`wip`, WebFetch):
  request body `AddDevicesRequest` = a **bare** `Devices` array (minItems 1,
  maxItems 100) of CAMARA `Device`s; `201 AddDevicesSuccess` = an `AccessDevices`
  array (of `AccessDevice` {device,status}); plus 207 `AddDevicesPartialSuccess`,
  400/401/403/404, 409, 422 `NO_VALID_DEVICE`. In CamaraSim the leg appends the
  submitted devices to the access's recorded `recentAccessDevices` roster and
  recomputes `stats` **atomically** (new `store::update`, closure under the map
  lock, never across await; roster capped at the 100 most-recent to honour the
  schema `maxItems: 100`, stats derived from the same window so they agree), and
  returns the added `AccessDevices` (201). Two control planes (DESIGN §7): request
  validation (non-array body / array outside 1..=100 / device with no identifier /
  non-E.164 phoneNumber → 400 INVALID_ARGUMENT, validated **before** the store so
  a bad body wins over a 404) and the opaque `accessId` → store state (unknown →
  404, no reserved-suffix plane, mirroring `readAccess`); the per-device
  GRANTED/DENIED grant rides on each device's identifier exactly as
  `createAccess`. 207 partial-success + 422 folded into `AccessDevice.status`
  (documented cut, mirroring `createAccess`). New route + `add_devices` handler in
  `vwip.rs`; `x-correlator` echoed on 201/400/404. spec: added the
  `/accesses/{accessId}/devices/add` POST op (addDevicesToAccess) +
  `AddDevicesRequest`/`AddDevicesSuccess` schemas + `x-camarasim-scenarios` to
  `specs/dedicated-network-accesses/vwip/openapi.yaml`; header + description +
  cuts updated. tests: +6 (1 store-unit: update mutates-in-place / reports
  presence; 5 integration: create→add appends roster + updates stats + listDevices
  sees it, malformed-body-400-before-store (non-array/empty/>100/no-id/non-E164),
  unknown-access→404, wrong-scope→403, no-token→401). No new dep. `cargo test`
  1950 pass; `cargo build --release` ok. binary (release): 3,644,048 bytes (~3.5M).
- 2026-08-10 — dedicated-network-accesses: added the **device-roster read leg**,
  `GET /dedicated-network-accesses/vwip/accesses/{accessId}/devices`
  (`listDevices`, scope `dedicated-network-accesses:devices:read`) — the topmost
  unclaimed `[ ]` backlog leaf after last pass completed the Accesses CRUD.
  Confirmed the contract from the authoritative upstream
  `dedicated-network-accesses.yaml` (`wip`, WebFetch): `GET /accesses/{accessId}/devices`
  → 200 `AccessDevicesPage` (`{items: AccessDevices, pagination: Pagination}`) with
  `page`/`perPage`/`deviceStatus` query params, responses 200/400/401/403/404;
  `AccessDevice` = `{device, status(REQUESTED|GRANTED|DENIED)}`. In CamaraSim the
  roster is the access's recorded `recentAccessDevices` (already `AccessDevice[]`),
  so `listDevices` reads back exactly what `createAccess` resolved — no new store
  fn, reuses `store::get`. Three control planes (DESIGN §7): opaque `accessId` →
  store state (unknown → 404, no reserved-suffix plane, mirroring `readAccess`);
  optional `deviceStatus` filter (unknown → 400 INVALID_ARGUMENT) — a genuine
  second plane; `page`/`perPage` window (non-integer → 400 INVALID_ARGUMENT, `<1`
  → 400 OUT_OF_RANGE), validated before the store so a bad query wins over a 404.
  House pagination envelope (`page`/`perPage`/`totalCount`/`totalPages`, mirroring
  Network Profiles). New route + `list_devices` handler + pure
  `parse_devices_list_params`/`build_access_devices_page` helpers in `vwip.rs`;
  `x-correlator` echoed. spec: added the `/accesses/{accessId}/devices` GET op
  (listDevices) + `AccessDevices`/`AccessDevicesPage`/`Pagination` schemas + query
  params + `x-camarasim-scenarios` to
  `specs/dedicated-network-accesses/vwip/openapi.yaml`; header + description
  updated. tests: +9 (2 pure: params default/validate, page windowing/envelope;
  7 integration: create→list roster, deviceStatus filter + empty-page, pagination,
  bad-query-400-before-store (INVALID_ARGUMENT/OUT_OF_RANGE/unknown-status),
  unknown-access→404, wrong-scope→403, no-token→401). No new dep. `cargo test`
  1944 pass; `cargo build --release` ok. binary (release): 3,631,136 bytes (~3.5M).
- 2026-08-10 — dedicated-network-accesses: added the **list + delete legs**,
  `GET /dedicated-network-accesses/vwip/accesses` (`listAccesses`, scope
  `…:accesses:read`) and `DELETE /dedicated-network-accesses/vwip/accesses/{accessId}`
  (`deleteAccess`, scope `…:accesses:delete`) — the topmost unclaimed `[ ]`
  backlog leaf after last pass's readAccess, completing the Accesses CRUD.
  Confirmed the contract from the authoritative upstream
  `dedicated-network-accesses.yaml` (`wip`, WebFetch): `listAccesses` returns a
  **bare `AccessInfo[]`** (not paginated, maxItems 1,000,000) with an optional
  `networkId` query filter, responses 200/400/401/403/404; `deleteAccess`
  204/400/401/403/404, no body on success. Mirrors the sibling Networks legs:
  `listAccesses` snapshots `store::all()` and filters by the optional `networkId`
  (a genuine second control plane — non-UUID → 400, unknown-but-valid → `[]`, a
  list never 404s; the upstream `x-device` header filter is a documented cut);
  `deleteAccess` evicts via `store::remove()` → `204` single-use or `404`, keyed
  only on the opaque minted id (no reserved-suffix plane). Added
  `store::all`/`store::remove` + a self-contained query decoder (`url_form_pairs`
  /`url_decode`, no new dep); new routes + `list_access`/`delete_access` handlers
  in `vwip.rs`; `x-correlator` echoed on every response incl. the `204`. spec:
  added the `/accesses` GET op (listAccesses, `networkId` query param) and the
  `/accesses/{accessId}` DELETE op (deleteAccess) + their `x-camarasim-scenarios`
  to `specs/dedicated-network-accesses/vwip/openapi.yaml`; header comment +
  API description updated. tests: +10 (1 store-unit: all/remove single-use;
  1 pure: networkId-filter validation; 5 list integration:
  filter-returns-matching, unknown→`[]`, non-UUID→400, wrong-scope→403,
  no-token→401; 3 delete integration: evict-single-use-then-read-404,
  unknown→404, wrong-scope→403, no-token→401). No new dep. `cargo test` 1935
  pass; `cargo build --release` ok. binary (release): 3,616,856 bytes (~3.5M).
- 2026-08-10 — dedicated-network-accesses: added the read leg,
  `GET /dedicated-network-accesses/vwip/accesses/{accessId}` (`readAccess`,
  scope `dedicated-network-accesses:accesses:read`) — the topmost unclaimed
  `[ ]` backlog leaf after last pass's createAccess. Confirmed the contract from
  the authoritative upstream `dedicated-network-accesses.yaml` (`wip`, WebFetch):
  `GET /accesses/{accessId}`, accessId UUID (maxLength 36), responses
  200/400/401/403/404, 200 = `AccessInfo`. Mirrors the sibling `readNetwork`:
  keyed only on the in-memory store state (opaque minted UUID → no
  reserved-suffix plane) → `200 AccessInfo` (verbatim) or `404 NOT_FOUND` for an
  unknown/malformed id (the upstream 400 malformed-path case folded into 404, as
  in `readNetwork`). Added `store::get`; new route + `read_access` handler in
  `vwip.rs`; `x-correlator` echoed. spec: added the `/accesses/{accessId}` GET op
  (readAccess) + an `AccessId` path parameter + `x-camarasim-scenarios` to
  `specs/dedicated-network-accesses/vwip/openapi.yaml`; header comment updated.
  tests: +6 (2 store-unit: read-back / unknown-id-none; 4 integration:
  create-then-read-back verbatim, unknown→404, wrong-scope→403, no-token→401).
  No new dep. `cargo test` 1924 pass; `cargo build --release` ok.
  binary (release): 3,603,504 bytes (~3.5M).
- 2026-08-10 — dedicated-network-accesses: mounted the new **Accesses** sibling
  API and its create leg, `POST /dedicated-network-accesses/vwip/accesses`
  (`createAccess`, scope `dedicated-network-accesses:accesses:create`) — the
  topmost unclaimed `[ ]` backlog leaf after the Networks CRUD completed; picked
  the non-spatial sibling (`-accesses`) over the spatial `-areas`, honouring the
  phase order. Confirmed the contract from the authoritative upstream
  `dedicated-network-accesses.yaml` (`wip`, WebFetch): base path
  `dedicated-network-accesses/vwip`, `createAccess` 201/207/400/401/403/404/409/422,
  `CreateAccessRequest` = `BaseAccessInfo` (`networkId` required) + `devices`
  (1..=100 CAMARA `Device`s), `AccessInfo` with `stats`
  (totalDevices/Granted/Denied) + `recentAccessDevices` (`AccessDevice`
  {device,status}), `DeviceStatus` enum REQUESTED/GRANTED/DENIED. New module
  `src/apis/dedicated_network_accesses/{store.rs,vwip.rs}` +
  `dedicated_network_accesses.rs`, wired into `apis.rs`, `openapi.rs` and the `/`
  catalog. Three control planes (DESIGN §7): request validation → 400
  INVALID_ARGUMENT; `networkId` reserved suffix → canonical error (`…404` = no
  such network); per-device identifier → GRANTED/DENIED driving stats +
  recentAccessDevices. `sinkCredential` accepted, never echoed. Cuts documented
  in the spec: 207 multi-status (partial denials are data in the 201), 409/422
  request-level cases, sink notification, read/list/delete + `/devices…` legs.
  In-memory store mirrors `dedicated_network::store` (Mutex<HashMap>, lock never
  across await; SHA-256 UUID-v4 id mint, no uuid/rand dep). spec: added
  `specs/dedicated-network-accesses/vwip/openapi.yaml` (createAccess op + Device/
  Devices/DeviceStatus/AccessDevice/AccessStats/BaseAccessInfo/CreateAccessRequest/
  AccessInfo/AccessError schemas, x-camarasim-scenarios). tests: +17 (6 pure-unit:
  uuid/e164 shape, device_identifier precedence, device_status grant/deny,
  validate_device, build_access_info; 2 store; 9 integration: happy 2-device grant,
  devices omitted, reserved-suffix device denied+counted, reserved networkId →
  canonical error, malformed → 400 set, qosProfiles bounds, non-https sink, scope
  403, no-token 401). `cargo test` 1919 green (was 1902); `cargo build --release`
  green. No new dep. — binary: 3.5M (3,597,352 B)
- 2026-08-10 — dedicated-network: added the delete leg,
  `DELETE /dedicated-network/vwip/networks/{networkId}` (`deleteNetwork`, scope
  `dedicated-network:networks:delete`) — the topmost unclaimed `[ ]` backlog
  leaf after last pass's `listNetworks`, **completing the Networks CRUD**.
  Confirmed the contract from the authoritative upstream `dedicated-network.yaml`
  (`wip`, WebFetch): `deleteNetwork` returns `204 No Content` (x-correlator
  header) with errors 400/401/403/404 and scope `dedicated-network:networks:delete`.
  Mirrors QoD `deleteSession`: keyed only on the in-memory store state (the minted
  opaque `networkId` has no reserved-suffix plane) — a known id evicts its network
  → `204` (single-use), an unknown/already-deleted id → `404 NOT_FOUND`. Deletion
  is synchronous (no async `202`/`DELETE_REQUESTED`) with no `sink` notification
  (documented cut; the create leg records no notification target). New
  `store::remove` (returns the evicted `NetworkInfo`, lock never across await,
  mirroring QoD). `x-correlator` echoed on `204`+`404`. spec: added the
  `DELETE /networks/{networkId}` op (a `networkId` path param, 204 + x-correlator
  header, 401/403/404, `x-camarasim-scenarios`) to
  `specs/dedicated-network/vwip/openapi.yaml`; header comment updated
  (create+read+list+delete → CRUD complete). tests: +5 (create→delete 204 →
  gone (read 404, second delete 404 single-use); unknown id → 404; scope 403;
  no-token 401; x-correlator on 204+404). `cargo test` 1902 green (was 1897);
  `cargo build --release` green. No new dep. — binary: 3.5M (3,563,528 B)
- 2026-08-10 — dedicated-network: added the list leg,
  `GET /dedicated-network/vwip/networks` (`listNetworks`, scope
  `dedicated-network:networks:read`) — the topmost unclaimed `[ ]` backlog leaf
  after last pass's `readNetwork` (list/delete legs over the same store).
  Confirmed the contract from the authoritative upstream `dedicated-network.yaml`
  (`wip`, WebFetch): `GET /networks` returns a **bare JSON array** of
  `NetworkInfo` (no list wrapper, `200`, empty when none) with one optional
  `name` query param (`maxLength: 1024`); no pagination. Scoped to the list leg
  this pass; `deleteNetwork` recorded as the one remaining `[ ]` sub-step. Two
  planes (DESIGN §7): the in-memory store state (every network created this run)
  and the optional `name` filter (only networks whose `name` equals it; a `name`
  > 1024 chars → 400 INVALID_ARGUMENT). Shares QoD/carrier-billing's list shape;
  new `store::all()` (values clone, lock never across await) + a self-contained
  `application/x-www-form-urlencoded` decoder (`+`/`%XX`), so no query-string
  dep. `x-correlator` echoed. spec: added the `GET /networks` op (optional
  `name` query param, 200 array + two examples, 400/401/403,
  `x-camarasim-scenarios`) to `specs/dedicated-network/vwip/openapi.yaml`; header
  comment updated (create+read+list served, delete later). tests: +8
  (`parse_name_filter` unit; list returns created networks filtered by name,
  empty-array no-match, unfiltered array, name > maxLength → 400, scope 403,
  no-token 401, x-correlator). `cargo test` 1897 green (was 1889);
  `cargo build --release` green. No new dep. — binary: 3.4M (3,558,384 B)
- 2026-08-10 — dedicated-network: added the read leg,
  `GET /dedicated-network/vwip/networks/{networkId}` (`readNetwork`, scope
  `dedicated-network:networks:read`) — the natural next slice after last pass's
  `createNetwork` and the topmost unclaimed `[ ]` backlog leaf (the read/list/
  delete legs over the same store). The TLS `https://` sink leaf in Phase 3 stays
  a deliberate deferred cut (needs a rustls stack vs "keep the binary small").
  Mirrors QoD `getSession`: keyed only on the in-memory store state — a known,
  minted `networkId` reads its stored `NetworkInfo` back (`200`), an unknown one
  → `404 NOT_FOUND`; the minted opaque UUID has no reserved-suffix plane, so the
  store is the sole control plane (documented). Made `store::get` non-test
  (`readNetwork` now uses it). `x-correlator` echoed on `200`+`404`. spec: added
  the `GET /networks/{networkId}` op (a `networkId` path param, 200 NetworkInfo +
  example, 401/403/404, `x-camarasim-scenarios`) to
  `specs/dedicated-network/vwip/openapi.yaml`; header comment updated
  (create+read now served, list/delete later). tests: +5 (read-back verbatim +
  status, unknown → 404, scope 403, no-token 401, x-correlator on 200+404).
  `cargo test` 1889 green (was 1884); `cargo build --release` green. No new dep.
  — binary: 3.4M (3,549,184 B)
- 2026-08-10 — dedicated-network: added a **new API**,
  `POST /dedicated-network/vwip/networks` (`createNetwork`, scope
  `dedicated-network:networks:create`) — the resource sibling of last passes'
  Network Profiles catalog and the topmost unclaimed `[ ]` backlog leaf
  (the sibling Dedicated-Networks APIs). Confirmed the contract from the
  authoritative upstream `dedicated-network.yaml` (`wip`, WebFetch):
  `CreateNetwork` = `BaseNetworkInfo` (`name`, `networkProfileId` **oneOf**
  `qosProfileName`, required `serviceTime`{start,end}, required `serviceAreaId`,
  optional `sink`/`sinkCredential`) → `201 NetworkInfo` (`id` + `status`
  REQUESTED/RESERVED/ACTIVATED/TERMINATED), errors 400/401/403. Stateful,
  resource-oriented, two-legged. Scoped to the create leg this pass; the
  read/list/delete legs and the other siblings (`-accesses`, spatial `-areas`)
  recorded as remaining sub-steps. Two control planes (DESIGN §7): request
  validation (bad body/`name` length/the profile `oneOf`/non-UUID
  `networkProfileId`|`serviceAreaId`/empty `qosProfileName`/missing-or-malformed
  `serviceTime`/non-`https` `sink` → 400 INVALID_ARGUMENT; a UTC
  `serviceTime.end` before `start` → 400 OUT_OF_RANGE) and the required
  `serviceAreaId` UUID (reserved suffix → canonical CAMARA error; else `d % 3`
  picks the created `status`). `sinkCredential` accepted but never echoed (secret);
  create-only, so no notification delivered (documented cut). New
  `src/apis/dedicated_network/{,vwip,store}.rs` (in-memory store mirroring QoD,
  UUID minting via SHA-256, no `uuid`/`rand` dep) + self-contained RFC 3339
  date-time shape check (no new dep); wired into `apis::routes`, the openapi
  `SPECS` table, and the `/` catalog. spec: new vendored
  `specs/dedicated-network/vwip/openapi.yaml` (POST op + CreateNetwork/
  BaseNetworkInfo/NetworkInfo/ServiceTime/NetworkStatus/NetworkError + shared-error
  `$ref`s + `x-camarasim-scenarios`). tests: +18 (store: id shape/uniqueness,
  read-back; pure: uuid-shape, status selection wraps, rfc3339 shape,
  build-info omits secret/absent fields; integration: happy path echo + persist,
  serviceAreaId → status, qosProfileName alternative, reserved …404/…429/…422 →
  canonical error, oneOf both/neither → 400, non-UUID/missing-end/non-https →
  400, end<start → OUT_OF_RANGE, https sink echoed but credential not, scope 403,
  no-token 401, x-correlator on 201+404). `cargo test` 1884 green (was 1866);
  `cargo build --release` green. No new dep. — binary: 3.4M (3,543,144 B)
- 2026-08-10 — dedicated-network-profiles: added the second operation,
  `GET /dedicated-network-profiles/vwip/profiles` (`readNetworkProfiles`, scope
  `dedicated-network-profiles:profiles:read`) — the paginated catalog list, the
  natural next slice after last pass's single-profile lookup and the topmost
  unclaimed `[ ]` backlog leaf. Confirmed the contract from the authoritative
  upstream `dedicated-network-profiles.yaml` (WebFetch): `GET /profiles` with
  `perPage`/`page`/`name` query params → a `NetworkProfilesPage`
  (`{ items, pagination }`), errors 400/401/403. Returns a `NetworkProfilesPage`
  over the fixed 4-entry catalog. Two control planes (DESIGN §7): the optional
  exact-match `name` filter (unknown name → empty page; a list never 404s) and
  the `page`/`perPage` window (defaults 1/10; non-integer → 400 INVALID_ARGUMENT,
  `<1` → 400 OUT_OF_RANGE; `name` >1024 chars → 400 INVALID_ARGUMENT). No device
  identifier → no reserved-error plane (mirrors QoS Profiles' list). Each catalog
  item gets a stable canonical `id` (a UUID whose trailing three digits are the
  catalog index), so the list and the single-profile lookup agree —
  `GET /profiles/{id}` returns the same profile the list holds at that id. Reused
  the `RawQuery` + `serde_urlencoded` pagination pattern from Carrier Billing /
  Network Traffic Analysis; `pagination` envelope is the house
  `page`/`perPage`/`totalCount`/`totalPages` shape (canonical refs an external
  `CAMARA_common` Pagination — inlined an equivalent, documented in the spec).
  spec: added the `GET /profiles` op (params, 200 `NetworkProfilesPage` +
  example, 400/401/403, `x-camarasim-scenarios`) and the `NetworkProfilesPage` /
  `Pagination` schemas to `specs/dedicated-network-profiles/vwip/openapi.yaml`.
  tests: +21 (pure: template-id round-trips to its own template, page-envelope
  windowing/counts, empty-catalog zero-pages; integration: full-catalog default,
  list id → same profile via lookup, pagination + past-end empty, name filter
  known/unknown, bad-pagination 400s, scope 403 / no-token 401, x-correlator).
  `cargo test` 1866 green (was 1845); `cargo build --release` green. No new dep.
  — binary: 3.4M (3,509,856 B)
- 2026-08-10 — dedicated-network-profiles: added a **new API**,
  `GET /dedicated-network-profiles/vwip/profiles/{profileId}`
  (`readNetworkProfile`, scope `dedicated-network-profiles:profiles:read`) — the
  first CAMARA DedicatedNetworks operation, the stateless read-only catalog
  analogue of QoS Profiles. Chosen because every implemented API's CRUD +
  notifications are complete and the topmost `[ ]` backlog leaves are all
  deferred for concrete reasons (TLS `https://` sink delivery needs a rustls
  stack vs "keep the binary small" — an explicit deliberate-decision cut, not an
  automated pass; the campaign/intermediate-transition legs need a live
  engine), so the top *safe* unit is a new API under "Other CAMARA APIs as
  capacity allows". Confirmed the contract from the authoritative upstream
  `dedicated-network-profiles.yaml` (WebFetch): `NetworkProfile` (`id`,
  `maxNumberOfDevices`, `aggregatedUl/DlThroughput` `BitRate`, `qosProfiles`,
  `defaultQosProfile`), errors 400/401/403/404. Stateless, **non-spatial**
  (a network profile is a connectivity template, no coordinates), two-legged
  (the catalog exists independently of any subscriber — no device identifier).
  Scoped to the single-profile lookup this pass; the paginated `readNetworkProfiles`
  list and sibling Dedicated-Networks APIs recorded as remaining sub-steps.
  Control plane (DESIGN §7): the `profileId` path param in three layers — not
  UUID-shaped → 400 INVALID_ARGUMENT; reserved trailing-digit suffix → canonical
  CAMARA error (`…404` → 404 no-such-profile); else the trailing three digits
  `d` pick a fixed 4-entry template table (`d % 4`; `…000`/no-digits → template
  0), the requested id echoed back. New `src/apis/dedicated_network_profiles/{,
  vwip}.rs`; wired into `apis::routes`, the openapi `SPECS` table, and the `/`
  catalog. spec: new vendored `specs/dedicated-network-profiles/vwip/openapi.yaml`
  (GET op + NetworkProfile/BitRate/NetworkProfilesError + shared-error `$ref`s +
  `x-camarasim-scenarios`). tests: +11 (pure: uuid-shape, every-template-valid,
  digit-selection-wraps; integration: happy-path selected profile, distinct ids
  → distinct profiles, no-digits → template 0, reserved …404/…429/…422 →
  canonical error, malformed id → 400, scope 403, missing-token 401,
  x-correlator on 200+404). `cargo test` 1856 green (was 1845); `cargo build
  --release` green. No new dep. — binary: 3.4M (3,496,392 B)
- 2026-08-10 — sponsored-data: added `GET /campaign/{sponsorId}/{campaignId}/
  active-sponsorships` (`getActiveSponsorships`, scope
  `sponsored-data:campaign:read`) — lists a campaign's **currently-active**
  sponsorship sessions as `{sessionId, phoneNumber}` pairs + `totalCount`, keyed
  off the canonical CAMARA shape (fetched from the upstream SponsoredData wip
  spec). Two control planes (DESIGN §7): a reserved trailing-digit suffix on the
  campaignId UUID → canonical CAMARA error (mirrors `getCampaignStatus`); else the
  shared session store is scanned for the `(sponsorId, campaignId)` pair (new
  `store::all_matching`) and filtered to the active sessions. Extracted shared
  `is_active`/`consumption` helpers from `getSessionStatus` so "active" means the
  same for the list and the status read (no drift). Empty campaign → 200 empty
  array, `totalCount:0` (a list never 404s); malformed path ids → 400
  INVALID_ARGUMENT; `x-correlator` echoed. Spec: added the `active-sponsorships`
  path (params, 200 `ActiveSponsorships` schema, two examples, functional cases)
  to `specs/sponsored-data/vwip/openapi.yaml`; refreshed the header/divergence
  notes. Tests: 9 new (1 store `all_matching` unit; 2 pure builder units for
  `is_active`/`consumption` + `active_sponsorships_body`; 6 router active/empty/
  reserved-suffix/malformed/auth/correlator). `cargo test` 1845 green,
  `cargo build --release` green. No new dep. — binary: 3.4M (3,477,448 B)
- 2026-08-09 — sponsored-data: added `GET /campaign/{sponsorId}/{campaignId}/
  campaign-status` (`getCampaignStatus`, scope `sponsored-data:campaign:read`) —
  the first of the Sponsored Data *campaign* operations. Reports a whole
  campaign's operational state (distinct from a single session): `active`/
  `paused`/`completed`, its window, prepaid/postpaid billing type, and its
  data-volume balance. Campaign lifecycle CRUD (`manageCampaign`) is unmodelled,
  so there is no campaign store — the status is derived **statelessly** from the
  `campaignId`'s embedded UUID (DESIGN §7, matching how Network Health
  Assessment keys off a UUID `networkId`): malformed path ids → 400
  INVALID_ARGUMENT; the UUID's trailing three digits `d` — reserved suffix →
  canonical CAMARA error (`…404` → 404 campaign-not-found); else `d` even →
  `prepaid` (carries `contractedDataVolume`/`remainingDataVolume`) / odd →
  `postpaid` (`usedDataVolume` only), and `(d/2)%3` → status active/paused/
  completed with the matching `completionReason` (`completed` + `(d/6)` odd →
  `data_exhausted` spending the whole 1000 MB allotment, else `time_expired`).
  Window anchored to now (started 24 h ago; ongoing ends 24 h out, completed 1 h
  past). `x-correlator` echoed. Spec: added the `campaign-status` path (params,
  200 `CampaignStatus` schema, three examples, functional cases) + the
  `CampaignStatus` schema to `specs/sponsored-data/vwip/openapi.yaml`. Tests: 7
  new (pure builder facet-derivation across 5 control digits + router happy/
  type-status/reserved-suffix/malformed-id/auth/correlator). `cargo test` 1836
  green, `cargo build --release` green. No new dep. — binary: 3.3M (3,459,504 B)
- 2026-08-09 — iot-sim-fraud-prevention: added `bindType: AREALIMIT` /
  `unBindType: AREALIMIT`, completing the API's stateful round-trip for both
  facets. A second in-memory set in the shared store records which devices are
  area-restricted; the upstream bind carries no geometry (network-provisioned
  area), so only membership is stored and the `Circle` `limitArea` is synthesised
  deterministically from the identifier at query time (extracted shared
  `synth_circle`). An `AREALIMIT` query now lets a **stored restriction win**
  (`RESTRICTED` even for an even-tail default-`UNRESTRICTED` device), mirroring
  IMEIBIND's "stored binding wins"; an `AREALIMIT` unbind clears it (`200 {
  unbound: true }`) or `422 UNNECESSARY_UNBIND_AREALIMIT` when none is in force.
  The `BindType`/`UnBindType` enums gained `AREALIMIT` (both facets are
  independent: an IMEIBIND unbind leaves an AREALIMIT restriction in force). Spec:
  BindType/UnBindType enums + descriptions, bind/unbind operation cases +
  examples, `UNNECESSARY_UNBIND_AREALIMIT` in Unbind422, updated 400 messages, and
  the AreaLimit-query "stored wins" note. Tests: replaced the 2 obsolete
  "AREALIMIT-bind/unbind→400" tests with 7 new ones (round-trip, idempotent,
  unnecessary-unbind, facet-independence, reserved-suffix, unknown bind/unbind
  type). `cargo test` 1829 green, `cargo build --release` green. No new dep. —
  binary: 3.3M (3,443,800 B)
- 2026-08-09 — capabilities-and-restrictions: added Capabilities and Restrictions
  vwip (CAMARA CapabilitiesAndRuntimeRestrictions `wip`), a new stateless,
  non-spatial consumer-context capability-discovery API. `POST
  /capabilities-and-restrictions/vwip/retrieve` (`postServiceCapability`, scope
  `camara-capability:read`) → `201 CapabilityInfo` with one `CapabilityDetail`
  (bitmap branch) per query: a fixed 3-entry `bitmapCapabilities` catalogue +
  a context-derived `camaraCapabilitiesBitmap`. Control planes (DESIGN §7): the
  first query's first `resourceScopes.phoneNumber` reserved suffix → canonical
  CAMARA error; active bitmap derived from that phoneNumber's trailing digits
  (else an FNV hash of `overlayExtends`) `% 8`. Validation → 400 INVALID_ARGUMENT
  / OUT_OF_RANGE (>100 queries / >20 overlays). Vendored self-contained
  `specs/capabilities-and-restrictions/vwip/openapi.yaml` (faithful schema shapes;
  documented cuts: subscriptionRequest callback, CapabilitySetFootprint branch,
  ETag/304 caching, overlay resolution), wired into apis/openapi/catalog. 23 new
  tests; full suite 1824 green, `cargo build --release` green. No new dep (reused
  sha2-free FNV + serde_json). — binary: 3.3M (3439672 B)
- 2026-08-09 — sms: added Short Message Service v0alpha1 (CAMARA
  ShortMessageService 0.1.0-alpha.1), a new stateless, non-spatial, two-legged
  send-SMS API. `POST /sms/v0alpha1/short-message` (`send-sms`, scope
  `send-sms:short-message`) → `200 { msgId, timestamp }`; control plane is the
  first recipient `to[0]`'s reserved error suffix → canonical CAMARA error, else a
  deterministic UUID-shaped `msgId` (SHA-256 of from+to+message) + RFC 3339 UTC
  timestamp. Vendored `specs/sms/v0alpha1/openapi.yaml` from the upstream SMS.yaml,
  wired into apis/openapi/catalog; bumped crate `recursion_limit` to 256 (the `/`
  catalog `json!` outgrew the 128 default). 22 new tests; full suite 1801 green.
  No new dep (reused sha2). — binary: 3.3M (3404688 B)
- 2026-08-09 — iot-sim-fraud-prevention: added `queryType: AREALIMIT` to
  `POST /query` (the area-restriction facet). The `QueryType` enum now accepts
  both `IMEIBIND` and `AREALIMIT`; an AREALIMIT query returns
  `{ areaLimit: { areaLimitStatus: RESTRICTED|UNRESTRICTED, limitArea?: Circle } }`,
  derived statelessly from the identifier's trailing-digit parity (odd →
  RESTRICTED with a deterministic schema-valid `Circle` — lat [-90,90]/long
  [-180,180]/radius [1,200000]; else UNRESTRICTED), reusing the existing
  identifier resolution / two-/three-legged rule / reserved-error plane.
  `AREALIMIT` bind/unbind (which would set/clear a stored restriction) stays
  deferred — the `BindType`/`UnBindType` enums remain trimmed to `[IMEIBIND]`, so
  an AREALIMIT bind/unbind is still 400 INVALID_ARGUMENT. No new dep. Spec:
  `QueryType`/`QueryFraudPreventionResponse` updated + new `AreaLimit`/
  `AreaLimitStatus`/`Area`/`AreaType`/`Circle`/`Point` schemas + functional
  cases/examples in `specs/iot-sim-fraud-prevention/vwip/openapi.yaml`. Tests: +5
  (1 `area_limit` unit; 4 integration — odd→RESTRICTED+Circle, even→UNRESTRICTED,
  reserved suffix, unknown queryType→400; replaced the old
  "AREALIMIT-query→400" test). `cargo test` 1779 green, `cargo build --release`
  green. — binary: 3.2M (3,382,224 B)
- 2026-08-09 — qos-booking: implemented the `SCHEDULED`→`ACTIVATED`-at-window-start
  transition. A `SCHEDULED` (odd-tail), sink-bearing booking arms an off-path async
  timer (`spawn_activation`) that sleeps until `startTime`, flips it to `ACTIVATED`
  in place (new atomic `store::activate` + `store::peek_credential`), delivers a
  non-terminal `ACTIVATED` `status-changed` CloudEvent, then chains window expiry →
  `DURATION_EXPIRED`; concurrent delete is exactly-once. New self-contained RFC 3339
  parser (`unix_secs_from_rfc3339`/`days_from_civil`), no new dep. Spec updated
  (header/functional-cases/scenarios/sink/QosBookingEvent). 5 new tests (2 store, 1
  parser unit, 2 integration incl. peeked-credential-survives-to-delete); `cargo test`
  1775 green, `cargo build --release` green. Only TLS (`https://` sink) remains for
  QoS Booking. — binary: 3.3M (3374472 B)
- 2026-08-09 17:45Z — qos-booking: added the `DURATION_EXPIRED` `status-changed`
  transition (window ran to completion). Any `ACTIVATED`, sink-bearing booking that is
  *not* the `…002` network-drop tail now schedules a fire-and-forget async timer at
  creation (`spawn_window_expiry`, mirroring QoD's `spawn_expiry`; no re-read loop —
  qos-booking has no `extend`) that waits the booking's `duration` (from `startedAt` =
  creation), then evicts it (`store::remove` gate → a later `GET` is `404`) and delivers
  a CloudEvent (`bookingStatus: TERMINATED`, `statusInfo: DURATION_EXPIRED`) over raw TCP
  `http://`, single-use `sinkCredential` applied. Mutually exclusive with
  `NETWORK_TERMINATED` (by tail), so exactly one terminal event fires; concurrent-delete
  safe. No new dep. Spec: documented the window-expiry functional case + refreshed the
  header/sink/`QosBookingEvent` prose in `specs/qos-booking/vwip/openapi.yaml`
  (`statusInfo` enum already carried `DURATION_EXPIRED`). Tests: +2 (delivery+eviction at
  window end; ACCESSTOKEN bearer on the callback). `cargo test` 1770 green. binary
  (release): 3,364,640 bytes (~3.3M). Remaining qos-booking: `SCHEDULED`→`ACTIVATED`
  at-window-start + TLS (`https://`) sink.
- 2026-08-09 16:45Z — qos-booking: added the `NETWORK_TERMINATED` `status-changed`
  transition. A `…002` (even, non-zero → `ACTIVATED`) booking that records a `sink` is
  now dropped early by the simulated network — a 1 s fire-and-forget async timer
  (`spawn_network_termination`, mirroring QoD's `…001` case) evicts it from the store
  (a later `GET` → `404`) and delivers a CloudEvent (`bookingStatus: TERMINATED`,
  `statusInfo: NETWORK_TERMINATED`) over raw TCP `http://`, with the single-use
  `sinkCredential` applied; concurrent-delete safe (`store::remove` gate). No new dep.
  Spec: documented the `…002` early-drop functional case + refreshed the sink/event
  prose in `specs/qos-booking/vwip/openapi.yaml` (`statusInfo` enum already carried
  `NETWORK_TERMINATED`). Tests: +2 (delivery+eviction; credential on the callback).
  `cargo test` 1768 green. binary (release): 3,359,976 bytes (~3.3M).
- 2026-08-09 — qos-booking: began `sink` CloudEvents notifications — the
  `DELETE_REQUESTED` slice. Deleting a booking that recorded a `sink` now delivers a
  `status-changed` CloudEvent (`type: org.camaraproject.qos-booking.v0.status-changed`,
  `data.bookingStatus: TERMINATED`, `data.statusInfo: DELETE_REQUESTED`) fire-and-forget
  over a raw TCP `http://` POST, off the request path (still `204`); an `https://` sink
  is a no-op (no TLS client — documented cut). New `src/apis/qos_booking/notifications.rs`
  (mirrors QoS Provisioning: `status_changed_event`/`sink_authorization`/`spawn_delivery`/
  `deliver`/`parse_http_sink`), and a credential side-store in `store.rs`
  (`new_event_id`/`insert_credential`/`take_credential`) so the `sinkCredential`
  (ACCESSTOKEN → Bearer, PLAIN → Basic; REFRESHTOKEN cut) authenticates the callback
  single-use and is never echoed. Confirmed the canonical CAMARA event shape against the
  camaraproject/QoSBooking spec (BookingStatusChanged / BookingStatusInfo enums). Spec:
  documented the notification on the delete op + `sink`/`sinkCredential` schemas, added
  the `QosBookingEvent` CloudEvent schema, updated the header note. Tests: 3 handler
  (delete-with-sink fires the CloudEvent; ACCESSTOKEN Bearer + PLAIN Basic callbacks,
  secret never echoed) + 9 module/store units. No new dep. `cargo test` (1766) +
  `cargo build --release` green. binary: 3.3M (3,355,072 bytes). Remaining for QoS
  Booking vwip: the SCHEDULED/ACTIVATED/NETWORK_TERMINATED/expiry transitions + TLS sink.
- 2026-08-09 — qos-booking: added the retrieve-by-device leg `POST
  /qos-booking/vwip/retrieve-device-qos-bookings` (`retrieveBookingByDevice`,
  scope `qos-booking:device-qos-bookings:retrieve-by-device`). Lists a device's
  bookings as an array of `BookingInfo` (`200`, empty array when none — CAMARA
  never 404s on an empty collection). Device is the submitted `device` id, else
  the token subject (reuses `resolve_identifier`: `device` on a line token → 422
  `UNNECESSARY_IDENTIFIER`; no device + non-line subject → 422
  `MISSING_IDENTIFIER`). Two control planes (DESIGN §7): identifier reserved-error
  suffix → canonical CAMARA error (…404 → 404); else the in-memory store, scanned
  by each booking's echoed `device` (new `store::find_by_device`, mirroring QoD's
  `retrieveSessionsByDevice`). Not scoped per client (documented cut);
  `x-correlator` echoed. Spec: added the `/retrieve-device-qos-bookings` path
  (retrieveBookingByDevice, 200 array + 422/error set, x-camarasim-scenarios),
  `RetrieveBookingsInput` schema, updated header note. Tests: 7 handler
  (per-device filter, empty→[], reserved suffix, three-legged fallback,
  missing/unnecessary identifier, bad body, auth+scope) + 1 store
  (`find_by_device`). No new dep. `cargo test` (1752) + `cargo build --release`
  green. binary: 3.2M (3,343,312 bytes). Only `sink` CloudEvents notifications
  remain for QoS Booking vwip.
- 2026-08-09 — qos-booking: added the delete leg `DELETE
  /qos-booking/vwip/device-qos-bookings/{bookingId}` (`deleteBooking`, scope
  `qos-booking:device-qos-bookings:delete`). Deletes a stored booking, evicting it
  from the in-memory store (new `store::remove`): present → `204 No Content`
  (single-use), unknown/already-deleted → `404 NOT_FOUND`; store state the only
  control plane (opaque id → no reserved-identifier plane; mirrors QoS Provisioning
  `revokeQosAssignment` / QoD `deleteSession`). CAMARA's async `202 Accepted`
  form deferred with `sink` notifications (documented cut); `x-correlator` echoed on
  `204` and `404`. Verified the canonical op set against the CAMARA `QoSBooking`
  repo: the collection query is `POST /retrieve-device-qos-bookings`
  (`retrieveBookingByDevice`), NOT a plain `GET` list — corrected the backlog note.
  Spec: added the DELETE operation (deleteBooking, 204 + error set,
  x-camarasim-scenarios) + updated the header note. Tests: 4 handler (204+get→404,
  single-use, unknown→404, auth+scope) + 1 store (`remove`). No new dep. `cargo
  test` (1743) + `cargo build --release` green. binary: 3.2M (3,331,960 bytes).
  Retrieve-by-device + `sink` notifications remain for later passes.
- 2026-08-09 — qos-booking: added the read-back leg `GET
  /qos-booking/vwip/device-qos-bookings/{bookingId}` (`getBooking`, scope
  `qos-booking:device-qos-bookings:read`). Reads a created booking back from the
  in-memory store by its opaque, server-minted `bookingId` → `200` `BookingInfo`
  verbatim / `404 NOT_FOUND`; store state the only control plane (opaque id → no
  reserved-identifier plane; mirrors QoS Provisioning `getQosAssignmentById` / QoD
  `getSession`). `store::get` promoted from test-only to a live handler dependency.
  `x-correlator` echoed on `200` and `404`. Spec: added the GET path (getBooking,
  bookingId path param, 200 `BookingInfo`, error set, x-camarasim-scenarios) +
  updated the header note. Tests: 3 new (read-back verbatim, unknown→404, auth+scope).
  No new dep. `cargo test` (1738) + `cargo build --release` green. binary: 3.2M
  (3,325,952 bytes). List + DELETE legs and `sink` notifications remain for later passes.
- 2026-08-09 — qos-booking: NEW CAMARA API. Added the create leg `POST
  /qos-booking/vwip/device-qos-bookings` (`createBooking`, scope
  `qos-booking:device-qos-bookings:create`) — the time-boxed booking sibling of
  QualityOnDemand / QoS Provisioning (ConnectivityQualityManagement subproject,
  mounted at its canonical `vwip`, verified real & un-implemented against the CAMARA
  GitHub org). Books a `qosProfile` for a device over a `startTime`/`duration` window
  in a `serviceArea`, mints a UUID-shaped `bookingId`, persists the `BookingInfo`
  (new `src/apis/qos_booking/store.rs`, no uuid/rand dep — reuses sha2), `201`.
  Control planes (DESIGN §7): identifier reserved-error suffix (`…409`→409 CONFLICT);
  trailing digits → `bookingStatus` (`…000`/none→REQUESTED, odd→SCHEDULED,
  else→ACTIVATED+startedAt); `duration` (<1→400 OUT_OF_RANGE, >31622400→400
  QOS_BOOKING.DURATION_OUT_OF_RANGE); `serviceArea` (CIRCLE + AREANAME managed —
  center range→400, radius<1→422 INVALID_AREA, uncovered areaName→422 AREA_NOT_COVERED;
  POLYGON→422 NOT_MANAGED_AREA_TYPE; unknown→400); `qosProfile` unavailable→422
  QOS_PROFILE_NOT_APPLICABLE; `sink` non-http(s)→400 INVALID_SINK; two/three-legged
  identifier rule (422 UNNECESSARY/MISSING_IDENTIFIER). Read-back/list/delete +
  `sink` notifications deferred to later passes (sink validated+echoed, not delivered
  to — documented cut); `startTime` shape-validated, not used for status (cut). Spec:
  new `specs/qos-booking/vwip/openapi.yaml` (createBooking + full schemas + scenarios),
  wired into openapi SPECS + `/` catalog. Tests: 26 new (all cases above). No new dep.
  `cargo test` (1735) + `cargo build --release` green. binary: 3.2M (3,319,560 bytes).
- 2026-08-09 — application-endpoint-registration: added the full-replace update leg
  `PUT /application-endpoint-lists/{applicationEndpointListId}`
  (`updateApplicationEndpoint`, scope `…:application-endpoints:update`) — replaces
  the endpoints under an existing id with a fresh `ApplicationEndpointsInfo` body
  → `204 No Content` (a later `GET` reads back the replacement). Shares `register`'s
  body-validation + `applicationProfileId` control planes (bad body/field → 400
  INVALID_ARGUMENT / OUT_OF_RANGE; reserved suffix → canonical error; nil UUID → 422
  UNIDENTIFIABLE_APPLICATION_PROFILE), then the opaque store-state id (unknown → 404,
  non-UUID path → 400); body validated before store state (body 400 > 404, mirroring
  the Traffic Influence / Network Access Devices PATCH convention). New atomic
  `store::replace` (existence-check-and-swap under one lock hold); no new dep. This
  **completes the Application Endpoint Registration vwip lifecycle** (register / read /
  list / update / deregister). Spec: added the `put` operation
  (204/400/401/403/404/422/429/500/503 + scenarios) on the `{applicationEndpointListId}`
  path; the `…:update` scope resolves through the shared openId scheme; header comment
  updated. Tests: 10 new (update→204 + read-back replacement, unknown→404,
  malformed-id→400, invalid-body-wins-over-404, port→OUT_OF_RANGE, reserved-…422→422,
  nil-UUID→422, update-scope→403, missing-token→401; + `store::replace` unit).
  cargo test 1709 passed; release builds. — binary: 3267952 bytes (3.2M).
- 2026-08-09 — application-endpoint-registration: added the deregister leg
  `DELETE /application-endpoint-lists/{applicationEndpointListId}`
  (`deregisterApplicationEndpoint`, scope `…:application-endpoints:delete`) —
  removes the stored registration → `204 No Content` (single-use), `404 NOT_FOUND`
  for an unknown/already-deregistered id, `400 INVALID_ARGUMENT` for a non-UUID
  path value; store state the only control plane (server-minted id → no
  reserved-identifier suffix). New `store::remove`; no new dep. Scope + response
  codes verified against the upstream CAMARA spec (deregister → 204). Spec: added
  the `delete` operation (204/400/401/403/404/429/500/503 + scenarios) on the
  `{applicationEndpointListId}` path; the `…:delete` scope resolves through the
  shared openId scheme. Tests: 6 new (deregister→204 + gone, single-use→404,
  unknown→404, malformed→400, delete-scope→403, missing-token→401). cargo test
  1699 passed; release builds. — binary: 3257648 bytes (3.2M).
  Note: the QoD/Geofencing/etc. `https://` sink TLS-delivery items remain parked —
  they need a rustls TLS client + crypto backend (ring/aws-lc-rs), which conflicts
  with the project's pure-RustCrypto, small-binary stance (Cargo.toml, DESIGN §11);
  that dependency decision is left for a human, not an unattended pass.
- 2026-08-09 — application-endpoint-registration: added the list leg
  `GET /application-endpoint-lists` (`getAllRegisteredApplicationEndpoints`,
  scope `…:application-endpoints:read`) — a `200` array of `ApplicationEndpointList`
  (empty when none — a list never 404s), ordered by `applicationEndpointListId`
  (new `store::all()`); store state the only control plane, no per-client scoping
  (documented cut). Same pass corrected the stored/returned shape to canonical
  **nested** `ApplicationEndpointList` (`applicationEndpointListId` +
  `applicationEndpointsInfo`) — verified against the upstream CAMARA spec — so the
  read-back and list legs agree with CAMARA (replacing the earlier flat
  `ApplicationEndpointsInfoResponse`; read-back `store::get` returns nested now).
  Spec: added the GET list path (array, maxItems 20, of `ApplicationEndpointList`)
  + `getAllRegisteredApplicationEndpoints` scenarios; added the
  `ApplicationEndpointList` schema and repointed `getApplicationEndpointsById`'s
  `200` to it. Tests: 5 net new (store `all()`; list containment/nested-shape,
  sorted-by-id, read-scope→403, missing-token→401) + read-back happy-path updated
  to the nested shape. cargo test 1693 passed; release builds. — binary: 3251320 bytes (3.2M)
- 2026-08-07 19:50Z — application-endpoint-registration: added the read-back leg
  `GET /application-endpoint-lists/{applicationEndpointListId}`
  (`getApplicationEndpointsById`, scope `…:application-endpoints:read`) — returns
  the stored registration verbatim (`200`), `404 NOT_FOUND` for an unknown id,
  `400 INVALID_ARGUMENT` for a non-UUID path value; reuses `store::get`, no new
  dep. Spec: added the GET path, `ApplicationEndpointListIdPath` param, and
  `ApplicationEndpointsInfoResponse` schema. Tests: 6 new (read-back happy path,
  unknown→404, malformed→400, read-scope→403, missing-token→401). cargo test
  1688 passed; release builds. — binary: 3241016 bytes (3.1M)
- 2026-08-07 — application-endpoint-registration: added a **new API**,
  `POST /application-endpoint-lists` (`registerApplicationEndpoints`) at
  `/application-endpoint-registration/vwip` (CAMARA ApplicationEndpointRegistration, wip).
  Chosen because the backlog's remaining `[ ]` leaves are all deferred for concrete reasons
  (TLS-sink cases need a rustls stack vs "keep the binary small"; IoT AREALIMIT is spatial;
  the campaign / intermediate-transition / ongoing-stream legs need a live call/provisioning
  engine), so the top *safe* unit is a new API under "Other CAMARA APIs as capacity allows".
  Diffed the ~49 implemented modules against the authoritative CAMARA org repo list
  (GitHub search): most public APIs are done; several candidates were rejected (Scam Signal
  spec is private; VoiceVerificationCode/VoiceNotification are empty placeholders; QoSBooking
  is spatial — required `serviceArea`/Area). ApplicationEndpointRegistration is the strongest
  fit: **stateful, resource-oriented, non-spatial** (its `edgeCloudZone` is a placement id,
  not a coordinate — the same treatment as the already-mounted Application Endpoint Discovery
  / Optimal Edge Discovery), and it mirrors Application Profiles' store pattern almost
  exactly. Scoped to the single **register** endpoint this pass; read/list/update/deregister
  legs recorded as remaining sub-steps. Contract confirmed from the authoritative upstream
  `application-endpoint-registration.yaml` (WebFetch): `200` returns a bare
  `ApplicationEndpointListId` (UUID string); errors 400/401/403/404/422/429. Control planes
  (DESIGN §7): no device identifier → the request body drives 400s (`applicationEndpoints`
  1..=20, per-endpoint `anyOf` one of domainName/ipv4/ipv6 + `port` 1..=65535 → OUT_OF_RANGE,
  malformed IPv4/IPv6 via `std::net`, non-UUID `edgeCloudZoneId`, multi-line/over-length
  provider/description, non-UUID `applicationProfileId`, unknown field/type); the
  `applicationProfileId` reserved-error suffix → canonical CAMARA error (UUIDs carry digits,
  mirroring Network Health Assessment); the nil UUID → 422 UNIDENTIFIABLE_APPLICATION_PROFILE.
  Opaque UUID-shaped `applicationEndpointListId` minted via SHA-256(counter‖now) (no
  uuid/rand dep, mirroring Application Profiles / QoD stores); the rendered registration is
  persisted for the later read leg. New `src/apis/application_endpoint_registration/{,store,
  vwip}.rs`; wired into `apis::routes`, the openapi `SPECS` table, and the `/` catalog.
  spec: new vendored `specs/application-endpoint-registration/vwip/openapi.yaml`
  (POST op + ApplicationEndpointsInfo/ApplicationEndpoint/EdgeCloudZone/ApplicationEndpointListId
  + error schema + `x-camarasim-scenarios`, shared-error `$ref`s). tests: +18 (store: id
  uniqueness/shape, insert/get round-trip; vwip pure units uuid-shape + single-line; and
  integration 200-with-uuid, domain+ipv6 accepted, empty-array 400, no-address 400, port
  OUT_OF_RANGE, malformed IPv4 400, non-UUID edgeCloudZoneId 400, unknown-field 400,
  non-UUID profile 400, reserved …404→404, reserved …422→SERVICE_NOT_APPLICABLE, nil→422
  UNIDENTIFIABLE_APPLICATION_PROFILE, scope 403, missing-token 401). cargo test 1683 pass
  (was 1665); cargo build --release clean; no new dep. — binary: 3,233,488 bytes (~3.08 MiB)
- 2026-08-07 17:55Z — network-access-devices: added `PATCH /reboot-requests/{rebootRequestId}`
  (`updateRebootRequest`) — the **update leg**, the last `[ ]` leaf of the reboot-request
  lifecycle. **This completes the Reboot Requests lifecycle and Network Access Devices vwip**
  (both flipped `[~]`→`[x]`). Chosen as the smallest safe increment continuing the last three
  passes' create/read/delete legs on this API: the topmost `[ ]` items across the backlog are
  all the repeatedly-deferred heavyweight "TLS (`https://` sink) delivery" cases, which need a
  large new rustls stack (conflicts with the "keep the binary small" guardrail — 7 duplicated
  raw-TCP delivery modules, no egress needed) and a TLS test server with certs (hard to reach a
  green one-pass increment) — so I journal them as still-deferred and took the safe adjacent
  slice per the prime directive ("a tiny merged improvement beats a large unfinished one").
  Body is `merge-patch+json` (RFC 7386, mirroring Traffic Influence's `patchTrafficInfluence`)
  over the mutable `atTime`/`message`; a supplied value replaces, `null` clears, and
  identity/target/audit keys (`id`/`devices`/`createdAt`/`modifiedAt`) + unknown keys are
  ignored; `modifiedAt` bumped on success. Two control planes (DESIGN §7): the request body is
  validated first (malformed `atTime` / >255 `message` → 400 INVALID_ARGUMENT, before the store
  is touched — so a body 400 beats even an unknown-id 404), then the opaque store-state id **plus**
  the stored request's **schedule state** — a pending **scheduled** reboot (carries `atTime`) →
  `200` (persists), an **immediate** reboot (no `atTime`, already fired) → `409
  NETWORK_ACCESS_DEVICES.INCOMPATIBLE_STATE` (store left unchanged), unknown id → `404`. No live
  reboot engine, so "already fired" is modelled by the absent `atTime` and `devices` isn't
  re-targeted (documented simplifications of CAMARA's scheduled-reboot semantics). New
  decline-aware atomic `store::update_with` (get-modify-write under one lock hold, closure returns
  `Ok`/`Err` so the 409 declines before mutating; no new dep — the `.patch()` MethodRouter method
  needs no extra import). spec: new `patch:` op under `/reboot-requests/{rebootRequestId}`
  (`updateRebootRequest`, 200/400/401/403/404/409/429/500/503 + merge-patch requestBody +
  `x-camarasim-scenarios`), `RebootRequestUpdate` + `NetworkAccessDevicesError` schemas (409
  code enum, mirroring `CarrierBillingError`), top-comment/info-cut prose updated (PATCH now live,
  lifecycle complete). tests: +11 (store: update_with apply/decline/missing; vwip: validate_patch
  + apply_merge pure units, scheduled-update-persists, null-clears + empty-body no-op, read-only/
  unknown ignored, immediate→409 store-unchanged, unknown-id 404, body-400-wins matrix, scope
  403/token 401 leave it intact, x-correlator on 200+404). cargo test 1665 pass (was 1654);
  cargo build --release clean, no new dep. — binary: 3,191,080 bytes (~3.04 MiB)
- 2026-08-07 — network-access-devices: added `DELETE /reboot-requests/{rebootRequestId}`
  (`deleteRebootRequest`) — the delete leg of the reboot-request lifecycle, the top
  actionable `[ ]` leaf (respecting phase order: the remaining `[ ]` items above are
  deferred for concrete reasons — TLS-sink cases conflict with "keep the binary small",
  AREALIMIT is spatial, and the campaign/intermediate-transition legs need a live
  call/provisioning engine; PATCH is the natural sibling but delete is the smaller,
  self-contained increment after last pass's read leg). Confirmed the canonical
  contract via the authoritative CAMARA NetworkAccessManagement `network-access-devices.yaml`
  (WebFetch): `deleteRebootRequest` → `204` (no content) + error set `400/401/403/404/500/503`.
  Reused the existing `store::remove` (already present + tested from the create pass):
  single-use eviction — stored id → `204 No Content`, unknown/already-deleted → `404
  NOT_FOUND`. Store state is the sole control plane (opaque server-minted id; no
  reserved-identifier plane, mirrors QoD `deleteSession` / Traffic Influence
  `deleteTrafficInfluence`). `sub`-ownership not enforced (documented cut, mirroring the
  read). `x-correlator` echoed on `204` and `404`. Combined the id route to
  `get(get_reboot_request).delete(delete_reboot_request)` (MethodRouter chain, no new
  import). Dropped the now-stale `#![allow(dead_code)]` + note in `store.rs` (all of
  insert/get/remove/new_id are live). spec: new `delete:` op under
  `/reboot-requests/{rebootRequestId}` (204 + shared error set + `x-camarasim-scenarios`),
  top-comment/info-cut prose updated (only PATCH remains deferred). tests: +5 (204-then-gone
  round-trip, single-use second-delete 404, unknown-id 404, scope 403 / token 401 leave the
  resource intact, x-correlator on 204+404). cargo test 1654 pass (was 1649);
  cargo build --release clean; no new dep. — binary: 3,178,488 bytes (~3.03 MiB)
- 2026-08-07 — network-access-devices: added `GET /reboot-requests/{rebootRequestId}`
  (`getRebootRequest`) — the read leg of the reboot-request lifecycle. Store state is
  the sole control plane (opaque server-minted id): stored → 200 verbatim, else 404.
  Reused existing `store::get`; spec: new `/reboot-requests/{rebootRequestId}` GET path
  + x-camarasim-scenarios, top-comment/cuts updated. tests: +4 (read-back / 404 / scope
  gating / x-correlator). 1649 tests green; no new dep. — binary: 3.1M (3172656 B)

2026-08-07 17:05Z — network-access-devices: begin the **stateful Reboot Requests lifecycle** — add `POST /reboot-requests` (`createRebootRequest`, scope `network-access-devices:reboot`), the top actionable `[ ]` leaf across the backlog. Chosen respecting phase order (stateless & non-spatial preferred, then stateful non-spatial before spatial/TLS): every item above it is done or deferred for a concrete reason — the rustls TLS-sink cases conflict with "keep the binary small", the AREALIMIT query is spatial, and the sponsored-data campaign / Click-to-Dial & Traffic-Influence intermediate-transition legs need a live call/provisioning engine. Reboot Requests is stateful **non-spatial** — the natural next slice after the last two passes built this API's list + by-id read. Fetched the authoritative CAMARA NetworkAccessManagement contract via `raw.githubusercontent.com` (main `network-access-devices.yaml` + the `RebootRequests`/`NAM_Common` modules): POST → `201 RebootRequest`; `RebootRequestCreate` = optional `devices`(uuid[], maxItems 100) / `atTime`(RFC 3339) / `message`(≤255); `RebootRequest` = ResourceIdentifier(`id`) + those + ResourceAudit(`createdAt`/`modifiedAt`/`createdBy`/`modifiedBy`), `required: [id, devices]` — **no `status` field**. New `src/apis/network_access_devices/store.rs` (`Mutex<HashMap>`, lock never across await, opaque UUID-v4-shaped id from counter+clock, no uuid/rand/new dep, `#![allow(dead_code)]` until the read/delete legs land — mirrors the traffic-influence store's first pass). Handler: two control planes (DESIGN §7) — reserved subject suffix → canonical CAMARA error (account-level, mirroring list/read); and `devices` matched against the subject's deterministic device set (`device_list`) — an explicit list must hold UUID-shaped ids (else 400) the subscriber owns (else 404), an omitted/empty list reboots **all** of them (materialising the schema-required `devices`). `message`/`atTime` validated (self-contained RFC 3339 parser + civil-date formatter, no dep) + echoed; malformed body/unknown field/non-JSON → 400. `createdBy`/`modifiedBy` (uuid) omitted (subject isn't a UUID — documented cut). `201` + `Location`, persisted so later GET/PATCH/DELETE read it back. Renamed the module's `LIST_SCOPE` → `SCOPE` (all three ops share `network-access-devices:reboot`). Spec: `specs/network-access-devices/vwip/openapi.yaml` — new `/reboot-requests` POST path (201 + full shared error set + `x-camarasim-scenarios`), `RebootRequestCreate`/`RebootRequest` schemas, refreshed header + Documented-cuts prose (create now live; GET/PATCH/DELETE + createdBy/modifiedBy the remaining cuts). Tests: +12 (store: id shape/uniqueness, insert/get/remove single-use; vwip: uuid+rfc3339 unit, reboot-all default + Location + audit fields, store round-trip, explicit valid target echo, message/atTime echo, unknown-UUID 404, malformed-inputs 400 matrix (non-UUID device / bad atTime / >255 message / unknown field / non-JSON), reserved subject 503, scope 403 / token 401, x-correlator on 201+400), net +12. cargo test 1645 pass (was 1633); cargo build --release clean, no new dep. — binary: 3,165,944 bytes (~3.02 MiB, +22,232 B)
2026-08-07 16:05Z — network-access-devices: add `GET /network-access-devices/{networkAccessDeviceId}` (`getNetworkAccessDevice`, scope `network-access-devices:reboot`) — the per-device read, the top actionable `[ ]` leaf across the whole backlog. Chosen respecting phase order (stateless & non-spatial preferred): every item above it is done or deferred for a concrete reason — the rustls TLS-sink cases conflict with "keep the binary small", and the AREALIMIT spatial query / sponsored-data campaign management / Click-to-Dial & Traffic-Influence intermediate-transition legs need a live call/provisioning engine. The list endpoint is fully deterministic from the token subject, so — contrary to the backlog note's "stateful; needs a store" — the read is implemented **statelessly**: it regenerates the subject's device set ([`device_list`]) and returns the device whose `id` matches the path param. Two control planes (DESIGN §7): the subject's reserved-error suffix → canonical CAMARA error (account-level, mirroring the list, so a `…503` subject → 503 regardless of id); else the id vs the subject's set — a matching id → `200` with that `NetworkAccessDevice`, any other id (unknown / another subscriber's / malformed) → `404 NOT_FOUND` (no store to tell them apart — a documented simplification of CAMARA's stateful resource read). `x-correlator` echoed on success and error. No new dep (reuses `sha2`/`errors`/`scenarios`). Spec: `specs/network-access-devices/vwip/openapi.yaml` — new `/network-access-devices/{networkAccessDeviceId}` path (get op, `format: uuid` path param, 200 `NetworkAccessDevice` example, shared errors.yaml `$ref`s, `x-camarasim-scenarios`), plus refreshed header/cut prose (the per-device read is no longer a cut; only the reboot-request lifecycle remains deferred). Code: 2nd route + `get_device` handler + refreshed module doc in `vwip.rs`. Tests: +6 router integration (id round-trips from the subject's set incl. multi-device …002; unknown & malformed id → 404; another subscriber's id → 404; reserved-suffix subject → 503; missing-scope 403 / missing-token 401; x-correlator echo on 200 + 404), net +6. cargo test 1633 pass (was 1627); cargo build --release clean. — binary: 3,143,712 bytes (~3.00 MiB, +8,264 B)
2026-08-07 12:51Z — network-access-devices: **new stateless, non-spatial CAMARA API** — mount `GET /network-access-devices` at `/network-access-devices/vwip` (CAMARA NetworkAccessManagement, wip; scope `network-access-devices:reboot`, `getNetworkAccessDevices`). Chosen because the last seven passes' PLAIN-sinkCredential sweep is complete and every remaining `[ ]` leaf is deferred for a concrete reason (rustls TLS-sink cases conflict with "keep the binary small"; the AREALIMIT spatial query, sponsored-data campaign management, and the Click-to-Dial/Traffic-Influence intermediate-transition legs need a live call/provisioning engine) — so, respecting phase order (stateless & non-spatial first), the top actionable item was a **fresh** API under "Other CAMARA APIs as capacity allows". The prior journal's claim that "the simpler non-spatial new-API space is exhausted" was premature: the CAMARA NetworkAccessManagement repo's Network Access Devices listing was unimplemented (confirmed via `raw.githubusercontent.com`). It lists a subscriber's operator-supplied access equipment (gateways/routers/access points — operator infrastructure, not end-user devices). No request body, so — like Number Verification's `GET /device-phone-number` — the token **subject** is the sole control plane (DESIGN §7): reserved suffix → canonical CAMARA error; else the subject's trailing three digits `d` (or `0` when none) drive both the device **count** (`d == 0` → 1, else `((d-1) % 3) + 1`, 1–3) and each device's **status** (device `i` → `[connected, disconnected, unavailable][(d + i) % 3]`), so `…000` is one connected gateway, `…002` two `[unavailable, connected]`, `…003` three. Each device's UUID-shaped `id` + EUI-48 `hardwareAddress` are deterministic from the subject + index (SHA-256; reuses the already-vendored `sha2`, no new dep). `serviceSite` + the inherited Commonalities `Device` end-user identifier fields are omitted for operator devices (schema-valid cut — only `id` required); the per-device read and the stateful reboot-request lifecycle are deferred (backlogged). New files: `specs/network-access-devices/vwip/openapi.yaml` (self-contained: inline `NetworkAccessDevice`/`NetworkAccessDeviceList`, shared errors.yaml `$ref`s, full `x-camarasim-scenarios`), `src/apis/network_access_devices.rs` + `src/apis/network_access_devices/vwip.rs`; wired into `apis.rs` (mod + merge + doc), `apis/openapi.rs` (SPECS), and `main.rs` catalog. Tests: 2 pure-unit (count/status digit plane; deterministic well-shaped id/MAC) + 6 router integration (happy-path single device, digit-driven two devices, reserved-suffix 404/503, missing-scope 403, missing-token 401, x-correlator echo on success + error), net +8; the shared catalog/openapi consistency tests (`catalog_lists_mounted_apis`, `catalog_spec_urls_match_served_specs_and_resolve`, `serves_every_mounted_api_spec`) still green so the new API is catalogued and its spec resolves. cargo test 1627 pass (was 1619); cargo build --release clean, no new dep. — binary: 3,135,448 bytes (~2.99 MiB, +18,024 B)
2026-08-07 15:05Z — traffic-influence: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) on the traffic-influence initial-event callback — the direct continuation of the last six passes' identical QoD → geofencing → carrier-billing → qos-provisioning → session-insights → click-to-dial increments, now applied to the **last remaining sibling** notifications module (the 14:05Z click-to-dial journal named traffic_influence as the final module still cutting PLAIN). `sink_authorization` is duplicated per-API, so no prior pass reached this one; **this completes the PLAIN sinkCredential sweep across every sink-bearing API**. It remains the smallest dependency-free actionable increment: the top `[ ]` leaves across the repo are still the repeatedly-deferred heavyweight rustls TLS-sink cases (conflict with the "keep the binary small" guardrail — no egress needed) and the spatial/campaign/background-worker functional cuts (AREALIMIT spatial query, sponsored-data campaign management, and the Click-to-Dial intermediate-transition / Traffic-Influence ongoing-stream legs that need a live call/provisioning engine); the simpler non-spatial new-API space is exhausted. `notifications::sink_authorization` now matches on `credentialType`: `ACCESSTOKEN` → `Bearer <accessToken>` (unchanged), `PLAIN` → `Basic <base64(identifier:secret)>` for a non-empty `identifier` with a present `secret` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated), else `None`; REFRESHTOKEN still a documented cut. The credential is derived once at create through this one pure helper and carried on the fire-and-forget initial-event delivery, so PLAIN now authenticates the traffic-influence callback in one change (reuses the already-vendored `base64` STANDARD engine; no new dep). Spec: `specs/traffic-influence/vwip/openapi.yaml` — `SinkCredential` schema (added `identifier`/`secret` properties + Basic-auth semantics + per-property descriptions), the API-summary notifications block, the schema description, and a new `x-camarasim-scenarios` PLAIN case (Basic on the initial-event callback). Code doc-comments refreshed likewise (notifications.rs module bullet + `sink_authorization` doc, `vwip.rs` `ValidSubscription` doc + create-site comment that claimed PLAIN was a cut). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_is_none_for_refreshtoken_and_junk`) and added an end-to-end router test (`the_initial_event_callback_carries_a_plain_sink_credential_as_basic`) asserting a real initial-event callback carries `Authorization: Basic Y2JpZDpjYnNlY3JldA==`, net +3. cargo test 1619 pass (was 1616); cargo build --release clean, no new dep. — binary: 3,117,424 bytes (~2.97 MiB, +1,376 B)
2026-08-07 14:05Z — click-to-dial: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) on the `status-changed` callbacks — the direct continuation of the last five passes' identical QoD → geofencing → carrier-billing → qos-provisioning → session-insights increments, now applied to the next sink-bearing API in backlog/phase order (`sink_authorization` is duplicated per-API, so no prior pass reached this one; the 13:05Z session-insights journal named click_to_dial/traffic_influence as the two remaining siblings still cutting PLAIN, and click_to_dial is first in backlog order). It remains the smallest dependency-free actionable increment: the top `[ ]` leaves across the repo are still the repeatedly-deferred heavyweight rustls TLS-sink cases (conflict with the "keep the binary small" guardrail — no egress needed) and the spatial/campaign/background-worker functional cuts (the AREALIMIT spatial query, sponsored-data campaign management, and the Click-to-Dial intermediate-transition / Traffic-Influence ongoing-stream legs that need a live call/provisioning engine); the simpler non-spatial new-API space is exhausted. `notifications::sink_authorization` now matches on `credentialType`: `ACCESSTOKEN` → `Bearer <accessToken>` (unchanged), `PLAIN` → `Basic <base64(identifier:secret)>` for a non-empty `identifier` with a present `secret` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated), else `None`; REFRESHTOKEN still a documented cut. Because both click-to-dial callback paths (the create-time event whose credential is derived inline from the create body, and the terminal `terminateCall` event whose sink+derived credential is taken single-use from the `callId` sink side-store) route their credential through this one pure helper, PLAIN now authenticates every click-to-dial callback in one change (reuses the already-vendored `base64` STANDARD engine; no new dep). Spec: `specs/click-to-dial/vwip/openapi.yaml` — `SinkCredential` schema (added `identifier`/`secret` properties + Basic-auth semantics), the API-summary notifications block, the `createCall`-callback/`CreateCallRequest`/`sinkCredential`-field/`terminateCall` descriptions (shared Bearer/Basic tail), a new `createCall` `x-camarasim-scenarios` PLAIN case (+ ACCESSTOKEN parity case) and a `terminateCall` scenario case. Code doc-comments refreshed likewise (notifications.rs module bullet + `sink_authorization` doc, `vwip.rs` `sinkCredential`-field comment that claimed PLAIN was a cut). Only click_to_dial is updated this pass (the helper is per-API; the last remaining sibling notifications module — traffic_influence — still cuts PLAIN). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_is_none_for_refreshtoken_and_junk`) and added an end-to-end router test (`a_plain_sink_credential_authenticates_the_callback_as_basic`) asserting a real create-time callback carries `Authorization: Basic Y2JpZDpjYnNlY3JldA==`, net +3. cargo test 1616 pass (was 1613); cargo build --release clean, no new dep. — binary: 3,116,048 bytes (~2.97 MiB, +1,568 B)
2026-08-07 13:05Z — session-insights: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) on the session-insights callbacks — the direct continuation of the last four passes' identical QoD → geofencing → carrier-billing → qos-provisioning increments, now applied to the next sink-bearing API that still cut PLAIN (session_insights is the first such sibling in backlog/phase order; `sink_authorization` is duplicated per-API, so no prior pass reached it). It remains the smallest dependency-free actionable increment: the top `[ ]` leaves across the repo are still the repeatedly-deferred heavyweight rustls TLS-sink cases (conflict with the "keep the binary small" guardrail — no egress needed) and the spatial/campaign/background-worker functional cuts (the AREALIMIT spatial query, sponsored-data campaign management, and Click-to-Dial/Traffic-Influence ongoing-stream legs that need a live call/provisioning engine); the simpler non-spatial new-API space is exhausted. `notifications::sink_authorization` now matches on `credentialType`: `ACCESSTOKEN` → `Bearer <accessToken>` (unchanged), `PLAIN` → `Basic <base64(identifier:secret)>` for a non-empty `identifier` with a present `secret` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated), else `None`; REFRESHTOKEN still a documented cut. Because the credential is derived once at `createSession` through this one pure helper and stashed in the `sessionId`-keyed side-store (then *peeked* on each `network-quality-score` delivery and *taken* single-use on the terminal `session-ended` legs — SESSION_DELETED / NETWORK_TERMINATED / SESSION_EXPIRED), PLAIN now authenticates every session-insights callback in one change (reuses the already-vendored `base64` STANDARD engine; no new dep). Spec: `specs/session-insights/vwip/openapi.yaml` — `sinkCredential` schema (added `credentialType`/`accessToken`/`accessTokenType`/`identifier`/`secret` properties + Basic-auth semantics), the header note, the `createSession` network-termination/expiry + `deleteSession` + `sendSessionMetrics` descriptions (shared Bearer/Basic tail), a new `createSession` PLAIN `x-camarasim-scenarios` case, and a new `deleteSession` PLAIN scenario case. Code doc-comments refreshed likewise (notifications.rs module bullet + `sink_authorization` doc, v0_3-style `vwip.rs` field/create-site comments that claimed PLAIN was a cut). Only session_insights is updated this pass (the helper is per-API; the remaining sibling notifications modules — click_to_dial/traffic_influence — still cut PLAIN). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_for_refreshtoken_and_junk`) and added an end-to-end router test (`a_plain_sink_credential_authenticates_the_callback_as_basic`) asserting a real delete `session-ended` callback carries `Authorization: Basic Y2JpZDpjYnNlY3JldA==`, net +3. cargo test 1613 pass (was 1610); cargo build --release clean, no new dep. — binary: 3,114,480 bytes (~2.97 MiB, +2,976 B)
2026-08-07 12:05Z — qos-provisioning: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) on the QoS-assignment status-change callbacks — the direct continuation of the last three passes' identical QoD (Phase 3) → geofencing (Phase 4) → carrier-billing (first Phase-5 sink API) increments, now applied to the next sink-bearing API in backlog/phase order (`sink_authorization` is duplicated per-API, so no prior pass reached this one). It remains the smallest dependency-free actionable increment: the top `[ ]` leaves across the repo are still the repeatedly-deferred heavyweight rustls TLS-sink cases (conflict with the "keep the binary small" guardrail — no egress needed) and the spatial/campaign/background-worker functional cuts; the simpler non-spatial new-API space is exhausted. `notifications::sink_authorization` now matches on `credentialType`: `ACCESSTOKEN` → `Bearer <accessToken>` (unchanged), `PLAIN` → `Basic <base64(identifier:secret)>` for a non-empty `identifier` with a present `secret` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated), else `None`; REFRESHTOKEN still a documented cut. Because both qos-provisioning callback paths (the non-terminal AVAILABLE-on-provisioning event that *peeks* the credential, and the terminal revoke/`DELETE_REQUESTED` + `NETWORK_TERMINATED` events that *take* it single-use from the `assignmentId` side-store) route their credential through this one pure helper, PLAIN now authenticates every qos-provisioning callback in one change (reuses the already-vendored `base64` STANDARD engine; no new dep). Spec: `specs/qos-provisioning/v0.3/openapi.yaml` — `SinkCredential` schema (added `identifier`/`secret` properties + per-property descriptions + Basic-auth semantics), the API-summary notifications block, the `createQosAssignment`/`statusChanged`-callback/`revokeQosAssignment` descriptions (shared Bearer/Basic tail), a new `createQosAssignment` `x-camarasim-scenarios` PLAIN case, and the `revokeQosAssignment` scenario result. Code doc-comments refreshed likewise (notifications.rs module bullet, v0_3.rs API-doc/`sinkCredential`-field/create-site comments that claimed PLAIN was a cut). Only qos-provisioning is updated this pass (the helper is per-API; the remaining sibling notifications modules — session_insights/click_to_dial/traffic_influence — still cut PLAIN). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_for_refreshtoken_and_junk`) and added an end-to-end router test (`a_plain_sink_credential_authenticates_the_callback_as_basic`) asserting a real revoke `DELETE_REQUESTED` callback carries `Authorization: Basic Y2JpZDpjYnNlY3JldA==`, net +3. cargo test 1610 pass (was 1607); cargo build --release clean, no new dep. — binary: 3,111,504 bytes (~2.97 MiB, +2,448 B)
2026-08-07 11:00Z — carrier-billing: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) on the charging callbacks — the direct continuation of the last two passes' identical QoD (Phase 3) then geofencing (Phase 4) increments, now applied to the next sink-bearing API in phase order (Carrier Billing is the first Phase-5 API; `sink_authorization` is duplicated per-API, so neither prior pass reached this one). It remains the smallest dependency-free actionable increment: the top `[ ]` leaves across the repo are still the repeatedly-deferred heavyweight rustls TLS-sink cases (conflicting with the "keep the binary small" guardrail — no egress needed) and the spatial/campaign/background-worker functional cuts; the simpler non-spatial new-API space is exhausted. `notifications::sink_authorization` now matches on `credentialType`: `ACCESSTOKEN` → `Bearer <accessToken>` (unchanged), `PLAIN` → `Basic <base64(identifier:secret)>` for a non-empty `identifier` with a present `secret` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated), else `None`; REFRESHTOKEN still a documented cut. Because all carrier-billing callbacks (one-step `payment-completed`, `payment-reserved`, `payment-pending-validation`, and the terminal `payment-completed`/`payment-cancelled`/`payment-denied` from the prepare-time credential side-store) route their credential through this one pure helper, PLAIN now authenticates every carrier-billing callback in one change (reuses the already-vendored `base64` STANDARD engine; no new dep). Spec: `specs/carrier-billing/v0.5/openapi.yaml` — `SinkCredential` schema (added `identifier`/`secret` properties + Basic-auth semantics + per-property descriptions), the top notifications-summary block, all five callback descriptions (shared Bearer/Basic tail), the three confirm/cancel/deny operation descriptions, two `x-camarasim-scenarios` PLAIN cases (createPayment + preparePayment), and refreshed the stale `PreparePayment` `sink`/`sinkCredential` "not acted on in this slice" note (notifications have long shipped). Code doc-comments in `notifications.rs`/`v0_5.rs` refreshed likewise (the module bullet claiming sinkCredential "not acted on" was stale). Only carrier-billing is updated this pass (the helper is per-API; the remaining sibling notifications modules — qos_provisioning/session_insights/click_to_dial/traffic_influence — still cut PLAIN). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_for_refreshtoken_and_junk`) and added an end-to-end router test (`a_plain_sink_credential_authenticates_the_charging_notification_as_basic`) asserting a real charge callback carries `Authorization: Basic Y2JpZDpjYnNlY3JldA==`, net +3. cargo test 1607 pass (was 1604); cargo build --release clean, no new dep. — binary: 3,109,056 bytes (~2.97 MiB, +2,576 B)
2026-08-07 10:05Z — geofencing-subscriptions: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) on the geofencing callbacks — the direct continuation of last pass's identical QoD increment, applied to the next sink-bearing API in phase order (Phase 4 < the Phase-5 siblings; `sink_authorization` is duplicated per-API, so QoD's PLAIN did not reach this one). It is the smallest dependency-free actionable increment: the top `[ ]` leaves across the repo remain the repeatedly-deferred heavyweight rustls TLS-sink cases (conflicting with the "keep the binary small" guardrail — no egress needed, the canonical CAMARA `PlainCredential` shape was already confirmed last pass via `raw.githubusercontent.com`) and the spatial/campaign/background-worker functional cuts. `notifications::sink_authorization` now returns `Basic <base64(identifier:secret)>` for a `credentialType: PLAIN` credential with a non-empty `identifier` and a present `secret` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated). Because all three geofencing callbacks (initial-event, movement, expiry/subscription-ended) already route their credential through this one pure helper, PLAIN now authenticates every geofencing callback in one change; ACCESSTOKEN unchanged, REFRESHTOKEN still a documented cut. Pure function, no new dependency (reuses the already-vendored `base64` STANDARD engine). Spec: `specs/geofencing-subscriptions/v0.4/openapi.yaml` — header note, `createSubscription` description + notifications callback description + `sinkCredential` schema (added `credentialType`/`accessToken`/`accessTokenType`/`identifier`/`secret` properties + Basic-auth semantics), and a new `x-camarasim-scenarios` case (PLAIN sinkCredential → `Authorization: Basic …`); inline `v0_4.rs` comments refreshed. Only geofencing is updated this pass (the helper is per-API; the remaining sibling notifications modules — qos_provisioning/session_insights/click_to_dial/traffic_influence/carrier_billing — still cut PLAIN). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_for_refreshtoken_and_junk`) and added an end-to-end router test (`initial_event_applies_the_plain_sink_credential_as_basic`) asserting a real initial-event callback carries `Authorization: Basic YWxhZGRpbjpvcGVuc2VzYW1l`, net +3. cargo test 1604 pass (was 1601); cargo build --release clean, no new dep. — binary: 3,106,480 bytes (~2.96 MiB, +2,816 B)
2026-08-07 09:10Z — quality-on-demand: extend the CloudEvents `sinkCredential` handling from ACCESSTOKEN-only to also apply **PLAIN** (RFC 7617 HTTP Basic) — a continuation of QoD's in-progress notifications backlog sub-step, and the smallest dependency-free actionable increment available (the top `[ ]` leaves are all the repeatedly-deferred heavyweight rustls TLS-sink cases, which conflict with the "keep the binary small" guardrail; the remaining functional cuts across the repo are spatial/campaign or need a live provisioning/call engine; and adding a whole new CAMARA API was infeasible this pass — camaraproject.org egress is blocked, so a canonical upstream spec couldn't be fetched, though raw.githubusercontent.com *is* reachable and was used to confirm the `SinkCredential`/`PlainCredential` schema). Verified the canonical CAMARA Commonalities `PlainCredential` shape via `raw.githubusercontent.com` (fields `identifier` + `secret`, discriminated on `credentialType`); a plain identifier/secret pair's standard application is HTTP Basic, so `sink_authorization` now returns `Basic <base64(identifier:secret)>` for a `credentialType: PLAIN` credential with a non-empty `identifier` (empty `secret` permitted — the schema requires the field, not a value; a missing `identifier`/`secret` field or empty `identifier` → unauthenticated, as before). ACCESSTOKEN behaviour unchanged; REFRESHTOKEN still a documented cut (needs a token-exchange round trip). Pure function, no new dependency (reuses the already-vendored `base64` STANDARD engine). Spec: `specs/quality-on-demand/v1/openapi.yaml` — header divergence note, `createSession` description + notifications callback description + `sinkCredential` schema (added `identifier`/`secret` properties + Basic-auth semantics) all updated, and a new `x-camarasim-scenarios` case (PLAIN sinkCredential → `Authorization: Basic …`). Only QoD is updated this pass (the `sink_authorization` helper is duplicated per-API — scoped to one API's scenario set per AGENT.md; the sibling notifications modules still cut PLAIN). Tests: split the old ACCESSTOKEN-only unit test into three (`…_for_accesstoken`, new `…_for_plain` with base64 vectors incl. empty-secret + missing-field/empty-identifier rejections, `…_for_refreshtoken_and_junk`), net +2. cargo test 1601 pass (was 1599); cargo build --release clean, no new dep. — binary: 3,103,664 bytes (~2.96 MiB, +1,488 B)
2026-08-07 08:15Z — click-to-dial: add the vwip `terminateCall` **terminate-time** `status-changed` CloudEvent — the top dependency-free actionable leaf (the `[ ]` items above are the repeatedly-deferred heavyweight rustls TLS-sink cases, the spatial AREALIMIT / campaign cuts, and background-worker "ongoing stream" features needing a live engine). Chose the request-triggered `terminateCall` termination signal, which needs no live call engine — it mirrors QoD's `deleteSession` → `DELETE_REQUESTED` exactly (a discrete event fired on the delete request, not from a background progression). Split the old bundled leaf so only the *intermediate* transitions (`callingCaller`/`callingCallee`/`connected`/spontaneous-`failed`) stay deferred. Implementation: `store` gains a **sink side-store** (`callId` → `(sink, auth)`) — `insert_sink` at create, `take_sink` (single-use) at terminate — kept apart from the `Call` map so the callback secret is never echoed by `getCall`/`getRecording`; `store::remove` now returns the removed `Call` (was `bool`) so `terminateCall` can read the participants for the event `data`. `createCall` records the sink+derived ACCESSTOKEN bearer alongside its existing create-time delivery; `terminateCall` — after the atomic `remove` (so the terminal event fires at most once vs a concurrent delete) — delivers a single terminal event (`state: disconnected`, `reason` set) fire-and-forget over raw TCP, `http://` only (`https://`/no-sink → 204 with no event, documented cut), the ACCESSTOKEN bearer applied. New `notifications::terminated_event` reuses `status_changed_event` and injects the `reason` (create-time signature untouched, no churn). Spec: `terminateCall` description + `x-camarasim-scenarios` document the terminal event (noting it is *not* an OpenAPI `callbacks` object — a bodyless DELETE can't reference the create body's `sink` via a runtime expression); `CallStatus`/`StatusChangedEvent`/`CallEventData.reason` descriptions and the API-level notifications-cut block updated (create+terminate now modelled; intermediate stream + TLS deferred). Tests: +6 (store: `remove` returns the Call, sink side-store single-use/absent; notifications: `terminated_event` shape; vwip router: terminate fires the terminal CloudEvent + content, the terminal callback bearer, terminate of a no-sink call fires nothing, https sink terminate delivers nothing). cargo test 1599 pass (was 1593); cargo build --release clean, no new dep (reuses sha2/tokio/serde_json). — binary: 3,102,176 bytes (~2.96 MiB, +4,936 B)
2026-08-07 03:40Z — Phase 5 (other CAMARA APIs): **Predictive Connectivity Data vwip** — a new stateless, **area-keyed** CAMARA API (no device identifier), the connectivity-forecast counterpart of Population Density Data. Re-checked the CAMARA org repo list (93 repos, 4 pages) against the 46 already-mounted APIs: the simpler non-spatial single-op space is exhausted (remaining un-mounted repos are heavy multi-resource/stateful — DedicatedNetworks/NetworkAccessManagement/ModelAsAService/VPNs — spatial, notification-only, or empty/archived sandbox stubs). `PredictiveConnectivityData` (wip) is a clean single-operation, area-keyed API whose geometry-is-the-control-plane shape exactly mirrors the already-implemented Population Density Data, so it's the smallest, lowest-risk, dependency-free actionable increment (chosen over the repeatedly-deferred heavyweight rustls TLS-sink leaves and the deferred spatial/campaign/background-worker cuts). Fetched + read the authoritative upstream `predictive-connectivity-data.yaml` via WebFetch (version `wip`, single op `POST /retrieve`, `retrieveConnectivity`, scope `predictive-connectivity-data:read`). New `src/apis/predictive_connectivity_data/{,vwip}.rs` mirror Population Density Data's synchronous `GEOHASHLIST` path + self-contained RFC 3339 parser (no new dep). It forecasts, per grid cell, a stack of vertical **layers** (altitude bands, `layerThickness`=30 m) each rated `GC`/`MC`/`NC`/`ND`. Control planes (DESIGN §7): first geohash reserved suffix → canonical CAMARA error (before the window); each geohash's FNV-1a hash fixes its cell (`h%7==0` → NO_DATA/all-`ND`, else ground score `h%100` degrading 15 pts/layer up); target `serviceLevel` (`C2`/`STREAM_4K`/`BEST_EFFORT`) sets rating strictness (genuine 2nd plane — `C2` never rates a layer better than `BEST_EFFORT`); `height` sets layer count (`height/30+1`, default 4, echoed as `requestedHeight`); `includeSignalStrength` toggles the per-layer dBm band; cell mix fixes `status` (all/some/none NO_DATA). Capability planes: `POLYGON`→422 UNSUPPORTED_AREA_TYPE, geohash len>9→422 UNSUPPORTED_PRECISION, >100→422 UNSUPPORTED_SYNC_RESPONSE; window checks INVALID_END_TIME/MAX_TIME_PERIOD_EXCEEDED (7 days); unknown serviceLevel/networkType/areaType, bad geohash, empty/>1000 list, precision-with-GEOHASHLIST, height∉0..=250 → 400 INVALID_ARGUMENT. Documented cuts: POLYGON, async sink/CloudEvents (202 flow), hourly slicing (single slice), absolute start-time checks, UNSUPPORTED_SERVICE_LEVEL (all 3 supported). Vendored + annotated spec `specs/predictive-connectivity-data/vwip/openapi.yaml` (RetrieveConnectivityRequest/ServiceLevel/NetworkType/Area/ConnectivityDataResponse/TimedConnectivityData/CellConnectivityData/LayerConnectivity, functional cases), served at `/…/openapi.yaml`; wired into apis.rs routes/mod/doc, openapi.rs SPECS, and the `/` catalog. Tests: 35 new (units: geohash validation, serviceLevel parse, cell determinism, layer-count-from-height, service-level monotonicity, altitude signal degradation, status summary, rfc3339; router: one-cell-per-geohash, default-layers/no-SS, all-NO_DATA area, height→layers+echo, includeSignalStrength band, serviceLevel changes ratings, reserved suffix + wins-over-window, unknown serviceLevel/networkType/areaType, POLYGON, precision, out-of-range height, too-precise geohash, malformed geohash, empty list, >100 sync, window INVALID_END_TIME/MAX_TIME_PERIOD/malformed, unknown field, missing serviceLevel, empty body, 401/403, x-correlator). cargo test 1593 pass (was 1558); cargo build --release clean, no new dep. — binary: 3,097,240 bytes (~2.95 MiB, +43,992 B)
2026-08-07 02:40Z — Phase 5 (other CAMARA APIs): **Application Endpoint Discovery vwip** — a new stateless, non-spatial, device-keyed EdgeCloud API. Re-checked the CAMARA org repo list against the 45 already-mounted APIs (the prior journal declared Traffic Influence "the last" but the org still lists un-mounted repos): `ApplicationEndpointDiscovery` was uncovered and is the endpoint-level successor to Simple/Optimal Edge Discovery (returns the concrete application endpoints — port + IP/FQDN — not just a zone), stateless and non-spatial, so it fits the phase-order preference. Fetched + vendored the authoritative upstream `application-endpoint-discovery.yaml` (version `wip`, single op `POST /retrieve-optimal-app-endpoints`, scope `application-endpoint-discovery:app-endpoints:read`). New `src/apis/application_endpoint_discovery/{,vwip}.rs` mirror Optimal Edge Discovery's device-object two-legged/three-legged identifier resolution (422 UNNECESSARY_/MISSING_IDENTIFIER, 400 empty device). Structural plane: exactly one of `appId`/`applicationEndpointsId` (both UUID; neither/both/non-UUID → 400 INVALID_ARGUMENT — a documented tightening of the upstream `anyOf`, keeping the echo unambiguous). Two control planes (DESIGN §7): the application identifier's trailing three digits `d` — `…000` → 404 NOT_FOUND (app/endpoints not registered), else `(d % 3) + 1` `ApplicationEndpoint`s (1–3) with a rank-rotating address family (fqdn / single ipv4 / single ipv6, so a 3-endpoint answer shows all three), a port, and an `edgeCloudZone`; and the resolved device identifier's reserved-error suffix → canonical CAMARA error (checked first, so the device-not-found 404 is reachable distinctly from the app-not-found 404). `DeviceResponse` (phoneNumber only) echoed only when the request `device` carried multiple identifiers (faithful to the CAMARA "only included when multiple device identifiers come in the request" rule). Deterministic endpoints/zone-ids via SHA-256 (no uuid/rand/new dep; addresses use RFC 5737/3849 documentation ranges). Vendored + annotated spec `specs/application-endpoint-discovery/vwip/openapi.yaml` (EndpointDiscoveryInfo/Result, ApplicationEndpoint anyOf address family, functional cases), served at `/…/openapi.yaml`; wired into apis.rs routes/mod/doc, openapi.rs SPECS, and the `/` catalog. Also refreshed the stale local `main`/`origin/main` tracking ref at pass start (a `git fetch` showed origin/main had advanced to the current tip; the local ref was behind — no divergence, work was already pushed). Tests: 29 new (uuid validation, endpoint count/determinism/all-family units, e164, device-count/precedence; router: single/multi endpoint, appId & applicationEndpointsId echo, device echo only-when-multiple, app …000 → 404, device reserved suffix wins, three-legged subject + reserved subject, unnecessary/missing identifier, missing/both/non-UUID app id, empty device, bad phone, unknown field, bad JSON, 401/403, x-correlator). cargo test 1558 pass (was 1529); cargo build --release clean, no new dep. — binary: 3,053,248 bytes (~2.91 MiB, +37,576 B)
2026-08-07 01:47Z — click-to-dial: add the vwip `status-changed` **create-time** CloudEvents leg (`sink` on `createCall`) — the top dependency-free actionable leaf (the remaining `[ ]` leaves are the four heavyweight rustls TLS-sink cases, deferred spatial/campaign cuts, and background-worker "ongoing stream" features). Verified the authoritative CAMARA contract via WebFetch (`camaraproject/ClickToDial`): event type `org.camaraproject.click-to-dial.v0.status-changed`; `data` = `callId`/`caller`/`callee`/`status{state,reason?}`/`recordingResult?`/`callDuration?`/`timestamp`; the `sink`/`sinkCredential` (ACCESSTOKEN bearer) ride directly on `createCall` (no subscription registry, no `initialEvent` config) — so CamaraSim delivers a **single create-time event** reflecting the `initiating` state, mirroring QoD / Session Insights' first CloudEvents pass. New `src/apis/click_to_dial/notifications.rs` copies the established dependency-free helper set (`EVENT_TYPE`/`SOURCE`/`new_event_id` (counter→UUID via sha2)/`status_changed_event`/`sink_authorization`/`spawn_delivery`/`deliver`/`parse_http_sink`); the credential is derived from the create body so no side-store is needed (the event fires synchronously within `createCall`, unlike QoD's later-fired events). `vwip.rs`: the previously-inert `sink`/`sinkCredential` fields are now read — a successful `201` with a `sink` fires the event off the request path (`http://` only; `https://` + malformed sink a no-op cut, matching the sibling APIs' first pass); errored/duplicate creates fire nothing; `INITIATING_STATE` const shared by the `201` body and the event so they never drift. Spec: `createCall` `callbacks.statusChanged` documenting the CloudEvent to `{$request.body#/sink}`, new `SinkCredential`/`StatusChangedEvent`/`CallEventData` schemas, `sink`/`sinkCredential` field + `CreateCallRequest` descriptions refreshed, two new `x-camarasim-scenarios` cases, and the API-level notifications cut rewritten (create-time event modelled; ongoing transitions + TLS deferred). Tests: +12 (notifications units: uuid-id uniqueness/shape, event CloudEvent shape, parse_http_sink, http delivery, bearer header, https no-op, sink_authorization; vwip router: create-with-sink fires the event + content + no-auth, sinkCredential bearer on the callback, create-without-sink fires nothing, https sink 201 + no delivery (timeout), errored create (reserved …404) fires nothing (timeout)). cargo test 1529 pass (was 1517); cargo build --release clean, no new dep (reuses sha2/tokio/serde_json). — binary: 3,015,672 bytes (~2.88 MiB, +12,352 B)
2026-08-07 00:55Z — traffic-influence: add the vwip `subscriptionRequest` **initial-event** CloudEvents leg — the last remaining Traffic Influence leaf and the top dependency-free actionable item (the other `[ ]` leaves are the three heavyweight rustls TLS-sink cases, deferred spatial/campaign cuts, and the ongoing state-change stream that needs a background provisioning worker). Verified the authoritative CAMARA contract via WebFetch (`camaraproject/TrafficInfluence` — EdgeCloud is deprecated, APIs moved to per-API repos): `SubscriptionRequest` requires `sink`(`^https://.+$`)/`protocol`/`types`/`config`(`subscriptionDetail` required, optional `initialEvent`/`subscriptionExpireTime`/`subscriptionMaxEvents`); event type `org.camaraproject.traffic-influence.v1.traffic-influence-change`; `SinkCredential` = PLAIN/ACCESSTOKEN/REFRESHTOKEN. Scoped to the **initial event** (`config.initialEvent: true`), mirroring QoD/QoS-Provisioning's first notification pass. New `src/apis/traffic_influence/notifications.rs` (copies the established dependency-free helper set: `traffic_influence_change_event`/`sink_authorization`/`spawn_delivery`/`deliver`/`parse_http_sink`) — the CloudEvent `data` is the created `TrafficInfluence` verbatim (faithful to `TrafficInfluenceNotification` inheriting the resource; `selected_appInstanceId`/`deviceResponse` a documented cut). `vwip.rs`: `PostTrafficInfluence` now parses an optional `subscriptionRequest` (shared by the per-device create via the flattened base); new `validate_subscription` (in `validate_base`, so a resource-field 400 wins, and both 400s win over the `appId` reserved-error plane) requires `protocol: HTTP` (only delivery protocol — MQTT/AMQP/… → 400), the single change-event `types`, and `config.subscriptionDetail`; `sink` relaxed to accept `http://` (loopback receivers) + `https://` (upstream-mandated) but delivered to `http://` only (no TLS client — `https://` a no-op cut). `finalize` fires one initial `traffic-influence-change` event to the sink (fire-and-forget, off the request path) when `initialEvent` is set, with the ACCESSTOKEN `sinkCredential` bearer applied (never echoed). Self-contained `rfc3339_utc`/`now_unix_secs`/`civil_from_days`/`is_valid_sink` helpers (no chrono/time dep). Spec: added `subscriptionRequest`+`SubscriptionRequest`/`SubscriptionConfig`/`SinkCredential`/`SubscriptionEventType` schemas to `specs/traffic-influence/vwip/openapi.yaml`, new create `x-camarasim-scenarios` cases, refreshed header divergence notes (initial-event now modelled; ongoing-stream/TLS deferred). Tests: +13 (notifications unit: event shape / parse_http_sink / http delivery / auth header / sink_authorization / https no-op; vwip: validate_subscription accept+reject matrix, is_valid_sink, initial-event fires the CloudEvent over the router, sinkCredential bearer on the callback, initialEvent:false fires nothing (timeout), https sink still 201 + no-op, malformed subscriptionRequest → 400). cargo test 1517 pass (was 1504); cargo build --release clean, no new dep. — binary: 3,003,320 bytes (~2.86 MiB, +34,736 B)
2026-08-06 23:20Z — traffic-influence: add vwip `POST /traffic-influence-devices` (`postTrafficInfluenceDevice`, scope `traffic-influence:traffic-influence-devices:write`) — the per-device create variant, the natural next slice after create+read+delete+PATCH and the smallest dependency-free actionable leaf (the remaining `[ ]` leaves are the three heavyweight rustls TLS-sink cases and Traffic Influence's own deferred `subscriptionRequest` CloudEvents). Upstream `PostTrafficInfluenceDevice` extends `PostTrafficInfluence` with a required `device`; refactored the create handler into shared `validate_base` (base-field validation → `ValidInput`) + `finalize` (appId reserved-error/state control planes → mint/persist/`201`+`Location`), so `postTrafficInfluence` and the new `postTrafficInfluenceDevice` share one code path. The device (`minProperties: 1`; phoneNumber E.164 / networkAccessIdentifier / ipv4Address{publicAddress required, publicPort range-checked} / ipv6Address, `deny_unknown_fields`) is validated only — for privacy it is **never echoed nor persisted** (upstream: "if a resource is related to a user, the parameter Device is not exchanged"), and `appId` stays the sole control plane. Base fields validated before the device, so a base 400 wins; both 400s win over an appId reserved-suffix scenario. Creates the same `TrafficInfluence` resource in the shared store → `201` (no device in the body), `Location` under `/traffic-influences`, readable back via `getTrafficInfluence`. New local `is_valid_e164` (mirrors connected_network_type). Spec: added the `/traffic-influence-devices` POST path (postTrafficInfluenceDevice, 201/400/401/403/404/409/422/429/500/503, `x-camarasim-scenarios`) + `Device`/`DeviceIpv4Addr`/`PostTrafficInfluenceDevice` (allOf PostTrafficInfluence + required device) schemas; refreshed header/divergence notes (device create now mounted, privacy cut documented). Tests: 13 new (1 unit `validate_device` accept/reject across all identifier kinds + minProperties + bad port; 12 router: happy-path 201 with device-not-echoed + read-back verbatim, state-from-appId-tail, reserved suffixes, each identifier kind accepted, missing/empty device 400, malformed identifiers incl. unknown-key 400, publicPort OUT_OF_RANGE, shared base-field validation, device-scope required (collection write scope → 403), missing-token 401, non-JSON 400, x-correlator on 201+404). cargo test 1504 pass; cargo build --release clean, no new dep. — binary: 2,968,584 bytes (~2.83 MiB, +~35.7 KB)

2026-08-06 22:37Z — traffic-influence: add vwip `PATCH /traffic-influences/{trafficInfluenceID}` (`patchTrafficInfluence`, scope `traffic-influence:traffic-influences:write`) — the update leg of the resource lifecycle, the natural next slice after create+read+delete and the smallest dependency-free actionable leaf (the remaining `[ ]` leaves are the three heavyweight rustls TLS-sink cases and the deferred per-device-create / subscriptionRequest / AREALIMIT / campaign cuts). Body is a JSON **merge-patch** (`application/merge-patch+json`, RFC 7386) over the mutable placement/filter fields: a supplied field replaces, an explicit `null` clears an optional field, and identity/read-only fields (`trafficInfluenceID`/`appId`/`state`) plus unknown keys are ignored (mirrors create ignoring read-only fields). CamaraSim runs no provisioning worker, so PATCH does **not** re-derive lifecycle `state` — the resource keeps its create-time `state` (documented cut). Two control planes (DESIGN §7): the request body (validated first — malformed `apiConsumerId`/`appInstanceId`/`edgeCloudRegion`/`edgeCloudZoneId`/filter or non-object body → 400 INVALID_ARGUMENT, `sourcePort`/`destinationPort` ∉ 0..=65535 → 400 OUT_OF_RANGE) and the opaque, operator-minted `trafficInfluenceID`'s store state (present → 200 updated & persisted, unknown/already-deleted → 404 NOT_FOUND) — body checked before the store so a body 400 wins over the unknown-id 404 (mirrors Application Profiles `updateApplicationProfile`). New atomic `store::update_with` (get-modify-write under one lock hold, never across await, returns the updated clone or None — no new dep). `x-correlator` echoed on every response. Spec: added the `patch` op (merge-patch requestBody, `200`/`400` responses, `PatchTrafficInfluence` schema with nullable optionals, `x-camarasim-scenarios`) to the `/traffic-influences/{trafficInfluenceID}` path item in `specs/traffic-influence/vwip/openapi.yaml`. Tests: 14 new (4 pure units: validate_patch accepts/ignores read-only, null-clears, rejects malformed incl. OUT_OF_RANGE, apply_merge sets/clears; 1 store unit update_with; 9 router: mutable-field update+persist read-back, null-clear leaves siblings, empty-body no-op 200, read-only ignored, unknown-id 404, body-400-wins-over-404, out-of-range port, non-JSON 400, missing-token 401, wrong-scope 403 with resource surviving, x-correlator on 200+404). cargo test 1491 pass; cargo build --release clean, no new dep. — binary: 2,931,976 bytes (~2.80 MiB, +~16 KB)
2026-08-06 21:34Z — traffic-influence: add vwip `DELETE /traffic-influences/{trafficInfluenceID}` (`deleteTrafficInfluence`, scope `traffic-influence:traffic-influences:delete`) — the delete leg of the resource lifecycle, the natural next slice after create+read. Verified the authoritative CAMARA `traffic-influence.yaml` via WebFetch: DELETE is **asynchronous** (`202 Accepted`, resource → `deletion in progress`), scope `…:delete`, path param `trafficInfluenceID`. New `store::remove` (atomic check-and-remove under the store lock, never across await, no new dep — mirrors Click to Dial `terminateCall` / Blockchain delete): a present resource → `202 Accepted` (CamaraSim has no background lifecycle worker, so it evicts synchronously — the `deletion in progress`/`deleted` steady states are a documented cut; a read after delete → 404), single-use so a second delete → `404`; unknown/never-created id → `404 NOT_FOUND`. Opaque, operator-minted `trafficInfluenceID` → store state is the only control plane (no reserved-identifier plane, like the read leg). `x-correlator` echoed on success and error. Chose this over the three still-deferred `[ ]` TLS-sink leaves (QoD / QoS-Provisioning / Session-Insights `https://` sink delivery — each needs a heavyweight rustls TLS client vs the raw-TCP, HTTP-client-free CloudEvents design and the "keep the binary small" guardrail) and the deferred AREALIMIT/campaign/subscriptionRequest cuts — the smallest, lowest-risk, dependency-free actionable leaf. Remaining Traffic Influence leaves: `PATCH` update, per-device create, subscriptionRequest CloudEvents (deferred). Spec: added the `delete` op + `202` response + `x-camarasim-scenarios` to the `/traffic-influences/{trafficInfluenceID}` path item in `specs/traffic-influence/vwip/openapi.yaml`; refreshed header/divergence notes (create+read+delete now mounted; async-delete cut documented). Tests: 7 new (1 store unit `remove_evicts_once_then_reports_absent`; 6 router: create→delete 202→read 404 round-trip, single-use second-delete 404, unknown-id 404, missing-token 401, delete-scope 403 with resource surviving, x-correlator on 202 + 404). cargo test 1475 pass; cargo build --release clean, no new dep. — binary: 2,915,784 bytes (~2.78 MiB, +~6.1 KB)
2026-08-06 20:48Z — traffic-influence: add vwip `GET /traffic-influences/{trafficInfluenceID}` (`getTrafficInfluence`, scope `traffic-influence:traffic-influences:read`) — the read-back leg of the resource lifecycle, the natural next slice after last pass's create. The created resource was already persisted in the shared in-memory `store` (rendered `TrafficInfluence` JSON); this pass wires `store::get` to a new handler mounted at the item route (`:traffic_influence_id`). Opaque, operator-minted `trafficInfluenceID` → store state is the only control plane (no reserved-identifier plane — mirrors QoD `getSession` / Click to Dial `getCall` / Carrier Billing `retrievePayment`): a stored resource → `200` returned verbatim; an unknown/never-created id → `404 NOT_FOUND`. `x-correlator` echoed on success and error. Removed the now-stale `#![allow(dead_code)]` from store.rs (`get` is live). Spec: added the `/traffic-influences/{trafficInfluenceID}` GET path + `TrafficInfluenceIDPath` param + scenarios, updated header/divergence notes (create+read now mounted; PATCH/DELETE/per-device/subscriptionRequest still deferred). Tests: 5 new endpoint tests (verbatim read-back of the full optional set, unknown-id 404, missing-token 401, write-scope-can't-read 403, x-correlator on 200 + 404). cargo test 1468 pass; cargo build --release clean, no new dep. — binary: 2,909,696 bytes (~2.77 MiB, +~6.3 KB)
2026-08-06 20:01Z — Phase 5 (other CAMARA APIs): **Traffic Influence vwip** — a new resource-oriented, stateful EdgeCloud traffic-steering API (the last CAMARA API with a released/wip spec not yet mounted; confirmed against the APIBacklog + the authoritative `traffic-influence.yaml` via WebFetch — the simpler stateless/non-spatial space is exhausted). Scoped to the create leg only: `POST /traffic-influences` (`postTrafficInfluence`, scope `traffic-influence:traffic-influences:write`) live at `/traffic-influence/vwip/traffic-influences`. New `src/apis/traffic_influence/{,store,vwip}.rs` mirror the sponsored_data/carrier_billing resource-store pattern (`Mutex<HashMap>`, lock never held across await, no new dep; store persists the rendered resource for a future `getTrafficInfluenceById`). An API consumer (`apiConsumerId`) names an app (`appId`) + optional edge placement (`appInstanceId`/`edgeCloudRegion`/`edgeCloudZoneId`, source/destination traffic filters) → `201` with a minted UUID `trafficInfluenceID`, the placement echoed, a lifecycle `state`, and a `Location` header. Two control planes (DESIGN §7) on `appId` (a hex UUID, caller-controlled digits): reserved error suffix → canonical CAMARA error; else trailing three digits `d` → state (`d%3`: 0→ordered/1→created/2→active). Validation → 400 INVALID_ARGUMENT (missing/malformed ids or non-JSON body) / OUT_OF_RANGE (port outside 0..=65535). Vendored + annotated spec `specs/traffic-influence/vwip/openapi.yaml` (PostTrafficInfluence/TrafficInfluence schemas, State enum, functional cases), served at `/…/openapi.yaml`; wired into apis.rs routes/mod, openapi.rs SPECS, and the `/` catalog. Read/update/delete, per-device create, and subscriptionRequest CloudEvents deferred (documented cuts). Tests: 16 new (uuid/state/build_response units, id uniqueness, store round-trip, happy path + Location + persistence, state-from-tail, optional echo, reserved suffixes, required/optional validation, port ranges, bad JSON, auth 401/403, x-correlator). cargo test 1463 pass; cargo build --release clean, no new dep. — binary: 2,903,384 bytes (~2.77 MiB, +~35 KB)
2026-08-06 18:59Z — Phase 5 (other CAMARA APIs): **Most Frequent Location vwip** — a new stateless, device-keyed API (the only non-deferred backlog item left was "Other CAMARA APIs as capacity allows"; verified the authoritative CAMARA `most-frequent-location.yaml` spec via WebFetch since ~all simpler non-spatial APIs are already done). `POST /verify` (`verifyFrequentLocation`, scope `most-frequent-location:verify`) live at `/most-frequent-location/vwip/verify`: answers `{ score: 0..=100 }` for how frequently a device resides within a supplied `geoReference`, never the location. New `src/apis/most_frequent_location/{,vwip}.rs` mirrors connected_network_type's device-object two-/three-legged identifier resolution (422 UNNECESSARY_/MISSING_IDENTIFIER, 400 empty device). Two control planes (DESIGN §7): `geoReference` validated first (COVERAGE_ZONE lat/long range → 400 OUT_OF_RANGE; POSTAL_CODE `00000` → 400 `MOST_FREQUENT_LOCATION.POSTAL_CODE_NOT_VALID`; unknown type/foreign fields → 400 INVALID_ARGUMENT), then the resolved identifier's reserved-error suffix → canonical CAMARA error (`…422`→SERVICE_NOT_APPLICABLE); else `score = (identifier trailing three digits + area offset) % 101` — both device and area are genuine planes (area offset = round(|lat|)+round(|long|) for a zone, postal trailing-three for a code). Vendored + annotated spec `specs/most-frequent-location/vwip/openapi.yaml` (GeoReference discriminator on `type`, functional cases), served at `/…/openapi.yaml`; wired into apis.rs routes/mod, openapi.rs SPECS, and the `/` catalog. IDENTIFIER_NOT_FOUND / INFORMATION_NOT_AVAILABLE / UNSUPPORTED_IDENTIFIER sub-cases folded into the shared reserved-suffix codes (documented cut). Tests: 23 new (score/area unit maths, coverage-zone & postal happy paths, area-as-second-plane, reserved suffixes incl. …422, two-/three-legged resolution, geoReference validation, auth, x-correlator). cargo test 1447 pass; cargo build --release clean, no new dep. — binary: 2,868,400 bytes (~2.73 MiB, +~28 KB)

2026-08-06 17:51Z — click-to-dial: `createCall` becomes **stateful** — the top remaining actionable, dependency-free leaf: a re-create of a still-live call for the same `caller`/`callee` pair now returns `409 ALREADY_EXISTS` instead of overwriting (the `callId` is deterministic from the pair, so a duplicate id = the same live call). Changed `store::insert` → atomic `store::insert_new(id, call) -> bool` (check-and-insert under one lock hold, never across await, no new dep — mirrors blockchain_public_address's `insert`); create maps `false` → 409 ALREADY_EXISTS; a `terminateCall` evict makes the pair creatable again. A `callee` reserved suffix `…409` still yields the canonical CONFLICT (distinct code). Spec: createCall `409` now an inline response (ALREADY_EXISTS + reserved-suffix CONFLICT examples) + new scenario case + description; removed the stale "Deterministic re-create" cut. Tests: 3 new endpoint tests (409 on re-create + call unchanged; re-creatable after terminate; x-correlator on 409), store unit test rewritten (refuses duplicate, preserves live call, re-creatable after evict); gave 3 existing happy-path tests unique callees (the shared store made the old identical pair collide). Skipped the deferred cuts as before (TLS `https://` sinks need rustls; AREALIMIT spatial; campaign-management; `status-changed` sinks). Remaining Click to Dial leaf: `status-changed` CloudEvents (deferred). cargo test 1424 pass; cargo build --release clean, no new dep. — binary: 2,839,992 bytes (~2.71 MiB, +~2.3 KB)

2026-08-06 16:52Z — click-to-dial: add vwip `GET /calls/{callId}/recording` (getRecording, scope `click-to-dial:recordings:read`) — 200 RecordingResource (base64 silent-WAV `content`, `audio/wav`) for a `recordingEnabled:true` call; 404 NOT_FOUND for a non-recorded or unknown call; spec + scenarios updated; skipped the three top-most `[ ]` TLS-sink items (QoD/QoS-Provisioning/Session-Insights) — each needs a new rustls TLS client + TLS-server test scaffolding, an unsafe fit for one autonomous pass. cargo test 1421 pass; no new dep (reuses base64). — binary: 2,837,672 bytes (~2.71 MiB)

- 2026-08-06 — Phase 5 (stateful delete): **Click to Dial vwip —
  `DELETE /calls/{callId}`** (`terminateCall`, `click-to-dial:calls:delete`), the
  natural next slice after `getCall`. New `store::remove` (atomic check-and-remove
  under the store lock, **no new dep**): a present call → `204 No Content` and is
  evicted; unknown/already-terminated id → `404 NOT_FOUND` (single-use, so a later
  `getCall`/`terminateCall` is a 404). Opaque `callId` → store state is the only
  control plane (mirrors `getCall`); no `sink` signalling (deferred). `x-correlator`
  echoed. Left the higher-priority backlog leaves alone as before: the three
  TLS-sink cases still need a rustls TLS client (heavy dep vs the raw-TCP,
  HTTP-client-free CloudEvents design — not a small pass), and `AREALIMIT` /
  campaign-management / `status-changed` sinks are deferred cuts; picked the top
  remaining actionable, dependency-free leaf. Deferred within Click to Dial:
  `getRecording` + the `409 ALREADY_EXISTS` duplicate-call case. Spec: `delete`
  op on `/calls/{callId}` (`204` response + `terminateCall` `x-camarasim-scenarios`)
  in the vendored `click-to-dial/vwip` spec; description/cuts updated
  (catalog↔spec contract test still green). 6 new tests (5 endpoint + 1 store
  unit); `cargo test` 1414 green; `cargo build --release` clean. — binary:
  2,827,104 bytes (~2.83 MB, +~5.7 KB, no new dep)
- 2026-08-06 — Phase 3/5 (stateful read-back): **Click to Dial vwip —
  `GET /calls/{callId}`** (`getCall`, `click-to-dial:calls:read`), making the API
  stateful. The immediately-preceding pass added the stateless `createCall`; this
  is the natural next slice. `createCall` now **persists** the created `Call` in a
  new in-memory store (`src/apis/click_to_dial/store.rs`; `Mutex<HashMap>`, lock
  never held across `.await`, **no new dep**), and `getCall` reads it back verbatim
  (`200`) or `404 NOT_FOUND` for an unknown/never-created id. Opaque `callId` →
  store state is the only control plane (no reserved-identifier plane here; mirrors
  QoD `getSession` / Carrier Billing `retrievePayment`). `createCall`'s `201` is
  unchanged (a re-create overwrites the identical deterministic value; `409
  ALREADY_EXISTS` still deferred). Skipped the three deferred TLS-sink leaves
  (`https://` sinks need a rustls TLS client — a heavy dep vs the project's raw-TCP,
  HTTP-client-free CloudEvents design; not a small/testable pass) and the spatial
  `AREALIMIT` / campaign-management cuts. Spec: `getCall` path + `CallId` param +
  `getCall` `x-camarasim-scenarios` in the vendored `click-to-dial/vwip` spec
  (catalog↔spec contract test still green). 7 new tests (5 endpoint + 2 store
  units); `cargo test` 1408 green; `cargo build --release` clean. — binary:
  2,821,368 bytes (~2.82 MB, +~7.5 KB, no new dep)
- 2026-08-06 — Phase 5 ("other APIs"): **Click to Dial vwip — new
  two-legged CAMARA API: `POST /calls`** (`createCall`,
  `click-to-dial:calls:create`). All remaining in-progress backlog leaves are
  the same deferred cuts (TLS `https://` sinks needing rustls — no clean
  pure-Rust TLS given the project's ring/OpenSSL-free design, §11; spatial IoT
  `AREALIMIT`; Sponsored Data campaign-management), so — respecting phase order
  (stateless/non-spatial preferred) — picked the first endpoint of a not-yet-
  mounted CAMARA API. Verified the whole GitHub org repo list against the 44
  already-mounted; the remaining uncovered stateless-ish, non-spatial candidate
  was Click to Dial (PredictiveConnectivityData/RainfallIntensity are spatial;
  bookings/management APIs are heavier state). Fetched + vendored the canonical
  upstream contract, trimmed to the one operation implemented. Two-legged/
  business-facing (both participants in the body, like Verified Caller — no
  identifier dance). Callee is the identifier: reserved suffix → canonical CAMARA
  error; `…000` callee → 422 CALLEE_NOT_AVAILABLE, `…000` caller → 422
  CALLER_NOT_AVAILABLE (checked first); `recordingEnabled` + `…777` callee → 422
  RECORDING_NOT_SUPPORTED (`777` alone is reachable); equal numbers → 422
  SAME_CALLER_CALLEE; non-E.164 → 422 INVALID_PHONE_NUMBER; missing/unknown-field/
  bad-body → 400 INVALID_ARGUMENT. `Call { status: initiating }`; `callId`
  deterministic UUID from the pair (reuses `sha2`, **no new dep**). Stateless
  create — persistence + `getCall`/`terminateCall`/`getRecording` + the 409
  ALREADY_EXISTS case + `status-changed` notifications deferred (documented cuts).
  Wired into `apis.rs` router + `openapi.rs` served specs + `/` catalog. Spec:
  new `specs/click-to-dial/vwip/openapi.yaml` (createCall path, CreateCallRequest/
  Call/Party/CallStatus/ClickToDialError schemas + `x-camarasim-scenarios`). 19
  new tests; `cargo test` 1401 green; `cargo build --release` clean. — binary:
  2,813,880 bytes (~2.7 MB, +~29 KB, no new dep)
- 2026-08-06 — Phase 5 ("other APIs"): **Sponsored Data vwip — `DELETE
  /sponsorship/{sponsorId}/{campaignId}/{sessionId}/revoke`** (`revokeSponsorship`,
  new CamaraSim-assigned scope `sponsored-data:sponsorship:delete`), the last
  remaining Sponsored Data operation bar the deferred campaign-management set. The
  three earlier-phase `[ ]` leaves are the same deferred TLS-sink infra (a
  `https://` sink needs a rustls TLS client + a TLS test harness — a dedicated
  dependency-adding pass, not a small increment) so I took this clean stateful
  single-endpoint DELETE instead. Confirmed the upstream contract by fetching the
  CAMARA SponsoredData spec: `DELETE …/revoke` → `200 { sponsorId, campaignId,
  sessionId, phoneNumber, startTime, endTime, requestResult:"successful_revocation" }`.
  Store state is the only control plane (like `getSessionStatus`): a matching id
  is **evicted** (single-use) → 200; unknown id, or a `sponsorId`/`campaignId`
  mismatch → 404 (a mismatch is left in place). New atomic
  `store::remove_matching` (check-and-remove under one lock, no new dep). Revoke
  evicts, so `getSessionStatus`'s `endReason` `session_revoked` stays
  documented-but-unreached. Spec: added the `revoke` path + `RevokedSponsorship`
  schema + `x-camarasim-scenarios`, refreshed the header/endReason notes in
  `specs/sponsored-data/vwip/openapi.yaml`. Tests: +6 vwip (pure body shape;
  evict+single-use, unknown→404, mismatch keeps session, 401, 403, x-correlator)
  + 1 store unit (`remove_matching`). `cargo test` 1379 green; `cargo build
  --release` ok. — binary: 2.78 MB (2,784,904 B, +~10.4 KB, no new dep)
- 2026-08-06 — Phase 5 ("other APIs"): **Sponsored Data vwip — `GET
  /sponsorship/{sponsorId}/{campaignId}/{sessionId}/session-status`**
  (`getSessionStatus`, `sponsored-data:sponsorship:read`), making the API
  **stateful**. New in-memory session store (`src/apis/sponsored_data/store.rs`;
  `Mutex<HashMap>`, no new dep); `startSponsorship` now persists the granted
  session so status reads it back. Control planes (DESIGN §7): opaque `sessionId`
  → store state (unknown, or `sponsorId`/`campaignId` not matching the stored
  session → 404 NOT_FOUND); stored `phoneNumber` tail → `dataVolumeConsumed =
  d%(grant+1)` / `dataVolumeAvailable`; granted window → `sessionStatus`
  (past `endTime` → inactive/`validity_expired`; grant fully consumed →
  inactive/`data_exhausted`; else `active`). Spec: added the `getSessionStatus`
  path + `SponsorshipSessionStatus` schema + `x-camarasim-scenarios` to
  `specs/sponsored-data/vwip/openapi.yaml`. Tests: +9 (pure status derivation;
  read-back, data-exhausted, unknown/mismatched → 404, 401, 403, x-correlator) +
  1 store unit. `cargo test` 1371 green; `cargo build --release` ok. — binary:
  2.77 MB (2,774,488 B, +~17.6 KB, no new dep)
- 2026-08-06 — Phase 5 ("other APIs"): **Sponsored Data vwip — new
  phone-number-keyed CAMARA API: `POST /sponsorship`** (`startSponsorship`,
  `sponsored-data:sponsorship:create`). All remaining in-progress backlog leaves
  were deferred cuts (TLS `https://` sinks needing rustls; spatial IoT
  `AREALIMIT`) and every clean stateless/non-spatial CAMARA repo is already
  mounted, so — respecting phase order — picked the simplest *first endpoint* of
  a not-yet-covered API: Sponsored Data's `POST /sponsorship`, a non-spatial,
  phone-number-keyed sponsorship-session create (mirrors QoD's `createSession`
  shape). Fetched + vendored the canonical upstream contract, trimmed to the one
  operation implemented. Three control planes (DESIGN §7): `phoneNumber`
  reserved-error suffix; `dataVolume` (1–1000 MB, default 50); `duration`
  (1–1440 min, default 10, → `endTime`). Required-field patterns validated
  (email-ish `sponsorId`, `UUID@domain` `campaignId`, E.164 `phoneNumber`, v4
  `callbackToken`) → 400; out-of-range volume/duration → 400 OUT_OF_RANGE. No
  persistence yet (the `201` is fully determined by the request) — session store
  + `session-status`/`revoke`, the `webhookUrl` callback, and campaign management
  deferred; scope CamaraSim-assigned (wip contract has no securitySchemes).
  Self-contained validators + RFC 3339 formatter + UUID minter — **no new
  dependency** (reuses `sha2`). Wired into `apis.rs` router + `openapi.rs` served
  specs + `/` catalog. 16 new tests; `cargo test` 1362 pass, `cargo build
  --release` green. — binary: 2,756,840 bytes (~2.76 MB, +~28 KB, no new dep)
- 2026-08-06 — Phase 5: **Network Traffic Analysis vwip — new stateless,
  non-spatial CAMARA API: `GET /traffic-analysis`** (`getTrafficAnalysis`,
  `network-traffic-analysis:traffic-analysis:read`). All in-progress backlog leaf
  items were deferred cuts (TLS `https://` sinks needing rustls; spatial IoT
  `AREALIMIT`), so per phase discipline picked a new stateless/non-spatial API:
  the NetworkInsights suite's Network Traffic Analysis, the traffic counterpart of
  the already-done Network Health Assessment (which had flagged it "out of scope").
  Vendored the canonical CAMARA spec (fetched from the NetworkInsights repo) and
  annotated it with the parameter-driven cases; inlined the Commonalities
  `DateTime`/`Page`/`PerPage`/`Pagination` schemas so it resolves against the
  shared `errors.yaml`/`auth`. Three control planes (DESIGN §7): `networkId`
  reserved-error suffix; `networkId` trailing digits → app count (`(d%5)+1` from a
  fixed DPI catalog) + traffic scale (`…000` → no-data empty `records`); window ×
  `frequency` (DAY/HOUR) → time-slot count (min 1, capped at 100). `app` filter +
  `page`/`perPage` paging. Server stays non-blocking; **no new dependency**
  (self-contained RFC 3339 parser+formatter, mirroring the sibling modules).
  Wired into `apis.rs` router + `openapi.rs` served specs + `/` catalog. 16 new
  tests; `cargo test` 1346 pass, `cargo build --release` green. — binary: 2,728,696 bytes (~2.7 MB)
- 2026-08-06 — Phase 5: **IoT SIM Fraud Prevention vwip — stateful `IMEIBIND`
  round-trip: `POST /bind` + `POST /unbind` + `query` store integration**. Took
  the top unclaimed sub-item of the in-progress IoT SIM API (bind/unbind are
  stateful → higher priority than the remaining spatial `AREALIMIT` cut and the
  TLS-sink items). New in-memory binding store
  (`src/apis/iot_sim_fraud_prevention/store.rs`; `Mutex<HashMap>` identifier→IMEI,
  lock never held across await, **no new dep**). `bindDeviceImei`
  (scope `iot-sim-fraud-prevention:bind`) records the SIM↔IMEI association →
  `200 { bound: true }` (idempotent); `unBindDeviceImei`
  (`…:unbind`) removes it → `200 { unbound: true }`, or `422
  UNNECESSARY_UNBIND_IMEI` when nothing is bound; `query` now prefers a stored
  binding (BOUND with the stored IMEI) over its stateless trailing-digit default,
  so bind→query→unbind is coherent. Same identifier resolution + two-/three-legged
  422 rule + reserved-error plane as `query`; `bindType`/`unBindType` trimmed to
  `[IMEIBIND]` → `AREALIMIT` → 400 INVALID_ARGUMENT. Vendored spec extended with
  `/bind` + `/unbind` paths, Bind/UnBind request+response schemas, `BindType`/
  `UnBindType` enums, and `BindBadRequest400`/`UnbindBadRequest400`/`Unbind422`
  responses (all `$ref`s resolve; wired via the existing served-spec table). 16
  new tests (store unit, full round-trip, idempotency, unnecessary-unbind,
  reserved suffixes, AREALIMIT/missing-type 400s, two-/three-legged 422s,
  three-legged bind↔query coherence, scope 403s, 401, x-correlator). Full suite
  1330 green; `cargo build --release` clean. Deferred: `AREALIMIT` (spatial).
  — binary: 2.6M (2,693,344 bytes, +~23 KB, no new dep)
- 2026-08-06 — Phase 5 ("other APIs"): **IoT SIM Fraud Prevention vwip — `POST
  /query` (`IMEIBIND`)** (new CAMARA IoTSIMFraudPrevention API, version `wip`).
  Chose a stateless, non-spatial, device-identifier-keyed slice over the
  remaining TLS-sink items (heavy rustls dep, and all stateful → lower priority)
  and the other unimplemented CAMARA repos (bind/unbind/edge/IoT ones are
  stateful or spatial). Fetched + vendored the canonical upstream contract,
  trimmed to the one operation implemented: `query` (scope
  `iot-sim-fraud-prevention:query`), `QueryType` enum cut to `[IMEIBIND]` so an
  `AREALIMIT` request → 400 INVALID_ARGUMENT (spec never over-claims). Device
  (phoneNumber/nai/ipv4/ipv6) or three-legged-token identifier with the CAMARA
  two-/three-legged 422 rule (`UNNECESSARY_IDENTIFIER`/`MISSING_IDENTIFIER`,
  mirrors Number Recycling). Two control planes (DESIGN §7): identifier
  reserved-error suffix; trailing-digit parity → BOUND (odd; synthesised
  Luhn-valid 15-digit IMEI, self-contained — no new dep) vs UNBOUND. Deferred:
  `AREALIMIT` (spatial), `POST /bind` + `POST /unbind` (stateful). Wired into
  router, catalog, and served-spec table. 21 new tests; full suite 1314 green;
  `cargo build --release` clean. — binary: 2.6M (2,670,192 bytes, +~31 KB, no new dep)
- 2026-08-06 — Phase 5 ("other APIs"): **Consent Info vwip — `POST /retrieve`**
  (new API, CAMARA ConsentInfo, wip). Chose a stateless, non-spatial,
  phone-number-keyed API over the remaining TLS-sink items (each needs a heavy
  rustls TLS client — against "keep the binary small", and all belong to
  *stateful* APIs, so lower priority than a new stateless/non-spatial one by phase
  order). Fetched the canonical upstream contract (`consent-info/vwip`:
  `retrieveStatus`, scope `consent-info:retrieve`; body `{scopes, purpose,
  requestCaptureUrl, phoneNumber?, callbackUrl?}`; response `{statusInfo[],
  captureUrl?}`) and vendored it. Three control planes (DESIGN §7): identifier
  reserved-error suffix; identifier trailing digits → consent state (`d % 6` over
  valid/PENDING/REQUESTED/DENIED/EXPIRED/OBJECTED); `requestCaptureUrl` gates the
  `captureUrl`. Two request 403 planes (`NOT_ALLOWED_SCOPES_PURPOSE` via a
  `forbidden` scope, `INVALID_CALLBACK_URL`). Mirrors Subscription Status's
  two-/three-legged identifier rule. No new dependency (self-contained RFC 3339
  formatter + FNV-1a capture token). 22 new tests; full suite 1293 green;
  `cargo build --release` clean. binary: 2.6M (2,639,104 bytes).
- 2026-08-06 — Phase 5 ("other APIs"): **Network Health Assessment vwip — `GET
  /health-scores`** (new API, CAMARA NetworkInsights suite, wip). CamaraSim's
  first **network-keyed** aggregate API: a two-legged (`client_credentials`)
  query returning a network module's latest `0..100` health `score` (never
  device-level data). Vendored the canonical CAMARA contract (verified against
  the upstream `NetworkInsights/code/API_definitions/network-health-assessment.yaml`:
  `getHealthScores`, scope `network-health-assessment:health-scores:read`, required
  `networkId` (UUID) + `netType` (`NET`/`NET_WIRELESS`/`NET_TRANSPORT`/`NET_CORE`)
  query params, `HealthInfo` `{networkId, netType, score?, scoringTime?}`). Two
  control planes (DESIGN §7): the `networkId` reserved-error suffix → canonical
  CAMARA error (UUIDs carry digits, shared convention applies unchanged); else the
  trailing three digits `d` set the score (`d/10`) and `netType` shifts it per
  module (`clamp(d/10 − moduleIndex, 0, 100)`), with `…000`/no-digits → the spec's
  no-data `{score:null, scoringTime:null}`. Wired into the router, catalog, and
  served-spec table; self-contained RFC 3339 formatter (mirrors
  connected_network_type); **no new dependency**. 13 new tests (score bands,
  per-module shift, clamp, no-data, reserved suffixes, UUID/netType validation,
  auth, x-correlator). `cargo test` 1271 passing; `cargo build --release` OK.
  — binary: 2.5M (2,606,056 B, +~20 KB for the new module/spec, no new dep)
- 2026-08-06 — Phase 5: **Session Insights vwip — `session-ended` `SESSION_EXPIRED`
  (expiry-timer) leg**. `createSession` now schedules an async expiry timer
  (`spawn_session_expiry`) for a time-bounded, sink-bearing session (any non-`…000`,
  non-`…001` tail): at `expiresAt` it evicts the session and delivers the terminal
  `session-ended` CloudEvent (`terminationReason: SESSION_EXPIRED`) over raw TCP
  (`http://` only), ACCESSTOKEN `sinkCredential` bearer applied + taken single-use,
  exactly-once vs a concurrent delete/network-termination. Overcame the prior
  "can't be tested" deferral: the timer fires immediately when driven with an
  already-past `expiresAt`, so the real path is exercised end-to-end without waiting
  the fixed 24 h. Spec: header + createSession/delete/metrics descriptions +
  `x-camarasim-scenarios` time-bounded case + `SessionEndedEvent` enum note refreshed.
  Test: past-expiry session → SESSION_EXPIRED CloudEvent + bearer + eviction. No new
  dep. `cargo test` 1258 passing; binary: 2,586,096 bytes (~2.5M, unchanged).
- 2026-08-06 — Phase 5: **Session Insights vwip — `session-ended` `NETWORK_TERMINATED`
  leg on `createSession`**. A `…001` identifier now creates an ordinary `ACTIVE`
  session, then the simulated network drops it early: `spawn_network_termination`
  (1 s fixed grace, mirroring QoD/QoS Provisioning's `…001` convention) evicts the
  session and delivers the terminal `session-ended` CloudEvent
  (`terminationReason: NETWORK_TERMINATED`) to the `http://` sink, fire-and-forget
  over raw TCP (no HTTP-client dep); ACCESSTOKEN `sinkCredential` bearer applied and
  taken single-use, exactly-once vs a concurrent delete. Spec: createSession
  description + `x-camarasim-scenarios` `…001` case; header + delete notes refreshed.
  Test: `…001` session → NETWORK_TERMINATED CloudEvent + bearer + eviction (404).
  The `SESSION_EXPIRED` expiry-timer leg stays deferred (fixed 24 h lifetime isn't
  timer-testable). 1257 tests green, no new dep. — binary: 2.5M (2579608 B)
- 2026-08-06 — Phase 5: **Session Insights vwip — `session-ended` CloudEvent
  (`SESSION_DELETED`) on `deleteSession`**. The delete leg of the deferred
  `session-ended` notification: a deleted session that recorded a `sink` now receives
  the **terminal** `org.camaraproject.session-insights.v0.session-ended` CloudEvent
  (`data.terminationReason: SESSION_DELETED`), fire-and-forget over raw TCP (`http://`
  only; `https://`/TLS a documented cut; no HTTP-client dep). Verified the canonical
  event against the live CAMARA SessionInsights spec (`session-ended` + `SessionEndedData`
  `{sessionId, terminationReason: NETWORK_TERMINATED|SESSION_EXPIRED|ACCESS_TOKEN_EXPIRED|
  SESSION_DELETED}`). The ACCESSTOKEN `sinkCredential` bearer authenticates the callback
  and is *taken* single-use (terminal event → nothing later needs it; secret never
  echoed). New `notifications::session_ended_event` builder + `SESSION_ENDED_EVENT_TYPE`;
  `deleteSession` fires it (still `204`). `SESSION_EXPIRED` (expiry timer) /
  `NETWORK_TERMINATED` legs + TLS sink still deferred. spec: header + delete op doc the
  event, delete scenarios record it, new `SessionEndedEvent` schema. tests: 3 new (event
  shape, end-to-end delete→sink fires SESSION_DELETED, callback carries the bearer + secret
  never echoed). 1256 tests green, no new dep. — binary: 2.5M (2574720 B)
- 2026-08-06 — Phase 5: **Session Insights vwip — `sinkCredential` (ACCESSTOKEN
  bearer) auth on the `network-quality-score` callback**. A session created with a
  `credentialType: ACCESSTOKEN` `sinkCredential` now authenticates its callback with
  `Authorization: Bearer <token>` (RFC 6750). New `sink_authorization` helper +
  credential side-store (`store::insert_credential`/`peek_credential`/`take_credential`),
  mirroring QoS Provisioning: the derived bearer is stashed by `sessionId` at
  `createSession` (never echoed by `GET`/`retrieve-sessions`), *peeked*
  non-destructively on every `sendSessionMetrics` delivery (metrics may repeat), and
  dropped on `deleteSession`. `PLAIN`/`REFRESHTOKEN` (or none) → unauthenticated
  (documented cut); `https://`/TLS sink + `session-ended` still deferred. spec: metrics
  op docs the bearer auth + secret-never-echoed, `sinkCredential` schema updated. tests:
  4 new (helper enum, authenticated delivery, end-to-end callback carries bearer + GET
  never echoes it, side-store peek/take). 1253 tests green, no new dep. — binary: 2.5M
  (2569960 B)
- 2026-08-06 — Phase 5: **Session Insights vwip — `network-quality-score` CloudEvent
  on `sendSessionMetrics`**. First notification slice for Session Insights: a `204`
  for a session that recorded a `sink` now fires an
  `org.camaraproject.session-insights.v0.network-quality-score` CloudEvent to that
  sink, fire-and-forget over raw TCP (`http://` only; `https://` a documented no-op
  cut — no TLS client; no HTTP-client dep). New `notifications.rs` (mirrors QoD /
  QoS Provisioning) + `store::new_event_id`. `data.qualityScore` (0–100) is
  deterministic from the submitted metrics (`100 − (10−loss)·8 − delay/20 −
  jitter/20`, clamped), so the `MetricsPayload` is a genuine control plane. spec:
  metrics op docs the notification + score formula, new `NetworkQualityScoreEvent`
  schema, scenarios record the fired event. tests: 6 new (score derivation, event
  shape, http-sink delivery, non-http no-op, end-to-end metrics→sink). `sinkCredential`
  auth / `session-ended` / TLS still deferred. 1249 tests green, no new dep. —
  binary: 2.5M (2567744 B)
- 2026-08-05 — Phase 5: **Session Insights vwip — `POST /sessions/{sessionId}/metrics`
  (`sendSessionMetrics`, scope `session-insights:sessions:write`)**. Submit the
  application-observed `MetricsPayload` (`packetDelay`/`jitter` Durations,
  `packetLossErrorRate` 1–10 exponent, optional `upstreamRate`/`downstreamRate`
  Rates). CAMARA returns `204` to acknowledge receipt — the quality score is
  delivered later via a `sink` notification (deferred) — so the sim validates the
  payload, confirms the session exists, and answers `204` without persisting the
  metrics. Keyed only on store state (opaque `sessionId`, mirroring delete): known
  id + valid payload → `204`; unknown/deleted id → `404 NOT_FOUND`; malformed body
  / missing required figure / bad `unit` → `400 INVALID_ARGUMENT`; a value or
  `packetLossErrorRate` outside its range → `400 OUT_OF_RANGE` (new `out_of_range`
  helper). Payload validated before the store lookup. `410 Gone` (metrics for an
  expired session) a documented cut — no retained expired state in this slice.
  Spec: added the `/sessions/{sessionId}/metrics` path + `sendSessionMetrics` op
  (204 / inline 400 with INVALID_ARGUMENT+OUT_OF_RANGE examples / shared error set
  / `x-camarasim-scenarios`) and the `MetricsPayload`/`Duration`/`Rate`/
  `TimeUnitEnum`/`RateUnitEnum` schemas; added `OUT_OF_RANGE` to the error enum;
  header comment updated. Verified against the authoritative CAMARA SessionInsights
  spec (camaraproject/SessionInsights `main`, version `wip`) via WebFetch. Tests:
  12 new (204 happy path, required-only accepted, unknown→404, deleted→404,
  missing-figure→400, out-of-range value/loss/rate→OUT_OF_RANGE, bad unit→400,
  malformed body→400, body-before-lookup, wrong-scope→403, no-token→401, correlator
  on 204/404). `cargo test` 1243 pass; release builds. No new dep. — binary: 2.5M (2 557 312 bytes)
- 2026-08-05 — Phase 5: **Session Insights vwip — `POST /retrieve-sessions`
  (`retrieveSessionsByDevice`, scope `session-insights:sessions:read`)**. Lists a
  device's sessions as an array of `SessionInfo` (`200`; `[]` when none — never
  404s). New `store::find_by_device` (device-echo scan, lock never across await,
  mirrors QoD's store). Device = submitted `device` id, else token subject
  (two/three-legged; 422 MISSING_IDENTIFIER when neither); identifier
  reserved-error suffix → canonical CAMARA error (checked first); a non-E.164
  subject echo matches nothing → `200 []`. Spec: added the `/retrieve-sessions`
  path + `retrieveSessionsByDevice` op (200 array / error set /
  `x-camarasim-scenarios`) and a `RetrieveSessionsInput` schema; header comment
  updated. Tests: 9 new (store find-by-device + 8 integration — device isolation,
  empty array, reserved suffix, subject fallback, bad body, scope/auth,
  correlator). `cargo test` 1231 pass; release builds. — binary: 2.5M (2 537 168 bytes)
- 2026-08-05 — Phase 5: **Session Insights vwip — `DELETE /sessions/{sessionId}`
  (`deleteSession`, scope `session-insights:sessions:delete`)**. Mirrors the QoD
  delete: new `store::remove` (single-use eviction, returns the `SessionInfo` or
  `None`); handler → `204 No Content` on a hit, `404 NOT_FOUND` on unknown/already
  -deleted, `x-correlator` echoed on both. No `session-ended` CloudEvent (sink
  notifications still deferred). Spec: added the `delete` operation + `204` and
  `x-camarasim-scenarios` under `/sessions/{sessionId}`; updated header comment.
  Tests: 7 new (store round-trip + 6 integration — 204+gone, single-use, unknown
  404, forbidden scope, unauthenticated, correlator on 204/404). `cargo test`
  1222 pass; release builds. — binary: 2.5M (2 527 800 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Session Insights vwip —
  `POST /sessions` (`createSession`) + `GET /sessions/{sessionId}` (`getSession`)**,
  CamaraSim's first slice of the CAMARA SessionInsights (wip) API — a **stateful,
  resource-oriented**, non-spatial application-session resource. Verified the
  authoritative CAMARA spec via WebFetch (camaraproject/SessionInsights `main`;
  version `wip`; base `/session-insights/vwip`; `createSession` requestBody
  `SessionRequest {applicationProfileId(uuid,req), device?, applicationServer(req),
  applicationSessionId?≤256, sink(uri,req), sinkCredential?}`; response `SessionInfo
  {id(uuid,req), applicationSessionId?, device?, applicationServer(req), sink(req),
  startsAt(req), expiresAt?, status(ACTIVE|EXPIRED|DELETED,req)}`; getSession →
  200/404). Chosen over the two remaining `[ ]` sink-TLS leaves (still deferred:
  rustls is a heavyweight, multi-MB dep vs the small-binary guardrail) and the
  other unimplemented CAMARA repos (all stateful/edge/complex — ConsentManagement,
  QoSBooking, eSimRemoteManagement, EdgeApplicationManagement, IoTDeviceManagement);
  SessionInsights is the least-spatial and mirrors the existing QoD create/read
  template almost exactly. Scoped to the natural create/read pair; DELETE /
  retrieve-sessions / metrics / CloudEvents notifications deferred to later passes
  (recorded as sub-steps). New `src/apis/session_insights/{,.rs,store.rs,vwip.rs}`
  (in-memory `Mutex<HashMap>` store, lock never held across await, UUID-shaped id
  via `SHA-256(counter‖now)` — no uuid/rand dep, mirroring QoD). Model (DESIGN §7):
  identifier = submitted `device` id else token subject (two/three-legged, 422
  MISSING_IDENTIFIER when neither); reserved-error suffix → canonical CAMARA error
  (`…409` → createSession 409 CONFLICT, `…422` → SERVICE_NOT_APPLICABLE); else
  `status:ACTIVE`, `startsAt:now`, and `expiresAt` present (now+24h) unless the tail
  is `…000`/no-digits (open-ended). Validation: missing applicationProfileId /
  applicationServer(no endpoint) / sink(non-http(s)) → 400; applicationSessionId
  >256 → 400; malformed phoneNumber → 400. Unknown top-level fields tolerated;
  Device strict. Self-contained RFC 3339 formatter (no new dep). Spec: new
  `specs/session-insights/vwip/openapi.yaml` (vendored + annotated with
  `x-camarasim-scenarios`; full shared reserved-error set). Wired into
  `apis::routes`, the `/` catalog + its contract test, and `openapi::SPECS`. Tests:
  21 new (3 pure units — RFC 3339 formatter, e164, http-uri; 18 router: time-bounded
  vs open-ended create echoing inputs, read-back verbatim, unknown → 404,
  three-legged subject fallback, reserved-suffix 404/409/422/429, missing
  profileId/server/sink + malformed sink/phone/body → 400, scope/auth on POST & GET,
  x-correlator on success/error). `cargo test` 1215 passed; `cargo build --release`
  OK. No new dependency. — binary: 2.5M (2,522,880 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Device Authenticity vwip —
  `POST /check-status` (`checkImeiStatus`, scope
  `device-authenticity:check-status`)**, a new stateless, non-spatial,
  **IMEI-keyed** anti-fraud query — the register / operational status of a device.
  Chosen over the remaining `[ ]` leaves (QoD / QoS-Provisioning `https://` sink
  TLS delivery — still deferred: a rustls TLS client is a heavyweight, multi-MB
  dependency vs the "keep binary small" guardrail), as a higher-phase-priority
  stateless API. Verified the authoritative CAMARA spec via WebFetch
  (camaraproject/DeviceAuthenticity `main`; version `wip`; base
  `/device-authenticity/vwip`; requestBody required, `RequestBody {imei required,
  ^[0-9]{15}$}`; response `ImeiStatus {imei, operationalStatus(enum:
  allowed|lost|stolen|blacklisted|blocked|fraud|non-payment|regulatory|unknown),
  reportedDate?}` + `CommonResponseBody {lastChecked}`; errors
  400/401/403/404(IDENTIFIER_NOT_FOUND)/422(SERVICE_NOT_APPLICABLE)/429). New
  `src/apis/device_authenticity/{,.rs,vwip.rs}`. Model (DESIGN §7): the IMEI is the
  required identifier (no two/three-legged fallback — device named explicitly);
  reserved-error suffix → canonical CAMARA error (`…404`→NOT_FOUND, `…422`→
  SERVICE_NOT_APPLICABLE); else trailing three digits `d` pick the status by
  `d % 9` over the nine-value enum in order (`…000`→allowed default, `…001`→lost,
  …, `…008`→unknown), a non-`allowed` status carrying `reportedDate` = now − d h.
  Unknown request fields tolerated (CAMARA RequestBody omits
  additionalProperties:false). Self-contained RFC 3339 formatter (no new dep,
  mirroring Device Swap). Spec: new `specs/device-authenticity/vwip/openapi.yaml`
  (vendored + annotated with `x-camarasim-scenarios`; full shared reserved-error
  set). Wired into `apis::routes`, the `/` catalog, `openapi::SPECS`, and the
  catalog contract test. Tests: 16 new (2 pure units — 15-digit IMEI validation,
  RFC 3339 formatter; 14 router: allowed default w/o reportedDate, each status in
  order, adverse reportedDate, mod-9 wrap, reserved-error suffixes, missing/
  non-15-digit/malformed body → 400, unknown fields tolerated, scope/auth,
  x-correlator on success/error). `cargo test` 1194 passed; `cargo build --release`
  OK. No new dependency. — binary: 2.4M (2,486,648 bytes)
- 2026-08-05 — Phase 5 (Carrier Billing v0.5): **`GET /payments`
  (`retrievePayments`) now applies its `page`/`perPage`/`order` query
  parameters** — closing the "accepted but not applied" documented cut. The
  stored payments are sorted by `paymentCreationDate` (`paymentId` breaks ties
  for deterministic order over the process-global store) in `order` (asc/desc,
  default desc), then the `page`-th window of `perPage` (defaults 1/10) is
  returned; non-integer `page`/`perPage` → 400 INVALID_ARGUMENT, `<1` → 400
  OUT_OF_RANGE, unknown `order` → 400 INVALID_ARGUMENT; unknown query params
  ignored. Chosen over the two remaining `[ ]` leaves (QoD / QoS-Provisioning
  `https://` sink TLS delivery — still deferred: a rustls TLS client is a
  heavyweight, multi-MB dependency vs the "keep binary small" guardrail, and is
  hard to test without a TLS-server harness) as a small, safe, no-new-dep slice
  fully covered by the already-vendored spec. Sort/paginate logic is a pure
  helper (`apply_list_params`), unit-tested independent of the shared store;
  param parsing/validation reuses `RawQuery` + `serde_urlencoded` (both already
  deps). Spec: rewrote the `retrievePayments` description, the three parameter
  descriptions, and the `x-camarasim-scenarios` cases to the applied behaviour.
  Tests: 6 pure `apply_list_params`/`parse_list_params` cases + 4 through-the-app
  cases (400 INVALID_ARGUMENT / OUT_OF_RANGE, perPage bounds the page); updated
  the one pre-existing containment test to a large `perPage`. 1180 tests green,
  no new deps. — binary: 2.4M (2468040 B)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Subscription Status vwip —
  `POST /retrieve-subscription-status` (`retrieveSubscriptionStatus`, scope
  `subscription-status:retrieve-subscription-status`)**, a new stateless,
  non-spatial, phone-number-keyed line-status query — the live service status of
  a mobile line. Chosen over the remaining `[ ]` leaves (QoD / QoS-Provisioning
  `https://` sink TLS delivery — still deferred: a rustls TLS client is a
  heavyweight, multi-MB dependency vs the "keep binary small" guardrail), and as
  a higher-phase-priority stateless API. Verified the authoritative CAMARA spec
  via WebFetch (camaraproject/SubscriptionStatus `main`; version `wip`; base
  `/subscription-status/vwip`; requestBody required, `SubscriptionStatusRequest
  {phoneNumber?}` additionalProperties:false; `SubscriptionStatusResponse
  {voiceSmsIn(active|suspended), voiceSmsOut(active|suspended),
  dataService(active|suspended|throttled)}` all required; declared errors
  200/400/401/403/404(IDENTIFIER_NOT_FOUND)/422(SERVICE_NOT_APPLICABLE,
  MISSING_IDENTIFIER, UNNECESSARY_IDENTIFIER)). New
  `src/apis/subscription_status/{,.rs,vwip.rs}`, mirroring Number Recycling's
  two-legged/three-legged identifier rule (422 UNNECESSARY/MISSING; empty body
  accepted for three-legged). Model (DESIGN §7): reserved-error suffix →
  canonical CAMARA error (`…404`→NOT_FOUND, `…422`→SERVICE_NOT_APPLICABLE,
  matching the API's own service-level 422 code); else the identifier's trailing
  three digits `d` are a status bitfield (bit0→voiceSmsIn suspended,
  bit1→voiceSmsOut suspended, `(d>>2)%3`→dataService active/suspended/throttled),
  so `…000` is the all-active default and every field is independently
  controllable. Spec: new `specs/subscription-status/vwip/openapi.yaml` (vendored
  + annotated with `x-camarasim-scenarios`; full shared reserved-error set,
  `IDENTIFIER_NOT_FOUND` in the error enum for completeness). Wired into
  `apis::routes`, the `/` catalog, `openapi::SPECS`, and the catalog contract
  test. Tests: 16 new (1 E.164 unit + 15 router: all-active default, per-field
  independence, dataService's three states, reserved-error suffixes, three-legged
  subject + empty body, UNNECESSARY/MISSING identifier, validation set, scope/
  auth, x-correlator on success/error). `cargo test` 1170 passed; `cargo build
  --release` OK. No new dependency. — binary: 2.4M (2,458,096 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Application Profiles vwip —
  `PATCH /application-profiles/{applicationProfileId}`
  (`updateApplicationProfile`, scope `application-profiles:update`)**, the update
  leg that **completes Application Profiles vwip** (the only remaining `[ ]` leaf
  besides the deferred QoD / QoS-Provisioning `https://` sink TLS delivery, which
  still needs a heavyweight multi-MB rustls TLS client vs the "keep binary small"
  guardrail). Verified against the authoritative CAMARA spec via WebFetch
  (camaraproject/ApplicationProfiles `main`): `PATCH`, scope
  `application-profiles:update`, body the threshold set, `200`/400/401/403/404/429;
  the op is a **full-set replacement** ("update the complete set … with the new
  set of thresholds"), not JSON Merge Patch. Body is an `ApplicationProfileRequest`
  validated by the same rules as create — factored create's parse+validate into a
  shared `parse_and_validate` (+ `render_profile`) so create and update cannot
  drift — replacing the stored thresholds in place while keeping the
  `applicationProfileId`; the body is validated before the store so a malformed
  body 400 wins over the unknown-id 404. Path plane mirrors read/delete (non-UUID
  → 400 INVALID_ARGUMENT, well-formed unknown → 404 NOT_FOUND). Atomic
  check-and-swap `store::replace` (contains-then-insert under one lock, so a
  concurrent delete can't resurrect an evicted profile). `x-correlator` echoed.
  Spec: added the `patch` operation + `x-camarasim-scenarios` to the
  `/application-profiles/{applicationProfileId}` path item, refreshed the header
  and API description (`specs/application-profiles/vwip/openapi.yaml`). Tests: 10
  new (1 store unit `replace_overwrites_existing_and_reports_absent_for_unknown`;
  9 router: replace-in-place + read-back + id-unchanged + correlator, unknown-id
  404, malformed-id 400, empty-body anyOf 400, out-of-range 400, bad-body-400-wins
  -over-unknown-id-404, update-scope 403, missing-token 401). `cargo test` 1154
  passed; `cargo build --release` OK. No new dependency. — binary: 2.4M
  (2,437,312 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Application Profiles vwip —
  `DELETE /application-profiles/{applicationProfileId}`
  (`deleteApplicationProfile`, scope `application-profiles:delete`)**, the delete
  leg of the profile CRUD lifecycle. Chosen over the remaining `[ ]` leaf items
  (QoD / QoS-Provisioning `https://` sink TLS delivery — still deferred: a rustls
  TLS client is a heavyweight, multi-MB dependency vs the "keep binary small /
  justify every dep" guardrail) and over the sibling PATCH sub-item, as the
  smallest, lowest-risk single endpoint that lands green with no new dep. Keyed
  only on store state (opaque server-minted id → no reserved-identifier plane,
  mirroring QoS Provisioning's `revokeQosAssignment` / Blockchain's delete):
  stored id → `204 No Content` (single-use eviction via new `store::remove`);
  well-formed unknown/already-deleted id → `404 NOT_FOUND`; non-UUID path → `400
  INVALID_ARGUMENT` (mirrors `readApplicationProfile`); `x-correlator` echoed.
  Spec: added the `delete` operation + `x-camarasim-scenarios` to the existing
  `/application-profiles/{applicationProfileId}` path item in
  `specs/application-profiles/vwip/openapi.yaml` (204/400/401/403/404/429). Tests:
  7 new (1 store unit `remove_evicts_once_then_reports_absent`; 6 router tests:
  delete→read 404 round-trip + correlator echo, single-use second-delete 404,
  unknown-id 404, malformed-id 400, delete-scope 403 (profile survives), missing
  token 401). `cargo test` 1145 passed; `cargo build --release` OK. No new
  dependency. PATCH (`updateApplicationProfile`) remains the last
  Application Profiles sub-item. — binary: 2.4M (2,429,288 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Application Profiles vwip —
  `POST /application-profiles` (`createApplicationProfile`) +
  `GET /application-profiles/{applicationProfileId}` (`readApplicationProfile`)**,
  a new stateful, resource-oriented API — the quality-requirements registry the
  already-implemented Connectivity Insights API references by
  `applicationProfileId` (closing that noted cut's counterpart). Chosen over the
  remaining `[ ]` leaf items (QoD / QoS-Provisioning `https://` sink TLS delivery),
  which need a rustls TLS client — a heavyweight, multi-MB dependency that
  conflicts with the "keep the binary small / justify every dep" guardrail and has
  been deliberately deferred as a documented cut; a new stateless-ish API is also
  the higher phase-priority pick. Verified the authoritative CAMARA spec via
  WebFetch (camaraproject/ApplicationProfiles, `main`; version `wip`; base
  `/application-profiles/vwip`; scopes `application-profiles:{create,read,update,
  delete}`; request `ApplicationProfileRequest {anyOf networkQualityThresholds |
  computeResources}`; response `ApplicationProfile {applicationProfileId(uuid) +
  thresholds}`; errors 400/401/403/404/429). Scoped this pass to create + read-by-id
  (PATCH/DELETE are follow-up sub-items). New
  `src/apis/application_profiles/{,.rs,store.rs,vwip.rs}`: in-memory store (opaque
  UUID id, `Mutex<HashMap>`, no uuid/rand dep, mirroring QoS Provisioning); no
  identifier/`device`, so store state (read) and the request body (create) are the
  control planes (DESIGN §7). Create validation: `anyOf` + per-object
  `minProperties: 1` → 400 INVALID_ARGUMENT; numeric ranges (Duration ≥ 1, Rate/
  Compute 0..=1024, packetLossErrorRate 1..=10) → 400 OUT_OF_RANGE; unknown enum/
  field/type → 400 INVALID_ARGUMENT at parse. Read: 200 / 404 (unknown) / 400
  (non-UUID path). Spec: new `specs/application-profiles/vwip/openapi.yaml`
  (vendored + annotated with `x-camarasim-scenarios`; shared error `$ref`s). Wired
  into `apis::routes`, the `/` catalog, `openapi::SPECS`, and the catalog contract
  test. Tests: 19 new (2 store units + `is_uuid_shaped` unit + 16 router tests:
  create NQ/compute happy paths, create→read round-trip, 404/400 read cases, anyOf/
  minProperties/OUT_OF_RANGE/enum/unknown-field 400s, scope/auth). `cargo test`
  1138 passed; `cargo build --release` OK. No new dependency. — binary: 2.4M
  (2,423,504 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Verified Caller vwip
  `POST /pre-announce` (`createPreAnnouncement`)**, a new stateless, non-spatial,
  two-legged / business-facing anti-scam caller-trust API — the first Fall25 API
  added. Verified the authoritative CAMARA spec via WebFetch (CAMARA
  VerifiedCaller, `main`, version `wip`; base `/verified-caller/vwip`; scope
  `verified-caller:create`; request `CreatePreAnnouncementRequest
  {callingParticipant(req,PhoneNumber), calledParticipant(req,PhoneNumber),
  strategy?(SMS|BRAND_DISPLAY,≤32), timeToLive?(1..86400),
  registrationId?(uuid,≤256), dynamicDisplayName?(≤32), callReason?(≤32)}`;
  responses `201 AnnouncementInfo {preAnnouncementId(uuid,≤256),
  expiresAt(date-time)}` **or** `204`; errors 400/401/403/404/409/422/429).
  Confirmed Scam Signal is NOT a public CAMARA API (private GSMA repo) and the
  simple stateless phone-keyed space is otherwise exhausted, so picked this new
  Fall25 API. New `src/apis/verified_caller/{,.rs,vwip.rs}`: two-legged only
  (both participants required in the body → no two-legged/three-legged dance),
  stateless (no read-back → no store). Model (DESIGN §7): `calledParticipant` is
  the identifier — reserved suffix → canonical error (checked after validation);
  else `strategy` selects the response (`BRAND_DISPLAY` → 201 with a handle, `SMS`
  /omitted default → 204) and `timeToLive` (default 120 s) sets `expiresAt` = now
  + ttl on the 201 path; `preAnnouncementId` a deterministic UUID-shaped SHA-256
  token (reuses `sha2`, no new dep). Validation: bad body / non-E.164 participant
  / unknown strategy / over-length text fields → 400 INVALID_ARGUMENT; timeToLive
  ∉ 1..=86400 → 400 OUT_OF_RANGE. Spec: new
  `specs/verified-caller/vwip/openapi.yaml` (vendored + annotated with
  `x-camarasim-scenarios`; full shared reserved-error set). Wired into
  `apis::routes`, the `/` catalog, `openapi::SPECS`, and the catalog contract
  test. Tests: 20 new (4 pure units + 16 router tests: strategy 201/204 planes,
  timeToLive→expiresAt, reserved-error on calledParticipant, calling-participant
  is not the error plane, validation set, scope/auth, x-correlator on
  201/204/error). `cargo test` 1119 passed; `cargo build --release` OK. No new
  dependency. — binary: 2.3M (2,369,528 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Optimal Edge Discovery vwip
  `GET /regions` (`getRegions`)**, completing the API. Verified the authoritative
  CAMARA spec via WebFetch (CAMARA OptimalEdgeDiscovery, `main`; scope
  `optimal-edge-discovery:regions:read`; only an `x-correlator` header param; 200 →
  `GetEdgeCloudRegionsResponse` = `array<EdgeCloudRegion(string, ^[A-Za-z0-9-]+$,
  ≤64), maxItems 20>`; errors 400/401/403/404/422/429). A static "helper" catalog
  with no request input, so no functional/control planes beyond the auth error set:
  the handler returns the **distinct** `edgeCloudRegion` values of the fixed 6-entry
  `EDGE_ZONES` table in table order (new `distinct_regions`), scope-gated via
  `Claims::require_scope`, `x-correlator` echoed. Spec: added the `/regions`
  operation + `GetEdgeCloudRegionsResponse`/`EdgeCloudRegion` schemas +
  `x-camarasim-scenarios` (noting canonical 400/404/422 are unreachable here — no
  input to reject). Tests: `distinct_regions` unit + 4 router tests (200 catalog
  with pattern-valid regions; 403 on the zones scope; 401 no token; correlator
  echo). No new dependency (reuses the existing table). `cargo test` 1099 passed;
  `cargo build --release` OK. — binary: 2.3M (2,343,000 bytes)
- 2026-08-05 — Phase 5 (other CAMARA APIs): **Optimal Edge Discovery vwip
  `POST /retrieve-optimal-edge-cloud-zones` (`discoverOptimalEdge`)** — a new
  stateless, device-keyed edge/MEC discovery API, the *ranked* successor to Simple
  Edge Discovery. Verified the authoritative CAMARA spec via WebFetch (CAMARA
  OptimalEdgeDiscovery, `main`, version `wip`; base `/optimal-edge-discovery/vwip`;
  scope `optimal-edge-discovery:edge-zones:read`; request `OptimalEdgeDiscoveryInfo
  {applicationProfileId(req,uuid,≤36), device?, edgeCloudRegion?(^[A-Za-z0-9-]+$,
  ≤64)}`; 200 → `EdgeDiscoveryResponse {edgeCloudZones[1..20](req), applicationProfileId?,
  device?}` where `EdgeCloudZone {edgeCloudZoneId(uuid,req), edgeCloudZoneName(req),
  edgeCloudProvider(req), edgeCloudRegion?, edgeCloudZoneStatus? enum active|inactive|
  unknown default unknown}`; errors 400/401/403/404/422/429; two-legged/three-legged
  identifier rule). New `src/apis/optimal_edge_discovery/{,.rs,vwip.rs}` mirroring
  Simple Edge Discovery's device-object identifier resolution / two-legged-three-legged
  rule (422 UNNECESSARY/MISSING_IDENTIFIER) / reserved-error convention and its
  SHA-256 UUID-shaped zone id (no new dep). Model (DESIGN §7): required
  `applicationProfileId` UUID-validated (else 400); reserved suffix on the identifier
  → canonical error (checked first); else trailing three digits `d` pick the optimal
  zone (start `d % 6`) and list length `(d % 3) + 1` (1–3 zones) over a fixed 6-entry
  `(name, provider, region)` table, top zone `active`/rest `inactive`; optional
  `edgeCloudRegion` filters candidates (malformed → 400, well-formed-but-unknown →
  404); device echoed only for a phoneNumber request; `applicationProfileId` echoed.
  Wired into `apis.rs` routes, `openapi.rs` SPECS, and the `/` catalog (+ catalog
  test assertion). Spec: vendored+annotated `specs/optimal-edge-discovery/vwip/
  openapi.yaml` (path, request/response schemas, `EdgeCloudZoneStatus` enum, full
  shared error set, `x-camarasim-scenarios`). Tests: 27 new (7 unit: ranking start/
  count/wrap, length ≤ schema max, region filter + unknown→None, zone-id determinism,
  uuid+region validation, e164, device precedence; 20 integration: required fields +
  applicationProfileId & phone echo, ranking by tail, region narrows, unknown region
  404, non-phone omits echo, reserved suffix wins, three-legged subject + reserved
  subject, UNNECESSARY/MISSING_IDENTIFIER, missing/malformed applicationProfileId,
  malformed region, empty/bad-phone/unknown-field/malformed-json 400s, scope-forbidden,
  missing-token, x-correlator). `cargo test` 1094 green (was 1067); `cargo build
  --release` green. No new dependency. `GET /regions` (`getRegions`) deferred to a
  later slice. — binary: 2.3M (2,337,056 B; +33,560 B)

- 2026-08-05 — Phase 5 (other CAMARA APIs): **Media Streaming Rate vwip
  `POST /retrieve-maximum-downstream-media-rate`
  (`retrieveMaximumDownstreamMediaRate`)** — a new stateless, non-spatial,
  device-keyed network-quality query, **completing Media Streaming Rate vwip**.
  Verified the authoritative CAMARA spec via WebFetch (CAMARA
  DeviceMediaStreamingRate, `main`, version `wip`;
  `MediaStreamingRateRequest {device?}`; 200 →
  `MediaStreamingRateResponse {device?, maxDownstreamMediaBitRateSupported int32
  0..1024 (req), unit enum bps|kbps|Mbps|Gbps|Tbps (req)}`; scope
  `media-streaming-rate:retrieve-maximum-downstream-media-rate`; errors
  400/401/403/404/422/429/503; two-legged/three-legged identifier rule). New
  `src/apis/media_streaming_rate/{,.rs,vwip.rs}` reusing Device Data Volume's
  device-object identifier resolution / two-legged-three-legged rule (422
  UNNECESSARY/MISSING_IDENTIFIER) / reserved-error convention. Model: identifier
  trailing three digits `d` (0..=999; `…000`/no digits → 0) → magnitude `d`
  (always ≤ 1024) and `unit = [bps,kbps,Mbps,Gbps,Tbps][d % 5]`, both lowest-first
  from the `…000` floor (`0 bps`); reserved suffix checked first; device echoed
  only for a phoneNumber request. Wired into `apis.rs` routes, `openapi.rs` SPECS,
  and the `/` catalog (+ catalog test assertion). Spec: vendored+annotated
  `specs/media-streaming-rate/vwip/openapi.yaml` (path, request/response schemas,
  `BitRateUnitEnum`, full shared error set, `x-camarasim-scenarios`). Tests: 20
  new (rate+unit derivation & unit cycling, magnitude in range, enum validity,
  reserved suffix wins, non-phone omits echo, three-legged subject keying,
  422 identifier rules, empty/bad-phone/unknown-field/malformed validation, auth,
  x-correlator). `cargo test` 1067 green; `cargo build --release` green. No new
  dependency. — binary: 2.2M (2,303,496 B)
- 2026-08-05 — Phase 5: **Device Data Volume vwip `POST /check`
  (`checkDataVolume`)** — the companion to `retrieve`, **completing Device Data
  Volume vwip**. Verified the authoritative CAMARA spec via WebFetch
  (`CheckDataVolumeRequest {device?, volumeToCheck{value int32 0..1024, unit
  MiB|GiB}(req)}`; 200 → `CheckDataVolumeResponse {device?, lastStatusTime
  (nullable), thresholdExceeded(req)}`; scope `device-data-volume:read`; errors
  400/401/403/404/422/429; `thresholdExceeded` = remaining volume exceeds
  threshold). Added `check` handler to `src/apis/device_data_volume/vwip.rs`
  reusing retrieve's identifier resolution / two-legged-three-legged rule /
  reserved-error convention. Model: identifier trailing three digits `d` fix the
  device's **remaining** volume at `d*10` MiB (`…000`/no digits → 0 MiB),
  `volumeToCheck` normalised to MiB (GiB→value*1024), `thresholdExceeded =
  remaining > threshold` — so `volumeToCheck` is a genuine second control plane
  (the same device flips true↔false as the threshold moves). `volumeToCheck`
  required (missing/unknown `unit` → 400 INVALID_ARGUMENT; `value` ∉ 0..=1024 →
  400 OUT_OF_RANGE, checked before identifier resolution). Spec: added `/check`
  path + `CheckDataVolumeRequest`/`VolumeToCheck`/`VolumeUnitEnum`/
  `CheckDataVolumeResponse` schemas + `x-camarasim-scenarios`. Tests: 16 new
  (threshold as 2nd plane both ways, unit normalisation, zero-remaining, reserved
  suffix wins, three-legged, 422 identifier rules, range/missing/unknown-unit/
  empty-device validation, auth, x-correlator). `cargo test` 1047 green;
  `cargo build --release` green. No new dependency. — binary: 2.2M (2,280,448 B)
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
