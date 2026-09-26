# AGENT.md — how the CamaraSim build agent works

This file is the **source of truth** for how the autonomous build agent operates on
CamaraSim. Read it first, every pass, and follow it precisely. It sits alongside
[`DESIGN.md`](DESIGN.md) (what we are building and why) and
[`../PROGRESS.md`](../PROGRESS.md) (what is done, what is next, and the scan journal).

The agent runs as a scheduled routine with **no human watching live**. Each firing does
**exactly one small, verifiable pass, then stops.** Small and green beats big and broken.

---

## 1. The one-pass loop

Every firing follows these steps in order:

1. **Read the harness.** Read this file, then `PROGRESS.md`. Read the current target's
   vendored spec under `specs/` and only the source files you will edit. Do **not** read
   the whole repo — use `Grep`/`Glob` to locate code.
2. **Pick one item.** Take the top *unclaimed* backlog item in `PROGRESS.md`, respecting
   **phase order** (DESIGN §12): Phase 0 auth first, then stateless & non-spatial APIs
   before stateful/spatial ones. Mark it **in-progress** in `PROGRESS.md`.
3. **Scope it to one pass.** One endpoint, or one grant type, or one API's scenario set —
   **never a whole API** in a single pass. If the item is bigger, split it and do the
   first slice, updating the item's sub-steps.
4. **Implement it.** Keep the server **non-blocking** (tokio/axum, no blocking calls on
   the async runtime) and keep **all state in memory** (single node — DESIGN §2, §5).
5. **Maintain the spec.** For **every** request/response/behaviour change, update the
   vendored OpenAPI spec under `specs/` in the *same* pass, including the parameter-driven
   functional cases (DESIGN §7 and §9). The spec and the server never drift.
6. **Follow CAMARA exactly.** Honour the error model and the API **version in the URL**
   (DESIGN §8, §9). Per-version error catalogs and schemas must match.
7. **Test everything.** No behaviour ships untested (DESIGN §10). Add unit tests for
   scenario mapping, integration tests that hit the router in-process, and contract checks
   against the vendored spec where applicable.
8. **Verify green.** Both `cargo test` and `cargo build --release` must pass. Note the
   release binary size in the scan journal.
9. **Land it.** Commit on a branch `agent/<area>/<what>`, rebase on `main`, and **if
   green** merge to `main` and push (see §5).
10. **Record it.** In the same branch, mark the item done (or update its sub-steps) in
    `PROGRESS.md` and append a one-line entry to the scan journal.

**One pass only.** When the slice is landed and recorded, stop.

---

## 2. Hard rules (never violate)

- **Never leave `main` red.** If you cannot reach a green, tested increment, write a note
  in the `PROGRESS.md` scan journal explaining why and **stop without committing** (leave
  the tree clean). Do not merge a red or untested change.
- **Never skip tests.** Every behaviour change ships with tests in the same pass.
- **Never block the async runtime.** No blocking I/O, no `std::thread::sleep`, no blocking
  locks held across `.await` on the request path.
- **Never introduce external state.** In-memory only; single-node assumption holds.
- **Never do more than one small pass.** Resist scope creep even when the next step looks
  trivial. The next firing will take it.

---

## 3. CAMARA fidelity checklist

When implementing or changing an endpoint:

- **URL versioning** is canonical: `/{api-name}/v{MAJOR}/{resource}` (DESIGN §9). Serve
  each major version from its own module + spec + error catalog.
- **Error model**: body is `{ "status", "code", "message" }` with the CAMARA `code` enum
  for that version (DESIGN §8). Cover 400/401/403/404/409/422/429/5xx as the API defines.
- **Auth**: business endpoints are protected. Validate the Bearer JWT — scopes, audience,
  PKCE, expiry — per the CAMARA Security & Interoperability Profile (DESIGN §6). In sim
  mode consent auto-approves, but flow validation stays faithful.
- **Parameter-driven cases** (DESIGN §7): the request input (e.g. `phoneNumber`, `device`)
  is the control plane. Reuse the shared scenario convention in `src/scenarios.rs`
  (reserved suffixes select CAMARA errors). Default input → happy path. **Document every
  case in the endpoint's OpenAPI `description` / `x-camarasim-scenarios`** so users
  discover them from the spec alone.
- **Discovery**: `GET /` lists mounted APIs; `GET /{api}/v{n}/openapi.yaml` serves the
  spec; `GET /{api}/v{n}/docs` serves human docs. New mounts wire into
  `src/apis.rs::routes()` and the registry.

---

## 4. Where things live

| Path | What |
|---|---|
| `docs/AGENT.md` | This file — the operating procedure. |
| `docs/DESIGN.md` | Requirements, architecture, auth, versioning, error & scenario strategy. |
| `PROGRESS.md` | Backlog (phase-ordered), item status, and the scan journal. |
| `src/main.rs`, `src/server.rs`\* | Bootstrap and router composition. |
| `src/auth/` | OIDC/OAuth: discovery, JWKS, token (all grants), authorize, CIBA, verify, purpose/scopes. |
| `src/registry.rs` | API + version registry → canonical versioning + spec/docs serving. |
| `src/scenarios.rs` | Parameter → functional-case resolution (shared reserved-suffix convention). |
| `src/errors.rs` | CAMARA error model / per-version catalogs. |
| `src/apis/<api>/<version>.rs` | One module per API per major version (handlers, types, scenarios, tests). |
| `src/apis/<api>/store.rs` | In-memory state for stateful APIs. |
| `specs/<api>/<version>/` | Vendored, annotated CAMARA OpenAPI spec. |

\* Router composition currently lives in `src/main.rs` / `src/apis.rs`; keep entries
alphabetical-ish and mirror the module list in `src/apis.rs`.

`vwip` in a module/URL path means "work-in-progress version": the API is scaffolded but
not yet pinned to a released CAMARA version. Pinning a `vwip` API to its real `v{N}` (and
verifying its scenarios/errors against the released spec) is a normal backlog item.

---

## 5. Git workflow

- Branch name: `agent/<area>/<what>` (e.g. `agent/sim-swap/error-catalog`,
  `agent/auth/ciba-polling`, `agent/docs/bootstrap-harness`).
- Work on the branch, keep commits focused. Rebase onto the latest `main` before merging:
  `git fetch origin main && git rebase origin/main`.
- Re-run `cargo test` after the rebase. **Only if green**, fast-forward/merge to `main` and
  `git push -u origin main` (retry with exponential backoff on network errors).
- Update `PROGRESS.md` (item status + one-line journal entry) **in the same branch** so the
  record lands with the change.
- If `main` was reset or diverged, restart the branch from the latest `origin/main` rather
  than stacking onto stale history.

---

## 6. Toolchain & build

- Rust stable via `rust-toolchain.toml`. If `cargo` is missing, install via `rustup`
  non-interactively first.
- Keep the binary **as small as possible** (DESIGN §11: `opt-level="z"`, `lto`,
  `codegen-units=1`, `panic="abort"`, `strip`). **Justify any new dependency** and prefer
  `rustls` over OpenSSL to stay static and small. Measure and record the release binary
  size each pass.
- Fast inner loop: `cargo test` (debug). Ship gate: `cargo test` **and**
  `cargo build --release` both green.

---

## 7. Context discipline

Read only: `docs/AGENT.md`, `PROGRESS.md`, the current target's spec, and the files you
are editing. Never read the whole repo. Use `Grep`/`Glob` to locate code. If context grows
large mid-pass, run `/compact` and keep only the current target. Finish the one pass, then
stop.
</content>
</invoke>
