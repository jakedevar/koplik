# Census Vintage 2025 population and Gazetteer snapshots

These are the complete **unmodified** live response bytes captured on 2026-10-07 by
`koplik-ingest fetch census-population`, using the shared #1350 Census named-file allowlist,
polite live contact, one fetch per file per run, sequential delay and content-addressed store.
Nothing was trimmed or edited. Each `<filename>.retrieval.json` is its original receipt.

The exact URLs, SHA-256 pins, sizes and source/terms ids are in those receipts and
`crates/koplik-ingest/manifests/census-population-2025.json`; parser tests re-hash every file.

| File | Retrieved (UTC) | Bytes |
| --- | --- | --- |
| `NST-EST2025-ALLDATA.csv` | 2026-10-07T03:55:34Z | 53555 |
| `co-est2025-alldata.csv` | 2026-10-07T03:55:35Z | 2071735 |
| `2025_Gaz_counties_national.zip` | 2026-10-07T03:55:36Z | 138993 |
| `2025_Gaz_state_national.zip` | 2026-10-07T03:55:37Z | 2863 |

The county population CSV contains Latin-1 names outside Texas; the parser uses byte
records, never lossy substitution. The ZIP members are exactly
`2025_Gaz_counties_national.txt` and `2025_Gaz_state_national.txt`; both are pipe-delimited.
`INTPTLAT/INTPTLONG` are representative internal points, not population-weighted centroids.
Population is `POPESTIMATE2025` (July 1, 2025). See SOURCES.md for scope, gaps and the
2026-10-07 `census-access` decision approving this finite public-domain named-file capture.

Historical capture command (2026-10-07; it used the contact shown):

```bash
KOPLIK_CONTACT=https://github.com/jakedevar ~/.rsi/bin/cargo-slot cargo run -p koplik-ingest -- fetch census-population
```

Future captures use the current contact configuration described in `crates/koplik-ingest/README.md`.

Matching cached pins skip every request and preserve the original receipt. Different bytes
raise a pin mismatch and are not accepted automatically. Pin discovery used the same
fixed-file exception with the empty-body digest as an explicit nonmatching guard, reporting
measured hashes only (`--example census_population_pins`). The measured manifest was
committed at `ef54d28` before capture. No directory or alternate file was probed.

Copying the already verified capture to these fixture names is a local, re-runnable operation
(the run's stdout receipts were saved as `/tmp/1352-retrievals.jsonl`):

```python
import hashlib, json, pathlib
root = pathlib.Path("data/fixtures/census-population")
root.mkdir(exist_ok=True)
for line in pathlib.Path("/tmp/1352-retrievals.jsonl").read_text().splitlines():
    r = json.loads(line)
    name = r["url"].rsplit("/", 1)[1]
    blob = pathlib.Path("data/snapshots/blobs") / r["sha256"][:2] / r["sha256"]
    raw = blob.read_bytes()
    assert len(raw) == r["bytes"]
    assert hashlib.sha256(raw).hexdigest() == r["sha256"]
    with (root / name).open("xb") as f:
        f.write(raw)
    with (root / (name + ".retrieval.json")).open("x") as f:
        f.write(line + "\n")
```

Never replace historical fixtures. Future captures need new names if they differ.
Offline tests seed temporary stores from these committed bytes and exercise parsing,
provenance, v1 round trips, CLI output and the pinned-cache path using injected clients.
Deliberate malformed-input tests mutate values **in memory only**, never source files.
