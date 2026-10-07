---
date: 2026-10-07
issue: 1361
topic: 4-8 week forecast and its backtest on the 2025 West Texas measles outbreak
status: method pre-registered (this section was written and committed before any score was computed); results appended below once run
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
  the estimation window to be fully known, so the earliest possible origin is week 16 and
  the March growth phase cannot be forecast from this data.

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
