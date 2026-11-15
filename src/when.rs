//! Dates and hours. VTTL gives Belgian local time; the server clock is UTC.

/// Days since 1970-01-01 of a calendar date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// (year, month, day) of a number of days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Seconds that Belgium is ahead of UTC at a UTC time. Summer time runs from the last Sunday
/// of March to the last Sunday of October, both at 01:00 UTC.
pub fn offset(utc: i64) -> i64 {
    let (y, _, _) = civil_from_days(utc.div_euclid(86400));
    let last_sunday = |m: i64| {
        let d = days_from_civil(y, m, 31);
        d - (d + 4).rem_euclid(7) // 1970-01-01 was a Thursday
    };
    let summer = last_sunday(3) * 86400 + 3600..last_sunday(10) * 86400 + 3600;
    if summer.contains(&utc) { 7200 } else { 3600 }
}

/// The Belgian date ("2026-11-14") at a UTC time.
pub fn local_day(utc: i64) -> String {
    let (y, m, d) = civil_from_days((utc + offset(utc)).div_euclid(86400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// A Belgian date and hour ("2026-11-14", "19:00") as Unix seconds.
pub fn local_unix(day: &str, time: &str) -> Option<i64> {
    let mut p = day.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, d) = (p.next()??, p.next()??, p.next()??);
    let (h, min) = time.split_once(':')?;
    let (h, min) = (h.parse::<i64>().ok()?, min.get(..2)?.parse::<i64>().ok()?);
    let naive = days_from_civil(y, m, d) * 86400 + h * 3600 + min * 60;
    Some(naive - offset(naive - 3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belgian_time() {
        // Winter: UTC+1. 2026-11-14 19:00 = 18:00 UTC.
        let t = local_unix("2026-11-14", "19:00").unwrap();
        assert_eq!(t, days_from_civil(2026, 11, 14) * 86400 + 18 * 3600);
        assert_eq!(local_day(t), "2026-11-14");
        // Summer: UTC+2.
        let t = local_unix("2026-07-01", "12:00").unwrap();
        assert_eq!(t, days_from_civil(2026, 7, 1) * 86400 + 10 * 3600);
        // The change: 2026-03-29 and 2026-10-25, at 01:00 UTC.
        let march = days_from_civil(2026, 3, 29) * 86400 + 3600;
        assert_eq!((offset(march - 1), offset(march)), (3600, 7200));
        let october = days_from_civil(2026, 10, 25) * 86400 + 3600;
        assert_eq!((offset(october - 1), offset(october)), (7200, 3600));
        // 23:30 UTC is already the next day in Belgium.
        assert_eq!(local_day(days_from_civil(2026, 11, 14) * 86400 + 23 * 3600 + 1800), "2026-11-15");
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        assert!(local_unix("", "19:00").is_none());
        assert!(local_unix("2026-11-14", "").is_none());
    }
}
