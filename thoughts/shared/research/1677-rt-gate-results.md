# #1677 R_t infectiousness gate: step 2 results (#1691)

## Outcome: INCONCLUSIVE (inconclusive-by-design at the registered sizing stage)

The registered count-only sizing stage completed (76,000 replicates, no failure, overflow or
refusal) and **did not meet the S1 sampling requirement in either SI suite**: in each suite
only scenario 3 has at least 200 scoreable steps in both `[0,1)` and `[1,2)` (needed: three of
scenarios 2-5). The S2 sizing requirement is met (all 12 cells >= 200 in `[1,inf)`, the
smallest being 222 for sensitivity R = 0.8, Unknown). Under the preregistration ("If these
counts fail either requirement, step 2 ends as **inconclusive-by-design** ... do not run the
main study under this registration and do not implement the floor") **the main evaluation was
not run**. No posterior, credible interval, coverage or error metric was computed or inspected
in this step. No result here supports implementing the floor, and no threshold, scenario, sample
size or seed was changed. A redesigned study needs its own registration and count-only sizing.

| item | value |
| --- | --- |
| Preregistration | `thoughts/shared/research/1677-rt-gate-preregistration.md` at full SHA `b5e499000f8175f3fda90511f5a2a06e951608da` (rolling merge of #1689) |
| Code SHA that produced the sizing counts | `59d56abf8258e700cb4b707bf52c185ae044eaba` (simulator `crates/koplik-epi/src/rt_gate_sim/`, driver `crates/koplik-epi/examples/rt_gate_sim.rs`) |
| Lag-test result (before sizing) | `1677-rt-gate-data/lagtest.txt` (both suites PASS, below) |
| Sizing counts commit | `eb8893bf8d52ca210d536b2684acfd52aa57afcb` (own commit, before this file) |
| Commands | `~/.rsi/bin/cargo-slot cargo build --release -p koplik-epi --example rt_gate_sim`; `~/.rsi/bin/cargo-slot target/release/examples/rt_gate_sim lagtest`; `~/.rsi/bin/cargo-slot target/release/examples/rt_gate_sim sizing --out thoughts/shared/research/1677-rt-gate-data` |
| Seeds | sizing `s*1_000_003 + 1_000_000 + r`, r = 0..1999, s = 1..5 (manifest order, seed reused across a scenario's cells and both suites) |
| Runtime | 1981.1 s wall-clock, 32 threads (first launch was killed by a daemon restart after about 4 minutes and restarted from scratch with identical arguments; nothing from it was kept) |
| Artifacts | `1677-rt-gate-data/sizing.md` (sha256 `45bfda56baa345dec5ce4a7ab510afd3657c0672af3037c309efc9eb9984a144`), `sizing-events.csv` (per-replicate event totals, sha256 `9682859de264013adf613319226d11fc2e6bb3a17d500ef7155faec534755d3e`), `sizing-decision.txt` (`NO`) |

Generator lag test (N = 1,000,000; seeds 9,000,001 and 9,000,002; mean within 4 SE, SD within
1%; oracle is the literal gamma moments, no `GammaDist` method):

```
primary: seed 9000001, N 1000000: sample mean 11.69651 (target 11.7, 4 SE = 0.01200, |diff| = 0.00349) PASS; sample SD 2.99656 (target 3, 1% = 0.03000, |diff| = 0.00344) PASS
sensitivity: seed 9000002, N 1000000: sample mean 14.00320 (target 14, 4 SE = 0.01600, |diff| = 0.00320) PASS; sample SD 4.00328 (target 4, 1% = 0.04000, |diff| = 0.00328) PASS
```

Also covered by unit tests in `rt_gate_sim/tests.rs` (`tools/cargo-test.sh -p koplik-epi`):
the lag test, the 38-item / 56-cell manifest and seed ranges, MMWR calendar alignment of the
280-day horizon, Lambda bin edges, determinism and event conservation, burst placement,
batch hold/release and thinning logic, restart-regime switching, and agreement of the
count-only classifier with `estimate_series` status.

## Implementation choices the preregistration left open (disclosed, frozen before any run)

- Draw order per day: background import count, restart-cluster count (scenario 2, days at or
  after the restart Sunday), background import times, one shared time per cluster, burst
  individuals' independent times; a zero-mean Poisson draw consumes no random numbers.
- Burst day is day `7*19 + 3 = 136` (Wednesday of study week 20). Event IDs are assigned in
  creation order; the seed event has ID 0 and counts as an event on day 0.
- Within-day uniform times are clamped strictly below the next integer day against rounding.
- Scenario 2 extinction is detected when the event queue is empty after a day is processed
  (the queue then holds only the initial chain, as scenario 2 has no pre-restart imports);
  the restart week is the study week after that day's week.
- Replicates run on worker threads with their own seeded RNG; outputs are merged
  sequentially in (item, replicate) order (no cross-thread float reduction).
- Scoreable = baseline-ok (I >= 11, complete history, Lambda > 0), computed with the same
  ordered sums as `estimate_series` without constructing a posterior.

## Observations from the counts (descriptive, not outcomes of the gate)

- Scenario 2 restarts early: extinction of the R = 0.6 chain was detected in 2000/2000
  replicates of every cell and the mean restart week is 2.9-4.0, so the series is
  dominated by the R = 1.5 phase; scored weeks fall mostly at high Lambda, and `[1,2)` holds
  only 32 (primary) and 44 (sensitivity) scoreable steps pooled over k.
- Scenario 5 (thinned at 0.3) yields almost no `I >= 11` weeks at all.
- Scenario 4 has 110 (primary) and 84 (sensitivity) scoreable `[0,1)` steps.
- Event counts at R = 1.5 are larger than the idealised arithmetic in the registration
  suggested (up to 4.6 million events in one replicate); all were completed within the
  200 million cap.

## Limitations (verbatim from R3)

> no correction for delays/right truncation, imports treated as local, constant-R windows, weekly aggregation approximation (Nash et al. 2023), simulation shows frequentist coverage not scientific adequacy, values of k are stress values.

## Sizing stage output (generated, unedited)

Runtime 1981.1 s wall-clock on 32 threads.

No replicate failed, overflowed or was refused.

### Maximum event counts per generated item

| item | replicates | max events (replicate r, seed) | mean events | max pending beyond day 280 | mean pending | mean observed cases (40 wk) | notes |
|---|---|---|---|---|---|---|---|
| s1/primary/R=0.8/k=inf | 2000 | 382 (r=1998, seed 2002001) | 90.9 | 22 | 3.3 | 87.6 |  |
| s1/primary/R=0.8/k=1 | 2000 | 567 (r=1308, seed 2001311) | 90.7 | 34 | 3.3 | 87.4 |  |
| s1/primary/R=0.8/k=0.3 | 2000 | 485 (r=1308, seed 2001311) | 90.3 | 46 | 3.3 | 87.0 |  |
| s1/primary/R=1/k=inf | 2000 | 1457 (r=1521, seed 2001524) | 295.0 | 129 | 21.4 | 273.6 |  |
| s1/primary/R=1/k=1 | 2000 | 2618 (r=558, seed 2000561) | 290.5 | 232 | 20.7 | 269.8 |  |
| s1/primary/R=1/k=0.3 | 2000 | 3185 (r=1308, seed 2001311) | 301.7 | 350 | 21.1 | 280.6 |  |
| s1/primary/R=1.5/k=inf | 2000 | 1044108 (r=1370, seed 2001373) | 210898.1 | 347277 | 70318.6 | 140579.5 |  |
| s1/primary/R=1.5/k=1 | 2000 | 2000268 (r=660, seed 2000663) | 212269.0 | 666238 | 70757.2 | 141511.8 |  |
| s1/primary/R=1.5/k=0.3 | 2000 | 4033074 (r=1854, seed 2001857) | 224957.2 | 1342830 | 74967.7 | 149989.4 |  |
| s2/primary/restart/k=inf | 2000 | 2979301 (r=421, seed 3000427) | 498671.3 | 993260 | 166295.3 | 332376.0 | extinction detected 2000/2000; restart within horizon 2000/2000 (mean restart week 3.60) |
| s2/primary/restart/k=1 | 2000 | 3565644 (r=244, seed 3000250) | 541851.5 | 1189060 | 180701.7 | 361149.8 | extinction detected 2000/2000; restart within horizon 2000/2000 (mean restart week 3.30) |
| s2/primary/restart/k=0.3 | 2000 | 4592510 (r=577, seed 3000583) | 570963.1 | 1535098 | 190369.6 | 380593.5 | extinction detected 2000/2000; restart within horizon 2000/2000 (mean restart week 2.87) |
| s3/primary/burst/k=inf | 2000 | 792 (r=1637, seed 4001646) | 249.1 | 28 | 7.6 | 241.5 |  |
| s3/primary/burst/k=1 | 2000 | 824 (r=516, seed 4000525) | 251.0 | 39 | 7.4 | 243.6 |  |
| s3/primary/burst/k=0.3 | 2000 | 853 (r=1637, seed 4001646) | 250.2 | 88 | 7.5 | 242.7 |  |
| s4/primary/batches/k=inf | 2000 | 1501 (r=1500, seed 5001512) | 289.9 | 121 | 20.3 | 268.4 | week-40 held carry in 188/2000 replicates, 2442 cases total |
| s4/primary/batches/k=1 | 2000 | 2018 (r=934, seed 5000946) | 294.3 | 218 | 20.5 | 272.6 | week-40 held carry in 180/2000 replicates, 2448 cases total |
| s4/primary/batches/k=0.3 | 2000 | 4083 (r=1972, seed 5001984) | 291.8 | 416 | 20.2 | 270.3 | week-40 held carry in 167/2000 replicates, 2504 cases total |
| s5/primary/thinned/k=1 | 2000 | 2149 (r=1919, seed 6001934) | 302.5 | 208 | 21.6 | 84.2 |  |
| s1/sensitivity/R=0.8/k=inf | 2000 | 352 (r=1006, seed 2001009) | 88.6 | 29 | 3.9 | 84.7 |  |
| s1/sensitivity/R=0.8/k=1 | 2000 | 500 (r=1370, seed 2001373) | 88.9 | 37 | 3.9 | 85.0 |  |
| s1/sensitivity/R=0.8/k=0.3 | 2000 | 695 (r=238, seed 2000241) | 86.5 | 74 | 3.8 | 82.6 |  |
| s1/sensitivity/R=1/k=inf | 2000 | 1268 (r=1998, seed 2002001) | 254.3 | 110 | 21.3 | 233.0 |  |
| s1/sensitivity/R=1/k=1 | 2000 | 1867 (r=1308, seed 2001311) | 254.3 | 196 | 20.7 | 233.6 |  |
| s1/sensitivity/R=1/k=0.3 | 2000 | 2207 (r=1960, seed 2001963) | 254.9 | 259 | 21.3 | 233.6 |  |
| s1/sensitivity/R=1.5/k=inf | 2000 | 231341 (r=940, seed 2000943) | 48786.0 | 77069 | 16282.5 | 32503.5 |  |
| s1/sensitivity/R=1.5/k=1 | 2000 | 447538 (r=660, seed 2000663) | 47870.4 | 148363 | 15968.2 | 31902.3 |  |
| s1/sensitivity/R=1.5/k=0.3 | 2000 | 734625 (r=139, seed 2000142) | 48691.4 | 243740 | 16232.1 | 32459.4 |  |
| s2/sensitivity/restart/k=inf | 2000 | 741847 (r=379, seed 3000385) | 125586.0 | 246753 | 41937.0 | 83649.1 | extinction detected 2000/2000; restart within horizon 2000/2000 (mean restart week 3.95) |
| s2/sensitivity/restart/k=1 | 2000 | 789281 (r=775, seed 3000781) | 133900.1 | 264130 | 44714.6 | 89185.5 | extinction detected 2000/2000; restart within horizon 2000/2000 (mean restart week 3.59) |
| s2/sensitivity/restart/k=0.3 | 2000 | 1000772 (r=1071, seed 3001077) | 145053.3 | 332911 | 48454.3 | 96599.1 | extinction detected 2000/2000; restart within horizon 2000/2000 (mean restart week 3.07) |
| s3/sensitivity/burst/k=inf | 2000 | 531 (r=1940, seed 4001949) | 242.4 | 54 | 9.2 | 233.3 |  |
| s3/sensitivity/burst/k=1 | 2000 | 617 (r=1637, seed 4001646) | 243.1 | 53 | 9.3 | 233.7 |  |
| s3/sensitivity/burst/k=0.3 | 2000 | 844 (r=1312, seed 4001321) | 243.6 | 80 | 9.2 | 234.4 |  |
| s4/sensitivity/batches/k=inf | 2000 | 1133 (r=317, seed 5000329) | 250.0 | 105 | 20.8 | 228.2 | week-40 held carry in 182/2000 replicates, 1930 cases total |
| s4/sensitivity/batches/k=1 | 2000 | 1691 (r=1776, seed 5001788) | 254.1 | 148 | 21.0 | 232.1 | week-40 held carry in 174/2000 replicates, 1974 cases total |
| s4/sensitivity/batches/k=0.3 | 2000 | 3160 (r=1500, seed 5001512) | 255.4 | 290 | 21.4 | 233.1 | week-40 held carry in 157/2000 replicates, 1710 cases total |
| s5/sensitivity/thinned/k=1 | 2000 | 1788 (r=528, seed 6000543) | 256.5 | 199 | 21.4 | 70.6 |  |

### Count tables per analysis cell

Count-gate steps (scored weeks 7-40 with I >= 11) and baseline-scoreable steps (Lambda > 0, complete history), by Lambda bin. `unknown history` = count-gate steps whose eight-week look-back reaches before the series (Lambda unknown, unbinned); `Lambda=0` = count-gate steps with known history and Lambda = 0 (inside bin [0,0.25), not scoreable).

| cell | scored steps | I>=11 | unknown history | Lambda=0 | I>=11 in [0,0.25) | I>=11 in [0.25,0.5) | I>=11 in [0.5,1) | I>=11 in [1,2) | I>=11 in [2,5) | I>=11 in [5,inf) | scoreable [0,0.25) | scoreable [0.25,0.5) | scoreable [0.5,1) | scoreable [1,2) | scoreable [2,5) | scoreable [5,inf) | scoreable [0,1) | scoreable [1,inf) | baseline-ok | reason MissingCount | reason BelowThreshold | reason NoInfectivity |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| s1/primary/R=0.8/k=inf/Unknown | 68000 | 460 | 5 | 0 | 0 | 0 | 0 | 0 | 33 | 422 | 0 | 0 | 0 | 0 | 33 | 422 | 0 | 455 | 455 | 4000 | 63545 | 0 |
| s1/primary/R=0.8/k=inf/Zero | 68000 | 460 | 0 | 0 | 0 | 0 | 0 | 0 | 34 | 426 | 0 | 0 | 0 | 0 | 34 | 426 | 0 | 460 | 460 | 0 | 67540 | 0 |
| s1/primary/R=0.8/k=1/Unknown | 68000 | 1144 | 28 | 0 | 0 | 0 | 0 | 3 | 85 | 1028 | 0 | 0 | 0 | 3 | 85 | 1028 | 0 | 1116 | 1116 | 4000 | 62884 | 0 |
| s1/primary/R=0.8/k=1/Zero | 68000 | 1144 | 0 | 0 | 0 | 0 | 0 | 3 | 88 | 1053 | 0 | 0 | 0 | 3 | 88 | 1053 | 0 | 1144 | 1144 | 0 | 66856 | 0 |
| s1/primary/R=0.8/k=0.3/Unknown | 68000 | 2480 | 67 | 0 | 0 | 2 | 13 | 33 | 251 | 2114 | 0 | 2 | 13 | 33 | 251 | 2114 | 15 | 2398 | 2413 | 4000 | 61587 | 0 |
| s1/primary/R=0.8/k=0.3/Zero | 68000 | 2480 | 0 | 0 | 0 | 2 | 14 | 35 | 263 | 2166 | 0 | 2 | 14 | 35 | 263 | 2166 | 16 | 2464 | 2480 | 0 | 65520 | 0 |
| s1/primary/R=1/k=inf/Unknown | 68000 | 16971 | 59 | 0 | 0 | 0 | 0 | 1 | 122 | 16789 | 0 | 0 | 0 | 1 | 122 | 16789 | 0 | 16912 | 16912 | 4000 | 47088 | 0 |
| s1/primary/R=1/k=inf/Zero | 68000 | 16971 | 0 | 0 | 0 | 0 | 0 | 2 | 131 | 16838 | 0 | 0 | 0 | 2 | 131 | 16838 | 0 | 16971 | 16971 | 0 | 51029 | 0 |
| s1/primary/R=1/k=1/Unknown | 68000 | 16213 | 165 | 0 | 0 | 1 | 0 | 10 | 266 | 15771 | 0 | 1 | 0 | 10 | 266 | 15771 | 1 | 16047 | 16048 | 4000 | 47952 | 0 |
| s1/primary/R=1/k=1/Zero | 68000 | 16213 | 0 | 0 | 0 | 1 | 0 | 10 | 290 | 15912 | 0 | 1 | 0 | 10 | 290 | 15912 | 1 | 16212 | 16213 | 0 | 51787 | 0 |
| s1/primary/R=1/k=0.3/Unknown | 68000 | 14962 | 298 | 0 | 0 | 4 | 12 | 46 | 476 | 14126 | 0 | 4 | 12 | 46 | 476 | 14126 | 16 | 14648 | 14664 | 4000 | 49336 | 0 |
| s1/primary/R=1/k=0.3/Zero | 68000 | 14962 | 0 | 0 | 0 | 4 | 13 | 49 | 510 | 14386 | 0 | 4 | 13 | 49 | 510 | 14386 | 17 | 14945 | 14962 | 0 | 53038 | 0 |
| s1/primary/R=1.5/k=inf/Unknown | 68000 | 61808 | 1431 | 0 | 0 | 0 | 0 | 0 | 98 | 60279 | 0 | 0 | 0 | 0 | 98 | 60279 | 0 | 60377 | 60377 | 4000 | 3623 | 0 |
| s1/primary/R=1.5/k=inf/Zero | 68000 | 61808 | 0 | 0 | 0 | 0 | 0 | 0 | 192 | 61616 | 0 | 0 | 0 | 0 | 192 | 61616 | 0 | 61808 | 61808 | 0 | 6192 | 0 |
| s1/primary/R=1.5/k=1/Unknown | 68000 | 57746 | 1327 | 0 | 0 | 0 | 2 | 8 | 245 | 56164 | 0 | 0 | 2 | 8 | 245 | 56164 | 2 | 56417 | 56419 | 4000 | 7581 | 0 |
| s1/primary/R=1.5/k=1/Zero | 68000 | 57746 | 0 | 0 | 0 | 0 | 2 | 13 | 326 | 57405 | 0 | 0 | 2 | 13 | 326 | 57405 | 2 | 57744 | 57746 | 0 | 10254 | 0 |
| s1/primary/R=1.5/k=0.3/Unknown | 68000 | 49192 | 1107 | 0 | 0 | 7 | 12 | 56 | 460 | 47550 | 0 | 7 | 12 | 56 | 460 | 47550 | 19 | 48066 | 48085 | 4000 | 15915 | 0 |
| s1/primary/R=1.5/k=0.3/Zero | 68000 | 49192 | 0 | 0 | 0 | 8 | 13 | 65 | 530 | 48576 | 0 | 8 | 13 | 65 | 530 | 48576 | 21 | 49171 | 49192 | 0 | 18808 | 0 |
| s2/primary/restart/k=inf/Unknown | 68000 | 61896 | 2201 | 413 | 684 | 6 | 33 | 3 | 1 | 58968 | 271 | 6 | 33 | 3 | 1 | 58968 | 310 | 58972 | 59282 | 4000 | 4305 | 413 |
| s2/primary/restart/k=1/Unknown | 68000 | 61962 | 2233 | 418 | 656 | 5 | 16 | 7 | 15 | 59030 | 238 | 5 | 16 | 7 | 15 | 59030 | 259 | 59052 | 59311 | 4000 | 4271 | 418 |
| s2/primary/restart/k=0.3/Unknown | 68000 | 61711 | 2196 | 424 | 623 | 1 | 16 | 22 | 72 | 58781 | 199 | 1 | 16 | 22 | 72 | 58781 | 216 | 58875 | 59091 | 4000 | 4485 | 424 |
| s3/primary/burst/k=inf/Unknown | 68000 | 12076 | 38 | 0 | 17 | 11 | 59 | 203 | 1070 | 10678 | 17 | 11 | 59 | 203 | 1070 | 10678 | 87 | 11951 | 12038 | 4000 | 51962 | 0 |
| s3/primary/burst/k=1/Unknown | 68000 | 13819 | 109 | 0 | 32 | 31 | 95 | 246 | 1071 | 12235 | 32 | 31 | 95 | 246 | 1071 | 12235 | 158 | 13552 | 13710 | 4000 | 50290 | 0 |
| s3/primary/burst/k=0.3/Unknown | 68000 | 14817 | 224 | 0 | 68 | 56 | 157 | 413 | 1102 | 12797 | 68 | 56 | 157 | 413 | 1102 | 12797 | 281 | 14312 | 14593 | 4000 | 49407 | 0 |
| s4/primary/batches/k=inf/Unknown | 68000 | 17016 | 95 | 0 | 5 | 0 | 10 | 135 | 1173 | 15598 | 5 | 0 | 10 | 135 | 1173 | 15598 | 15 | 16906 | 16921 | 4000 | 47079 | 0 |
| s4/primary/batches/k=1/Unknown | 68000 | 16553 | 183 | 0 | 4 | 1 | 28 | 141 | 1041 | 15155 | 4 | 1 | 28 | 141 | 1041 | 15155 | 33 | 16337 | 16370 | 4000 | 47630 | 0 |
| s4/primary/batches/k=0.3/Unknown | 68000 | 13904 | 276 | 0 | 5 | 4 | 53 | 186 | 877 | 12503 | 5 | 4 | 53 | 186 | 877 | 12503 | 62 | 13566 | 13628 | 4000 | 50372 | 0 |
| s5/primary/thinned/k=1/Unknown | 68000 | 2274 | 1 | 0 | 0 | 0 | 0 | 3 | 122 | 2148 | 0 | 0 | 0 | 3 | 122 | 2148 | 0 | 2273 | 2273 | 4000 | 61727 | 0 |
| s1/sensitivity/R=0.8/k=inf/Unknown | 68000 | 225 | 3 | 0 | 0 | 0 | 0 | 0 | 23 | 199 | 0 | 0 | 0 | 0 | 23 | 199 | 0 | 222 | 222 | 4000 | 63778 | 0 |
| s1/sensitivity/R=0.8/k=inf/Zero | 68000 | 225 | 0 | 0 | 0 | 0 | 0 | 0 | 23 | 202 | 0 | 0 | 0 | 0 | 23 | 202 | 0 | 225 | 225 | 0 | 67775 | 0 |
| s1/sensitivity/R=0.8/k=1/Unknown | 68000 | 896 | 15 | 0 | 0 | 0 | 0 | 0 | 80 | 801 | 0 | 0 | 0 | 0 | 80 | 801 | 0 | 881 | 881 | 4000 | 63119 | 0 |
| s1/sensitivity/R=0.8/k=1/Zero | 68000 | 896 | 0 | 0 | 0 | 0 | 0 | 0 | 82 | 814 | 0 | 0 | 0 | 0 | 82 | 814 | 0 | 896 | 896 | 0 | 67104 | 0 |
| s1/sensitivity/R=0.8/k=0.3/Unknown | 68000 | 1915 | 44 | 0 | 0 | 0 | 5 | 19 | 187 | 1660 | 0 | 0 | 5 | 19 | 187 | 1660 | 5 | 1866 | 1871 | 4000 | 62129 | 0 |
| s1/sensitivity/R=0.8/k=0.3/Zero | 68000 | 1915 | 0 | 0 | 0 | 0 | 6 | 20 | 196 | 1693 | 0 | 0 | 6 | 20 | 196 | 1693 | 6 | 1909 | 1915 | 0 | 66085 | 0 |
| s1/sensitivity/R=1/k=inf/Unknown | 68000 | 13328 | 22 | 0 | 0 | 0 | 0 | 0 | 125 | 13181 | 0 | 0 | 0 | 0 | 125 | 13181 | 0 | 13306 | 13306 | 4000 | 50694 | 0 |
| s1/sensitivity/R=1/k=inf/Zero | 68000 | 13328 | 0 | 0 | 0 | 0 | 0 | 0 | 127 | 13201 | 0 | 0 | 0 | 0 | 127 | 13201 | 0 | 13328 | 13328 | 0 | 54672 | 0 |
| s1/sensitivity/R=1/k=1/Unknown | 68000 | 13741 | 86 | 0 | 0 | 0 | 0 | 6 | 209 | 13440 | 0 | 0 | 0 | 6 | 209 | 13440 | 0 | 13655 | 13655 | 4000 | 50345 | 0 |
| s1/sensitivity/R=1/k=1/Zero | 68000 | 13741 | 0 | 0 | 0 | 0 | 0 | 6 | 220 | 13515 | 0 | 0 | 0 | 6 | 220 | 13515 | 0 | 13741 | 13741 | 0 | 54259 | 0 |
| s1/sensitivity/R=1/k=0.3/Unknown | 68000 | 13203 | 175 | 0 | 0 | 0 | 8 | 25 | 383 | 12612 | 0 | 0 | 8 | 25 | 383 | 12612 | 8 | 13020 | 13028 | 4000 | 50972 | 0 |
| s1/sensitivity/R=1/k=0.3/Zero | 68000 | 13203 | 0 | 0 | 0 | 0 | 10 | 25 | 408 | 12760 | 0 | 0 | 10 | 25 | 408 | 12760 | 10 | 13193 | 13203 | 0 | 54797 | 0 |
| s1/sensitivity/R=1.5/k=inf/Unknown | 68000 | 58738 | 715 | 0 | 0 | 0 | 0 | 0 | 116 | 57907 | 0 | 0 | 0 | 0 | 116 | 57907 | 0 | 58023 | 58023 | 4000 | 5977 | 0 |
| s1/sensitivity/R=1.5/k=inf/Zero | 68000 | 58738 | 0 | 0 | 0 | 0 | 0 | 0 | 170 | 58568 | 0 | 0 | 0 | 0 | 170 | 58568 | 0 | 58738 | 58738 | 0 | 9262 | 0 |
| s1/sensitivity/R=1.5/k=1/Unknown | 68000 | 54709 | 817 | 0 | 0 | 0 | 0 | 5 | 224 | 53663 | 0 | 0 | 0 | 5 | 224 | 53663 | 0 | 53892 | 53892 | 4000 | 10108 | 0 |
| s1/sensitivity/R=1.5/k=1/Zero | 68000 | 54709 | 0 | 0 | 0 | 0 | 1 | 5 | 288 | 54415 | 0 | 0 | 1 | 5 | 288 | 54415 | 1 | 54708 | 54709 | 0 | 13291 | 0 |
| s1/sensitivity/R=1.5/k=0.3/Unknown | 68000 | 46246 | 799 | 0 | 0 | 0 | 8 | 43 | 451 | 44945 | 0 | 0 | 8 | 43 | 451 | 44945 | 8 | 45439 | 45447 | 4000 | 18553 | 0 |
| s1/sensitivity/R=1.5/k=0.3/Zero | 68000 | 46246 | 0 | 0 | 0 | 0 | 8 | 49 | 516 | 45673 | 0 | 0 | 8 | 49 | 516 | 45673 | 8 | 46238 | 46246 | 0 | 21754 | 0 |
| s2/sensitivity/restart/k=inf/Unknown | 68000 | 60612 | 1932 | 413 | 714 | 13 | 42 | 3 | 28 | 57880 | 301 | 13 | 42 | 3 | 28 | 57880 | 356 | 57911 | 58267 | 4000 | 5320 | 413 |
| s2/sensitivity/restart/k=1/Unknown | 68000 | 60751 | 1997 | 418 | 678 | 9 | 24 | 6 | 53 | 57984 | 260 | 9 | 24 | 6 | 53 | 57984 | 293 | 58043 | 58336 | 4000 | 5246 | 418 |
| s2/sensitivity/restart/k=0.3/Unknown | 68000 | 60653 | 1964 | 423 | 634 | 3 | 21 | 35 | 121 | 57875 | 211 | 3 | 21 | 35 | 121 | 57875 | 235 | 58031 | 58266 | 4000 | 5311 | 423 |
| s3/sensitivity/burst/k=inf/Unknown | 68000 | 10792 | 16 | 0 | 7 | 9 | 43 | 214 | 1097 | 9406 | 7 | 9 | 43 | 214 | 1097 | 9406 | 59 | 10717 | 10776 | 4000 | 53224 | 0 |
| s3/sensitivity/burst/k=1/Unknown | 68000 | 12365 | 55 | 0 | 20 | 17 | 95 | 290 | 1089 | 10799 | 20 | 17 | 95 | 290 | 1089 | 10799 | 132 | 12178 | 12310 | 4000 | 51690 | 0 |
| s3/sensitivity/burst/k=0.3/Unknown | 68000 | 13719 | 126 | 0 | 68 | 32 | 180 | 378 | 1069 | 11866 | 68 | 32 | 180 | 378 | 1069 | 11866 | 280 | 13313 | 13593 | 4000 | 50407 | 0 |
| s4/sensitivity/batches/k=inf/Unknown | 68000 | 13407 | 38 | 0 | 1 | 1 | 15 | 116 | 1251 | 11985 | 1 | 1 | 15 | 116 | 1251 | 11985 | 17 | 13352 | 13369 | 4000 | 50631 | 0 |
| s4/sensitivity/batches/k=1/Unknown | 68000 | 13689 | 104 | 0 | 1 | 2 | 22 | 114 | 1034 | 12412 | 1 | 2 | 22 | 114 | 1034 | 12412 | 25 | 13560 | 13585 | 4000 | 50415 | 0 |
| s4/sensitivity/batches/k=0.3/Unknown | 68000 | 13009 | 185 | 0 | 8 | 4 | 30 | 126 | 875 | 11781 | 8 | 4 | 30 | 126 | 875 | 11781 | 42 | 12782 | 12824 | 4000 | 51176 | 0 |
| s5/sensitivity/thinned/k=1/Unknown | 68000 | 1203 | 1 | 0 | 0 | 0 | 1 | 1 | 103 | 1097 | 0 | 0 | 1 | 1 | 103 | 1097 | 1 | 1201 | 1202 | 4000 | 62798 | 0 |

### Pools

S1 pools (scenarios 2-5, all k cells, Unknown): scoreable steps

| suite | scenario | scoreable [0,1) | scoreable [1,2) | both >= 200 |
|---|---|---|---|---|
| primary | 2 | 785 | 32 | false |
| primary | 3 | 526 | 862 | true |
| primary | 4 | 110 | 462 | false |
| primary | 5 | 0 | 3 | false |
| sensitivity | 2 | 884 | 44 | false |
| sensitivity | 3 | 471 | 882 | true |
| sensitivity | 4 | 84 | 356 | false |
| sensitivity | 5 | 1 | 1 | false |

S2 cells (scenario 1, k = inf): scoreable steps in [1,inf)

| suite | R | before-series | scoreable [1,inf) | >= 200 |
|---|---|---|---|---|
| primary | 0.8 | Unknown | 455 | true |
| primary | 0.8 | Zero | 460 | true |
| primary | 1 | Unknown | 16912 | true |
| primary | 1 | Zero | 16971 | true |
| primary | 1.5 | Unknown | 60377 | true |
| primary | 1.5 | Zero | 61808 | true |
| sensitivity | 0.8 | Unknown | 222 | true |
| sensitivity | 0.8 | Zero | 225 | true |
| sensitivity | 1 | Unknown | 13306 | true |
| sensitivity | 1 | Zero | 13328 | true |
| sensitivity | 1.5 | Unknown | 58023 | true |
| sensitivity | 1.5 | Zero | 58738 | true |

### Sizing decision

S1 sizing (>= 3 of scenarios 2-5 with >= 200 scoreable steps in both [0,1) and [1,2)): primary NOT met, sensitivity NOT met. S2 sizing (every scenario-1 k=inf cell >= 200 scoreable steps in [1,inf)): met. Run complete: true. Main run authorised by the registered sizing rule: **NO (inconclusive-by-design)**.
