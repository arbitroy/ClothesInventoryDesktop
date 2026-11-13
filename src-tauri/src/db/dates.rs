use super::{DbError, DbResult};
use chrono::{Local, NaiveDate, TimeZone};

/// First and last millisecond of a day, in the computer's local time.
pub fn day_bounds_ms(date: &str) -> DbResult<(i64, i64)> {
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| DbError::InvalidDate)?;
    let next = day.succ_opt().ok_or(DbError::InvalidDate)?;

    let start = local_midnight_ms(day)?;
    let end = local_midnight_ms(next)? - 1; // one millisecond before the next day
    Ok((start, end))
}

fn local_midnight_ms(day: NaiveDate) -> DbResult<i64> {
    let midnight = day.and_hms_opt(0, 0, 0).ok_or(DbError::InvalidDate)?;
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .ok_or(DbError::InvalidDate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_bounds_cover_one_day() {
        let (start, end) = day_bounds_ms("2026-06-15").unwrap();
        assert_eq!(end - start, 24 * 60 * 60 * 1000 - 1);

        // the next day starts right after this one ends
        let (next_start, _) = day_bounds_ms("2026-06-16").unwrap();
        assert_eq!(next_start, end + 1);
    }

    #[test]
    fn bad_dates_are_refused() {
        assert!(matches!(day_bounds_ms("tomorrow"), Err(DbError::InvalidDate)));
        assert!(matches!(day_bounds_ms("2026-13-40"), Err(DbError::InvalidDate)));
    }
}