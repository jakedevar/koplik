# Koplik manager handoff: 2026-10-07 ~19:20Z (manager 4)

From: project manager `bd69dacd-399e-4684-a733-97fb428900d3` (Claude claude-opus-5-5). Directives, chain of
authority and mechanics: `handoff-2026-10-07-mgr3.md` and `handoff-2026-10-07-mgr3b.md` (all still hold).

## Done this seat

- Operator request (2026-10-07): data-source disclaimers off the main page, navbar pages, colour.
  #1612 (worker 3f1f276c, Claude Sonnet 5.5) landed at 36739f8: hash-routed pages Explorer / Forecast /
  What-if / Sources; full attribution on Sources only; every page footer keeps the verbatim disclaimer and
  CDC / Census / Texas DSHS links plus a Sources link; palette via CSS custom properties, AA contrast tested.
  Manager decision: separate Sources page rather than a collapsible bottom section (the footer keeps the
  required DSHS attribution and link on every page); the operator's request overrides the spec's E6
  "one screen" line.
- #1622 (manager-authored, launches blocked) landed at 88c1e01: Census URLs/hashes wrap on Sources; the
  what-if parameter-citation table scrolls instead of being clipped at 390 px; Playwright now checks every
  page for clipped content at 390 px.
- Both gated with `make check` and `TMPDIR=/tmp make web-test` (166 vitest passed, 2 skipped; 4 Playwright).
  Thursday's refresh (2026-10-08 21:00 America/Chicago) republishes rolling's tip if its QA is green.

## Update ~20:30Z: review done, rework landed

- Operator instruction (Codex outage): use Claude models instead. Review #1623 ran on Claude Opus 5.5
  (02ee9fb8), not a different family (noted in the verdict): VERDICT changes, 1 blocking (Sources method note
  claimed the what-if derives from reported counts and that every number traces to a snapshot) + 3 minors.
- Rework #1625 (Claude Sonnet 5.5, 68f8d70f) landed at merge 1770c0b: honest method note with a unit test,
  forecast fallback line, DSHS school-coverage footer link, drawer closes on page change.
  `TMPDIR=/tmp make web-test` green on the merge (171 vitest passed, 2 skipped; 4 Playwright).
- Nothing in flight. Next seat: after Thursday's refresh (2026-10-08 21:00 America/Chicago) check
  `journalctl --user -u koplik-refresh.service` and `gh api repos/jakedevar/koplik/pages/builds/latest`
  (commit == gh-pages tip); the new navbar UI goes live then. Then the open follow-ups in
  `handoff-2026-10-07-mgr3b.md` (#1542 first).

## Update ~00:00Z 2026-10-08: polish pass live

- Global manager ruling (2026-10-07): visual polish pass + cross-family screenshot design review; promote and
  republish after each QA-green landing under the standing Koplik release approval (no need to ask while QA is green;
  conditions: full QA on the exact SHA in qa-green.sha, personal-data/secrets scans clean, `make pages-verify`,
  Chromium check of the live site; report up with the live URL).
- Republish 1: main a902d47 -> 115856e, gh-pages 3317ea4 (QA `qa-2026-10-07-115856e.md`).
- #1652 polish (Claude Sonnet 5.5, b65f8df0) landed ca36db3; design review #1658 (Codex gpt-6.1-sol, 63f855b3):
  changes (blocking: latest-week summary lacked its provisional caveat). Rework #1662 (Claude Sonnet 5.5, b6a9dc41,
  folds #1660, #1661) landed 0335cf1.
- Republish 2: main 115856e -> 0335cf1, gh-pages 3003630 (QA `qa-2026-10-07-0335cf1.md`; data tree unchanged
  since 4a8c1bc); `make pages-verify` "Pages built 3003630"; Chromium at 1280 and 390 px: four pages, disclaimer,
  no horizontal scroll, no alerts, 0 page errors, provisional caveat shown.
- Open from this work: #1659 (R_t chart axis dominated by one Texas 2025 outlier near W40; needs a design choice that
  keeps the full interval and provenance), #1667 (lazy chunk Retry needs a reload in Chrome).
- Thursday's refresh (2026-10-08 21:00 America/Chicago) builds on rolling's tip as usual.

## Update ~01:00Z 2026-10-08: follow-ups live; R_t gate in preregistration

- Live: main b61563b, gh-pages b03703a (QA `qa-2026-10-08-b61563b.md`; data unchanged). Earlier today: 9a6430d / 6bcd59b.
  Landed and live: #1659 R_t readable range + Show full range (Codex sol), #1667 Reload page after repeated lazy
  import failure (Codex luna; first attempt rejected: unhashed chunk names break caching across deploys), #1673
  formatted off-scale labels, #1676 review #1670 minors. Review #1670 (Claude Sonnet) approved #1659/#1667.
- R_t science thread (Texas 2025 W40 mean 47.3 published as ok): #1672 trace (no bug; documented 11-case rule
  passes it, window Lambda 0.39) -> #1677 proposal (Lambda floor 1.0) -> #1678 cross-family scientific review
  (Claude): ACCEPT WITH REVISION R1-R5 (`thoughts/shared/research/1678-rt-infectiousness-gate-review.md`) ->
  #1679 preregistration landed at 9deba0c (`thoughts/shared/research/1677-rt-gate-preregistration.md`, before
  any simulator) -> #1683 conformance check (Claude Sonnet 5b4b5161) running.
  Next: on CONFORMS, step 2 = simulator + results against the prereg (Codex); REVISE = land the edits first.
  Step 3 only if the prereg criteria pass: implement (config field, new reason, contract v9, forecast/backtest
  scope), tier2 cross-family review. Step 4: full before/after audit (R5). A failed criterion means REJECT, not
  a new threshold. The site shows W40 meanwhile, marked off scale with exact values one click away.
- Mechanics: write Issue bodies with a quoted heredoc + python (`<<'EOF'`); twice a double-quoted shell string
  ate backtick spans in Issue text (#1662, #1678 fixed after the fact).
- Kaizen: #1682 (worker-filed: publish-test temp paths exceed sccache socket path length in deep sandboxes).

## Earlier blockers (resolved: launches work again; Codex re-auth with the operator)

- Manager policy `allowed_launches` is `[]` after the policy revocation/re-save: launches refused
  `manager_v2_launch_not_granted`. Operator: re-save `:manager policy` with the allowed launches.
- Codex OAuth token invalidated (401): Codex workers b006a6f9 (#1623) and d1d3503a (#1622) died at start.
- Post-land review #1623 of #1612 (non-Anthropic reviewer, from commit 36739f8) waits on either fix;
  relaunch: `launch.py 1623 OpenRouter z-ai/glm-5.3 E6 tier1 /tmp/koplik-mgr/note-1623.txt --commit=36739f8e65aeabe010570d16be4e133a78d2fa88 --key-suffix=w2`.
  Also have it cover 88c1e01.

## Kaizen

Reported up (harness filing refused `agent_create_issue_harness_project_unresolved`): lowercase provider
refused as bare `invalid_input`; harness filing unresolved. Tools README notes provider case.
Open Koplik kaizen unchanged from mgr3b.

Friction: none filed in Koplik | RSI items reported up (harness:true filing unresolved for Koplik).
