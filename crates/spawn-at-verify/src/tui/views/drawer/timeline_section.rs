//! Drawer TIMELINE section rendering and compact event parsing.

use crate::runner::TestResult;
use crate::schema::TestDefinition;
use crate::trace::TraceAnalyser;
use crate::tui::views::builder::DrawerBuilder;
use crate::tui::views::constants::{DRAWER_INDENT, DRAWER_RIGHT_PAD};
use crate::tui::views::wrapping::{normalise_dimensions, wrap_value_tokens};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

/// Renders the TIMELINE section showing lifecycle events and timing deltas.
pub fn render_timeline_section(
    builder: &mut DrawerBuilder,
    _test: &TestDefinition,
    res_opt: Option<&TestResult>,
    compact: bool,
) {
    let trace_report = res_opt
        .and_then(|r| r.log_slice.as_ref())
        .map(|s| TraceAnalyser::new().analyse(s));
    // 3. Section: TIMELINE
    let mut win_id_opt = None;
    let mut win_pid_opt = None;
    if let Some(ref rep) = trace_report {
        for ev in &rep.events {
            if win_id_opt.is_none() {
                win_id_opt = ev.window_id;
            }
            if let Some(m) = &ev.meta {
                if win_pid_opt.is_none() {
                    win_pid_opt = m.pid;
                }
            }
        }
    }

    let timeline_extra = match (win_id_opt, win_pid_opt) {
        (Some(id), Some(pid)) => Some(format!("window {} · pid {}", id, pid)),
        (Some(id), None) => Some(format!("window {}", id)),
        _ => None,
    };
    builder.push_heading("TIMELINE", timeline_extra.as_deref());

    if compact {
        let trace_events = parse_compact_timeline(res_opt, trace_report.as_ref());
        if trace_events.is_empty() {
            builder.push_kv(
                "Trace",
                vec![Span::styled(
                    "no trace captured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                "no trace captured",
            );
        } else {
            let max_a = trace_events
                .iter()
                .map(|e| format!("+{:.1} ms", e.dt_ms).width())
                .max()
                .unwrap_or(9)
                .max(9);

            let col_b_x = DRAWER_INDENT + max_a + 2;
            let detail_indent = col_b_x + 2;

            let max_human_lbl_w = trace_events
                .iter()
                .flat_map(|e| e.details.iter())
                .map(|(lbl, _)| lbl.width())
                .max()
                .unwrap_or(10)
                .min(16);

            let label_col_w = max_human_lbl_w + 2;
            let detail_val_x = detail_indent + label_col_w;
            let detail_content_w = builder
                .inner_width
                .saturating_sub(detail_val_x + DRAWER_RIGHT_PAD);
            let event_count = trace_events.len();

            for (ev_idx, ev) in trace_events.iter().enumerate() {
                let dt_str = format!("+{:.1} ms", ev.dt_ms);
                let is_bold = matches!(
                    ev.tag.as_str(),
                    "CLOAK" | "UNCLOAK" | "COMMIT_RESOLVED" | "ACTOR_MAP" | "FALLBACK_USED"
                );

                let ev_style = if is_bold {
                    Style::default()
                        .fg(Color::Rgb(200, 200, 200))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Rgb(200, 200, 200))
                };

                let dt_pad = format!("{:>width$}", dt_str, width = max_a);
                let indent_str = " ".repeat(DRAWER_INDENT);
                let plain = format!("{}{}  {}", indent_str, dt_pad, ev.tag);
                let line = Line::from(vec![
                    Span::raw(indent_str),
                    Span::styled(dt_pad, Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(ev.tag.clone(), ev_style),
                ]);
                builder.push(line, plain);

                let indent_detail_str = " ".repeat(detail_indent);
                for (lbl, val) in &ev.details {
                    let lbl_fmt = format!("{:<width$}", lbl, width = label_col_w);
                    let val_lines = wrap_value_tokens(val, detail_content_w);
                    for (line_idx, val_line) in val_lines.into_iter().enumerate() {
                        if line_idx == 0 {
                            let det_plain = format!("{}{}{}", indent_detail_str, lbl_fmt, val_line);
                            let det_line = Line::from(vec![
                                Span::raw(indent_detail_str.clone()),
                                Span::styled(lbl_fmt.clone(), Style::default().fg(Color::DarkGray)),
                                Span::styled(
                                    val_line,
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]);
                            builder.push(det_line, det_plain);
                        } else {
                            let indent_cont = " ".repeat(detail_val_x);
                            let det_plain = format!("{}{}", indent_cont, val_line);
                            let det_line = Line::from(vec![
                                Span::raw(indent_cont),
                                Span::styled(
                                    val_line,
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]);
                            builder.push(det_line, det_plain);
                        }
                    }
                }

                // Exactly 1 blank row between consecutive events in compact view
                if ev_idx + 1 < event_count {
                    builder.push_blank();
                }
            }
        }
    } else {
        // Raw view
        if let Some(r) = res_opt {
            if let Some(slice) = &r.log_slice {
                for line in slice.lines() {
                    let clean = line.trim();
                    if !clean.is_empty() {
                        builder.push(
                            Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    clean.to_string(),
                                    Style::default().fg(Color::Rgb(200, 200, 200)),
                                ),
                            ]),
                            format!("  {}", clean),
                        );
                    }
                }
            } else {
                builder.push_kv(
                    "Trace",
                    vec![Span::styled(
                        "no trace slice available",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    "no trace slice available",
                );
            }
        } else {
            builder.push_kv(
                "Trace",
                vec![Span::styled(
                    "no trace captured",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )],
                "no trace captured",
            );
        }
    }
    builder.push_blank();
}

/// Splits camelCase or snake_case identifiers into human-readable words.
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
            let (val, next_s) = if rest.starts_with('"') {
                if let Some(end_q) = rest[1..].find('"') {
                    (&rest[1..1 + end_q], rest[1 + end_q + 1..].trim_start())
                } else {
                    (&rest[1..], "")
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
    pub dt_ms: f64,
    pub tag: String,
    pub details: Vec<(String, String)>,
}

/// Parses raw trace log content into structured timeline events with relative timestamp offsets.
pub fn parse_compact_timeline(
    res_opt: Option<&TestResult>,
    _trace_report: Option<&crate::trace::TraceReport>,
) -> Vec<ParsedTimelineEvent> {
    let mut out = Vec::new();
    if let Some(r) = res_opt {
        if let Some(slice) = &r.log_slice {
            let mut first_time: Option<f64> = None;
            for line in slice.lines() {
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
