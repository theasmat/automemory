use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSpanDigest {
    pub project_id: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_minutes: f64,
    pub active_agents: Vec<String>,
    pub metrics: ActivityMetrics,
    pub achievements: Vec<String>,
    pub failures: Vec<TimeSpanFailureSummary>,
    pub files_modified: Vec<String>,
    pub files_inspected: Vec<String>,
    pub executive_summary: String,
    pub key_commands_executed: Vec<String>,
}

impl TimeSpanDigest {
    pub fn empty(project_id: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            project_id,
            start_time: now,
            end_time: now,
            duration_minutes: 0.0,
            active_agents: Vec::new(),
            metrics: ActivityMetrics::default(),
            achievements: Vec::new(),
            failures: Vec::new(),
            files_modified: Vec::new(),
            files_inspected: Vec::new(),
            executive_summary: String::new(),
            key_commands_executed: Vec::new(),
        }
    }
}

impl Default for TimeSpanDigest {
    fn default() -> Self {
        Self::empty(None)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivityMetrics {
    pub total_tool_calls: usize,
    pub edits_count: usize,
    pub reads_count: usize,
    pub commands_count: usize,
    pub tests_run: usize,
    pub tests_passed: usize,
    pub tests_failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeSpanFailureSummary {
    pub tool_or_command: String,
    pub error_snippet: String,
    pub is_resolved: bool,
    pub fix_applied: Option<String>,
    pub event_id: String,
}

/// Helper to parse human-readable time span strings into (start_time, end_time).
/// Supports minutes (m), hours (h), days (d), weeks (w), and 'today' (UTC midnight).
pub fn parse_time_span_str(s: &str) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let now = Utc::now();
    let trimmed = s.trim().to_lowercase();

    if trimmed == "today" {
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc())
            .ok_or_else(|| anyhow!("Failed to compute start of today"))?;
        return Ok((today_start, now));
    }

    let split_idx = trimmed
        .find(|c: char| c.is_alphabetic())
        .ok_or_else(|| anyhow!("Missing time unit in '{s}'. Supported: m, h, d, w, today"))?;

    let (val_str, unit) = trimmed.split_at(split_idx);
    let val: i64 = val_str
        .trim()
        .parse()
        .map_err(|_| anyhow!("Invalid time span number in '{s}'"))?;

    let duration = match unit.trim() {
        "m" | "min" | "mins" | "minute" | "minutes" => Duration::minutes(val),
        "h" | "hr" | "hrs" | "hour" | "hours" => Duration::hours(val),
        "d" | "day" | "days" => Duration::days(val),
        "w" | "week" | "weeks" => Duration::weeks(val),
        _ => return Err(anyhow!("Unknown time span unit '{unit}' in '{s}'. Use: m, h, d, w, today")),
    };

    Ok((now - duration, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_span_units() {
        let (s, e) = parse_time_span_str("30m").expect("parse 30m");
        assert_eq!((e - s).num_minutes(), 30);

        let (s, e) = parse_time_span_str("2h").expect("parse 2h");
        assert_eq!((e - s).num_hours(), 2);

        let (s, e) = parse_time_span_str("1d").expect("parse 1d");
        assert_eq!((e - s).num_days(), 1);

        let (s, e) = parse_time_span_str("2 weeks").expect("parse 2 weeks");
        assert_eq!((e - s).num_weeks(), 2);
    }

    #[test]
    fn test_parse_time_span_today() {
        let (s, e) = parse_time_span_str("today").expect("parse today");
        assert!(s <= e);
    }

    #[test]
    fn test_parse_time_span_invalid() {
        assert!(parse_time_span_str("invalid").is_err());
        assert!(parse_time_span_str("42").is_err());
    }
}
