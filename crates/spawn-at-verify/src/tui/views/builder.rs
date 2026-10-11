//! Drawer content line assembly builder.

use super::constants::{DRAWER_INDENT, DRAWER_LABEL_W, DRAWER_LABEL_W_MAX, DRAWER_RIGHT_PAD};
use super::wrapping::wrap_value_tokens;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

/// Shared assembly builder for consistent layout and alignment across drawer sections.
pub struct DrawerBuilder {
    pub lines: Vec<Line<'static>>,
    pub plain: Vec<String>,
    pub width: usize,
    pub inner_width: usize,
    pub val_col_width: usize,
    /// Shared label column width used by every section (>= 14, <= 24).
    pub label_w: usize,
    /// Absolute column where values begin: `DRAWER_INDENT + label_w`.
    pub value_x: usize,
    /// Display width of the longest label pushed so far (used by the sizing probe pass).
    pub max_label_seen: usize,
}

impl DrawerBuilder {
    /// Constructs a DrawerBuilder for the given width using the minimum label column.
    pub fn new(width: usize) -> Self {
        Self::with_label_w(width, DRAWER_LABEL_W)
    }

    /// Constructs a DrawerBuilder whose label column is `label_w` columns wide
    /// (clamped to 14..=24), so every section shares the same value column.
    pub fn with_label_w(width: usize, label_w: usize) -> Self {
        let w = width.max(50);
        let inner_w = w.saturating_sub(2);
        let label_w = label_w.clamp(DRAWER_LABEL_W, DRAWER_LABEL_W_MAX);
        let value_x = DRAWER_INDENT + label_w;
        let content_w = inner_w.saturating_sub(value_x + DRAWER_RIGHT_PAD);
        Self {
            lines: Vec::new(),
            plain: Vec::new(),
            width: w,
            inner_width: inner_w,
            val_col_width: content_w,
            label_w,
            value_x,
            max_label_seen: 0,
        }
    }

    /// Records a label's display width so the sizing pass can find the longest label.
    pub fn note_label(&mut self, label: &str) {
        self.max_label_seen = self.max_label_seen.max(label.width());
    }

    /// Pads `label` to the shared label column, truncating with `…` so that at
    /// least two spaces always separate it from the value column.
    pub fn format_label(&self, label: &str) -> String {
        let max = self.label_w.saturating_sub(2);
        let mut text = String::new();
        let mut w = 0;
        if label.width() <= max {
            text.push_str(label);
            w = label.width();
        } else {
            for ch in label.chars() {
                let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
                if w + cw > max.saturating_sub(1) {
                    break;
                }
                text.push(ch);
                w += cw;
            }
            text.push('…');
            w += 1;
        }
        text.push_str(&" ".repeat(self.label_w.saturating_sub(w)));
        text
    }

    /// Computes the usable content width inside the drawer excluding borders.
    pub fn content_width(&self) -> usize {
        self.val_col_width
    }

    /// Appends a raw Line and plain text to the drawer buffer.
    pub fn push(&mut self, line: Line<'static>, text: String) {
        self.lines.push(line);
        self.plain.push(text);
    }

    /// Appends an empty blank separator line to the drawer buffer.
    pub fn push_blank(&mut self) {
        self.lines.push(Line::raw(""));
        self.plain.push(String::new());
    }

    /// Appends a section heading banner line to the drawer buffer.
    pub fn push_heading(&mut self, name: &str, extra: Option<&str>) {
        let (prefix_spans, prefix_len, plain_prefix) = match extra {
            Some(ex) => {
                let p_plain = format!(" ── {} ── {} ", name, ex);
                let p_len = p_plain.width();
                let spans = vec![
                    Span::raw(" "),
                    Span::styled("── ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        name.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ── ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        ex.to_string(),
                        Style::default().fg(Color::Rgb(200, 200, 200)),
                    ),
                    Span::raw(" "),
                ];
                (spans, p_len, p_plain)
            }
            None => {
                let p_plain = format!(" ── {} ", name);
                let p_len = p_plain.width();
                let spans = vec![
                    Span::raw(" "),
                    Span::styled("── ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        name.to_string(),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                ];
                (spans, p_len, p_plain)
            }
        };

        let target_len = self.inner_width.saturating_sub(1);
        let rule_len = target_len.saturating_sub(prefix_len);
        let rule = "─".repeat(rule_len);
        let plain_line = format!("{}{}", plain_prefix, rule);

        let mut all_spans = prefix_spans;
        all_spans.push(Span::styled(rule, Style::default().fg(Color::DarkGray)));
        self.push(Line::from(all_spans), plain_line);
    }

    /// Appends an aligned single-line key-value pair to the drawer buffer.
    pub fn push_kv(&mut self, label: &str, val_spans: Vec<Span<'static>>, val_plain: &str) {
        if val_plain.width() > self.val_col_width {
            let style = val_spans.first().map(|s| s.style).unwrap_or_default();
            self.push_wrapped_kv(label, val_plain, style);
            return;
        }
        self.note_label(label);
        let indent_str = " ".repeat(DRAWER_INDENT);
        let label_fmt = self.format_label(label);
        let plain_line = format!("{}{}{}", indent_str, label_fmt, val_plain);
        let mut spans = vec![
            Span::raw(indent_str),
            Span::styled(label_fmt, Style::default().fg(Color::DarkGray)),
        ];
        spans.extend(val_spans);
        self.push(Line::from(spans), plain_line);
    }

    /// Appends a multi-line wrapped key-value pair with hanging indent alignment.
    pub fn push_wrapped_kv(&mut self, label: &str, val_text: &str, val_style: Style) {
        self.note_label(label);
        let indent_str = " ".repeat(DRAWER_INDENT);
        let label_fmt = self.format_label(label);
        let lines = wrap_value_tokens(val_text, self.val_col_width);
        for (i, line) in lines.into_iter().enumerate() {
            if i == 0 {
                let plain = format!("{}{}{}", indent_str, label_fmt, line);
                let spans = vec![
                    Span::raw(indent_str.clone()),
                    Span::styled(label_fmt.clone(), Style::default().fg(Color::DarkGray)),
                    Span::styled(line, val_style),
                ];
                self.push(Line::from(spans), plain);
            } else {
                let indent_val = " ".repeat(self.value_x);
                let plain = format!("{}{}", indent_val, line);
                let spans = vec![Span::raw(indent_val), Span::styled(line, val_style)];
                self.push(Line::from(spans), plain);
            }
        }
    }
}
