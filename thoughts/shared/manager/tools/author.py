#!/usr/bin/env python3
"""author.py <issue> <worker_session_id> <sha40> [note]: record implementation stage (authorship) on the work row."""
import json, subprocess, sys
n, sess, sha = sys.argv[1:4]
note = sys.argv[4] if len(sys.argv) > 4 else f"Worker {sess[:8]} reported RESULT {sha[:9]} green"
def rpc(verb, params):
    out = subprocess.run(["rsi-rpc", verb, "--params", json.dumps(params)], capture_output=True, text=True)
    return json.loads(out.stdout)
ver = None; cursor = None
while True:
    p = {"section": "work", "limit": 64}
    if cursor: p["cursor"] = cursor
    r = rpc("AgentManagerInspect", p)["result"]
    for row in r.get("rows", []):
        if row.get("key") == f"issue-{n}": ver = row.get("row_version")
    cursor = r.get("next_cursor")
    if not cursor or ver is not None: break
fence = {"scope_version": r["scope_version"], "policy_version": r["policy"]["row_version"]}
p = {"fence": fence, "idempotency_key": f"koplik-impl-{n}-{sha[:12]}", "change": {"update": "stage", "key": f"issue-{n}", "expected_row_version": ver, "stage": "implementation", "state": "passed", "note": note, "evidence": {"source_session_id": sess, "source_commit": sha, "artifact_path": "", "artifact_commit": sha, "closure_evidence_id": None}}}
out = subprocess.run(["rsi-rpc", "AgentManagerUpdate", "--params", json.dumps(p)], capture_output=True, text=True)
print(n, "ver", ver, "->", out.stdout.replace("\n", " ")[:400])
