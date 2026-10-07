//! Contract v7 `ForecastProvenance` (#1503): the per-series skill of a backtest over a family of
//! series. A series' skill is `measured` exactly when the backtest's entry for it carries scores,
//! which it does exactly when the pre-registered floor was reached; a pseudo-real-time backtest
//! says so; the pooled numbers add up. The v5 rules (rows, bands, the report-vintage backtest)
//! still hold.

use koplik_contracts::v7::*;
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

fn policy() -> Value {
    // The evidence floor here matches the invented series backtest's (6 targets from 3 origin weeks).
    json!({"rule": "published only where the measured skill meets the criterion",
           "minimum_targets": 6, "minimum_origin_weeks": 3,
           "minimum_coverage_90": 0.75, "maximum_crps_over_persistence": 1.0})
}

/// A valid companion for [`rows`]`(&["12"])`: "12" has a measured skill the policy admits, so it is
/// published; "48" has insufficient data for one, so its forecast is withheld; "01" was not
/// forecast; "48001" is a county series nobody scored.
fn companion() -> Value {
    let mut series_backtest = series_backtest();
    // Pooled: 6 + 3 = 9 targets (horizon n 5,4,0); forecasts 3 + 2 = 5; series 2.
    series_backtest["pooled"]["scores"] = scores(&[5, 4, 0]);
    json!({
        "contract_version": 7, "artifact": "weekly-cases",
        "statement": "A forecast, not a fact", "method": "renewal projection",
        "origin_week": {"year": 2026, "week": 36}, "latest_data_week": {"year": 2026, "week": 38},
        "origin_rule": "two provisional weeks dropped", "horizon_weeks": 8, "run_count": 1000,
        "seed": 20250101u64, "levels": LEVELS,
        "input": {"artifact": "weekly-cases", "sha256": HASH, "rows": 6844},
        "parameters": [
            {"parameter": "window_weeks", "value": 3, "source": "a rule", "url": null, "note": "fixed before any score"}
        ],
        "publication_policy": policy(),
        "series": [
            {"geography": "01", "case_definition": "confirmed_or_unknown_status", "status": "insufficient_data",
             "reason": "below_threshold", "withheld": null, "cases_in_window": 2, "skill": INSUFFICIENT},
            {"geography": "12", "case_definition": "confirmed_or_unknown_status", "status": "forecast",
             "reason": null, "withheld": null, "cases_in_window": 40, "skill": MEASURED},
            {"geography": "48", "case_definition": "confirmed_or_unknown_status", "status": "withheld",
             "reason": null, "withheld": "insufficient_data_for_skill", "cases_in_window": 15, "skill": INSUFFICIENT},
            {"geography": "48001", "case_definition": "confirmed", "status": "insufficient_data",
             "reason": "below_threshold", "withheld": null, "cases_in_window": 1, "skill": NOT_TESTED}
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
    assert_eq!(FORECAST_PROVENANCE_VERSION, 7);
    p.check_against(&rows(&["12"])).unwrap();
    // The withheld series has no rows: a companion that withholds a series does not describe its rows.
    assert!(p.check_against(&rows(&["12", "48"])).is_err());
    assert!(p.check_against(&[]).is_err());
    assert_eq!(p.series[1].skill, SeriesSkill::Measured);
    assert_eq!(p.series[2].skill, SeriesSkill::InsufficientData);
    assert_eq!(p.series[3].skill, SeriesSkill::NotBacktested);
}

#[test]
fn the_companion_can_have_no_backtest_at_all_and_then_withholds_every_forecast() {
    let mut v = companion();
    v["series_backtest"] = Value::Null;
    for s in v["series"].as_array_mut().unwrap() {
        s["skill"] = json!(NOT_TESTED);
    }
    // Without a measured skill nothing is published: the forecast series is withheld, not shown.
    v["series"][1]["status"] = json!("withheld");
    v["series"][1]["withheld"] = json!("not_backtested");
    v["series"][2]["withheld"] = json!("not_backtested");
    let p = parse(v).unwrap();
    p.check_against(&[]).unwrap();
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
/// is a valid v7 companion that describes its rows: the web's own checks mirror this contract.
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
    // Texas is published (its measured skill meets the policy); Kansas is measured but below it, so withheld.
    let status = |g: &str| {
        companion
            .series
            .iter()
            .find(|s| s.geography.to_string() == g)
            .map(|s| (s.status, s.withheld))
            .unwrap()
    };
    assert_eq!(status("48"), (ForecastStatus::Forecast, None));
    assert_eq!(
        status("20"),
        (
            ForecastStatus::Withheld,
            Some(WithheldReason::SkillBelowPolicy)
        )
    );
    let skills: Vec<_> = companion
        .series
        .iter()
        .map(|s| (s.geography.to_string(), s.skill))
        .collect();
    assert_eq!(
        skills,
        [
            ("20".to_owned(), SeriesSkill::Measured),
            ("35".to_owned(), SeriesSkill::NotBacktested),
            ("40".to_owned(), SeriesSkill::NotBacktested),
            ("48".to_owned(), SeriesSkill::Measured),
        ]
    );
}

fn policy_value() -> PublicationPolicy {
    serde_json::from_value(policy()).unwrap()
}

#[test]
fn the_policy_admits_exactly_what_it_states() {
    let p = policy_value();
    // (targets, origin weeks, 90% coverage, mean CRPS, persistence error); the floor is 6 from 3.
    assert!(
        p.admits(6, 3, 0.75, 4.0, 4.0),
        "the thresholds themselves are admitted"
    );
    assert!(p.admits(60, 30, 1.0, 0.0, 0.0));
    assert!(
        !p.admits(6, 3, 0.7499, 1.0, 4.0),
        "coverage below the floor"
    );
    assert!(!p.admits(6, 3, 0.9, 4.0001, 4.0), "worse than persistence");
    assert!(
        !p.admits(6, 3, 0.9, 1.0, 0.0),
        "any error against a zero baseline is worse"
    );
    // The evidence floor, at its boundaries: one target or one origin week short is not enough, however good the scores.
    assert!(!p.admits(5, 3, 1.0, 0.0, 4.0), "5 targets, floor 6");
    assert!(!p.admits(6, 2, 1.0, 0.0, 4.0), "2 origin weeks, floor 3");
    assert!(!p.admits(1, 1, 1.0, 0.0, 4.0));
    let lenient = PublicationPolicy {
        maximum_crps_over_persistence: 2.0,
        ..p
    };
    assert!(lenient.admits(6, 3, 0.8, 8.0, 4.0));
    assert!(!lenient.admits(6, 3, 0.8, 8.1, 4.0));
}

/// A report-vintage backtest (`BacktestSkill`) of the Texas DSHS outbreak total, invented numbers.
fn west_texas_skill() -> Value {
    json!({
        "name": "the 2025 West Texas outbreak",
        "series": "Texas DSHS outbreak total by report date", "geography": "48",
        "case_definition": "confirmed", "protocol": "real time by report vintage",
        "seed": 20250101u64, "targets": 3, "forecast_dates": 2, "origin_weeks": 2,
        "mean_crps": 3.5, "coverage_50": 0.5, "coverage_90": 0.6, "mean_persistence_abs_error": 5.0,
        "by_horizon": [{"horizon": 1, "n": 3, "mean_crps": 3.5, "coverage_50": 0.5, "coverage_90": 0.6}],
        "report_path": "data/reports/backtest/west-texas-2025.json", "report_sha256": HASH, "manifest_sha256": HASH,
        "limitations": ["one outbreak"]
    })
}

/// A companion publishing the series the report-vintage backtest scored (`backtested`, geography 48,
/// confirmed), with a 40-target, 10-origin-week evidence floor, and scores that meet the rest of
/// the policy. The evidence is `targets` scored targets from `origin_weeks` distinct origin weeks.
fn legacy_companion(targets: u32, origin_weeks: u32, status: &str, withheld: Value) -> Value {
    let mut skill = west_texas_skill();
    skill["targets"] = json!(targets);
    skill["forecast_dates"] = json!(origin_weeks);
    skill["origin_weeks"] = json!(origin_weeks);
    skill["coverage_90"] = json!(0.9);
    skill["coverage_50"] = json!(0.5);
    skill["mean_crps"] = json!(1.0);
    skill["mean_persistence_abs_error"] = json!(5.0);
    skill["by_horizon"] = json!([{"horizon": 1, "n": targets, "mean_crps": 1.0, "coverage_50": 0.5, "coverage_90": 0.9}]);
    let mut v = companion();
    v["publication_policy"]["minimum_targets"] = json!(40);
    v["publication_policy"]["minimum_origin_weeks"] = json!(10);
    v["series_backtest"] = Value::Null;
    v["backtest"] = skill;
    v["series"] = json!([{"geography": "48", "case_definition": "confirmed", "status": status,
        "reason": null, "withheld": withheld, "cases_in_window": 15, "skill": "backtested"}]);
    v
}

#[test]
fn the_evidence_floor_applies_to_the_report_vintage_backtest_too() {
    // 40 targets from 10 origin weeks: the floor itself is enough, and the series is published.
    let at_floor = parse(legacy_companion(40, 10, "forecast", Value::Null)).unwrap();
    at_floor.check_against(&rows(&["48"])).unwrap();
    // One target short, or one origin week short, cannot be published, however good its scores ...
    for (targets, origin_weeks) in [(39, 10), (40, 9), (1, 1), (39, 9)] {
        let e = parse(legacy_companion(
            targets,
            origin_weeks,
            "forecast",
            Value::Null,
        ))
        .expect_err("below the evidence floor");
        assert!(
            e.contains("does not meet the publication policy"),
            "{targets} targets from {origin_weeks} origin weeks: {e}"
        );
        // ... it is withheld, for having a measured skill below the policy.
        let w = parse(legacy_companion(
            targets,
            origin_weeks,
            "withheld",
            json!("skill_below_policy"),
        ))
        .unwrap();
        assert_eq!(w.series[0].withheld, Some(WithheldReason::SkillBelowPolicy));
        w.check_against(&[]).unwrap();
    }
    // At the floor it cannot be withheld: the policy admits it.
    let e = parse(legacy_companion(
        40,
        10,
        "withheld",
        json!("skill_below_policy"),
    ))
    .unwrap_err();
    assert!(
        e.contains("meets the publication policy but is withheld"),
        "{e}"
    );
}

#[test]
fn the_evidence_floor_applies_to_the_series_backtest_too() {
    // "12" has 6 targets from 3 origin weeks; a policy asking for one more of either refuses it.
    rejects(
        |v| v["publication_policy"]["minimum_targets"] = json!(7),
        "does not meet the publication policy",
    );
    rejects(
        |v| v["publication_policy"]["minimum_origin_weeks"] = json!(4),
        "does not meet the publication policy",
    );
    // A floor of nothing is not a policy.
    rejects(
        |v| v["publication_policy"]["minimum_targets"] = json!(0),
        "evidence floor must be at least 1",
    );
    rejects(
        |v| v["publication_policy"]["minimum_origin_weeks"] = json!(0),
        "evidence floor must be at least 1",
    );
}

#[test]
fn a_forecast_is_published_only_where_the_policy_admits_the_series_own_skill() {
    // "12" is published because its measured scores (90% coverage 1.0, CRPS 2.5 against 4.0) meet it.
    assert_eq!(
        parse(companion()).unwrap().series[1].status,
        ForecastStatus::Forecast
    );
    // Worse than persistence, or coverage too low: it cannot stay published.
    rejects(
        |v| v["series_backtest"]["by_series"][1]["measured"]["mean_crps"] = json!(9.0),
        "does not meet the publication policy",
    );
    rejects(
        |v| v["series_backtest"]["by_series"][1]["measured"]["coverage_90"] = json!(0.5),
        "does not meet the publication policy",
    );
    // A stricter policy than the scores support withdraws it (CRPS 2.5 is above half of 4.0).
    rejects(
        |v| v["publication_policy"]["maximum_crps_over_persistence"] = json!(0.5),
        "does not meet the publication policy",
    );
    // A series that does not claim a measured skill is never published.
    rejects(
        |v| v["series"][1]["skill"] = json!(NOT_TESTED),
        "does not meet the publication policy",
    );
}

#[test]
fn a_withheld_series_says_why_and_the_reason_must_match_its_skill() {
    // The measured series that meets the policy cannot be withheld...
    rejects(
        |v| {
            v["series"][1]["status"] = json!("withheld");
            v["series"][1]["withheld"] = json!("skill_below_policy");
        },
        "meets the publication policy but is withheld",
    );
    // ...and with scores below the policy it is withheld for being below it, not for another reason.
    let mut below = companion();
    below["series_backtest"]["by_series"][1]["measured"]["mean_crps"] = json!(9.0);
    below["series"][1]["status"] = json!("withheld");
    below["series"][1]["withheld"] = json!("skill_below_policy");
    parse(below.clone()).unwrap();
    below["series"][1]["withheld"] = json!("not_backtested");
    assert!(parse(below).unwrap_err().contains("wrong reason"));
    rejects(
        |v| v["series"][2]["withheld"] = json!("skill_below_policy"),
        "wrong reason",
    );
    rejects(
        |v| v["series"][2]["withheld"] = json!("not_backtested"),
        "wrong reason",
    );
    // The status and its reason fields agree.
    rejects(
        |v| v["series"][2]["withheld"] = Value::Null,
        "states why it is withheld",
    );
    rejects(
        |v| v["series"][1]["withheld"] = json!("not_backtested"),
        "not withheld",
    );
    rejects(
        |v| v["series"][0]["withheld"] = json!("not_backtested"),
        "not withheld",
    );
    rejects(
        |v| v["series"][2]["reason"] = json!("below_threshold"),
        "no insufficient-data reason",
    );
    rejects(
        |v| v["series"][2]["cases_in_window"] = Value::Null,
        "states the cases in its window",
    );
}

#[test]
fn the_policy_is_carried_and_validated() {
    rejects(
        |v| v["publication_policy"]["rule"] = json!(" "),
        "rule must not be empty",
    );
    rejects(
        |v| v["publication_policy"]["minimum_coverage_90"] = json!(1.5),
        "share between 0 and 1",
    );
    rejects(
        |v| v["publication_policy"]["maximum_crps_over_persistence"] = json!(-1.0),
        "finite and not negative",
    );
    rejects(
        |v| v["publication_policy"]["extra"] = json!(1),
        "unknown field",
    );
    rejects(
        |v| {
            v.as_object_mut().unwrap().remove("publication_policy");
        },
        "publication_policy",
    );
}

#[test]
fn a_refused_projection_is_a_series_with_no_forecast_and_the_rest_go_on() {
    let mut v = companion();
    v["series"][0]["reason"] = json!("projection_overflow");
    let p = parse(v).unwrap();
    assert_eq!(
        p.series[0].reason,
        Some(InsufficientReason::ProjectionOverflow)
    );
    assert_eq!(p.series[0].status, ForecastStatus::InsufficientData);
    p.check_against(&rows(&["12"])).unwrap();
}
