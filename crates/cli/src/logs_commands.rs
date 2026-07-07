use crate::cli::LogsArgs;
use crate::daemon_lifecycle;
use anyhow::{Context as _, Result};
use chrono::{DateTime, Duration, Utc};
use std::io::SeekFrom;
use tokio::io::{AsyncBufReadExt as _, AsyncSeekExt as _, BufReader};

pub async fn run(args: LogsArgs) -> Result<()> {
    let path = daemon_lifecycle::log_path()?;
    if !path.exists() {
        println!("No daemon log yet at {}", path.display());
        return Ok(());
    }

    let cutoff = args.since.as_deref().map(parse_since).transpose()?;

    if args.follow {
        return follow(&path).await;
    }

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading daemon log {}", path.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let selected: Vec<&str> = match cutoff {
        Some(cutoff) => filter_since(&lines, cutoff),
        None => lines,
    };
    let start = selected.len().saturating_sub(args.lines);
    for line in &selected[start..] {
        println!("{line}");
    }
    Ok(())
}

async fn follow(path: &std::path::Path) -> Result<()> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("opening daemon log {}", path.display()))?;
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::End(0)).await?;
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            continue;
        }
        print!("{line}");
    }
}

/// Keeps lines whose leading RFC3339 timestamp is at or after `cutoff`. Lines without a
/// parseable leading timestamp (wrapped continuations of the previous entry) are kept
/// exactly when the entry they continue was kept.
fn filter_since<'a>(lines: &[&'a str], cutoff: DateTime<Utc>) -> Vec<&'a str> {
    let mut keep_previous = false;
    lines
        .iter()
        .filter(|line| match line_timestamp(line) {
            Some(timestamp) => {
                keep_previous = timestamp >= cutoff;
                keep_previous
            }
            None => keep_previous,
        })
        .copied()
        .collect()
}

fn line_timestamp(line: &str) -> Option<DateTime<Utc>> {
    let plain = strip_ansi(line);
    let token = plain.split_whitespace().next()?;
    DateTime::parse_from_rfc3339(token)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // Skip a CSI escape sequence: ESC '[' ... final-byte (0x40-0x7e).
            if chars.next() == Some('[') {
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_since(value: &str) -> Result<DateTime<Utc>> {
    let value = value.trim();
    let split_at = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split_at);
    let amount: i64 = number
        .parse()
        .with_context(|| format!("invalid --since duration '{value}'"))?;
    let duration = match unit {
        "s" | "" => Duration::seconds(amount),
        "m" => Duration::minutes(amount),
        "h" => Duration::hours(amount),
        "d" => Duration::days(amount),
        other => anyhow::bail!("unknown --since unit '{other}' (use s, m, h, or d)"),
    };
    Ok(Utc::now() - duration)
}

#[cfg(test)]
mod tests {
    use super::{filter_since, line_timestamp, parse_since, strip_ansi};
    use chrono::{Duration, Utc};

    #[test]
    fn strip_ansi_removes_color_codes() {
        assert_eq!(
            strip_ansi("\u{1b}[2m2026-07-07T05:51:32.856330Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m hi"),
            "2026-07-07T05:51:32.856330Z  INFO hi"
        );
    }

    #[test]
    fn line_timestamp_parses_rfc3339_prefix() {
        let line = "\u{1b}[2m2026-07-07T05:51:32.856330Z\u{1b}[0m INFO steward_daemon: hello";
        assert!(line_timestamp(line).is_some());
    }

    #[test]
    fn line_timestamp_is_none_for_continuation_lines() {
        assert!(line_timestamp("    at some/path.rs:42").is_none());
    }

    #[test]
    fn parse_since_supports_units() {
        let now = Utc::now();
        assert!(parse_since("30m").unwrap() <= now - Duration::minutes(30) + Duration::seconds(1));
        assert!(parse_since("2h").unwrap() <= now - Duration::hours(2) + Duration::seconds(1));
    }

    #[test]
    fn filter_since_keeps_continuations_of_kept_entries() {
        let cutoff = Utc::now() - Duration::minutes(1);
        let recent = (Utc::now()).to_rfc3339();
        let old = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let old_line = format!("{old} INFO old entry");
        let old_continuation = "    continuation of old";
        let recent_line = format!("{recent} INFO recent entry");
        let lines = vec![old_line.as_str(), old_continuation, recent_line.as_str()];
        let kept = filter_since(&lines, cutoff);
        assert_eq!(kept, vec![recent_line.as_str()]);
    }
}
