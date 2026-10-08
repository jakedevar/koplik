# Issue #1672: Texas 2025 W40 R_t investigation

Investigation base: `32a71680fc5b98024d1c8d5fe1eaec26b57962a0` (rolling).
Published branch: `6bcd59b6edcc06a80b91f7f3108110b3625e62e3` (gh-pages).
No method, contract, source snapshots, or released data were changed.

## Published artifact → counts → immutable snapshot

SHA256 of published `data/v6/rt.json`:
`4f521f930d7d524ede8dc39619d938d5205c5b943394599a4eda98da2642f29a`.
SHA256 of published `data/v6/weekly-cases.json`:
`6dffa4cfd99c41639be579b88075c0f40afda0ede3d626e57937507606ae5583`.
Both are contracts v6 artifact envelopes; Texas is geography `48`, using
`confirmed_or_unknown_status` weekly case rows. Their Texas provenance index 0
resolves to source `cdc-nndss-weekly-measles`, snapshot SHA256
`c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`,
retrieved `2026-10-07T09:55:29Z`. This same hash also has an earlier retrieval
at `2026-10-07T02:19:30Z` in `data/release/retrievals.jsonl`; the published
artifacts use the later retrieval. The released blob was independently hashed
and matched its content-addressed filename.

Source URL (exactly as recorded):
<https://data.cdc.gov/resource/x9gk-5huc.json?$select=states,year,week,label,m3,m3_flag&$where=label%20in%28%27Measles,%20Indigenous%27,%27Measles,%20Imported%27%29%20AND%20year%20in%28%272025%27,%272026%27%29&$order=year,week,states,label&$limit=50000>.
Licence marker: `cdc-open-data-terms-unconfirmed` (existing input; no new
licence acceptance or acquisition).

`koplik-ingest/src/cdc.rs` defines weekly cases as differences of summed
Indigenous + Imported year-to-date `m3` counts. These are new reports, not an
onset-date epidemic curve. Raw snapshot rows show Imported = 16 throughout
W31–W42; Indigenous is 742, 742, 743, 770 in W37–W40. Therefore W40's
27 = (770 + 16) - (743 + 16). There is no import-category increase in this
particular jump. The snapshot does not establish whether these reports were
on-time, delayed, or batched; no such cause is inferred as fact.

## Exact Cori-window trace

`koplik-pipeline::rt_config()` returns `RtConfig::default()`. Its renewal
window is **one weekly step** (W40 alone), distinct from the **eight-week
infectiousness look-back** (W32–W39). Gamma serial interval mean 11.7 days,
SD 3.0 days; same-week mass and the tail beyond eight weeks are removed and
weights renormalized. Measured dropped mass: 0.00445756531381192.
Prior is Gamma(shape 1, scale 5); minimum cases = 11; credible levels = 0.5
and 0.95; history before the series is unknown.

| Lag | Input week | New reports | Serial weight | Infectiousness contribution |
| --- | --- | ---: | ---: | ---: |
| 1 | W39 | 1 | 0.38337901353367709 | 0.38337901353367709 |
| 2 | W38 | 0 | 0.55526490461224698 | 0 |
| 3 | W37 | 0 | 0.06042499631088016 | 0 |
| 4 | W36 | 9 | 0.00092681540427970 | 0.00834133863851733 |
| 5 | W35 | 8 | 0.00000426089753497 | 0.00003408718027972 |
| 6 | W34 | 1 | 0.00000000922930283 | 0.00000000922930283 |
| 7 | W33 | 1 | 0.00000000001206704 | 0.00000000001206704 |
| 8 | W32 | 3 | 0.00000000000001122 | 0.00000000000003365 |

All eight counts are known; sum Lambda = **0.39175444859387765**.
Sum I in the window = **27**. Posterior =
Gamma(shape `1 + 27 = 28`, scale `1 / (0.2 + Lambda) = 1.6898901265147936`).
Posterior mean = **47.31692354241422**; CV = **0.18898223650461360**.
Published 50% interval = **[41.018707295846774, 53.002913122355565]**;
95% interval = **[31.441752064675583, 66.38493810820933]**.

The status is `ok`: 27 >= 11 and Lambda > 0. The implementation's
`NoInfectivity` condition is zero/nonpositive or NaN infectiousness, not a
small-positive-exposure threshold. The minimum-count criterion correctly
applies to the numerator (window incidence), not Lambda. W40 is not
provisional: the Texas grid extends to 2026 W38, and only its last two weeks
are provisional. Provisional flags are independent of sufficiency status.

W9 also follows the rule: 21 cases, Lambda = 4.98392717593780255, posterior
Gamma(shape 22, scale 0.19290394445386747), mean 4.243886777985084,
95% interval [2.659621249354341, 6.19235757862207]. An off-scale chart interval
alone is not evidence that the estimator misapplied its adequacy rule.

## Method decision

The code implements the documented rule correctly. **Under that rule W40
should not have been marked insufficient.** This conclusion concerns the
implementation, not biological plausibility of the reporting-series estimate.

[Cori et al. 2013](https://pmc.ncbi.nlm.nih.gov/articles/PMC3816335/)
defines infectiousness by weighted previous incidence and estimates a constant
R over a chosen window. Its limitations include assumptions of stable
reporting and attribution of incident cases to earlier observed cases.
[EpiEstim estimate_R.R](https://github.com/mrc-ide/EpiEstim/blob/master/R/estimate_R.R)
computes shape as prior shape plus window local incidence, and posterior
scale as the inverse of prior rate plus window infectiousness. Its desired-CV
case threshold is `ceil(1 / cv^2 - a_prior)`; Koplik strengthens this warning
into a hard gate for every window. With a fixed gamma posterior,
CV = `1 / sqrt(a + sum I)` regardless of Lambda. It controls relative
posterior precision conditional on the model, not the validity of the
transmission/reporting assumptions. The current min-case gate therefore
cannot prevent a high ratio after near-zero incidence. Applying 11 to Lambda
instead, capping R at a typical R0, or extending only this point's window
would change the method; none is an implementation correction.

Follow-up **#1677**, “Pre-register an infectiousness adequacy gate for weekly
Cori R_t”, records a candidate configurable floor of sum Lambda >= 1.0. This
is an explicit project policy proposal, not a Cori-derived scientific default;
it requires independent scientific review and independently specified
simulation validation before implementation or a full affected-estimate audit.
It freezes the candidate rather than optimizing against this point or the
backtest. No method change is made in #1672.
Before/after affected estimates for this investigation: **empty list**.

## Verification

`tools/cargo-test.sh -p koplik-epi` passed the offline namespace gate and
all 78 crate tests (31 unit + 47 integration; 0 doctests). It runs every cargo
command through `~/.rsi/bin/cargo-slot` and uses this sandbox's `target/`.

A disposable diagnostic under ignored `target/issue-1672/` expands the
published v6 provenance references, deserializes all weekly rows, calls
`estimate_weekly` with the unchanged default configuration, and compares
all **13,688 published R_t rows**. Geography/week/level/status/provisional
flags, numeric presence, and provenance match; maximum absolute numeric
round-trip difference is **2.84217094304040074e-14**. It also calls
`estimate_series` and independently sums the SI-weighted counts to print
the table and posterior above. Execution was isolated with
`tools/offline-test.sh target/issue-1672/trace`; no external network was used.
This is a live-release investigation, not a new fixture-based test.
`make check` completed successfully: governed dependency fetch followed by
an isolated offline workspace/all-targets check. Logs:
`/tmp/issue-1672-epi-tests.log`, `/tmp/issue-1672-check.log`, and
`/tmp/issue-1672-trace.log` (local, not committed).

An independent direct reading of the immutable raw snapshot also verified all
**91** reported Texas weekly counts against the published artifact using the
documented cumulative-difference rule. Release metadata hashes:
`ingest.manifest.json` =
`b44233fb045ee2124e7169b0fd75ef9e2ad89672ca9f4695ddb6990afc777d25`;
`retrievals.jsonl` =
`1a072055ba922aced652b6daaa1810d32e93a26eaaf730ca9bc0aba13d08e017`.

Only this investigation document changes in the repository. No new test was
needed for a documentation-only outcome; existing method tests cover the
case threshold, zero-exposure guard, missing history, provisional flags and
agreement with an offline EpiEstim worked-example fixture. Pipeline tests and
native/wasm determinism were not rerun because no pipeline or engine source
changed. No baseline test failures were encountered.

For reproduction, read the two artifacts with
`git show 6bcd59b6edcc06a80b91f7f3108110b3625e62e3:data/v6/rt.json` and
`git show 6bcd59b6edcc06a80b91f7f3108110b3625e62e3:data/v6/weekly-cases.json`;
expand each row's provenance indices against its envelope dictionary,
deserialize the weekly rows as contracts v3, and run
`koplik_epi::rt::estimate_weekly(rows, &RtConfig::default())`. For the table,
discretize the configured serial interval and sum `w[k] * I[t-k]` for k=1..8.
Do not run ingest or replace release inputs. The final fetch found
origin/rolling still at the investigation base.

## Integrator handoff

Tier2 reviewer should first check that the 11-case CV rule applies to sum I,
while the existing infectiousness guard only excludes zero/nonpositive/NaN
Lambda; then check snapshot lineage, one-week versus eight-week windows, and
the distinction between new reports and onset incidence. #1677's floor is a
separate unapproved policy proposal. There is no contract version change or
public push. Conflict risk is limited to this new investigation file.

Friction: none
