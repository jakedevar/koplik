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
    pattern: String,
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
            rules: vec![Rule {
                allow: false,
                pattern: "/".into(),
            }],
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
                            g.rules.push(Rule {
                                allow: key == "allow",
                                pattern: normalize_encoding(value),
                            });
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
    pub fn allowed(&self, path_and_query: &str) -> bool {
        let path_and_query = &normalize_encoding(path_and_query);
        let mut best: Option<(&Rule, usize)> = None;
        for r in &self.rules {
            if matches(&r.pattern, path_and_query) {
                let len = r.pattern.len();
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

/// RFC 9309 section 2.2.2 percent-encoding normalisation, applied to both rule patterns and
/// request paths before comparing: an escaped unreserved octet (`%62` for `b`) is decoded;
/// any other escape keeps its escape with upper-case hex (`%2f` -> `%2F`, so an escaped
/// reserved character never turns into a separator or wildcard); octets outside ASCII, and
/// ASCII that is neither unreserved nor reserved, are escaped. `*` and `$` stay literal so
/// patterns keep their wildcard meaning.
fn normalize_encoding(s: &str) -> String {
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
        // Stray `%` that is not an escape stays as it is.
        let r = Robots::parse("User-agent: *\nDisallow: /100%\n", AGENT);
        assert!(!r.allowed("/100%"));
    }

    #[test]
    fn disallow_all_and_allow_all() {
        assert!(!Robots::disallow_all().allowed("/x"));
        assert!(Robots::allow_all().allowed("/x"));
    }
}
