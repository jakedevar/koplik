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
    let g = Geography::new(gaines(), "Gaines County", prov()).unwrap();
    let j = round_trip(&g);
    assert_eq!(j["level"], json!("county"));
    assert_eq!(j["centroid"], serde_json::Value::Null);
    let g = g.with_centroid(Centroid::new(32.74, -102.64).unwrap());
    assert_eq!(round_trip(&g)["centroid"]["latitude"], json!(32.74));
    // Level must agree with the id.
    let bad = json!({"id": "48", "level": "county", "name": "Texas", "centroid": null,
        "provenance": [prov_json()]});
    assert!(serde_json::from_value::<Geography>(bad).is_err());
    // Centroid out of range is rejected.
    for (lat, lon) in [(91.0, 0.0), (-90.5, 0.0), (0.0, 180.5), (0.0, -181.0)] {
        let bad = json!({"id": "48165", "level": "county", "name": "G",
            "centroid": {"latitude": lat, "longitude": lon}, "provenance": [prov_json()]});
        assert!(
            serde_json::from_value::<Geography>(bad).is_err(),
            "{lat} {lon}"
        );
    }
    assert!(Centroid::new(f64::NAN, 0.0).is_err());
    // Empty provenance is rejected.
    let bad = json!({"id": "48165", "level": "county", "name": "G", "centroid": null,
        "provenance": []});
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

fn coverage_row() -> KindergartenMmrCoverage {
    KindergartenMmrCoverage {
        geography: gaines(),
        school_year: "2023-24".parse().unwrap(),
        coverage: CoverageValue::Reported {
            coverage_pct: 82.0,
            exemption_pct: Some(14.0),
        },
        imputed: false,
        imputation_method: None,
        provenance: prov(),
    }
}

#[test]
fn coverage_round_trip() {
    let c = coverage_row();
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
    bad["coverage"]["coverage_pct"] = json!(101.0);
    assert!(serde_json::from_value::<KindergartenMmrCoverage>(bad).is_err());
    assert!("2023-25".parse::<SchoolYear>().is_err());
}

#[test]
fn coverage_missing_is_not_zero() {
    let zero = KindergartenMmrCoverage {
        coverage: CoverageValue::Reported {
            coverage_pct: 0.0,
            exemption_pct: None,
        },
        ..coverage_row()
    };
    let missing = KindergartenMmrCoverage {
        coverage: CoverageValue::Missing {
            reason: MissingReason::NotReported,
        },
        ..coverage_row()
    };
    let zj = round_trip(&zero);
    let mj = round_trip(&missing);
    assert_ne!(zj["coverage"], mj["coverage"]);
    assert_eq!(zero.coverage.coverage_pct(), Some(0.0));
    assert_eq!(missing.coverage.coverage_pct(), None);
    // A missing value cannot be flagged imputed.
    let mut bad = serde_json::to_value(&missing).unwrap();
    bad["imputed"] = json!(true);
    bad["imputation_method"] = json!("guess");
    assert!(serde_json::from_value::<KindergartenMmrCoverage>(bad).is_err());
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
        status: RtStatus::Ok,
        provisional: true,
        mean: Some(1.4),
        lower: Some(0.9),
        upper: Some(2.1),
        interval_level: 0.9,
        provenance: prov(),
    };
    assert_eq!(round_trip(&ok)["provisional"], json!(true));
    round_trip(&RtEstimate {
        provisional: false,
        ..ok.clone()
    });
    // Insufficient data and provisional can hold together; the estimate is null either way.
    for provisional in [false, true] {
        let insufficient = RtEstimate {
            status: RtStatus::InsufficientData,
            provisional,
            mean: None,
            lower: None,
            upper: None,
            ..ok.clone()
        };
        let j = round_trip(&insufficient);
        assert_eq!(j["status"], json!("insufficient_data"));
        assert_eq!(j["provisional"], json!(provisional));
        // An estimate under insufficient_data is rejected.
        let mut bad = j.clone();
        bad["mean"] = json!(1.0);
        assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
    }
    // A missing estimate under ok is rejected; "provisional" is no longer a status.
    let mut bad = serde_json::to_value(&ok).unwrap();
    bad["mean"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
    let mut bad = serde_json::to_value(&ok).unwrap();
    bad["status"] = json!("provisional");
    assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
    let mut bad = serde_json::to_value(&ok).unwrap();
    bad["lower"] = json!(3.0);
    assert!(serde_json::from_value::<RtEstimate>(bad).is_err());
}

fn node(code: &str, baseline: BaselineCoverage) -> ScenarioNode {
    ScenarioNode {
        id: code.parse().unwrap(),
        population: 21_000,
        baseline_coverage: baseline,
        centroid: Centroid::new(32.74, -102.64).unwrap(),
        initial_exposed: 2,
        initial_infectious: 3,
        provenance: prov(),
    }
}

fn reported(pct: f64) -> BaselineCoverage {
    BaselineCoverage::Reported {
        coverage_pct: pct,
        imputed: false,
        imputation_method: None,
        provenance: prov(),
    }
}

fn scenario() -> ScenarioInput {
    ScenarioInput {
        nodes: vec![node("48079", reported(90.0)), node("48165", reported(82.0))],
        start_week: week(2025, 5),
        coverage_overrides: vec![CoverageOverride {
            geography: gaines(),
            coverage_pct: 95.0,
        }],
        parameters: SeirParameters {
            r0: R0::UniformPrior {
                min: 12.0,
                max: 18.0,
            },
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

fn rejects(mutate: impl FnOnce(&mut serde_json::Value)) {
    let mut j = serde_json::to_value(scenario()).unwrap();
    mutate(&mut j);
    assert!(serde_json::from_value::<ScenarioInput>(j).is_err());
}

#[test]
fn scenario_round_trip_keeps_full_u64_seed() {
    let s = scenario();
    let text = serde_json::to_string(&s).unwrap();
    assert!(text.contains("18446744073709551615"));
    assert_eq!(serde_json::from_str::<ScenarioInput>(&text).unwrap(), s);
    // A fixed R0 and a missing-but-overridden baseline also round trip.
    let mut f = scenario();
    f.parameters.r0 = R0::Fixed { value: 15.0 };
    f.nodes[1].baseline_coverage = BaselineCoverage::Missing {
        reason: MissingReason::NotReported,
    };
    round_trip(&f);
    // An imputed baseline round trips with its method.
    let mut imp = scenario();
    imp.nodes[0].baseline_coverage = BaselineCoverage::Reported {
        coverage_pct: 88.0,
        imputed: true,
        imputation_method: Some("state median".into()),
        provenance: prov(),
    };
    round_trip(&imp);
}

#[test]
fn scenario_rejections() {
    rejects(|j| j["run_count"] = json!(0));
    rejects(|j| j["nodes"] = json!([]));
    rejects(|j| {
        let n = j["nodes"][0].clone();
        j["nodes"][1] = n; // duplicate id
    });
    rejects(|j| j["nodes"].as_array_mut().unwrap().reverse()); // not in FIPS order
    rejects(|j| j["nodes"][0]["population"] = json!(0));
    rejects(|j| j["nodes"][0]["initial_exposed"] = json!(21_000)); // E + I > population
    rejects(|j| j["nodes"][0]["centroid"]["latitude"] = json!(90.1));
    rejects(|j| j["nodes"][0]["centroid"]["longitude"] = json!(-180.1));
    rejects(|j| j["parameters"]["r0"] = json!({"kind": "fixed", "value": -1.0}));
    rejects(|j| j["parameters"]["r0"] = json!({"kind": "uniform_prior", "min": 18.0, "max": 12.0}));
    rejects(|j| j["parameters"]["r0"] = json!({"kind": "uniform_prior", "min": 0.0, "max": 12.0}));
    rejects(|j| j["parameters"]["r0"] = json!(15.0)); // bare number is no longer a shape
    rejects(|j| j["coverage_overrides"][0]["geography"] = json!("48001")); // unlisted node
    rejects(|j| {
        let o = j["coverage_overrides"][0].clone();
        j["coverage_overrides"].as_array_mut().unwrap().push(o); // duplicate override
    });
    // Missing baseline without an override is rejected (never guessed).
    rejects(|j| {
        j["nodes"][0]["baseline_coverage"] = json!({"status": "missing", "reason": "not_reported"})
    });
    rejects(|j| j["nodes"][0]["baseline_coverage"]["coverage_pct"] = json!(120.0));
    // Node provenance must not be empty.
    rejects(|j| j["nodes"][0]["provenance"] = json!([]));
    // Imputed baseline needs a method, and a method needs the flag.
    rejects(|j| j["nodes"][0]["baseline_coverage"]["imputed"] = json!(true));
    rejects(|j| j["nodes"][0]["baseline_coverage"]["imputation_method"] = json!("guess"));
    rejects(|j| {
        j["nodes"][0]["baseline_coverage"]["imputed"] = json!(true);
        j["nodes"][0]["baseline_coverage"]["imputation_method"] = json!("  ");
    });
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

#[test]
fn schema_carries_simple_runtime_bounds() {
    use schemars::schema_for;
    let v = serde_json::to_value(schema_for!(ScenarioInput)).unwrap();
    let defs = &v["$defs"];
    assert_eq!(v["properties"]["run_count"]["minimum"], json!(1));
    assert_eq!(v["properties"]["nodes"]["minItems"], json!(1));
    assert_eq!(
        defs["ScenarioNode"]["properties"]["population"]["minimum"],
        json!(1)
    );
    let p = &defs["SeirParameters"]["properties"];
    assert_eq!(p["horizon_days"]["minimum"], json!(1));
    assert_eq!(p["time_step_days"]["exclusiveMinimum"], json!(0));
    for k in ["mmr_effectiveness_one_dose", "mmr_effectiveness_two_doses"] {
        assert_eq!((&p[k]["minimum"], &p[k]["maximum"]), (&json!(0), &json!(1)));
    }
    let c = &defs["Centroid"]["properties"];
    assert_eq!(c["latitude"]["minimum"], json!(-90));
    assert_eq!(c["longitude"]["maximum"], json!(180));
    assert_eq!(
        defs["CoverageOverride"]["properties"]["coverage_pct"]["maximum"],
        json!(100)
    );
}
