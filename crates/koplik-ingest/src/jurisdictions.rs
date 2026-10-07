//! NNDSS reporting-jurisdiction names -> state FIPS (the contracts key). Names are matched
//! exactly as CDC prints them; an unrecognised name is an error, never silently dropped.

use koplik_contracts::v1::StateFips;

/// (NNDSS name, state FIPS). New York City reports separately from New York State in NNDSS;
/// both map to FIPS 36 and the connector adds them (documented in `SOURCES.md`).
const STATES: &[(&str, u8)] = &[
    ("Alabama", 1),
    ("Alaska", 2),
    ("Arizona", 4),
    ("Arkansas", 5),
    ("California", 6),
    ("Colorado", 8),
    ("Connecticut", 9),
    ("Delaware", 10),
    ("District of Columbia", 11),
    ("Florida", 12),
    ("Georgia", 13),
    ("Hawaii", 15),
    ("Idaho", 16),
    ("Illinois", 17),
    ("Indiana", 18),
    ("Iowa", 19),
    ("Kansas", 20),
    ("Kentucky", 21),
    ("Louisiana", 22),
    ("Maine", 23),
    ("Maryland", 24),
    ("Massachusetts", 25),
    ("Michigan", 26),
    ("Minnesota", 27),
    ("Mississippi", 28),
    ("Missouri", 29),
    ("Montana", 30),
    ("Nebraska", 31),
    ("Nevada", 32),
    ("New Hampshire", 33),
    ("New Jersey", 34),
    ("New Mexico", 35),
    ("New York", 36),
    ("New York City", 36),
    ("North Carolina", 37),
    ("North Dakota", 38),
    ("Ohio", 39),
    ("Oklahoma", 40),
    ("Oregon", 41),
    ("Pennsylvania", 42),
    ("Rhode Island", 44),
    ("South Carolina", 45),
    ("South Dakota", 46),
    ("Tennessee", 47),
    ("Texas", 48),
    ("Utah", 49),
    ("Vermont", 50),
    ("Virginia", 51),
    ("Washington", 53),
    ("West Virginia", 54),
    ("Wisconsin", 55),
    ("Wyoming", 56),
    ("American Samoa", 60),
    ("Guam", 66),
    ("Commonwealth of Northern Mariana Islands", 69),
    ("Puerto Rico", 72),
    ("U.S. Virgin Islands", 78),
];

/// Rows that are sums or categories, not a geography we publish.
const AGGREGATES: &[&str] = &[
    "Total",
    "U.S. Residents",
    "U.S. Territories",
    "Non-U.S. Residents",
    "New England",
    "Middle Atlantic",
    "East North Central",
    "West North Central",
    "South Atlantic",
    "East South Central",
    "West South Central",
    "Mountain",
    "Pacific",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jurisdiction {
    State(StateFips),
    /// A regional or national total, intentionally not published as a geography.
    Aggregate,
    Unknown,
}

pub fn lookup(name: &str) -> Jurisdiction {
    if let Some((_, f)) = STATES.iter().find(|(n, _)| *n == name) {
        return Jurisdiction::State(StateFips::new(*f).expect("table FIPS is valid"));
    }
    if AGGREGATES.contains(&name) {
        return Jurisdiction::Aggregate;
    }
    Jurisdiction::Unknown
}

/// Every published state-level geography, ascending by FIPS, with the NNDSS names that
/// contribute to it.
pub fn state_components() -> Vec<(StateFips, Vec<&'static str>)> {
    let mut out: Vec<(StateFips, Vec<&'static str>)> = Vec::new();
    for (name, f) in STATES {
        let fips = StateFips::new(*f).expect("table FIPS is valid");
        match out.iter_mut().find(|(s, _)| *s == fips) {
            Some((_, names)) => names.push(name),
            None => out.push((fips, vec![name])),
        }
    }
    out.sort_by_key(|(s, _)| *s);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifty_states_dc_and_five_territories() {
        let comps = state_components();
        assert_eq!(comps.len(), 56);
        let ny = comps.iter().find(|(s, _)| s.code() == 36).unwrap();
        assert_eq!(ny.1, vec!["New York", "New York City"]);
        assert_eq!(
            lookup("Texas"),
            Jurisdiction::State(StateFips::new(48).unwrap())
        );
        assert_eq!(lookup("Pacific"), Jurisdiction::Aggregate);
        assert_eq!(lookup("Atlantis"), Jurisdiction::Unknown);
    }
}
