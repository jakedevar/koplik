# Koplik manager handoff: 2026-10-07 ~10:45Z (manager 3, second handoff)

From: project manager `8672df09-6f27-49c3-b67c-d2d8dc9e5317` (Claude claude-opus-5-5). Supersedes
`handoff-2026-10-07-mgr3.md` for live state (read that one for directives, mechanics and history;
everything in its "Chain of authority and directives" section still holds). Context ~57%.

**Directive updates (global manager, 2026-10-07 ~10:50Z), overriding the first handoff:**
- The hand `uptime` check is retired: the RSI daemon enforces host-load admission (manager creates are
  held while the 1-minute load is above 40, released fairly; deploy f064bccd0, RSI #1417). The
  per-project cap of about 5 concurrent build-heavy Koplik sessions still applies.
- File RSI harness defects with `AgentCreateIssue {harness: true}` (goes straight into the Rsi project,
  RSI #1389) instead of mailing kaizen to the global manager.

**Live:** https://jakedevar.github.io/koplik/ serving the first live weekly refresh (main = rolling =
a902d47, gh-pages = 4a8c1bc; Pages build triggered by hand, see #1537).

## Global manager direction (2026-10-07, after launch)

Work the queue at a modest pace (at most 3 concurrent sessions): #1422, #1503, #1439 (all DONE), and
a scheduled weekly live refresh (APPROVED under the operator's standing Koplik release approval):
weekly after CDC's Thursday update; data-only commit (only `data/release/**`); full QA + personal-data
and secrets scans; only if all green, fast-forward main and publish; on any failure publish nothing and
file an Issue. Code changes still go through normal landing; main promotion + republish are covered by
the same approval while QA is green. **When the queue is empty, hand off and stop; the global manager
leaves the seat vacant until there is new work.**

## Done since the first handoff

- #1422 (contracts v6 row artifacts; payload 20.9 MB -> 4.4 MB), #1491, #1492.
- #1503 (contracts v7): pre-registered pseudo-real-time backtest on the CDC state series; measured 90%
  coverage 39.0% (682/1748), worse than persistence; publication rule (written after the first result,
  before implementation): a series is published only with >= 40 targets from >= 10 origin weeks, 90%
  coverage >= 0.75 and mean CRPS <= persistence. Today no state forecast is published; the page says so.
  Also #1513 fixed. #1515 (bounded-growth variant) and #1514 (dated NNDSS vintages) are the routes back.
- #1439 (contracts v8): Texas DSHS cumulative-by-report county series, printed counts only (138 printed,
  204 missing with reasons); #1523 (legacy derivation still infers zeros; nothing published uses it).
- #1507 (+#1509 atomic release, #1516 live-mode tests): weekly refresh. Installed:
  `koplik-refresh.timer` (Thu 21:00 America/Chicago, Persistent=true; next 2026-10-08), launcher in
  `~/.local/lib/koplik-refresh/`, state in `~/.rsi/koplik-refresh/`, private scan patterns in
  `~/.config/koplik/pii-patterns` (mode 600; never print or commit), Census contact in the gitignored
  `~/koplik/.env.local`. **First live run by hand: GREEN** (run 2026-10-07-ba24944d, 24 min: ingest 19
  min, QA 5 min); data-only commit a902d47 (131 files under data/release only), one atomic push of
  rolling/main/gh-pages, 0 personal-pattern hits in the full history of rolling and gh-pages.
  Linger is off (`loginctl enable-linger` is the operator's call): a missed run fires at next login.

## In flight

| Item | Session | Next |
|---|---|---|
| #1537 refresh must request + verify the GitHub Pages build (Pages did not rebuild after the first refresh's atomic push; built by hand with `gh api -X POST repos/jakedevar/koplik/pages/builds`), plus #1528 minors | worker 76429b0a (Codex gpt-6.1-sol) | Claude tier2 review, land via ~/.rsi/koplik-mgr/integrate BEFORE Thu 2026-10-08 21:00 America/Chicago. If it cannot land in time, after Thursday's run check `gh api repos/jakedevar/koplik/pages/builds/latest` and trigger the build by hand. |

## Open follow-ups (no gates)

#1512 (v6 web test coverage), #1517 (late tsc in make web-test), #1523 (legacy inferred zeros),
#1514, #1515, #1400, #1420, #1424, #1434, #1445, #1457, kaizen #1362, #1364, #1367, #1368, #1378, #1384,
#1386, #1416, #1423, #1435, #1446.

## Mechanics added

- Contract numbers collide when two changes add versions in parallel: land one, have the other's author
  renumber on top of it (v(n+1) re-exports the newly landed v(n)), and diff the approved content modulo
  the rename before landing.
- Headless Chrome on the hub: run probes with `TMPDIR=/tmp` (socket path length) and
  `executablePath: '/opt/google/chrome/chrome'`.
- A daemon deploy drain can refuse AgentContinueChild/launch with `deploy_draining`; retry after
  `AgentGetDaemonInfo.deploy_drain.release_by`.

Friction: #1537 | GitHub Pages not rebuilding after a mirrored atomic push (filed in Koplik; product-side).
