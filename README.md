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
