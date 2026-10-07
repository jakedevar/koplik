//! Backtest the published forecast on the CDC NNDSS state series (#1503), pseudo-real-time
//! (revised counts truncated at each forecast date), and write the report.
//!
//! ```text
//! cargo run --release -p koplik-pipeline --example backtest_cdc_states -- \
//!     data/fixtures/cdc/nndss-measles-weekly.retrieval.json data/reports/backtest/cdc-states.json
//! ```
//!
//! Offline: the snapshot is the committed real-byte fixture next to the retrieval record, and is
//! re-hashed against it. The protocol and every parameter are fixed in
//! `koplik_pipeline::nndss_backtest` and `koplik_epi::backtest::truncated`, pre-registered in
//! `thoughts/shared/research/backtest-cdc-states.md`; nothing here is chosen from a score. The
//! run is deterministic: the same snapshot gives a byte-identical report.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use koplik_epi::backtest::run::Summary;
use koplik_ingest::store::Retrieval;
use koplik_pipeline::nndss_backtest::{Report, run};

fn fmt(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| "n/a".to_owned(), |x| format!("{x:.digits$}"))
}

fn row(name: &str, s: &Summary, ratio: Option<f64>) -> String {
    format!(
        "| {name} | {} | {} | {} | {} | {} | {} |",
        s.n,
        fmt(s.mean_crps, 2),
        fmt(s.mean_persistence_abs_error, 2),
        fmt(ratio, 2),
        fmt(s.coverage_50, 2),
        fmt(s.coverage_90, 2)
    )
}

fn print(report: &Report) {
    let p = &report.primary;
    println!(
        "snapshot sha256 {} (retrieved {}); {} weekly rows; weeks {} to {}; truth through {}",
        report.input.sha256,
        report.input.retrieved_at,
        report.input.rows,
        p.first_week,
        p.last_week,
        p.last_truth_week
    );
    println!(
        "window {} wk, look-back {} wk, min cases {}, {} members, seed {}, provisional weeks {}; floor: {} targets from {} origin weeks\n",
        p.window_weeks,
        p.max_lag_weeks,
        p.min_cases,
        p.run_count,
        p.seed,
        p.provisional_weeks,
        p.floor.min_targets,
        p.floor.min_origin_weeks
    );
    println!(
        "| horizon | n | mean CRPS | persistence MAE | CRPS/persistence | 50% coverage | 90% coverage |"
    );
    println!("|---|---|---|---|---|---|---|");
    for h in &p.by_horizon {
        let ratio = match (h.mean_crps, h.mean_persistence_abs_error) {
            (Some(c), Some(m)) if m > 0.0 => Some(c / m),
            _ => None,
        };
        println!("{}", row(&h.horizon.to_string(), h, ratio));
    }
    println!("{}", row("**all**", &p.pooled, p.crps_over_persistence));
    println!(
        "\npooled: {} scored targets from {} (series, origin) forecasts across {} series; pooled measured: {}; series measured: {}\n",
        p.pooled.n, p.forecasts_scored, p.series_scored, p.pooled_measured, p.series_measured
    );
    println!(
        "| series | origins considered | forecast | origin weeks scored | n | mean CRPS | persistence MAE | CRPS/persistence | 50% | 90% | measured |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for s in p.series.iter().filter(|s| s.pooled.n > 0) {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            s.geography,
            s.origins_considered,
            s.origins_forecast,
            s.origin_weeks_scored,
            s.pooled.n,
            fmt(s.pooled.mean_crps, 2),
            fmt(s.pooled.mean_persistence_abs_error, 2),
            fmt(s.crps_over_persistence, 2),
            fmt(s.pooled.coverage_50, 2),
            fmt(s.pooled.coverage_90, 2),
            if s.measured { "yes" } else { "no" }
        );
    }
    // Descriptive only, added after the first run and never used for a decision or a published
    // number: how often a forecast's CRPS was below the persistence baseline's absolute error,
    // and the origins the method refused to forecast.
    let (mut scored, mut better) = (0_u32, 0_u32);
    for s in &p.series {
        for t in s.origins.iter().flat_map(|o| o.scores.iter()) {
            if let (Some(crps), Some(persistence)) = (t.crps, t.persistence_abs_error) {
                scored += 1;
                better += u32::from(crps < persistence);
            }
        }
    }
    let considered: u32 = p.series.iter().map(|s| s.origins_considered).sum();
    let forecast: u32 = p.series.iter().map(|s| s.origins_forecast).sum();
    let mut why: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    for s in &p.series {
        for (reason, n) in &s.not_forecast {
            *why.entry(reason.as_str()).or_default() += n;
        }
    }
    println!(
        "\ndescriptive: forecast CRPS below the persistence error in {better} of {scored} scored targets; {considered} origins considered, {forecast} forecast, none: {why:?}"
    );
    let unscored: Vec<String> = p
        .series
        .iter()
        .filter(|s| s.pooled.n == 0)
        .map(|s| s.geography.to_string())
        .collect();
    println!(
        "\n{} series with no scored target: {}",
        unscored.len(),
        unscored.join(", ")
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [retrieval, out] = args.as_slice() else {
        eprintln!("usage: backtest_cdc_states <snapshot.retrieval.json> <out.json>");
        return ExitCode::from(2);
    };
    match go(Path::new(retrieval), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("backtest_cdc_states: {e}");
            ExitCode::FAILURE
        }
    }
}

fn go(retrieval_path: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let record: Retrieval = serde_json::from_slice(&fs::read(retrieval_path)?)?;
    // The bytes sit next to the record: `x.retrieval.json` describes `x.json`.
    let name = retrieval_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".retrieval.json"))
        .ok_or("the retrieval record must be named <snapshot>.retrieval.json")?;
    let bytes_path: PathBuf = retrieval_path.with_file_name(format!("{name}.json"));
    let bytes = fs::read(&bytes_path)?;
    let report = run(&bytes, &record)?;
    let mut json = serde_json::to_vec(&report)?;
    json.push(b'\n');
    fs::write(out, &json)?;
    print(&report);
    eprintln!("wrote {} ({} bytes)", out.display(), json.len());
    Ok(())
}
