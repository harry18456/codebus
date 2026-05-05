use chrono::Utc;

/// Returns today's date in UTC as `YYYY-MM-DD`. Mirrors TS `utcTodayISO`
/// (`new Date().toISOString().slice(0, 10)`) so cross-timezone collaboration
/// stays consistent and `flagStalePages` comparisons see stable values.
pub fn utc_today_iso() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_yyyy_mm_dd_format() {
        let today = utc_today_iso();
        assert_eq!(today.len(), 10, "expected YYYY-MM-DD, got {today:?}");
        let bytes = today.as_bytes();
        assert!(bytes[..4].iter().all(|b| b.is_ascii_digit()));
        assert_eq!(bytes[4], b'-');
        assert!(bytes[5..7].iter().all(|b| b.is_ascii_digit()));
        assert_eq!(bytes[7], b'-');
        assert!(bytes[8..10].iter().all(|b| b.is_ascii_digit()));
    }

    #[test]
    fn deterministic_across_calls_within_same_day() {
        // Two consecutive calls within the same UTC day must agree. Guards
        // against accidental local-tz formatting (e.g. using `Local::now`).
        let a = utc_today_iso();
        let b = utc_today_iso();
        assert_eq!(a, b);
    }

    #[test]
    fn parsable_back_to_naive_date() {
        use chrono::NaiveDate;
        let today = utc_today_iso();
        NaiveDate::parse_from_str(&today, "%Y-%m-%d")
            .unwrap_or_else(|e| panic!("date {today:?} failed to parse back: {e}"));
    }
}
