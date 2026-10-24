//! Pinned public ChapterDraft timestamps have no suffix: this instance interprets
//! them as UTC, never the process/database session's local timezone.
//! Keep this validator independent of serde and SQL so impossible calendar dates
//! are refused before a value can reach PostgreSQL's timestamp parser.
pub(crate) fn valid_publish_at(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 19 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T'
        || b[13] != b':' || b[16] != b':'
        || b.iter().enumerate().any(|(i, x)| ![4, 7, 10, 13, 16].contains(&i) && !x.is_ascii_digit())
    { return false; }
    let number = |start: usize, end: usize| -> u32 {
        b[start..end].iter().fold(0, |n, x| n * 10 + u32::from(x - b'0'))
    };
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if leap { 29 } else { 28 },
        _ => return false,
    };
    year != 0 && day >= 1 && day <= days
        && number(11, 13) < 24 && number(14, 16) < 60 && number(17, 19) < 60
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_calendar_timestamp_without_offset() {
        for value in ["0001-01-01T00:00:00", "2000-02-29T23:59:59", "2028-02-29T00:00:00", "9999-12-31T23:59:59"] {
            assert!(valid_publish_at(value), "{value}");
        }
        for value in ["", "0000-01-01T00:00:00", "1900-02-29T00:00:00", "2026-02-29T00:00:00",
            "2026-00-01T00:00:00", "2026-13-01T00:00:00", "2026-01-00T00:00:00", "2026-04-31T00:00:00",
            "2026-01-01T24:00:00", "2026-01-01T00:60:00", "2026-01-01T00:00:60",
            "2026-1-01T00:00:00", "2026-01-01t00:00:00", "2026-01-01 00:00:00", "2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00+00:00", "2026-01-01T00:00:00.000", "２０２６-01-01T00:00:00", "2026-01-01T00:00:0x"] {
            assert!(!valid_publish_at(value), "{value}");
        }
    }
}
