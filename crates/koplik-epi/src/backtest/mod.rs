//! Honest forecast scoring and the report-vintage backtest.
//!
//! - [`crps_sample`]: the continuous ranked probability score of an ensemble, exact for the
//!   empirical distribution of its members.
//! - [`interval_covers`]: whether a central interval read from published quantiles covers
//!   the observation.
//! - [`vintages`]: weekly series from cumulative report versions, using only the versions
//!   first seen by a cutoff time (the information cutoff on *revisions*).
//! - [`run`]: forecast from every forecast date, score against the final series, summarise
//!   exactly as measured.
//! - [`truncated`]: the pseudo-real-time backtest for a source with no revision history:
//!   revised counts truncated at each forecast date, scored per series and pooled (#1503).
//! - [`manifest`]: read the ingest crate's vintage manifest (parsing bytes only; no I/O).
//!
//! Scores are reported as measured. No parameter in `crate::forecast` is chosen from them.

pub mod manifest;
pub mod run;
pub mod truncated;
pub mod vintages;

use koplik_contracts::v1::ForecastQuantile;

/// CRPS of the empirical distribution of `sample` against `observed`.
///
/// Gneiting T, Raftery AE. *Strictly proper scoring rules, prediction, and estimation.*
/// J Am Stat Assoc 2007;102(477):359-378, eq. (21): `CRPS(F, y) = E_F|X - y| - ½ E_F|X - X'|`
/// with `X, X'` independent draws from `F`. For the empirical distribution of `n` members
/// that is `(1/n) Σ_i |x_i - y| - (1/(2n²)) Σ_i Σ_j |x_i - x_j|`, computed exactly in
/// `O(n log n)` from the sorted members: `Σ_i Σ_j |x_i - x_j| = 2 Σ_i (2i - n + 1) x_(i)`
/// (0-based `i`). This is the plain ensemble estimator (Zamo & Naveau 2018 call it the
/// "NRG" form), not the finite-ensemble-adjusted "fair" CRPS; it is reported as such.
/// Units are those of the sample (cases); lower is better; a point forecast scores its
/// absolute error.
pub fn crps_sample(sample: &[f64], observed: f64) -> f64 {
    assert!(!sample.is_empty(), "CRPS of an empty sample");
    let mut sorted = sample.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite sample"));
    let n = sorted.len() as f64;
    let mut abs_error = 0.0_f64;
    let mut pair_sum = 0.0_f64;
    for (i, x) in sorted.iter().enumerate() {
        abs_error += (x - observed).abs();
        pair_sum += (2.0 * i as f64 - n + 1.0) * x;
    }
    abs_error / n - pair_sum / (n * n)
}

/// [`crps_sample`] for integer members and an integer observation.
pub fn crps_counts(sample: &[u64], observed: u32) -> f64 {
    let sample: Vec<f64> = sample.iter().map(|&x| x as f64).collect();
    crps_sample(&sample, f64::from(observed))
}

/// Whether the central `level` interval (quantiles `(1 - level)/2` and `(1 + level)/2`,
/// bounds inclusive) covers `observed`. `None` when either quantile is not published.
pub fn interval_covers(quantiles: &[ForecastQuantile], level: f64, observed: f64) -> Option<bool> {
    let find = |p: f64| {
        quantiles
            .iter()
            .find(|q| (q.level - p).abs() < 1e-9)
            .map(|q| q.value)
    };
    let lower = find((1.0 - level) / 2.0)?;
    let upper = find((1.0 + level) / 2.0)?;
    Some(lower <= observed && observed <= upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_crps(sample: &[f64], y: f64) -> f64 {
        let n = sample.len() as f64;
        let e1: f64 = sample.iter().map(|x| (x - y).abs()).sum::<f64>() / n;
        let mut e2 = 0.0;
        for a in sample {
            for b in sample {
                e2 += (a - b).abs();
            }
        }
        e1 - e2 / (2.0 * n * n)
    }

    /// Closed forms: a point mass scores its absolute error; a two-point distribution
    /// `{a, b}` with equal mass scores `(|a - y| + |b - y|)/2 - |a - b|/4`; the discrete
    /// uniform on `{1..n}` at `y = 0` scores `mean - (n² - 1)/(6n)` (from
    /// `Σ_i Σ_j |i - j| = n(n² - 1)/3`).
    #[test]
    fn crps_matches_closed_forms() {
        assert_eq!(crps_sample(&[3.0], 5.0), 2.0);
        assert_eq!(crps_sample(&[3.0, 3.0, 3.0], 1.0), 2.0);
        for (a, b, y) in [
            (0.0_f64, 10.0_f64, 4.0_f64),
            (2.0, 2.5, 9.0),
            (-1.0, 1.0, 0.0),
        ] {
            let expected = ((a - y).abs() + (b - y).abs()) / 2.0 - (a - b).abs() / 4.0;
            assert!((crps_sample(&[b, a], y) - expected).abs() < 1e-12);
        }
        let n = 10.0;
        let uniform: Vec<f64> = (1..=10).map(f64::from).collect();
        let expected = 5.5 - (n * n - 1.0) / (6.0 * n);
        assert!((crps_sample(&uniform, 0.0) - expected).abs() < 1e-12);
    }

    #[test]
    fn crps_sorted_formula_equals_the_double_sum() {
        let sample = [4.0, 1.0, 9.0, 2.5, 2.5, 7.0, 0.0, 11.0];
        for y in [-3.0, 0.0, 2.5, 5.1, 20.0] {
            assert!((crps_sample(&sample, y) - naive_crps(&sample, y)).abs() < 1e-12);
        }
        assert_eq!(
            crps_counts(&[1, 2, 3], 2),
            crps_sample(&[1.0, 2.0, 3.0], 2.0)
        );
    }

    /// Proper-score sanity: the CRPS is non-negative, zero only for a point mass at the
    /// observation, and a sharper forecast centred on the truth scores better.
    #[test]
    fn crps_is_non_negative_and_rewards_sharpness() {
        assert_eq!(crps_sample(&[5.0, 5.0], 5.0), 0.0);
        let wide: Vec<f64> = (0..=20).map(f64::from).collect();
        let narrow: Vec<f64> = (8..=12).map(f64::from).collect();
        assert!(crps_sample(&narrow, 10.0) < crps_sample(&wide, 10.0));
        assert!(crps_sample(&wide, 10.0) >= 0.0);
    }

    #[test]
    fn interval_coverage_reads_the_right_quantiles() {
        let q = |level, value| ForecastQuantile { level, value };
        let qs = [
            q(0.05, 2.0),
            q(0.25, 4.0),
            q(0.5, 5.0),
            q(0.75, 6.0),
            q(0.95, 9.0),
        ];
        assert_eq!(interval_covers(&qs, 0.5, 4.0), Some(true));
        assert_eq!(interval_covers(&qs, 0.5, 6.0), Some(true));
        assert_eq!(interval_covers(&qs, 0.5, 6.5), Some(false));
        assert_eq!(interval_covers(&qs, 0.9, 6.5), Some(true));
        assert_eq!(interval_covers(&qs, 0.9, 1.0), Some(false));
        assert_eq!(interval_covers(&qs, 0.8, 5.0), None);
    }
}
