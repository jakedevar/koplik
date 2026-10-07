//! Agreement with EpiEstim's published worked example (offline fixture; sources inside it).

use koplik_contracts::v1::RtStatus;
use koplik_epi::rt::{BeforeSeries, GammaDist, RenewalConfig, SerialInterval, estimate_series};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    incidence: Vec<u32>,
    mean_si: f64,
    std_si: f64,
    window_days: u32,
    mean_prior: f64,
    std_prior: f64,
    expected: Vec<Expected>,
}

#[derive(Deserialize)]
struct Expected {
    t_start: u32,
    t_end: u32,
    mean: f64,
    std: f64,
    q025: f64,
    q975: f64,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!(
        "../../../data/fixtures/rt/epiestim_flu2009.json"
    ))
    .unwrap()
}

#[test]
fn reproduces_epiestim_flu2009_parametric_si_head() {
    let fx = fixture();
    let counts: Vec<Option<u32>> = fx.incidence.iter().map(|&c| Some(c)).collect();
    let si = SerialInterval {
        mean_days: fx.mean_si,
        sd_days: fx.std_si,
    }
    .discretize_daily_cori(fx.incidence.len() as u32)
    .unwrap();
    let cfg = RenewalConfig {
        window: fx.window_days,
        prior: GammaDist::from_mean_sd(fx.mean_prior, fx.std_prior).unwrap(),
        // EpiEstim only warns below its CV threshold; disable ours to compare every window.
        min_cases: 1,
        levels: vec![0.95],
        // EpiEstim sums infectivity over observed days only: zero before the series.
        before_series: BeforeSeries::Zero,
    };
    let est = estimate_series(&counts, &si, &cfg).unwrap();
    assert_eq!(fx.expected.len(), 6);
    for e in &fx.expected {
        assert_eq!(e.t_end - e.t_start + 1, fx.window_days);
        // EpiEstim indices are 1-based; `step` is the 0-based index of the window's last day.
        let s = &est[(e.t_end - 1) as usize];
        assert_eq!(s.status, RtStatus::Ok, "t_end={}: {:?}", e.t_end, s.reason);
        let post = s.posterior.unwrap();
        let ci = s.intervals[0];
        assert!(
            (post.mean() - e.mean).abs() < 1e-5,
            "t_end={} mean {} vs {}",
            e.t_end,
            post.mean(),
            e.mean
        );
        assert!(
            (post.sd() - e.std).abs() < 1e-4,
            "t_end={} sd {} vs {}",
            e.t_end,
            post.sd(),
            e.std
        );
        assert!(
            (ci.lower - e.q025).abs() < 1e-5,
            "t_end={} q025 {} vs {}",
            e.t_end,
            ci.lower,
            e.q025
        );
        assert!(
            (ci.upper - e.q975).abs() < 1e-5,
            "t_end={} q975 {} vs {}",
            e.t_end,
            ci.upper,
            e.q975
        );
    }
    // Windows that would start on day 1 (Λ undefined) are not estimated, as in EpiEstim.
    assert_eq!(
        est[(fx.window_days - 1) as usize].status,
        RtStatus::InsufficientData
    );
}
