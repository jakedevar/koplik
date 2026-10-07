//! MMWR (epidemiological) weeks.
//!
//! CDC definition (https://wwwn.cdc.gov/nndss/document/MMWR_Week_overview.pdf): an MMWR
//! week runs Sunday to Saturday; week 1 of a year is the first week with at least four
//! days in that calendar year; a year therefore has 52 or 53 weeks. Weeks that straddle
//! New Year belong to the year that holds four or more of their days.

use chrono::{Datelike, Duration, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Supported year range (keeps all date arithmetic far from `NaiveDate` limits).
pub const MIN_YEAR: u16 = 1900;
pub const MAX_YEAR: u16 = 2200;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MmwrError {
    #[error("MMWR year {0} outside supported range {MIN_YEAR}..={MAX_YEAR}")]
    YearOutOfRange(i32),
    #[error("MMWR week {week} invalid for year {year} (that year has {weeks} weeks)")]
    WeekOutOfRange { year: u16, week: u8, weeks: u8 },
}

/// An MMWR year and week number (`week` is `1..=52` or `1..=53`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MmwrWeek {
    /// MMWR year (not always the calendar year of every day in the week).
    #[schemars(range(min = 1900, max = 2200))]
    pub year: u16,
    /// Week of the MMWR year; 1 to 52, or 53 in a 53-week year.
    #[schemars(range(min = 1, max = 53))]
    pub week: u8,
}

/// Sunday that starts MMWR week 1 of `year`.
fn week1_start(year: i32) -> NaiveDate {
    let jan1 = NaiveDate::from_ymd_opt(year, 1, 1).expect("year in supported range");
    let d = i64::from(jan1.weekday().num_days_from_sunday());
    // Jan 1 on Sun..Wed: its week has >= 4 days in `year`, so it is week 1.
    if d <= 3 {
        jan1 - Duration::days(d)
    } else {
        jan1 + Duration::days(7 - d)
    }
}

impl MmwrWeek {
    /// Validated constructor.
    pub fn new(year: u16, week: u8) -> Result<Self, MmwrError> {
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(MmwrError::YearOutOfRange(i32::from(year)));
        }
        let weeks = Self::weeks_in_year(year)?;
        if week == 0 || week > weeks {
            return Err(MmwrError::WeekOutOfRange { year, week, weeks });
        }
        Ok(Self { year, week })
    }

    /// 52 or 53.
    pub fn weeks_in_year(year: u16) -> Result<u8, MmwrError> {
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(MmwrError::YearOutOfRange(i32::from(year)));
        }
        let days = (week1_start(i32::from(year) + 1) - week1_start(i32::from(year))).num_days();
        Ok((days / 7) as u8)
    }

    /// The MMWR week containing `date`.
    pub fn from_date(date: NaiveDate) -> Result<Self, MmwrError> {
        let y = date.year();
        let year = if date >= week1_start(y + 1) {
            y + 1
        } else if date >= week1_start(y) {
            y
        } else {
            y - 1
        };
        let year_u16 = u16::try_from(year)
            .ok()
            .filter(|v| (MIN_YEAR..=MAX_YEAR).contains(v))
            .ok_or(MmwrError::YearOutOfRange(year))?;
        let week = ((date - week1_start(year)).num_days() / 7 + 1) as u8;
        Ok(Self {
            year: year_u16,
            week,
        })
    }

    /// Sunday on which this week starts.
    pub fn start_date(self) -> NaiveDate {
        week1_start(i32::from(self.year)) + Duration::days(7 * (i64::from(self.week) - 1))
    }

    /// Saturday on which this week ends.
    pub fn end_date(self) -> NaiveDate {
        self.start_date() + Duration::days(6)
    }

    /// The following week.
    pub fn next(self) -> Result<Self, MmwrError> {
        Self::from_date(self.start_date() + Duration::days(7))
    }

    /// The preceding week.
    pub fn prev(self) -> Result<Self, MmwrError> {
        Self::from_date(self.start_date() - Duration::days(7))
    }
}

impl std::fmt::Display for MmwrWeek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-W{:02}", self.year, self.week)
    }
}

impl<'de> Deserialize<'de> for MmwrWeek {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            year: u16,
            week: u8,
        }
        let r = Raw::deserialize(d)?;
        MmwrWeek::new(r.year, r.week).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn year_boundaries() {
        assert_eq!(
            MmwrWeek::from_date(d(2024, 12, 29)).unwrap(),
            MmwrWeek {
                year: 2025,
                week: 1
            }
        );
        assert_eq!(
            MmwrWeek::from_date(d(2024, 12, 28)).unwrap(),
            MmwrWeek {
                year: 2024,
                week: 52
            }
        );
        assert_eq!(MmwrWeek::weeks_in_year(2025).unwrap(), 53);
        assert_eq!(MmwrWeek::weeks_in_year(2024).unwrap(), 52);
        let w53 = MmwrWeek {
            year: 2025,
            week: 53,
        };
        assert_eq!(
            (w53.start_date(), w53.end_date()),
            (d(2025, 12, 28), d(2026, 1, 3))
        );
        assert_eq!(MmwrWeek::from_date(d(2026, 1, 3)).unwrap(), w53);
        assert_eq!(
            MmwrWeek::from_date(d(2026, 1, 4)).unwrap(),
            MmwrWeek {
                year: 2026,
                week: 1
            }
        );
        assert_eq!(
            MmwrWeek::from_date(d(2025, 1, 1)).unwrap(),
            MmwrWeek {
                year: 2025,
                week: 1
            }
        );
        assert_eq!(
            w53.next().unwrap(),
            MmwrWeek {
                year: 2026,
                week: 1
            }
        );
        assert_eq!(
            MmwrWeek {
                year: 2026,
                week: 1
            }
            .prev()
            .unwrap(),
            w53
        );
    }

    #[test]
    fn round_trips_every_day_1990_to_2060() {
        let mut date = d(1990, 1, 1);
        while date < d(2060, 1, 1) {
            let w = MmwrWeek::from_date(date).unwrap();
            assert!(w.start_date() <= date && date <= w.end_date(), "{date} {w}");
            assert_eq!(w.start_date().weekday().num_days_from_sunday(), 0);
            assert_eq!(MmwrWeek::new(w.year, w.week).unwrap(), w);
            assert_eq!(MmwrWeek::from_date(w.start_date()).unwrap(), w);
            assert_eq!(MmwrWeek::from_date(w.end_date()).unwrap(), w);
            date += Duration::days(1);
        }
        for year in 1990..2060u16 {
            let n = MmwrWeek::weeks_in_year(year).unwrap();
            assert!(n == 52 || n == 53);
            let last = MmwrWeek { year, week: n };
            // Week 1 has >= 4 days in its calendar year.
            let first = MmwrWeek { year, week: 1 };
            assert!(first.end_date().year() == i32::from(year));
            assert!(first.end_date().ordinal() >= 4);
            assert_eq!(
                last.next().unwrap(),
                MmwrWeek {
                    year: year + 1,
                    week: 1
                }
            );
        }
    }

    #[test]
    fn rejects_invalid_weeks() {
        assert!(MmwrWeek::new(2025, 0).is_err());
        assert!(MmwrWeek::new(2025, 54).is_err());
        assert!(MmwrWeek::new(2024, 53).is_err());
        assert!(MmwrWeek::new(1800, 1).is_err());
        assert!(serde_json::from_str::<MmwrWeek>(r#"{"year":2024,"week":53}"#).is_err());
        assert!(serde_json::from_str::<MmwrWeek>(r#"{"year":2025,"week":53}"#).is_ok());
        assert!(serde_json::from_str::<MmwrWeek>(r#"{"year":2025,"week":1,"x":1}"#).is_err());
    }
}
