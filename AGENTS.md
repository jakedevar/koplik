# AGENTS.md

Instructions for AI coding agents in this repo. `CLAUDE.md` is a symlink to this
file. This repo is built the way `rsi` builds itself: the same principles,
integrator model and hard rules, adapted to a data-and-science product. The
product spec is `thoughts/shared/project/koplik-spec.md`; read the section
your Issue names, not the whole file.

## How to work here

**Make it work, then make it right, then make it fast.** Land small, working
changes on `rolling` often. A working change on `rolling` beats a perfect one
stranded on a branch.

- **The principle is the ground truth, not the code.** Every Issue states its
  intent and acceptance criteria; tests assert that intent. When code
  conflicts with the stated principle, change the code.
- Read the code you change and follow its existing patterns. Keep diffs focused.
- Make technical decisions yourself and note consequential ones in your commit
  or handoff. Ask the operator only about `main` or releases, publishing
  anything publicly, real spending, credentials, deleting data, accepting a
  data licence, or a genuine product choice.
- If a process step blocks a working change and is not a hard rule below, take
  the reasonable path, say so in your RESULT or handoff, and keep moving. Do
  not stall waiting for ceremony.
- Tests: cover what you change. The landing gate is **no new failures relative
  to `rolling`**. Name baseline reds you hit; fix unrelated reds separately.
- Preserve existing behaviour unless the task is to change it.

## Hard rules (the only ones)

1. **Git.** Never `--force`, `--force-with-lease` or a `+refspec`. `rolling`
   only moves by fast-forward. A rejected push means fetch, integrate, retry.
   Never push `main`: it is operator-only and is promoted from a QA-green SHA.
   `origin` refuses non-fast-forward updates and ref deletion.
2. **Sandbox.** Work and commit in your `sandbox_root` on its assigned branch,
   never in the shared `~/koplik` worktree. Do not checkout, switch or reset
   your sandbox branch.
3. **Data integrity.** Never fabricate, hand-edit or silently impute data.
   Raw source snapshots are immutable and content-addressed (sha256). Every
   published number must trace back to snapshot hash → source URL → retrieval time.
   Missing or ambiguous data is shown as missing, never guessed.
4. **Contracts.** Shared data shapes live in `koplik-contracts`. A shape change is
   a new contract version with regenerated JSON Schema. Released contract
   versions are immutable. It is the equivalent of a migration.
5. **Science honesty.** Report measured scores exactly as measured. Never tune to
   the backtest answer. Below the minimum-count threshold, publish "insufficient
   data" rather than an estimate. Every model parameter is configurable and cites
   its source in a code comment.
6. **Determinism.** All stochastic code takes an explicit seed. The same seed and
   inputs give identical output on native and `wasm32` builds. Follow the
   portability rules in the spec's E4 (portable RNG, no `usize` in draws, `libm`
   for transcendentals, ordered maps, no parallel float reductions).
7. **Network.** Only `koplik-ingest` touches the network. Every test runs offline
   against committed fixtures in `data/fixtures/`.
8. **Secrets.** Never write credentials or `$RSI_SESSION_TOKEN` into files,
   logs, prompts or `--params`.
9. **Formatting.** Format only the files you changed:
   `git diff --name-only --diff-filter=d -- '*.rs' | xargs -r rustfmt --edition 2024`.
   Never `cargo fmt` a whole crate you did not change. Stage explicit paths,
   never `git add -A`.
10. **Identity in tests.** Never assert that a user-visible name, title or label
    is absent. Assert the positive end state.
11. **Commit before you finish.** Uncommitted sandbox work can be reclaimed.
    Commit scoped changes with the repo's message style and no attribution footers.
    Every commit carries the trailer `Rsi-Session: <your RSI session id>`. It is
    provenance, not attribution: it maps each commit to the session (and model)
    that wrote it. See the worker contract.
12. **No bare `git stash`.** The stash is shared across worktrees.
13. **Long runs.** Run every cargo command through the machine's resource governor
    (`~/.rsi/bin/cargo-slot cargo ...`). Run it in the foreground and `tee` long
    runs to a log. Never report a run you did not see finish as green.

## Project map

Koplik is a static web app for measles outbreak intelligence (cases by US state and
Texas county, R_t, an in-browser "what-if" simulation, an outbreak forecast, and
provenance on every number). Planned layout; the Contracts Issue creates the workspace:

- `crates/koplik-contracts`: shared types, contract versions, JSON Schema.
- `crates/koplik-ingest`: source connectors and the content-addressed snapshot store.
- `crates/koplik-epi`: SEIR engine, nowcast, R_t, backtest scoring. Pure: no I/O.
- `crates/koplik-wasm`: `wasm-bindgen` facade over `koplik-epi`.
- `crates/koplik-pipeline`: CLI stages: ingest → validate → infer → forecast → build.
- `web/`: TypeScript + Vite + MapLibre. It consumes pipeline artifacts and the WASM engine.
- `data/fixtures/` (committed test inputs); `data/snapshots/` (gitignored raw archive).
- `SOURCES.md`: every source with its URL, licence or terms, and cadence.
- `tools/hooks/`: the bare `origin`'s guard and mirror hooks (operator-owned; do not edit).

## Build and test

Rust is pinned by `rust-toolchain.toml` (1.94.1, the same version as `rsi`, plus `wasm32-unknown-unknown`).
The first Issues create these Makefile targets. Until they exist, use the cargo equivalents:

```bash
make check       # cargo check --workspace --all-targets
make test        # cargo test --workspace (offline, fixtures only)
make wasm        # build koplik-wasm for wasm32 + bindings
make determinism # native vs wasm identical-trajectory test
make web-test    # web unit + e2e
make pipeline    # run the pipeline from snapshots to web/public/data
make serve       # serve the built site locally
make publish     # build the site for GitHub Pages and push it to origin's gh-pages branch
```

## Landing

`rolling` is agent intake: the manager (the integrator) merges worker commits,
runs `make check` plus the tests for the touched modules, and fast-forwards
`rolling`. Work is landed when `git merge-base --is-ancestor <SHA> origin/rolling`
holds.

Pre-merge review happens only for contract-version changes, epidemiological
method changes (engine, R_t, forecast, backtest scoring), provenance code, and
network, credential or publish changes. It is one plain reviewer pass by a
different model family, with the verdict noted in the merge commit. Everything
else lands first and gets one post-land review.

The QA sweep records each passing `rolling` SHA in `thoughts/shared/qa/qa-green.sha`
(a file, not a branch). The operator promotes `main` only from that SHA.

Public publishing is the operator's decision. The operator approves it by adding a
`github` remote to the bare `origin`. From then on, `origin` mirrors every accepted
update to `rolling`, `main` and `gh-pages` to the public GitHub repo (hook
`tools/hooks/origin-post-receive`), so **a push to `origin` is then a public push**.
Agents never push to GitHub directly.

Workers follow `thoughts/shared/manager/worker-contract.md`. The manager follows
`thoughts/shared/manager/manager-brief.md`.

## Agent control

Drive the daemon with `rsi-rpc <Verb>` (`rsi-rpc agent` lists verbs) or the
native `rsi_control_*` tools. Messages wrapped in `<rsid-daemon-message>` come
from the daemon, not the human.

## Where things are

- Spec `thoughts/shared/project/koplik-spec.md`
- Plans `thoughts/shared/plans/`; research `thoughts/shared/research/`; notes `thoughts/shared/notes/`
- Manager brief and worker contract `thoughts/shared/manager/`
- QA pointer `thoughts/shared/qa/qa-green.sha`
