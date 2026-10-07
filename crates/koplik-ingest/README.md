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

## `KOPLIK_CONTACT` (required for live fetching)

Every request identifies the client as
`koplik-ingest/<version> (measles data demonstration project; <contact>)`. The contact is
never invented: `fetch` **refuses to run**, before opening the store or sending anything,
unless `KOPLIK_CONTACT` is set to a non-blank, verified e-mail address or repository URL.

```bash
KOPLIK_CONTACT=ops@example.org koplik-ingest fetch cdc-cases
```

`make pipeline` fetches, so it needs `KOPLIK_CONTACT` too until the operator's verified default is
committed (decision record `ingest-contact`). Parsing, listing and every test are offline and need no
contact.

Tests run offline against `data/fixtures/`; the HTTP client is injected (`http::HttpClient`).
