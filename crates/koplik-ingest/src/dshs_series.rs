//! From parsed DSHS reports to the vintage manifest and county series.
//!
//! # Vintages
//! A *vintage* is one distinct version of a DSHS report: the same date and the same content
//! (format, outbreak total, every county row). Many Internet Archive captures show the same
//! version; the vintage keeps the earliest one (`first_seen_at`: the capture time, or the
//! retrieval time for a live fetch) because that is the evidence of when the numbers were
//! public. A report dated D can only have been *used* at a forecast date on or after both D
//! and `first_seen_at`, and `first_seen_at` is an upper bound on when it appeared, not an
//! exact publication time. The manifest lists every vintage held, with its provenance.
//!
//! # Derivation (documented in `SOURCES.md`)
//! DSHS publishes **cumulative** outbreak cases per county. Only vintages with a readable
//! outbreak county table take part (the Tableau-era pages publish no county breakdown that
//! can be fetched; those vintages appear in the manifest with `county_detail: false` and
//! contribute no county numbers: missing stays missing). If several vintages share a report
//! date, the one first seen last supersedes the others in the series (all stay in the
//! manifest).
//!
//! * *Cumulative per county*: the printed row. A county absent from a table counts as
//!   `reported 0` only when the table's rows are all readable and add up to its printed
//!   Total (so the table demonstrably lists every case); otherwise it is `missing:ambiguous`.
//!   Counties never listed in any vintage get no rows at all.
//! * *Per report interval*: cumulative(this report) - cumulative(previous report), `reported`
//!   when both are `reported` and the cumulative did not fall; a fall (a case removed or
//!   reclassified) or an unreadable value is `missing:ambiguous`.
//! * *Per MMWR week*, a week being the MMWR week that contains the report date: for week W
//!   take the last county-detail report dated in W and the last one before W. The weekly count
//!   is their difference (rules as for intervals) only when both are *the last report of any
//!   kind* in their weeks and the earlier one is in week W-1; so a week whose last report has
//!   no county breakdown (the Tableau-era pages) is not given a partial count. Otherwise the
//!   week is `missing:ambiguous` (it has a report but the count cannot be completed or
//!   split, e.g. after a gap or a breakdown-less last report). A week with no county-detail
//!   report is `missing:not_reported`, and so is the first report's week (that cumulative
//!   includes everything before it and cannot be assigned to one week). The exact multi-week
//!   increase stays available in the interval series.
//! * The week is the week the report is dated, i.e. when DSHS published the cumulative, not
//!   the week of rash onset. Reporting lags onset; treat recent weeks as provisional.
//!
//! County names map to FIPS only through [`crate::census_counties::CountyLookup`]. A name that
//! does not map to exactly one county is returned in [`Series::unmapped`] and its cases are
//! left out of the numbers rather than assigned anywhere.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, Utc};
use koplik_contracts::v1::{
    CaseCount, CountyFips, GeoId, MissingReason, MmwrWeek, Provenance, Provenances, Sha256Hex,
    WeeklyCaseCount,
};
use serde::{Deserialize, Serialize};

use crate::census_counties::{CountyLookup, LookupError};
use crate::dshs::{Report, ReportFormat};
use crate::dshs_sources::Capture;
use crate::error::{IngestError, Result};
use crate::store::Retrieval;

pub const MANIFEST_VERSION: u32 = 1;

/// One stored snapshot of a vintage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRef {
    pub source_id: String,
    /// The URL fetched: the Internet Archive capture URL, or DSHS's own URL for a live fetch.
    pub url: String,
    /// DSHS's URL for the document (equals `url` for a live fetch).
    pub original_url: String,
    /// When the Internet Archive captured it; `None` for a live fetch.
    pub capture_time: Option<DateTime<Utc>>,
    pub retrieved_at: DateTime<Utc>,
    pub sha256: Sha256Hex,
    pub licence_id: String,
}

impl SnapshotRef {
    pub fn from_retrieval(r: &Retrieval) -> Result<Self> {
        let capture = Capture::parse_url(&r.url);
        Ok(Self {
            source_id: r.source_id.clone(),
            url: r.url.clone(),
            original_url: capture
                .as_ref()
                .map_or_else(|| r.url.clone(), |c| c.original.clone()),
            capture_time: capture.as_ref().map(Capture::time).transpose()?,
            retrieved_at: r.retrieved_at,
            sha256: r.sha256.clone(),
            licence_id: r.licence_id.clone(),
        })
    }

    /// The earliest evidence this snapshot existed: capture time, else retrieval time.
    pub fn seen_at(&self) -> DateTime<Utc> {
        self.capture_time.unwrap_or(self.retrieved_at)
    }

    pub fn provenance(&self) -> Provenance {
        Provenance {
            source_id: self.source_id.clone(),
            url: self.url.clone(),
            retrieved_at: self.retrieved_at,
            sha256: self.sha256.clone(),
            licence_id: self.licence_id.clone(),
        }
    }
}

/// One distinct report version and the snapshots that show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vintage {
    pub report: Report,
    /// Every snapshot with this exact content, earliest first.
    pub snapshots: Vec<SnapshotRef>,
}

impl Vintage {
    pub fn first(&self) -> &SnapshotRef {
        &self.snapshots[0]
    }
    pub fn first_seen_at(&self) -> DateTime<Utc> {
        self.first().seen_at()
    }
    pub fn has_county_detail(&self) -> bool {
        self.report.outbreak_counties.is_some()
    }
}

/// The content that makes two reports the same version.
fn content_key(r: &Report) -> String {
    let counties = r.outbreak_counties.as_ref().map(|t| {
        t.entries
            .iter()
            .map(|e| (e.name.clone(), e.raw.clone()))
            .collect::<Vec<_>>()
    });
    format!(
        "{}|{:?}|{:?}|{:?}|{:?}",
        r.report_date,
        r.format,
        r.outbreak_total,
        counties,
        r.other_tables
            .iter()
            .map(|t| (
                &t.caption,
                t.entries
                    .iter()
                    .map(|e| (&e.name, &e.raw))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    )
}

/// Group parsed reports into vintages, ordered by report date then first-seen time. The
/// vintage's report is the one read from its earliest snapshot.
pub fn group_vintages(reports: Vec<Report>) -> Result<Vec<Vintage>> {
    let mut by_key: BTreeMap<String, Vec<(SnapshotRef, Report)>> = BTreeMap::new();
    for report in reports {
        let snap = SnapshotRef::from_retrieval(&report.retrieval)?;
        by_key
            .entry(content_key(&report))
            .or_default()
            .push((snap, report));
    }
    let mut out: Vec<Vintage> = by_key
        .into_values()
        .map(|mut group| {
            group.sort_by(|a, b| (a.0.seen_at(), &a.0.url).cmp(&(b.0.seen_at(), &b.0.url)));
            let report = group[0].1.clone();
            Vintage {
                report,
                snapshots: group.into_iter().map(|(s, _)| s).collect(),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        (a.report.report_date, a.first_seen_at()).cmp(&(b.report.report_date, b.first_seen_at()))
    });
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Manifest

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub report_date: NaiveDate,
    pub format: ReportFormat,
    pub outbreak_total: Option<u32>,
    /// Whether this vintage has a readable outbreak county table.
    pub county_detail: bool,
    pub county_rows: usize,
    pub data_as_of: Option<String>,
    /// Earliest evidence this version was public (capture time, else retrieval time).
    pub first_seen_at: DateTime<Utc>,
    /// The earliest snapshot: the one numbers derived from this vintage cite.
    pub first_snapshot: SnapshotRef,
    /// How many stored snapshots show exactly this content.
    pub snapshot_count: usize,
    /// Latest capture/retrieval time with exactly this content.
    pub last_seen_at: DateTime<Utc>,
    /// Parser issues for this vintage, verbatim.
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VintageManifest {
    pub manifest_version: u32,
    pub description: String,
    pub entries: Vec<ManifestEntry>,
    /// Captures the Internet Archive's index listed for the outbreak page that could not be
    /// fetched (for example a persistent HTTP 500); their versions, if different, are absent
    /// from `entries`.
    pub unretrieved_captures: Vec<String>,
}

pub fn manifest(vintages: &[Vintage]) -> VintageManifest {
    VintageManifest {
        manifest_version: MANIFEST_VERSION,
        description: "Every distinct Texas DSHS 2025 measles outbreak report version held as a \
                      snapshot (Internet Archive capture or live fetch). Use first_seen_at, \
                      not report_date alone, to decide whether a version was available at a \
                      forecast date. Vintages with county_detail=false have no county numbers."
            .into(),
        entries: vintages
            .iter()
            .map(|v| ManifestEntry {
                report_date: v.report.report_date,
                format: v.report.format,
                outbreak_total: v.report.outbreak_total,
                county_detail: v.has_county_detail(),
                county_rows: v
                    .report
                    .outbreak_counties
                    .as_ref()
                    .map_or(0, |t| t.entries.len()),
                data_as_of: v.report.data_as_of.clone(),
                first_seen_at: v.first_seen_at(),
                first_snapshot: v.first().clone(),
                snapshot_count: v.snapshots.len(),
                last_seen_at: v
                    .snapshots
                    .iter()
                    .map(SnapshotRef::seen_at)
                    .max()
                    .expect("a vintage has a snapshot"),
                issues: v.report.issues.clone(),
            })
            .collect(),
        unretrieved_captures: Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------
// Series

/// A county name that did not map to exactly one county.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unmapped {
    pub report_date: NaiveDate,
    pub name: String,
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CumulativeRow {
    pub geography: GeoId,
    pub report_date: NaiveDate,
    pub cases: CaseCount,
    pub provenance: Provenances,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntervalRow {
    pub geography: GeoId,
    pub from_report: NaiveDate,
    pub to_report: NaiveDate,
    pub new_cases: CaseCount,
    pub provenance: Provenances,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Series {
    pub cumulative: Vec<CumulativeRow>,
    pub intervals: Vec<IntervalRow>,
    pub weekly: Vec<WeeklyCaseCount>,
    pub unmapped: Vec<Unmapped>,
}

fn missing(reason: MissingReason) -> CaseCount {
    CaseCount::Missing { reason }
}

/// New cases between two cumulative values (see the module docs).
fn difference(prev: CaseCount, now: CaseCount) -> CaseCount {
    match (prev, now) {
        (CaseCount::Reported { count: a }, CaseCount::Reported { count: b }) if b >= a => {
            CaseCount::Reported { count: b - a }
        }
        _ => missing(MissingReason::Ambiguous),
    }
}

struct Point<'a> {
    date: NaiveDate,
    week: MmwrWeek,
    /// County -> cumulative as published (absent counties handled per the module docs).
    counties: BTreeMap<CountyFips, CaseCount>,
    /// True when every county of the state is known to be listed (table adds up to its Total).
    complete: bool,
    vintage: &'a Vintage,
}

impl Point<'_> {
    fn cumulative(&self, c: CountyFips) -> CaseCount {
        match self.counties.get(&c) {
            Some(v) => *v,
            None if self.complete => CaseCount::Reported { count: 0 },
            None => missing(MissingReason::Ambiguous),
        }
    }
}

pub fn derive(vintages: &[Vintage], lookup: &CountyLookup) -> Result<Series> {
    let mut unmapped = Vec::new();

    // One point per report date among vintages with county detail; the vintage first seen last wins.
    let mut chosen: BTreeMap<NaiveDate, &Vintage> = BTreeMap::new();
    for v in vintages.iter().filter(|v| v.has_county_detail()) {
        let slot = chosen.entry(v.report.report_date).or_insert(v);
        if v.first_seen_at() >= slot.first_seen_at() {
            *slot = v;
        }
    }
    let mut points: Vec<Point<'_>> = Vec::new();
    for (date, v) in &chosen {
        let table = v
            .report
            .outbreak_counties
            .as_ref()
            .expect("filtered on county detail");
        let mut counties = BTreeMap::new();
        for e in &table.entries {
            match lookup.lookup(&e.name) {
                Ok(fips) => {
                    let value = match e.cases {
                        Some(count) => CaseCount::Reported { count },
                        None => missing(MissingReason::Ambiguous),
                    };
                    if counties.insert(fips, value).is_some() {
                        unmapped.push(Unmapped {
                            report_date: *date,
                            name: e.name.clone(),
                            raw: e.raw.clone(),
                            reason: "county listed twice in one table".into(),
                        });
                        counties.insert(fips, missing(MissingReason::Ambiguous));
                    }
                }
                Err(err) => unmapped.push(Unmapped {
                    report_date: *date,
                    name: e.name.clone(),
                    raw: e.raw.clone(),
                    reason: match err {
                        LookupError::NotFound => "not a county name in the Census file".into(),
                        LookupError::Ambiguous(f) => format!("matches several counties: {f:?}"),
                    },
                }),
            }
        }
        let complete = unmapped.iter().all(|u| u.report_date != *date)
            && table.sum().is_some()
            && table.sum() == table.total;
        points.push(Point {
            date: *date,
            week: MmwrWeek::from_date(*date)
                .map_err(|e| IngestError::Parse(format!("report date {date}: {e}")))?,
            counties,
            complete,
            vintage: v,
        });
    }

    // Dates of every report held, with or without county detail.
    let all_dates: Vec<NaiveDate> = vintages
        .iter()
        .map(|v| v.report.report_date)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let all_counties: BTreeSet<CountyFips> = points
        .iter()
        .flat_map(|p| p.counties.keys().copied())
        .collect();
    let prov = |ps: &[&Vintage]| -> Provenances {
        let mut recs: Vec<Provenance> = Vec::new();
        for v in ps {
            let p = v.first().provenance();
            if !recs.contains(&p) {
                recs.push(p);
            }
        }
        Provenances::new(recs).expect("at least one vintage")
    };

    let mut cumulative = Vec::new();
    let mut intervals = Vec::new();
    let mut weekly = Vec::new();
    for &county in &all_counties {
        let geo = GeoId::County(county);
        for (i, p) in points.iter().enumerate() {
            cumulative.push(CumulativeRow {
                geography: geo,
                report_date: p.date,
                cases: p.cumulative(county),
                provenance: prov(&[p.vintage]),
            });
            if i > 0 {
                let q = &points[i - 1];
                intervals.push(IntervalRow {
                    geography: geo,
                    from_report: q.date,
                    to_report: p.date,
                    new_cases: difference(q.cumulative(county), p.cumulative(county)),
                    provenance: prov(&[q.vintage, p.vintage]),
                });
            }
        }
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            continue;
        };
        let mut week = first.week;
        loop {
            let in_week = points.iter().rfind(|p| p.week == week);
            let before = points.iter().rfind(|p| p.week < week);
            let after = points.iter().find(|p| p.week > week);
            // The last report of any kind in a week; a week's county count is only complete
            // when that report is the county-detail one used.
            let last_any = |w: MmwrWeek| {
                all_dates
                    .iter()
                    .rfind(|d| MmwrWeek::from_date(**d).is_ok_and(|dw| dw == w))
                    .copied()
            };
            let complete = |now: &Point<'_>, prev: &Point<'_>| {
                prev.week.next().is_ok_and(|n| n == week)
                    && last_any(week) == Some(now.date)
                    && last_any(prev.week) == Some(prev.date)
            };
            let (confirmed, provenance) = match (in_week, before) {
                (Some(now), Some(prev)) if complete(now, prev) => (
                    difference(prev.cumulative(county), now.cumulative(county)),
                    prov(&[prev.vintage, now.vintage]),
                ),
                (Some(now), Some(prev)) => (
                    missing(MissingReason::Ambiguous),
                    prov(&[prev.vintage, now.vintage]),
                ),
                (Some(now), None) => (missing(MissingReason::NotReported), prov(&[now.vintage])),
                (None, _) => {
                    // No report this week: cite the reports on either side.
                    let mut bracket: Vec<&Vintage> = Vec::new();
                    bracket.extend(before.map(|p| p.vintage));
                    bracket.extend(after.map(|p| p.vintage));
                    (missing(MissingReason::NotReported), prov(&bracket))
                }
            };
            weekly.push(WeeklyCaseCount {
                geography: geo,
                week,
                confirmed,
                provenance,
            });
            if week >= last.week {
                break;
            }
            week = week
                .next()
                .map_err(|e| IngestError::Parse(format!("MMWR week after {week}: {e}")))?;
        }
    }
    Ok(Series {
        cumulative,
        intervals,
        weekly,
        unmapped,
    })
}

// ---------------------------------------------------------------------------------------
// From the store

/// A snapshot that could not be parsed; reported, never skipped silently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseFailure {
    pub url: String,
    pub sha256: Sha256Hex,
    pub error: String,
}

/// Everything derived from the store's DSHS snapshots.
#[derive(Debug, Clone)]
pub struct Built {
    pub vintages: Vec<Vintage>,
    pub manifest: VintageManifest,
    pub series: Series,
    pub failures: Vec<ParseFailure>,
}

const DSHS_SOURCES: [&str; 4] = [
    crate::dshs_sources::SOURCE_PAGE_LIVE,
    crate::dshs_sources::SOURCE_PAGE_WAYBACK,
    crate::dshs_sources::SOURCE_REPORT_LIVE,
    crate::dshs_sources::SOURCE_REPORT_WAYBACK,
];

/// Outbreak-page captures named by a stored CDX listing but not held in the store.
fn unretrieved_captures(store: &crate::store::SnapshotStore) -> Result<Vec<String>> {
    let held: BTreeSet<String> = store
        .retrievals(Some(crate::dshs_sources::SOURCE_PAGE_WAYBACK))?
        .into_iter()
        .map(|r| r.url)
        .collect();
    let mut missing = BTreeSet::new();
    for r in store.retrievals(Some(crate::dshs_sources::SOURCE_CDX))? {
        if !r.url.contains("news-alerts/measles-outbreak-2025") {
            continue;
        }
        for c in crate::dshs_sources::parse_cdx(&store.get_verified(&r.sha256)?)? {
            if !held.contains(&c.url()) {
                missing.insert(c.url());
            }
        }
    }
    Ok(missing.into_iter().collect())
}

/// Parse every stored DSHS snapshot (offline) and derive the manifest and series.
pub fn build_from_store(
    store: &crate::store::SnapshotStore,
    lookup: &CountyLookup,
) -> Result<Built> {
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for r in store.retrievals(None)? {
        if !DSHS_SOURCES.contains(&r.source_id.as_str()) {
            continue;
        }
        let bytes = store.get_verified(&r.sha256)?;
        match crate::dshs::parse_report(&bytes, &r) {
            Ok(report) => reports.push(report),
            Err(e) => failures.push(ParseFailure {
                url: r.url.clone(),
                sha256: r.sha256.clone(),
                error: e.to_string(),
            }),
        }
    }
    let vintages = group_vintages(reports)?;
    let mut manifest = manifest(&vintages);
    manifest.unretrieved_captures = unretrieved_captures(store)?;
    let series = derive(&vintages, lookup)?;
    Ok(Built {
        vintages,
        manifest,
        series,
        failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dshs::{CountyEntry, CountyTable};
    use crate::store::sha256_of;
    use koplik_contracts::v1::StateFips;

    const CENSUS: &str = "STATE|STATEFP|COUNTYFP|COUNTYNS|COUNTYNAME|CLASSFP|FUNCSTAT\n\
        TX|48|165|01383880|Gaines County|H1|A\n\
        TX|48|445|01383987|Terry County|H1|A\n\
        TX|48|141|01383855|El Paso County|H1|A\n\
        TX|48|303|01383944|Lubbock County|H1|A\n\
        OK|40|001|00000001|Gaines County|H1|A\n";

    fn retrieval(url: &str, at: &str, body: &str) -> Retrieval {
        Retrieval {
            source_id: "dshs-measles-outbreak-page-wayback".into(),
            url: url.into(),
            retrieved_at: at.parse().unwrap(),
            sha256: sha256_of(body.as_bytes()),
            licence_id: "l".into(),
            http_status: 200,
            content_type: None,
            bytes: body.len() as u64,
        }
    }

    fn lookup() -> CountyLookup {
        CountyLookup::parse(
            CENSUS.as_bytes(),
            StateFips::new(48).unwrap(),
            retrieval("https://x/census", "2026-10-07T00:00:00Z", CENSUS),
        )
        .unwrap()
    }

    /// A county-table report dated `date`, captured at `capture` (14-digit), listing `rows`
    /// with the given printed total.
    fn report(date: &str, capture: &str, rows: &[(&str, &str)], total: Option<u32>) -> Report {
        let entries = rows
            .iter()
            .map(|(n, c)| CountyEntry {
                name: (*n).into(),
                raw: (*c).into(),
                cases: crate::dshs::parse_count_cell(c),
            })
            .collect();
        Report {
            format: ReportFormat::HtmlCountyTable,
            report_date: date.parse().unwrap(),
            data_as_of: None,
            outbreak_total: total,
            outbreak_counties: Some(CountyTable {
                caption: "Texas Case Count by County".into(),
                entries,
                total,
            }),
            other_tables: vec![],
            issues: vec![],
            retrieval: retrieval(
                &format!("https://web.archive.org/web/{capture}id_/https://www.dshs.texas.gov/p"),
                "2026-10-07T01:00:00Z",
                &format!("{date}{capture}{rows:?}"),
            ),
        }
    }

    fn fips(n: u32) -> GeoId {
        GeoId::County(CountyFips::new(n).unwrap())
    }

    fn weekly(s: &Series, geo: GeoId) -> Vec<(String, CaseCount)> {
        s.weekly
            .iter()
            .filter(|w| w.geography == geo)
            .map(|w| (w.week.to_string(), w.confirmed))
            .collect()
    }

    fn rep(n: u32) -> CaseCount {
        CaseCount::Reported { count: n }
    }
    fn amb() -> CaseCount {
        missing(MissingReason::Ambiguous)
    }
    fn nr() -> CaseCount {
        missing(MissingReason::NotReported)
    }

    #[test]
    fn identical_content_from_several_captures_is_one_vintage_with_the_earliest_first() {
        let rows = [("Gaines", "107"), ("Terry", "22")];
        let a = report("2025-03-04", "20250306000000", &rows, Some(129));
        let b = report("2025-03-04", "20250305000000", &rows, Some(129));
        let c = report(
            "2025-03-04",
            "20250307000000",
            &[("Gaines", "108")],
            Some(108),
        );
        let v = group_vintages(vec![a, b, c]).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].snapshots.len(), 2);
        assert_eq!(
            v[0].first().capture_time.unwrap().to_rfc3339(),
            "2025-03-05T00:00:00+00:00"
        );
        assert_eq!(
            v[0].first().original_url,
            "https://www.dshs.texas.gov/p".to_string()
        );
        let m = manifest(&v);
        assert_eq!(m.entries[0].snapshot_count, 2);
        assert_eq!(m.entries[1].outbreak_total, Some(108));
    }

    #[test]
    fn weekly_counts_are_differences_of_the_last_report_in_each_week() {
        // 2025-03-04 is MMWR week 10, 03-07 week 10, 03-11 week 11, 03-14 week 11, 03-18 week 12.
        let v = group_vintages(vec![
            report(
                "2025-03-04",
                "20250305000000",
                &[("Gaines", "100")],
                Some(100),
            ),
            report(
                "2025-03-07",
                "20250308000000",
                &[("Gaines", "110"), ("Terry", "2")],
                Some(112),
            ),
            report(
                "2025-03-11",
                "20250312000000",
                &[("Gaines", "120"), ("Terry", "5")],
                Some(125),
            ),
            report(
                "2025-03-14",
                "20250315000000",
                &[("Gaines", "130"), ("Terry", "9")],
                Some(139),
            ),
            report(
                "2025-03-18",
                "20250319000000",
                &[("Gaines", "131"), ("Terry", "9")],
                Some(140),
            ),
        ])
        .unwrap();
        let s = derive(&v, &lookup()).unwrap();
        // First report's week cannot be apportioned; later weeks are differences of the
        // last report of each week: week 11 = (130 - 110), week 12 = (131 - 130).
        assert_eq!(
            weekly(&s, fips(48165)),
            vec![
                ("2025-W10".into(), nr()),
                ("2025-W11".into(), rep(20)),
                ("2025-W12".into(), rep(1)),
            ]
        );
        // Terry is absent from the first table, whose rows add up to its Total: a real zero,
        // so week 11 is 9 - 2 = 7.
        assert_eq!(weekly(&s, fips(48445))[1], ("2025-W11".into(), rep(7)));
        // Intervals are per report pair, exact.
        let terry: Vec<_> = s
            .intervals
            .iter()
            .filter(|i| i.geography == fips(48445))
            .map(|i| (i.from_report.to_string(), i.new_cases))
            .collect();
        assert_eq!(terry[0], ("2025-03-04".into(), rep(2)));
        assert_eq!(terry.len(), 4);
        // Counties never listed get no rows.
        assert!(weekly(&s, fips(48141)).is_empty());
    }

    #[test]
    fn a_week_without_a_report_is_not_reported_and_the_week_after_a_gap_is_ambiguous() {
        // Weeks 10, 11 and 13 have reports; week 12 has none.
        let v = group_vintages(vec![
            report(
                "2025-03-04",
                "20250305000000",
                &[("Gaines", "100")],
                Some(100),
            ),
            report(
                "2025-03-11",
                "20250312000000",
                &[("Gaines", "120")],
                Some(120),
            ),
            report(
                "2025-03-25",
                "20250326000000",
                &[("Gaines", "150")],
                Some(150),
            ),
        ])
        .unwrap();
        let s = derive(&v, &lookup()).unwrap();
        assert_eq!(
            weekly(&s, fips(48165)),
            vec![
                ("2025-W10".into(), nr()),
                ("2025-W11".into(), rep(20)),
                ("2025-W12".into(), nr()),
                ("2025-W13".into(), amb()),
            ]
        );
        // The exact increase over the gap stays available as an interval.
        assert_eq!(s.intervals.last().unwrap().new_cases, rep(30));
        // The gap week cites the reports on both sides of it.
        let gap = s.weekly.iter().find(|w| w.week.week == 12).unwrap();
        assert_eq!(gap.provenance.as_slice().len(), 2);
    }

    #[test]
    fn a_falling_cumulative_is_ambiguous_not_negative() {
        let v = group_vintages(vec![
            report(
                "2025-03-04",
                "20250305000000",
                &[("Lubbock", "11")],
                Some(11),
            ),
            report(
                "2025-03-11",
                "20250312000000",
                &[("Lubbock", "10")],
                Some(10),
            ),
        ])
        .unwrap();
        let s = derive(&v, &lookup()).unwrap();
        assert_eq!(s.intervals[0].new_cases, amb());
        assert_eq!(weekly(&s, fips(48303))[1], ("2025-W11".into(), amb()));
    }

    #[test]
    fn an_absent_county_is_zero_only_when_the_table_adds_up() {
        // Second table's rows (5 + 2 = 7) do not match its printed Total (9): Terry's absence
        // from it cannot be read as zero.
        let v = group_vintages(vec![
            report(
                "2025-03-04",
                "20250305000000",
                &[("Gaines", "5"), ("Terry", "1")],
                Some(6),
            ),
            report(
                "2025-03-11",
                "20250312000000",
                &[("Gaines", "5"), ("Lubbock", "2")],
                Some(9),
            ),
        ])
        .unwrap();
        let s = derive(&v, &lookup()).unwrap();
        let lubbock = s
            .intervals
            .iter()
            .find(|i| i.geography == fips(48303))
            .unwrap();
        // Lubbock was absent from the consistent first table: a real zero, so +2.
        assert_eq!(lubbock.new_cases, rep(2));
        let terry = s
            .intervals
            .iter()
            .find(|i| i.geography == fips(48445))
            .unwrap();
        assert_eq!(terry.new_cases, amb());
    }

    #[test]
    fn unmappable_and_unreadable_entries_stay_visible() {
        let v = group_vintages(vec![
            report(
                "2025-03-04",
                "20250305000000",
                &[("Gaines", "5"), ("Tarrent", "1"), ("Terry", "many")],
                Some(6),
            ),
            report(
                "2025-03-11",
                "20250312000000",
                &[("Gaines", "6"), ("Terry", "1")],
                Some(7),
            ),
        ])
        .unwrap();
        let s = derive(&v, &lookup()).unwrap();
        assert_eq!(s.unmapped.len(), 1);
        assert_eq!(s.unmapped[0].name, "Tarrent");
        assert_eq!(s.unmapped[0].reason, "not a county name in the Census file");
        // Terry's cell was unreadable in the first report: ambiguous, not zero.
        let terry = s
            .intervals
            .iter()
            .find(|i| i.geography == fips(48445))
            .unwrap();
        assert_eq!(terry.new_cases, amb());
        // With an unmapped name the table is not provably complete: absent counties are
        // ambiguous, never zero.
        assert!(s.cumulative.iter().any(|c| c.cases == amb()));
    }

    #[test]
    fn county_names_map_only_through_the_census_file() {
        let l = lookup();
        assert_eq!(l.lookup("El Paso").unwrap().code(), 48141);
        assert_eq!(l.lookup(" el paso county ").unwrap().code(), 48141);
        assert_eq!(l.lookup("Gaines").unwrap().code(), 48165);
        assert_eq!(l.lookup("Narnia"), Err(LookupError::NotFound));
        assert_eq!(l.len(), 4);
    }

    #[test]
    fn reports_without_county_detail_are_in_the_manifest_but_not_the_series() {
        let mut narrative = report("2025-04-22", "20250424000000", &[], Some(624));
        narrative.format = ReportFormat::HtmlNarrativeOnly;
        narrative.outbreak_counties = None;
        let v = group_vintages(vec![
            report("2025-03-04", "20250305000000", &[("Gaines", "5")], Some(5)),
            narrative,
        ])
        .unwrap();
        let m = manifest(&v);
        assert_eq!(m.entries.len(), 2);
        assert!(!m.entries[1].county_detail);
        assert_eq!(m.entries[1].outbreak_total, Some(624));
        let s = derive(&v, &lookup()).unwrap();
        assert_eq!(s.cumulative.len(), 1);
    }

    #[test]
    fn a_week_whose_last_report_has_no_county_breakdown_gets_no_partial_count() {
        // Tue 03-25 (week 13) has county rows; Fri 03-28 (also week 13) is dashboard-only.
        let mut dashboard = report("2025-03-28", "20250329000000", &[], Some(400));
        dashboard.format = ReportFormat::HtmlNarrativeOnly;
        dashboard.outbreak_counties = None;
        let v = group_vintages(vec![
            report(
                "2025-03-18",
                "20250319000000",
                &[("Gaines", "100")],
                Some(100),
            ),
            report(
                "2025-03-25",
                "20250326000000",
                &[("Gaines", "150")],
                Some(150),
            ),
            dashboard,
        ])
        .unwrap();
        let s = derive(&v, &lookup()).unwrap();
        assert_eq!(
            weekly(&s, fips(48165)),
            vec![("2025-W12".into(), nr()), ("2025-W13".into(), amb())]
        );
        // The Tue-to-Tue interval is still exact.
        assert_eq!(s.intervals[0].new_cases, rep(50));
    }
}
