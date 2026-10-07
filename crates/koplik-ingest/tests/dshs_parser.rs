//! Offline parser tests for the Texas DSHS outbreak reports, against the real bytes in
//! `data/fixtures/dshs/` (see its README for what each one is).

use std::path::PathBuf;

use koplik_contracts::v1::{CaseCount, CountyFips, GeoId, MissingReason, MmwrWeek, StateFips};
use koplik_ingest::census_counties::CountyLookup;
use koplik_ingest::dshs::{self, CountyTable, Report, ReportFormat};
use koplik_ingest::dshs_series::{self, VintageManifest};
use koplik_ingest::store::{Retrieval, RetrievalMeta, SnapshotStore, sha256_of};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/fixtures/dshs")
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn retrieval(name: &str) -> Retrieval {
    let line = std::fs::read_to_string(dir().join(format!("{name}.retrieval.json"))).unwrap();
    serde_json::from_str(line.trim()).unwrap()
}

fn report(name: &str) -> Report {
    dshs::parse_report(&bytes(name), &retrieval(name)).unwrap()
}

fn counts(t: &CountyTable) -> Vec<(&str, u32)> {
    t.entries
        .iter()
        .map(|e| (e.name.as_str(), e.cases.unwrap()))
        .collect()
}

fn value(t: &CountyTable, county: &str) -> u32 {
    t.entries
        .iter()
        .find(|e| e.name == county)
        .unwrap_or_else(|| panic!("no row for {county}"))
        .cases
        .unwrap()
}

fn texas() -> StateFips {
    StateFips::new(48).unwrap()
}

fn census() -> CountyLookup {
    CountyLookup::parse(
        &bytes("census-national_county2020.txt"),
        texas(),
        retrieval("census-national_county2020.txt"),
    )
    .unwrap()
}

#[test]
fn fixtures_are_the_recorded_snapshots() {
    let tsv = std::fs::read_to_string(dir().join("fixtures.tsv")).unwrap();
    let mut n = 0;
    for line in tsv.lines().filter(|l| !l.trim().is_empty()) {
        let (name, sha) = line.split_once('\t').unwrap();
        let b = bytes(name);
        assert_eq!(sha256_of(&b).as_str(), sha, "{name} bytes");
        assert_eq!(retrieval(name).sha256.as_str(), sha, "{name} record");
        assert_eq!(retrieval(name).bytes as usize, b.len(), "{name} size");
        n += 1;
    }
    assert_eq!(n, 11);
}

#[test]
fn format_1_html_county_table_first_version() {
    let r = report("page-2025-03-04.html");
    assert_eq!(r.format, ReportFormat::HtmlCountyTable);
    assert_eq!(r.report_date.to_string(), "2025-03-04");
    assert_eq!(r.outbreak_total, Some(159));
    let t = r.outbreak_counties.as_ref().unwrap();
    assert_eq!(t.caption, "Texas Case Count by County");
    assert_eq!(
        counts(t),
        vec![
            ("Dallam", 4),
            ("Dawson", 9),
            ("Ector", 2),
            ("Gaines", 107),
            ("Lubbock", 3),
            ("Lynn", 2),
            ("Martin", 3),
            ("Terry", 22),
            ("Yoakum", 7),
        ]
    );
    assert_eq!((t.total, t.sum()), (Some(159), Some(159)));
    assert_eq!(r.other_tables.len(), 1);
    assert_eq!(
        counts(&r.other_tables[0]),
        vec![("Harris", 2), ("Rockwall", 1), ("Travis", 1)]
    );
    assert!(r.issues.is_empty(), "{:?}", r.issues);
}

#[test]
fn format_1_html_county_table_later_caption_and_gzip_body() {
    // The Archive replays this capture gzip-encoded; the snapshot keeps those bytes.
    let raw = bytes("page-2025-03-25.html.gz");
    assert_eq!(&raw[..2], &[0x1f, 0x8b]);
    let r = report("page-2025-03-25.html.gz");
    assert_eq!(r.format, ReportFormat::HtmlCountyTable);
    assert_eq!(r.report_date.to_string(), "2025-03-25");
    assert_eq!(r.outbreak_total, Some(327));
    let t = r.outbreak_counties.as_ref().unwrap();
    assert_eq!(t.caption, "Texas Outbreak Case Count by County");
    assert_eq!(t.entries.len(), 15);
    assert_eq!(value(t, "Gaines"), 226);
    assert_eq!(value(t, "Terry"), 37);
    assert_eq!(value(t, "Lubbock"), 10);
    assert_eq!((t.total, t.sum()), (Some(327), Some(327)));
    assert!(r.issues.is_empty(), "{:?}", r.issues);
}

#[test]
fn format_2_dashboard_pages_have_the_total_but_no_county_breakdown() {
    let first = report("page-2025-03-28.html.gz");
    assert_eq!(first.format, ReportFormat::HtmlNarrativeOnly);
    assert_eq!(first.report_date.to_string(), "2025-03-28");
    assert_eq!(first.outbreak_total, Some(400));
    // Missing stays missing: no county table is invented from the dashboard.
    assert!(first.outbreak_counties.is_none());
    assert!(first.issues.is_empty(), "{:?}", first.issues);

    let april = report("page-2025-04-22.html");
    assert_eq!(april.format, ReportFormat::HtmlNarrativeOnly);
    assert_eq!(april.report_date.to_string(), "2025-04-22");
    assert_eq!(april.outbreak_total, Some(624));
    assert!(april.outbreak_counties.is_none());

    let last = report("page-live-2026-10-07.html");
    assert_eq!(last.report_date.to_string(), "2025-08-12");
    assert_eq!(last.outbreak_total, Some(762));
    assert!(last.outbreak_counties.is_none());
    assert_eq!(last.other_tables[0].total, Some(39));
    assert_eq!(last.other_tables[0].entries.len(), 19);
}

#[test]
fn footnote_markers_stay_in_the_raw_cell_and_do_not_change_the_count() {
    let april = report("page-2025-04-22.html");
    let other = &april.other_tables[0];
    let upshur = other.entries.iter().find(|e| e.name == "Upshur").unwrap();
    assert_eq!((upshur.raw.as_str(), upshur.cases), ("15**", Some(15)));
    let harris = other.entries.iter().find(|e| e.name == "Harris").unwrap();
    assert_eq!((harris.raw.as_str(), harris.cases), ("4*", Some(4)));
    assert_eq!(other.total, Some(25));
}

#[test]
fn a_table_whose_rows_do_not_add_to_its_total_is_flagged_not_corrected() {
    let r = report("page-2025-05-30.html.gz");
    let t = &r.other_tables[0];
    assert_eq!((t.sum(), t.total), (Some(35), Some(32)));
    assert!(
        r.issues
            .iter()
            .any(|i| i.contains("rows sum to 35") && i.contains("total is 32")),
        "{:?}",
        r.issues
    );
    assert_eq!(r.outbreak_total, Some(738));
}

#[test]
fn format_3_pdf_reports_place_values_in_the_right_column() {
    for (name, date, as_of) in [
        (
            "report-2025-11-24.pdf",
            "2025-11-24",
            "11/24/2025, 6:26:00 PM",
        ),
        (
            "report-2025-12-23.pdf",
            "2025-12-23",
            "12/17/2025, 11:00:00 AM",
        ),
        (
            "report-2026-01-12.pdf",
            "2026-01-12",
            "1/12/2026, 2:00:00 PM",
        ),
    ] {
        let r = report(name);
        assert_eq!(r.format, ReportFormat::PdfDataReport, "{name}");
        assert_eq!(r.report_date.to_string(), date, "{name}");
        assert!(r.data_as_of.as_deref().unwrap().ends_with(as_of), "{name}");
        assert_eq!(r.outbreak_total, Some(762), "{name}");
        let t = r.outbreak_counties.as_ref().unwrap();
        assert_eq!(t.entries.len(), 38, "{name}");
        assert_eq!(value(t, "Gaines"), 414);
        assert_eq!(value(t, "El Paso"), 59);
        assert_eq!(value(t, "Terry"), 60);
        // Rockwall appears in the outbreak column and the travel column; Midland in the
        // outbreak column and the "other Texas cases" column: positions, not order, decide.
        assert_eq!(value(t, "Rockwall"), 1);
        assert_eq!(value(t, "Midland"), 6);
        let travel = r
            .other_tables
            .iter()
            .find(|t| t.caption.contains("International Travel"))
            .unwrap();
        let other = r
            .other_tables
            .iter()
            .find(|t| t.caption.contains("Other Texas Cases"))
            .unwrap();
        assert_eq!(travel.entries.len(), 9, "{name}");
        assert_eq!(travel.total, Some(16));
        assert_eq!(value(travel, "Williamson"), 6);
        assert_eq!(value(travel, "Rockwall"), 1);
        assert!(!travel.entries.iter().any(|e| e.name == "Gaines"));
        assert_eq!(other.entries.len(), 15, "{name}");
        assert_eq!(other.total, Some(25));
        assert_eq!(value(other, "Midland"), 2);
        assert_eq!(value(other, "Harris"), 4);
        assert!(!other.entries.iter().any(|e| e.name == "Cochran"));
        // DSHS's own table does not add up: the outbreak rows sum to 763 against a printed
        // Grand Total of 762. Reported as published and flagged, not reconciled.
        assert_eq!((t.sum(), t.total), (Some(763), Some(762)), "{name}");
        assert!(
            r.issues.iter().any(|i| i.contains("rows sum to 763")),
            "{name}: {:?}",
            r.issues
        );
        assert!(
            r.issues.iter().any(|i| i.contains("182 potential cases")),
            "{name}"
        );
    }
}

#[test]
fn county_names_map_to_fips_through_the_census_file() {
    let l = census();
    assert_eq!(l.len(), 254);
    assert_eq!(l.lookup("Gaines").unwrap().code(), 48165);
    assert_eq!(l.lookup("El Paso").unwrap().code(), 48141);
    assert_eq!(l.lookup("McLennan").unwrap().code(), 48309);
    assert_eq!(l.lookup("Fort Bend").unwrap().code(), 48157);
    assert_eq!(l.retrieval.source_id, "census-county-codes-2020-wayback");
    // Every county name DSHS printed in any fixture maps.
    for name in [
        "page-2025-03-04.html",
        "page-2025-03-25.html.gz",
        "report-2026-01-12.pdf",
    ] {
        let r = report(name);
        for t in std::iter::once(r.outbreak_counties.as_ref().unwrap()).chain(r.other_tables.iter())
        {
            for e in &t.entries {
                assert!(l.lookup(&e.name).is_ok(), "{name}: {} did not map", e.name);
            }
        }
    }
    for name in ["page-2025-04-22.html", "page-live-2026-10-07.html"] {
        for e in &report(name).other_tables[0].entries {
            assert!(l.lookup(&e.name).is_ok(), "{name}: {} did not map", e.name);
        }
    }
}

/// A store holding every fixture, recorded with its own retrieval fields.
fn store_with_fixtures() -> (tempfile::TempDir, SnapshotStore) {
    let dir_ = tempfile::tempdir().unwrap();
    let store = SnapshotStore::open(dir_.path()).unwrap();
    let tsv = std::fs::read_to_string(dir().join("fixtures.tsv")).unwrap();
    for line in tsv.lines().filter(|l| !l.trim().is_empty()) {
        let name = line.split_once('\t').unwrap().0;
        if name.starts_with("cdx-") {
            continue;
        }
        let r = retrieval(name);
        store
            .record(
                RetrievalMeta {
                    source_id: r.source_id,
                    url: r.url,
                    retrieved_at: r.retrieved_at,
                    licence_id: r.licence_id,
                    http_status: r.http_status,
                    content_type: r.content_type,
                },
                &bytes(name),
            )
            .unwrap();
    }
    (dir_, store)
}

fn fips(code: u32) -> GeoId {
    GeoId::County(CountyFips::new(code).unwrap())
}

#[test]
fn derived_series_from_the_fixtures_with_provenance() {
    let (_d, store) = store_with_fixtures();
    let built =
        dshs_series::build_from_store(&store, &CountyLookup::from_store(&store, texas()).unwrap())
            .unwrap();
    assert!(built.failures.is_empty(), "{:?}", built.failures);
    assert!(built.series.unmapped.is_empty());
    // Vintages: 03-04, 03-25, 03-28, 04-22, 05-30, 08-12 (live), and the three PDFs.
    let dates: Vec<String> = built
        .vintages
        .iter()
        .map(|v| v.report.report_date.to_string())
        .collect();
    assert_eq!(
        dates,
        [
            "2025-03-04",
            "2025-03-25",
            "2025-03-28",
            "2025-04-22",
            "2025-05-30",
            "2025-08-12",
            "2025-11-24",
            "2025-12-23",
            "2026-01-12"
        ]
    );
    // The manifest says which versions have county detail and when each was first seen.
    let m = &built.manifest;
    assert_eq!(m.entries.iter().filter(|e| e.county_detail).count(), 5);
    let mar25 = &m.entries[1];
    assert!(
        mar25
            .first_seen_at
            .to_rfc3339()
            .starts_with("2025-03-26T07:56:")
    );
    assert!(
        mar25
            .first_snapshot
            .url
            .starts_with("https://web.archive.org/web/20250326")
    );
    assert!(
        mar25
            .first_snapshot
            .url
            .ends_with("/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025")
            || mar25
                .first_snapshot
                .url
                .ends_with("/https://dshs.texas.gov/news-alerts/measles-outbreak-2025")
    );
    assert_eq!(
        mar25.first_snapshot.original_url,
        mar25.first_snapshot.url.split("id_/").nth(1).unwrap()
    );
    assert!(mar25.first_snapshot.capture_time.is_some());

    // Cumulative Gaines as published, with its provenance.
    let gaines = fips(48165);
    let cum: Vec<_> = built
        .series
        .cumulative
        .iter()
        .filter(|c| c.geography == gaines)
        .collect();
    assert_eq!(
        cum.iter()
            .map(|c| (c.report_date.to_string(), c.cases))
            .collect::<Vec<_>>(),
        [
            ("2025-03-04".to_owned(), CaseCount::Reported { count: 107 }),
            ("2025-03-25".to_owned(), CaseCount::Reported { count: 226 }),
            ("2025-11-24".to_owned(), CaseCount::Reported { count: 414 }),
            ("2025-12-23".to_owned(), CaseCount::Reported { count: 414 }),
            ("2026-01-12".to_owned(), CaseCount::Reported { count: 414 }),
        ]
    );
    let p = &cum[0].provenance.as_slice()[0];
    assert_eq!(
        p.sha256.as_str(),
        "f808ad80e3f32b6163aeddcb58c552598a10c28aa0c9c4dac7437d86921380b7"
    );
    assert!(
        p.url
            .starts_with("https://web.archive.org/web/20250305184528id_/")
    );
    // Intervals are exact between consecutive county-detail reports, however far apart.
    let iv: Vec<_> = built
        .series
        .intervals
        .iter()
        .filter(|i| i.geography == gaines)
        .collect();
    assert_eq!(iv[0].new_cases, CaseCount::Reported { count: 119 });
    assert_eq!(iv[1].new_cases, CaseCount::Reported { count: 188 });
    assert_eq!(iv[2].new_cases, CaseCount::Reported { count: 0 });
    // Weekly: MMWR week 10 (03-04) has no earlier report to difference against; week 13 has
    // a report but the previous one is three weeks back (not split); weeks with no county
    // report are not_reported; 2025 week 48 is the PDF's week after a seven-month gap.
    let wk = |y: u16, w: u8| {
        built
            .series
            .weekly
            .iter()
            .find(|r| r.geography == gaines && r.week == MmwrWeek::new(y, w).unwrap())
            .unwrap()
            .cases
    };
    let nr = CaseCount::Missing {
        reason: MissingReason::NotReported,
    };
    let amb = CaseCount::Missing {
        reason: MissingReason::Ambiguous,
    };
    assert_eq!(wk(2025, 10), nr);
    assert_eq!(wk(2025, 11), nr);
    assert_eq!(wk(2025, 13), amb);
    assert_eq!(wk(2025, 14), nr);
    assert_eq!(wk(2025, 48), amb);
    assert_eq!(wk(2025, 49), nr);
    assert!(
        built
            .series
            .weekly
            .iter()
            .all(|w| w.case_definition == koplik_contracts::v3::CaseDefinition::Confirmed)
    );
    // Weekly rows round-trip through the contracts type.
    let json = serde_json::to_string(&built.series.weekly).unwrap();
    let back: Vec<koplik_contracts::v3::WeeklyCaseCount> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, built.series.weekly);
}

#[test]
fn the_committed_manifest_matches_what_the_fixtures_say() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/dshs/vintage-manifest.json");
    let committed: VintageManifest =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(committed.manifest_version, dshs_series::MANIFEST_VERSION);
    assert!(
        committed
            .entries
            .windows(2)
            .all(|w| w[0].report_date <= w[1].report_date)
    );
    for e in &committed.entries {
        assert!(e.first_seen_at <= e.last_seen_at);
        assert!(e.snapshot_count >= 1);
        assert_eq!(e.first_snapshot.sha256.as_str().len(), 64);
    }
    // Entries whose first snapshot is a fixture agree with what the fixture parses to.
    let tsv = std::fs::read_to_string(dir().join("fixtures.tsv")).unwrap();
    let mut checked = 0;
    for line in tsv.lines().filter(|l| !l.trim().is_empty()) {
        let (name, sha) = line.split_once('\t').unwrap();
        let Some(e) = committed
            .entries
            .iter()
            .find(|e| e.first_snapshot.sha256.as_str() == sha)
        else {
            continue;
        };
        let r = report(name);
        assert_eq!(e.report_date, r.report_date, "{name}");
        assert_eq!(e.format, r.format, "{name}");
        assert_eq!(e.outbreak_total, r.outbreak_total, "{name}");
        assert_eq!(e.county_detail, r.outbreak_counties.is_some(), "{name}");
        checked += 1;
    }
    assert!(
        checked >= 7,
        "only {checked} fixtures matched a manifest entry"
    );
    // Every report date DSHS published a county table for is present.
    let detail: Vec<String> = committed
        .entries
        .iter()
        .filter(|e| e.county_detail)
        .map(|e| e.report_date.to_string())
        .collect();
    assert_eq!(
        detail,
        [
            "2025-03-04",
            "2025-03-07",
            "2025-03-11",
            "2025-03-14",
            "2025-03-18",
            "2025-03-21",
            "2025-03-25",
            "2025-11-24",
            "2025-12-23",
            "2026-01-12"
        ]
    );
}

#[test]
fn counts_are_called_confirmed_only_where_dshs_labels_them_so() {
    // PDF: the table's own title. HTML: a "... Confirmed Cases ..." table adding to the total.
    let pdf = report("report-2026-01-12.pdf");
    assert!(
        pdf.confirmed_basis
            .as_deref()
            .unwrap()
            .contains("Confirmed Cases in Texas Residents")
    );
    for name in ["page-2025-03-04.html", "page-2025-03-25.html.gz"] {
        let basis = report(name).confirmed_basis;
        assert!(
            basis
                .as_deref()
                .is_some_and(|b| b.contains("Confirmed Cases")),
            "{name}: {basis:?}"
        );
    }
    // The dashboard-only page has no such table: not established, so not labelled confirmed
    // (it has no county rows to label anyway).
    assert_eq!(report("page-2025-03-28.html.gz").confirmed_basis, None);
}

/// The Mar 4 page re-parsed from edited bytes, standing in for a later capture of a revised
/// version; `capture` is its 14-digit Archive capture time.
fn revised(from: &[u8], find: &str, replace: &str, capture: &str) -> Report {
    let text = std::str::from_utf8(from).unwrap();
    assert!(text.contains(find), "{find:?} not in the fixture");
    let mut r = retrieval("page-2025-03-04.html");
    r.url = format!(
        "https://web.archive.org/web/{capture}id_/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025"
    );
    dshs::parse_html_report(text.replace(find, replace).as_bytes(), &r).unwrap()
}

#[test]
fn a_revised_version_is_never_merged_into_an_earlier_one() {
    let base_bytes = bytes("page-2025-03-04.html");
    let original = report("page-2025-03-04.html");

    // Identical content captured again is the same version, kept as a second snapshot.
    let again = revised(
        &base_bytes,
        "Texas Case Count by County",
        "Texas Case Count by County",
        "20250306000000",
    );
    let v = dshs_series::group_vintages(vec![original.clone(), again]).unwrap();
    assert_eq!((v.len(), v[0].snapshots.len()), (1, 2));

    // DSHS drops the "Confirmed Cases" label: a different version with its own (absent) basis.
    let unlabelled = revised(
        &base_bytes,
        "Vaccination Status of Confirmed Cases",
        "Vaccination Status of Cases",
        "20250307000000",
    );
    assert!(original.confirmed_basis.is_some() && unlabelled.confirmed_basis.is_none());
    let v = dshs_series::group_vintages(vec![original.clone(), unlabelled]).unwrap();
    assert_eq!(v.len(), 2);
    assert!(v[0].is_confirmed() && !v[1].is_confirmed());

    // The printed Total changes from 159 to 160: a different version that keeps its own
    // total and its own parser warnings.
    let bumped = revised(
        &base_bytes,
        "<strong>159</strong>",
        "<strong>160</strong>",
        "20250308000000",
    );
    assert_eq!(bumped.outbreak_counties.as_ref().unwrap().total, Some(160));
    assert!(!bumped.issues.is_empty());
    let v = dshs_series::group_vintages(vec![original.clone(), bumped]).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(
        v[0].report.outbreak_counties.as_ref().unwrap().total,
        Some(159)
    );
    assert!(v[0].report.issues.is_empty());
    assert_eq!(
        v[1].report.outbreak_counties.as_ref().unwrap().total,
        Some(160)
    );
    assert!(
        v[1].report.issues.iter().any(|i| i.contains("160")),
        "{:?}",
        v[1].report.issues
    );
    let m = dshs_series::manifest(&v);
    assert_eq!(m.entries.len(), 2);
}

#[test]
fn derived_rows_and_the_manifest_cite_the_census_snapshot_that_keyed_the_counties() {
    let (_d, store) = store_with_fixtures();
    let lookup = CountyLookup::from_store(&store, texas()).unwrap();
    let built = dshs_series::build_from_store(&store, &lookup).unwrap();
    let census = retrieval("census-national_county2020.txt");
    let linked = |p: &koplik_contracts::v3::Provenances| {
        p.as_slice().iter().any(|r| {
            r.sha256 == census.sha256
                && r.url == census.url
                && r.retrieved_at == census.retrieved_at
                && r.source_id == "census-county-codes-2020-wayback"
        })
    };
    assert!(
        census
            .url
            .starts_with("https://web.archive.org/web/20250206022004id_/https://www2.census.gov/")
    );
    assert!(!built.series.weekly.is_empty());
    assert!(built.series.weekly.iter().all(|w| linked(&w.provenance)));
    assert!(
        built
            .series
            .cumulative
            .iter()
            .all(|c| linked(&c.provenance))
    );
    assert!(built.series.intervals.iter().all(|i| linked(&i.provenance)));
    // The DSHS snapshot is still cited next to it.
    assert!(built.series.cumulative[0].provenance.as_slice().len() >= 2);
    let pin = built.manifest.county_lookup.as_ref().unwrap();
    assert_eq!(pin.sha256, census.sha256);
    assert_eq!(pin.url, census.url);
    assert_eq!(
        pin.capture_time.unwrap().to_rfc3339(),
        "2025-02-06T02:20:04+00:00"
    );
    assert_eq!(
        pin.original_url,
        "https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt"
    );
}
