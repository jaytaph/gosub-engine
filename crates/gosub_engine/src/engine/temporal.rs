//! The date and time input types: parsing, serialising, and the numbers behind them.
//!
//! Each of `date`, `month`, `week`, `time` and `datetime-local` has its own string format
//! and its own idea of what `valueAsNumber` means - months since 1970-01 for one,
//! milliseconds since midnight for another. Everything those types need to be sanitized,
//! stepped and range-checked comes from here.

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};

/// Milliseconds in a day, the scale most of these formats count in.
const MS_PER_DAY: f64 = 86_400_000.0;
const MS_PER_WEEK: f64 = MS_PER_DAY * 7.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Date,
    Month,
    Week,
    Time,
    DateTimeLocal,
}

impl Kind {
    /// The temporal kind of an `<input>` type keyword, if it is one.
    pub fn of(input_type: &str) -> Option<Self> {
        match input_type {
            "date" => Some(Kind::Date),
            "month" => Some(Kind::Month),
            "week" => Some(Kind::Week),
            "time" => Some(Kind::Time),
            "datetime-local" => Some(Kind::DateTimeLocal),
            _ => None,
        }
    }

    /// The step a control of this kind moves by when it has no `step` attribute: a day, a
    /// month, a week - but a *minute* for the two that carry a time, not a second.
    pub fn default_step(&self) -> f64 {
        match self {
            Kind::Date => MS_PER_DAY,
            Kind::Month => 1.0,
            Kind::Week => MS_PER_WEEK,
            Kind::Time | Kind::DateTimeLocal => 60_000.0,
        }
    }

    /// Where the step grid starts when there is no `min`. Zero for everything except a
    /// week: 1970-01-01 was a Thursday, so week steps count from the Monday before it.
    pub fn default_step_base(&self) -> f64 {
        match self {
            Kind::Week => -259_200_000.0,
            _ => 0.0,
        }
    }

    /// What one unit of the `step` attribute is worth. `step="2"` is two days on a date,
    /// two seconds on a time, two weeks on a week.
    pub fn step_scale(&self) -> f64 {
        match self {
            Kind::Date => MS_PER_DAY,
            Kind::Month => 1.0,
            Kind::Week => MS_PER_WEEK,
            Kind::Time | Kind::DateTimeLocal => 1000.0,
        }
    }
}

/// Parse a value of `kind` into its number, or `None` when the string is not a valid one.
pub fn parse(kind: Kind, value: &str) -> Option<f64> {
    match kind {
        Kind::Date => parse_date(value).map(date_to_number),
        Kind::Month => parse_month(value).map(|(year, month)| month_to_number(year, month)),
        Kind::Week => parse_week(value).map(date_to_number),
        Kind::Time => parse_time(value).map(time_to_number),
        Kind::DateTimeLocal => {
            let (date, time) = split_datetime(value)?;
            Some(date_to_number(parse_date(date)?) + time_to_number(parse_time(time)?))
        }
    }
}

/// Turn a number back into a value string, or `None` when it is out of range.
pub fn serialize(kind: Kind, number: f64) -> Option<String> {
    match kind {
        Kind::Date => Some(format_date(date_from_number(number)?)),
        Kind::Month => {
            let (year, month) = month_from_number(number)?;
            Some(format!("{year:04}-{month:02}"))
        }
        Kind::Week => {
            let date = date_from_number(number)?;
            let iso = date.iso_week();
            Some(format!("{:04}-W{:02}", iso.year(), iso.week()))
        }
        // A time wraps into its day: the spec's setter takes any number of milliseconds.
        Kind::Time => Some(format_time(number.rem_euclid(MS_PER_DAY))),
        Kind::DateTimeLocal => {
            let date = date_from_number(number)?;
            let within_day = number - date_to_number(date);
            Some(format!("{}T{}", format_date(date), format_time(within_day)))
        }
    }
}

/// The instant `valueAsDate` reports, in milliseconds since the epoch.
///
/// Not the same as the kind's own number: a month counts months, but its *date* is the first
/// day of that month. A `datetime-local` has no instant at all - it names no moment in time.
pub fn to_instant(kind: Kind, number: f64) -> Option<f64> {
    match kind {
        Kind::Date | Kind::Week | Kind::Time => Some(number),
        Kind::Month => {
            let (year, month) = month_from_number(number)?;
            Some(date_to_number(NaiveDate::from_ymd_opt(year, month, 1)?))
        }
        Kind::DateTimeLocal => None,
    }
}

/// The reverse: an instant back into whatever number the kind counts in.
pub fn from_instant(kind: Kind, instant: f64) -> Option<f64> {
    match kind {
        Kind::Date | Kind::Week | Kind::Time => Some(instant),
        Kind::Month => {
            let date = date_from_number(instant)?;
            Some(month_to_number(date.year(), date.month()))
        }
        Kind::DateTimeLocal => None,
    }
}

/// Whether `value` is a valid string of that kind.
pub fn is_valid(kind: Kind, value: &str) -> bool {
    parse(kind, value).is_some()
}

// ── parsing ───────────────────────────────────────────────────────────────────

/// `yyyy-mm-dd`, with at least four year digits and no year zero.
fn parse_date(value: &str) -> Option<NaiveDate> {
    let (year_text, rest) = split_year(value)?;
    let (month, rest) = take_two_digits(rest.strip_prefix('-')?)?;
    let (day, rest) = take_two_digits(rest.strip_prefix('-')?)?;
    if !rest.is_empty() {
        return None;
    }
    NaiveDate::from_ymd_opt(year_text, month as u32, day as u32)
}

/// `yyyy-mm`.
fn parse_month(value: &str) -> Option<(i32, u32)> {
    let (year, rest) = split_year(value)?;
    let (month, rest) = take_two_digits(rest.strip_prefix('-')?)?;
    if !rest.is_empty() || !(1..=12).contains(&month) {
        return None;
    }
    Some((year, month as u32))
}

/// `yyyy-Www`, returning the Monday that week starts on.
fn parse_week(value: &str) -> Option<NaiveDate> {
    let (year, rest) = split_year(value)?;
    let (week, rest) = take_two_digits(rest.strip_prefix("-W")?)?;
    if !rest.is_empty() || week < 1 {
        return None;
    }
    NaiveDate::from_isoywd_opt(year, week as u32, chrono::Weekday::Mon)
}

/// `HH:MM`, `HH:MM:SS` or `HH:MM:SS.fff`.
fn parse_time(value: &str) -> Option<NaiveTime> {
    let (hour, rest) = take_two_digits(value)?;
    let (minute, rest) = take_two_digits(rest.strip_prefix(':')?)?;
    if hour > 23 || minute > 59 {
        return None;
    }
    if rest.is_empty() {
        return NaiveTime::from_hms_opt(hour as u32, minute as u32, 0);
    }
    let rest = rest.strip_prefix(':')?;
    let (second, rest) = take_two_digits(rest)?;
    if second > 59 {
        return None;
    }
    let millis = match rest.strip_prefix('.') {
        None if rest.is_empty() => 0,
        None => return None,
        Some(fraction) => parse_fraction(fraction)?,
    };
    NaiveTime::from_hms_milli_opt(hour as u32, minute as u32, second as u32, millis)
}

/// A `datetime-local` joins its halves with `T` or a single space.
fn split_datetime(value: &str) -> Option<(&str, &str)> {
    value.split_once('T').or_else(|| value.split_once(' '))
}

/// The year: four or more digits, and never zero.
fn split_year(value: &str) -> Option<(i32, &str)> {
    let digits = value.chars().take_while(char::is_ascii_digit).count();
    if digits < 4 {
        return None;
    }
    let year: i32 = value[..digits].parse().ok()?;
    if year < 1 {
        return None;
    }
    Some((year, &value[digits..]))
}

fn take_two_digits(value: &str) -> Option<(i64, &str)> {
    let head = value.get(..2)?;
    if !head.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((head.parse().ok()?, &value[2..]))
}

/// One to three fraction digits, scaled to milliseconds.
fn parse_fraction(fraction: &str) -> Option<u32> {
    if fraction.is_empty() || fraction.len() > 3 || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value: u32 = fraction.parse().ok()?;
    Some(value * 10u32.pow(3 - fraction.len() as u32))
}

// ── numbers ───────────────────────────────────────────────────────────────────

/// 1970-01-01 counted the way chrono counts days, so the epoch needs no fallible lookup.
/// `epoch_is_where_we_think_it_is` keeps this honest.
const EPOCH_DAYS_FROM_CE: i32 = 719_163;

fn date_to_number(date: NaiveDate) -> f64 {
    (date.num_days_from_ce() - EPOCH_DAYS_FROM_CE) as f64 * MS_PER_DAY
}

fn date_from_number(number: f64) -> Option<NaiveDate> {
    let days = (number / MS_PER_DAY).floor();
    if !days.is_finite() || days.abs() > 4_000_000.0 {
        return None;
    }
    NaiveDate::from_num_days_from_ce_opt(days as i32 + EPOCH_DAYS_FROM_CE)
}

fn month_to_number(year: i32, month: u32) -> f64 {
    ((year - 1970) * 12 + month as i32 - 1) as f64
}

fn month_from_number(number: f64) -> Option<(i32, u32)> {
    if !number.is_finite() || number.abs() > 1_000_000.0 {
        return None;
    }
    let months = number.floor() as i64;
    let year = 1970 + months.div_euclid(12);
    let month = months.rem_euclid(12) + 1;
    (year >= 1).then_some((year as i32, month as u32))
}

fn time_to_number(time: NaiveTime) -> f64 {
    let seconds = time.num_seconds_from_midnight() as f64;
    seconds * 1000.0 + (time.nanosecond() / 1_000_000) as f64
}

// ── serialising ───────────────────────────────────────────────────────────────

fn format_date(date: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

/// `HH:MM`, growing seconds and milliseconds only when they carry something.
fn format_time(ms_within_day: f64) -> String {
    let total = ms_within_day.max(0.0).round() as i64;
    let (millis, total_seconds) = (total % 1000, total / 1000);
    let (seconds, total_minutes) = (total_seconds % 60, total_seconds / 60);
    let (minutes, hours) = (total_minutes % 60, total_minutes / 60);
    if millis != 0 {
        return format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}");
    }
    if seconds != 0 {
        return format!("{hours:02}:{minutes:02}:{seconds:02}");
    }
    format!("{hours:02}:{minutes:02}")
}

/// A `datetime-local` needs the whole thing back, including a midnight time.
#[allow(dead_code)]
fn format_datetime(when: NaiveDateTime) -> String {
    format!(
        "{}T{}",
        format_date(when.date()),
        format_time(time_to_number(when.time()))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_months_instant_is_the_first_of_that_month() {
        let december = parse(Kind::Month, "2019-12").expect("2019-12 parses");
        let instant = to_instant(Kind::Month, december).expect("a month has an instant");
        assert_eq!(serialize(Kind::Date, instant).as_deref(), Some("2019-12-01"));
        assert_eq!(from_instant(Kind::Month, instant), Some(december));
        // A local datetime names no moment, so it has no date at all.
        assert_eq!(to_instant(Kind::DateTimeLocal, 0.0), None);
    }

    #[test]
    fn epoch_is_where_we_think_it_is() {
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("1970-01-01 is a date");
        assert_eq!(epoch.num_days_from_ce(), EPOCH_DAYS_FROM_CE);
    }

    /// The vectors come straight from wpt's input-valueasnumber.html.
    #[test]
    fn dates_round_trip_and_reject_impossible_ones() {
        assert_eq!(parse(Kind::Date, "2019-12-10"), Some(1_575_936_000_000.0));
        assert_eq!(parse(Kind::Date, "2016-02-29"), Some(1_456_704_000_000.0));
        for bad in ["", "0000-12-10", "2019-00-12", "2019-12-00", "2019-13-10", "2019-02-29"] {
            assert_eq!(parse(Kind::Date, bad), None, "{bad} should not parse");
        }
        assert_eq!(serialize(Kind::Date, 0.0).as_deref(), Some("1970-01-01"));
        assert_eq!(
            serialize(Kind::Date, 1_575_936_000_000.0).as_deref(),
            Some("2019-12-10")
        );
        assert_eq!(
            serialize(Kind::Date, 1_456_704_000_000.0).as_deref(),
            Some("2016-02-29")
        );
    }

    #[test]
    fn months_count_from_1970() {
        assert_eq!(parse(Kind::Month, "2019-12"), Some(599.0));
        assert_eq!(parse(Kind::Month, "1969-12"), Some(-1.0));
        assert_eq!(parse(Kind::Month, "0000-12"), None);
        assert_eq!(parse(Kind::Month, "2019-00"), None);
        assert_eq!(serialize(Kind::Month, 599.0).as_deref(), Some("2019-12"));
        assert_eq!(serialize(Kind::Month, -1.0).as_deref(), Some("1969-12"));
    }

    #[test]
    fn weeks_are_the_monday_they_start_on() {
        assert_eq!(parse(Kind::Week, "2019-W50"), Some(1_575_849_600_000.0));
        assert_eq!(parse(Kind::Week, "1969-W20"), Some(-20_217_600_000.0));
        for bad in ["", "0000-W50", "2019-W00", "2019-W60"] {
            assert_eq!(parse(Kind::Week, bad), None, "{bad} should not parse");
        }
        assert_eq!(serialize(Kind::Week, 0.0).as_deref(), Some("1970-W01"));
        assert_eq!(serialize(Kind::Week, 1_575_849_600_000.0).as_deref(), Some("2019-W50"));
        assert_eq!(serialize(Kind::Week, -20_217_600_000.0).as_deref(), Some("1969-W20"));
    }

    #[test]
    fn times_are_milliseconds_into_the_day_and_wrap_on_the_way_back() {
        assert_eq!(parse(Kind::Time, "00:00"), Some(0.0));
        assert_eq!(parse(Kind::Time, "12:00"), Some(43_200_000.0));
        assert_eq!(parse(Kind::Time, "23:59"), Some(86_340_000.0));
        for bad in ["", "24:00", "00:60"] {
            assert_eq!(parse(Kind::Time, bad), None, "{bad} should not parse");
        }
        assert_eq!(serialize(Kind::Time, 0.0).as_deref(), Some("00:00"));
        assert_eq!(serialize(Kind::Time, 43_200_000.0).as_deref(), Some("12:00"));
        // Any number of milliseconds lands somewhere inside a day, forwards or backwards.
        assert_eq!(serialize(Kind::Time, -3_600_000.0).as_deref(), Some("23:00"));
        assert_eq!(
            serialize(Kind::Time, 2.734_333_707_189_448e26).as_deref(),
            Some("10:54:10.944")
        );
    }

    #[test]
    fn a_local_datetime_is_a_date_plus_a_time() {
        assert_eq!(
            parse(Kind::DateTimeLocal, "2019-12-10T00:00"),
            Some(1_575_936_000_000.0)
        );
        assert_eq!(
            parse(Kind::DateTimeLocal, "2019-12-10T12:00"),
            Some(1_575_979_200_000.0)
        );
        assert_eq!(parse(Kind::DateTimeLocal, ""), None);
        assert_eq!(
            serialize(Kind::DateTimeLocal, 1_575_979_200_000.0).as_deref(),
            Some("2019-12-10T12:00")
        );
        assert_eq!(
            serialize(Kind::DateTimeLocal, -86_400_000.0).as_deref(),
            Some("1969-12-31T00:00")
        );
        // Unlike a time, a datetime that lands outside any representable date has no value.
        assert_eq!(serialize(Kind::DateTimeLocal, 2.734_333_707_189_448e26), None);
    }

    #[test]
    fn seconds_and_fractions_only_appear_when_they_carry_something() {
        assert_eq!(parse(Kind::Time, "12:00:30"), Some(43_230_000.0));
        assert_eq!(parse(Kind::Time, "12:00:30.5"), Some(43_230_500.0));
        assert_eq!(parse(Kind::Time, "12:00:30.25"), Some(43_230_250.0));
        assert_eq!(parse(Kind::Time, "12:00:30.125"), Some(43_230_125.0));
        assert_eq!(parse(Kind::Time, "12:00:30.1255"), None);
        assert_eq!(serialize(Kind::Time, 43_230_000.0).as_deref(), Some("12:00:30"));
        assert_eq!(serialize(Kind::Time, 43_230_125.0).as_deref(), Some("12:00:30.125"));
    }
}
