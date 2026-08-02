# CamaraSim

A virtual [CAMARA](https://camaraproject.org) operator for testing telecom API
integrations — spec-faithful endpoints, working OAuth2/OIDC/CIBA auth, and deterministic,
parameter-driven scenarios. Non-blocking Rust, one small binary, in-memory state.

> Point your CAMARA client at CamaraSim instead of a live operator to develop and test
> integrations locally or in CI — no network connection required.

## Quick start

Requires a stable Rust toolchain ([rustup](https://rustup.rs)).

```bash
# run it (defaults to port 8080; override with PORT)
cargo run                      # -> camarasimulator listening on 0.0.0.0:8080
PORT=9000 cargo run

# check it's alive
curl localhost:8080/health     # -> ok
curl localhost:8080/           # -> { "service": "camarasimulator", "apis": [...] }

# test everything
cargo test

# build the size-optimised release binary
cargo build --release          # -> target/release/camarasimulator
```

Once APIs are mounted, each is served under canonical URL versioning
(`/{api}/v{MAJOR}/…`) with its OpenAPI at `/{api}/v{n}/openapi.yaml`. Browse mounted
APIs at `GET /`.

## How it works

- **Auth:** real signed JWTs via `client_credentials`, `authorization_code`+PKCE, and
  CIBA (auto-consent in sim mode, but flows are validated faithfully).
- **Scenarios:** the request input is the control plane — the input identifier (e.g.
  `phoneNumber`) selects the functional case, including every CAMARA error. Each case is
  documented in the endpoint's OpenAPI spec.
- **State:** in-memory only (single-node assumption).

## API examples

Every business endpoint is protected. First get a Bearer token from the built-in OAuth2
server via `client_credentials` (any `client_id`/secret is accepted in sim mode), then
call the API. The `phoneNumber` (or `device`) in the request is the control plane: the
examples below use inputs that select the happy path.

**Get a token.** Request the scopes you need — space-delimited; the one below asks for
all of them at once:

```bash
curl -s -X POST localhost:8080/oauth2/token \
  -d grant_type=client_credentials \
  -d client_id=demo \
  --data-urlencode 'scope=number-verification:verify number-verification:device-phone-number:read sim-swap:check sim-swap:retrieve-date kyc-match:match device-reachability-status:read device-roaming-status:read device-identifier:retrieve-type device-identifier:retrieve-identifier device-identifier:retrieve-ppid one-time-password-sms:send-validate quality-on-demand:sessions:create quality-on-demand:sessions:read'
```

Response — copy the `access_token` into the calls below (shown as `<access_token>`):

```json
{"access_token":"eyJhbGciOiJSUzI1NiIsImtpZCI6...","expires_in":3600,"scope":"number-verification:verify ...","token_type":"Bearer"}
```

**Available scopes**, one per protected endpoint:

| Scope | Endpoint |
|---|---|
| `number-verification:verify` | `POST /number-verification/v1/verify` |
| `number-verification:device-phone-number:read` | `GET /number-verification/v1/device-phone-number` |
| `sim-swap:check` | `POST /sim-swap/v2/check` |
| `sim-swap:retrieve-date` | `POST /sim-swap/v2/retrieve-date` |
| `kyc-match:match` | `POST /kyc-match/v0.3/match` |
| `device-reachability-status:read` | `POST /device-reachability-status/v1/retrieve` |
| `device-roaming-status:read` | `POST /device-roaming-status/v1/retrieve` |
| `device-identifier:retrieve-type` | `POST /device-identifier/v0.3/retrieve-type` |
| `device-identifier:retrieve-identifier` | `POST /device-identifier/v0.3/retrieve-identifier` |
| `device-identifier:retrieve-ppid` | `POST /device-identifier/v0.3/retrieve-ppid` |
| `one-time-password-sms:send-validate` | `POST /one-time-password-sms/v1/{send-code,validate-code}` |
| `quality-on-demand:sessions:create` | `POST /quality-on-demand/v1/sessions` |
| `quality-on-demand:sessions:read` | `GET /quality-on-demand/v1/sessions/{sessionId}` |

### Number Verification v1

```bash
# Is this the caller's number? -> {"devicePhoneNumberVerified":true}
curl -s -X POST localhost:8080/number-verification/v1/verify \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012"}'

# The caller's own number -> {"devicePhoneNumber":"+123456789012"}
curl -s localhost:8080/number-verification/v1/device-phone-number \
  -H 'Authorization: Bearer <access_token>'
```

### SIM Swap v2

```bash
# Swapped within maxAge hours? -> {"swapped":true}
curl -s -X POST localhost:8080/sim-swap/v2/check \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012","maxAge":240}'

# Timestamp of the last SIM change -> {"latestSimChange":"…","monitoredPeriod":10}
curl -s -X POST localhost:8080/sim-swap/v2/retrieve-date \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012"}'
```

### KYC Match v0.3

```bash
# Confirm customer attributes -> {"nameMatch":"true","emailMatch":"true"}
curl -s -X POST localhost:8080/kyc-match/v0.3/match \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012","name":"John Smith","email":"j@example.com"}'
```

### Device Reachability Status v1

```bash
# -> {"reachable":true,"connectivity":["DATA","SMS"]}
curl -s -X POST localhost:8080/device-reachability-status/v1/retrieve \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789012"}}'
```

### Device Roaming Status v1

```bash
# -> {"roaming":true,"countryCode":262,"countryName":["DE"]}
curl -s -X POST localhost:8080/device-roaming-status/v1/retrieve \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789012"}}'
```

### Device Identifier v0.3

The `device` may be identified by `phoneNumber`, `networkAccessIdentifier`,
`ipv4Address`, or `ipv6Address` (first present wins).

```bash
# Device type/model -> {"manufacturer":"OnePlus","model":"OnePlus 12","tac":"35847104",…}
curl -s -X POST localhost:8080/device-identifier/v0.3/retrieve-type \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789001"}}'

# Hardware identifier -> {"imei":"354385090000024","imeisv":"…","model":"Pixel 8 Pro",…}
curl -s -X POST localhost:8080/device-identifier/v0.3/retrieve-identifier \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789002"}}'

# Pseudonymous, stable device id -> {"ppid":"5dacd245-…",…}
curl -s -X POST localhost:8080/device-identifier/v0.3/retrieve-ppid \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789001"}}'
```

### One Time Password SMS v1

The simulator "sends" a deterministic code — the phone number's last 6 digits,
zero-padded — so a headless caller can compute what to validate (`+123456789012` →
`789012`). Take the `authenticationId` from the send-code response and pass it to
validate-code.

```bash
# Send the code -> {"authenticationId":"…"}; message must contain the {{code}} placeholder
curl -s -X POST localhost:8080/one-time-password-sms/v1/send-code \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012","message":"Your code is {{code}}"}'

# Validate it (code 789012 for +123456789012) -> HTTP 204 No Content
curl -s -X POST localhost:8080/one-time-password-sms/v1/validate-code \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"authenticationId":"<authenticationId>","code":"789012"}'
```

### Quality on Demand v1

A stateful, resource-oriented API: create a QoS session, then read it back by its
`sessionId`. The identifier's trailing digits pick the case — `…000` grants a
`REQUESTED` (pending) session, any other tail an `AVAILABLE` one, and a reserved
suffix (e.g. `…409`) the matching CAMARA error.

```bash
# Create a session -> 201 {"sessionId":"…","qosStatus":"AVAILABLE",…}
curl -s -X POST localhost:8080/quality-on-demand/v1/sessions \
  -H 'Authorization: Bearer <access_token>' \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789012"},"applicationServer":{"ipv4Address":"203.0.113.0/24"},"qosProfile":"QOS_L","duration":3600}'

# Read it back by id (from the create response) -> 200 SessionInfo
curl -s localhost:8080/quality-on-demand/v1/sessions/<sessionId> \
  -H 'Authorization: Bearer <access_token>'
```

## Project layout

| Path | What |
|---|---|
| `docs/DESIGN.md` | Requirements, architecture, auth, versioning, error & scenario strategy |
| `docs/AGENT.md` | Playbook for the autonomous hourly build agent |
| `PROGRESS.md` | Live backlog + scan journal (what's done / next / tried) |
| `src/` | The server (grown API-by-API) |
| `specs/` | Vendored, annotated CAMARA OpenAPI specs per API/version |

## Development

This project is built incrementally by an **autonomous agent** that runs once per hour,
making one small, tested, spec-accurate increment per pass. See `docs/AGENT.md` for the
operating rules and `PROGRESS.md` for current status. Contributions follow the same bar:
non-blocking, in-memory, every change tested, OpenAPI kept in sync.
