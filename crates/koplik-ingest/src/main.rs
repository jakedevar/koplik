//! `koplik-ingest`: the pipeline's entry point to the network and the snapshot store.
//!
//! ```text
//! koplik-ingest fetch cdc-cases [--store DIR] [--first-year Y] [--last-year Y]
//! koplik-ingest parse cdc-cases [--store DIR] [--out FILE]
//! koplik-ingest fetch census-boundaries [--store DIR]
//! koplik-ingest parse census-boundaries [--store DIR] --out DIR
//! koplik-ingest list [--store DIR] [--source ID]
//! ```
//!
//! `fetch` is the only command that uses the network, and it refuses to run unless
//! `KOPLIK_CONTACT` holds a verified contact (an e-mail address or repository URL) that is
//! sent in the User-Agent; nothing is ever invented. `parse` and `list` are offline and need no
//! contact: `parse` reads the latest stored CDC snapshot (re-verifying its SHA-256) and writes
//! contracts v3 weekly-case rows (`cases` + `case_definition`, each with a v1 `Provenance`)
//! as JSON (stdout, or `--out`). Census parsing writes both GeoJSON files to `--out DIR`.

use std::process::ExitCode;
use std::time::Duration;

use chrono::{Datelike, Utc};
use koplik_ingest::cdc;
use koplik_ingest::census_boundaries;
use koplik_ingest::census_counties;
use koplik_ingest::coverage;
use koplik_ingest::dshs_series;
use koplik_ingest::dshs_sources::{self, FetchOutcome};
use koplik_ingest::error::{IngestError, Result};
use koplik_ingest::http::UreqClient;
use koplik_ingest::polite::{PoliteConfig, PoliteFetcher, SystemTimekeeper, contact_from_env};
use koplik_ingest::source::{SourceSpec, fetch_to_store};
use koplik_ingest::store::{DEFAULT_ROOT, PutOutcome, SnapshotStore};

const USAGE: &str = "usage:
  koplik-ingest fetch cdc-cases [--store DIR] [--first-year Y] [--last-year Y]
  koplik-ingest parse cdc-cases [--store DIR] [--out FILE]
  koplik-ingest fetch census-boundaries [--store DIR]
  koplik-ingest parse census-boundaries [--store DIR] --out DIR
  koplik-ingest fetch census-counties [--store DIR] [--direct 1]
  koplik-ingest fetch dshs-live [--store DIR]
  koplik-ingest fetch dshs-wayback [--store DIR] [--from YYYYMMDD] [--to YYYYMMDD] [--interval-secs N]
  koplik-ingest fetch dshs-reports [--store DIR] [--interval-secs N]
  koplik-ingest parse dshs-cases [--store DIR] [--out DIR]
  koplik-ingest fetch cdc-coverage [--store DIR] [--first-year Y] [--last-year Y]
  koplik-ingest parse cdc-coverage [--store DIR] [--first-year Y] [--last-year Y] [--out FILE] [--gaps FILE]
  koplik-ingest fetch texas-coverage [--store DIR] [--year Y]
  koplik-ingest parse texas-coverage [--store DIR] [--year Y] [--out FILE] [--gaps FILE]
  koplik-ingest list [--store DIR] [--source ID]";

/// Largest response body accepted (the CDC measles query is about 1 MB).
const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;

struct Flags(Vec<String>);

impl Flags {
    fn take(&mut self, name: &str) -> Result<Option<String>> {
        match self.0.iter().position(|a| a == name) {
            None => Ok(None),
            Some(i) => {
                if i + 1 >= self.0.len() {
                    return Err(IngestError::Invalid(format!("{name} needs a value")));
                }
                let v = self.0.remove(i + 1);
                self.0.remove(i);
                Ok(Some(v))
            }
        }
    }
    fn done(&self) -> Result<()> {
        match self.0.first() {
            None => Ok(()),
            Some(a) => Err(IngestError::Invalid(format!("unexpected argument {a:?}"))),
        }
    }
}

fn year(flag: Option<String>, default: u16) -> Result<u16> {
    match flag {
        None => Ok(default),
        Some(s) => s
            .parse()
            .map_err(|_| IngestError::Invalid(format!("bad year {s:?}"))),
    }
}

/// A live fetcher: refuses without a verified contact (before any request), then paces
/// requests to one host at `interval_secs` (the Internet Archive rate-limits hard).
fn live_fetcher(interval_secs: u64) -> Result<PoliteFetcher<UreqClient, SystemTimekeeper>> {
    let cfg = PoliteConfig {
        min_interval: Duration::from_secs(interval_secs.max(1)),
        max_attempts: 5,
        ..PoliteConfig::live(contact_from_env().as_deref())?
    };
    let client = UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES);
    Ok(PoliteFetcher::new(client, SystemTimekeeper::new(), cfg))
}

/// One line per URL; a non-zero exit when any fetch failed (failures are never dropped).
fn report_outcomes(results: &[(SourceSpec, FetchOutcome)]) -> Result<()> {
    let mut failed = 0;
    for (spec, outcome) in results {
        match outcome {
            FetchOutcome::Skipped(r) => eprintln!("have    {} {}", r.sha256, spec.url),
            FetchOutcome::Fetched(r, _) => {
                eprintln!("fetched {} {}", r.sha256, spec.url);
                println!(
                    "{}",
                    serde_json::to_string(r).expect("Retrieval serialises")
                );
            }
            FetchOutcome::Failed(e) => {
                failed += 1;
                eprintln!("FAILED  {} {e}", spec.url);
            }
        }
    }
    if failed > 0 {
        return Err(IngestError::Http(format!(
            "{failed} of {} fetches failed",
            results.len()
        )));
    }
    Ok(())
}

fn to_pretty<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).expect("serialises")
}

fn run(args: Vec<String>) -> Result<()> {
    let mut args = args.into_iter();
    let cmd = args
        .next()
        .ok_or_else(|| IngestError::Invalid("missing command".into()))?;
    let source = match cmd.as_str() {
        "fetch" | "parse" => Some(args.next().ok_or_else(|| {
            IngestError::Invalid("missing source (cdc-cases, dshs-cases, ...)".into())
        })?),
        _ => None,
    };
    let mut flags = Flags(args.collect());
    let store_dir = flags
        .take("--store")?
        .unwrap_or_else(|| DEFAULT_ROOT.to_owned());
    match (cmd.as_str(), source.as_deref()) {
        ("fetch" | "parse", Some(src @ ("cdc-coverage" | "texas-coverage"))) => {
            let (first, last) = if src == "cdc-coverage" {
                (
                    year(flags.take("--first-year")?, coverage::FIRST_YEAR)?,
                    year(flags.take("--last-year")?, coverage::LAST_YEAR)?,
                )
            } else {
                let y = year(flags.take("--year")?, coverage::TEXAS_BASELINE_YEAR)?;
                (y, y)
            };
            let spec = if src == "cdc-coverage" {
                coverage::cdc_source_spec(first, last)?
            } else {
                coverage::texas_source_spec(first)?
            };
            if cmd == "fetch" {
                flags.done()?;
                // Match the reviewed CDC fetch path: no contact, no request or store.
                let cfg = PoliteConfig::live(contact_from_env().as_deref())?;
                let store = SnapshotStore::open(&store_dir)?;
                let mut fetcher = PoliteFetcher::new(
                    UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES),
                    SystemTimekeeper::new(),
                    cfg,
                );
                let specs = if src == "texas-coverage" {
                    vec![coverage::county_source_spec(), spec]
                } else {
                    vec![spec]
                };
                for spec in specs {
                    let (r, _) = fetch_to_store(&mut fetcher, &store, &spec)?;
                    eprintln!(
                        "stored {}: sha256 {} ({} bytes)",
                        r.source_id, r.sha256, r.bytes
                    );
                    println!(
                        "{}",
                        serde_json::to_string(&r).expect("retrieval serialises")
                    );
                }
                Ok(())
            } else {
                let out = flags.take("--out")?;
                let gaps = flags.take("--gaps")?;
                flags.done()?;
                if out.is_some() && out == gaps {
                    return Err(IngestError::Invalid(
                        "--out and --gaps need different paths".into(),
                    ));
                }
                let store = SnapshotStore::open(&store_dir)?;
                let rows = if src == "cdc-coverage" {
                    coverage::parse_latest_cdc(&store, first, last)?
                } else {
                    coverage::parse_latest_texas(&store, first)?
                };
                let missing = coverage::gaps(&rows);
                eprintln!(
                    "parsed {} coverage rows; {} missing",
                    rows.len(),
                    missing.len()
                );
                if let Some(path) = gaps {
                    std::fs::write(
                        &path,
                        serde_json::to_string(&missing).expect("gaps serialise"),
                    )
                    .map_err(|e| IngestError::io(path, e))?;
                }
                let json = serde_json::to_string(&rows).expect("rows serialise");
                match out {
                    Some(path) => std::fs::write(&path, json).map_err(|e| IngestError::io(path, e)),
                    None => {
                        println!("{json}");
                        Ok(())
                    }
                }
            }
        }
        ("fetch", Some("cdc-cases")) => {
            let first = year(flags.take("--first-year")?, cdc::FIRST_YEAR)?;
            let last = year(
                flags.take("--last-year")?,
                u16::try_from(Utc::now().year()).unwrap_or(cdc::FIRST_YEAR),
            )?;
            flags.done()?;
            // Identify the client before anything else: no contact, no request (and no store).
            let cfg = PoliteConfig::live(contact_from_env().as_deref())?;
            let spec = cdc::source_spec(first, last)?;
            let store = SnapshotStore::open(&store_dir)?;
            let client = UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES);
            let mut fetcher = PoliteFetcher::new(client, SystemTimekeeper::new(), cfg);
            let (r, outcome) = fetch_to_store(&mut fetcher, &store, &spec)?;
            let note = match outcome {
                PutOutcome::Created => "new snapshot",
                PutOutcome::AlreadyPresent => "unchanged bytes, no new blob",
            };
            eprintln!(
                "{note}: sha256 {} ({} bytes) retrieved {}",
                r.sha256,
                r.bytes,
                r.retrieved_at.to_rfc3339()
            );
            println!(
                "{}",
                serde_json::to_string(&r).expect("Retrieval serialises")
            );
            Ok(())
        }
        ("parse", Some("cdc-cases")) => {
            let out = flags.take("--out")?;
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            let (retrieval, rows) = cdc::parse_latest(&store)?;
            eprintln!(
                "parsed {} weekly rows from snapshot {}",
                rows.len(),
                retrieval.sha256
            );
            let json = serde_json::to_string(&rows).expect("rows serialise");
            match out {
                Some(path) => std::fs::write(&path, json).map_err(|e| IngestError::io(path, e)),
                None => {
                    println!("{json}");
                    Ok(())
                }
            }
        }
        ("fetch", Some("census-counties")) => {
            let direct = flags.take("--direct")?;
            flags.done()?;
            let (spec, secs) = match direct {
                Some(_) => (census_counties::source_spec(), 1),
                None => (census_counties::wayback_spec(), 8),
            };
            let mut fetcher = live_fetcher(secs)?;
            let store = SnapshotStore::open(&store_dir)?;
            let (r, _) = fetch_to_store(&mut fetcher, &store, &spec)?;
            println!(
                "{}",
                serde_json::to_string(&r).expect("Retrieval serialises")
            );
            Ok(())
        }
        ("fetch", Some("dshs-live")) => {
            flags.done()?;
            let mut fetcher = live_fetcher(1)?;
            let store = SnapshotStore::open(&store_dir)?;
            let specs = [
                dshs_sources::live_spec(
                    dshs_sources::SOURCE_PAGE_LIVE,
                    dshs_sources::OUTBREAK_PAGE_URL,
                ),
                dshs_sources::live_spec(
                    dshs_sources::SOURCE_REPORT_LIVE,
                    dshs_sources::FINAL_REPORT_URL,
                ),
            ];
            let results = specs
                .iter()
                .map(|s| {
                    let r = fetch_to_store(&mut fetcher, &store, s);
                    (s.clone(), r)
                })
                .collect::<Vec<_>>();
            for (spec, r) in results {
                let (r, _) = r?;
                eprintln!("{} sha256 {}", spec.url, r.sha256);
                println!(
                    "{}",
                    serde_json::to_string(&r).expect("Retrieval serialises")
                );
            }
            Ok(())
        }
        ("fetch", Some("dshs-wayback")) => {
            let from = flags.take("--from")?.unwrap_or_else(|| "20250301".into());
            let to = flags.take("--to")?.unwrap_or_else(|| "20250910".into());
            // Archive.org rate-limits well below one request a second (HTTP 429), so pace it.
            let secs = year(flags.take("--interval-secs")?, 8)?;
            flags.done()?;
            let mut fetcher = live_fetcher(u64::from(secs))?;
            let store = SnapshotStore::open(&store_dir)?;
            let results = dshs_sources::fetch_outbreak_captures(&mut fetcher, &store, &from, &to)?;
            report_outcomes(&results)
        }
        ("fetch", Some("dshs-reports")) => {
            let secs = year(flags.take("--interval-secs")?, 8)?;
            flags.done()?;
            let mut fetcher = live_fetcher(u64::from(secs))?;
            let store = SnapshotStore::open(&store_dir)?;
            let results = dshs_sources::fetch_report_documents(&mut fetcher, &store)?;
            report_outcomes(&results)
        }
        ("parse", Some("dshs-cases")) => {
            let out = flags.take("--out")?.unwrap_or_else(|| "data/dshs".into());
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            let lookup = census_counties::CountyLookup::from_store(
                &store,
                koplik_contracts::v1::StateFips::new(48).expect("48 is Texas"),
            )?;
            let built = dshs_series::build_from_store(&store, &lookup)?;
            std::fs::create_dir_all(&out).map_err(|e| IngestError::io(&out, e))?;
            let write = |name: &str, json: String| -> Result<()> {
                let path = std::path::Path::new(&out).join(name);
                std::fs::write(&path, json + "\n").map_err(|e| IngestError::io(path, e))
            };
            write("vintage-manifest.json", to_pretty(&built.manifest))?;
            write("cumulative.json", to_pretty(&built.series.cumulative))?;
            write("intervals.json", to_pretty(&built.series.intervals))?;
            write("weekly.json", to_pretty(&built.series.weekly))?;
            write("unmapped.json", to_pretty(&built.series.unmapped))?;
            write("parse-failures.json", to_pretty(&built.failures))?;
            eprintln!(
                "{} vintages ({} with county detail), {} weekly rows, {} unmapped names, {} snapshots failed to parse; written to {out}",
                built.vintages.len(),
                built
                    .vintages
                    .iter()
                    .filter(|v| v.has_county_detail())
                    .count(),
                built.series.weekly.len(),
                built.series.unmapped.len(),
                built.failures.len()
            );
            Ok(())
        }
        ("fetch", Some("census-boundaries")) => {
            flags.done()?;
            let cfg = PoliteConfig::live(contact_from_env().as_deref())?;
            let store = SnapshotStore::open(&store_dir)?;
            let client = UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES);
            let mut fetcher = PoliteFetcher::new(client, SystemTimekeeper::new(), cfg);
            for retrieval in census_boundaries::fetch(&mut fetcher, &store)? {
                println!(
                    "{}",
                    serde_json::to_string(&retrieval).expect("Retrieval serialises")
                );
            }
            Ok(())
        }
        ("parse", Some("census-boundaries")) => {
            let out = flags
                .take("--out")?
                .ok_or_else(|| IngestError::Invalid("census-boundaries needs --out DIR".into()))?;
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            for retrieval in census_boundaries::write_latest(&store, &out)? {
                eprintln!("converted Census snapshot {}", retrieval.sha256);
            }
            Ok(())
        }
        ("list", None) => {
            let src = flags.take("--source")?;
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            for r in store.retrievals(src.as_deref())? {
                println!(
                    "{}",
                    serde_json::to_string(&r).expect("Retrieval serialises")
                );
            }
            Ok(())
        }
        (c, s) => Err(IngestError::Invalid(format!(
            "unknown command {c} {}",
            s.unwrap_or("")
        ))),
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            if matches!(e, IngestError::Invalid(_)) {
                eprintln!("{USAGE}");
            }
            ExitCode::FAILURE
        }
    }
}
