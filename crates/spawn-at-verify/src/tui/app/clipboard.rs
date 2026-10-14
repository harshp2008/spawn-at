//! Clipboard provider integrations (OSC 52, wl-copy, xclip) and drawer export.

use super::state::TuiApp;
use std::path::PathBuf;

impl TuiApp {
    /// Copies formatted drawer content to the system clipboard or OSC 52 sequence.
    pub fn copy_drawer_text(&mut self) {
        if let Some(test) = self.selected_test().cloned() {
            let (_lines, plain) =
                crate::tui::views::build_drawer_content(self, &test, 80, !self.drawer_raw_view);
            let line_count = plain.lines().count();
            let res = if let Some(cb) = &self.clipboard_fn {
                cb(&plain)
            } else {
                system_copy_to_clipboard(&plain)
            };
            match res {
                Ok(()) => {
                    self.drawer_status = Some((
                        format!("Copied {} lines", line_count),
                        std::time::Instant::now(),
                    ));
                }
                Err(e) => {
                    self.drawer_status = Some((
                        format!("Copy failed: {}; use w", e),
                        std::time::Instant::now(),
                    ));
                }
            }
        }
    }

    /// Writes formatted drawer content to a debug dump file on disk.
    pub fn save_drawer_text(&mut self) -> Result<PathBuf, String> {
        if let Some(test) = self.selected_test().cloned() {
            let (_lines, plain) =
                crate::tui::views::build_drawer_content(self, &test, 80, !self.drawer_raw_view);
            let safe_id = test.id.replace('/', "-");
            let file_name = format!("{}.log.txt", safe_id);
            let path = self.report_dir.join(file_name);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &plain)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            self.drawer_status = Some((path.display().to_string(), std::time::Instant::now()));
            Ok(path)
        } else {
            Err("No test selected".into())
        }
    }
}

/// External clipboard providers tried in order: program and the arguments that make it read
/// the clipboard text from stdin.
const CLIPBOARD_PROVIDERS: &[(&str, &[&str])] = &[
    ("wl-copy", &[]),
    ("xclip", &["-selection", "clipboard"]),
    ("xsel", &["--clipboard", "--input"]),
];

/// Pipes `text` into `program` and succeeds only if the process exits with status 0.
///
/// A missing binary, a failed stdin write, a non-zero exit, or death by signal all return
/// `Err` naming `program`, so the caller can fall through to the next provider.
fn run_provider(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {}", program, e))?;
    let write_res = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(text.as_bytes()),
        None => Ok(()),
    };
    // Always reap the child, even when the write failed (e.g. EPIPE from an early exit).
    let status = child.wait().map_err(|e| format!("{}: {}", program, e))?;
    if !status.success() {
        return Err(format!("{} exited with {}", program, status));
    }
    write_res.map_err(|e| format!("{}: writing stdin failed: {}", program, e))
}

/// Tries each provider in order, then `osc52`; the first success wins.
///
/// The final error lists why every provider failed.
fn copy_via(
    providers: &[(&str, &[&str])],
    text: &str,
    osc52: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for (program, args) in providers {
        match run_provider(program, args, text) {
            Ok(()) => return Ok(()),
            Err(e) => failures.push(e),
        }
    }
    match osc52(text) {
        Ok(()) => Ok(()),
        Err(e) => {
            failures.push(format!("OSC 52: {}", e));
            Err(format!(
                "no clipboard provider succeeded ({})",
                failures.join("; ")
            ))
        }
    }
}

/// Invokes system clipboard tools (wl-copy, xclip, xsel) in turn, checking each exit status,
/// and falls back to OSC 52 when all of them fail.
pub fn system_copy_to_clipboard(text: &str) -> Result<(), String> {
    copy_via(CLIPBOARD_PROVIDERS, text, copy_osc52)
}

/// Emits an OSC 52 terminal escape sequence to populate the terminal clipboard.
fn copy_osc52(text: &str) -> Result<(), String> {
    use std::io::Write;
    let b64 = base64_encode(text.as_bytes());
    let osc = format!("\x1b]52;c;{}\x07", b64);
    let mut stdout = std::io::stdout();
    stdout
        .write_all(osc.as_bytes())
        .map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Encodes arbitrary byte slices into standard Base64 representation.
fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 3) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Unique scratch path under the OS temp dir for one test.
    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "spawn-at-verify-clip-{}-{}",
            std::process::id(),
            name
        ))
    }

    /// First provider is a fake that exits 3, second is a fake that stores stdin in a file:
    /// the failure must fall through to the second, and OSC 52 must never run.
    #[test]
    fn test_failing_provider_falls_through_to_next_provider() {
        let out = temp_path("next");
        let _ = std::fs::remove_file(&out);
        let script = format!("cat > {}", out.display());
        let providers: &[(&str, &[&str])] =
            &[("sh", &["-c", "exit 3"]), ("sh", &["-c", script.as_str()])];
        let osc_calls = Cell::new(0);
        let res = copy_via(providers, "hello clip", |_| {
            osc_calls.set(osc_calls.get() + 1);
            Ok(())
        });
        assert_eq!(res, Ok(()));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "hello clip");
        assert_eq!(osc_calls.get(), 0, "OSC 52 must not run after a success");
        let _ = std::fs::remove_file(&out);
    }

    /// Every provider fails (exit 1, and a binary that does not exist, like xsel here):
    /// OSC 52 must run exactly once with the original text and its result is returned.
    #[test]
    fn test_all_providers_failing_falls_back_to_osc52() {
        let providers: &[(&str, &[&str])] = &[
            ("sh", &["-c", "exit 1"]),
            ("spawn-at-verify-no-such-clipboard-tool", &[]),
        ];
        let seen = std::cell::RefCell::new(Vec::new());
        let res = copy_via(providers, "fallback text", |t| {
            seen.borrow_mut().push(t.to_string());
            Ok(())
        });
        assert_eq!(res, Ok(()));
        assert_eq!(*seen.borrow(), vec!["fallback text".to_string()]);
    }

    /// A provider that succeeds first short-circuits: a later provider that would write a
    /// marker file never runs.
    #[test]
    fn test_successful_provider_short_circuits_remaining_providers() {
        let marker = temp_path("marker");
        let _ = std::fs::remove_file(&marker);
        let script = format!("touch {}", marker.display());
        let providers: &[(&str, &[&str])] = &[
            ("sh", &["-c", "cat >/dev/null"]),
            ("sh", &["-c", script.as_str()]),
        ];
        let res = copy_via(providers, "x", |_| Err("osc must not run".into()));
        assert_eq!(res, Ok(()));
        assert!(!marker.exists(), "second provider must not run");
    }

    /// When providers and OSC 52 all fail, the error names each provider's failure reason.
    #[test]
    fn test_total_failure_reports_every_reason() {
        let providers: &[(&str, &[&str])] = &[
            ("sh", &["-c", "exit 7"]),
            ("spawn-at-verify-no-such-clipboard-tool", &[]),
        ];
        let err = copy_via(providers, "x", |_| Err("stdout closed".into())).unwrap_err();
        assert!(err.contains("sh exited with"), "{}", err);
        assert!(err.contains("exit status: 7"), "{}", err);
        assert!(
            err.contains("spawn-at-verify-no-such-clipboard-tool"),
            "{}",
            err
        );
        assert!(err.contains("OSC 52: stdout closed"), "{}", err);
    }

    /// The real provider table lists wl-copy, xclip, xsel in that order.
    #[test]
    fn test_provider_order_is_wl_copy_xclip_xsel() {
        let names: Vec<&str> = CLIPBOARD_PROVIDERS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["wl-copy", "xclip", "xsel"]);
    }
}
