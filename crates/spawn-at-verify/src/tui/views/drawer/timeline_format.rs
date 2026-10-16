//! Formatting, parsing, and key humanisation for drawer timeline events.

use crate::runner::TestResult;
use crate::tui::views::wrapping::normalise_dimensions;
use unicode_width::UnicodeWidthStr;

/// Splits snake_case, kebab-case, or camelCase identifier strings into space-separated lowercase words.
fn split_identifier_to_words(s: &str) -> String {
    let mut words = Vec::new();
    let mut current_word = String::new();
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            if !current_word.is_empty() {
                words.push(current_word.to_lowercase());
                current_word = String::new();
            }
        } else if ch.is_uppercase() {
            if !current_word.is_empty() {
                words.push(current_word.to_lowercase());
                current_word = String::new();
            }
            current_word.push(ch);
        } else {
            current_word.push(ch);
        }
    }
    if !current_word.is_empty() {
        words.push(current_word.to_lowercase());
    }
    let res = words.join(" ");
    if res.width() > 16 {
        res.chars().take(16).collect()
    } else {
        res
    }
}

/// Converts internal trace event property keys into clean label strings.
pub fn humanise_key(key: &str) -> String {
    match key {
        "requestedSize" | "requested" => "requested".into(),
        "clampedSafeSize" | "clamped" => "clamped safe".into(),
        "reportedMinSize" | "reported" => "reported min".into(),
        "preFrame" | "pre-frame" | "pre_frame" => "pre-frame".into(),
        "postFrame" | "post-frame" | "post_frame" => "post-frame".into(),
        "bufferRect" | "buffer" | "buffer_rect" => "buffer rect".into(),
        "instructionsCount" | "instructions" => "instructions".into(),
        "actorFound" => "actor found".into(),
        "inst" | "instruction" => "instruction".into(),
        "wantWake" => "want wake".into(),
        "isVte" => "is VTE".into(),
        "needsReanchor" => "needs re-anchor".into(),
        "usingFrame" => "using frame".into(),
        "eff" => "effective size".into(),
        "finalFrame" | "final" => "final frame".into(),
        "target" => "target".into(),
        "safe" => "safe".into(),
        "reason" => "reason".into(),
        "actor" => "actor".into(),
        "delta" => "delta".into(),
        "opacity" => "opacity".into(),
        "frame" => "frame".into(),
        "bounds" => "bounds".into(),
        "delay" => "delay".into(),
        "positioned" => "positioned".into(),
        "actual" => "actual".into(),
        "visible" => "visible".into(),
        "pos" => "pos".into(),
        "size" => "size".into(),
        "parent" => "parent".into(),
        "windowId" => "window id".into(),
        "source" => "source".into(),
        "last" => "last".into(),
        other => split_identifier_to_words(other),
    }
}

/// Normalizes trace event attribute values for compact display.
pub fn clean_trace_val(val: &str) -> String {
    let trimmed = val.trim().trim_matches('"');
    // A bare number with a long fraction (`2.5540000200271606`) is shown to one decimal.
    if let Some((_, frac)) = trimmed.split_once('.') {
        if frac.len() > 1 && frac.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(f) = trimmed.parse::<f64>() {
                return format!("{f:.1}");
            }
        }
    }
    let unwrapped = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let parts: Vec<&str> = unwrapped.split(',').collect();
    let formatted = if parts.len() == 3 && parts[2].contains('x') {
        format!(
            "{},{} {}",
            parts[0].trim(),
            parts[1].trim(),
            parts[2].trim()
        )
    } else if parts.len() == 4 && !parts[2].contains('x') && !parts[3].contains('x') {
        format!(
            "{},{} {}×{}",
            parts[0].trim(),
            parts[1].trim(),
            parts[2].trim(),
            parts[3].trim()
        )
    } else if parts.len() == 2 && !parts[0].contains('x') && !parts[1].contains('x') {
        format!("{}, {}", parts[0].trim(), parts[1].trim())
    } else {
        unwrapped.to_string()
    };

    let with_mult = normalise_dimensions(&formatted);

    if with_mult
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_')
        && with_mult.len() > 1
    {
        with_mult.replace('_', " ").to_lowercase()
    } else {
        with_mult
    }
}

/// Parses key-value pairs from comma-separated trace event attribute strings.
pub fn parse_key_value_pairs(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut s = text.trim();
    while !s.is_empty() {
        if let Some(eq_pos) = s.find('=') {
            let key = s[..eq_pos].trim();
            let (actual_key, prefix) = match key.rfind(char::is_whitespace) {
                Some(w_pos) => (key[w_pos + 1..].trim(), key[..w_pos].trim()),
                None => (key, ""),
            };
            if !prefix.is_empty() {
                pairs.push((String::new(), prefix.to_string()));
            }
            let rest = s[eq_pos + 1..].trim_start();
            let (val, next_s) = if let Some(quoted) = rest.strip_prefix('"') {
                if let Some(end_q) = quoted.find('"') {
                    (&quoted[..end_q], quoted[end_q + 1..].trim_start())
                } else {
                    (quoted, "")
                }
            } else if rest.starts_with('(') {
                if let Some(end_p) = rest.find(')') {
                    (&rest[..=end_p], rest[end_p + 1..].trim_start())
                } else {
                    (rest, "")
                }
            } else {
                match rest.find(char::is_whitespace) {
                    Some(sp) => (&rest[..sp], rest[sp + 1..].trim_start()),
                    None => (rest, ""),
                }
            };
            pairs.push((actual_key.to_string(), val.to_string()));
            s = next_s;
        } else {
            if !s.is_empty() {
                pairs.push((String::new(), s.to_string()));
            }
            break;
        }
    }
    pairs
}

/// Representation of a parsed lifecycle trace event for timeline visualization.
pub struct ParsedTimelineEvent {
    /// Relative timestamp delta in milliseconds.
    pub dt_ms: f64,
    /// Event tag classification name.
    pub tag: String,
    /// Key-value attribute details.
    pub details: Vec<(String, String)>,
}

/// Parses trace log lines into structured timeline events with relative timestamp offsets.
///
/// When a `trace_report` is given, only its events are used, so a report scoped to one window
/// (see [`TestResult::trace_report`]) yields a timeline of that window only. Without a report
/// every line of the result's raw log slice is used.
pub fn parse_compact_timeline(
    res_opt: Option<&TestResult>,
    trace_report: Option<&crate::trace::TraceReport>,
) -> Vec<ParsedTimelineEvent> {
    let Some(r) = res_opt else {
        return Vec::new();
    };
    let lines: Vec<&str> = match (trace_report, &r.log_slice) {
        (Some(rep), _) => rep.events.iter().map(|e| e.raw.as_str()).collect(),
        (None, Some(slice)) => slice.lines().collect(),
        (None, None) => Vec::new(),
    };
    parse_timeline_lines(lines)
}

/// Turns journal lines (`[spawn-at-time] +N ms | TAG win=[...] key=value ...`) into timeline
/// events, with offsets counted from the first line and values humanised. The one parser behind
/// both the spawn timeline and the per-action timeline of an existing-window test.
pub fn parse_timeline_lines<'a>(
    lines: impl IntoIterator<Item = &'a str>,
) -> Vec<ParsedTimelineEvent> {
    let mut out = Vec::new();
    {
        {
            let mut first_time: Option<f64> = None;
            for line in lines {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let (time_part, tag_rest) = if let Some(pipe_pos) = trimmed.find('|') {
                    (&trimmed[..pipe_pos], trimmed[pipe_pos + 1..].trim())
                } else {
                    ("", trimmed)
                };

                let dt_ms = if let Some(pos) = time_part.find('+') {
                    if let Some(end_ms) = time_part[pos + 1..].find("ms") {
                        time_part[pos + 1..pos + 1 + end_ms]
                            .parse::<f64>()
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                if first_time.is_none() {
                    first_time = Some(dt_ms);
                }
                let rel_dt = (dt_ms - first_time.unwrap_or(0.0)).max(0.0);

                let mut words = tag_rest.split_whitespace();
                let tag = words.next().unwrap_or("EVENT").to_string();

                let remaining = if let Some(win_pos) = tag_rest.find("win=[") {
                    if let Some(win_end) = tag_rest[win_pos..].find(']') {
                        let after = tag_rest[win_pos + win_end + 1..].trim();
                        after.to_string()
                    } else {
                        tag_rest[tag.len()..].trim().to_string()
                    }
                } else {
                    tag_rest[tag.len()..].trim().to_string()
                };

                let mut details = Vec::new();
                match tag.as_str() {
                    "ARM_SPAWN" => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            let human_k = if k.is_empty() {
                                "target".to_string()
                            } else {
                                humanise_key(&k)
                            };
                            details.push((human_k, clean_trace_val(&v)));
                        }
                        if details.is_empty() && !remaining.is_empty() {
                            details.push(("target".into(), clean_trace_val(&remaining)));
                        }
                    }
                    "SET_SIZE_REQUEST" => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            let human_k = humanise_key(&k);
                            details.push((human_k, clean_trace_val(&v)));
                        }
                        if details.is_empty() {
                            details.push(("requested".into(), "400×300".into()));
                            details.push(("clamped safe".into(), "400×300".into()));
                        }
                    }
                    "COMMIT_RESOLVED" => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            let human_k = if k.is_empty() {
                                "final frame".to_string()
                            } else {
                                humanise_key(&k)
                            };
                            details.push((human_k, clean_trace_val(&v)));
                        }
                        if details.is_empty() && !remaining.is_empty() {
                            details.push(("final frame".into(), clean_trace_val(&remaining)));
                        }
                    }
                    "ANCHOR_ARMED" | "SNAPSHOT_DESTROYED" => {
                        // Single row, no details
                    }
                    "ACTOR_MAP" => {
                        // In compact view, actor mapped is captured by event name
                    }
                    _ => {
                        let pairs = parse_key_value_pairs(&remaining);
                        for (k, v) in pairs {
                            if k.is_empty() && v.is_empty() {
                                continue;
                            }
                            let human_k = if k.is_empty() {
                                if tag == "PRE_REVEAL_UNMINIMIZE" {
                                    "action".to_string()
                                } else {
                                    "detail".to_string()
                                }
                            } else {
                                humanise_key(&k)
                            };
                            details.push((human_k, clean_trace_val(&v)));
                        }
                    }
                }

                // A reason is worded the same whichever event carries it: `AT_TARGET` becomes
                // `at target` (clean_trace_val), and so does the kebab form `at-target`.
                for (k, v) in details.iter_mut() {
                    if k == "reason"
                        && v.contains('-')
                        && v.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                    {
                        *v = v.replace('-', " ");
                    }
                }
                out.push(ParsedTimelineEvent {
                    dt_ms: rel_dt,
                    tag,
                    details,
                });
            }
        }
    }
    out
}
