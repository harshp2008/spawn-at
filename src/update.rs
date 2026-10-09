//! # Update Manager & Release Checker
//!
//! Handles manual upgrades (`spawn-at update`), periodic 24-hour background release checks,
//! semver-based release filtering, and update notifications.

use crate::config::{current_timestamp_secs, Config};
use clap::Args;
use dialoguer::Select;
use serde::Deserialize;
use std::fs;
use std::io::IsTerminal;
use std::process::Command;

#[macro_export]
macro_rules! update_debug {
    ($($arg:tt)*) => {
        if std::env::var("SPAWN_AT_DEBUG").map(|v| v == "1" || v == "true").unwrap_or(false) {
            eprintln!($($arg)*);
        }
    };
}

/// Command-line arguments for the `spawn-at update` subcommand.
#[derive(Args, Debug, Clone, Default)]
pub struct UpdateArgs {
    /// Only check GitHub for available updates without downloading or installing
    #[arg(long)]
    pub check: bool,

    /// Override the configured release channel: 'stable' (official) or 'beta' (includes pre-releases)
    #[arg(long)]
    pub channel: Option<String>,

    /// Force re-download and reinstall even if already on the latest version
    #[arg(long)]
    pub force: bool,

    /// Run non-interactively without prompting
    #[arg(long)]
    pub headless: bool,
}

/// GitHub release representation from the GitHub REST API.
#[derive(Deserialize, Debug, Clone)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: Vec<GitHubAsset>,
}

/// Asset attached to a GitHub release.
#[derive(Deserialize, Debug, Clone)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// Returns the dynamic version string formatted for `spawn-at -V` / `--version`.
pub fn get_version_display() -> String {
    if let Some(ver) = option_env!("SPAWN_AT_VERSION") {
        format!("spawn-at {}", ver)
    } else {
        let commit_id = option_env!("SPAWN_AT_COMMIT_ID").unwrap_or("unknown");
        let last_release = option_env!("SPAWN_AT_LAST_RELEASE").unwrap_or("v0.1.0");
        format!(
            "DEV BUILD\nlast release was {}\n-----------------------------------------------------------------\nPrevious Commit ID: {}",
            last_release, commit_id
        )
    }
}

/// Returns the normalized version tag representing the currently running binary.
pub fn get_current_release_tag() -> String {
    if let Some(ver) = option_env!("SPAWN_AT_VERSION") {
        let clean = ver.trim().trim_start_matches('v');
        format!("v{}", clean)
    } else {
        "dev".to_string()
    }
}

/// Parses a version string into a `semver::Version` after stripping leading 'v'.
pub fn parse_semver(v: &str) -> Option<semver::Version> {
    let clean = v.trim().trim_start_matches('v');
    semver::Version::parse(clean).ok()
}

/// Returns the parsed semver version of the currently running build if parseable.
pub fn get_running_semver() -> Option<semver::Version> {
    if let Some(ver) = option_env!("SPAWN_AT_VERSION") {
        parse_semver(ver)
    } else {
        None
    }
}

/// Normalizes and validates the configured release channel.
pub fn normalize_channel(ch: &str) -> &'static str {
    match ch.trim().to_ascii_lowercase().as_str() {
        "stable" => "stable",
        "beta" => "beta",
        other => {
            update_debug!("[spawn-at-debug] Unrecognized or missing channel '{}', defaulting to 'stable'", other);
            "stable"
        }
    }
}

/// Checks whether an upgrade from `current` to `target` traverses v0.1.x to v0.2.x+.
pub fn is_v01_to_v02_upgrade(current: &semver::Version, target: &semver::Version) -> bool {
    current.major == 0 && current.minor == 1 && target.major == 0 && target.minor >= 2
}

/// Identifies the best eligible update candidate according to channel rules and SemVer 2.0.0.
pub fn find_update_candidate<'a>(
    releases: &'a [GitHubRelease],
    channel: &str,
    running_version: Option<&str>,
) -> Option<(&'a GitHubRelease, semver::Version)> {
    let running_semver = match running_version.and_then(parse_semver) {
        Some(v) => v,
        None => {
            update_debug!("[spawn-at-debug] Running build version is not parseable semver; suppressing update check");
            return None;
        }
    };

    let effective_channel = normalize_channel(channel);

    let mut eligible: Vec<(&'a GitHubRelease, semver::Version)> = Vec::new();

    for release in releases {
        if release.draft {
            continue;
        }
        if effective_channel == "stable" && release.prerelease {
            continue;
        }
        let semver_val = match parse_semver(&release.tag_name) {
            Some(v) => v,
            None => continue,
        };

        if semver_val > running_semver {
            eligible.push((release, semver_val));
        }
    }

    eligible.into_iter().max_by(|a, b| a.1.cmp(&b.1))
}

/// Queries GitHub `/releases/latest` for the newest official stable release.
pub fn fetch_github_latest_release() -> Result<Option<GitHubRelease>, String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "User-Agent: spawn-at",
            "https://api.github.com/repos/harshp2008/spawn-at/releases/latest",
        ])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "GitHub API query failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let release: GitHubRelease = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse GitHub release response: {}", e))?;

    Ok(Some(release))
}

/// Queries the GitHub API for releases list of `harshp2008/spawn-at`.
pub fn fetch_github_releases() -> Result<Vec<GitHubRelease>, String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "User-Agent: spawn-at",
            "https://api.github.com/repos/harshp2008/spawn-at/releases",
        ])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "GitHub API query failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let releases: Vec<GitHubRelease> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse GitHub releases response: {}", e))?;

    Ok(releases)
}

/// Resolves the candidate release based on the configured channel and current build version.
pub fn resolve_target_release(channel: &str, running_ver_str: Option<&str>) -> Result<Option<GitHubRelease>, String> {
    let effective_channel = normalize_channel(channel);
    let releases = if effective_channel == "stable" {
        match fetch_github_latest_release() {
            Ok(Some(r)) => vec![r],
            Ok(None) => vec![],
            Err(e) => {
                // If /releases/latest fails, fallback to full releases list
                update_debug!("[spawn-at-debug] Latest release endpoint failed, falling back to /releases: {}", e);
                fetch_github_releases()?
            }
        }
    } else {
        fetch_github_releases()?
    };

    Ok(find_update_candidate(&releases, effective_channel, running_ver_str).map(|(r, _)| r.clone()))
}

/// Resolves the latest available release for the channel unconditionally (for force installations).
pub fn resolve_latest_release_for_channel(channel: &str) -> Result<Option<GitHubRelease>, String> {
    let effective_channel = normalize_channel(channel);
    if effective_channel == "stable" {
        let rel_opt = fetch_github_latest_release()?;
        if let Some(ref r) = rel_opt {
            if !r.draft && !r.prerelease && parse_semver(&r.tag_name).is_some() {
                return Ok(Some(r.clone()));
            }
        }
    }

    let releases = fetch_github_releases()?;
    let candidate = releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter(|r| if effective_channel == "stable" { !r.prerelease } else { true })
        .filter_map(|r| parse_semver(&r.tag_name).map(|v| (r, v)))
        .max_by(|a, b| a.1.cmp(&b.1))
        .map(|(r, _)| r);

    Ok(candidate)
}

/// Triggers a non-blocking background process to query GitHub if the cache is older than 24 hours.
pub fn trigger_background_check_if_needed(cfg: &mut Config) {
    if !cfg.update.auto_check {
        return;
    }

    let now = current_timestamp_secs();
    let elapsed = now.saturating_sub(cfg.update.last_check_time);

    // 24 hours = 86400 seconds
    if elapsed > 86400 {
        cfg.update.last_check_time = now;
        let _ = cfg.save();

        if let Ok(exe) = std::env::current_exe() {
            let _ = Command::new(exe)
                .args(["update", "--check", "--headless"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

use crate::diagnostics::ColorCapability;

/// Strips ANSI CSI color escape sequences from a string.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Computes visible character width in terminal columns (excluding ANSI escapes).
pub fn visible_width(s: &str) -> usize {
    strip_ansi(s).chars().count()
}

/// Checks whether the environment supports UTF-8 box-drawing characters.
pub fn supports_utf8() -> bool {
    let lang = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_uppercase();
    if lang == "C" || lang == "POSIX" {
        false
    } else {
        true
    }
}

/// Queries current terminal width in columns if available.
pub fn terminal_width() -> Option<usize> {
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(w) = cols.parse::<usize>() {
            if w > 0 {
                return Some(w);
            }
        }
    }
    #[cfg(unix)]
    unsafe {
        let mut winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if libc::ioctl(libc::STDERR_FILENO, libc::TIOCGWINSZ, &mut winsize) == 0 && winsize.ws_col > 0 {
            return Some(winsize.ws_col as usize);
        }
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) == 0 && winsize.ws_col > 0 {
            return Some(winsize.ws_col as usize);
        }
    }
    None
}

/// Formats the update banner in npm update-notifier style with exact multi-color hierarchy.
pub fn format_update_banner(
    current: &str,
    latest: &str,
    cap: ColorCapability,
    utf8: bool,
    term_width: Option<usize>,
) -> String {
    let clean_current = current.trim().trim_start_matches('v');
    let clean_latest = latest.trim().trim_start_matches('v');

    let cur_semver = parse_semver(clean_current);
    let lat_semver = parse_semver(clean_latest);

    let compat_note_str = if let (Some(ref c), Some(ref l)) = (cur_semver, lat_semver) {
        if is_v01_to_v02_upgrade(c, l) {
            Some("Note: The v0.2 CLI is not compatible with v0.1 commands")
        } else {
            None
        }
    } else {
        None
    };

    let arrow = if utf8 { "→" } else { "->" };

    let (line1, line2, compat_line) = match cap {
        ColorCapability::Disabled => {
            let l1 = format!("Update available  {} {} {}", clean_current, arrow, clean_latest);
            let l2 = "Run  spawn-at update  to update".to_string();
            let l3 = compat_note_str.map(|s| s.to_string());
            (l1, l2, l3)
        }
        ColorCapability::Basic => {
            let l1 = format!("Update available  \x1b[2m{}\x1b[0m {} \x1b[32m{}\x1b[0m", clean_current, arrow, clean_latest);
            let l2 = "Run  \x1b[36mspawn-at update\x1b[0m  to update".to_string();
            let l3 = compat_note_str.map(|s| format!("\x1b[33m{}\x1b[0m", s));
            (l1, l2, l3)
        }
        ColorCapability::Color256 => {
            let l1 = format!("Update available  \x1b[2m{}\x1b[0m {} \x1b[32m{}\x1b[0m", clean_current, arrow, clean_latest);
            let l2 = "Run  \x1b[36mspawn-at update\x1b[0m  to update".to_string();
            let l3 = compat_note_str.map(|s| format!("\x1b[38;5;208m{}\x1b[0m", s));
            (l1, l2, l3)
        }
        ColorCapability::TrueColor => {
            let l1 = format!("Update available  \x1b[2m{}\x1b[0m {} \x1b[32m{}\x1b[0m", clean_current, arrow, clean_latest);
            let l2 = "Run  \x1b[36mspawn-at update\x1b[0m  to update".to_string();
            let l3 = compat_note_str.map(|s| format!("\x1b[38;2;255;140;0m{}\x1b[0m", s));
            (l1, l2, l3)
        }
    };

    let w1 = visible_width(&line1);
    let w2 = visible_width(&line2);
    let w3 = compat_line.as_ref().map(|l| visible_width(l)).unwrap_or(0);
    let content_max = std::cmp::max(w1, std::cmp::max(w2, w3));
    let inner_width = content_max + 6; // 3 left spaces + 3 right spaces

    let total_box_width = inner_width + 4; // 2 indent spaces + 2 border columns

    // Fall back to plain text if terminal is narrower than box
    if let Some(tw) = term_width {
        if tw < total_box_width {
            let mut fallback = match cap {
                ColorCapability::Disabled => {
                    format!("Update available: {} {} {}\nRun 'spawn-at update' to update", clean_current, arrow, clean_latest)
                }
                _ => {
                    format!("Update available: \x1b[2m{}\x1b[0m {} \x1b[32m{}\x1b[0m\nRun \x1b[36mspawn-at update\x1b[0m to update", clean_current, arrow, clean_latest)
                }
            };
            if let Some(ref note) = compat_line {
                fallback.push('\n');
                fallback.push_str(note);
            }
            return fallback;
        }
    }

    let (tl, tr, bl, br, h, v) = if utf8 {
        ("╭", "╮", "╰", "╯", "─", "│")
    } else {
        ("+", "+", "+", "+", "-", "|")
    };

    let (border_col, border_rst) = match cap {
        ColorCapability::Disabled => ("", ""),
        _ => ("\x1b[33m", "\x1b[0m"),
    };

    let bar_left = format!("  {border_col}{v}{border_rst}   ");
    let bar_right = format!("{border_col}{v}{border_rst}\n");
    let empty_row = format!("  {border_col}{v}{border_rst}{}{border_col}{v}{border_rst}\n", " ".repeat(inner_width));

    let mut out = String::new();
    out.push_str(&format!("  {border_col}{tl}{}{tr}{border_rst}\n", h.repeat(inner_width)));
    out.push_str(&empty_row);

    let pad1 = inner_width.saturating_sub(w1 + 3);
    out.push_str(&format!("{bar_left}{line1}{}{bar_right}", " ".repeat(pad1)));

    let pad2 = inner_width.saturating_sub(w2 + 3);
    out.push_str(&format!("{bar_left}{line2}{}{bar_right}", " ".repeat(pad2)));

    if let Some(ref line3) = compat_line {
        let pad3 = inner_width.saturating_sub(w3 + 3);
        out.push_str(&format!("{bar_left}{line3}{}{bar_right}", " ".repeat(pad3)));
    }

    out.push_str(&empty_row);
    out.push_str(&format!("  {border_col}{bl}{}{br}{border_rst}", h.repeat(inner_width)));

    out
}

/// Renders the npm update-notifier style boxed update banner at the bottom of standard command output.
pub fn render_boxed_notice(current: &str, latest: &str) {
    let cap = crate::diagnostics::detect_color_capability();
    let banner = format_update_banner(current, latest, cap, supports_utf8(), terminal_width());
    eprintln!("\n{}", banner);
}

/// Checks whether update notification should be rendered (stderr and stdout are TTYs, notify enabled).
pub fn should_render_update_notice(stderr_is_tty: bool, stdout_is_tty: bool, notify: bool) -> bool {
    stderr_is_tty && stdout_is_tty && notify
}

/// Hook called at the end of `main()` to render update notifications if appropriate.
pub fn render_update_notice_if_available() {
    // 1. Must be an interactive TTY on both stderr and stdout
    if !should_render_update_notice(std::io::stderr().is_terminal(), std::io::stdout().is_terminal(), true) {
        return;
    }

    // 2. Load configuration
    let mut cfg = Config::load_or_default();
    if !cfg.update.notify {
        return;
    }

    // 3. Trigger 24h background check if due
    trigger_background_check_if_needed(&mut cfg);

    // 4. Running version must be parseable semver
    let running_semver = match get_running_semver() {
        Some(v) => v,
        None => {
            update_debug!("[spawn-at-debug] Running build version is not parseable semver; suppressing update banner.");
            return;
        }
    };

    // 5. Look up cached candidate for current channel
    let channel = normalize_channel(&cfg.update.channel);
    let cached_tag = match cfg.update.cached_version_for_channel(channel) {
        Some(t) => t,
        None => return,
    };

    let cached_semver = match parse_semver(cached_tag) {
        Some(v) => v,
        None => return,
    };

    // 6. Compare strictly against running version at display time
    if cached_semver > running_semver {
        render_boxed_notice(&format!("{}", running_semver), cached_tag);
    }
}

/// Downloads and installs the specified release binary and updates extensions.
pub fn perform_upgrade(release: &GitHubRelease) -> Result<(), String> {
    let asset_name = "spawn-at-x86_64-unknown-linux-gnu.tar.gz";
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| format!("Release {} is missing pre-built binary asset '{}'", release.tag_name, asset_name))?;

    // Secure temporary directory with 0700 permissions
    let tmp_dir = std::env::temp_dir().join(format!("spawn-at-update-{}-{}", release.tag_name, std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create tmp dir: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp_dir, fs::Permissions::from_mode(0o700));
    }

    let tar_path = tmp_dir.join(asset_name);

    println!("\x1b[1;36mDownloading {}\x1b[0m", release.tag_name);
    let curl_status = Command::new("curl")
        .arg("-#")
        .arg("-L")
        .arg("-o")
        .arg(&tar_path)
        .arg(&asset.browser_download_url)
        .status()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !curl_status.success() {
        return Err("Download failed with non-zero exit code".to_string());
    }

    println!("\x1b[1;36mExtracting package...\x1b[0m");
    let tar_status = Command::new("tar")
        .arg("-xzf")
        .arg(&tar_path)
        .arg("--no-same-owner")
        .arg("-C")
        .arg(&tmp_dir)
        .status()
        .map_err(|e| format!("Failed to extract package: {}", e))?;

    if !tar_status.success() {
        return Err("Failed to extract release archive".to_string());
    }

    let new_binary = tmp_dir.join("spawn-at");
    if !new_binary.exists() {
        return Err("Extracted archive did not contain 'spawn-at' executable".to_string());
    }

    // Safety: Verify that the extracted binary is a regular file within tmp_dir
    let meta = fs::symlink_metadata(&new_binary)
        .map_err(|e| format!("Failed to inspect extracted binary: {}", e))?;
    if !meta.file_type().is_file() {
        return Err("Extracted 'spawn-at' is not a regular file".to_string());
    }

    // Deploy extensions from the new binary itself
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    if desktop.contains("GNOME") {
        println!("Deploying GNOME Shell extension via newly unpacked binary...");
        let ext_status = Command::new(&new_binary)
            .args(["install", "--skip-bin", "--headless"])
            .status();
        if let Err(e) = ext_status {
            crate::diagnostics::render_warning(&format!("Failed to install GNOME extension from new binary: {}", e));
        }
    }

    // Identify target installation path
    let current_exe = std::env::current_exe().map_err(|e| format!("Failed to locate current binary: {}", e))?;
    println!("Replacing binary at: {}", current_exe.display());

    // Replace the running binary atomically
    let backup_path = current_exe.with_extension("old");
    let _ = fs::rename(&current_exe, &backup_path);

    if let Err(e) = fs::copy(&new_binary, &current_exe) {
        // Rollback on failure
        let _ = fs::rename(&backup_path, &current_exe);
        return Err(format!("Failed to copy new binary to '{}': {}", current_exe.display(), e));
    }
    let _ = fs::remove_file(&backup_path);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755));
    }

    // Check session type for reload prompt
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    if desktop.contains("GNOME") {
        let is_wayland = crate::platform::linux::gnome::is_wayland_session();
        if std::io::stdout().is_terminal() {
            let prompt_choice = if is_wayland {
                let choices = &[
                    "No - Keep session running (I'll log out later if needed) [Default]",
                    "Yes - Log out now to ensure extension is cleanly loaded (Destructive: closes apps)",
                ];
                let choice = Select::new()
                    .with_prompt("Do you want to log out of your session now to complete the reload?")
                    .items(choices)
                    .default(0)
                    .interact();
                matches!(choice, Ok(1))
            } else {
                let choices = &[
                    "Yes - Reload GNOME Shell in-place now (Non-destructive: apps stay open) [Default]",
                    "No - Keep session running",
                ];
                let choice = Select::new()
                    .with_prompt("Do you want to reload GNOME Shell now? (Non-destructive on X11)")
                    .items(choices)
                    .default(0)
                    .interact();
                matches!(choice, Ok(0))
            };

            if prompt_choice {
                if is_wayland {
                    println!("Logging out to complete GNOME Shell extension upgrade...");
                    let _ = Command::new("gnome-session-quit").args(["--logout", "--no-prompt"]).spawn();
                } else {
                    crate::platform::linux::gnome::restart_gnome_shell_x11();
                }
            }
        }
    }

    // Clean up temporary directory
    let _ = fs::remove_dir_all(&tmp_dir);

    // Update cached versions in config
    let mut cfg = Config::load_or_default();
    if release.prerelease {
        cfg.update.cached_beta_version = Some(release.tag_name.clone());
    } else {
        cfg.update.cached_stable_version = Some(release.tag_name.clone());
    }
    cfg.update.last_check_time = current_timestamp_secs();
    let _ = cfg.save();

    println!("\x1b[1;32mSuccessfully upgraded spawn-at to {}!\x1b[0m", release.tag_name);
    Ok(())
}

/// Executes the `spawn-at update` command with interactive TUI or headless automation.
pub fn run_update(args: UpdateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load_or_default();
    let current_tag = get_current_release_tag();
    let running_semver = get_running_semver();
    let raw_channel = args.channel.as_deref().unwrap_or(&config.update.channel);
    let channel = normalize_channel(raw_channel);

    if args.headless && args.check {
        // Background check: update cached stable and beta versions silently
        if let Ok(releases) = fetch_github_releases() {
            let mut highest_stable: Option<(String, semver::Version)> = None;
            let mut highest_beta: Option<(String, semver::Version)> = None;

            for r in releases {
                if r.draft {
                    continue;
                }
                if let Some(v) = parse_semver(&r.tag_name) {
                    if r.prerelease {
                        if highest_beta.as_ref().map_or(true, |(_, bv)| v > *bv) {
                            highest_beta = Some((r.tag_name, v));
                        }
                    } else if highest_stable.as_ref().map_or(true, |(_, sv)| v > *sv) {
                        highest_stable = Some((r.tag_name, v));
                    }
                }
            }

            if let Some((stable_tag, _)) = highest_stable {
                config.update.cached_stable_version = Some(stable_tag);
            }
            if let Some((beta_tag, _)) = highest_beta {
                config.update.cached_beta_version = Some(beta_tag);
            }
            config.update.last_check_time = current_timestamp_secs();
            let _ = config.save();
        }
        return Ok(());
    }

    println!("\x1b[1;36m=== spawn-at Update Manager ===\x1b[0m\n");
    println!("Current version: {}", current_tag);
    println!("Checking GitHub for releases (Channel: {})...", channel);

    let running_semver_str = running_semver.as_ref().map(|v| format!("v{}", v));

    let candidate_opt = match resolve_target_release(channel, running_semver_str.as_deref()) {
        Ok(cand) => cand,
        Err(e) => {
            crate::diagnostics::render_error(&format!("Error checking updates: {}", e));
            return Ok(());
        }
    };

    if let Some(ref rel) = candidate_opt {
        if rel.prerelease {
            config.update.cached_beta_version = Some(rel.tag_name.clone());
        } else {
            config.update.cached_stable_version = Some(rel.tag_name.clone());
        }
        config.update.last_check_time = current_timestamp_secs();
        let _ = config.save();
    }

    if args.check {
        if let Some(ref rel) = candidate_opt {
            println!("An update is available: {} → {}", current_tag, rel.tag_name);
            if let (Some(cur_v), Some(tgt_v)) = (&running_semver, parse_semver(&rel.tag_name)) {
                if is_v01_to_v02_upgrade(cur_v, &tgt_v) {
                    println!("Note: spawn-at v0.2 CLI is not backward-compatible with v0.1 command syntax.");
                }
            }
        } else {
            println!("spawn-at is up to date ({}).", current_tag);
        }
        return Ok(());
    }

    let release_to_install = if let Some(rel) = candidate_opt {
        Some(rel)
    } else if args.force {
        match resolve_latest_release_for_channel(channel)? {
            Some(rel) => Some(rel),
            None => {
                println!("No release found for channel '{}'.", channel);
                return Ok(());
            }
        }
    } else {
        None
    };

    let release = match release_to_install {
        Some(r) => r,
        None => {
            if running_semver.is_none() {
                println!("Running build does not have a parseable release version (dev build).");
                println!("spawn-at update refuses to upgrade without an eligible candidate. Use --force to override.");
            } else {
                println!("spawn-at is up to date ({}).", current_tag);
                println!("You are already running the latest version for channel '{}'. Use --force to reinstall.", channel);
            }
            return Ok(());
        }
    };

    let rel_type = if release.prerelease { "Pre-release" } else { "Official Release" };
    println!("Selected version: {} ({})\n", release.tag_name, rel_type);

    if let (Some(cur_v), Some(tgt_v)) = (&running_semver, parse_semver(&release.tag_name)) {
        if is_v01_to_v02_upgrade(cur_v, &tgt_v) {
            println!("Note: spawn-at v0.2 CLI is not backward-compatible with v0.1 command syntax.\n");
        }
    }

    let has_update = running_semver.as_ref().map_or(false, |cur| {
        parse_semver(&release.tag_name).map_or(false, |tgt| tgt > *cur)
    });

    // Interactive TUI Menu
    if !args.headless && std::io::stdout().is_terminal() {
        let mut options = Vec::new();
        if has_update || args.force {
            options.push(format!("Upgrade to {} now [Recommended]", release.tag_name));
        } else {
            options.push(format!("Reinstall {} (Force download)", release.tag_name));
        }
        options.push(format!("Change update channel (Currently: {})", channel));
        options.push("Re-check GitHub for updates".to_string());
        options.push("Cancel".to_string());

        let selection = Select::new()
            .with_prompt("What would you like to do?")
            .items(&options)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                perform_upgrade(&release).map_err(Box::<dyn std::error::Error>::from)?;
            }
            1 => {
                let channels = &["Official releases only (stable) [Recommended]", "Include beta/pre-releases (beta)"];
                let ch_sel = Select::new()
                    .with_prompt("Select release channel:")
                    .items(channels)
                    .default(if channel == "beta" { 1 } else { 0 })
                    .interact()?;
                let new_ch = if ch_sel == 1 { "beta" } else { "stable" };
                config.update.channel = new_ch.to_string();
                config.save().map_err(Box::<dyn std::error::Error>::from)?;
                println!("Updated channel to '{}'. Re-checking GitHub for releases...\n", new_ch);
                return run_update(args);
            }
            2 => {
                println!("Re-checking GitHub...");
                return run_update(args);
            }
            _ => {
                println!("Update cancelled.");
            }
        }
    } else {
        // Headless execution
        if has_update || args.force {
            perform_upgrade(&release).map_err(Box::<dyn std::error::Error>::from)?;
        } else {
            println!("Already up to date. Use --force to reinstall.");
        }
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn mock_release(tag: &str, prerelease: bool, draft: bool) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.to_string(),
            name: Some(tag.to_string()),
            prerelease,
            draft,
            assets: vec![],
        }
    }

    #[test]
    fn test_stable_channel_remote_only_older_stable() {
        let releases = vec![mock_release("v0.1.0", false, false)];
        let cand = find_update_candidate(&releases, "stable", Some("0.2.0"));
        assert!(cand.is_none());
    }

    #[test]
    fn test_stable_channel_running_beta_remote_stable() {
        let releases = vec![mock_release("v0.2.0", false, false)];
        let cand = find_update_candidate(&releases, "stable", Some("0.2.0-beta.1"));
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().0.tag_name, "v0.2.0");
    }

    #[test]
    fn test_stable_channel_remote_prerelease_only() {
        let releases = vec![mock_release("v0.3.0-beta.1", true, false)];
        let cand = find_update_candidate(&releases, "stable", Some("0.2.0"));
        assert!(cand.is_none());
    }

    #[test]
    fn test_beta_channel_running_beta_remote_newer_beta() {
        let releases = vec![mock_release("v0.2.0-beta.2", true, false)];
        let cand = find_update_candidate(&releases, "beta", Some("0.2.0-beta.1"));
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().0.tag_name, "v0.2.0-beta.2");
    }

    #[test]
    fn test_beta_channel_remote_older_published_later_by_date() {
        // Remote has 0.1.1 published later by date (first entry in list)
        let releases = vec![
            mock_release("v0.1.1", false, false),
            mock_release("v0.1.0", false, false),
        ];
        let cand = find_update_candidate(&releases, "beta", Some("0.2.0-beta.1"));
        assert!(cand.is_none(), "0.1.1 is older than 0.2.0-beta.1, must not be offered");
    }

    #[test]
    fn test_draft_releases_ignored_on_both_channels() {
        let releases = vec![
            mock_release("v0.3.0", false, true),
            mock_release("v0.3.0-beta.1", true, true),
        ];
        assert!(find_update_candidate(&releases, "stable", Some("0.2.0")).is_none());
        assert!(find_update_candidate(&releases, "beta", Some("0.2.0")).is_none());
    }

    #[test]
    fn test_non_semver_tags_ignored() {
        let releases = vec![
            mock_release("pre-cleanup-beta", false, false),
            mock_release("nightly-build", true, false),
        ];
        assert!(find_update_candidate(&releases, "stable", Some("0.1.0")).is_none());
        assert!(find_update_candidate(&releases, "beta", Some("0.1.0")).is_none());
    }

    #[test]
    fn test_dev_label_build_no_banner() {
        let releases = vec![
            mock_release("v0.2.0", false, false),
            mock_release("v0.3.0-beta.1", true, false),
        ];
        assert!(find_update_candidate(&releases, "stable", None).is_none());
        assert!(find_update_candidate(&releases, "beta", None).is_none());
        assert!(find_update_candidate(&releases, "stable", Some("dev")).is_none());
        assert!(find_update_candidate(&releases, "beta", Some("dev-commit")).is_none());
    }

    #[test]
    fn test_missing_or_invalid_channel_defaults_to_stable() {
        let releases = vec![
            mock_release("v0.3.0-beta.1", true, false),
            mock_release("v0.2.0", false, false),
        ];
        // With invalid channel, defaults to stable -> ignores prerelease v0.3.0-beta.1, offers v0.2.0
        let cand = find_update_candidate(&releases, "unknown_channel", Some("0.1.0"));
        assert!(cand.is_some());
        assert_eq!(cand.unwrap().0.tag_name, "v0.2.0");

        let cand_empty = find_update_candidate(&releases, "", Some("0.1.0"));
        assert!(cand_empty.is_some());
        assert_eq!(cand_empty.unwrap().0.tag_name, "v0.2.0");
    }

    #[test]
    fn test_spawn_at_update_refuses_to_select_version_banner_would_not_offer() {
        let releases = vec![
            mock_release("v0.1.0", false, false),
            mock_release("v0.3.0-beta.1", true, false),
        ];
        // On stable channel with running 0.2.0, banner offers nothing
        let banner_cand = find_update_candidate(&releases, "stable", Some("0.2.0"));
        assert!(banner_cand.is_none());

        // Update candidate resolution must also offer nothing
        let update_cand = find_update_candidate(&releases, "stable", Some("0.2.0"));
        assert!(update_cand.is_none());
    }

    #[test]
    fn test_v01_to_v02_compatibility_note() {
        let v01 = parse_semver("0.1.0").unwrap();
        let v02 = parse_semver("0.2.0").unwrap();
        let v02_beta = parse_semver("0.2.0-beta.1").unwrap();
        let v03 = parse_semver("0.3.0").unwrap();

        assert!(is_v01_to_v02_upgrade(&v01, &v02));
        assert!(is_v01_to_v02_upgrade(&v01, &v02_beta));
        assert!(!is_v01_to_v02_upgrade(&v02_beta, &v02));
        assert!(!is_v01_to_v02_upgrade(&v02, &v03));
    }

    #[test]
    fn test_boxed_notice_rendering() {
        render_boxed_notice("v0.1.0", "v0.2.0");
    }

    #[test]
    fn test_banner_snapshot_color_on() {
        let banner = format_update_banner("0.2.0-beta.1", "0.2.0", ColorCapability::TrueColor, true, Some(80));

        // Exact escape sequences:
        // Yellow border: \x1b[33m
        assert!(banner.contains("\x1b[33m╭"), "Top left yellow corner");
        assert!(banner.contains("╮\x1b[0m"), "Top right yellow corner reset");
        assert!(banner.contains("\x1b[33m╰"), "Bottom left yellow corner");
        assert!(banner.contains("╯\x1b[0m"), "Bottom right yellow corner reset");

        // Old version dim: \x1b[2m
        assert!(banner.contains("\x1b[2m0.2.0-beta.1\x1b[0m"), "Old version dim");

        // Plain arrow: ' → ' (without escape surrounding it)
        assert!(banner.contains("\x1b[0m → \x1b[32m"), "Plain arrow between dim and green");

        // New version green: \x1b[32m
        assert!(banner.contains("\x1b[32m0.2.0\x1b[0m"), "New version green");

        // Command cyan: \x1b[36m
        assert!(banner.contains("\x1b[36mspawn-at update\x1b[0m"), "Command cyan");

        // Verify every line has identical visible width
        let lines: Vec<&str> = banner.lines().collect();
        assert!(lines.len() >= 5);
        let expected_w = visible_width(lines[0]);
        for line in &lines {
            assert_eq!(visible_width(line), expected_w, "Line visible width mismatch: {:?}", line);
        }
    }

    #[test]
    fn test_banner_snapshot_color_off() {
        let banner = format_update_banner("0.2.0-beta.1", "0.2.0", ColorCapability::Disabled, true, Some(80));

        // No ANSI escape codes
        assert!(!banner.contains("\x1b"), "Color off must contain no ANSI escapes");

        // UTF-8 box characters present
        assert!(banner.contains('╭'));
        assert!(banner.contains('╮'));
        assert!(banner.contains('╰'));
        assert!(banner.contains('╯'));
        assert!(banner.contains('│'));
        assert!(banner.contains('─'));
        assert!(banner.contains("Update available  0.2.0-beta.1 → 0.2.0"));
        assert!(banner.contains("Run  spawn-at update  to update"));

        // All lines equal visible width
        let lines: Vec<&str> = banner.lines().collect();
        let expected_w = visible_width(lines[0]);
        for line in &lines {
            assert_eq!(visible_width(line), expected_w);
        }
    }

    #[test]
    fn test_banner_snapshot_ascii_color_off() {
        let banner = format_update_banner("0.2.0-beta.1", "0.2.0", ColorCapability::Disabled, false, Some(80));

        // No ANSI escapes
        assert!(!banner.contains("\x1b"));

        // ASCII border characters
        assert!(banner.contains('+'));
        assert!(banner.contains('-'));
        assert!(banner.contains('|'));
        assert!(banner.contains("->"));
        assert!(banner.contains("Update available  0.2.0-beta.1 -> 0.2.0"));
        assert!(banner.contains("Run  spawn-at update  to update"));

        let lines: Vec<&str> = banner.lines().collect();
        let expected_w = visible_width(lines[0]);
        for line in &lines {
            assert_eq!(visible_width(line), expected_w);
        }
    }

    #[test]
    fn test_banner_with_compat_note_color_on() {
        let banner = format_update_banner("0.1.0", "0.2.0", ColorCapability::TrueColor, true, Some(80));
        // Orange compatibility note in TrueColor: \x1b[38;2;255;140;0m
        assert!(banner.contains("\x1b[38;2;255;140;0mNote: The v0.2 CLI is not compatible with v0.1 commands\x1b[0m"));

        let lines: Vec<&str> = banner.lines().collect();
        let expected_w = visible_width(lines[0]);
        for line in &lines {
            assert_eq!(visible_width(line), expected_w);
        }

        // Test 256-color fallback
        let banner_256 = format_update_banner("0.1.0", "0.2.0", ColorCapability::Color256, true, Some(80));
        assert!(banner_256.contains("\x1b[38;5;208mNote: The v0.2 CLI is not compatible with v0.1 commands\x1b[0m"));

        // Test 16-color fallback
        let banner_basic = format_update_banner("0.1.0", "0.2.0", ColorCapability::Basic, true, Some(80));
        assert!(banner_basic.contains("\x1b[33mNote: The v0.2 CLI is not compatible with v0.1 commands\x1b[0m"));
    }

    #[test]
    fn test_banner_width_different_version_string_lengths() {
        let version_pairs = [
            ("0.1.0", "0.2.0"),
            ("0.2.0-beta.1", "0.2.0-beta.2"),
            ("0.2.0-alpha.10.build.456", "0.2.0-beta.11.build.789"),
            ("v1.0.0", "v2.0.0-rc.1"),
        ];

        for (cur, lat) in version_pairs {
            for cap in [ColorCapability::Disabled, ColorCapability::Basic, ColorCapability::Color256, ColorCapability::TrueColor] {
                for utf8 in [true, false] {
                    let banner = format_update_banner(cur, lat, cap, utf8, Some(100));
                    let lines: Vec<&str> = banner.lines().collect();
                    assert!(lines.len() >= 5);
                    let expected_w = visible_width(lines[0]);
                    for (idx, line) in lines.iter().enumerate() {
                        assert_eq!(
                            visible_width(line),
                            expected_w,
                            "Width mismatch on line {} for versions ({}, {}), cap: {:?}, utf8: {}\nBanner:\n{}",
                            idx, cur, lat, cap, utf8, banner
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_banner_narrow_terminal_fallback() {
        // Box is around 40-50 chars wide. A terminal width of 30 should trigger plain fallback.
        let fallback = format_update_banner("0.2.0-beta.1", "0.2.0", ColorCapability::TrueColor, true, Some(30));
        assert!(!fallback.contains('╭'), "Should not contain box characters");
        assert!(!fallback.contains('│'), "Should not contain box characters");
        assert!(fallback.contains("Update available: \x1b[2m0.2.0-beta.1\x1b[0m → \x1b[32m0.2.0\x1b[0m"));
        assert!(fallback.contains("Run \x1b[36mspawn-at update\x1b[0m to update"));

        // Disabled color narrow terminal
        let fallback_plain = format_update_banner("0.2.0-beta.1", "0.2.0", ColorCapability::Disabled, true, Some(30));
        assert!(!fallback_plain.contains("\x1b"));
        assert!(fallback_plain.contains("Update available: 0.2.0-beta.1 → 0.2.0"));
        assert!(fallback_plain.contains("Run 'spawn-at update' to update"));
    }

    #[test]
    fn test_not_a_tty_suppresses_banner() {
        assert!(!should_render_update_notice(false, true, true));
        assert!(!should_render_update_notice(true, false, true));
        assert!(!should_render_update_notice(false, false, true));
        assert!(!should_render_update_notice(true, true, false));
        assert!(should_render_update_notice(true, true, true));
    }
}
