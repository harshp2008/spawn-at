//! Title redaction, privacy filtering, and timestamp formatting utilities.

use std::fs;
use std::time::SystemTime;

/// Retrieves the real system hostname without falling back to "localhost".
pub fn get_real_hostname() -> String {
    // 1. Check kernel hostname
    if let Ok(name) = fs::read_to_string("/proc/sys/kernel/hostname") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    // 2. Check /etc/hostname
    if let Ok(name) = fs::read_to_string("/etc/hostname") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    // 3. Check hostname command
    if let Ok(output) = std::process::Command::new("hostname").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    // 4. Environment variables
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "localhost".to_string())
}

/// Formats a SystemTime into a human-readable, sortable `YYYYMMDD-HHMMSS` timestamp.
pub fn format_sortable_timestamp(now: SystemTime) -> String {
    let secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_epoch_secs_sortable(secs)
}

/// Formats unix epoch seconds into a human-readable, sortable `YYYYMMDD-HHMMSS` timestamp.
pub fn format_epoch_secs_sortable(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem_secs = (secs % 86400) as u32;
    let hour = rem_secs / 3600;
    let min = (rem_secs % 3600) / 60;
    let sec = rem_secs % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", y, m, d, hour, min, sec)
}

/// Redacts window titles from text unless `include_titles` is true.
///
/// Ensures that ONLY window titles are redacted and NEVER other command arguments,
/// options, flags, or values (e.g. `--class`, `--anchor`, `--pos`, `--size`, `--margin`,
/// or flags containing "title" like `--title-format`).
pub fn redact_titles(text: &str, include_titles: bool) -> String {
    if include_titles {
        return text.to_string();
    }

    // 1. Redact CLI flags: `--title <VAL>` or `-t <VAL>`
    let cli_re = regex::Regex::new(
        r#"(?x)
        (^|[\s=])                  # Starts at line start, whitespace, or equals
        (--title|-t)               # Exact flag
        \s+                        # Followed by whitespace
        (?:
            "([^"\\]*(?:\\.[^"\\]*)*)"   # Double quoted value
          | '([^'\\]*(?:\\.[^'\\]*)*)'   # Single quoted value
          | (\S+)                        # Non-whitespace token
        )
    "#,
    )
    .unwrap();

    let text_after_cli = cli_re.replace_all(text, |caps: &regex::Captures| {
        let prefix = &caps[1];
        let flag = &caps[2];
        format!("{}{} \"[REDACTED]\"", prefix, flag)
    });

    // 2. Redact JSON field: "title": "..."
    let json_re = regex::Regex::new(r#"(?i)("title"\s*:\s*)"(?:[^"\\]|\\.)*""#).unwrap();
    let text_after_json = json_re.replace_all(&text_after_cli, r#"$1"[REDACTED]""#);

    // 3. Redact explicit label in text: title: "..." or title = "..."
    let label_re = regex::Regex::new(
        r#"(?i)\b(title\s*[:=]\s*)(?:"[^"\\]*(?:\\.[^"\\]*)*"|'[^'\\]*(?:\\.[^'\\]*)*')(?:\s|$|,)"#,
    )
    .unwrap();
    let text_after_label = label_re.replace_all(&text_after_json, |caps: &regex::Captures| {
        if caps[0].contains("[REDACTED]") {
            caps[0].to_string()
        } else {
            format!("{}[REDACTED] ", &caps[1])
        }
    });

    text_after_label.to_string()
}
