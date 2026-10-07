# Sources

Every external source Koplik ingests: where it lives, the terms it comes with, how often it
changes and when someone last checked. The `source_id` is the id carried in every provenance
record; the `licence_id` is defined in the licence table below. A row marked **operator
decision required before publishing** must not feed anything public until the operator
clears it (AGENTS.md: accepting a data licence is an operator decision).

| source_id | Source | Exact URL (as recorded in provenance) | Licence id | Cadence | Last verified |
| --- | --- | --- | --- | --- | --- |
| `cdc-nndss-weekly-measles` | CDC NNDSS Weekly Data, measles rows (data.cdc.gov dataset `x9gk-5huc`) | see "CDC NNDSS query" below | `cdc-open-data-terms-unconfirmed` | Weekly (CDC republishes the weekly tables; dataset last updated 2026-09-30) | 2026-10-07 |
| `census-cb-2024-states-20m` | US Census Bureau, 2024 cartographic state boundaries, 1:20m | https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip | `us-census-public-domain` | Annual vintage; pinned to 2024 | 2026-10-07 (catalog verified; live ZIP blocked by robots) |
| `census-cb-2024-counties-20m` | US Census Bureau, 2024 cartographic county boundaries, 1:20m; derived output filters STATEFP=48 | https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_county_20m.zip | `us-census-public-domain` | Annual vintage; pinned to 2024 | 2026-10-07 (catalog verified; live ZIP blocked by robots) |
| `cdc-schoolvaxview-kindergarten` | CDC SchoolVaxView, kindergarten MMR and any exemptions (Socrata `ijqb-a7ye`) | Query below; exact URL in retrieval metadata | `cdc-schoolvaxview-terms-unconfirmed` | Annual school year | 2026-10-07 |
| `texas-dshs-kindergarten-2023` | Texas DSHS 2023–24 kindergarten coverage, published county worksheet | https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/2023-2024_School_Vaccination_Coverage_Levels_Kindergarten.xlsx | `texas-dshs-terms-unconfirmed` | Annual | 2026-10-07 |
| `texas-dshs-kindergarten-2024` | Texas DSHS 2024–25 kindergarten coverage, published county worksheet | https://www.dshs.texas.gov/sites/default/files/LIDS-Immunizations/xls/2024-2025_School_Vaccination_Coverage_Levels_Kindergarten.xlsx | `texas-dshs-terms-unconfirmed` | Annual | 2026-10-07 |
| `texas-dshs-county-fips` | Texas DSHS county name/FIPS crosswalk (identity only) | https://www.dshs.texas.gov/center-health-statistics/texas-county-numbers-public-health-regions | `texas-dshs-terms-unconfirmed` | As revised | 2026-10-07 |

## Licences and terms

| licence_id | Terms | Status |
| --- | --- | --- |
| `us-census-public-domain` | US Census Bureau geographic materials are public domain US government works. [2024 technical documentation, §1.2](https://www2.census.gov/geo/pdfs/maps-data/data/tiger/tgrshp2024/TGRSHP2024_TechDoc.pdf) states Census materials may be reproduced and requests source attribution. Boundaries are statistical depictions, not legal land descriptions (§1.1). | Public domain; cite the US Census Bureau in provenance and display attribution. |
| `cdc-open-data-terms-unconfirmed` | The dataset page and its metadata name no licence (`license: null`); the publisher is CDC's Office of Public Health Data, Surveillance, and Technology (contact `NNDSSWeb@cdc.gov`). CDC data is a US federal agency product, but nobody has confirmed the reuse terms for this dataset. The dataset's own notes say counts are provisional, subject to ongoing revision, and "presented as published each week". | **Operator decision required before publishing.** Koplik shows the figures only with their provenance and the demonstration disclaimer; confirm the terms (or ask CDC) before the site goes public. |

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
This bounds output without silently removing data. **Full-file sizes have not been measured**
because no boundary ZIP was retrieved in this worker session.

**Pipeline entry points.** `fetch(&mut polite_fetcher, &store)` stores both ZIPs with the
existing write-once snapshot store. `write_latest(&store, output_dir)` converts both verified
latest snapshots before writing `states.geojson` and `tx-counties.geojson`; the pipeline
should pass `web/public/data/geo/` as the directory. For a historical build use a store
containing the selected retrievals (conversion is deterministic for identical bytes and
retrieval metadata; a new retrieval time correctly changes provenance).

```
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- fetch census-boundaries
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- parse census-boundaries --out web/public/data/geo
```

**Current capture blocker (#1373).** On 2026-10-07 the live fetcher refused the ZIPs. The
unmodified robots response at `data/fixtures/census/robots.txt` starts with an empty
`User-agent: *` followed by `User-agent: RavenCrawler` and `Disallow: /`. Under
[RFC 9309 §2.1–2.2](https://www.rfc-editor.org/rfc/rfc9309.html#section-2.2), consecutive
user-agent lines share rules and intervening blank lines do not separate groups. Thus the
wildcard disallows the download. No transport override, crawler impersonation, or fabricated
boundary fixture is supplied. Real boundary fixture tests and full-file byte measurements
remain required once permitted primary source bytes are available; the diagnostic robots
fixture is **not** a substitute for that acceptance criterion.
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
