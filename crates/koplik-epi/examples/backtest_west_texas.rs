//! Backtest of the renewal-projection forecast on the Texas DSHS 2025 West Texas outbreak
//! total, by report vintage. Reads a `koplik-ingest` vintage manifest, writes the full
//! report as JSON and prints the summary as Markdown.
//!
//! Usage: `cargo run -p koplik-epi --example backtest_west_texas -- <vintage-manifest.json> <out.json>`
//!
//! The protocol and every parameter are fixed in `koplik_epi::forecast` and
//! `koplik_epi::backtest::run` (pre-registered; nothing here is chosen from a score). The
//! window sensitivity is reported in full next to the pre-registered primary (window 3)
//! and does not change it.

use std::{env, fs, process};

use koplik_contracts::v3::GeoId;
use koplik_epi::backtest::manifest::outbreak_total_vintages;
use koplik_epi::backtest::run::{BacktestConfig, BacktestReport, run_backtest, wednesdays_between};
use koplik_epi::forecast::ForecastConfig;
use koplik_epi::rt::RenewalConfig;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Base seed of the backtest: an arbitrary fixed constant chosen before any run.
const SEED: u64 = 20_250_101;

#[derive(Serialize)]
struct Output {
    manifest_sha256: String,
    protocol: &'static str,
    scope: &'static str,
    primary: BacktestReport,
    /// Every other window, reported in full; the primary is not chosen from these.
    sensitivity_window: Vec<BacktestReport>,
}

fn fmt(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("{x:.digits$}"))
}

fn print_summary(label: &str, r: &BacktestReport) {
    println!(
        "### {label} (window {} wk, look-back {} wk, min cases {}, {} members, seed {})\n",
        r.window_weeks, r.max_lag_weeks, r.min_cases, r.run_count, r.seed
    );
    println!("| horizon | n | mean CRPS | 50% coverage | 90% coverage | persistence MAE |");
    println!("|---|---|---|---|---|---|");
    for s in r.by_horizon.iter().chain(std::iter::once(&r.pooled)) {
        let name = if s.horizon == 0 {
            "all".to_string()
        } else {
            s.horizon.to_string()
        };
        println!(
            "| {name} | {} | {} | {} | {} | {} |",
            s.n,
            fmt(s.mean_crps, 2),
            fmt(s.coverage_50, 2),
            fmt(s.coverage_90, 2),
            fmt(s.mean_persistence_abs_error, 2)
        );
    }
    println!();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: backtest_west_texas <vintage-manifest.json> <out.json>");
        process::exit(2);
    }
    let raw = fs::read(&args[1]).unwrap_or_else(|e| {
        eprintln!("cannot read {}: {e}", args[1]);
        process::exit(1);
    });
    let manifest_sha256 = hex::encode(Sha256::digest(&raw));
    let text = String::from_utf8(raw).expect("manifest is UTF-8");
    let vintages = outbreak_total_vintages(&text).unwrap_or_else(|e| {
        eprintln!("manifest: {e}");
        process::exit(1);
    });
    let start = vintages
        .iter()
        .map(|v| v.first_seen_at)
        .min()
        .expect("non-empty");
    let end = vintages
        .iter()
        .map(|v| v.first_seen_at)
        .max()
        .expect("non-empty");
    let geography: GeoId = "48".parse().unwrap();
    let run = |window: u32| {
        let cfg = BacktestConfig {
            forecast: ForecastConfig {
                renewal: RenewalConfig {
                    window,
                    ..RenewalConfig::default()
                },
                ..ForecastConfig::default()
            },
            seed: SEED,
            geography,
            forecast_dates: wednesdays_between(start, end),
        };
        run_backtest(&vintages, &cfg).unwrap_or_else(|e| {
            eprintln!("backtest: {e}");
            process::exit(1);
        })
    };
    let primary = run(ForecastConfig::DEFAULT_WINDOW_WEEKS);
    let sensitivity_window: Vec<BacktestReport> = [1, 2, 4]
        .into_iter()
        .filter(|w| *w != ForecastConfig::DEFAULT_WINDOW_WEEKS)
        .map(run)
        .collect();

    println!("manifest sha256: {manifest_sha256}");
    println!(
        "versions: {}; forecast dates: {} (Wednesdays 00:00 UTC from {} to {})\n",
        vintages.len(),
        primary.origins.len(),
        start.to_rfc3339(),
        end.to_rfc3339()
    );
    println!("| week | truth |");
    println!("|---|---|");
    for (w, c) in &primary.truth {
        println!(
            "| {w} | {} |",
            c.map_or("missing".to_string(), |c| c.to_string())
        );
    }
    println!();
    println!("| forecast date | origin | versions known | status | window cases | R mean [90%] |");
    println!("|---|---|---|---|---|---|");
    for o in &primary.origins {
        println!(
            "| {} | {} | {} | {}{} | {} | {} |",
            o.forecast_date.format("%Y-%m-%d"),
            o.origin_week.map_or("none".to_string(), |w| w.to_string()),
            o.known_versions,
            o.status,
            o.reason
                .as_ref()
                .map_or(String::new(), |r| format!(" ({r})")),
            o.cases_in_window
                .map_or("n/a".to_string(), |c| c.to_string()),
            match (o.r_mean, o.r_lower_90, o.r_upper_90) {
                (Some(m), Some(l), Some(u)) => format!("{m:.2} [{l:.2}, {u:.2}]"),
                _ => "n/a".to_string(),
            }
        );
    }
    println!();
    print_summary("Primary, pre-registered", &primary);
    for r in &sensitivity_window {
        print_summary("Sensitivity", r);
    }

    let out = Output {
        manifest_sha256,
        protocol: "real-time by report vintage: at each forecast date only versions with first_seen_at <= the date are used; truth is the final series from every version",
        scope: "Texas DSHS outbreak total (all outbreak-associated confirmed cases, every county combined), weekly by report date; no county-level backtest (county breakdowns are unavailable for 2025-03-28..2025-08-12, Issue #1402)",
        primary,
        sensitivity_window,
    };
    let json = serde_json::to_string_pretty(&out).expect("serialisable");
    fs::write(&args[2], json + "\n").unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", args[2]);
        process::exit(1);
    });
}
