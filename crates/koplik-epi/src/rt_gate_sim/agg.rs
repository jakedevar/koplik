//! Deterministic aggregation of replicate outputs, preregistered metrics and decisions.
//! Every reduction is sequential in (item, replicate) order.

use std::ops::Range;

use crate::rt::BeforeSeries;

use super::design::{GenItem, HIGH_BINS, LOW_BINS, NBINS, REPLICATES};
use super::run::{RepOut, StepCounts};

/// Minimum scoreable steps for any pool or S2 cell to be evaluable (prereg S1/S2).
pub const MIN_STEPS: u64 = 200;
/// S1: 95% coverage in `[0,1)` must be at most 0.80 (as 80/100 exact).
pub const S1_MAX_COVERAGE_PCT: u64 = 80;
/// S1: coverage gap versus `[1,2)` must be at least 0.10 (10/100 exact).
pub const S1_MIN_GAP_PCT: u64 = 10;
/// S2: 95% coverage in `[1,inf)` at least 0.93 (93/100 exact).
pub const S2_MIN_COVERAGE_PCT: u64 = 93;

#[derive(Debug, Clone)]
pub struct BinAgg {
    pub n: Vec<u32>,
    pub h95: Vec<u32>,
    pub h50: Vec<u32>,
    pub gt3: u64,
    /// (error, replicate, week) for nearest-rank percentiles.
    pub errs: Vec<(f64, u32, u8)>,
}

impl BinAgg {
    fn new() -> Self {
        BinAgg {
            n: vec![0; REPLICATES as usize],
            h95: vec![0; REPLICATES as usize],
            h50: vec![0; REPLICATES as usize],
            gt3: 0,
            errs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CellAgg {
    pub item: usize,
    pub variant: BeforeSeries,
    pub counts: StepCounts,
    pub bins: Vec<BinAgg>,
}

#[derive(Debug, Clone, Default)]
pub struct ItemAgg {
    pub replicates: u32,
    pub events_sum: u128,
    pub events_max: u64,
    pub events_max_replicate: u32,
    pub pending_sum: u128,
    pub pending_max: u64,
    pub observed_total_sum: u128,
    pub extinct_replicates: u32,
    pub restart_replicates: u32,
    pub restart_week_sum: u64,
    pub carry_replicates: u32,
    pub carry_cases: u64,
}

#[derive(Debug, Clone)]
pub struct StageAgg {
    pub items: Vec<ItemAgg>,
    pub cells: Vec<CellAgg>,
}

pub fn aggregate(items: &[GenItem], slots: &[Option<RepOut>]) -> StageAgg {
    let mut agg_items = vec![ItemAgg::default(); items.len()];
    let mut cells = Vec::new();
    let mut cell_of = Vec::new(); // cell_of[item][variant] = index
    for (i, it) in items.iter().enumerate() {
        let mut v = Vec::new();
        for &variant in &it.variants {
            v.push(cells.len());
            cells.push(CellAgg {
                item: i,
                variant,
                counts: StepCounts::default(),
                bins: (0..NBINS).map(|_| BinAgg::new()).collect(),
            });
        }
        cell_of.push(v);
    }
    for (i, _) in items.iter().enumerate() {
        for r in 0..REPLICATES {
            let Some(o) = &slots[i * REPLICATES as usize + r as usize] else {
                continue;
            };
            let a = &mut agg_items[i];
            a.replicates += 1;
            a.events_sum += u128::from(o.total_events);
            if o.total_events > a.events_max || a.replicates == 1 {
                a.events_max = o.total_events;
                a.events_max_replicate = r;
            }
            a.pending_sum += u128::from(o.pending_events);
            a.pending_max = a.pending_max.max(o.pending_events);
            a.observed_total_sum += u128::from(o.observed_total);
            if o.extinction_day.is_some() {
                a.extinct_replicates += 1;
            }
            if let Some(w) = o.restart_week {
                a.restart_replicates += 1;
                a.restart_week_sum += u64::from(w) + 1; // 1-based study week of restart
            }
            if o.week40_held_carry > 0 {
                a.carry_replicates += 1;
                a.carry_cases += u64::from(o.week40_held_carry);
            }
            for (vi, vo) in o.variants.iter().enumerate() {
                let c = &mut cells[cell_of[i][vi]];
                c.counts.add(&vo.counts);
                for rec in &vo.recs {
                    let b = &mut c.bins[rec.bin as usize];
                    b.n[r as usize] += 1;
                    b.h95[r as usize] += u32::from(rec.cover95);
                    b.h50[r as usize] += u32::from(rec.cover50);
                    b.gt3 += u64::from(rec.gt3);
                    b.errs.push((rec.err, r, rec.week));
                }
            }
        }
    }
    StageAgg {
        items: agg_items,
        cells,
    }
}

/// Preregistered metrics for a pool of cells over a bin range (replicate-paired sums).
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub n: u64,
    pub h95: u64,
    pub h50: u64,
    pub nonempty: u32,
    pub se95: Option<f64>,
    pub se50: Option<f64>,
    pub median_err: Option<f64>,
    pub p90_err: Option<f64>,
    pub gt3: u64,
}

fn cluster_se(n_r: &[u64], h_r: &[u64], sum_n: u64, sum_h: u64) -> Option<f64> {
    if sum_n == 0 {
        return None;
    }
    let p = sum_h as f64 / sum_n as f64;
    let m = REPLICATES as f64;
    let mut ss = 0.0_f64;
    for r in 0..n_r.len() {
        let d = h_r[r] as f64 - p * n_r[r] as f64;
        ss += d * d;
    }
    Some(libm::sqrt(m / (m - 1.0) * ss) / sum_n as f64)
}

pub fn metrics(cells: &[&CellAgg], bins: Range<usize>) -> Metrics {
    let m = REPLICATES as usize;
    let mut n_r = vec![0u64; m];
    let mut h95_r = vec![0u64; m];
    let mut h50_r = vec![0u64; m];
    let mut errs: Vec<(f64, u32, u8)> = Vec::new();
    let mut gt3 = 0;
    for c in cells {
        for b in bins.clone() {
            let ba = &c.bins[b];
            for r in 0..m {
                n_r[r] += u64::from(ba.n[r]);
                h95_r[r] += u64::from(ba.h95[r]);
                h50_r[r] += u64::from(ba.h50[r]);
            }
            gt3 += ba.gt3;
            errs.extend_from_slice(&ba.errs);
        }
    }
    let n: u64 = n_r.iter().sum();
    let h95: u64 = h95_r.iter().sum();
    let h50: u64 = h50_r.iter().sum();
    errs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let nn = errs.len() as u64;
    // Nearest rank ceil(p*N), one-based: median ceil(N/2), 90th ceil(9N/10).
    let pick = |num: u64, den: u64| -> Option<f64> {
        if nn == 0 {
            None
        } else {
            let rank = (nn * num).div_ceil(den);
            Some(errs[(rank - 1) as usize].0)
        }
    };
    Metrics {
        n,
        h95,
        h50,
        nonempty: n_r.iter().filter(|&&x| x > 0).count() as u32,
        se95: cluster_se(&n_r, &h95_r, n, h95),
        se50: cluster_se(&n_r, &h50_r, n, h50),
        median_err: pick(1, 2),
        p90_err: pick(9, 10),
        gt3,
    }
}

/// Sum of baseline-scoreable steps over a pool of cells in a bin range (count-only).
pub fn scoreable(cells: &[&CellAgg], bins: Range<usize>) -> u64 {
    cells
        .iter()
        .map(|c| bins.clone().map(|b| c.counts.bin_scoreable[b]).sum::<u64>())
        .sum()
}

/// One S1 pool row: scenario 2..=5 in one suite, all its k cells (Unknown only).
#[derive(Debug, Clone)]
pub struct S1Row {
    pub suite: &'static str,
    pub scenario: u8,
    pub n_low: u64,
    pub n_ref: u64,
    /// Only with posterior metrics (main stage).
    pub cov_low: Option<(u64, u64)>,
    pub cov_ref: Option<(u64, u64)>,
    /// Both bins have >= 200 scoreable steps.
    pub evaluable: bool,
    pub c1_min_low: bool,
    pub c2_cov_le_80: Option<bool>,
    pub c3_gap_ge_10: Option<bool>,
    pub meets: Option<bool>,
}

pub fn pool_cells<'a>(
    items: &[GenItem],
    agg: &'a StageAgg,
    suite: &str,
    scenario: u8,
) -> Vec<&'a CellAgg> {
    agg.cells
        .iter()
        .filter(|c| items[c.item].scenario == scenario && items[c.item].si.name == suite)
        .collect()
}

pub fn s1_rows(items: &[GenItem], agg: &StageAgg, with_coverage: bool) -> Vec<S1Row> {
    let mut rows = Vec::new();
    for suite in ["primary", "sensitivity"] {
        for s in 2..=5u8 {
            let pool = pool_cells(items, agg, suite, s);
            let n_low = scoreable(&pool, LOW_BINS);
            let n_ref = scoreable(&pool, 3..4);
            let evaluable = n_low >= MIN_STEPS && n_ref >= MIN_STEPS;
            let mut row = S1Row {
                suite: if suite == "primary" {
                    "primary"
                } else {
                    "sensitivity"
                },
                scenario: s,
                n_low,
                n_ref,
                cov_low: None,
                cov_ref: None,
                evaluable,
                c1_min_low: n_low >= MIN_STEPS,
                c2_cov_le_80: None,
                c3_gap_ge_10: None,
                meets: None,
            };
            if with_coverage {
                let ml = metrics(&pool, LOW_BINS);
                let mr = metrics(&pool, 3..4);
                row.cov_low = Some((ml.h95, ml.n));
                row.cov_ref = Some((mr.h95, mr.n));
                if evaluable {
                    // Exact integer arithmetic: h/n <= 0.80 and h_ref/n_ref - h/n >= 0.10.
                    let c2 = ml.h95 * 100 <= S1_MAX_COVERAGE_PCT * ml.n;
                    let lhs = (u128::from(mr.h95) * u128::from(ml.n))
                        .saturating_sub(u128::from(ml.h95) * u128::from(mr.n));
                    // (h_ref*n_low - h_low*n_ref)*100 >= 10 * n_low*n_ref
                    let ge = (u128::from(mr.h95) * u128::from(ml.n))
                        >= (u128::from(ml.h95) * u128::from(mr.n))
                        && lhs * 100
                            >= u128::from(S1_MIN_GAP_PCT) * u128::from(ml.n) * u128::from(mr.n);
                    row.c2_cov_le_80 = Some(c2);
                    row.c3_gap_ge_10 = Some(ge);
                    row.meets = Some(c2 && ge && row.c1_min_low);
                }
            }
            rows.push(row);
        }
    }
    rows
}

/// One S2 cell: scenario 1, k = infinity, one R, one before-series variant, one suite.
#[derive(Debug, Clone)]
pub struct S2Row {
    pub suite: &'static str,
    pub r: f64,
    pub variant: BeforeSeries,
    pub cell: usize,
    pub n_high: u64,
    pub evaluable: bool,
    pub cov_high: Option<(u64, u64)>,
    pub passes: Option<bool>,
}

pub fn s2_rows(items: &[GenItem], agg: &StageAgg, with_coverage: bool) -> Vec<S2Row> {
    let mut rows = Vec::new();
    for suite in ["primary", "sensitivity"] {
        for (ci, c) in agg.cells.iter().enumerate() {
            let it = &items[c.item];
            if it.scenario != 1 || it.spec.k.is_some() || it.si.name != suite {
                continue;
            }
            let n_high = scoreable(&[c], HIGH_BINS);
            let evaluable = n_high >= MIN_STEPS;
            let mut row = S2Row {
                suite: if suite == "primary" {
                    "primary"
                } else {
                    "sensitivity"
                },
                r: it.r_label.expect("scenario 1 has R"),
                variant: c.variant,
                cell: ci,
                n_high,
                evaluable,
                cov_high: None,
                passes: None,
            };
            if with_coverage {
                let m = metrics(&[c], HIGH_BINS);
                row.cov_high = Some((m.h95, m.n));
                if evaluable {
                    row.passes = Some(m.h95 * 100 >= S2_MIN_COVERAGE_PCT * m.n);
                }
            }
            rows.push(row);
        }
    }
    rows
}

/// Sizing requirement (count-only): in each suite >= 3 scenarios evaluable for S1, and every
/// S2 cell evaluable. Returns (s1_ok_primary, s1_ok_sensitivity, s2_all_ok).
pub fn sizing_requirements(items: &[GenItem], agg: &StageAgg) -> (bool, bool, bool) {
    let s1 = s1_rows(items, agg, false);
    let count = |suite: &str| {
        s1.iter()
            .filter(|r| r.suite == suite && r.evaluable)
            .count()
    };
    let s2 = s2_rows(items, agg, false);
    (
        count("primary") >= 3,
        count("sensitivity") >= 3,
        s2.iter().all(|r| r.evaluable),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Reject,
    Inconclusive,
}

impl Outcome {
    pub fn name(self) -> &'static str {
        match self {
            Outcome::Pass => "PASS",
            Outcome::Reject => "REJECT",
            Outcome::Inconclusive => "INCONCLUSIVE",
        }
    }
}

/// Frozen decision. `complete` is false if any replicate failed. Sampling requirements come
/// first; then any numeric failure in an evaluable S1/S2 test means REJECT.
pub fn decide(items: &[GenItem], agg: &StageAgg, complete: bool) -> (Outcome, String) {
    let s1 = s1_rows(items, agg, true);
    let s2 = s2_rows(items, agg, true);
    if !complete {
        return (Outcome::Inconclusive, "incomplete run".to_string());
    }
    for suite in ["primary", "sensitivity"] {
        let ev = s1
            .iter()
            .filter(|r| r.suite == suite && r.evaluable)
            .count();
        if ev < 3 {
            return (
                Outcome::Inconclusive,
                format!(
                    "S1 sampling: only {ev} of scenarios 2-5 reach 200 scoreable steps in both [0,1) and [1,2) in the {suite} suite"
                ),
            );
        }
    }
    if let Some(r) = s2.iter().find(|r| !r.evaluable) {
        return (
            Outcome::Inconclusive,
            format!(
                "S2 sampling: {} R={} {} has {} scoreable steps in [1,inf)",
                r.suite,
                r.r,
                super::design::variant_name(r.variant),
                r.n_high
            ),
        );
    }
    let mut reasons = Vec::new();
    for suite in ["primary", "sensitivity"] {
        let met = s1
            .iter()
            .filter(|r| r.suite == suite && r.meets == Some(true))
            .count();
        if met < 3 {
            reasons.push(format!(
                "S1 fails in {suite}: {met} of scenarios 2-5 meet all conditions"
            ));
        }
    }
    for r in &s2 {
        if r.passes == Some(false) {
            reasons.push(format!(
                "S2 fails: {} R={} {} coverage {}/{}",
                r.suite,
                r.r,
                super::design::variant_name(r.variant),
                r.cov_high.unwrap().0,
                r.cov_high.unwrap().1
            ));
        }
    }
    if reasons.is_empty() {
        (
            Outcome::Pass,
            "S1 and every S2 cell pass in both suites".to_string(),
        )
    } else {
        (Outcome::Reject, reasons.join("; "))
    }
}
