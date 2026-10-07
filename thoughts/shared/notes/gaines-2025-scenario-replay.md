# Gaines County 2025 what-if scenario: replay against DSHS counts (#1455)

Status: measured once, on the fixture-built scenario, after the seeding rule was committed
(`wip(#1455): pre-registered seeding rule`, `crates/koplik-pipeline/src/scenario.rs`). Nothing in
the rule, the parameters or the seed was adjusted after this run. **Superseded: see the
correction at the end of this note; rule v1 below was withdrawn on first principles.**

Reproduce: `make pipeline-fixtures` then
`cargo run --release -p koplik-pipeline --example scenario_replay`.

What was run: Gaines County alone (FIPS 48165; population 23,956, Census Vintage 2025), baseline
kindergarten MMR coverage 81.97% (Texas DSHS 2023-24), seeded with the 107 confirmed cases the
earliest retained DSHS report (dated 2025-03-04, MMWR 2025-W10) gives for Gaines County, no
under-reporting inflation, initial exposed 0, `koplik_epi::default_parameters()` unchanged
(R0 uniform 12 to 18, latent 10 d, infectious 8 d, MMR 97% for two doses, no gravity), seed
20250304, 1,000 runs, 180-day horizon from 2025-03-02 (day 0).

## Measured output

```text
scenario: 1 nodes, start MMWR 2025-W10 (day 0 = 2025-03-02), seed 20250304, 1000 runs, initial infectious 107 in 48165
engine fingerprint (member 0): 3ff287fa47e8e4965025a18229f15231bfef459b665321ec68418ede330c147b

simulated cumulative infections (48165), median [50% band] [90% band]
 day       date   median                    50%                    90%
   0 2025-03-02      107        107-107                107-107        
   7 2025-03-09      334        310-362                282-395        
  14 2025-03-16      602        533-685                468-777        
  21 2025-03-23     1001        861-1172               729-1371       
  28 2025-03-30     1562       1310-1852              1091-2174       
  35 2025-04-06     2248       1890-2661              1548-3046       
  42 2025-04-13     2954       2515-3414              2072-3779       
  49 2025-04-20     3576       3115-3965              2621-4251       
  56 2025-04-27     4016       3586-4311              3119-4510       
  63 2025-05-04     4290       3952-4512              3519-4644       
  70 2025-05-11     4450       4180-4620              3834-4714       
  77 2025-05-18     4549       4332-4677              4054-4751       
  84 2025-05-25     4608       4428-4713              4202-4772       
  91 2025-06-01     4642       4489-4731              4300-4784       
  98 2025-06-08     4664       4526-4743              4362-4790       
 105 2025-06-15     4678       4553-4748              4408-4793       
 112 2025-06-22     4686       4570-4752              4434-4795       
 119 2025-06-29     4691       4580-4754              4455-4797       
 126 2025-07-06     4693       4585-4757              4465-4797       
 133 2025-07-13     4695       4589-4757              4476-4797       
 140 2025-07-20     4696       4592-4758              4483-4797       
 147 2025-07-27     4697       4595-4758              4485-4798       
 154 2025-08-03     4697       4597-4759              4487-4799       
 161 2025-08-10     4698       4597-4759              4489-4799       
 168 2025-08-17     4698       4597-4759              4491-4799       
 175 2025-08-24     4698       4598-4759              4492-4799       
 180 2025-08-29     4698       4599-4759              4492-4799       

observed: DSHS cumulative confirmed cases reported by each report date
    report  day confirmed  simulated cumulative infections at that day (median [90%])
2025-03-04    2       107  181 [162-202]
2025-03-25   23       226  1143 [824-1586]
2025-11-24  267       414  (after the 180-day horizon: not simulated)
2025-12-23  296       414  (after the 180-day horizon: not simulated)
2026-01-12  316       414  (after the 180-day horizon: not simulated)
```

## Reading it

* The simulated outbreak is far larger and faster than the reported one. By the 2025-03-25 report
  the simulation's median cumulative infections are 1,143 (90% band 824 to 1,586) against 226
  confirmed cases reported, and the simulation levels off near 4,700 infections (about 20% of the
  county) against 414 confirmed cases in the last three DSHS reports (all after the horizon).
* This is the measured result of the pre-registered, unfitted model. It is not a defect to be
  tuned away. Plausible reasons, none tested and none a basis for changing the scenario: the model
  mixes the whole county homogeneously (the real outbreak was concentrated in a close community),
  treats kindergarten coverage as the immunity of every resident, seeds *cumulative* confirmed
  cases as all currently infectious (no recovered start), counts every infection while DSHS counts
  confirmed cases, and ignores the interventions and behaviour change of 2025.
* Simulated cumulative infections and DSHS confirmed counts are different quantities. The
  comparison is an illustration of what this model does, not a score. The scored backtest is
  E5's, not this panel's.

## Correction (2026-10-07): rule v1 withdrawn; the scenario is now a hypothetical introduction

The text above is kept verbatim as the record of rule v1 and its replay. This section corrects
the method; it does not revise the measurement.

**The error.** Rule v1 seeded the 107 *cumulative* confirmed cases DSHS had recorded by
2025-03-04 as the number of people *currently infectious*. Those are different quantities: the
count includes everyone ever confirmed since the outbreak began, most of whom had already
recovered or would no longer transmit, while the model's infectious period is about 8 days. The
mistake was in the rule, not in the engine, and it was the manager's own default that this
worker implemented as written. The review of the landed SHA found it.

**The replay outcome had been seen.** The measured replay above was run and read before this
correction. The reason for withdrawing rule v1 is the first-principles argument in the paragraph
above, which does not depend on the replay's result (a seeding that matched the observed counts
would be equally invalid), and no parameter was changed in response to it. The new rule is
registered below before any scenario is built under it.

**Why no data-derived seeding of the 2025 outbreak is possible from what we hold.** Current
infectious prevalence at a start date needs the cases whose infectious window (about 8 days)
covers that date, that is, the new cases of the preceding days. The retained report vintages
with county tables are 2025-03-04, 2025-03-25, 2025-11-24, 2025-12-23 and 2026-01-12. The
closest pair, 2025-03-04 and 2025-03-25, is 21 days apart, so no pair of vintages spans one
infectious period, and spreading the difference between them over days would be imputation
(AGENTS.md rule 3). A historical replay is therefore "insufficient data" (rule 5).

**Decision.** The what-if panel publishes an explicit *hypothetical introduction* for Gaines
County: what could happen if one infectious person arrived, given the county's population and
kindergarten MMR coverage. It is not a reconstruction or forecast of the 2025 outbreak, and no
comparison with reported cases is shown or scored.

**Pre-registered rule v2** (registered in the commit that adds this section, before the
scenario was rebuilt or run under it; the full text is the module documentation of
`crates/koplik-pipeline/src/scenario.rs`):

| Input | Rule |
| --- | --- |
| Population, centroid, baseline coverage | Unchanged and sourced: Census Vintage 2025 `POPESTIMATE2025`, Census 2025 Gazetteer internal point, Texas DSHS 2023-24 kindergarten MMR coverage. |
| Initial infectious | **1**, a stated assumption (configurable in the pipeline); no source is claimed for it. |
| Initial exposed | **0**, a stated assumption. |
| Start week | The MMWR week containing July 1 of the population estimate's year (2025-W27), a neutral reference week stated as an assumption; the engine has no seasonality, so it only labels day 0. |
| Parameters | `koplik_epi::default_parameters()` unchanged, each cited. |
| Seed, runs | Seed 20250304 (unchanged, arbitrary, recorded), 1,000 runs. |
| Nodes | Gaines County alone (the cited defaults have no gravity coupling). |
| DSHS report vintages | Not an input. |

**Housekeeping.** The `scenario_replay` example that produced the rule v1 table above is removed:
it compared simulated infections with confirmed counts, which the panel no longer does. Rule v1's
output can be reproduced from commit `4e57c19` (`cargo run --release -p koplik-pipeline --example
scenario_replay` there). The new `scenario_report` example prints the new scenario's ensemble and
compares it with nothing.

### Measured output of the hypothetical introduction (rule v2)

Measured once on the fixture-built scenario, after the rule above was committed
(`wip(#1455): pre-registered hypothetical-introduction seeding (rule v2)`); no input, parameter or
seed changed afterwards. Reproduce: `make pipeline-fixtures`, then
`cargo run --release -p koplik-pipeline --example scenario_report`.

What was run: Gaines County alone (FIPS 48165; population 23,956, Census Vintage 2025), baseline
kindergarten MMR coverage 81.97% (Texas DSHS 2023-24), one infectious person introduced and nobody
exposed (stated assumptions), reference week 2025-W27 (day 0 = 2025-06-29), `koplik_epi::default_parameters()`
unchanged (R0 uniform 12 to 18, latent 10 d, infectious 8 d, MMR 97% for two doses, no gravity),
seed 20250304, 1,000 runs, 180-day horizon.

```text
scenario: 1 node(s), reference week MMWR 2025-W27 (day 0 = 2025-06-29), seed 20250304, 1000 runs, 1 infectious and 0 exposed introduced into 48165, population 23956
engine fingerprint (member 0): a319fe2d5fe7619077056606e7027de8a4ff097572e07e60f7a23e3323aa2b5f

simulated cumulative infections in 48165, median [50% band] [90% band]
 day       date   median                    50%                    90%
   0 2025-06-29        1          1-1                    1-1          
  14 2025-07-13        5          2-9                    1-16         
  28 2025-07-27       14          2-30                   1-67         
  42 2025-08-10       36          3-100                  1-238        
  56 2025-08-24      108          3-309                  1-716        
  70 2025-09-07      304          3-847                  1-1900       
  84 2025-09-21      784          3-1983                 1-3386       
  98 2025-10-05     1792          3-3381                 1-4298       
 112 2025-10-19     3060          3-4225                 1-4629       
 126 2025-11-02     3928          3-4550                 1-4730       
 140 2025-11-16     4332          3-4661                 1-4766       
 154 2025-11-30     4506          3-4706                 1-4779       
 168 2025-12-14     4578          3-4721                 1-4784       
 180 2025-12-26     4604          3-4729                 1-4787       

at day 180: 266 of 1000 runs ended with fewer than 10 infections beyond the introduced people; smallest 1, largest 4827, median 4604
```

Reading it, as measured and nothing more: the introduction fizzles out in 266 of the 1,000 runs
(fewer than 10 infections beyond the introduced person). In the others it grows to the
coverage-derived susceptible pool of 4,909 people (about 20% of the county; the largest run reaches 4,827), which is why
the median at day 180 (4,604) sits near the top of the range while the 50% band's lower end stays
at 3. The scenario starts from one assumed person, mixes the county homogeneously and treats
kindergarten coverage as every resident's immunity; it says what this simple model does under those
assumptions. It is not compared with reported cases, is not a forecast, and is not a fit.
