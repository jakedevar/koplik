//! Errors for the ingest crate.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("blob {sha256} failed verification: bytes hash to {actual}")]
    BlobCorrupt { sha256: String, actual: String },
    #[error("Census pin mismatch for {url}: expected {expected}, fetched {actual}")]
    PinMismatch {
        url: String,
        expected: String,
        actual: String,
    },
    #[error("named Census file already requested in this pipeline run: {0}")]
    NamedFileAlreadyRequested(String),
    #[error("blob {0} is not in the snapshot store")]
    BlobMissing(String),
    #[error("retrieval log line {line} is not valid: {message}")]
    BadLogLine { line: usize, message: String },
    #[error(
        "live fetching is disabled because KOPLIK_CONTACT is set but blank: unset it to use the default contact, or set it to a verified e-mail address or repository URL (see SOURCES.md)"
    )]
    ContactRequired,
    #[error("invalid argument: {0}")]
    Invalid(String),
    #[error("http: {0}")]
    Http(String),
    #[error("robots.txt for {host} disallows {url}")]
    RobotsDisallowed { host: String, url: String },
    #[error(
        "robots.txt for {host} asks for a {delay_secs} s crawl delay, above the configured limit of {limit_secs} s; not fetching"
    )]
    CrawlDelayTooLong {
        host: String,
        delay_secs: f64,
        limit_secs: u64,
    },
    #[error("unexpected HTTP status {status} for {url}")]
    BadStatus { status: u16, url: String },
    #[error("no snapshot retrieved yet for source {0}")]
    NoSnapshot(String),
    #[error("parse error: {0}")]
    Parse(String),
}

impl IngestError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, IngestError>;
