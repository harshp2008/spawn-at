//! # Spawn Command Handler
//!
//! Orchestrates the multi-stage window placement and application launch pipeline:
//! 1. Resolves geometry and workareas from compositor backend.
//! 2. Arms the backend driver (fail-open on driver error).
//! 3. Launches the child process with armed environment variables.
//! 4. Executes post-spawn interception hooks and waits for window mapping.

use crate::cli::args::SpawnArgs;
use crate::commands::wait_for_spawn;
use crate::platform::{CompositorBackend, DriverError, PlacementParams};
use serde::{Deserialize, Serialize};
use spawn_at_core::driver::{Batch, Entry, FocusIntent, Reveal, Urgency};
use spawn_at_core::geometry::{
    check_geometry_diagnostics, resolve_workarea, Anchor, Area, GeometryDiagnostic, Rect,
};
use std::time::Duration;

/// Structured JSON output emitted to stdout when `--json` is supplied to `spawn-at spawn`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SpawnOutputJson {
    pub window_id: Option<u64>,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub size_raised: bool,
    pub reused_existing_window: bool,
    pub source: String,
}

/// Executes the spawn subcommand pipeline.
pub async fn run_spawn(
    driver: &(dyn CompositorBackend + Sync),
    spawn_args: SpawnArgs,
    no_wait: bool,
) -> Result<(), DriverError> {
    if spawn_args.command.is_empty() {
        return Err(DriverError::Execution(
            "No command specified to spawn.".into(),
        ));
    }

    if let Err(e) = spawn_args.validate() {
        return Err(DriverError::Execution(e.into()));
    }

    // 1. Calculate target geometry parameters
    let is_cursor_anchor = spawn_args.geometry.anchor == Some(Anchor::Cursor);
    let (cursor_x, cursor_y) = driver.get_cursor_position().await.unwrap_or((0, 0));

    if spawn_args.geometry.area == Area::Screen {
        if let Some(v) = driver.protocol_version().await {
            if v < 4 {
                return Err(DriverError::Execution(
                    format!(
                        "extension is older than the CLI expects (protocol {}, need 4): run `spawn-at install` and log out and back in",
                        v
                    )
                    .into(),
                ));
            }
        }
    }

    let boundary_rects = if spawn_args.geometry.area == Area::Screen {
        driver.get_monitors().await.unwrap_or_default()
    } else {
        driver.get_workareas().await.unwrap_or_default()
    };

    let target_boundary =
        if is_cursor_anchor || spawn_args.geometry.monitor.eq_ignore_ascii_case("cursor") {
            resolve_workarea(&boundary_rects, (cursor_x, cursor_y), "cursor").unwrap_or_default()
        } else {
            resolve_workarea(
                &boundary_rects,
                (cursor_x, cursor_y),
                &spawn_args.geometry.monitor,
            )
            .unwrap_or_default()
        };

    let pos = spawn_args.geometry.pos.as_ref().map(|p| (p[0], p[1]));
    let size = spawn_args.geometry.size.as_ref().and_then(|s| {
        if s.len() == 2 {
            let w = s[0].parse::<u32>().ok()?;
            let h = s[1].parse::<u32>().ok()?;
            Some((w, h))
        } else {
            None
        }
    });

    let params = PlacementParams {
        pos,
        offset: None,
        size,
        anchor: spawn_args.geometry.anchor,
        pivot: spawn_args.geometry.pivot,
        margin: spawn_args.geometry.margin,
        margin_top: spawn_args.geometry.margin_top,
        margin_bottom: spawn_args.geometry.margin_bottom,
        margin_left: spawn_args.geometry.margin_left,
        margin_right: spawn_args.geometry.margin_right,
        area: Some(spawn_args.geometry.area),
        cursor_pos: Some((cursor_x, cursor_y)),
        workarea: target_boundary,
        clamp: spawn_args.geometry.clamp,
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

    // 2. Generate unique activation token / entry key conforming to freedesktop startup notification spec
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let entry_key = format!("spawn-at-{}-{}_TIME{}", std::process::id(), ts_ms, ts_ms);

    let app_hint = spawn_args
        .class
        .clone()
        .unwrap_or_else(|| driver.resolve_id(&spawn_args.command, None));

    let batch = Batch {
        id: 1,
        entries: vec![Entry {
            key: entry_key.clone(),
            app_hint: app_hint.clone(),
            placement: params.clone(),
        }],
        reveal: Reveal::Together,
        focus: FocusIntent::Exclusive,
        urgency: Urgency::Normal,
        deadline: Duration::from_millis(15000),
    };

    // Pre-record existing window IDs so wait_for_spawn captures the newly spawned window accurately
    let pre_existing_ids: std::collections::HashSet<u64> = driver
        .get_windows()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| w.id)
        .collect();

    // 2. Prepare claim wait subscription BEFORE calling arm (prevents missing fast claims)
    let claim_waiter = driver.prepare_claim_wait(&app_hint).await.ok().flatten();

    // 3. Arm the driver with the declarative batch intent (fail-open on arm failure)
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

    // 4. Launch child process with armed environment variables
    let mut cmd = std::process::Command::new(&spawn_args.command[0]);
    cmd.args(&spawn_args.command[1..]);
    if let Some(ref armed) = armed_opt {
        for (k, v) in &armed.launch_env {
            cmd.env(k, v);
        }
    }

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

    if armed_opt.is_none() && spawn_args.json && !no_wait {
        let output = SpawnOutputJson {
            window_id: None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            size_raised: false,
            reused_existing_window: false,
            source: "poll".to_string(),
        };
        println!("{}", serde_json::to_string(&output).unwrap());
    }

    if let Some(ref _armed) = armed_opt {
        // Polymorphic post-spawn interception hook
        if let Err(e) = driver.post_spawn(child.id(), &batch).await {
            crate::diagnostics::render_warning(&format!("Window post-spawn hook failed: {}", e));
        }

        if no_wait && spawn_args.json {
            let output = SpawnOutputJson {
                window_id: None,
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                size_raised: false,
                reused_existing_window: false,
                source: "poll".to_string(),
            };
            println!("{}", serde_json::to_string(&output).unwrap());
        }

        if !no_wait {
            let mut claim_result: Option<(u64, Rect, bool)> = None;

            if let Some(mut waiter) = claim_waiter {
                match waiter.wait_claim(Duration::from_millis(2000)).await {
                    Ok(claim) => {
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
                        Duration::from_millis(2000),
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
                                    size_raised: false,
                                    reused_existing_window: is_reused,
                                    source: "poll".to_string(),
                                };
                                println!("{}", serde_json::to_string(&output).unwrap());
                            }
                            return Ok(());
                        }
                    }
                }
            };

            // Settle re-check (~150-300ms) before final geometry diagnostics
            tokio::time::sleep(Duration::from_millis(200)).await;
            if let Some(id) = win_id {
                if let Ok(settled_rect) = driver.get_window_rect(id).await {
                    final_rect = settled_rect;
                }

                // If explicit placement was requested and the client re-maximized itself, re-assert once, bounded.
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

            if spawn_args.json {
                let output = SpawnOutputJson {
                    window_id: win_id,
                    x: final_rect.x,
                    y: final_rect.y,
                    w: final_rect.width,
                    h: final_rect.height,
                    size_raised,
                    reused_existing_window: false,
                    source: source.to_string(),
                };
                println!("{}", serde_json::to_string(&output).unwrap());
            }

            crate::diagnostics::verify_and_report_placement(&params, final_rect, None, size_raised);
        }
    }

    Ok(())
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

        let res = run_spawn(&backend, args, false).await;
        assert!(res.is_ok());

        let recorded = calls.lock().unwrap().clone();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0], "prepare_claim_wait");
        assert_eq!(recorded[1], "arm");
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

        let res = run_spawn(&backend, args, false).await;
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
            size_raised: false,
            reused_existing_window: false,
            source: "claim".to_string(),
        };
        let json_val: serde_json::Value = serde_json::to_value(&success_claim).unwrap();
        assert_eq!(json_val["window_id"], 101);
        assert_eq!(json_val["x"], 16);
        assert_eq!(json_val["y"], 56);
        assert_eq!(json_val["w"], 400);
        assert_eq!(json_val["h"], 300);
        assert_eq!(json_val["size_raised"], false);
        assert_eq!(json_val["reused_existing_window"], false);
        assert_eq!(json_val["source"], "claim");

        // 2. Success via fallback (poll)
        let success_fallback = SpawnOutputJson {
            window_id: Some(202),
            x: 50,
            y: 50,
            w: 400,
            h: 300,
            size_raised: true,
            reused_existing_window: false,
            source: "poll".to_string(),
        };
        let json_val: serde_json::Value = serde_json::to_value(&success_fallback).unwrap();
        assert_eq!(json_val["window_id"], 202);
        assert_eq!(json_val["size_raised"], true);
        assert_eq!(json_val["reused_existing_window"], false);
        assert_eq!(json_val["source"], "poll");

        // 3. Reused existing window (window_id null, reused_existing_window true)
        let reused = SpawnOutputJson {
            window_id: None,
            x: 100,
            y: 150,
            w: 800,
            h: 600,
            size_raised: false,
            reused_existing_window: true,
            source: "poll".to_string(),
        };
        let json_val: serde_json::Value = serde_json::to_value(&reused).unwrap();
        assert!(json_val["window_id"].is_null());
        assert_eq!(json_val["x"], 100);
        assert_eq!(json_val["y"], 150);
        assert_eq!(json_val["w"], 800);
        assert_eq!(json_val["h"], 600);
        assert_eq!(json_val["reused_existing_window"], true);
        assert_eq!(json_val["source"], "poll");

        // 4. Timeout (window_id null, reused_existing_window false)
        let timeout = SpawnOutputJson {
            window_id: None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            size_raised: false,
            reused_existing_window: false,
            source: "poll".to_string(),
        };
        let json_val: serde_json::Value = serde_json::to_value(&timeout).unwrap();
        assert!(json_val["window_id"].is_null());
        assert_eq!(json_val["x"], 0);
        assert_eq!(json_val["y"], 0);
        assert_eq!(json_val["w"], 0);
        assert_eq!(json_val["h"], 0);
        assert_eq!(json_val["reused_existing_window"], false);
        assert_eq!(json_val["source"], "poll");
    }
}
