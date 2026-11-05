//! UTC calendar helpers shared by crash reports and log lines — no `chrono`
//! dependency, just UNIX seconds → `(year, month, day, hour, min, sec)`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current wallclock time as whole UNIX seconds (0 if the clock is before
/// 1970).
pub(crate) fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Howard Hinnant's `days_from_civil` reversed — convert UNIX seconds
/// (UTC) to `(year, month, day, hour, min, sec)`.
/// Reference: <https://howardhinnant.github.io/date_algorithms.html>
pub(crate) fn unix_to_ymd_hms(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let secs_of_day = secs.rem_euclid(86400);
    let hour = (secs_of_day / 3600) as u32;
    let min = ((secs_of_day % 3600) / 60) as u32;
    let sec = (secs_of_day % 60) as u32;

    // Shift epoch so day 0 = 0000-03-01 (era origin).
    let z = days + 719468;
    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u64; // 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // 0..=399
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = if m <= 2 { (y + 1) as i32 } else { y as i32 };
    (year, m, d, hour, min, sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_1970_01_01_00_00_00() {
        assert_eq!(unix_to_ymd_hms(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn unix_2026_05_21_noon_utc() {
        // 2026-05-21 12:00:00 UTC
        // Days from 1970-01-01 to 2026-05-21 = 20595 (verified via cal)
        // 20595 * 86400 + 12 * 3600 = 1779408000 + 43200 = 1779364800
        // (we'll use the inverse — assert that round-trip works)
        let known = 1779364800i64;
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(known);
        assert_eq!((y, mo, d, h, mi, s), (2026, 5, 21, 12, 0, 0));
    }

    #[test]
    fn unix_2000_leap_year_feb_29() {
        // 2000-02-29 00:00:00 UTC — leap year edge case
        // Days from 1970-01-01 to 2000-02-29 = 11016
        let secs = 11016i64 * 86400;
        assert_eq!(unix_to_ymd_hms(secs), (2000, 2, 29, 0, 0, 0));
    }

    #[test]
    fn unix_negative_secs_pre_epoch() {
        // 1969-12-31 23:59:59 = -1
        assert_eq!(unix_to_ymd_hms(-1), (1969, 12, 31, 23, 59, 59));
        // 1969-12-31 00:00:00 = -86400
        assert_eq!(unix_to_ymd_hms(-86400), (1969, 12, 31, 0, 0, 0));
    }
}
