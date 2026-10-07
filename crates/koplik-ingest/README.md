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
| unset | `polite::DEFAULT_CONTACT` = `https://github.com/jakedevar/koplik` (the public Koplik repository; decision `ingest-contact`, #1413, switched at the public release, #1449) |
| set, non-blank | that value (an e-mail address or repository URL), trimmed |
| set but empty or blank | none: live fetching **refuses** before opening the store or sending any request (explicit opt-out) |

```bash
koplik-ingest fetch cdc-cases                                  # default contact
KOPLIK_CONTACT=ops@example.org koplik-ingest fetch cdc-cases   # override
KOPLIK_CONTACT= koplik-ingest fetch cdc-cases                  # refuses
```

### Census hosts: `KOPLIK_CENSUS_CONTACT`

Requests to the US Census Bureau (`www2.census.gov` and any `*.census.gov` host; an archive.org
request is not a Census request) identify the operator's own Census contact, per the operator's
2026-10-07 instruction (#1427). That address is **never committed**: it comes from, in order,

1. the `KOPLIK_CENSUS_CONTACT` environment variable;
2. a `KOPLIK_CENSUS_CONTACT=...` line in `.env.local` at the repository root (gitignored by the
   `.env.*` rule; plain `KEY=VALUE` lines, `#` comments, optional quotes);
3. otherwise the general contact above, with a one-line notice on stderr at the first Census
   request.

A Census contact that is set but blank (in the environment or in `.env.local`) **refuses**, like a
blank `KOPLIK_CONTACT`. A non-UTF-8 value refuses with its own message. Every other host keeps the
general contact, including a redirect from a Census host to a non-Census host. All of this is
resolved in `PoliteConfig::live_from_env`, the one entry point for every live path: the CLI, the
examples, and the pipeline's ingest stage (the pipeline, #1359, is expected to call it when it
lands; until then the CLI and examples are the live paths).

Parsing, listing and every test are offline and need no contact.

Tests run offline against `data/fixtures/`; the HTTP client is injected (`http::HttpClient`).
