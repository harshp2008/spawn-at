mod cli;
mod drivers;
mod geometry;

use clap::Parser;
use cli::Cli;
use drivers::detect_driver;
use geometry::find_active_monitor;
use std::path::Path;
use std::time::Duration;

fn matches_app_name(win: &crate::geometry::WindowInfo, expected_name: &str) -> bool {
    let exp_lower = expected_name.to_lowercase();
    let wm_lower = win.wm_class.to_lowercase();

    if wm_lower.is_empty() || exp_lower.is_empty() {
        return false;
    }

    // Direct check (case-insensitive)
    if wm_lower == exp_lower || wm_lower.contains(&exp_lower) {
        return true;
    }

    // Normalized strings: lowercase, alphanumeric and hyphens only
    let exp_norm: String = exp_lower
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    let wm_norm: String = wm_lower
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();

    if !exp_norm.is_empty() && (wm_norm == exp_norm || wm_norm.contains(&exp_norm)) {
        return true;
    }

    // Stripped of all non-alphanumeric (e.g. org.gnome.TextEditor -> orggnometexteditor vs gnome-text-editor -> gnometexteditor)
    let exp_alphanum: String = exp_lower.chars().filter(|c| c.is_alphanumeric()).collect();
    let wm_alphanum: String = wm_lower.chars().filter(|c| c.is_alphanumeric()).collect();

    if !exp_alphanum.is_empty()
        && (wm_alphanum == exp_alphanum || wm_alphanum.contains(&exp_alphanum))
    {
        return true;
    }

    // Check desktop file ID segments in wm_class (e.g., com.example.AppName matching AppName or vice-versa)
    for part in wm_lower.split('.') {
        let part_clean: String = part.chars().filter(|c| c.is_alphanumeric()).collect();
        if !part_clean.is_empty() && part_clean.len() >= 4 && part_clean == exp_alphanum {
            return true;
        }
    }

    false
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let driver = detect_driver().await?;

    let (target_x, target_y);

    // 1. Determine base coordinates
    if let Some(pos) = &cli.pos {
        target_x = pos[0];
        target_y = pos[1];
    } else {
        let (cx, cy) = driver.get_cursor().await.unwrap_or((0, 0));
        let (ox, oy) = if let Some(offset) = &cli.offset {
            (offset[0], offset[1])
        } else {
            (0, 0)
        };
        target_x = cx + ox;
        target_y = cy + oy;
    }

    // 2. Fetch Monitors and Calculate Active Monitor
    let monitors = driver.get_monitors().await.unwrap_or_default();
    let active_monitor = find_active_monitor(target_x, target_y, &monitors);

    let size_w = cli.size.as_ref().map(|s| s[0]);
    let size_h = cli.size.as_ref().map(|s| s[1]);

    let global_margin = cli.margin.unwrap_or(0);
    let bound_t = cli.bound_top.unwrap_or(0);
    let bound_b = cli.bound_bottom.unwrap_or(0);
    let bound_l = cli.bound_left.unwrap_or(0);
    let bound_r = cli.bound_right.unwrap_or(0);

    // 3. Snapshot existing windows
    let old_windows = driver.get_windows().await.unwrap_or_default();
    let old_ids: std::collections::HashSet<_> = old_windows.iter().map(|w| w.id.clone()).collect();

    // 4. Spawn target command
    let mut cmd_iter = cli.command.iter();
    let command_name = cmd_iter.next().unwrap().clone();
    let mut child = tokio::process::Command::new(&command_name)
        .args(cmd_iter)
        .spawn()?;

    let expected_binary_name = Path::new(&command_name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // 5. Polling for new window
    let mut new_win_id = None;
    for iteration in 0..200 {
        let current_windows = driver.get_windows().await.unwrap_or_default();
        let current_ids: std::collections::HashSet<_> =
            current_windows.iter().map(|w| w.id.clone()).collect();
        let diff: Vec<_> = current_ids.difference(&old_ids).collect();

        if !diff.is_empty() {
            new_win_id = Some(diff[diff.len() - 1].clone());
            break;
        }

        // Continuous Fallback: If after 20 iterations (300ms) no new ID appears in diff:
        if iteration >= 20 {
            for win in &current_windows {
                if matches_app_name(win, &expected_binary_name) {
                    new_win_id = Some(win.id.clone());
                    break;
                }
            }
            if new_win_id.is_some() {
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(15)).await;

        if let Ok(Some(_)) = child.try_wait() {
            // Child process exited prematurely but it might have spawned a daemon
        }
    }

    // 6. Move the newly found window
    if let Some(win_id) = new_win_id {
        let _ = driver
            .move_window(
                &win_id,
                target_x,
                target_y,
                size_w,
                size_h,
                active_monitor,
                bound_t,
                bound_b,
                bound_l,
                bound_r,
                global_margin,
            )
            .await;
    } else {
        eprintln!(
            "Warning: Did not detect new window spawn for {}",
            expected_binary_name
        );
    }

    Ok(())
}
