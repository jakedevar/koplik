//! A source is a stable id, an exact URL and a licence id; fetching one stores the bytes and
//! logs the retrieval. Connectors (CDC today; Texas DSHS, Census, ... later) build a
//! [`SourceSpec`] and parse the stored bytes; they never touch the network or the disk layout.

use crate::error::Result;
use crate::http::HttpClient;
use crate::polite::{PoliteFetcher, Timekeeper};
use crate::store::{PutOutcome, Retrieval, RetrievalMeta, SnapshotStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpec {
    /// Stable id listed in `SOURCES.md`.
    pub source_id: String,
    /// Exact URL, including the full query string for API sources.
    pub url: String,
    /// Licence or terms id listed in `SOURCES.md`.
    pub licence_id: String,
}

/// Fetch `spec` politely and record the bytes and the retrieval in `store`.
pub fn fetch_to_store<C: HttpClient, T: Timekeeper>(
    fetcher: &mut PoliteFetcher<C, T>,
    store: &SnapshotStore,
    spec: &SourceSpec,
) -> Result<(Retrieval, PutOutcome)> {
    let got = fetcher.fetch(&spec.url)?;
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

    #[test]
    fn refetching_unchanged_bytes_adds_a_log_line_but_no_blob() {
        let c = FakeClient::default();
        c.on("https://h.example/robots.txt", FakeClient::status(404, b""));
        c.on("https://h.example/d.json?a=1", FakeClient::ok(b"[1,2,3]"));
        let mut f = PoliteFetcher::new(c, FakeTime::default(), PoliteConfig::default());
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        let spec = SourceSpec {
            source_id: "s".into(),
            url: "https://h.example/d.json?a=1".into(),
            licence_id: "l".into(),
        };
        let (r1, o1) = fetch_to_store(&mut f, &store, &spec).unwrap();
        let (r2, o2) = fetch_to_store(&mut f, &store, &spec).unwrap();
        assert_eq!((o1, o2), (PutOutcome::Created, PutOutcome::AlreadyPresent));
        assert_eq!(r1.sha256, r2.sha256);
        assert_eq!(r1.http_status, 200);
        assert_eq!(r1.url, spec.url);
        assert_eq!(store.retrievals(Some("s")).unwrap().len(), 2);
    }
}
