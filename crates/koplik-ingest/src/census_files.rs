//! The operator's `census-access` decision (2026-10-07, option A with limits) permits
//! direct downloads of a fixed, committed set of public-domain Census files. This is not
//! a crawler exemption: only exact HTTPS URLs in a reviewed, SHA-256-pinned manifest qualify.
//! See SOURCES.md for the RFC 9309 group reading and the scope of the decision.

use std::collections::BTreeMap;

use koplik_contracts::v1::Sha256Hex;
use serde::Deserialize;

use crate::error::{IngestError, Result};
use crate::http::HttpClient;
use crate::polite::{PoliteFetcher, Timekeeper};
use crate::source::SourceSpec;
use crate::store::{PutOutcome, Retrieval, RetrievalMeta, SnapshotStore};

/// A validated manifest. No wildcards, directories, alternate hosts, or redirects qualify.
#[derive(Debug, Clone)]
pub struct NamedFileAllowlist(BTreeMap<String, Sha256Hex>);

impl NamedFileAllowlist {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Pin {
            url: String,
            sha256: Sha256Hex,
        }
        let pins: Vec<Pin> = serde_json::from_slice(bytes)
            .map_err(|e| IngestError::Invalid(format!("Census file manifest: {e}")))?;
        Self::new(pins.into_iter().map(|p| (p.url, p.sha256)))
    }

    pub fn new(entries: impl IntoIterator<Item = (String, Sha256Hex)>) -> Result<Self> {
        let mut pins = BTreeMap::new();
        for (url, hash) in entries {
            let path = url
                .strip_prefix("https://www2.census.gov/")
                .ok_or_else(|| {
                    IngestError::Invalid(format!(
                        "Census manifest requires exact https://www2.census.gov/ URL: {url}"
                    ))
                })?;
            let supported = (path.starts_with("geo/tiger/GENZ")
                || path.starts_with("geo/tiger/TIGER"))
                || path.starts_with("geo/docs/maps-data/data/gazetteer/")
                || path.starts_with("programs-surveys/popest/datasets/");
            let file = path.ends_with(".zip") || path.ends_with(".csv") || path.ends_with(".txt");
            if !supported
                || !file
                || path.split('/').any(|s| matches!(s, "" | "." | ".."))
                || !path
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.'))
            {
                return Err(IngestError::Invalid(format!(
                    "not a named cartographic, Gazetteer, or population file: {url}"
                )));
            }
            if pins.insert(url.clone(), hash).is_some() {
                return Err(IngestError::Invalid(format!(
                    "duplicate Census manifest URL: {url}"
                )));
            }
        }
        if pins.is_empty() {
            return Err(IngestError::Invalid("empty Census manifest".into()));
        }
        Ok(Self(pins))
    }

    pub fn pin(&self, url: &str) -> Option<&Sha256Hex> {
        self.0.get(url)
    }
}

/// Cached, verified pins need no network request and preserve their original provenance.
/// A new manifest pin is a reviewed source update, never an automatic acceptance of changes.
pub fn fetch_to_store<C: HttpClient, T: Timekeeper>(
    fetcher: &mut PoliteFetcher<C, T>,
    store: &SnapshotStore,
    spec: &SourceSpec,
    allowlist: &NamedFileAllowlist,
) -> Result<(Retrieval, PutOutcome)> {
    let pin = allowlist.pin(&spec.url).ok_or_else(|| {
        IngestError::Invalid(format!("URL missing from Census manifest: {}", spec.url))
    })?;
    if let Some(record) = store
        .retrievals(Some(&spec.source_id))?
        .into_iter()
        .rev()
        .find(|r| {
            r.url == spec.url
                && r.sha256 == *pin
                && r.licence_id == spec.licence_id
                && (200..=299).contains(&r.http_status)
        })
    {
        store.get_verified(&record.sha256)?;
        return Ok((record, PutOutcome::AlreadyPresent));
    }
    let got = fetcher.fetch_census_file(&spec.url, allowlist)?;
    store.record(
        RetrievalMeta {
            source_id: spec.source_id.clone(),
            url: spec.url.clone(),
            retrieved_at: got.retrieved_at,
            licence_id: spec.licence_id.clone(),
            http_status: got.status,
            content_type: got.content_type,
        },
        &got.body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polite::PoliteConfig;
    use crate::polite::fakes::{FakeClient, FakeTime};
    use crate::store::sha256_of;

    const URL: &str = "https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip";
    const OTHER: &str = "https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_county_20m.zip";
    // Real recorded bytes stand in for the opaque response body in transport tests; they
    // are not represented as boundary data or parsed as ZIPs by these tests.
    const BODY: &[u8] = include_bytes!("../../../data/fixtures/census/robots.txt");
    fn manifest() -> NamedFileAllowlist {
        NamedFileAllowlist::new([(URL.into(), sha256_of(BODY))]).unwrap()
    }
    fn source() -> SourceSpec {
        SourceSpec {
            source_id: "transport-test".into(),
            url: URL.into(),
            licence_id: "us-census-public-domain".into(),
        }
    }
    fn fetcher(client: FakeClient, time: FakeTime) -> PoliteFetcher<FakeClient, FakeTime> {
        PoliteFetcher::new(
            client,
            time,
            PoliteConfig::live(Some("https://github.com/jakedevar")).unwrap(),
        )
    }

    #[test]
    fn exact_pin_bypasses_disallow_but_other_url_still_obeys_robots() {
        let c = FakeClient::default();
        c.on("https://www2.census.gov/robots.txt", FakeClient::ok(BODY));
        c.on(URL, FakeClient::ok(BODY));
        let time = FakeTime::default();
        let mut f = fetcher(c.clone(), time.clone());
        assert!(matches!(
            f.fetch_census_file(OTHER, &manifest()),
            Err(IngestError::RobotsDisallowed { .. })
        ));
        assert_eq!(f.fetch_census_file(URL, &manifest()).unwrap().body, BODY);
        assert_eq!(c.calls.borrow().len(), 2);
        assert_eq!(c.calls.borrow()[1].0, URL);
        assert!(
            c.calls
                .borrow()
                .iter()
                .all(|(_, agent)| agent.contains("https://github.com/jakedevar"))
        );
        assert!(
            time.sleeps
                .borrow()
                .iter()
                .any(|d| *d >= std::time::Duration::from_secs(1))
        );
    }

    #[test]
    fn mismatch_reports_both_hashes_and_stores_nothing() {
        let c = FakeClient::default();
        c.on(URL, FakeClient::ok(BODY));
        let bad =
            NamedFileAllowlist::new([(URL.into(), sha256_of(b"not the recorded bytes"))]).unwrap();
        let mut f = fetcher(c.clone(), FakeTime::default());
        let temp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(temp.path()).unwrap();
        let error = fetch_to_store(&mut f, &store, &source(), &bad).unwrap_err();
        match error {
            IngestError::PinMismatch {
                expected, actual, ..
            } => {
                assert_eq!(expected, sha256_of(b"not the recorded bytes").to_string());
                assert_eq!(actual, sha256_of(BODY).to_string());
            }
            e => panic!("unexpected error: {e}"),
        }
        assert!(store.retrievals(None).unwrap().is_empty());
        assert!(matches!(
            f.fetch_census_file(URL, &bad),
            Err(IngestError::NamedFileAlreadyRequested(_))
        ));
        assert_eq!(c.calls.borrow().len(), 1);
    }

    #[test]
    fn cached_pin_skips_every_network_request_and_preserves_provenance() {
        let temp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(temp.path()).unwrap();
        let c = FakeClient::default();
        c.on(URL, FakeClient::ok(BODY));
        let mut f = fetcher(c.clone(), FakeTime::default());
        let (first, _) = fetch_to_store(&mut f, &store, &source(), &manifest()).unwrap();
        let never = FakeClient::default();
        let mut cached = fetcher(never.clone(), FakeTime::default());
        let (again, outcome) = fetch_to_store(&mut cached, &store, &source(), &manifest()).unwrap();
        assert_eq!(first, again);
        assert_eq!(outcome, PutOutcome::AlreadyPresent);
        assert!(never.calls.borrow().is_empty());
        assert_eq!(store.retrievals(None).unwrap().len(), 1);
        // A cache claim alone never overrides corruption.
        std::fs::write(store.blob_path(&first.sha256), b"damaged").unwrap();
        assert!(matches!(
            fetch_to_store(&mut cached, &store, &source(), &manifest()),
            Err(IngestError::BlobCorrupt { .. })
        ));
        assert!(never.calls.borrow().is_empty());
    }

    #[test]
    fn named_file_does_not_retry_errors_or_follow_redirects() {
        for status in [302, 429, 503] {
            let c = FakeClient::default();
            let mut response = FakeClient::status(status, b"").unwrap();
            response.location = Some(OTHER.into());
            c.on(URL, Ok(response));
            let mut f = fetcher(c.clone(), FakeTime::default());
            assert!(matches!(
                f.fetch_census_file(URL, &manifest()),
                Err(IngestError::BadStatus { .. })
            ));
            assert!(matches!(
                f.fetch_census_file(URL, &manifest()),
                Err(IngestError::NamedFileAlreadyRequested(_))
            ));
            assert_eq!(c.calls.borrow().len(), 1);
            assert_eq!(c.calls.borrow()[0].0, URL);
        }
    }

    #[test]
    fn rejects_broad_or_ambiguous_allowlists() {
        for url in [
            "https://example.org/file.zip",
            "http://www2.census.gov/geo/tiger/GENZ2024/shp/x.zip",
            "https://www2.census.gov/geo/tiger/GENZ2024/shp/",
            "https://www2.census.gov/robots.txt",
            "https://www2.census.gov/geo/tiger/GENZ2024/shp/x.zip?download=1",
            "https://www2.census.gov/geo/tiger/GENZ2024/../private.zip",
            "https://www2.census.gov/geo/tiger/GENZ2024/shp/%78.zip",
        ] {
            assert!(
                NamedFileAllowlist::new([(url.into(), sha256_of(BODY))]).is_err(),
                "{url}"
            );
        }
        assert!(
            NamedFileAllowlist::new([(URL.into(), sha256_of(BODY)), (URL.into(), sha256_of(BODY))])
                .is_err()
        );
    }
}
