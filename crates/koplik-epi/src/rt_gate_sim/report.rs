//! Markdown tables for the sizing and main stages (generated from the aggregates; nothing is
//! hand-edited).

use std::fmt::Write;

use super::agg::{
    CellAgg, Metrics, StageAgg, decide, metrics, pool_cells, s1_rows, s2_rows, scoreable,
    sizing_requirements,
};
use super::design::{
    BIN_NAMES, GenItem, HIGH_BINS, LOW_BINS, NBINS, REPLICATES, Stage, seed, variant_name,
};
use super::run::Failure;

fn f4(x: Option<f64>) -> String {
    x.map_or("missing".to_string(), |v| format!("{v:.4}"))
}

fn frac(num: u64, den: u64) -> String {
    if den == 0 {
        "missing".to_string()
    } else {
        format!("{:.4}", num as f64 / den as f64)
    }
}

pub fn cell_label(items: &[GenItem], c: &CellAgg) -> String {
    format!("{}/{}", items[c.item].label, variant_name(c.variant))
}

pub fn events_table(items: &[GenItem], agg: &StageAgg, stage: Stage) -> String {
    let mut s = String::new();
    writeln!(
        s,
        "| item | replicates | max events (replicate r, seed) | mean events | max pending beyond day 280 | mean pending | mean observed cases (40 wk) | notes |\n|---|---|---|---|---|---|---|---|"
    )
    .unwrap();
    for (i, it) in items.iter().enumerate() {
        let a = &agg.items[i];
        let n = u128::from(a.replicates.max(1));
        let mut notes = String::new();
        if it.scenario == 2 {
            let mean_week = if a.restart_replicates > 0 {
                format!(
                    "{:.2}",
                    a.restart_week_sum as f64 / f64::from(a.restart_replicates)
                )
            } else {
                "missing".to_string()
            };
            write!(
                notes,
                "extinction detected {}/{}; restart within horizon {}/{} (mean restart week {})",
                a.extinct_replicates, a.replicates, a.restart_replicates, a.replicates, mean_week
            )
            .unwrap();
        }
        if it.scenario == 4 {
            write!(
                notes,
                "week-40 held carry in {}/{} replicates, {} cases total",
                a.carry_replicates, a.replicates, a.carry_cases
            )
            .unwrap();
        }
        writeln!(
            s,
            "| {} | {} | {} (r={}, seed {}) | {:.1} | {} | {:.1} | {:.1} | {} |",
            it.label,
            a.replicates,
            a.events_max,
            a.events_max_replicate,
            seed(it.scenario, stage, a.events_max_replicate),
            (a.events_sum / n) as f64 + ((a.events_sum % n) as f64) / n as f64,
            a.pending_max,
            (a.pending_sum / n) as f64 + ((a.pending_sum % n) as f64) / n as f64,
            (a.observed_total_sum / n) as f64 + ((a.observed_total_sum % n) as f64) / n as f64,
            notes
        )
        .unwrap();
    }
    s
}

pub fn failures_section(failures: &[Failure]) -> String {
    if failures.is_empty() {
        return "No replicate failed, overflowed or was refused.\n".to_string();
    }
    let mut s =
        String::from("**Incomplete run: failed replicates** (scenario/cell/seed/cause):\n\n");
    for f in failures {
        writeln!(
            s,
            "- {} r={} seed={}: {}",
            f.label, f.replicate, f.seed, f.error
        )
        .unwrap();
    }
    s
}

/// Count tables (sizing and main share them): one row per analysis cell.
pub fn count_tables(items: &[GenItem], agg: &StageAgg) -> String {
    let mut s = String::new();
    writeln!(s, "Count-gate steps (scored weeks 7-40 with I >= 11) and baseline-scoreable steps (Lambda > 0, complete history), by Lambda bin. `unknown history` = count-gate steps whose eight-week look-back reaches before the series (Lambda unknown, unbinned); `Lambda=0` = count-gate steps with known history and Lambda = 0 (inside bin [0,0.25), not scoreable).\n").unwrap();
    write!(
        s,
        "| cell | scored steps | I>=11 | unknown history | Lambda=0 |"
    )
    .unwrap();
    for b in BIN_NAMES {
        write!(s, " I>=11 in {b} |").unwrap();
    }
    for b in BIN_NAMES {
        write!(s, " scoreable {b} |").unwrap();
    }
    writeln!(s, " scoreable [0,1) | scoreable [1,inf) | baseline-ok | reason MissingCount | reason BelowThreshold | reason NoInfectivity |").unwrap();
    write!(s, "|---|---|---|---|---|").unwrap();
    for _ in 0..(2 * NBINS + 6) {
        write!(s, "---|").unwrap();
    }
    writeln!(s).unwrap();
    for c in &agg.cells {
        let k = &c.counts;
        write!(
            s,
            "| {} | {} | {} | {} | {} |",
            cell_label(items, c),
            k.scored_steps,
            k.count_gate,
            k.unknown_history,
            k.zero_lambda
        )
        .unwrap();
        for b in 0..NBINS {
            write!(s, " {} |", k.bin_steps[b]).unwrap();
        }
        for b in 0..NBINS {
            write!(s, " {} |", k.bin_scoreable[b]).unwrap();
        }
        let lo: u64 = LOW_BINS.map(|b| k.bin_scoreable[b]).sum();
        let hi: u64 = HIGH_BINS.map(|b| k.bin_scoreable[b]).sum();
        writeln!(
            s,
            " {lo} | {hi} | {} | {} | {} | {} |",
            k.baseline_ok, k.reason_missing, k.reason_below, k.reason_no_infectivity
        )
        .unwrap();
    }
    s
}

pub fn s1_s2_count_tables(items: &[GenItem], agg: &StageAgg) -> String {
    let mut s = String::new();
    writeln!(
        s,
        "S1 pools (scenarios 2-5, all k cells, Unknown): scoreable steps\n"
    )
    .unwrap();
    writeln!(s, "| suite | scenario | scoreable [0,1) | scoreable [1,2) | both >= 200 |\n|---|---|---|---|---|").unwrap();
    for r in s1_rows(items, agg, false) {
        writeln!(
            s,
            "| {} | {} | {} | {} | {} |",
            r.suite, r.scenario, r.n_low, r.n_ref, r.evaluable
        )
        .unwrap();
    }
    writeln!(
        s,
        "\nS2 cells (scenario 1, k = inf): scoreable steps in [1,inf)\n"
    )
    .unwrap();
    writeln!(
        s,
        "| suite | R | before-series | scoreable [1,inf) | >= 200 |\n|---|---|---|---|---|"
    )
    .unwrap();
    for r in s2_rows(items, agg, false) {
        writeln!(
            s,
            "| {} | {} | {} | {} | {} |",
            r.suite,
            r.r,
            variant_name(r.variant),
            r.n_high,
            r.evaluable
        )
        .unwrap();
    }
    s
}

pub fn sizing_decision(items: &[GenItem], agg: &StageAgg, failures: &[Failure]) -> (bool, String) {
    let (p, q, s2) = sizing_requirements(items, agg);
    let go = failures.is_empty() && p && q && s2;
    let text = format!(
        "S1 sizing (>= 3 of scenarios 2-5 with >= 200 scoreable steps in both [0,1) and [1,2)): primary {}, sensitivity {}. S2 sizing (every scenario-1 k=inf cell >= 200 scoreable steps in [1,inf)): {}. Run complete: {}. Main run authorised by the registered sizing rule: **{}**.",
        if p { "met" } else { "NOT met" },
        if q { "met" } else { "NOT met" },
        if s2 { "met" } else { "NOT met" },
        failures.is_empty(),
        if go {
            "YES"
        } else {
            "NO (inconclusive-by-design)"
        }
    );
    (go, text)
}

fn metric_row(label: &str, bin: &str, m: &Metrics, bin_steps: u64, newly: u64) -> String {
    format!(
        "| {label} | {bin} | {bin_steps} | {} | {newly} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        m.n,
        frac(newly, bin_steps),
        frac(m.h95, m.n),
        f4(m.se95),
        frac(m.h50, m.n),
        f4(m.se50),
        m.nonempty,
        f4(m.median_err),
        f4(m.p90_err),
        frac(m.gt3, m.n),
    )
}

const METRIC_HEAD: &str = "| pool | bin | I>=11 in bin | scoreable n | newly withheld | newly withheld / I>=11 in bin | coverage 95% | MC SE 95% | coverage 50% | MC SE 50% | nonempty replicates | median abs log err | p90 abs log err | frac R-hat/R > 3 |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n";

fn pool_table(label: &str, pool: &[&CellAgg]) -> String {
    let mut s = String::new();
    let steps_in = |bins: std::ops::Range<usize>| -> u64 {
        pool.iter()
            .map(|c| bins.clone().map(|b| c.counts.bin_steps[b]).sum::<u64>())
            .sum()
    };
    let rows: Vec<(String, std::ops::Range<usize>)> = (0..NBINS)
        .map(|b| (BIN_NAMES[b].to_string(), b..b + 1))
        .chain([
            ("union [0,1)".to_string(), LOW_BINS),
            ("union [1,inf)".to_string(), HIGH_BINS),
        ])
        .collect();
    for (name, range) in rows {
        let m = metrics(pool, range.clone());
        let steps = steps_in(range.clone());
        // Newly withheld by the candidate floor: baseline-scoreable steps with Lambda < 1.
        let newly = if range.end <= LOW_BINS.end {
            scoreable(pool, range.clone())
        } else {
            0
        };
        s.push_str(&metric_row(label, &name, &m, steps, newly));
    }
    s
}

pub fn cell_summary_table(items: &[GenItem], agg: &StageAgg) -> String {
    let mut s = String::new();
    writeln!(s, "| cell | scored steps | I>=11 | baseline-ok | MissingCount | BelowThreshold | NoInfectivity | newly withheld (scoreable, Lambda<1) | newly / all I>=11 steps | newly / baseline-ok | candidate insufficient / all scored steps | candidate insufficient / I>=11 steps |\n|---|---|---|---|---|---|---|---|---|---|---|---|").unwrap();
    for c in &agg.cells {
        let k = &c.counts;
        let newly: u64 = LOW_BINS.map(|b| k.bin_scoreable[b]).sum();
        let cand_all = k.scored_steps - k.baseline_ok + newly;
        let cand_gate = k.count_gate - k.baseline_ok + newly;
        writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            cell_label(items, c),
            k.scored_steps,
            k.count_gate,
            k.baseline_ok,
            k.reason_missing,
            k.reason_below,
            k.reason_no_infectivity,
            newly,
            frac(newly, k.count_gate),
            frac(newly, k.baseline_ok),
            frac(cand_all, k.scored_steps),
            frac(cand_gate, k.count_gate)
        )
        .unwrap();
    }
    s
}

pub fn per_cell_metric_tables(items: &[GenItem], agg: &StageAgg) -> String {
    let mut s = String::from(METRIC_HEAD);
    for c in &agg.cells {
        s.push_str(&pool_table(&cell_label(items, c), &[c]));
    }
    s
}

pub fn pooled_metric_tables(items: &[GenItem], agg: &StageAgg) -> String {
    let mut s = String::new();
    for suite in ["primary", "sensitivity"] {
        for sc in 2..=5u8 {
            let pool = pool_cells(items, agg, suite, sc);
            writeln!(
                s,
                "\n#### Scenario {sc}, {suite} suite, all k cells pooled\n"
            )
            .unwrap();
            s.push_str(METRIC_HEAD);
            s.push_str(&pool_table(&format!("s{sc}/{suite}/all-k"), &pool));
        }
    }
    s
}

pub fn verdict_section(items: &[GenItem], agg: &StageAgg, failures: &[Failure]) -> String {
    let mut s = String::new();
    writeln!(s, "#### S1 (scenarios 2-5, pooled over k, per suite)\n").unwrap();
    writeln!(s, "| suite | scenario | scoreable [0,1) | cov95 [0,1) | scoreable [1,2) | cov95 [1,2) | n[0,1)>=200 | cov[0,1)<=0.80 | ref n>=200 and gap>=0.10 | meets S1 conditions |\n|---|---|---|---|---|---|---|---|---|---|").unwrap();
    for r in s1_rows(items, agg, true) {
        let c = |x: Option<(u64, u64)>| x.map_or("missing".to_string(), |(h, n)| frac(h, n));
        let b = |x: Option<bool>| x.map_or("not evaluable".to_string(), |v| v.to_string());
        writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.suite,
            r.scenario,
            r.n_low,
            c(r.cov_low),
            r.n_ref,
            c(r.cov_ref),
            r.c1_min_low,
            b(r.c2_cov_le_80),
            b(r.c3_gap_ge_10),
            b(r.meets)
        )
        .unwrap();
    }
    writeln!(
        s,
        "\n#### S2 (scenario 1, k = inf, per R, per before-series variant, per suite)\n"
    )
    .unwrap();
    writeln!(s, "| suite | R | before-series | scoreable [1,inf) | cov95 [1,inf) | >= 0.93 | context cov95 [1,2) | [2,5) | [5,inf) |\n|---|---|---|---|---|---|---|---|---|").unwrap();
    for r in s2_rows(items, agg, true) {
        let c = &agg.cells[r.cell];
        let ctx = |b: usize| {
            let m = metrics(&[c], b..b + 1);
            format!("{} (n={})", frac(m.h95, m.n), m.n)
        };
        writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.suite,
            r.r,
            variant_name(r.variant),
            r.n_high,
            r.cov_high
                .map_or("missing".to_string(), |(h, n)| frac(h, n)),
            r.passes
                .map_or("not evaluable".to_string(), |v| v.to_string()),
            ctx(3),
            ctx(4),
            ctx(5)
        )
        .unwrap();
    }
    let (outcome, why) = decide(items, agg, failures.is_empty());
    writeln!(s, "\n#### Outcome\n\n**{}**: {}", outcome.name(), why).unwrap();
    s
}

pub fn replicates_note() -> String {
    format!("{REPLICATES} replicates per generated item")
}
