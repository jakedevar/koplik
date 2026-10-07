#!/usr/bin/env python3
"""review.py <issue> <sha40> <base_sha> <provider> <model> [focus-file] [--key-suffix=s]: request one DB-native review."""
import json, subprocess, sys
args = [a for a in sys.argv[1:] if not a.startswith("--")]
suffix = next((a.split("=",1)[1] for a in sys.argv[1:] if a.startswith("--key-suffix=")), "r1")
n, sha, base, provider, model = args[:5]
focus = open(args[5]).read().strip() if len(args) > 5 else ""
def rpc(verb, params):
    out = subprocess.run(["rsi-rpc", verb, "--params", json.dumps(params)], capture_output=True, text=True)
    return json.loads(out.stdout)
# find the work row version
ver = None; cursor = None
while True:
    p = {"section": "work", "limit": 64}
    if cursor: p["cursor"] = cursor
    r = rpc("AgentManagerInspect", p)["result"]
    for row in r.get("rows", []):
        if row.get("key") == f"issue-{n}":
            ver = row.get("row_version")
    cursor = r.get("next_cursor")
    if not cursor or ver is not None: break
fence = {"scope_version": r["scope_version"], "policy_version": r["policy"]["row_version"]}
issue = rpc("AgentGetIssue", {"display_number": int(n)})["result"]
query = f"""Independent tier2 review of Koplik Issue #{n} ({issue['title']}) at source commit {sha}.
You are a read-only reviewer from a different model family than the author. Do not edit, commit or push.

Read: `AGENTS.md` (hard rules), `thoughts/shared/manager/worker-contract.md` section 4a (reviewers), the Issue (its intent and acceptance criteria are reproduced below), and the diff `git diff {base}..{sha}` (base = the rolling tip the work branched from). Build and run the touched crates' tests yourself (`~/.rsi/bin/cargo-slot cargo test -p <crate>`, foreground) and report what you saw finish.

Check, in order: (1) every acceptance criterion is met, and the tests assert the Issue's intent rather than the code as written; (2) AGENTS.md hard rules: data integrity (no fabricated, hand-edited or silently imputed data; missing stays missing; provenance on derived rows), science honesty (parameters configurable and cited; no tuning to a backtest; insufficient data below threshold), determinism and the spec's E4 portability rules where relevant (seeded ChaCha8, no usize in draws or hashed output, libm for transcendentals, ordered maps, no parallel float reductions), network only in koplik-ingest with offline tests, contracts versioning (released versions immutable, JSON Schema regenerated); (3) correctness bugs, edge cases (zero counts, missing weeks, empty inputs), units.
{("Focus for this change:\n" + focus + "\n") if focus else ""}
Verdict rules: `accepted` only if there is no blocking finding. `changes_requested` for any finding that must be fixed before merge (mark it blocking, severity error, with file:line and the concrete fix). Minor items are non-blocking warnings or info. Report only what you verified; if you could not verify something, say so in a finding rather than guessing.

Submit exactly one receipt with AgentSubmitReviewReceipt (your assignment id is in your launch context). Before your final message, make sure no process you started is still running (stop any build server you started, e.g. `sccache --stop-server` if present). Your final message starts with `REVIEW {sha} issue=#{n} verdict=<approve|changes>`, then the line `RESULT {sha} issue=#{n} tests="<exact test command you ran>" status=green` (daemon bookkeeping: it marks your turn as reported), then `- <blocker|major|minor> <file:line> <finding and concrete fix>` lines, then `Friction: none | <one line>`. End your turn right after it.

--- Issue #{n} body ---
{issue['body']}"""
p = {"fence": fence, "idempotency_key": f"koplik-review-{n}-{suffix}", "change": {"update": "request_review", "key": f"issue-{n}", "expected_row_version": ver, "source_commit": sha, "query": query, "launch": {"provider": provider, "model": model, "effort": "high"}}}
for extra in sys.argv[1:]:
    if extra.startswith("--delta-of="): p["change"]["delta_of"] = extra.split("=",1)[1]
    if extra.startswith("--findings="): p["change"]["finding_keys"] = extra.split("=",1)[1].split(",")
path = f"/tmp/koplik-mgr/review-{n}-{suffix}.json"; json.dump(p, open(path, "w"))
out = subprocess.run(["rsi-rpc","AgentManagerUpdate","--params","@"+path],capture_output=True,text=True)
print(n, "work_ver", ver, "fence", fence, "->", out.stdout[:500].replace("\n"," "), out.stderr[-200:] if "error" in out.stdout else "")
