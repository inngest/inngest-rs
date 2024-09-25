use std::time::Duration;

const MILLISECOND_IN_NANO: u128 = 1_000_000;
const SECOND_IN_NANO: u128 = 1000 * MILLISECOND_IN_NANO;
const MINUTE_IN_NANO: u128 = 60 * SECOND_IN_NANO;
const HOUR_IN_NANO: u128 = 60 * MINUTE_IN_NANO;
const DAY_IN_NANO: u128 = 24 * HOUR_IN_NANO;

pub(crate) fn to_string(dur: Duration) -> String {
    dur_str("", dur)
}

fn dur_str(acc: &str, dur: Duration) -> String {
    let ns = dur.as_nanos();

    if ns / DAY_IN_NANO > 0 {
        let day = ns / DAY_IN_NANO;
        let day_str = format!("{}d", day);

        let remain = ns % DAY_IN_NANO;
        let next = Duration::new(
            (remain / SECOND_IN_NANO) as u64,
            (remain % SECOND_IN_NANO) as u32,
        );
        return format!("{}{}", acc, dur_str(&day_str, next));
    }

    if ns / HOUR_IN_NANO > 0 {
        let hour = ns / HOUR_IN_NANO;
        let hour_str = format!("{}h", hour);

        let remain = ns % HOUR_IN_NANO;
        let next = Duration::new(
            (remain / SECOND_IN_NANO) as u64,
            (remain % SECOND_IN_NANO) as u32,
        );
        return format!("{}{}", acc, dur_str(&hour_str, next));
    }

    if ns / MINUTE_IN_NANO > 0 {
        let min = ns / MINUTE_IN_NANO;
        let min_str = format!("{}m", min);

        let remain = ns % MINUTE_IN_NANO;
        let next = Duration::new(
            (remain / SECOND_IN_NANO) as u64,
            (remain % SECOND_IN_NANO) as u32,
        );
        return format!("{}{}", acc, dur_str(&min_str, next));
    }

    if ns / SECOND_IN_NANO > 0 {
        let sec = ns / SECOND_IN_NANO;
        let sec_str = format!("{}s", sec);
        let next = Duration::new(0, (ns % SECOND_IN_NANO) as u32);
        return format!("{}{}", acc, dur_str(&sec_str, next));
    }

    if ns / MILLISECOND_IN_NANO > 0 {
        let ms = ns / MILLISECOND_IN_NANO;
        let ms_str = format!("{}ms", ms);
        let next = Duration::new(0, (ns % MILLISECOND_IN_NANO) as u32);
        return format!("{}{}", acc, dur_str(&ms_str, next));
    }

    if ns > 0 {
        format!("{}{}ns", acc, ns)
    } else {
        acc.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_string_nanosecond() {
        let d = to_string(Duration::from_nanos(100));
        assert_eq!("100ns", d);
    }

    #[test]
    fn to_string_millisecond() {
        let d = to_string(Duration::from_millis(10));
        assert_eq!("10ms", d);
    }

    #[test]
    fn to_string_millisecond_ns() {
        let ms = Duration::from_millis(2);
        let ns = Duration::from_nanos(100);
        let d = to_string(ms + ns);

        assert_eq!("2ms100ns", d)
    }

    #[test]
    fn to_string_second() {
        let d = to_string(Duration::from_secs(45));
        assert_eq!("45s", d);
    }

    #[test]
    fn to_string_second_ns() {
        let sec = Duration::from_secs(45);
        let ns = Duration::from_nanos(10);
        let d = to_string(sec + ns);
        assert_eq!("45s10ns", d);
    }

    #[test]
    fn to_string_minute() {
        let min = 3;
        let d = to_string(Duration::from_secs(min * 60));
        assert_eq!("3m", d);
    }

    #[test]
    fn to_string_minute_ms() {
        let min = Duration::from_secs(3 * 60);
        let ms = Duration::from_millis(100);
        let d = to_string(min + ms);
        assert_eq!("3m100ms", d);
    }

    #[test]
    fn to_string_hour() {
        let hour_in_sec = 60 * 60;
        let hour = Duration::from_secs(2 * hour_in_sec);
        let d = to_string(hour);
        assert_eq!("2h", d);
    }

    #[test]
    fn to_string_hour_m() {
        let hour_in_sec = 60 * 60;
        let hour = Duration::from_secs(2 * hour_in_sec);
        let min = Duration::from_secs(30 * 60);
        let d = to_string(hour + min);
        assert_eq!("2h30m", d);
    }

    #[test]
    fn to_string_day() {
        let day_in_sec = 60 * 60 * 24;
        let day = Duration::from_secs(7 * day_in_sec);
        let d = to_string(day);
        assert_eq!("7d", d);
    }

    #[test]
    fn to_string_day_s() {
        let day_in_sec = 60 * 60 * 24;
        let day = Duration::from_secs(7 * day_in_sec);
        let d = to_string(day + Duration::from_secs(20));
        assert_eq!("7d20s", d);
    }
}
