#!/usr/bin/env python3
"""launch.py <issue> <provider> <model> <epic> <tier1|tier2> [note-file] [--key-suffix s]: launch one Issue-bound worker."""
import json, subprocess, sys
E = {"E1":"5ee2c462-154a-4434-aa0b-f8834d69e318","E2":"00abe984-189e-4655-9d9a-eb2bfc6cc98a","E3":"e6a97c97-9729-4918-966b-2fb44b117c9a","E4":"24c8aff1-921e-4897-9cba-9ee8928d3a8f","E5":"b5f6c80e-9da3-49f5-bfe9-4a33a8d11f37","E6":"bbe1ff42-d466-4f74-9c35-5e356b4300b6","E7":"51b52ab3-a41d-499d-87e9-95049e7726a0"}
args = [a for a in sys.argv[1:] if not a.startswith("--")]
suffix = next((a.split("=",1)[1] for a in sys.argv[1:] if a.startswith("--key-suffix=")), "w1")
src = next((a.split("=",1)[1] for a in sys.argv[1:] if a.startswith("--commit=")), None)
n, provider, model, epic, tier = args[:5]
note = open(args[5]).read().strip() if len(args) > 5 else ""
issue = json.loads(subprocess.run(["rsi-rpc","AgentGetIssue","--params",json.dumps({"display_number":int(n)})],capture_output=True,text=True).stdout)["result"]
title = issue["title"]
review = ("Review: this is a **tier2** change. Do not wait for review; the manager requests one reviewer pass from a different model family against your final SHA. Say in your RESULT lines what the reviewer should check first."
          if tier == "tier2" else
          "Review: tier1. It lands first and gets one post-land review.")
brief = f"""You are a short-lived Koplik worker bound to Issue #{n} ({title}). Read it first with AgentGetIssue {{"display_number": {n}}}: its intent and acceptance criteria are what you build to. The worker contract says the manager pastes the Issue text into your prompt; here the Issue in the tracker is canonical, so read it there.

Read before you start, in your sandbox (your current working directory, a checkout of origin/rolling):
- `AGENTS.md` (hard rules; it wins over everything else here)
- `thoughts/shared/manager/worker-contract.md` (scope, build, verify, commit, RESULT line)
- the spec sections the Issue names in `thoughts/shared/project/koplik-spec.md`
- the code you build on (`crates/koplik-contracts` v1 and anything the Issue names); follow its existing patterns.
{("\n" + note + "\n") if note else ""}
Rules that bite:
- Work and commit only in your sandbox, on its assigned branch. Never push. Never checkout, switch or reset your branch. No bare `git stash`. Stage explicit paths, never `git add -A`.
- If `origin/rolling` moves while you work, `git fetch origin && git merge origin/rolling` (never rebase) and re-run your tests.
- Every commit, including checkpoints, carries the trailer: `git commit -m "<subject>" --trailer "Rsi-Session: $RSI_SESSION_ID"` (if empty, use the id from your `rsi/<id>` branch name). Commit `wip(#{n}): <step>` checkpoints after each completed step.
- Run all cargo through `~/.rsi/bin/cargo-slot cargo ...`, in the foreground, and `tee` long runs to a log. Never report a run you did not see finish as green.
- Format only the files you changed: `git diff --name-only --diff-filter=d -- '*.rs' | xargs -r rustfmt --edition 2024`.
- Tests run offline against committed fixtures in `data/fixtures/`. Only `koplik-ingest` code touches the network. Fetching crates or npm packages for the build is fine.
- Data integrity, science honesty and determinism (AGENTS.md rules 3, 5, 6) outrank schedule. Never fabricate or hand-edit data; missing stays missing.
- Baton rule: once your context is compacted, or past about 60% of your window, commit (WIP is fine), append a handoff to Issue #{n} (AgentUpdateIssue with only `body` = the current body unchanged plus your handoff, and the row_version you just read), and end your turn with `PIPELINE HANDOFF — BATON <sha>`.
- File other defects you find with AgentCreateIssue; do not fix them here. Do not spawn children or arm wakes.

{review}

Finish: the first line of your final message is
`RESULT <full sha> issue=#{n} tests="<exact test command>" status=<green|red: reason|partial: reason>`
then at most ten lines: what changed, what the integrator must know (contract version, conflict risk, follow-up Issues filed, measured numbers, any licence that needs an operator decision). The last line is `Friction: none | #N | <one line, not filed because ...>`. Then end your turn."""
p = {"issue": int(n), "parent_epic_id": E[epic], "idempotency_key": f"koplik-launch-{n}-{suffix}", "launch": {"provider": provider, "model": model, "effort": "high"}, "brief": brief}
if src: p["sandbox_source"] = {"commit": src}
path = f"/tmp/koplik-mgr/launch-{n}-{suffix}.json"; json.dump(p, open(path, "w"))
out = subprocess.run(["rsi-rpc","AgentManagerLaunchIssueWorker","--params","@"+path],capture_output=True,text=True)
try:
    r = json.loads(out.stdout)["result"]; print(n, provider, model, "worker", r["worker_session_id"], "op", r["action"]["operation_id"], r["action"]["state"], "issue", r["issue_status"])
except Exception:
    print(n, "LAUNCH FAILED", out.stdout[:600], out.stderr[-300:])
