# Census 2024 cartographic boundaries (1:20m)

These are the complete, **unmodified** ZIP response bodies captured through
`koplik-ingest fetch census-boundaries`, the pinned named-file exception, and the existing
write-once snapshot store. Both archives together are 1,087,901 bytes (~1.04 MiB), so we
keep the real inputs intact rather than trim or regenerate SHP/DBF records. This allows
offline tests to check every supplied state and every Texas county, including islands.
No attribute value or coordinate was hand-edited.

| File | Exact source URL | Retrieved (UTC) | Raw bytes | Raw SHA-256 |
| --- | --- | --- | ---: | --- |
| `cb_2024_us_state_20m.zip` | https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip | 2026-10-07T03:45:26Z | 187,671 | `7bc773d83c01b6df69b8aada9c2b5983d97f22f4551947cc5c00b87c663223ef` |
| `cb_2024_us_county_20m.zip` | https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_county_20m.zip | 2026-10-07T03:45:27Z | 900,230 | `c9a54fd95422436b8822e0b070a45a9bc3de08eb13f943cdba66e7a9c2a135ff` |

The matching `.retrieval.json` files are the original store retrieval records. The exact
URL/hash pairs also live in the committed manifest:
`crates/koplik-ingest/manifests/census-boundaries-2024.json`. Licence: public-domain US Census
Bureau geographic materials; terms and the operator's named-file decision are in `SOURCES.md`.

The DBF headers contain 52 state records and 3,222 national county records. Conversion
retains all 52 supplied state features (50 states, DC and Puerto Rico) and filters the
county input to all 254 Texas counties. The source PRJ is geographic NAD83 degrees. See
`SOURCES.md` for the overview map's approximate WGS84 coordinate treatment, fixed
simplification/rounding policy, geometry validation, and provenance semantics.

Measured with these exact retrieval records and the fixed converter policy:

| Output | Features | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| `states.geojson` | 52 | 233,074 | `a0349eb44abd31691527f8f9bf287775c477070ccf430d2cebde1663c6f4e820` |
| `tx-counties.geojson` | 254 | 169,613 | `1351d270dfe641ea2ee41cd31bc139d303f308aed174dcae500267b9cdb920bd` |

Offline converter tests re-hash both raw fixtures, assert the manifest pins, repeat
conversion byte-for-byte, enforce the 1,000,000-byte output ceiling, check FIPS ordering,
source names, ring winding/closure, four-decimal coordinates, islands, and contracts v1
provenance on every feature. The pipeline writer is tested from a temporary snapshot
store populated with these same bytes and retrieval metadata.

Capture commands (all network operations in `koplik-ingest`, identified contact, sequential
paced file requests, no directory discovery or redirects):

```bash
export KOPLIK_CONTACT=https://github.com/jakedevar
export CARGO_TARGET_DIR=/tmp/koplik-1350-target
~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- fetch census-boundaries --store data/snapshots
~/.rsi/bin/cargo-slot cargo run --offline -p koplik-ingest -- parse census-boundaries --store data/snapshots --out /tmp/1350-geo
```

Initial pin discovery used this same validated exact-file exception with deliberately
nonmatching provisional digests, accepted no bytes, and reported measured hashes. The
measured pins were committed at `9bf15e6` before the fixture capture above. Runtime pin
updates are prohibited: changed source bytes raise an error naming expected and fetched
hashes, and a deliberate manifest update requires review.

Copying the captured immutable bytes and their receipts was a local operation:

```python
import hashlib, json, pathlib, shutil
for line in open("data/snapshots/retrievals.jsonl"):
    r = json.loads(line)
    if not r["source_id"].startswith("census-cb-2024-"):
        continue
    dest = pathlib.Path("data/fixtures/census") / r["url"].rsplit("/", 1)[1]
    blob = pathlib.Path("data/snapshots/blobs") / r["sha256"][:2] / r["sha256"]
    assert hashlib.sha256(blob.read_bytes()).hexdigest() == r["sha256"]
    assert not dest.exists()  # do not replace historical fixtures
    shutil.copyfile(blob, dest)
    with dest.with_suffix(".retrieval.json").open("x") as out:
        json.dump(r, out, indent=2)
        out.write("\n")
```

For a future capture use new fixture names if the historical files already exist. Cached
snapshots matching the pins skip every network request and preserve their original receipts.

## Historical robots diagnostic

`robots.txt` remains the unmodified response from `https://www2.census.gov/robots.txt`:
HTTP 200, 398 bytes, retrieved `2026-10-07T02:30:41Z`, SHA-256
`d85e1ad2100fdc145e5b2c35553488738d62d57fb9da34c5322f8d63757195a1`.
The exact record is `robots.retrieval.json`. This fixture reproduces the conservative
RFC 9309 wildcard/RavenCrawler group reading that initially blocked capture (#1373).
The `census-access` option A decision (#1375) explicitly permits only pinned named-file
downloads; normal fetches, including every other URL on the same host, still respect
robots. No crawler impersonation or broad host exception is used.

The original diagnostic capture used
`~/.rsi/bin/cargo-slot cargo run -p koplik-ingest --example capture_census_robots`.
That example only GETs `/robots.txt` (implicitly allowed by RFC 9309 §2.2.2), records it
in the snapshot store, and refuses to overwrite committed historical fixtures.
