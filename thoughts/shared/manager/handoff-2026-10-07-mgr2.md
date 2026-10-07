# Koplik manager handoff: 2026-10-07 ~04:55Z (manager 2 -> manager 3)

From: root manager `f708be95-12b4-4fb3-b1fe-6005130b5fa7` (Claude claude-opus-5-5 xhigh), successor of `7c2df1ae`
(earlier history, RSI mechanics and older kaizen: `handoff-2026-10-07-mgr1.md`). Reason: context at ~55%; handing
off at a clean point (every in-flight session watched, nothing half-merged).

Read first: `thoughts/shared/manager/manager-brief.md`, `AGENTS.md`, `thoughts/shared/manager/worker-contract.md`,
the published rsi playbook (`git -C ~/rsi show origin/rolling:.claude/skills/rsi-project-manager/SKILL.md`), and
`thoughts/shared/manager/tools/README.md`. Release tracking: **Issue #1449**.

## Chain of authority and directives (restate in every handoff)

- **Global manager** above this seat: node `73306b5f`, session `eca6124e` (operator-appointed). Its standing rules:
  - Technical, data and product questions never go to the operator: decide them, or ask the global manager with
    `AgentReportUp`; record its ruling in the Issue and SOURCES.md. **Never create operator `decision` records.**
  - Host load: before launching a build/test session check `uptime`; queue while the 1-minute load is above 40;
    at most 5 concurrent build-heavy Koplik sessions. The cap of 20 active sessions is a ceiling, not a target.
  - Created sessions: this seat's policy (operator v3 snapshot) allows 32 (21 used at handoff). A successor the
    global manager seats with AgentManagerAppointChild inherits its grant (128 created). Prefer continuations
    (`AgentContinueChild` on a completed worker: free, keeps context).
- Direct operator instructions (they arrive at tool boundaries as "[message from operator ...]") outrank the
  global manager; surface conflicts, then follow the operator.
- "Make it work first, right second, fast third." "The ground truth is the principle." Honesty rules in AGENTS.md
  (data integrity, science honesty, determinism, offline tests) outrank schedule.
- Models: only Claude (`claude-sonnet-5-5`, `claude-fable-5-1`) and Codex (`gpt-6.1-sol`, `gpt-6-astra`;
  `gpt-6-luna` easy only), effort `high`. Every tier-2 review comes from the other family. Prefer Claude to author
  epidemiology code (Codex safety refusal, #1368). Manager: Claude `claude-opus-5-5` `xhigh`.
- Beats at real milestones: `notify-send -a Koplik "Koplik: BEAT"` + `printf '%s %s\n' "$(date -u +%FT%TZ)" "BEAT"
  >> ~/Videos/koplik/beats.log` (BEAT under 80 chars).

## The release (operator-approved; currently HALTED by my mistake)

Operator, via the global manager, ~04:40Z: "Permission granted, whenever it's ready." The manager runs the release
itself once these hold: (1) #1359 landed and a full QA pass on one rolling SHA is green (`make check test`,
`make web-test`, Playwright smoke, offline pipeline), recorded in `thoughts/shared/qa/qa-green.sha`; (2) Must Issues
closed (#1413, #1414 landed); (3) a secrets/personal-data scan of the full history reachable from that SHA is clean;
the operator's e-mail address must appear in no tracked file, fixture, log or commit. Steps then, in order:
(a) fast-forward `main` on origin to the QA-green SHA; (b) switch the non-Census default contact to
`https://github.com/jakedevar/koplik` (land it; re-promote main if after the QA SHA); (c) `gh repo edit
jakedevar/koplik --visibility public --accept-visibility-change-consequences`; (d) enable Pages from gh-pages and run
`make publish` through origin; (e) check the live site (attribution section + one chart), record the URL, report
up once. Stop at the last good step on any failure.

**Blocker (precondition 3):** commit `ac67064` (my handoff, 04:06Z) quoted the operator's e-mail address. It is on
rolling and mirrored to the private GitHub repo; not on main; nothing else in history has it (scan:
`git log origin/rolling -S<address>`). The tip was cleaned at `f68ceb2`. I reported options A (accept), B (operator
rewrites history with `git filter-repo --replace-text` on the bare origin, then force-updates/recreates the private
GitHub repo; recommended), C (squashed public repo; not recommended) in AgentReportUp `4b42fdd7`. **Await the
answer.** If B: every rolling commit from ac67064 on gets a new SHA; rebuild in-flight branches (#1359, #1421,
#1427 redo) by cherry-picking their own non-merge commits onto the rewritten rolling (never merge a branch that
still contains ac67064), and re-run their gates.

## Other rulings in force

- `census-access` = option A with limits (pinned named-file allowlist, `crates/koplik-ingest/manifests/`); answered.
- `ingest-contact` = `https://github.com/jakedevar` default (#1413, landed); the ledger record is still pending, so
  **create_session under E2 is refused**: launch E2 work under E3/E7 when it fits, or by continuation.
- Census contact = the operator's e-mail (direct instruction ~04:05Z; address in Issue #1427 and the operator's
  gitignored local config ONLY). #1427 redo is in flight (below). After it lands, write the address into
  `~/koplik/.env.local` (untracked; verify `git check-ignore .env.local`) as `KOPLIK_CENSUS_CONTACT=...`.
- Source terms: CDC and Census public domain (17 USC 105); Texas DSHS public information with attribution + link
  (#1414 landed). GitHub: private repo `jakedevar/koplik` = `github` remote of the bare origin; origin's
  post-receive hook mirrors rolling/main/gh-pages (agents never push GitHub directly).

## Landed on rolling (tip f68ceb2)

This seat: #1357 3767e22; #1348 (+#1370 #1381 #1385) 13d451c; #1351 ef0aed6; #1358 ad4114e; #1360 e5fe87d; #1382
fee4233; #1349 3db7ff2; #1350 e7f7194; #1413 82e0bf8; #1414 7d3892a; #1352 e650a05; handoffs ac67064, f68ceb2.
Must weight landed 39 of 45 (87%). DB `accept` is refused (`manager_v2_prerequisite_unaccepted`, root cause #1362):
land on git evidence + review receipt verdict in the merge commit.

## In flight (~04:55Z) - re-arm an on_terminal watch on each and verify `watch_state: enabled`

| Item | Session | Next |
|---|---|---|
| #1359 pipeline (Must, tier2) | worker a71fec27 (claude-fable-5-1) | on RESULT: author.py + Codex review; then QA |
| #1421 publish from HEAD (Must, tier2) | worker 963e73c0 (gpt-6.1-sol, continuation) | on RESULT: Claude review, land |
| #1427 Census contact redo (Must, tier2) | worker 74694b28 (claude-sonnet-5-5), base f68ceb2 | on RESULT: grep the diff for the address (must be absent) then Codex review, land |
| #1361 forecast + backtest (Should) | review bf61f1e8 (gpt-6.1-sol, assignment c6e1a75d) of 2b1cef03 | accepted -> land (gates: epi + ingest tests, determinism, check); note it changed #1349's confirmed_basis rule |
| post-land review #1382+#1414 | reviewer 0c65aa21 (gpt-6.1-sol, assignment 20f28b12) | it appears to have filed #1450-#1453; triage them |

Do NOT merge branch `rsi/a12a8837-...` at or after `04834a4` (it commits the operator's address).

## Queue after that

QA worker on one rolling SHA once #1359 lands -> land qa-green.sha -> release steps (if the history question is
answered). Then: #1447 network-less test gates (Must tooling), #1424 Census allowlist hardening, #1434 and
#1450-#1453 web follow-ups, #1439 (product decision on near-empty Texas county weekly rows: decide or ask the
global manager), #1420 RtEstimate case definition (contracts v4), #1400, #1422, #1445, #1383, #1369, #1371, #1366.
README architecture diagram + final report (`thoughts/shared/notes/final-report.md`) before "done".

## Mechanics learned this seat

- Never run `rustfmt` on `lib.rs`: it recurses into every module; format only the files you changed.
- Landing gates can outlive the 10-minute Bash cap under load: use `AgentSubmitJob` (kind build/test,
  `worktree` = `~/.rsi/koplik-mgr/integrate`, wake none) + one `AgentScheduleWake` mode `when` jobs_terminal.
  `make web-test`/`determinism` are not job kinds: run them in the foreground when the load is low (#1446).
- Branches that landed in parallel can conflict semantically: after a merge run the touched crates' tests AND
  `make web-test` (the attribution test couples SOURCES.md and web/src/attribution.ts).
- An offline test that relies on an unset env var can become a live network test after a semantic change (it
  happened once landing #1352; #1447). Review merged tests for `env_remove` on live commands.
- AgentHalt of a reviewer makes the daemon re-issue the review on a fresh session (#1435).
- E2 create_session is blocked by the pending `ingest-contact` record.
- Integration worktree `~/.rsi/koplik-mgr/integrate` (detached), `CARGO_TARGET_DIR=~/.rsi/koplik-mgr/target`;
  after each push `git -C ~/koplik merge --ff-only origin/rolling`. Union-merge helper used for additive
  conflicts: keep both sides, then fix by hand anything inside a function or table.

## Kaizen this seat

Filed: #1394, #1426 (moved to Rsi #1428), #1435, #1446 (worker-filed; manager evidence added). Cancelled as moot or
duplicate: #1387, #1388, #1403. Open from before: #1362, #1364, #1367, #1368, #1378, #1384, #1386; also #1416, #1423.

Friction: #1435, #1446 | succession: hand the seat over via the global manager (AppointChild inherits 128 creations).
