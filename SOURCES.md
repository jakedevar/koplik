# Sources

Every external source Koplik ingests: where it lives, the terms it comes with, how often it
changes and when someone last checked. The `source_id` is the id carried in every provenance
record; the `licence_id` is defined in the licence table below. A row marked **operator
decision required before publishing** must not feed anything public until the operator
clears it (AGENTS.md: accepting a data licence is an operator decision).

| source_id | Source | Exact URL (as recorded in provenance) | Licence id | Cadence | Last verified |
| --- | --- | --- | --- | --- | --- |
| `cdc-nndss-weekly-measles` | CDC NNDSS Weekly Data, measles rows (data.cdc.gov dataset `x9gk-5huc`) | see "CDC NNDSS query" below | `cdc-open-data-terms-unconfirmed` | Weekly (CDC republishes the weekly tables; dataset last updated 2026-09-30) | 2026-10-07 |
| `dshs-measles-outbreak-page` | Texas DSHS "Measles Outbreak" page (live fetch) | `https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025` | `dshs-copyright-noncommercial-no-alteration` | Twice weekly (Tue, Fri) March to early June 2025, then Tuesdays; last update 2025-08-12, outbreak declared over 2025-08-18 | 2026-10-07 |
| `dshs-measles-outbreak-page-wayback` | The same page, one Internet Archive capture per day, 2025-03-05 to 2025-09-02 (each its own snapshot) | `https://web.archive.org/web/<capture time>id_/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025` | `internet-archive-terms-of-use` (content is DSHS's: also `dshs-copyright-noncommercial-no-alteration`) | One capture per day from the CDX index | 2026-10-07 |
| `dshs-measles-data-report` / `dshs-measles-data-report-wayback` | DSHS "2025 Measles Data Report" PDFs: 2025-11-24, 2025-12-23, 2026-01-12 (live final report and Archive captures) | `https://www.dshs.texas.gov/sites/default/files/Admin-Meales/doc/2025-measles-outbreak-data-report-011226.pdf` and the Archive captures of `...2025-measles-data-report-nov-2025.pdf`, `...2025-measles-outbreak-data-report-12-23-25.pdf` | `dshs-copyright-noncommercial-no-alteration` | Three reports (about monthly), then the outbreak ended | 2026-10-07 |
| `wayback-cdx-listing` | Internet Archive CDX index answers listing captures (stored so "which captures existed" stays on record) | `https://web.archive.org/cdx/search/cdx?url=...&output=json&fl=timestamp,original,statuscode,digest&filter=statuscode:200&from=...&to=...` | `internet-archive-terms-of-use` | On demand | 2026-10-07 |
| `census-county-codes-2020-wayback` | Census national county reference file `national_county2020.txt` (county name to FIPS) from the Internet Archive capture of 2025-02-06 | `https://web.archive.org/web/20250206022004id_/https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt` | `census-open-data-terms-unconfirmed` | Rarely (2020 vintage codes) | 2026-10-07 |
| `cdc-schoolvaxview-kindergarten` | CDC SchoolVaxView, kindergarten MMR and any exemptions (Socrata `ijqb-a7ye`) | Query below; exact URL in retrieval metadata | `cdc-schoolvaxview-terms-unconfirmed` | Annual school year | 2026-10-07 |
| `texas-dshs-kindergarten-2023` | Texas DSHS 2023–24 kindergarten coverage, published county worksheet | https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/2023-2024_School_Vaccination_Coverage_Levels_Kindergarten.xlsx | `texas-dshs-terms-unconfirmed` | Annual | 2026-10-07 |
| `texas-dshs-kindergarten-2024` | Texas DSHS 2024–25 kindergarten coverage, published county worksheet | https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/2024-2025_School_Vaccination_Coverage_Levels_Kindergarten.xlsx | `texas-dshs-terms-unconfirmed` | Annual | 2026-10-07 |
| `texas-dshs-county-fips` | Texas DSHS county name/FIPS crosswalk (identity only) | https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions | `texas-dshs-terms-unconfirmed` | As revised | 2026-10-07 |

## Licences and terms

| licence_id | Terms | Status |
| --- | --- | --- |
| `cdc-open-data-terms-unconfirmed` | The dataset page and its metadata name no licence (`license: null`); the publisher is CDC's Office of Public Health Data, Surveillance, and Technology (contact `NNDSSWeb@cdc.gov`). CDC data is a US federal agency product, but nobody has confirmed the reuse terms for this dataset. The dataset's own notes say counts are provisional, subject to ongoing revision, and "presented as published each week". | **Operator decision required before publishing.** Koplik shows the figures only with their provenance and the demonstration disclaimer; confirm the terms (or ask CDC) before the site goes public. |
| `dshs-copyright-noncommercial-no-alteration` | DSHS "Copyright and Disclaimer" (`https://www.dshs.texas.gov/site-policies/copyright-disclaimer`, read 2026-10-07): "Unless otherwise noted on an individual document, file, home page, or the like, DSHS grants permission to copy and distribute files, documents and information provided for non-commercial use, so long as the information is copied and distributed without alteration." The outbreak page and PDFs carry no other notice. | **Operator decision required before publishing.** Whether tables and maps derived from the counts (differenced, re-keyed by FIPS, charted) count as "without alteration", and whether the demonstration is "non-commercial", is the operator's call; until then show DSHS figures only with provenance and a pointer to DSHS's own pages, or ask DSHS. |
| `internet-archive-terms-of-use` | The Internet Archive's terms of use (`https://archive.org/about/terms.php`) cover use of its service; the archived bytes remain the original publisher's content under the original publisher's terms. | Archive captures are used as evidence of what DSHS published and when; nothing is republished from the Archive. Same operator decision as the DSHS row for the content itself. |
| `census-open-data-terms-unconfirmed` | US Census Bureau reference files are a federal agency product published for download; the file carries no licence statement and nobody has confirmed its reuse terms. | **Operator decision required before publishing** (low risk: facts, not creative content; county names and FIPS codes). |

## CDC NNDSS weekly measles cases by state (`cdc-nndss-weekly-measles`)

**Query (exact URL, built by `koplik-ingest`, recorded in every provenance record):**

```
https://data.cdc.gov/resource/x9gk-5huc.json?$select=states,year,week,label,m3,m3_flag&$where=label%20in%28%27Measles,%20Indigenous%27,%27Measles,%20Imported%27%29%20AND%20year%20in%28%272025%27,%272026%27%29&$order=year,week,states,label&$limit=50000
```

i.e. `$select=states,year,week,label,m3,m3_flag`, `$where=label in('Measles, Indigenous','Measles, Imported') AND year in('2025','2026')`,
`$order=year,week,states,label`, `$limit=50000` (a response with 50,000 rows is refused as possibly truncated;
the real response has 12,740). `koplik-ingest fetch cdc-cases --first-year Y --last-year Y` builds the
same query for other years. A re-fetch with the same URL is reproducible; the data behind it changes weekly.

**Why this source.** NNDSS Weekly Data is the only CDC source found that publishes a weekly series per
state through a documented API, with a stable schema, explicit missing-value flags and every report
week kept as its own row (2025 weeks 1-53 and 2026 so far). The CDC "Measles Cases and Outbreaks"
page could not be used: an automated request to `https://www.cdc.gov/measles/data-research/index.html`
returned HTTP 403 on 2026-10-07 (and I did not work around the block), so it cannot be fetched
reproducibly. data.cdc.gov's `robots.txt` allows `/resource/` with `Crawl-delay: 1`.

**Client identification.** Requests send `koplik-ingest/<version> (measles data demonstration project; <contact>)`.
The contact is whatever the operator verified and put in the `KOPLIK_CONTACT` environment variable (an e-mail
address or repository URL); `koplik-ingest fetch` refuses to run, before any request, if it is unset or blank,
and Koplik never invents one. `make pipeline` fetches, so it needs `KOPLIK_CONTACT` until the operator's
verified default is committed (decision record `ingest-contact`). Offline parsing and all tests need none.

**Columns used.** `states` (reporting jurisdiction name), `year` and `week` (MMWR reporting year and
week of the weekly table), `label` (`Measles, Indigenous` or `Measles, Imported`), `m3` (cumulative
year-to-date count as published that week), `m3_flag` (`-`, `U`, `N`, `NN`, `NP`, `NC` when there is no
number). `m1` ("current week") is not used.

**How weekly counts are derived** (`crates/koplik-ingest/src/cdc.rs`, tested against the recorded fixture):

1. The weekly table's own "current week" column (`m1`) leaves out cases that are added to earlier weeks:
   for Texas 2025 its weeks sum to 288 while the published cumulative ends at 803. The cumulative `m3` is
   the complete accounting, so weekly new cases are the week-over-week difference of `m3`
   (week 1 of a year: `m3` itself).
2. Per state, `m3` is first summed over Indigenous + Imported (and, for New York, over the separately
   reporting New York State and New York City), then differenced. A case reclassified between Imported
   and Indigenous therefore does not read as a decrease (it happens in the real data: California 2025
   week 35).
3. "Week" is the MMWR **reporting** week of the CDC table, i.e. when cases reached CDC, not the onset
   week. It lags onset by days to weeks and the most recent weeks are provisional; downstream
   R_t code must treat it that way.
4. Missing stays missing, never zero. No row for a week: `not_reported`. Flags `U`, `N`, `NN`, `NP`,
   `NC`: `not_reported`. A cumulative that falls (the 16 cases in the fixture are corrections, e.g.
   California 2025 week 34: 20 -> 19), a value that is not a whole non-negative number, or a week
   that follows a missing cumulative: `ambiguous`. The flag `-` ("No reported cases") means the
   jurisdiction reports and has none, so a cumulative of `-` is a real zero.
5. Geographies are the 50 states, DC and 5 territories keyed by state FIPS (56). Regional and national
   totals and `Non-U.S. Residents` are skipped; an unrecognised jurisdiction name is an error.
6. **Case definition: confirmed OR unknown status, not "confirmed".** CDC's NNDSS Event Code List
   lists the publication criteria for measles (rubeola), event code 10140, in column F: **"Cases with
   confirmed and unknown case status are printed."** Both the 2025 and the 2026 editions say this
   (read directly from the workbooks, 2026-10-07):
   - 2025 v2: <https://ndc.services.cdc.gov/wp-content/uploads/National_Notifiable_Diseases_Surveillance_System_Event_Code_List_2025_v2_2025Nov21-508.xlsx>
     (row for event code 10140, same text on each of its event-code tabs).
   - 2026 v1: <https://ndc.services.cdc.gov/wp-content/uploads/National_Notifiable_Diseases_Surveillance_System_Event_Code_List_2026_v1_2026Jan12.xlsx>,
     sheet "Event Codes", cell F98.

   The weekly query has no case-status field, so confirmed cases cannot be separated from
   unknown-status ones. The connector therefore emits **contracts v3** rows
   (`WeeklyCaseCount.cases` with `case_definition = confirmed_or_unknown_status`) and never
   `confirmed`; v1's `confirmed` field is not used for this source. UI and R_t consumers must say
   "confirmed or unknown-status cases reported to NNDSS".

**Fixture.** `data/fixtures/cdc/nndss-measles-weekly.json` is the unmodified response retrieved
2026-10-07T02:19:30Z, sha256 `c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`
(see `data/fixtures/cdc/README.md`).

## Texas DSHS 2025 West Texas outbreak, cases by county over time (`dshs-*`)

Code: `crates/koplik-ingest/src/{dshs_sources,dshs,dshs_series,census_counties}.rs`. Every `fetch` needs `KOPLIK_CONTACT` (a contact address or repository URL; it refuses without one). Commands:
`koplik-ingest fetch census-counties | dshs-live | dshs-reports | dshs-wayback`, then the offline
`koplik-ingest parse dshs-cases --out DIR` (manifest, cumulative, interval and weekly series (contracts v3 rows), unmapped names,
parse failures). Fixtures and their provenance: `data/fixtures/dshs/README.md`.

**Three formats, found by reading the archived pages** (the manifest `data/dshs/vintage-manifest.json` lists every
version held):

1. *HTML county table*, 2025-03-04 to 2025-03-25 (seven versions, Tuesdays and Fridays): the outbreak page's
   "Texas Case Count by County" table (later captioned "Texas Outbreak Case Count by County"), County | Cases,
   cumulative, with a Total row, plus "2025 Texas Measles Cases Not Associated with the Outbreak in West Texas".
   Earlier versions (late January to March 3) are not on this page in the Archive; they exist only as DSHS news
   releases and health alerts, which are not parsed.
2. *Tableau dashboard*, 2025-03-28 to 2025-08-12 (30 versions): the table is replaced by an embedded dashboard
   at `tabexternal.dshs.texas.gov`. That host answers every automated request with HTTP 403 (its `robots.txt` too, which
   the polite fetcher reads as "no crawling"), and it is not archived, so **county counts for these 30 weeks cannot be
   fetched**. The page text still gives the cumulative outbreak total ("At this time, N cases have been confirmed since
   late January"; 400 on 03-28 up to 762 from 07-15) and the "not associated" table; both are parsed, the counties
   stay missing.
3. *PDF data reports*, 2025-11-24, 2025-12-23, 2026-01-12: "Table 1: Confirmed Cases in Texas Residents" by home
   county (outbreak, international travel, other Texas cases). Columns are told apart by the right edge of each
   cell's text against the header row (`pdf-extract` character positions). The report footnote says 182 further
   potential cases from March 2025 in Gaines County children could not be classified as confirmed and are not in the
   counts; that footnote is carried as a parser issue on every PDF vintage. **The three PDFs' outbreak rows add to 763
   against a printed Grand Total of 762** (their percentages use 803 = 762 + 16 + 25 as denominator); reported as
   published and flagged, never reconciled.

**Past versions.** Internet Archive captures through the polite fetcher: the CDX index lists the first capture of each
day (`collapse=timestamp:8`); each capture is fetched with the `id_` URL form (unmodified archived bytes, which the
Archive may replay gzip-encoded: the snapshot keeps the exact bytes and the parser un-gzips) and stored as its own
snapshot whose provenance `url` is the capture URL (capture time and original URL are read back from it). The
Archive rate-limits well below one request a second (HTTP 429), so the fetch runs at one request per 8 seconds.
128 of 129 listed captures are held; the one capture that failed (HTTP 500 on repeated tries) is listed in the manifest as
`unretrieved_captures`. One capture per day can miss a version published and replaced within a day.

**Vintage manifest** (`data/dshs/vintage-manifest.json`, `manifest_version` 1; ingest-local, not a contracts type):
one entry per distinct report version (same date and content), with `report_date` (the date DSHS prints: the page's
"News Updates" date, the PDF title date), `first_seen_at` (earliest capture time, or retrieval time for a live fetch;
an upper bound on when it was public), the first snapshot's provenance (capture URL, original URL, capture time,
retrieval time, sha256, licence), how many snapshots show it, parser issues, and whether it has county detail. 40
versions: 7 with a county table, 30 dashboard-only, 3 PDFs. A backtest that needs "what could be known at date D" must
use versions with `first_seen_at` <= D (the 2025-11-24 report first appears in the Archive on 2025-12-17).

**County keys.** Names map to 5-digit FIPS only through the Census `national_county2020.txt` snapshot
(`CountyLookup`); matching folds case, a trailing "County" and whitespace, nothing fuzzier. A name that matches no
county or several is returned in `unmapped` and its cases are left out; every county name in an outbreak county table (the 7 HTML and 3 PDF versions) maps (254 Texas
counties in the file; the "not associated" tables' names also map in the fixtures). `www2.census.gov/robots.txt` has `User-agent: *` and `User-agent: RavenCrawler` in one group
(blank line between) ahead of `Disallow: /`, which RFC 9309 reads as disallowing every crawler, so the polite fetcher
refuses the live host; the Archive's capture of the same file (2025-02-06) is used instead, with the capture URL in
provenance. The live host is not worked around. (`koplik-ingest fetch census-counties --direct 1` tries it.)

**Derivation** (`dshs_series.rs`, module docs have the full rules; tested in `tests/dshs_parser.rs` and the unit
tests there). DSHS publishes cumulative cases per county per report date; only versions with a readable county table
take part.

1. *Cumulative*: the printed row. A county absent from a table is `reported 0` only if that table's rows are all
   readable and add up to its printed Total; otherwise `missing: ambiguous`. Counties never listed anywhere get no rows.
2. *Per report interval*: cumulative minus the previous county-detail report's, exact however far apart the reports
   are (03-25 to 11-24 is one interval). A fall, or an unreadable cell: `missing: ambiguous`.
3. *Per MMWR week* (the week containing the report date; this is when DSHS published, not rash onset, and it lags
   onset): the difference between the last report in week W and the last report in week W-1, only when both are the
   last report of any kind in their weeks, so a week whose last report is dashboard-only gets no partial count.
   Otherwise `missing: ambiguous` when the week has a county report that cannot be completed or split (the first
   county report after a gap, or the week of a dashboard-only last report), `missing: not_reported` for a week with no
   county report and for the first report's week (its cumulative includes all earlier cases). With the real data
   only 2025 weeks 11 and 12 have reported weekly counts per county; the rest are missing by construction.
4. *Case definition.* Weekly rows are contracts **v3** `WeeklyCaseCount` with `case_definition: confirmed`, and a
   version enters them only if its own labelling establishes confirmed cases (`confirmed_basis` in the manifest): the
   PDF's table title "Table 1: Confirmed Cases in Texas Residents", or, for the HTML pages, a "... Confirmed Cases ..."
   (vaccination status) table whose cells add up to the outbreak total. All 10 county-detail versions pass. The early
   pages' prose says "cases have been identified" rather than "confirmed"; the matching total in the confirmed table is
   the only evidence that the counted population is confirmed cases, so re-check with DSHS before the UI says so
   for March. The PDF footnote's 182 unclassifiable Gaines County reports are not in any count.
5. A Texas "outbreak total" row is not emitted as a `WeeklyCaseCount`: it is the West Texas outbreak total, not
   Texas's cases, so it lives in the manifest (`outbreak_total` per version) for the backtest.

**Caveats not resolved here.** DSHS's classification ("confirmed") and revisions: counts were revised (a Lubbock vaccine
reaction was removed in March; cases reclassified into and out of the outbreak). Report dates are Tuesday/Friday
publication dates, not data-as-of times, except the PDFs, which print "Preliminary Data as of". The final outbreak
count by county is the PDF's; between 2025-03-25 and 2025-11-24 only state-level outbreak totals exist.

## Kindergarten MMR coverage (#1351)

| licence_id | Terms | Status |
| --- | --- | --- |
| `cdc-schoolvaxview-terms-unconfirmed` | The SchoolVaxView dataset is provided by CDC NCIRD; dataset-specific reuse terms have not been confirmed. | **Operator decision required before publishing.** |
| `texas-dshs-terms-unconfirmed` | DSHS's pages carry an all-rights-reserved copyright footer; no explicit dataset redistribution licence was confirmed for the workbooks or county crosswalk. | **Operator decision required before publishing.** |
| `census-county-codes-terms-unconfirmed` | Terms unconfirmed; the HTTP-200 Missing Key response is retained only as a rejected discovery fixture, never consumed as county identities or coverage data. | Fixture never consumed; **operator decision required before publishing.** |

### CDC SchoolVaxView (`cdc-schoolvaxview-kindergarten`)

[Dataset and field definitions](https://data.cdc.gov/Vaccinations/Vaccination-Coverage-and-Exemptions-among-Kinderga/ijqb-a7ye).
`coverage::cdc_source_spec(first,last)` builds the following reproducible default query:

```
https://data.cdc.gov/resource/ijqb-a7ye.json?$select=vaccine,dose,geography_type,geography,year_season,coverage_estimate,foot_notes,survey_type&$where=year_season%20in%28%272023-24%27,%272024-25%27%29%20AND%20%28vaccine%3D%27MMR%27%20OR%20%28vaccine%3D%27Exemption%27%20AND%20dose%3D%27Any%20Exemption%27%29%29&$order=year_season,geography,vaccine&$limit=5000
```

Selects `vaccine,dose,geography_type,geography,year_season,coverage_estimate,foot_notes,survey_type`;
filters school years 2023–24 and 2024–25 and `MMR` or `Exemption`/`Any Exemption`; orders by
school year, geography and vaccine; explicitly limits to 5,000 rows. A response reaching that
limit is refused. The complete untrimmed fixture has 214 rows. The annual MMR series has an
empty dose field; it is the published kindergarten MMR coverage, not the separate `MMR (PAC)`
(potentially achievable coverage) measure.

The parser emits 50 states plus DC for every requested year (102 v1 rows for the default).
It joins the published **Any Exemption** percentage by state/year; it does not sum medical
and nonmedical rates or use exemption counts. National totals/medians and separately published
NYC, rest-of-New-York and Houston rows are excluded. New York uses the directly published
statewide estimate, not a sum or average of component percentages. Unknown geography or
vaccine/dose labels and duplicate cells are errors. Territory estimates are not present in
this source; the state output scope is explicitly 50 states plus DC.

The captured source has no MMR row for Montana in either year or West Virginia in 2024–25.
These are `missing/not_reported`, with snapshot provenance, and appear in the committed gaps
report. Both coverage and exemption estimates may have state-specific survey limitations;
`foot_notes` and `survey_type` are retained in the immutable snapshot and must be consulted
before interpreting a kindergarten survey as county or whole-population susceptibility.

### Texas DSHS coverage and county identity

[DSHS school coverage landing page](https://www.dshs.texas.gov/immunizations/data/school/coverage).
These are the actual `.xlsx` downloads linked by DSHS (the page calls them XLS).
The parser checks the workbook's kindergarten/year title, then reads **Coverage by County**,
locating the `County` and `MMR` headers (row 1 in 2023–24, row 3 in 2024–25).
It uses DSHS's county aggregates directly, never district averages. Excel stores these
rates as fractions; the only numeric transformation is multiplication by 100 into contract
percentage units. The [DSHS county/FIPS table](https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions)
provides explicit identifiers. The join folds ASCII case only (DSHS spells DeWitt/McCulloch/
McLennan/McMullen differently between its own tables); unknown names, duplicate identities
or duplicate county rows fail. FIPS is never inferred from alphabetical order. Every county
row carries both workbook and identity-crosswalk provenance.

**2025 outbreak baseline: 2023–24.** This is the last completed school year before the
outbreak began in January 2025. It gives a pre-outbreak annual kindergarten cohort baseline;
it is not a claim that these exact current bytes were available at a historical forecast
cutoff. The 2024–25 workbook is also ingested as a separate measured cohort, not silently
substituted for the baseline. The 2023–24 Gaines County (48165) fraction is
`0.81967213114754101` (81.9672131147541% after unit conversion); the 2024–25 fraction is
`0.77256317689530685` (77.25631768953069% after unit conversion). There is no imputation.

DSHS covers responding public/private schools and excludes home schools. A kindergarten
rate is not a whole-county/all-age vaccination rate. The workbook note omits small schools
from the **district** list; no attempt is made to reconstruct suppressed district records.
The county sheet explicitly says `NR=No Report` in 2024–25: schools did not respond or had no
kindergarten class. `NR` stays `missing/not_reported`. The 2023–24 gaps are Crane (48103),
Loving (48301), Real (48385) and Stonewall (48433); 2024–25 has Loving (48301).
All 254 counties are emitted for each year, including any absent county as `not_reported`.
This coverage workbook does not publish kindergarten **any** exemptions by county;
`exemption_pct` is null throughout. Do not substitute DSHS's separate K–12 conscientious
exemption measure into a kindergarten any-exemption field.

### Missing data, provenance and use

For either source: absent/blank/NR/NA/NReq values are not reported; explicit suppression
markers are suppressed; bounded percentages (e.g. `<0.1`), malformed, nonfinite and out-of-range
values are ambiguous. Literal numeric zero is a reported zero. Missing exemptions are null
(the v1 contract has no separate missing reason for exemptions); no bound or midpoint is
invented. No rows are imputed, and contract v1 is unchanged.

`koplik-ingest fetch cdc-coverage` and `fetch texas-coverage --year 2023|2024` reuse the polite
fetcher and content-addressed store; the Texas command also snapshots the identity table.
`parse cdc-coverage` and `parse texas-coverage --year 2023|2024` are offline. Use `--out FILE`
for the v1 row array and `--gaps FILE` for the subset with missing coverage, including reasons
and provenance. CDC `--first-year`/`--last-year` are school-year **start** years and must match
the stored query when parsing; defaults are 2023 and 2024. Texas defaults to baseline 2023.
Committed gaps reports in `data/reports/coverage/` are derived from the pinned fixtures;
offline tests regenerate and compare them. Fixture reproduction is documented in
`data/fixtures/coverage/README.md`.

Source-discovery constraint: the Census county API returned an HTTP-200 HTML **Missing Key**
page, not data; the Census reference-file URL was denied by robots.txt. Neither was bypassed.
The rejected API bytes are a negative fixture, never a crosswalk or coverage source. Its
source/terms ids are recorded solely as discovery provenance, not approved publication terms.
Credential handling follow-up: #1374. The implementation uses the public DSHS table instead.
