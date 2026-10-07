//! Koplik pipeline stages. Each stage is a pure function of its inputs on disk: it reads what
//! the previous stage wrote (or the snapshot store), writes its outputs, and writes one
//! manifest recording the SHA-256 of every input and output. Running a stage twice on the same
//! inputs produces byte-identical files, because nothing here records a clock: the only times in
//! any artifact are the retrieval times recorded in the snapshot store.
//!
//! ```text
//! ingest    network (live) or data/fixtures (--from-fixtures)  -> <store>  + <work>/ingest.manifest.json
//! validate  <store> latest snapshots                           -> <work>/validate/{weekly-cases,coverage,geographies,
//!                                                                 us-states,texas-counties,dshs-*,gaps}.json
//!                                                                 <work>/scenarios/gaines-2025{,.provenance}.json
//! infer     <work>/validate/weekly-cases.json                  -> <work>/infer/rt.json
//! forecast  <work>/validate/weekly-cases.json                  -> <work>/forecast/forecast{,.provenance}.json
//!                                                                 <work>/forecast/withheld.json (audit only, never published)
//!           + the committed backtest reports in <reports>         <work>/forecast/backtest-{west-texas-2025,cdc-states}.json
//! build     <work>/validate + <work>/infer + <work>/forecast   -> <out>/v6/*.json, <out>/scenarios/*.json,
//!                                                                 <out>/forecasts/*.json, <out>/manifest.json
//! ```
//!
//! Sources: CDC NNDSS weekly cases by state (v3 rows, `confirmed_or_unknown_status`), Texas
//! DSHS 2025 outbreak cases by county (v3 rows, `confirmed`, from report vintages), CDC
//! SchoolVaxView and Texas DSHS kindergarten MMR coverage, the DSHS county/FIPS crosswalk,
//! the Census county reference file and the Census 2024 cartographic boundaries. A source
//! that is not in the store is reported as `missing` in the manifests; the build writes
//! everything else and an explicitly empty artifact where the web app needs a file, never a
//! guessed one. The what-if scenario (#1455) is built in `validate` from the Census 2025
//! population and Gazetteer snapshots and the kindergarten coverage rows, by the pre-registered
//! rule in [`scenario`]: a hypothetical introduction into Gaines County, not a replay of the
//! 2025 outbreak. Its companion `scenarios/gaines-2025.provenance.json` (contract v4) states the
//! seeding as an assumption and cites every parameter. The 4-8 week forecast (#1465) is made in
//! `forecast` from the validated weekly case series by the pre-registered method of
//! `koplik_epi::forecast`; see [`forecast_stage`] for the rule, which series it forecasts and why
//! the rest are `insufficient_data`. Its companion `forecasts/weekly-cases.provenance.json`
//! (contract v7) carries the method, parameter citations, seed, input hash and the backtests'
//! measured skills with their scope.

pub mod forecast_stage;
pub mod nndss_backtest;
pub mod scenario;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Datelike, Utc};
use koplik_contracts::v1::{
    GeoId, Geography, KindergartenMmrCoverage, Provenances, RtEstimate, Sha256Hex, StateFips,
};
use koplik_contracts::v3::WeeklyCaseCount;
use koplik_contracts::{v1, v3, v6, v7};
use koplik_epi::rt::{RtConfig, RtError, case_definitions, estimate_weekly};
use koplik_ingest::census_boundaries::BoundaryKind;
use koplik_ingest::dshs_sources::FetchOutcome;
use koplik_ingest::error::IngestError;
use koplik_ingest::http::UreqClient;
use koplik_ingest::polite::{PoliteConfig, PoliteFetcher, SystemTimekeeper};
use koplik_ingest::source::{SourceSpec, fetch_to_store};
use koplik_ingest::store::{DEFAULT_ROOT, Retrieval, RetrievalMeta, SnapshotStore, sha256_of};
use koplik_ingest::{
    cdc, census_boundaries, census_counties, census_population, coverage, dshs_series,
    dshs_sources, jurisdictions,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_WORK: &str = "data/pipeline";
pub const DEFAULT_OUT: &str = "web/public/data";
pub const DEFAULT_FIXTURES: &str = "data/fixtures";
/// Committed reports the forecast stage reads (the backtest report, `backtest/*.json`).
pub const DEFAULT_REPORTS: &str = "data/reports";
/// Largest response body accepted by live ingest (the CDC measles query is about 1 MB).
const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;
/// Gap between requests to web.archive.org: it rate-limits well below one request a second
/// (HTTP 429), so captures are paced as the ingest CLI paces them (`--interval-secs 8`).
const ARCHIVE_INTERVAL_SECS: u64 = 8;
/// Window of outbreak-page captures to list (`YYYYMMDD`), the ingest CLI's defaults: the
/// outbreak page first appeared in March 2025 and its last update is dated August 12, 2025.
const DSHS_CAPTURES_FROM: &str = "20250301";
const DSHS_CAPTURES_TO: &str = "20250910";
/// Texas, the only state with a county-level case source.
const TEXAS: u8 = 48;
/// Manifest item for the derived DSHS county series (several snapshots feed it).
const DSHS_CASES_ITEM: &str = "texas-dshs-outbreak-cases";
/// The snapshots `dshs_series::build_from_store` reads (every version of the outbreak page
/// and of the data-report PDFs, live or archived).
const DSHS_REPORT_SOURCES: [&str; 4] = [
    dshs_sources::SOURCE_PAGE_LIVE,
    dshs_sources::SOURCE_PAGE_WAYBACK,
    dshs_sources::SOURCE_REPORT_LIVE,
    dshs_sources::SOURCE_REPORT_WAYBACK,
];

/// Names of the artifacts the web app loads from `<out>/v6/` (see `web/README.md`).
pub const WEB_ROW_ARTIFACTS: [&str; 4] = ["geographies", "weekly-cases", "coverage", "rt"];
pub const WEB_BOUNDARY_ARTIFACTS: [&str; 2] = ["us-states", "texas-counties"];
/// The what-if scenario the web app loads from `<out>/scenarios/` (see `web/src/scenario.ts`).
pub const SCENARIO_ARTIFACT: &str = "gaines-2025";
/// The forecast the web app loads from `<out>/forecasts/` (see `web/src/forecast.ts`): v1
/// `Forecast` rows, their v7 provenance companion and the backtest reports the skills were read from.
pub const FORECAST_ARTIFACT: &str = forecast_stage::ARTIFACT;
pub const FORECAST_BACKTEST_ARTIFACT: &str = "backtest-west-texas-2025";
/// The pseudo-real-time NNDSS state-series backtest report published beside it (#1503).
pub const FORECAST_SERIES_BACKTEST_ARTIFACT: &str = "backtest-cdc-states";

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Ingest(#[from] IngestError),
    #[error(transparent)]
    Rt(#[from] RtError),
    #[error(transparent)]
    Forecast(#[from] koplik_epi::forecast::ForecastError),
    #[error("json error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid arguments: {0}")]
    Invalid(String),
    #[error("fixture {path}: {reason}")]
    Fixture { path: PathBuf, reason: String },
    #[error("{0}")]
    Data(String),
}

impl PipelineError {
    fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, PipelineError>;

/// Where snapshots come from. The two modes never share a store by default and nothing ever
/// falls back from one to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// `ingest` fetches from the network (contact required).
    Live,
    /// `ingest` seeds the store from the committed real-byte fixtures (offline).
    Fixtures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Ingest,
    Validate,
    Infer,
    Forecast,
    Build,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Ingest,
        Stage::Validate,
        Stage::Infer,
        Stage::Forecast,
        Stage::Build,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Ingest => "ingest",
            Stage::Validate => "validate",
            Stage::Infer => "infer",
            Stage::Forecast => "forecast",
            Stage::Build => "build",
        }
    }

    pub fn parse(s: &str) -> Option<Stage> {
        Stage::ALL.into_iter().find(|st| st.name() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    /// Snapshot store root.
    pub store: PathBuf,
    /// Stage outputs and manifests.
    pub work: PathBuf,
    /// Web artifacts root (`web/public/data`).
    pub out: PathBuf,
    /// Fixture root for [`Mode::Fixtures`].
    pub fixtures: PathBuf,
    /// Committed reports root (`data/reports`): the forecast stage reads the backtest report.
    pub reports: PathBuf,
}

impl Config {
    /// Live snapshots live in the repository's store; fixture snapshots in a store of their own.
    pub fn default_store(mode: Mode, work: &Path) -> PathBuf {
        match mode {
            Mode::Live => PathBuf::from(DEFAULT_ROOT),
            Mode::Fixtures => work.join("fixture-snapshots"),
        }
    }

    pub fn manifest_path(&self, stage: Stage) -> PathBuf {
        self.work.join(format!("{}.manifest.json", stage.name()))
    }
}

/// A file a stage read or wrote. `path` is relative to the root named by the manifest field
/// that lists it (the store for snapshot blobs, the work directory for stage outputs, the
/// web artifact root for build outputs), so manifests are byte-identical across checkouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileHash {
    pub path: String,
    pub sha256: Sha256Hex,
    pub bytes: u64,
}

/// What a stage found for one source or artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemStatus {
    Present {
        /// The snapshot this item was read from or written as (sources).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retrieval: Option<Retrieval>,
        /// Row count of a row artifact.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rows: Option<u64>,
        /// Rows whose value is an explicit `missing` (gaps), for row artifacts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gaps: Option<u64>,
    },
    /// The input does not exist (not ingested, blocked, or not yet available).
    Missing { reason: String },
    /// Intentionally not produced (no implementation yet, or not applicable to this mode).
    Skipped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub stage: Stage,
    pub mode: Mode,
    /// Files read, hashed as read.
    pub inputs: Vec<FileHash>,
    /// Files written, hashed as written (the manifest itself is not listed).
    pub outputs: Vec<FileHash>,
    /// Per source (ingest, validate) or per artifact (build).
    pub items: BTreeMap<String, ItemStatus>,
    pub notes: Vec<String>,
    /// The build manifest embeds the manifests of the stages it consumed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stages: BTreeMap<String, Manifest>,
}

impl Manifest {
    fn new(stage: Stage, mode: Mode) -> Self {
        Self {
            stage,
            mode,
            inputs: Vec::new(),
            outputs: Vec::new(),
            items: BTreeMap::new(),
            notes: Vec::new(),
            stages: BTreeMap::new(),
        }
    }

    /// One line for the terminal.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for (name, item) in &self.items {
            parts.push(match item {
                ItemStatus::Present {
                    rows: Some(rows),
                    gaps,
                    ..
                } => format!(
                    "{name}: {rows} rows{}",
                    gaps.map(|g| format!(" ({g} missing)")).unwrap_or_default()
                ),
                ItemStatus::Present {
                    retrieval: Some(r), ..
                } => format!("{name}: {} ({} bytes)", r.sha256, r.bytes),
                ItemStatus::Present { .. } => format!("{name}: present"),
                ItemStatus::Missing { reason } => format!("{name}: MISSING ({reason})"),
                ItemStatus::Skipped { reason } => format!("{name}: skipped ({reason})"),
            });
        }
        format!(
            "{} [{}]: {} input(s), {} output(s); {}",
            self.stage.name(),
            match self.mode {
                Mode::Live => "live",
                Mode::Fixtures => "fixtures",
            },
            self.inputs.len(),
            self.outputs.len(),
            parts.join("; ")
        )
    }
}

/// SHA-256 of `bytes` as hex.
fn hash_bytes(bytes: &[u8]) -> Sha256Hex {
    sha256_of(bytes)
}

fn hash_file(root: &Path, rel: &str) -> Result<FileHash> {
    let path = root.join(rel);
    let bytes = fs::read(&path).map_err(|e| PipelineError::io(&path, e))?;
    Ok(FileHash {
        path: rel.to_owned(),
        sha256: hash_bytes(&bytes),
        bytes: bytes.len() as u64,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|e| PipelineError::io(path, e))?;
    serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Serialise compactly with a trailing newline. Collections are sorted by the caller, so the
/// bytes depend only on the data.
fn json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("pipeline artifacts serialise");
    bytes.push(b'\n');
    bytes
}

/// Write `bytes` at `root/rel` (creating parents) and return its hash record.
fn write_file(root: &Path, rel: &str, bytes: &[u8]) -> Result<FileHash> {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| PipelineError::io(dir, e))?;
    }
    fs::write(&path, bytes).map_err(|e| PipelineError::io(&path, e))?;
    Ok(FileHash {
        path: rel.to_owned(),
        sha256: hash_bytes(bytes),
        bytes: bytes.len() as u64,
    })
}

/// Remove a stage's previous output directory so its outputs are a function of this run's
/// inputs alone: a source that disappeared between runs leaves no stale file behind. Only a
/// stage's own directory under the work or out root is ever cleared.
fn clear_dir(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PipelineError::io(path, e)),
    }
}

fn write_manifest(config: &Config, manifest: &Manifest) -> Result<()> {
    let path = config.manifest_path(manifest.stage);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| PipelineError::io(dir, e))?;
    }
    fs::write(&path, json_bytes(manifest)).map_err(|e| PipelineError::io(&path, e))
}

/// Run one stage and write its manifest.
pub fn run_stage(stage: Stage, config: &Config) -> Result<Manifest> {
    let manifest = match stage {
        Stage::Ingest => ingest(config)?,
        Stage::Validate => validate(config)?,
        Stage::Infer => infer(config)?,
        Stage::Forecast => forecast(config)?,
        Stage::Build => build(config)?,
    };
    write_manifest(config, &manifest)?;
    Ok(manifest)
}

/// Relative path of a blob inside the store (`blobs/<2 hex>/<sha256>`).
fn blob_rel(sha: &Sha256Hex) -> String {
    let s = sha.as_str();
    format!("blobs/{}/{s}", &s[..2])
}

// ---------------------------------------------------------------------------------------------
// ingest

/// The single-URL sources the pipeline ingests with one fetch each, in fetch order.
/// `last_year` bounds the CDC case query. The Census county file is the archived copy
/// (`SOURCES.md` explains why not the live host); the DSHS outbreak captures, data-report
/// PDFs and Census boundary ZIPs have their own fetch routines (see `ingest_live`).
pub fn source_specs(last_year: u16) -> Result<Vec<SourceSpec>> {
    Ok(vec![
        cdc::source_spec(cdc::FIRST_YEAR, last_year)?,
        coverage::cdc_source_spec(coverage::FIRST_YEAR, coverage::LAST_YEAR)?,
        coverage::county_source_spec(),
        coverage::texas_source_spec(2023)?,
        coverage::texas_source_spec(2024)?,
        census_counties::wayback_spec(),
        dshs_sources::live_spec(
            dshs_sources::SOURCE_PAGE_LIVE,
            dshs_sources::OUTBREAK_PAGE_URL,
        ),
        dshs_sources::live_spec(
            dshs_sources::SOURCE_REPORT_LIVE,
            dshs_sources::FINAL_REPORT_URL,
        ),
    ])
}

/// Every source id the pipeline reads from the store (and therefore seeds from fixtures).
pub fn known_source_ids() -> Result<BTreeSet<String>> {
    let mut ids: BTreeSet<String> = source_specs(cdc::FIRST_YEAR)?
        .into_iter()
        .map(|s| s.source_id)
        .collect();
    ids.extend(
        [
            census_counties::SOURCE_ID,
            dshs_sources::SOURCE_PAGE_WAYBACK,
            dshs_sources::SOURCE_REPORT_WAYBACK,
            dshs_sources::SOURCE_CDX,
            BoundaryKind::States.source_id(),
            BoundaryKind::TexasCounties.source_id(),
        ]
        .map(str::to_owned),
    );
    for source in census_population::SOURCES {
        ids.insert(census_population::source_spec(source)?.source_id);
    }
    Ok(ids)
}

fn http_client() -> UreqClient {
    UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES)
}

fn ingest(config: &Config) -> Result<Manifest> {
    match config.mode {
        Mode::Live => ingest_live(config),
        Mode::Fixtures => ingest_fixtures(config),
    }
}

fn ingest_live(config: &Config) -> Result<Manifest> {
    // Identify the client before anything else, through the one live entry point (#1427):
    // KOPLIK_CONTACT (or the default) for every host, and for Census hosts KOPLIK_CENSUS_CONTACT,
    // else the gitignored .env.local, else the general contact with a notice. A blank or
    // non-UTF-8 value refuses: no contact, no request, no store, no work dir.
    let cfg = PoliteConfig::live_from_env()?;
    let last_year = u16::try_from(Utc::now().year()).unwrap_or(cdc::FIRST_YEAR);
    let specs = source_specs(last_year.max(cdc::FIRST_YEAR))?;
    let store = SnapshotStore::open(&config.store)?;
    let mut fetcher = PoliteFetcher::new(http_client(), SystemTimekeeper::new(), cfg.clone());
    let mut m = Manifest::new(Stage::Ingest, Mode::Live);
    // One source failing must not lose the others; the manifest says what is missing.
    let missing = |m: &mut Manifest, source_id: &str, e: &dyn std::fmt::Display| {
        eprintln!("ingest: {source_id}: {e}");
        m.items.insert(
            source_id.to_owned(),
            ItemStatus::Missing {
                reason: format!("fetch failed: {e}"),
            },
        );
    };
    for spec in specs {
        match fetch_to_store(&mut fetcher, &store, &spec) {
            Ok((r, _)) => {
                m.outputs
                    .push(hash_file(&config.store, &blob_rel(&r.sha256))?);
                m.items.insert(
                    spec.source_id.clone(),
                    ItemStatus::Present {
                        retrieval: Some(r),
                        rows: None,
                        gaps: None,
                    },
                );
            }
            Err(e) => missing(&mut m, &spec.source_id, &e),
        }
    }
    // Census boundaries: two pinned named files (the connector refuses changed bytes).
    match census_boundaries::fetch(&mut fetcher, &store) {
        Ok(retrievals) => {
            for r in retrievals {
                m.outputs
                    .push(hash_file(&config.store, &blob_rel(&r.sha256))?);
                m.items.insert(
                    r.source_id.clone(),
                    ItemStatus::Present {
                        retrieval: Some(r),
                        rows: None,
                        gaps: None,
                    },
                );
            }
        }
        Err(e) => {
            for kind in [BoundaryKind::States, BoundaryKind::TexasCounties] {
                missing(&mut m, kind.source_id(), &e);
            }
        }
    }
    // Census 2025 population estimates and Gazetteer internal points (#1352): four pinned named
    // files (the connector refuses bytes that differ from the pin).
    for source in census_population::SOURCES {
        let source_id = census_population::source_spec(source)?.source_id;
        match census_population::fetch_source(&mut fetcher, &store, source) {
            Ok((r, _)) => {
                m.outputs
                    .push(hash_file(&config.store, &blob_rel(&r.sha256))?);
                m.items.insert(
                    source_id,
                    ItemStatus::Present {
                        retrieval: Some(r),
                        rows: None,
                        gaps: None,
                    },
                );
            }
            Err(e) => missing(&mut m, &source_id, &e),
        }
    }
    // Internet Archive captures of the DSHS outbreak page and data reports, paced for
    // archive.org; captures already held are not requested again.
    let mut archive = PoliteFetcher::new(
        http_client(),
        SystemTimekeeper::new(),
        PoliteConfig {
            min_interval: Duration::from_secs(ARCHIVE_INTERVAL_SECS),
            max_attempts: 5,
            ..cfg
        },
    );
    let captures = dshs_sources::fetch_outbreak_captures(
        &mut archive,
        &store,
        DSHS_CAPTURES_FROM,
        DSHS_CAPTURES_TO,
    );
    let reports = dshs_sources::fetch_report_documents(&mut archive, &store);
    for (source_id, fetched) in [
        (dshs_sources::SOURCE_PAGE_WAYBACK, captures),
        (dshs_sources::SOURCE_REPORT_WAYBACK, reports),
    ] {
        match fetched {
            Ok(outcomes) => record_outcomes(&mut m, config, &store, source_id, &outcomes)?,
            Err(e) => missing(&mut m, source_id, &e),
        }
    }
    m.outputs.sort_by(|a, b| a.path.cmp(&b.path));
    m.outputs.dedup();
    Ok(m)
}

/// Record a bulk fetch: every snapshot now held for `source_id` (and the CDX listings that
/// named the captures) is an output; each failed capture is a note, never dropped.
fn record_outcomes(
    m: &mut Manifest,
    config: &Config,
    store: &SnapshotStore,
    source_id: &str,
    outcomes: &[(SourceSpec, FetchOutcome)],
) -> Result<()> {
    let mut failed: u64 = 0;
    for (spec, outcome) in outcomes {
        if let FetchOutcome::Failed(e) = outcome {
            failed += 1;
            m.notes.push(format!("{}: fetch failed: {e}", spec.url));
        }
    }
    for id in [source_id, dshs_sources::SOURCE_CDX] {
        let held = store.retrievals(Some(id))?;
        for r in &held {
            m.outputs
                .push(hash_file(&config.store, &blob_rel(&r.sha256))?);
        }
        if let Some(latest) = held.iter().max_by_key(|r| r.retrieved_at) {
            m.items.insert(
                id.to_owned(),
                ItemStatus::Present {
                    retrieval: Some(latest.clone()),
                    rows: Some(held.len() as u64),
                    gaps: if id == source_id { Some(failed) } else { None },
                },
            );
        }
    }
    Ok(())
}

/// Seed the store from the committed fixtures: every `*.retrieval.json` under the fixture root
/// whose source the pipeline knows, with its bytes re-hashed against the record. Nothing is
/// trimmed, edited or invented; the retrieval times are the fixtures' own.
fn ingest_fixtures(config: &Config) -> Result<Manifest> {
    let known = known_source_ids()?;
    let mut records = Vec::new();
    collect_retrievals(&config.fixtures, &mut records)?;
    if records.is_empty() {
        return Err(PipelineError::Fixture {
            path: config.fixtures.clone(),
            reason: "no *.retrieval.json fixtures found".into(),
        });
    }
    let store = SnapshotStore::open(&config.store)?;
    let mut m = Manifest::new(Stage::Ingest, Mode::Fixtures);
    for record_path in records {
        let r: Retrieval = read_json(&record_path)?;
        let rel = record_path
            .strip_prefix(&config.fixtures)
            .unwrap_or(&record_path)
            .to_string_lossy()
            .replace('\\', "/");
        if !known.contains(&r.source_id) {
            m.notes.push(format!(
                "skipped {rel}: source {} is not one the pipeline ingests",
                r.source_id
            ));
            continue;
        }
        let bytes_path = fixture_bytes_path(&record_path)?;
        let bytes = fs::read(&bytes_path).map_err(|e| PipelineError::io(&bytes_path, e))?;
        if hash_bytes(&bytes) != r.sha256 || bytes.len() as u64 != r.bytes {
            return Err(PipelineError::Fixture {
                path: bytes_path,
                reason: format!(
                    "bytes do not match the retrieval record (sha256 {}, {} bytes)",
                    r.sha256, r.bytes
                ),
            });
        }
        m.inputs.push(FileHash {
            path: rel.clone(),
            sha256: hash_bytes(
                &fs::read(&record_path).map_err(|e| PipelineError::io(&record_path, e))?,
            ),
            bytes: fs::metadata(&record_path)
                .map_err(|e| PipelineError::io(&record_path, e))?
                .len(),
        });
        // Idempotent: a record already in the log is not appended again.
        let already = store.retrievals(Some(&r.source_id))?.contains(&r);
        if !already {
            let (stored, _) = store.record(
                RetrievalMeta {
                    source_id: r.source_id.clone(),
                    url: r.url.clone(),
                    retrieved_at: r.retrieved_at,
                    licence_id: r.licence_id.clone(),
                    http_status: r.http_status,
                    content_type: r.content_type.clone(),
                },
                &bytes,
            )?;
            if stored != r {
                return Err(PipelineError::Fixture {
                    path: record_path,
                    reason: "the store recorded a different retrieval than the fixture".into(),
                });
            }
        }
        m.outputs
            .push(hash_file(&config.store, &blob_rel(&r.sha256))?);
        // Several fixtures of one source are all seeded; the manifest lists the latest.
        let latest = store
            .latest(&r.source_id)?
            .expect("just recorded this source");
        m.items.insert(
            r.source_id.clone(),
            ItemStatus::Present {
                retrieval: Some(latest),
                rows: None,
                gaps: None,
            },
        );
    }
    for id in &known {
        m.items
            .entry(id.clone())
            .or_insert_with(|| ItemStatus::Missing {
                reason: "no fixture snapshot for this source".into(),
            });
    }
    m.outputs.sort_by(|a, b| a.path.cmp(&b.path));
    m.outputs.dedup();
    Ok(m)
}

/// Every `*.retrieval.json` under `dir`, in sorted path order.
fn collect_retrievals(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| PipelineError::io(dir, e))?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()
        .map_err(|e| PipelineError::io(dir, e))?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_retrievals(&path, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".retrieval.json"))
        {
            out.push(path);
        }
    }
    Ok(())
}

/// The bytes a retrieval record describes: `<name>.retrieval.json` sits next to `<name>`
/// (`cdc-2023-25.json.retrieval.json` -> `cdc-2023-25.json`), or next to the one file that
/// shares its stem (`nndss-measles-weekly.retrieval.json` -> `nndss-measles-weekly.json`,
/// `cb_2024_us_county_20m.retrieval.json` -> `cb_2024_us_county_20m.zip`). Two candidates
/// are an error, never a guess.
fn fixture_bytes_path(record: &Path) -> Result<PathBuf> {
    let name = record
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".retrieval.json"))
        .ok_or_else(|| PipelineError::Fixture {
            path: record.to_path_buf(),
            reason: "not a *.retrieval.json record".into(),
        })?;
    let dir = record.parent().unwrap_or_else(|| Path::new("."));
    let exact = dir.join(name);
    if exact.is_file() {
        return Ok(exact);
    }
    let prefix = format!("{name}.");
    let mut candidates: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| PipelineError::io(dir, e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix) && !n.ends_with(".retrieval.json"))
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(PipelineError::Fixture {
            path: record.to_path_buf(),
            reason: format!("no fixture bytes named {name} or {name}.* beside the record"),
        }),
        many => Err(PipelineError::Fixture {
            path: record.to_path_buf(),
            reason: format!(
                "{} files share the stem {name}; the record must sit next to exactly one",
                many.len()
            ),
        }),
    }
}

// ---------------------------------------------------------------------------------------------
// validate

/// A parsed source, or the reason it is absent. Any other error (corrupt blob, unparsable
/// bytes, mismatched retrieval) is fatal: a bad source is reported, never skipped.
fn present<T>(r: std::result::Result<T, IngestError>) -> Result<std::result::Result<T, String>> {
    match r {
        Ok(v) => Ok(Ok(v)),
        Err(IngestError::NoSnapshot(id)) => Ok(Err(format!("no snapshot of {id} in the store"))),
        Err(e) => Err(e.into()),
    }
}

fn record_source(
    m: &mut Manifest,
    store_root: &Path,
    store: &SnapshotStore,
    source_id: &str,
    rows: Option<u64>,
    gaps: Option<u64>,
) -> Result<()> {
    let retrieval = store
        .latest(source_id)?
        .expect("a parsed source has a retrieval");
    m.inputs
        .push(hash_file(store_root, &blob_rel(&retrieval.sha256))?);
    m.items.insert(
        source_id.to_owned(),
        ItemStatus::Present {
            retrieval: Some(retrieval),
            rows,
            gaps,
        },
    );
    Ok(())
}

/// Gap report: per source, how many rows are explicitly missing and which geographies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapReport {
    /// source id -> rows with an explicit `missing` value, as `"<geography> <period>: <reason>"`.
    pub gaps: BTreeMap<String, Vec<String>>,
}

fn validate(config: &Config) -> Result<Manifest> {
    let store = SnapshotStore::open(&config.store)?;
    clear_dir(&config.work.join("validate"))?;
    clear_dir(&config.work.join("scenarios"))?;
    let mut m = Manifest::new(Stage::Validate, config.mode);
    let mut report = GapReport::default();

    // Weekly cases by state (CDC, contracts v3 rows with an explicit case definition).
    let mut cases: Vec<WeeklyCaseCount> = Vec::new();
    let mut state_name_provenance: Option<Retrieval> = None;
    match present(cdc::parse_latest(&store))? {
        Ok((retrieval, rows)) => {
            let gaps = case_gaps(&rows);
            record_source(
                &mut m,
                &config.store,
                &store,
                cdc::SOURCE_ID,
                Some(rows.len() as u64),
                Some(gaps.len() as u64),
            )?;
            report.gaps.insert(cdc::SOURCE_ID.to_owned(), gaps);
            state_name_provenance = Some(retrieval);
            cases.extend(rows);
        }
        Err(reason) => {
            m.items
                .insert(cdc::SOURCE_ID.to_owned(), ItemStatus::Missing { reason });
        }
    }

    // Weekly cases by Texas county (DSHS report vintages, v3 rows `confirmed`). County names
    // map to FIPS only through the Census county file, so without it there is no series.
    let texas = StateFips::new(TEXAS).expect("48 is Texas");
    match present(census_counties::CountyLookup::from_store(&store, texas))? {
        Ok(lookup) => {
            m.inputs.push(hash_file(
                &config.store,
                &blob_rel(&lookup.retrieval.sha256),
            )?);
            m.items.insert(
                lookup.retrieval.source_id.clone(),
                ItemStatus::Present {
                    retrieval: Some(lookup.retrieval.clone()),
                    rows: Some(lookup.len() as u64),
                    gaps: None,
                },
            );
            let built = dshs_series::build_from_store(&store, &lookup)?;
            for r in store.retrievals(None)? {
                if DSHS_REPORT_SOURCES.contains(&r.source_id.as_str()) {
                    m.inputs
                        .push(hash_file(&config.store, &blob_rel(&r.sha256))?);
                }
            }
            if built.vintages.is_empty() {
                m.items.insert(
                    DSHS_CASES_ITEM.to_owned(),
                    ItemStatus::Missing {
                        reason: "no DSHS outbreak report snapshots in the store".into(),
                    },
                );
            } else {
                let gaps = case_gaps(&built.series.weekly);
                let with_detail = built
                    .vintages
                    .iter()
                    .filter(|v| v.has_county_detail())
                    .count();
                m.notes.push(format!(
                    "DSHS: {} report vintages ({with_detail} with a county table), {} county names unmapped, {} snapshots failed to parse",
                    built.vintages.len(),
                    built.series.unmapped.len(),
                    built.failures.len()
                ));
                for f in &built.failures {
                    m.notes.push(format!(
                        "DSHS snapshot {} ({}) did not parse: {}",
                        f.sha256, f.url, f.error
                    ));
                }
                m.items.insert(
                    DSHS_CASES_ITEM.to_owned(),
                    ItemStatus::Present {
                        retrieval: None,
                        rows: Some(built.series.weekly.len() as u64),
                        gaps: Some(gaps.len() as u64),
                    },
                );
                report.gaps.insert(DSHS_CASES_ITEM.to_owned(), gaps);
                for (name, bytes) in [
                    ("dshs-vintages", json_bytes(&built.manifest)),
                    ("dshs-cumulative", json_bytes(&built.series.cumulative)),
                    ("dshs-intervals", json_bytes(&built.series.intervals)),
                    ("dshs-unmapped", json_bytes(&built.series.unmapped)),
                    ("dshs-parse-failures", json_bytes(&built.failures)),
                ] {
                    m.outputs.push(write_file(
                        &config.work,
                        &format!("validate/{name}.json"),
                        &bytes,
                    )?);
                }
                cases.extend(built.series.weekly);
            }
        }
        Err(reason) => {
            m.items.insert(
                DSHS_CASES_ITEM.to_owned(),
                ItemStatus::Missing {
                    reason: format!("county FIPS lookup unavailable: {reason}"),
                },
            );
        }
    }
    cases.sort_by(|a, b| (a.geography, a.week).cmp(&(b.geography, b.week)));

    // Kindergarten MMR coverage (contracts v1 rows): CDC states, Texas counties.
    let mut cov: Vec<KindergartenMmrCoverage> = Vec::new();
    let mut coverage_sources: Vec<(
        String,
        std::result::Result<Vec<KindergartenMmrCoverage>, String>,
    )> = vec![(
        coverage::CDC_SOURCE_ID.to_owned(),
        present(coverage::parse_latest_cdc(
            &store,
            coverage::FIRST_YEAR,
            coverage::LAST_YEAR,
        ))?,
    )];
    for year in [2023u16, 2024] {
        coverage_sources.push((
            coverage::texas_source_spec(year)?.source_id,
            present(coverage::parse_latest_texas(&store, year))?,
        ));
    }
    for (source_id, parsed) in coverage_sources {
        match parsed {
            Ok(rows) => {
                let gaps: Vec<String> = coverage::gaps(&rows)
                    .iter()
                    .map(|r| {
                        let reason = match r.coverage {
                            v1::CoverageValue::Missing { reason } => format!("{reason:?}"),
                            v1::CoverageValue::Reported { .. } => unreachable!("gaps are missing"),
                        };
                        format!("{} {}: {reason}", r.geography, r.school_year)
                    })
                    .collect();
                record_source(
                    &mut m,
                    &config.store,
                    &store,
                    &source_id,
                    Some(rows.len() as u64),
                    Some(gaps.len() as u64),
                )?;
                report.gaps.insert(source_id, gaps);
                cov.extend(rows);
            }
            Err(reason) => {
                m.items.insert(source_id, ItemStatus::Missing { reason });
            }
        }
    }
    cov.sort_by(|a, b| (a.geography, a.school_year).cmp(&(b.geography, b.school_year)));

    // Boundaries: Census 2024 cartographic files converted to the GeoJSON the web app reads
    // (`properties.GEOID`, `NAME` and provenance on every feature). Written only when the
    // snapshot exists; the build writes an explicitly empty collection otherwise.
    let mut boundary_names: BTreeMap<GeoId, (String, Retrieval)> = BTreeMap::new();
    for (kind, artifact) in [
        (BoundaryKind::States, "us-states"),
        (BoundaryKind::TexasCounties, "texas-counties"),
    ] {
        match present(
            store.latest(kind.source_id()).and_then(|r| {
                r.ok_or_else(|| IngestError::NoSnapshot(kind.source_id().to_owned()))
            }),
        )? {
            Ok(r) => {
                let bytes = store.get_verified(&r.sha256)?;
                let geojson = census_boundaries::convert(&bytes, &r, kind)?;
                let value: serde_json::Value =
                    serde_json::from_slice(&geojson).map_err(|source| PipelineError::Json {
                        path: config.work.join(format!("validate/{artifact}.json")),
                        source,
                    })?;
                let features = value["features"]
                    .as_array()
                    .ok_or_else(|| PipelineError::Data(format!("{artifact}: no features")))?;
                for f in features {
                    let id: GeoId = serde_json::from_value(f["properties"]["GEOID"].clone())
                        .map_err(|e| PipelineError::Data(format!("{artifact}: GEOID: {e}")))?;
                    let name = f["properties"]["NAME"].as_str().ok_or_else(|| {
                        PipelineError::Data(format!("{artifact}: {id} has no NAME"))
                    })?;
                    boundary_names.insert(id, (name.to_owned(), r.clone()));
                }
                record_source(
                    &mut m,
                    &config.store,
                    &store,
                    kind.source_id(),
                    Some(features.len() as u64),
                    None,
                )?;
                m.outputs.push(write_file(
                    &config.work,
                    &format!("validate/{artifact}.json"),
                    &geojson,
                )?);
                m.items.insert(
                    artifact.to_owned(),
                    ItemStatus::Present {
                        retrieval: None,
                        rows: Some(features.len() as u64),
                        gaps: None,
                    },
                );
            }
            Err(reason) => {
                m.items
                    .insert(kind.source_id().to_owned(), ItemStatus::Missing { reason });
            }
        }
    }

    // Geographies: a display name with provenance for every geography that has an
    // observation or a boundary. Name sources, in order: the Census boundary file's NAME
    // (public domain; counties get the " County" suffix the Census county file uses), the
    // NNDSS name the CDC cases snapshot carries (states), the DSHS county/FIPS crosswalk
    // (Texas counties). A geography with observations but no name source is an error.
    let mut needed: BTreeSet<GeoId> = BTreeSet::new();
    needed.extend(cases.iter().map(|r| r.geography));
    needed.extend(cov.iter().map(|r| r.geography));
    needed.extend(boundary_names.keys().copied());
    let state_names: BTreeMap<StateFips, &'static str> = jurisdictions::state_components()
        .into_iter()
        .map(|(fips, names)| (fips, names[0]))
        .collect();
    let county_names = match present(store.latest(coverage::COUNTY_SOURCE_ID).and_then(|r| {
        r.ok_or_else(|| IngestError::NoSnapshot(coverage::COUNTY_SOURCE_ID.to_owned()))
    }))? {
        Ok(r) => {
            let bytes = store.get_verified(&r.sha256)?;
            let names = coverage::texas_county_names(&bytes, &r)?;
            record_source(
                &mut m,
                &config.store,
                &store,
                coverage::COUNTY_SOURCE_ID,
                Some(names.len() as u64),
                None,
            )?;
            needed.extend(names.keys().map(|f| GeoId::County(*f)));
            Some((r, names))
        }
        Err(reason) => {
            m.items.insert(
                coverage::COUNTY_SOURCE_ID.to_owned(),
                ItemStatus::Missing { reason },
            );
            None
        }
    };
    let mut geographies: Vec<Geography> = Vec::new();
    for id in needed {
        let named = match (boundary_names.get(&id), id) {
            (Some((name, r)), GeoId::State(_)) => Some((name.clone(), r.provenance())),
            (Some((name, r)), GeoId::County(_)) => Some((format!("{name} County"), r.provenance())),
            (None, GeoId::State(fips)) => match (&state_name_provenance, state_names.get(&fips)) {
                (Some(r), Some(name)) => Some(((*name).to_owned(), r.provenance())),
                _ => None,
            },
            (None, GeoId::County(fips)) => match &county_names {
                // DSHS lists bare names ("Gaines"); the display convention adds the level.
                Some((r, names)) if names.contains_key(&fips) => {
                    Some((format!("{} County", names[&fips]), r.provenance()))
                }
                _ => None,
            },
        };
        let (name, provenance) = named.ok_or_else(|| {
            PipelineError::Data(format!(
                "geography {id} has observations but no named source snapshot"
            ))
        })?;
        geographies.push(
            Geography::new(id, name, Provenances::one(provenance)).map_err(PipelineError::Data)?,
        );
    }
    geographies.sort_by_key(|g| g.id);

    // Census 2025 population estimates and Gazetteer internal points (#1352): absent snapshots
    // are reported missing; a present one that does not parse is an error.
    let county_population = census_snapshot(
        &mut m,
        config,
        &store,
        census_population::COUNTY_SOURCE_ID,
        || census_population::parse_latest_populations(&store, false),
    )?;
    let county_gazetteer = census_snapshot(
        &mut m,
        config,
        &store,
        census_population::COUNTY_GEO_SOURCE_ID,
        || census_population::parse_latest_geographies(&store, false),
    )?;
    let state_gazetteer = census_snapshot(
        &mut m,
        config,
        &store,
        census_population::STATE_GEO_SOURCE_ID,
        || census_population::parse_latest_geographies(&store, true),
    )?;
    // Geography.centroid is the Gazetteer internal point (a representative point, not a
    // population-weighted or geometric centroid); where the Gazetteer has none it stays null,
    // and the Gazetteer snapshot is added to the geography's source records.
    let internal_points: BTreeMap<GeoId, &Geography> = county_gazetteer
        .iter()
        .chain(state_gazetteer.iter())
        .flat_map(|(_, rows)| rows.iter())
        .filter(|g| g.centroid.is_some())
        .map(|g| (g.id, g))
        .collect();
    for g in &mut geographies {
        if let Some(point) = internal_points.get(&g.id) {
            let mut records = g.provenance.as_slice().to_vec();
            for p in point.provenance.as_slice() {
                if !records.contains(p) {
                    records.push(p.clone());
                }
            }
            g.centroid = point.centroid;
            g.provenance =
                Provenances::new(records).map_err(|e| PipelineError::Data(e.to_string()))?;
        }
    }

    // The what-if scenario (#1455): a hypothetical introduction into Gaines County, built by the
    // pre-registered rule in `scenario` from the Census population and Gazetteer snapshots and
    // the kindergarten coverage rows. The DSHS report vintages are not an input.
    let scenario_item = match (&county_population, &county_gazetteer) {
        (Some((_, populations)), Some((_, gazetteer))) => {
            let sources = scenario::Sources {
                populations,
                gazetteer,
                coverage: &cov,
            };
            match scenario::build(&scenario::ScenarioConfig::default(), &sources) {
                Ok(built) => {
                    for (rel, bytes) in [
                        (
                            format!("scenarios/{SCENARIO_ARTIFACT}.json"),
                            json_bytes(&built.input),
                        ),
                        (
                            format!("scenarios/{SCENARIO_ARTIFACT}.provenance.json"),
                            json_bytes(&built.provenance),
                        ),
                    ] {
                        m.outputs.push(write_file(&config.work, &rel, &bytes)?);
                    }
                    ItemStatus::Present {
                        retrieval: None,
                        rows: Some(built.input.nodes.len() as u64),
                        gaps: Some(built.provenance.excluded_nodes.len() as u64),
                    }
                }
                Err(scenario::ScenarioError::Missing(reason)) => ItemStatus::Missing { reason },
                Err(scenario::ScenarioError::Invalid(reason)) => {
                    return Err(PipelineError::Data(format!("what-if scenario: {reason}")));
                }
            }
        }
        _ => ItemStatus::Missing {
            reason: "the scenario needs the Census county population and the Census county Gazetteer in the store".into(),
        },
    };
    m.items.insert(SCENARIO_ARTIFACT.to_owned(), scenario_item);

    for (name, bytes, rows) in [
        ("weekly-cases", json_bytes(&cases), cases.len()),
        ("coverage", json_bytes(&cov), cov.len()),
        ("geographies", json_bytes(&geographies), geographies.len()),
    ] {
        let rel = format!("validate/{name}.json");
        m.outputs.push(write_file(&config.work, &rel, &bytes)?);
        m.items.insert(
            name.to_owned(),
            ItemStatus::Present {
                retrieval: None,
                rows: Some(rows as u64),
                gaps: None,
            },
        );
    }
    m.outputs.push(write_file(
        &config.work,
        "validate/gaps.json",
        &json_bytes(&report),
    )?);
    m.inputs.sort_by(|a, b| a.path.cmp(&b.path));
    m.inputs.dedup();
    m.outputs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(m)
}

/// Parse the latest snapshot of a Census population or Gazetteer source and record it as a
/// validate input; `None` (and a `missing` item) when the store holds none. A snapshot that is
/// there but does not parse is an error, never skipped.
fn census_snapshot<T>(
    m: &mut Manifest,
    config: &Config,
    store: &SnapshotStore,
    source_id: &str,
    parse: impl FnOnce() -> std::result::Result<(Retrieval, Vec<T>), IngestError>,
) -> Result<Option<(Retrieval, Vec<T>)>> {
    if store.latest(source_id)?.is_none() {
        m.items.insert(
            source_id.to_owned(),
            ItemStatus::Missing {
                reason: format!("no snapshot of {source_id} in the store"),
            },
        );
        return Ok(None);
    }
    let (retrieval, rows) = parse()?;
    record_source(
        m,
        &config.store,
        store,
        source_id,
        Some(rows.len() as u64),
        None,
    )?;
    Ok(Some((retrieval, rows)))
}

/// Rows whose count is an explicit `missing`, as `"<geography> <week>: <reason>"`.
fn case_gaps(rows: &[WeeklyCaseCount]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| match r.cases {
            v3::CaseCount::Missing { reason } => {
                Some(format!("{} {}: {reason:?}", r.geography, r.week))
            }
            v3::CaseCount::Reported { .. } => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// infer

/// R_t configuration: `koplik_epi::rt::RtConfig::default()` (every default cites its source
/// there: Cori et al. 2013 / EpiEstim prior and threshold, measles serial interval, two
/// provisional weeks).
pub fn rt_config() -> RtConfig {
    RtConfig::default()
}

fn infer(config: &Config) -> Result<Manifest> {
    clear_dir(&config.work.join("infer"))?;
    let mut m = Manifest::new(Stage::Infer, config.mode);
    let rel = "validate/weekly-cases.json";
    let path = config.work.join(rel);
    if !path.is_file() {
        return Err(PipelineError::Data(format!(
            "{} not found: run the validate stage first",
            path.display()
        )));
    }
    m.inputs.push(hash_file(&config.work, rel)?);
    let cases: Vec<WeeklyCaseCount> = read_json(&path)?;
    // R_t inherits the case definition of its input rows (koplik_epi::rt::weekly); the v1
    // RtEstimate row has no field for it, so the manifest records it per definition.
    let definitions = case_definitions(&cases)?;
    let mut by_definition: BTreeMap<String, u64> = BTreeMap::new();
    for d in definitions.values() {
        *by_definition
            .entry(
                serde_json::to_value(d)
                    .expect("enum serialises")
                    .as_str()
                    .expect("string")
                    .to_owned(),
            )
            .or_default() += 1;
    }
    for (definition, geographies) in &by_definition {
        m.notes.push(format!(
            "R_t for {geographies} geographies describes cases under definition {definition}"
        ));
    }
    let rt: Vec<RtEstimate> = estimate_weekly(&cases, &rt_config())?;
    let insufficient = rt
        .iter()
        .filter(|r| r.status == v1::RtStatus::InsufficientData)
        .count();
    m.outputs
        .push(write_file(&config.work, "infer/rt.json", &json_bytes(&rt))?);
    m.items.insert(
        "rt".to_owned(),
        ItemStatus::Present {
            retrieval: None,
            rows: Some(rt.len() as u64),
            gaps: Some(insufficient as u64),
        },
    );
    if cases.is_empty() {
        m.notes
            .push("no weekly case rows: R_t has nothing to estimate".into());
    }
    Ok(m)
}

// ---------------------------------------------------------------------------------------------
// forecast

/// Work-directory paths of the forecast stage's outputs.
const FORECAST_ROWS_REL: &str = "forecast/forecast.json";
/// The forecasts the publication policy withheld, kept for audit in the work directory. `build`
/// never copies it: a withheld forecast is never published.
const FORECAST_WITHHELD_REL: &str = "forecast/withheld.json";
const FORECAST_PROVENANCE_REL: &str = "forecast/forecast.provenance.json";
const FORECAST_BACKTEST_REL: &str = "forecast/backtest-west-texas-2025.json";
const FORECAST_SERIES_BACKTEST_REL: &str = "forecast/backtest-cdc-states.json";

fn forecast(config: &Config) -> Result<Manifest> {
    clear_dir(&config.work.join("forecast"))?;
    let mut m = Manifest::new(Stage::Forecast, config.mode);
    let rel = "validate/weekly-cases.json";
    let path = config.work.join(rel);
    if !path.is_file() {
        return Err(PipelineError::Data(format!(
            "{} not found: run the validate stage first",
            path.display()
        )));
    }
    m.inputs.push(hash_file(&config.work, rel)?);
    let bytes = fs::read(&path).map_err(|e| PipelineError::io(&path, e))?;
    let cases: Vec<WeeklyCaseCount> =
        serde_json::from_slice(&bytes).map_err(|source| PipelineError::Json {
            path: path.clone(),
            source,
        })?;

    // The measured skill is attached only from the committed report run with exactly this
    // configuration; otherwise the companion says there is none.
    let report_path = config.reports.join(forecast_stage::BACKTEST_REPORT_REL);
    let report = match fs::read(&report_path) {
        Ok(report) => Some(report),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(PipelineError::io(&report_path, e)),
    };
    let skill = match &report {
        None => {
            m.notes.push(format!(
                "no backtest skill attached: {} is not present",
                forecast_stage::BACKTEST_REPORT_PATH
            ));
            None
        }
        Some(report) => match forecast_stage::skill_from_report(report, &forecast_stage::forecast_config()) {
            Ok(skill) => Some(skill),
            Err(why) => {
                m.notes
                    .push(format!("no backtest skill attached: {why}"));
                None
            }
        },
    };

    // The pseudo-real-time backtest of the CDC NNDSS state series (#1503), attached the same way:
    // only from the committed report run with exactly this configuration and carrying its label.
    let series_report_path = config.reports.join(forecast_stage::SERIES_BACKTEST_REPORT_REL);
    let series_report = match fs::read(&series_report_path) {
        Ok(report) => Some(report),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(PipelineError::io(&series_report_path, e)),
    };
    let series_backtest = match &series_report {
        None => {
            m.notes.push(format!(
                "no series backtest attached: {} is not present",
                forecast_stage::SERIES_BACKTEST_REPORT_PATH
            ));
            None
        }
        Some(report) => match forecast_stage::series_backtest_from_report(
            report,
            &forecast_stage::forecast_config(),
            rt_config().provisional_weeks,
        ) {
            Ok(b) => Some(b),
            Err(why) => {
                m.notes
                    .push(format!("no series backtest attached: {why}"));
                None
            }
        },
    };

    match forecast_stage::build(
        &cases,
        forecast_stage::input_of(&bytes, cases.len()),
        skill,
        series_backtest,
    )? {
        None => {
            m.items.insert(
                "forecast".to_owned(),
                ItemStatus::Missing {
                    reason: "validate wrote no weekly case rows: there is nothing to forecast"
                        .into(),
                },
            );
        }
        Some(built) => {
            let p = &built.provenance;
            let insufficient = p
                .series
                .iter()
                .filter(|s| s.status == v7::ForecastStatus::InsufficientData)
                .count();
            let withheld = p
                .series
                .iter()
                .filter(|s| s.status == v7::ForecastStatus::Withheld)
                .count();
            m.notes.push(format!(
                "origin MMWR {} (latest week with data {}); seed {}; {} series published, {} withheld by the publication policy, {} insufficient data; the pre-registered method and defaults of koplik_epi::forecast, unchanged",
                p.origin_week,
                p.latest_data_week,
                p.seed,
                p.series.len() - insufficient - withheld,
                withheld,
                insufficient
            ));
            m.outputs.push(write_file(
                &config.work,
                FORECAST_ROWS_REL,
                &json_bytes(&built.rows),
            )?);
            // The withheld forecasts stay in the work directory for audit; `build` never reads or copies them.
            m.outputs.push(write_file(
                &config.work,
                FORECAST_WITHHELD_REL,
                &json_bytes(&built.withheld_rows),
            )?);
            m.outputs.push(write_file(
                &config.work,
                FORECAST_PROVENANCE_REL,
                &json_bytes(&built.provenance),
            )?);
            if let (Some(skill), Some(report)) = (&p.backtest, &report) {
                // The report the skill was read from, byte for byte, so the published numbers
                // can be checked against it (`skill.report_sha256`).
                let copy = write_file(&config.work, FORECAST_BACKTEST_REL, report)?;
                if copy.sha256 != skill.report_sha256 {
                    return Err(PipelineError::Data(
                        "the copied backtest report does not match the hash its skill cites".into(),
                    ));
                }
                m.outputs.push(copy);
                m.notes.push(format!(
                    "backtest skill read from {} (sha256 {})",
                    skill.report_path, skill.report_sha256
                ));
            }
            if let (Some(b), Some(report)) = (&p.series_backtest, &series_report) {
                // The series backtest's report, byte for byte, so every published number can be
                // checked against it (`b.report_sha256`).
                let copy = write_file(&config.work, FORECAST_SERIES_BACKTEST_REL, report)?;
                if copy.sha256 != b.report_sha256 {
                    return Err(PipelineError::Data(
                        "the copied series backtest report does not match the hash it cites".into(),
                    ));
                }
                m.outputs.push(copy);
                m.notes.push(format!(
                    "series backtest read from {} (sha256 {}); {} of {} series have a measured skill",
                    b.report_path,
                    b.report_sha256,
                    p.series.iter().filter(|s| s.skill == v7::SeriesSkill::Measured).count(),
                    p.series.len()
                ));
            }
            m.items.insert(
                "forecast".to_owned(),
                ItemStatus::Present {
                    retrieval: None,
                    rows: Some(built.rows.len() as u64),
                    gaps: Some(insufficient as u64),
                },
            );
        }
    }
    m.inputs.sort_by(|a, b| a.path.cmp(&b.path));
    m.outputs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(m)
}

// ---------------------------------------------------------------------------------------------
// build

/// An explicitly empty GeoJSON FeatureCollection: the web app requires the file and shows its
/// "unavailable" state for an empty one. Never a drawn or guessed boundary.
fn empty_feature_collection() -> serde_json::Value {
    serde_json::json!({"type": "FeatureCollection", "features": []})
}

fn build(config: &Config) -> Result<Manifest> {
    let mut m = Manifest::new(Stage::Build, config.mode);
    // Earlier manifests, when this work directory has them: embedded, and hashed as inputs so
    // the build manifest changes whenever a consumed manifest does.
    for stage in [
        Stage::Ingest,
        Stage::Validate,
        Stage::Infer,
        Stage::Forecast,
    ] {
        let path = config.manifest_path(stage);
        if path.is_file() {
            let rel = format!("{}.manifest.json", stage.name());
            m.inputs.push(hash_file(&config.work, &rel)?);
            m.stages
                .insert(stage.name().to_owned(), read_json::<Manifest>(&path)?);
        } else {
            m.notes.push(format!(
                "no {} manifest in {}",
                stage.name(),
                config.work.display()
            ));
        }
    }
    // The output tree is a function of the inputs: clear what a previous build wrote.
    for rel in ["v1", "v6", "scenarios", "forecasts", "manifest.json"] {
        let path = config.out.join(rel);
        let removed = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else if path.exists() {
            fs::remove_file(&path)
        } else {
            Ok(())
        };
        removed.map_err(|e| PipelineError::io(&path, e))?;
    }
    fs::create_dir_all(&config.out).map_err(|e| PipelineError::io(&config.out, e))?;

    // Row artifacts, re-read as their contract types so a malformed stage output cannot be
    // published, and written in the layout the web loader reads.
    for name in WEB_ROW_ARTIFACTS {
        let (rel, rows) = match name {
            "rt" => (
                "infer/rt.json".to_owned(),
                row_artifact::<RtEstimate>(config, "infer/rt.json")?,
            ),
            "weekly-cases" => (
                "validate/weekly-cases.json".to_owned(),
                row_artifact::<WeeklyCaseCount>(config, "validate/weekly-cases.json")?,
            ),
            "coverage" => (
                "validate/coverage.json".to_owned(),
                row_artifact::<KindergartenMmrCoverage>(config, "validate/coverage.json")?,
            ),
            _ => (
                "validate/geographies.json".to_owned(),
                row_artifact::<Geography>(config, "validate/geographies.json")?,
            ),
        };
        match rows {
            Some((rows, bytes)) => {
                let input = hash_file(&config.work, &rel)?;
                m.inputs.push(input);
                m.outputs
                    .push(write_file(&config.out, &format!("v6/{name}.json"), &bytes)?);
                m.items.insert(
                    name.to_owned(),
                    ItemStatus::Present {
                        retrieval: None,
                        rows: Some(rows),
                        gaps: None,
                    },
                );
            }
            None => {
                return Err(PipelineError::Data(format!(
                    "{} not found: run the earlier stages first",
                    config.work.join(&rel).display()
                )));
            }
        }
    }

    // Boundaries: validate writes them when the Census snapshots are in the store; otherwise
    // an explicitly empty collection keeps the site loadable and the manifest says why.
    for name in WEB_BOUNDARY_ARTIFACTS {
        let rel = format!("validate/{name}.json");
        let path = config.work.join(&rel);
        if path.is_file() {
            let value: serde_json::Value = read_json(&path)?;
            if value.get("type").and_then(|t| t.as_str()) != Some("FeatureCollection") {
                return Err(PipelineError::Data(format!(
                    "{} is not a GeoJSON FeatureCollection",
                    path.display()
                )));
            }
            m.inputs.push(hash_file(&config.work, &rel)?);
            let bytes = fs::read(&path).map_err(|e| PipelineError::io(&path, e))?;
            m.outputs
                .push(write_file(&config.out, &format!("v6/{name}.json"), &bytes)?);
            m.items.insert(
                name.to_owned(),
                ItemStatus::Present {
                    retrieval: None,
                    rows: value
                        .get("features")
                        .and_then(|f| f.as_array())
                        .map(|f| f.len() as u64),
                    gaps: None,
                },
            );
        } else {
            m.outputs.push(write_file(
                &config.out,
                &format!("v6/{name}.json"),
                &json_bytes(&empty_feature_collection()),
            )?);
            m.items.insert(
                name.to_owned(),
                ItemStatus::Missing {
                    reason: "no Census cartographic boundary snapshot in the store; an empty FeatureCollection is written so the site loads".into(),
                },
            );
        }
    }
    // The what-if scenario (#1455): validate writes it with its provenance companion (#1400).
    // A scenario is only published together with a companion that agrees with it.
    let scenario_rel = format!("scenarios/{SCENARIO_ARTIFACT}.json");
    let provenance_rel = format!("scenarios/{SCENARIO_ARTIFACT}.provenance.json");
    let scenario_path = config.work.join(&scenario_rel);
    if scenario_path.is_file() {
        // Deserialising a ScenarioInput runs the contract's own validation.
        let scenario: v1::ScenarioInput = read_json(&scenario_path)?;
        let provenance_path = config.work.join(&provenance_rel);
        if !provenance_path.is_file() {
            return Err(PipelineError::Data(format!(
                "{} exists without its provenance companion {}; a scenario is never published without one",
                scenario_path.display(),
                provenance_path.display()
            )));
        }
        let provenance: koplik_contracts::v4::ScenarioProvenance = read_json(&provenance_path)?;
        provenance
            .check_against(&scenario)
            .map_err(|e| PipelineError::Data(format!("{}: {e}", provenance_path.display())))?;
        for rel in [&scenario_rel, &provenance_rel] {
            m.inputs.push(hash_file(&config.work, rel)?);
            let path = config.work.join(rel);
            let bytes = fs::read(&path).map_err(|e| PipelineError::io(&path, e))?;
            m.outputs.push(write_file(&config.out, rel, &bytes)?);
        }
        m.items.insert(
            SCENARIO_ARTIFACT.to_owned(),
            ItemStatus::Present {
                retrieval: None,
                rows: Some(scenario.nodes.len() as u64),
                gaps: Some(provenance.excluded_nodes.len() as u64),
            },
        );
    } else {
        m.items.insert(
            SCENARIO_ARTIFACT.to_owned(),
            ItemStatus::Missing {
                reason: "validate wrote no scenario (an input is missing from the store, see its manifest); nothing is published and the what-if panel shows unavailable".into(),
            },
        );
    }

    // The forecast (#1465): rows are only published beside a companion that describes them, and
    // the skill beside them only with the report it was read from.
    let rows_path = config.work.join(FORECAST_ROWS_REL);
    if rows_path.is_file() {
        // Deserialising runs the contract's own validation of every row and of the companion.
        let rows: Vec<v1::Forecast> = read_json(&rows_path)?;
        let provenance_path = config.work.join(FORECAST_PROVENANCE_REL);
        if !provenance_path.is_file() {
            return Err(PipelineError::Data(format!(
                "{} exists without its provenance companion {}; a forecast is never published without one",
                rows_path.display(),
                provenance_path.display()
            )));
        }
        let provenance: v7::ForecastProvenance = read_json(&provenance_path)?;
        provenance
            .check_against(&rows)
            .map_err(|e| PipelineError::Data(format!("{}: {e}", provenance_path.display())))?;
        let mut files = vec![
            (FORECAST_ROWS_REL, format!("forecasts/{FORECAST_ARTIFACT}.json")),
            (
                FORECAST_PROVENANCE_REL,
                format!("forecasts/{FORECAST_ARTIFACT}.provenance.json"),
            ),
        ];
        if let Some(skill) = &provenance.backtest {
            let report_path = config.work.join(FORECAST_BACKTEST_REL);
            let report = fs::read(&report_path).map_err(|e| PipelineError::io(&report_path, e))?;
            if hash_bytes(&report) != skill.report_sha256 {
                return Err(PipelineError::Data(format!(
                    "{} does not match the report hash its skill cites",
                    report_path.display()
                )));
            }
            files.push((
                FORECAST_BACKTEST_REL,
                format!("forecasts/{FORECAST_BACKTEST_ARTIFACT}.json"),
            ));
        }
        if let Some(b) = &provenance.series_backtest {
            let report_path = config.work.join(FORECAST_SERIES_BACKTEST_REL);
            let report = fs::read(&report_path).map_err(|e| PipelineError::io(&report_path, e))?;
            if hash_bytes(&report) != b.report_sha256 {
                return Err(PipelineError::Data(format!(
                    "{} does not match the report hash its series backtest cites",
                    report_path.display()
                )));
            }
            files.push((
                FORECAST_SERIES_BACKTEST_REL,
                format!("forecasts/{FORECAST_SERIES_BACKTEST_ARTIFACT}.json"),
            ));
        }
        for (from, to) in files {
            m.inputs.push(hash_file(&config.work, from)?);
            let path = config.work.join(from);
            let bytes = if from == FORECAST_ROWS_REL {
                pack_rows::<v1::Forecast>(&path)?
            } else {
                fs::read(&path).map_err(|e| PipelineError::io(&path, e))?
            };
            m.outputs.push(write_file(&config.out, &to, &bytes)?);
        }
        m.items.insert(
            "forecast".to_owned(),
            ItemStatus::Present {
                retrieval: None,
                rows: Some(rows.len() as u64),
                gaps: Some(
                    provenance
                        .series
                        .iter()
                        .filter(|s| s.status == v7::ForecastStatus::InsufficientData)
                        .count() as u64,
                ),
            },
        );
    } else {
        m.items.insert(
            "forecast".to_owned(),
            ItemStatus::Missing {
                reason: "the forecast stage wrote no forecast (see its manifest); nothing is published and the page shows the forecast unavailable".into(),
            },
        );
    }

    // The published manifest (the same bytes as the work copy written by run_stage).
    let path = config.out.join("manifest.json");
    fs::write(&path, json_bytes(&m)).map_err(|e| PipelineError::io(&path, e))?;
    Ok(m)
}

/// Pack a stage output into a v6 artifact, or `None` when it does not exist. Rows are re-parsed as
/// their contract type so every published row is a valid, provenance-carrying contract row.
fn row_artifact<T: v6::PublicationRow>(config: &Config, rel: &str) -> Result<Option<(u64, Vec<u8>)>> {
    let path = config.work.join(rel);
    if !path.is_file() {
        return Ok(None);
    }
    let rows: Vec<T> = read_json(&path)?;
    Ok(Some((rows.len() as u64, pack_rows::<T>(&path)?)))
}

/// Preserve source JSON numbers exactly while changing only provenance storage.
fn pack_rows<T: v6::PublicationRow>(path: &Path) -> Result<Vec<u8>> {
    let bytes = fs::read(path).map_err(|e| PipelineError::io(path, e))?;
    v6::pack_json::<T>(&bytes).map_err(|source| PipelineError::Json {
        path: path.to_path_buf(),
        source,
    })
}
