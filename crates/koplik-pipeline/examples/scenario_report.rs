//! Report what the published Gaines County what-if scenario does, as measured (#1455).
//!
//! ```text
//! cargo run --release -p koplik-pipeline --example scenario_report -- [--out DIR]
//! ```
//!
//! The scenario is a hypothetical introduction (one assumed infectious person), not a
//! reconstruction of the 2025 outbreak, and this report compares it with nothing: it prints the
//! simulated ensemble exactly as the engine produces it from the scenario a finished pipeline run
//! published (`make pipeline-fixtures`). Simulated *cumulative infections* include the introduced
//! people and exclude vaccine immunity; they are not reported cases.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use koplik_contracts::v1::ScenarioInput;
use koplik_pipeline::{DEFAULT_OUT, SCENARIO_ARTIFACT};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut out = PathBuf::from(DEFAULT_OUT);
    while let Some(flag) = args.next() {
        match (flag.as_str(), args.next().map(PathBuf::from)) {
            ("--out", Some(v)) => out = v,
            _ => {
                eprintln!("usage: scenario_report [--out DIR]");
                return ExitCode::from(2);
            }
        }
    }
    match run(&out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("scenario_report: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(out: &PathBuf) -> Result<(), String> {
    let path = out.join(format!("scenarios/{SCENARIO_ARTIFACT}.json"));
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let input: ScenarioInput =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let ensemble = koplik_epi::simulate_ensemble(&input).map_err(|e| e.to_string())?;
    let (focus_index, focus) = input
        .nodes
        .iter()
        .enumerate()
        .find(|(_, n)| n.initial_infectious > 0 || n.initial_exposed > 0)
        .ok_or("the scenario introduces nobody")?;
    let start = input.start_week.start_date();
    println!(
        "scenario: {} node(s), reference week MMWR {} (day 0 = {start}), seed {}, {} runs, {} infectious and {} exposed introduced into {}, population {}",
        input.nodes.len(),
        input.start_week,
        input.seed,
        input.run_count,
        focus.initial_infectious,
        focus.initial_exposed,
        focus.id,
        focus.population
    );
    println!(
        "engine fingerprint (member 0): {}",
        ensemble.members[0].fingerprint
    );
    println!(
        "\nsimulated cumulative infections in {}, median [50% band] [90% band]",
        focus.id
    );
    println!(
        "{:>4} {:>10} {:>8} {:>22} {:>22}",
        "day", "date", "median", "50%", "90%"
    );
    for d in ensemble.daily.iter().filter(|d| {
        d.geography == focus.id && (d.day % 14 == 0 || d.day == input.parameters.horizon_days)
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
    // How many of the runs end with the introduction fizzling out, and how large the rest get.
    let introduced = u64::from(focus.initial_infectious) + u64::from(focus.initial_exposed);
    let mut totals: Vec<u64> = ensemble
        .members
        .iter()
        .map(|m| {
            let first = m.steps[0].nodes[focus_index].susceptible;
            let last = m.steps.last().expect("a trajectory has steps").nodes[focus_index];
            first - last.susceptible + introduced
        })
        .collect();
    totals.sort_unstable();
    let fizzled = totals.iter().filter(|t| **t < introduced + 10).count();
    let n = totals.len();
    let median = if n % 2 == 1 {
        totals[n / 2] as f64
    } else {
        (totals[n / 2 - 1] + totals[n / 2]) as f64 / 2.0
    };
    println!(
        "\nat day {}: {fizzled} of {n} runs ended with fewer than 10 infections beyond the introduced people; smallest {}, largest {}, median {median}",
        input.parameters.horizon_days,
        totals[0],
        totals[n - 1],
    );
    Ok(())
}
