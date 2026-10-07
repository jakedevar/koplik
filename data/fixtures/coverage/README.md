# Kindergarten coverage source snapshots

All files below are complete, untrimmed live response bytes, captured through the ingest
crate's existing polite fetcher and content-addressed store. No values were edited. Each
`<file>.retrieval.json` is the exact retrieval record; the offline tests verify SHA-256 and
byte count. Never re-record these files in place: choose new output names for newer bytes.

| File | Retrieval time (UTC) | SHA-256 |
| --- | --- | --- |
| `cdc-2023-25.json` | 2026-10-07T02:27:44Z | `36dde045aff5579612edced09a49f47bcb8a607a40dbf7b0332aed5428d378d6` |
| `texas-2023-24.xlsx` | 2026-10-07T02:27:22Z | `79d242139bfb4ca0247e90a588541bf14435db52f0a87fb1182535878232bbba` |
| `texas-2024-25.xlsx` | 2026-10-07T02:27:35Z | `3568feaf4bb383351b9bf5117e21b4275f08ccae6feff45bf2140c80756b10d2` |
| `texas-county-fips.html` | 2026-10-07T02:30:41Z | `ea80fb5af16a53eb67d0a504dec9b1fcc159c8e5d4baf57d972aa21dacbfbd82` |
| `census-api-missing-key.html` | 2026-10-07T02:29:49Z | `c2f4687e09b80676de7f68dec92ebea395209616dbc6f895520e547c5f68c0f5` |

The last file is a **rejected discovery response**, HTTP 200 but HTML asking for an API key.
It is a regression input for refusing a non-data response, not an ingested crosswalk. We
requested no credentials. See #1374 and SOURCES.md. The DSHS crosswalk is the identity source.

To repeat a capture with the exact recorded URL, source id and terms id, choose a new output
path and use the fixture recorder (run from the repository root):

Live capture requires an operator-supplied contact in `KOPLIK_CONTACT`; a missing or blank
contact refuses before creating a store or sending a request. Offline parsing needs no contact.

```bash
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest --example coverage_fetch -- SOURCE_ID 'EXACT_URL' LICENCE_ID NEW_OUTPUT_PATH
```

The three parameters come directly from that fixture's retrieval JSON. The recorder writes
new files exclusively (`create_new`), retrieves through the polite fetcher, records immutable
raw bytes in `data/snapshots/`, then copies the verified blob and retrieval record. Repeating
the URL can yield revised source bytes; it does not recreate a past response. There is no
trimming command because nothing was trimmed.

Normal ingestion uses `fetch cdc-coverage` and `fetch texas-coverage --year 2023|2024`.
Offline fixture-seeded store and CLI parsing, including gaps reports, are exercised by:

```bash
~/.rsi/bin/cargo-slot cargo test -p koplik-ingest --test coverage_parser --offline
```

Gaps reports under `data/reports/coverage/` are derived v1 rows, not raw fixtures. That test
re-parses these pinned source snapshots and asserts the reports match exactly.
