# Koplik

**Measles outbreak intelligence, with a primary source behind every number.**

**Live site: https://jakedevar.github.io/koplik/**

> Demonstration project; not medical or public-health advice; not affiliated with CDC, Texas DSHS,
> the US Census Bureau or WHO.

Koplik spots are the earliest visible sign of measles. Koplik answers, for any US state and for
Texas counties:

| Question | What the site shows | Source |
|---|---|---|
| What is happening? | Weekly cases on a map and charts, always named by case definition: CDC NNDSS "confirmed or unknown-status cases" by state; Texas DSHS "confirmed cases" by county. Definitions are never summed or mixed; missing reports stay missing. | CDC NNDSS, Texas DSHS (live pages and Internet Archive captures) |
| How fast is it spreading? | The effective reproduction number R_t (Cori et al. method) with its credible interval, published only above a minimum case count. | derived, with provenance |
| What if? | An in-browser WebAssembly SEIR simulator for a **hypothetical** introduction of one infectious person into Gaines County, Texas, with its real population and kindergarten MMR coverage. It is not a reconstruction or forecast of the 2025 outbreak. Native and WebAssembly runs are bit-identical for the same seed. | Census 2025 estimates, DSHS coverage, cited parameters |
| Where next? | A 4-8 week renewal-projection forecaster (Nouvellet et al. 2018) with an honest pseudo-real-time backtest on the 2025 West Texas outbreak, reported exactly as measured (mean CRPS 3.66; 90% intervals contained the truth in 30 of 48 targets). Published on the site per #1465. | `crates/koplik-epi`, `data/reports/backtest/` |
| Why trust it? | Every number opens a provenance drawer: source URL, retrieval time, the sha256 of the raw snapshot it came from, the source's terms and attribution. | `SOURCES.md` |

## Architecture

```mermaid
flowchart LR
  subgraph sources[Primary sources]
    CDC[CDC NNDSS weekly tables]
    DSHS[Texas DSHS outbreak pages<br/>+ Internet Archive captures]
    VAX[CDC / DSHS kindergarten<br/>MMR coverage]
    CEN[Census boundaries, 2025 population,<br/>Gazetteer: fixed named-file allowlist]
  end
  subgraph rust[Rust workspace]
    ING[koplik-ingest<br/>polite fetcher, robots, pinned files<br/>content-addressed snapshot store]
    CON[koplik-contracts<br/>versioned shapes v1-v5 + JSON Schema]
    EPI[koplik-epi<br/>SEIR engine, R_t, forecast, backtest<br/>pure, seeded, portable]
    WASM[koplik-wasm<br/>wasm-bindgen facade]
    PIPE[koplik-pipeline<br/>ingest, validate, infer, forecast, build<br/>manifests hash every input and output]
  end
  subgraph web[web/ TypeScript + Vite + MapLibre]
    SITE[Static site<br/>map, charts, R_t, what-if, provenance drawer]
  end
  CDC & DSHS & VAX & CEN -->|only network access| ING
  ING -->|sha256 snapshots| PIPE
  CON -.shapes.-> ING & PIPE & SITE
  EPI --> PIPE
  EPI --> WASM --> SITE
  PIPE -->|web/public/data/*.json + manifest| SITE
  SITE -->|make publish: gh-pages, fast-forward only| PAGES[GitHub Pages]
```

- **Only `koplik-ingest` touches the network.** Tests run offline against committed real-byte
  fixtures in `data/fixtures/`, inside a network namespace (`tools/offline-test.sh`).
- **Determinism.** Every stochastic step takes an explicit seed; native and `wasm32` give identical
  trajectories (`make determinism`); pipeline re-runs are byte-identical.
- **Contracts.** Shared data shapes are versioned in `koplik-contracts`; a released version never
  changes, a new shape is a new version with regenerated JSON Schema and generated web types.
- **Publishing.** `make publish` builds the site from the committed HEAD in a scratch archive, runs
  the offline pipeline there, and fast-forwards `gh-pages` on the repository's origin.

## Build and test

Rust is pinned by `rust-toolchain.toml` (with the `wasm32-unknown-unknown` target); Node and a
Chromium are needed for the web tests.

```bash
make check              # cargo check --workspace --all-targets
make test               # Rust tests, network-isolated
make web-test           # web unit tests, Playwright smoke test, publish test (network-isolated)
make determinism        # native vs wasm32 identical trajectories
make pipeline-fixtures  # build web/public/data from the committed fixtures (offline)
make serve              # serve the site locally
make pipeline           # live: fetch the sources, then build (identifies itself; see SOURCES.md)
```

## Honest limits

- The published data are built from committed source snapshots; each number shows its own
  retrieval time. The reviewed weekly refresh timer is installed by the manager (see below).
- Texas DSHS published county tables only for some reports, so most Texas county weeks are shown as
  "No data" rather than estimated (#1439).
- The what-if panel is a hypothetical introduction, not a fit to the 2025 outbreak: the retained
  DSHS reports are too far apart to seed one honestly (see
  `thoughts/shared/notes/gaines-2025-scenario-replay.md`).
- The forecaster's skill was measured on the Texas DSHS outbreak total; series it was not tested on
  say so (#1503).

## How it was built

Koplik is built by an agent team run by the RSI harness manager, using the integrator model RSI
uses to build itself: workers on sandbox branches, one independent reviewer from another model
family for method, contract, provenance and network changes, and landing on `rolling` by
fast-forward only.

| Read | For |
|------|-----|
| [`AGENTS.md`](AGENTS.md) | How work is done here: principles, hard rules, landing |
| [`SOURCES.md`](SOURCES.md) | Every source: URL, terms, attribution, cadence, client identification |
| [`thoughts/shared/project/koplik-spec.md`](thoughts/shared/project/koplik-spec.md) | Product spec: Epics, intent, acceptance criteria |
| [`thoughts/shared/research/backtest-2025-west-texas.md`](thoughts/shared/research/backtest-2025-west-texas.md) | The pre-registered forecast backtest and its measured scores |
| [`thoughts/shared/manager/`](thoughts/shared/manager/) | Manager brief, worker contract, handoffs |

Branches: `rolling` is agent intake (fast-forward only). `main` is promoted from a QA-green SHA
(`thoughts/shared/qa/qa-green.sha`). `gh-pages` is the published site.

## Weekly refresh (#1507)

After tier2 review, the manager installs from `~/koplik` with
`make install-refresh-timer`, and runs the first refresh by hand with
`systemctl --user start koplik-refresh.service`. The timer runs Thursdays at
21:00 America/Chicago with `Persistent=true`. Inspect it with
`systemctl --user list-timers koplik-refresh.timer` and
`journalctl --user -u koplik-refresh.service`. The operator enables logged-out
timers with `loginctl enable-linger "$USER"`. Without lingering, `Persistent=true`
runs a missed refresh at the next login. The service PATH includes `~/.local/bin`
for user-installed notification and RSI tools.

The installer copies a tiny standalone launcher to
`~/.local/lib/koplik-refresh/bootstrap.mjs`. At each activation it clones the
local bare origin into the state directory and executes the refresh script
from the fetched `origin/rolling` tip; edits in `~/koplik` cannot change the
weekly refresh code. The launcher removes its scratch clone after execution.
The refresh creates a fresh detached clone at `origin/rolling` under
`~/.rsi/koplik-refresh/<run-id>/worktree`, never in the shared checkout or an
agent sandbox. Existing release blobs are immutable and the retrieval log is
append-only. The candidate commit may change only `data/release/**`; staged,
unstaged, untracked and committed paths are guarded. Tests still use
`data/fixtures/`. After verified release, the run's compiler `target/` is deleted.
Each run first
prunes green run directories older than the last two. Failed source trees and
reports remain for diagnosis; compiler targets for failed, interrupted or dry
runs are removed after 14 days. A directory lock prevents overlapping runs;
after a killed run, inspect the journal before removing `~/.rsi/koplik-refresh/lock`.

Before installation, configure the Census contact in the shared checkout's
**gitignored** `.env.local` (`KOPLIK_CENSUS_CONTACT`), and put literal private
personal-data scan patterns, one per line, in
`~/.config/koplik/pii-patterns` (outside git; do not print their contents).
`KOPLIK_ENV_LOCAL` passes only the config path to ingest. The refresh clears
inherited Census overrides so that the shared file is the contact authority.
No contact is copied into the refresh worktree or command logs. Before ingest,
preflight requires a non-blank Census contact in valid UTF-8 config and at least
one usable pattern. Pattern lines are trimmed (including CRLF); blank/comment
lines contribute no patterns. Missing or unusable config or patterns refuses
the run before any live request.

The refresh prepares a data-only commit D at the fetched `origin/rolling` tip,
then runs `make check`, isolated `make test`, `make web-test`, and
`make determinism` on D, builds release data twice and compares every output
hash, and scans D's complete reachable history (commit identities/messages,
blob contents and filenames). `.env*` paths are forbidden in every tree.
`data/release/qa/<run-id>.json` holds the base SHA, checks, output hashes and
scan counts; the refresh does not write the code-tree QA pointer.

The existing publisher builds D locally with `PUBLISH_DRY_RUN=1` and
`PUBLISH_PREPARE_OUTPUT`, imports the exact gh-pages commit G into the refresh
clone without changing refs, and records its source, tree and parent. G's
parent must be the fetched gh-pages tip (or G is a root if the branch is absent).
Refresh verifies G contains `data/manifest.json` and all six v6 artifacts and
uses D's `data/release/` outputs. Every blob and filename in G's tree, including
the web bundle, then passes the personal-data and secrets scans. Only after
all preparation succeeds does one
`KOPLIK_PROMOTE_MAIN=1 git push --atomic origin D:refs/heads/rolling
D:refs/heads/main G:refs/heads/gh-pages` release all three refs. Refresh verifies
they equal D, D and G afterward. Every update is fast-forward only.

Any preparation failure or rejected atomic push leaves all three refs unchanged
and publishes nothing. A concurrent advance skips or rejects the run; the next
run starts from the new tips and repeats QA, with no rebase, force push or
partial retry. Every failure writes
`~/.rsi/koplik-refresh/FAILED-<run-id>.md`. Failing commands include their last
200 lines, with the contact, personal-pattern hits and secrets masked before
persistence. The journal contains phase identifiers without raw child output.
With an RSI session token the refresh also attempts an Issue labeled
`refresh-failure` through `rsi-rpc AgentCreateIssue`. An unattended user timer
normally has no RSI token: the FAILED report plus a `notify-send` attempt is
the supported failure path (also used on RPC refusal).

`make publish` archives committed HEAD and builds offline from `data/release/`
when it exists. Only absence of that directory permits fixture fallback;
a corrupt or incomplete release store refuses publication. The published
`data/publication.json` manifest names the selected input and hashes
`data/manifest.json`. The initial release store was written by the pipeline's
fixture ingest: 22 raw snapshots and original retrieval receipts, with all
12 v6 pipeline artifacts byte-identical to the fixture build.
`make pipeline-release` uses the same selection locally. `make refresh-test`
exercises fake ingest, fake QA and temporary repositories inside the offline
gate, including an offline-prepared live-mode candidate through the real QA
commands. Test stores are seeded by offline fixture ingest rather than copied
from mutable publication data. A separate initial-seed reproduction proof
prints a skip reason once the committed release manifest has mode `live`.
No timer installation, live request or real-origin push is part of tests.
`--dry-run` on the refresh suppresses promotion/publication but **still performs
live ingest**; it is for the manager's reviewed manual run, not an offline test.
