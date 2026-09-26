# AGENT.md — How the CamaraSim build agent works

This is the **source of truth for how the build agent operates**. Read it first,
every pass, and follow it precisely. It formalises the recurring build-agent
workflow; `docs/DESIGN.md` remains the source of truth for *what* to build
(requirements, architecture, auth, versioning, error model, scenarios).

> **Provenance.** This file and `PROGRESS.md` were reconstructed on 2026-09-26
> after they were found missing from the repository (the history had been
> squashed to a single "Initial commit"). The workflow below is transcribed
> faithfully from the scheduled build-agent prompt; if the operator has a
> canonical version of this file, prefer it and reconcile.

## Prime directive

Do exactly **ONE small, verifiable pass, then stop.** `main` is always green.
No behaviour ships untested. The vendored OpenAPI spec never drifts from the code.

## The pass, step by step

1. **Pick the work.** Read `PROGRESS.md`. Pick the top *unclaimed* backlog item,
   respecting **phase order** (see `docs/DESIGN.md` §12): Phase 0 auth first, then
   stateless & non-spatial APIs before stateful/spatial ones. Mark it *in-progress*
   in `PROGRESS.md` before starting.
2. **Scope it down** so it fits a single pass: one endpoint, or one grant type, or
   one API's scenario set — **not a whole API**.
3. **Implement it.** The server stays **non-blocking** (tokio + axum). **All state
   is in memory** (single-node assumption; no external DB). Follow the existing
   module layout: one module per API per major version under `src/apis/`.
4. **Maintain the spec.** For **every** request/response/behaviour change, update
   the vendored OpenAPI under `specs/` in the **same pass**, and document the
   parameter-driven functional cases there (`docs/DESIGN.md` §7 and §9 — endpoint
   `description` and/or `x-camarasim-scenarios`).
5. **Follow CAMARA exactly**, including the error model (`docs/DESIGN.md` §8) and
   honouring the **API version in the URL**. Per-version error catalogs and
   response schemas must match the version mounted in the path.
6. **Test every change.** Unit tests per handler/scenario mapping; in-process
   integration tests that hit the router and assert status + body; contract tests
   against the vendored spec; auth tests per grant type and failure mode. No
   behaviour ships untested.
7. **Verify green.** Both `cargo test` and `cargo build --release` must pass. Note
   the **release binary size** (`target/release/camarasimulator`) and flag any
   regression.
8. **Commit & integrate.** Commit on a branch `agent/<area>/<what>`. Rebase on
   `main`. If green, merge to `main` and push.
9. **Update `PROGRESS.md`.** Mark the item done (or update its sub-steps) and
   append a **one-line entry to the scan journal**, committed in the same branch.

## Context discipline

Read **only**: this file, `PROGRESS.md`, the current target's spec, and the files
you are editing. Never read the whole repo. Use Grep/Glob to locate code. If
context grows large mid-pass, `/compact` and keep only the current target.

## Toolchain & dependencies

- Rust stable (`rust-toolchain.toml` pins it). If `cargo` is missing, install via
  `rustup` non-interactively first.
- Keep the binary **as small as possible**; the release profile is tuned for size
  (`docs/DESIGN.md` §11). **Justify any new dependency** and prefer pure-Rust
  crates; prefer **rustls over OpenSSL**.

## Stop conditions

- **One pass only.** Stop after a single verifiable increment.
- If you **cannot reach a green, tested increment**, record a note in the
  `PROGRESS.md` scan journal and **stop WITHOUT committing** code changes.
- **Never leave `main` red. Never skip tests.**
</content>
</invoke>
