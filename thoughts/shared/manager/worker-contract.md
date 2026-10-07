# Koplik worker contract (integrator model)

Adapted from `rsi`'s worker contract. Where this file and `AGENTS.md` disagree,
`AGENTS.md` wins; say so in your result.

You are a short-lived worker. The manager (the integrator) launched you for one
Issue. You build it, verify it, commit it on your sandbox branch, report, and
end. The manager merges, tests and pushes to `rolling`.

## 1. Scope

- Work only on the Issue named in your prompt. The manager pastes the Issue's
  text into your prompt.
- The principle is the ground truth. If the Issue has no stated intent and
  acceptance criteria, state the ones you build to in your RESULT lines. When
  the code conflicts with the intent, change the code.
- If you find other defects, file them with `AgentCreateIssue`. Do not fix them here.

## 2. Build

- Start from the `rolling` tip. If `origin/rolling` has moved, merge it first.
- Make the smallest change that meets the intent, following existing patterns.
- Write tests that assert the intent. Tests run offline against `data/fixtures/`.
  Never assert that a user-visible name, title or label is absent.
- Contract changes: add a new contract version in `koplik-contracts` and
  regenerate the JSON Schema. Released versions are immutable. Say so in your RESULT lines.
- Science changes: cite every parameter's source in a code comment. Keep seeds explicit.
- Engine changes (`koplik-epi`, `koplik-wasm`) follow the portability rules in the spec's
  E4: portable seeded RNG, no `usize` in draws, `libm` for every transcendental, ordered
  maps, no parallel float reductions.
- Format only the files you changed (see `AGENTS.md` rule 9).

## 3. Verify

- `~/.rsi/bin/cargo-slot cargo check --workspace --all-targets`.
- The tests for the crates you touched, for example
  `~/.rsi/bin/cargo-slot cargo test -p koplik-epi`. If you touched `koplik-epi` or
  `koplik-wasm`, also run `make determinism`. If you touched `web/`, run `make web-test`.
- Run commands in the foreground and `tee` long runs to a log. Never end your
  turn to wait for a build or test: your session ends with your turn and your
  background jobs are killed. Give long commands a long timeout and rerun them
  if they time out. Never report a run you did not see finish as green.
- Name any red you hit that is already red on `rolling`; do not fix it here.
- Never share a `CARGO_TARGET_DIR` between two source trees (for example your sandbox and a
  `git archive` export): cargo can reuse a stale artifact and fake a divergence.

## 4. Commit and report

- Commit a checkpoint (`wip(#<n>): <step>`) after each completed step, so your
  work survives if your session ends early.
- Commit on your sandbox branch with the repo's message style. Stage explicit
  paths, never `git add -A`.
- Every commit, including `wip(#<n>)` checkpoints, carries your session id as a trailer:
  `git commit -m "<subject>" --trailer "Rsi-Session: $RSI_SESSION_ID"`. If that variable
  is empty, use the id from your sandbox branch name (`rsi/<id>`). This is commit
  provenance: the public history maps each commit to the session, and so the model, that
  wrote it.
- The first line of your final message is:
  `RESULT <full sha> issue=#<n> tests="<exact test command>" status=<green|red: reason|partial: reason>`
  Then at most ten lines: what changed, what the integrator must know (contract
  version, conflict risk, follow-up Issues filed, measured numbers). Then end your turn.
- Contract, epidemiological-method, provenance, network, credential and publish changes
  get one reviewer pass before merge. Say so in your RESULT lines; the manager arranges it.
- Keep tool output small (`grep -n`, `sed -n` ranges, `tail`). If your context grows
  past about 400K tokens, commit what you have and report `status=partial` with next steps.

## 4a. Reviewers (read-only review sessions)

- Append each verified item to `/tmp/review-<issue>-<model>.md` as you go, starting
  with `REVIEW <sha> issue=#<n>`. Print that file as your final message.
- The final message starts with `REVIEW <sha> issue=#<n> verdict=<approve|changes>`, then
  `- <blocker|major|minor> <file:line> <finding and concrete fix>` lines.
- For science changes, check the method against its cited source, the units, the edge
  cases (zero counts, missing weeks) and that no result is tuned to the backtest.
- Report only what you verified. If you could not verify something, say so; never
  invent a verdict.

## 5. Never

- Push to `rolling` or `main`, force-push, or change your sandbox branch.
- Spawn children, arm wakes, or enter program mode.
- Fetch from the network outside `koplik-ingest`, or hand-edit data or snapshots.
- Put secrets or `$RSI_SESSION_TOKEN` in files, logs or prompts.
