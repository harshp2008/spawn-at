use super::app::{ModalState, TuiApp};
use crate::runner::{AutoStatus, FinalStatus, HumanVerdictKind};
use crate::schema::TestKind;
use crate::trace::TraceAnalyser;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap},
    Frame,
};

/// Main UI drawing entry point.
pub fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    // Check minimum terminal dimensions (<60x15 per Item D.5)
    if area.width < 60 || area.height < 15 {
        let msg = "Terminal too small — need at least 60x15";
        let paragraph = Paragraph::new(Line::from(vec![Span::styled(
            msg,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" spawn-at verify ")
                .border_style(Style::default().fg(Color::Yellow)),
        );
        f.render_widget(paragraph, area);
        return;
    }

    // 1 line top padding, 1 line bottom padding
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                                       // Top padding
            Constraint::Length(if !app.fixture_exists { 2 } else { 1 }), // Header line (+ banner if fixture missing)
            Constraint::Min(8),                                          // Content panels
            Constraint::Length(1),                                       // Command bar
            Constraint::Length(1),                                       // Bottom padding
        ])
        .split(area);

    draw_header(f, app, chunks[1]);
    draw_body(f, app, chunks[2]);
    draw_footer(f, app, chunks[3]);

    if let ModalState::HumanVerdict { test_id } = &app.modal {
        draw_verdict_modal(f, app, test_id, area);
    }
}

fn draw_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mut lines = Vec::new();

    // Fixture missing banner if needed
    if !app.fixture_exists {
        lines.push(Line::from(vec![
            Span::styled(
                " ⚠ FIXTURE MISSING: ",
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " Run `cargo build -p spawn-at-verify --bin fixture --features fixture` ({})",
                    app.fixture_path.display()
                ),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Progress counts
    let all_filtered = app.filtered_tests();
    let total_count = all_filtered.len();
    let done_count = all_filtered
        .iter()
        .filter(|t| {
            app.results
                .get(&t.id)
                .map(|r| r.final_status != FinalStatus::Pending)
                .unwrap_or(false)
        })
        .count();

    let waiting_count = all_filtered
        .iter()
        .filter(|t| match t.kind {
            TestKind::Human | TestKind::Both => app
                .results
                .get(&t.id)
                .map(|r| r.human_verdict.verdict == HumanVerdictKind::Unset)
                .unwrap_or(true),
            TestKind::Auto => false,
        })
        .count();

    let filter_label = app.filter_mode.label();

    lines.push(Line::from(vec![
        Span::styled(
            "spawn-at verify",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} (f)", filter_label),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}/{} done", done_count, total_count),
            Style::default()
                .fg(if done_count > 0 {
                    Color::Green
                } else {
                    Color::White
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} waiting for you", waiting_count),
            Style::default()
                .fg(if waiting_count > 0 {
                    Color::Yellow
                } else {
                    Color::DarkGray
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let header_widget = Paragraph::new(lines);
    f.render_widget(header_widget, area);
}

fn draw_body(f: &mut Frame, app: &TuiApp, area: Rect) {
    if app.detail_drawer_open {
        // Expanded full-screen detail drawer view
        draw_detail_drawer(f, app, area, true);
    } else if area.width < 75 {
        // Narrow (<75 per Item D.1): single pane; list full width; Enter opens overview/drawer full screen
        draw_test_table_pane(f, app, area);
    } else {
        // Side by side: Left test suite + Right overview panel
        let (left_pct, right_pct) = if area.width >= 110 {
            (55, 45) // Wide
        } else {
            (52, 48) // Medium
        };
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_pct),
                Constraint::Percentage(right_pct),
            ])
            .split(area);

        draw_test_table_pane(f, app, main_chunks[0]);
        draw_right_panel_default(f, app, main_chunks[1]);
    }
}

fn draw_test_table_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // Test list
            Constraint::Length(1), // Kinds legend
            Constraint::Length(3), // Result box (compact 1-2 lines per Item D.4)
        ])
        .split(area);

    draw_test_table(f, app, chunks[0]);
    draw_table_legend(f, chunks[1]);
    draw_result_box(f, app, chunks[2]);
}

fn draw_table_legend(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" Kinds: ", Style::default().fg(Color::DarkGray)),
        Span::styled("auto", Style::default().fg(Color::White)),
        Span::styled(
            " = verified by math & trace · ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("you", Style::default().fg(Color::Yellow)),
        Span::styled(
            " = visual review only · ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("both", Style::default().fg(Color::Cyan)),
        Span::styled(" = auto + visual", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

pub fn get_test_category(cat_or_id: &str) -> &'static str {
    if cat_or_id.starts_with("preflight") || cat_or_id == "basics" {
        "Preflight"
    } else if cat_or_id.starts_with("placement") || cat_or_id == "placement" {
        "Placement"
    } else if cat_or_id.starts_with("cloak") || cat_or_id == "animation-and-cloak" {
        "Cloak & Delayed Paint"
    } else if cat_or_id.starts_with("transform") || cat_or_id == "transforms" {
        "Transforms"
    } else if cat_or_id.starts_with("focus") {
        "Focus"
    } else if cat_or_id.starts_with("query") || cat_or_id == "queries" {
        "Queries"
    } else if cat_or_id.starts_with("robust")
        || cat_or_id.starts_with("resilience")
        || cat_or_id == "robustness"
    {
        "Robustness"
    } else if cat_or_id.starts_with("platform") {
        "Platform"
    } else {
        "General"
    }
}

pub fn pad_str(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - count))
    }
}

/// Suffix truncation: keeps the distinguishing suffix (e.g. "…/delay-50"), never the shared prefix (Item C.4).
pub fn truncate_id_suffix(id: &str, max_len: usize) -> String {
    let count = id.chars().count();
    if count <= max_len {
        id.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let take = max_len - 1;
        let suffix: String = id.chars().rev().take(take).collect();
        let suffix: String = suffix.chars().rev().collect();
        format!("…{}", suffix)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnLayout {
    pub cursor_width: usize,
    pub status_width: usize,
    pub id_width: usize,
    pub kind_width: Option<usize>,
    pub title_width: Option<usize>,
    pub use_short_name: bool,
}

impl ColumnLayout {
    pub fn compute(inner_width: usize, terminal_width: u16) -> Self {
        let cursor_width = 2;
        let status_width = 18; // Wide enough for "auto ✔ | you: ✔"
        if terminal_width >= 110 {
            // Wide (>=110): list | overview side by side, all columns
            let fixed = cursor_width + status_width + 6;
            let remaining = inner_width.saturating_sub(fixed);
            let id_width = 28.min(remaining / 2).max(18);
            let title_width = remaining.saturating_sub(id_width);
            Self {
                cursor_width,
                status_width,
                id_width,
                kind_width: Some(6),
                title_width: Some(title_width),
                use_short_name: false,
            }
        } else if terminal_width >= 75 {
            // Medium (75-109): side by side; list shows Status + short name + Kind; drop Title column
            let fixed = cursor_width + status_width + 6;
            let id_width = inner_width.saturating_sub(fixed).max(12);
            Self {
                cursor_width,
                status_width,
                id_width,
                kind_width: Some(6),
                title_width: None,
                use_short_name: true,
            }
        } else {
            // Narrow (<75): single pane; list full width; status NEVER dropped.
            // Drop Title, then Kind if narrow, shorten ID.
            if inner_width >= 42 {
                let fixed = cursor_width + status_width + 6;
                let id_width = inner_width.saturating_sub(fixed).max(8);
                Self {
                    cursor_width,
                    status_width,
                    id_width,
                    kind_width: Some(6),
                    title_width: None,
                    use_short_name: true,
                }
            } else {
                let fixed = cursor_width + status_width;
                let id_width = inner_width.saturating_sub(fixed).max(8);
                Self {
                    cursor_width,
                    status_width,
                    id_width,
                    kind_width: None,
                    title_width: None,
                    use_short_name: true,
                }
            }
        }
    }
}

fn draw_test_table(f: &mut Frame, app: &TuiApp, area: Rect) {
    let table_title = if let Some(cat) = &app.category_filter {
        format!(" Test Suite [Category: {}] ", cat)
    } else {
        " Test Suite ".to_string()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(table_title)
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 || inner.width < 10 {
        return;
    }

    let inner_w = inner.width as usize;
    let layout = ColumnLayout::compute(inner_w, f.area().width);

    // Split inner into 1-line header and list body
    let table_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    // Render header row (strictly identical starting x-coordinates per Item C.1)
    let mut header_spans = Vec::new();
    header_spans.push(Span::styled("  ", Style::default())); // Cursor column
    header_spans.push(Span::styled(
        pad_str("Status", layout.status_width),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    let id_header = if layout.use_short_name {
        "Name"
    } else {
        "Test ID"
    };
    header_spans.push(Span::styled(
        pad_str(id_header, layout.id_width),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));
    if let Some(kw) = layout.kind_width {
        header_spans.push(Span::styled(
            pad_str("Kind", kw),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(_tw) = layout.title_width {
        header_spans.push(Span::styled(
            "Title",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(header_spans)), table_chunks[0]);

    // Build List items (category separator rows spanning full width + test rows)
    let filtered = app.filtered_tests();
    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_item_idx: Option<usize> = None;
    let mut last_category: Option<&str> = None;

    for (test_idx, test) in filtered.iter().enumerate() {
        let category = get_test_category(test.category());
        let cat_key = test.category();
        let is_collapsed = app.collapsed_categories.contains(cat_key)
            || app.collapsed_categories.contains(category);

        let cat_tests: Vec<_> = filtered
            .iter()
            .filter(|t| get_test_category(t.category()) == category)
            .collect();
        let cat_total = cat_tests.len();

        if last_category != Some(category) {
            last_category = Some(category);

            // Compute category roll-up status
            let cat_passes = cat_tests
                .iter()
                .filter(|t| {
                    app.results
                        .get(&t.id)
                        .map(|r| {
                            r.final_status == FinalStatus::Pass
                                || r.auto_status == AutoStatus::AutoPass
                        })
                        .unwrap_or(false)
                })
                .count();
            let cat_fails = cat_tests
                .iter()
                .filter(|t| {
                    app.results
                        .get(&t.id)
                        .map(|r| {
                            r.final_status == FinalStatus::Fail
                                || r.auto_status == AutoStatus::AutoFail
                        })
                        .unwrap_or(false)
                })
                .count();

            let roll_up_str = if cat_passes == cat_total && cat_total > 0 {
                format!("{}/{} ✔", cat_total, cat_total)
            } else if cat_fails > 0 {
                format!(
                    "{}/{} ✖ · {}/{} ✔",
                    cat_fails, cat_total, cat_passes, cat_total
                )
            } else if cat_passes > 0 {
                format!("{}/{} ✔", cat_passes, cat_total)
            } else {
                format!("0/{} done", cat_total)
            };

            // Full-width category rule line spanning all columns (colspan per Item C.2)
            let icon = if is_collapsed { "▶ [+] " } else { "▼ " };
            let sep_prefix = format!("── {}{} ", icon, category);
            let sep_suffix = format!("  {} ──", roll_up_str);
            let prefix_len = sep_prefix.chars().count();
            let suffix_len = sep_suffix.chars().count();
            let fill_count = inner_w.saturating_sub(prefix_len + suffix_len);
            let full_line = format!("{}{}{}", sep_prefix, "─".repeat(fill_count), sep_suffix);

            items.push(ListItem::new(Line::from(vec![Span::styled(
                full_line,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )])));
        }

        if is_collapsed {
            let is_first_in_cat = filtered
                .iter()
                .take(test_idx)
                .all(|t| get_test_category(t.category()) != category);
            if is_first_in_cat {
                if cat_tests.iter().any(|t| {
                    filtered.iter().position(|ft| ft.id == t.id) == Some(app.selected_index)
                }) {
                    selected_item_idx = Some(items.len());
                }
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!(
                            "▶ [{}] collapsed ({} tests folded — press Space to expand)",
                            category, cat_total
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])));
            }
            continue;
        }

        if test_idx == app.selected_index {
            selected_item_idx = Some(items.len());
        }

        let res = app.results.get(&test.id);
        let status_span = match res {
            None => Span::styled(
                pad_str("● not run", layout.status_width),
                Style::default().fg(Color::DarkGray),
            ),
            Some(r) => match test.kind {
                TestKind::Auto => {
                    let is_warn = r
                        .message
                        .as_deref()
                        .map(|m| m.contains("dirty build") || m.contains("Preflight warning"))
                        .unwrap_or(false);
                    if is_warn {
                        Span::styled(
                            pad_str("! warn", layout.status_width),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        match r.auto_status {
                            AutoStatus::AutoPass => Span::styled(
                                pad_str("✔ pass", layout.status_width),
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            AutoStatus::AutoFail => Span::styled(
                                pad_str("✖ fail", layout.status_width),
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            ),
                            AutoStatus::Skipped => Span::styled(
                                pad_str("⊘ skip", layout.status_width),
                                Style::default().fg(Color::DarkGray),
                            ),
                            AutoStatus::Error => Span::styled(
                                pad_str("✖ error", layout.status_width),
                                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                            ),
                            AutoStatus::Blocked => Span::styled(
                                pad_str("⊘ blocked", layout.status_width),
                                Style::default().fg(Color::DarkGray),
                            ),
                            AutoStatus::Pending => Span::styled(
                                pad_str("● not run", layout.status_width),
                                Style::default().fg(Color::DarkGray),
                            ),
                            AutoStatus::Running => Span::styled(
                                pad_str("⏳ running", layout.status_width),
                                Style::default().fg(Color::Cyan),
                            ),
                        }
                    }
                }
                TestKind::Both => {
                    let auto_char = match r.auto_status {
                        AutoStatus::AutoPass => "✔",
                        AutoStatus::AutoFail => "✖",
                        _ => "●",
                    };
                    let human_str = match r.human_verdict.verdict {
                        HumanVerdictKind::Unset => {
                            if app.waiting_for_human.as_deref() == Some(&test.id) {
                                "?"
                            } else {
                                "pending"
                            }
                        }
                        HumanVerdictKind::Pass => "✔",
                        HumanVerdictKind::Fail => "✖",
                        HumanVerdictKind::Skip => "⊘",
                        HumanVerdictKind::Flaky => "!",
                    };

                    let both_text = format!("auto {} | you: {}", auto_char, human_str);
                    let both_style = if r.final_status == FinalStatus::Fail
                        || r.auto_status == AutoStatus::AutoFail
                        || r.human_verdict.verdict == HumanVerdictKind::Fail
                    {
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                    } else if r.final_status == FinalStatus::Pass
                        || (r.auto_status == AutoStatus::AutoPass
                            && r.human_verdict.verdict == HumanVerdictKind::Pass)
                    {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else if app.waiting_for_human.as_deref() == Some(&test.id) {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    Span::styled(pad_str(&both_text, layout.status_width), both_style)
                }
                TestKind::Human => match r.human_verdict.verdict {
                    HumanVerdictKind::Pass => Span::styled(
                        pad_str("✔ pass", layout.status_width),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    HumanVerdictKind::Fail => Span::styled(
                        pad_str("✖ fail", layout.status_width),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    HumanVerdictKind::Skip => Span::styled(
                        pad_str("⊘ skip", layout.status_width),
                        Style::default().fg(Color::DarkGray),
                    ),
                    HumanVerdictKind::Flaky => Span::styled(
                        pad_str("! flaky", layout.status_width),
                        Style::default().fg(Color::Magenta),
                    ),
                    HumanVerdictKind::Unset => {
                        if app.waiting_for_human.as_deref() == Some(&test.id) {
                            Span::styled(
                                pad_str("? waiting", layout.status_width),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            Span::styled(
                                pad_str("● pending", layout.status_width),
                                Style::default().fg(Color::Yellow),
                            )
                        }
                    }
                },
            },
        };

        let cursor_str = if test_idx == app.selected_index {
            "> "
        } else {
            "  "
        };
        let cursor_span = Span::styled(
            cursor_str,
            if test_idx == app.selected_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        );

        let raw_id = if layout.use_short_name {
            test.id.rsplit('/').next().unwrap_or(&test.id)
        } else {
            &test.id
        };
        let truncated_id = truncate_id_suffix(raw_id, layout.id_width);
        let id_span = Span::styled(
            pad_str(&truncated_id, layout.id_width),
            if test_idx == app.selected_index {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        );

        let mut row_spans = vec![cursor_span, status_span, id_span];

        if let Some(kw) = layout.kind_width {
            let (kind_label, kind_color) = match test.kind {
                TestKind::Auto => ("auto", Color::DarkGray),
                TestKind::Human => ("you", Color::Yellow),
                TestKind::Both => ("both", Color::Cyan),
            };
            row_spans.push(Span::styled(
                pad_str(kind_label, kw),
                Style::default().fg(kind_color),
            ));
        }

        if let Some(_tw) = layout.title_width {
            row_spans.push(Span::styled(&test.title, Style::default().fg(Color::Gray)));
        }

        items.push(ListItem::new(Line::from(row_spans)));
    }

    let list_widget = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(selected_item_idx);
    f.render_stateful_widget(list_widget, table_chunks[1], &mut state);
}

fn draw_result_box(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();
    let res_opt = test_opt.and_then(|t| app.results.get(&t.id));

    let mut lines = Vec::new();

    if let (Some(test), Some(res)) = (test_opt, res_opt) {
        // Auto status line
        match res.auto_status {
            AutoStatus::AutoPass => {
                let is_dirty_warn = res
                    .message
                    .as_deref()
                    .map(|m| m.contains("dirty build") || m.contains("Preflight warning"))
                    .unwrap_or(false);

                if is_dirty_warn {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "! Auto: WARN ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            res.message.as_deref().unwrap_or("Warning detected"),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]));
                } else {
                    let mut timing_info = String::new();
                    if let Some(slice) = &res.log_slice {
                        let report = TraceAnalyser::new().analyse(slice);
                        if let Some(ms) = report.cloak_duration_ms {
                            timing_info =
                                format!(" (cloak: {:.1}ms, budget: 100ms) · Trace: PASS", ms);
                        } else if report.is_pass() {
                            timing_info = " · Trace: 0 violations, PASS".into();
                        }
                    }
                    lines.push(Line::from(vec![
                        Span::styled(
                            "✔ Auto: PASS",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(timing_info, Style::default().fg(Color::White)),
                    ]));
                }
            }
            AutoStatus::AutoFail => {
                let reason = res.message.as_deref().unwrap_or("Verification failed");
                lines.push(Line::from(vec![
                    Span::styled(
                        "✖ Auto: FAIL - ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(reason, Style::default().fg(Color::LightRed)),
                ]));
            }
            AutoStatus::Skipped => {
                lines.push(Line::from(vec![
                    Span::styled("⊘ Auto: SKIPPED - ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        res.message.as_deref().unwrap_or("Skipped requirements"),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
            AutoStatus::Error => {
                lines.push(Line::from(vec![
                    Span::styled(
                        "✖ Auto: ERROR - ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        res.message.as_deref().unwrap_or("Harness execution error"),
                        Style::default().fg(Color::LightRed),
                    ),
                ]));
            }
            AutoStatus::Blocked => {
                lines.push(Line::from(vec![
                    Span::styled("⊘ Auto: BLOCKED - ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        res.message.as_deref().unwrap_or("Blocked by preflight"),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
            }
            AutoStatus::Pending => {
                lines.push(Line::styled(
                    "● Auto: Not executed yet",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            AutoStatus::Running => {
                lines.push(Line::styled(
                    "⏳ Auto: Running assertions...",
                    Style::default().fg(Color::Cyan),
                ));
            }
        }

        // Visual review prompt line if waiting
        if (test.kind == TestKind::Human || test.kind == TestKind::Both)
            && app.waiting_for_human.as_deref() == Some(&test.id)
        {
            lines.push(Line::from(vec![
                Span::styled(
                    "? Visual: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Press 'p' (Pass), 'x' (Fail), 's' (Skip), 'k' (Flaky)",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
    } else {
        lines.push(Line::styled(
            "● Not run yet. Select test and press 'r' to execute or 'a' to run all.",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let border_color = if let Some(res) = res_opt {
        if res.final_status == FinalStatus::Fail
            || res.auto_status == AutoStatus::AutoFail
            || res.final_status == FinalStatus::Error
        {
            Color::Red
        } else if res.final_status == FinalStatus::Pass || res.auto_status == AutoStatus::AutoPass {
            Color::Green
        } else if app.waiting_for_human.is_some() {
            Color::Yellow
        } else {
            Color::DarkGray
        }
    } else {
        Color::DarkGray
    };

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Result ")
                .border_style(Style::default().fg(border_color))
                .padding(Padding::horizontal(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn draw_right_panel_default(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        let res_opt = app.results.get(&test.id);

        // Group 1: Title, accent colour (Item B)
        lines.push(Line::styled(
            &test.title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::raw(""));

        // Group 2: ID and Kind with fixed label column (width 7, values line up)
        let kind_desc = match test.kind {
            TestKind::Auto => "auto — verified by math & trace",
            TestKind::Human => "you — visual review only",
            TestKind::Both => "both — auto + visual review",
        };
        lines.push(Line::from(vec![
            Span::styled("ID:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(&test.id, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Kind:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(kind_desc, Style::default().fg(Color::Gray)),
        ]));
        lines.push(Line::raw(""));

        // Group 3: Description paragraph, wrapped
        for desc_line in test.description.lines() {
            lines.push(Line::styled(desc_line, Style::default().fg(Color::Gray)));
        }
        lines.push(Line::raw(""));

        // Group 4: What to look for
        lines.push(Line::styled(
            "What to look for:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        match test.kind {
            TestKind::Auto => {
                // Auto cases never show a visual question (Item B)
                lines.push(Line::styled(
                    "Nothing to watch: verified automatically.",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            TestKind::Human | TestKind::Both => {
                if let Some(human) = &test.human {
                    for watch_line in human.watch.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("• ", Style::default().fg(Color::Yellow)),
                            Span::styled(
                                watch_line,
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }
                } else {
                    lines.push(Line::styled(
                        "• Window placed accurately without flicker or jump.",
                        Style::default().fg(Color::White),
                    ));
                }
            }
        }
        lines.push(Line::raw(""));

        // Group 5: Outcome
        lines.push(Line::styled(
            "Outcome:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            let auto_label = match res.auto_status {
                AutoStatus::AutoPass => Span::styled(
                    "PASS",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                AutoStatus::AutoFail => Span::styled(
                    "FAIL",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                AutoStatus::Skipped => {
                    Span::styled("SKIPPED", Style::default().fg(Color::DarkGray))
                }
                AutoStatus::Error => Span::styled("ERROR", Style::default().fg(Color::Red)),
                AutoStatus::Blocked => {
                    Span::styled("BLOCKED", Style::default().fg(Color::DarkGray))
                }
                AutoStatus::Pending => Span::styled("PENDING", Style::default().fg(Color::Yellow)),
                AutoStatus::Running => Span::styled("RUNNING", Style::default().fg(Color::Cyan)),
            };

            let auto_sym = match res.auto_status {
                AutoStatus::AutoPass => Span::styled("✔ ", Style::default().fg(Color::Green)),
                AutoStatus::AutoFail | AutoStatus::Error => {
                    Span::styled("✖ ", Style::default().fg(Color::Red))
                }
                _ => Span::styled("● ", Style::default().fg(Color::DarkGray)),
            };

            lines.push(Line::from(vec![
                auto_sym,
                Span::styled("Auto: ", Style::default().fg(Color::White)),
                auto_label,
                Span::styled(
                    format!(" ({} ms)", res.duration_ms),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            // Detail line: ONLY show for failure reasons or warnings; REMOVE redundant "Assertions passed" on pass! (Item A.1, A.2)
            if let Some(msg) = &res.message {
                let is_pass = res.auto_status == AutoStatus::AutoPass
                    && (res.final_status == FinalStatus::Pass
                        || res.final_status == FinalStatus::Pending);
                let is_redundant_pass =
                    is_pass && (msg == "Assertions passed" || msg == "Preflight check passed");
                if !is_redundant_pass {
                    let is_warn = msg.contains("warning")
                        || msg.contains("Warn")
                        || msg.contains("dirty build");
                    let detail_style = if is_warn {
                        Style::default().fg(Color::Yellow)
                    } else if res.auto_status == AutoStatus::AutoFail
                        || res.final_status == FinalStatus::Fail
                    {
                        Style::default().fg(Color::LightRed)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    for msg_line in msg.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("Detail: ", Style::default().fg(Color::DarkGray)),
                            Span::styled(msg_line, detail_style),
                        ]));
                    }
                }
            }

            // Cloak & Trace line
            if let Some(slice) = &res.log_slice {
                let report = TraceAnalyser::new().analyse(slice);
                let trace_part = if report.is_pass() {
                    Span::styled("Trace: 0 violations", Style::default().fg(Color::Green))
                } else {
                    Span::styled(
                        format!("Trace: {} violations", report.violations.len()),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )
                };

                if let Some(ms) = report.cloak_duration_ms {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("Cloak: {:.1} ms (budget 100 ms) · ", ms),
                            Style::default().fg(Color::White),
                        ),
                        trace_part,
                    ]));
                } else {
                    lines.push(Line::from(vec![trace_part]));
                }
            }

            // Visual review outcome if relevant
            if test.kind == TestKind::Human || test.kind == TestKind::Both {
                match res.human_verdict.verdict {
                    HumanVerdictKind::Pass => {
                        lines.push(Line::from(vec![
                            Span::styled("✔ ", Style::default().fg(Color::Green)),
                            Span::styled("You: PASS", Style::default().fg(Color::Green)),
                        ]));
                    }
                    HumanVerdictKind::Fail => {
                        lines.push(Line::from(vec![
                            Span::styled("✖ ", Style::default().fg(Color::Red)),
                            Span::styled("You: FAIL", Style::default().fg(Color::Red)),
                        ]));
                    }
                    HumanVerdictKind::Skip => {
                        lines.push(Line::from(vec![
                            Span::styled("⊘ ", Style::default().fg(Color::DarkGray)),
                            Span::styled("You: SKIPPED", Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                    HumanVerdictKind::Flaky => {
                        lines.push(Line::from(vec![
                            Span::styled("! ", Style::default().fg(Color::Magenta)),
                            Span::styled("You: FLAKY", Style::default().fg(Color::Magenta)),
                        ]));
                    }
                    HumanVerdictKind::Unset => {
                        if app.waiting_for_human.as_deref() == Some(&test.id) {
                            lines.push(Line::from(vec![
                                Span::styled("? ", Style::default().fg(Color::Yellow)),
                                Span::styled(
                                    "You: Waiting for verdict",
                                    Style::default().fg(Color::Yellow),
                                ),
                            ]));
                        } else {
                            lines.push(Line::from(vec![
                                Span::styled("● ", Style::default().fg(Color::DarkGray)),
                                Span::styled(
                                    "You: Pending review",
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));
                        }
                    }
                }
            }
        } else {
            lines.push(Line::styled(
                "● Not executed yet",
                Style::default().fg(Color::DarkGray),
            ));
        }
    } else {
        lines.push(Line::styled(
            "No test selected",
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Clipping check
    let available_height = area.height.saturating_sub(2) as usize;
    let inner_width = area.width.saturating_sub(4).max(1) as usize;

    let mut total_visual_rows = 0;
    for line in &lines {
        let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        let rows = if len == 0 {
            1
        } else {
            len.div_ceil(inner_width)
        };
        total_visual_rows += rows;
    }

    let is_clipped = total_visual_rows > available_height;

    let final_lines: Vec<Line> = if is_clipped && available_height > 2 {
        let hint_rows = if 46 <= inner_width { 2 } else { 3 };
        let max_rows = available_height.saturating_sub(hint_rows + 2);
        let mut truncated = Vec::new();
        let mut cur_rows = 0;
        for line in lines {
            let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            let rows = if len == 0 {
                1
            } else {
                len.div_ceil(inner_width)
            };
            if cur_rows + rows > max_rows {
                break;
            }
            cur_rows += rows;
            truncated.push(line);
        }
        truncated.push(Line::from(vec![Span::styled(
            "▼ more: press Enter or d to expand details/logs",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        truncated
    } else {
        let mut padded = lines;
        if total_visual_rows + 2 <= available_height {
            padded.push(Line::raw(""));
            padded.push(Line::styled(
                "Press Enter or d for full details, oracle diff & trace logs",
                Style::default().fg(Color::DarkGray),
            ));
        } else if total_visual_rows < available_height {
            padded.push(Line::styled(
                "Press Enter or d for full details, oracle diff & trace logs",
                Style::default().fg(Color::DarkGray),
            ));
        }
        padded
    };

    let p = Paragraph::new(final_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Test Overview ")
                .padding(Padding::horizontal(1)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn draw_detail_drawer(f: &mut Frame, app: &TuiApp, area: Rect, is_expanded: bool) {
    let test_opt = app.selected_test();

    let mut lines = Vec::new();

    if let Some(test) = test_opt {
        let res_opt = app.results.get(&test.id);

        // Header info in drawer
        lines.push(Line::styled(
            &test.title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&test.id, Style::default().fg(Color::White)),
            Span::styled(" · Kind: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:?}", test.kind), Style::default().fg(Color::Gray)),
        ]));
        lines.push(Line::raw(""));

        // 1. FAILURE REASON FIRST
        if let Some(res) = res_opt {
            if res.final_status == FinalStatus::Fail
                || res.auto_status == AutoStatus::AutoFail
                || res.final_status == FinalStatus::Error
            {
                lines.push(Line::styled(
                    "✖ FAILURE REASON:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
                let reason_text = res
                    .message
                    .as_deref()
                    .unwrap_or("Test execution failed without specific message");
                for line in reason_text.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            line,
                            Style::default()
                                .fg(Color::LightRed)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
                lines.push(Line::raw(""));
            }
        }

        // 2. GEOMETRIC ORACLE EXPECTED VS ACTUAL
        lines.push(Line::styled(
            "--- 1. Geometric Oracle Comparison ---",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(rect) = &test.expect.rect {
            lines.push(Line::from(format!(
                "Expected: anchor={:?}, pivot={:?}, size={:?}, margin={:?}, tolerance=±{}px",
                rect.anchor.as_deref().unwrap_or("none"),
                rect.pivot.as_deref().unwrap_or("none"),
                rect.size,
                rect.margin,
                rect.tolerance_px
            )));
        } else {
            lines.push(Line::raw(
                "Expected: (No geometric rect expectation defined)",
            ));
        }

        if let Some(res) = res_opt {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&res.stdout) {
                if let (Some(x), Some(y), Some(w), Some(h)) = (
                    val.get("x").and_then(|v| v.as_i64()),
                    val.get("y").and_then(|v| v.as_i64()),
                    val.get("w").and_then(|v| v.as_u64()),
                    val.get("h").and_then(|v| v.as_u64()),
                ) {
                    let actual_color = if res.auto_status == AutoStatus::AutoFail {
                        Color::LightRed
                    } else {
                        Color::Green
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            "Actual Geometry: ",
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(
                                "pos=({}, {}), size=({}x{}), source={}",
                                x,
                                y,
                                w,
                                h,
                                val.get("source")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("claim")
                            ),
                            Style::default().fg(actual_color),
                        ),
                    ]));
                }
            }
        }
        lines.push(Line::raw(""));

        // 3. TRACE DIAGNOSTICS & TIMELINE
        lines.push(Line::styled(
            "--- 2. Compositor Actor Trace Diagnostics ---",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            if let Some(slice) = &res.log_slice {
                let report = TraceAnalyser::new().analyse(slice);
                if report.is_pass() {
                    lines.push(Line::from(vec![
                        Span::styled("✓ Trace Clean: ", Style::default().fg(Color::Green)),
                        Span::raw(
                            "Zero cloak or geometry violations recorded by compositor extension.",
                        ),
                    ]));
                } else {
                    for v in &report.violations {
                        lines.push(Line::styled(
                            format!("  ⚠ {:?}: {}", v.kind, v.message),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ));
                    }
                }

                if let Some(ms) = report.cloak_duration_ms {
                    lines.push(Line::from(format!(
                        "Measured Cloak Duration: {:.2}ms (Provisional budget: 100ms)",
                        ms
                    )));
                }

                lines.push(Line::styled(
                    "Trace Event Timeline:",
                    Style::default().fg(Color::DarkGray),
                ));
                for l in slice.lines().take(12) {
                    lines.push(Line::from(format!("  {}", l)));
                }
            } else {
                lines.push(Line::raw("No trace slice captured for this run."));
            }
        } else {
            lines.push(Line::raw("Test has not been executed yet."));
        }
        lines.push(Line::raw(""));

        // 4. CAPTURED STDOUT / STDERR (No red on passing tests! Item A.1)
        lines.push(Line::styled(
            "--- 3. Captured Output (Stdout & Stderr) ---",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(res) = res_opt {
            if !res.stderr.is_empty() {
                let stderr_color = if res.final_status == FinalStatus::Fail
                    || res.auto_status == AutoStatus::AutoFail
                {
                    Color::LightRed
                } else {
                    Color::DarkGray
                };
                lines.push(Line::styled(
                    "Stderr Output:",
                    Style::default().fg(stderr_color),
                ));
                for line in res.stderr.lines().take(12) {
                    lines.push(Line::from(format!("  {}", line)));
                }
            } else {
                lines.push(Line::styled(
                    "Stderr: (empty)",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if !res.stdout.is_empty() {
                lines.push(Line::styled(
                    "Stdout Output:",
                    Style::default().fg(Color::DarkGray),
                ));
                for line in res.stdout.lines().take(6) {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
        } else {
            lines.push(Line::raw("Output not yet available."));
        }
        lines.push(Line::raw(""));

        // 5. HUMAN VISUAL REVIEW
        if let Some(human) = &test.human {
            lines.push(Line::styled(
                "--- 4. Visual Review Watch Prompt ---",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(vec![
                Span::styled(
                    "Watch Prompt: ",
                    Style::default().add_modifier(Modifier::UNDERLINED),
                ),
                Span::raw(&human.watch),
            ]));

            let (v_str, v_color) = res_opt
                .map(|r| match r.human_verdict.verdict {
                    HumanVerdictKind::Pass => ("PASS", Color::Green),
                    HumanVerdictKind::Fail => ("FAIL", Color::Red),
                    HumanVerdictKind::Skip => ("SKIP", Color::DarkGray),
                    HumanVerdictKind::Flaky => ("FLAKY", Color::Magenta),
                    HumanVerdictKind::Unset => ("PENDING", Color::Yellow),
                })
                .unwrap_or(("PENDING", Color::Yellow));

            lines.push(Line::from(vec![
                Span::styled("Human Verdict: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    v_str,
                    Style::default().fg(v_color).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::raw(""));
        }

        // 6. METADATA
        lines.push(Line::styled(
            "--- 5. Spec Details ---",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(format!(
            "Action: {} {:?}",
            test.action.command, test.action.args
        )));
        if let Some(sub) = &test.subject {
            lines.push(Line::from(format!("Subject: {} {:?}", sub.app, sub.args)));
        }
    } else {
        lines.push(Line::raw("No test selected."));
    }

    let total_lines = lines.len();
    let current_line = (app.detail_scroll as usize + 1).min(total_lines.max(1));

    let title = format!(
        " Test Details & Logs (line {}/{} · j/k scroll, Esc/Enter close) ",
        current_line, total_lines
    );

    let detail_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(if is_expanded {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                })
                .padding(Padding::horizontal(1)),
        )
        .scroll((app.detail_scroll, 0))
        .wrap(Wrap { trim: true });

    f.render_widget(detail_widget, area);
}

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let test_opt = app.selected_test();
    let msg = if app.waiting_for_human.is_some() {
        if app.detail_drawer_open {
            "j/k: Scroll · p: Pass · x: Fail · s: Skip · Esc/Enter/d: Close drawer · q: Quit"
        } else {
            "p: Pass · x: Fail · s: Skip · k: Flaky · r: Re-run · Esc: Cancel · q: Quit"
        }
    } else if app.detail_drawer_open {
        "j/k: Scroll · Esc/Enter/d: Close drawer · q: Quit"
    } else {
        let is_human = test_opt
            .map(|t| t.kind == TestKind::Human || t.kind == TestKind::Both)
            .unwrap_or(false);
        if is_human {
            "r: Run · a: Run all · R: Run Cat · Tab: Next Cat · Space: Fold · Enter/d: Drawer · f: Filter · v: Verdict · q: Quit"
        } else {
            "r: Run · a: Run all · R: Run Cat · Tab: Next Cat · Space: Fold · Enter/d: Drawer · f: Filter · q: Quit"
        }
    };

    let footer_widget = Paragraph::new(Line::from(vec![
        Span::styled(
            " Keys: ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(msg, Style::default().fg(Color::White)),
    ]));

    f.render_widget(footer_widget, area);
}

fn draw_verdict_modal(f: &mut Frame, app: &TuiApp, test_id: &str, area: Rect) {
    let popup_area = centered_rect(60, 40, area);
    f.render_widget(Clear, popup_area);

    let test_def = app.registry.get_test(test_id);
    let watch_prompt = test_def
        .and_then(|t| t.human.as_ref())
        .map(|h| h.watch.as_str())
        .unwrap_or("No specific watch prompt");

    let text = vec![
        Line::from(vec![
            Span::styled(
                "Test: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(test_id),
        ]),
        Line::raw(""),
        Line::styled(
            "Visual Review Watch Instruction:",
            Style::default().fg(Color::Yellow),
        ),
        Line::from(format!("  \"{}\"", watch_prompt)),
        Line::raw(""),
        Line::styled(
            "Select Human Review Verdict:",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(vec![
            Span::styled(
                " [P] Pass ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Window positioned cleanly with zero visual flicker"),
        ]),
        Line::from(vec![
            Span::styled(
                " [X] Fail ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Visual defect, flicker, flash, or wrong geometry observed"),
        ]),
        Line::from(vec![
            Span::styled(
                " [S] Skip ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Cannot be visually verified in current environment"),
        ]),
        Line::from(vec![
            Span::styled(
                " [K] Flaky ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - Inconsistent visual behavior across multiple runs"),
        ]),
        Line::raw(""),
        Line::styled(
            "Press [Esc] to cancel without recording.",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Record Human Review Verdict ")
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
