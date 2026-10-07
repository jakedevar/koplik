//! Offline pipeline tests against the committed real-byte fixtures in `data/fixtures/`.
//! Nothing here touches the network: live ingest is only exercised up to its contact refusal.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use koplik_contracts::v1::{
    BaselineCoverage, Forecast, GeoId, Geography, KindergartenMmrCoverage, RtEstimate, RtStatus,
    ScenarioInput,
};
use koplik_contracts::v3::{CaseDefinition, WeeklyCaseCount};
use koplik_contracts::v4::ScenarioProvenance;
use koplik_contracts::v7::{
    ForecastProvenance, ForecastStatus, InformationBasis, InsufficientReason, SeriesSkill,
};
use koplik_ingest::store::sha256_of;
use koplik_pipeline::{Config, FileHash, ItemStatus, Manifest, Mode, Stage, run_stage};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_config(root: &Path) -> Config {
    let work = root.join("work");
    Config {
        mode: Mode::Fixtures,
        store: Config::default_store(Mode::Fixtures, &work),
        work,
        out: root.join("out"),
        fixtures: repo().join("data/fixtures"),
        reports: repo().join("data/reports"),
    }
}

fn run_all(config: &Config) -> BTreeMap<Stage, Manifest> {
    Stage::ALL
        .into_iter()
        .map(|s| (s, run_stage(s, config).unwrap()))
        .collect()
}

/// Every regular file under `root`, with its bytes, keyed by relative path.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn assert_hashes_match(root: &Path, files: &[FileHash]) {
    assert!(!files.is_empty());
    for f in files {
        let bytes = fs::read(root.join(&f.path)).unwrap_or_else(|e| panic!("{}: {e}", f.path));
        assert_eq!(sha256_of(&bytes), f.sha256, "{}", f.path);
        assert_eq!(bytes.len() as u64, f.bytes, "{}", f.path);
    }
}

fn read<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> T {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn read_rows<T: koplik_contracts::v6::PublicationRow>(path: &Path) -> Vec<T> {
    read::<koplik_contracts::v6::RowArtifact<T>>(path).rows
}

/// Compare every expanded field, including the exact ordered provenance on every row.
fn assert_lossless<T: koplik_contracts::v6::PublicationRow + PartialEq + std::fmt::Debug>(
    published: &Path,
    original: &Path,
) {
    let expanded = read_rows::<T>(published);
    let original: Vec<T> = read(original);
    assert_eq!(expanded, original, "{}", published.display());
}

#[test]
fn fixture_pipeline_is_byte_identical_on_rerun_and_manifest_hashes_match_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    let first = run_all(&config);
    let before = tree(dir.path());
    let second = run_all(&config);
    assert_eq!(first, second);
    assert_eq!(before, tree(dir.path()), "a re-run changed some bytes");
    // Seeding is idempotent: every fixture retrieval of a known source is logged once.
    let ingest = &first[&Stage::Ingest];
    let log = fs::read_to_string(config.store.join("retrievals.jsonl")).unwrap();
    assert_eq!(log.lines().count(), ingest.outputs.len());

    // Manifest hashes match the files they describe, and the work copy of each manifest is
    // what run_stage returned.
    for (stage, m) in &first {
        let on_disk: Manifest = read(&config.manifest_path(*stage));
        assert_eq!(&on_disk, m);
        assert_eq!(m.mode, Mode::Fixtures);
        match stage {
            Stage::Ingest => {
                assert_hashes_match(&config.fixtures, &m.inputs);
                assert_hashes_match(&config.store, &m.outputs);
            }
            Stage::Validate => {
                assert_hashes_match(&config.store, &m.inputs);
                assert_hashes_match(&config.work, &m.outputs);
            }
            Stage::Infer => {
                assert_hashes_match(&config.work, &m.inputs);
                assert_hashes_match(&config.work, &m.outputs);
            }
            Stage::Forecast => {
                assert_hashes_match(&config.work, &m.inputs);
                assert_hashes_match(&config.work, &m.outputs);
                assert_eq!(
                    m.inputs.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
                    ["validate/weekly-cases.json"]
                );
                assert!(matches!(m.items["forecast"], ItemStatus::Present { .. }));
            }
            Stage::Build => {
                assert_hashes_match(&config.work, &m.inputs);
                assert_hashes_match(&config.out, &m.outputs);
                assert_eq!(
                    m.stages.keys().cloned().collect::<Vec<_>>(),
                    ["forecast", "infer", "ingest", "validate"]
                );
                assert_eq!(&m.stages["validate"], &first[&Stage::Validate]);
                let published: Manifest = read(&config.out.join("manifest.json"));
                assert_eq!(&published, m);
            }
        }
    }

    // The ingest manifest's outputs are every snapshot the rows will cite; the rejected Census
    // API response is skipped by source id, not ingested.
    let snapshots: BTreeSet<String> = ingest
        .outputs
        .iter()
        .map(|f| f.sha256.to_string())
        .collect();
    assert!(snapshots.len() >= 16, "{}", snapshots.len());
    assert!(!ingest.items.contains_key("census-2020-texas-county-names"));
    assert!(
        ingest
            .notes
            .iter()
            .any(|n| n.contains("census-api-missing-key"))
    );
    for id in [
        "cdc-nndss-weekly-measles",
        "dshs-measles-outbreak-page-wayback",
        "dshs-measles-data-report-wayback",
        "census-county-codes-2020-wayback",
        "census-cb-2024-states-20m",
        "census-cb-2024-counties-20m",
        "census-county-population-2025",
        "census-county-gazetteer-2025",
        "census-state-population-2025",
        "census-state-gazetteer-2025",
    ] {
        assert!(
            matches!(ingest.items[id], ItemStatus::Present { .. }),
            "{id}"
        );
    }

    // Published artifacts are valid contract rows, each tracing to an ingested snapshot.
    let out = config.out.join("v6");
    let cases: Vec<WeeklyCaseCount> = read_rows(&out.join("weekly-cases.json"));
    let coverage: Vec<KindergartenMmrCoverage> = read_rows(&out.join("coverage.json"));
    let geographies: Vec<Geography> = read_rows(&out.join("geographies.json"));
    let rt: Vec<RtEstimate> = read_rows(&out.join("rt.json"));
    assert_lossless::<WeeklyCaseCount>(
        &out.join("weekly-cases.json"),
        &config.work.join("validate/weekly-cases.json"),
    );
    assert_lossless::<KindergartenMmrCoverage>(
        &out.join("coverage.json"),
        &config.work.join("validate/coverage.json"),
    );
    assert_lossless::<Geography>(
        &out.join("geographies.json"),
        &config.work.join("validate/geographies.json"),
    );
    assert_lossless::<RtEstimate>(&out.join("rt.json"), &config.work.join("infer/rt.json"));
    assert_lossless::<Forecast>(
        &config.out.join("forecasts/weekly-cases.json"),
        &config.work.join("forecast/forecast.json"),
    );
    // CDC: 56 states and territories x 91 weeks, confirmed-or-unknown; DSHS: Texas counties,
    // confirmed only, derived from the report vintages in the fixtures.
    let (states, counties): (Vec<_>, Vec<_>) = cases
        .iter()
        .partition(|r| matches!(r.geography, GeoId::State(_)));
    assert_eq!(states.len(), 56 * (53 + 38));
    assert!(
        states
            .iter()
            .all(|r| r.case_definition == CaseDefinition::ConfirmedOrUnknownStatus)
    );
    assert!(!counties.is_empty());
    assert!(counties.iter().all(|r| {
        r.case_definition == CaseDefinition::Confirmed && r.geography.to_string().starts_with("48")
    }));
    // The fixtures hold five DSHS vintages with a county table, weeks apart, so the derived
    // weekly county series is mostly explicit `missing`; the cumulative series carries the
    // published Gaines County counts and every row traces to the DSHS and Census snapshots.
    let cumulative: Vec<serde_json::Value> =
        read(&config.work.join("validate/dshs-cumulative.json"));
    assert!(cumulative.iter().any(|r| {
        r["geography"] == "48165"
            && r["cases"]["status"] == "reported"
            && r["cases"]["count"].as_u64().unwrap() > 0
    }));
    assert_eq!(coverage.len(), 51 * 2 + 254 * 2);
    assert_eq!(geographies.len(), 56 + 254);
    // One estimate row per (geography, grid week, level); the grid fills omitted weeks.
    assert!(rt.len() >= cases.len() * 2);
    let mut provenance = Vec::new();
    provenance.extend(cases.iter().flat_map(|r| r.provenance.as_slice()));
    provenance.extend(coverage.iter().flat_map(|r| r.provenance.as_slice()));
    provenance.extend(geographies.iter().flat_map(|r| r.provenance.as_slice()));
    provenance.extend(rt.iter().flat_map(|r| r.provenance.as_slice()));
    for p in provenance {
        assert!(
            snapshots.contains(p.sha256.as_str()),
            "{} {}",
            p.source_id,
            p.url
        );
    }
    // Names come from the Census boundary files when present (public domain), else from the
    // observation source: Guam has CDC rows but no boundary at this scale.
    let named = |id: &str| {
        let g = geographies.iter().find(|g| g.id.to_string() == id).unwrap();
        (g.name.clone(), g.provenance.as_slice()[0].source_id.clone())
    };
    assert_eq!(
        named("48165"),
        (
            "Gaines County".to_owned(),
            "census-cb-2024-counties-20m".to_owned()
        )
    );
    assert_eq!(
        named("48"),
        ("Texas".to_owned(), "census-cb-2024-states-20m".to_owned())
    );
    assert_eq!(
        named("66"),
        ("Guam".to_owned(), "cdc-nndss-weekly-measles".to_owned())
    );
    // Sorted output (determinism is by construction, not by luck).
    assert!(
        cases
            .windows(2)
            .all(|w| (w[0].geography, w[0].week) < (w[1].geography, w[1].week))
    );
    assert!(geographies.windows(2).all(|w| w[0].id < w[1].id));
    // R_t publishes where counts allow and says so where they do not (Texas, 2025 week 9).
    let tx = rt
        .iter()
        .find(|r| {
            r.geography.to_string() == "48"
                && r.week.to_string() == "2025-W09"
                && r.interval_level == 0.95
        })
        .unwrap();
    assert_eq!(tx.status, RtStatus::Ok);
    assert!(rt.iter().any(|r| r.status == RtStatus::InsufficientData));

    // Boundaries are the converted Census files, with provenance on every feature and a
    // geography for every GEOID (the web loader requires both).
    let build = &first[&Stage::Build];
    let ids: BTreeSet<String> = geographies.iter().map(|g| g.id.to_string()).collect();
    for (name, n, source) in [
        ("us-states", 52, "census-cb-2024-states-20m"),
        ("texas-counties", 254, "census-cb-2024-counties-20m"),
    ] {
        assert!(
            matches!(build.items[name], ItemStatus::Present { rows: Some(rows), .. } if rows == n),
            "{name}"
        );
        let fc: serde_json::Value = read(&out.join(format!("{name}.json")));
        let features = fc["features"].as_array().unwrap();
        assert_eq!(features.len(), n as usize);
        for f in features {
            assert!(ids.contains(f["properties"]["GEOID"].as_str().unwrap()));
            assert_eq!(f["properties"]["provenance"][0]["source_id"], source);
        }
    }
    // The what-if scenario (#1455) is built from store snapshots only and published with its
    // provenance companion; both are in the build manifest's hashed outputs.
    assert!(matches!(
        build.items["gaines-2025"],
        ItemStatus::Present { rows: Some(1), .. }
    ));
    let scenario: ScenarioInput = read(&config.out.join("scenarios/gaines-2025.json"));
    let provenance: ScenarioProvenance =
        read(&config.out.join("scenarios/gaines-2025.provenance.json"));
    provenance.check_against(&scenario).unwrap();
    for rel in [
        "scenarios/gaines-2025.json",
        "scenarios/gaines-2025.provenance.json",
    ] {
        assert!(build.outputs.iter().any(|f| f.path == rel), "{rel}");
        assert!(build.inputs.iter().any(|f| f.path == rel), "{rel}");
    }
    let gaines = &scenario.nodes[0];
    assert_eq!(scenario.nodes.len(), 1);
    assert_eq!(gaines.id.to_string(), "48165");
    assert_eq!(gaines.population, 23956);
    // A hypothetical introduction (#1455), not a replay: one assumed infectious person, no DSHS
    // report involved, on a neutral reference week (the week of the estimate's July 1).
    assert_eq!((gaines.initial_infectious, gaines.initial_exposed), (1, 0));
    assert_eq!(scenario.start_week.to_string(), "2025-W27");
    assert_eq!(scenario.seed, koplik_pipeline::scenario::SEED);
    assert_eq!(scenario.run_count, 1000);
    assert_eq!(scenario.parameters, koplik_epi::default_parameters());
    assert!(scenario.coverage_overrides.is_empty());
    assert!(provenance.statement.starts_with(
        "Hypothetical: what could happen if one infectious person arrived in Gaines County"
    ));
    assert!(
        provenance
            .statement
            .contains("not a reconstruction or forecast of the 2025 outbreak")
    );
    assert_eq!(provenance.seeding.geography, gaines.id);
    // Every record the scenario cites is a snapshot the ingest manifest hashed.
    let scenario_records =
        gaines
            .provenance
            .as_slice()
            .iter()
            .chain(match &gaines.baseline_coverage {
                BaselineCoverage::Reported { provenance, .. } => provenance.as_slice(),
                BaselineCoverage::Missing { .. } => panic!("Gaines coverage is reported"),
            });
    for p in scenario_records {
        assert!(snapshots.contains(p.sha256.as_str()), "{}", p.source_id);
    }
    // Every population and Gazetteer snapshot the scenario read is a validate input.
    for id in [
        "census-county-population-2025",
        "census-county-gazetteer-2025",
    ] {
        let sha = match &first[&Stage::Ingest].items[id] {
            ItemStatus::Present {
                retrieval: Some(r), ..
            } => r.sha256.clone(),
            other => panic!("{id}: {other:?}"),
        };
        assert!(
            first[&Stage::Validate]
                .inputs
                .iter()
                .any(|f| f.sha256 == sha),
            "{id}"
        );
    }
    // Geography.centroid is the Gazetteer internal point, with the Gazetteer in its sources.
    let gaines_geo = geographies
        .iter()
        .find(|g| g.id.to_string() == "48165")
        .unwrap();
    assert_eq!(gaines_geo.centroid, Some(gaines.centroid));
    assert!(
        gaines_geo
            .provenance
            .as_slice()
            .iter()
            .any(|p| p.source_id == "census-county-gazetteer-2025")
    );
    assert!(
        geographies
            .iter()
            .filter(|g| g.id.to_string().starts_with("48"))
            .all(|g| g.centroid.is_some())
    );
    let validate = &first[&Stage::Validate];
    assert!(matches!(
        validate.items["texas-dshs-outbreak-cases"],
        ItemStatus::Present { .. }
    ));
    assert!(config.work.join("validate/dshs-vintages.json").is_file());
    assert!(matches!(
        validate.items["gaines-2025"],
        ItemStatus::Present { .. }
    ));
    // No artifact carries a run time: the only timestamps are recorded retrieval times.
    let text = String::from_utf8(fs::read(config.out.join("manifest.json")).unwrap()).unwrap();
    assert!(!text.contains("run_at") && !text.contains("generated_at"));
}

#[test]
fn build_publishes_the_scenario_with_its_companion_and_never_one_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    run_all(&config);
    // Without a converted boundary file the build writes an explicitly empty collection.
    fs::remove_file(config.work.join("validate/texas-counties.json")).unwrap();
    let m = run_stage(Stage::Build, &config).unwrap();
    assert!(matches!(
        m.items["us-states"],
        ItemStatus::Present { rows: Some(52), .. }
    ));
    assert!(matches!(
        m.items["texas-counties"],
        ItemStatus::Missing { .. }
    ));
    let empty: serde_json::Value = read(&config.out.join("v6/texas-counties.json"));
    assert_eq!(empty["features"].as_array().unwrap().len(), 0);
    assert!(matches!(m.items["gaines-2025"], ItemStatus::Present { .. }));
    assert!(config.out.join("scenarios/gaines-2025.json").is_file());
    assert!(
        config
            .out
            .join("scenarios/gaines-2025.provenance.json")
            .is_file()
    );
    assert_hashes_match(&config.out, &m.outputs);

    // A scenario without its companion is refused, not published bare.
    let companion = config.work.join("scenarios/gaines-2025.provenance.json");
    let saved = fs::read(&companion).unwrap();
    fs::remove_file(&companion).unwrap();
    let err = run_stage(Stage::Build, &config).unwrap_err().to_string();
    assert!(err.contains("provenance companion"), "{err}");

    // So is a companion that describes a different scenario (here an explicitly synthetic one).
    fs::write(&companion, &saved).unwrap();
    fs::copy(
        repo().join("data/fixtures/seir/synthetic-scenario.json"),
        config.work.join("scenarios/gaines-2025.json"),
    )
    .unwrap();
    let err = run_stage(Stage::Build, &config).unwrap_err().to_string();
    assert!(err.contains("does not describe the scenario"), "{err}");

    // Removing the scenario removes the artifact: the output tree is a function of the inputs.
    fs::remove_file(config.work.join("scenarios/gaines-2025.json")).unwrap();
    fs::remove_file(&companion).unwrap();
    let m = run_stage(Stage::Build, &config).unwrap();
    assert!(matches!(m.items["gaines-2025"], ItemStatus::Missing { .. }));
    assert!(!config.out.join("scenarios").exists());
}

#[test]
fn a_source_that_disappears_between_runs_leaves_no_stale_output_and_is_reported_missing() {
    let dir = tempfile::tempdir().unwrap();
    let full = fixture_config(dir.path());
    run_all(&full);
    let populated = tree(&full.out);
    assert!(full.work.join("validate/us-states.json").is_file());
    assert!(full.work.join("validate/dshs-vintages.json").is_file());
    // Same work and out trees, but a store with no snapshots at all.
    let empty = Config {
        store: dir.path().join("empty-store"),
        ..full.clone()
    };
    for stage in [Stage::Validate, Stage::Infer, Stage::Build] {
        run_stage(stage, &empty).unwrap();
    }
    let validate: Manifest = read(&empty.manifest_path(Stage::Validate));
    for name in [
        "us-states",
        "texas-counties",
        "dshs-vintages",
        "dshs-cumulative",
    ] {
        assert!(
            !empty.work.join(format!("validate/{name}.json")).exists(),
            "stale {name}"
        );
    }
    for id in [
        "cdc-nndss-weekly-measles",
        "census-cb-2024-states-20m",
        "census-cb-2024-counties-20m",
        "texas-dshs-outbreak-cases",
        "texas-dshs-county-fips",
    ] {
        assert!(
            matches!(validate.items[id], ItemStatus::Missing { .. }),
            "{id}"
        );
    }
    let build: Manifest = read(&empty.out.join("manifest.json"));
    for name in ["us-states", "texas-counties"] {
        assert!(
            matches!(build.items[name], ItemStatus::Missing { .. }),
            "{name}"
        );
    }
    // No source, no scenario: nothing stale in the work or web tree, and both stages say why.
    assert!(matches!(
        validate.items["gaines-2025"],
        ItemStatus::Missing { .. }
    ));
    assert!(matches!(
        build.items["gaines-2025"],
        ItemStatus::Missing { .. }
    ));
    assert!(!empty.work.join("scenarios").exists());
    assert!(!empty.out.join("scenarios").exists());
    // The published artifacts stay mutually consistent: no boundary without a geography, no
    // row without a geography, every file the web loader needs present.
    let out = empty.out.join("v6");
    let geographies: Vec<Geography> = read_rows(&out.join("geographies.json"));
    let cases: Vec<WeeklyCaseCount> = read_rows(&out.join("weekly-cases.json"));
    let rt: Vec<RtEstimate> = read_rows(&out.join("rt.json"));
    assert!(geographies.is_empty() && cases.is_empty() && rt.is_empty());
    for name in ["us-states", "texas-counties"] {
        let fc: serde_json::Value = read(&out.join(format!("{name}.json")));
        assert_eq!(fc["type"], "FeatureCollection");
        assert_eq!(fc["features"].as_array().unwrap().len(), 0);
    }
    assert_hashes_match(&empty.out, &build.outputs);
    // And the other way round: the sources come back, so do the artifacts, byte-identical.
    for stage in [Stage::Validate, Stage::Infer, Stage::Build] {
        run_stage(stage, &full).unwrap();
    }
    assert_eq!(populated, tree(&full.out));
    let again = run_all(&full);
    assert_eq!(
        again[&Stage::Build].outputs,
        read::<Manifest>(&full.out.join("manifest.json")).outputs
    );
}

#[test]
fn build_hashes_every_stage_manifest_it_consumes() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    let first = run_all(&config);
    let build = &first[&Stage::Build];
    let hashed: BTreeMap<String, FileHash> = build
        .inputs
        .iter()
        .filter(|f| f.path.ends_with(".manifest.json"))
        .map(|f| (f.path.clone(), f.clone()))
        .collect();
    assert_eq!(
        hashed.keys().cloned().collect::<Vec<_>>(),
        [
            "forecast.manifest.json",
            "infer.manifest.json",
            "ingest.manifest.json",
            "validate.manifest.json"
        ]
    );
    assert_hashes_match(&config.work, &hashed.values().cloned().collect::<Vec<_>>());
    // A consumed manifest that changes changes the recorded hash.
    let path = config.manifest_path(Stage::Forecast);
    let mut forecast: Manifest = read(&path);
    forecast.notes.push("changed for the test".into());
    fs::write(&path, serde_json::to_vec(&forecast).unwrap()).unwrap();
    let rebuilt = run_stage(Stage::Build, &config).unwrap();
    let after = rebuilt
        .inputs
        .iter()
        .find(|f| f.path == "forecast.manifest.json")
        .unwrap();
    assert_ne!(after.sha256, hashed["forecast.manifest.json"].sha256);
    assert_eq!(rebuilt.stages["forecast"], forecast);
    assert_hashes_match(&config.work, &rebuilt.inputs);
}

#[test]
fn fixture_bytes_that_do_not_match_their_record_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = dir.path().join("fixtures/cdc");
    fs::create_dir_all(&fixtures).unwrap();
    for name in [
        "nndss-measles-weekly.json",
        "nndss-measles-weekly.retrieval.json",
    ] {
        fs::copy(
            repo().join("data/fixtures/cdc").join(name),
            fixtures.join(name),
        )
        .unwrap();
    }
    let mut config = fixture_config(dir.path());
    config.fixtures = dir.path().join("fixtures");
    assert!(run_stage(Stage::Ingest, &config).is_ok());
    let tampered = dir.path().join("tampered");
    fs::create_dir_all(tampered.join("cdc")).unwrap();
    fs::copy(
        fixtures.join("nndss-measles-weekly.retrieval.json"),
        tampered.join("cdc/nndss-measles-weekly.retrieval.json"),
    )
    .unwrap();
    let mut bytes = fs::read(fixtures.join("nndss-measles-weekly.json")).unwrap();
    bytes.push(b' ');
    fs::write(tampered.join("cdc/nndss-measles-weekly.json"), bytes).unwrap();
    config.fixtures = tampered;
    config.work = dir.path().join("work2");
    config.store = Config::default_store(Mode::Fixtures, &config.work);
    let err = run_stage(Stage::Ingest, &config).unwrap_err().to_string();
    assert!(err.contains("do not match the retrieval record"), "{err}");
}

#[test]
fn validate_on_an_empty_store_reports_every_source_missing_and_writes_empty_rows() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    let m = run_stage(Stage::Validate, &config).unwrap();
    assert!(m.items.values().all(|i| match i {
        ItemStatus::Missing { .. } => true,
        ItemStatus::Present { rows: Some(0), .. } => true,
        _ => false,
    }));
    let cases: Vec<WeeklyCaseCount> = read(&config.work.join("validate/weekly-cases.json"));
    assert!(cases.is_empty());
    // infer still runs (nothing to estimate) and build needs every stage output.
    let infer = run_stage(Stage::Infer, &config).unwrap();
    assert!(matches!(
        infer.items["rt"],
        ItemStatus::Present { rows: Some(0), .. }
    ));
}

/// Every spawned binary opts out of live fetching: a blank `KOPLIK_CONTACT` refuses before any
/// request (an unset variable would resolve to the default contact and go to the network).
fn cli() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_koplik-pipeline"));
    cmd.env("KOPLIK_CONTACT", "");
    cmd
}

#[test]
fn live_ingest_refuses_a_blank_contact_and_touches_nothing() {
    for contact in ["", "  "] {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("snapshots");
        let work = dir.path().join("work");
        let out = cli()
            .args(["ingest", "--store"])
            .arg(&store)
            .arg("--work")
            .arg(&work)
            .env("KOPLIK_CONTACT", contact)
            .output()
            .unwrap();
        assert!(!out.status.success(), "{contact:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("KOPLIK_CONTACT"), "{err}");
        assert!(
            !store.exists() && !work.exists(),
            "nothing may be created before the contact check"
        );
    }
}

/// Live ingest resolves its identity through `PoliteConfig::live_from_env` (#1427): a blank
/// Census contact refuses before any request even when the general contact is set. Proxies point
/// at an unroutable TEST-NET address and the working directory has no `.env.local`, so a
/// regression could not reach any real host.
#[test]
fn live_ingest_refuses_a_blank_census_contact_and_touches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("snapshots");
    let work = dir.path().join("work");
    let out = cli()
        .current_dir(dir.path())
        .args(["ingest", "--store"])
        .arg(&store)
        .arg("--work")
        .arg(&work)
        .env("KOPLIK_CONTACT", "pipeline-test@example.invalid")
        .env("KOPLIK_CENSUS_CONTACT", " ")
        .env("HTTP_PROXY", "http://203.0.113.1:9")
        .env("HTTPS_PROXY", "http://203.0.113.1:9")
        .env("ALL_PROXY", "http://203.0.113.1:9")
        .env("NO_PROXY", "")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Census contact"), "{err}");
    assert!(
        !store.exists() && !work.exists(),
        "nothing may be created before the contact check"
    );
}

#[test]
fn cli_runs_every_stage_from_fixtures_offline() {
    let dir = tempfile::tempdir().unwrap();
    let out = cli()
        .args(["all", "--from-fixtures", "--work"])
        .arg(dir.path().join("work"))
        .arg("--out")
        .arg(dir.path().join("out"))
        .arg("--fixtures")
        .arg(repo().join("data/fixtures"))
        .arg("--reports")
        .arg(repo().join("data/reports"))
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    for stage in Stage::ALL {
        assert!(
            err.contains(&format!("{} [fixtures]", stage.name())),
            "{err}"
        );
        assert!(
            dir.path()
                .join(format!("work/{}.manifest.json", stage.name()))
                .is_file()
        );
    }
    assert!(
        dir.path()
            .join("work/fixture-snapshots/retrievals.jsonl")
            .is_file()
    );
    assert!(dir.path().join("out/v6/weekly-cases.json").is_file());
    assert!(dir.path().join("out/forecasts/weekly-cases.json").is_file());
    assert!(
        dir.path()
            .join("out/forecasts/backtest-west-texas-2025.json")
            .is_file()
    );
    // Unknown stages and dangling flags are usage errors.
    let bad = cli().args(["publish"]).output().unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("usage"));
    let bad = cli().args(["build", "--work"]).output().unwrap();
    assert!(!bad.status.success());
}

// ---------------------------------------------------------------------------------------------
// forecast (#1465)

/// The forecasts the publication policy withheld: audit rows in the work directory, never published.
fn withheld_rows(config: &Config) -> Vec<Forecast> {
    read(&config.work.join("forecast/withheld.json"))
}

fn forecast_outputs(config: &Config) -> (Vec<Forecast>, ForecastProvenance) {
    (
        read(&config.work.join("forecast/forecast.json")),
        read(&config.work.join("forecast/forecast.provenance.json")),
    )
}

#[test]
fn forecast_runs_the_pre_registered_method_on_the_published_series_with_its_provenance() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    run_all(&config);
    let (rows, p) = forecast_outputs(&config);
    let cases: Vec<WeeklyCaseCount> = read(&config.work.join("validate/weekly-cases.json"));

    // The method and its configuration are the pre-registered defaults, with an explicit seed.
    assert_eq!(
        (p.seed, p.run_count, p.horizon_weeks),
        (koplik_pipeline::forecast_stage::FORECAST_SEED, 1000, 8)
    );
    assert_eq!(p.levels.len(), 23);
    let parameter = |name: &str| p.parameters.iter().find(|x| x.parameter == name).unwrap();
    assert_eq!(parameter("window_weeks").value, serde_json::json!(3));
    assert_eq!(parameter("min_cases").value, serde_json::json!(11));
    assert_eq!(parameter("seed").value, serde_json::json!(p.seed));

    // The origin is two weeks before the latest data, and every target is after it.
    let latest = cases.iter().map(|r| r.week).max().unwrap();
    assert_eq!(p.latest_data_week, latest);
    assert_eq!(p.origin_week, latest.prev().unwrap().prev().unwrap());
    // The method forecast series the rule lets it forecast; the publication policy then withheld
    // every one of them, so the published rows are exactly the series with status `forecast`.
    let withheld = withheld_rows(&config);
    assert!(
        !withheld.is_empty(),
        "the fixtures hold series the rule lets us forecast"
    );
    assert!(
        rows.iter()
            .chain(&withheld)
            .all(|r| r.origin_week == p.origin_week && r.target_week > p.origin_week)
    );
    p.check_against(&rows).unwrap();

    // The input series is hashed as it was read, and every series is accounted for once.
    let weekly = fs::read(config.work.join("validate/weekly-cases.json")).unwrap();
    assert_eq!(p.input.sha256, sha256_of(&weekly));
    assert_eq!(p.input.rows as usize, cases.len());
    let geographies: BTreeSet<GeoId> = cases.iter().map(|r| r.geography).collect();
    assert_eq!(
        p.series
            .iter()
            .map(|s| s.geography)
            .collect::<BTreeSet<_>>(),
        geographies
    );

    // A series is forecast only where the minimum-count rule holds; every other series says why.
    let made: Vec<_> = p
        .series
        .iter()
        .filter(|s| s.status != ForecastStatus::InsufficientData)
        .collect();
    assert!(!made.is_empty());
    for s in &made {
        assert!(
            s.cases_in_window.unwrap() >= 11,
            "{}: {:?}",
            s.geography,
            s.cases_in_window
        );
    }
    let thin = p
        .series
        .iter()
        .filter(|s| s.reason == Some(InsufficientReason::BelowThreshold))
        .collect::<Vec<_>>();
    assert!(!thin.is_empty());
    assert!(thin.iter().all(|s| s.cases_in_window.unwrap() < 11));
    // What was published is what the policy admitted; what was kept for audit is what it withheld.
    let published: BTreeSet<GeoId> = rows.iter().map(|r| r.geography).collect();
    assert_eq!(
        published,
        made.iter()
            .filter(|s| s.status == ForecastStatus::Forecast)
            .map(|s| s.geography)
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        withheld
            .iter()
            .map(|r| r.geography)
            .collect::<BTreeSet<_>>(),
        made.iter()
            .filter(|s| s.status == ForecastStatus::Withheld)
            .map(|s| s.geography)
            .collect::<BTreeSet<_>>()
    );
    // Texas counties come from DSHS reports that end long before the origin: insufficient, not
    // carried forward.
    let county = p
        .series
        .iter()
        .find(|s| s.geography.to_string() == "48165")
        .unwrap();
    assert_eq!(county.status, ForecastStatus::InsufficientData);
    assert!(
        rows.iter()
            .chain(&withheld)
            .all(|r| r.geography.to_string() != "48165")
    );
    let rows: Vec<Forecast> = rows.into_iter().chain(withheld).collect();

    // Every forecast describes the case definition of its own series.
    for s in &p.series {
        let mut definitions: Vec<_> = cases
            .iter()
            .filter(|r| r.geography == s.geography)
            .map(|r| r.case_definition)
            .collect();
        definitions.dedup();
        assert_eq!(definitions, [s.case_definition]);
    }
    // Every row carries source records that are the series' own.
    for r in &rows {
        let sources: BTreeSet<&str> = r
            .provenance
            .as_slice()
            .iter()
            .map(|x| x.source_id.as_str())
            .collect();
        let own: BTreeSet<&str> = cases
            .iter()
            .filter(|c| c.geography == r.geography)
            .flat_map(|c| c.provenance.as_slice().iter().map(|x| x.source_id.as_str()))
            .collect();
        assert_eq!(sources, own);
    }
}

#[test]
fn the_backtest_skill_is_the_committed_report_exactly_and_its_scope_is_stated() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    run_all(&config);
    let (_, p) = forecast_outputs(&config);
    let skill = p
        .backtest
        .as_ref()
        .expect("the committed report matches the configuration");
    let path = repo().join("data/reports/backtest/west-texas-2025.json");
    let bytes = fs::read(&path).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let pooled = &report["primary"]["pooled"];
    assert_eq!(skill.report_sha256, sha256_of(&bytes));
    assert_eq!(
        skill.report_path,
        "data/reports/backtest/west-texas-2025.json"
    );
    assert_eq!(skill.targets as u64, pooled["n"].as_u64().unwrap());
    assert_eq!(skill.mean_crps, pooled["mean_crps"].as_f64().unwrap());
    assert_eq!(skill.coverage_50, pooled["coverage_50"].as_f64().unwrap());
    assert_eq!(skill.coverage_90, pooled["coverage_90"].as_f64().unwrap());
    assert_eq!(
        skill.manifest_sha256.as_str(),
        report["manifest_sha256"].as_str().unwrap()
    );
    assert_eq!(
        skill.seed, p.seed,
        "the published forecast runs with the backtest's seed"
    );
    assert_eq!(skill.seed, report["primary"]["seed"].as_u64().unwrap());
    assert_eq!(skill.by_horizon.len(), 8);
    // The measured numbers, as recorded in thoughts/shared/research/backtest-2025-west-texas.md.
    assert_eq!(skill.targets, 48);
    assert_eq!(format!("{:.2}", skill.mean_crps), "3.66");
    assert_eq!(
        (
            format!("{:.2}", skill.coverage_50),
            format!("{:.2}", skill.coverage_90)
        ),
        ("0.48".to_owned(), "0.62".to_owned())
    );
    assert_eq!((skill.forecast_dates, skill.origin_weeks), (7, 5));
    // The backtest scored the Texas DSHS outbreak total, not any series forecast here.
    assert!(skill.series.contains("Texas DSHS outbreak total"));
    assert!(skill.series.contains("confirmed"));
    assert!(skill.limitations[0].contains("no county-level backtest"));
    // None of the series forecast is the DSHS outbreak total: the report-vintage backtest speaks
    // for none of them (the NNDSS state series have their own evaluation, checked below).
    assert!(p.series.iter().all(|s| s.skill != SeriesSkill::Backtested));
    assert!(
        p.scope_note.contains("0 are published")
            && p.scope_note.contains("withheld by the publication policy")
            && p.scope_note.contains("scored one series only"),
        "{}",
        p.scope_note
    );
    // The limitations quote the measured coverage with one precision and counts, and the range of
    // the scored targets (not of the whole history: 61 appears only there).
    let limitations = skill.limitations.join("\n");
    assert!(limitations.contains("62.5% (30 of 48)"), "{limitations}");
    assert!(limitations.contains("47.9% (23 of 48)"), "{limitations}");
    assert!(
        limitations
            .contains("48 weekly counts the forecasts were scored against ran from 0 to 10 cases"),
        "{limitations}"
    );
    assert!(!limitations.contains("61"), "{limitations}");
    // The report is published byte for byte beside the rows, so the numbers can be checked.
    assert_eq!(
        fs::read(config.work.join("forecast/backtest-west-texas-2025.json")).unwrap(),
        bytes
    );
}

/// The series published as forecast, by the skill the companion gives them.
fn skills_of(p: &ForecastProvenance) -> BTreeMap<GeoId, SeriesSkill> {
    p.series.iter().map(|s| (s.geography, s.skill)).collect()
}

#[test]
fn the_series_backtest_is_the_committed_report_exactly_and_labelled_pseudo_real_time() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    run_all(&config);
    let (_, p) = forecast_outputs(&config);
    let b = p
        .series_backtest
        .as_ref()
        .expect("the committed report matches the configuration");
    let path = repo().join("data/reports/backtest/cdc-states.json");
    let bytes = fs::read(&path).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(b.report_sha256, sha256_of(&bytes));
    assert_eq!(b.report_path, "data/reports/backtest/cdc-states.json");
    // It is the pseudo-real-time label, never real-time, in the typed basis and in the words.
    assert_eq!(b.basis, InformationBasis::PseudoRealTime);
    assert!(
        b.protocol
            .contains("pseudo-real-time (revised counts truncated at each forecast date)"),
        "{}",
        b.protocol
    );
    assert!(
        b.limitations
            .join("\n")
            .contains("not a real-time backtest")
    );
    // It ran on the committed NNDSS snapshot, and says so.
    let fixture = fs::read(repo().join("data/fixtures/cdc/nndss-measles-weekly.json")).unwrap();
    assert_eq!(b.input_sha256, sha256_of(&fixture));
    assert_eq!(
        b.input_sha256.as_str(),
        report["input"]["sha256"].as_str().unwrap()
    );
    // The floor and the seed are the pre-registered ones, and the published forecast's seed.
    assert_eq!((b.minimum_targets, b.minimum_origin_weeks), (40, 10));
    assert_eq!((b.seed, b.provisional_weeks), (p.seed, 2));
    // Every number is the report's, to the bit.
    let primary = &report["primary"];
    let pooled = b
        .pooled
        .as_ref()
        .expect("the pooled result reaches the floor");
    assert_eq!(
        pooled.scores.targets as u64,
        primary["pooled"]["n"].as_u64().unwrap()
    );
    assert_eq!(
        pooled.scores.mean_crps,
        primary["pooled"]["mean_crps"].as_f64().unwrap()
    );
    assert_eq!(
        pooled.scores.coverage_50,
        primary["pooled"]["coverage_50"].as_f64().unwrap()
    );
    assert_eq!(
        pooled.scores.coverage_90,
        primary["pooled"]["coverage_90"].as_f64().unwrap()
    );
    assert_eq!(pooled.scores.by_horizon.len(), 8);
    // The measured numbers, as recorded in thoughts/shared/research/backtest-cdc-states.md.
    assert_eq!(pooled.scores.targets, 1748);
    assert_eq!((pooled.forecasts, pooled.series), (239, 22));
    assert_eq!(
        (
            format!("{:.2}", pooled.scores.coverage_50),
            format!("{:.2}", pooled.scores.coverage_90)
        ),
        ("0.21".to_owned(), "0.39".to_owned())
    );
    // One entry per series of the report; scores only for the series that reach the floor.
    let reported = primary["series"].as_array().unwrap();
    assert_eq!(b.by_series.len(), reported.len());
    for (entry, series) in b.by_series.iter().zip(reported) {
        assert_eq!(
            entry.geography.to_string(),
            series["geography"].as_str().unwrap()
        );
        assert_eq!(
            entry.targets as u64,
            series["pooled"]["n"].as_u64().unwrap()
        );
        assert_eq!(
            entry.measured.is_some(),
            series["measured"].as_bool().unwrap()
        );
        if let Some(m) = &entry.measured {
            assert!(entry.targets >= 40 && entry.origin_weeks >= 10);
            assert_eq!(m.mean_crps, series["pooled"]["mean_crps"].as_f64().unwrap());
            assert_eq!(
                m.mean_persistence_abs_error,
                series["pooled"]["mean_persistence_abs_error"]
                    .as_f64()
                    .unwrap()
            );
        } else {
            assert!(entry.targets < 40 || entry.origin_weeks < 10);
        }
    }
    let texas = b
        .by_series
        .iter()
        .find(|e| e.geography.to_string() == "48")
        .unwrap();
    assert_eq!((texas.targets, texas.origin_weeks), (288, 36));
    assert_eq!(
        format!("{:.2}", texas.measured.as_ref().unwrap().mean_crps),
        "3374.24"
    );
    let measured: BTreeSet<String> = b
        .by_series
        .iter()
        .filter(|e| e.measured.is_some())
        .map(|e| e.geography.to_string())
        .collect();
    assert_eq!(
        measured.into_iter().collect::<Vec<_>>(),
        ["04", "20", "35", "42", "45", "48", "49"]
    );

    // Each series' skill is its own entry's: NNDSS state series are measured or insufficient, the
    // Texas DSHS county series (a different source and definition) were never scored.
    let skills = skills_of(&p);
    for s in &p.series {
        match s.case_definition {
            CaseDefinition::ConfirmedOrUnknownStatus => {
                let entry = b.by_series.iter().find(|e| e.geography == s.geography);
                match entry {
                    Some(e) if e.measured.is_some() => {
                        assert_eq!(
                            skills[&s.geography],
                            SeriesSkill::Measured,
                            "{}",
                            s.geography
                        )
                    }
                    Some(_) => assert_eq!(
                        skills[&s.geography],
                        SeriesSkill::InsufficientData,
                        "{}",
                        s.geography
                    ),
                    None => assert_eq!(skills[&s.geography], SeriesSkill::NotBacktested),
                }
            }
            _ => assert_eq!(
                skills[&s.geography],
                SeriesSkill::NotBacktested,
                "{}",
                s.geography
            ),
        }
    }
    assert!(
        skills.values().any(|k| *k == SeriesSkill::Measured)
            && skills.values().any(|k| *k == SeriesSkill::InsufficientData)
            && skills.values().any(|k| *k == SeriesSkill::NotBacktested),
        "the fixtures exercise every status: {skills:?}"
    );
    assert!(
        p.scope_note.contains("pseudo-real-time"),
        "{}",
        p.scope_note
    );
    // The publication policy, applied mechanically to the committed measurements: no NNDSS state
    // series qualifies (every series with a measured skill has a mean CRPS above its persistence
    // error), so no forecast of any is published; Pennsylvania, measured, is withheld for being
    // below the policy, and the others for having insufficient data for a skill.
    let policy = &p.publication_policy;
    assert_eq!(
        (
            policy.minimum_coverage_90,
            policy.maximum_crps_over_persistence
        ),
        (0.75, 1.0)
    );
    for e in b.by_series.iter().filter_map(|e| e.measured.as_ref()) {
        assert!(!policy.admits(e.coverage_90, e.mean_crps, e.mean_persistence_abs_error));
    }
    assert!(
        !p.series
            .iter()
            .any(|s| s.status == ForecastStatus::Forecast),
        "no state series qualifies today"
    );
    use koplik_contracts::v7::WithheldReason::{InsufficientDataForSkill, SkillBelowPolicy};
    let withheld: Vec<(String, _)> = p
        .series
        .iter()
        .filter(|s| s.status == ForecastStatus::Withheld)
        .map(|s| (s.geography.to_string(), s.withheld.unwrap()))
        .collect();
    assert_eq!(
        withheld,
        [
            ("21".to_owned(), InsufficientDataForSkill),
            ("24".to_owned(), InsufficientDataForSkill),
            ("36".to_owned(), InsufficientDataForSkill),
            ("39".to_owned(), InsufficientDataForSkill),
            ("42".to_owned(), SkillBelowPolicy),
            ("55".to_owned(), InsufficientDataForSkill),
        ]
    );
    assert!(read::<Vec<Forecast>>(&config.work.join("forecast/forecast.json")).is_empty());
    // The report is copied byte for byte beside the rows, and published by build.
    assert_eq!(
        fs::read(config.work.join("forecast/backtest-cdc-states.json")).unwrap(),
        bytes
    );
    run_stage(Stage::Build, &config).unwrap();
    assert_eq!(
        fs::read(config.out.join("forecasts/backtest-cdc-states.json")).unwrap(),
        bytes
    );
}

#[test]
fn a_series_backtest_run_with_another_configuration_or_without_its_label_is_not_attached() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = fixture_config(dir.path());
    let original: serde_json::Value = read(&repo().join("data/reports/backtest/cdc-states.json"));
    let attempt = |config: &mut Config, name: &str, edit: &dyn Fn(&mut serde_json::Value)| {
        let reports = dir.path().join(name);
        fs::create_dir_all(reports.join("backtest")).unwrap();
        let mut report = original.clone();
        edit(&mut report);
        fs::write(
            reports.join("backtest/cdc-states.json"),
            serde_json::to_vec(&report).unwrap(),
        )
        .unwrap();
        config.reports = reports;
        let all = run_all(config);
        (
            all[&Stage::Forecast].notes.clone(),
            forecast_outputs(config).1,
        )
    };
    for (name, edit) in [
        (
            "other-window",
            (&|r: &mut serde_json::Value| r["primary"]["window_weeks"] = serde_json::json!(2))
                as &dyn Fn(&mut serde_json::Value),
        ),
        ("other-seed", &|r| {
            r["primary"]["seed"] = serde_json::json!(1)
        }),
        ("other-provisional-weeks", &|r| {
            r["primary"]["provisional_weeks"] = serde_json::json!(1)
        }),
        ("real-time-label", &|r| {
            r["protocol"] = serde_json::json!("real-time by report vintage")
        }),
        ("other-source", &|r| {
            r["input"]["source_id"] = serde_json::json!("texas-dshs")
        }),
    ] {
        let (notes, p) = attempt(&mut config, name, edit);
        assert!(
            notes
                .iter()
                .any(|n| n.contains("no series backtest attached")),
            "{name}: {notes:?}"
        );
        assert!(p.series_backtest.is_none(), "{name}");
        assert!(
            p.series
                .iter()
                .all(|s| s.skill == SeriesSkill::NotBacktested),
            "{name}: without the evaluation no series claims a skill"
        );
    }
    // No report at all.
    config.reports = dir.path().join("no-reports");
    let all = run_all(&config);
    assert!(forecast_outputs(&config).1.series_backtest.is_none());
    assert!(
        all[&Stage::Forecast]
            .notes
            .iter()
            .any(|n| n.contains("no series backtest attached"))
    );
}

#[test]
fn without_a_report_for_exactly_this_configuration_no_skill_is_attached() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = fixture_config(dir.path());
    config.reports = dir.path().join("no-reports");
    let all = run_all(&config);
    let (rows, p) = forecast_outputs(&config);
    assert!(p.backtest.is_none());
    assert!(p.series_backtest.is_none());
    assert!(p.scope_note.contains("no skill has been measured"));
    // The forecast itself does not depend on the report, but without any measured skill none of it
    // can meet the publication policy: every forecast the method made is withheld, none published.
    assert!(rows.is_empty());
    assert!(!withheld_rows(&config).is_empty());
    let made: Vec<_> = p
        .series
        .iter()
        .filter(|s| s.status != ForecastStatus::InsufficientData)
        .collect();
    assert!(!made.is_empty());
    assert!(made.iter().all(|s| {
        s.status == ForecastStatus::Withheld
            && s.withheld == Some(koplik_contracts::v7::WithheldReason::NotBacktested)
    }));
    assert!(
        all[&Stage::Forecast]
            .notes
            .iter()
            .any(|n| n.contains("no backtest skill attached"))
    );
    assert!(
        !config
            .work
            .join("forecast/backtest-west-texas-2025.json")
            .exists()
    );
    assert!(
        !config
            .out
            .join("forecasts/backtest-west-texas-2025.json")
            .exists()
    );
    assert!(config.out.join("forecasts/weekly-cases.json").is_file());

    // A report run with another configuration is not attached either: its numbers would
    // describe a different method.
    let reports = dir.path().join("other-reports");
    fs::create_dir_all(reports.join("backtest")).unwrap();
    let mut report: serde_json::Value =
        read(&repo().join("data/reports/backtest/west-texas-2025.json"));
    report["primary"]["window_weeks"] = serde_json::json!(2);
    fs::write(
        reports.join("backtest/west-texas-2025.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    config.reports = reports;
    let m = run_stage(Stage::Forecast, &config).unwrap();
    assert!(
        m.notes
            .iter()
            .any(|n| n.contains("no backtest skill attached")),
        "{:?}",
        m.notes
    );
    assert!(forecast_outputs(&config).1.backtest.is_none());
}

#[test]
fn build_publishes_the_forecast_with_its_companion_and_never_one_without_it() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    run_all(&config);
    let m = run_stage(Stage::Build, &config).unwrap();
    assert!(matches!(m.items["forecast"], ItemStatus::Present { .. }));
    let out = config.out.join("forecasts");
    let rows: Vec<Forecast> = read_rows(&out.join("weekly-cases.json"));
    let companion: ForecastProvenance = read(&out.join("weekly-cases.provenance.json"));
    companion.check_against(&rows).unwrap();
    // Every forecast the method made is withheld, so the published artifact holds no forecast row,
    // and the audit rows are not published anywhere.
    assert!(rows.is_empty());
    assert!(
        companion
            .series
            .iter()
            .any(|s| s.status == ForecastStatus::Withheld)
    );
    assert!(!out.join("withheld.json").exists());
    assert!(
        m.outputs
            .iter()
            .all(|f| !f.path.contains("withheld") && !f.path.contains("forecast/forecast.json"))
    );
    assert!(!m.inputs.iter().any(|f| f.path.contains("withheld")));
    let skill = companion.backtest.as_ref().unwrap();
    assert_eq!(
        sha256_of(&fs::read(out.join("backtest-west-texas-2025.json")).unwrap()),
        skill.report_sha256
    );
    assert_hashes_match(&config.out, &m.outputs);
    assert!(
        m.inputs
            .iter()
            .any(|f| f.path == "forecast/forecast.provenance.json")
    );

    // Rows without their companion are refused, not published bare.
    let companion_path = config.work.join("forecast/forecast.provenance.json");
    let saved = fs::read(&companion_path).unwrap();
    fs::remove_file(&companion_path).unwrap();
    let err = run_stage(Stage::Build, &config).unwrap_err().to_string();
    assert!(err.contains("provenance companion"), "{err}");

    // So are rows that a companion does not describe (here, a withheld series' rows slipped into
    // the published file).
    fs::write(&companion_path, &saved).unwrap();
    assert!(
        read::<Vec<Forecast>>(&config.work.join("forecast/forecast.json")).is_empty(),
        "the policy withheld every series, so nothing is published"
    );
    let audit = withheld_rows(&config);
    assert!(!audit.is_empty());
    fs::write(
        config.work.join("forecast/forecast.json"),
        serde_json::to_vec(&audit).unwrap(),
    )
    .unwrap();
    let err = run_stage(Stage::Build, &config).unwrap_err().to_string();
    assert!(err.contains("does not describe the forecast rows"), "{err}");

    // A report copy that does not match the hash its skill cites is refused too.
    run_stage(Stage::Forecast, &config).unwrap();
    fs::write(
        config.work.join("forecast/backtest-west-texas-2025.json"),
        b"{}\n",
    )
    .unwrap();
    let err = run_stage(Stage::Build, &config).unwrap_err().to_string();
    assert!(err.contains("does not match the report hash"), "{err}");

    // With no forecast the artifact is absent and the manifest says so.
    fs::remove_dir_all(config.work.join("forecast")).unwrap();
    let m = run_stage(Stage::Build, &config).unwrap();
    assert!(matches!(m.items["forecast"], ItemStatus::Missing { .. }));
    assert!(!config.out.join("forecasts").exists());
}

#[test]
fn forecast_refuses_to_run_before_validate_and_publishes_nothing_for_an_empty_series() {
    let dir = tempfile::tempdir().unwrap();
    let config = fixture_config(dir.path());
    let err = run_stage(Stage::Forecast, &config).unwrap_err().to_string();
    assert!(err.contains("run the validate stage first"), "{err}");
    // An empty store validates to an empty series: nothing to forecast, said as missing.
    fs::create_dir_all(&config.store).unwrap();
    run_stage(Stage::Validate, &config).unwrap();
    let m = run_stage(Stage::Forecast, &config).unwrap();
    assert!(matches!(m.items["forecast"], ItemStatus::Missing { .. }));
    assert!(!config.work.join("forecast/forecast.json").exists());
}
