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
        Self::build(timeout, max_body_bytes, true)
    }

    /// Test-only client that ignores HTTP_PROXY/HTTPS_PROXY/ALL_PROXY, so a loopback test can
    /// never send traffic off the machine whatever the environment (AGENTS.md rule 7).
    #[cfg(test)]
    fn loopback_only(timeout: Duration, max_body_bytes: u64) -> Self {
        Self::build(timeout, max_body_bytes, false)
    }

    fn build(timeout: Duration, max_body_bytes: u64, proxy_from_env: bool) -> Self {
        let mut config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(Duration::from_secs(10)))
            .max_redirects(0)
            .http_status_as_error(false);
        if !proxy_from_env {
            config = config.proxy(None);
        }
        let agent: ureq::Agent = config.build().into();
        Self {
            agent,
            max_body_bytes,
        }
    }
}

/// The `User-Agent` header value, marked sensitive: it embeds the operator's contact, and
/// ureq's debug request log prints `User-Agent` by header name, so only the value's own
/// `Debug` (which prints `Sensitive` for a sensitive value) keeps the contact out of logs.
/// The bytes on the wire are unchanged.
pub(crate) fn sensitive_user_agent(user_agent: &str) -> Result<ureq::http::HeaderValue> {
    let mut value = ureq::http::HeaderValue::from_str(user_agent)
        .map_err(|_| IngestError::Http("the User-Agent is not a valid header value".to_owned()))?;
    value.set_sensitive(true);
    Ok(value)
}

impl HttpClient for UreqClient {
    fn get(&self, url: &str, user_agent: &str) -> Result<HttpResponse> {
        let mut resp = self
            .agent
            .get(url)
            .header("User-Agent", sensitive_user_agent(user_agent)?)
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

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;

    use super::*;

    // Dummy contact only: the operator's real Census contact never appears in the repository.
    const UA: &str =
        "koplik-ingest/0.0.0 (measles data demonstration project; census-contact@example.invalid)";

    #[test]
    fn the_user_agent_value_is_sensitive_but_unchanged_on_the_wire() {
        let value = sensitive_user_agent(UA).unwrap();
        assert!(value.is_sensitive());
        assert_eq!(value.as_bytes(), UA.as_bytes());
        for text in [format!("{value:?}"), format!("{value:#?}")] {
            assert!(!text.contains("example.invalid"), "{text}");
            assert!(text.contains("Sensitive"), "{text}");
        }
        let mut headers = ureq::http::HeaderMap::new();
        headers.insert(ureq::http::header::USER_AGENT, value);
        let text = format!("{headers:?}");
        assert!(!text.contains("example.invalid"), "{text}");
        assert!(sensitive_user_agent("bad\nvalue").is_err());
    }

    struct Capture(Mutex<Vec<String>>);

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            self.0.lock().unwrap().push(record.args().to_string());
        }
        fn flush(&self) {}
    }

    /// Loopback only (no external network): ureq's own debug log of a real request must not
    /// carry the contact, while the server receives the User-Agent intact.
    #[test]
    fn ureq_debug_logs_do_not_carry_the_contact_and_the_server_receives_it() {
        static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));
        log::set_logger(&CAPTURE).unwrap();
        log::set_max_level(log::LevelFilter::Trace);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let mut buf = [0u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = sock.read(&mut buf).unwrap();
                seen.extend_from_slice(&buf[..n]);
            }
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });
        let client = UreqClient::loopback_only(Duration::from_secs(10), 1024);
        let r = client
            .get(&format!("http://127.0.0.1:{port}/x"), UA)
            .unwrap();
        assert_eq!(r.body, b"ok");
        let request = server.join().unwrap();
        assert!(request.contains(&format!("user-agent: {UA}")), "{request}");
        let logs = CAPTURE.0.lock().unwrap().join("\n");
        assert!(logs.contains("Request"), "ureq logged no request: {logs}");
        assert!(!logs.contains("example.invalid"), "{logs}");
    }
}
