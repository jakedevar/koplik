//! `koplik-ingest`: the pipeline's entry point to the network and the snapshot store.
//!
//! ```text
//! koplik-ingest fetch cdc-cases [--store DIR] [--first-year Y] [--last-year Y]
//! koplik-ingest parse cdc-cases [--store DIR] [--out FILE]
//! koplik-ingest list [--store DIR] [--source ID]
//! ```
//!
//! `fetch` is the only command that uses the network. `parse` is offline: it reads the latest
//! stored CDC snapshot (re-verifying its SHA-256) and writes contracts v1 weekly-case rows as
//! JSON (stdout, or `--out`). Set `KOPLIK_CONTACT` to a repository URL or contact address to
//! identify the client to the data hosts.

use std::process::ExitCode;
use std::time::Duration;

use chrono::{Datelike, Utc};
use koplik_ingest::cdc;
use koplik_ingest::census_counties;
use koplik_ingest::dshs_series;
use koplik_ingest::dshs_sources::{self, FetchOutcome};
use koplik_ingest::error::{IngestError, Result};
use koplik_ingest::http::UreqClient;
use koplik_ingest::polite::{PoliteConfig, PoliteFetcher, SystemTimekeeper};
use koplik_ingest::source::{SourceSpec, fetch_to_store};
use koplik_ingest::store::{DEFAULT_ROOT, PutOutcome, SnapshotStore};

const USAGE: &str = "usage:
  koplik-ingest fetch cdc-cases [--store DIR] [--first-year Y] [--last-year Y]
  koplik-ingest parse cdc-cases [--store DIR] [--out FILE]
  koplik-ingest fetch census-counties [--store DIR] [--direct 1]
  koplik-ingest fetch dshs-live [--store DIR]
  koplik-ingest fetch dshs-wayback [--store DIR] [--from YYYYMMDD] [--to YYYYMMDD]
  koplik-ingest fetch dshs-reports [--store DIR]
  koplik-ingest parse dshs-cases [--store DIR] [--out DIR]
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

fn fetcher(interval_secs: u64) -> PoliteFetcher<UreqClient, SystemTimekeeper> {
    let client = UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES);
    let cfg = PoliteConfig {
        min_interval: Duration::from_secs(interval_secs.max(1)),
        max_attempts: 5,
        ..PoliteConfig::default()
    };
    PoliteFetcher::new(client, SystemTimekeeper::new(), cfg)
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
        "fetch" | "parse" => Some(
            args.next()
                .ok_or_else(|| IngestError::Invalid("missing source (cdc-cases)".into()))?,
        ),
        _ => None,
    };
    let mut flags = Flags(args.collect());
    let store_dir = flags
        .take("--store")?
        .unwrap_or_else(|| DEFAULT_ROOT.to_owned());
    match (cmd.as_str(), source.as_deref()) {
        ("fetch", Some("cdc-cases")) => {
            let first = year(flags.take("--first-year")?, cdc::FIRST_YEAR)?;
            let last = year(
                flags.take("--last-year")?,
                u16::try_from(Utc::now().year()).unwrap_or(cdc::FIRST_YEAR),
            )?;
            flags.done()?;
            let spec = cdc::source_spec(first, last)?;
            let store = SnapshotStore::open(&store_dir)?;
            let client = UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES);
            let mut fetcher =
                PoliteFetcher::new(client, SystemTimekeeper::new(), PoliteConfig::default());
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
            let store = SnapshotStore::open(&store_dir)?;
            let (spec, secs) = match direct {
                Some(_) => (census_counties::source_spec(), 1),
                None => (census_counties::wayback_spec(), 8),
            };
            let mut fetcher = fetcher(secs);
            let (r, _) = fetch_to_store(&mut fetcher, &store, &spec)?;
            println!(
                "{}",
                serde_json::to_string(&r).expect("Retrieval serialises")
            );
            Ok(())
        }
        ("fetch", Some("dshs-live")) => {
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            let mut fetcher = fetcher(1);
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
            let store = SnapshotStore::open(&store_dir)?;
            let mut fetcher = fetcher(u64::from(secs));
            let results = dshs_sources::fetch_outbreak_captures(&mut fetcher, &store, &from, &to)?;
            report_outcomes(&results)
        }
        ("fetch", Some("dshs-reports")) => {
            let secs = year(flags.take("--interval-secs")?, 8)?;
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            let mut fetcher = fetcher(u64::from(secs));
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
