//! Offline pipeline tests against the committed real-byte fixtures in `data/fixtures/`.
//! Nothing here touches the network: live ingest is only exercised up to its contact refusal.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use koplik_contracts::v1::{
    BaselineCoverage, GeoId, Geography, KindergartenMmrCoverage, RtEstimate, RtStatus,
    ScenarioInput,
};
use koplik_contracts::v3::{CaseDefinition, WeeklyCaseCount};
use koplik_ingest::store::sha256_of;
use koplik_pipeline::scenario::ScenarioProvenance;
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
                assert!(m.inputs.is_empty() && m.outputs.is_empty());
                assert!(matches!(m.items["forecast"], ItemStatus::Skipped { .. }));
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
    let out = config.out.join("v1");
    let cases: Vec<WeeklyCaseCount> = read(&out.join("weekly-cases.json"));
    let coverage: Vec<KindergartenMmrCoverage> = read(&out.join("coverage.json"));
    let geographies: Vec<Geography> = read(&out.join("geographies.json"));
    let rt: Vec<RtEstimate> = read(&out.join("rt.json"));
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
    // The earliest DSHS vintage with a Gaines count: the 2025-03-04 report (107), MMWR 2025-W10.
    assert_eq!(
        (gaines.initial_infectious, gaines.initial_exposed),
        (107, 0)
    );
    assert_eq!(scenario.start_week.to_string(), "2025-W10");
    assert_eq!(scenario.seed, koplik_pipeline::scenario::SEED);
    assert_eq!(scenario.run_count, 1000);
    assert_eq!(scenario.parameters, koplik_epi::default_parameters());
    assert!(scenario.coverage_overrides.is_empty());
    assert_eq!(provenance.seeding.report_date.to_string(), "2025-03-04");
    assert!(provenance.seeding.skipped_vintages.is_empty());
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
    for p in scenario_records.chain(provenance.seeding.provenance.iter()) {
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
    let empty: serde_json::Value = read(&config.out.join("v1/texas-counties.json"));
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
    let out = empty.out.join("v1");
    let geographies: Vec<Geography> = read(&out.join("geographies.json"));
    let cases: Vec<WeeklyCaseCount> = read(&out.join("weekly-cases.json"));
    let rt: Vec<RtEstimate> = read(&out.join("rt.json"));
    assert!(geographies.is_empty() && cases.is_empty() && rt.is_empty());
    for name in ["us-states", "texas-counties"] {
        let fc: serde_json::Value = read(&out.join(format!("{name}.json")));
        assert_eq!(fc["type"], "FeatureCollection");
        assert_eq!(fc["features"].as_array().unwrap().len(), 0);
    }
    assert_hashes_match(&empty.out, &build.outputs);
    // And the other way round: the sources come back, so do the artifacts, byte-identical.
    let before = tree(&full.out);
    for stage in [Stage::Validate, Stage::Infer, Stage::Build] {
        run_stage(stage, &full).unwrap();
    }
    assert_ne!(before, tree(&full.out));
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
    assert!(dir.path().join("out/v1/weekly-cases.json").is_file());
    // Unknown stages and dangling flags are usage errors.
    let bad = cli().args(["publish"]).output().unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("usage"));
    let bad = cli().args(["build", "--work"]).output().unwrap();
    assert!(!bad.status.success());
}
