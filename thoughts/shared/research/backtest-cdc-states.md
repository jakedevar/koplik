---
date: 2026-10-07
issue: 1503
topic: backtest of the 4-8 week forecast on the CDC NNDSS state series the site publishes
status: protocol pre-registered at commit 99785dc (before any score on the NNDSS series was computed; amendment 1 at 4b348c9, also before any score); results measured at the run recorded under Results
---

# Forecast backtest: CDC NNDSS state series

This file is committed in stages. The first commit (99785dc) holds only the protocol below,
written before the backtest code existed and before any score on these series was computed.
The results section was added after the run, exactly as measured. Nothing in the protocol is changed after the
first commit; if something has to change it is added as a dated amendment with its reason,
never edited in place.

## Why

#1361 scored the forecaster on the Texas DSHS 2025 outbreak total (confirmed cases, by report
date, real-time by report vintage). The site publishes forecasts of a different thing: the CDC
NNDSS weekly measles series per state (`confirmed_or_unknown_status`, differenced from the
published cumulative). That series, and the origin rule the site uses (latest week less two
provisional weeks), were never scored. This backtest scores exactly the published method, on
exactly the published series, with exactly the published origin rule. It does not change the
method or any default (`koplik_epi::forecast::ForecastConfig::default()`, seed 20250101).

## Data (fixed)

- **The committed real-byte snapshot** `data/fixtures/cdc/nndss-measles-weekly.json`
  (SHA-256 `c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`, retrieved
  2026-10-07T02:19:30Z; 2025 weeks 1-53 and 2026 weeks 1-38), read by
  `koplik_ingest::cdc::parse_weekly_cases`: the same function and the same rows the pipeline's
  validate stage publishes as `weekly-cases`. Nothing is trimmed, edited or imputed. Offline.
- There is **one** retrieval of this source. CDC publishes no revision history, so a
  real-time-by-vintage backtest (as #1361 did for DSHS) is **not possible** yet. This backtest
  is therefore **pseudo-real-time (revised counts truncated at each forecast date)**: at each
  forecast date the forecaster sees the rows of the one retrieved series up to the origin week,
  and nothing after (`forecast_weekly` discards later rows first). It is never to be described
  as real-time.
- What that may hide: the dataset's own notes call its counts provisional and "presented as
  published each week" (SOURCES.md); if CDC rewrote earlier weeks' published cumulative when it
  republished, the series we hold carries those later revisions, so a forecaster at the time
  saw different (probably less complete) counts than this backtest feeds the method. We hold
  one retrieval and cannot measure that. The score may therefore be somewhat optimistic about
  input quality. A real-time backtest needs successive retrievals (follow-up filed in the
  report).
- The count is by **CDC report week** (the week-over-week growth of the published year-to-date
  cumulative), not by symptom onset; counts of cases that CDC added to the cumulative later are
  in the later week.

## Which series (fixed rule, not chosen from any score)

**Every geography in that series**: the 50 states, DC and the 5 territories (56 geographies,
each under its own `confirmed_or_unknown_status` definition). No geography is selected or
dropped. A geography contributes a forecast at an origin only where the method's own
minimum-count rule holds at that origin (every count in the 3-week window and the 3-week
look-back known, at least 11 cases in the window, some infectivity: the same
`insufficient_data` rule the site applies); elsewhere it contributes nothing, as on the site.
Texas DSHS county series (`confirmed`) are out of scope: they are a different source and
definition and stay "not backtested".

## Forecast dates, origins and information cutoff (fixed)

- A forecast is made for every MMWR week `L` from the first week of the series to its last week
  at which the data through `L` is held: "the latest published week is `L`".
- **Origin rule = the published rule**: origin week `O = L - 2` (the latest week less the two
  provisional weeks of spec E5). This is what is scored (`origin_of` in the forecast stage),
  including the dropping of the two provisional weeks. Because `L` only fixes `O`, the forecast
  for `O` is made once, from the rows with week at most `O`.
- **Information cutoff**: the forecaster is given only rows with week at most `O` (the
  retrieved, revised values; see above), and the same `forecast_weekly`, `ForecastConfig::default()`
  (window 3, look-back 3, minimum 11 cases, 8 weeks ahead, 1,000 members, the 23 hub
  quantile levels) and seed 20250101 as the site. Nothing is re-run or re-seeded after seeing a
  score.
- Origins `O` run from the first week to the last week `O` with `O + 2` in the series, i.e.
  `L` never lies beyond the retrieved data.

## What is scored (fixed)

- **Targets**: horizons `h = 1..8`, target week `O + h`.
- **Truth**: the count of the target week in the retrieved series. A target is **scored** only
  if that count is `reported` (a `missing` week, including a cumulative that fell, is not scored,
  never zero) **and** the target week is at most the last week of the series less 2: the two
  newest weeks of the retrieved series are still provisional (spec E5) and are not used as
  truth.
- **Scores per scored target**, as in #1361 (`koplik_epi::backtest`): the CRPS of the member
  ensemble (Gneiting & Raftery 2007, eq. 21, the plain ensemble estimator, in cases); whether
  the central 50% and 90% intervals of the published quantiles (0.25-0.75, 0.05-0.95, bounds
  inclusive) contain the truth; the absolute error of the **persistence baseline** (the origin
  week's count carried forward; its CRPS is its absolute error).
- **Summaries**: plain means over scored targets, per horizon 1..8 and pooled, each with `n`;
  per series (all horizons pooled, and per horizon in the JSON); and over all series pooled
  and per horizon. Nothing is dropped, weighted, trimmed or re-run on a score. The ratio
  "mean CRPS / mean persistence absolute error" is reported beside them (below 1: the forecast's
  mean error is smaller than carrying the last count forward). CRPS is in cases, so pooled over
  series it is dominated by the series with the largest counts; the per-series rows say which.

## What counts as "measured skill" (fixed)

- **A series has a measured skill** only if the backtest scored at least **40 targets** for it
  **and** those come from at least **10 distinct origin weeks**. 40 is about the size of the
  #1361 backtest (48), which that report already calls too small to rank methods; the 10-origin
  floor is there because the 8 horizons of one origin are not independent evidence, so the
  origins, not the targets, are the unit that matters. These numbers were chosen from this
  reasoning, not from any count or score of these series.
- A series below either floor is published as **"insufficient data for a measured skill"**
  with its own `n` and number of origins, whatever its scores would have been. No pooled or
  other series' number is attached to it.
- The **pooled** NNDSS result is a statement about the forecasts the method made on these
  series where its minimum-count rule held, dominated by the largest. It is shown as
  its own evaluation, labelled pooled, and is never presented as the skill of any one series.
  It is "measured" under the same floors, applied to scored targets and to distinct
  (series, origin) forecasts.
- "Measured" is not "good". The page states the numbers, the persistence comparison and the
  coverage against nominal as measured, with the scope above; it does not say a series
  "is well forecast".

## Pre-registered expectations (not tested against, not tuned to)

Poisson offspring without overdispersion (the method's known limitation, #1361) leads to the
expectation that coverage falls short of nominal, as it did for West Texas. State series that
are mostly zero make the persistence baseline hard to beat and the CRPS small in cases. Either
way the number reported is the number measured.

## Outputs (fixed)

- Code: `koplik_epi::backtest` (pseudo-real-time runner over weekly rows), a pipeline example
  that parses the committed snapshot and writes the report.
- `data/reports/backtest/cdc-states.json` (every scored target, every origin, every series,
  the input hash) and the results section of this file.
- The published forecast's provenance companion links each NNDSS state series to its measured
  skill, or says it has none, and the page shows the numbers in plain words, separately from
  the West Texas evaluation.

## Amendments (dated; the protocol above is never edited in place)

### Amendment 1 (2026-10-07, before any score on these series was computed): refused projections

The first run of the backtest stopped with `ForecastError::Overflow` ("projected mean ... exceeds
1099511627776") at one origin of one series, **before producing any output**: no score of any kind
had been seen. The protocol above did not say what a backtest does when the method refuses to
publish. The rule, fixed now and not from a score:

- The method refuses a projection whose mean passes 2^40 (`MAX_PROJECTED_MEAN`: "an error, never a
  clipped number"). That happens when a burst of cases follows weeks with almost none, so the
  look-back infectivity is tiny and the posterior for `R` is in the hundreds. At such an origin the
  method makes **no forecast**; the origin is counted under the reason `projection_overflow`,
  listed in the report per series, never scored and never turned into a number.
- Consequence for reading the result, stated in the report: the scores are **conditional on the
  method making a forecast**. The refused origins are the ones where the method would have been
  furthest off, so leaving them out flatters the scored set; the number of refused origins is
  reported so the reader can see how many there are. (The published forecast stage today aborts as
  a whole when one series overflows, which is a separate defect filed as its own Issue.)

Nothing else in the protocol changes: the floors, the origins, the targets and the scores are as
registered above.

## Results (exactly as measured)

Run: `~/.rsi/bin/cargo-slot cargo run --release -p koplik-pipeline --example backtest_cdc_states -- data/fixtures/cdc/nndss-measles-weekly.retrieval.json data/reports/backtest/cdc-states.json`.
The report holds every scored target. The run is deterministic: two runs gave a byte-identical
report (SHA-256 of the committed file is recorded in the published companion; compute it with
`sha256sum`).

**Label: pseudo-real-time (revised counts truncated at each forecast date). Not real-time.**
One retrieval of the NNDSS series is held; see "What that may hide" above.

**Data and configuration.** Snapshot `c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`
(retrieved 2026-10-07T02:19:30Z), 5,096 weekly rows, 56 geographies, MMWR 2025-W01 to 2026-W38; truth
through 2026-W36 (the last two weeks are provisional and never truth). The published configuration
(`ForecastConfig::default()`, window 3, look-back 3, minimum 11 cases, 8 weeks, 1,000 members, seed
20250101, two provisional weeks), unchanged.

**Origins.** 4,984 (series, origin) pairs were considered (56 series x 89 origin weeks, 2025-W01 to
2026-W36). The method made a forecast at **245** of them; at the other 4,739 it made none:
below the 11-case minimum 4,354, incomplete window 168, a missing count in the window or look-back 208,
no infectivity 8, and **1** refused projection (Utah, `projection_overflow`, amendment 1). Of the 245
forecasts, 239 have at least one scored target (the other six are the newest origins, whose targets
are in the provisional tail). **22 of the 56 series were ever forecast with a scored target; 34 never
were** (their counts never reached 11 in a 3-week window with a known look-back, or were missing):
01, 02, 05, 09, 10, 11, 13, 15, 16, 17, 18, 19, 22, 23, 25, 28, 29, 31, 32, 33, 34, 37, 40, 44, 46,
47, 50, 54, 56, 60, 66, 69, 72, 78.

**Pooled, all 22 series (1,748 scored targets from 239 forecasts).** Plain means; CRPS and persistence
error are in cases, so the series with the largest counts and the largest projections dominate.

| horizon | n | mean CRPS (cases) | persistence MAE | 50% coverage | 90% coverage |
|---|---|---|---|---|---|
| 1 | 238 | 27.75 | 11.93 | 0.15 | 0.35 |
| 2 | 231 | 115.19 | 13.61 | 0.21 | 0.37 |
| 3 | 224 | 839.62 | 16.04 | 0.19 | 0.34 |
| 4 | 217 | 8,300.28 | 16.72 | 0.20 | 0.38 |
| 5 | 213 | 93,040.13 | 17.01 | 0.23 | 0.41 |
| 6 | 210 | 1,117,990.09 | 16.16 | 0.24 | 0.45 |
| 7 | 209 | 13,904,737.36 | 16.09 | 0.22 | 0.41 |
| 8 | 206 | 180,233,240.48 | 16.38 | 0.26 | 0.42 |
| **all** | **1,748** | **23,049,631.33** | **15.42** | **0.21** | **0.39** |

**Per series** (all horizons pooled; "measured" is the pre-registered floor of at least 40 scored
targets from at least 10 origin weeks):

| series | forecasts | origin weeks scored | n | mean CRPS | persistence MAE | 50% coverage | 90% coverage | measured |
|---|---|---|---|---|---|---|---|---|
| Utah (49) | 39 | 39 | 296 | 114,085,022.84 | 19.45 | 0.19 | 0.29 | **yes** |
| Texas (48) | 36 | 36 | 288 | 3,374.24 | 14.35 | 0.19 | 0.35 | **yes** |
| Arizona (04) | 25 | 25 | 200 | 35.78 | 7.71 | 0.19 | 0.58 | **yes** |
| South Carolina (45) | 19 | 19 | 152 | 245,629.69 | 53.89 | 0.16 | 0.28 | **yes** |
| Kansas (20) | 14 | 14 | 112 | 14,916,265.56 | 3.28 | 0.46 | 0.77 | **yes** |
| New Mexico (35) | 14 | 14 | 112 | 39.17 | 5.50 | 0.27 | 0.46 | **yes** |
| Pennsylvania (42) | 18 | 17 | 108 | 384.47 | 25.71 | 0.13 | 0.34 | **yes** |
| California (06) | 9 | 9 | 72 | 694,008.45 | 4.71 | 0.26 | 0.42 | no (insufficient data) |
| Florida (12) | 7 | 7 | 56 | 62,118,435.83 | 18.38 | 0.16 | 0.20 | no (insufficient data) |
| Washington (53) | 8 | 8 | 55 | 18,458,831.42 | 4.07 | 0.31 | 0.35 | no (insufficient data) |
| Ohio (39) | 10 | 9 | 47 | 581.96 | 6.30 | 0.26 | 0.34 | no (insufficient data) |
| Wisconsin (55) | 8 | 7 | 38 | 2,751.81 | 10.18 | 0.18 | 0.26 | no (insufficient data) |
| Virginia (51) | 5 | 5 | 35 | 1,348.43 | 12.43 | 0.03 | 0.11 | no (insufficient data) |
| North Dakota (38) | 4 | 4 | 32 | 57.53 | 6.47 | 0.22 | 0.44 | no (insufficient data) |
| New York (36) | 6 | 5 | 27 | 1,360.47 | 4.93 | 0.07 | 0.11 | no (insufficient data) |
| Colorado (08) | 3 | 3 | 24 | 8,854,238.25 | 7.67 | 0.33 | 0.33 | no (insufficient data) |
| Minnesota (27) | 3 | 3 | 24 | 2,334,097.52 | 4.00 | 0.25 | 0.33 | no (insufficient data) |
| Montana (30) | 3 | 3 | 24 | 1,281.97 | 3.50 | 0.08 | 0.38 | no (insufficient data) |
| Michigan (26) | 2 | 2 | 16 | 4.78 | 2.44 | 0.00 | 0.88 | no (insufficient data) |
| Oregon (41) | 4 | 4 | 16 | 4.60 | 2.38 | 0.38 | 0.88 | no (insufficient data) |
| Maryland (24) | 4 | 3 | 8 | 118.07 | 7.38 | 0.12 | 0.12 | no (insufficient data) |
| Kentucky (21) | 4 | 3 | 6 | 24.44 | 4.83 | 0.00 | 0.50 | no (insufficient data) |

**7 series have a measured skill** (Arizona, Kansas, New Mexico, Pennsylvania, South Carolina, Texas,
Utah); the other 15 with a scored target, and the 34 with none, have insufficient data for one.
The pooled result is measured under the same floors (1,748 targets, 239 forecasts).

## What this shows (and does not)

- **On these series the pre-registered forecaster was badly calibrated and, in mean error, far worse
  than carrying the last count forward.** Pooled, the 50% interval held the observed count 21% of the
  time (nominal 50%) and the 90% interval 39% (nominal 90%); at horizon 1 already 15% and 35%. No
  series, measured or not, had a mean CRPS below its persistence error; the forecast's CRPS was below
  the persistence error in 548 of 1,748 scored targets (31%, descriptive, added after the run).
- **The mean CRPS grows by orders of magnitude with the horizon** (28 cases at horizon 1, 180 million
  at horizon 8). That is the method, not a bug in the scoring: `R` is held constant over the horizon
  with Poisson offspring and no ceiling, so an origin at which a state's counts jump (a handful of
  weeks of zeros and then a week of dozens of cases, a pattern these NNDSS counts show, for example
  Florida's 61 cases in 2026-W05 after weeks of one or two) gives a posterior for `R` of 20 or more and
  a projection that multiplies every week. The CRPS of such an ensemble is the distance to a count
  that did not follow. The Florida origin of 2026-W05 (R mean 22) projects a median of about 4 billion
  cases at horizon 8 against an observed 0; these scores are in the report as measured.
  In NNDSS the weekly count is the growth of a published cumulative, so a state that reports in
  batches shows batch-sized jumps; whether the jump is a real surge or a reporting batch cannot be
  told from the series, and the method cannot either.
- **The scores are conditional on the method making a forecast** (amendment 1). One origin of 246 that
  met the minimum-count rule was refused for overflow and is not scored; the refused origins are where
  the method would have been furthest off, so this flatters the result, slightly (1 origin).
- **Pseudo-real-time, one retrieval.** If CDC rewrote earlier weeks when it republished, a forecaster
  at the time saw different counts; the effect on these scores cannot be measured from one retrieval.
  Re-scoring with successive retrievals is the follow-up (below).
- **A small, correlated sample per series.** The 8 horizons of one origin and adjacent origins share
  data; the number of independent forecasts is far nearer the origin weeks (2 to 39 per series) than
  the targets. No interval is given for any score; small differences between series mean nothing.
- **It is by CDC report week, for confirmed or unknown-status cases**, not by onset and not
  confirmed-only, and it scores the 22 series that ever passed the minimum-count rule: it says nothing
  about states whose counts never did (their forecast is `insufficient_data`, with no number).
- **No parameter was changed** and the pre-registered floors and defaults were not touched after the
  scores were seen. The persistence comparison is the one the protocol fixed; no other baseline (for
  example a forecast that caps `R` or a negative-binomial offspring) was run, because that would be a
  new method and would be pre-registered the same way before scoring.

## Reproduce

```bash
~/.rsi/bin/cargo-slot cargo run --release -p koplik-pipeline --example backtest_cdc_states -- \
  data/fixtures/cdc/nndss-measles-weekly.retrieval.json data/reports/backtest/cdc-states.json
```

Offline, deterministic: the same snapshot gives the same bytes. The pipeline's `forecast` stage
reads `data/reports/backtest/cdc-states.json` and attaches each NNDSS state series' measured skill,
or its absence, to the published forecast's provenance companion (contracts v7), only when the
report was run with exactly the published configuration; it also copies the report unchanged next
to the forecasts so every number on the page can be checked against it.

## Follow-ups (filed as their own Issues, not fixed here: #1514, #1515, #1513)

- #1514: a real-time backtest needs successive dated retrievals of the NNDSS series (the store keeps every
  retrieval; only one real-byte snapshot is committed). Build vintages from them with
  `koplik_epi::backtest::vintages`-style versions once there are enough.
- #1515: the method explodes after a one-week burst of cases (see above). Any change (a cap on `R`,
  negative-binomial offspring, a smoothed input) is a method change and must be pre-registered and
  scored the same way before it replaces the published default.
- #1513: the forecast stage aborts the whole run when one series' projection passes 2^40, instead of
  marking that series.

## Publication (contract v7)

The pipeline's `forecast` stage reads `data/reports/backtest/cdc-states.json` and attaches the
evaluation to the forecast's provenance companion as `series_backtest` (contract **v7**: v6 is #1422's row
artifacts, which landed on `rolling` while this was in flight, so v7 is the next free number; v5 and v6
are frozen). It attaches only a report run with exactly the published configuration (window,
look-back, minimum cases, horizon, members, seed, provisional weeks), on the NNDSS source, whose
protocol carries the label "pseudo-real-time (revised counts truncated at each forecast date)";
otherwise nothing is attached and the companion says so. Each forecast series' `skill` is
`measured` (its own entry carries scores because it reached the floor), `insufficient data for a
measured skill` (the evaluation ran on it but scored too little; its counts are published, its
scores are not), or `not backtested; no measured skill` (not an NNDSS state series, for example the
Texas DSHS county series). The contract's deserializer rejects a series whose skill disagrees with
its entry, a floor that is not what the scores say, pooled numbers that do not add up, and a
pseudo-real-time backtest that does not say so. The page shows, above each chart, the measured
numbers for that series in plain words (or that it has none), and a separate evaluation block
(pooled, series with a measured skill, limitations, the exact report) apart from the West Texas
evaluation. The report is copied byte for byte beside the forecasts
(`forecasts/backtest-cdc-states.json`).

At the origin of the published forecast (2026-W36) the fixture run forecasts 6 series: Pennsylvania
(measured: 108 targets from 17 origin weeks, mean CRPS 384.47 against 25.71 for carrying the last
count forward, 90% coverage 34.3%) and Kentucky, Maryland, New York, Ohio and Wisconsin, which have
insufficient data for a measured skill.

## Publication policy (decided 2026-10-07 by the manager after this result; fixed here before the code that applies it)

**Status of this section, stated honestly.** The measured result above came first, and it is the
reason for this policy: the pre-registered forecaster failed its own test on the published series. So
the policy is *not* pre-registered with respect to that result; it is a rule written from first
principles, with thresholds fixed below and never tuned, and it is pre-registered with respect to
every later result (a new origin, a new snapshot, a new series, a new method, a new evaluation).
It is applied mechanically by `koplik_pipeline::forecast_stage` and re-checked by the contract's
deserializer (`koplik_contracts::v7::PublicationPolicy::admits`) and by the web page; nobody decides
a series by hand.

**The rule.** *A series' forecast is published only if its method has a measured skill on that very
series that meets the criterion; otherwise the forecast is withheld and the page says so and why.*

A series meets the criterion exactly when **all** of these hold, on the scores the evaluation
measured for that series (pooled over its scored targets, all horizons):

1. **It has a measured skill**: the evaluation scored at least 40 targets of it from at least 10
   origin weeks (the floor above, or the report-vintage backtest of that very series). Below the
   floor the series has insufficient data for a skill and is withheld, whatever its scores.
2. **Its 90% intervals are calibrated well enough: `coverage_90 >= 0.75`.** First principles: the
   shown bands are the 50% and 90% bands, so a published band must be about what its label says. 0.75
   is the nominal 0.90 less 0.15, the sampling noise of a coverage estimate resting on about 10
   independent origins (a binomial standard error at 0.9 with 10 draws is about 0.095, so 1.5
   standard errors is about 0.14). Anything lower is a band that is detectably too narrow.
3. **It is no worse than the benchmark: `mean CRPS <= 1.0 x` the persistence baseline's mean absolute
   error** (carrying the origin week's count forward, the baseline the protocol fixed). A forecast
   that costs more error than repeating the latest complete week's count tells the reader less than
   the number they already have.

These are applied to the series' scores pooled over its scored targets (all 8 horizons), not per
horizon: a series that clears the 40-target floor has only about 5 targets per horizon, too few to
judge a horizon alone, and the long horizons, where the forecaster failed, dominate the pooled mean
CRPS anyway. The thresholds are carried in the companion (`publication_policy`) so the page and the
contract check the same numbers.

**Mechanical consequences.**

- A forecast series whose skill is `measured` (or `backtested`, for the Texas DSHS outbreak total)
  and that meets the criterion is `forecast`: its rows are published.
- Any other series for which the method would have forecast (its minimum-count rule held) is
  `withheld`, with the reason: `not_backtested` (no evaluation scored it), `insufficient_data_for_skill`
  (the evaluation ran on it but below the floor) or `skill_below_policy` (measured, but coverage or
  CRPS fail the criterion). No rows are published for it.
- A series for which the method's rule did not hold stays `insufficient_data` (there is nothing to
  withhold), with the estimator's reason, or `projection_overflow` when the method refused the
  projection.
- The evaluation, with its numbers and scope, is always published, whether or not any series
  qualifies. The withheld series' forecasts are computed and kept in the pipeline's work directory
  for audit (`forecast/withheld.json`); they are never copied to the published data.

**Applied to today's result (2026-10-07 snapshot, origin 2026-W36).** No series qualifies, and the
outcome does not depend on the thresholds: the author had already seen the per-series table when
writing them, so this is stated plainly. Seven series reach the floor (AZ, KS, NM, PA, SC, TX, UT).
Only one has `coverage_90 >= 0.75` (Kansas, 76.8%), and its mean CRPS is 14,916,265.56 against 3.28
for persistence; the other six fail both criteria. Every one of the seven has a mean CRPS above its persistence
error (the ratio runs from 4.64 for Arizona to about 5.9 million for Utah), so criterion 3 alone
refuses all of them, whatever the coverage threshold is. Pennsylvania, the only series with a
measured skill that the site would otherwise have forecast at this origin, has 90% coverage 34.3% and
mean CRPS 384.47 against 25.71. The panel therefore publishes no forecast and says why, in plain
words, with the pooled measured numbers (90% intervals contained the true count 39.0% (682 of 1,748)
of the time; mean CRPS 23,049,631.33 against 15.42 for repeating the latest complete week's count).

**What would change this.** A series becomes publishable only by a measured result, never by editing
the thresholds: (a) a pre-registered bounded-growth variant of the method (#1515: negative-binomial
offspring, a cited cap on R, or a smoothed input), scored on the same protocol and shown to meet the
criterion on a series; (b) a real-time backtest by report vintage instead of pseudo-real-time
truncation, once dated retrievals of the source exist (#1514), if it measures a different result. The
refusal of a projection that grows past the method's limit (#1513) no longer aborts the stage: that
series is marked `insufficient_data` with the reason `projection_overflow`, and the rest are
published or withheld as above.
