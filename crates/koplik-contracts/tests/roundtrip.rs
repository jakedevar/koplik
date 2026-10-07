//! Serde round trip per top-level type, plus rejection tests.

use koplik_contracts::v1::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    v: &T,
) -> serde_json::Value {
    let json = serde_json::to_value(v).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, v);
    json
}

fn prov_json() -> serde_json::Value {
    json!({
        "source_id": "cdc-measles-weekly",
        "url": "https://data.cdc.gov/example",
        "retrieved_at": "2026-09-30T12:00:00Z",
        "sha256": "ab".repeat(32),
        "licence_id": "us-gov-public-domain"
    })
}

fn prov() -> Provenances {
    serde_json::from_value(json!([prov_json()])).unwrap()
}

fn gaines() -> GeoId {
    "48165".parse().unwrap()
}

fn week(year: u16, week: u8) -> MmwrWeek {
    MmwrWeek::new(year, week).unwrap()
}

#[test]
fn fips_round_trip() {
    assert_eq!(round_trip(&"04".parse::<StateFips>().unwrap()), json!("04"));
    assert_eq!(
        round_trip(&"04013".parse::<CountyFips>().unwrap()),
        json!("04013")
    );
    assert_eq!(round_trip(&gaines()), json!("48165"));
    assert_eq!(
        round_trip(&GeoId::State("48".parse().unwrap())),
        json!("48")
    );
}

#[test]
fn geography_round_trip() {
    let g = Geography::new(gaines(), "Gaines County").unwrap();
    assert_eq!(round_trip(&g)["level"], json!("county"));
    // Level must agree with the id.
    let bad = json!({"id": "48", "level": "county", "name": "Texas"});
    assert!(serde_json::from_value::<Geography>(bad).is_err());
}

#[test]
fn mmwr_round_trip() {
    assert_eq!(
        round_trip(&week(2025, 53)),
        json!({"year": 2025, "week": 53})
    );
}

#[test]
fn provenance_round_trip() {
    let p: Provenance = serde_json::from_value(prov_json()).unwrap();
    round_trip(&p);
    let mut bad = prov_json();
    bad["sha256"] = json!("xyz");
    assert!(serde_json::from_value::<Provenance>(bad).is_err());
    let mut bad = prov_json();
    bad["retrieved_at"] = json!("yesterday");
    assert!(serde_json::from_value::<Provenance>(bad).is_err());
}

#[test]
fn weekly_case_count_round_trip_and_missing_is_not_zero() {
    let zero = WeeklyCaseCount {
        geography: gaines(),
        week: week(2025, 10),
        confirmed: CaseCount::Reported { count: 0 },
        provenance: prov(),
    };
    let missing = WeeklyCaseCount {
        confirmed: CaseCount::Missing {
            reason: MissingReason::NotReported,
        },
        ..zero.clone()
    };
    let zj = round_trip(&zero);
    let mj = round_trip(&missing);
    assert_ne!(zj["confirmed"], mj["confirmed"]);
    assert_eq!(zero.confirmed.count(), Some(0));
    assert_eq!(missing.confirmed.count(), None);
}

#[test]
fn coverage_round_trip() {
    let c = KindergartenMmrCoverage {
        geography: gaines(),
        school_year: "2023-24".parse().unwrap(),
        coverage_pct: 82.0,
        exemption_pct: Some(14.0),
        imputed: false,
        imputation_method: None,
        provenance: prov(),
    };
    assert_eq!(round_trip(&c)["school_year"], json!("2023-24"));
    let imputed = KindergartenMmrCoverage {
        imputed: true,
        imputation_method: Some("state median".into()),
        ..c.clone()
    };
    round_trip(&imputed);
    // imputed without a method, or a method without the flag, is rejected.
    let mut bad = serde_json::to_value(&imputed).unwrap();
    bad["imputation_method"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<KindergartenMmrCoverage>(bad).is_err());
    let mut bad = serde_json::to_value(&c).unwrap();
    bad["imputation_method"] = json!("x");
    assert!(serde_json::from_value::<KindergartenMmrCoverage>(bad).is_err());
    let mut bad = serde_json::to_value(&c).unwrap();
    bad["coverage_pct"] = json!(101.0);
    assert!(serde_json::from_value::<KindergartenMmrCoverage>(bad).is_err());
    assert!("2023-25".parse::<SchoolYear>().is_err());
}

#[test]
fn population_round_trip() {
    round_trip(&Population {
        geography: gaines(),
        year: 2024,
        count: 21_000,
        provenance: prov(),
    });
}

#[test]
fn rt_round_trip() {
    let ok = RtEstimate {
        geography: gaines(),
        week: week(2025, 12),
        status: RtStatus::Provisional,
        mean: Some(1.4),
        lower: Some(0.9),
        upper: Some(2.1),
        interval_level: 0.9,
        provenance: prov(),
    };
    round_trip(&ok);
    let insufficient = RtEstimate {
        status: RtStatus::InsufficientData,
        mean: None,
        lower: None,
        upper: None,
        ..ok.clone()
    };
    assert_eq!(
        round_trip(&insufficient)["status"],
        json!("insufficient_data")
    );
    // An estimate under insufficient_data, or a missing one under ok, is rejected.
    let mut bad = serde_json::to_value(&insufficient).unwrap();
    bad["mean"] = json!(1.0);
    assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
    let mut bad = serde_json::to_value(&ok).unwrap();
    bad["mean"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
    let mut bad = serde_json::to_value(&ok).unwrap();
    bad["lower"] = json!(3.0);
    assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
}

fn scenario() -> ScenarioInput {
    ScenarioInput {
        geographies: vec![gaines(), "48079".parse().unwrap()],
        start_week: week(2025, 5),
        coverage_overrides: vec![CoverageOverride {
            geography: gaines(),
            coverage_pct: 95.0,
        }],
        initial_infections: vec![InitialInfection {
            geography: gaines(),
            infectious: 3,
        }],
        parameters: SeirParameters {
            r0: 15.0,
            latent_period_days: 11.0,
            infectious_period_days: 8.0,
            mmr_effectiveness_one_dose: 0.93,
            mmr_effectiveness_two_doses: 0.97,
            time_step_days: 0.5,
            horizon_days: 180,
            gravity: Some(GravityParameters {
                scale: 1.0e-5,
                origin_exponent: 1.0,
                destination_exponent: 1.0,
                distance_exponent: 2.0,
            }),
        },
        seed: u64::MAX,
        run_count: 1000,
    }
}

#[test]
fn scenario_round_trip_keeps_full_u64_seed() {
    let s = scenario();
    let text = serde_json::to_string(&s).unwrap();
    assert!(text.contains("18446744073709551615"));
    assert_eq!(serde_json::from_str::<ScenarioInput>(&text).unwrap(), s);
    let mut bad = serde_json::to_value(&s).unwrap();
    bad["run_count"] = json!(0);
    assert!(serde_json::from_value::<ScenarioInput>(bad).is_err());
    let mut bad = serde_json::to_value(&s).unwrap();
    bad["parameters"]["r0"] = json!(-1.0);
    assert!(serde_json::from_value::<ScenarioInput>(bad).is_err());
    let mut bad = serde_json::to_value(&s).unwrap();
    bad["geographies"] = json!(["48165", "48165"]);
    assert!(serde_json::from_value::<ScenarioInput>(bad).is_err());
}

#[test]
fn forecast_round_trip() {
    let f = Forecast {
        geography: gaines(),
        origin_week: week(2025, 10),
        target_week: week(2025, 13),
        quantiles: vec![
            ForecastQuantile {
                level: 0.05,
                value: 1.0,
            },
            ForecastQuantile {
                level: 0.5,
                value: 6.0,
            },
            ForecastQuantile {
                level: 0.95,
                value: 20.0,
            },
        ],
        seed: 42,
        run_count: 1000,
        provenance: prov(),
    };
    round_trip(&f);
    let mut bad = serde_json::to_value(&f).unwrap();
    bad["quantiles"][1]["value"] = json!(0.5);
    assert!(serde_json::from_value::<Forecast>(bad).is_err());
    let mut bad = serde_json::to_value(&f).unwrap();
    bad["target_week"] = json!({"year": 2025, "week": 10});
    assert!(serde_json::from_value::<Forecast>(bad).is_err());
}

#[test]
fn unknown_fields_rejected() {
    let mut j = serde_json::to_value(Population {
        geography: gaines(),
        year: 2024,
        count: 1,
        provenance: prov(),
    })
    .unwrap();
    j["extra"] = json!(1);
    assert!(serde_json::from_value::<Population>(j).is_err());
}

#[test]
fn invalid_keys_rejected_inside_rows() {
    let mut j = serde_json::to_value(Population {
        geography: gaines(),
        year: 2024,
        count: 1,
        provenance: prov(),
    })
    .unwrap();
    j["geography"] = json!("4816");
    assert!(serde_json::from_value::<Population>(j.clone()).is_err());
    j["geography"] = json!("48165");
    j["provenance"] = json!([]);
    assert!(serde_json::from_value::<Population>(j).is_err());
}
