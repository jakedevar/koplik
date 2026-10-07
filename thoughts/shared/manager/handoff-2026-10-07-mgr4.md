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

## Next seat: first actions

1. Policy: the global manager re-seats Koplik with AgentManagerAppointChild; the new seat gets the global
   grant's policy with the full launch list (the empty `allowed_launches` below is fixed by that).
2. Launch #1623 (post-land review of #1612 at 36739f8 plus #1622 at 88c1e01; a non-Anthropic reviewer, since
   the author is Claude Sonnet and #1622 is Claude Opus). Codex re-authentication is with the operator: use
   Codex gpt-6.1-sol once it works, or OpenRouter z-ai/glm-5.3 (command below) if you do not want to wait.
   Before launching, add 88c1e01 (Sources/what-if narrow-screen fix) to #1623's body.
3. After Thursday's refresh (2026-10-08 21:00 America/Chicago): check `journalctl --user -u koplik-refresh.service`
   and `gh api repos/jakedevar/koplik/pages/builds/latest` (commit == gh-pages tip). The new navbar UI goes live then.
4. Otherwise the queue is the open follow-ups list in `handoff-2026-10-07-mgr3b.md` (#1542 first).

## Blocked (operator)

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
