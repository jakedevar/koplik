//! Contract v5 `ForecastProvenance`: round trip, rejection of malformed companions, and the
//! check that a companion describes the forecast rows it sits beside.

use koplik_contracts::v5::*;
use serde_json::{Value, json};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NOT_TESTED: &str = "not backtested; no measured skill";
const LEVELS: [f64; 5] = [0.05, 0.25, 0.5, 0.75, 0.95];

fn origin() -> MmwrWeek {
    serde_json::from_value(json!({"year": 2026, "week": 36})).unwrap()
}

fn rows_for(geography: &str, horizon: u32) -> Vec<Value> {
    let mut target = origin();
    (1..=horizon)
        .map(|h| {
            target = target.next().unwrap();
            json!({
                "geography": geography,
                "origin_week": {"year": 2026, "week": 36},
                "target_week": target,
                "quantiles": LEVELS.iter().enumerate().map(|(i, l)| json!({"level": l, "value": f64::from(h) + i as f64})).collect::<Vec<_>>(),
                "seed": 20250101u64,
                "run_count": 1000,
                "provenance": [{"source_id": "cdc-nndss-weekly-measles", "url": "https://example.invalid/series",
                    "retrieved_at": "2026-10-07T02:19:30Z", "sha256": HASH, "licence_id": "cdc-open-data-terms-unconfirmed"}]
            })
        })
        .collect()
}

fn rows(geographies: &[&str]) -> Vec<Forecast> {
    geographies
        .iter()
        .flat_map(|g| rows_for(g, 8))
        .map(|v| serde_json::from_value(v).unwrap())
        .collect()
}

fn skill() -> Value {
    json!({
        "name": "the 2025 West Texas outbreak",
        "series": "Texas DSHS outbreak total by report date", "geography": "48",
        "case_definition": "confirmed", "protocol": "real time by report vintage",
        "seed": 20250101u64, "targets": 3, "forecast_dates": 2, "origin_weeks": 2,
        "mean_crps": 3.5, "coverage_50": 0.5, "coverage_90": 0.6, "mean_persistence_abs_error": 5.0,
        "by_horizon": [
            {"horizon": 1, "n": 2, "mean_crps": 3.0, "coverage_50": 0.5, "coverage_90": 0.5},
            {"horizon": 2, "n": 1, "mean_crps": 4.5, "coverage_50": 0.0, "coverage_90": 1.0},
            {"horizon": 3, "n": 0, "mean_crps": null, "coverage_50": null, "coverage_90": null}
        ],
        "report_path": "data/reports/backtest/west-texas-2025.json", "report_sha256": HASH, "manifest_sha256": HASH,
        "limitations": ["one outbreak"]
    })
}

/// A valid companion for [`rows`]`(&["12", "48"])`; "01" was considered and not forecast.
fn companion() -> Value {
    json!({
        "contract_version": 5, "artifact": "weekly-cases",
        "statement": "A forecast, not a fact", "method": "renewal projection",
        "origin_week": {"year": 2026, "week": 36}, "latest_data_week": {"year": 2026, "week": 38},
        "origin_rule": "two provisional weeks dropped", "horizon_weeks": 8, "run_count": 1000,
        "seed": 20250101u64, "levels": LEVELS,
        "input": {"artifact": "weekly-cases", "sha256": HASH, "rows": 6844},
        "parameters": [
            {"parameter": "window_weeks", "value": 3, "source": "a rule", "url": null, "note": "fixed before any score"},
            {"parameter": "min_cases", "value": 11, "source": "EpiEstim", "url": null, "note": "CV 0.3"}
        ],
        "series": [
            {"geography": "01", "case_definition": "confirmed_or_unknown_status", "status": "insufficient_data",
             "reason": "below_threshold", "cases_in_window": 2, "skill": NOT_TESTED},
            {"geography": "12", "case_definition": "confirmed_or_unknown_status", "status": "forecast",
             "reason": null, "cases_in_window": 40, "skill": NOT_TESTED},
            {"geography": "48", "case_definition": "confirmed_or_unknown_status", "status": "forecast",
             "reason": null, "cases_in_window": 15, "skill": NOT_TESTED}
        ],
        "backtest": skill(),
        "scope_note": "none of these series was backtested"
    })
}

fn parse(v: Value) -> Result<ForecastProvenance, String> {
    serde_json::from_value(v).map_err(|e| e.to_string())
}

fn rejects(edit: impl FnOnce(&mut Value), expect: &str) {
    let mut v = companion();
    edit(&mut v);
    let e = parse(v).expect_err("must be rejected");
    assert!(e.contains(expect), "{e:?} should mention {expect:?}");
}

#[test]
fn a_valid_companion_round_trips() {
    let p = parse(companion()).unwrap();
    assert_eq!(serde_json::to_value(&p).unwrap(), companion());
    assert_eq!(p.series.len(), 3);
    assert_eq!(p.contract_version, FORECAST_PROVENANCE_VERSION);
}

#[test]
fn a_companion_without_a_backtest_is_valid() {
    let mut v = companion();
    v["backtest"] = Value::Null;
    assert!(parse(v).is_ok());
}

#[test]
fn malformed_companions_are_rejected() {
    rejects(
        |v| v["contract_version"] = json!(4),
        "contract_version must be 5",
    );
    rejects(
        |v| v["statement"] = json!("  "),
        "statement must not be empty",
    );
    rejects(
        |v| v["scope_note"] = json!(""),
        "scope_note must not be empty",
    );
    rejects(|v| v["horizon_weeks"] = json!(0), "horizon_weeks");
    rejects(|v| v["run_count"] = json!(0), "run_count");
    rejects(
        |v| v["levels"] = json!([0.5, 0.5]),
        "levels must increase strictly",
    );
    rejects(
        |v| v["levels"] = json!([0.5, 1.0]),
        "levels must increase strictly",
    );
    rejects(
        |v| v["latest_data_week"] = json!({"year": 2026, "week": 30}),
        "latest_data_week",
    );
    rejects(
        |v| v["parameters"] = json!([]),
        "parameters must not be empty",
    );
    rejects(
        |v| v["parameters"][1]["parameter"] = json!("window_weeks"),
        "cited twice",
    );
    rejects(
        |v| v["parameters"][0]["source"] = json!(""),
        "source must not be empty",
    );
    rejects(
        |v| v["series"][1]["geography"] = json!("01"),
        "listed twice",
    );
    rejects(|v| v["unexpected"] = json!(1), "unknown field");
}

#[test]
fn a_series_states_its_status_and_reason_consistently() {
    rejects(|v| v["series"][0]["reason"] = Value::Null, "needs a reason");
    rejects(
        |v| v["series"][1]["reason"] = json!("missing_count"),
        "has no reason",
    );
    rejects(
        |v| v["series"][1]["cases_in_window"] = Value::Null,
        "states the cases in its window",
    );
}

#[test]
fn backtest_skill_is_validated_and_only_the_scored_series_can_claim_it() {
    rejects(|v| v["backtest"]["coverage_90"] = json!(1.5), "coverage_90");
    rejects(|v| v["backtest"]["mean_crps"] = json!(-1.0), "mean_crps");
    rejects(|v| v["backtest"]["name"] = json!(" "), "backtest.name");
    rejects(|v| v["backtest"]["targets"] = json!(0), "targets");
    rejects(
        |v| v["backtest"]["targets"] = json!(4),
        "sum of the horizons",
    );
    rejects(|v| v["backtest"]["limitations"] = json!([]), "limitations");
    rejects(
        |v| v["backtest"]["by_horizon"][1]["horizon"] = json!(5),
        "horizons 1, 2",
    );
    rejects(
        |v| v["backtest"]["by_horizon"][2]["mean_crps"] = json!(1.0),
        "no targets",
    );
    rejects(
        |v| v["backtest"]["by_horizon"][0]["coverage_50"] = Value::Null,
        "lacks its coverage_50",
    );
    rejects(|v| v["backtest"]["origin_weeks"] = json!(3), "origin_weeks");
    // A series that is not the scored one cannot be marked backtested ...
    rejects(
        |v| v["series"][2]["skill"] = json!("backtested"),
        "not the backtested series",
    );
    // ... nor any series when there is no backtest at all.
    rejects(
        |v| {
            v["backtest"] = Value::Null;
            v["series"][2]["skill"] = json!("backtested");
        },
        "there is no backtest",
    );
    // The scored series (same geography and definition) may.
    let mut v = companion();
    v["series"][2]["case_definition"] = json!("confirmed");
    v["series"][2]["skill"] = json!("backtested");
    assert!(parse(v).is_ok());
}

#[test]
fn a_companion_describes_exactly_the_rows_beside_it() {
    let p = parse(companion()).unwrap();
    p.check_against(&rows(&["12", "48"])).unwrap();

    let differs = |edit: &dyn Fn(&mut Vec<Forecast>), what: &str| {
        let mut r = rows(&["12", "48"]);
        edit(&mut r);
        let e = p.check_against(&r).expect_err("must differ");
        assert!(e.contains(what), "{e:?} should mention {what:?}");
    };
    differs(&|r| r[0].seed = 1, "seed");
    differs(&|r| r[0].run_count = 10, "run_count");
    differs(
        &|r| r[0].origin_week = r[0].origin_week.prev().unwrap(),
        "origin week",
    );
    differs(
        &|r| {
            r[0].quantiles.pop();
        },
        "quantile levels",
    );
    differs(&|r| r[0].quantiles[0].level = 0.01, "quantile levels");
    differs(&|r| r.truncate(8), "series that were forecast");
    differs(
        &|r| {
            r.remove(3);
        },
        "horizons",
    );
    differs(&|r| r[2].target_week = r[3].target_week, "target weeks");
    // A series that was not forecast has no rows.
    let mut with_insufficient = rows(&["12", "48"]);
    with_insufficient.extend(rows(&["01"]));
    let e = p.check_against(&with_insufficient).unwrap_err();
    assert!(e.contains("series that were forecast"), "{e}");
    // A forecast series without rows is as wrong.
    assert!(p.check_against(&[]).is_err());
}

#[test]
fn rows_need_the_bands_the_page_draws() {
    let mut v = companion();
    v["levels"] = json!([0.1, 0.5, 0.9]);
    let p = parse(v).unwrap();
    let mut r = rows(&["12", "48"]);
    for row in &mut r {
        row.quantiles = vec![
            ForecastQuantile {
                level: 0.1,
                value: 1.0,
            },
            ForecastQuantile {
                level: 0.5,
                value: 2.0,
            },
            ForecastQuantile {
                level: 0.9,
                value: 3.0,
            },
        ];
    }
    assert!(p.check_against(&r).unwrap_err().contains("bands"));
}

#[test]
fn nothing_forecast_is_a_valid_empty_set_of_rows() {
    let mut v = companion();
    for s in v["series"].as_array_mut().unwrap() {
        s["status"] = json!("insufficient_data");
        s["reason"] = json!("below_threshold");
    }
    let p = parse(v).unwrap();
    p.check_against(&[]).unwrap();
    assert!(p.check_against(&rows(&["12"])).is_err());
}
