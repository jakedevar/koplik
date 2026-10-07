# Sources

Every external source Koplik ingests: where it lives, the terms it comes with, how often it
changes and when someone last checked. The `source_id` is the id carried in every provenance
record; the `licence_id` is defined in the licence table below. A row marked **operator
decision required before publishing** must not feed anything public until the operator
clears it (AGENTS.md: accepting a data licence is an operator decision).

| source_id | Source | Exact URL (as recorded in provenance) | Licence id | Cadence | Last verified |
| --- | --- | --- | --- | --- | --- |
| `cdc-nndss-weekly-measles` | CDC NNDSS Weekly Data, measles rows (data.cdc.gov dataset `x9gk-5huc`) | see "CDC NNDSS query" below | `cdc-open-data-terms-unconfirmed` | Weekly (CDC republishes the weekly tables; dataset last updated 2026-09-30) | 2026-10-07 |
| `census-state-population-2025` | Census Population Estimates Program, Vintage 2025 state totals | `https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/state/totals/NST-EST2025-ALLDATA.csv` | `census-ftp-terms-unconfirmed` | Annual vintage | 2026-10-06 |
| `census-county-population-2025` | Census Population Estimates Program, Vintage 2025 county totals | `https://www2.census.gov/programs-surveys/popest/datasets/2020-2025/counties/totals/co-est2025-alldata.csv` | `census-ftp-terms-unconfirmed` | Annual vintage | 2026-10-06 |
| `census-county-gazetteer-2025` | Census 2025 Gazetteer, national counties (Texas GEOIDs and internal points) | `https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_counties_national.zip` | `census-ftp-terms-unconfirmed` | Annual | 2026-10-06 |
| `census-state-gazetteer-2025` | Census 2025 Gazetteer, national states (state descriptors and internal points) | `https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2025_Gazetteer/2025_Gaz_state_national.zip` | `census-ftp-terms-unconfirmed` | Annual | 2026-10-06 |

## Licences and terms

| licence_id | Terms | Status |
| --- | --- | --- |
| `cdc-open-data-terms-unconfirmed` | The dataset page and its metadata name no licence (`license: null`); the publisher is CDC's Office of Public Health Data, Surveillance, and Technology (contact `NNDSSWeb@cdc.gov`). CDC data is a US federal agency product, but nobody has confirmed the reuse terms for this dataset. The dataset's own notes say counts are provisional, subject to ongoing revision, and "presented as published each week". | **Operator decision required before publishing.** Koplik shows the figures only with their provenance and the demonstration disclaimer; confirm the terms (or ask CDC) before the site goes public. |
| `census-ftp-terms-unconfirmed` | The Census Bureau publishes these statistical products as public data, and Census employee-created works generally are not subject to U.S. copyright; the FTP release pages do not state dataset-specific reuse terms. | **Operator decision required before publishing.** Confirm applicable reuse terms for the PEP estimates and Gazetteer files before public release. |

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
6. Caveat not verified here: whether NNDSS measles rows contain confirmed cases only or confirmed and
   probable. The dataset notes say "cases"; `WeeklyCaseCount.confirmed` is filled from them as published.
   Re-check against CDC's measles case classification before the UI says "confirmed".

**Fixture.** `data/fixtures/cdc/nndss-measles-weekly.json` is the unmodified response retrieved
2026-10-07T02:19:30Z, sha256 `c4f6862d093b10c59b3519bdef76864d4d95df10a5068f8c829ad5d95d3f3f0e`
(see `data/fixtures/cdc/README.md`).

## Census population and geography (`census-*-2025`)

The connector uses the latest PEP vintage covering 2024: Vintage 2025, with `POPESTIMATE2025`
as the July 1, 2025 resident population estimate. The state file supplies state-level rows and
the county file supplies Texas county rows. The 2025 Gazetteer internal-point latitude and
longitude form each state and Texas county `Geography.centroid`; these are representative
internal points, not population-weighted centroids. Every output carries snapshot SHA-256, the
exact download URL, retrieval time, and the Census terms id in provenance.

The source URLs are currently refused by the existing polite fetcher because `robots.txt` for
`www2.census.gov` disallows these paths. No bytes have been archived yet; do not treat any
population or centroid output as available until an operator-approved, robots-compliant source
path is established and real-byte fixtures have been committed. The Census 2025 release pages
identify Vintage 2025 as the latest completed vintage and describe the Gazetteer coordinates as
representative latitude/longitude values. Terms are left for operator decision because no
dataset-specific reuse licence was verified.
