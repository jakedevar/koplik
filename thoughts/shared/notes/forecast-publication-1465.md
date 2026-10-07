---
date: 2026-10-07
issue: 1465
topic: publishing the 4-8 week forecast (#1361 method) and closing the #1352 population wiring
status: built after the method and its defaults were fixed (#1361); nothing here was chosen from a score
---

# Publishing the forecast (#1465)

## What is published

`koplik-pipeline forecast` runs `koplik_epi::forecast::forecast_weekly` on `validate/weekly-cases.json`
with `ForecastConfig::default()` unchanged and the seed `20250101` (the backtest's), and writes v1
`Forecast` rows plus a **contract v5** `ForecastProvenance` companion (new version; v1 to v4 are
untouched and now frozen through v4). `build` publishes `forecasts/weekly-cases.json`, its companion and
the backtest report the skill was read from (byte for byte), or none of them. The web panel
("Where next?") shows the forecasts; the backtest is a separate section ("How we evaluate forecasts").

## Decisions (stated, not tuned)

- **Origin week = latest week with a row, less 2 provisional weeks** (spec E5, the rule R_t uses). A
  forecast anchored on the last, still-being-reported weeks would be biased by reporting delay; the
  backtest used "latest complete week" for the same reason. The provisional weeks are drawn on the chart
  as dashed bars so the reader sees how the forecast sits against them. This rule is **not** backtested on
  CDC data (see below).
- **Which series.** Every geography in the input, each under its own case definition (a mixed series is
  an error). A series is forecast only where the method's own rule holds at the origin: every count in the
  3-week window and the 3-week look-back known, at least 11 cases in the window, some infectivity.
  Otherwise the companion says `insufficient_data` with the reason and no row exists.
- **Scope of the skill.** The backtest scored the **Texas DSHS outbreak total by report date** (confirmed,
  outbreak-associated, every county combined), 48 targets from 7 forecast dates / 5 origin weeks. No
  published series is that series: the forecasts are of CDC NNDSS state series
  (`confirmed_or_unknown_status`), and Texas DSHS county series end long before the origin. So **no
  published forecast has been backtested**: each series says `not backtested; no measured skill` in the
  companion, and the page says so first, above every such chart, with no skill number or calibration
  word beside it (review 9d8d6386 of e656f57 found the earlier "read the bands as optimistic" carried the
  DSHS calibration over to series it was never measured on). The backtest has its own section. The skill is attached
  only from a report run with exactly the published configuration (window, look-back, minimum cases,
  horizon, members, seed); a mismatch detaches it with a manifest note.
- **Measured skill shown as measured** (pooled): mean CRPS 3.66 cases, persistence baseline 5.79; 90%
  intervals contained the truth 30 of 48 times (62.5%), 50% intervals 23 of 48 (47.9%): too narrow, and
  the evaluation section says so about the backtest only (the sentence is conditional on the numbers).
  Percentages are shown with one decimal and their counts everywhere, page and companion (0.625 is
  neither 62% nor 63%). The range of counts quoted for the backtest (0 to 10 cases) is that of the 48
  scored targets, not of the whole history (which reaches 61 in March 2025, before any forecast could be
  made).

## Measured on the committed fixtures (`make pipeline-fixtures`)

Latest data week 2026-W38, origin 2026-W36, targets W37 to W44. Of 94 series, **6 are forecast**
(Kentucky, Maryland, New York, Ohio, Pennsylvania, Wisconsin; 22 to 283 cases in the window); 88 are
`insufficient_data`: 49 below the 11-case threshold, 38 with a missing count in the window or look-back
(the Texas DSHS county series stop in 2026-W02), 1 with no infectivity in the look-back (Texas). Two runs
of the pipeline give byte-identical `web/public/data` and stage outputs.

One thing a reviewer should look at: for Kentucky the provisional 2026-W38 count (28) sits above the
90% band (upper bound 14) of the origin-W36 forecast. That is what the stated origin rule produces when the latest weeks
jump; the chart shows it rather than hiding it.

## What is not known

- Whether this method is skilful on CDC NNDSS state series. CDC publishes no revision history, so a
  real-time backtest needs retained snapshots (the store keeps them from now on) and until then only a
  pseudo-real-time truncation, which must be labelled as such (spec E5). Follow-up: #1503.
- Whether dropping two provisional weeks is better than using them. Not scored; chosen on the reasoning
  above, before any run.

## Item 2 of #1465 (#1352 population and centroids): closed, nothing to add

#1352 and #1455 already wired both with provenance, and the "not ingested (#1352)" text is gone from the
build: `validate` reads the Census Vintage 2025 population and the Gazetteer internal points from the
store; 306 of 310 geographies carry `centroid` with the Gazetteer snapshot in their provenance (the 4
without are American Samoa, Guam, the Northern Mariana Islands and the U.S. Virgin Islands, which the
Census state Gazetteer file does not list; they stay null, never guessed); population is a field of each
scenario node (23,956 for Gaines County) with its snapshot. Nothing else reads population (the
forecast is of counts, not rates), so no separate `population` artifact is published, as `web/README.md`
already says.
