//! Checks on what the apps pass in, before it reaches the core: ids, dates,
//! time zone offsets and folders.

use std::path::PathBuf;

use chrono::{FixedOffset, NaiveDate};

use crate::types::invalid;
use crate::Result;

/// Ids are UUIDs (made by the core, or by the old Rails app). Anything
/// else can't name a row, so it's refused before it reaches a query.
pub(crate) fn id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(invalid("that id isn't valid"))
    }
}

/// A calendar date written `YYYY-MM-DD`, exactly: no time, no time zone,
/// a four-digit year.
pub(crate) fn date(s: &str) -> Result<NaiveDate> {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    let shaped = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && digits(0..4)
        && digits(5..7)
        && digits(8..10);
    let parsed = shaped
        .then(|| {
            // Only ASCII digits, so these slices are on character boundaries.
            let n = |r: std::ops::Range<usize>| s[r].parse::<u32>().ok();
            let year = i32::try_from(n(0..4)?).ok()?;
            NaiveDate::from_ymd_opt(year, n(5..7)?, n(8..10)?)
        })
        .flatten();
    parsed.ok_or_else(|| invalid("dates must be written YYYY-MM-DD"))
}

/// An optional date; `None` clears it.
pub(crate) fn optional_date(s: Option<&str>) -> Result<Option<NaiveDate>> {
    s.map(date).transpose()
}

/// The local time zone's offset from UTC, in minutes east (UTC+2 is 120).
/// Real offsets run from -12 to +14 hours.
pub(crate) fn utc_offset(minutes: i32) -> Result<FixedOffset> {
    if !(-18 * 60..=18 * 60).contains(&minutes) {
        return Err(invalid("that time zone offset isn't valid"));
    }
    FixedOffset::east_opt(minutes * 60).ok_or_else(|| invalid("that time zone offset isn't valid"))
}

/// A folder the user picked: an absolute path to a folder that exists.
pub(crate) fn existing_dir(s: &str) -> Result<PathBuf> {
    let path = PathBuf::from(s);
    if s.is_empty() || !path.is_absolute() || !path.is_dir() {
        return Err(invalid("that isn't a folder"));
    }
    Ok(path)
}

/// Where the journal lives: an absolute path. It's created if needed.
pub(crate) fn absolute(s: &str) -> Result<PathBuf> {
    let path = PathBuf::from(s);
    if s.is_empty() || !path.is_absolute() {
        return Err(invalid("the journal's folder must be an absolute path"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(id("7f1c8f0e-8a4b-4c55-9a3e-2b1d6c1f9e01").is_ok());
        assert!(id("abc123").is_ok());
        for bad in [
            "",
            "a b",
            "x'; DROP TABLE users; --",
            "../x",
            &"a".repeat(65),
        ] {
            assert!(id(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn dates() {
        assert_eq!(
            date("2026-09-05").unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 5).unwrap()
        );
        assert_eq!(
            date("2024-02-29").unwrap(),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
        for bad in [
            "",
            "2026-9-5",
            "2026-02-30",
            "2025-02-29",
            "2026-13-01",
            "2026-00-10",
            "26-09-05",
            "2026/09/05",
            "2026-09-05T00:00",
            "+2026-09-05",
            "２０２６-09-05",
            "20260-09-05",
            "2026-0é-05",
        ] {
            assert!(date(bad).is_err(), "{bad:?}");
        }
        assert_eq!(optional_date(None).unwrap(), None);
        assert!(optional_date(Some("nope")).is_err());
    }

    #[test]
    fn offsets() {
        assert_eq!(utc_offset(0).unwrap().local_minus_utc(), 0);
        assert_eq!(utc_offset(120).unwrap().local_minus_utc(), 7200);
        assert_eq!(utc_offset(-570).unwrap().local_minus_utc(), -34200);
        assert_eq!(utc_offset(14 * 60).unwrap().local_minus_utc(), 50400);
        assert!(utc_offset(18 * 60 + 1).is_err());
        assert!(utc_offset(i32::MIN).is_err());
        assert!(utc_offset(i32::MAX).is_err());
    }

    #[test]
    fn folders() {
        let dir = tempfile::tempdir().unwrap();
        let shown = dir.path().to_string_lossy().into_owned();
        assert_eq!(existing_dir(&shown).unwrap(), dir.path());
        assert!(existing_dir("").is_err());
        assert!(existing_dir("relative/folder").is_err());
        assert!(existing_dir(&dir.path().join("missing").to_string_lossy()).is_err());
        let file = dir.path().join("f.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(existing_dir(&file.to_string_lossy()).is_err());

        assert!(absolute(&shown).is_ok());
        assert!(absolute("").is_err());
        assert!(absolute("journal").is_err());
    }
}
