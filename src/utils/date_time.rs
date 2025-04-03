use chrono::DateTime;
use humantime::format_duration;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WEEK_IN_SECONDS: u64 = 60 * 60 * 24 * 7;

pub(crate) fn format_timestamp(timestamp: u64) -> String {
    let timestamp = UNIX_EPOCH + Duration::from_secs(timestamp);
    let now = SystemTime::now();
    let diff = if timestamp > now {
        timestamp.duration_since(now).unwrap()
    } else {
        now.duration_since(timestamp).unwrap()
    };

    let timestamp = if diff.as_secs() > WEEK_IN_SECONDS {
        let date_time = DateTime::from_timestamp(
            timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            0,
        )
        .unwrap();

        date_time.format("%v").to_string()
    } else {
        let diff = Duration::new(diff.as_secs(), 0);
        let human_diff = format_duration(diff);

        if timestamp > now {
            format!("in {human_diff}")
        } else {
            format!("{human_diff} ago")
        }
    };

    timestamp
}
