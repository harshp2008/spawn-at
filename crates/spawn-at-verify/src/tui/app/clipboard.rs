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

/// Invokes system clipboard tools (wl-copy, xclip, xsel) or falls back to OSC 52.
pub fn system_copy_to_clipboard(text: &str) -> Result<(), String> {
    // 1. Try wl-copy
    if let Ok(mut child) = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 2. Try xclip (-selection clipboard)
    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 3. Try xsel (--clipboard --input)
    if let Ok(mut child) = std::process::Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(text.as_bytes());
        }
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    // 4. Try OSC 52
    if copy_osc52(text).is_ok() {
        return Ok(());
    }

    Err("no clipboard provider (wl-copy, xclip, xsel, OSC 52) succeeded".into())
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
