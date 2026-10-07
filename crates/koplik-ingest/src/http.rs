//! HTTP transport boundary. [`HttpClient`] is the only thing that touches the network; the
//! polite fetcher is generic over it so every test runs against an injected fake.

use std::time::Duration;

use crate::error::{IngestError, Result};

/// One HTTP response (redirects are NOT followed by the client; the fetcher follows them so
/// each hop is checked against robots.txt and the per-host rate limit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub location: Option<String>,
    /// `Retry-After` in whole seconds, when given in that form.
    pub retry_after_secs: Option<u64>,
    pub body: Vec<u8>,
}

pub trait HttpClient {
    /// GET `url` with the given `User-Agent`. Transport failures (DNS, connect, timeout, TLS)
    /// are `Err`; any HTTP status is `Ok`.
    fn get(&self, url: &str, user_agent: &str) -> Result<HttpResponse>;
}

/// Live client: `ureq` over rustls, no credentials, bounded timeouts and body size.
pub struct UreqClient {
    agent: ureq::Agent,
    max_body_bytes: u64,
}

impl UreqClient {
    pub fn new(timeout: Duration, max_body_bytes: u64) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(Duration::from_secs(10)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            agent,
            max_body_bytes,
        }
    }
}

impl HttpClient for UreqClient {
    fn get(&self, url: &str, user_agent: &str) -> Result<HttpResponse> {
        let mut resp = self
            .agent
            .get(url)
            .header("User-Agent", user_agent)
            .header("Accept", "application/json, text/plain;q=0.9, */*;q=0.5")
            .call()
            .map_err(|e| IngestError::Http(format!("{url}: {e}")))?;
        let header = |name: &str| {
            resp.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let status = resp.status().as_u16();
        let content_type = header("content-type");
        let location = header("location");
        let retry_after_secs = header("retry-after").and_then(|v| v.trim().parse().ok());
        let body = resp
            .body_mut()
            .with_config()
            .limit(self.max_body_bytes)
            .read_to_vec()
            .map_err(|e| IngestError::Http(format!("{url}: reading body: {e}")))?;
        Ok(HttpResponse {
            status,
            content_type,
            location,
            retry_after_secs,
            body,
        })
    }
}
