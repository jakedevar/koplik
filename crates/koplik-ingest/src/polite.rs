//! Polite fetching: identifying User-Agent, robots.txt honoured, at least one second between
//! requests to one host (more if robots.txt asks), bounded retries with exponential backoff,
//! with the explicit pinned Census named-file exception in `fetch_census_file`,
//! manual redirect following (each hop re-checked), and no credentials of any kind.

use std::collections::{BTreeSet, HashMap};

use crate::census_files::NamedFileAllowlist;
use crate::store::sha256_of;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::error::{IngestError, Result};
use crate::http::{HttpClient, HttpResponse};
use crate::robots::Robots;

/// Hard floor for the gap between two requests to one host.
pub const MIN_HOST_INTERVAL: Duration = Duration::from_secs(1);

/// Environment variable that overrides the contact sent in the User-Agent (an e-mail address
/// or a repository URL). Unset: [`DEFAULT_CONTACT`]. Set but empty or blank: an explicit opt-out,
/// so live fetching refuses.
pub const CONTACT_ENV: &str = "KOPLIK_CONTACT";

/// Contact sent in the User-Agent when `KOPLIK_CONTACT` is unset: the public Koplik repository
/// (no personal e-mail). Decision `ingest-contact`, ruled 2026-10-07 by the global manager (node
/// 73306b5f) under the operator's standing directive (#1413: the operator's public GitHub profile
/// until a public repository existed); switched to the repository at the public release, step (b)
/// of #1449. Census hosts use the Census contact instead (#1427).
pub const DEFAULT_CONTACT: &str = "https://github.com/jakedevar/koplik";

/// Pure contact resolution from the raw value of `KOPLIK_CONTACT` (`None` = unset):
/// non-blank value -> that value, trimmed; unset -> [`DEFAULT_CONTACT`]; set but empty/blank
/// -> `None` (explicit opt-out; live fetching refuses).
pub fn resolve_contact(env_value: Option<&str>) -> Option<String> {
    match env_value {
        None => Some(DEFAULT_CONTACT.to_owned()),
        Some(v) => Some(v.trim().to_owned()).filter(|c| !c.is_empty()),
    }
}

/// THE contact resolver: every live fetch (all CLI arms, the pipeline's ingest stage, examples)
/// reads the contact through this function, via [`PoliteConfig::live_from_env`]. Reads
/// `KOPLIK_CONTACT` as [`resolve_contact`] describes; a value that is not valid UTF-8 counts as
/// set-but-unusable, so it refuses rather than falling back to the default.
pub fn contact_from_env() -> Option<String> {
    match std::env::var(CONTACT_ENV) {
        Ok(v) => resolve_contact(Some(&v)),
        Err(std::env::VarError::NotPresent) => resolve_contact(None),
        Err(std::env::VarError::NotUnicode(_)) => None,
    }
}

/// [`contact_from_env`] with the reason a refusal happened: a blank value is
/// [`IngestError::ContactRequired`], a non-UTF-8 value is [`IngestError::ContactNotUtf8`].
fn general_contact_checked(raw: Option<&std::ffi::OsStr>) -> Result<String> {
    match raw {
        None => Ok(DEFAULT_CONTACT.to_owned()),
        Some(v) => match v.to_str() {
            Some(v) => resolve_contact(Some(v)).ok_or(IngestError::ContactRequired),
            None => Err(IngestError::ContactNotUtf8 {
                variable: CONTACT_ENV.to_owned(),
            }),
        },
    }
}

/// Environment variable carrying the contact for Census requests. Operator instruction
/// (2026-10-07, #1427): Census requests identify the operator's own contact. The address is
/// deliberately NOT in the repository: it comes from this variable or from [`LOCAL_CONFIG_FILE`].
pub const CENSUS_CONTACT_ENV: &str = "KOPLIK_CENSUS_CONTACT";

/// Gitignored local config at the repository root (`KEY=VALUE` lines; the `.env.*` rule in
/// `.gitignore` covers it). Holds [`CENSUS_CONTACT_ENV`] when it is not in the environment.
pub const LOCAL_CONFIG_FILE: &str = ".env.local";

/// Whether `host` (a bare host name: no port, no userinfo) is a US Census Bureau host:
/// `census.gov` or any `*.census.gov`, compared case-insensitively with one trailing dot
/// (the absolute form `www2.census.gov.`) removed. A lookalike such as `evilcensus.gov` or
/// `census.gov.evil.com` is not one, and archive.org is not one even when it serves a copy of a
/// Census file.
pub fn is_census_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    let name = lower.strip_suffix('.').unwrap_or(&lower);
    name == "census.gov"
        || name.strip_suffix(".census.gov").is_some_and(|labels| {
            !labels.is_empty() && !labels.starts_with('.') && !labels.contains("..")
        })
}

/// Whether the request URL's host is a Census host. The host comes from the parsed URL, so
/// ports, userinfo (`https://census.gov@evil.com/` is evil.com) and malformed authorities
/// (`census.gov:443.evil.com` does not parse) cannot be mistaken for Census. An unparsable
/// URL is not a Census URL.
pub fn is_census_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(is_census_host))
        .unwrap_or(false)
}

/// Value of `key` in simple `KEY=VALUE` config text: `#` comment lines and blank lines are
/// skipped, an optional `export ` prefix and one pair of surrounding quotes are removed, the
/// last assignment wins, and there are no inline comments or interpolation.
pub fn config_value(text: &str, key: &str) -> Option<String> {
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim_start);
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let v = v.trim();
        let unquoted = ['"', '\'']
            .iter()
            .find_map(|q| v.strip_prefix(*q).and_then(|r| r.strip_suffix(*q)))
            .unwrap_or(v);
        found = Some(unquoted.to_owned());
    }
    found
}

/// The repository root above `start`: the nearest ancestor holding `.git` (a directory, or the
/// file a linked worktree has).
pub fn repository_root(start: &std::path::Path) -> Option<std::path::PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(std::path::Path::to_path_buf)
}

/// Where the Census contact came from; the stderr notice and the tests name it, the value
/// itself is never logged.
#[derive(Clone, PartialEq, Eq)]
pub enum CensusContact {
    /// A configured, non-blank contact.
    Configured(String),
    /// Nothing configured: Census requests use the general contact.
    Fallback,
}

/// Redacted: the contact is operator-private, so no `{:?}` path may print it.
impl std::fmt::Debug for CensusContact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configured(_) => f.write_str("Configured(<redacted>)"),
            Self::Fallback => f.write_str("Fallback"),
        }
    }
}

/// Pure Census contact resolution. `env` is the raw `KOPLIK_CENSUS_CONTACT` (`None` = unset);
/// `local` is the repository's `.env.local` text (`None` = no file). The environment wins over
/// the file. A configured but blank value refuses, so it is never mistaken for "unset".
pub fn resolve_census_contact(
    env: Option<&std::ffi::OsStr>,
    local: Option<&str>,
) -> Result<CensusContact> {
    let (value, source) = match env {
        Some(raw) => (
            raw.to_str()
                .ok_or_else(|| IngestError::ContactNotUtf8 {
                    variable: CENSUS_CONTACT_ENV.to_owned(),
                })?
                .to_owned(),
            format!("the {CENSUS_CONTACT_ENV} environment variable"),
        ),
        None => match local.and_then(|t| config_value(t, CENSUS_CONTACT_ENV)) {
            Some(v) => (v, format!("{CENSUS_CONTACT_ENV} in {LOCAL_CONFIG_FILE}")),
            None => return Ok(CensusContact::Fallback),
        },
    };
    match value.trim() {
        "" => Err(IngestError::CensusContactBlank {
            source_name: source,
        }),
        v => Ok(CensusContact::Configured(v.to_owned())),
    }
}

/// Reads the Census contact: the environment variable alone when it is set (the file is not
/// even opened, so an unreadable `.env.local` cannot break a configured run), otherwise the
/// local config found from `start` upward.
fn census_contact_from(
    env: Option<&std::ffi::OsStr>,
    start: Option<&std::path::Path>,
) -> Result<CensusContact> {
    if env.is_some() {
        return resolve_census_contact(env, None);
    }
    let local = match start.and_then(repository_root) {
        Some(root) => {
            let path = root.join(LOCAL_CONFIG_FILE);
            match std::fs::read_to_string(&path) {
                Ok(text) => Some(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(source) => return Err(IngestError::Io { path, source }),
            }
        }
        None => None,
    };
    resolve_census_contact(None, local.as_deref())
}

fn census_contact_from_env() -> Result<CensusContact> {
    census_contact_from(
        std::env::var_os(CENSUS_CONTACT_ENV).as_deref(),
        std::env::current_dir().ok().as_deref(),
    )
}

/// One line on stderr (no contact value in it) when Census requests use the general contact.
pub const CENSUS_FALLBACK_NOTICE: &str = "koplik-ingest: no Census contact configured (set KOPLIK_CENSUS_CONTACT or add it to .env.local); Census requests use the general contact";

/// Offline User-Agent for tests and diagnostics: [`user_agent_with`] the resolved contact.
/// Live fetching goes through [`live_user_agent`], which refuses when the contact was blanked.
pub fn default_user_agent() -> String {
    user_agent_with(contact_from_env().as_deref())
}

/// `koplik-ingest/<version> (measles data demonstration project[; <contact>])` (a blank
/// contact counts as none). Nothing is ever invented: with no contact there is none.
pub fn user_agent_with(contact: Option<&str>) -> String {
    let base = format!(
        "koplik-ingest/{} (measles data demonstration project",
        env!("CARGO_PKG_VERSION")
    );
    match contact.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => format!("{base}; {c})"),
        None => format!("{base})"),
    }
}

/// The User-Agent for live requests: requires a non-blank, operator-supplied contact.
pub fn live_user_agent(contact: Option<&str>) -> Result<String> {
    match contact.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => Ok(user_agent_with(Some(c))),
        None => Err(IngestError::ContactRequired),
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

#[derive(Clone)]
pub struct PoliteConfig {
    pub user_agent: String,
    /// User-Agent for Census hosts ([`is_census_url`]) when a Census contact is configured;
    /// `None` sends [`PoliteConfig::user_agent`] to every host.
    pub census_user_agent: Option<String>,
    /// Set by [`PoliteConfig::live_from_env`] when Census requests fall back to the general
    /// contact: the fetcher then prints [`CENSUS_FALLBACK_NOTICE`] once, at the first Census request.
    pub census_fallback_notice: bool,
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

/// Redacted: both User-Agents embed a contact, which may be an operator-private address.
impl std::fmt::Debug for PoliteConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PoliteConfig")
            .field("user_agent", &"<redacted>")
            .field(
                "census_user_agent",
                &self.census_user_agent.as_ref().map(|_| "<redacted>"),
            )
            .field("census_fallback_notice", &self.census_fallback_notice)
            .field("min_interval", &self.min_interval)
            .field("max_attempts", &self.max_attempts)
            .field("backoff_base", &self.backoff_base)
            .field("backoff_cap", &self.backoff_cap)
            .field("max_redirects", &self.max_redirects)
            .field("max_crawl_delay", &self.max_crawl_delay)
            .finish()
    }
}

impl PoliteConfig {
    /// Configuration for live fetching: identifies the client with `contact` (normally
    /// [`contact_from_env`]) and refuses, before any request, when there is none.
    pub fn live(contact: Option<&str>) -> Result<Self> {
        Ok(Self {
            user_agent: live_user_agent(contact)?,
            ..Self::default()
        })
    }

    /// The User-Agent sent for a request to `url`: the Census one on Census hosts
    /// ([`is_census_url`]) when configured, the general one everywhere else.
    pub fn user_agent_for(&self, url: &str) -> &str {
        match &self.census_user_agent {
            Some(ua) if is_census_url(url) => ua,
            _ => &self.user_agent,
        }
    }

    /// [`PoliteConfig::live`] plus a Census contact resolved by [`resolve_census_contact`].
    pub fn live_with_census(contact: Option<&str>, census: &CensusContact) -> Result<Self> {
        let mut cfg = Self::live(contact)?;
        match census {
            CensusContact::Configured(c) => cfg.census_user_agent = Some(user_agent_with(Some(c))),
            CensusContact::Fallback => cfg.census_fallback_notice = true,
        }
        Ok(cfg)
    }
}

impl PoliteConfig {
    /// [`PoliteConfig::live_with_census`] with the contacts read from the environment and the
    /// local config: the one entry point for every live path (CLI arms, examples, the pipeline).
    /// Non-Census hosts get the general contact (`KOPLIK_CONTACT`, else [`DEFAULT_CONTACT`]);
    /// Census hosts get `KOPLIK_CENSUS_CONTACT`, else [`LOCAL_CONFIG_FILE`], else the general
    /// contact with a stderr notice. Refuses, before any request, on a blank or non-UTF-8 value.
    pub fn live_from_env() -> Result<Self> {
        let general = general_contact_checked(std::env::var_os(CONTACT_ENV).as_deref())?;
        Self::live_with_census(Some(&general), &census_contact_from_env()?)
    }
}

impl Default for PoliteConfig {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            census_user_agent: None,
            census_fallback_notice: false,
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
    /// Attempts, including failures, for this pipeline-run fetcher. Never retry named files.
    census_attempted: BTreeSet<String>,
    census_notice_pending: bool,
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
        let census_notice_pending = cfg.census_fallback_notice;
        Self {
            client,
            time,
            cfg,
            last_request: HashMap::new(),
            robots: HashMap::new(),
            census_attempted: BTreeSet::new(),
            census_notice_pending,
        }
    }

    /// The fallback notice, once, the first time a Census host is about to be requested with
    /// the general contact. Never carries a contact value.
    fn take_census_notice(&mut self, url: &str) -> Option<&'static str> {
        if self.census_notice_pending && is_census_url(url) {
            self.census_notice_pending = false;
            Some(CENSUS_FALLBACK_NOTICE)
        } else {
            None
        }
    }

    /// Operator-approved `census-access` exception, 2026-10-07 (option A with limits).
    /// www2.census.gov's empty wildcard agent group inherits RavenCrawler's Disallow: /
    /// under RFC 9309. The decision permits fixed, named public-domain file downloads,
    /// not crawling. Exact validated manifest pins alone bypass robots; every other URL
    /// still uses `fetch`. Preserve pacing/identity, make only one attempt per URL per
    /// fetcher/run, follow no redirects, and reject changed bytes before storing them.
    pub fn fetch_census_file(
        &mut self,
        url: &str,
        manifest: &NamedFileAllowlist,
    ) -> Result<Fetched> {
        let Some(pin) = manifest.pin(url) else {
            return self.fetch(url);
        };
        let target = split_url(url)?;
        if target.host != "www2.census.gov" || target.scheme != "https" {
            return Err(IngestError::Invalid(format!(
                "not an approved Census host: {url}"
            )));
        }
        if !self.census_attempted.insert(url.to_owned()) {
            return Err(IngestError::NamedFileAlreadyRequested(url.to_owned()));
        }
        let interval = self.host_interval(&target.host)?;
        if let Some(notice) = self.take_census_notice(url) {
            eprintln!("{notice}");
        }
        self.throttle(&target.host, interval);
        // No request() retry loop and no redirect following: at most one GET per file/run.
        let response = self.client.get(url, self.cfg.user_agent_for(url))?;
        if !(200..=299).contains(&response.status) {
            return Err(IngestError::BadStatus {
                status: response.status,
                url: url.to_owned(),
            });
        }
        let actual = sha256_of(&response.body);
        if actual != *pin {
            return Err(IngestError::PinMismatch {
                url: url.to_owned(),
                expected: pin.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(Fetched {
            url: url.to_owned(),
            final_url: url.to_owned(),
            status: response.status,
            content_type: response.content_type,
            body: response.body,
            retrieved_at: self.time.utc_now(),
        })
    }

    /// The general User-Agent (non-Census hosts).
    pub fn user_agent(&self) -> &str {
        &self.cfg.user_agent
    }

    /// The User-Agent this fetcher sends for a request to `url`.
    pub fn user_agent_for(&self, url: &str) -> &str {
        self.cfg.user_agent_for(url)
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
            if let Some(notice) = self.take_census_notice(url) {
                eprintln!("{notice}");
            }
            self.throttle(host, interval);
            let outcome = self.client.get(url, self.cfg.user_agent_for(url));
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
            Ok(r) if (200..300).contains(&r.status) => Robots::parse(
                &String::from_utf8_lossy(&r.body),
                self.cfg.user_agent_for(&url),
            ),
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
    fn user_agent_names_project_version_and_contact_and_invents_nothing() {
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
                "koplik-ingest/{} (measles data demonstration project; ops@example.org)",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn live_fetching_requires_a_contact_before_any_request() {
        for missing in [None, Some(""), Some("   \t")] {
            assert!(
                matches!(
                    PoliteConfig::live(missing),
                    Err(IngestError::ContactRequired)
                ),
                "{missing:?}"
            );
            assert!(matches!(
                live_user_agent(missing),
                Err(IngestError::ContactRequired)
            ));
        }
        // A configured contact reaches the wire in the User-Agent of every request.
        let cfg = PoliteConfig::live(Some(" https://example.org/koplik ")).unwrap();
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::ok(b"[]"));
        let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg);
        f.fetch(URL).unwrap();
        let want = format!(
            "koplik-ingest/{} (measles data demonstration project; https://example.org/koplik)",
            env!("CARGO_PKG_VERSION")
        );
        assert!(!c.calls.borrow().is_empty());
        assert!(c.calls.borrow().iter().all(|(_, ua)| *ua == want));
    }

    #[test]
    fn contact_resolution_unset_default_override_blank_refuse() {
        // The committed default is the public Koplik repository: a URL, never an e-mail.
        assert_eq!(DEFAULT_CONTACT, "https://github.com/jakedevar/koplik");
        // (raw KOPLIK_CONTACT value, resolved contact); `None` raw = unset.
        let table: [(Option<&str>, Option<&str>); 6] = [
            (None, Some(DEFAULT_CONTACT)),
            (
                Some("https://example.org/koplik"),
                Some("https://example.org/koplik"),
            ),
            (Some("  ops@example.org\n"), Some("ops@example.org")),
            (Some(""), None),
            (Some("   "), None),
            (Some("\t\n"), None),
        ];
        for (raw, want) in table {
            assert_eq!(resolve_contact(raw).as_deref(), want, "{raw:?}");
        }
    }

    /// One injected-client fetch with the config for a raw `KOPLIK_CONTACT` value; returns the
    /// User-Agents that reached the (fake) wire.
    fn user_agents_for(raw: Option<&str>) -> Result<Vec<String>> {
        let cfg = PoliteConfig::live(resolve_contact(raw).as_deref())?;
        let c = FakeClient::default();
        c.on(ROBOTS, FakeClient::status(404, b""));
        c.on(URL, FakeClient::ok(b"[]"));
        let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg);
        f.fetch(URL)?;
        let uas = c.calls.borrow().iter().map(|(_, ua)| ua.clone()).collect();
        Ok(uas)
    }

    #[test]
    fn unset_contact_sends_the_default_and_an_override_replaces_it() {
        let ua = |contact: &str| {
            format!(
                "koplik-ingest/{} (measles data demonstration project; {contact})",
                env!("CARGO_PKG_VERSION")
            )
        };
        let unset = user_agents_for(None).unwrap();
        assert!(!unset.is_empty());
        assert!(
            unset
                .iter()
                .all(|u| *u == ua("https://github.com/jakedevar/koplik"))
        );
        let over = user_agents_for(Some("https://example.org/koplik")).unwrap();
        assert!(!over.is_empty());
        assert!(over.iter().all(|u| *u == ua("https://example.org/koplik")));
    }

    #[test]
    fn blank_contact_refuses_before_any_request() {
        for blank in ["", "  ", "\t"] {
            // Refused while building the config, so no fetcher and no request can exist.
            assert!(
                matches!(
                    PoliteConfig::live(resolve_contact(Some(blank)).as_deref()),
                    Err(IngestError::ContactRequired)
                ),
                "{blank:?}"
            );
            assert!(matches!(
                user_agents_for(Some(blank)),
                Err(IngestError::ContactRequired)
            ));
        }
    }

    const CENSUS_URL: &str = "https://www2.census.gov/geo/tiger/x.zip";
    const CENSUS_ROBOTS: &str = "https://www2.census.gov/robots.txt";
    const ARCHIVE_URL: &str = "https://web.archive.org/web/2025/https://www2.census.gov/x.zip";
    const ARCHIVE_ROBOTS: &str = "https://web.archive.org/robots.txt";
    // Dummy addresses only: the operator's real Census contact never appears in the repository.
    const DUMMY_CENSUS: &str = "census-contact@example.invalid";

    fn ua(contact: &str) -> String {
        format!(
            "koplik-ingest/{} (measles data demonstration project; {contact})",
            env!("CARGO_PKG_VERSION")
        )
    }

    fn cfg_for(env: Option<&str>, local: Option<&str>) -> Result<PoliteConfig> {
        let census = resolve_census_contact(env.map(std::ffi::OsStr::new), local)?;
        PoliteConfig::live_with_census(Some("https://example.org/general"), &census)
    }

    /// (url, User-Agent) pairs that reached the fake wire when fetching `urls` in order.
    fn wire_for(cfg: PoliteConfig, urls: &[&str]) -> Vec<(String, String)> {
        let c = FakeClient::default();
        for robots in [CENSUS_ROBOTS, ARCHIVE_ROBOTS] {
            c.on(robots, FakeClient::status(404, b""));
        }
        for u in urls {
            c.on(u, FakeClient::ok(b"x"));
        }
        let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg);
        for u in urls {
            f.fetch(u).unwrap();
        }
        let calls = c.calls.borrow().clone();
        calls
    }

    #[test]
    fn census_urls_are_classified_from_the_parsed_host() {
        for u in [
            "https://www2.census.gov/x",
            "https://census.gov/x",
            "https://geo.census.gov:443/x",
            "https://api.census.gov:8443/x",
            "https://www2.census.gov./x",
            "https://WWW2.CENSUS.GOV/x",
            "http://www2.Census.Gov.:80/x",
            "https://user:pw@www2.census.gov/x",
        ] {
            assert!(is_census_url(u), "{u}");
        }
        for u in [
            "https://web.archive.org/web/2025/https://www2.census.gov/x.zip",
            "https://census.gov@evil.com/x",
            "https://www2.census.gov@evil.com/x",
            "https://census.gov.evil.com/x",
            "https://evilcensus.gov/x",
            "https://notcensus.gov/x",
            "https://census.gov:443.evil.com/x",
            "https://evil.com/?h=www2.census.gov",
            "https://evil.com/www2.census.gov/x",
            "https://data.cdc.gov/x",
            "https://.census.gov/x",
            "not a url",
        ] {
            assert!(!is_census_url(u), "{u}");
        }
        assert!(is_census_host("Www2.Census.Gov.") && !is_census_host("census.gov.evil.com"));
    }

    #[test]
    fn debug_output_never_carries_a_contact() {
        let cfg = cfg_for(Some(DUMMY_CENSUS), None).unwrap();
        let census = CensusContact::Configured(DUMMY_CENSUS.to_owned());
        for text in [
            format!("{cfg:?}"),
            format!("{cfg:#?}"),
            format!("{census:?}"),
        ] {
            assert!(!text.contains(DUMMY_CENSUS), "{text}");
            assert!(!text.contains("example.org/general"), "{text}");
            assert!(text.contains("<redacted>"), "{text}");
        }
        assert_eq!(format!("{:?}", CensusContact::Fallback), "Fallback");
        // Errors name the source of a value, never the value.
        let e = resolve_census_contact(Some(std::ffi::OsStr::new(" ")), None).unwrap_err();
        assert!(!e.to_string().contains(DUMMY_CENSUS));
    }

    #[test]
    fn a_configured_environment_contact_never_opens_the_local_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        // Unreadable as text (a directory) and non-UTF-8: neither may matter when the
        // environment variable is set.
        std::fs::create_dir(dir.path().join(LOCAL_CONFIG_FILE)).unwrap();
        let env = std::ffi::OsStr::new(DUMMY_CENSUS);
        assert_eq!(
            census_contact_from(Some(env), Some(dir.path())).unwrap(),
            CensusContact::Configured(DUMMY_CENSUS.to_owned())
        );
        // Only an unset variable needs the file, and then the failure is reported.
        assert!(matches!(
            census_contact_from(None, Some(dir.path())),
            Err(IngestError::Io { .. })
        ));
        std::fs::remove_dir(dir.path().join(LOCAL_CONFIG_FILE)).unwrap();
        std::fs::write(dir.path().join(LOCAL_CONFIG_FILE), b"K=\xff\xfe\n").unwrap();
        assert!(census_contact_from(None, Some(dir.path())).is_err());
        assert_eq!(
            census_contact_from(Some(env), Some(dir.path())).unwrap(),
            CensusContact::Configured(DUMMY_CENSUS.to_owned())
        );
        // A readable file is used when the variable is unset; a missing one means fallback.
        std::fs::write(
            dir.path().join(LOCAL_CONFIG_FILE),
            format!("{CENSUS_CONTACT_ENV}={DUMMY_CENSUS}\n"),
        )
        .unwrap();
        assert_eq!(
            census_contact_from(None, Some(dir.path())).unwrap(),
            CensusContact::Configured(DUMMY_CENSUS.to_owned())
        );
        std::fs::remove_file(dir.path().join(LOCAL_CONFIG_FILE)).unwrap();
        assert_eq!(
            census_contact_from(None, Some(dir.path())).unwrap(),
            CensusContact::Fallback
        );
    }

    #[test]
    fn absolute_and_uppercase_census_hosts_get_the_census_contact_on_robots_and_content() {
        let cfg = cfg_for(Some(DUMMY_CENSUS), None).unwrap();
        for url in [
            "https://www2.census.gov./geo/x.zip",
            "https://WWW2.CENSUS.GOV/geo/x.zip",
        ] {
            let c = FakeClient::default();
            let origin = &url[..url.find("/geo").unwrap()];
            // The fetcher lower-cases the host of the robots.txt URL it derives.
            c.on(
                &format!("{}/robots.txt", origin.to_ascii_lowercase()),
                FakeClient::status(404, b""),
            );
            c.on(url, FakeClient::ok(b"x"));
            let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg.clone());
            f.fetch(url).unwrap();
            let calls = c.calls.borrow();
            assert_eq!(calls.len(), 2, "{calls:?}");
            assert!(
                calls.iter().all(|(_, a)| *a == ua(DUMMY_CENSUS)),
                "{calls:?}"
            );
        }
        // A userinfo trick is refused outright by the fetcher and never classified as Census.
        assert_eq!(
            cfg.user_agent_for("https://census.gov@evil.com/"),
            ua("https://example.org/general")
        );
    }

    #[test]
    fn config_value_reads_simple_key_value_lines() {
        let text =
            "# comment\n\nOTHER=1\nexport KOPLIK_CENSUS_CONTACT = \"quoted@example.invalid\" \n";
        assert_eq!(
            config_value(text, CENSUS_CONTACT_ENV).as_deref(),
            Some("quoted@example.invalid")
        );
        assert_eq!(config_value("A=1\nA=2\n", "A").as_deref(), Some("2"));
        assert_eq!(config_value("A='x y'\n", "A").as_deref(), Some("x y"));
        assert_eq!(config_value("A=\n", "A").as_deref(), Some(""));
        assert_eq!(config_value("AA=1\n", "A"), None);
        assert_eq!(config_value("# A=1\n", "A"), None);
    }

    #[test]
    fn census_contact_resolution_env_then_local_then_fallback_blank_refuses() {
        let local = format!("{CENSUS_CONTACT_ENV}=local@example.invalid\n");
        let env = |v: &str| Some(std::ffi::OsStr::new(v).to_owned());
        let got = |e: Option<std::ffi::OsString>, l: Option<&str>| {
            resolve_census_contact(e.as_deref(), l).map_err(|e| e.to_string())
        };
        assert_eq!(got(None, None), Ok(CensusContact::Fallback));
        assert_eq!(got(None, Some("OTHER=1\n")), Ok(CensusContact::Fallback));
        assert_eq!(
            got(None, Some(&local)),
            Ok(CensusContact::Configured("local@example.invalid".into()))
        );
        // The environment wins over the file.
        assert_eq!(
            got(env(" env@example.invalid "), Some(&local)),
            Ok(CensusContact::Configured("env@example.invalid".into()))
        );
        // Blank is a refusal, in the environment and in the file, never "unset".
        for blank in ["", "  ", "\t"] {
            let e = got(env(blank), Some(&local)).unwrap_err();
            assert!(
                e.contains("is blank") && e.contains("environment variable"),
                "{e}"
            );
        }
        let e = got(None, Some(&format!("{CENSUS_CONTACT_ENV}=\n"))).unwrap_err();
        assert!(
            e.contains("is blank") && e.contains(LOCAL_CONFIG_FILE),
            "{e}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_contact_values_refuse_without_calling_them_blank() {
        use std::os::unix::ffi::OsStrExt;
        let bad = std::ffi::OsStr::from_bytes(b"ops\xff@example.invalid");
        let general = general_contact_checked(Some(bad)).unwrap_err().to_string();
        assert!(
            general.contains("KOPLIK_CONTACT") && general.contains("UTF-8"),
            "{general}"
        );
        assert!(!general.contains("blank"), "{general}");
        let census = resolve_census_contact(Some(bad), None)
            .unwrap_err()
            .to_string();
        assert!(
            census.contains("KOPLIK_CENSUS_CONTACT") && census.contains("UTF-8"),
            "{census}"
        );
        assert!(!census.contains("blank"), "{census}");
        // Blank and unset keep their own meanings on the general contact.
        assert!(matches!(
            general_contact_checked(Some(std::ffi::OsStr::new("  "))),
            Err(IngestError::ContactRequired)
        ));
        assert_eq!(general_contact_checked(None).unwrap(), DEFAULT_CONTACT);
    }

    #[test]
    fn census_hosts_send_the_census_contact_and_other_hosts_the_general_one() {
        let cfg = cfg_for(Some(DUMMY_CENSUS), None).unwrap();
        assert_eq!(cfg.user_agent_for(CENSUS_URL), ua(DUMMY_CENSUS));
        let wire = wire_for(cfg, &[CENSUS_URL, ARCHIVE_URL]);
        // robots.txt and the file itself, for both hosts.
        assert_eq!(wire.len(), 4, "{wire:?}");
        for (url, agent) in &wire {
            let want = if url.starts_with("https://www2.census.gov/") {
                ua(DUMMY_CENSUS)
            } else {
                // An archive.org request is not a Census request, even for a Census file.
                ua("https://example.org/general")
            };
            assert_eq!(*agent, want, "{url}");
        }
    }

    #[test]
    fn a_redirect_off_census_carries_the_general_contact_on_the_destination() {
        let cfg = cfg_for(Some(DUMMY_CENSUS), None).unwrap();
        let dest = "https://files.example.org/census-copy.zip";
        let c = FakeClient::default();
        c.on(CENSUS_ROBOTS, FakeClient::status(404, b""));
        c.on(
            CENSUS_URL,
            Ok(HttpResponse {
                status: 302,
                content_type: None,
                location: Some(dest.to_owned()),
                retry_after_secs: None,
                body: Vec::new(),
            }),
        );
        c.on(
            "https://files.example.org/robots.txt",
            FakeClient::status(404, b""),
        );
        c.on(dest, FakeClient::ok(b"x"));
        let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg);
        assert_eq!(f.fetch(CENSUS_URL).unwrap().final_url, dest);
        let calls = c.calls.borrow();
        assert_eq!(calls.len(), 4, "{calls:?}");
        for (url, agent) in calls.iter() {
            let want = if url.starts_with("https://www2.census.gov/") {
                ua(DUMMY_CENSUS)
            } else {
                // The destination's robots.txt and content are not Census requests.
                ua("https://example.org/general")
            };
            assert_eq!(*agent, want, "{url}");
        }
    }

    #[test]
    fn a_pinned_census_file_carries_the_census_contact() {
        use crate::store::sha256_of;
        let body = b"pinned bytes";
        let url = "https://www2.census.gov/geo/tiger/GENZ2024/shp/cb_2024_us_state_20m.zip";
        let manifest = NamedFileAllowlist::new([(url.to_owned(), sha256_of(body))]).unwrap();
        let c = FakeClient::default();
        c.on(url, FakeClient::ok(body));
        let cfg = cfg_for(Some(DUMMY_CENSUS), None).unwrap();
        let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg);
        assert_eq!(f.fetch_census_file(url, &manifest).unwrap().body, body);
        let calls = c.calls.borrow();
        assert_eq!(*calls, vec![(url.to_owned(), ua(DUMMY_CENSUS))]);
    }

    #[test]
    fn census_contact_from_the_local_config_reaches_census_requests() {
        let local = format!("{CENSUS_CONTACT_ENV}={DUMMY_CENSUS}\n");
        let wire = wire_for(cfg_for(None, Some(&local)).unwrap(), &[CENSUS_URL]);
        assert!(!wire.is_empty());
        assert!(wire.iter().all(|(_, a)| *a == ua(DUMMY_CENSUS)), "{wire:?}");
    }

    #[test]
    fn unconfigured_census_contact_falls_back_to_the_general_contact_and_says_so_once() {
        let cfg = cfg_for(None, None).unwrap();
        assert!(cfg.census_fallback_notice);
        let c = FakeClient::default();
        c.on(CENSUS_ROBOTS, FakeClient::status(404, b""));
        c.on(CENSUS_URL, FakeClient::ok(b"x"));
        let mut f = PoliteFetcher::new(c.clone(), FakeTime::default(), cfg);
        // Non-Census hosts never trigger the notice; the first Census host does, once.
        assert_eq!(f.take_census_notice("https://data.cdc.gov/x"), None);
        assert_eq!(
            f.take_census_notice(CENSUS_URL),
            Some(CENSUS_FALLBACK_NOTICE)
        );
        assert_eq!(f.take_census_notice(CENSUS_URL), None);
        assert!(!CENSUS_FALLBACK_NOTICE.contains('@'));
        f.fetch(CENSUS_URL).unwrap();
        assert!(
            c.calls
                .borrow()
                .iter()
                .all(|(_, a)| *a == ua("https://example.org/general"))
        );
        // A configured contact needs no notice.
        assert!(
            !cfg_for(Some(DUMMY_CENSUS), None)
                .unwrap()
                .census_fallback_notice
        );
    }

    #[test]
    fn repository_root_finds_the_nearest_git_ancestor_and_the_local_config_is_gitignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(repository_root(&nested).as_deref(), Some(dir.path()));
        // The real local config name must stay out of version control: it holds the contact.
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let status = std::process::Command::new("git")
            .args(["check-ignore", "-q", LOCAL_CONFIG_FILE])
            .current_dir(repo)
            .status()
            .unwrap();
        assert!(status.success(), "{LOCAL_CONFIG_FILE} is not gitignored");
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
    fn encoded_special_character_rules_block_the_literal_urls_without_a_request() {
        // RFC 9309 section 2.2.3, Figure 6: `%2A` and `%24` in a rule match a literal `*`
        // and `$` in the requested URI.
        let c = FakeClient::default();
        c.on(
            ROBOTS,
            FakeClient::ok(
                b"User-agent: *\nDisallow: /path/file-with-a-%2A.html\nDisallow: /path/foo-%24\n",
            ),
        );
        let literal = [
            "https://data.example.gov/path/file-with-a-*.html",
            "https://data.example.gov/path/foo-$",
        ];
        for url in literal {
            c.on(url, FakeClient::ok(b"x"));
        }
        let mut f = fetcher(&c, &FakeTime::default());
        for url in literal {
            assert!(
                matches!(f.fetch(url), Err(IngestError::RobotsDisallowed { .. })),
                "{url}"
            );
        }
        assert!(
            c.calls
                .borrow()
                .iter()
                .all(|(u, _)| !literal.contains(&u.as_str())),
            "no request may be sent for a forbidden URL"
        );
        // A different path on the same host is still fetched.
        c.on("https://data.example.gov/path/other", FakeClient::ok(b"ok"));
        assert_eq!(
            f.fetch("https://data.example.gov/path/other").unwrap().body,
            b"ok"
        );
    }

    #[test]
    fn normalisation_cannot_newly_allow_a_url_the_legacy_matcher_refused() {
        // (robots.txt, forbidden URL path): both were refused before the Figure 6 fix.
        let cases: [(&[u8], &str); 2] = [
            (b"User-agent: *\nDisallow: /a*bcd\nAllow: /a$b\n", "/a$bcd"),
            (
                b"User-agent: *\nDisallow: /private\nAllow: /private/%24\n",
                "/private/$",
            ),
        ];
        for (robots_txt, path) in cases {
            let c = FakeClient::default();
            c.on(ROBOTS, FakeClient::ok(robots_txt));
            let url = format!("https://data.example.gov{path}");
            c.on(&url, FakeClient::ok(b"x"));
            let mut f = fetcher(&c, &FakeTime::default());
            assert!(
                matches!(f.fetch(&url), Err(IngestError::RobotsDisallowed { .. })),
                "{url}"
            );
            assert!(
                c.calls.borrow().iter().all(|(u, _)| *u != url),
                "zero data requests for {url}"
            );
        }
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
