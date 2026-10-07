//! Backtest machinery: the vintage information cutoff, the weekly derivation from
//! cumulative versions, the runner's bookkeeping, and the manifest reader on a committed
//! synthetic fixture.

use chrono::{DateTime, NaiveDate, Utc};
use koplik_contracts::v3::{CaseCount, CaseDefinition, GeoId, MissingReason, MmwrWeek, Provenance};
use koplik_epi::backtest::manifest::{ManifestError, outbreak_total_vintages};
use koplik_epi::backtest::run::{BacktestConfig, BacktestError, run_backtest, wednesdays_between};
use koplik_epi::backtest::vintages::{ReportVintage, weekly_from_vintages};
use koplik_epi::forecast::ForecastConfig;

fn texas() -> GeoId {
    "48".parse().unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn vintage(report_date: &str, first_seen_at: &str, cumulative: u32) -> ReportVintage {
    ReportVintage {
        report_date: NaiveDate::parse_from_str(report_date, "%Y-%m-%d").unwrap(),
        first_seen_at: at(first_seen_at),
        cumulative,
        case_definition: Some(CaseDefinition::Confirmed),
        provenance: Provenance {
            source_id: "synthetic-vintage-test".into(),
            url: format!("https://example.test/{report_date}"),
            retrieved_at: at("2026-09-30T12:00:00Z"),
            sha256: koplik_contracts::v3::Sha256Hex::new("ab".repeat(32)).unwrap(),
            licence_id: "synthetic".into(),
        },
    }
}

/// Tuesday/Friday versions over MMWR weeks 10..=14 of 2025 (2025-03-02 .. 2025-04-05).
fn synthetic() -> Vec<ReportVintage> {
    vec![
        vintage("2025-03-04", "2025-03-05T18:00:00Z", 100),
        vintage("2025-03-07", "2025-03-08T02:00:00Z", 130),
        vintage("2025-03-11", "2025-03-12T19:00:00Z", 150),
        vintage("2025-03-14", "2025-03-15T04:00:00Z", 170),
        vintage("2025-03-18", "2025-03-19T01:00:00Z", 180),
        // Seen only on the Monday after: not known at a Saturday or Sunday cutoff.
        vintage("2025-03-21", "2025-03-24T07:00:00Z", 200),
        vintage("2025-03-25", "2025-03-26T08:00:00Z", 210),
        // A fall within the week: the week's count comes from its last version.
        vintage("2025-03-28", "2025-03-29T03:00:00Z", 205),
        // A fall across weeks: cases reclassified, the week is ambiguous.
        vintage("2025-04-01", "2025-04-02T07:00:00Z", 200),
        vintage("2025-04-04", "2025-04-05T03:00:00Z", 202),
        vintage("2025-04-08", "2025-04-09T07:00:00Z", 230),
    ]
}

/// The synthetic versions with the 03-21 total unlabelled (as the DSHS pages that say
/// "cases have been identified" rather than "confirmed").
fn with_unlabelled() -> Vec<ReportVintage> {
    let mut v = synthetic();
    v[5].case_definition = None;
    v
}

fn week(n: u8) -> MmwrWeek {
    MmwrWeek::new(2025, n).unwrap()
}

#[test]
fn weekly_counts_come_from_each_weeks_last_version() {
    let s = weekly_from_vintages(texas(), &synthetic(), None).unwrap();
    assert_eq!(s.known_versions, 11);
    assert_eq!(s.last_report_week, Some(week(15)));
    assert_eq!(s.last_complete_week, Some(week(14)));
    let by_week: Vec<(MmwrWeek, CaseCount)> = s.rows.iter().map(|r| (r.week, r.cases)).collect();
    assert_eq!(
        by_week,
        vec![
            // First week: its cumulative covers everything before it.
            (
                week(10),
                CaseCount::Missing {
                    reason: MissingReason::NotReported
                }
            ),
            (week(11), CaseCount::Reported { count: 40 }),
            (week(12), CaseCount::Reported { count: 30 }),
            // Last version of the week (205) minus last of the previous week (200).
            (week(13), CaseCount::Reported { count: 5 }),
            // 202 < 205: a fall is ambiguous, never a negative or a zero.
            (
                week(14),
                CaseCount::Missing {
                    reason: MissingReason::Ambiguous
                }
            ),
        ]
    );
    // Provenance names both versions the difference was taken from.
    let urls: Vec<&str> = s.rows[1]
        .provenance
        .as_slice()
        .iter()
        .map(|p| p.url.as_str())
        .collect();
    assert_eq!(
        urls,
        vec![
            "https://example.test/2025-03-07",
            "https://example.test/2025-03-14"
        ]
    );
    assert!(
        s.rows
            .iter()
            .all(|r| r.case_definition == CaseDefinition::Confirmed)
    );
}

#[test]
fn an_unlabelled_version_still_bounds_its_week_but_yields_no_confirmed_count() {
    let s = weekly_from_vintages(texas(), &with_unlabelled(), None).unwrap();
    let by_week: Vec<(MmwrWeek, CaseCount)> = s.rows.iter().map(|r| (r.week, r.cases)).collect();
    // Week 12's last version (03-21) is unlabelled: week 12 (it ends there) and week 13
    // (it starts there) are ambiguous; the 03-18 version is not promoted to "last of week".
    assert_eq!(by_week[2].0, week(12));
    assert_eq!(by_week[3].0, week(13));
    for (_, cases) in &by_week[2..4] {
        assert_eq!(
            *cases,
            CaseCount::Missing {
                reason: MissingReason::Ambiguous
            }
        );
    }
    assert_eq!(by_week[1], (week(11), CaseCount::Reported { count: 40 }));
    assert!(
        s.rows
            .iter()
            .all(|r| r.case_definition == CaseDefinition::Confirmed)
    );
    // A series with no labelled version has no confirmed counts: no rows, but what was
    // known is still reported.
    let mut none = synthetic();
    for v in &mut none {
        v.case_definition = None;
    }
    let s = weekly_from_vintages(texas(), &none, None).unwrap();
    assert!(s.rows.is_empty());
    assert_eq!(
        (s.known_versions, s.last_complete_week),
        (11, Some(week(14)))
    );
}

#[test]
fn a_vintage_first_seen_after_the_cutoff_cannot_change_the_known_series() {
    let cutoff = at("2025-03-27T00:00:00Z");
    let base = weekly_from_vintages(texas(), &synthetic(), Some(cutoff)).unwrap();
    let mut later = synthetic();
    let mut other = vintage("2025-03-20", "2025-04-16T00:00:00Z", 999);
    other.case_definition = Some(CaseDefinition::ConfirmedOrUnknownStatus);
    later.push(other);
    // Dated before the cutoff but seen after it, with another definition: invisible.
    assert_eq!(
        weekly_from_vintages(texas(), &later, Some(cutoff)).unwrap(),
        base
    );
    // Once it is known, the mixture is an error.
    assert!(weekly_from_vintages(texas(), &later, None).is_err());
}

#[test]
fn scoring_levels_are_validated_up_front() {
    let vintages = long_synthetic();
    let cfg = BacktestConfig {
        forecast: ForecastConfig {
            run_count: 10,
            levels: vec![0.5],
            ..ForecastConfig::default()
        },
        seed: 3,
        geography: texas(),
        forecast_dates: wednesdays_between(
            vintages.first().unwrap().first_seen_at,
            vintages.last().unwrap().first_seen_at,
        ),
    };
    let err = run_backtest(&vintages, &cfg).unwrap_err();
    assert!(matches!(err, BacktestError::Config(_)), "{err}");
}

#[test]
fn versions_first_seen_after_the_cutoff_are_unknown() {
    // Sunday 2025-03-23: the 03-21 version (seen Monday 03-24) is not yet known, so week
    // 12 is not complete and the latest complete week is 11.
    let s = weekly_from_vintages(texas(), &synthetic(), Some(at("2025-03-23T00:00:00Z"))).unwrap();
    assert_eq!(s.known_versions, 5);
    assert_eq!(s.last_report_week, Some(week(12)));
    assert_eq!(s.last_complete_week, Some(week(11)));
    assert_eq!(s.rows.last().unwrap().week, week(11));
    // Two days later the Friday version is known, but week 12 is complete only once a
    // version dated in a later week is known (a week still being reported never gets a
    // partial count).
    let s = weekly_from_vintages(texas(), &synthetic(), Some(at("2025-03-25T00:00:00Z"))).unwrap();
    assert_eq!(s.known_versions, 6);
    assert_eq!(s.last_report_week, Some(week(12)));
    assert_eq!(s.last_complete_week, Some(week(11)));
    let s = weekly_from_vintages(texas(), &synthetic(), Some(at("2025-03-27T00:00:00Z"))).unwrap();
    assert_eq!(s.known_versions, 7);
    assert_eq!(s.last_complete_week, Some(week(12)));
    assert_eq!(
        s.rows.last().unwrap().cases,
        CaseCount::Reported { count: 30 }
    );
    // Before anything was seen: nothing.
    let s = weekly_from_vintages(texas(), &synthetic(), Some(at("2025-03-01T00:00:00Z"))).unwrap();
    assert_eq!(s.known_versions, 0);
    assert_eq!(s.last_complete_week, None);
    assert!(s.rows.is_empty());
}

#[test]
fn forecast_dates_are_wednesdays_in_range() {
    let dates = wednesdays_between(at("2025-03-05T18:00:00Z"), at("2025-04-02T07:00:00Z"));
    let days: Vec<String> = dates.iter().map(|d| d.to_rfc3339()).collect();
    assert_eq!(
        days,
        vec![
            "2025-03-12T00:00:00+00:00",
            "2025-03-19T00:00:00+00:00",
            "2025-03-26T00:00:00+00:00",
            "2025-04-02T00:00:00+00:00",
        ]
    );
    assert!(wednesdays_between(at("2025-03-06T00:00:00Z"), at("2025-03-07T00:00:00Z")).is_empty());
}

/// A long synthetic outbreak (one version per week) so the runner can make forecasts; the
/// test checks the bookkeeping, not the score.
fn long_synthetic() -> Vec<ReportVintage> {
    let weekly = [
        20, 25, 30, 38, 45, 50, 56, 60, 62, 61, 58, 52, 45, 40, 33, 28, 22, 18, 15, 12,
    ];
    let mut cumulative = 0;
    let mut out = Vec::new();
    let mut day = NaiveDate::from_ymd_opt(2025, 3, 4).unwrap(); // a Tuesday, MMWR week 10
    for c in weekly {
        cumulative += c;
        let seen = day
            .succ_opt()
            .unwrap()
            .and_hms_opt(18, 0, 0)
            .unwrap()
            .and_utc();
        out.push(vintage(&day.to_string(), &seen.to_rfc3339(), cumulative));
        day += chrono::Duration::days(7);
    }
    out
}

#[test]
fn runner_scores_only_targets_with_truth_and_reports_as_measured() {
    let vintages = long_synthetic();
    let dates = wednesdays_between(
        vintages.first().unwrap().first_seen_at,
        vintages.last().unwrap().first_seen_at,
    );
    let cfg = BacktestConfig {
        forecast: ForecastConfig {
            run_count: 100,
            ..ForecastConfig::default()
        },
        seed: 3,
        geography: texas(),
        forecast_dates: dates.clone(),
    };
    let report = run_backtest(&vintages, &cfg).unwrap();
    assert_eq!(report.origins.len(), dates.len());
    assert_eq!(report.versions, 20);
    assert_eq!(report.window_weeks, 3);
    assert_eq!(report.max_lag_weeks, 3);
    assert_eq!(report.min_cases, 11);
    // Truth: weeks 11..=28 reported (week 10's cumulative cannot be split), week 29 is the
    // last version's own week and is not complete.
    assert_eq!(report.truth.len(), 19);
    assert_eq!(report.truth[0], (week(10), None));
    assert_eq!(report.truth[1], (week(11), Some(25)));
    assert_eq!(report.truth.last().unwrap(), &(week(28), Some(15)));

    let ok: Vec<_> = report.origins.iter().filter(|o| o.status == "ok").collect();
    assert!(ok.len() >= 8, "{} forecasts made", ok.len());
    let first_ok = ok[0];
    // Early origins lack look-back; before that the first date has an origin but no row.
    assert!(report.origins[0].status != "ok");
    // Scores: a target with truth has every score, one without has none.
    for o in &ok {
        assert_eq!(o.scores.len(), 8);
        for s in &o.scores {
            assert_eq!(s.observed.is_some(), s.crps.is_some());
            assert_eq!(s.observed.is_some(), s.in_50.is_some());
            if let (Some(crps), Some(err), Some(y)) = (s.crps, s.persistence_abs_error, s.observed)
            {
                assert!(crps >= 0.0);
                assert_eq!(
                    err,
                    (f64::from(y) - f64::from(o.origin_count.unwrap())).abs()
                );
            }
            assert!(s.lower_90 <= s.lower_50 && s.lower_50 <= s.median);
            assert!(s.median <= s.upper_50 && s.upper_50 <= s.upper_90);
        }
    }
    // A horizon-1 target of the first forecast is the week after its origin.
    assert_eq!(
        first_ok.scores[0].target_week,
        first_ok.origin_week.unwrap().next().unwrap()
    );
    // Summaries count exactly the scored targets.
    let scored: usize = ok
        .iter()
        .flat_map(|o| o.scores.iter())
        .filter(|s| s.crps.is_some())
        .count();
    assert_eq!(report.pooled.n as usize, scored);
    assert_eq!(report.by_horizon.len(), 8);
    assert_eq!(
        report.by_horizon.iter().map(|s| s.n).sum::<u32>(),
        report.pooled.n
    );
    for s in report
        .by_horizon
        .iter()
        .chain(std::iter::once(&report.pooled))
    {
        if s.n > 0 {
            assert!((0.0..=1.0).contains(&s.coverage_50.unwrap()));
            assert!((0.0..=1.0).contains(&s.coverage_90.unwrap()));
            assert!(s.mean_crps.unwrap() >= 0.0);
        } else {
            assert!(s.mean_crps.is_none());
        }
    }
    // Deterministic: the same run twice is identical.
    assert_eq!(run_backtest(&vintages, &cfg).unwrap(), report);
}

#[test]
fn manifest_fixture_parses_into_confirmed_outbreak_totals() {
    let raw = include_str!("../../../data/fixtures/forecast/vintage-manifest-synthetic.json");
    let meta: serde_json::Value = serde_json::from_str(raw).unwrap();
    assert_eq!(meta["synthetic"], true);
    let vintages = outbreak_total_vintages(raw).unwrap();
    assert_eq!(vintages.len(), 4);
    // Sorted by first-seen time, which is not report-date order here.
    let dates: Vec<String> = vintages.iter().map(|v| v.report_date.to_string()).collect();
    assert_eq!(
        dates,
        vec!["2025-03-04", "2025-03-11", "2025-03-07", "2025-03-14"]
    );
    assert_eq!(vintages[0].cumulative, 159);
    assert_eq!(
        vintages[0].provenance.source_id,
        "synthetic-vintage-fixture"
    );
    assert_eq!(vintages[0].provenance.sha256.as_str(), &"cd".repeat(32));
    // Three versions are labelled confirmed; the one without a basis is unlabelled.
    let labelled: Vec<Option<CaseDefinition>> =
        vintages.iter().map(|v| v.case_definition).collect();
    assert_eq!(
        labelled,
        vec![
            Some(CaseDefinition::Confirmed),
            None,
            Some(CaseDefinition::Confirmed),
            Some(CaseDefinition::Confirmed)
        ]
    );

    let mut doc: serde_json::Value = serde_json::from_str(raw).unwrap();
    doc["manifest_version"] = serde_json::json!(2);
    assert!(matches!(
        outbreak_total_vintages(&doc.to_string()).unwrap_err(),
        ManifestError::Version(2)
    ));
}
