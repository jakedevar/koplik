# #1677 R_t infectiousness gate: simulation preregistration (#1679)

## Registration and scope

Written on git SHA `ba1529d621b4a7c30b8f2875d66c0f7d30b63495` (the
starting `origin/rolling`), before any simulator for this study was written or
run. This file implements revision R3 of the [#1678 scientific
review](1678-rt-infectiousness-gate-review.md), with R1's rationale and R2/R4's
future implementation plan. The manager must land this file before launching
step 2. The results file must record the full SHA of the commit containing this
registration on rolling; the starting SHA above is not that registration SHA.

No outcome data was inspected beyond the [#1672
trace](1672-texas-w40-rt.md). No published artifacts, additional published rows'
infectiousness, backtest scores, or simulation outcomes were inspected for this
registration. No simulator, method, contract, or data change accompanies it.
The rule was motivated by #1672 W40; the value 1.0 was fixed before any other
published row's Lambda was inspected outside that trace. W40's removal is a
consequence, not a criterion. The trace already contains W9 as well as W40;
this disclosure does not claim blindness to those observations. Registration
protects against tuning to a subsequent sweep or backtest, not against W40's
role in motivating the proposal.

All choices below are fixed before outcomes. Failure means **REJECT**, never
selection of another threshold, seed, scenario subset, reporting model, or
parameter cell. Inadequate sampling means **inconclusive**, with counts and no
implementation. Any later redesigned study requires a separate registration;
it cannot replace or relabel this study's result.

## Candidate rule and rationale (R1)

Let `I = sum_{s in window} I_s` and
`Lambda = sum_{s in window} sum_{k >= 1} w_k I_{s-k}`. Keep the existing
minimum-case gate `I >= 11`. Among otherwise publishable steps, withhold when
`Lambda < min_window_infectiousness`, with candidate default **1.0**. Equality
publishes. The floor is **per estimation window of tau steps**, not per day,
week, or lag. The proposed default is tied to the current weekly R_t
`RenewalConfig.window = 1`; it has different stringency for a longer window.
Existing missing-history and zero/nonpositive/NaN-infectivity guards remain.

Lambda is expected window incidence at R = 1; with tau = 1 it is the
SI-weighted mean preceding weekly count. For a Gamma(shape a, scale b) prior,
the posterior mean has the exact identity, for Lambda > 0:

```
(a + I)/(1/b + Lambda)
  = [1/(1 + b*Lambda)]*(a*b)
    + [b*Lambda/(1 + b*Lambda)]*(I/Lambda).
```

The prior weight is exactly `1/(1 + b*Lambda)`, independent of I. At the
default b = 5, Lambda >= 1.0 bounds that weight by **1/6**. Changing the prior
scale changes the equivalent prior-weight bound; changing tau changes Lambda.
This is a necessary-condition screen against almost no observed infectors,
**not an adequacy guarantee**. Under offspring overdispersion the heuristic
`CV^2 approximately 1/I + 1/(k*Lambda)` illustrates uncertainty that the
Poisson posterior's `1/sqrt(a + I)` does not capture (#1678 R1, citing
Lloyd-Smith et al. 2005). Even Lambda = 1 can leave large uncertainty. Passing
the floor does not establish adequacy or freedom from reporting/import error.

With the existing I >= 11 rule, every newly withheld step has
`0 < Lambda < 1`, hence `I/Lambda > 11`. This screens onset, restart,
importation, and batching regimes where attribution to the observed preceding
cases can fail. It is not a monotone ratio gate: I = 100, Lambda = 1 publishes;
I = 11, Lambda = 0.99 is withheld. The floor is a deliberately weak **Koplik
project policy**, not a biological cap or a derived scientific default.
No cited source supplies the number 1.0. The review's Cori (2013), EpiEstim,
Thompson (2019), Gostic (2020), and Nash (2023) references supply method or
limitation context, not this numerical cutoff.

## Independent generator and fixed parameter sources

The generator is an individual-level branching process at daily resolution,
with continuous event times within each day. It does not draw weekly counts
from the estimator's Poisson(R*Lambda) likelihood, use its weekly weights to
generate offspring, fit published counts, or draw R from the estimator prior.
Each event represents an infection/onset proxy. Gamma serial intervals are
used as generation-time proxies, as in the production method; no separate
latent period, symptom delay, recovery process, population depletion, or
susceptible pool is modelled. This is a branching stress study, not a fitted
SEIR reconstruction or fabricated source data. Synthetic outputs must remain
explicitly labelled simulation data, separate from raw source snapshots.

| Parameter | Fixed value or construction | Source and interpretation |
| --- | --- | --- |
| Primary SI | Gamma, mean 11.7 days, SD 3.0 days; shape `(mean/SD)^2`, scale `SD^2/mean` | #1678 R3; `SerialInterval::MEASLES` in `rt/serial_interval.rs` cites CDC MMWR 2026;75(33), Klinkenberg/Nishiura 2011, and Vink et al. 2014; source attribution inherited from that code, not newly verified here |
| SI sensitivity | Gamma, mean 14 days, SD 4 days | #1678 R3, a stress choice, not an alternative measles estimate |
| Offspring | Poisson(mean R) when k = infinity; otherwise Gamma-Poisson mixture: individual mean drawn Gamma(shape k, scale R/k), then Poisson of that mean | #1678 R3; negative-binomial variance `R + R^2/k`, distributional context Lloyd-Smith et al. 2005 as cited in review |
| Dispersion grid | k in {infinity, 1.0, 0.3} in scenarios 1–4; k = 1.0 in scenario 5 | Scenario 1 grid and scenario 5 value from R3; using the same grid in 2–4 is this registration's fixed stress design; none are claimed measles estimates |
| R trajectories | Scenario table below | #1678 R3 stress values, not fitted epidemiological parameters |
| Imports and observations | Scenario table and reporting rules below | Rates, burst, holding probability, thinning probability from R3; unspecified timing/cluster/initialization choices frozen here as design assumptions |
| Initial condition | One latent seed event at time 0.5 days in every scenario, subject to that scenario's observation model (including holding in 4 and thinning in 5) | Arbitrary fixed design assumption, independent of W40's 27 reports; no pre-series events |
| Duration and burn-in | 40 complete MMWR weeks (280 days); weeks 1–6 burn-in, score weeks 7–40 | #1678 R3; burn-in retained in estimator input, not discarded from its history |
| Calendar | Day 0 = Sunday 2025-01-05; weeks grouped by contracts v1 `MmwrWeek::from_date` | Arbitrary calendar anchor, not a historical replay; Sunday–Saturday grouping is contracts v1 MMWR convention |
| Replicates and seeds | 2000 per scenario × parameter cell; r = 0,…,1999; seed `s*1_000_003 + r` | #1678 R3; explicit scenario IDs below |
| Estimator | `estimate_series`, `RenewalConfig::default()`, SI `SerialInterval::MEASLES.discretize_weekly(8)` | Production code at starting SHA: window 1, Gamma prior shape 1/scale 5, min_cases 11, levels [0.5, 0.95], `BeforeSeries::Unknown`; eight-week lag from `RtConfig::default()` |
| Candidate and metric constants | Floor 1.0; bins, levels, error ratio 3, counts and pass bounds below | Policy from #1677/#1678; diagnostic and acceptance constants from R3, with explicitly stated stricter decisions below |

In the SI sensitivity suite change **generator SI only**; keep production
estimator SI unchanged. This intentionally tests SI misspecification and is
fixed here, rather than selecting a matching estimator after results. Both
suites must pass for support (stricter than a report-only sensitivity).

### Event construction and random draws

Use the repo's portable `rand_chacha::ChaCha8Rng` (0.9), initialized with
`rand_core::SeedableRng::seed_from_u64(seed)` (0.9). Use the midpoint uniform
mapping in `koplik-epi/src/sampling.rs`, portable Poisson/binomial samplers
from that file, and gamma inverse-CDF sampling with `rt::GammaDist::quantile`
of that uniform. Sharing numerical utilities does not make the individual
branching generator a weekly renewal generator. New simulator code belongs
only to step 2. Every stochastic choice is seeded; no OS entropy, platform
distribution sampler, `usize`/`isize` random draws, unordered iteration, or
parallel float reductions. All seeded transcendentals use `libm` (spec E4).

For each calendar day, draw its import count first. Import rates expressed
per week are homogeneous Poisson arrivals with daily mean rate/7. Each
import gets an independent uniform time inside that day. Restart-cluster
arrivals use the analogous daily Poisson(cluster rate/7); all members share
the cluster's uniform arrival time. Burst arrivals use the same within-day
uniform timing after background imports. Process all events chronologically,
breaking ties by monotonically assigned u64 event ID. For each processed
individual draw its offspring count (mixture mean first for finite k), then
one gamma lag per child in child-index order, with positive continuous lag
added to the parent's time. Imports transmit under the same local R as locals.
Children within the same day are processed before advancing to the next day.
Daily incidence is the count of events whose time has that day's integer
floor. Aggregate those daily counts to MMWR weeks **after** generation.
Do not round lags to weeks, discard same-week offspring, renormalize generator
lags, or use the estimator's truncated look-back to kill transmission chains.

Keep pending events beyond day 280 for extinction detection, without adding
them to the 40-week incidence or processing their descendants beyond the
study horizon. No survival-conditioned replicates, retries with new seeds,
forced extinction, incidence caps, or replacements for empty trajectories.
Overflow, a resource refusal, invalid sampler output, or an incomplete run
must be reported with scenario/cell/seed; it cannot be silently omitted or
treated as a full pass. An incomplete study cannot support implementation.

## Scenario list and cell expansion

IDs s = 1,…,5 are fixed. Repeat the complete list for primary and sensitivity
generator SI. Values omitted from a row inherit the common rules above;
observation is complete and on time except in scenarios 4 and 5.

| s | Scenario | R and imports | Cells per SI suite |
| --- | --- | --- | --- |
| 1 | Steady transmission | Constant R in {0.8, 1.0, 1.5, 3.0}; background 0.5 imported individuals/week throughout | Four R values × three k values × {Unknown, Zero} before-series variants = 24 |
| 2 | Extinction and restart | R = 0.6, no imports, until the initial seed's chain has no pending descendants; from the next week's Sunday permanently R = 1.5 and 0.2 imported clusters/week | Three k values, Unknown only = 3 |
| 3 | Imported-case burst | Constant R = 0.8, background 1 imported individual/week; additionally 15 imported individuals on the Wednesday of study week 20 | Three k values, Unknown only = 3 |
| 4 | Reporting batches | Underlying generator as scenario 1 with R = 1.0 and 0.5 imports/week; weekly hold probability 0.1, release rules below | Three k values, Unknown only = 3 |
| 5 | Incomplete ascertainment | Underlying generator as scenario 1 with R = 1.0, k = 1.0, and 0.5 imports/week; independently retain each event with probability 0.3 | One k, Unknown only = 1 |

Restart clusters have **15 individuals each**, a fixed stress construction
using R3's burst size, not a measured import-cluster size. Extinction means
no future descendants scheduled from the initial chain, not merely an empty
day/week or zero estimator Lambda. Its detection is recorded after processing
the day that leaves that queue empty. R changes only on the following
Sunday, so cohort truth remains constant within each week. If extinction or
restart does not occur within the horizon, retain that replicate and report
this fact; do not force a transition. Subsequent extinctions do not change
the permanently switched R or stop the cluster-import process.

Scenario 1's label means constant R, not a claim that an equilibrium was
reached in six weeks. Before-series Zero is a second analysis of the **same
generated series**, only in scenario 1. It does not add prehistory. Unknown
still withholds weeks whose eight-week look-back reaches before the series,
even after the six-week burn-in; do not shorten the look-back to gain samples.

There are 34 analysis cells per SI suite. List all cells and replicates in
the step-2 manifest in table order, R order as listed, k order infinity/1/0.3,
then Unknown/Zero. Seeds are explicitly the integer ranges
1,000,003–1,002,002; 2,000,006–2,002,005; 3,000,009–3,002,008;
4,000,012–4,002,011; 5,000,015–5,002,014 for s = 1,…,5 respectively.
Reuse the scenario seed across its parameter cells and SI suites as a fixed
common-random-number design; do not add a cell offset or hunt for seeds.
Do not count paired analyses/cells as independent replicates in uncertainty.

### Observation rules and truth

For batches, first finish the latent event process, then draw independent
weekly Bernoulli(0.1) holding indicators in week order, using that replicate's
continued RNG stream. If week's onset count is C_w and its holding indicator
is H_w, reports are `(1-H_w)*C_w + H_(w-1)*C_(w-1)` with no pre-series carry.
Previous held cases always release next week, even if the current week's
cases are held; nothing is repeatedly held. Reports are assigned to their
release week. Week-40 held cases lie beyond the horizon: disclose that carry,
do not flush it into week 40. Complete reporting means known counts including
zero, not unknown/missing counts.

For ascertainment, independently thin the daily onset counts with
Binomial(count, 0.3), in day order after generating the full latent process,
using the replicate's continued stream. Thin imported and local events alike.
Thinning does not remove individuals or descendants from transmission.
Aggregate retained daily counts to weeks. Other scenarios retain all events.

Truth for week w is the generator's **mean offspring number assigned to the
infector cohort in that window's week**, R_w, not realized offspring/counts
and not the Cori window estimand. All events in that week have the specified
R_w, even in the restart scenario. Use the prescribed regime's R_w also when
the cohort is empty; do not estimate truth from outcomes. Imports are treated
as local by the estimator; in scenario 3 truth stays R = 0.8 throughout the
burst. This is intentional misspecification. Reporting-week estimates in
scenario 4 are compared with underlying cohort R = 1.0, not a reporting ratio.
Cohort reproduction, infection-time reproduction, and the constant-R Cori
window estimand can differ; coverage here measures agreement with the stated
cohort truth and does not prove estimator adequacy.

## Metrics, denominators, and Monte Carlo uncertainty

Run the unchanged baseline `estimate_series` for both credible levels before
applying the candidate as a classification to otherwise publishable steps.
Compute Lambda independently from the observed weekly series and production
weights in the same ordered sums; do not add the future config field now.
The floor changes status/publication, not retained means or intervals.
Use posterior **mean** for R-hat. Coverage includes the endpoints.

All required metrics are conditional on the existing **I >= 11** count gate
in scored weeks 7–40. Report the following Lambda bins separately for **every
cell**, including cells with zero steps:
`[0,0.25)`, `[0.25,0.5)`, `[0.5,1)`, `[1,2)`, `[2,5)`, `[5,infinity)`.
Equality belongs to the bin on the right. Also report their prespecified
union `[0,1)` for S1. Never omit a cell because it looks unhelpful.

Report total count-gate steps, count in each bin, baseline scoreable count,
old insufficiency reasons, newly withheld count, coverage at 95% and 50%
with Monte Carlo standard errors, median and 90th percentile of
`abs(log(R-hat/R_w))`, fraction `R-hat/R_w > 3`, and fraction of **all
I >= 11 steps** newly withheld by the floor. Per-bin newly withheld fractions
use that bin's count-gate denominator; the all-step fraction uses all scored
I >= 11 weeks, including old insufficient weeks. Also report total candidate
insufficient fraction and the fraction newly withheld among baseline-ok steps
to distinguish these denominators. S3 is report-only.

Coverage/error metrics use baseline-ok steps with complete usable history
and positive Lambda, including steps the candidate would withhold. Otherwise
there is no interval/mean to score. Do not impute a posterior for
`NoInfectivity`, count missing intervals as successes, or evaluate only
candidate-retained steps in `[0,1)`. Report Lambda = 0 separately within
`[0,0.25)` with coverage/errors marked **missing** and its existing reason.
Unknown Lambda from incomplete history is a separate unbinned count, not
zero. Empty/undefined denominators give counts of zero and **missing**
metrics, never an estimated zero rate. At least 200 **scoreable** low-bin
steps are required for S1 (stricter than counting unscoreable steps).

For empirical error percentiles sort by error then replicate/week and use
the nearest-rank definition `ceil(p*N)` (one-based) for p = 0.5 and 0.9.
No interpolation or trimming. Pool step-level indicators for coverage and
fractions. For each reported coverage, let n_r be the scoreable denominator
and h_r the coverage-success count in replicate r, p = sum(h_r)/sum(n_r),
M = 2000. Report replicate-cluster Monte Carlo SE:

```
SE = sqrt(M/(M-1) * sum_r (h_r - p*n_r)^2) / sum_r n_r.
```

This accounts for dependence between weeks within a replicate. For a
scenario pooled across k, first sum h_r and n_r over its fixed k cells for
each r (seeds are paired); do not pretend each cell is another independent
replicate. Primary/sensitivity and Unknown/Zero are never pooled. Report
counts of nonempty replicates alongside SE. Point criteria below use the
measured coverage, without rounding, SE adjustments, or significance-based
substitutes.

## Frozen decision rule (S1/S2/S3)

For S1, prespecify one scenario-level table for each of scenarios 2–5,
pooling **all** its listed k cells and replicates by summing steps; never
choose the best k. Require the per-cell tables as well. Evaluate each SI
suite separately. A scenario meets S1 only when:

- It has at least **200 scoreable steps in [0,1)**.
- Its 95% CrI coverage in [0,1) is **<= 0.80**.
- That coverage is at least **0.10 lower** than coverage in [1,2) in the
  same scenario, suite, and fixed pool; the reference bin must be nonempty.

**S1:** at least **three distinct scenarios out of 2–5** must meet all three
conditions in each suite. Three k cells of one scenario never count as three
scenarios. If fewer than three scenarios reach the 200 scoreable low-bin
steps in either suite, the study is **inconclusive**: report counts and do
not implement. Missing reference coverage also cannot count as support; if
it prevents evaluation of three qualifying scenarios, report inconclusive.
If enough scenarios can be evaluated and fewer than three meet the numeric
criteria, **REJECT**.

**S2:** in scenario 1 with k = infinity, 95% CrI coverage in [1,2) must be
**>= 0.93**. Require this separately for **each** of the four R cells, each
before-series variant, and each SI suite, rather than selecting a cell or
using a pool that hides failures. This is a stricter prespecified reading of
R3. Empty S2 reference cells make the study inconclusive, not a vacuous pass;
an evaluable S2 cell below 0.93 means REJECT once the sampling requirements
are met. The primary Poisson generator is the review's "correctly specified"
offspring benchmark; imports, weekly aggregation, prior mismatch and the
sensitivity SI still limit that description. No matching-prior calibration
is assumed for these fixed R values.

**S3:** report the withheld fraction, with no target and no acceptance
criterion. Do not judge success by W40 disappearing, by a plausible maximum
R, by an acceptable-looking withheld fraction, or by improved backtest scores.

Full completion and all sampling requirements come first; otherwise the
outcome is inconclusive and all observed failures/counts remain disclosed.
When evaluable, **SUPPORT** requires S1 and every S2 cell to pass in both
suites. Any numeric failure then means **REJECT**. Neither REJECT nor
inconclusive permits implementation or a re-picked threshold. Do not expand
replicate counts, lengthen burn-in/horizon, add bursts, collapse bins, or
relax these criteria to obtain support. Record the decision and all cells
in the committed results file, even if the decision is inconvenient.

Advance expectation from R3: under a correctly specified Poisson model a
Bayesian estimate with a matching prior is roughly calibrated even at small
Lambda, so a benefit can only appear under misspecified scenarios. This is
an expectation, not an outcome and not a guarantee for the fixed production
prior or weekly approximation. If S1 fails, the floor is unsupported and
must not be implemented or retuned.

The results file must record this limitations text verbatim from R3:

> no correction for delays/right truncation, imports treated as local, constant-R windows, weekly aggregation approximation (Nash et al. 2023), simulation shows frequentist coverage not scientific adequacy, values of k are stress values.

## Step 3 implementation plan only (R2/R4)

Proceed only after this registration is on rolling and step 2's complete
results, registration SHA, and SUPPORT decision are committed. Tier2
cross-family pre-merge review still applies. Nothing below is implemented
by #1679.

- Add `min_window_infectiousness: f64` to `RenewalConfig`, validated finite
  and >= 0. Default 1.0 **only if the registered criteria pass**; 0.0 must
  reproduce current behavior exactly. Doc comment cites #1677/#1678 and
  explains policy status, window/prior dependence, and no adequacy guarantee.
- Preserve structural `IncompleteWindow`, then missing window/look-back
  counts, then `min_cases`, then existing `NoInfectivity` (Lambda <= 0 or
  NaN), then `BelowInfectivityFloor { lambda, floor }`. The estimator's
  currently Eq-compatible reason enum will need an intentional type change
  if this diagnostic variant carries f64; contract enums carry **no floats**.
  Zero exposure retains its old reason. Published status stays
  `insufficient_data`, with numeric fields absent when withheld.
- Apply the same configurable gate to forecast origins through
  `ForecastConfig.renewal`/`estimate_series`, and consequently to both
  `backtest/truncated.rs` and `backtest/run.rs`. This is an explicit **yes**
  to shared scope, not an R_t-only gate. Forecast defaults currently use a
  three-step window and three-week lag, unlike R_t's one/eight; 1.0 applies
  to the whole forecast window, without rescaling or retuning. Report
  changed origin sets, `reason_name` counts, and scores exactly as measured;
  no tuned re-scoring or changed publication policy.
- Create **contract v9**, with regenerated JSON Schema, for the new
  forecast-provenance `InsufficientReason` value (`below_infectivity_floor`).
  Released v7 and all other released versions remain immutable; update
  producing/consuming code and version envelopes together. Current v1
  `RtStatus` is only `ok`/`insufficient_data`, and `RtEstimate` has no reason.
  For this plan confine the detailed R_t reason to the estimator; do not
  claim `rt.json` exposes it or add an unplanned reason field. Public
  forecast provenance carries the v9 reason.
- Future tests cover below/equal/above floor, small positive versus zero
  Lambda, missing weeks/history, ordinary well-supported estimates, invalid
  floor, and floor = 0 identical to baseline. Equality uses an exactly
  representable Lambda, e.g. `DiscreteSerialInterval::from_weights` with
  single-lag weight 1.0; never infer equality from gamma-SI rounding. Scoped
  epi/pipeline tests and native/wasm determinism follow the repository gate.

## Step 4 report-only audit order (R5)

Only after step 2's results and registration SHA are committed, and after a
supported implementation, produce the full before/after audit for identical
immutable inputs: published gh-pages
`6bcd59b6edcc06a80b91f7f3108110b3625e62e3`, snapshot
`c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`
with source URL/retrieval lineage recorded in #1672. Include all
geographies/weeks/levels, old/new status and numbers, Lambda and I,
forecast-origin and backtest-origin count changes, and the Lambda
distribution of all published rows once. These are report-only context,
never threshold selection or simulation pass criteria. No acquisition,
licence acceptance, public publishing, or audit is authorized by this file.

Reviewer should first check frozen generator independence, the complete
cell/seed list and scoreable-step denominators against R3, then the stricter
S1/S2 interpretation, W40 disclosure/R1 identity, and conditional v9 scope.
