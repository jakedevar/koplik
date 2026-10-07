//! Minimal robots.txt parser (RFC 9309 subset: groups, `Allow`/`Disallow` with `*` and `$`
//! wildcards, longest-match wins with `Allow` preferred on ties, `Crawl-delay`).

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Robots {
    rules: Vec<Rule>,
    crawl_delay_secs: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct Rule {
    allow: bool,
    /// Canonical form (RFC 9309 section 2.2.2/2.2.3): see [`Mode::Rule`].
    pattern: String,
    /// Legacy form (`Mode::Legacy`): the matcher that shipped before the Figure 6 fix.
    legacy: String,
}

impl Rule {
    fn new(allow: bool, raw: &str) -> Self {
        Self {
            allow,
            pattern: normalize_encoding(raw, Mode::Rule),
            legacy: normalize_encoding(raw, Mode::Legacy),
        }
    }
}

impl Robots {
    /// No restrictions (used when the host serves no robots.txt: 404/410).
    pub fn allow_all() -> Self {
        Self::default()
    }

    /// Everything disallowed (used when robots.txt cannot be fetched because of a server
    /// error or because it is itself forbidden: the conservative reading of RFC 9309).
    pub fn disallow_all() -> Self {
        Self {
            rules: vec![Rule::new(false, "/")],
            crawl_delay_secs: None,
        }
    }

    /// Parse `text` for the product token `agent` (matched case-insensitively as a prefix of
    /// the group's `User-agent`; falls back to the `*` group).
    pub fn parse(text: &str, agent: &str) -> Self {
        struct Group {
            agents: Vec<String>,
            rules: Vec<Rule>,
            delay: Option<f64>,
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut in_agent_run = false;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "user-agent" => {
                    if !in_agent_run {
                        groups.push(Group {
                            agents: Vec::new(),
                            rules: Vec::new(),
                            delay: None,
                        });
                    }
                    groups
                        .last_mut()
                        .expect("group pushed")
                        .agents
                        .push(value.to_ascii_lowercase());
                    in_agent_run = true;
                }
                "allow" | "disallow" => {
                    in_agent_run = false;
                    if let Some(g) = groups.last_mut() {
                        // An empty `Disallow:` means allow everything: no rule.
                        if !value.is_empty() {
                            g.rules.push(Rule::new(key == "allow", value));
                        }
                    }
                }
                "crawl-delay" => {
                    in_agent_run = false;
                    if let (Some(g), Ok(d)) = (groups.last_mut(), value.parse::<f64>())
                        && d.is_finite()
                        && d >= 0.0
                    {
                        g.delay = Some(d);
                    }
                }
                _ => in_agent_run = false,
            }
        }
        let agent = agent.to_ascii_lowercase();
        let specific = groups.iter().filter(|g| {
            g.agents
                .iter()
                .any(|a| a != "*" && agent.starts_with(a.as_str()))
        });
        let chosen: Vec<&Group> = {
            let s: Vec<&Group> = specific.collect();
            if s.is_empty() {
                groups
                    .iter()
                    .filter(|g| g.agents.iter().any(|a| a == "*"))
                    .collect()
            } else {
                s
            }
        };
        let mut out = Robots::default();
        for g in chosen {
            out.rules.extend(g.rules.iter().cloned());
            if let Some(d) = g.delay {
                out.crawl_delay_secs = Some(out.crawl_delay_secs.map_or(d, |c: f64| c.max(d)));
            }
        }
        out
    }

    pub fn crawl_delay_secs(&self) -> Option<f64> {
        self.crawl_delay_secs
    }

    /// Whether `path_and_query` (starting with `/`) may be fetched.
    ///
    /// A URL is fetched only if **both** matchers allow it: the canonical one (RFC 9309
    /// section 2.2.2/2.2.3, including Figure 6: `%2A`/`%24` in a rule match a literal `*`/`$`
    /// in the URI) and the legacy one, which keeps `*`/`$` literal in the URI and never
    /// escapes them. Section 2.2.2 requires that "most octets" be compared after
    /// normalisation, and that normalisation can lengthen a rule (`/a$b` -> `/a%24b`), which
    /// can flip the longest-match precedence and turn a refusal into a permission. Taking the
    /// conjunction makes it true by construction that normalisation never newly grants an
    /// `Allow` exception: the fetcher refuses at least everything the legacy matcher refused,
    /// plus the Figure 6 literal forms (round-2 review finding
    /// `robots-normalization-allow-regression`).
    pub fn allowed(&self, path_and_query: &str) -> bool {
        self.decide(path_and_query, Mode::Rule) && self.decide(path_and_query, Mode::Legacy)
    }

    /// One matcher's decision: longest matching rule wins, `Allow` on ties, no match allows.
    /// `Mode::Rule` selects the canonical rule patterns and URI form, `Mode::Legacy` the legacy
    /// ones (`Mode::Uri` is the canonical URI normalisation and is not a decision mode).
    fn decide(&self, path_and_query: &str, mode: Mode) -> bool {
        let canonical = mode == Mode::Rule;
        let path = &normalize_encoding(
            path_and_query,
            if canonical { Mode::Uri } else { Mode::Legacy },
        );
        let mut best: Option<(&Rule, usize)> = None;
        for r in &self.rules {
            let p = if canonical { &r.pattern } else { &r.legacy };
            if matches(p, path) {
                let len = p.len();
                let better = match best {
                    None => true,
                    Some((b, bl)) => len > bl || (len == bl && r.allow && !b.allow),
                };
                if better {
                    best = Some((r, len));
                }
            }
        }
        best.is_none_or(|(r, _)| r.allow)
    }
}

/// What is being normalised: a robots.txt rule path or a request URI path.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `*` is a wildcard and a final `$` an end anchor; any other `$` is a literal.
    Rule,
    /// Every `*` and `$` is a literal octet, so it is escaped and can only match a rule that
    /// spells it `%2A`/`%24` or covers it with a wildcard (RFC 9309 section 2.2.3, Figure 6).
    Uri,
    /// The pre-Figure-6 behaviour, for both rules and URIs: `*` and `$` pass through untouched.
    Legacy,
}

/// RFC 9309 section 2.2.2 percent-encoding normalisation, applied to rule paths and request
/// paths before comparing: an escaped unreserved octet (`%62` for `b`) is decoded; any other
/// escape keeps its escape with upper-case hex (`%2f` -> `%2F`, so an escaped reserved
/// character never turns into a separator or wildcard); octets outside ASCII, and ASCII that
/// is neither unreserved nor reserved, are escaped. The `*`/`$` handling differs by [`Mode`].
fn normalize_encoding(s: &str, mode: Mode) -> String {
    const RESERVED: &[u8] = b":/?#[]@!$&'()*+,;=";
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let escaped = (b == b'%' && i + 2 < bytes.len())
            .then(|| {
                let hi = (bytes[i + 1] as char).to_digit(16)?;
                let lo = (bytes[i + 2] as char).to_digit(16)?;
                Some((hi * 16 + lo) as u8)
            })
            .flatten();
        let unreserved =
            |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~');
        match escaped {
            Some(c) if unreserved(c) => {
                out.push(char::from(c));
                i += 3;
            }
            Some(c) => {
                out.push_str(&format!("%{c:02X}"));
                i += 3;
            }
            None if matches!(b, b'*' | b'$')
                && (mode == Mode::Uri
                    || (mode == Mode::Rule && b == b'$' && i + 1 < bytes.len())) =>
            {
                out.push_str(&format!("%{b:02X}"));
                i += 1;
            }
            None if unreserved(b) || RESERVED.contains(&b) || b == b'%' => {
                out.push(char::from(b));
                i += 1;
            }
            None => {
                out.push_str(&format!("%{b:02X}"));
                i += 1;
            }
        }
    }
    out
}

/// Prefix match with `*` (any run) and a trailing `$` (end anchor).
fn matches(pattern: &str, path: &str) -> bool {
    let (pat, anchored) = match pattern.strip_suffix('$') {
        Some(p) => (p, true),
        None => (pattern, false),
    };
    let parts: Vec<&str> = pat.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            if !path.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 && anchored {
            // The last segment must end the path (and not overlap what was already consumed).
            return path.len() >= pos + part.len() && path.ends_with(part);
        } else {
            match path[pos..].find(part) {
                Some(at) => pos += at + part.len(),
                None => return false,
            }
        }
    }
    !anchored || pos == path.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENT: &str = "koplik-ingest";

    #[test]
    fn wildcard_group_and_longest_match() {
        let r = Robots::parse(
            "User-agent: *\nCrawl-delay: 2\nDisallow: /browse?*&q=\nDisallow: /private\nAllow: /private/open\n",
            AGENT,
        );
        assert_eq!(r.crawl_delay_secs(), Some(2.0));
        assert!(r.allowed("/resource/x9gk-5huc.json?$limit=5"));
        assert!(!r.allowed("/browse?category=a&q=measles"));
        assert!(r.allowed("/browse?category=a"));
        assert!(!r.allowed("/private/x"));
        assert!(r.allowed("/private/open/page"));
    }

    #[test]
    fn specific_agent_group_beats_wildcard() {
        let text = "User-agent: *\nDisallow: /\n\nUser-agent: koplik\nDisallow: /nope\n";
        let r = Robots::parse(text, AGENT);
        assert!(r.allowed("/data"));
        assert!(!r.allowed("/nope"));
    }

    #[test]
    fn empty_disallow_allows_all_and_end_anchor_works() {
        let r = Robots::parse("User-agent: *\nDisallow:\n", AGENT);
        assert!(r.allowed("/anything"));
        let r = Robots::parse("User-agent: *\nDisallow: /*.json$\n", AGENT);
        assert!(!r.allowed("/a/b.json"));
        assert!(r.allowed("/a/b.json?x=1"));
    }

    #[test]
    fn percent_encoded_equivalents_compare_equal() {
        // Unreserved octets decode: %62 is `b`, in the rule or in the path.
        let r = Robots::parse("User-agent: *\nDisallow: /pri%76ate/a%62c\n", AGENT);
        assert!(!r.allowed("/private/abc"));
        assert!(!r.allowed("/%70rivate/a%62c"));
        assert!(!r.allowed("/private/a%62c/more"));
        assert!(r.allowed("/private/abd"));
        // Reserved octets stay encoded: %2F is not a path separator, in either direction.
        let r = Robots::parse("User-agent: *\nDisallow: /a%2fb\n", AGENT);
        assert!(!r.allowed("/a%2Fb"));
        assert!(!r.allowed("/a%2fb"));
        assert!(r.allowed("/a/b"));
        let r = Robots::parse("User-agent: *\nDisallow: /a/b\n", AGENT);
        assert!(r.allowed("/a%2Fb"));
        // Non-ASCII: literal UTF-8 equals its escaped form, any hex case.
        let r = Robots::parse("User-agent: *\nDisallow: /caf\u{e9}\n", AGENT);
        assert!(!r.allowed("/caf%C3%A9"));
        assert!(!r.allowed("/caf%c3%a9"));
        let r = Robots::parse("User-agent: *\nDisallow: /caf%c3%a9\n", AGENT);
        assert!(!r.allowed("/caf\u{e9}"));
        // An escaped `*` or `$` is a literal octet, not a wildcard or an end anchor.
        let r = Robots::parse("User-agent: *\nDisallow: /x%2A\n", AGENT);
        assert!(r.allowed("/xyz"));
        assert!(!r.allowed("/x%2a"));
        assert!(!r.allowed("/x*"));
        // Stray `%` that is not an escape stays as it is.
        let r = Robots::parse("User-agent: *\nDisallow: /100%\n", AGENT);
        assert!(!r.allowed("/100%"));
    }

    /// RFC 9309 section 2.2.3, Figure 6 (and the forms that must keep their pattern meaning):
    /// `(rule path, URI path, rule matches)`.
    const FIGURE_6: &[(&str, &str, bool)] = &[
        ("/foo/bar?baz=quz", "/foo/bar?baz=quz", true),
        ("/foo/bar/\u{30c4}", "/foo/bar/%E3%83%84", true),
        ("/foo/bar/%E3%83%84", "/foo/bar/%E3%83%84", true),
        ("/foo/bar/%E3%83%84", "/foo/bar/\u{30c4}", true),
        ("/foo/bar/%62%61%7A", "/foo/bar/baz", true),
        (
            "/path/file-with-a-%2A.html",
            "/path/file-with-a-%2A.html",
            true,
        ),
        (
            "/path/file-with-a-%2A.html",
            "/path/file-with-a-*.html",
            true,
        ),
        (
            "/path/file-with-a-%2A.html",
            "/path/file-with-a-x.html",
            false,
        ),
        ("/path/foo-%24", "/path/foo-%24", true),
        ("/path/foo-%24", "/path/foo-$", true),
        ("/path/foo-%24", "/path/foo-x", false),
        // Unescaped specials in a rule keep their syntax: `*` wildcard, final `$` anchor,
        // a non-final `$` is a literal.
        ("/path/file-with-a-*.html", "/path/file-with-a-*.html", true),
        ("/path/file-with-a-*.html", "/path/file-with-a-x.html", true),
        ("/path/foo-$", "/path/foo-", true),
        ("/path/foo-$", "/path/foo-x", false),
        ("/path/a$b", "/path/a$b", true),
        ("/path/a$b", "/path/a%24b", true),
    ];

    #[test]
    fn rfc_9309_figure_6_percent_encoding_rows() {
        for (rule, uri, blocked) in FIGURE_6 {
            let r = Robots::parse(&format!("User-agent: *\nDisallow: {rule}\n"), AGENT);
            assert_eq!(!r.allowed(uri), *blocked, "Disallow: {rule} vs {uri}");
        }
    }

    #[test]
    fn normalisation_never_turns_a_refusal_into_a_permission() {
        // Round-2 review finding: expanding `/a$b` to `/a%24b` made it tie with `/a*bcd` and
        // win the Allow tie, newly allowing `/a$bcd`; the legacy matcher refused it.
        let r = Robots::parse("User-agent: *\nDisallow: /a*bcd\nAllow: /a$b\n", AGENT);
        assert!(!r.allowed("/a$bcd"));
        // Likewise an Allow written with an escape must not newly open a Disallowed subtree.
        let r = Robots::parse(
            "User-agent: *\nDisallow: /private\nAllow: /private/%24\n",
            AGENT,
        );
        assert!(!r.allowed("/private/$"));
        // The explicit escaped spelling the site allowed is still allowed (both matchers agree).
        assert!(r.allowed("/private/%24"));
    }

    /// A reference for the matcher's decisions as of 46e00729: an adapted rewrite of that
    /// commit's normalization and precedence that shares the production `matches()` (which
    /// is byte-identical to 46e00729's), not a verbatim copy. `*` and `$` pass through
    /// untouched in rules and in URIs. The #1348 rev-2 round-3 reviewer cross-checked it
    /// against the historical source over 1,860,867 comparisons with identical decisions.
    fn legacy_reference_allowed(rules: &[(bool, String)], path: &str) -> bool {
        fn norm(s: &str) -> String {
            const RESERVED: &[u8] = b":/?#[]@!$&'()*+,;=";
            let unreserved =
                |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~');
            let b = s.as_bytes();
            let (mut out, mut i) = (String::new(), 0);
            while i < b.len() {
                let esc = (b[i] == b'%' && i + 2 < b.len())
                    .then(|| {
                        let hi = (b[i + 1] as char).to_digit(16)?;
                        let lo = (b[i + 2] as char).to_digit(16)?;
                        Some((hi * 16 + lo) as u8)
                    })
                    .flatten();
                match esc {
                    Some(c) if unreserved(c) => {
                        out.push(char::from(c));
                        i += 3;
                    }
                    Some(c) => {
                        out.push_str(&format!("%{c:02X}"));
                        i += 3;
                    }
                    None if unreserved(b[i]) || RESERVED.contains(&b[i]) || b[i] == b'%' => {
                        out.push(char::from(b[i]));
                        i += 1;
                    }
                    None => {
                        out.push_str(&format!("%{:02X}", b[i]));
                        i += 1;
                    }
                }
            }
            out
        }
        let path = norm(path);
        let mut best: Option<(bool, usize)> = None;
        for (allow, raw) in rules {
            let p = norm(raw);
            if matches(&p, &path) {
                let better = match best {
                    None => true,
                    Some((ba, bl)) => p.len() > bl || (p.len() == bl && *allow && !ba),
                };
                if better {
                    best = Some((*allow, p.len()));
                }
            }
        }
        best.is_none_or(|(a, _)| a)
    }

    /// Deterministic splitmix64, so the differential test is seeded and offline.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    #[test]
    fn seeded_differential_everything_the_legacy_matcher_refused_is_still_refused() {
        const TOKENS: &[&str] = &["/", "a", "b", "*", "$", "%2A", "%24", "%2a", "%7E", "~"];
        let mut rng = Rng(0x1348_0000_0000_0001);
        let (mut refused_legacy, mut figure6_extra) = (0u32, 0u32);
        for _ in 0..4000 {
            let rules: Vec<(bool, String)> = (0..1 + rng.below(4))
                .map(|_| {
                    let n = 1 + rng.below(5);
                    let mut p = String::from("/");
                    for _ in 0..n {
                        p.push_str(TOKENS[rng.below(TOKENS.len())]);
                    }
                    (rng.below(2) == 0, p)
                })
                .collect();
            let text: String = std::iter::once("User-agent: *\n".to_owned())
                .chain(rules.iter().map(|(allow, p)| {
                    format!("{}: {p}\n", if *allow { "Allow" } else { "Disallow" })
                }))
                .collect();
            let robots = Robots::parse(&text, AGENT);
            for _ in 0..16 {
                let mut path = String::from("/");
                for _ in 0..1 + rng.below(6) {
                    path.push_str(TOKENS[rng.below(TOKENS.len())]);
                }
                let legacy = legacy_reference_allowed(&rules, &path);
                let now = robots.allowed(&path);
                if !legacy {
                    refused_legacy += 1;
                    assert!(!now, "legacy refused but now allowed: {text:?} {path:?}");
                } else if !now {
                    figure6_extra += 1;
                }
            }
        }
        // The generator must exercise both directions, or the test proves nothing.
        assert!(refused_legacy > 5000, "{refused_legacy}");
        assert!(figure6_extra > 50, "{figure6_extra}");
        // Figure 6 literal forms stay refused.
        for (rule, uri) in [
            ("/path/file-with-a-%2A.html", "/path/file-with-a-*.html"),
            ("/path/foo-%24", "/path/foo-$"),
        ] {
            let r = Robots::parse(&format!("User-agent: *\nDisallow: {rule}\n"), AGENT);
            assert!(!r.allowed(uri));
        }
    }

    #[test]
    fn disallow_all_and_allow_all() {
        assert!(!Robots::disallow_all().allowed("/x"));
        assert!(Robots::allow_all().allowed("/x"));
    }
}
