# Koplik: final report (2026-10-07)

Written by manager 3 (session 8672df09, Claude claude-opus-5-5) at the public launch.

**Live site:** https://jakedevar.github.io/koplik/ · **Repository:** https://github.com/jakedevar/koplik
(public; default branch `main`, promoted only from a QA-green SHA).

## What shipped

| Spec question | Shipped | Where |
|---|---|---|
| What is happening? | CDC NNDSS weekly cases by state ("confirmed or unknown-status cases"), Texas DSHS cases by county ("confirmed cases") from live pages and Internet Archive report vintages; map, charts and tables never mix definitions; missing stays missing | `koplik-ingest`, `koplik-pipeline`, `web/` (#1348, #1349, #1382, #1450, #1451) |
| How fast? | R_t (Cori et al.) with credible intervals; "insufficient data" below the minimum count | `koplik-epi::rt` (#1355) |
| What if? | In-browser WebAssembly SEIR simulator, bit-identical to native for a seed; published scenario: a **hypothetical introduction** of one infectious person into Gaines County with real Census 2025 population and DSHS 2023-24 kindergarten MMR coverage | `koplik-epi`, `koplik-wasm`, #1353, #1354, #1357, #1455 |
| Where next? | 4-8 week renewal-projection forecast (Nouvellet et al. 2018) with a pre-registered, pseudo-real-time backtest on the 2025 West Texas outbreak, reported exactly as measured: mean CRPS 3.66, 50% coverage 23/48, 90% coverage 30/48 (intervals too narrow); state forecasts that were not backtested say "No measured skill for this series" | #1361, #1465 |
| Why trust it? | Provenance drawer on every number (source URL, retrieval time, snapshot sha256, terms and attribution); SOURCES.md terms table; content-addressed snapshot store; pipeline manifests hash every input and output | #1348, #1358, #1414, #1458 |

Engineering properties held at release: only `koplik-ingest` touches the network; every test runs
inside a network namespace against committed real-byte fixtures (#1447); native == wasm32
trajectories; byte-identical pipeline re-runs; contracts versioned v1-v5 with JSON Schema (released
versions frozen); `make publish` builds from the committed HEAD and fast-forwards `gh-pages`.

## Release

Operator-approved (#1449). QA-green 5ba510f, then 80d6547 (default contact switched to the public
repository URL), full QA on the operator's satellite laptop each time; main promoted; repo made
public; Pages published from gh-pages 22b6876; live site verified in headless Chromium.
Records: `thoughts/shared/qa/qa-2026-10-07-5ba510f.md`, `qa-2026-10-07-80d6547.md`, Issue #1449.

Before release the operator had the repository history scrubbed of one handoff commit that quoted
personal data (option B); in-flight branches were rebuilt on the scrubbed history with trees
verified identical, and the full history of both origin and a fresh GitHub mirror scanned clean.

## Science honesty: corrections made in the open

- #1455: the manager's first seeding rule seeded a cumulative confirmed count as current infectious
  prevalence. An independent Codex review blocked it; the retained DSHS reports (21 days apart) cannot
  support a data-derived seeding of the 2025 outbreak, so the panel became an explicit hypothetical.
  The original rule and its replay are kept, with a dated correction
  (`thoughts/shared/notes/gaines-2025-scenario-replay.md`).
- #1361: two review rounds fixed an information leak (future case definitions affecting earlier
  forecasts) and an accepted-configuration panic; scores were unchanged and byte-reproduced by every
  reviewer.
- #1465: a review blocked wording that transferred the West Texas backtest's calibration to state
  series it never tested; un-tested series now say so first, and the evaluation sits in its own section.

## Known limits and next work

- No scheduled live refresh: the site shows the committed snapshots with their retrieval times.
- Texas county weekly series are mostly "No data" (DSHS rarely published county tables); a
  cumulative-by-report series with its own contract version is the planned fix (#1439).
- Forecast skill is unmeasured on the published state series (#1503).
- Large payloads: `rt.json` about 14 MB because provenance repeats per row (#1422).
- Open follow-ups: #1400 (fully closed by #1455's companion; verify and close), #1420, #1424, #1434,
  #1445, #1457, #1503, kaizen #1362, #1364, #1367, #1368, #1378, #1384, #1386, #1416, #1423, #1435, #1446.

## Process notes for the next manager

- Land through `~/.rsi/koplik-mgr/integrate` (detached worktree of `~/koplik`); after each push,
  `git -C ~/koplik merge --ff-only origin/rolling`. Manager sandboxes created before the scrub are on
  the old history: never push from them (origin refuses descendants of the scrubbed commit).
- main: `KOPLIK_PROMOTE_MAIN=1 git push origin <qa-green-sha>:refs/heads/main`, only for a QA-green SHA
  under an operator-approved release.
- QA on arch-laptop: `~/koplik-qa/qa.sh <sha>` (clone of origin over ssh; unpushed commits arrive as a
  git bundle); delete its target/ and node_modules afterwards.
- DB-native `request_review` is unreliable across Epics and revised commits (#1364): launch the
  reviewer as a bound Issue session from the exact commit instead.
