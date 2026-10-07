# koplik-ingest

The only crate that touches the network: a polite fetcher, a write-once content-addressed
snapshot store (`data/snapshots/`, gitignored) and the source connectors. The CDC connector
parses stored bytes into **contracts v3** `WeeklyCaseCount` rows (`cases` +
`case_definition = confirmed_or_unknown_status`), each with a v1 `Provenance` (snapshot sha256,
source URL, retrieval time, licence id). See `SOURCES.md` for sources, terms and derivations.

```bash
koplik-ingest fetch cdc-cases [--store DIR] [--first-year Y] [--last-year Y]   # network
koplik-ingest parse cdc-cases [--store DIR] [--out FILE]                       # offline
koplik-ingest list [--store DIR] [--source ID]                                 # offline
```

## User-Agent and `KOPLIK_CONTACT`

Every live request identifies the client as
`koplik-ingest/<version> (measles data demonstration project; <contact>)`. The contact comes
from one resolver, `polite::contact_from_env` (used by `PoliteConfig::live_from_env`, so by every
live CLI command and the pipeline's ingest stage):

| `KOPLIK_CONTACT` | contact sent |
| --- | --- |
| unset | `polite::DEFAULT_CONTACT` = `https://github.com/jakedevar` (operator's public profile; decision `ingest-contact`, #1413) |
| set, non-blank | that value (an e-mail address or repository URL), trimmed |
| set but empty or blank | none: live fetching **refuses** before opening the store or sending any request (explicit opt-out) |

```bash
koplik-ingest fetch cdc-cases                                  # default contact
KOPLIK_CONTACT=ops@example.org koplik-ingest fetch cdc-cases   # override
KOPLIK_CONTACT= koplik-ingest fetch cdc-cases                  # refuses
```

Parsing, listing and every test are offline and need no contact. When a public Koplik repository
exists, switch `DEFAULT_CONTACT` to its URL.

Tests run offline against `data/fixtures/`; the HTTP client is injected (`http::HttpClient`).
