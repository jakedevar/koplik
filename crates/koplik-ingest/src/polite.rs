//! Polite fetching: identifying User-Agent, robots.txt honoured, at least one second between
//! requests to one host (more if robots.txt asks), bounded retries with exponential backoff,
//! manual redirect following (each hop re-checked), and no credentials of any kind.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::error::{IngestError, Result};
use crate::http::{HttpClient, HttpResponse};
use crate::robots::Robots;

/// Hard floor for the gap between two requests to one host.
pub const MIN_HOST_INTERVAL: Duration = Duration::from_secs(1);

/// Project identification sent with every request. `KOPLIK_CONTACT` (a repository URL or
/// contact address) overrides the default contact part.
pub const DEFAULT_CONTACT: &str = "https://github.com/koplik/koplik";

pub fn default_user_agent() -> String {
    let contact = std::env::var("KOPLIK_CONTACT")
        .ok()
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CONTACT.to_owned());
    format!(
        "koplik-ingest/{} (measles outbreak research demo; +{})",
        env!("CARGO_PKG_VERSION"),
        contact.trim()
    )
}

/// Time and sleeping, injectable so tests never wait.
pub trait Timekeeper {
    fn monotonic(&self) -> Duration;
    fn sleep(&self, d: Duration);
    fn utc_now(&self) -> DateTime<Utc>;
}

pub struct SystemTimekeeper {
    start: Instant,
}

impl SystemTimekeeper {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemTimekeeper {
    fn default() -> Self {
        Self::new()
    }
}

impl Timekeeper for SystemTimekeeper {
    fn monotonic(&self) -> Duration {
        self.start.elapsed()
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
    fn utc_now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

#[derive(Debug, Clone)]
pub struct PoliteConfig {
    pub user_agent: String,
    /// Gap between requests to one host; clamped up to [`MIN_HOST_INTERVAL`].
    pub min_interval: Duration,
    /// Total tries per URL (first try included) for transport errors, 429 and 5xx.
    pub max_attempts: u32,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
    pub max_redirects: u32,
}

impl Default for PoliteConfig {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            min_interval: MIN_HOST_INTERVAL,
            max_attempts: 3,
            backoff_base: Duration::from_secs(2),
            backoff_cap: Duration::from_secs(60),
            max_redirects: 3,
        }
    }
}

/// A successful (2xx) fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The URL that was asked for (what provenance records).
    pub url: String,
    /// The URL that finally answered, when redirects were followed.
    pub final_url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub retrieved_at: DateTime<Utc>,
}

pub struct PoliteFetcher<C: HttpClient, T: Timekeeper> {
    client: C,
    time: T,
    cfg: PoliteConfig,
    last_request: HashMap<String, Duration>,
    robots: HashMap<String, Robots>,
}

/// `(scheme, host, path_and_query)` of an http(s) URL without credentials.
fn split_url(url: &str) -> Result<(&str, String, String)> {
    let bad = |m: &str| IngestError::Invalid(format!("{m}: {url}"));
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| bad("URL has no scheme"))?;
    if scheme != "http" && scheme != "https" {
        return Err(bad("only http and https URLs are fetched"));
    }
    let rest = rest.split('#').next().unwrap_or("");
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.is_empty() || authority.contains('@') {
        return Err(bad("URL must have a host and no credentials"));
    }
    let path = if tail.is_empty() || tail.starts_with('?') {
        format!("/{tail}")
    } else {
        tail.to_owned()
    };
    Ok((scheme, authority.to_ascii_lowercase(), path))
}

impl<C: HttpClient, T: Timekeeper> PoliteFetcher<C, T> {
    pub fn new(client: C, time: T, mut cfg: PoliteConfig) -> Self {
        cfg.min_interval = cfg.min_interval.max(MIN_HOST_INTERVAL);
        cfg.max_attempts = cfg.max_attempts.max(1);
        Self {
            client,
            time,
            cfg,
            last_request: HashMap::new(),
            robots: HashMap::new(),
        }
    }

    pub fn user_agent(&self) -> &str {
        &self.cfg.user_agent
    }

    /// Wait until the host has had its quiet interval, then stamp it.
    fn throttle(&mut self, host: &str, interval: Duration) {
        if let Some(&last) = self.last_request.get(host) {
            let due = last + interval;
            let now = self.time.monotonic();
            if now < due {
                self.time.sleep(due - now);
            }
        }
        self.last_request
            .insert(host.to_owned(), self.time.monotonic());
    }

    fn host_interval(&self, host: &str) -> Duration {
        let delay = self
            .robots
            .get(host)
            .and_then(Robots::crawl_delay_secs)
            .map(|s| Duration::from_secs_f64(s.min(60.0)))
            .unwrap_or_default();
        self.cfg.min_interval.max(delay)
    }

    fn backoff(&self, attempt: u32, retry_after: Option<u64>) -> Duration {
        let exp = self
            .cfg
            .backoff_base
            .saturating_mul(1u32 << (attempt - 1).min(16));
        retry_after
            .map(Duration::from_secs)
            .unwrap_or(exp)
            .min(self.cfg.backoff_cap)
    }

    /// One request with retries. Returns the final response (any status) or the last
    /// transport error once attempts are exhausted. Only 429, 5xx and transport errors retry.
    fn request(&mut self, host: &str, url: &str) -> Result<HttpResponse> {
        let mut attempt = 1;
        loop {
            let interval = self.host_interval(host);
            self.throttle(host, interval);
            let outcome = self.client.get(url, &self.cfg.user_agent);
            let (retryable, retry_after) = match &outcome {
                Ok(r) if r.status == 429 || r.status >= 500 => (true, r.retry_after_secs),
                Ok(_) => (false, None),
                Err(_) => (true, None),
            };
            if !retryable || attempt >= self.cfg.max_attempts {
                return outcome;
            }
            self.time.sleep(self.backoff(attempt, retry_after));
            attempt += 1;
        }
    }

    fn robots_for(&mut self, scheme: &str, host: &str) -> Result<()> {
        if self.robots.contains_key(host) {
            return Ok(());
        }
        let url = format!("{scheme}://{host}/robots.txt");
        let robots = match self.request(host, &url) {
            Ok(r) if (200..300).contains(&r.status) => {
                Robots::parse(&String::from_utf8_lossy(&r.body), &self.cfg.user_agent)
            }
            // RFC 9309: robots.txt absent (4xx) means no restrictions; forbidden means none allowed.
            Ok(r) if r.status == 401 || r.status == 403 => Robots::disallow_all(),
            Ok(r) if (400..500).contains(&r.status) => Robots::allow_all(),
            // Server error or unreachable after retries: do not crawl what we cannot vet.
            _ => Robots::disallow_all(),
        };
        self.robots.insert(host.to_owned(), robots);
        Ok(())
    }

    /// GET `url` politely. Redirects are followed (bounded) and every hop is vetted against
    /// robots.txt. Anything but a final 2xx is an error.
    pub fn fetch(&mut self, url: &str) -> Result<Fetched> {
        let mut current = url.to_owned();
        for _ in 0..=self.cfg.max_redirects {
            let (scheme, host, path) = split_url(&current)?;
            let scheme = scheme.to_owned();
            self.robots_for(&scheme, &host)?;
            if !self.robots[&host].allowed(&path) {
                return Err(IngestError::RobotsDisallowed { host, url: current });
            }
            let resp = self.request(&host, &current)?;
            match resp.status {
                200..=299 => {
                    return Ok(Fetched {
                        url: url.to_owned(),
                        final_url: current,
                        status: resp.status,
                        content_type: resp.content_type,
                        body: resp.body,
                        retrieved_at: self.time.utc_now(),
                    });
                }
                301 | 302 | 303 | 307 | 308 => {
                    let loc = resp.location.ok_or_else(|| {
                        IngestError::Http(format!("redirect without Location from {current}"))
                    })?;
                    current = if loc.contains("://") {
                        loc
                    } else if loc.starts_with('/') {
                        format!("{scheme}://{host}{loc}")
                    } else {
                        return Err(IngestError::Http(format!(
                            "unsupported relative redirect {loc:?} from {current}"
                        )));
                    };
                }
                status => {
                    return Err(IngestError::BadStatus {
                        status,
                        url: current,
                    });
                }
            }
        }
        Err(IngestError::Http(format!(
            "more than {} redirects from {url}",
            self.cfg.max_redirects
        )))
    }
}

#[cfg(test)]
pub(crate) mod fakes {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use super::*;

    /// Scripted responses per URL, in order; the last one repeats.
    #[derive(Clone, Default)]
    pub struct FakeClient {
        pub script: Rc<RefCell<HashMap<String, VecDeque<Result<HttpResponse>>>>>,
        pub calls: Rc<RefCell<Vec<(String, String)>>>,
    }

    impl FakeClient {
        pub fn on(&self, url: &str, r: Result<HttpResponse>) {
            self.script
                .borrow_mut()
                .entry(url.to_owned())
                .or_default()
                .push_back(r);
        }
        pub fn ok(body: &[u8]) -> Result<HttpResponse> {
            Self::status(200, body)
        }
        pub fn status(status: u16, body: &[u8]) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status,
                content_type: Some("application/json".into()),
                location: None,
                retry_after_secs: None,
                body: body.to_vec(),
            })
        }
    }

    impl HttpClient for FakeClient {
        fn get(&self, url: &str, user_agent: &str) -> Result<HttpResponse> {
            self.calls
                .borrow_mut()
                .push((url.to_owned(), user_agent.to_owned()));
            let mut script = self.script.borrow_mut();
            match script.get_mut(url) {
                Some(q) if q.len() > 1 => q.pop_front().unwrap(),
                Some(q) if q.len() == 1 => clone_result(&q[0]),
                _ => FakeClient::status(404, b"not found"),
            }
        }
    }

    fn clone_result(r: &Result<HttpResponse>) -> Result<HttpResponse> {
        match r {
            Ok(r) => Ok(r.clone()),
            Err(e) => Err(IngestError::Http(e.to_string())),
        }
    }

    /// Virtual clock: `sleep` advances time and records the request.
    #[derive(Clone, Default)]
    pub struct FakeTime {
        pub now: Rc<RefCell<Duration>>,
        pub sleeps: Rc<RefCell<Vec<Duration>>>,
    }

    impl Timekeeper for FakeTime {
        fn monotonic(&self) -> Duration {
            *self.now.borrow()
        }
        fn sleep(&self, d: Duration) {
            self.sleeps.borrow_mut().push(d);
            *self.now.borrow_mut() += d;
        }
        fn utc_now(&self) -> DateTime<Utc> {
            DateTime::parse_from_rfc3339("2026-10-06T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::*;
    use super::*;

    const URL: &str = "https://data.example.gov/resource/abc.json?$limit=5";
    const ROBOTS: &str = "https://data.example.gov/robots.txt";

    fn fetcher(client: &FakeClient, time: &FakeTime) -> PoliteFetcher<FakeClient, FakeTime> {
        PoliteFetcher::new(client.clone(), time.clone(), PoliteConfig::default())
    }

    #[test]
    fn identifies_itself_and_stamps_retrieval_time() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::ok(b"[]"));
        let mut f = fetcher(&c, &FakeTime::default());
        let got = f.fetch(URL).unwrap();
        assert_eq!(got.body, b"[]");
        assert_eq!(got.url, URL);
        assert_eq!(got.retrieved_at.to_rfc3339(), "2026-10-06T12:00:00+00:00");
        for (_, ua) in c.calls.borrow().iter() {
            assert!(ua.starts_with("koplik-ingest/"), "{ua}");
            assert!(ua.contains("https://"), "{ua}");
        }
    }

    #[test]
    fn waits_at_least_one_second_between_requests_to_one_host() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::ok(b"[]"));
        let t = FakeTime::default();
        let mut f = fetcher(&c, &t);
        f.fetch(URL).unwrap();
        f.fetch(URL).unwrap();
        // robots.txt, then the data, then the data again: two enforced gaps of >= 1 s.
        assert_eq!(c.calls.borrow().len(), 3);
        let slept: Duration = t.sleeps.borrow().iter().sum();
        assert!(slept >= Duration::from_secs(2), "{slept:?}");
        assert!(
            t.sleeps
                .borrow()
                .iter()
                .all(|d| *d >= Duration::from_secs(1))
        );
    }

    #[test]
    fn robots_crawl_delay_widens_the_gap_and_floor_is_one_second() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::ok(b"User-agent: *\nCrawl-delay: 5\n"));
        c.on(URL, FakeClient::ok(b"[]"));
        let t = FakeTime::default();
        let mut f = fetcher(&c, &t);
        f.fetch(URL).unwrap();
        assert!(
            t.sleeps
                .borrow()
                .iter()
                .any(|d| *d >= Duration::from_secs(5))
        );
        // A zero configured interval is clamped up to the 1 s floor.
        let cfg = PoliteConfig {
            min_interval: Duration::ZERO,
            ..PoliteConfig::default()
        };
        let c2 = FakeClient::default();
        c2.on(ROBOTS, FakeClient::status(404, b""));
        c2.on(URL, FakeClient::ok(b"[]"));
        let t2 = FakeTime::default();
        let mut f2 = PoliteFetcher::new(c2, t2.clone(), cfg);
        f2.fetch(URL).unwrap();
        assert!(
            t2.sleeps
                .borrow()
                .iter()
                .all(|d| *d >= Duration::from_secs(1))
        );
        assert!(!t2.sleeps.borrow().is_empty());
    }

    #[test]
    fn robots_disallow_blocks_the_fetch_without_requesting_the_url() {
        let c = FakeClient::default();
        c.on(
            ROBOTS,
            FakeClient::ok(b"User-agent: *\nDisallow: /resource/\n"),
        );
        c.on(URL, FakeClient::ok(b"[]"));
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(
            f.fetch(URL),
            Err(IngestError::RobotsDisallowed { .. })
        ));
        assert!(c.calls.borrow().iter().all(|(u, _)| u != URL));
    }

    #[test]
    fn unreachable_robots_means_do_not_crawl() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(503, b""));
        c.on(URL, FakeClient::ok(b"[]"));
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(
            f.fetch(URL),
            Err(IngestError::RobotsDisallowed { .. })
        ));
    }

    #[test]
    fn retries_are_bounded_with_growing_backoff() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::status(503, b"busy"));
        c.on(URL, FakeClient::status(503, b"busy"));
        c.on(URL, FakeClient::ok(b"fine"));
        let t = FakeTime::default();
        let mut f = fetcher(&c, &t);
        assert_eq!(f.fetch(URL).unwrap().body, b"fine");
        let data_calls = c.calls.borrow().iter().filter(|(u, _)| u == URL).count();
        assert_eq!(data_calls, 3);

        // Always failing: gives up after max_attempts (3) with the last status.
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::status(500, b"boom"));
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(
            f.fetch(URL),
            Err(IngestError::BadStatus { status: 500, .. })
        ));
        assert_eq!(c.calls.borrow().iter().filter(|(u, _)| u == URL).count(), 3);
    }

    #[test]
    fn client_errors_do_not_retry() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::status(404, b"gone"));
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(
            f.fetch(URL),
            Err(IngestError::BadStatus { status: 404, .. })
        ));
        assert_eq!(c.calls.borrow().iter().filter(|(u, _)| u == URL).count(), 1);
    }

    #[test]
    fn transport_errors_retry_then_surface() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, Err(IngestError::Http("timeout".into())));
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(f.fetch(URL), Err(IngestError::Http(_))));
        assert_eq!(c.calls.borrow().iter().filter(|(u, _)| u == URL).count(), 3);
    }

    #[test]
    fn redirects_are_followed_and_revetted_but_bounded() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        let mut redirect = FakeClient::status(302, b"").unwrap();
        redirect.location = Some("/final".into());
        c.on(URL, Ok(redirect));
        c.on("https://data.example.gov/final", FakeClient::ok(b"done"));
        let mut f = fetcher(&c, &FakeTime::default());
        let got = f.fetch(URL).unwrap();
        assert_eq!(got.body, b"done");
        assert_eq!(got.url, URL);
        assert_eq!(got.final_url, "https://data.example.gov/final");

        // A redirect loop ends in an error rather than spinning.
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        let mut r = FakeClient::status(302, b"").unwrap();
        r.location = Some("/final".into());
        c.on("https://data.example.gov/final", Ok(r.clone()));
        let mut r0 = r;
        r0.location = Some("/final".into());
        c.on(URL, Ok(r0));
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(f.fetch(URL), Err(IngestError::Http(_))));
    }

    #[test]
    fn rejects_credentials_and_non_http_urls() {
        let c = FakeClient::default();
        let mut f = fetcher(&c, &FakeTime::default());
        for bad in [
            "https://user:pw@data.example.gov/x",
            "ftp://data.example.gov/x",
            "data.example.gov/x",
        ] {
            assert!(
                matches!(f.fetch(bad), Err(IngestError::Invalid(_))),
                "{bad}"
            );
        }
        assert!(c.calls.borrow().is_empty());
    }
}
