//! County name -> FIPS lookup built from the Census national county reference file, fetched
//! through the snapshot store (so every FIPS code traces to a snapshot hash, URL and
//! retrieval time). Nothing here is typed by hand: an unknown or ambiguous name is an error
//! value for the caller to report, never a guess.
//!
//! File: `national_county2020.txt` (pipe-delimited, header
//! `STATE|STATEFP|COUNTYFP|COUNTYNS|COUNTYNAME|CLASSFP|FUNCSTAT`), e.g.
//! `TX|48|141|01383855|El Paso County|H1|A`.

use std::collections::BTreeMap;

use koplik_contracts::v1::{CountyFips, StateFips};

use crate::error::{IngestError, Result};
use crate::source::SourceSpec;
use crate::store::{Retrieval, SnapshotStore};

pub const SOURCE_ID: &str = "census-county-codes-2020";
/// The Census Bureau's reference-file page lists these as public downloads; terms are
/// recorded in `SOURCES.md` (US federal agency product, reuse terms not formally confirmed).
pub const LICENCE_ID: &str = "census-open-data-terms-unconfirmed";
pub const URL: &str =
    "https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt";

/// Source id when the file is taken from an Internet Archive capture (see [`wayback_spec`]).
pub const SOURCE_ID_WAYBACK: &str = "census-county-codes-2020-wayback";
/// The archived capture used when the live host cannot be fetched: taken 2025-02-06, during
/// the outbreak. (The file's Texas rows are identical in the two versions the Archive holds.)
pub const WAYBACK_TIMESTAMP: &str = "20250206022004";

pub fn source_spec() -> SourceSpec {
    SourceSpec {
        source_id: SOURCE_ID.to_owned(),
        url: URL.to_owned(),
        licence_id: LICENCE_ID.to_owned(),
    }
}

/// The same file from the Internet Archive. `www2.census.gov/robots.txt` lists
/// `User-agent: *` and `User-agent: RavenCrawler` in one group (a blank line between them)
/// ahead of `Disallow: /`, which RFC 9309 reads as "no crawler may fetch anything", so the
/// polite fetcher refuses the live URL. The Archive's capture is fetched instead, with its
/// capture URL and time in provenance; the live host is not worked around.
pub fn wayback_spec() -> SourceSpec {
    let capture = crate::dshs_sources::Capture {
        timestamp: WAYBACK_TIMESTAMP.to_owned(),
        original: URL.to_owned(),
    };
    SourceSpec {
        source_id: SOURCE_ID_WAYBACK.to_owned(),
        url: capture.url(),
        licence_id: LICENCE_ID.to_owned(),
    }
}

/// Suffixes the Census appends to county names ("Harris County", "Orleans Parish", ...).
const SUFFIXES: [&str; 6] = [
    " County",
    " Parish",
    " Borough",
    " Census Area",
    " Municipio",
    " Municipality",
];

/// Normalise a county name for matching: lower case, no "County"-style suffix, no periods,
/// whitespace collapsed. "McLennan", "Mclennan County" and "McLennan  " all match; this is
/// punctuation/case folding only, never fuzzy matching.
pub fn normalise_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let base = SUFFIXES
        .iter()
        .find_map(|s| lower.strip_suffix(&s.to_lowercase()))
        .unwrap_or(&lower);
    base.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('.', "")
}

/// Why a name did not map to exactly one county.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupError {
    NotFound,
    /// More than one county of the state normalises to this name (listed by FIPS).
    Ambiguous(Vec<CountyFips>),
}

/// Counties of one state keyed by normalised name.
#[derive(Debug, Clone)]
pub struct CountyLookup {
    state: StateFips,
    by_name: BTreeMap<String, Vec<CountyFips>>,
    /// The retrieval the table was built from (provenance of every code).
    pub retrieval: Retrieval,
}

impl CountyLookup {
    /// Parse the Census file for `state`. Every data line must have the documented seven
    /// fields; anything else is an error rather than a skipped line.
    pub fn parse(bytes: &[u8], state: StateFips, retrieval: Retrieval) -> Result<Self> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| IngestError::Parse(format!("county file is not UTF-8: {e}")))?;
        let mut lines = text.lines();
        let header = lines.next().unwrap_or("");
        if header.trim_end() != "STATE|STATEFP|COUNTYFP|COUNTYNS|COUNTYNAME|CLASSFP|FUNCSTAT" {
            return Err(IngestError::Parse(format!(
                "unexpected county file header {header:?}"
            )));
        }
        let mut by_name: BTreeMap<String, Vec<CountyFips>> = BTreeMap::new();
        for (i, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split('|').collect();
            if f.len() != 7 {
                return Err(IngestError::Parse(format!(
                    "county file line {}: expected 7 fields, got {}",
                    i + 2,
                    f.len()
                )));
            }
            let bad = |what: &str| {
                IngestError::Parse(format!("county file line {}: bad {what}: {line:?}", i + 2))
            };
            let st: u8 = f[1].parse().map_err(|_| bad("STATEFP"))?;
            if st != state.code() {
                continue;
            }
            let cty: u16 = f[2].parse().map_err(|_| bad("COUNTYFP"))?;
            let fips = CountyFips::from_parts(state, cty).map_err(|_| bad("county code"))?;
            by_name.entry(normalise_name(f[4])).or_default().push(fips);
        }
        if by_name.is_empty() {
            return Err(IngestError::Parse(format!(
                "no counties for state {} in the county file",
                state.code()
            )));
        }
        Ok(Self {
            state,
            by_name,
            retrieval,
        })
    }

    /// Parse the latest stored snapshot of the Census county file (offline).
    pub fn from_store(store: &SnapshotStore, state: StateFips) -> Result<Self> {
        // A direct snapshot of the live file wins over an archived capture.
        let retrieval = match store.latest(SOURCE_ID)? {
            Some(r) => r,
            None => store
                .latest(SOURCE_ID_WAYBACK)?
                .ok_or_else(|| IngestError::NoSnapshot(SOURCE_ID.to_owned()))?,
        };
        let bytes = store.get_verified(&retrieval.sha256)?;
        Self::parse(&bytes, state, retrieval)
    }

    pub fn state(&self) -> StateFips {
        self.state
    }

    pub fn len(&self) -> usize {
        self.by_name.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    pub fn lookup(&self, name: &str) -> std::result::Result<CountyFips, LookupError> {
        match self.by_name.get(&normalise_name(name)).map(Vec::as_slice) {
            None | Some([]) => Err(LookupError::NotFound),
            Some([one]) => Ok(*one),
            Some(many) => Err(LookupError::Ambiguous(many.to_vec())),
        }
    }
}
