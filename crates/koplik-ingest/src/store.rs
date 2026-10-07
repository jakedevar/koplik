//! Content-addressed, write-once snapshot store with an append-only retrieval log.
//!
//! Layout under the store root (default `data/snapshots/`, gitignored):
//!
//! ```text
//! blobs/<first 2 hex>/<sha256>   raw bytes, named by their SHA-256, never rewritten
//! retrievals.jsonl               one JSON `Retrieval` per line, append-only
//! tmp/                           staging area for atomic writes
//! ```
//!
//! API (small on purpose; every connector uses only these):
//! - [`SnapshotStore::put`]: store bytes, idempotent, returns the digest.
//! - [`SnapshotStore::get_verified`]: read bytes back, re-hashing them.
//! - [`SnapshotStore::record`]: put + log one retrieval, returns the [`Retrieval`].
//! - [`SnapshotStore::retrievals`] / [`latest`](SnapshotStore::latest) /
//!   [`as_of`](SnapshotStore::as_of): query the log by source and date.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, NaiveDate, SecondsFormat, TimeZone, Utc};
use koplik_contracts::v1::{Provenance, Sha256Hex};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{IngestError, Result};

/// Default store root, relative to the repository root.
pub const DEFAULT_ROOT: &str = "data/snapshots";

const LOG_FILE: &str = "retrievals.jsonl";

/// SHA-256 of `bytes` as the contracts digest type.
pub fn sha256_of(bytes: &[u8]) -> Sha256Hex {
    Sha256Hex::new(hex::encode(Sha256::digest(bytes))).expect("hex digest is 64 lowercase hex")
}

/// Whether a `put` wrote a new blob or found the identical bytes already stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutOutcome {
    Created,
    AlreadyPresent,
}

/// What the caller knows about one retrieval; the store adds the digest and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalMeta {
    pub source_id: String,
    /// The exact URL fetched (for API sources the full query string, so a re-fetch is reproducible).
    pub url: String,
    pub retrieved_at: DateTime<Utc>,
    pub licence_id: String,
    pub http_status: u16,
    pub content_type: Option<String>,
}

/// One line of the append-only retrieval log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retrieval {
    pub source_id: String,
    pub url: String,
    /// RFC 3339 UTC, whole seconds.
    pub retrieved_at: DateTime<Utc>,
    pub sha256: Sha256Hex,
    pub licence_id: String,
    pub http_status: u16,
    pub content_type: Option<String>,
    pub bytes: u64,
}

impl Retrieval {
    /// The contracts v1 provenance record every derived row carries.
    pub fn provenance(&self) -> Provenance {
        Provenance {
            source_id: self.source_id.clone(),
            url: self.url.clone(),
            retrieved_at: self.retrieved_at,
            sha256: self.sha256.clone(),
            licence_id: self.licence_id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotStore {
    root: PathBuf,
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

impl SnapshotStore {
    /// Open (creating if needed) a store rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for dir in [root.join("blobs"), root.join("tmp")] {
            fs::create_dir_all(&dir).map_err(|e| IngestError::io(&dir, e))?;
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path where the blob with this digest lives (it may not exist).
    pub fn blob_path(&self, sha: &Sha256Hex) -> PathBuf {
        let s = sha.as_str();
        self.root.join("blobs").join(&s[..2]).join(s)
    }

    /// Store `bytes` under their SHA-256. Write-once: temp file, fsync, atomic rename; an
    /// existing blob is never opened for writing, so re-fetching unchanged bytes is a no-op.
    pub fn put(&self, bytes: &[u8]) -> Result<(Sha256Hex, PutOutcome)> {
        let sha = sha256_of(bytes);
        let dest = self.blob_path(&sha);
        if dest.exists() {
            return Ok((sha, PutOutcome::AlreadyPresent));
        }
        let dir = dest.parent().expect("blob path has a parent");
        fs::create_dir_all(dir).map_err(|e| IngestError::io(dir, e))?;
        let tmp = self.root.join("tmp").join(format!(
            "{}.{}.{}",
            sha,
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let write = || -> std::io::Result<()> {
            let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            Ok(())
        };
        if let Err(e) = write() {
            let _ = fs::remove_file(&tmp);
            return Err(IngestError::io(&tmp, e));
        }
        // `hard_link` fails if `dest` appeared meanwhile (concurrent writer), so an existing
        // blob can never be replaced; rename would silently overwrite.
        let linked = fs::hard_link(&tmp, &dest);
        let _ = fs::remove_file(&tmp);
        let outcome = match linked {
            Ok(()) => PutOutcome::Created,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => PutOutcome::AlreadyPresent,
            Err(e) => return Err(IngestError::io(&dest, e)),
        };
        // Make the new directory entry durable.
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
        Ok((sha, outcome))
    }

    /// Read a blob and verify that its bytes still hash to its address.
    pub fn get_verified(&self, sha: &Sha256Hex) -> Result<Vec<u8>> {
        let path = self.blob_path(sha);
        let mut bytes = Vec::new();
        match File::open(&path) {
            Ok(mut f) => f
                .read_to_end(&mut bytes)
                .map_err(|e| IngestError::io(&path, e))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(IngestError::BlobMissing(sha.to_string()));
            }
            Err(e) => return Err(IngestError::io(&path, e)),
        };
        let actual = sha256_of(&bytes);
        if &actual != sha {
            return Err(IngestError::BlobCorrupt {
                sha256: sha.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(bytes)
    }

    /// Store the bytes and append one record to the retrieval log. The blob is durable before
    /// the log line is written, so every log line points at an existing blob. Re-fetching
    /// unchanged bytes adds a log line but no blob.
    pub fn record(&self, meta: RetrievalMeta, bytes: &[u8]) -> Result<(Retrieval, PutOutcome)> {
        let (sha, outcome) = self.put(bytes)?;
        let retrieval = Retrieval {
            source_id: meta.source_id,
            url: meta.url,
            retrieved_at: whole_seconds(meta.retrieved_at),
            sha256: sha,
            licence_id: meta.licence_id,
            http_status: meta.http_status,
            content_type: meta.content_type,
            bytes: bytes.len() as u64,
        };
        self.append_log(&retrieval)?;
        Ok((retrieval, outcome))
    }

    fn log_path(&self) -> PathBuf {
        self.root.join(LOG_FILE)
    }

    fn append_log(&self, r: &Retrieval) -> Result<()> {
        let path = self.log_path();
        let mut line = serde_json::to_string(r).expect("Retrieval serialises");
        line.push('\n');
        // A crash mid-append can leave an unterminated fragment; terminate it so this record
        // starts on its own line (readers skip an unterminated final fragment).
        let needs_newline = match fs::read(&path) {
            Ok(b) => b.last().is_some_and(|c| *c != b'\n'),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(IngestError::io(&path, e)),
        };
        if needs_newline {
            line.insert(0, '\n');
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| IngestError::io(&path, e))?;
        f.write_all(line.as_bytes())
            .and_then(|()| f.sync_all())
            .map_err(|e| IngestError::io(&path, e))
    }

    /// Every retrieval in log order (oldest first). `source_id` filters when given.
    pub fn retrievals(&self, source_id: Option<&str>) -> Result<Vec<Retrieval>> {
        let path = self.log_path();
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(IngestError::io(&path, e)),
        };
        let terminated = text.ends_with('\n');
        let lines: Vec<&str> = text.lines().collect();
        let mut out = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Retrieval>(line) {
                Ok(r) => {
                    if source_id.is_none_or(|s| s == r.source_id) {
                        out.push(r);
                    }
                }
                // A torn final write (no trailing newline) is skipped, anything else is an error.
                Err(_) if i + 1 == lines.len() && !terminated => {}
                Err(e) => {
                    return Err(IngestError::BadLogLine {
                        line: i + 1,
                        message: e.to_string(),
                    });
                }
            }
        }
        Ok(out)
    }

    /// The most recent retrieval of a source (by `retrieved_at`, ties by log order).
    pub fn latest(&self, source_id: &str) -> Result<Option<Retrieval>> {
        Ok(self
            .retrievals(Some(source_id))?
            .into_iter()
            .max_by_key(|r| r.retrieved_at))
    }

    /// What was published on `date` (UTC): the last retrieval taken on or before the end of
    /// that day, or `None` if the source had not been retrieved yet.
    pub fn as_of(&self, source_id: &str, date: NaiveDate) -> Result<Option<Retrieval>> {
        let end =
            Utc.from_utc_datetime(&date.succ_opt().unwrap_or(date).and_time(Default::default()));
        Ok(self
            .retrievals(Some(source_id))?
            .into_iter()
            .filter(|r| r.retrieved_at < end)
            .max_by_key(|r| r.retrieved_at))
    }
}

fn whole_seconds(t: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&t.to_rfc3339_opts(SecondsFormat::Secs, true))
        .expect("own RFC 3339 output parses")
        .with_timezone(&Utc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(url: &str, at: &str) -> RetrievalMeta {
        RetrievalMeta {
            source_id: "test-src".into(),
            url: url.into(),
            retrieved_at: DateTime::parse_from_rfc3339(at)
                .unwrap()
                .with_timezone(&Utc),
            licence_id: "test-licence".into(),
            http_status: 200,
            content_type: Some("application/json".into()),
        }
    }

    fn blob_count(store: &SnapshotStore) -> usize {
        let mut n = 0;
        for shard in fs::read_dir(store.root().join("blobs")).unwrap() {
            n += fs::read_dir(shard.unwrap().path()).unwrap().count();
        }
        n
    }

    #[test]
    fn same_bytes_twice_is_one_blob_and_two_retrievals() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        let (r1, o1) = store
            .record(meta("https://x/a", "2026-10-01T00:00:00Z"), b"hello")
            .unwrap();
        let (r2, o2) = store
            .record(meta("https://x/a", "2026-10-02T00:00:00Z"), b"hello")
            .unwrap();
        assert_eq!(o1, PutOutcome::Created);
        assert_eq!(o2, PutOutcome::AlreadyPresent);
        assert_eq!(r1.sha256, r2.sha256);
        assert_eq!(blob_count(&store), 1);
        let log = store.retrievals(Some("test-src")).unwrap();
        assert_eq!(log, vec![r1, r2]);
    }

    #[test]
    fn changed_bytes_keep_every_version() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        let (a, _) = store
            .record(meta("https://x/a", "2026-10-01T00:00:00Z"), b"v1")
            .unwrap();
        let (b, _) = store
            .record(meta("https://x/a", "2026-10-08T00:00:00Z"), b"v2")
            .unwrap();
        assert_eq!(blob_count(&store), 2);
        assert_eq!(store.get_verified(&a.sha256).unwrap(), b"v1");
        assert_eq!(store.get_verified(&b.sha256).unwrap(), b"v2");
    }

    #[test]
    fn existing_blob_is_never_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        let (sha, _) = store.put(b"original").unwrap();
        let path = store.blob_path(&sha);
        // Tamper with the stored file (same inode kept), then put the same bytes again:
        // a rewrite would restore the content; the store must leave the file alone.
        fs::write(&path, b"tampered").unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        let (_, outcome) = store.put(b"original").unwrap();
        assert_eq!(outcome, PutOutcome::AlreadyPresent);
        assert_eq!(fs::read(&path).unwrap(), b"tampered");
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn put_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        store.put(b"a").unwrap();
        store.put(b"a").unwrap();
        assert_eq!(fs::read_dir(dir.path().join("tmp")).unwrap().count(), 0);
    }

    #[test]
    fn get_verified_detects_corruption_and_absence() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        let (sha, _) = store.put(b"original").unwrap();
        assert_eq!(store.get_verified(&sha).unwrap(), b"original");
        fs::write(store.blob_path(&sha), b"tampered").unwrap();
        assert!(matches!(
            store.get_verified(&sha),
            Err(IngestError::BlobCorrupt { .. })
        ));
        let other = sha256_of(b"never stored");
        assert!(matches!(
            store.get_verified(&other),
            Err(IngestError::BlobMissing(_))
        ));
    }

    #[test]
    fn blob_is_addressed_by_its_sha256() {
        // Known vector: sha256("abc").
        assert_eq!(
            sha256_of(b"abc").as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn retrieval_log_queries_by_source_and_date() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        store
            .record(meta("https://x/a", "2026-10-01T10:00:00Z"), b"v1")
            .unwrap();
        store
            .record(meta("https://x/a", "2026-10-08T10:00:00Z"), b"v2")
            .unwrap();
        let mut other = meta("https://y/b", "2026-10-03T00:00:00Z");
        other.source_id = "other".into();
        store.record(other, b"zzz").unwrap();
        assert_eq!(store.retrievals(None).unwrap().len(), 3);
        assert_eq!(store.retrievals(Some("test-src")).unwrap().len(), 2);
        assert_eq!(
            store.latest("test-src").unwrap().unwrap().sha256,
            sha256_of(b"v2")
        );
        let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        assert_eq!(store.as_of("test-src", d("2026-09-30")).unwrap(), None);
        assert_eq!(
            store
                .as_of("test-src", d("2026-10-01"))
                .unwrap()
                .unwrap()
                .sha256,
            sha256_of(b"v1")
        );
        assert_eq!(
            store
                .as_of("test-src", d("2026-10-07"))
                .unwrap()
                .unwrap()
                .sha256,
            sha256_of(b"v1")
        );
        assert_eq!(
            store
                .as_of("test-src", d("2026-10-08"))
                .unwrap()
                .unwrap()
                .sha256,
            sha256_of(b"v2")
        );
        assert_eq!(store.latest("nope").unwrap(), None);
    }

    #[test]
    fn log_survives_a_torn_final_line() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        store
            .record(meta("https://x/a", "2026-10-01T00:00:00Z"), b"v1")
            .unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(store.log_path())
            .unwrap();
        f.write_all(b"{\"source_id\":\"tru").unwrap();
        assert_eq!(store.retrievals(None).unwrap().len(), 1);
        store
            .record(meta("https://x/a", "2026-10-02T00:00:00Z"), b"v2")
            .unwrap();
        // The fragment is now a terminated, invalid line: surfaced, not silently dropped.
        assert!(matches!(
            store.retrievals(None),
            Err(IngestError::BadLogLine { line: 2, .. })
        ));
    }

    #[test]
    fn provenance_carries_the_retrieval() {
        let dir = tempfile::tempdir().unwrap();
        let store = SnapshotStore::open(dir.path()).unwrap();
        let (r, _) = store
            .record(meta("https://x/a?q=1", "2026-10-01T00:00:00Z"), b"v1")
            .unwrap();
        let p = r.provenance();
        assert_eq!(p.source_id, "test-src");
        assert_eq!(p.url, "https://x/a?q=1");
        assert_eq!(p.sha256, r.sha256);
        assert_eq!(p.licence_id, "test-licence");
        assert_eq!(
            p.retrieved_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            "2026-10-01T00:00:00Z"
        );
    }
}
