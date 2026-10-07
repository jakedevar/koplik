//! Parsers for the Texas DSHS 2025 West Texas measles outbreak reports, one per format DSHS
//! used during 2025. Every parser reads stored snapshot bytes only (offline) and keeps what
//! it cannot read visible in [`Report::issues`] or as `cases: None`; nothing is dropped,
//! guessed or defaulted to zero.
//!
//! Formats (see `SOURCES.md` for the dates and the evidence):
//!
//! 1. **`HtmlCountyTable`**: the outbreak page carries a table captioned "Texas Case Count by
//!    County" (County | Cases, cumulative cases of the outbreak, with a Total row) and a
//!    second table, "2025 Texas Measles Cases Not Associated with the Outbreak in West Texas".
//!    First seen in the Internet Archive on 2025-03-05.
//! 2. **`HtmlNarrativeOnly`**: from late April the page shows the outbreak counties in an
//!    embedded Tableau dashboard (`tabexternal.dshs.texas.gov`, which refuses automated
//!    requests and is not archived), so the page text has only the cumulative outbreak total
//!    ("At this time, N cases have been confirmed since late January") and the table of
//!    cases not associated with the outbreak. County detail for these vintages is missing,
//!    and stays missing.
//! 3. **`PdfDataReport`**: the "2025 Measles Data Report" PDFs (from 2025-11-24) with "Table 1:
//!    Confirmed Cases in Texas Residents": Home County | 2025 Texas Outbreak | International
//!    Travel | Other Texas Cases, each cell `N (percent)`. Columns are identified by the
//!    right edge of each cell's text against the header row, never by order of appearance.
//!
//! The report date is the date DSHS states (the page's "News Updates" date, the PDF title
//! date), not the capture date. Capture time is a separate fact kept in provenance.

use std::io::Read;

use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::error::{IngestError, Result};
use crate::store::Retrieval;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    HtmlCountyTable,
    HtmlNarrativeOnly,
    PdfDataReport,
}

/// One county row exactly as published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountyEntry {
    /// County name as printed (e.g. `Fort Bend`), whitespace-trimmed only.
    pub name: String,
    /// The cell text as printed (e.g. `4*`, `15**`, `414 (51.56)`).
    pub raw: String,
    /// The case count, or `None` when the cell is not a plain whole number (with optional
    /// `*` footnote markers and, in the PDF, a trailing `(percent)`).
    pub cases: Option<u32>,
}

/// A county table as published: its rows and its own Total row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountyTable {
    pub caption: String,
    pub entries: Vec<CountyEntry>,
    /// The printed Total (HTML) or Grand Total (PDF), when present and readable.
    pub total: Option<u32>,
}

impl CountyTable {
    /// Sum of the readable rows, or `None` when any row is unreadable.
    pub fn sum(&self) -> Option<u32> {
        self.entries.iter().map(|e| e.cases).sum()
    }
}

/// One DSHS report version, as published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub format: ReportFormat,
    /// The date DSHS states for this version.
    pub report_date: NaiveDate,
    /// The PDF's "Preliminary Data as of" text, verbatim.
    pub data_as_of: Option<String>,
    /// Cumulative outbreak cases as printed in the narrative, or the PDF's outbreak Grand Total.
    pub outbreak_total: Option<u32>,
    /// The outbreak cases by county; `None` when this version has no readable county table.
    pub outbreak_counties: Option<CountyTable>,
    /// Every other county table found (cases not associated with the outbreak, travel, other).
    pub other_tables: Vec<CountyTable>,
    /// Everything that could not be read or does not add up, verbatim and visible.
    pub issues: Vec<String>,
    /// The snapshot this report was read from.
    pub retrieval: Retrieval,
}

/// Archived captures can arrive gzip-encoded (the Archive replays the stored bytes with their
/// original `Content-Encoding`); the snapshot keeps the bytes exactly as received.
pub fn decode_body(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut out)
            .map_err(|e| IngestError::Parse(format!("gzip body: {e}")))?;
        Ok(out)
    } else {
        Ok(bytes.to_vec())
    }
}

/// Parse one stored snapshot as whichever format it is.
pub fn parse_report(bytes: &[u8], retrieval: &Retrieval) -> Result<Report> {
    let body = decode_body(bytes)?;
    if body.starts_with(b"%PDF") {
        parse_pdf_report(&body, retrieval)
    } else {
        parse_html_report(&body, retrieval)
    }
}

// ---------------------------------------------------------------------------------------
// HTML

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("static selector parses")
}

fn text_of(e: ElementRef<'_>) -> String {
    e.text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `March 4, 2025` -> date.
fn parse_long_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%B %e, %Y")
        .or_else(|_| NaiveDate::parse_from_str(s.trim(), "%B %d, %Y"))
        .ok()
}

/// A cell like `4`, `4*`, `15**` (HTML) or `414 (51.56)` (PDF): the whole-number case count.
/// `None` for anything else. Footnote markers are `*` only; the marker text stays in `raw`.
pub fn parse_count_cell(raw: &str) -> Option<u32> {
    let s = raw.trim();
    let s = s.split_once(" (").map_or(s, |(n, rest)| {
        // The PDF's trailing "(percent)": require it to close properly.
        if rest.ends_with(')') { n } else { s }
    });
    let digits = s.trim_end_matches('*');
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn parse_html_table(table: ElementRef<'_>) -> Option<CountyTable> {
    let caption = table
        .select(&sel("caption"))
        .next()
        .map(text_of)
        .unwrap_or_default();
    let rows: Vec<Vec<String>> = table
        .select(&sel("tr"))
        .map(|tr| {
            tr.select(&sel("th, td"))
                .map(text_of)
                .collect::<Vec<String>>()
        })
        .collect();
    let header = rows.first()?;
    // A county table has County as its first column header and one count column.
    if header.len() != 2 || !header[0].eq_ignore_ascii_case("county") {
        return None;
    }
    let mut entries = Vec::new();
    let mut total = None;
    for row in &rows[1..] {
        if row.len() != 2 {
            entries.push(CountyEntry {
                name: row.first().cloned().unwrap_or_default(),
                raw: row.join(" | "),
                cases: None,
            });
            continue;
        }
        if row[0].eq_ignore_ascii_case("total") {
            total = parse_count_cell(&row[1]);
            continue;
        }
        entries.push(CountyEntry {
            name: row[0].clone(),
            raw: row[1].clone(),
            cases: parse_count_cell(&row[1]),
        });
    }
    Some(CountyTable {
        caption,
        entries,
        total,
    })
}

/// Cumulative outbreak total from the narrative: "At this time, 709 cases have been confirmed
/// since late January" / "159 cases have been identified". Exactly one match is accepted;
/// zero or several is reported, not guessed.
fn narrative_total(text: &str, issues: &mut Vec<String>) -> Option<u32> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut found = Vec::new();
    for w in words.windows(5) {
        // "<N> cases have been confirmed|identified"
        if w[1] == "cases"
            && w[2] == "have"
            && w[3] == "been"
            && (w[4].starts_with("confirmed") || w[4].starts_with("identified"))
            && let Ok(n) = w[0].trim_start_matches(',').replace(',', "").parse::<u32>()
        {
            found.push(n);
        }
    }
    match found.as_slice() {
        [n] => Some(*n),
        [] => {
            issues.push("no \"N cases have been confirmed/identified\" sentence found".into());
            None
        }
        many => {
            issues.push(format!("several case totals in the narrative: {many:?}"));
            None
        }
    }
}

pub fn parse_html_report(body: &[u8], retrieval: &Retrieval) -> Result<Report> {
    let html = std::str::from_utf8(body)
        .map_err(|e| IngestError::Parse(format!("page is not UTF-8: {e}")))?;
    let doc = Html::parse_document(html);
    let mut issues = Vec::new();

    let date_el = doc
        .select(&sel("div.field--name-field-news-date"))
        .next()
        .map(text_of)
        .ok_or_else(|| IngestError::Parse("no News Updates date element on the page".into()))?;
    let report_date = parse_long_date(&date_el)
        .ok_or_else(|| IngestError::Parse(format!("unreadable report date {date_el:?}")))?;

    let content = doc
        .select(&sel("div.field--name-field-content-block"))
        .next()
        .ok_or_else(|| IngestError::Parse("no content block on the page".into()))?;
    let narrative = text_of(content);
    let narrative_total = narrative_total(&narrative, &mut issues);

    let mut county_tables: Vec<CountyTable> = content
        .select(&sel("table"))
        .filter_map(parse_html_table)
        .collect();

    // The outbreak table is the one captioned "... Case Count by County"; the table of cases
    // not associated with the outbreak says so in its caption.
    let is_outbreak = |t: &CountyTable| {
        let c = t.caption.to_lowercase();
        c.contains("by county") && !c.contains("not associated")
    };
    let n_outbreak = county_tables.iter().filter(|t| is_outbreak(t)).count();
    let outbreak_counties = match n_outbreak {
        0 => None,
        1 => {
            let i = county_tables
                .iter()
                .position(is_outbreak)
                .expect("counted one");
            Some(county_tables.remove(i))
        }
        n => {
            issues.push(format!(
                "{n} tables look like the outbreak county table; none used"
            ));
            None
        }
    };

    let outbreak_total = match (&outbreak_counties, narrative_total) {
        (Some(t), Some(n)) => {
            if t.total.is_some_and(|tt| tt != n) {
                issues.push(format!(
                    "narrative total {n} differs from the table's Total {:?}",
                    t.total
                ));
            }
            Some(n)
        }
        (Some(t), None) => t.total,
        (None, n) => n,
    };
    if let Some(t) = &outbreak_counties {
        check_table(t, &mut issues);
    }
    for t in &county_tables {
        check_table(t, &mut issues);
    }

    if outbreak_counties.is_none() && doc.select(&sel("tableau-viz")).next().is_none() {
        issues.push(
            "no outbreak county table and no embedded dashboard: unrecognised page layout".into(),
        );
    }
    let format = if outbreak_counties.is_some() {
        ReportFormat::HtmlCountyTable
    } else {
        ReportFormat::HtmlNarrativeOnly
    };
    Ok(Report {
        format,
        report_date,
        data_as_of: None,
        outbreak_total,
        outbreak_counties,
        other_tables: county_tables,
        issues,
        retrieval: retrieval.clone(),
    })
}

/// Unreadable rows and a Total that does not equal the sum of the rows stay visible.
fn check_table(t: &CountyTable, issues: &mut Vec<String>) {
    for e in &t.entries {
        if e.cases.is_none() {
            issues.push(format!(
                "table {:?}: unreadable count {:?} for {:?}",
                t.caption, e.raw, e.name
            ));
        }
    }
    if let (Some(sum), Some(total)) = (t.sum(), t.total)
        && sum != total
    {
        issues.push(format!(
            "table {:?}: rows sum to {sum} but the printed total is {total}",
            t.caption
        ));
    }
    if t.total.is_none() {
        issues.push(format!("table {:?}: no readable Total row", t.caption));
    }
}

// ---------------------------------------------------------------------------------------
// PDF

/// A run of characters on one line of one page, with its horizontal extent (PDF points).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub x0: f64,
    pub x1: f64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfLine {
    pub page: u32,
    pub y: f64,
    pub segments: Vec<Segment>,
}

struct CharCollector {
    page: u32,
    // (page, x, y, advance, text)
    chars: Vec<(u32, f64, f64, f64, String)>,
}

impl pdf_extract::OutputDev for CharCollector {
    fn begin_page(
        &mut self,
        page_num: u32,
        _media_box: &pdf_extract::MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> std::result::Result<(), pdf_extract::OutputError> {
        self.page = page_num;
        Ok(())
    }
    fn end_page(&mut self) -> std::result::Result<(), pdf_extract::OutputError> {
        Ok(())
    }
    fn output_character(
        &mut self,
        trm: &pdf_extract::Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        char: &str,
    ) -> std::result::Result<(), pdf_extract::OutputError> {
        self.chars.push((
            self.page,
            trm.m31,
            trm.m32,
            width * font_size,
            char.to_owned(),
        ));
        Ok(())
    }
    fn begin_word(&mut self) -> std::result::Result<(), pdf_extract::OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> std::result::Result<(), pdf_extract::OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> std::result::Result<(), pdf_extract::OutputError> {
        Ok(())
    }
}

/// Characters closer than this (PDF points) join into one segment; table columns are further
/// apart than any inter-word gap in these reports.
const SEGMENT_GAP: f64 = 6.0;

/// Positioned text lines of a PDF, top to bottom, left to right.
pub fn pdf_lines(bytes: &[u8]) -> Result<Vec<PdfLine>> {
    let doc = pdf_extract::Document::load_mem(bytes)
        .map_err(|e| IngestError::Parse(format!("PDF: {e}")))?;
    let mut c = CharCollector {
        page: 0,
        chars: Vec::new(),
    };
    pdf_extract::output_doc(&doc, &mut c)
        .map_err(|e| IngestError::Parse(format!("PDF text: {e}")))?;

    // Group by (page, baseline y rounded to half a point).
    let mut keyed: std::collections::BTreeMap<(u32, i64), Vec<(f64, f64, String)>> =
        std::collections::BTreeMap::new();
    for (page, x, y, w, s) in c.chars {
        keyed
            .entry((page, (y * 2.0).round() as i64))
            .or_default()
            .push((x, w, s));
    }
    let mut lines: Vec<PdfLine> = keyed
        .into_iter()
        .map(|((page, y2), mut chars)| {
            chars.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut segments: Vec<Segment> = Vec::new();
            for (x, w, s) in chars {
                match segments.last_mut() {
                    Some(seg) if x - seg.x1 <= SEGMENT_GAP => {
                        seg.x1 = x + w;
                        seg.text.push_str(&s);
                    }
                    _ => segments.push(Segment {
                        x0: x,
                        x1: x + w,
                        text: s,
                    }),
                }
            }
            for seg in &mut segments {
                seg.text = seg.text.trim().to_owned();
            }
            segments.retain(|s| !s.text.is_empty());
            PdfLine {
                page,
                y: y2 as f64 / 2.0,
                segments,
            }
        })
        .filter(|l| !l.segments.is_empty())
        .collect();
    // Page ascending, then top of page first (larger y first).
    lines.sort_by(|a, b| a.page.cmp(&b.page).then(b.y.total_cmp(&a.y)));
    Ok(lines)
}

/// How far (points) a cell's right edge may sit from its header's right edge.
const COLUMN_TOLERANCE: f64 = 6.0;

fn parse_slash_date(s: &str) -> Option<NaiveDate> {
    // "1/12/26" or "11/24/25" (two-digit year) or "1/12/2026".
    let mut p = s.trim().split('/');
    let (m, d, y) = (p.next()?, p.next()?, p.next()?);
    if p.next().is_some() {
        return None;
    }
    let (m, d, y): (u32, u32, i32) = (m.parse().ok()?, d.parse().ok()?, y.parse().ok()?);
    let y = if y < 100 { 2000 + y } else { y };
    NaiveDate::from_ymd_opt(y, m, d)
}

pub fn parse_pdf_report(body: &[u8], retrieval: &Retrieval) -> Result<Report> {
    let lines = pdf_lines(body)?;
    let mut issues = Vec::new();
    let line_text = |l: &PdfLine| {
        l.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };

    let title = lines
        .iter()
        .map(line_text)
        .find(|t| t.contains("Measles Data Report"))
        .ok_or_else(|| IngestError::Parse("PDF has no \"Measles Data Report\" title".into()))?;
    let report_date = title
        .rsplit(['–', '-'])
        .next()
        .and_then(parse_slash_date)
        .ok_or_else(|| IngestError::Parse(format!("unreadable report date in title {title:?}")))?;
    let data_as_of = lines
        .iter()
        .map(line_text)
        .find(|t| t.starts_with("Preliminary Data as of"));

    // Header row: first segment "Home County"; the other segments name the columns.
    let (hi, header) = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.segments.first().is_some_and(|s| s.text == "Home County"))
        .ok_or_else(|| IngestError::Parse("PDF has no \"Home County\" table header".into()))?;
    let columns: Vec<(&str, f64)> = header.segments[1..]
        .iter()
        .map(|s| (s.text.as_str(), s.x1))
        .collect();
    if columns.is_empty() {
        return Err(IngestError::Parse(
            "PDF table header has no value columns".into(),
        ));
    }

    let mut tables: Vec<CountyTable> = columns
        .iter()
        .map(|(name, _)| CountyTable {
            caption: format!("Table 1: Confirmed Cases in Texas Residents / {name}"),
            entries: Vec::new(),
            total: None,
        })
        .collect();
    let mut notes = Vec::new();
    let mut in_table = true;
    for line in &lines[hi + 1..] {
        let first = &line.segments[0];
        if !in_table {
            if first.text.starts_with('*') {
                notes.push(line_text(line));
            }
            continue;
        }
        let is_total = first.text == "Grand Total";
        if first.x0 > header.segments[0].x1 + 40.0 {
            // Indented text (a continuation or a footnote) is not a county row.
            issues.push(format!("unplaced PDF text: {}", line_text(line)));
            continue;
        }
        for seg in &line.segments[1..] {
            let col = columns
                .iter()
                .position(|(_, edge)| (seg.x1 - edge).abs() <= COLUMN_TOLERANCE);
            let Some(ci) = col else {
                issues.push(format!(
                    "{:?}: cell {:?} (x {:.0}-{:.0}) fits no column header",
                    first.text, seg.text, seg.x0, seg.x1
                ));
                continue;
            };
            if is_total {
                tables[ci].total = parse_count_cell(&seg.text);
            } else {
                tables[ci].entries.push(CountyEntry {
                    name: first.text.clone(),
                    raw: seg.text.clone(),
                    cases: parse_count_cell(&seg.text),
                });
            }
        }
        if is_total {
            in_table = false;
        }
    }
    issues.extend(notes.into_iter().map(|n| format!("footnote: {n}")));
    for t in &tables {
        check_table(t, &mut issues);
    }

    // The outbreak column is the one headed "... Outbreak ...".
    let oi = columns
        .iter()
        .position(|(n, _)| n.contains("Outbreak"))
        .ok_or_else(|| IngestError::Parse("PDF table has no outbreak column".into()))?;
    let outbreak = tables.remove(oi);
    let outbreak_total = outbreak.total;
    Ok(Report {
        format: ReportFormat::PdfDataReport,
        report_date,
        data_as_of,
        outbreak_total,
        outbreak_counties: Some(outbreak),
        other_tables: tables,
        issues,
        retrieval: retrieval.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_cells_are_whole_numbers_with_optional_markers_and_percent() {
        assert_eq!(parse_count_cell("107"), Some(107));
        assert_eq!(parse_count_cell("4*"), Some(4));
        assert_eq!(parse_count_cell("15**"), Some(15));
        assert_eq!(parse_count_cell("414 (51.56)"), Some(414));
        assert_eq!(parse_count_cell(""), None);
        assert_eq!(parse_count_cell("n/a"), None);
        assert_eq!(parse_count_cell("1.5"), None);
        assert_eq!(parse_count_cell("12 (3"), None);
    }

    #[test]
    fn long_and_slash_dates() {
        assert_eq!(
            parse_long_date("March 4, 2025"),
            NaiveDate::from_ymd_opt(2025, 3, 4)
        );
        assert_eq!(
            parse_slash_date("1/12/26"),
            NaiveDate::from_ymd_opt(2026, 1, 12)
        );
        assert_eq!(
            parse_slash_date("11/24/2025"),
            NaiveDate::from_ymd_opt(2025, 11, 24)
        );
        assert_eq!(parse_slash_date("13/40/25"), None);
    }

    #[test]
    fn gzip_bodies_are_decoded_and_plain_bodies_pass_through() {
        use std::io::Write;
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(b"hello").unwrap();
        let gz = enc.finish().unwrap();
        assert_eq!(decode_body(&gz).unwrap(), b"hello");
        assert_eq!(decode_body(b"hello").unwrap(), b"hello");
    }
}
