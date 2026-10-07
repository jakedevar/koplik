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

/// Project identification sent with every request: project name and version only, because no
/// contact has been verified yet. When `KOPLIK_CONTACT` (a verified contact address or
/// repository URL, supplied by the operator) is set, it is appended.
pub fn default_user_agent() -> String {
    user_agent_with(std::env::var("KOPLIK_CONTACT").ok().as_deref())
}

/// The User-Agent for an optional contact (blank counts as none).
pub fn user_agent_with(contact: Option<&str>) -> String {
    let base = format!(
        "koplik-ingest/{} (measles data demonstration project",
        env!("CARGO_PKG_VERSION")
    );
    match contact.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{base}; contact: {c})"),
        None => format!("{base})"),
    }
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
    /// Longest robots.txt `Crawl-delay` that is honoured. A larger delay is never shortened:
    /// the fetch is refused with [`IngestError::CrawlDelayTooLong`] instead.
    pub max_crawl_delay: Duration,
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
            max_crawl_delay: Duration::from_secs(120),
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
    /// robots.txt rules per origin (`scheme://authority`); throttling is per host.
    robots: HashMap<String, Robots>,
}

/// A parsed http(s) URL without credentials.
struct Target {
    scheme: String,
    /// Lower-cased authority with the scheme's default port removed (`example.org`,
    /// `example.org:8080`). Throttling is keyed by this, so `http` and `https` share a host.
    host: String,
    path: String,
}

impl Target {
    /// robots.txt scope: scheme plus normalised authority.
    fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.host)
    }
}

fn split_url(url: &str) -> Result<Target> {
    let bad = |m: &str| IngestError::Invalid(format!("{m}: {url}"));
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| bad("URL has no scheme"))?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(bad("only http and https URLs are fetched"));
    }
    let rest = rest.split('#').next().unwrap_or("");
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.is_empty() || authority.contains('@') {
        return Err(bad("URL must have a host and no credentials"));
    }
    let mut host = authority.to_ascii_lowercase();
    let default_port = if scheme == "https" { ":443" } else { ":80" };
    if let Some(h) = host.strip_suffix(default_port) {
        host = h.to_owned();
    }
    let path = if tail.is_empty() || tail.starts_with('?') {
        format!("/{tail}")
    } else {
        tail.to_owned()
    };
    Ok(Target { scheme, host, path })
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

    /// Gap to keep before the next request to `host`: the configured interval, or the host's
    /// robots.txt `Crawl-delay` when that is longer. A delay above `max_crawl_delay` is refused,
    /// never shortened.
    fn host_interval(&self, host: &str) -> Result<Duration> {
        let delay_secs = self
            .robots
            .iter()
            .filter(|(origin, _)| origin_host(origin) == host)
            .filter_map(|(_, r)| r.crawl_delay_secs())
            .fold(0.0_f64, f64::max);
        let delay = Duration::try_from_secs_f64(delay_secs).unwrap_or(Duration::MAX);
        if delay > self.cfg.max_crawl_delay {
            return Err(IngestError::CrawlDelayTooLong {
                host: host.to_owned(),
                delay_secs,
                limit_secs: self.cfg.max_crawl_delay.as_secs(),
            });
        }
        Ok(self.cfg.min_interval.max(delay))
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
            let interval = self.host_interval(host)?;
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

    fn robots_for(&mut self, t: &Target) {
        let origin = t.origin();
        if self.robots.contains_key(&origin) {
            return;
        }
        let url = format!("{origin}/robots.txt");
        let robots = match self.request(&t.host, &url) {
            Ok(r) if (200..300).contains(&r.status) => {
                Robots::parse(&String::from_utf8_lossy(&r.body), &self.cfg.user_agent)
            }
            // RFC 9309: robots.txt absent (4xx) means no restrictions; forbidden means none allowed.
            Ok(r) if r.status == 401 || r.status == 403 => Robots::disallow_all(),
            Ok(r) if (400..500).contains(&r.status) => Robots::allow_all(),
            // Server error or unreachable after retries: do not crawl what we cannot vet.
            _ => Robots::disallow_all(),
        };
        self.robots.insert(origin, robots);
    }

    /// GET `url` politely. Redirects are followed (bounded) and every hop is vetted against the
    /// robots.txt of its own origin. Anything but a final 2xx is an error.
    pub fn fetch(&mut self, url: &str) -> Result<Fetched> {
        let mut current = url.to_owned();
        for _ in 0..=self.cfg.max_redirects {
            let t = split_url(&current)?;
            self.robots_for(&t);
            if !self.robots[&t.origin()].allowed(&t.path) {
                return Err(IngestError::RobotsDisallowed {
                    host: t.host,
                    url: current,
                });
            }
            let resp = self.request(&t.host, &current)?;
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
                        format!("{}{loc}", t.origin())
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

/// Host part of an origin key (`https://example.org:8080` -> `example.org:8080`).
fn origin_host(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, h)| h)
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
        }
    }

    #[test]
    fn default_user_agent_names_project_and_version_and_invents_no_contact() {
        let ua = user_agent_with(None);
        assert_eq!(
            ua,
            format!(
                "koplik-ingest/{} (measles data demonstration project)",
                env!("CARGO_PKG_VERSION")
            )
        );
        assert!(!ua.contains("://") && !ua.contains('@'), "{ua}");
        assert_eq!(user_agent_with(Some("  ")), ua);
        assert_eq!(
            user_agent_with(Some(" ops@example.org ")),
            format!(
                "koplik-ingest/{} (measles data demonstration project; contact: ops@example.org)",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn crawl_delay_above_a_minute_is_honoured_in_full() {
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::ok(b"User-agent: *\nCrawl-delay: 90\n"));
        c.on(URL, FakeClient::ok(b"[]"));
        let t = FakeTime::default();
        let mut f = fetcher(&c, &t);
        f.fetch(URL).unwrap();
        // The data request waits the whole 90 s after robots.txt, not a capped 60 s.
        assert!(
            t.sleeps.borrow().contains(&Duration::from_secs(90)),
            "{:?}",
            t.sleeps
        );
        f.fetch(URL).unwrap();
        assert_eq!(
            t.sleeps
                .borrow()
                .iter()
                .filter(|d| **d == Duration::from_secs(90))
                .count(),
            2
        );
    }

    #[test]
    fn crawl_delay_above_the_limit_refuses_instead_of_fetching_sooner() {
        let c = FakeClient::default();
        c.on(
            ROBOTS,
            FakeClient::ok(b"User-agent: *\nCrawl-delay: 3600\n"),
        );
        c.on(URL, FakeClient::ok(b"[]"));
        let t = FakeTime::default();
        let mut f = fetcher(&c, &t);
        assert!(matches!(
            f.fetch(URL),
            Err(IngestError::CrawlDelayTooLong {
                limit_secs: 120,
                ..
            })
        ));
        assert!(c.calls.borrow().iter().all(|(u, _)| u != URL));
        assert!(
            t.sleeps
                .borrow()
                .iter()
                .all(|d| *d < Duration::from_secs(3600))
        );
        // Raising the limit makes the same delay acceptable.
        let cfg = PoliteConfig {
            max_crawl_delay: Duration::from_secs(7200),
            ..PoliteConfig::default()
        };
        let t = FakeTime::default();
        let mut f = PoliteFetcher::new(c.clone(), t.clone(), cfg);
        f.fetch(URL).unwrap();
        assert!(t.sleeps.borrow().contains(&Duration::from_secs(3600)));
    }

    #[test]
    fn robots_are_per_origin_and_a_cross_scheme_redirect_is_revetted() {
        const HTTP_ROBOTS: &str = "http://data.example.gov/robots.txt";
        const START: &str = "http://data.example.gov/start";
        let c = FakeClient::default();
        // http origin allows everything; the https origin of the SAME host forbids /secret.
        c.on(HTTP_ROBOTS, FakeClient::status(404, b""));
        c.on(
            ROBOTS,
            FakeClient::ok(b"User-agent: *\nDisallow: /secret\n"),
        );
        let mut redirect = FakeClient::status(301, b"").unwrap();
        redirect.location = Some("https://data.example.gov/secret/data".into());
        c.on(START, Ok(redirect));
        c.on("https://data.example.gov/secret/data", FakeClient::ok(b"x"));
        let t = FakeTime::default();
        let mut f = fetcher(&c, &t);
        assert!(matches!(
            f.fetch(START),
            Err(IngestError::RobotsDisallowed { .. })
        ));
        let calls: Vec<String> = c.calls.borrow().iter().map(|(u, _)| u.clone()).collect();
        assert!(calls.contains(&HTTP_ROBOTS.to_owned()));
        assert!(
            calls.contains(&ROBOTS.to_owned()),
            "https robots fetched for its own origin"
        );
        assert!(!calls.contains(&"https://data.example.gov/secret/data".to_owned()));
        // Throttling stays per host: the https robots request waited behind the http requests.
        assert!(t.sleeps.borrow().len() >= 2);
        assert!(
            t.sleeps
                .borrow()
                .iter()
                .all(|d| *d >= Duration::from_secs(1))
        );
    }

    #[test]
    fn default_ports_normalise_into_one_origin() {
        let a = split_url("HTTPS://Data.Example.Gov:443/x").unwrap();
        let b = split_url("https://data.example.gov/y").unwrap();
        assert_eq!(a.origin(), b.origin());
        assert_ne!(
            split_url("http://data.example.gov/").unwrap().origin(),
            b.origin()
        );
        assert_eq!(split_url("http://data.example.gov/").unwrap().host, b.host);
        assert_ne!(
            split_url("https://data.example.gov:8443/").unwrap().host,
            b.host
        );
    }

    #[test]
    fn percent_encoded_paths_are_vetted_including_on_redirect_hops() {
        let c = FakeClient::default();
        c.on(
            ROBOTS,
            FakeClient::ok(b"User-agent: *\nDisallow: /private\nDisallow: /a%2fb\n"),
        );
        // The start URL is allowed; it redirects to an encoded spelling of a forbidden path.
        let mut r1 = FakeClient::status(302, b"").unwrap();
        r1.location = Some("/%70rivate/data".into());
        c.on(URL, Ok(r1));
        c.on(
            "https://data.example.gov/%70rivate/data",
            FakeClient::ok(b"x"),
        );
        let mut f = fetcher(&c, &FakeTime::default());
        assert!(matches!(
            f.fetch(URL),
            Err(IngestError::RobotsDisallowed { .. })
        ));
        assert!(
            c.calls
                .borrow()
                .iter()
                .all(|(u, _)| !u.contains("%70rivate"))
        );
        // Encoded reserved character matches its rule; the unencoded separator does not.
        for (url, blocked) in [
            ("https://data.example.gov/a%2Fb", true),
            ("https://data.example.gov/a/b", false),
        ] {
            c.on(url, FakeClient::ok(b"x"));
            let mut f = fetcher(&c, &FakeTime::default());
            assert_eq!(
                matches!(f.fetch(url), Err(IngestError::RobotsDisallowed { .. })),
                blocked,
                "{url}"
            );
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
