# Koplik manager handoff: 2026-10-07 ~07:50Z (manager 3)

From: project manager `8672df09-6f27-49c3-b67c-d2d8dc9e5317` (Claude claude-opus-5-5), successor of
f708be95 (`handoff-2026-10-07-mgr2.md`). Reason: the project's planned scope is complete and live;
clean point (nothing in flight, nothing half-merged). Context at handoff ~42%.

**Live:** https://jakedevar.github.io/koplik/ · **Repo:** https://github.com/jakedevar/koplik (public,
default branch main). main = 1b761f5 (QA-green), gh-pages = 98c0264, rolling = this commit's parent
chain from 55f1a71. Final report: `thoughts/shared/notes/final-report.md`. Release record: #1449 (closed).

## Chain of authority and directives (restate in every handoff)

- Global manager above this seat: node `73306b5f`, session `eca6124e`.
- Operator directives:
  - Decide yourself. Technical, data and product questions never go to the operator: decide them,
    or ask the global manager with AgentReportUp. Do not record operator `decision` records. Only
    real gates go up: main or releases, real money, credentials, deleting user data.
  - Managers run on Claude Opus 5.5; workers on Sonnet 5.5, Haiku 5.5, gpt-6.1-sol or gpt-6-luna
    (this seat's allowed_launches also include gpt-6-astra; claude-fable-5-1 is NOT allowed);
    reviewers from the other model family.
  - Build fast, fix forward. Tests never touch the network (#1447: enforced by a network namespace).
  - Host load: check `uptime` before launching build/test sessions; queue above a 1-minute load of
    40; at most 5 concurrent build-heavy Koplik sessions.
  - Kaizen: RSI harness problems are worked around and reported up with AgentReportUp (Koplik Issues
    never reach RSI's queue).
- Standing rule: no committed artifact ever quotes the operator's personal data; refer to it as
  "the operator's Census contact (local config)". It lives only in the gitignored `~/koplik/.env.local`.
- main moves only to a QA-green SHA under an operator-approved release:
  `KOPLIK_PROMOTE_MAIN=1 git push origin <sha>:refs/heads/main` (origin's hook otherwise refuses).
  The 2026-10-07 approval ("Permission granted, whenever it's ready") covered the launch and its
  republication; ask the global manager before the next main promotion.

## Done this seat

Landed: #1421, #1361, #1359, #1450, #1451, #1452, #1453, #1458, #1427 (+ pipeline live_from_env
follow-up), #1462, #1447, #1455 (contracts v4), #1491, #1492, #1465 (contracts v5), README and final
report. Release steps (a)-(e) of #1449 done; forecast republished (QA `qa-2026-10-07-1b761f5.md`).
History scrub (operator option B) absorbed: every in-flight branch rebuilt with identical trees.
Product/science decisions recorded in Issues: #1439 (county series), #1455 (hypothetical introduction),
#1465 (no-measured-skill wording; backtest in its own section).

## Queue (no gates)

1. #1503: measure forecast skill on the published state series (pre-register first).
2. #1439 (a): cumulative-by-report Texas county series as its own contract version (tier2 review).
3. #1422: payload size (rt.json ~14 MB; provenance repeated per row).
4. Live refresh: a scheduled live `make pipeline` + republish (needs a global-manager ruling before
   any automated main promotion or publish).
5. Smaller: #1400 (verify closed by #1455's companion), #1420, #1424, #1434, #1445, #1457.

## Mechanics learned

- Land through `~/.rsi/koplik-mgr/integrate` (detached worktree of `~/koplik`,
  `CARGO_TARGET_DIR=~/.rsi/koplik-mgr/target`); after each push `git -C ~/koplik merge --ff-only
  origin/rolling`. Manager/worker sandboxes created before the scrub sit on the old history: never
  push from them; cherry-pick their own commits (or rebuild one commit with the identical tree).
- Reviews: launch the reviewer as a bound Issue session from the exact commit
  (AgentManagerLaunchIssueWorker with sandbox_source commit); DB-native request_review refuses across
  Epics and revised commits (#1364). Ask workers to copy their full SHA from `git rev-parse HEAD`
  (one invented a SHA).
- QA on arch-laptop (`ssh arch-laptop`, 32 cores): `~/koplik-qa/qa.sh <sha>` (clone of origin;
  unpushed commits via `git bundle` with a named ref); run `npm ci` in web/ first after a cleanup;
  delete `~/koplik-qa/target` and `web/node_modules` afterwards. The script's heredoc via ssh must not
  also redirect stdin.
- /tmp is a 31 GB tmpfs: tell workers and reviewers to keep CARGO_TARGET_DIR out of /tmp.
- A daemon deploy drain holds create_session for up to ~10 min while receipts say "queued".
- Live-site probe: `/tmp/kmgr3/probe/live.cjs` style Playwright script run on the laptop (the hub has
  no Playwright browser in its cache).

## Kaizen

Filed or evidenced: #1364 (evidence: out-of-scope author, source_changed), #1492 (fixed), RSI items
reported up (deploy-drain receipts, request_review limits, unverified RESULT SHAs, /tmp target dirs,
fable not allowed). Open: #1362, #1364, #1367, #1368, #1378, #1384, #1386, #1416, #1423, #1435, #1446, #1457.

Friction: #1364, #1492 | RSI items reported up in AgentReportUp (release report), not filable in Koplik.
