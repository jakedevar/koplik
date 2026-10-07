# Sources

Every external source Koplik ingests: where it lives, the terms it comes with, how often it
changes and when someone last checked. The `source_id` is the id carried in every provenance
record; the `licence_id` is defined in the licence table below, together with the ruling that
applies to it and the attribution text the site shows. Terms were ruled on 2026-10-07 (see the
table); they are not a publishing gate.

| source_id | Source | Exact URL (as recorded in provenance) | Licence id | Cadence | Last verified |
| --- | --- | --- | --- | --- | --- |
| `cdc-nndss-weekly-measles` | CDC NNDSS Weekly Data, measles rows (data.cdc.gov dataset `x9gk-5huc`) | see "CDC NNDSS query" below | `cdc-open-data-terms-unconfirmed` (US federal public domain) | Weekly (CDC republishes the weekly tables; dataset last updated 2026-09-30) | 2026-10-07 |
| `dshs-measles-outbreak-page` | Texas DSHS "Measles Outbreak" page (live fetch) | `https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025` | `dshs-copyright-noncommercial-no-alteration` (none stated; public information, attribution and link) | Twice weekly (Tue, Fri) March to early June 2025, then Tuesdays; last update 2025-08-12, outbreak declared over 2025-08-18 | 2026-10-07 |
| `dshs-measles-outbreak-page-wayback` | The same page, one Internet Archive capture per day, 2025-03-05 to 2025-09-02 (each its own snapshot) | `https://web.archive.org/web/<capture time>id_/https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025` | `internet-archive-terms-of-use` (Archive terms plus the publisher's; content is DSHS's: also `dshs-copyright-noncommercial-no-alteration`, none stated, attribution and link) | One capture per day from the CDX index | 2026-10-07 |
| `dshs-measles-data-report` / `dshs-measles-data-report-wayback` | DSHS "2025 Measles Data Report" PDFs: 2025-11-24, 2025-12-23, 2026-01-12 (live final report and Archive captures) | `https://www.dshs.texas.gov/sites/default/files/Admin-Meales/doc/2025-measles-outbreak-data-report-011226.pdf` and the Archive captures of `...2025-measles-data-report-nov-2025.pdf`, `...2025-measles-outbreak-data-report-12-23-25.pdf` | `dshs-copyright-noncommercial-no-alteration` (none stated; public information, attribution and link) | Three reports (about monthly), then the outbreak ended | 2026-10-07 |
| `wayback-cdx-listing` | Internet Archive CDX index answers listing captures (stored so "which captures existed" stays on record) | `https://web.archive.org/cdx/search/cdx?url=...&output=json&fl=timestamp,original,statuscode,digest&filter=statuscode:200&from=...&to=...` | `internet-archive-terms-of-use` (Archive terms plus the publisher's) | On demand | 2026-10-07 |
| `census-county-codes-2020-wayback` | Census national county reference file `national_county2020.txt` (county name to FIPS) from the Internet Archive capture of 2025-02-06 | `https://web.archive.org/web/20250206022004id_/https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt` | `census-open-data-terms-unconfirmed` (US federal public domain) | Rarely (2020 vintage codes) | 2026-10-07 |
| `census-cb-2024-states-20m` | US Census Bureau, 2024 cartographic state boundaries, 1:20m | https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip | `us-census-public-domain` (US federal public domain) | Annual vintage; pinned to 2024 | 2026-10-07 (named-file decision; pinned manifest) |
| `census-cb-2024-counties-20m` | US Census Bureau, 2024 cartographic county boundaries, 1:20m; derived output filters STATEFP=48 | https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_county_20m.zip | `us-census-public-domain` (US federal public domain) | Annual vintage; pinned to 2024 | 2026-10-07 (named-file decision; pinned manifest) |
| `census-state-population-2025` | Census Population Estimates Program, Vintage 2025 state totals | `https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/state/totals/NST-EST2025-ALLDATA.csv` | `us-census-public-domain` (US federal public domain) | Annual vintage; pinned to 2025 | 2026-10-07 |
| `census-county-population-2025` | Census Population Estimates Program, Vintage 2025 county totals | `https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/counties/totals/co-est2025-alldata.csv` | `us-census-public-domain` (US federal public domain) | Annual vintage; pinned to 2025 | 2026-10-07 |
| `census-county-gazetteer-2025` | Census 2025 Gazetteer, national counties (Texas GEOIDs and internal points) | `https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_counties_national.zip` | `us-census-public-domain` (US federal public domain) | Annual; pinned to 2025 | 2026-10-07 |
| `census-state-gazetteer-2025` | Census 2025 Gazetteer, national states (state descriptors and internal points) | `https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_state_national.zip` | `us-census-public-domain` (US federal public domain) | Annual; pinned to 2025 | 2026-10-07 |
| `cdc-schoolvaxview-kindergarten` | CDC SchoolVaxView, kindergarten MMR and any exemptions (Socrata `ijqb-a7ye`) | Query below; exact URL in retrieval metadata | `cdc-schoolvaxview-terms-unconfirmed` (US federal public domain) | Annual school year | 2026-10-07 |
| `texas-dshs-kindergarten-2023` | Texas DSHS 2023–24 kindergarten coverage, published county worksheet | https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/2023-2024_School_Vaccination_Coverage_Levels_Kindergarten.xlsx | `texas-dshs-terms-unconfirmed` (none stated; public information, attribution and link) | Annual | 2026-10-07 |
| `texas-dshs-kindergarten-2024` | Texas DSHS 2024–25 kindergarten coverage, published county worksheet | https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/2024-2025_School_Vaccination_Coverage_Levels_Kindergarten.xlsx | `texas-dshs-terms-unconfirmed` (none stated; public information, attribution and link) | Annual | 2026-10-07 |
| `texas-dshs-county-fips` | Texas DSHS county name/FIPS crosswalk (identity only) | https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions | `texas-dshs-terms-unconfirmed` (none stated; public information, attribution and link) | As revised | 2026-10-07 |

## Licences and terms

**Ruling (2026-10-07, global manager node 73306b5f, under the operator's standing directive).**
CDC and Census data are US federal public domain (17 USC 105). Texas DSHS data is public
information: we use it with attribution and a link. Internet Archive captures are used under the
Archive's terms of use plus the underlying publisher's terms. This is not a publishing gate.
The terms are recorded here and shown on the site (attribution section and provenance drawer;
`web/src/attribution.ts` is the web copy of this table and a unit test keeps them in step).

**Licence ids are immutable.** `licence_id` values already written into fixtures, retrieval
metadata and emitted rows are not rewritten, even where the id says "unconfirmed". The "Recorded
licence id" column maps each recorded id to the ruling; the web's attribution table uses the same
mapping. A new id is only introduced for newly written data.

| Recorded licence id | Licence (ruling) | Terms | Attribution text to show | Sources |
| --- | --- | --- | --- | --- |
| `cdc-open-data-terms-unconfirmed` | US federal public domain (17 USC 105) | The dataset page and metadata name no licence (`license: null`); the publisher is CDC's Office of Public Health Data, Surveillance, and Technology (`NNDSSWeb@cdc.gov`). CDC is a US federal agency, so its data is a US government work in the public domain (17 USC 105). The dataset notes say counts are provisional, subject to ongoing revision, and "presented as published each week". | "Source: Centers for Disease Control and Prevention (CDC), NNDSS Weekly Data, https://data.cdc.gov/resource/x9gk-5huc. Counts are provisional and combine confirmed and unknown-status cases. Koplik is not affiliated with or endorsed by CDC." | `cdc-nndss-weekly-measles` |
| `cdc-schoolvaxview-terms-unconfirmed` | US federal public domain (17 USC 105) | The SchoolVaxView dataset is provided by CDC NCIRD, a US federal agency; US government work in the public domain (17 USC 105). | "Source: Centers for Disease Control and Prevention (CDC), SchoolVaxView, Vaccination Coverage and Exemptions among Kindergartners, https://data.cdc.gov/Vaccinations/Vaccination-Coverage-and-Exemptions-among-Kinderga/ijqb-a7ye. Not affiliated with or endorsed by CDC." | `cdc-schoolvaxview-kindergarten` |
| `us-census-public-domain` | US federal public domain (17 USC 105) | US Census Bureau geographic materials are public domain US government works. [2024 technical documentation, §1.2](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2024/TGRSHP2024_TechDoc.pdf) states Census materials may be reproduced and requests source attribution. Boundaries are statistical depictions, not legal land descriptions (§1.1). | "Source: US Census Bureau, 2024 cartographic boundary files (1:20,000,000), https://www.census.gov/geographies/mapping-files/time-series/geo/cartographic-boundary.html; Vintage 2025 population estimates, https://www.census.gov/programs-surveys/popest.html; 2025 Gazetteer files, https://www.census.gov/geographies/reference-files/time-series/geo/gazetteer-files.html. Boundaries are statistical depictions, not legal land descriptions." | `census-cb-2024-states-20m`, `census-cb-2024-counties-20m`, `census-state-population-2025`, `census-county-population-2025`, `census-county-gazetteer-2025`, `census-state-gazetteer-2025` |
| `census-open-data-terms-unconfirmed` | US federal public domain (17 USC 105) | A US Census Bureau reference file (`national_county2020.txt`: county names and FIPS codes), a US government work in the public domain (17 USC 105); the file carries no licence statement of its own. | "Source: US Census Bureau, national county reference file (2020 codes), https://www2.census.gov/geo/docs/reference/codes2020/national_county2020.txt." | `census-county-codes-2020-wayback` |
| `census-county-codes-terms-unconfirmed` | US federal public domain (17 USC 105); not consumed | Recorded only on the rejected Census county API HTTP-200 "Missing Key" response, which is a negative fixture and is never read as county identities or coverage data. | None: no data from this response is shown. | (negative fixture only) |
| `dshs-copyright-noncommercial-no-alteration` | none stated (public information; attribution and link required) | DSHS states no data licence. Its "Copyright and Disclaimer" page (`https://www.dshs.texas.gov/site-policies/copyright-disclaimer`, read 2026-10-07) says: "Unless otherwise noted on an individual document, file, home page, or the like, DSHS grants permission to copy and distribute files, documents and information provided for non-commercial use, so long as the information is copied and distributed without alteration." The outbreak page and PDFs carry no other notice. The ruling treats DSHS data as public information used with attribution and a link; Koplik therefore always names DSHS, links the DSHS page, and states which figures it derived (differenced, re-keyed by FIPS, charted) rather than presenting them as DSHS's own. | "Source: Texas Department of State Health Services (DSHS), 2025 Measles Outbreak data, https://www.dshs.texas.gov/news-alerts/measles-outbreak-2025. Counts were processed by Koplik (differenced by week, keyed by county FIPS); DSHS states no data licence. Not affiliated with or endorsed by DSHS." | `dshs-measles-outbreak-page`, `dshs-measles-outbreak-page-wayback`, `dshs-measles-data-report`, `dshs-measles-data-report-wayback` |
| `texas-dshs-terms-unconfirmed` | none stated (public information; attribution and link required) | DSHS's pages carry a copyright footer and DSHS states no data licence for the kindergarten coverage workbooks or the county crosswalk (see the DSHS notice quoted above). The ruling treats them as public information used with attribution and a link. | "Source: Texas Department of State Health Services (DSHS), School Vaccination Coverage Levels, Kindergarten, https://www.dshs.texas.gov/immunizations/data/school/coverage; county identifiers from https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions. Percentages converted by Koplik from DSHS fractions; DSHS states no data licence. Not affiliated with or endorsed by DSHS." | `texas-dshs-kindergarten-2023`, `texas-dshs-kindergarten-2024`, `texas-dshs-county-fips` |
| `internet-archive-terms-of-use` | Internet Archive terms of use plus the underlying publisher's terms | The Internet Archive's terms of use (`https://archive.org/about/terms.php`) cover use of its service; the archived bytes remain the original publisher's content under the publisher's terms (DSHS: see above; Census: public domain). Captures are evidence of what the publisher published and when; nothing is republished from the Archive. | "Archived copy retrieved through the Internet Archive Wayback Machine (https://web.archive.org); content is by its original publisher, credited as above." | `dshs-measles-outbreak-page-wayback`, `dshs-measles-data-report-wayback`, `wayback-cdx-listing`, `census-county-codes-2020-wayback` |

Rows tagged `synthetic-*` (`synthetic-test-only`, `synthetic-project-test-input`) are invented test
inputs, not sources; the site labels them as such and never lists them as attribution.
A source's rows may carry two licence ids (an Archive capture of DSHS content carries
`internet-archive-terms-of-use` and the DSHS id): both apply.

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

**Client identification.** Every live request, from every source in this file, sends
`koplik-ingest/<version> (measles data demonstration project; <contact>)`. The contact is
resolved in one place (`polite::contact_from_env`, via `PoliteConfig::live_from_env`): the
`KOPLIK_CONTACT` environment variable when set and non-blank (an e-mail address or repository URL);
the committed default `polite::DEFAULT_CONTACT = https://github.com/jakedevar` (the operator's public
GitHub profile, no e-mail; decision `ingest-contact`, 2026-10-07, #1413) when it is unset; and
**a refusal, before the store is opened or any request is sent, when it is set but empty or blank**
(an explicit opt-out). When a public Koplik repository exists, switch the default to its URL.
Offline parsing and all tests need no contact.

**Census contact (per host).** Requests to Census hosts (`www2.census.gov`, any `*.census.gov`;
not archive.org) carry the operator's own Census contact instead, per the operator's 2026-10-07
instruction (#1427). The address is not stored in this repository: it comes from
`KOPLIK_CENSUS_CONTACT` or from the gitignored `.env.local` at the repository root. Unset in both,
Census requests use the general contact above and print one notice on stderr; a blank value
refuses before any request. The same resolver (`PoliteConfig::live_from_env`) serves every live
path today (the CLI and examples); the pipeline's Census ingest (#1359) is intended to call it too
and will inherit it once it lands. Which contact a request carries is decided from the parsed
request URL's host (case-insensitive, one trailing dot removed, exactly `census.gov` or a
`*.census.gov` name).

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

Code: `crates/koplik-ingest/src/{dshs_sources,dshs,dshs_series,census_counties}.rs`. Every `fetch` identifies the client with the contact described under "Client identification" above (default `https://github.com/jakedevar`, override `KOPLIK_CONTACT`; a blank value refuses). Commands:
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

## Census cartographic boundaries (`census-cb-2024-states-20m`, `census-cb-2024-counties-20m`)

**Vintage and scale.** Pin 2024 to use one published vintage preceding the 2025 outbreak,
rather than silently follow annual geographic changes. The [Census 2024 naming guide](https://www2.census.gov/geo/tiger/GENZ2024/2024_file_name_def.pdf)
and [primary file listing](https://www2.census.gov/geo/tiger/GENZ2024/shp/) identify the two
national 1:20,000,000 shapefile ZIPs. Choose 1:20m for small national/state overview maps;
it is already generalized and needs no tiles, credentials, GDAL, or a separate HTTP client.
It is not suitable for parcel-level analysis. The raw county snapshot remains national;
only derived GeoJSON filters `STATEFP=48`. States retain all features supplied at this scale;
geographies absent from a source remain absent.

**Deterministic display conversion.** `koplik_ingest::census_boundaries::convert` reads SHP
and DBF in Rust, checks record counts and FIPS fields, assigns holes by containment, and
sorts features by string `GEOID` (also their feature `id`). `NAME` is the source display name.
Every feature carries an array of contracts v1 provenance records (hash → exact ZIP URL →
retrieval time). The byte digest must agree with the retrieval even for direct converter calls.
The source projection must be unprojected NAD83 degrees. This overview conversion uses NAD83
longitude/latitude as an approximate WGS84 position, without a high-accuracy datum/grid
transformation; do not use this output as survey coordinates.

Apply Visvalingam-Whyatt simplification with a fixed triangle-area tolerance of 0.000001
square degrees, then round longitude/latitude to four decimal places. This is a display
policy, not a scientific parameter; [the algorithm's documentation](https://docs.rs/geo/0.33.1/geo/algorithm/simplify_vw/trait.SimplifyVwPreserve.html)
defines the tolerance in triangle-area units. Validate geometry after both operations.
If simplification yields invalid geometry, keep the rounded original; if rounding itself
invalidates the source, fail explicitly. No island or hole is intentionally dropped.
Normalize exterior rings counterclockwise and holes clockwise for RFC 7946, preserve
source component order, emit compact JSON, and reject artifacts above 1,000,000 bytes.
This bounds output without silently removing data. Full-file measurements and the recorded
retrievals are documented with the fixtures in `data/fixtures/census/README.md`.

**Measured artifacts (2026-10-07).** The unmodified ZIP fixtures produce 52 state features
in 233,074 bytes and all 254 Texas county features in 169,613 bytes, including contracts
v1 provenance per feature. Both remain below the 1,000,000-byte ceiling. Exact source
digests, retrieval times, and output fingerprints are in `data/fixtures/census/README.md`.

**Pipeline entry points.** `fetch(&mut polite_fetcher, &store)` stores both ZIPs with the
existing write-once snapshot store. `write_latest(&store, output_dir)` converts both verified
latest snapshots before writing `states.geojson` and `tx-counties.geojson`; the pipeline
should pass `web/public/data/geo/` as the directory. For a historical build use a store
containing the selected retrievals (conversion is deterministic for identical bytes and
retrieval metadata; a different explicitly supplied retrieval changes provenance; cached fetches preserve the original record).

```
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- fetch census-boundaries
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- parse census-boundaries --out web/public/data/geo
```

**Named-file access decision (2026-10-07).** The global manager ruled `census-access`
option A, with limits, under the operator's standing directive (#1375, resolving #1373).
These public-domain files are published for direct download (17 USC 105); fetching a fixed,
named handful is not crawling. The recorded [RFC 9309 group reading](https://www.rfc-editor.org/rfc/rfc9309.html#section-2.2)
still applies: the empty wildcard `User-agent: *` entry and following `User-agent:
RavenCrawler` entry share `Disallow: /` despite the intervening blank line. The exception
is an explicit decision, not a change to that robots interpretation.

Only exact HTTPS `www2.census.gov` file URLs with SHA-256 pins in a committed, reviewed
manifest qualify. The reusable `census_files::NamedFileAllowlist` permits named TIGER/
cartographic boundary, Gazetteer, and population-estimate files; no wildcard, directory,
query, fragment, path traversal, alternate host, or redirect qualifies. The boundary
connector's manifest is `crates/koplik-ingest/manifests/census-boundaries-2024.json`.
Use one shared `PoliteFetcher` per pipeline run: it tracks attempts and makes at most one
GET per named file, including failures (no retries or redirects). Requests remain
sequential, paced at least one second apart, and carry the polite identifying User-Agent.
`PoliteConfig::live_from_env()` resolves the contact (`KOPLIK_CONTACT`, else the committed
`ingest-contact` default `https://github.com/jakedevar`; blank refuses).
Every other URL and host still follows the reviewed robots policy.

`census_files::fetch_to_store` re-verifies cached bytes against their content address and
skips the network entirely when a retrieval for the exact source URL/licence matches the
pin. It preserves that retrieval time and provenance, making repeated cached builds
byte-identical. A fetched digest mismatch reports both hashes, writes no snapshot, and
never updates the pin automatically. A deliberate pin update is a reviewed code change.
Initial pin discovery used the same exact-file exception with deliberately nonmatching
provisional digests: requests failed verification, exposed measured hashes for review,
and accepted/stored no data. The committed manifest contains only measured pins; subsequent
fixture capture verifies those pins before storing raw immutable response bytes.

## Kindergarten MMR coverage (#1351)

Terms for the coverage sources are in the licence table above (`cdc-schoolvaxview-terms-unconfirmed`,
`texas-dshs-terms-unconfirmed`, `census-county-codes-terms-unconfirmed`).

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
source/terms ids are recorded solely as discovery provenance; no data from that response is shown or attributed.
Credential handling follow-up: #1374. The implementation uses the public DSHS table instead.
## Census population and geography (`census-*-2025`)

The connector uses **Vintage 2025** PEP estimates, July 1, 2025 resident population
(`POPESTIMATE2025`, not a Census count or the 2024 column). The Census Bureau published
[2025 national/state estimates](https://www.census.gov/newsroom/press-kits/2026/national-state-population-estimates.html)
in January 2026 and [2025 county totals](https://www.census.gov/data/datasets/time-series/demo/popest/2020s-counties-total.html)
in March 2026. This is the latest annual vintage covering 2024/2025 at capture time.
Each new vintage revises the earlier annual series; the four committed pins identify this
specific capture, not every future response at the URLs.

The [2025 Gazetteer](https://www.census.gov/geographies/reference-files/time-series/geo/gazetteer-files.2025.html)
[record layouts](https://www.census.gov/programs-surveys/geography/technical-documentation/records-layout/gaz-record-layouts/gaz25-record-layouts.html)
define `INTPTLAT` and `INTPTLONG` in decimal degrees. These are Census representative internal
points, used as `Geography.centroid` for distance/gravity inputs, **not population-weighted or
geometric centroids**. County/state names come from `NAME`, and keys from zero-padded `GEOID`.
Both shapes remain contract v1, compatible with the v2/v3 geography/scenario input aliases.
No released contract or schema is changed.

### Named-file capture, provenance and terms

The 2026-10-07 `census-access` decision, option A with limits under the operator's standing
directive, authorizes these named Census population-estimate and Gazetteer works as public
domain US government materials (17 USC 105), with source attribution. The existing
`us-census-public-domain` id applies. This implements that decision; it is not a new licence
acceptance by this worker. The URLs above and the exact pins in
`crates/koplik-ingest/manifests/census-population-2025.json` are the entire four-file scope:

| File | SHA-256 | Bytes |
| --- | --- | --- |
| `NST-EST2025-ALLDATA.csv` | `92188e29cb0a67dcf95afa7d6c47359409782f086478b70ea4128eb70e223ca9` | 53555 |
| `co-est2025-alldata.csv` | `4f5a499d851e2cb48fd7a5405e5a9235453a8a66933657aacd10df0e264f35d5` | 2071735 |
| `2025_Gaz_counties_national.zip` | `4c90d0f805779923b5958ab13d0c1e9b99fe4932b786bfcf75dd739bb2dcb4ea` | 138993 |
| `2025_Gaz_state_national.zip` | `5c0bb56f4824af366538d73bffd229e790d301356624302eeca24d09cf27ba30` | 2863 |

All captures reuse #1350's `census_files::fetch_to_store` and the same polite fetcher/store,
with `KOPLIK_CONTACT=https://github.com/jakedevar`. A single `fetch census-population` run
requests the four files sequentially, with the enforced per-host delay; each URL gets at most
one GET per run, without retries, redirects, directory walking or link following. A verified
cached snapshot matching a pin returns its original retrieval receipt without any request.
Robots rules remain honored for every URL outside these named-file manifests. No API key
is required, requested, or placed in provenance. Changed bytes fail pin verification before
storage. Normal public parsers also verify source id, exact URL, terms id, HTTP success,
byte length and pinned snapshot hash before emitting rows.

Initial pin discovery followed the same process documented for #1350: the fixed four named
URLs were queried once through that exception against an explicitly nonmatching empty-body
digest; `PinMismatch` reported the measured hashes without storing data. The measured
manifest was committed at `ef54d28` **before** one sequential capture. Discovery never
updates a manifest or accepts revised bytes automatically. The real-byte files and original
retrieval receipts are in `data/fixtures/census-population/`; nothing was trimmed or edited.

### Parser scope and missing data

Population CSVs use exact named headers and summary levels: `040` from the state file
(50 states, DC, Puerto Rico; 52 rows) and `050` with `STATE=48` from the county file (254 rows).
State totals, US totals, regions/divisions and counties outside Texas are excluded from the
appropriate output. The county CSV has Latin-1 text outside Texas; the parser reads byte
records and decodes only the numeric/key fields, preserving source bytes and avoiding lossy
UTF-8 replacement. It does not use county-name spelling to identify population rows.

The Gazetteer selects exactly `2025_Gaz_state_national.txt` or
`2025_Gaz_counties_national.txt`, not a ZIP member containing a convenient substring. It
bounds decompression, requires pipe-delimited columns, filters county GEOIDs by Texas FIPS,
and emits ordered `Geography` rows. It never extracts archive paths to disk.

Every expected geography must occur exactly once. Missing/duplicate/unexpected geography
keys refuse the complete export and identify the affected FIPS; an incomplete national file
cannot pass merely because its row count looks plausible. Population v1 has no missing-count
variant, so blank, suppressed, malformed, negative, fractional or overflowing population
values refuse population export with the geography named in the error; there is no zero
fill, omission or imputation. A genuine numeric zero remains zero. Missing, malformed,
nonfinite or out-of-range coordinate pairs become `Geography.centroid: null`, v1's explicit
missing form. A genuine `(0, 0)` remains a point. The pinned snapshots have no gaps for the
52 published state-level geographies or 254 Texas counties; all their internal points are
present. Other US territories are outside these population/Gazetteer files' published scope.

`fetch census-population` captures all four files in one governed run; individual source
commands are `census-state-population`, `census-county-population`, `census-texas-counties` and
`census-states`. The corresponding `parse` commands run entirely offline with optional
`--out FILE` and emit v1 arrays. Each population/geography row retains the raw-file SHA-256,
exact source URL, retrieval time and `us-census-public-domain` id. Gaines County (48165) in
these bytes has resident population **23,956** and internal point **32.743942, -102.631561**.
Statewide Texas population is **31,709,821**. No population or coordinate is imputed.
