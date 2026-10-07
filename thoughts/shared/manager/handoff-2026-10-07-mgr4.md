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
