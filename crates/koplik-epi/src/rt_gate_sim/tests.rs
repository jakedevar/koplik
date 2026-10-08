use chrono::{Duration, NaiveDate};
use koplik_contracts::v1::{MmwrWeek, RtStatus};
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use super::design::*;
use super::generator::*;
use crate::rt::{BeforeSeries, estimate_series};

fn spec(r: f64, k: Option<f64>, bg: f64) -> GenSpec {
    GenSpec {
        si: PRIMARY_SI,
        k,
        regime: Regime::Constant(r),
        background_per_week: bg,
        burst: None,
    }
}

/// Preregistered generator-lag test (N = 1e6, seeds 9_000_001 / 9_000_002, mean within 4 SE,
/// SD within 1%) against literal gamma moments 11.7/3.0 and 14/4. A failure is a generator
/// bug, never a tolerance to widen.
#[test]
fn generator_lag_moments_match_both_suites() {
    for si in [PRIMARY_SI, SENSITIVITY_SI] {
        match lag_test(&si) {
            Ok(r) => eprintln!("{r}"),
            Err(r) => panic!("lag test failed: {r}"),
        }
    }
}

#[test]
fn manifest_has_preregistered_cells_and_seeds() {
    let items = manifest();
    assert_eq!(items.len(), 38);
    let cells: usize = items.iter().map(|i| i.variants.len()).sum();
    assert_eq!(cells, 56);
    for suite in ["primary", "sensitivity"] {
        let c: usize = items
            .iter()
            .filter(|i| i.si.name == suite)
            .map(|i| i.variants.len())
            .sum();
        assert_eq!(c, 28);
    }
    // Table order: first item is s1 R=0.8 k=inf, then k=1, k=0.3, then R=1.0.
    assert_eq!(items[0].label, "s1/primary/R=0.8/k=inf");
    assert_eq!(items[1].spec.k, Some(1.0));
    assert_eq!(items[2].spec.k, Some(0.3));
    assert_eq!(items[3].label, "s1/primary/R=1/k=inf");
    assert_eq!(items[19].label, "s1/sensitivity/R=0.8/k=inf");
    // Registered seed ranges.
    assert_eq!(seed(1, Stage::Main, 0), 1_000_003);
    assert_eq!(seed(1, Stage::Main, 1999), 1_002_002);
    assert_eq!(seed(5, Stage::Main, 0), 5_000_015);
    assert_eq!(seed(5, Stage::Main, 1999), 5_002_014);
    assert_eq!(seed(1, Stage::Sizing, 0), 2_000_003);
    assert_eq!(seed(1, Stage::Sizing, 1999), 2_002_002);
    assert_eq!(seed(5, Stage::Sizing, 0), 6_000_015);
    assert_eq!(seed(5, Stage::Sizing, 1999), 6_002_014);
}

#[test]
fn calendar_weeks_are_consecutive_mmwr_weeks_from_the_anchor() {
    let day0 = NaiveDate::from_ymd_opt(2025, 1, 5).unwrap();
    let first = MmwrWeek::from_date(day0).unwrap();
    assert_eq!(
        day0,
        first.start_date(),
        "day 0 is the Sunday that starts a week"
    );
    let mut prev = first;
    for d in 1..DAYS as i64 {
        let w = MmwrWeek::from_date(day0 + Duration::days(d)).unwrap();
        if d % 7 == 0 {
            assert_eq!(w.start_date(), day0 + Duration::days(d));
            assert_eq!(w.start_date() - prev.start_date(), Duration::days(7));
            prev = w;
        } else {
            assert_eq!(w, prev);
        }
    }
}

#[test]
fn bins_put_equality_in_the_right_hand_bin() {
    assert_eq!(bin_of(0.0), 0);
    assert_eq!(bin_of(0.2499), 0);
    assert_eq!(bin_of(0.25), 1);
    assert_eq!(bin_of(0.5), 2);
    assert_eq!(bin_of(0.999), 2);
    assert_eq!(bin_of(1.0), 3);
    assert_eq!(bin_of(1.999), 3);
    assert_eq!(bin_of(2.0), 4);
    assert_eq!(bin_of(5.0), 5);
    assert_eq!(bin_of(1e9), 5);
}

#[test]
fn generator_is_deterministic_and_conserves_events() {
    let s = spec(1.0, Some(0.3), 0.5);
    let a = generate_latent(&s, 12345).unwrap();
    let b = generate_latent(&s, 12345).unwrap();
    let c = generate_latent(&s, 12346).unwrap();
    assert_eq!(a.daily, b.daily);
    assert_eq!(a.total_events, b.total_events);
    assert_ne!(a.daily, c.daily);
    let processed: u64 = a.daily.iter().map(|&x| u64::from(x)).sum();
    assert_eq!(a.total_events, processed + a.pending_events);
}

#[test]
fn no_transmission_leaves_only_the_seed_imports_and_burst() {
    // R = 0: no descendants. Seed at 0.5 plus a burst of 15 on day 136 exactly.
    let mut s = spec(0.0, None, 0.0);
    s.burst = Some(Burst { day: 136, size: 15 });
    let l = generate_latent(&s, 7).unwrap();
    assert_eq!(l.daily[0], 1);
    assert_eq!(l.daily[136], 15);
    assert_eq!(l.total_events, 16);
    assert_eq!(l.pending_events, 0);
    let weekly = observe(l, Observation::Complete).unwrap();
    assert_eq!(weekly.counts[0], 1);
    assert_eq!(weekly.counts[19], 15, "Wednesday of study week 20");
    assert_eq!(weekly.counts.iter().map(|&c| u64::from(c)).sum::<u64>(), 16);
}

fn crafted(daily_week: &[u32], seed: u64) -> Latent {
    let mut daily = vec![0u32; DAYS];
    for (w, &c) in daily_week.iter().enumerate() {
        daily[7 * w] = c;
    }
    Latent {
        daily,
        total_events: 0,
        pending_events: 0,
        extinction_day: None,
        restart_week: None,
        truth: vec![1.0; WEEKS],
        rng: ChaCha8Rng::seed_from_u64(seed),
    }
}

#[test]
fn batches_hold_and_release_next_week() {
    let weeks: Vec<u32> = (1..=40).collect();
    // Hold probability 1: every week is held; reports are the previous week's count.
    let s = observe(
        crafted(&weeks, 1),
        Observation::Batches {
            hold_probability: 1.0,
        },
    )
    .unwrap();
    assert_eq!(s.counts[0], 0);
    for w in 1..WEEKS {
        assert_eq!(s.counts[w], weeks[w - 1], "week {w}");
    }
    assert_eq!(
        s.week40_held_carry, 40,
        "week-40 cases are carried beyond the horizon"
    );
    // Hold probability 0: complete reporting, no carry.
    let s = observe(
        crafted(&weeks, 1),
        Observation::Batches {
            hold_probability: 0.0,
        },
    )
    .unwrap();
    assert_eq!(s.counts, weeks);
    assert_eq!(s.week40_held_carry, 0);
}

#[test]
fn thinning_retains_by_binomial_and_does_not_touch_the_latent_truth() {
    let weeks: Vec<u32> = vec![100; 40];
    let all = observe(
        crafted(&weeks, 1),
        Observation::Thinned {
            retain_probability: 1.0,
        },
    )
    .unwrap();
    assert_eq!(all.counts, weeks);
    let none = observe(
        crafted(&weeks, 1),
        Observation::Thinned {
            retain_probability: 0.0,
        },
    )
    .unwrap();
    assert!(none.counts.iter().all(|&c| c == 0));
    let some = observe(
        crafted(&weeks, 1),
        Observation::Thinned {
            retain_probability: 0.3,
        },
    )
    .unwrap();
    let mean = some.counts.iter().map(|&c| f64::from(c)).sum::<f64>() / 40.0;
    assert!((mean - 30.0).abs() < 5.0, "mean {mean}");
}

#[test]
fn restart_regime_switches_on_the_sunday_after_extinction() {
    let regime = Regime::ExtinctionRestart {
        r_before: 0.6,
        r_after: 0.9, // lighter than the study's 1.5 so the debug-build test stays fast
        cluster_per_week: 0.2,
        cluster_size: 3,
    };
    let s = GenSpec {
        si: PRIMARY_SI,
        k: Some(0.3),
        regime,
        background_per_week: 0.0,
        burst: None,
    };
    let mut checked = 0;
    for sd in 0..200u64 {
        let l = generate_latent(&s, sd).unwrap();
        let Some(d) = l.extinction_day else {
            assert!(l.restart_week.is_none());
            assert!(l.truth.iter().all(|&r| r == 0.6));
            continue;
        };
        let sw = l.restart_week.unwrap();
        assert_eq!(sw, d / 7 + 1);
        for w in 0..WEEKS as u32 {
            let want = if w >= sw { 0.9 } else { 0.6 };
            assert_eq!(l.truth[w as usize], want);
        }
        // Nothing happens between extinction and the restart Sunday.
        for day in (d + 1)..(7 * sw).min(DAYS as u32) {
            assert_eq!(l.daily[day as usize], 0);
        }
        checked += 1;
    }
    assert!(
        checked > 10,
        "expected extinctions at k = 0.3, got {checked}"
    );
}

#[test]
fn classification_agrees_with_estimate_series_status() {
    let (si, cfg) = estimator();
    for (sdx, r) in [(1u64, 1.0), (2, 1.2), (3, 0.9)] {
        let l =
            generate_series(&spec(r, Some(1.0), 0.5), Observation::Complete, 500 + sdx).unwrap();
        for before in [BeforeSeries::Unknown, BeforeSeries::Zero] {
            let infos = classify(&l.counts, si.weights(), before, cfg.min_cases);
            let mut c2 = cfg.clone();
            c2.before_series = before;
            let opt: Vec<Option<u32>> = l.counts.iter().map(|&c| Some(c)).collect();
            let est = estimate_series(&opt, &si, &c2).unwrap();
            for t in 0..WEEKS {
                assert_eq!(
                    est[t].status == RtStatus::Ok,
                    infos[t].class == StepClass::Ok,
                    "step {t} {before:?}"
                );
            }
        }
    }
}

/// Off-study seeds for plumbing tests (review #1699): above every registered seed (main
/// `s*1_000_003 + r`, sizing +1_000_000, so at most 6_002_014) and below the lag-test seeds
/// (9_000_001, 9_000_002). Tests never run a registered main-stage seed.
const PLUMBING_SEED_BASE: u64 = 7_000_000;

#[test]
fn plumbing_seeds_are_off_study() {
    let last = PLUMBING_SEED_BASE + 100 * 64 + 12;
    for s in 1..=5u8 {
        for stage in [Stage::Main, Stage::Sizing] {
            assert!(seed(s, stage, 1999) < PLUMBING_SEED_BASE);
        }
    }
    assert!(last < 9_000_001);
}

/// The test build refuses to run registered main-stage replicates at all.
#[test]
#[should_panic(expected = "tests must not run registered main-stage replicates")]
fn registered_main_replicates_are_refused_under_test() {
    let items = manifest();
    let _ = super::run::run_rep(&items[0], Stage::Main, 0);
}

/// End-to-end plumbing on cheap cells and a few replicates with off-study seeds: both stages
/// aggregate, the estimator agrees with the count-only classification, every report renders.
#[test]
fn stages_aggregate_and_render_on_cheap_cells() {
    use super::agg::{aggregate, decide};
    use super::report::*;
    use super::run::run_rep_seeded;
    let all = manifest();
    let items: Vec<GenItem> = all
        .into_iter()
        .filter(|i| {
            i.si.name == "primary"
                && (i.scenario >= 3
                    || (i.scenario == 1 && i.r_label == Some(1.0) && i.spec.k.is_none()))
        })
        .collect();
    assert!(items.len() >= 5);
    for stage in [Stage::Sizing, Stage::Main] {
        let mut slots: Vec<Option<super::run::RepOut>> = vec![None; items.len() * 2000];
        for (i, it) in items.iter().enumerate() {
            for r in 0..12u32 {
                let sd = PLUMBING_SEED_BASE + (i as u64) * 100 + u64::from(r);
                slots[i * 2000 + r as usize] = Some(run_rep_seeded(it, stage, sd).unwrap());
            }
        }
        let agg = aggregate(&items, &slots);
        let _ = events_table(&items, &agg, stage);
        let _ = count_tables(&items, &agg);
        let _ = s1_s2_count_tables(&items, &agg);
        let _ = sizing_decision(&items, &agg, &[]);
        if stage == Stage::Main {
            let _ = cell_summary_table(&items, &agg);
            let _ = per_cell_metric_tables(&items, &agg);
            let _ = pooled_metric_tables(&items, &agg);
            let _ = verdict_section(&items, &agg, &[]);
            let (o, _) = decide(&items, &agg, true);
            assert_eq!(
                o,
                super::agg::Outcome::Inconclusive,
                "12 replicates cannot reach 200 steps"
            );
            // The sizing-style counts equal the main-stage counts structure (same classifier).
            let c = &agg.cells[0];
            assert_eq!(c.counts.scored_steps, 12 * 34);
            let scoreable: u64 = c.counts.bin_scoreable.iter().sum();
            assert_eq!(scoreable, c.counts.baseline_ok);
            let recs: u64 = c
                .bins
                .iter()
                .map(|b| b.n.iter().map(|&x| u64::from(x)).sum::<u64>())
                .sum();
            assert_eq!(
                recs, c.counts.baseline_ok,
                "one posterior record per scoreable step"
            );
        }
    }
}

/// Review #1699 minor: a restart scheduled at or after the end of the 40-week horizon (extinction
/// in the last week) is counted apart, not as "restart within horizon". Fabricated records,
/// confined to this unit test.
#[test]
fn restart_beyond_the_horizon_is_not_counted_as_within() {
    use super::agg::aggregate;
    use super::run::RepOut;
    let item = manifest().into_iter().find(|i| i.scenario == 2).unwrap();
    let rep = |restart: Option<u32>| RepOut {
        total_events: 0,
        pending_events: 0,
        extinction_day: restart.map(|_| 1),
        restart_week: restart,
        week40_held_carry: 0,
        observed_total: 0,
        variants: item
            .variants
            .iter()
            .map(|_| super::run::VariantOut {
                counts: Default::default(),
                recs: Vec::new(),
            })
            .collect(),
    };
    let mut slots: Vec<Option<RepOut>> = vec![None; 2000];
    slots[0] = Some(rep(Some(3)));
    slots[1] = Some(rep(Some(WEEKS as u32 - 1))); // last in-horizon week
    slots[2] = Some(rep(Some(WEEKS as u32))); // day 279 extinction: restart on day 280
    slots[3] = Some(rep(None));
    let agg = aggregate(std::slice::from_ref(&item), &slots);
    let a = &agg.items[0];
    assert_eq!(a.extinct_replicates, 3);
    assert_eq!(a.restart_replicates, 2);
    assert_eq!(a.restart_beyond_horizon, 1);
    assert_eq!(a.restart_week_sum, (3 + 1) + WEEKS as u64);
}
