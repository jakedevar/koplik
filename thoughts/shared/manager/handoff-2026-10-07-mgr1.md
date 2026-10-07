# Koplik manager handoff: 2026-10-07 ~02:50Z (manager 1 → manager 2)

From: root manager `7c2df1ae-5567-45cb-b829-3cce39cff950` (Claude claude-opus-5-5).
To: successor launched by `succeed_manager` as Claude / `claude-opus-5-5` / `xhigh`.
Why now: operator, 2026-10-07 ~02:45Z: "When you reach a good stopping point, please pass the baton to another manager."

Read first: `thoughts/shared/manager/manager-brief.md`, `AGENTS.md`, `thoughts/shared/manager/worker-contract.md`,
the spec sections you need, and the published rsi playbook
(`git -C ~/rsi fetch -q origin && git -C ~/rsi show origin/rolling:.claude/skills/rsi-project-manager/SKILL.md`,
sections "First five minutes", "The integrator model", "Baton pass", "Control-surface facts", "Wakes").
Helper scripts are in `thoughts/shared/manager/tools/` (see its README).

## Operator directives (restate in every handoff)

- Autonomous management: use engineering judgment; never ask the operator to approve a routine decision. Ask only
  for new authority: `main` or releases, public publishing, spending beyond the grant, credentials, deleting data,
  accepting a data licence, or a genuine product choice (including the repo's licence). Name the exact gate.
- "Make it work first, right second, fast third."
- "The ground truth is the principle we are trying to achieve." Tests assert the Issue's intent.
- **Models (operator, 2026-10-07 ~02:00Z; supersedes the brief's roster):** no DeepSeek and no OpenRouter at all;
  only the newest models from the direct Anthropic (Claude Code) and OpenAI (Codex) providers. Roster in use:
  Claude `claude-sonnet-5-5`, Claude `claude-fable-5-1`, Codex `gpt-6.1-sol`, Codex `gpt-6-astra` (Codex
  `gpt-6-luna` only for easy tasks), effort `high`. Manager and successor: Claude `claude-opus-5-5` `xhigh`.
  The brief's ">= 3 families in the first fan-out" and "every family authors an Issue" rules are superseded:
  two families; every tier-2 review comes from the other family (Anthropic <-> OpenAI).
- Spread the work across both families; real Issues only.
- Honesty rules in `AGENTS.md` (data integrity, science honesty, determinism) outrank schedule.
- Beats at each real milestone (`notify-send -a Koplik "Koplik: BEAT"` and append to `~/Videos/koplik/beats.log`).
- Operator asked for progress as a percentage (02:42Z); answered: 17% of Must-tier weight landed, about half done
  counting implemented work.

## Seat, policy, budget

- Project `b1e5af47-2425-41f5-a31d-8845556135d3`. Fence at handoff: scope_version 2, policy_version 3 (re-read; never
  reuse). Policy v3 (operator raised it at 01:4xZ): max_created_containers 50, max_active_sessions 20,
  max_created_sessions 32 (11 used before this succession; the successor costs one), allowed_launches empty
  (unrestricted for create_session; fails closed for topologies).
- Brief caps workers at six at once; reviewers are not counted.

## Structure

Groups: "Data & Science" `7fd248c4-c09c-4f68-aaef-cade46b41cb1`, "Product" `d1b37575-d190-4a22-8bad-291dd289597f`.
Epics: E1 `5ee2c462-154a-4434-aa0b-f8834d69e318`, E2 `00abe984-189e-4655-9d9a-eb2bfc6cc98a`,
E3 `e6a97c97-9729-4918-966b-2fb44b117c9a`, E4 `24c8aff1-921e-4897-9cba-9ee8928d3a8f`,
E5 `b5f6c80e-9da3-49f5-bfe9-4a33a8d11f37`, E6 `bbe1ff42-d466-4f74-9c35-5e356b4300b6`,
E7 `51b52ab3-a41d-499d-87e9-95049e7726a0`. Every Must Issue has a daemon work row `issue-<n>` (tier, weight,
dependencies).

## Landed on rolling (tip 2726a42)

| Merge | Issue | Author | Review |
|---|---|---|---|
| e4f9239fcb57f7c75772711aff467198e7113337 | #1347 E1 contracts v1 (+ rework #1363) | Claude claude-sonnet-5-5 (83e594e5) | tier2 gpt-6.1-sol: changes, delta accepted |
| db8b73eeab1cd8e3a63011304ef4c18ceaa9ee5d | #1356 E6 web app | Codex gpt-6.1-sol (cffb8127) | post-land claude-sonnet-5-5: changes -> #1372 |
| ba54f6de1d80b5009a2c226c01fb78d9c1abd5c0 | Makefile: npm ci stamp for web-test/serve | manager | — |
| c5900ed252ec4c97d2fe2ddd234615b3af042734 | #1372 web follow-up | Codex gpt-6.1-sol (cffb8127) | — |
| 2726a42 | #1380 web items 1-2 (R_t paired levels) | Codex gpt-6.1-sol (cffb8127) | covered by #1355 delta review |

Closed: #1347, #1363, #1356, #1372.

## In flight (sessions, exact SHAs, next action)

Arm an `on_terminal` watch on every running session below (your predecessor's watches do not wake you), and verify
`watch_state: enabled` with `AgentGetProgress` (kaizen #1384: a re-arm can dedupe onto a consumed job).

| Issue | Session (model) | State | Next action |
|---|---|---|---|
| #1348 store + CDC (+ #1370, #1381 contracts v2 case definition) | worker a12a8837 (claude-sonnet-5-5), done at 27fa26b2cef6a8b31e9126a0920a91432dd0044d | closure review round 3: reviewer 34250633 (Codex gpt-6-astra), assignment 22c802e2-df9e-4ef5-a3d3-4b3d082d8c7e | accepted -> merge rsi/a12a8837 (verdict chain in the merge commit: rounds c5515750 changes, b60941b9 changes, 22c802e2), close #1348 #1370 #1381. Round budget for spec revision 1 is then spent: further changes need a new spec revision. |
| #1351 coverage | worker b0b31e2c (gpt-6.1-sol) at ba2f389e6b729ac6c569d9e9947d234d855a1a13 | review ACCEPTED (claude-sonnet-5-5, fd6c07b8) | land only AFTER #1348 (its branch contains pre-review store fed92ae); merge, make check test, close. |
| #1349 Texas DSHS cases | worker 804eb510 (claude-sonnet-5-5), branched from fed92ae | running | RESULT -> tier2 review by Codex; it needs a Census FIPS lookup (see census-access). Lands after #1348. |
| #1353 engine (rework #1379 BTPE step 5.3) | worker 6828158e (claude-fable-5-1) | running | RESULT -> author.py 1353, delta review delta_of 26528fd3-7c4d-45b7-93ce-766869a2b91b keys btpe_stirling_bound,primary_source_limits (Codex; round 2 may reuse gpt-6.1-sol). Merge with #1355 by union (below). |
| #1355 R_t (rework #1380) | worker 75ab1fe3 (claude-fable-5-1); bc7c65b fixed daily_negative_sd; now merging rolling 2726a42 | running | RESULT -> author.py 1355, delta review delta_of aaafe6eb-1590-4712-bd48-9f6b68f9fb9e keys rt_loader_rejects_levels,rt_chart_breaks_paired_levels,daily_negative_sd (Codex gpt-6.1-sol). |
| #1354 WASM + make determinism | worker ea567f9d (gpt-6.1-sol) done at 013f86299dd34b6f97a45b8bd271e69cef18fad9 | review by 718b2e5f (claude-sonnet-5-5), assignment a35749f4-f1e6-44cd-aa04-e73ce691b508 | Land after #1353: continue ea567f9d to merge rolling (BTPE fix changes the golden fingerprint) and, if #1381's v2 has landed, move its simulation result types to contracts **v3** (= v2 + simulation types); then make determinism + make check test. |
| #1350 Census boundaries | worker 8b8b85d9 (gpt-6.1-sol), wip f497f1786675a7a9cde74d1ef7613e0a8fe9ae6e | partial: Census robots | blocked on operator decision `census-access`; relaunch (`launch.py ... --commit=f497f17...`) after it. |
| #1352 population + centroids | worker 40826974 (gpt-6-luna), wip 872d5428ca6f66b8e1cf9cceea97b75e507e5278 | partial: Census robots | blocked on `census-access`; relaunch from wip after it (consider a stronger model). |

Not started (dependencies on the work rows): #1357 what-if (needs #1354, #1351), #1358 drawer + Playwright smoke
(Chromium in RSI sandboxes needs `TMPDIR=/tmp`, #1367), #1359 pipeline (needs sources + R_t; must convert case rows to
v2), #1360 make publish, #1361 forecast + backtest (Should). Follow-ups: #1382 (v2 case counts end to end: web, R_t,
DSHS; Must), #1383 coverage caveats (Should), #1366 MMWR extreme dates (minor), #1369 R_t imported cases, #1371 Xia 2004
gravity source.

`koplik-epi` merge of #1353 and #1355: both create the crate. Resolve by union: `lib.rs` declares
`defaults, ensemble, fingerprint, gravity (private), rt, sampling (private), seir` plus #1353's re-exports;
`Cargo.toml` takes #1355's description and the union of dependencies (sha2, hex, thiserror, libm, rand_chacha 0.9,
rand_core 0.9; dev serde, serde_json; the `seir_ensemble` bench); `Cargo.lock`: take ours, let cargo re-resolve.
A trial merge of the pre-rework branches was green (57 tests).

## Operator decisions

- **Pending: `census-access`** (manager decision record, epic E3, work issue-1352; tracker Issue #1375). www2.census.gov
  robots.txt groups `User-agent: *` with RavenCrawler's `Disallow: /` under RFC 9309 (blank line does not end a group);
  api.census.gov requires a key. Options: A allowlisted exception for named public-domain files (recommended), B Census
  API key (credential; population only), C labelled secondary mirror. Disclosure already made: one manual GET of
  cb_2024_us_state_20m.zip to /dev/null.
- **To ask before publishing (one combined request):** data licences/terms (CDC NNDSS x9gk-5huc: no licence in
  metadata; CDC SchoolVaxView; Texas DSHS cases and coverage), and a verified `KOPLIK_CONTACT` for the User-Agent
  (never the operator's e-mail). Then `main` promotion from the QA-green SHA, then public publishing.

## RSI mechanics learned (save yourself the digging)

- Work rows of kind product need `required_gates` implementation, review, verification, integration.
- `request_review` needs a prior implementation `stage` update with evidence (`author.py`); else
  `manager_review_author_missing` (#1364).
- Reviewers re-invoked by the no-result guard break DB acceptance: `accept` is refused
  `manager_review_source_not_accepted` (#1362). Land on git evidence plus the receipt verdict, recorded in the merge
  commit. `review.py`'s prompt asks reviewers for a RESULT line to avoid the re-invocation.
- Max three review rounds per spec revision; round three must use a model not used in earlier rounds
  (`manager_review_closure_specialist_required`).
- Reworks: `AgentContinueChild` on the completed worker (no new session charged), then re-arm and verify its watch.
- Codex's safety filter stopped gpt-6-astra while it was writing the SEIR engine ("possible biological risk"); Codex
  gpt-6.1-sol reviews of the epidemiology code passed. Prefer Claude to author epidemiology code (#1368).
- `rsi-rpc` prints a socket line on stderr: use `2>/dev/null` before `jq`.
- Integration: detached worktree `~/.rsi/koplik-mgr/integrate`, `CARGO_TARGET_DIR=~/.rsi/koplik-mgr/target`;
  after each push fast-forward the shared `~/koplik` (`git merge --ff-only origin/rolling`).

## Kaizen

Filed (RSI-side, open): #1362 (no-result guard re-invokes reviewers, breaks acceptance), #1364 (review author step),
#1368 (provider safety refusal), #1384 (watch re-arm dedupes onto a consumed job); worker-filed and kept: #1367
(Chromium TMPDIR), #1378 (no-result after RESULT). Cancelled as duplicates or Koplik work: #1365, #1373, #1374, #1376,
#1377. Relabelled #1375 to the data-access decision. Fixed: none yet (all RSI-side).

## Next actions (in order)

1. First five minutes: verify the seat, drain the inbox (operator answer to `census-access`?), arm and verify watches.
2. As each review or worker reports: land #1348 then #1351; review #1349; delta-review #1353 and #1355, merge them
   together (union resolution above); land #1354 after #1353 (v3 if needed).
3. After `census-access`: relaunch #1350 and #1352 from their wip commits.
4. Then #1382, #1357, #1359, #1358, #1360; QA worker on the rolling tip (`make test determinism web-test pipeline`
   from a clean checkout) and land the QA-green SHA pointer; then the publish sequence in the brief.

Friction: none beyond the kaizen above.
