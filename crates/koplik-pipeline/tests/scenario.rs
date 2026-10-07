//! The what-if scenario builder (#1455) against the committed real-byte fixtures: the
//! pre-registered hypothetical-introduction rule, its configurable parts, and what it refuses.
//! Offline.

use std::path::PathBuf;

use koplik_contracts::v1::{
    BaselineCoverage, CountyFips, CoverageValue, GeoId, GravityParameters, KindergartenMmrCoverage,
    MmwrWeek, Population, ScenarioInput,
};
use koplik_contracts::v4::ScenarioProvenance;
use koplik_ingest::store::SnapshotStore;
use koplik_ingest::{census_population, coverage};
use koplik_pipeline::scenario::{self, Neighbourhood, ScenarioConfig, ScenarioError, Sources};
use koplik_pipeline::{Config, Mode, Stage, run_stage};

struct Fixture {
    _dir: tempfile::TempDir,
    populations: Vec<Population>,
    gazetteer: Vec<koplik_contracts::v1::Geography>,
    coverage: Vec<KindergartenMmrCoverage>,
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    let config = Config {
        mode: Mode::Fixtures,
        store: Config::default_store(Mode::Fixtures, &work),
        work,
        out: dir.path().join("out"),
        fixtures: repo().join("data/fixtures"),
    };
    run_stage(Stage::Ingest, &config).unwrap();
    let store = SnapshotStore::open(&config.store).unwrap();
    let mut rows = coverage::parse_latest_texas(&store, 2023).unwrap();
    rows.extend(coverage::parse_latest_texas(&store, 2024).unwrap());
    Fixture {
        populations: census_population::parse_latest_populations(&store, false)
            .unwrap()
            .1,
        gazetteer: census_population::parse_latest_geographies(&store, false)
            .unwrap()
            .1,
        coverage: rows,
        _dir: dir,
    }
}

fn sources(f: &Fixture) -> Sources<'_> {
    Sources {
        populations: &f.populations,
        gazetteer: &f.gazetteer,
        coverage: &f.coverage,
    }
}

/// A short ensemble keeps debug-build engine runs quick; the method is unchanged.
fn quick() -> ScenarioConfig {
    ScenarioConfig {
        run_count: 8,
        ..ScenarioConfig::default()
    }
}

fn gaines() -> GeoId {
    GeoId::County(CountyFips::new(48165).unwrap())
}

#[test]
fn default_rule_introduces_one_infectious_person_into_gaines() {
    let f = load();
    let built = scenario::build(&ScenarioConfig::default(), &sources(&f)).unwrap();
    let input = &built.input;
    // Gaines alone: the cited defaults carry no gravity, so no neighbour would be coupled.
    assert_eq!(input.nodes.len(), 1);
    let node = &input.nodes[0];
    assert_eq!(node.id, gaines());
    assert_eq!(node.population, 23956);
    // A stated assumption, not data: one infectious person, nobody exposed.
    assert_eq!((node.initial_infectious, node.initial_exposed), (1, 0));
    // The neutral reference week: the MMWR week containing July 1, 2025 (the population
    // estimate's reference date); it is not an outbreak date.
    assert_eq!(input.start_week.to_string(), "2025-W27");
    assert_eq!(input.seed, 20_250_304);
    assert_eq!(input.run_count, 1000);
    assert_eq!(input.parameters, koplik_epi::default_parameters());
    assert!(input.parameters.gravity.is_none());
    assert!(input.coverage_overrides.is_empty());
    match node.baseline_coverage {
        BaselineCoverage::Reported {
            coverage_pct,
            imputed,
            ..
        } => {
            assert!(!imputed);
            assert!((coverage_pct - 81.9672131147541).abs() < 1e-9);
        }
        BaselineCoverage::Missing { .. } => panic!("Gaines coverage is reported"),
    }
    let p = &built.provenance;
    assert_eq!(p.seeding.geography, gaines());
    assert_eq!(
        (p.seeding.initial_infectious, p.seeding.initial_exposed),
        (1, 0)
    );
    assert_eq!(
        p.statement,
        "Hypothetical: what could happen if one infectious person arrived in Gaines County, given its population and kindergarten MMR coverage. This is not a reconstruction or forecast of the 2025 outbreak."
    );
    assert!(p.seeding.assumption.contains("not data"));
    assert!(p.seeding.start_week_basis.contains("changes no result"));
    assert_eq!(p.seed, "20250304");
    // Every parameter carries its citation and the value the scenario ran with.
    let values = serde_json::to_value(input.parameters).unwrap();
    assert_eq!(p.parameters.len(), values.as_object().unwrap().len());
    for c in &p.parameters {
        assert!(
            !c.source.is_empty() && !c.note.is_empty(),
            "{}",
            c.parameter
        );
        assert_eq!(c.value, values[c.parameter.as_str()], "{}", c.parameter);
    }
    p.check_against(input).unwrap();
    // The engine runs it.
    let ensemble = koplik_epi::simulate_ensemble(&ScenarioInput {
        run_count: 4,
        ..input.clone()
    })
    .unwrap();
    assert_eq!(ensemble.members.len(), 4);
}

#[test]
fn building_twice_gives_identical_bytes() {
    let f = load();
    let a = scenario::build(&ScenarioConfig::default(), &sources(&f)).unwrap();
    let b = scenario::build(&ScenarioConfig::default(), &sources(&f)).unwrap();
    assert_eq!(
        serde_json::to_vec(&a.input).unwrap(),
        serde_json::to_vec(&b.input).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&a.provenance).unwrap(),
        serde_json::to_vec(&b.provenance).unwrap()
    );
}

#[test]
fn the_introduced_people_and_the_reference_week_are_configurable_and_recorded() {
    let f = load();
    let config = ScenarioConfig {
        initial_infectious: 3,
        initial_exposed: 2,
        start_week: Some(MmwrWeek::new(2025, 10).unwrap()),
        ..quick()
    };
    let built = scenario::build(&config, &sources(&f)).unwrap();
    let node = &built.input.nodes[0];
    assert_eq!((node.initial_infectious, node.initial_exposed), (3, 2));
    assert_eq!(built.input.start_week.to_string(), "2025-W10");
    let p = &built.provenance;
    assert_eq!(
        (p.seeding.initial_infectious, p.seeding.initial_exposed),
        (3, 2)
    );
    assert_eq!(p.seeding.start_week.to_string(), "2025-W10");
    assert!(
        p.statement
            .contains("3 infectious people and 2 exposed people")
    );
    assert!(p.seeding.start_week_basis.contains("configuration"));
    // A scenario that introduces nobody has nothing to simulate.
    let nobody = ScenarioConfig {
        initial_infectious: 0,
        initial_exposed: 0,
        ..quick()
    };
    assert!(matches!(
        scenario::build(&nobody, &sources(&f)),
        Err(ScenarioError::Invalid(_))
    ));
    // More people than the coverage-derived susceptible pool is refused by the engine.
    let too_many = ScenarioConfig {
        initial_infectious: 23_000,
        ..quick()
    };
    assert!(matches!(
        scenario::build(&too_many, &sources(&f)),
        Err(ScenarioError::Invalid(m)) if m.contains("engine")
    ));
}

#[test]
fn a_missing_input_is_missing_never_guessed() {
    let f = load();
    // No population for the seeded county.
    let populations: Vec<Population> = f
        .populations
        .iter()
        .filter(|p| p.geography != gaines())
        .cloned()
        .collect();
    let s = Sources {
        populations: &populations,
        ..sources(&f)
    };
    assert!(matches!(
        scenario::build(&quick(), &s),
        Err(ScenarioError::Missing(m)) if m.contains("population")
    ));
    // No baseline coverage row for the seeded county: it is not replaced by another year.
    let rows: Vec<KindergartenMmrCoverage> = f
        .coverage
        .iter()
        .filter(|r| !(r.geography == gaines() && r.school_year.start_year() == 2023))
        .cloned()
        .collect();
    let s = Sources {
        coverage: &rows,
        ..sources(&f)
    };
    assert!(matches!(
        scenario::build(&quick(), &s),
        Err(ScenarioError::Missing(m)) if m.contains("coverage")
    ));
    // No Gazetteer internal point, so no centroid.
    let gazetteer: Vec<_> = f
        .gazetteer
        .iter()
        .map(|g| {
            let mut g = g.clone();
            if g.id == gaines() {
                g.centroid = None;
            }
            g
        })
        .collect();
    let s = Sources {
        gazetteer: &gazetteer,
        ..sources(&f)
    };
    assert!(matches!(
        scenario::build(&quick(), &s),
        Err(ScenarioError::Missing(m)) if m.contains("internal point")
    ));
}

#[test]
fn a_neighbourhood_couples_nearby_counties_and_excludes_those_with_a_missing_input() {
    let f = load();
    let focus = f
        .gazetteer
        .iter()
        .find(|g| g.id == gaines())
        .unwrap()
        .centroid
        .unwrap();
    // A county within the radius that has no baseline coverage must be excluded, not imputed.
    let baseline = |id: GeoId| {
        f.coverage
            .iter()
            .find(|r| r.geography == id && r.school_year.start_year() == 2023)
    };
    let gap = f
        .gazetteer
        .iter()
        .find(|g| {
            matches!(
                baseline(g.id).map(|r| r.coverage),
                Some(CoverageValue::Missing { .. })
            ) && g.centroid.is_some()
        })
        .expect("the 2023-24 fixtures have counties with missing coverage");
    let radius = koplik_epi::distance_km(focus, gap.centroid.unwrap()) + 1.0;
    let config = ScenarioConfig {
        neighbourhood: Some(Neighbourhood {
            radius_km: radius,
            gravity: GravityParameters {
                scale: 1.0,
                origin_exponent: 1.0,
                destination_exponent: 1.0,
                distance_exponent: 1.0,
            },
        }),
        ..quick()
    };
    let built = scenario::build(&config, &sources(&f)).unwrap();
    let input = &built.input;
    assert!(input.nodes.len() > 1);
    assert!(input.nodes.windows(2).all(|w| w[0].id < w[1].id));
    assert!(input.parameters.gravity.is_some());
    for n in &input.nodes {
        let near = koplik_epi::distance_km(
            focus,
            f.gazetteer
                .iter()
                .find(|g| g.id == n.id)
                .unwrap()
                .centroid
                .unwrap(),
        );
        assert!(near <= radius, "{} is {near} km away", n.id);
        // Only the introduced county starts with infection, and no baseline is imputed.
        if n.id == gaines() {
            assert_eq!((n.initial_exposed, n.initial_infectious), (0, 1));
        } else {
            assert_eq!((n.initial_exposed, n.initial_infectious), (0, 0));
        }
        assert!(matches!(
            n.baseline_coverage,
            BaselineCoverage::Reported { imputed: false, .. }
        ));
    }
    let excluded = built
        .provenance
        .excluded_nodes
        .iter()
        .find(|e| e.geography == gap.id)
        .expect("the county with missing coverage is listed as excluded");
    assert!(excluded.reason.contains("missing"), "{}", excluded.reason);
    assert!(input.nodes.iter().all(|n| n.id != gap.id));
    built.provenance.check_against(input).unwrap();
}

#[test]
fn the_provenance_companion_round_trips_and_a_changed_scenario_does_not_match() {
    let f = load();
    let built = scenario::build(&quick(), &sources(&f)).unwrap();
    let json = serde_json::to_vec(&built.provenance).unwrap();
    let back: ScenarioProvenance = serde_json::from_slice(&json).unwrap();
    assert_eq!(back, built.provenance);
    let mut other = built.input.clone();
    other.nodes[0].initial_infectious += 1;
    assert!(back.check_against(&other).is_err());
    let mut other = built.input.clone();
    other.seed += 1;
    assert!(back.check_against(&other).is_err());
}
