//! Where the Texas DSHS 2025 West Texas measles outbreak numbers come from, and how past
//! versions of them are reached.
//!
//! DSHS published the outbreak in three formats during 2025 (see [`crate::dshs`]): an HTML
//! county table on the outbreak page (March to mid April), a Tableau dashboard embedded in
//! the same page (late April to August; its host answers automated requests with HTTP 403,
//! which this crate does not work around), and PDF "2025 Measles Data Report" files (from
//! November). DSHS keeps only the current version of each page, so earlier versions come from
//! Internet Archive captures. Each capture is its own snapshot: its provenance URL is the
//! capture URL (`https://web.archive.org/web/<14-digit capture time>id_/<original URL>`; the
//! `id_` form returns the archived bytes unmodified), which carries both the capture time and
//! the original URL. [`Capture::parse_url`] reads them back.

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::error::{IngestError, Result};
use crate::http::HttpClient;
use crate::polite::{PoliteFetcher, Timekeeper};
use crate::source::{SourceSpec, fetch_to_store};
use crate::store::{PutOutcome, Retrieval, SnapshotStore};

/// The outbreak page as DSHS serves it now.
pub const OUTBREAK_PAGE_URL: &str = "https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025";
/// The final data report linked from DSHS's "Measles (Rubeola) Data" page.
pub const FINAL_REPORT_URL: &str = "https://www.dshs.texas.gov/sites/default/files/Admin-Meales/doc/2025-measles-outbreak-data-report-011226.pdf";
/// Earlier data reports, found through the Internet Archive's CDX listing of the DSHS
/// `Admin-Meales/doc/` directory (the live site no longer links them).
pub const EARLIER_REPORT_URLS: [&str; 2] = [
    "https://www.dshs.texas.gov/sites/default/files/Admin-Meales/doc/2025-measles-data-report-nov-2025.pdf",
    "https://www.dshs.texas.gov/sites/default/files/Admin-Meales/doc/2025-measles-outbreak-data-report-12-23-25.pdf",
];

/// Source ids (listed in `SOURCES.md`).
pub const SOURCE_PAGE_LIVE: &str = "dshs-measles-outbreak-page";
pub const SOURCE_PAGE_WAYBACK: &str = "dshs-measles-outbreak-page-wayback";
pub const SOURCE_REPORT_LIVE: &str = "dshs-measles-data-report";
pub const SOURCE_REPORT_WAYBACK: &str = "dshs-measles-data-report-wayback";
pub const SOURCE_CDX: &str = "wayback-cdx-listing";

/// DSHS's "Copyright and Disclaimer" page grants permission to copy and distribute for
/// non-commercial use "so long as the information is copied and distributed without
/// alteration". Whether Koplik's derived series and map fit that is an operator decision.
pub const DSHS_LICENCE_ID: &str = "dshs-copyright-noncommercial-no-alteration";
/// The Internet Archive's own terms of use for its captures; see `SOURCES.md`.
pub const WAYBACK_LICENCE_ID: &str = "internet-archive-terms-of-use";

const WAYBACK_HOST: &str = "https://web.archive.org";

/// One Internet Archive capture of a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// `YYYYMMDDhhmmss`, UTC.
    pub timestamp: String,
    /// The URL as archived (scheme and host as DSHS served it at the time).
    pub original: String,
}

impl Capture {
    /// The URL whose body is the unmodified archived bytes.
    pub fn url(&self) -> String {
        format!("{WAYBACK_HOST}/web/{}id_/{}", self.timestamp, self.original)
    }

    /// Read a capture URL built by [`Capture::url`] back into its parts.
    pub fn parse_url(url: &str) -> Option<Self> {
        let rest = url.strip_prefix(WAYBACK_HOST)?.strip_prefix("/web/")?;
        let (stamp, original) = rest.split_once("id_/")?;
        if stamp.len() != 14 || !stamp.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(Self {
            timestamp: stamp.to_owned(),
            original: original.to_owned(),
        })
    }

    /// Capture time as UTC.
    pub fn time(&self) -> Result<DateTime<Utc>> {
        NaiveDateTime::parse_from_str(&self.timestamp, "%Y%m%d%H%M%S")
            .map(|t| t.and_utc())
            .map_err(|e| {
                IngestError::Parse(format!("bad capture timestamp {:?}: {e}", self.timestamp))
            })
    }
}

/// CDX query listing 200-status captures of `original` between two `YYYYMMDD` dates, the
/// first of each day (`collapse=timestamp:8`). The CDX answer is itself stored as a snapshot,
/// so which captures existed when the list was taken stays on record.
pub fn cdx_url(original_without_scheme: &str, from: &str, to: &str, one_per_day: bool) -> String {
    let collapse = if one_per_day {
        "&collapse=timestamp:8"
    } else {
        ""
    };
    format!(
        "{WAYBACK_HOST}/cdx/search/cdx?url={original_without_scheme}&output=json&fl=timestamp,original,statuscode,digest&filter=statuscode:200&from={from}&to={to}{collapse}"
    )
}

/// Parse a CDX `output=json` listing (a header row, then one row per capture).
pub fn parse_cdx(bytes: &[u8]) -> Result<Vec<Capture>> {
    let rows: Vec<Vec<String>> = serde_json::from_slice(bytes)
        .map_err(|e| IngestError::Parse(format!("CDX listing is not a JSON table: {e}")))?;
    let mut rows = rows.into_iter();
    let Some(header) = rows.next() else {
        return Ok(Vec::new());
    };
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| IngestError::Parse(format!("CDX listing has no {name} column")))
    };
    let (ts, orig) = (col("timestamp")?, col("original")?);
    rows.map(|r| match (r.get(ts), r.get(orig)) {
        (Some(t), Some(o)) => Ok(Capture {
            timestamp: t.clone(),
            original: o.clone(),
        }),
        _ => Err(IngestError::Parse("short CDX row".into())),
    })
    .collect()
}

pub fn cdx_spec(url: String) -> SourceSpec {
    SourceSpec {
        source_id: SOURCE_CDX.to_owned(),
        url,
        licence_id: WAYBACK_LICENCE_ID.to_owned(),
    }
}

pub fn capture_spec(source_id: &str, c: &Capture) -> SourceSpec {
    SourceSpec {
        source_id: source_id.to_owned(),
        url: c.url(),
        licence_id: WAYBACK_LICENCE_ID.to_owned(),
    }
}

pub fn live_spec(source_id: &str, url: &str) -> SourceSpec {
    SourceSpec {
        source_id: source_id.to_owned(),
        url: url.to_owned(),
        licence_id: DSHS_LICENCE_ID.to_owned(),
    }
}

/// What happened to one capture during a bulk fetch.
#[derive(Debug)]
pub enum FetchOutcome {
    /// Already in the store (same source and URL): not requested again.
    Skipped(Retrieval),
    Fetched(Retrieval, PutOutcome),
    Failed(String),
}

/// Fetch `specs` politely, skipping any URL already retrieved for the same source, and keep
/// going past individual failures (they are returned, never dropped).
pub fn fetch_missing<C: HttpClient, T: Timekeeper>(
    fetcher: &mut PoliteFetcher<C, T>,
    store: &SnapshotStore,
    specs: &[SourceSpec],
) -> Result<Vec<(SourceSpec, FetchOutcome)>> {
    let mut out = Vec::with_capacity(specs.len());
    for spec in specs {
        let existing = store
            .retrievals(Some(&spec.source_id))?
            .into_iter()
            .find(|r| r.url == spec.url);
        let outcome = match existing {
            Some(r) => FetchOutcome::Skipped(r),
            None => match fetch_to_store(fetcher, store, spec) {
                Ok((r, o)) => FetchOutcome::Fetched(r, o),
                Err(e) => FetchOutcome::Failed(e.to_string()),
            },
        };
        out.push((spec.clone(), outcome));
    }
    Ok(out)
}

/// List the Internet Archive's first capture of each day of the outbreak page between two
/// `YYYYMMDD` dates (storing the CDX answer as a snapshot) and fetch every capture not yet in
/// the store. The CDX index keys URLs without the `www.` prefix, so one query lists captures
/// under both host spellings (the listing's `original` column says which). One capture per
/// day means a version published and replaced within a day, or an update first captured
/// later in the day than another capture, can be missed; the manifest records exactly
/// which captures were held.
pub fn fetch_outbreak_captures<C: HttpClient, T: Timekeeper>(
    fetcher: &mut PoliteFetcher<C, T>,
    store: &SnapshotStore,
    from: &str,
    to: &str,
) -> Result<Vec<(SourceSpec, FetchOutcome)>> {
    let (listing, _) = fetch_to_store(
        fetcher,
        store,
        &cdx_spec(cdx_url(
            "www.dshs.texas.gov/news-alerts/measles-outbreak-2025",
            from,
            to,
            true,
        )),
    )?;
    let mut specs: Vec<SourceSpec> = parse_cdx(&store.get_verified(&listing.sha256)?)?
        .iter()
        .map(|c| capture_spec(SOURCE_PAGE_WAYBACK, c))
        .collect();
    specs.sort_by(|a, b| a.url.cmp(&b.url));
    specs.dedup();
    fetch_missing(fetcher, store, &specs)
}

/// The Internet Archive's earliest capture of each DSHS data-report PDF, plus the live final
/// report, fetched into the store.
pub fn fetch_report_documents<C: HttpClient, T: Timekeeper>(
    fetcher: &mut PoliteFetcher<C, T>,
    store: &SnapshotStore,
) -> Result<Vec<(SourceSpec, FetchOutcome)>> {
    let mut specs = vec![live_spec(SOURCE_REPORT_LIVE, FINAL_REPORT_URL)];
    for original in EARLIER_REPORT_URLS.iter().chain([&FINAL_REPORT_URL]) {
        let no_scheme = original.trim_start_matches("https://");
        let (listing, _) = fetch_to_store(
            fetcher,
            store,
            &cdx_spec(cdx_url(no_scheme, "20250101", "20261231", false)),
        )?;
        if let Some(first) = parse_cdx(&store.get_verified(&listing.sha256)?)?
            .into_iter()
            .min_by(|a, b| a.timestamp.cmp(&b.timestamp))
        {
            specs.push(capture_spec(SOURCE_REPORT_WAYBACK, &first));
        }
    }
    fetch_missing(fetcher, store, &specs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_url_round_trips_with_time_and_original() {
        let c = Capture {
            timestamp: "20250305184528".into(),
            original: OUTBREAK_PAGE_URL.into(),
        };
        let url = c.url();
        assert_eq!(
            url,
            "https://web.archive.org/web/20250305184528id_/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025"
        );
        assert_eq!(Capture::parse_url(&url), Some(c.clone()));
        assert_eq!(c.time().unwrap().to_rfc3339(), "2025-03-05T18:45:28+00:00");
        assert_eq!(Capture::parse_url("https://example.org/x"), None);
        assert_eq!(
            Capture::parse_url("https://web.archive.org/web/2025id_/https://x"),
            None
        );
    }

    #[test]
    fn cdx_listing_parses_header_then_rows() {
        let json = br#"[["timestamp","original","statuscode","digest"],
            ["20250305184528","https://www.dshs.texas.gov/a","200","AAA"],
            ["20250306061540","https://dshs.texas.gov/a","200","BBB"]]"#;
        let caps = parse_cdx(json).unwrap();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[1].timestamp, "20250306061540");
        assert_eq!(caps[1].original, "https://dshs.texas.gov/a");
        assert!(parse_cdx(b"[]").unwrap().is_empty());
        assert!(parse_cdx(b"not json").is_err());
    }

    #[test]
    fn captures_are_fetched_once_each_and_failures_are_reported_not_dropped() {
        use crate::polite::PoliteConfig;
        use crate::polite::fakes::{FakeClient, FakeTime};
        let page = "https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025";
        let c = FakeClient::default();
        c.on(
            "https://web.archive.org/robots.txt",
            FakeClient::status(404, b""),
        );
        let cdx = cdx_url(
            "www.dshs.texas.gov/news-alerts/measles-outbreak-2025",
            "20250301",
            "20250310",
            true,
        );
        let listing = format!(
            r#"[["timestamp","original","statuscode","digest"],
               ["20250305184528","{page}","200","A"],
               ["20250306061540","{page}","200","B"]]"#
        );
        c.on(&cdx, FakeClient::ok(listing.as_bytes()));
        c.on(
            &format!("https://web.archive.org/web/20250305184528id_/{page}"),
            FakeClient::ok(b"first"),
        );
        c.on(
            &format!("https://web.archive.org/web/20250306061540id_/{page}"),
            FakeClient::status(404, b"gone"),
        );
        let mut f = PoliteFetcher::new(c, FakeTime::default(), PoliteConfig::default());
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();

        let r = fetch_outbreak_captures(&mut f, &store, "20250301", "20250310").unwrap();
        assert!(matches!(r[0].1, FetchOutcome::Fetched(..)));
        assert!(matches!(&r[1].1, FetchOutcome::Failed(e) if e.contains("404")));
        // The capture's provenance URL carries its capture time and the original URL.
        let kept = store.retrievals(Some(SOURCE_PAGE_WAYBACK)).unwrap();
        assert_eq!(kept.len(), 1);
        let cap = Capture::parse_url(&kept[0].url).unwrap();
        assert_eq!(
            (cap.timestamp.as_str(), cap.original.as_str()),
            ("20250305184528", page)
        );
        assert_eq!(kept[0].licence_id, WAYBACK_LICENCE_ID);

        // A second run does not request the held capture again; the failed one is retried.
        let r = fetch_outbreak_captures(&mut f, &store, "20250301", "20250310").unwrap();
        assert!(matches!(r[0].1, FetchOutcome::Skipped(_)));
        assert!(matches!(r[1].1, FetchOutcome::Failed(_)));
        assert_eq!(
            store.retrievals(Some(SOURCE_PAGE_WAYBACK)).unwrap().len(),
            1
        );
    }
}
