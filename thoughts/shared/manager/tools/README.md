# Manager helper scripts (Koplik)

Thin wrappers over `rsi-rpc` used by the root manager. Run with `python3 -I`. They write
their request JSON under `/tmp/koplik-mgr/` (create it first) and print one line per call.

- `launch.py <issue> <Provider> <model> <epic E1..E7> <tier1|tier2> [note-file] [--key-suffix=w2] [--commit=<sha40>]`:
  Provider names are case-sensitive (`Claude`, `Codex`, `OpenRouter`); lowercase is refused as bare `invalid_input`.
  `AgentManagerLaunchIssueWorker` with the standard worker brief (worker contract, hard rules,
  baton rule, RESULT and Friction lines). Epic ids are hard-coded for this project.
- `author.py <issue> <worker-session> <sha40> [note]`: records the implementation stage with
  evidence on work row `issue-<n>`; `request_review` is refused `manager_review_author_missing`
  without it (kaizen #1364).
- `review.py <issue> <sha40> <base> <provider> <model> [focus-file] [--key-suffix=r2] [--delta-of=<assignment>] [--findings=k1,k2]`:
  DB-native `request_review`. The prompt asks reviewers for a RESULT line too, so the
  no-result guard does not re-invoke them (#1362).

Read finding text (read-only):
`sqlite3 -readonly ~/.rsi/rsi.db "SELECT f.finding_key,f.severity,f.blocking,f.location,f.summary FROM manager_review_findings f JOIN manager_review_receipts r ON r.receipt_id=f.receipt_id WHERE r.assignment_id='<id>';"`
