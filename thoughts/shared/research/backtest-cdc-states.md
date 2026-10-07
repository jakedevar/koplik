---
date: 2026-10-07
issue: 1503
topic: backtest of the 4-8 week forecast on the CDC NNDSS state series the site publishes
status: PRE-REGISTERED (this section was committed before any score on the NNDSS series was computed); results are appended below the line "Results" in a later commit
---

# Forecast backtest: CDC NNDSS state series

This file is committed **twice**. The first commit holds only the protocol below, written
before the backtest code existed and before any score on these series was computed. The
second adds the results exactly as measured. Nothing in the protocol is changed after the
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
