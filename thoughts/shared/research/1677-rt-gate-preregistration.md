# #1677 R_t infectiousness gate: simulation preregistration (#1679)

## Registration and scope

Written on git SHA `ba1529d621b4a7c30b8f2875d66c0f7d30b63495` (the
starting `origin/rolling`), before any simulator for this study was written or
run. This file implements revision R3 of the [#1678 scientific
review](1678-rt-infectiousness-gate-review.md), with R1's rationale and R2/R4's
future implementation plan. The manager must land this revised file before
launching step 2, beginning with the count-only sizing stage below. The
results file must record the full SHA of the commit containing this
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
| Replicates and seeds | 2000 per scenario × parameter cell in each stage; r = 0,…,1999; main seed `s*1_000_003 + r`; count-only sizing seed `s*1_000_003 + 1_000_000 + r` | #1678 R3 (main); #1683 E3 (sizing); explicit scenario IDs below |
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
branching generator a weekly renewal generator. These samplers are
`pub(crate)`, so step 2 must put the dev-only simulator inside `koplik-epi`
where those utilities are accessible, without a production visibility change.
Sharing `GammaDist::quantile` also risks a shared generator/estimator bug:
step 2 must add a generator-lag validation test for both SI suites, checking
sampled mean and SD against the independently calculated gamma moments
11.7/3.0 days and 14/4 days. The moment oracle must not use
`GammaDist::{cdf,quantile,mean,sd}`; record the check and its result before the
main run. New simulator code belongs only to step 2. Every stochastic choice
is seeded; no OS entropy, platform distribution sampler, `usize`/`isize` random draws, unordered iteration, or
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
| 1 | Steady transmission | Constant R in {0.8, 1.0, 1.5}; background 0.5 imported individuals/week throughout | Three R values × three k values × {Unknown, Zero} before-series variants = 18 |
| 2 | Extinction and restart | R = 0.6, no imports, until the initial seed's chain has no pending descendants; from the next week's Sunday permanently R = 1.5 and 0.2 imported clusters/week | Three k values, Unknown only = 3 |
| 3 | Imported-case burst | Constant R = 0.8, background 1 imported individual/week; additionally 15 imported individuals on the Wednesday of study week 20 | Three k values, Unknown only = 3 |
| 4 | Reporting batches | Underlying generator as scenario 1 with R = 1.0 and 0.5 imports/week; weekly hold probability 0.1, release rules below | Three k values, Unknown only = 3 |
| 5 | Incomplete ascertainment | Underlying generator as scenario 1 with R = 1.0, k = 1.0, and 0.5 imports/week; independently retain each event with probability 0.3 | One k, Unknown only = 1 |

R = 3.0 was removed before any run because it is computationally infeasible
at individual level and structurally cannot populate the low Lambda bins,
not because of any outcome (#1683 E1); no cohort-aggregated substitute is
permitted. Scenario 2 remains heavy but feasible; the step-2 results file
must report the maximum event count per cell in both sizing and main stages.

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

There are 28 analysis cells per SI suite (56 across both suites). List all
cells and replicates in the step-2 manifest in table order, R order as listed,
k order infinity/1/0.3,
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
  same scenario, suite, and fixed pool; that [1,2) reference bin must also
  have at least **200 scoreable steps**, otherwise the scenario does not
  qualify.

**S1:** at least **three distinct scenarios out of 2–5** must meet all three
conditions in each suite. Three k cells of one scenario never count as three
scenarios. If fewer than three scenarios reach 200 scoreable steps in
**both** [0,1) and [1,2) in either suite, the study is **inconclusive**:
report counts and do not implement. Missing reference coverage cannot count as support either.
If enough scenarios can be evaluated and fewer than three meet the numeric
criteria, **REJECT**.

**S2:** in scenario 1 with k = infinity, 95% CrI coverage in [1,2) must be
**>= 0.93**. Require this separately for **each** of the three R cells, each
before-series variant, and each SI suite, rather than selecting a cell or
using a pool that hides failures. This is a stricter prespecified reading of
R3. An S2 cell with fewer than **200 scoreable steps in [1,2)** is not
evaluable and makes the study inconclusive; an evaluable cell (>= 200) below
0.93 means **REJECT** once the study's sampling requirements are met.
The primary Poisson generator is the review's "correctly specified"
offspring benchmark; imports, weekly aggregation, prior mismatch and the
sensitivity SI still limit that description. No matching-prior calibration
is assumed for these fixed R values. S2 measures generator-estimator
calibration in [1,2), which the candidate floor does not alter; an S2 pass
is not evidence of benefit or absence of harm from the floor.

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

## Registered count-only sizing stage (step 2 pre-step; #1683 E3)

Before any main-run inference, step 2 must run the **same individual-level
generator, observation models, cells, SI suites, 2000 replicates per cell,
40-week horizon and six-week burn-in** using sizing seeds
`s*1_000_003 + 1_000_000 + r`, r = 0,…,1999. These seeds are disjoint from
the corresponding scenario's main-run range. The sizing ranges for s = 1,…,5
are 2,000,003–2,002,002; 3,000,006–3,002,005; 4,000,009–4,002,008;
5,000,012–5,002,011; 6,000,015–6,002,014. Preserve manifest order and seed
reuse across each scenario's parameter cells and SI suites; Unknown/Zero
remain paired analyses of one generated series. The main-run seed ranges
above stay unchanged and may not be replaced by the sizing seeds.

The sizing output contains **counts only**: event totals and the maximum
event count per replicate in every cell (count all generated events,
including those pending beyond day 280; identify pending counts separately),
and counts of scored-week I >= 11 steps and scoreable steps per cell in every
registered Lambda bin, including the [0,1) union and empty bins. Preserve the
separate zero-Lambda and unknown-history counts and insufficiency reasons.
Determine scoreability from the unchanged baseline's structural window,
known-count/history, min_cases and positive-Lambda guards, reproducing their
ordered sums without constructing a posterior or calling `estimate_series`.
Compute no CrI, coverage, posterior mean/R-hat, or error metric and inspect no
such values. Gamma quantiles used only for generator lag/mixture draws are
still allowed; posterior quantiles are not. Counts use exactly the main
study's scoreable-step definition, denominators and S1 pools.

Commit the complete sizing counts, per-cell event maxima, manifest, code SHA
and revised registration SHA in the step-2 results file **before** deciding
to launch the main run. Assess only the registered sample requirements:

- In **each** SI suite, at least three distinct scenarios from 2–5 must
  each have >= 200 scoreable steps in **both** [0,1) and [1,2), using their
  prespecified fixed k pools.
- **Every** S2 cell (scenario 1, k = infinity, each of the three R values,
  each before-series variant, each SI suite) must have >= 200 scoreable
  steps in [1,2).

If these counts fail either requirement, step 2 ends as
**inconclusive-by-design**: report all counts, do not run the main study under
this registration and do not implement the floor. An incomplete sizing run
likewise cannot authorize the main run; disclose its cell/seed and cause.
A redesigned study, including different imports, cluster sizes, sample size
or horizon, requires its own new registration and count-only sizing before
any inference. If sizing meets both requirements and completes, run the
main study with the original registered seeds unchanged. Its own sampling
requirements still apply independently; sizing success cannot substitute
for sufficient main-run samples. No adaptive top-up, early success stopping
or retuning is allowed in either stage. Outcome inspection is prohibited
during sizing; the main run computes only the registered metrics after the
sizing artifact is committed.

Sizing motivation is **arithmetic, not a simulation outcome** (#1683 E3):
I >= 11 with 0 < Lambda < 1 implies I/Lambda > 11, while 1 <= Lambda < 2
implies I/Lambda > 5.5. In the idealized correctly specified Poisson benchmark
with R <= 1.5 and Lambda < 2, mean R*Lambda < 3;
`P[Poisson(3) >= 11] = 0.000292336950647` (direct Poisson-tail sum).
Even 2000 × 34 scored weeks × that probability is only about 19.88 steps
before bin/history restrictions, far below 200. This is a loose benchmark,
not a bound or predicted count for the independent daily generator with
imports and weekly aggregation. It motivates the count-only check; the
actual counts, especially in the misspecified scenarios, remain unknown.

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

## Revision #1685 before any simulator (#1683 conformance check)

This revision addresses #1683's cross-family verdict **REVISE** (reviewer
session 5b4b5161), as reproduced in Issue #1685. It revises the registration
landed in full SHA `9deba0cd7b7eea766ee0827b22ffa9e7c3c57d8c` (#1679).
At revision time no simulator for this study had been written or run and
**no simulation outcome existed**. Only the explicitly labelled Poisson-tail
arithmetic above was calculated; no published outcomes beyond the #1672
trace were inspected. Land this revision before writing the simulator; the
results file must cite the full revised registration SHA on rolling.

Changes, and why:

1. **E1:** removed R = 3.0 from scenario 1 before any run for individual-level
   computational infeasibility and structural low-bin emptiness, preserving
   generator independence. Scenario 1 now has 18 cells and each SI suite 28;
   S2 now checks three R values. Required per-cell maximum event counts for
   the heavy but unchanged scenario 2 and all other cells in both stages.
2. **E2:** tightened S1's same-scenario/suite/pool [1,2) reference to >= 200
   scoreable steps and S2's per-cell [1,2) reference to >= 200. Smaller
   references are unevaluable/inconclusive, never numeric evidence for a
   pass or rejection; evaluable S2 coverage below 0.93 rejects once all
   sampling requirements are met.
3. **E3:** registered a count-only pre-step with the exact disjoint-per-scenario
   offset seed formula, all cells/bins and event counts, committed before
   main inference. Insufficient S1 or S2 sizing counts stop step 2 as
   inconclusive-by-design; any redesign needs its own registration and sizing.
   Main-run seeds, sample requirements and numeric criteria remain frozen.
   Updated stage-order and replicate/seed text to include this pre-step.
4. **Non-blocking notes:** scoped the dev-only simulator inside `koplik-epi`
   for access to existing private samplers, required independently checked
   lag mean/SD for both suites because gamma quantiles are shared, and
   clarified that S2 tests calibration in an unaffected bin rather than
   establishing benefit or no harm from the floor.

Every other frozen element remains unchanged: scenario 2–5 parameters,
initialization, event/observation/truth rules, SI suites and estimator,
main-run manifest order and seeds, candidate floor, bins, coverage/error
criteria, Monte Carlo uncertainty, limitations, and conditional
implementation/contract-v9 and report-only audit scope.

Reviewer should first check E1–E3: 18/28-cell expansion and unchanged main
seeds, both 200-step reference minima, and the count-only sizing stop/commit
order with no posterior/outcome inspection. Then check preserved generator
independence, scenario 2–5 parameters, W40 disclosure/R1 identity and
conditional v9 scope.
