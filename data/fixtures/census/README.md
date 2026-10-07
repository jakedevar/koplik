# Census capture status — boundary fixtures still required

`robots.txt` is the unmodified response from `https://www2.census.gov/robots.txt`,
recorded by the existing ingest HTTP client and snapshot store. It is diagnostic policy
text, **not** a state/county boundary fixture. The retrieval record is
`robots.retrieval.json`; its SHA-256 is
`d85e1ad2100fdc145e5b2c35553488738d62d57fb9da34c5322f8d63757195a1`,
retrieved `2026-10-07T02:30:41Z`, HTTP 200, 398 bytes.

The `real_census_robots_policy_blocks_fetch_before_any_zip_or_snapshot` offline test
reproduces the live download refusal. The empty wildcard and subsequent RavenCrawler
agent line share `Disallow: /` under RFC 9309, even with a blank line between them.
Follow-up #1373 tracks obtaining permitted primary Census bytes. Do not bypass this
restriction or edit the policy fixture.

The diagnostic capture was performed with:

```bash
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest --example capture_census_robots
```

This example only GETs `/robots.txt` (implicitly allowed by RFC 9309 §2.2.2), records
it in `data/snapshots`, and copies the real response into this directory. Do not rerun
it to overwrite this committed historical fixture; use a new output name for a newer
capture. The initial connector fetch was:

```bash
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- fetch census-boundaries --store data/snapshots
```

Once the source policy permits Koplik, capture both actual ZIPs through that command,
keep their exact URL/retrieval time/raw digest records, and add boundary fixtures and
success-path tests. No SHP/DBF values or coordinates may be hand-edited. Add a documented, re-runnable offline extraction command to retain a few real
records with SHP/DBF record payloads unchanged.
Test both full files against the 1,000,000-byte artifact ceiling and repeat conversion
with identical retrieval metadata to verify byte-identical output. Assert feature FIPS,
source names, all 254 Texas counties in the full input (if actually supplied), holes,
closed rings, rounding, and provenance from the recorded bytes; measure rather than guess.
