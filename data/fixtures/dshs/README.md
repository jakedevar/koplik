# Texas DSHS outbreak report fixtures

Every file here is a **byte-for-byte copy** of a snapshot-store blob (nothing trimmed, no
value edited), next to `<name>.retrieval.json`, the store's retrieval-log line for it (source
id, exact URL, retrieval time, SHA-256, licence id). `fixtures.tsv` lists name and SHA-256;
`export-fixtures.sh` re-creates them from a store (the commands to fill one are in the
script). The `fixtures_are_the_recorded_snapshots` test re-hashes every file.

| Fixture | What it is | Format |
| --- | --- | --- |
| `page-2025-03-04.html` | Internet Archive capture (2025-03-05 18:45:28 UTC) of the outbreak page, report dated March 4 | HTML county table ("Texas Case Count by County") |
| `page-2025-03-25.html.gz` | capture 2025-03-26 07:56 UTC, report dated March 25; gzip bytes exactly as the Archive replayed them | HTML county table, caption "Texas Outbreak Case Count by County", 15 counties |
| `page-2025-03-28.html.gz` | capture 2025-03-29 02:44 UTC, report dated March 28: the first version with the Tableau dashboard in place of the table | narrative total only |
| `page-2025-04-22.html` | capture 2025-04-24, report dated April 22; its "not associated" table has `*`/`**` footnoted cells | narrative total only |
| `page-2025-05-30.html.gz` | capture 2025-05-31, report dated May 30; its "not associated" table rows add to 35 against a printed total of 32 (a real inconsistency, kept visible) | narrative total only |
| `page-live-2026-10-07.html` | the live page as served on 2026-10-07 (report dated August 12, the last update) | narrative total only |
| `report-2025-11-24.pdf`, `report-2025-12-23.pdf` | "2025 Measles Data Report" PDFs, Internet Archive captures of 2025-12-17 and 2026-01-08 | PDF Table 1 |
| `report-2026-01-12.pdf` | the same report dated 1/12/26 as served live on 2026-10-07 (the Archive's capture of 2026-02-09 has identical bytes) | PDF Table 1 |
| `census-national_county2020.txt` | Census national county reference file, Internet Archive capture 2025-02-06 (see `SOURCES.md` for why not the live host) | county names to FIPS |
| `cdx-outbreak-page-20250301-20250910.json` | the Archive's CDX listing of first-capture-per-day of the outbreak page, 2025-03-01 to 2025-09-10 (129 captures) | listing |

The parser tests assert values measured from these bytes. Do not re-record a file in place; add
new snapshots under new names.
