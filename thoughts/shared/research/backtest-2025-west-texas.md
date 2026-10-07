---
date: 2026-10-07
issue: 1361
topic: 4-8 week forecast and its backtest on the 2025 West Texas measles outbreak
status: method pre-registered at commit 705bd94 (before any score was computed); results measured at the run recorded below
---

# Forecast backtest: 2025 West Texas outbreak

## Scope (read this before the numbers)

The backtest scores forecasts of the **Texas DSHS outbreak total** (all outbreak-associated
confirmed cases, every county combined) **by report date**, using **report vintages**: at each
forecast date only the report versions first seen at or before that date are used, exactly
as a forecaster at that date could have. It is a real-time backtest by vintage, not a
pseudo-real-time truncation of revised counts.

What it is **not**:

- Not a county-level backtest. DSHS published county breakdowns only for 2025-03-04 to
  2025-03-25 (seven HTML versions) and in three PDF data reports (2025-11-24, 2025-12-23,
  2026-01-12); for 2025-03-28 to 2025-08-12 the county table was a Tableau dashboard that
  refuses automated access and was never archived (Issue #1402). The only number every
  version prints is the outbreak total, so that is what is forecast and scored. The county
  gap is never filled.
- Not a symptom-onset series. The weekly count is the growth of the printed cumulative total
  between the last version of one MMWR week and the last version of the week before, i.e.
  when DSHS published the cases. Reporting lags onset.
- Not a full outbreak. The archived versions start on 2025-03-04 (cumulative 159); the
  outbreak's January-February growth exists only in news releases that are not parsed.
  The first weekly count is MMWR week 11 (2025-03-09 to 03-15). Under the honest
  before-series rule (unknown, not zero) a forecast needs the serial-interval look-back and
  the estimation window to be fully known; with weeks 13-15 also missing (see "Data actually
  available"), the earliest possible origin is week 21, and the March-April growth phase
  cannot be forecast from this data.

Every number in the report traces to a version's snapshot: `data/reports/backtest/west-texas-2025.json`
records the manifest's SHA-256, and the manifest records each version's capture URL, capture
time (`first_seen_at`) and snapshot hash.

## Method (pre-registered)

Implemented in `crates/koplik-epi/src/forecast.rs` and `src/backtest/`; every parameter
cites its source in a code comment. Fixed before any score was computed and not changed after.

**Forecast.** Renewal-equation projection (Nouvellet et al. 2018, *Epidemics* 22:29-35,
doi:10.1016/j.epidem.2017.02.012; the RECON `projections` package):

1. `R` at the origin week is the Cori et al. 2013 posterior over the last **3 weeks**
   (`Gamma(a + ΣI, 1/(1/b + ΣΛ))`), reusing the R_t module: EpiEstim prior mean 5, SD 5;
   measles serial interval mean 11.7 d, SD 3.0 d (CDC MMWR 2026;75(33), Vink et al. 2014),
   discretised to MMWR weeks; look-back **3 weeks** by the rule "smallest look-back with
   under 1% dropped serial-interval mass" (measured: 6.6% at 2 weeks, 0.54% at 3, of which
   0.45% is same-week mass no weekly look-back can keep). Before the series is **unknown**.
   The window of 3 weeks is a pre-registered choice (≈1.8 mean serial intervals: at least one
   generation of infectors and infectees, short enough to follow a change within a month);
   Nouvellet et al. chose windows ad hoc per outbreak, which is what this rule replaces.
2. **Insufficient data** (no forecast) when any count in the window or look-back is missing,
   when the window holds fewer than **11** cases (EpiEstim's posterior-CV 0.3 rule, the same
   threshold as the published R_t), or when the look-back infectivity is zero.
3. Each of **1,000** members draws one `R` from the posterior (inverse transform) and
   projects `I_{t+h} ~ Poisson(R · Σ_k w_k I_{t+h-k})` for `h = 1..8`, observed counts
   before the origin and its own counts after. `R` is constant over the horizon. Poisson
   offspring without an overdispersion parameter is the `projections` default and is a
   known limitation: it under-disperses relative to real measles clusters, so coverage is
   expected to fall short of nominal at long horizons.
4. Published quantiles: the 23 FluSight/hubverse levels (0.01 … 0.99), Hyndman-Fan type 7.

**Backtest protocol** (`koplik_epi::backtest::run`):

- Forecast dates: every **Wednesday 00:00 UTC** from the first to the last `first_seen_at`
  in the manifest. The origin is the latest *complete* week at that time: a week is complete
  only when a known version is dated in a later week, so a week still being reported never
  gets a partial count. What is known is decided by `first_seen_at` alone, never by DSHS's
  schedule.
- Truth: the same weekly derivation from every version. Versions are immutable snapshots,
  so the truth and the real-time values agree except for incompleteness; a target week
  without a reported truth is not scored.
- Scores, exactly as measured, per horizon 1..8 and pooled: mean **CRPS** of the member
  ensemble (Gneiting & Raftery 2007, eq. 21, the plain ensemble estimator, in cases),
  **50% and 90% interval coverage** of the published quantiles (bounds inclusive), with `n`.
  Reference: the **persistence** baseline (origin week's count carried forward; its CRPS is
  its absolute error). Seed 20250101, fixed before the run.
- Sensitivity: windows 1, 2 and 4 weeks are run and reported in full next to the primary.
  They are context, not a selection: the primary stays 3 whatever they show.

## Tests

`crates/koplik-epi/tests/forecast.rs` and `tests/backtest.rs`, `src/backtest/mod.rs`,
`src/sampling.rs`: CRPS against closed forms (point mass, two-point, discrete uniform; the
sorted formula against the double sum); 50%/90% coverage on synthetic data generated by the
model's own process; the information cutoff on weeks (rows after the origin never change the
forecast, a missing origin is insufficient even when later weeks exist) and on revisions (a
version first seen after the cutoff is unknown even when dated before it); the Poisson
sampler against the exact pmf; determinism (same seed bit-identical, other seeds and
geographies differ).

## Data actually available (measured)

Manifest `data/dshs/vintage-manifest.json`, SHA-256
`00ce05dd4730969da533bf1de684f07db8ae9485785912d28a9007f4f2b90a25`, 40 versions. Three
things decide how much of the outbreak can be forecast, and none of them is a modelling
choice:

1. The archived page versions start on 2025-03-04, so week 10 (the first) cannot be split
   and the first weekly count is week 11.
2. The four dashboard-only versions of 2025-03-28 to 04-08 state their total as "cases have
   been *identified*", not "confirmed". They keep their place as each week's last report but
   their totals carry no case definition, so weeks 13, 14 and 15 are `missing:ambiguous`.
   (The 26 later dashboard-only versions say "have been confirmed"; this run needed the
   ingest crate to record that sentence as the version's `confirmed_basis`, which it now
   does; the manifest was regenerated from the same snapshots and differs only in those 26
   fields. The 03-28..04-08 pages were checked by reading each version's snapshot.)
3. Nothing after the 2025-08-12 version until the 2025-11-24 PDF (same total, 762), so the
   truth ends at week 33.

Weekly truth (confirmed outbreak cases by report week): W11 61, W12 50, W13-15 missing,
W16 56, W17 49, W18 37, W19 26, W20 9, W21 10, W22 10, W23 4, W24 2, W25 6, W26 0, W27 3,
W28 0, W29 9, W30-33 0.

With a 3-week window and a 3-week look-back that must be fully known, the first possible
origin is **week 21** (2025-05-18 to 05-24): the March-April growth phase, where a forecast
would matter most, is not forecastable from this data (follow-up #1445: parse the
January-March DSHS news releases). After week 25 the window holds fewer than 11 cases most
weeks, so those origins are `insufficient_data`, as the published R_t would be.

Forecast dates: 48 Wednesdays from 2025-03-12 to 2026-02-04. Forecasts were made on 7 of
them, from 5 distinct origins (a forecast date whose latest complete week has not advanced
repeats the previous forecast; 06-11 repeats 06-04's origin W22 and 07-30 repeats 07-23's
W29, and both are counted as the protocol says, so those two origins carry double weight):

| forecast date | origin | window cases | R mean [90%] |
|---|---|---|---|
| 2025-05-28 | W21 | 45 | 0.47 [0.36, 0.58] |
| 2025-06-04 | W22 | 29 | 0.47 [0.34, 0.62] |
| 2025-06-11 | W22 | 29 | (same forecast) |
| 2025-06-18 | W23 | 24 | 0.61 [0.43, 0.83] |
| 2025-06-25 | W24 | 16 | 0.60 [0.38, 0.86] |
| 2025-07-23 | W29 | 12 | 1.90 [1.12, 2.84] |
| 2025-07-30 | W29 | 12 | (same forecast) |

## Results (exactly as measured)

Run: commit of this report, `~/.rsi/bin/cargo-slot cargo run -p koplik-epi --example backtest_west_texas -- data/dshs/vintage-manifest.json data/reports/backtest/west-texas-2025.json`
(deterministic; the JSON holds every scored target).

**Primary, pre-registered** (window 3 wk, look-back 3 wk, min cases 11, 1,000 members, seed 20250101):

| horizon | n | mean CRPS (cases) | 50% coverage | 90% coverage | persistence MAE |
|---|---|---|---|---|---|
| 1 | 7 | 2.83 | 0.29 | 0.43 | 5.14 |
| 2 | 7 | 3.86 | 0.57 | 0.71 | 6.29 |
| 3 | 7 | 5.03 | 0.14 | 0.43 | 5.57 |
| 4 | 7 | 6.66 | 0.57 | 0.57 | 6.43 |
| 5 | 5 | 2.37 | 0.40 | 0.80 | 7.00 |
| 6 | 5 | 1.91 | 0.60 | 0.80 | 6.80 |
| 7 | 5 | 3.36 | 0.60 | 0.60 | 3.60 |
| 8 | 5 | 1.75 | 0.80 | 0.80 | 5.40 |
| **all** | **48** | **3.66** | **0.48** | **0.62** | **5.79** |

Reading: on the tail of the outbreak (weekly counts 0-10), the forecast's CRPS of 3.7 cases
beats carrying the last count forward (5.8), but its intervals are too narrow: the 90%
interval covered 62% of the truths (nominal 90%), the 50% interval 48%. That is the
under-dispersion the method section anticipated for Poisson offspring with a constant `R`;
the week-29 origin (a late cluster of 9 cases after two zero weeks, `R` ≈ 1.9) then
over-projects the following zero weeks. Horizons 5-8 look better than 1-4 only because the
week-29 origin has no truth beyond week 33 and drops out of them.

**Window sensitivity**, reported in full. These were run after the primary and do not change
it; they say how much the result depends on the one pre-registered choice. Different windows
also change *which* origins pass the 11-case threshold (hence the different `n`), so the
rows are not like-for-like:

| window | n | mean CRPS | 50% coverage | 90% coverage | persistence MAE |
|---|---|---|---|---|---|
| 1 wk | 8 | 2.49 | 0.38 | 0.88 | 20.50 |
| 2 wk | 32 | 2.09 | 0.44 | 0.88 | 5.75 |
| **3 wk (primary)** | **48** | **3.66** | **0.48** | **0.62** | **5.79** |
| 4 wk | 63 | 2.15 | 0.56 | 0.71 | 4.40 |

Per-horizon sensitivity tables are in the JSON (`sensitivity_window`).

## What this does and does not show

- It is an honest, real-time-by-vintage score of a simple cited method on the tail of one
  outbreak: 5 distinct origins, 48 scored targets. It is far too small to rank methods or to
  claim calibration; it is enough to say the intervals are too narrow and to ship the number.
- It says nothing about forecasting the growth phase, county-level spread, or symptom-onset
  incidence: the data for those is not held (#1402, #1445).
- Nothing was tuned: the window, look-back, threshold, prior, serial interval, member count
  and seed were fixed and committed before this section existed, and the sensitivity rows
  were not used to change the primary.
- Next steps that would be legitimate *before* re-scoring: negative-binomial offspring with a
  dispersion cited from the measles literature; a time-varying `R` over the horizon; the
  January-March series (#1445). Each would be pre-registered the same way.
