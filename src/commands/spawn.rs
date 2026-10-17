//! The handler of `spawn-at spawn`: places the window of a new application before it is shown.
//!
//! The order in `run_spawn_with` is: read the boundary rectangle and fill `PlacementParams` from
//! the flags; build one `Batch` entry keyed by a fresh startup key; record the ids of the windows
//! that exist now; `arm` the backend; start the command with the environment `arm` returned;
//! call `post_spawn`; then, unless `no_wait`, wait for the window (a backend claim signal first,
//! polling `wait_for_spawn` second), re-read its geometry and report it.
//!
//! Arming is fail-open: if `arm` fails the command is still started, unplaced, with a warning. The
//! JSON printed for `--json` is `SpawnOutputJson`. This module does not decide how a backend
//! intercepts the window; it only calls the `CompositorBackend` methods.

use crate::cli::args::SpawnArgs;
use crate::commands::wait_for_spawn;
use crate::platform::capability::HelperNeed;
use crate::platform::{CompositorBackend, DriverError, PlacementParams};
use serde::{Deserialize, Serialize};
use spawn_at_core::driver::{Batch, Entry, FocusIntent, Reveal, Urgency};
use spawn_at_core::geometry::{
    check_geometry_diagnostics, resolve_workarea, Anchor, Area, GeometryDiagnostic, Rect,
};
use spawn_at_core::wait_override::WaitOverride;
use std::time::Duration;

/// Structured JSON output emitted to stdout when `--json` is supplied to `spawn-at spawn`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SpawnOutputJson {
    /// Compositor id of the placed window; `None` when no window was found, or in the early
    /// output printed without waiting.
    pub window_id: Option<u64>,
    /// Final position and size in px; all 0 when no window was found.
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// `x` and `y` minus the top-left corner of the boundary rectangle (0, 0 for the unbounded
    /// area).
    pub rel_x: i32,
    pub rel_y: i32,
    /// The `--area` value that `rel_x` and `rel_y` are relative to.
    pub area: String,
    /// `raised_w` or `raised_h`, or the backend claim reported the size as raised. Always false
    /// without `--size`.
    pub size_raised: bool,
    /// Final width is strictly larger than the requested width.
    #[serde(default)]
    pub raised_w: bool,
    /// Final height is strictly larger than the requested height.
    #[serde(default)]
    pub raised_h: bool,
    /// The application showed an already existing window instead of a new one; nothing was moved.
    pub reused_existing_window: bool,
    /// How the window was found: `"claim"` (backend signal) or `"poll"` (window list polling, also
    /// used for the output printed without a wait).
    pub source: String,
    /// How the size request ended (protocol 6 extension, native Wayland window). Left out of the
    /// JSON when the extension reported none: an older extension, a legacy-strategy window, a
    /// poll fallback or no `--size`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize: Option<spawn_at_core::resize_report::ResizeReport>,
}

/// Runs `spawn-at spawn` with the settings from the user's config file. A file that is missing,
/// unreadable or malformed never fails the spawn: the defaults apply and a warning is printed.
/// For the errors and output see [`run_spawn_with`].
pub async fn run_spawn(
    driver: &(dyn CompositorBackend + Sync),
    spawn_args: SpawnArgs,
    no_wait: bool,
) -> Result<(), DriverError> {
    let store = crate::platform::linux::config_store::LinuxConfigStore::from_env();
    let loaded = spawn_at_core::settings::load(&store);
    run_spawn_with(driver, spawn_args, no_wait, loaded).await
}

/// [`run_spawn`] with the settings already read (`Loaded::default()` is "no config file").
///
/// Errors (`DriverError::Execution` unless noted): an empty command; invalid arguments;
/// `--area screen` without the helper it needs; the command could not be started (the armed
/// token is disarmed first).
///
/// Not errors, all returning `Ok(())`: `arm` failing (warning, then the command runs unplaced
/// with no wait); `post_spawn` failing (warning); the window not appearing within the
/// `window_timeout_ms` setting, or the application reusing an old window (a warning or
/// diagnostic, and with `--json` an output with `window_id: null`).
///
/// With `json`, exactly one `SpawnOutputJson` line goes to stdout, except when `arm` failed and
/// `no_wait` is set: then nothing is printed. The child's own stdout and stderr go to
/// `/dev/null` in that case (see [`configure_child_process`]).
pub async fn run_spawn_with(
    driver: &(dyn CompositorBackend + Sync),
    spawn_args: SpawnArgs,
    no_wait: bool,
    loaded: spawn_at_core::settings::Loaded,
) -> Result<(), DriverError> {
    for warning in &loaded.warnings {
        crate::diagnostics::render_warning(warning);
    }
    let file = loaded.settings;
    let window_timeout = Duration::from_millis(u64::from(
        file.window_timeout_ms
            .unwrap_or(spawn_at_core::settings::WINDOW_TIMEOUT_DEFAULT_MS),
    ));
    if spawn_args.command.is_empty() {
        return Err(DriverError::Execution(
            "No command specified to spawn.".into(),
        ));
    }

    if let Err(e) = spawn_args.validate() {
        return Err(DriverError::Execution(e.into()));
    }

    // 1. Placement parameters. The cursor is read only for `--anchor cursor` / `--monitor cursor`;
    // if that read fails the position falls back to (0, 0) without a message (`transform` returns
    // the error instead).
    let is_cursor_anchor = spawn_args.geometry.anchor == Some(Anchor::Cursor);
    let is_cursor_monitor = spawn_args
        .geometry
        .monitor
        .as_deref()
        .is_some_and(|m| m.eq_ignore_ascii_case("cursor"));
    let needs_cursor = is_cursor_anchor || is_cursor_monitor;
    let (cursor_x, cursor_y) = if needs_cursor {
        driver.get_cursor_position().await.unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    if spawn_args.geometry.area == Area::Screen {
        let need = HelperNeed::ScreenArea;
        if let Some(message) = driver.helper_status(need).await.message(need) {
            return Err(DriverError::Execution(message.into()));
        }
    }

    let is_unbounded = spawn_args.geometry.area == Area::Unbounded;

    // Boundary: an empty rectangle for the unbounded area; otherwise the screen or work area of the
    // monitor chosen from `get_layout` (`--monitor`, default primary). If `get_layout` fails, the
    // monitor or work area list is searched instead ("cursor" when the cursor is needed, else
    // `--monitor`, else "primary"). A failing list or an unresolved monitor gives an empty
    // rectangle, not an error.
    let target_boundary = if is_unbounded {
        Rect::default()
    } else {
        let layout_res = driver.get_layout().await;
        if let Ok(layout) = layout_res {
            let (chosen, fallback) = spawn_at_core::geometry::resolve_monitor_from_layout(
                &layout,
                (cursor_x, cursor_y),
                spawn_args.geometry.monitor.as_deref(),
            );
            if fallback {
                crate::diagnostics::render_debug(
                    "No monitor flagged primary; falling back to monitor index 0.",
                );
            }
            if let Some(m) = chosen {
                if spawn_args.geometry.area == Area::Screen {
                    m.screen.map(Rect::from).unwrap_or_default()
                } else {
                    m.workarea.map(Rect::from).unwrap_or_default()
                }
            } else {
                Rect::default()
            }
        } else {
            let boundary_rects = if spawn_args.geometry.area == Area::Screen {
                driver.get_monitors().await.unwrap_or_default()
            } else {
                driver.get_workareas().await.unwrap_or_default()
            };
            if needs_cursor {
                resolve_workarea(&boundary_rects, (cursor_x, cursor_y), "cursor")
                    .unwrap_or_default()
            } else {
                let mon_str = spawn_args.geometry.monitor.as_deref().unwrap_or("primary");
                resolve_workarea(&boundary_rects, (cursor_x, cursor_y), mon_str).unwrap_or_default()
            }
        }
    };

    let (ref_x, ref_y) = if is_unbounded {
        (0, 0)
    } else {
        (target_boundary.x, target_boundary.y)
    };
    let area_str = spawn_args.geometry.area.to_string();

    // With an anchor the position flag is an offset from the anchor; without one it is absolute.
    let pos_or_offset = spawn_args.geometry.pos_or_offset();
    let (pos, offset) = if spawn_args.geometry.anchor.is_some() {
        (None, pos_or_offset)
    } else {
        (pos_or_offset, None)
    };

    // `--size` needs exactly two values that parse as u32; anything else gives no size here
    // (`validate` is expected to have rejected it already; not verified for every input).
    let size = spawn_args.geometry.size.as_ref().and_then(|s| {
        if s.len() == 2 {
            let w = s[0].parse::<u32>().ok()?;
            let h = s[1].parse::<u32>().ok()?;
            Some((w, h))
        } else {
            None
        }
    });

    let cursor_pos = if needs_cursor {
        Some((cursor_x, cursor_y))
    } else {
        None
    };

    let params = PlacementParams {
        pos,
        offset,
        size,
        anchor: spawn_args.geometry.anchor,
        pivot: spawn_args.geometry.pivot,
        margin: if is_unbounded {
            0
        } else {
            spawn_args.geometry.margin()
        },
        margin_top: if is_unbounded {
            None
        } else {
            spawn_args.geometry.margin_top
        },
        margin_bottom: if is_unbounded {
            None
        } else {
            spawn_args.geometry.margin_bottom
        },
        margin_left: if is_unbounded {
            None
        } else {
            spawn_args.geometry.margin_left
        },
        margin_right: if is_unbounded {
            None
        } else {
            spawn_args.geometry.margin_right
        },
        area: Some(spawn_args.geometry.area),
        cursor_pos,
        workarea: target_boundary,
        clamp: if is_unbounded {
            false
        } else {
            spawn_args.geometry.clamp()
        },
    };

    for diag in check_geometry_diagnostics(&params) {
        match diag {
            GeometryDiagnostic::Oversized { w, h } => {
                crate::diagnostics::render_warning(&format!(
                    "Window is oversized ({w}x{h}); bottom and right margins ignored."
                ));
            }
            GeometryDiagnostic::SubMinimumSize { w, h } => {
                crate::diagnostics::render_info(&format!(
                    "Requested size ({w}x{h}) is below toolkit minimums; window will expand."
                ));
            }
            _ => {}
        }
    }

    // 2. The entry key: `spawn-at-<our pid>-<ms since epoch>_TIME<same ms>`. It is also the
    // `startup_id` that `wait_for_spawn` looks for in a window's title or class. Whether it follows
    // the freedesktop startup-notification format is not verified.
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let entry_key = format!("spawn-at-{}-{}_TIME{}", std::process::id(), ts_ms, ts_ms);

    let app_hint = spawn_args
        .class
        .clone()
        .unwrap_or_else(|| driver.resolve_id(&spawn_args.command, None));

    // Developer override of the resize wait (hidden --wait-strategy / --wait-cap-ms). A bad value
    // is dropped with a warning; the spawn goes on with the rest.
    // A flag beats the config file; with neither, `wait` is `WaitOverride::default()` and the
    // instruction carries no extra field.
    let (wait_flags, wait_problems) = spawn_at_core::settings::wait_flags(
        spawn_args.wait_strategy.as_deref(),
        spawn_args.wait_cap_ms.as_deref(),
    );
    for problem in &wait_problems {
        crate::diagnostics::render_warning(problem);
    }
    let wait: WaitOverride = spawn_at_core::settings::wait_override(&wait_flags, &file);

    // One batch with one entry. Fixed values: id 1, reveal together, exclusive focus, normal
    // urgency, 15000 ms deadline (not the `window_timeout_ms` setting, which only bounds the
    // waits below).
    let batch = Batch {
        id: 1,
        entries: vec![Entry {
            key: entry_key.clone(),
            app_hint: app_hint.clone(),
            placement: params.clone(),
            wait,
        }],
        reveal: Reveal::Together,
        focus: FocusIntent::Exclusive,
        urgency: Urgency::Normal,
        deadline: Duration::from_millis(15000),
    };

    // Ids of the windows open now, so `wait_for_spawn` can tell a new window from an old one. A
    // failing `get_windows` gives an empty set.
    let pre_existing_ids: std::collections::HashSet<u64> = driver
        .get_windows()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.id)
        .collect();

    // 3. Subscribe to the claim signal before `arm`, so a claim that arrives right after arming
    // is not missed. A backend without one, or an error, gives `None`.
    let claim_waiter = driver.prepare_claim_wait(&app_hint).await.ok().flatten();

    // 4. Arm the backend. A failure only warns: the command is started anyway, unplaced.
    let armed_opt = match driver.arm(batch.clone()).await {
        Ok(a) => Some(a),
        Err(e) => {
            crate::diagnostics::render_warning(&format!(
                "Placement arming failed [driver: {}]: {}. Launching application without placement.",
                driver.name(),
                e
            ));
            None
        }
    };

    // 5. Start the command with the environment variables `arm` returned. The child is never
    // waited for.
    let mut cmd = std::process::Command::new(&spawn_args.command[0]);
    cmd.args(&spawn_args.command[1..]);
    if let Some(ref armed) = armed_opt {
        for (k, v) in &armed.launch_env {
            cmd.env(k, v);
        }
    }
    configure_child_process(&mut cmd, spawn_args.json);

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            if let Some(ref armed) = armed_opt {
                if let Some(ref token) = armed.token {
                    let _ = driver.disarm(token).await;
                }
            }
            return Err(DriverError::Execution(
                format!("Failed to spawn command '{}': {}", spawn_args.command[0], e).into(),
            ));
        }
    };

    // Arm failed, `--json`, waiting allowed: print the placeholder line (id null, geometry 0,
    // source "poll") right away. No wait follows, because the block below needs an armed backend.
    if armed_opt.is_none() && spawn_args.json && !no_wait {
        let output = SpawnOutputJson {
            window_id: None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            rel_x: 0 - ref_x,
            rel_y: 0 - ref_y,
            area: area_str.clone(),
            size_raised: false,
            raised_w: false,
            raised_h: false,
            reused_existing_window: false,
            source: "poll".to_string(),
            resize: None,
        };
        println!("{}", serde_json::to_string(&output).unwrap());
    }

    if let Some(ref _armed) = armed_opt {
        // Backend hook that runs after the child exists, with its pid and the batch. What it does
        // is up to the backend.
        if let Err(e) = driver.post_spawn(child.id(), &batch).await {
            crate::diagnostics::render_warning(&format!("Window post-spawn hook failed: {}", e));
        }

        // `--no-wait --json`: the same placeholder line, then the function returns.
        if no_wait && spawn_args.json {
            let output = SpawnOutputJson {
                window_id: None,
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                rel_x: 0 - ref_x,
                rel_y: 0 - ref_y,
                area: area_str.clone(),
                size_raised: false,
                raised_w: false,
                raised_h: false,
                reused_existing_window: false,
                source: "poll".to_string(),
                resize: None,
            };
            println!("{}", serde_json::to_string(&output).unwrap());
        }

        if !no_wait {
            let mut claim_result: Option<(u64, Rect, bool)> = None;
            let mut claim_resize: Option<spawn_at_core::resize_report::ResizeReport> = None;

            // Claim first, up to `window_timeout`. An `Execution` error (the backend reported a
            // failed placement) is printed as a warning; any other error, including a timeout,
            // is silent. Both fall back to polling, which waits up to `window_timeout` again.
            if let Some(mut waiter) = claim_waiter {
                match waiter.wait_claim(window_timeout).await {
                    Ok(claim) => {
                        claim_resize = claim.resize.clone();
                        claim_result = Some((
                            claim.window_id,
                            Rect {
                                x: claim.x,
                                y: claim.y,
                                width: claim.w,
                                height: claim.h,
                            },
                            claim.size_raised,
                        ));
                    }
                    Err(DriverError::Execution(e)) => {
                        crate::diagnostics::render_warning(&e.to_string());
                    }
                    Err(_) => {
                        // Claim signal timed out or was not received for this specific target;
                        // fall back to polling below.
                    }
                }
            }

            let source = if claim_result.is_some() {
                "claim"
            } else {
                "poll"
            };

            let (win_id, mut final_rect, size_raised) = match claim_result {
                Some((id, rect, raised)) => (Some(id), rect, raised),
                None => {
                    match wait_for_spawn(
                        driver,
                        child.id(),
                        &app_hint,
                        &entry_key,
                        &pre_existing_ids,
                        window_timeout,
                    )
                    .await
                    {
                        Ok(w) => {
                            let id = w.id;
                            (
                                id,
                                Rect {
                                    x: w.x,
                                    y: w.y,
                                    width: w.w as u32,
                                    height: w.h as u32,
                                },
                                false,
                            )
                        }
                        Err(e) => {
                            // "Reused" is recognised by the text of the error that
                            // `wait_for_spawn` builds, so changing that text changes this.
                            // Either way the command returns `Ok(())`, so a window that never
                            // appeared still gives exit code 0.
                            let is_reused = e.to_string().contains("reused an existing window");
                            if !is_reused {
                                crate::diagnostics::render_warning(&format!(
                                    "Timed out waiting for window to map: {}",
                                    e
                                ));
                            }

                            if spawn_args.json {
                                let (x, y, w, h) = if is_reused {
                                    let existing_win =
                                        driver.get_windows().await.ok().and_then(|windows| {
                                            windows.into_iter().find(|win| {
                                                win.id.is_some_and(|id| {
                                                    pre_existing_ids.contains(&id)
                                                }) && (!app_hint.is_empty()
                                                    && (crate::commands::match_window_app_hint(
                                                        &app_hint, &win.class, &win.title,
                                                    ) || win.app_id.as_ref().is_some_and(
                                                        |aid| {
                                                            crate::commands::match_window_app_hint(
                                                                &app_hint, aid, &win.title,
                                                            )
                                                        },
                                                    )))
                                            })
                                        });
                                    if let Some(ew) = existing_win {
                                        (ew.x, ew.y, ew.w as u32, ew.h as u32)
                                    } else {
                                        (0, 0, 0, 0)
                                    }
                                } else {
                                    (0, 0, 0, 0)
                                };

                                let output = SpawnOutputJson {
                                    window_id: None,
                                    x,
                                    y,
                                    w,
                                    h,
                                    rel_x: x - ref_x,
                                    rel_y: y - ref_y,
                                    area: area_str.clone(),
                                    size_raised: false,
                                    raised_w: false,
                                    raised_h: false,
                                    reused_existing_window: is_reused,
                                    source: "poll".to_string(),
                                    resize: None,
                                };
                                println!("{}", serde_json::to_string(&output).unwrap());
                            }
                            return Ok(());
                        }
                    }
                }
            };

            // Read the geometry again for up to 300 ms until it satisfies `params`
            // (`settle_window_geometry`); the last rectangle seen replaces the claimed or polled
            // one.
            if let Some(settled_rect) = crate::commands::settle_window_geometry(
                driver,
                win_id,
                &app_hint,
                &params,
                Duration::from_millis(300),
            )
            .await
            {
                final_rect = settled_rect;
            }
            if let Some(id) = win_id {
                // If a size, position or anchor was requested and the window is maximized now (the
                // application maximized itself), send `Restore` and the same placement once more,
                // wait 150 ms and re-read the rectangle. Once only; the results of the two calls
                // are ignored.
                let has_explicit_placement =
                    params.size.is_some() || params.pos.is_some() || params.anchor.is_some();
                if has_explicit_placement {
                    if let Ok(windows) = driver.get_windows().await {
                        if let Some(w) = windows.into_iter().find(|w| w.id == Some(id)) {
                            if w.maximized {
                                let _ = driver
                                    .set_window_state(
                                        &id.to_string(),
                                        crate::platform::WindowState::Restore,
                                    )
                                    .await;
                                let _ = driver
                                    .transform_window(
                                        &id.to_string(),
                                        params.clone(),
                                        final_rect.width,
                                        final_rect.height,
                                    )
                                    .await;
                                tokio::time::sleep(Duration::from_millis(150)).await;
                                if let Ok(reasserted_rect) = driver.get_window_rect(id).await {
                                    final_rect = reasserted_rect;
                                }
                            }
                        }
                    }
                }
            }

            // Strictly larger than requested counts as raised. Without `--size` all three are
            // false, even if the claim said `size_raised`.
            let (raised_w, raised_h, size_raised) = if let Some(req) = params.size {
                let rw = final_rect.width > req.0;
                let rh = final_rect.height > req.1;
                (rw, rh, rw || rh || size_raised)
            } else {
                (false, false, false)
            };

            if spawn_args.json {
                let output = SpawnOutputJson {
                    window_id: win_id,
                    x: final_rect.x,
                    y: final_rect.y,
                    w: final_rect.width,
                    h: final_rect.height,
                    rel_x: final_rect.x - ref_x,
                    rel_y: final_rect.y - ref_y,
                    area: area_str.clone(),
                    size_raised,
                    raised_w,
                    raised_h,
                    reused_existing_window: false,
                    source: source.to_string(),
                    resize: claim_resize.map(|r| match params.size {
                        Some((w, h)) => r.with_requested(w, h),
                        None => r,
                    }),
                };
                println!("{}", serde_json::to_string(&output).unwrap());
            }

            crate::diagnostics::verify_and_report_placement(&params, final_rect, None, size_raised);
            if is_unbounded {
                let monitors = driver.get_monitors().await.unwrap_or_default();
                if spawn_at_core::geometry::window_straddles_monitors(final_rect, &monitors) {
                    crate::diagnostics::render_info("Window straddles monitors.");
                }
            }
        }
    }

    Ok(())
}

/// Sets up the standard streams of the child and detaches it from this process.
///
/// - stdin is always `/dev/null`.
/// - stdout (and, separately, stderr) is `/dev/null` when `is_json` is true or that stream of
///   this process is not a terminal, so a caller capturing our output (`OUT=$(spawn-at ...)`)
///   is not held open by the app; otherwise the child inherits it.
/// - Detaching is `host().detach_child`; on Linux it calls `setsid()` in the child and falls back
///   to `setpgid(0, 0)` if that fails.
pub fn configure_child_process(cmd: &mut std::process::Command, is_json: bool) {
    use std::io::IsTerminal;

    cmd.stdin(std::process::Stdio::null());

    if is_json || !std::io::stdout().is_terminal() {
        cmd.stdout(std::process::Stdio::null());
    } else {
        cmd.stdout(std::process::Stdio::inherit());
    }

    if is_json || !std::io::stderr().is_terminal() {
        cmd.stderr(std::process::Stdio::null());
    } else {
        cmd.stderr(std::process::Stdio::inherit());
    }

    crate::platform::host().detach_child(cmd);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Armed, Batch, ClaimResult, ClaimSubscription, Driver, WindowMetadata};
    use std::sync::{Arc, Mutex};

    struct TestClaimSub;

    #[async_trait::async_trait]
    impl ClaimSubscription for TestClaimSub {
        async fn wait_claim(&mut self, _timeout: Duration) -> Result<ClaimResult, DriverError> {
            Ok(ClaimResult {
                target_id: "test-app".into(),
                success: true,
                window_id: 101,
                x: 100,
                y: 100,
                w: 400,
                h: 300,
                size_raised: false,
                error: String::new(),
                resize: None,
            })
        }
    }

    struct OrderingMockBackend {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl Driver for OrderingMockBackend {
        async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
            self.calls.lock().unwrap().push("arm".into());
            Ok(Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for OrderingMockBackend {
        fn name(&self) -> &'static str {
            "OrderingMock"
        }

        fn resolve_id(&self, _command: &[String], _explicit_class: Option<&str>) -> String {
            "test-app".into()
        }

        fn supports_claim_wait(&self) -> bool {
            true
        }

        async fn prepare_claim_wait(
            &self,
            _target_id: &str,
        ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
            self.calls.lock().unwrap().push("prepare_claim_wait".into());
            Ok(Some(Box::new(TestClaimSub)))
        }

        async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
            Ok(vec![Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }])
        }

        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            Ok(vec![WindowMetadata {
                id: Some(101),
                pid: Some(std::process::id()),
                title: "Test".into(),
                class: "test-app".into(),
                app_id: Some("test-app".into()),
                x: 100,
                y: 100,
                w: 400,
                h: 300,
                focused: true,
                maximized: false,
                minimized: false,
                ..Default::default()
            }])
        }
    }

    #[tokio::test]
    async fn test_subscribe_before_arm_ordering() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let backend = OrderingMockBackend {
            calls: calls.clone(),
        };

        let args = SpawnArgs {
            geometry: crate::cli::args::GeometryArgs {
                size: Some(vec!["400".into(), "300".into()]),
                ..Default::default()
            },
            command: vec!["true".into()],
            ..Default::default()
        };

        let res = run_spawn_with(&backend, args, false, Default::default()).await;
        assert!(res.is_ok());

        let recorded = calls.lock().unwrap().clone();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], "prepare_claim_wait");
        assert_eq!(recorded[1], "arm");
    }

    /// Backend that keeps the batch it was armed with.
    struct BatchRecorder {
        inner: OrderingMockBackend,
        batches: Arc<Mutex<Vec<Batch>>>,
    }

    #[async_trait::async_trait]
    impl Driver for BatchRecorder {
        async fn arm(&self, batch: Batch) -> Result<Armed, DriverError> {
            self.batches.lock().unwrap().push(batch.clone());
            self.inner.arm(batch).await
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for BatchRecorder {
        fn name(&self) -> &'static str {
            "BatchRecorder"
        }

        fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
            self.inner.resolve_id(command, explicit_class)
        }

        fn supports_claim_wait(&self) -> bool {
            true
        }

        async fn prepare_claim_wait(
            &self,
            target_id: &str,
        ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
            self.inner.prepare_claim_wait(target_id).await
        }

        async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
            self.inner.get_workareas().await
        }

        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            self.inner.get_windows().await
        }
    }

    /// Runs `run_spawn` for a 400x300 window with the given hidden wait flags and returns the
    /// batch that reached `arm` and the warnings printed on the way.
    async fn arm_with_wait_flags(
        strategy: Option<&str>,
        cap_ms: Option<&str>,
    ) -> (Batch, Vec<String>) {
        arm_with_wait_flags_and_config(strategy, cap_ms, Default::default()).await
    }

    /// [`arm_with_wait_flags`] with a config file already read (`loaded`).
    async fn arm_with_wait_flags_and_config(
        strategy: Option<&str>,
        cap_ms: Option<&str>,
        loaded: spawn_at_core::settings::Loaded,
    ) -> (Batch, Vec<String>) {
        let batches = Arc::new(Mutex::new(Vec::new()));
        let backend = BatchRecorder {
            inner: OrderingMockBackend {
                calls: Arc::new(Mutex::new(Vec::new())),
            },
            batches: batches.clone(),
        };
        let args = SpawnArgs {
            geometry: crate::cli::args::GeometryArgs {
                size: Some(vec!["400".into(), "300".into()]),
                ..Default::default()
            },
            wait_strategy: strategy.map(String::from),
            wait_cap_ms: cap_ms.map(String::from),
            command: vec!["true".into()],
            ..Default::default()
        };
        crate::diagnostics::start_capturing_diagnostics();
        let res = run_spawn_with(&backend, args, false, loaded).await;
        assert!(
            res.is_ok(),
            "an override must never fail the spawn: {res:?}"
        );
        let warnings = crate::diagnostics::take_captured_diagnostics()
            .unwrap_or_default()
            .into_iter()
            .filter(|(lvl, _)| *lvl == crate::diagnostics::DiagnosticLevel::Warning)
            .map(|(_, msg)| msg)
            .collect();
        let batch = batches
            .lock()
            .unwrap()
            .first()
            .cloned()
            .expect("arm was called");
        (batch, warnings)
    }

    /// Runs `run_spawn_with` for a 400x300 window with the given focus modifiers and returns the
    /// batch that reached `arm`, plus every backend call in order.
    async fn arm_with_focus_modifiers(
        modifiers: crate::cli::FocusModifierArgs,
    ) -> (Batch, Vec<String>) {
        let batches = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let backend = BatchRecorder {
            inner: OrderingMockBackend {
                calls: calls.clone(),
            },
            batches: batches.clone(),
        };
        let args = SpawnArgs {
            geometry: crate::cli::args::GeometryArgs {
                size: Some(vec!["400".into(), "300".into()]),
                ..Default::default()
            },
            focus_modifiers: modifiers,
            command: vec!["true".into()],
            ..Default::default()
        };
        let res = run_spawn_with(&backend, args, false, Default::default()).await;
        assert!(res.is_ok(), "{res:?}");
        let mut batch = batches
            .lock()
            .unwrap()
            .first()
            .cloned()
            .expect("arm was called");
        // The entry key holds the process id and a millisecond timestamp, different every run.
        batch.entries[0].key = "<key>".into();
        let recorded = calls.lock().unwrap().clone();
        (batch, recorded)
    }

    /// CHARACTERIZATION of audit gap 5 (`docs/TEST_COVERAGE_AUDIT.md`): `spawn --focus`,
    /// `--no-focus` and `--defocus` are parsed, but `run_spawn_with` never reads
    /// `SpawnArgs::focus_modifiers`: it builds the batch with `FocusIntent::Exclusive`
    /// (`src/commands/spawn.rs`, the `Batch { .. }` literal) whatever the flags say, and calls
    /// no focus method on the backend. This test pins that: all four runs arm the same batch
    /// and make the same backend calls. It documents CURRENT behavior, not a contract. When the
    /// flags are given an effect (for example `--no-focus` -> `FocusIntent::Leave`), this test
    /// must change in the same commit, and the GNOME extension's focus handling needs a decision
    /// first (see the audit report).
    #[tokio::test]
    async fn test_spawn_focus_modifiers_have_no_effect_today() {
        use crate::cli::FocusModifierArgs;
        let none = FocusModifierArgs::default();
        let focus = FocusModifierArgs {
            focus: true,
            ..Default::default()
        };
        let no_focus = FocusModifierArgs {
            no_focus: true,
            ..Default::default()
        };
        let defocus = FocusModifierArgs {
            defocus: true,
            ..Default::default()
        };

        let (base, base_calls) = arm_with_focus_modifiers(none).await;
        assert_eq!(base.focus, FocusIntent::Exclusive);
        assert_eq!(base_calls, ["prepare_claim_wait", "arm"]);

        for (name, modifiers) in [
            ("--focus", focus),
            ("--no-focus", no_focus),
            ("--defocus", defocus),
        ] {
            let (batch, calls) = arm_with_focus_modifiers(modifiers).await;
            assert_eq!(batch, base, "{name} changed the armed batch");
            assert_eq!(calls, base_calls, "{name} changed the backend calls");
        }
    }

    #[tokio::test]
    async fn test_wait_flags_reach_the_armed_entry() {
        let (batch, warnings) = arm_with_wait_flags(Some("legacy"), Some("200")).await;
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(batch.entries.len(), 1);
        let wait = batch.entries[0].wait;
        assert_eq!(
            wait.strategy,
            spawn_at_core::wait_override::WaitStrategy::Legacy
        );
        assert_eq!(wait.cap_ms, Some(200));
    }

    #[tokio::test]
    async fn test_without_wait_flags_the_entry_has_no_override() {
        // The default must stay the default end to end: no override on the entry, no warning,
        // and no "wait" key if the entry is serialised.
        let (batch, warnings) = arm_with_wait_flags(None, None).await;
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(batch.entries[0].wait.is_default());
        let json = serde_json::to_string(&batch.entries[0]).unwrap();
        assert!(!json.contains("wait"), "{json}");
        // `--wait-strategy auto` is the same as saying nothing.
        let (batch, warnings) = arm_with_wait_flags(Some("auto"), None).await;
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(batch.entries[0].wait.is_default());
    }

    #[tokio::test]
    async fn test_invalid_wait_flags_are_ignored_with_one_warning_each() {
        let (batch, warnings) = arm_with_wait_flags(Some("fast"), Some("0")).await;
        assert!(batch.entries[0].wait.is_default());
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(
            warnings[0].contains("--wait-strategy 'fast'"),
            "{warnings:?}"
        );
        assert!(warnings[1].contains("--wait-cap-ms '0'"), "{warnings:?}");
        // A valid half survives an invalid other half.
        let (batch, warnings) = arm_with_wait_flags(Some("ready-wait"), Some("abc")).await;
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(
            batch.entries[0].wait.strategy,
            spawn_at_core::wait_override::WaitStrategy::ReadyWait
        );
        assert_eq!(batch.entries[0].wait.cap_ms, None);
    }

    fn from_file(text: &str) -> spawn_at_core::settings::Loaded {
        spawn_at_core::settings::load_text(text, "test.toml")
    }

    /// The instructions the GNOME driver would send for the entry that reached `arm`, as JSON.
    fn instruction_bytes(batch: &Batch) -> String {
        serde_json::to_string(
            &crate::platform::linux::gnome::mechanics::build_instructions_for_entry(
                &batch.entries[0],
            ),
        )
        .unwrap()
    }

    /// No config file and no flags: the arming instruction of a 400x300 spawn is the one a build
    /// without the config sent (the entry with `WaitOverride::default()`), and its wait step is
    /// exactly `{"WaitForCommit":{"timeout_ms":300}}`. An empty file, a file with only unrelated
    /// keys, and `strategy = "auto"` change nothing either.
    #[tokio::test]
    async fn test_default_spawn_instruction_bytes_are_unchanged() {
        let (plain, _) = arm_with_wait_flags(None, None).await;
        let mut reference = plain.entries[0].clone();
        reference.wait = Default::default();
        let before = serde_json::to_string(
            &crate::platform::linux::gnome::mechanics::build_instructions_for_entry(&reference),
        )
        .unwrap();
        assert_eq!(instruction_bytes(&plain), before);
        assert!(
            before.contains(r#"{"WaitForCommit":{"timeout_ms":300}}"#),
            "{before}"
        );
        for file in [
            "",
            "[update]\nchannel = \"beta\"\n",
            "[wait]\nstrategy = \"auto\"\n",
            "[spawn]\nwindow_timeout_ms = 5000\n",
        ] {
            let (b, warnings) = arm_with_wait_flags_and_config(None, None, from_file(file)).await;
            assert!(warnings.is_empty(), "{file:?}: {warnings:?}");
            assert_eq!(instruction_bytes(&b), before, "{file:?}");
        }
    }

    /// A value in the file is sent in the existing optional fields; a flag beats it; a flag of
    /// `auto` beats a file `legacy`.
    #[tokio::test]
    async fn test_config_file_values_reach_the_instruction_and_flags_win() {
        let file = "[wait]\ncap_ms = 120\nstrategy = \"legacy\"\n";
        let (b, warnings) = arm_with_wait_flags_and_config(None, None, from_file(file)).await;
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            instruction_bytes(&b).contains(
                r#"{"WaitForCommit":{"timeout_ms":300,"strategy":"legacy","cap_ms":120}}"#
            ),
            "{}",
            instruction_bytes(&b)
        );
        let (b, _) =
            arm_with_wait_flags_and_config(Some("ready-wait"), Some("90"), from_file(file)).await;
        assert!(
            instruction_bytes(&b).contains(
                r#"{"WaitForCommit":{"timeout_ms":300,"strategy":"ready-wait","cap_ms":90}}"#
            ),
            "{}",
            instruction_bytes(&b)
        );
        let (b, _) = arm_with_wait_flags_and_config(Some("auto"), None, from_file(file)).await;
        assert!(
            instruction_bytes(&b).contains(r#"{"WaitForCommit":{"timeout_ms":300,"cap_ms":120}}"#),
            "{}",
            instruction_bytes(&b)
        );
    }

    /// A malformed file or a bad value never fails the spawn: the warnings are printed, the rest
    /// of the file still applies, and a bad flag falls back to the file value.
    #[tokio::test]
    async fn test_bad_config_warns_and_the_spawn_goes_on() {
        let (b, warnings) =
            arm_with_wait_flags_and_config(None, None, from_file("[wait\ncap_ms = 1\n")).await;
        assert!(b.entries[0].wait.is_default());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("line 1"), "{warnings:?}");
        let (b, warnings) = arm_with_wait_flags_and_config(
            None,
            None,
            from_file("[wait]\ncap_ms = 9999\nstrategy = \"legacy\"\nnope = 1\n"),
        )
        .await;
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert_eq!(b.entries[0].wait.cap_ms, None);
        assert_eq!(
            b.entries[0].wait.strategy,
            spawn_at_core::wait_override::WaitStrategy::Legacy
        );
        let (b, _) =
            arm_with_wait_flags_and_config(None, Some("0"), from_file("[wait]\ncap_ms = 77\n"))
                .await;
        assert_eq!(b.entries[0].wait.cap_ms, Some(77));
    }

    struct FallbackMockBackend {
        calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl Driver for FallbackMockBackend {
        async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
            self.calls.lock().unwrap().push("arm".into());
            Ok(Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for FallbackMockBackend {
        fn name(&self) -> &'static str {
            "FallbackMock"
        }

        fn resolve_id(&self, _command: &[String], _explicit_class: Option<&str>) -> String {
            "true".into()
        }

        fn supports_claim_wait(&self) -> bool {
            false
        }

        async fn prepare_claim_wait(
            &self,
            _target_id: &str,
        ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push("prepare_claim_wait_fallback".into());
            Ok(None) // Extension lacks signal or driver does not support claim wait
        }

        async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
            Ok(vec![Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }])
        }

        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            self.calls
                .lock()
                .unwrap()
                .push("get_windows_polling".into());
            Ok(vec![WindowMetadata {
                id: Some(202),
                pid: Some(std::process::id()),
                title: "Fallback Test".into(),
                class: "true".into(),
                app_id: Some("true".into()),
                x: 50,
                y: 50,
                w: 400,
                h: 300,
                focused: true,
                maximized: false,
                minimized: false,
                ..Default::default()
            }])
        }
    }

    #[tokio::test]
    async fn test_fallback_to_polling_when_claim_wait_unsupported() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let backend = FallbackMockBackend {
            calls: calls.clone(),
        };

        let args = SpawnArgs {
            geometry: crate::cli::args::GeometryArgs {
                size: Some(vec!["400".into(), "300".into()]),
                ..Default::default()
            },
            command: vec!["true".into()],
            ..Default::default()
        };

        let res = run_spawn_with(&backend, args, false, Default::default()).await;
        assert!(res.is_ok());

        let recorded = calls.lock().unwrap().clone();
        assert!(recorded.contains(&"prepare_claim_wait_fallback".to_string()));
        assert!(recorded.contains(&"arm".to_string()));
        assert!(recorded.contains(&"get_windows_polling".to_string()));
    }

    #[test]
    fn test_spawn_output_json_shapes() {
        // 1. Success via claim
        let success_claim = SpawnOutputJson {
            window_id: Some(101),
            x: 16,
            y: 56,
            w: 400,
            h: 300,
            rel_x: 16,
            rel_y: 16,
            area: "workarea".to_string(),
            size_raised: false,
            raised_w: false,
            raised_h: false,
            reused_existing_window: false,
            source: "claim".to_string(),
            resize: None,
        };
        let json_val: serde_json::Value = serde_json::to_value(&success_claim).unwrap();
        assert_eq!(json_val["window_id"], 101);
        assert_eq!(json_val["x"], 16);
        assert_eq!(json_val["y"], 56);
        assert_eq!(json_val["w"], 400);
        assert_eq!(json_val["h"], 300);
        assert_eq!(json_val["rel_x"], 16);
        assert_eq!(json_val["rel_y"], 16);
        assert_eq!(json_val["area"], "workarea");
        assert_eq!(json_val["size_raised"], false);
        assert_eq!(json_val["raised_w"], false);
        assert_eq!(json_val["raised_h"], false);
        assert_eq!(json_val["reused_existing_window"], false);
        assert_eq!(json_val["source"], "claim");

        // 2. Success via fallback (poll with height raised)
        let success_fallback = SpawnOutputJson {
            window_id: Some(202),
            x: 50,
            y: 50,
            w: 400,
            h: 514,
            rel_x: 50,
            rel_y: 50,
            area: "screen".to_string(),
            size_raised: true,
            raised_w: false,
            raised_h: true,
            reused_existing_window: false,
            source: "poll".to_string(),
            resize: None,
        };
        let json_val: serde_json::Value = serde_json::to_value(&success_fallback).unwrap();
        assert_eq!(json_val["window_id"], 202);
        assert_eq!(json_val["x"], 50);
        assert_eq!(json_val["y"], 50);
        assert_eq!(json_val["rel_x"], 50);
        assert_eq!(json_val["rel_y"], 50);
        assert_eq!(json_val["area"], "screen");
        assert_eq!(json_val["size_raised"], true);
        assert_eq!(json_val["raised_w"], false);
        assert_eq!(json_val["raised_h"], true);
        assert_eq!(json_val["reused_existing_window"], false);
        assert_eq!(json_val["source"], "poll");

        // 3. Reused existing window (window_id null, reused_existing_window true)
        let reused = SpawnOutputJson {
            window_id: None,
            x: 100,
            y: 150,
            w: 800,
            h: 600,
            rel_x: 100,
            rel_y: 110,
            area: "workarea".to_string(),
            size_raised: false,
            raised_w: false,
            raised_h: false,
            reused_existing_window: true,
            source: "poll".to_string(),
            resize: None,
        };
        let json_val: serde_json::Value = serde_json::to_value(&reused).unwrap();
        assert!(json_val["window_id"].is_null());
        assert_eq!(json_val["x"], 100);
        assert_eq!(json_val["y"], 150);
        assert_eq!(json_val["w"], 800);
        assert_eq!(json_val["h"], 600);
        assert_eq!(json_val["rel_x"], 100);
        assert_eq!(json_val["rel_y"], 110);
        assert_eq!(json_val["area"], "workarea");
        assert_eq!(json_val["reused_existing_window"], true);
        assert_eq!(json_val["source"], "poll");

        // 4. Timeout (window_id null, reused_existing_window false)
        let timeout = SpawnOutputJson {
            window_id: None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            rel_x: 0,
            rel_y: -40,
            area: "workarea".to_string(),
            size_raised: false,
            raised_w: false,
            raised_h: false,
            reused_existing_window: false,
            source: "poll".to_string(),
            resize: None,
        };
        let json_val: serde_json::Value = serde_json::to_value(&timeout).unwrap();
        assert!(json_val["window_id"].is_null());
        assert_eq!(json_val["x"], 0);
        assert_eq!(json_val["y"], 0);
        assert_eq!(json_val["w"], 0);
        assert_eq!(json_val["h"], 0);
        assert_eq!(json_val["rel_x"], 0);
        assert_eq!(json_val["rel_y"], -40);
        assert_eq!(json_val["area"], "workarea");
        assert_eq!(json_val["reused_existing_window"], false);
        assert_eq!(json_val["source"], "poll");
    }

    fn sample_output(
        resize: Option<spawn_at_core::resize_report::ResizeReport>,
    ) -> SpawnOutputJson {
        SpawnOutputJson {
            window_id: Some(7),
            x: 1104,
            y: 464,
            w: 494,
            h: 292,
            rel_x: 1104,
            rel_y: 424,
            area: "workarea".to_string(),
            size_raised: false,
            raised_w: false,
            raised_h: false,
            reused_existing_window: false,
            source: "claim".to_string(),
            resize,
        }
    }

    /// With a protocol 6 outcome the JSON has a `resize` object with the keys outcome, requested,
    /// final, waited_ms and reason, and every pre-existing key keeps its value.
    #[test]
    fn test_spawn_output_json_resize_object() {
        let report = spawn_at_core::resize_report::ResizeReport::from_wire(
            "adjusted", 494, 292, 41, "quiet",
        )
        .with_requested(500, 300);
        let value = serde_json::to_value(sample_output(Some(report))).unwrap();
        assert_eq!(
            value["resize"],
            serde_json::json!({
                "outcome": "adjusted",
                "requested": {"w": 500, "h": 300},
                "final": {"w": 494, "h": 292},
                "waited_ms": 41,
                "reason": "quiet"
            })
        );
        assert_eq!(value["window_id"], 7);
        assert_eq!(value["source"], "claim");
    }

    /// Without an outcome (protocol 5, legacy-strategy window, poll fallback) the key is absent,
    /// so the output is exactly what it was before protocol 6.
    #[test]
    fn test_spawn_output_json_without_resize_has_no_resize_key() {
        let value = serde_json::to_value(sample_output(None)).unwrap();
        assert!(value.as_object().unwrap().get("resize").is_none());
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "area",
                "h",
                "raised_h",
                "raised_w",
                "rel_x",
                "rel_y",
                "reused_existing_window",
                "size_raised",
                "source",
                "w",
                "window_id",
                "x",
                "y"
            ]
        );
    }

    /// A consumer reading output from an older CLI (no `resize` key) still parses it.
    #[test]
    fn test_spawn_output_json_from_older_cli_parses() {
        let old = r#"{"window_id":7,"x":1,"y":2,"w":3,"h":4,"rel_x":1,"rel_y":2,"area":"workarea",
            "size_raised":false,"reused_existing_window":false,"source":"claim"}"#;
        let parsed: SpawnOutputJson = serde_json::from_str(old).unwrap();
        assert!(parsed.resize.is_none());
    }

    #[test]
    fn test_capturing_stdout_returns_without_waiting_for_child() {
        let mut child_cmd = std::process::Command::new("sh");
        child_cmd.args(["-c", "sleep 5"]);
        configure_child_process(&mut child_cmd, true);

        let start = std::time::Instant::now();
        let mut child = child_cmd.spawn().expect("failed to spawn child");

        // The process was detached and stdio redirected to null;
        // spawn returns immediately and child does not hold any inherited pipe.
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_child_in_different_session_or_process_group() {
        let mut child_cmd = std::process::Command::new("sleep");
        child_cmd.arg("10");
        configure_child_process(&mut child_cmd, true);

        let mut child = child_cmd.spawn().expect("failed to spawn sleep");
        let child_pid = child.id() as i32;

        unsafe {
            let parent_sid = libc::getsid(0);
            let child_sid = libc::getsid(child_pid);
            let parent_pgid = libc::getpgrp();
            let child_pgid = libc::getpgid(child_pid);

            assert!(
                child_sid != parent_sid || child_pgid != parent_pgid,
                "Child was not placed in its own session or process group!"
            );
        }

        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn test_sigint_to_parent_process_group_does_not_reach_child() {
        let mut child_cmd = std::process::Command::new("sleep");
        child_cmd.arg("10");
        configure_child_process(&mut child_cmd, true);

        let mut child = child_cmd.spawn().expect("failed to spawn child");
        let child_pid = child.id() as i32;

        unsafe {
            let child_pgid = libc::getpgid(child_pid);
            let current_pgid = libc::getpgrp();
            assert_ne!(
                child_pgid, current_pgid,
                "Child should have its own process group distinct from parent"
            );

            // Child is alive
            assert_eq!(libc::kill(child_pid, 0), 0);
        }

        let _ = child.kill();
        let _ = child.wait();
    }
}
