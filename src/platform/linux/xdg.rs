//! # Linux XDG Desktop Entry & Wayland App ID Resolver
//!
//! ## Architectural Purpose
//!
//! In the Linux desktop ecosystem (GNOME, KDE, wlroots, X11):
//! - Applications are launched by binary name (e.g., `gnome-text-editor`, `google-chrome-stable`,
//!   or flatpak/snap commands).
//! - Wayland compositors (like Mutter) track surfaces and windows via FreeDesktop `app_id`
//!   strings (e.g., `org.gnome.TextEditor`, `com.spotify.Client`), derived from the filename
//!   of the `.desktop` file without the `.desktop` extension.
//!
//! This module scans standard XDG data paths, user directories, Flatpak exports, and Snap
//! desktop locations to resolve an executable binary to its canonical Wayland App ID.

use std::fs;
use std::path::{Path, PathBuf};

/// Resolves a Linux binary command into its canonical Wayland Application ID.
///
/// ### Search Order:
/// 1. **Explicit Override:** If `explicit_class` is provided (`Some(...)`), it is returned immediately.
/// 2. **XDG Applications Directories:** Scans:
///    - `$HOME/.local/share/applications`
///    - `$HOME/.local/share/flatpak/exports/share/applications`
///    - `$XDG_DATA_DIRS` (defaulting to `/usr/local/share:/usr/share`) + `/applications` subpaths
///    - System Flatpak exports (`/var/lib/flatpak/exports/share/applications`)
///    - Snap desktop entries (`/var/lib/snapd/desktop/applications`)
/// 3. **Desktop File Matching:** Matches the `Exec=` key against the invoked executable name.
/// 4. **Fallback:** Returns the binary's basename or wildcard `"*"` if no `.desktop` file matches.
pub fn resolve_linux_app_id(command_bin: &str, explicit_class: Option<&str>) -> String {
    // 1. Honor explicit user override
    if let Some(cls) = explicit_class {
        let trimmed = cls.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // Extract the base filename of the binary (e.g., "/usr/bin/alacritty" -> "alacritty")
    let bin_name = Path::new(command_bin)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command_bin);

    // If wildcard is passed, return it directly
    if bin_name == "*" {
        return "*".to_string();
    }

    // Direct mapping for daemonized/split binaries where Mutter's wm_class
    // diverges from the .desktop filename or command binary
    const KNOWN_WM_CLASS_OVERRIDES: &[(&str, &str)] = &[
        ("gnome-terminal", "org.gnome.Terminal"),
    ];

    for &(bin, target_class) in KNOWN_WM_CLASS_OVERRIDES {
        if bin_name == bin {
            return target_class.to_string();
        }
    }

    // 2. Build list of desktop entry search directories
    let search_dirs = collect_xdg_application_dirs();

    // 3. Scan `.desktop` files for matching Exec= entries
    for dir in search_dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Some(app_id) = match_desktop_file(&content, &path, bin_name) {
                            return app_id;
                        }
                    }
                }
            }
        }
    }

    // 4. Fallback to the raw binary name
    bin_name.to_string()
}

/// Collects all standard and containerized (Flatpak / Snap) application directories.
fn collect_xdg_application_dirs() -> Vec<PathBuf> {
    let mut search_dirs = Vec::new();

    // User-specific application directories
    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        search_dirs.push(home_path.join(".local/share/applications"));
        search_dirs.push(home_path.join(".local/share/flatpak/exports/share/applications"));
    }

    // Parse $XDG_DATA_DIRS (defaulting to /usr/local/share:/usr/share)
    let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

    for dir in xdg_data_dirs.split(':') {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            let app_dir = PathBuf::from(trimmed).join("applications");
            if !search_dirs.contains(&app_dir) {
                search_dirs.push(app_dir);
            }
        }
    }

    // Global Flatpak & Snap export paths
    let extra_dirs = [
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];

    for dir in extra_dirs {
        if !search_dirs.contains(&dir) && dir.exists() {
            search_dirs.push(dir);
        }
    }

    search_dirs
}

/// Helper to parse a desktop file and check if its `Exec=` line matches the target binary.
fn match_desktop_file(content: &str, path: &Path, bin_name: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(exec_part) = trimmed.strip_prefix("Exec=") {
            let exec_cmd = exec_part.trim();
            // Handle optional "env VAR=VAL ... <binary>" or "flatpak run <id>" prefixes
            let tokens: Vec<&str> = exec_cmd.split_whitespace().collect();
            let mut iter = tokens.iter();

            let mut candidate_bin = "";
            while let Some(token) = iter.next() {
                if *token == "env" || token.contains('=') {
                    continue;
                }
                candidate_bin = token.trim_matches(|c| c == '"' || c == '\'');
                break;
            }

            let exec_bin_name = Path::new(candidate_bin)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(candidate_bin);

            if !exec_bin_name.is_empty() && exec_bin_name == bin_name {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    return Some(stem.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_class_override() {
        assert_eq!(
            resolve_linux_app_id("gnome-text-editor", Some("my.custom.App")),
            "my.custom.App"
        );
        assert_eq!(resolve_linux_app_id("alacritty", Some("*")), "*");
    }

    #[test]
    fn test_match_desktop_file_exec_parsing() {
        let sample = "\
[Desktop Entry]
Name=Text Editor
Exec=gnome-text-editor %U
Type=Application
";
        let path = Path::new("/usr/share/applications/org.gnome.TextEditor.desktop");
        let result = match_desktop_file(sample, path, "gnome-text-editor");
        assert_eq!(result, Some("org.gnome.TextEditor".to_string()));
    }

    #[test]
    fn test_match_desktop_file_with_env() {
        let sample = "\
[Desktop Entry]
Name=Custom Tool
Exec=env GDK_BACKEND=wayland custom-tool --flag
Type=Application
";
        let path = Path::new("/usr/share/applications/com.example.CustomTool.desktop");
        let result = match_desktop_file(sample, path, "custom-tool");
        assert_eq!(result, Some("com.example.CustomTool".to_string()));
    }

    #[test]
    fn test_collect_xdg_dirs_defaults() {
        let dirs = collect_xdg_application_dirs();
        assert!(!dirs.is_empty());
    }
}
