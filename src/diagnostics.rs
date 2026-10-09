//! # Centralized Diagnostics Layer
//!
//! Provides typed diagnostic messages and a unified reporting pipeline for both
//! `spawn` and `transform` commands across all platforms.

use spawn_at_core::geometry::{calculate_rect_from_placement, PlacementParams, Rect};
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    Disabled,
    Basic,
    Color256,
    TrueColor,
}

/// Detects terminal color capabilities with support for truecolor, 256color, ANSI fallback,
/// NO_COLOR, TERM=dumb, and TTY checks.
pub fn detect_color_capability_for_stream(is_terminal: bool) -> ColorCapability {
    if !is_terminal {
        return ColorCapability::Disabled;
    }
    if std::env::var_os("NO_COLOR").map_or(false, |v| !v.is_empty()) {
        return ColorCapability::Disabled;
    }
    if std::env::var("TERM").map_or(false, |t| t == "dumb") {
        return ColorCapability::Disabled;
    }
    if std::env::var("COLORTERM").map_or(false, |ct| ct == "truecolor" || ct == "24bit") {
        return ColorCapability::TrueColor;
    }
    if std::env::var("TERM").map_or(false, |t| t.contains("256color")) {
        return ColorCapability::Color256;
    }
    ColorCapability::Basic
}

/// Detects color capability on stderr.
pub fn detect_color_capability() -> ColorCapability {
    detect_color_capability_for_stream(std::io::stderr().is_terminal())
}

/// Formats the diagnostic prefix label according to the color capability.
pub fn format_label(level: DiagnosticLevel, cap: ColorCapability) -> String {
    match cap {
        ColorCapability::Disabled => match level {
            DiagnosticLevel::Info => "[spawn-at] INFO:".to_string(),
            DiagnosticLevel::Warning => "[spawn-at] WARNING:".to_string(),
            DiagnosticLevel::Error => "[spawn-at] ERROR:".to_string(),
        },
        ColorCapability::Basic => match level {
            DiagnosticLevel::Info => "\x1b[1;34m[spawn-at] INFO:\x1b[0m".to_string(),
            DiagnosticLevel::Warning => "\x1b[1;33m[spawn-at] WARNING:\x1b[0m".to_string(),
            DiagnosticLevel::Error => "\x1b[1;31m[spawn-at] ERROR:\x1b[0m".to_string(),
        },
        ColorCapability::Color256 => match level {
            DiagnosticLevel::Info => "\x1b[1;34m[spawn-at] INFO:\x1b[0m".to_string(),
            DiagnosticLevel::Warning => "\x1b[1;38;5;208m[spawn-at] WARNING:\x1b[0m".to_string(),
            DiagnosticLevel::Error => "\x1b[1;31m[spawn-at] ERROR:\x1b[0m".to_string(),
        },
        ColorCapability::TrueColor => match level {
            DiagnosticLevel::Info => "\x1b[1;34m[spawn-at] INFO:\x1b[0m".to_string(),
            DiagnosticLevel::Warning => "\x1b[1;38;2;255;140;0m[spawn-at] WARNING:\x1b[0m".to_string(),
            DiagnosticLevel::Error => "\x1b[1;31m[spawn-at] ERROR:\x1b[0m".to_string(),
        },
    }
}

/// Formats a complete labeled message given a level, message body, and color capability.
pub fn format_message_with_capability(level: DiagnosticLevel, msg: &str, cap: ColorCapability) -> String {
    format!("{} {}", format_label(level, cap), msg)
}

/// Renders a diagnostic message directly to stderr with color detection.
pub fn render_diagnostic_message(level: DiagnosticLevel, msg: &str) {
    eprintln!("{}", format_message_with_capability(level, msg, detect_color_capability()));
}

/// Centralized renderer for user-facing INFO diagnostics.
pub fn render_info(msg: &str) {
    render_diagnostic_message(DiagnosticLevel::Info, msg);
}

/// Centralized renderer for user-facing WARNING diagnostics.
pub fn render_warning(msg: &str) {
    render_diagnostic_message(DiagnosticLevel::Warning, msg);
}

/// Centralized renderer for user-facing ERROR diagnostics.
pub fn render_error(msg: &str) {
    render_diagnostic_message(DiagnosticLevel::Error, msg);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diagnostic {
    /// Window mapped/transformed successfully at target position and size.
    PlacedSuccess {
        pos: (i32, i32),
        size: (u32, u32),
    },
    /// The requested size was below toolkit minimums and was expanded.
    SizeRaisedToMinimum {
        requested: (u32, u32),
        minimum: Option<(u32, u32)>,
        actual: (u32, u32),
        placed_pos: (i32, i32),
    },
    /// Final size differs from requested size without a known toolkit minimum.
    FinalSizeDiffers {
        requested: (u32, u32),
        actual: (u32, u32),
        placed_pos: (i32, i32),
    },
    /// Window was not positioned at target, with cause if known.
    PositionNotReached {
        actual_pos: (i32, i32),
        actual_size: (u32, u32),
        expected_pos: (i32, i32),
        requested_size: Option<(u32, u32)>,
        clamped_to_workarea: bool,
    },
    /// Window placed off-screen because clamping was disabled (`--clamp false`).
    OffScreenUnclamped {
        actual_pos: (i32, i32),
        actual_size: (u32, u32),
    },
    /// Window never mapped within the timeout duration.
    WindowNeverMapped {
        app_hint: String,
        pid: u32,
        timeout_ms: u64,
    },
    /// The app reused a pre-existing window rather than creating a new one.
    ReusedExistingWindow {
        class: String,
        title: String,
        pid: Option<u32>,
    },
}

impl Diagnostic {
    pub fn level(&self) -> DiagnosticLevel {
        match self {
            Diagnostic::PlacedSuccess { .. } => DiagnosticLevel::Info,
            Diagnostic::SizeRaisedToMinimum { .. } => DiagnosticLevel::Info,
            Diagnostic::FinalSizeDiffers { .. } => DiagnosticLevel::Info,
            Diagnostic::PositionNotReached { .. } => DiagnosticLevel::Warning,
            Diagnostic::OffScreenUnclamped { .. } => DiagnosticLevel::Warning,
            Diagnostic::WindowNeverMapped { .. } => DiagnosticLevel::Warning,
            Diagnostic::ReusedExistingWindow { .. } => DiagnosticLevel::Warning,
        }
    }

    pub fn message_body(&self) -> String {
        match self {
            Diagnostic::PlacedSuccess { pos, size } => {
                format!(
                    "Window mapped at final size ({}x{}) and positioned at ({}, {}).",
                    size.0, size.1, pos.0, pos.1
                )
            }
            Diagnostic::SizeRaisedToMinimum {
                requested,
                minimum,
                actual,
                placed_pos,
            } => {
                if let Some(min) = minimum {
                    format!(
                        "Requested size ({}x{}), window minimum is {}x{}; placed at ({}, {}) [{}x{}].",
                        requested.0, requested.1, min.0, min.1, placed_pos.0, placed_pos.1, actual.0, actual.1
                    )
                } else {
                    format!(
                        "Requested size ({}x{}), final size is {}x{}; placed at ({}, {}) [{}x{}].",
                        requested.0, requested.1, actual.0, actual.1, placed_pos.0, placed_pos.1, actual.0, actual.1
                    )
                }
            }
            Diagnostic::FinalSizeDiffers {
                requested,
                actual,
                placed_pos,
            } => {
                format!(
                    "Requested size ({}x{}), final size is {}x{}; placed at ({}, {}) [{}x{}].",
                    requested.0, requested.1, actual.0, actual.1, placed_pos.0, placed_pos.1, actual.0, actual.1
                )
            }
            Diagnostic::PositionNotReached {
                actual_pos,
                actual_size,
                expected_pos,
                requested_size,
                clamped_to_workarea,
            } => {
                let mut msg = format!(
                    "Window mapped at ({}, {}) [{}x{}] but expected position was ({}, {}).",
                    actual_pos.0, actual_pos.1, actual_size.0, actual_size.1, expected_pos.0, expected_pos.1
                );
                if *clamped_to_workarea {
                    if let Some(req) = requested_size {
                        if req.0 != actual_size.0 || req.1 != actual_size.1 {
                            msg.push_str(&format!(
                                " Final size differs from the requested size ({}x{}); clamped to the work area.",
                                req.0, req.1
                            ));
                            return msg;
                        }
                    }
                    msg.push_str(" Clamped to the work area.");
                } else {
                    msg.push_str(" Window was not positioned at target.");
                }
                msg
            }
            Diagnostic::OffScreenUnclamped {
                actual_pos,
                actual_size,
            } => {
                format!(
                    "Window positioned off-screen at ({}, {}) [{}x{}] (--clamp false).",
                    actual_pos.0, actual_pos.1, actual_size.0, actual_size.1
                )
            }
            Diagnostic::WindowNeverMapped {
                app_hint,
                pid,
                timeout_ms,
            } => {
                format!(
                    "Timed out waiting for window to map: Window matching app hint '{}' (PID: {}) did not appear within {}ms.",
                    app_hint, pid, timeout_ms
                )
            }
            Diagnostic::ReusedExistingWindow {
                class,
                title,
                pid,
            } => {
                let pid_str = pid.map(|p| format!(" (PID: {})", p)).unwrap_or_default();
                format!(
                    "Application reused an existing window{}: class '{}', title '{}'. Spawn-at does not reposition pre-existing windows on spawn.",
                    pid_str, class, title
                )
            }
        }
    }

    pub fn format_message_with_capability(&self, cap: ColorCapability) -> String {
        format_message_with_capability(self.level(), &self.message_body(), cap)
    }

    pub fn format_message(&self) -> String {
        self.format_message_with_capability(detect_color_capability())
    }
}

pub fn render_diagnostic(diag: &Diagnostic) {
    render_diagnostic_message(diag.level(), &diag.message_body());
}

/// Evaluates a window's final placed geometry against the desired placement parameters,
/// returning the appropriate diagnostics.
pub fn evaluate_placement_diagnostics(
    params: &PlacementParams,
    actual_rect: Rect,
    min_size: Option<(u32, u32)>,
    size_raised: bool,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    // Calculate expected position based on requested parameters
    let expected_for_requested = calculate_rect_from_placement(params, actual_rect.width, actual_rect.height);
    let pos_reached_for_requested = (actual_rect.x - expected_for_requested.x).abs() <= 1
        && (actual_rect.y - expected_for_requested.y).abs() <= 1;

    // Also calculate expected position based on actual settled size (e.g. for anchored windows raised to minimum)
    let mut params_for_actual = params.clone();
    params_for_actual.size = Some((actual_rect.width, actual_rect.height));
    let expected_for_actual = calculate_rect_from_placement(&params_for_actual, actual_rect.width, actual_rect.height);

    let is_maximized_like = actual_rect.width >= params.workarea.width && actual_rect.height >= params.workarea.height;
    let pos_reached_for_actual = !is_maximized_like
        && (actual_rect.x - expected_for_actual.x).abs() <= 1
        && (actual_rect.y - expected_for_actual.y).abs() <= 1;

    let pos_reached = pos_reached_for_requested || pos_reached_for_actual;

    let size_differs = params.size.map_or(false, |req| {
        req.0 != actual_rect.width || req.1 != actual_rect.height
    });

    if pos_reached {
        if size_differs {
            let requested = params.size.unwrap();
            if size_raised || min_size.is_some() {
                diags.push(Diagnostic::SizeRaisedToMinimum {
                    requested,
                    minimum: min_size.or(Some((actual_rect.width, actual_rect.height))),
                    actual: (actual_rect.width, actual_rect.height),
                    placed_pos: (actual_rect.x, actual_rect.y),
                });
            } else {
                diags.push(Diagnostic::FinalSizeDiffers {
                    requested,
                    actual: (actual_rect.width, actual_rect.height),
                    placed_pos: (actual_rect.x, actual_rect.y),
                });
            }
        } else {
            diags.push(Diagnostic::PlacedSuccess {
                pos: (actual_rect.x, actual_rect.y),
                size: (actual_rect.width, actual_rect.height),
            });
        }
    } else {
        // Position was not reached
        let is_off_screen = !params.clamp && (actual_rect.x < params.workarea.x || actual_rect.y < params.workarea.y);
        if is_off_screen {
            diags.push(Diagnostic::OffScreenUnclamped {
                actual_pos: (actual_rect.x, actual_rect.y),
                actual_size: (actual_rect.width, actual_rect.height),
            });
        } else {
            diags.push(Diagnostic::PositionNotReached {
                actual_pos: (actual_rect.x, actual_rect.y),
                actual_size: (actual_rect.width, actual_rect.height),
                expected_pos: (expected_for_requested.x, expected_for_requested.y),
                requested_size: params.size,
                clamped_to_workarea: params.clamp,
            });
        }
    }

    diags
}

/// Unified entrypoint called by both `spawn` and `transform` after applying placement.
pub fn verify_and_report_placement(
    params: &PlacementParams,
    actual_rect: Rect,
    min_size: Option<(u32, u32)>,
    size_raised: bool,
) {
    let diags = evaluate_placement_diagnostics(params, actual_rect, min_size, size_raised);
    for diag in diags {
        render_diagnostic(&diag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spawn_at_core::geometry::Anchor;

    fn sample_workarea() -> Rect {
        Rect {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
        }
    }

    #[test]
    fn test_diagnostic_size_below_minimum() {
        let params = PlacementParams {
            anchor: Some(Anchor::BottomRight),
            size: Some((30, 20)),
            margin: 16,
            workarea: sample_workarea(),
            clamp: true,
            ..Default::default()
        };

        // Window placed at bottom right (1542, 934) expanded to minimum 362x130
        let actual = Rect {
            x: 1542,
            y: 934,
            width: 362,
            height: 130,
        };

        let diags = evaluate_placement_diagnostics(&params, actual, Some((362, 130)), true);
        assert_eq!(diags.len(), 1);
        match &diags[0] {
            Diagnostic::SizeRaisedToMinimum {
                requested,
                minimum,
                actual,
                placed_pos,
            } => {
                assert_eq!(*requested, (30, 20));
                assert_eq!(*minimum, Some((362, 130)));
                assert_eq!(*actual, (362, 130));
                assert_eq!(*placed_pos, (1542, 934));
            }
            other => panic!("Expected SizeRaisedToMinimum, got {:?}", other),
        }

        let msg = diags[0].format_message();
        assert!(msg.contains("Requested size (30x20), window minimum is 362x130; placed at (1542, 934) [362x130]"));
    }

    #[test]
    fn test_diagnostic_position_mismatch_clamped() {
        let params = PlacementParams {
            pos: Some((100, 1000)),
            size: Some((30, 30)),
            workarea: sample_workarea(),
            clamp: true,
            ..Default::default()
        };

        // Window clamped to (0, 40) at 1920x1040 (maximized)
        let actual = Rect {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
        };

        let diags = evaluate_placement_diagnostics(&params, actual, None, false);
        assert_eq!(diags.len(), 1);
        match &diags[0] {
            Diagnostic::PositionNotReached {
                actual_pos,
                actual_size,
                expected_pos,
                requested_size,
                clamped_to_workarea,
            } => {
                assert_eq!(*actual_pos, (0, 40));
                assert_eq!(*actual_size, (1920, 1040));
                assert_eq!(*expected_pos, (100, 1000));
                assert_eq!(*requested_size, Some((30, 30)));
                assert!(*clamped_to_workarea);
            }
            other => panic!("Expected PositionNotReached, got {:?}", other),
        }

        let msg = diags[0].format_message();
        assert!(msg.contains("Final size differs from the requested size (30x30); clamped to the work area."));
    }

    #[test]
    fn test_diagnostic_true_timeout() {
        let diag = Diagnostic::WindowNeverMapped {
            app_hint: "org.gnome.Terminal".into(),
            pid: 1234,
            timeout_ms: 2000,
        };
        let msg = diag.format_message();
        assert!(msg.contains("Timed out waiting for window to map"));
        assert!(msg.contains("org.gnome.Terminal"));
        assert!(msg.contains("PID: 1234"));
    }

    #[test]
    fn test_diagnostic_reused_existing_window() {
        let diag = Diagnostic::ReusedExistingWindow {
            class: "gnome-text-editor".into(),
            title: "Text Editor".into(),
            pid: Some(5555),
        };
        let msg = diag.format_message();
        assert!(msg.contains("Application reused an existing window (PID: 5555)"));
    }

    #[test]
    fn test_diagnostic_offscreen_unclamped() {
        let params = PlacementParams {
            pos: Some((-500, -200)),
            size: Some((800, 600)),
            workarea: sample_workarea(),
            clamp: false,
            ..Default::default()
        };

        let actual = Rect {
            x: -500,
            y: -200,
            width: 800,
            height: 600,
        };

        // When clamp is false and expected is (-500, -200)
        let expected = calculate_rect_from_placement(&params, actual.width, actual.height);
        assert_eq!(expected.x, -500);
        assert_eq!(expected.y, -200);

        // Position was reached at requested offscreen coordinates
        let diags = evaluate_placement_diagnostics(&params, actual, None, false);
        assert_eq!(diags.len(), 1);
        assert!(matches!(diags[0], Diagnostic::PlacedSuccess { .. }));
    }

    #[test]
    fn test_spawn_and_transform_diagnostics_parity() {
        // Both spawn and transform commands invoke evaluate_placement_diagnostics.
        // Verify that an identical placement situation produces identical diagnostics.
        let params = PlacementParams {
            anchor: Some(Anchor::Center),
            size: Some((800, 600)),
            workarea: sample_workarea(),
            clamp: true,
            ..Default::default()
        };

        let actual = Rect {
            x: 560,
            y: 260,
            width: 800,
            height: 600,
        };

        let diags_spawn = evaluate_placement_diagnostics(&params, actual, None, false);
        let diags_transform = evaluate_placement_diagnostics(&params, actual, None, false);

        assert_eq!(diags_spawn, diags_transform);
        assert_eq!(diags_spawn.len(), 1);
        assert_eq!(diags_spawn[0].level(), DiagnosticLevel::Info);
        assert_eq!(
            diags_spawn[0].format_message_with_capability(ColorCapability::TrueColor),
            diags_transform[0].format_message_with_capability(ColorCapability::TrueColor)
        );
        assert_eq!(
            diags_spawn[0].format_message_with_capability(ColorCapability::Disabled),
            diags_transform[0].format_message_with_capability(ColorCapability::Disabled)
        );
    }

    #[test]
    fn test_renderer_snapshot_color_on_exact_escapes() {
        // INFO: bold blue \x1b[1;34m
        let info_msg = format_message_with_capability(
            DiagnosticLevel::Info,
            "Requested size (30x20) is below toolkit minimums; window will expand.",
            ColorCapability::TrueColor,
        );
        assert_eq!(
            info_msg,
            "\x1b[1;34m[spawn-at] INFO:\x1b[0m Requested size (30x20) is below toolkit minimums; window will expand."
        );

        // WARNING (TrueColor): 24-bit orange \x1b[1;38;2;255;140;0m
        let warn_truecolor = format_message_with_capability(
            DiagnosticLevel::Warning,
            "Window is oversized (2000x1200); bottom and right margins ignored.",
            ColorCapability::TrueColor,
        );
        assert_eq!(
            warn_truecolor,
            "\x1b[1;38;2;255;140;0m[spawn-at] WARNING:\x1b[0m Window is oversized (2000x1200); bottom and right margins ignored."
        );

        // WARNING (256-color): 208 orange \x1b[1;38;5;208m
        let warn_256 = format_message_with_capability(
            DiagnosticLevel::Warning,
            "Window is oversized (2000x1200); bottom and right margins ignored.",
            ColorCapability::Color256,
        );
        assert_eq!(
            warn_256,
            "\x1b[1;38;5;208m[spawn-at] WARNING:\x1b[0m Window is oversized (2000x1200); bottom and right margins ignored."
        );

        // WARNING (Basic 16-color fallback): ANSI yellow \x1b[1;33m
        let warn_basic = format_message_with_capability(
            DiagnosticLevel::Warning,
            "Window is oversized (2000x1200); bottom and right margins ignored.",
            ColorCapability::Basic,
        );
        assert_eq!(
            warn_basic,
            "\x1b[1;33m[spawn-at] WARNING:\x1b[0m Window is oversized (2000x1200); bottom and right margins ignored."
        );

        // ERROR: bold red \x1b[1;31m
        let error_msg = format_message_with_capability(
            DiagnosticLevel::Error,
            "No active window matched criteria.",
            ColorCapability::TrueColor,
        );
        assert_eq!(
            error_msg,
            "\x1b[1;31m[spawn-at] ERROR:\x1b[0m No active window matched criteria."
        );
    }

    #[test]
    fn test_renderer_snapshot_color_off() {
        let info_msg = format_message_with_capability(
            DiagnosticLevel::Info,
            "Requested size (30x20) is below toolkit minimums; window will expand.",
            ColorCapability::Disabled,
        );
        assert_eq!(
            info_msg,
            "[spawn-at] INFO: Requested size (30x20) is below toolkit minimums; window will expand."
        );

        let warn_msg = format_message_with_capability(
            DiagnosticLevel::Warning,
            "Window is oversized (2000x1200); bottom and right margins ignored.",
            ColorCapability::Disabled,
        );
        assert_eq!(
            warn_msg,
            "[spawn-at] WARNING: Window is oversized (2000x1200); bottom and right margins ignored."
        );

        let error_msg = format_message_with_capability(
            DiagnosticLevel::Error,
            "No active window matched criteria.",
            ColorCapability::Disabled,
        );
        assert_eq!(
            error_msg,
            "[spawn-at] ERROR: No active window matched criteria."
        );
    }

    #[test]
    fn test_not_a_tty_disables_color() {
        assert_eq!(detect_color_capability_for_stream(false), ColorCapability::Disabled);
    }

    #[test]
    fn test_no_color_environment_variable() {
        // Test helper using custom stream capability logic
        let orig_no_color = std::env::var("NO_COLOR").ok();
        let orig_term = std::env::var("TERM").ok();

        std::env::set_var("NO_COLOR", "1");
        assert_eq!(detect_color_capability_for_stream(true), ColorCapability::Disabled);

        std::env::remove_var("NO_COLOR");
        std::env::set_var("TERM", "dumb");
        assert_eq!(detect_color_capability_for_stream(true), ColorCapability::Disabled);

        // Restore environment
        match orig_no_color {
            Some(v) => std::env::set_var("NO_COLOR", v),
            None => std::env::remove_var("NO_COLOR"),
        }
        match orig_term {
            Some(v) => std::env::set_var("TERM", v),
            None => std::env::remove_var("TERM"),
        }
    }

    #[test]
    fn test_no_raw_spawn_at_prints_outside_renderer() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let src_dir = manifest_dir.join("src");
        let mut violations = Vec::new();

        fn check_dir(dir: &std::path::Path, violations: &mut Vec<String>) {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    check_dir(&path, violations);
                } else if path.extension().map_or(false, |ext| ext == "rs") {
                    if path.file_name().map_or(false, |name| name == "diagnostics.rs") {
                        continue;
                    }
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    for (line_idx, line) in content.lines().enumerate() {
                        let trimmed = line.trim();
                        // Ignore SPAWN_AT_DEBUG macro calls and definitions
                        if trimmed.starts_with("//")
                            || trimmed.contains("x11_debug!")
                            || trimmed.contains("SPAWN_AT_DEBUG")
                        {
                            continue;
                        }
                        if (trimmed.contains("eprintln!")
                            || trimmed.contains("println!")
                            || trimmed.contains("eprint!")
                            || trimmed.contains("print!"))
                            && trimmed.contains("[spawn-at]")
                        {
                            violations.push(format!(
                                "{}:{}: {}",
                                path.strip_prefix(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
                                    .unwrap_or(&path)
                                    .display(),
                                line_idx + 1,
                                trimmed
                            ));
                        }
                    }
                }
            }
        }

        check_dir(&src_dir, &mut violations);
        assert!(
            violations.is_empty(),
            "Found raw '[spawn-at]' print calls outside src/diagnostics.rs:\n{}",
            violations.join("\n")
        );
    }
}

