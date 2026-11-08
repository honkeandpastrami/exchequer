//! Calendar months, the unit of settlement (design §4.3). Written `YYYY-MM`.

use std::fmt;
use std::str::FromStr;

use time::{Date, Month as CalendarMonth};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a month (YYYY-MM): {0:?}")]
pub struct MonthError(String);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Month {
    year: i32,
    month: CalendarMonth,
}

impl Month {
    pub fn new(year: i32, month: u8) -> Result<Self, MonthError> {
        let bad = || MonthError(format!("{year:04}-{month:02}"));
        if !(1..=9999).contains(&year) {
            return Err(bad());
        }
        let month = CalendarMonth::try_from(month).map_err(|_| bad())?;
        Ok(Self { year, month })
    }

    /// The month `date` falls in.
    #[must_use]
    pub const fn of(date: Date) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
        }
    }

    #[must_use]
    pub const fn year(&self) -> i32 {
        self.year
    }

    #[must_use]
    pub const fn number(&self) -> u8 {
        self.month as u8
    }

    #[must_use]
    pub fn first_day(&self) -> Date {
        Date::from_calendar_date(self.year, self.month, 1).expect("day 1 exists")
    }

    #[must_use]
    pub fn last_day(&self) -> Date {
        self.next()
            .first_day()
            .previous_day()
            .expect("not the first day ever")
    }

    #[must_use]
    pub const fn previous(&self) -> Self {
        match self.month {
            CalendarMonth::January => Self {
                year: self.year - 1,
                month: CalendarMonth::December,
            },
            m => Self {
                year: self.year,
                month: m.previous(),
            },
        }
    }

    #[must_use]
    pub const fn next(&self) -> Self {
        match self.month {
            CalendarMonth::December => Self {
                year: self.year + 1,
                month: CalendarMonth::January,
            },
            m => Self {
                year: self.year,
                month: m.next(),
            },
        }
    }

    #[must_use]
    pub fn contains(&self, date: Date) -> bool {
        Self::of(date) == *self
    }
}

impl FromStr for Month {
    type Err = MonthError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || MonthError(s.to_owned());
        let (y, m) = s.split_once('-').ok_or_else(bad)?;
        if y.len() != 4 || m.len() != 2 || !(y.bytes().chain(m.bytes())).all(|b| b.is_ascii_digit())
        {
            return Err(bad());
        }
        Self::new(y.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?).map_err(|_| bad())
    }
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.number())
    }
}

impl fmt::Debug for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Month({self})")
    }
}

impl serde::Serialize for Month {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Month {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;

    #[test]
    fn parse_and_display() {
        let m: Month = "2026-07".parse().unwrap();
        assert_eq!(m.to_string(), "2026-07");
        assert_eq!(m.first_day(), date!(2026 - 07 - 01));
        assert_eq!(m.last_day(), date!(2026 - 07 - 31));
        for bad in [
            "2026-7",
            "2026-13",
            "2026-00",
            "26-07",
            "2026/07",
            "2026-07-01",
            "+026-07",
        ] {
            assert!(bad.parse::<Month>().is_err(), "{bad}");
        }
    }

    #[test]
    fn neighbours() {
        let jan: Month = "2026-01".parse().unwrap();
        assert_eq!(jan.previous().to_string(), "2025-12");
        assert_eq!(jan.previous().next(), jan);
        assert_eq!(
            "2024-02".parse::<Month>().unwrap().last_day(),
            date!(2024 - 02 - 29)
        );
        assert!(jan < jan.next());
        assert_eq!(Month::of(date!(2026 - 01 - 31)), jan);
    }
}
