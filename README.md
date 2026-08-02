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

Every business endpoint is protected: obtain a Bearer token from the built-in OAuth2
server (`client_credentials` shown here — any `client_id`/secret is accepted in sim mode)
with the endpoint's scope, then call the API. The `phoneNumber` (or `device`) in the
request is the control plane: the examples below use inputs that select the happy path.

```bash
# Mint a token for a given scope (any client_id works in sim mode)
token() {
  curl -s -X POST localhost:8080/oauth2/token \
    -d grant_type=client_credentials -d client_id=demo -d "scope=$1" \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["access_token"])'
}
```

### Number Verification v1

```bash
# Is this the caller's number? -> {"devicePhoneNumberVerified":true}
curl -s -X POST localhost:8080/number-verification/v1/verify \
  -H "Authorization: Bearer $(token number-verification:verify)" \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012"}'

# The caller's own number -> {"devicePhoneNumber":"+123456789012"}
curl -s localhost:8080/number-verification/v1/device-phone-number \
  -H "Authorization: Bearer $(token number-verification:device-phone-number:read)"
```

### SIM Swap v2

```bash
# Swapped within maxAge hours? -> {"swapped":true}
curl -s -X POST localhost:8080/sim-swap/v2/check \
  -H "Authorization: Bearer $(token sim-swap:check)" \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012","maxAge":240}'

# Timestamp of the last SIM change -> {"latestSimChange":"…","monitoredPeriod":10}
curl -s -X POST localhost:8080/sim-swap/v2/retrieve-date \
  -H "Authorization: Bearer $(token sim-swap:retrieve-date)" \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012"}'
```

### KYC Match v0.3

```bash
# Confirm customer attributes -> {"nameMatch":"true","emailMatch":"true"}
curl -s -X POST localhost:8080/kyc-match/v0.3/match \
  -H "Authorization: Bearer $(token kyc-match:match)" \
  -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012","name":"John Smith","email":"j@example.com"}'
```

### Device Reachability Status v1

```bash
# -> {"reachable":true,"connectivity":["DATA","SMS"]}
curl -s -X POST localhost:8080/device-reachability-status/v1/retrieve \
  -H "Authorization: Bearer $(token device-reachability-status:read)" \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789012"}}'
```

### Device Roaming Status v1

```bash
# -> {"roaming":true,"countryCode":262,"countryName":["DE"]}
curl -s -X POST localhost:8080/device-roaming-status/v1/retrieve \
  -H "Authorization: Bearer $(token device-roaming-status:read)" \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789012"}}'
```

### Device Identifier v0.3

The `device` may be identified by `phoneNumber`, `networkAccessIdentifier`,
`ipv4Address`, or `ipv6Address` (first present wins).

```bash
# Device type/model -> {"manufacturer":"OnePlus","model":"OnePlus 12","tac":"35847104",…}
curl -s -X POST localhost:8080/device-identifier/v0.3/retrieve-type \
  -H "Authorization: Bearer $(token device-identifier:retrieve-type)" \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789001"}}'

# Hardware identifier -> {"imei":"354385090000024","imeisv":"…","model":"Pixel 8 Pro",…}
curl -s -X POST localhost:8080/device-identifier/v0.3/retrieve-identifier \
  -H "Authorization: Bearer $(token device-identifier:retrieve-identifier)" \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789002"}}'

# Pseudonymous, stable device id -> {"ppid":"5dacd245-…",…}
curl -s -X POST localhost:8080/device-identifier/v0.3/retrieve-ppid \
  -H "Authorization: Bearer $(token device-identifier:retrieve-ppid)" \
  -H 'Content-Type: application/json' \
  -d '{"device":{"phoneNumber":"+123456789001"}}'
```

### One Time Password SMS v1

Both endpoints share the `one-time-password-sms:send-validate` scope. The simulator
"sends" a deterministic code — the phone number's last 6 digits, zero-padded — so a
headless caller can compute what to validate (`+123456789012` → `789012`).

```bash
TOK=$(token one-time-password-sms:send-validate)

# Send the code -> {"authenticationId":"…"}; message must contain the {{code}} placeholder
AID=$(curl -s -X POST localhost:8080/one-time-password-sms/v1/send-code \
  -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
  -d '{"phoneNumber":"+123456789012","message":"Your code is {{code}}"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["authenticationId"])')

# Validate it -> HTTP 204 No Content
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
  localhost:8080/one-time-password-sms/v1/validate-code \
  -H "Authorization: Bearer $TOK" -H 'Content-Type: application/json' \
  -d "{\"authenticationId\":\"$AID\",\"code\":\"789012\"}"
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
