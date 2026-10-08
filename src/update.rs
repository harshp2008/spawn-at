//! # Update Manager & Release Checker
//!
//! Handles manual upgrades (`spawn-at update`), periodic 24-hour background release checks,
//! and npm-style boxed update notifications for interactive terminal sessions.

use crate::config::{current_timestamp_secs, Config};
use clap::Args;
use dialoguer::Select;
use serde::Deserialize;
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

/// Command-line arguments for the `spawn-at update` subcommand.
#[derive(Args, Debug, Clone, Default)]
pub struct UpdateArgs {
    /// Only check GitHub for available updates without downloading or installing
    #[arg(long)]
    pub check: bool,

    /// Override the configured release channel: 'stable' (official) or 'all' (beta/pre-releases)
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
        option_env!("SPAWN_AT_LAST_RELEASE")
            .unwrap_or("v0.1.0")
            .to_string()
    }
}

/// Parses a semver-like version string into (major, minor, patch, is_prerelease).
pub fn parse_version_tuple(v: &str) -> (u32, u32, u32, bool) {
    let clean = v.trim().trim_start_matches('v');
    let is_prerelease = clean.contains('-') || clean.contains("alpha") || clean.contains("beta") || clean.contains("rc");
    let base = clean.split('-').next().unwrap_or(clean);
    let mut parts = base.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch, is_prerelease)
}

/// Determines whether `latest` represents a strictly newer version than `current`.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest_trim = latest.trim();
    let current_trim = current.trim();
    if latest_trim.is_empty() || latest_trim == current_trim {
        return false;
    }

    let (l_maj, l_min, l_pat, l_pre) = parse_version_tuple(latest_trim);
    let (c_maj, c_min, c_pat, c_pre) = parse_version_tuple(current_trim);

    if (l_maj, l_min, l_pat) > (c_maj, c_min, c_pat) {
        return true;
    }
    if (l_maj, l_min, l_pat) == (c_maj, c_min, c_pat) {
        // A non-prerelease is newer than a prerelease of the same version
        if c_pre && !l_pre {
            return true;
        }
    }
    false
}

/// Queries the GitHub API for releases of `harshp2008/spawn-at`.
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

/// Resolves the candidate release based on the configured channel ("stable" vs "all").
pub fn resolve_target_release(channel: &str) -> Result<GitHubRelease, String> {
    let releases = fetch_github_releases()?;
    let candidate = releases
        .into_iter()
        .filter(|r| !r.draft)
        .find(|r| if channel == "all" { true } else { !r.prerelease })
        .ok_or_else(|| format!("No matching releases found for channel '{}'", channel))?;
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

/// Renders an npm-style boxed update banner at the bottom of standard command output.
pub fn render_boxed_notice(current: &str, latest: &str) {
    let line1 = format!("Update available: {} → {}", current, latest);
    let line2 = "Run 'spawn-at update' to upgrade to the latest version".to_string();

    let line1_len = line1.chars().count();
    let line2_len = line2.chars().count();
    let content_len = std::cmp::max(line1_len, line2_len);
    let inner_width = std::cmp::max(content_len + 6, 64);

    let top = format!("┌{}┐", "─".repeat(inner_width));
    let empty = format!("│{}│", " ".repeat(inner_width));
    let pad1 = inner_width.saturating_sub(line1_len + 3);
    let pad2 = inner_width.saturating_sub(line2_len + 3);

    println!("\n{}", top);
    println!("{}", empty);
    println!("│   {}{}│", line1, " ".repeat(pad1));
    println!("│   {}{}│", line2, " ".repeat(pad2));
    println!("{}", empty);
    println!("└{}┘\n", "─".repeat(inner_width));
}

/// Hook called at the end of `main()` to render update notifications if appropriate.
pub fn render_update_notice_if_available() {
    // 1. Must be an interactive TTY
    if !std::io::stdout().is_terminal() {
        return;
    }

    // 2. Load configuration
    let mut cfg = Config::load_or_default();
    if !cfg.update.notify {
        return;
    }

    // 4. Trigger 24h background check if due
    trigger_background_check_if_needed(&mut cfg);

    // 5. If cached version is newer, render boxed banner
    let current = get_current_release_tag();
    if is_newer_version(&cfg.update.latest_cached_version, &current) {
        render_boxed_notice(&current, &cfg.update.latest_cached_version);
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

    let tmp_dir = PathBuf::from(format!("/tmp/spawn-at-update-{}", release.tag_name));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create tmp dir: {}", e))?;

    let tar_path = tmp_dir.join(asset_name);

    println!("\x1b[1;36mDownloading {}\x1b[0m", release.tag_name);
    let curl_status = Command::new("curl")
        .args(["-#", "-L", "-o", tar_path.to_str().unwrap(), &asset.browser_download_url])
        .status()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !curl_status.success() {
        return Err("Download failed with non-zero exit code".to_string());
    }

    println!("\x1b[1;36mExtracting package...\x1b[0m");
    let tar_status = Command::new("tar")
        .args(["-xzf", tar_path.to_str().unwrap(), "-C", tmp_dir.to_str().unwrap()])
        .status()
        .map_err(|e| format!("Failed to extract package: {}", e))?;

    if !tar_status.success() {
        return Err("Failed to extract release archive".to_string());
    }

    let new_binary = tmp_dir.join("spawn-at");
    if !new_binary.exists() {
        return Err("Extracted archive did not contain 'spawn-at' executable".to_string());
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

    // Re-deploy GNOME Shell extension if GNOME is the current desktop
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    if desktop.contains("GNOME") {
        if let Ok(home) = std::env::var("HOME") {
            let ext_dir = PathBuf::from(home).join(".local/share/gnome-shell/extensions/spawn-at@harsh.local");
            if ext_dir.exists() {
                println!("Re-deploying GNOME Shell extension files...");
                let ext_js = include_str!("../assets/gnome/extension.esm.js");
                let meta_json = include_str!("../assets/gnome/metadata.json");
                let _ = fs::write(ext_dir.join("extension.js"), ext_js);
                let _ = fs::write(ext_dir.join("metadata.json"), meta_json);
            }
        }

        // Check session type for reload prompt
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

    // Update cached version in config
    let mut cfg = Config::load_or_default();
    cfg.update.latest_cached_version = release.tag_name.clone();
    cfg.update.last_check_time = current_timestamp_secs();
    let _ = cfg.save();

    println!("\x1b[1;32mSuccessfully upgraded spawn-at to {}!\x1b[0m", release.tag_name);
    Ok(())
}

/// Executes the `spawn-at update` command with interactive TUI or headless automation.
pub fn run_update(args: UpdateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load_or_default();
    let current_tag = get_current_release_tag();
    let channel = args.channel.as_deref().unwrap_or(&config.update.channel).to_string();

    if args.headless && args.check {
        if let Ok(release) = resolve_target_release(&channel) {
            config.update.latest_cached_version = release.tag_name;
            config.update.last_check_time = current_timestamp_secs();
            let _ = config.save();
        }
        return Ok(());
    }

    println!("\x1b[1;36m=== spawn-at Update Manager ===\x1b[0m\n");
    println!("Current version: {}", current_tag);
    println!("Checking GitHub for releases (Channel: {})...", channel);

    let release = match resolve_target_release(&channel) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("\x1b[1;31mError checking updates\x1b[0m: {}", e);
            return Ok(());
        }
    };

    config.update.latest_cached_version = release.tag_name.clone();
    config.update.last_check_time = current_timestamp_secs();
    let _ = config.save();

    let rel_type = if release.prerelease { "Pre-release" } else { "Official Release" };
    println!("Latest available version: {} ({})\n", release.tag_name, rel_type);

    let has_update = is_newer_version(&release.tag_name, &current_tag);

    if args.check {
        if has_update {
            println!("An update is available: {} → {}", current_tag, release.tag_name);
        } else {
            println!("spawn-at is up to date ({}).", current_tag);
        }
        return Ok(());
    }

    if !has_update && !args.force {
        println!("You are already running the latest version for channel '{}'.", channel);
        if !std::io::stdout().is_terminal() || args.headless {
            return Ok(());
        }
    }

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
                let channels = &["Official releases only (stable) [Recommended]", "Include beta/pre-releases (all)"];
                let ch_sel = Select::new()
                    .with_prompt("Select release channel:")
                    .items(channels)
                    .default(if channel == "all" { 1 } else { 0 })
                    .interact()?;
                let new_ch = if ch_sel == 1 { "all" } else { "stable" };
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
mod tests {
    use super::*;

    #[test]
    fn test_version_tuple_parsing() {
        assert_eq!(parse_version_tuple("v0.1.0"), (0, 1, 0, false));
        assert_eq!(parse_version_tuple("0.2.0"), (0, 2, 0, false));
        assert_eq!(parse_version_tuple("v0.2.0-alpha"), (0, 2, 0, true));
        assert_eq!(parse_version_tuple("v1.12.3-rc1"), (1, 12, 3, true));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.2.0", "v0.1.0"));
        assert!(is_newer_version("v1.0.0", "v0.9.9"));
        assert!(is_newer_version("v0.2.0", "v0.2.0-alpha"));

        assert!(!is_newer_version("v0.1.0", "v0.1.0"));
        assert!(!is_newer_version("v0.1.0", "v0.2.0"));
        assert!(!is_newer_version("", "v0.1.0"));
    }

    #[test]
    fn test_boxed_notice_rendering() {
        // Ensure rendering does not panic
        render_boxed_notice("v0.1.0", "v0.2.0");
    }
}
