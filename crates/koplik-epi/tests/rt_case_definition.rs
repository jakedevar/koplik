//! R_t reads contracts v3 case rows of any case definition and inherits the input's definition.

use koplik_contracts::v1;
use koplik_contracts::v3::{
    CaseCount, CaseDefinition, GeoId, MmwrWeek, Provenances, WeeklyCaseCount,
};
use koplik_epi::rt::{RtConfig, RtError, case_definitions, estimate_weekly};
use serde_json::json;

fn prov() -> Provenances {
    serde_json::from_value(json!([{
        "source_id": "cdc-measles-weekly",
        "url": "https://data.cdc.gov/example/a",
        "retrieved_at": "2026-09-30T12:00:00Z",
        "sha256": "ab".repeat(32),
        "licence_id": "us-gov-public-domain"
    }]))
    .unwrap()
}

fn row(geo: &str, w: u8, count: u32, definition: CaseDefinition) -> WeeklyCaseCount {
    WeeklyCaseCount {
        geography: geo.parse::<GeoId>().unwrap(),
        week: MmwrWeek::new(2025, w).unwrap(),
        cases: CaseCount::Reported { count },
        case_definition: definition,
        provenance: prov(),
    }
}

const COUNTS: [u32; 12] = [3, 5, 8, 13, 20, 31, 40, 52, 60, 66, 70, 71];

fn series(geo: &str, definition: CaseDefinition) -> Vec<WeeklyCaseCount> {
    COUNTS
        .iter()
        .enumerate()
        .map(|(i, &c)| row(geo, i as u8 + 1, c, definition))
        .collect()
}

#[test]
fn estimates_are_the_same_numbers_for_every_case_definition() {
    // The estimator works on the counts as given; the definition only says what they count.
    let cfg = RtConfig::default();
    let confirmed = estimate_weekly(&series("48", CaseDefinition::Confirmed), &cfg).unwrap();
    let nndss = estimate_weekly(
        &series("48", CaseDefinition::ConfirmedOrUnknownStatus),
        &cfg,
    )
    .unwrap();
    assert!(!confirmed.is_empty());
    assert_eq!(confirmed, nndss);
}

#[test]
fn a_v1_row_is_read_as_confirmed_and_matches_its_lossless_v3_upgrade() {
    let cfg = RtConfig::default();
    let v3 = series("48", CaseDefinition::Confirmed);
    let v1_rows: Vec<v1::WeeklyCaseCount> = v3
        .iter()
        .map(|r| v1::WeeklyCaseCount {
            geography: r.geography,
            week: r.week,
            confirmed: r.cases.clone(),
            provenance: r.provenance.clone(),
        })
        .collect();
    assert_eq!(
        estimate_weekly(&v1_rows, &cfg).unwrap(),
        estimate_weekly(&v3, &cfg).unwrap()
    );
    let upgraded: Vec<WeeklyCaseCount> = v1_rows.iter().cloned().map(Into::into).collect();
    assert_eq!(upgraded, v3);
    assert_eq!(
        case_definitions(&v1_rows).unwrap()[&"48".parse::<GeoId>().unwrap()],
        CaseDefinition::Confirmed
    );
}

#[test]
fn definitions_may_differ_between_geographies_and_are_reported_per_geography() {
    let mut rows = series("48", CaseDefinition::ConfirmedOrUnknownStatus);
    rows.extend(series("48165", CaseDefinition::Confirmed));
    let definitions = case_definitions(&rows).unwrap();
    assert_eq!(definitions.len(), 2);
    assert_eq!(
        definitions[&"48".parse::<GeoId>().unwrap()],
        CaseDefinition::ConfirmedOrUnknownStatus
    );
    assert_eq!(
        definitions[&"48165".parse::<GeoId>().unwrap()],
        CaseDefinition::Confirmed
    );
    let out = estimate_weekly(&rows, &RtConfig::default()).unwrap();
    assert!(out.iter().any(|e| e.geography.to_string() == "48"));
    assert!(out.iter().any(|e| e.geography.to_string() == "48165"));
}

#[test]
fn a_series_that_mixes_definitions_is_rejected_not_blended() {
    let mut rows = series("48", CaseDefinition::ConfirmedOrUnknownStatus);
    rows[6].case_definition = CaseDefinition::Confirmed;
    let err = estimate_weekly(&rows, &RtConfig::default()).unwrap_err();
    assert!(
        matches!(err, RtError::MixedCaseDefinition { geography } if geography.to_string() == "48")
    );
    assert!(matches!(
        case_definitions(&rows),
        Err(RtError::MixedCaseDefinition { .. })
    ));
}
