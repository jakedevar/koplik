# Koplik manager handoff: 2026-10-07 ~04:10Z (manager 2, live state)

From: root manager `f708be95-12b4-4fb3-b1fe-6005130b5fa7` (Claude claude-opus-5-5 xhigh), successor of
`7c2df1ae` (see `handoff-2026-10-07-mgr1.md` for the earlier history, RSI mechanics and kaizen list).
This file is the current state for a successor; refresh it before any baton pass.

Read first: `thoughts/shared/manager/manager-brief.md`, `AGENTS.md`, `thoughts/shared/manager/worker-contract.md`,
the published rsi playbook (`git -C ~/rsi show origin/rolling:.claude/skills/rsi-project-manager/SKILL.md`), and
`thoughts/shared/manager/tools/README.md`. My working files were under `/tmp/koplik-mgr2/` (not durable).

## Operator directives (restate in every handoff)

- Autonomous management; ask only for new authority. **Since 2026-10-07 a global manager sits above this seat**
  (node `73306b5f`, session `eca6124e`, operator-appointed). Its standing rules:
  - Technical, data and product questions never go to the operator. Decide them, or ask the global manager with
    `AgentReportUp`; record its ruling in the Issue and SOURCES.md. **Never create operator `decision` records.**
  - Real gates go up through the global manager only: `main` or a public release, real money, credentials,
    deleting user data. Main promotion + public publishing go up as ONE item once QA is green.
  - Host load: before launching a build/test session check `uptime`; queue while the 1-minute load is above 40;
    at most 5 concurrent build-heavy Koplik sessions. The cap of 20 active sessions is a ceiling, not a target.
- Direct operator instructions (delivered at tool boundaries) outrank the global manager; surface conflicts.
- "Make it work first, right second, fast third." "The ground truth is the principle." Honesty rules in AGENTS.md
  (data integrity, science honesty, determinism) outrank schedule.
- Models: only Claude (`claude-sonnet-5-5`, `claude-fable-5-1`) and Codex (`gpt-6.1-sol`, `gpt-6-astra`; `gpt-6-luna`
  easy only), effort `high`; no OpenRouter/DeepSeek. Manager: Claude `claude-opus-5-5` `xhigh`. Every tier-2 review
  comes from the other family; prefer Claude for epidemiology code (Codex safety refusal, #1368).
- Beats at each real milestone (`notify-send -a Koplik "Koplik: BEAT"` + `~/Videos/koplik/beats.log`).

## Rulings and operator instructions in force

- `census-access` = option A with limits (global manager 03:35Z; operator answered the record 03:54Z): fixed
  manifest of exact www2.census.gov URLs with pinned SHA-256, one fetch per file per run, polite UA, no directory
  walks. Implemented by #1350 (`census_files.rs`, `manifests/census-boundaries-2024.json`). Tracker #1375 closed.
- `ingest-contact` = `https://github.com/jakedevar` as committed default (#1413, in review); `KOPLIK_CONTACT`
  overrides; blank refuses. The decision record is still pending in the ledger: until the operator answers it,
  **create_session under E2 is refused** (`manager_v2_pending_operator_decision`); run E2 work as continuations.
  When the repo goes public, switch the default to `https://github.com/jakedevar/koplik`.
- **Operator, ~04:05Z: Census requests use the operator's own e-mail address as the contact** (the address is in
  Issue #1427 and the operator's gitignored local config only; it must never be committed to any tracked file,
  fixture, log or commit message). Overrides the ruling for Census requests. An earlier revision of this file
  (commit ac67064) quoted the address; see the release Issue for how that is being handled.
- Source terms: CDC and Census are US federal public domain; Texas DSHS is public information used with attribution
  and a link (#1414, in progress). Not a gate.
- GitHub: private repo `jakedevar/koplik` created 2026-10-07 ~04:08Z (operator authorization via the global
  manager). It is the `github` remote of the bare origin `~/git/koplik.git`, so origin's post-receive hook mirrors
  every accepted push of `rolling`, `main` and `gh-pages`. Secrets scan of rolling's full history before the first
  push: no token/key patterns, no `.env`/key files, no personal e-mail (commits use jake@rsi.dev; only agency and
  example.* addresses in content), `data/` is 4.5 MB of fixtures and manifests. Do not push `main`; do not enable
  Pages. Going public = the one combined gate (QA green -> main -> repo public -> Pages -> `make publish`).

## Landed on rolling this seat

#1357 (3767e22), #1348 + #1370 #1381 #1385 (13d451c; 3 review rounds in rev 2), #1351 (ef0aed6), #1358 (ad4114e),
#1360 (e5fe87d), #1382 (fee4233), #1349 (3db7ff2; 2 rounds), #1350 (e7f7194). Must tier landed: #1347 #1348 #1349
#1350 #1351 #1353 #1354 #1355 #1356 #1357 #1358 #1360 #1382. DB `accept` stays refused
(`manager_v2_prerequisite_unaccepted`, root cause #1362): land on git evidence + receipt verdict in the merge commit.

## In flight (~04:10Z)

| Issue | Session | State / next |
|---|---|---|
| #1359 pipeline (Must, tier2) | a71fec27 claude-fable-5-1 | running; on RESULT: Codex review; it should use `PoliteConfig::live_from_env()` (#1413) and one fetcher per run (#1424) |
| #1352 population + centroids (Must, tier2) | b0b31e2c gpt-6.1-sol (continuation) | running on #1350's allowlist + wip 872d5428; on RESULT: Claude review |
| #1413 default contact (Must, tier2) | a12a8837 sonnet; review 4c8387ab gpt-6.1-sol (assignment 8e0e5ecb) | in review; accepted -> land |
| #1414 terms + attribution (Must, tier1) | c6662c8e sonnet (continuation) | running; land, then post-land review |
| #1361 forecast + backtest (Should, tier2) | 959e3d13 claude-fable-5-1 | running (paused once for load); backtest scope per #1402 |
| post-land review #1357+#1358 | 2a7eef29 sonnet (assignment e1f66831) | halted for load; resume by continuation when a slot frees |

Queued: post-land review of #1382 and #1414; #1427 Census contact (after #1413, continuation of a12a8837 or 804eb510);
#1424 Census allowlist hardening; #1421 publish from HEAD; #1420 RtEstimate case definition (contracts v4); #1383
coverage caveats; #1400 parameter provenance; #1366; #1369; #1371. QA worker once #1359 gives an offline pipeline
target (`make test determinism web-test` + the offline pipeline from a clean checkout) -> land `qa-green.sha` ->
bring main + publishing to the global manager as one item.

Budget: created sessions 20 of 128 (grant v4). Integration worktree `~/.rsi/koplik-mgr/integrate` (detached),
`CARGO_TARGET_DIR=~/.rsi/koplik-mgr/target`; after each push `git -C ~/koplik merge --ff-only origin/rolling`.

## Kaizen this seat

Filed: #1394 (ancestor cap refusal opaque), #1426 (operator: redesign the manager decisions board). Cancelled as
moot/duplicate: #1387, #1388, #1403. Global manager filed #1415 (agents cannot settle decision records).
Still open from before: #1362, #1364, #1367, #1368, #1378, #1384, #1386.

Friction: none beyond the kaizen above.
