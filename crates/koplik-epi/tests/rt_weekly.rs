//! Weekly R_t behaviour: synthetic recovery, zeros, missing weeks, threshold, provisional
//! flags, contract validity and determinism.

use koplik_contracts::v1::{
    CaseCount, GeoId, MissingReason, MmwrWeek, Provenances, RtEstimate, RtStatus, WeeklyCaseCount,
};
use koplik_epi::rt::{
    BeforeSeries, InsufficientReason, RenewalConfig, RtConfig, SerialInterval, estimate_series,
    estimate_weekly,
};
use serde_json::json;

fn prov(tag: &str) -> Provenances {
    serde_json::from_value(json!([{
        "source_id": "cdc-measles-weekly",
        "url": format!("https://data.cdc.gov/example/{tag}"),
        "retrieved_at": "2026-09-30T12:00:00Z",
        "sha256": "ab".repeat(32),
        "licence_id": "us-gov-public-domain"
    }]))
    .unwrap()
}

fn texas() -> GeoId {
    "48".parse().unwrap()
}

fn gaines() -> GeoId {
    "48165".parse().unwrap()
}

fn week(n: u8) -> MmwrWeek {
    MmwrWeek::new(2025, n).unwrap()
}

fn reported(geo: GeoId, w: u8, count: u32) -> WeeklyCaseCount {
    WeeklyCaseCount {
        geography: geo,
        week: week(w),
        confirmed: CaseCount::Reported { count },
        provenance: prov("a"),
    }
}

fn missing(geo: GeoId, w: u8) -> WeeklyCaseCount {
    WeeklyCaseCount {
        geography: geo,
        week: week(w),
        confirmed: CaseCount::Missing {
            reason: MissingReason::NotReported,
        },
        provenance: prov("b"),
    }
}

/// Deterministic renewal series: I_t = round(R · Σ_k w_k I_{t-k}), zero before the seed.
fn synthetic(r: f64, seed: u32, weeks: usize, cfg: &RtConfig) -> Vec<u32> {
    let si = cfg
        .serial_interval
        .discretize_weekly(cfg.max_lag_weeks)
        .unwrap();
    let w = si.weights();
    let mut series = vec![seed];
    for t in 1..weeks {
        let mut lambda = 0.0;
        for (k, wk) in w.iter().enumerate().skip(1) {
            if t >= k {
                lambda += wk * f64::from(series[t - k]);
            }
        }
        series.push(libm::round(r * lambda) as u32);
    }
    series
}

fn rows_at(rows: &[RtEstimate], w: u8, level: f64) -> &RtEstimate {
    rows.iter()
        .find(|r| r.week == week(w) && r.interval_level == level)
        .unwrap()
}

#[test]
fn synthetic_renewal_series_recovers_known_r() {
    let cfg = RtConfig::default();
    for (r, seed) in [(1.5, 60u32), (0.7, 2000u32)] {
        let series = synthetic(r, seed, 14, &cfg);
        let counts: Vec<Option<u32>> = series.iter().map(|&c| Some(c)).collect();
        let si = cfg
            .serial_interval
            .discretize_weekly(cfg.max_lag_weeks)
            .unwrap();
        let renewal = RenewalConfig {
            before_series: BeforeSeries::Zero,
            ..RenewalConfig::default()
        };
        let est = estimate_series(&counts, &si, &renewal).unwrap();
        let mut checked = 0;
        for s in &est[1..] {
            if s.status != RtStatus::Ok {
                continue;
            }
            let post = s.posterior.unwrap();
            let ci95 = s.intervals.iter().find(|c| c.level == 0.95).unwrap();
            assert!(
                ci95.lower <= r && r <= ci95.upper,
                "R={r} step {}: {ci95:?}",
                s.step
            );
            assert!(
                (post.mean() - r).abs() < 0.1,
                "R={r} step {} mean {}",
                s.step,
                post.mean()
            );
            checked += 1;
        }
        assert!(checked >= 8, "R={r}: only {checked} windows estimated");
    }
}

#[test]
fn unknown_history_withholds_early_weeks_and_zero_history_does_not() {
    let cfg = RtConfig::default();
    let series = synthetic(1.3, 100, 14, &cfg);
    let counts: Vec<Option<u32>> = series.iter().map(|&c| Some(c)).collect();
    let si = cfg
        .serial_interval
        .discretize_weekly(cfg.max_lag_weeks)
        .unwrap();
    let est = estimate_series(&counts, &si, &RenewalConfig::default()).unwrap();
    // Look-back reaches before the series for the first max_lag steps.
    for s in &est[..cfg.max_lag_weeks as usize] {
        assert_eq!(s.status, RtStatus::InsufficientData);
        assert!(matches!(
            s.reason,
            Some(InsufficientReason::IncompleteWindow | InsufficientReason::MissingCount)
        ));
    }
    for s in &est[cfg.max_lag_weeks as usize..] {
        assert_eq!(s.status, RtStatus::Ok, "step {}: {:?}", s.step, s.reason);
    }
}

#[test]
fn zero_counts_are_insufficient_never_an_estimate_of_zero() {
    let rows: Vec<_> = (1..=12).map(|w| reported(texas(), w, 0)).collect();
    let out = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    assert_eq!(out.len(), 24, "two levels per week");
    for r in &out {
        assert_eq!(r.status, RtStatus::InsufficientData);
        assert_eq!((r.mean, r.lower, r.upper), (None, None, None));
    }
}

#[test]
fn cases_after_a_silent_history_have_no_infectivity_to_divide_by() {
    // Eight reported-zero weeks then a burst: the renewal equation has no infectors in the
    // data for week 9, so R is undefined; the method must not report a prior-driven value.
    let mut rows: Vec<_> = (1..=8).map(|w| reported(texas(), w, 0)).collect();
    rows.push(reported(texas(), 9, 40));
    rows.push(reported(texas(), 10, 30));
    rows.push(reported(texas(), 11, 50));
    let out = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    assert_eq!(rows_at(&out, 9, 0.95).status, RtStatus::InsufficientData);
    assert_eq!(rows_at(&out, 10, 0.95).status, RtStatus::Ok);
    assert_eq!(rows_at(&out, 11, 0.95).status, RtStatus::Ok);
}

#[test]
fn missing_weeks_are_unknown_not_zero() {
    let cfg = RtConfig {
        max_lag_weeks: 3,
        ..RtConfig::default()
    };
    // Steady 30 cases/week over weeks 1..=20, with week 8 explicitly missing and week 14
    // absent from the rows (a gap).
    let rows: Vec<_> = (1..=20)
        .filter(|w| *w != 14)
        .map(|w| {
            if w == 8 {
                missing(texas(), w)
            } else {
                reported(texas(), w, 30)
            }
        })
        .collect();
    let out = estimate_weekly(&rows, &cfg).unwrap();
    let status = |w: u8| rows_at(&out, w, 0.95).status;
    // Week 4 is the first with a full 3-week look-back inside the series.
    assert_eq!(status(4), RtStatus::Ok);
    assert_eq!(status(7), RtStatus::Ok);
    // The missing week itself, and every week whose look-back touches it (8 + 3).
    for w in 8..=11 {
        assert_eq!(status(w), RtStatus::InsufficientData, "week {w}");
        assert_eq!(rows_at(&out, w, 0.95).mean, None);
    }
    assert_eq!(status(12), RtStatus::Ok);
    assert_eq!(status(13), RtStatus::Ok);
    // The gap behaves exactly like an explicit missing week.
    for w in 14..=17 {
        assert_eq!(status(w), RtStatus::InsufficientData, "week {w}");
    }
    assert_eq!(status(18), RtStatus::Ok);
    // With a steady series R ≈ 1 wherever it is estimated.
    let r18 = rows_at(&out, 18, 0.95);
    assert!(r18.lower.unwrap() < 1.0 && 1.0 < r18.upper.unwrap());
}

#[test]
fn minimum_count_threshold_is_a_hard_edge() {
    let cfg = RtConfig::default();
    let min = cfg.renewal.min_cases;
    assert_eq!(min, 11, "ceil(1/0.3^2 - 1) with the shape-1 prior");
    for (count, expect) in [(min - 1, RtStatus::InsufficientData), (min, RtStatus::Ok)] {
        let rows: Vec<_> = (1..=12).map(|w| reported(gaines(), w, count)).collect();
        let out = estimate_weekly(&rows, &cfg).unwrap();
        let last = rows_at(&out, 12, 0.95);
        assert_eq!(last.status, expect, "count {count}");
        if expect == RtStatus::Ok {
            assert!(last.mean.is_some() && last.lower.is_some() && last.upper.is_some());
        } else {
            assert_eq!((last.mean, last.lower, last.upper), (None, None, None));
        }
    }
}

#[test]
fn most_recent_two_weeks_are_provisional_independently_of_status() {
    let mut rows: Vec<_> = (1..=12).map(|w| reported(texas(), w, 40)).collect();
    rows[11] = reported(texas(), 12, 1); // last week: too few cases, and provisional
    let out = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    for w in 1..=10 {
        assert!(!rows_at(&out, w, 0.95).provisional, "week {w}");
    }
    let w11 = rows_at(&out, 11, 0.95);
    assert!(w11.provisional && w11.status == RtStatus::Ok);
    let w12 = rows_at(&out, 12, 0.95);
    assert!(w12.provisional && w12.status == RtStatus::InsufficientData);
}

#[test]
fn output_rows_satisfy_the_v1_contract_and_nest_intervals() {
    let rows: Vec<_> = (1..=12)
        .map(|w| reported(gaines(), w, 25 + w as u32))
        .collect();
    let out = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    for r in &out {
        // The contract's deserializer enforces null-iff-insufficient and lower <= upper.
        let back: RtEstimate = serde_json::from_value(serde_json::to_value(r).unwrap()).unwrap();
        assert_eq!(&back, r);
        assert_eq!(
            r.provenance.as_slice().len(),
            1,
            "one distinct snapshot fed this series"
        );
    }
    let w12_50 = rows_at(&out, 12, 0.5);
    let w12_95 = rows_at(&out, 12, 0.95);
    assert_eq!(w12_50.mean, w12_95.mean);
    assert!(w12_95.lower.unwrap() < w12_50.lower.unwrap());
    assert!(w12_50.upper.unwrap() < w12_95.upper.unwrap());
    let m = w12_50.mean.unwrap();
    assert!(w12_50.lower.unwrap() < m && m < w12_50.upper.unwrap());
}

#[test]
fn geographies_are_independent_and_ordered() {
    let mut rows: Vec<_> = (1..=12).map(|w| reported(texas(), w, 40)).collect();
    rows.extend((1..=12).map(|w| reported(gaines(), w, 20)));
    let out = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    assert_eq!(out.len(), 48);
    assert!(out[..24].iter().all(|r| r.geography == texas()));
    assert!(out[24..].iter().all(|r| r.geography == gaines()));
    let weeks: Vec<_> = out[..24].iter().map(|r| r.week.week).collect();
    assert!(weeks.windows(2).all(|p| p[0] <= p[1]));
}

#[test]
fn duplicate_week_is_an_error() {
    let mut rows: Vec<_> = (1..=4).map(|w| reported(texas(), w, 10)).collect();
    rows.push(reported(texas(), 3, 11));
    let err = estimate_weekly(&rows, &RtConfig::default()).unwrap_err();
    assert!(
        err.to_string().contains("2025-W03") || err.to_string().contains("48"),
        "{err}"
    );
}

#[test]
fn same_inputs_give_bit_identical_output() {
    let rows: Vec<_> = (1..=15)
        .map(|w| reported(texas(), w, 17 + 3 * w as u32))
        .collect();
    let a = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    let b = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    assert_eq!(a, b);
    let bytes = |rows: &[RtEstimate]| -> Vec<u8> {
        rows.iter()
            .flat_map(|r| [r.mean, r.lower, r.upper])
            .flatten()
            .flat_map(f64::to_le_bytes)
            .collect()
    };
    assert_eq!(bytes(&a), bytes(&b));
}

#[test]
fn measles_default_cites_an_eleven_point_seven_day_interval() {
    let si = SerialInterval::MEASLES;
    assert_eq!((si.mean_days, si.sd_days), (11.7, 3.0));
    let g = si.gamma().unwrap();
    assert!((g.mean() - 11.7).abs() < 1e-12 && (g.sd() - 3.0).abs() < 1e-12);
}
