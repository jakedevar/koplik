//! Replay the published Gaines County 2025 scenario and print the measured outcome beside the
//! DSHS counts. A report, not a fit: it reads the scenario exactly as published and changes
//! nothing about it (#1455).
//!
//! ```text
//! cargo run --release -p koplik-pipeline --example scenario_replay -- [--work DIR] [--out DIR]
//! ```
//!
//! The scenario and the DSHS cumulative series come from a finished pipeline run
//! (`make pipeline-fixtures`). Simulated *cumulative infections* include the initial seeding
//! and exclude vaccine immunity; DSHS counts are *confirmed cases reported by a date*. They are
//! different quantities and the comparison is only illustrative.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::NaiveDate;
use koplik_contracts::v1::ScenarioInput;
use koplik_pipeline::{DEFAULT_OUT, DEFAULT_WORK, SCENARIO_ARTIFACT};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut work = PathBuf::from(DEFAULT_WORK);
    let mut out = PathBuf::from(DEFAULT_OUT);
    while let Some(flag) = args.next() {
        let value = args.next().map(PathBuf::from);
        match (flag.as_str(), value) {
            ("--work", Some(v)) => work = v,
            ("--out", Some(v)) => out = v,
            _ => {
                eprintln!("usage: scenario_replay [--work DIR] [--out DIR]");
                return ExitCode::from(2);
            }
        }
    }
    match run(&work, &out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("scenario_replay: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(work: &PathBuf, out: &PathBuf) -> Result<(), String> {
    let path = out.join(format!("scenarios/{SCENARIO_ARTIFACT}.json"));
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let input: ScenarioInput =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let ensemble = koplik_epi::simulate_ensemble(&input).map_err(|e| e.to_string())?;
    let focus = input
        .nodes
        .iter()
        .find(|n| n.initial_infectious > 0)
        .ok_or("the scenario seeds no node")?;
    let start = input.start_week.start_date();

    // Observed: DSHS cumulative confirmed cases of the seeded county, by report date.
    let cumulative: Vec<serde_json::Value> = {
        let p = work.join("validate/dshs-cumulative.json");
        serde_json::from_slice(&fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?)
            .map_err(|e| e.to_string())?
    };
    let observed: Vec<(NaiveDate, u64)> = cumulative
        .iter()
        .filter(|r| r["geography"] == focus.id.to_string() && r["cases"]["status"] == "reported")
        .map(|r| {
            (
                r["report_date"]
                    .as_str()
                    .unwrap_or_default()
                    .parse()
                    .expect("date"),
                r["cases"]["count"].as_u64().expect("count"),
            )
        })
        .collect();

    println!(
        "scenario: {} nodes, start MMWR {}-W{:02} (day 0 = {start}), seed {}, {} runs, initial infectious {} in {}",
        input.nodes.len(),
        input.start_week.year,
        input.start_week.week,
        input.seed,
        input.run_count,
        focus.initial_infectious,
        focus.id
    );
    println!(
        "engine fingerprint (member 0): {}",
        ensemble.members[0].fingerprint
    );
    println!(
        "\nsimulated cumulative infections ({}), median [50% band] [90% band]",
        focus.id
    );
    println!(
        "{:>4} {:>10} {:>8} {:>22} {:>22}",
        "day", "date", "median", "50%", "90%"
    );
    for d in ensemble.daily.iter().filter(|d| {
        d.geography == focus.id && (d.day % 7 == 0 || d.day == input.parameters.horizon_days)
    }) {
        let b = d.cumulative_infections;
        println!(
            "{:>4} {:>10} {:>8.0} {:>10.0}-{:<11.0} {:>10.0}-{:<11.0}",
            d.day,
            start + chrono::Duration::days(i64::from(d.day)),
            b.median,
            b.lower_50,
            b.upper_50,
            b.lower_90,
            b.upper_90
        );
    }
    println!("\nobserved: DSHS cumulative confirmed cases reported by each report date");
    println!(
        "{:>10} {:>4} {:>9}  simulated cumulative infections at that day (median [90%])",
        "report", "day", "confirmed"
    );
    for (date, count) in &observed {
        let day = (*date - start).num_days();
        let sim = u32::try_from(day).ok().and_then(|day| {
            ensemble
                .daily
                .iter()
                .find(|d| d.geography == focus.id && d.day == day)
        });
        match sim {
            Some(s) => println!(
                "{date:>10} {day:>4} {count:>9}  {:.0} [{:.0}-{:.0}]",
                s.cumulative_infections.median,
                s.cumulative_infections.lower_90,
                s.cumulative_infections.upper_90
            ),
            None => println!(
                "{date:>10} {day:>4} {count:>9}  (after the {}-day horizon: not simulated)",
                input.parameters.horizon_days
            ),
        }
    }
    Ok(())
}
