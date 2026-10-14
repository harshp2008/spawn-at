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

/// The text of a warning line: bold yellow when `cap` allows colour, otherwise the plain text.
pub fn highlight_warning(text: &str, cap: crate::diagnostics::ColorCapability) -> String {
    if cap == crate::diagnostics::ColorCapability::Disabled {
        text.to_string()
    } else {
        format!("\x1b[1;33m{text}\x1b[0m")
    }
}

/// Prints `text` on stdout as a highlighted warning (see [`highlight_warning`]); the colour
/// follows the same rules as the diagnostics labels: stdout is a terminal and NO_COLOR is unset.
pub fn println_warning(text: &str) {
    let cap =
        crate::diagnostics::detect_color_capability_for_stream(std::io::stdout().is_terminal());
    println!("{}", highlight_warning(text, cap));
}

/// The text of a success line: bold green when `cap` allows colour, otherwise the plain text.
pub fn highlight_success(text: &str, cap: crate::diagnostics::ColorCapability) -> String {
    if cap == crate::diagnostics::ColorCapability::Disabled {
        text.to_string()
    } else {
        format!("\x1b[1;32m{text}\x1b[0m")
    }
}

/// Prints `text` on stdout as a green success line, with the same colour rules as
/// [`println_warning`].
pub fn println_success(text: &str) {
    let cap =
        crate::diagnostics::detect_color_capability_for_stream(std::io::stdout().is_terminal());
    println!("{}", highlight_success(text, cap));
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

    const LOGOUT_WARNING: &str =
        "If the extension is not immediately active, log out and back in to complete loading.";

    fn escape_free(s: &str) -> bool {
        !s.contains('\x1b')
    }

    /// Not a terminal (piped): no escape codes, whatever NO_COLOR and TERM say.
    #[test]
    fn test_warning_has_no_escape_codes_when_piped() {
        let _env = crate::diagnostics::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let cap = crate::diagnostics::detect_color_capability_for_stream(false);
        assert_eq!(highlight_warning(LOGOUT_WARNING, cap), LOGOUT_WARNING);
    }

    /// A terminal with NO_COLOR set: no escape codes.
    #[test]
    fn test_warning_has_no_escape_codes_with_no_color() {
        let _env = crate::diagnostics::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("NO_COLOR");
        std::env::set_var("NO_COLOR", "1");
        let cap = crate::diagnostics::detect_color_capability_for_stream(true);
        match old {
            Some(v) => std::env::set_var("NO_COLOR", v),
            None => std::env::remove_var("NO_COLOR"),
        }
        let s = highlight_warning(LOGOUT_WARNING, cap);
        assert!(escape_free(&s), "{s:?}");
        assert_eq!(s, LOGOUT_WARNING);
    }

    /// A terminal without NO_COLOR: bold yellow (ESC[1;33m) around the unchanged text, reset after.
    #[test]
    fn test_warning_is_bold_yellow_when_colour_is_enabled() {
        let _env = crate::diagnostics::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("NO_COLOR");
        std::env::remove_var("NO_COLOR");
        let old_term = std::env::var_os("TERM");
        std::env::set_var("TERM", "xterm");
        let cap = crate::diagnostics::detect_color_capability_for_stream(true);
        if let Some(v) = old {
            std::env::set_var("NO_COLOR", v);
        }
        match old_term {
            Some(v) => std::env::set_var("TERM", v),
            None => std::env::remove_var("TERM"),
        }
        let s = highlight_warning(LOGOUT_WARNING, cap);
        assert_eq!(s, format!("\x1b[1;33m{LOGOUT_WARNING}\x1b[0m"));
    }

    /// The sentence the session end prints is exactly the documented one (no decoration in it).
    #[test]
    fn test_warning_text_is_unchanged() {
        assert_eq!(crate::session::LOGOUT_WARNING, LOGOUT_WARNING);
    }

    /// Every `=== ... ===` heading in the program goes through `Layout::section`: no source file
    /// other than ui.rs prints a cyan heading by hand (that is how the old double blank lines and
    /// the uncoloured-when-piped bug crept in).
    #[test]
    fn test_no_heading_is_printed_by_hand() {
        fn walk(dir: &std::path::Path, hits: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, hits);
                } else if p.extension().is_some_and(|x| x == "rs")
                    && !p.ends_with("ui.rs")
                    && std::fs::read_to_string(&p)
                        .unwrap()
                        .contains("\\x1b[1;36m===")
                {
                    hits.push(p.display().to_string());
                }
            }
        }
        let mut hits = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut hits,
        );
        assert!(hits.is_empty(), "headings printed by hand: {hits:?}");
    }

    /// The order of a clean install (removal sections, then the install sections) has one blank
    /// line between blocks and none doubled, at the start or at the end.
    #[test]
    fn test_clean_install_run_has_single_blank_lines() {
        let t = text(|l, o| {
            l.block_start(o).unwrap();
            writeln!(o, "An existing spawn-at was found:").unwrap();
            writeln!(o, "--clean-install: removing it first.").unwrap();
            l.block_start(o).unwrap();
            writeln!(o, "Clean install: removing the existing spawn-at first.").unwrap();
            l.section(o, "OS-Level Binary Uninstallation", false)
                .unwrap();
            writeln!(o, "Successfully removed binary: /x").unwrap();
            l.block_start(o).unwrap();
            writeln!(o, "Disabling GNOME extension: x").unwrap();
            writeln!(o, "Spawn-At extension successfully uninstalled.").unwrap();
            l.block_start(o).unwrap();
            writeln!(o, "Your configuration and settings were kept.").unwrap();
            l.section(o, "OS-Level Binary Installation", false).unwrap();
            writeln!(o, "Scope: User").unwrap();
            l.block_start(o).unwrap();
            writeln!(o, "Deploying embedded GNOME extension to: /y").unwrap();
        });
        assert!(!t.contains("\n\n\n"), "{t}");
        assert!(!t.starts_with('\n') && !t.ends_with("\n\n"), "{t}");
        assert!(
            t.contains("first.\n\n=== OS-Level Binary Uninstallation ===\n\nSuccessfully"),
            "{t}"
        );
    }

    /// The success line is bold green (ESC[1;32m) with colour, the plain text without.
    #[test]
    fn test_success_line_colour() {
        use crate::diagnostics::ColorCapability;
        assert_eq!(highlight_success("ok", ColorCapability::Disabled), "ok");
        assert_eq!(
            highlight_success("ok", ColorCapability::Basic),
            "\x1b[1;32mok\x1b[0m"
        );
    }

    /// Colour only changes the escape codes around the heading, not its text.
    #[test]
    fn test_heading_colour_is_only_escape_codes() {
        assert_eq!(heading("X", false), "=== X ===");
        assert_eq!(heading("X", true), "\x1b[1;36m=== X ===\x1b[0m");
    }
}
