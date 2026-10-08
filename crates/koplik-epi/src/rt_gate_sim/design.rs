//! The preregistered scenario list, cell expansion, seeds, Lambda bins and the count-only
//! step classification (prereg "Scenario list and cell expansion", "Metrics, denominators").

use crate::rt::{BeforeSeries, DiscreteSerialInterval, RenewalConfig, SerialInterval};

use super::generator::{
    Burst, GenSpec, Observation, PRIMARY_SI, Regime, SENSITIVITY_SI, SiSpec, WEEKS,
};

/// Replicates per scenario x parameter cell in each stage (prereg "Replicates and seeds").
pub const REPLICATES: u32 = 2000;
/// First scored week is 7 (burn-in is weeks 1-6): 0-based steps 6..=39.
pub const FIRST_SCORED_STEP: usize = 6;
/// The estimator's serial interval is always the production one, discretised to 8 weeks
/// (`RtConfig::default().max_lag_weeks`), whatever the generator SI.
pub const ESTIMATOR_MAX_LAG_WEEKS: u32 = 8;
/// Candidate floor (policy under test; only reported against, never applied to estimates).
pub const FLOOR: f64 = 1.0;
/// Lambda bin lower edges; bin `i` is `[EDGES[i], EDGES[i+1])`, the last is `[5, inf)`.
pub const BIN_EDGES: [f64; 6] = [0.0, 0.25, 0.5, 1.0, 2.0, 5.0];
pub const BIN_NAMES: [&str; 6] = [
    "[0,0.25)",
    "[0.25,0.5)",
    "[0.5,1)",
    "[1,2)",
    "[2,5)",
    "[5,inf)",
];
pub const NBINS: usize = 6;
/// Bins 0..=2 form the `[0,1)` union (S1); bins 3..=5 the `[1,inf)` union (S2, Revision #1689).
pub const LOW_BINS: std::ops::Range<usize> = 0..3;
pub const HIGH_BINS: std::ops::Range<usize> = 3..6;

/// Equality belongs to the bin on the right.
pub fn bin_of(lambda: f64) -> usize {
    let mut b = 0;
    for (i, e) in BIN_EDGES.iter().enumerate() {
        if lambda >= *e {
            b = i;
        }
    }
    b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Sizing,
    Main,
}

/// Seed of replicate `r` of scenario `s`: main `s*1_000_003 + r`; sizing adds 1_000_000
/// (prereg "Replicates and seeds", #1683 E3). Reused across the scenario's cells and suites.
pub fn seed(scenario: u8, stage: Stage, r: u32) -> u64 {
    let base = u64::from(scenario) * 1_000_003 + u64::from(r);
    match stage {
        Stage::Main => base,
        Stage::Sizing => base + 1_000_000,
    }
}

/// One generated latent series (shared by its before-series variants).
#[derive(Debug, Clone)]
pub struct GenItem {
    pub scenario: u8,
    pub label: String,
    pub si: SiSpec,
    pub spec: GenSpec,
    pub obs: Observation,
    pub variants: Vec<BeforeSeries>,
    /// Scenario 1 `R` (for S2 identification); `None` elsewhere.
    pub r_label: Option<f64>,
}

impl GenItem {
    pub fn k_label(&self) -> String {
        match self.spec.k {
            None => "inf".to_string(),
            Some(k) => format!("{k}"),
        }
    }
}

pub fn variant_name(v: BeforeSeries) -> &'static str {
    match v {
        BeforeSeries::Unknown => "Unknown",
        BeforeSeries::Zero => "Zero",
    }
}

/// The k order fixed by the manifest: infinity, 1.0, 0.3.
pub const K_ORDER: [Option<f64>; 3] = [None, Some(1.0), Some(0.3)];

/// Full manifest in preregistered table order, primary suite then sensitivity suite
/// (28 analysis cells per suite).
pub fn manifest() -> Vec<GenItem> {
    let mut items = Vec::new();
    for si in [PRIMARY_SI, SENSITIVITY_SI] {
        // Scenario 1: steady transmission, 3 R x 3 k x {Unknown, Zero}.
        for r in [0.8, 1.0, 1.5] {
            for k in K_ORDER {
                items.push(item(
                    1,
                    si,
                    k,
                    Regime::Constant(r),
                    0.5,
                    None,
                    Observation::Complete,
                    vec![BeforeSeries::Unknown, BeforeSeries::Zero],
                    Some(r),
                    format!("R={r}"),
                ));
            }
        }
        // Scenario 2: extinction and restart (no imports until the restart).
        for k in K_ORDER {
            items.push(item(
                2,
                si,
                k,
                Regime::ExtinctionRestart {
                    r_before: 0.6,
                    r_after: 1.5,
                    cluster_per_week: 0.2,
                    cluster_size: 15,
                },
                0.0,
                None,
                Observation::Complete,
                vec![BeforeSeries::Unknown],
                None,
                "restart".to_string(),
            ));
        }
        // Scenario 3: imported-case burst, 15 on the Wednesday (day 3) of study week 20.
        for k in K_ORDER {
            items.push(item(
                3,
                si,
                k,
                Regime::Constant(0.8),
                1.0,
                Some(Burst {
                    day: 7 * 19 + 3,
                    size: 15,
                }),
                Observation::Complete,
                vec![BeforeSeries::Unknown],
                None,
                "burst".to_string(),
            ));
        }
        // Scenario 4: reporting batches, underlying R = 1.0, 0.5 imports/week.
        for k in K_ORDER {
            items.push(item(
                4,
                si,
                k,
                Regime::Constant(1.0),
                0.5,
                None,
                Observation::Batches {
                    hold_probability: 0.1,
                },
                vec![BeforeSeries::Unknown],
                None,
                "batches".to_string(),
            ));
        }
        // Scenario 5: incomplete ascertainment, k = 1.0 only.
        items.push(item(
            5,
            si,
            Some(1.0),
            Regime::Constant(1.0),
            0.5,
            None,
            Observation::Thinned {
                retain_probability: 0.3,
            },
            vec![BeforeSeries::Unknown],
            None,
            "thinned".to_string(),
        ));
    }
    items
}

#[allow(clippy::too_many_arguments)]
fn item(
    scenario: u8,
    si: SiSpec,
    k: Option<f64>,
    regime: Regime,
    bg: f64,
    burst: Option<Burst>,
    obs: Observation,
    variants: Vec<BeforeSeries>,
    r_label: Option<f64>,
    tag: String,
) -> GenItem {
    let kl = k.map_or("inf".to_string(), |k| format!("{k}"));
    GenItem {
        scenario,
        label: format!("s{scenario}/{}/{tag}/k={kl}", si.name),
        si,
        spec: GenSpec {
            si,
            k,
            regime,
            background_per_week: bg,
            burst,
        },
        obs,
        variants,
        r_label,
    }
}

/// The production estimator inputs (`estimate_series` defaults and the 8-week weekly SI).
pub fn estimator() -> (DiscreteSerialInterval, RenewalConfig) {
    let si = SerialInterval::MEASLES
        .discretize_weekly(ESTIMATOR_MAX_LAG_WEEKS)
        .expect("production serial interval discretises");
    (si, RenewalConfig::default())
}

/// Baseline status of a step, in the estimator's own check order (window 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepClass {
    /// Step 0: its infectivity is undefined.
    IncompleteWindow,
    /// Look-back reaches before the series under `BeforeSeries::Unknown`.
    MissingCount,
    BelowThreshold,
    /// Known history but Lambda = 0.
    NoInfectivity,
    /// Baseline scoreable; carries Lambda.
    Ok,
}

#[derive(Debug, Clone, Copy)]
pub struct StepInfo {
    pub class: StepClass,
    pub cases: u32,
    /// Lambda when history is known (also 0 for `NoInfectivity`); `None` for
    /// `IncompleteWindow` / `MissingCount`.
    pub lambda: Option<f64>,
}

/// Reproduce the baseline's structural window, known-count/history, min_cases and
/// positive-Lambda guards with the same ordered sums as `estimate_series`
/// (`sum_{k>=1} w_k I_{t-k}` in ascending k, window 1), without a posterior.
pub fn classify(counts: &[u32], w: &[f64], before: BeforeSeries, min_cases: u32) -> Vec<StepInfo> {
    debug_assert_eq!(counts.len(), WEEKS);
    let mut out = Vec::with_capacity(counts.len());
    for t in 0..counts.len() {
        let cases = counts[t];
        if t == 0 {
            out.push(StepInfo {
                class: StepClass::IncompleteWindow,
                cases,
                lambda: None,
            });
            continue;
        }
        let mut lambda_sum = 0.0_f64;
        let mut missing = false;
        for (k, wk) in w.iter().enumerate().skip(1) {
            match t.checked_sub(k) {
                Some(idx) => lambda_sum += wk * f64::from(counts[idx]),
                None => match before {
                    BeforeSeries::Zero => {}
                    BeforeSeries::Unknown => {
                        missing = true;
                        break;
                    }
                },
            }
        }
        let (class, lambda) = if missing {
            (StepClass::MissingCount, None)
        } else if cases < min_cases {
            (StepClass::BelowThreshold, Some(lambda_sum))
        } else if lambda_sum.is_nan() || lambda_sum <= 0.0 {
            (StepClass::NoInfectivity, Some(lambda_sum))
        } else {
            (StepClass::Ok, Some(lambda_sum))
        };
        out.push(StepInfo {
            class,
            cases,
            lambda,
        });
    }
    out
}
