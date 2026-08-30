use chrono::{DateTime, Utc};

pub fn format_elapsed(started_at: DateTime<Utc>, ended_at: Option<DateTime<Utc>>) -> String {
    let end = ended_at.unwrap_or_else(Utc::now);
    let seconds = (end - started_at).num_seconds().max(0);
    format_elapsed_seconds(seconds)
}

pub fn format_elapsed_seconds(seconds: i64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    match (hours, minutes, secs) {
        (0, 0, s) => format!("⏱ {s}s"),
        (0, m, s) => format!("⏱ {m}m {s}s"),
        (h, m, s) => format!("⏱ {h}h {m}m {s}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::format_elapsed_seconds;

    #[test]
    fn formats_short_elapsed_time() {
        assert_eq!(format_elapsed_seconds(9), "⏱ 9s");
        assert_eq!(format_elapsed_seconds(63), "⏱ 1m 3s");
        assert_eq!(format_elapsed_seconds(3723), "⏱ 1h 2m 3s");
    }
}
