//! Driver for the #1677 R_t gate study, step 2 (research only; never part of the pipeline).
//!
//! `rt_gate_sim lagtest`
//! `rt_gate_sim sizing --out DIR [--threads N]`
//! `rt_gate_sim main --out DIR [--threads N]`   (refuses unless DIR/sizing-decision.txt is GO)
//!
//! All output is derived from the aggregates; fragments are concatenated into the committed
//! results file without hand-editing.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use koplik_epi::rt_gate_sim::agg::{aggregate, decide};
use koplik_epi::rt_gate_sim::design::{Stage, manifest, seed};
use koplik_epi::rt_gate_sim::generator::{PRIMARY_SI, SENSITIVITY_SI, lag_test};
use koplik_epi::rt_gate_sim::report::*;
use koplik_epi::rt_gate_sim::run::run_stage;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let threads = arg(&args, "--threads")
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    match cmd {
        "lagtest" => {
            let mut failed = false;
            for si in [PRIMARY_SI, SENSITIVITY_SI] {
                match lag_test(&si) {
                    Ok(r) => println!("{r}"),
                    Err(r) => {
                        failed = true;
                        println!("{r}");
                    }
                }
            }
            if failed {
                std::process::exit(1);
            }
        }
        "sizing" | "main" => {
            let stage = if cmd == "sizing" {
                Stage::Sizing
            } else {
                Stage::Main
            };
            let out = PathBuf::from(arg(&args, "--out").expect("--out DIR"));
            fs::create_dir_all(&out).unwrap();
            if stage == Stage::Main {
                let d = fs::read_to_string(out.join("sizing-decision.txt")).unwrap_or_default();
                if d.trim() != "GO" {
                    eprintln!("refusing: sizing-decision.txt is not GO ({d:?})");
                    std::process::exit(2);
                }
            }
            let items = manifest();
            let t0 = Instant::now();
            eprintln!("{cmd}: {} items, {threads} threads", items.len());
            let (slots, failures) = run_stage(&items, stage, threads, true);
            let secs = t0.elapsed().as_secs_f64();
            let agg = aggregate(&items, &slots);
            let mut md = String::new();
            md.push_str(&format!(
                "Runtime {secs:.1} s wall-clock on {threads} threads.\n\n"
            ));
            md.push_str(&failures_section(&failures));
            md.push_str("\n### Maximum event counts per generated item\n\n");
            md.push_str(&events_table(&items, &agg, stage));
            md.push_str("\n### Count tables per analysis cell\n\n");
            md.push_str(&count_tables(&items, &agg));
            md.push_str("\n### Pools\n\n");
            md.push_str(&s1_s2_count_tables(&items, &agg));
            if stage == Stage::Sizing {
                let (go, text) = sizing_decision(&items, &agg, &failures);
                md.push_str(&format!("\n### Sizing decision\n\n{text}\n"));
                fs::write(
                    out.join("sizing-decision.txt"),
                    if go { "GO\n" } else { "NO\n" },
                )
                .unwrap();
                let mut csv =
                    String::from("item,label,replicate,seed,total_events,pending_events\n");
                // Sorted by item then replicate; failed replicates are absent and listed above.
                for (i, it) in items.iter().enumerate() {
                    for r in 0..2000u32 {
                        if let Some(o) = &slots[i * 2000 + r as usize] {
                            csv.push_str(&format!(
                                "{i},{},{r},{},{},{}\n",
                                it.label,
                                seed(it.scenario, stage, r),
                                o.total_events,
                                o.pending_events
                            ));
                        }
                    }
                }
                fs::write(out.join("sizing-events.csv"), csv).unwrap();
                fs::write(out.join("sizing.md"), md).unwrap();
                println!("sizing: {}", if go { "GO" } else { "NO" });
            } else {
                md.push_str(
                    "\n### Per-cell summary (newly withheld and insufficiency fractions)\n\n",
                );
                md.push_str(&cell_summary_table(&items, &agg));
                md.push_str("\n### Per-cell metrics by Lambda bin\n\n");
                md.push_str(&per_cell_metric_tables(&items, &agg));
                md.push_str("\n### Scenario pools for S1 (all k cells pooled, per suite)\n");
                md.push_str(&pooled_metric_tables(&items, &agg));
                md.push_str("\n### Decision tables\n\n");
                md.push_str(&verdict_section(&items, &agg, &failures));
                let (o, why) = decide(&items, &agg, failures.is_empty());
                fs::write(out.join("main.md"), md).unwrap();
                println!("main: {} ({why})", o.name());
            }
        }
        _ => {
            eprintln!(
                "usage: rt_gate_sim lagtest | sizing --out DIR | main --out DIR [--threads N]"
            );
            std::process::exit(64);
        }
    }
}
