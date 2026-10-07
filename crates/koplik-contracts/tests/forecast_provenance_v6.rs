//! Contract v6 `ForecastProvenance` (#1503): the per-series skill of a backtest over a family of
//! series. A series' skill is `measured` exactly when the backtest's entry for it carries scores,
//! which it does exactly when the pre-registered floor was reached; a pseudo-real-time backtest
//! says so; the pooled numbers add up. The v5 rules (rows, bands, the report-vintage backtest)
//! still hold.

use koplik_contracts::v6::*;
use serde_json::{Value, json};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NOT_TESTED: &str = "not backtested; no measured skill";
const MEASURED: &str = "measured";
const INSUFFICIENT: &str = "insufficient data for a measured skill";
const PSEUDO: &str = "pseudo-real-time (revised counts truncated at each forecast date)";
const LEVELS: [f64; 5] = [0.05, 0.25, 0.5, 0.75, 0.95];

fn origin() -> MmwrWeek {
    serde_json::from_value(json!({"year": 2026, "week": 36})).unwrap()
}

fn rows(geographies: &[&str]) -> Vec<Forecast> {
    let mut out = Vec::new();
    for g in geographies {
        let mut target = origin();
        for h in 1..=8u32 {
            target = target.next().unwrap();
            out.push(
                serde_json::from_value(json!({
                    "geography": g,
                    "origin_week": {"year": 2026, "week": 36},
                    "target_week": target,
                    "quantiles": LEVELS.iter().enumerate().map(|(i, l)| json!({"level": l, "value": f64::from(h) + i as f64})).collect::<Vec<_>>(),
                    "seed": 20250101u64, "run_count": 1000,
                    "provenance": [{"source_id": "cdc-nndss-weekly-measles", "url": "https://example.invalid/series",
                        "retrieved_at": "2026-10-07T02:19:30Z", "sha256": HASH, "licence_id": "cdc-open-data-terms-unconfirmed"}]
                }))
                .unwrap(),
            );
        }
    }
    out
}

fn horizons(n: &[u32]) -> Value {
    Value::Array(
        n.iter()
            .enumerate()
            .map(|(i, n)| {
                if *n == 0 {
                    json!({"horizon": i + 1, "n": 0, "mean_crps": null, "coverage_50": null, "coverage_90": null})
                } else {
                    json!({"horizon": i + 1, "n": n, "mean_crps": 2.5, "coverage_50": 0.5, "coverage_90": 1.0})
                }
            })
            .collect(),
    )
}

fn scores(per_horizon: &[u32]) -> Value {
    json!({
        "targets": per_horizon.iter().sum::<u32>(), "mean_crps": 2.5, "coverage_50": 0.5,
        "coverage_90": 1.0, "mean_persistence_abs_error": 4.0, "by_horizon": horizons(per_horizon)
    })
}

/// The floor is 6 targets from 3 origin weeks here (the real one is 40 from 10).
fn series_backtest() -> Value {
    json!({
        "name": "the CDC NNDSS state series",
        "series": "CDC NNDSS weekly measles counts of each state, by report week",
        "case_definition": "confirmed_or_unknown_status", "basis": PSEUDO,
        "protocol": format!("{PSEUDO}: one retrieval is held"),
        "seed": 20250101u64, "provisional_weeks": 2, "minimum_targets": 6, "minimum_origin_weeks": 3,
        "pooled": {"forecasts": 5, "series": 2, "scores": scores(&[5, 4, 0])},
        "by_series": [
            {"geography": "01", "forecasts": 0, "origin_weeks": 0, "targets": 0, "measured": null},
            {"geography": "12", "forecasts": 4, "origin_weeks": 3, "targets": 6, "measured": scores(&[3, 3, 0])},
            {"geography": "48", "forecasts": 2, "origin_weeks": 2, "targets": 3, "measured": null}
        ],
        "report_path": "data/reports/backtest/cdc-states.json", "report_sha256": HASH, "input_sha256": HASH,
        "limitations": ["pseudo-real-time"]
    })
}

/// A valid companion for [`rows`]`(&["12", "48"])`: "12" has a measured skill, "48" has
/// insufficient data for one, "01" was not forecast, "48001" is a county series nobody scored.
fn companion() -> Value {
    let mut series_backtest = series_backtest();
    // Pooled: 6 + 3 = 9 targets (horizon n 5,4,0); forecasts 3 + 2 = 5; series 2.
    series_backtest["pooled"]["scores"] = scores(&[5, 4, 0]);
    json!({
        "contract_version": 6, "artifact": "weekly-cases",
        "statement": "A forecast, not a fact", "method": "renewal projection",
        "origin_week": {"year": 2026, "week": 36}, "latest_data_week": {"year": 2026, "week": 38},
        "origin_rule": "two provisional weeks dropped", "horizon_weeks": 8, "run_count": 1000,
        "seed": 20250101u64, "levels": LEVELS,
        "input": {"artifact": "weekly-cases", "sha256": HASH, "rows": 6844},
        "parameters": [
            {"parameter": "window_weeks", "value": 3, "source": "a rule", "url": null, "note": "fixed before any score"}
        ],
        "series": [
            {"geography": "01", "case_definition": "confirmed_or_unknown_status", "status": "insufficient_data",
             "reason": "below_threshold", "cases_in_window": 2, "skill": INSUFFICIENT},
            {"geography": "12", "case_definition": "confirmed_or_unknown_status", "status": "forecast",
             "reason": null, "cases_in_window": 40, "skill": MEASURED},
            {"geography": "48", "case_definition": "confirmed_or_unknown_status", "status": "forecast",
             "reason": null, "cases_in_window": 15, "skill": INSUFFICIENT},
            {"geography": "48001", "case_definition": "confirmed", "status": "insufficient_data",
             "reason": "below_threshold", "cases_in_window": 1, "skill": NOT_TESTED}
        ],
        "backtest": null,
        "series_backtest": series_backtest,
        "scope_note": "see each series' skill"
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
fn a_valid_companion_round_trips_and_describes_its_rows() {
    let p = parse(companion()).unwrap();
    assert_eq!(serde_json::to_value(&p).unwrap(), companion());
    assert_eq!(p.contract_version, FORECAST_PROVENANCE_VERSION);
    assert_eq!(FORECAST_PROVENANCE_VERSION, 6);
    p.check_against(&rows(&["12", "48"])).unwrap();
    assert!(p.check_against(&rows(&["12"])).is_err());
    assert_eq!(p.series[1].skill, SeriesSkill::Measured);
    assert_eq!(p.series[2].skill, SeriesSkill::InsufficientData);
    assert_eq!(p.series[3].skill, SeriesSkill::NotBacktested);
}

#[test]
fn the_companion_can_have_no_backtest_at_all() {
    let mut v = companion();
    v["series_backtest"] = Value::Null;
    for s in v["series"].as_array_mut().unwrap() {
        s["skill"] = json!(NOT_TESTED);
    }
    assert!(parse(v).is_ok());
}

#[test]
fn a_pseudo_real_time_backtest_says_so_and_is_never_called_real_time() {
    // The two bases are the only ones, spelled out.
    assert_eq!(
        serde_json::to_value(InformationBasis::PseudoRealTime).unwrap(),
        json!(PSEUDO)
    );
    assert_eq!(
        serde_json::to_value(InformationBasis::RealTimeByVintage).unwrap(),
        json!("real-time by report vintage")
    );
    rejects(
        |v| v["series_backtest"]["basis"] = json!("real-time"),
        "unknown variant",
    );
    rejects(
        |v| v["series_backtest"]["protocol"] = json!("each forecast used only what was known"),
        "must say so",
    );
}

#[test]
fn a_measured_skill_needs_scores_and_exactly_when_the_floor_is_reached() {
    // "12" reaches 6 targets from 3 origin weeks: its scores must be there.
    rejects(
        |v| v["series_backtest"]["by_series"][1]["measured"] = Value::Null,
        "present exactly when the floor",
    );
    // "48" is below the floor: scores must not be published for it.
    rejects(
        |v| v["series_backtest"]["by_series"][2]["measured"] = scores(&[2, 1]),
        "present exactly when the floor",
    );
    // Scores that are not over all of the series' scored targets.
    rejects(
        |v| v["series_backtest"]["by_series"][1]["measured"] = scores(&[2, 2, 0]),
        "all the series' scored targets",
    );
    // A series cannot claim a measured skill its entry does not have, nor deny one it has.
    rejects(|v| v["series"][2]["skill"] = json!(MEASURED), "disagrees");
    rejects(
        |v| v["series"][1]["skill"] = json!(INSUFFICIENT),
        "disagrees",
    );
    // Nor cite a backtest that did not run on it.
    rejects(
        |v| {
            v["series"][3]["skill"] = json!(INSUFFICIENT);
            v["series"][3]["case_definition"] = json!("confirmed_or_unknown_status");
        },
        "did not run on it",
    );
    // Nor one of another case definition.
    rejects(
        |v| {
            v["series"][1]["case_definition"] = json!("confirmed");
        },
        "case definition",
    );
    rejects(
        |v| v["series_backtest"] = Value::Null,
        "cites a series backtest but there is none",
    );
}

#[test]
fn the_floor_and_the_pooled_numbers_are_validated() {
    rejects(
        |v| v["series_backtest"]["minimum_targets"] = json!(0),
        "at least 1",
    );
    rejects(
        |v| v["series_backtest"]["pooled"]["scores"] = scores(&[5, 5, 0]),
        "sum over the series",
    );
    rejects(
        |v| v["series_backtest"]["pooled"]["forecasts"] = json!(4),
        "scored origin weeks",
    );
    rejects(
        |v| v["series_backtest"]["pooled"]["series"] = json!(3),
        "series with a scored target",
    );
    rejects(
        |v| v["series_backtest"]["by_series"][2]["geography"] = json!("12"),
        "listed twice",
    );
    rejects(
        |v| v["series_backtest"]["by_series"][1]["origin_weeks"] = json!(5),
        "origin_weeks cannot exceed",
    );
    rejects(
        |v| v["series_backtest"]["by_series"][1]["measured"]["by_horizon"] = horizons(&[1, 1, 0]),
        "sum of the horizons",
    );
    rejects(
        |v| v["series_backtest"]["limitations"] = json!([]),
        "limitations",
    );
    rejects(
        |v| v["series_backtest"]["pooled"]["scores"]["coverage_90"] = json!(1.5),
        "share between 0 and 1",
    );
    // Without a pooled result (below the floor) the evaluation is still valid.
    let mut v = companion();
    v["series_backtest"]["pooled"] = Value::Null;
    assert!(parse(v).is_ok());
}

#[test]
fn the_report_vintage_backtest_still_speaks_only_for_the_series_it_scored() {
    rejects(
        |v| v["series"][3]["skill"] = json!("backtested"),
        "marked backtested but there is no backtest",
    );
}

#[test]
fn unknown_skill_statuses_and_fields_are_rejected() {
    rejects(
        |v| v["series"][0]["skill"] = json!("skilful"),
        "unknown variant",
    );
    rejects(
        |v| v["series_backtest"]["extra"] = json!(1),
        "unknown field",
    );
    rejects(|v| v["series"][1]["extra"] = json!(1), "unknown field");
}

/// The synthetic forecast the web tests and the dev server load (`web/scripts/build-synthetic-fixtures.mjs`)
/// is a valid v6 companion that describes its rows: the web's own checks mirror this contract.
#[test]
fn the_synthetic_web_fixture_is_a_valid_companion_that_describes_its_rows() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/fixtures/web/synthetic-v1");
    let companion: ForecastProvenance = serde_json::from_slice(
        &std::fs::read(dir.join("synthetic-forecast.provenance.json")).unwrap(),
    )
    .unwrap();
    let rows: Vec<Forecast> =
        serde_json::from_slice(&std::fs::read(dir.join("synthetic-forecast.json")).unwrap())
            .unwrap();
    companion.check_against(&rows).unwrap();
    let backtest = companion.series_backtest.as_ref().unwrap();
    assert_eq!(backtest.basis, InformationBasis::PseudoRealTime);
    let skills: Vec<_> = companion
        .series
        .iter()
        .map(|s| (s.geography.to_string(), s.skill))
        .collect();
    assert_eq!(
        skills,
        [
            ("20".to_owned(), SeriesSkill::InsufficientData),
            ("35".to_owned(), SeriesSkill::NotBacktested),
            ("40".to_owned(), SeriesSkill::NotBacktested),
            ("48".to_owned(), SeriesSkill::Measured),
        ]
    );
}
