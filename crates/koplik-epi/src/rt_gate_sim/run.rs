//! Replicate execution for the sizing and main stages, and deterministic aggregation.
//!
//! Replicates are independent (own seed, own RNG) so they run on worker threads; their
//! outputs are merged strictly in (item, replicate) order, so no floating-point value is ever
//! reduced across threads (spec E4).

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use koplik_contracts::v1::RtStatus;

use crate::rt::{BeforeSeries, estimate_series};

use super::design::{
    FIRST_SCORED_STEP, GenItem, NBINS, REPLICATES, Stage, StepClass, bin_of, classify, estimator,
    seed,
};
use super::generator::{SimError, WEEKS, generate_series};

/// Step counts for one analysis cell and replicate (or summed over replicates).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepCounts {
    /// Scored weeks (7..=40) examined.
    pub scored_steps: u64,
    /// Baseline reasons over all scored steps.
    pub reason_missing: u64,
    pub reason_below: u64,
    pub reason_no_infectivity: u64,
    pub baseline_ok: u64,
    /// Scored weeks with I >= min_cases (the count gate), whatever their history.
    pub count_gate: u64,
    /// Count-gate steps whose look-back reaches before the series (unbinned, Lambda unknown).
    pub unknown_history: u64,
    /// Count-gate steps with known history and Lambda = 0 (inside bin 0, not scoreable).
    pub zero_lambda: u64,
    /// Count-gate steps with known Lambda, by bin (including Lambda = 0 in bin 0).
    pub bin_steps: [u64; NBINS],
    /// Baseline-scoreable steps (Lambda > 0) by bin.
    pub bin_scoreable: [u64; NBINS],
}

impl StepCounts {
    pub fn add(&mut self, o: &StepCounts) {
        self.scored_steps += o.scored_steps;
        self.reason_missing += o.reason_missing;
        self.reason_below += o.reason_below;
        self.reason_no_infectivity += o.reason_no_infectivity;
        self.baseline_ok += o.baseline_ok;
        self.count_gate += o.count_gate;
        self.unknown_history += o.unknown_history;
        self.zero_lambda += o.zero_lambda;
        for b in 0..NBINS {
            self.bin_steps[b] += o.bin_steps[b];
            self.bin_scoreable[b] += o.bin_scoreable[b];
        }
    }
}

/// One scoreable step's outcome (main stage).
#[derive(Debug, Clone, Copy)]
pub struct StepRec {
    pub week: u8,
    pub bin: u8,
    pub cover95: bool,
    pub cover50: bool,
    pub gt3: bool,
    /// `abs(ln(R-hat / R_w))`, R-hat the posterior mean.
    pub err: f64,
}

#[derive(Debug, Clone)]
pub struct VariantOut {
    pub counts: StepCounts,
    pub recs: Vec<StepRec>,
}

/// One replicate of one generated item.
#[derive(Debug, Clone)]
pub struct RepOut {
    pub total_events: u64,
    pub pending_events: u64,
    pub extinction_day: Option<u32>,
    pub restart_week: Option<u32>,
    pub week40_held_carry: u32,
    /// Sum of observed weekly counts over all 40 weeks.
    pub observed_total: u64,
    pub variants: Vec<VariantOut>,
}

fn count_steps(infos: &[super::design::StepInfo]) -> StepCounts {
    let mut c = StepCounts::default();
    let (_, cfg) = estimator();
    for info in &infos[FIRST_SCORED_STEP..WEEKS] {
        c.scored_steps += 1;
        match info.class {
            StepClass::MissingCount => c.reason_missing += 1,
            StepClass::BelowThreshold => c.reason_below += 1,
            StepClass::NoInfectivity => c.reason_no_infectivity += 1,
            StepClass::Ok => c.baseline_ok += 1,
            StepClass::IncompleteWindow => {}
        }
        if info.cases >= cfg.min_cases {
            c.count_gate += 1;
            match info.class {
                StepClass::MissingCount | StepClass::IncompleteWindow => c.unknown_history += 1,
                StepClass::NoInfectivity => {
                    c.zero_lambda += 1;
                    c.bin_steps[0] += 1;
                }
                StepClass::Ok => {
                    let b = bin_of(info.lambda.expect("ok step has lambda"));
                    c.bin_steps[b] += 1;
                    c.bin_scoreable[b] += 1;
                }
                StepClass::BelowThreshold => {}
            }
        }
    }
    c
}

/// Run one replicate. `score` selects the main stage (posterior metrics) over the count-only
/// sizing stage (no posterior, no CrI, no R-hat is ever computed).
pub fn run_rep(item: &GenItem, stage: Stage, r: u32) -> Result<RepOut, SimError> {
    // Hard stop (review #1699): no test may execute registered main-stage seeds. Tests that need
    // main-stage plumbing call `run_rep_seeded` with an explicit non-study seed.
    #[cfg(test)]
    assert!(
        stage != Stage::Main,
        "tests must not run registered main-stage replicates; use run_rep_seeded"
    );
    run_rep_seeded(item, stage, seed(item.scenario, stage, r))
}

/// Run one replicate with an explicit seed. The study driver reaches this only through
/// [`run_rep`] (registered seeds); unit tests pass off-study seeds.
pub fn run_rep_seeded(item: &GenItem, stage: Stage, sd: u64) -> Result<RepOut, SimError> {
    let series = generate_series(&item.spec, item.obs, sd)?;
    let (si, cfg) = estimator();
    let w = si.weights();
    let mut variants = Vec::new();
    for &before in &item.variants {
        let infos = classify(&series.counts, w, before, cfg.min_cases);
        let counts = count_steps(&infos);
        let mut recs = Vec::new();
        if stage == Stage::Main {
            let mut cfg_v = cfg.clone();
            cfg_v.before_series = before;
            let opt: Vec<Option<u32>> = series.counts.iter().map(|&c| Some(c)).collect();
            let est = estimate_series(&opt, &si, &cfg_v)?;
            for t in FIRST_SCORED_STEP..WEEKS {
                let info = &infos[t];
                let e = &est[t];
                let ok_est = e.status == RtStatus::Ok;
                if ok_est != (info.class == StepClass::Ok) {
                    return Err(SimError::Disagreement(format!(
                        "seed {sd} week {} class {:?} but estimator status {:?} reason {:?}",
                        t + 1,
                        info.class,
                        e.status,
                        e.reason
                    )));
                }
                if info.class != StepClass::Ok {
                    continue;
                }
                let post = e.posterior.expect("ok step has a posterior");
                let truth = series.truth[t];
                let rhat = post.mean();
                let i95 = e.intervals[1];
                let i50 = e.intervals[0];
                debug_assert!(i95.level == 0.95 && i50.level == 0.5);
                recs.push(StepRec {
                    week: (t + 1) as u8,
                    bin: bin_of(info.lambda.expect("lambda")) as u8,
                    // Coverage includes the endpoints.
                    cover95: i95.lower <= truth && truth <= i95.upper,
                    cover50: i50.lower <= truth && truth <= i50.upper,
                    gt3: rhat / truth > 3.0,
                    err: libm::fabs(libm::log(rhat / truth)),
                });
            }
        }
        variants.push(VariantOut { counts, recs });
    }
    Ok(RepOut {
        total_events: series.latent_total_events,
        pending_events: series.latent_pending_events,
        extinction_day: series.extinction_day,
        restart_week: series.restart_week,
        week40_held_carry: series.week40_held_carry,
        observed_total: series.counts.iter().map(|&c| u64::from(c)).sum(),
        variants,
    })
}

/// A replicate that did not complete, reported with scenario/cell/seed.
#[derive(Debug, Clone)]
pub struct Failure {
    pub item: usize,
    pub label: String,
    pub replicate: u32,
    pub seed: u64,
    pub error: SimError,
}

/// Run every (item, replicate) task of a stage; outputs are indexed `item * REPLICATES + r`.
pub fn run_stage(
    items: &[GenItem],
    stage: Stage,
    threads: usize,
    progress: bool,
) -> (Vec<Option<RepOut>>, Vec<Failure>) {
    let ntasks = items.len() * REPLICATES as usize;
    // Heaviest items first (R = 1.5 steady, restart) so threads stay balanced.
    let heavy = |i: usize| -> bool { items[i].r_label == Some(1.5) || items[i].scenario == 2 };
    let mut order: Vec<usize> = Vec::with_capacity(ntasks);
    for pass in [true, false] {
        for i in 0..items.len() {
            if heavy(i) == pass {
                for r in 0..REPLICATES as usize {
                    order.push(i * REPLICATES as usize + r);
                }
            }
        }
    }
    let cursor = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Result<RepOut, SimError>)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let n = cursor.fetch_add(1, Ordering::Relaxed);
                    if n >= order.len() {
                        break;
                    }
                    let task = order[n];
                    let item = task / REPLICATES as usize;
                    let r = (task % REPLICATES as usize) as u32;
                    local.push((task, run_rep(&items[item], stage, r)));
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if progress && d % 2000 == 0 {
                        eprintln!("  {d}/{ntasks} replicates");
                    }
                    if local.len() >= 512 {
                        results.lock().unwrap().append(&mut local);
                    }
                }
                results.lock().unwrap().append(&mut local);
            });
        }
    });
    let mut slots: Vec<Option<RepOut>> = (0..ntasks).map(|_| None).collect();
    let mut failures = Vec::new();
    for (task, res) in results.into_inner().unwrap() {
        match res {
            Ok(o) => slots[task] = Some(o),
            Err(error) => {
                let item = task / REPLICATES as usize;
                let r = (task % REPLICATES as usize) as u32;
                failures.push(Failure {
                    item,
                    label: items[item].label.clone(),
                    replicate: r,
                    seed: seed(items[item].scenario, stage, r),
                    error,
                });
            }
        }
    }
    failures.sort_by_key(|f| (f.item, f.replicate));
    (slots, failures)
}

pub fn before_series_of(v: BeforeSeries) -> &'static str {
    super::design::variant_name(v)
}
