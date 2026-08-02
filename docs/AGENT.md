# CamaraSim — Autonomous Agent Playbook

You are the **CamaraSim build agent**. A routine runs you **once per hour**. Each run does
**exactly ONE small, verifiable pass** and then stops. You are headless — never try to run
an interactive session, and never wait on a human.

Read this file, then [`../PROGRESS.md`](../PROGRESS.md). Do the next thing. Stop.

---

## Prime directive

Make **one small increment** that leaves `main` green, tested, spec-accurate, and building
small. A tiny merged improvement beats a large unfinished one. If you cannot find a safe
unit of work, record a note in the PROGRESS scan journal and stop **without committing**.

## Context discipline (important)

Keep context to only what the current pass needs. Do **not** read the whole repo.

Read, in order:
1. `docs/AGENT.md` (this file) and `PROGRESS.md` — always.
2. `docs/DESIGN.md` — only the sections relevant to the claimed item.
3. The **current target's** vendored spec under `specs/…` and the **files you are editing**.

Do not open unrelated API modules. If your context grows large mid-pass (e.g. after reading
a big spec), run **`/compact`** to shed everything except the current target, then continue.
Prefer `Grep`/`Glob` to locate code over reading files whole.

## One pass — procedure

1. **Sync.** Ensure you're on latest `main` (the routine gives you a fresh checkout).
2. **Toolchain.** If `cargo` is missing, install Rust via `rustup` (non-interactive) first.
3. **Pick one item.** Take the top unclaimed item in `PROGRESS.md` (respect the phase
   order — auth first, then stateless & non-spatial APIs). Mark it *in-progress* with the
   run timestamp. Scope it down until it fits one pass (e.g. *one* endpoint, or *one*
   grant type, or *one* API's scenario set — not a whole API at once).
4. **Implement** the increment (server stays non-blocking; state in memory only).
5. **OpenAPI.** For any request/response/behaviour change, update the vendored spec under
   `specs/…` in the **same** pass, including documenting the parameter-driven functional
   cases (see DESIGN §7, §9). Spec and code must not drift.
6. **Tests.** Add/extend tests so the change is covered (DESIGN §10). Every change is tested.
7. **Verify green:**
   - `cargo test`
   - `cargo build --release` (confirm it still builds; note binary size in the journal)
8. **Commit & land.** Branch `agent/<api-or-area>/<short-what>`, commit (see message
   format below), rebase on `main`, and if green **merge to `main` and push**.
9. **Record.** In `PROGRESS.md`: mark the item done (or update its remaining sub-steps) and
   append a one-line entry to the **scan journal** (what you did / what you found / binary
   size). Commit that in the same branch.
10. **Stop.** One pass only.

## Definition of done (per pass)

- [ ] `cargo test` passes and `cargo build --release` succeeds.
- [ ] Every behaviour change has a test.
- [ ] Vendored OpenAPI updated and functional cases documented in the spec.
- [ ] CAMARA error model honoured for the version being implemented.
- [ ] `PROGRESS.md` updated (status + scan-journal line).
- [ ] Merged to `main` and pushed (or, if nothing safe to do, journal note + no commit).

## Commit message format

```
<area>: <what changed in one line>

- functional cases / scenarios added (if any)
- spec: <what changed in specs/…>
- tests: <what was added>
- binary (release): <size>
```

## Guardrails

- **Never leave `main` red.** If you can't get to green, don't merge — journal it and stop.
- **Never skip tests.** No behaviour change ships untested.
- **Never introduce blocking I/O** on the request path. Async all the way.
- **Never add external state** (DB, cache server). In-memory only — single node.
- **Keep the binary small.** Justify every new dependency; prefer `rustls` over OpenSSL.
- **Stay CAMARA-canonical.** Match the official spec for the version in the URL; when you
  must trim a large case space, cover the main successes + the standard error set and say
  so in the spec.
- **Don't run the "real" thing** — there is no real network. There is nothing to benchmark.
