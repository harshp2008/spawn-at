//! Layout of the installer's terminal output: every block of output (a `=== ... ===` section or
//! a plain group of lines) is separated from the previous one by exactly one blank line, and
//! the output neither starts nor ends with a blank line.
//!
//! The rule is kept by printing the blank line *before* a block, never after it, and only once
//! something has already been printed. Callers must therefore not print their own separator
//! blank lines around blocks.

use std::io::{IsTerminal, Write};
use std::sync::Mutex;

/// Remembers whether any block has been printed yet.
#[derive(Debug, Default)]
pub struct Layout {
    started: bool,
}

impl Layout {
    pub const fn new() -> Self {
        Layout { started: false }
    }

    /// Prints the separating blank line, unless this is the first block.
    pub fn block_start(&mut self, out: &mut impl Write) -> std::io::Result<()> {
        if self.started {
            writeln!(out)?;
        }
        self.started = true;
        Ok(())
    }

    /// Prints a section: separator, `=== title ===` (cyan when `color`), one blank line.
    pub fn section(
        &mut self,
        out: &mut impl Write,
        title: &str,
        color: bool,
    ) -> std::io::Result<()> {
        self.block_start(out)?;
        writeln!(out, "{}", heading(title, color))?;
        writeln!(out)
    }
}

/// The heading line of a section.
pub fn heading(title: &str, color: bool) -> String {
    if color {
        format!("\x1b[1;36m=== {title} ===\x1b[0m")
    } else {
        format!("=== {title} ===")
    }
}

static LAYOUT: Mutex<Layout> = Mutex::new(Layout::new());

fn stdout_color() -> bool {
    crate::diagnostics::detect_color_capability_for_stream(std::io::stdout().is_terminal())
        != crate::diagnostics::ColorCapability::Disabled
}

/// Starts a block of stdout output (see the module comment).
pub fn block_start() {
    let _ = LAYOUT
        .lock()
        .unwrap()
        .block_start(&mut std::io::stdout().lock());
}

/// Prints a `=== title ===` section heading on stdout.
pub fn section(title: &str) {
    let _ = LAYOUT
        .lock()
        .unwrap()
        .section(&mut std::io::stdout().lock(), title, stdout_color());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(f: impl FnOnce(&mut Layout, &mut Vec<u8>)) -> String {
        let mut l = Layout::new();
        let mut out = Vec::new();
        f(&mut l, &mut out);
        String::from_utf8(out).unwrap()
    }

    /// The first section has no blank line before it; one blank line follows the heading.
    #[test]
    fn test_first_section_has_no_leading_blank() {
        let t = text(|l, o| l.section(o, "A", false).unwrap());
        assert_eq!(t, "=== A ===\n\n");
    }

    /// Two sections with a line each: exactly one blank line between the blocks.
    #[test]
    fn test_blocks_are_separated_by_one_blank_line() {
        let t = text(|l, o| {
            l.section(o, "A", false).unwrap();
            writeln!(o, "a line").unwrap();
            l.section(o, "B", false).unwrap();
            writeln!(o, "b line").unwrap();
            l.block_start(o).unwrap();
            writeln!(o, "plain block").unwrap();
        });
        assert_eq!(
            t,
            "=== A ===\n\na line\n\n=== B ===\n\nb line\n\nplain block\n"
        );
    }

    /// Whatever the order of sections and plain blocks, with a line of content in each, the
    /// text has no two blank lines in a row and no blank line at the start or the end.
    #[test]
    fn test_no_double_blank_lines_for_any_sequence() {
        for mask in 0u32..(1 << 8) {
            let t = text(|l, o| {
                for i in 0..8 {
                    if mask >> i & 1 == 1 {
                        l.section(o, "S", false).unwrap();
                    } else {
                        l.block_start(o).unwrap();
                    }
                    writeln!(o, "content {i}").unwrap();
                }
            });
            assert!(!t.contains("\n\n\n"), "mask {mask}: {t:?}");
            assert!(
                !t.starts_with('\n') && !t.ends_with("\n\n"),
                "mask {mask}: {t:?}"
            );
        }
    }

    /// Colour only changes the escape codes around the heading, not its text.
    #[test]
    fn test_heading_colour_is_only_escape_codes() {
        assert_eq!(heading("X", false), "=== X ===");
        assert_eq!(heading("X", true), "\x1b[1;36m=== X ===\x1b[0m");
    }
}
