//! # Linux XDG Desktop Entry & Wayland App ID Resolver
//!
//! Turns the program a user types (`gnome-text-editor`) into the id a window is expected to carry
//! (`org.gnome.TextEditor`). The rule here is: the id is the file name of the `.desktop` file
//! whose `Exec=` program is that command, without `.desktop`. That a window really reports that
//! id is an assumption of this module, not something it checks.
//!
//! It reads `.desktop` files from the user's XDG data directory, `$XDG_DATA_DIRS`, and the Flatpak
//! and Snap export directories. It reads only: no files are written and no process is started.
//! It does not talk to a compositor. The GNOME, Sway and X11 drivers call it from `resolve_id`
//! (`grep -rn resolve_linux_app_id src/`).

use std::fs;
use std::path::{Path, PathBuf};

/// The id to arm for `command_bin`: the `.desktop` file stem whose `Exec=` program has the same
/// base name, else the base name itself. Reads the disk (directory listings and file contents)
/// on every call; never fails.
///
/// In order, the first rule that applies wins:
/// 1. `explicit_class`, trimmed, when it is not empty after trimming (an all-blank value is
///    ignored). `"*"` passes through this way.
/// 2. `"*"` when the base name of `command_bin` is `"*"`.
/// 3. A built-in table: `gnome-terminal` gives `org.gnome.Terminal`. It is checked before any
///    file is read.
/// 4. The first match while scanning, in this order: `$HOME/.local/share/applications`,
///    `$HOME/.local/share/flatpak/exports/share/applications`, each `$XDG_DATA_DIRS` entry
///    (default `/usr/local/share:/usr/share`) plus `/applications`, then
///    `/var/lib/flatpak/exports/share/applications` and `/var/lib/snapd/desktop/applications`
///    when they exist. Within one directory the order is the one `read_dir` returns, which is
///    not defined, so two `.desktop` files with the same `Exec=` program can win in either order.
/// 5. The base name of `command_bin` (`/usr/bin/alacritty` gives `alacritty`). The wildcard is
///    never a fallback.
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

    // `gnome-terminal` is mapped by hand to the id of the terminal server. The extension also
    // pairs these ids in `_matchesArmKey`
    // (grep -n "gnome-terminal" assets/gnome/modern/extension.js).
    const KNOWN_WM_CLASS_OVERRIDES: &[(&str, &str)] = &[("gnome-terminal", "org.gnome.Terminal")];

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

/// The directories `resolve_linux_app_id` scans, in scan order, without duplicates of the
/// `$XDG_DATA_DIRS` entries. The two user directories and the `$XDG_DATA_DIRS` entries are
/// included whether or not they exist; the system Flatpak and Snap directories only if they do.
/// An unset `HOME` drops the two user directories; an unset `XDG_DATA_DIRS` uses the default.
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

/// `Some(file stem of path)` for the first `Exec=` line of `content` whose program has the base
/// name `bin_name`, else `None`. The program is the first word of the value that is not `env`
/// and does not contain `=`, with surrounding quotes removed. The whole file is searched, not
/// only the `[Desktop Entry]` group, so an `Exec=` of a desktop action counts too.
// The code does not handle "flatpak run <id>": the program is then `flatpak`, so such an entry
// matches only when the command typed is `flatpak`.
fn match_desktop_file(content: &str, path: &Path, bin_name: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(exec_part) = trimmed.strip_prefix("Exec=") {
            let exec_cmd = exec_part.trim();
            // Skip a leading `env` and any `VAR=VAL` words; the next word is the program.
            let mut candidate_bin = "";
            for token in exec_cmd.split_whitespace() {
                if token == "env" || token.contains('=') {
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
