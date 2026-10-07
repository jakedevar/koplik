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
//! `fetch` is the only command that uses the network. `parse` is offline: it reads the latest
//! stored CDC snapshot (re-verifying its SHA-256) and writes contracts v1 weekly-case rows as
//! JSON (stdout, or `--out`). Census parsing writes both GeoJSON files to `--out DIR`.
//! Set `KOPLIK_CONTACT` to a repository URL or contact address to
//! identify the client to the data hosts.

use std::process::ExitCode;
use std::time::Duration;

use chrono::{Datelike, Utc};
use koplik_ingest::error::{IngestError, Result};
use koplik_ingest::http::UreqClient;
use koplik_ingest::polite::{PoliteConfig, PoliteFetcher, SystemTimekeeper};
use koplik_ingest::source::fetch_to_store;
use koplik_ingest::store::{DEFAULT_ROOT, PutOutcome, SnapshotStore};
use koplik_ingest::{cdc, census_boundaries};

const USAGE: &str = "usage:
  koplik-ingest fetch cdc-cases [--store DIR] [--first-year Y] [--last-year Y]
  koplik-ingest parse cdc-cases [--store DIR] [--out FILE]
  koplik-ingest fetch census-boundaries [--store DIR]
  koplik-ingest parse census-boundaries [--store DIR] --out DIR
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
        ("fetch", Some("census-boundaries")) => {
            flags.done()?;
            let store = SnapshotStore::open(&store_dir)?;
            let client = UreqClient::new(Duration::from_secs(60), MAX_BODY_BYTES);
            let mut fetcher =
                PoliteFetcher::new(client, SystemTimekeeper::new(), PoliteConfig::default());
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
