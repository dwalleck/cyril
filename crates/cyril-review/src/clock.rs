use crate::{Result, ReviewError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Wall-clock timestamps and elapsed diagnostics report durations.
pub trait ReviewClock {
    fn gathered_at_utc(&self) -> Result<String>;
    fn diagnostics_elapsed(&self, started: Instant) -> Duration;
}

/// Production clock backed by host wall and monotonic clocks.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemReviewClock;

impl ReviewClock for SystemReviewClock {
    fn gathered_at_utc(&self) -> Result<String> {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(UNIX_EPOCH)
            .map_err(|source| ReviewError::Clock {
                message: format!("system time precedes Unix epoch: {source}"),
            })?;
        format_utc(elapsed)
    }

    fn diagnostics_elapsed(&self, started: Instant) -> Duration {
        started.elapsed()
    }
}

fn format_utc(elapsed: Duration) -> Result<String> {
    let seconds = elapsed.as_secs();
    let days = seconds / 86_400;
    let day_seconds = seconds % 86_400;
    let days = i64::try_from(days).map_err(|_| ReviewError::Clock {
        message: "UTC day count is outside the supported range".to_owned(),
    })?;
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}+00:00"
    ))
}

// Howard Hinnant's proleptic Gregorian conversion, expressed without an
// external date/time dependency.
fn civil_from_days(days_since_epoch: i64) -> (i64, u64, u64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = doy - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month as u64, day as u64)
}

#[cfg(test)]
mod tests {
    use super::{Duration, Result, format_utc};

    #[test]
    fn utc_seconds_preserve_leap_and_century_boundaries() -> Result<()> {
        // Independent Python datetime UTC samples, not the conversion formula.
        for (seconds, expected) in [
            (0, "1970-01-01T00:00:00+00:00"),
            (951_868_799, "2000-02-29T23:59:59+00:00"),
            (951_868_800, "2000-03-01T00:00:00+00:00"),
            (4_107_542_400, "2100-03-01T00:00:00+00:00"),
        ] {
            assert_eq!(format_utc(Duration::from_secs(seconds))?, expected);
        }
        assert_eq!(
            format_utc(Duration::new(951_868_799, 999_999_999))?,
            "2000-02-29T23:59:59+00:00",
        );
        Ok(())
    }
}
