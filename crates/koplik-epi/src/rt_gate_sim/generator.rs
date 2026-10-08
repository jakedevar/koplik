//! Individual-level branching generator at daily resolution (preregistration
//! `thoughts/shared/research/1677-rt-gate-preregistration.md`, "Independent generator and fixed
//! parameter sources" and "Event construction and random draws").
//!
//! Each event is an infection/onset proxy with a continuous time. Offspring are drawn per
//! individual (Poisson, or a Gamma-Poisson mixture for finite dispersion `k`), each child
//! after a Gamma serial-interval lag. Nothing here draws weekly counts from the estimator's
//! likelihood or uses its weekly weights. Daily incidence is aggregated to MMWR weeks only
//! afterwards. Every draw comes from one seeded `ChaCha8Rng` through the crate's portable
//! samplers; the only transcendental-free arithmetic outside them is `+`/`*`/comparison.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

use crate::rt::GammaDist;
use crate::sampling::{binomial, poisson, uniform};

/// Study horizon: 40 complete MMWR weeks of 7 days (prereg "Duration and burn-in").
pub const WEEKS: usize = 40;
pub const DAYS: usize = WEEKS * 7;
/// Hard cap on generated events per replicate. Reaching it is reported as an error with
/// scenario/cell/seed, never silently truncated (prereg "Overflow ... must be reported").
/// Far above the expected maximum (about 1e5 events at R = 1.5 over 280 days).
pub const MAX_EVENTS: u64 = 200_000_000;

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum SimError {
    #[error("sampler failure: {0}")]
    Sampler(String),
    #[error("event overflow: more than {MAX_EVENTS} events")]
    EventOverflow,
    #[error("estimator disagreement: {0}")]
    Disagreement(String),
}

impl From<crate::rt::RtError> for SimError {
    fn from(e: crate::rt::RtError) -> Self {
        SimError::Sampler(e.to_string())
    }
}

/// A gamma serial interval by mean and SD in days (shape `(mean/SD)^2`, scale `SD^2/mean`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SiSpec {
    pub name: &'static str,
    pub mean_days: f64,
    pub sd_days: f64,
}

/// Primary generator SI: mean 11.7 d, SD 3.0 d (#1678 R3; same value and sources as
/// `SerialInterval::MEASLES`: CDC MMWR 2026;75(33), Klinkenberg/Nishiura 2011, Vink et al. 2014).
pub const PRIMARY_SI: SiSpec = SiSpec {
    name: "primary",
    mean_days: 11.7,
    sd_days: 3.0,
};
/// Sensitivity generator SI: mean 14 d, SD 4 d (#1678 R3; a stress choice, not an alternative
/// measles estimate). Only the generator uses it; the estimator keeps the production SI.
pub const SENSITIVITY_SI: SiSpec = SiSpec {
    name: "sensitivity",
    mean_days: 14.0,
    sd_days: 4.0,
};

impl SiSpec {
    pub fn gamma(&self) -> Result<GammaDist, SimError> {
        Ok(GammaDist::from_mean_sd(self.mean_days, self.sd_days)?)
    }
}

/// Reproduction-number regime over the study (prereg scenario table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Regime {
    /// Constant `R` throughout (scenarios 1, 3, 4, 5).
    Constant(f64),
    /// Scenario 2: `r_before` until the initial chain has no pending descendants; from the
    /// next Sunday permanently `r_after`, with imported clusters at `cluster_per_week`.
    ExtinctionRestart {
        r_before: f64,
        r_after: f64,
        cluster_per_week: f64,
        /// Individuals per restart cluster: 15, R3's burst size reused as a fixed stress
        /// construction (not a measured cluster size).
        cluster_size: u32,
    },
}

/// A one-off burst of imported individuals on a given day, each with an independent
/// uniform time within the day (scenario 3: 15 on the Wednesday of study week 20).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burst {
    pub day: u32,
    pub size: u32,
}

/// Everything that defines one latent process.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenSpec {
    pub si: SiSpec,
    /// Offspring dispersion `k` (negative-binomial variance `R + R^2/k`); `None` is
    /// `k = infinity`, i.e. Poisson offspring (Lloyd-Smith et al. 2005 as cited by #1678 R3).
    pub k: Option<f64>,
    pub regime: Regime,
    /// Background imported individuals per week (homogeneous Poisson, daily mean rate/7).
    pub background_per_week: f64,
    pub burst: Option<Burst>,
}

/// How the latent daily onsets become the observed weekly series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Observation {
    /// Complete and on time.
    Complete,
    /// Scenario 4: each week's reports are held to the next week with this probability.
    Batches { hold_probability: f64 },
    /// Scenario 5: each daily onset independently retained with this probability.
    Thinned { retain_probability: f64 },
}

/// The result of one latent process, before observation.
#[derive(Debug, Clone)]
pub struct Latent {
    /// Events (including imports and the initial seed) per day, by integer floor of event time.
    pub daily: Vec<u32>,
    /// All generated events: processed (time < day 280) plus pending beyond it.
    pub total_events: u64,
    /// Events scheduled at or after day 280, kept only for extinction detection.
    pub pending_events: u64,
    /// Scenario 2: day whose end left the event queue empty (first time), if any.
    pub extinction_day: Option<u32>,
    /// Scenario 2: 0-based study week from whose Sunday the restart regime applies, if any.
    pub restart_week: Option<u32>,
    /// Cohort `R_w` for each study week (truth; never estimated from outcomes).
    pub truth: Vec<f64>,
    /// The generator's RNG, continued for the observation model.
    pub rng: ChaCha8Rng,
}

/// The observed weekly series with its truth.
#[derive(Debug, Clone)]
pub struct Series {
    /// Observed weekly counts, `WEEKS` entries (complete reporting: all known).
    pub counts: Vec<u32>,
    pub latent_total_events: u64,
    pub latent_pending_events: u64,
    pub extinction_day: Option<u32>,
    pub restart_week: Option<u32>,
    pub truth: Vec<f64>,
    /// Scenario 4: cases held out of week 40 into week 41 (beyond the horizon), disclosed.
    pub week40_held_carry: u32,
}

impl Regime {
    fn r_at(&self, time: f64, restart_week: Option<u32>) -> f64 {
        match *self {
            Regime::Constant(r) => r,
            Regime::ExtinctionRestart {
                r_before, r_after, ..
            } => {
                let week = week_of(time);
                match restart_week {
                    Some(sw) if week >= sw => r_after,
                    _ => r_before,
                }
            }
        }
    }

    fn truth_week(&self, week: u32, restart_week: Option<u32>) -> f64 {
        match *self {
            Regime::Constant(r) => r,
            Regime::ExtinctionRestart {
                r_before, r_after, ..
            } => match restart_week {
                Some(sw) if week >= sw => r_after,
                _ => r_before,
            },
        }
    }
}

fn week_of(time: f64) -> u32 {
    // Event times are positive and below 2^31 days, so the cast cannot overflow.
    (time / 7.0) as u32
}

/// One serial-interval lag: gamma inverse-CDF of a midpoint uniform via `GammaDist::quantile`
/// (prereg: sharing the quantile utility is allowed; `tests` validate its moments with an
/// oracle that does not use it).
pub fn sample_lag(rng: &mut ChaCha8Rng, si: &GammaDist) -> Result<f64, SimError> {
    let u = uniform(rng);
    Ok(si.quantile(u)?)
}

type Heap = BinaryHeap<Reverse<(u64, u64)>>;

fn push(heap: &mut Heap, next_id: &mut u64, time: f64) {
    // Positive finite f64 bit patterns order like the values; ties break by monotone id.
    heap.push(Reverse((time.to_bits(), *next_id)));
    *next_id += 1;
}

/// Uniform time within day `d`, clamped strictly below `d + 1` against rounding.
fn within_day(rng: &mut ChaCha8Rng, d: u32) -> f64 {
    let t = f64::from(d) + uniform(rng);
    let limit = f64::from_bits((f64::from(d) + 1.0).to_bits() - 1);
    if t > limit { limit } else { t }
}

/// Generate the latent process. Order of draws, fixed by the preregistration and the
/// implementation choices below (documented in the results file):
///
/// per day: (1) background import count, (2) restart-cluster count (scenario 2, only on days
/// at or after the restart Sunday), (3) background import times in order, (4) one shared time
/// per cluster, (5) burst individuals' independent times; then events are processed
/// chronologically (ties by event ID): offspring count (mixture mean first when `k` is
/// finite), then one lag per child in child order.
pub fn generate_latent(spec: &GenSpec, seed: u64) -> Result<Latent, SimError> {
    let si = spec.si.gamma()?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut heap: Heap = BinaryHeap::new();
    let mut next_id: u64 = 0;
    let mut daily = vec![0u32; DAYS];
    let mut restart_week: Option<u32> = None;
    let mut extinction_day: Option<u32> = None;
    // One latent seed event at time 0.5 days (prereg "Initial condition").
    push(&mut heap, &mut next_id, 0.5);
    let mut generated: u64 = 1;

    for d in 0..DAYS as u32 {
        let n_bg = poisson(&mut rng, spec.background_per_week / 7.0);
        let mut n_cl = 0;
        if let Regime::ExtinctionRestart {
            cluster_per_week, ..
        } = spec.regime
            && let Some(sw) = restart_week
            && d >= 7 * sw
        {
            n_cl = poisson(&mut rng, cluster_per_week / 7.0);
        }
        for _ in 0..n_bg {
            let t = within_day(&mut rng, d);
            push(&mut heap, &mut next_id, t);
            generated += 1;
        }
        if let Regime::ExtinctionRestart { cluster_size, .. } = spec.regime {
            for _ in 0..n_cl {
                let t = within_day(&mut rng, d);
                for _ in 0..cluster_size {
                    push(&mut heap, &mut next_id, t);
                    generated += 1;
                }
            }
        }
        if let Some(b) = spec.burst
            && b.day == d
        {
            for _ in 0..b.size {
                let t = within_day(&mut rng, d);
                push(&mut heap, &mut next_id, t);
                generated += 1;
            }
        }
        let day_end = f64::from(d) + 1.0;
        while let Some(&Reverse((bits, _id))) = heap.peek() {
            let t = f64::from_bits(bits);
            if t >= day_end {
                break;
            }
            heap.pop();
            daily[d as usize] = daily[d as usize]
                .checked_add(1)
                .ok_or(SimError::EventOverflow)?;
            let r = spec.regime.r_at(t, restart_week);
            let children = match spec.k {
                None => poisson(&mut rng, r),
                Some(k) => {
                    let g = GammaDist::new(k, r / k)?;
                    let mu = g.quantile(uniform(&mut rng))?;
                    poisson(&mut rng, mu)
                }
            };
            generated = generated.saturating_add(children);
            if generated > MAX_EVENTS {
                return Err(SimError::EventOverflow);
            }
            for _ in 0..children {
                let lag = sample_lag(&mut rng, &si)?;
                push(&mut heap, &mut next_id, t + lag);
            }
        }
        if let Regime::ExtinctionRestart { .. } = spec.regime
            && restart_week.is_none()
            && heap.is_empty()
        {
            extinction_day = Some(d);
            restart_week = Some(d / 7 + 1);
        }
    }
    let pending = heap.len() as u64;
    let truth = (0..WEEKS as u32)
        .map(|w| spec.regime.truth_week(w, restart_week))
        .collect();
    Ok(Latent {
        daily,
        total_events: generated,
        pending_events: pending,
        extinction_day,
        restart_week,
        truth,
        rng,
    })
}

/// Apply the observation model to the latent process (continuing its RNG stream) and
/// aggregate daily counts to the 40 MMWR weeks after generation.
pub fn observe(latent: Latent, obs: Observation) -> Result<Series, SimError> {
    let Latent {
        daily,
        total_events,
        pending_events,
        extinction_day,
        restart_week,
        truth,
        mut rng,
    } = latent;
    let mut carry = 0u32;
    let counts = match obs {
        Observation::Complete => weekly(&daily)?,
        Observation::Thinned { retain_probability } => {
            let mut kept = Vec::with_capacity(DAYS);
            for &c in &daily {
                kept.push(binomial(&mut rng, u64::from(c), retain_probability) as u32);
            }
            weekly(&kept)?
        }
        Observation::Batches { hold_probability } => {
            let c = weekly(&daily)?;
            // Independent Bernoulli(hold) indicators in week order, after the latent process.
            let held: Vec<bool> = (0..WEEKS)
                .map(|_| uniform(&mut rng) < hold_probability)
                .collect();
            let mut out = Vec::with_capacity(WEEKS);
            for w in 0..WEEKS {
                let own = if held[w] { 0 } else { c[w] };
                let released = if w > 0 && held[w - 1] { c[w - 1] } else { 0 };
                out.push(own.checked_add(released).ok_or(SimError::EventOverflow)?);
            }
            if held[WEEKS - 1] {
                carry = c[WEEKS - 1];
            }
            out
        }
    };
    Ok(Series {
        counts,
        latent_total_events: total_events,
        latent_pending_events: pending_events,
        extinction_day,
        restart_week,
        truth,
        week40_held_carry: carry,
    })
}

fn weekly(daily: &[u32]) -> Result<Vec<u32>, SimError> {
    daily
        .chunks(7)
        .map(|w| {
            w.iter()
                .try_fold(0u32, |a, &c| a.checked_add(c))
                .ok_or(SimError::EventOverflow)
        })
        .collect()
}

/// Generate and observe one replicate.
pub fn generate_series(spec: &GenSpec, obs: Observation, seed: u64) -> Result<Series, SimError> {
    observe(generate_latent(spec, seed)?, obs)
}

/// Generator-lag validation (prereg "Event construction", Revision #1689 item 3): draw `n`
/// lags through [`sample_lag`] from `ChaCha8Rng::seed_from_u64(seed)` and return the sample
/// mean and SD (divisor `n`).
pub fn lag_moments(si: &SiSpec, n: u32, seed: u64) -> Result<(f64, f64), SimError> {
    let g = si.gamma()?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut xs = Vec::with_capacity(n as usize);
    for _ in 0..n {
        xs.push(sample_lag(&mut rng, &g)?);
    }
    let nf = f64::from(n);
    let mean = xs.iter().sum::<f64>() / nf;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / nf;
    Ok((mean, libm::sqrt(var)))
}

/// Preregistered lag-test constants: N, seeds and tolerances.
pub const LAG_TEST_N: u32 = 1_000_000;
pub const LAG_TEST_SEED_PRIMARY: u64 = 9_000_001;
pub const LAG_TEST_SEED_SENSITIVITY: u64 = 9_000_002;
/// Mean within 4 standard errors (SE = target SD / sqrt(N)).
pub const LAG_TEST_MEAN_SE: f64 = 4.0;
/// SD within 1% relative.
pub const LAG_TEST_SD_REL: f64 = 0.01;

/// Run the preregistered lag test for one suite. `Ok(report)` on pass, `Err(report)` on fail.
/// The oracle is the literal target mean/SD; it uses no `GammaDist` method.
pub fn lag_test(si: &SiSpec) -> Result<String, String> {
    let seed = if si.name == "primary" {
        LAG_TEST_SEED_PRIMARY
    } else {
        LAG_TEST_SEED_SENSITIVITY
    };
    let (mean, sd) = lag_moments(si, LAG_TEST_N, seed).map_err(|e| e.to_string())?;
    let se = si.sd_days / libm::sqrt(f64::from(LAG_TEST_N));
    let mean_ok = libm::fabs(mean - si.mean_days) <= LAG_TEST_MEAN_SE * se;
    let sd_ok = libm::fabs(sd - si.sd_days) <= LAG_TEST_SD_REL * si.sd_days;
    let report = format!(
        "{}: seed {seed}, N {LAG_TEST_N}: sample mean {mean:.5} (target {}, 4 SE = {:.5}, |diff| = {:.5}) {}; sample SD {sd:.5} (target {}, 1% = {:.5}, |diff| = {:.5}) {}",
        si.name,
        si.mean_days,
        LAG_TEST_MEAN_SE * se,
        libm::fabs(mean - si.mean_days),
        if mean_ok { "PASS" } else { "FAIL" },
        si.sd_days,
        LAG_TEST_SD_REL * si.sd_days,
        libm::fabs(sd - si.sd_days),
        if sd_ok { "PASS" } else { "FAIL" },
    );
    if mean_ok && sd_ok {
        Ok(report)
    } else {
        Err(report)
    }
}
