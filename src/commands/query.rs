//! The handler of `spawn-at query` (`layout`, `pointer`, `windows`) and the function that fills in
//! each window's position relative to its monitor.
//!
//! It only reads: every call goes to a `CompositorBackend` getter and the result is printed to
//! stdout (plain table, or JSON with `--json`). It must not change window or compositor state.

use crate::cli::QueryCommands;
use crate::platform::capability::{HelperNeed, HelperStatus};
use crate::platform::CompositorBackend;
use crate::target::{RelPosition, WindowMetadata};
use spawn_at_core::geometry::{Area, LayoutRect, MonitorLayout, Rect};

/// Runs one `spawn-at query` subcommand and prints its result to stdout.
///
/// - `layout`: one table row per monitor (index, name, primary, scale, screen and work area as
///   `WxH@X,Y`, insets top/right/bottom/left), then the layout's note if it has one; `-` marks a
///   missing value. With `json` the serialized layout is printed instead.
/// - `pointer`: `x, y` of the cursor. There is no JSON form.
/// - `windows`: one row per window, or a JSON array with `json`. Relative coordinates are
///   computed for `area` (see [`compute_relative_window_coordinates`]). If
///   `helper_status(MonitorAndState)` says the extension helper is outdated, its message is
///   printed with `render_info` and the `monitor` and `state` of every window are dropped.
///
/// Errors: any failing backend call (`get_layout` for `layout`, `get_cursor_position` for
/// `pointer`, `get_windows` for `windows`) and JSON serialization. For `windows`, a failing
/// `get_layout` is not an error: the monitor and work area lists are tried instead, and if those
/// fail too, no window gets relative coordinates.
pub async fn run_query(
    backend: &dyn CompositorBackend,
    cmd: QueryCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        QueryCommands::Layout { json } => {
            let layout = backend.get_layout().await?;
            if json {
                println!("{}", serde_json::to_string(&layout)?);
            } else {
                println!(
                    "{:<3}  {:<12}  {:<7}  {:<5}  {:<17}  {:<17}  INSETS (T/R/B/L)",
                    "MON", "NAME", "PRIMARY", "SCALE", "SCREEN", "WORKAREA"
                );
                for m in &layout.monitors {
                    let mon_str = format!("{}", m.index);
                    let name_str = m.name.as_deref().unwrap_or("-");
                    let primary_str = if m.primary { "yes" } else { "no" };
                    let scale_str = m
                        .scale
                        .map(|s| format!("{}", s))
                        .unwrap_or_else(|| "-".to_string());
                    let screen_str = m
                        .screen
                        .map(|s| format!("{}x{}@{},{}", s.w, s.h, s.x, s.y))
                        .unwrap_or_else(|| "-".to_string());
                    let workarea_str = m
                        .workarea
                        .map(|w| format!("{}x{}@{},{}", w.w, w.h, w.x, w.y))
                        .unwrap_or_else(|| "-".to_string());
                    let insets_str = m
                        .insets
                        .map(|ins| format!("{}/{}/{}/{}", ins.top, ins.right, ins.bottom, ins.left))
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{:<3}  {:<12}  {:<7}  {:<5}  {:<17}  {:<17}  {}",
                        mon_str,
                        name_str,
                        primary_str,
                        scale_str,
                        screen_str,
                        workarea_str,
                        insets_str
                    );
                }
                if let Some(ref note) = layout.note {
                    println!("\nNote: {}", note);
                }
            }
        }
        QueryCommands::Pointer => {
            let (x, y) = backend.get_cursor_position().await?;
            println!("{}, {}", x, y);
        }
        QueryCommands::Windows { json, area } => {
            let need = HelperNeed::MonitorAndState;
            let status = backend.helper_status(need).await;
            let is_older = match status {
                HelperStatus::Outdated { found, .. } => Some(found),
                HelperStatus::Ok => None,
            };
            if let Some(message) = status.message(need) {
                crate::diagnostics::render_info(&message);
            }
            let mut windows = backend.get_windows().await?;
            if is_older.is_some() {
                for w in &mut windows {
                    w.monitor = None;
                    w.state = None;
                }
            }

            // The monitor and work area lists are fetched only when `get_layout` failed; with a
            // layout they stay `None`.
            let layout_opt = backend.get_layout().await.ok();
            let workareas_fallback = if layout_opt.is_none() {
                backend.get_workareas().await.ok()
            } else {
                None
            };
            let monitors_fallback = if layout_opt.is_none() {
                backend.get_monitors().await.ok()
            } else {
                None
            };

            compute_relative_window_coordinates(
                &mut windows,
                layout_opt.as_ref(),
                workareas_fallback.as_deref(),
                monitors_fallback.as_deref(),
                area,
            );

            if json {
                println!("{}", serde_json::to_string(&windows)?);
            } else {
                // Column widths are the longest cell, with minimums of 2 (ID), 3 (PID, MON), 5
                // (CLASS, STATE), 8 (GEOMETRY) and 3 (REL); FOCUSED is fixed at 7. CLASS and STATE
                // are measured in bytes, so a non-ASCII class makes its column wider than the text.
                let id_w = windows
                    .iter()
                    .map(|w| w.id.map(|i| i.to_string().len()).unwrap_or(1))
                    .max()
                    .unwrap_or(2)
                    .max(2);
                let pid_w = windows
                    .iter()
                    .map(|w| w.pid.map(|p| p.to_string().len()).unwrap_or(1))
                    .max()
                    .unwrap_or(3)
                    .max(3);
                let class_w = windows
                    .iter()
                    .map(|w| w.class.len())
                    .max()
                    .unwrap_or(5)
                    .max(5);
                let mon_w = windows
                    .iter()
                    .map(|w| w.monitor.map(|m| m.to_string().len()).unwrap_or(1))
                    .max()
                    .unwrap_or(3)
                    .max(3);
                let state_w = windows
                    .iter()
                    .map(|w| w.state.as_deref().unwrap_or("-").len())
                    .max()
                    .unwrap_or(5)
                    .max(5);
                let geom_w = windows
                    .iter()
                    .map(|w| format!("{}x{}@{},{}", w.w, w.h, w.x, w.y).len())
                    .max()
                    .unwrap_or(8)
                    .max(8);
                let rel_w = windows
                    .iter()
                    .map(|w| {
                        if let Some(ref r) = w.rel {
                            format!("{},{}", r.x, r.y).len()
                        } else {
                            1
                        }
                    })
                    .max()
                    .unwrap_or(3)
                    .max(3);
                let focused_w = 7;

                println!(
                    "{:<id_w$}  {:<pid_w$}  {:<class_w$}  {:<mon_w$}  {:<state_w$}  {:<geom_w$}  {:<rel_w$}  {:<focused_w$}  TITLE",
                    "ID", "PID", "CLASS", "MON", "STATE", "GEOMETRY", "REL", "FOCUSED"
                );
                for w in &windows {
                    let id_str =
                        w.id.map(|id| id.to_string())
                            .unwrap_or_else(|| "-".to_string());
                    let pid_str = w
                        .pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let mon_str = w
                        .monitor
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let state_str = w.state.clone().unwrap_or_else(|| "-".to_string());
                    let geom_str = format!("{}x{}@{},{}", w.w, w.h, w.x, w.y);
                    let rel_str = if let Some(ref r) = w.rel {
                        format!("{},{}", r.x, r.y)
                    } else {
                        "-".to_string()
                    };
                    let focused_str = if w.focused { "*" } else { "" };
                    println!(
                        "{:<id_w$}  {:<pid_w$}  {:<class_w$}  {:<mon_w$}  {:<state_w$}  {:<geom_w$}  {:<rel_w$}  {:<focused_w$}  {}",
                        id_str, pid_str, w.class, mon_str, state_str, geom_str, rel_str, focused_str, w.title
                    );
                }
            }
        }
    }
    Ok(())
}

/// Sets `rel` and `area` on every window: its position relative to the top-left corner of the
/// chosen area of its own monitor, in the same units as `x` and `y` (px).
///
/// - `Unbounded`: `rel` is the window's `x`, `y` and `area` is `"unbounded"`.
/// - `Screen` / `Workarea`: the monitor is looked up by the window's `monitor` index, in
///   `layout_opt` if given, else in `monitors_fallback` / `workareas_fallback` by list position.
///   `area` is then `"screen"` or `"workarea"`. A window with no `monitor`, or whose monitor is
///   not found, gets `rel = None` and `area = None`.
///
/// Does not fail and does not clamp: a window partly off its monitor gets negative `rel`.
pub fn compute_relative_window_coordinates(
    windows: &mut [WindowMetadata],
    layout_opt: Option<&MonitorLayout>,
    workareas_fallback: Option<&[Rect]>,
    monitors_fallback: Option<&[Rect]>,
    area: Area,
) {
    for w in windows {
        match area {
            Area::Unbounded => {
                w.rel = Some(RelPosition { x: w.x, y: w.y });
                w.area = Some("unbounded".to_string());
            }
            Area::Screen => {
                if let Some(mon_idx) = w.monitor {
                    let screen_rect = if let Some(layout) = layout_opt {
                        layout
                            .monitors
                            .iter()
                            .find(|m| m.index as i32 == mon_idx)
                            .and_then(|m| m.screen)
                    } else if let Some(mons) = monitors_fallback {
                        if (mon_idx as usize) < mons.len() {
                            Some(LayoutRect::from(mons[mon_idx as usize]))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(s) = screen_rect {
                        w.rel = Some(RelPosition {
                            x: w.x - s.x,
                            y: w.y - s.y,
                        });
                        w.area = Some("screen".to_string());
                    } else {
                        w.rel = None;
                        w.area = None;
                    }
                } else {
                    w.rel = None;
                    w.area = None;
                }
            }
            Area::Workarea => {
                if let Some(mon_idx) = w.monitor {
                    let workarea_rect = if let Some(layout) = layout_opt {
                        layout
                            .monitors
                            .iter()
                            .find(|m| m.index as i32 == mon_idx)
                            .and_then(|m| m.workarea)
                    } else if let Some(was) = workareas_fallback {
                        if (mon_idx as usize) < was.len() {
                            Some(LayoutRect::from(was[mon_idx as usize]))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(wa) = workarea_rect {
                        w.rel = Some(RelPosition {
                            x: w.x - wa.x,
                            y: w.y - wa.y,
                        });
                        w.area = Some("workarea".to_string());
                    } else {
                        w.rel = None;
                        w.area = None;
                    }
                } else {
                    w.rel = None;
                    w.area = None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::DriverError;
    use crate::target::WindowMetadata;
    use spawn_at_core::geometry::{LayoutInsets, LayoutRect, MonitorInfo, MonitorLayout, Rect};

    struct MockQueryBackend {
        workareas: Vec<Rect>,
        layout: Option<MonitorLayout>,
        windows: Vec<WindowMetadata>,
        cursor: (i32, i32),
        protocol: Option<u32>,
    }

    #[async_trait::async_trait]
    impl crate::platform::Driver for MockQueryBackend {
        async fn arm(
            &self,
            _batch: crate::platform::Batch,
        ) -> Result<crate::platform::Armed, DriverError> {
            Ok(crate::platform::Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for MockQueryBackend {
        fn name(&self) -> &'static str {
            "Mock"
        }
        fn resolve_id(&self, _command: &[String], _explicit_class: Option<&str>) -> String {
            "mock".to_string()
        }
        async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
            Ok(self.cursor)
        }
        async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
            Ok(self.workareas.clone())
        }
        async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
            if let Some(ref l) = self.layout {
                Ok(l.clone())
            } else {
                let workareas = self.get_workareas().await?;
                let monitors = workareas
                    .into_iter()
                    .enumerate()
                    .map(|(i, wa)| MonitorInfo {
                        index: i,
                        name: None,
                        primary: i == 0,
                        scale: None,
                        screen: None,
                        workarea: Some(wa.into()),
                        insets: None,
                    })
                    .collect();
                Ok(MonitorLayout {
                    schema_version: 1,
                    monitors,
                    note: None,
                })
            }
        }
        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            Ok(self.windows.clone())
        }
        async fn protocol_version(&self) -> Option<u32> {
            self.protocol
        }
    }

    #[tokio::test]
    async fn test_run_query_pointer() {
        let backend = MockQueryBackend {
            workareas: vec![],
            layout: None,
            windows: vec![],
            cursor: (100, 200),
            protocol: None,
        };
        assert!(run_query(&backend, QueryCommands::Pointer).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_query_windows_table_and_json() {
        let backend = MockQueryBackend {
            workareas: vec![],
            layout: None,
            windows: vec![
                WindowMetadata {
                    id: Some(706192865),
                    pid: Some(75448),
                    title: "spawn-at - Antigravity IDE - cli.rs".to_string(),
                    class: "antigravity-ide".to_string(),
                    app_id: None,
                    x: 0,
                    y: 40,
                    w: 1920,
                    h: 1040,
                    rel: None,
                    area: None,
                    focused: true,
                    maximized: false,
                    minimized: false,
                    monitor: Some(0),
                    state: Some("normal".to_string()),
                    workspace: Some(1),
                },
                WindowMetadata {
                    id: Some(706192866),
                    pid: Some(76342),
                    title: "Calculator".to_string(),
                    class: "org.gnome.Calculator".to_string(),
                    app_id: None,
                    x: 16,
                    y: 514,
                    w: 450,
                    h: 550,
                    rel: None,
                    area: None,
                    focused: false,
                    maximized: false,
                    minimized: false,
                    monitor: Some(0),
                    state: Some("normal".to_string()),
                    workspace: Some(1),
                },
            ],
            cursor: (0, 0),
            protocol: Some(5),
        };

        // Human readable table output (default workarea)
        assert!(run_query(
            &backend,
            QueryCommands::Windows {
                json: false,
                area: Area::Workarea,
            }
        )
        .await
        .is_ok());

        // JSON output
        assert!(run_query(
            &backend,
            QueryCommands::Windows {
                json: true,
                area: Area::Workarea,
            }
        )
        .await
        .is_ok());

        // Screen area
        assert!(run_query(
            &backend,
            QueryCommands::Windows {
                json: true,
                area: Area::Screen,
            }
        )
        .await
        .is_ok());

        // Unbounded area
        assert!(run_query(
            &backend,
            QueryCommands::Windows {
                json: true,
                area: Area::Unbounded,
            }
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_run_query_windows_stale_extension_protocol_4_warning() {
        crate::diagnostics::start_capturing_diagnostics();
        let backend = MockQueryBackend {
            workareas: vec![],
            layout: None,
            windows: vec![WindowMetadata {
                id: Some(101),
                pid: Some(202),
                title: "Editor".to_string(),
                class: "editor".to_string(),
                app_id: None,
                x: 0,
                y: 0,
                w: 800,
                h: 600,
                focused: true,
                maximized: false,
                minimized: false,
                monitor: Some(0),
                state: Some("normal".to_string()),
                workspace: Some(1),
                ..Default::default()
            }],
            cursor: (0, 0),
            protocol: Some(4),
        };

        assert!(run_query(
            &backend,
            QueryCommands::Windows {
                json: false,
                area: Area::Workarea,
            }
        )
        .await
        .is_ok());

        let diags = crate::diagnostics::take_captured_diagnostics().unwrap_or_default();
        assert!(diags.iter().any(|(level, msg)| {
            *level == crate::diagnostics::DiagnosticLevel::Info
                && msg.contains("extension is older than the CLI expects (protocol 4, need 5): MON and STATE are unavailable; run `spawn-at install` and log out and back in")
        }));
    }

    #[tokio::test]
    async fn test_run_query_layout_table_and_json() {
        let backend = MockQueryBackend {
            workareas: vec![Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            }],
            layout: Some(MonitorLayout {
                schema_version: 1,
                monitors: vec![MonitorInfo {
                    index: 0,
                    name: Some("DP-1".to_string()),
                    primary: true,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 0,
                        y: 40,
                        w: 1920,
                        h: 1040,
                    }),
                    insets: Some(LayoutInsets {
                        top: 40,
                        right: 0,
                        bottom: 0,
                        left: 0,
                    }),
                }],
                note: Some("Testing note".to_string()),
            }),
            windows: vec![],
            cursor: (0, 0),
            protocol: None,
        };

        assert!(run_query(&backend, QueryCommands::Layout { json: false })
            .await
            .is_ok());

        assert!(run_query(&backend, QueryCommands::Layout { json: true })
            .await
            .is_ok());
    }

    #[test]
    fn test_characterization_query_layout_json_schema() {
        let layout = MonitorLayout {
            schema_version: 1,
            monitors: vec![
                MonitorInfo {
                    index: 0,
                    name: Some("DP-1".to_string()),
                    primary: true,
                    scale: Some(1.0),
                    screen: Some(LayoutRect {
                        x: 0,
                        y: 0,
                        w: 1920,
                        h: 1080,
                    }),
                    workarea: Some(LayoutRect {
                        x: 0,
                        y: 32,
                        w: 1920,
                        h: 1048,
                    }),
                    insets: Some(LayoutInsets {
                        top: 32,
                        right: 0,
                        bottom: 0,
                        left: 0,
                    }),
                },
                MonitorInfo {
                    index: 1,
                    name: Some("HDMI-1".to_string()),
                    primary: false,
                    scale: Some(1.25),
                    screen: Some(LayoutRect {
                        x: 1920,
                        y: 0,
                        w: 2560,
                        h: 1440,
                    }),
                    workarea: Some(LayoutRect {
                        x: 1920,
                        y: 0,
                        w: 2560,
                        h: 1440,
                    }),
                    insets: Some(LayoutInsets {
                        top: 0,
                        right: 0,
                        bottom: 0,
                        left: 0,
                    }),
                },
            ],
            note: None,
        };
        let json_str = serde_json::to_string(&layout).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(val["schema_version"], 1);
        let monitors = val["monitors"].as_array().unwrap();
        assert_eq!(monitors.len(), 2);

        // Check exact schema keys and values
        assert_eq!(monitors[0]["index"], 0);
        assert_eq!(monitors[0]["name"], "DP-1");
        assert_eq!(monitors[0]["primary"], true);
        assert_eq!(monitors[0]["scale"], 1.0);
        assert_eq!(monitors[0]["screen"]["x"], 0);
        assert_eq!(monitors[0]["screen"]["y"], 0);
        assert_eq!(monitors[0]["screen"]["w"], 1920);
        assert_eq!(monitors[0]["screen"]["h"], 1080);
        assert_eq!(monitors[0]["workarea"]["x"], 0);
        assert_eq!(monitors[0]["workarea"]["y"], 32);
        assert_eq!(monitors[0]["workarea"]["w"], 1920);
        assert_eq!(monitors[0]["workarea"]["h"], 1048);
        assert_eq!(monitors[0]["insets"]["top"], 32);
        assert_eq!(monitors[0]["insets"]["right"], 0);
        assert_eq!(monitors[0]["insets"]["bottom"], 0);
        assert_eq!(monitors[0]["insets"]["left"], 0);
        assert!(val.get("note").is_none());
    }

    #[test]
    fn test_characterization_query_windows_json_schema() {
        let windows = vec![
            WindowMetadata {
                id: Some(101),
                pid: Some(202),
                title: "Terminal".to_string(),
                class: "org.gnome.Terminal".to_string(),
                app_id: None,
                x: 10,
                y: 20,
                w: 800,
                h: 600,
                rel: Some(crate::target::RelPosition { x: 10, y: 0 }),
                area: Some("workarea".to_string()),
                focused: true,
                maximized: false,
                minimized: false,
                monitor: Some(0),
                state: Some("normal".to_string()),
                workspace: Some(1),
            },
            WindowMetadata {
                id: None,
                pid: None,
                title: "Overlay".to_string(),
                class: "overlay".to_string(),
                app_id: None,
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
                rel: None,
                area: None,
                focused: false,
                maximized: true,
                minimized: false,
                monitor: None,
                state: Some("maximized".to_string()),
                workspace: None,
            },
        ];
        let json_str = serde_json::to_string(&windows).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // Check exact schema keys and nullable fields
        assert_eq!(arr[0]["id"], 101);
        assert_eq!(arr[0]["pid"], 202);
        assert_eq!(arr[0]["title"], "Terminal");
        assert_eq!(arr[0]["class"], "org.gnome.Terminal");
        assert_eq!(arr[0]["x"], 10);
        assert_eq!(arr[0]["y"], 20);
        assert_eq!(arr[0]["w"], 800);
        assert_eq!(arr[0]["h"], 600);
        assert_eq!(arr[0]["rel"]["x"], 10);
        assert_eq!(arr[0]["rel"]["y"], 0);
        assert_eq!(arr[0]["area"], "workarea");
        assert_eq!(arr[0]["focused"], true);
        assert_eq!(arr[0]["maximized"], false);
        assert_eq!(arr[0]["minimized"], false);
        assert_eq!(arr[0]["monitor"], 0);
        assert_eq!(arr[0]["state"], "normal");
        assert_eq!(arr[0]["workspace"], 1);

        assert!(arr[1]["id"].is_null());
        assert!(arr[1]["pid"].is_null());
        assert!(arr[1]["rel"].is_null());
        assert!(arr[1]["area"].is_null());
        assert_eq!(arr[1]["maximized"], true);
        assert!(arr[1]["monitor"].is_null());
        assert_eq!(arr[1]["state"], "maximized");
        assert!(arr[1]["workspace"].is_null());
    }

    #[test]
    fn test_query_windows_round_trip_all_area_modes() {
        use spawn_at_core::geometry::{calculate_rect_from_placement, PlacementParams, Rect};

        let wa = Rect {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
        };
        let screen = Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };

        let layout = MonitorLayout {
            schema_version: 1,
            monitors: vec![spawn_at_core::geometry::MonitorInfo {
                index: 0,
                name: None,
                primary: true,
                scale: None,
                screen: Some(screen.into()),
                workarea: Some(wa.into()),
                insets: None,
            }],
            note: None,
        };

        let base_windows = vec![WindowMetadata {
            id: Some(4067358578),
            pid: Some(408727),
            title: "Calculator".to_string(),
            class: "org.gnome.Calculator".to_string(),
            app_id: None,
            x: 700,
            y: 300,
            w: 367,
            h: 514,
            rel: None,
            area: None,
            focused: false,
            maximized: false,
            minimized: false,
            monitor: Some(0),
            state: Some("normal".to_string()),
            workspace: Some(1),
        }];

        // 1. Workarea mode round-trip
        let mut wins = base_windows.clone();
        compute_relative_window_coordinates(&mut wins, Some(&layout), None, None, Area::Workarea);
        let rel = wins[0].rel.as_ref().unwrap();
        assert_eq!(rel.x, 700);
        assert_eq!(rel.y, 260);
        let params_wa = PlacementParams {
            pos: Some((rel.x, rel.y)),
            area: Some(Area::Workarea),
            workarea: wa,
            clamp: false,
            ..Default::default()
        };
        let target_wa =
            calculate_rect_from_placement(&params_wa, wins[0].w as u32, wins[0].h as u32);
        assert_eq!(target_wa.x, 700);
        assert_eq!(target_wa.y, 300);

        // 2. Screen mode round-trip
        let mut wins = base_windows.clone();
        compute_relative_window_coordinates(&mut wins, Some(&layout), None, None, Area::Screen);
        let rel = wins[0].rel.as_ref().unwrap();
        assert_eq!(rel.x, 700);
        assert_eq!(rel.y, 300);
        let params_screen = PlacementParams {
            pos: Some((rel.x, rel.y)),
            area: Some(Area::Screen),
            workarea: screen,
            clamp: false,
            ..Default::default()
        };
        let target_screen =
            calculate_rect_from_placement(&params_screen, wins[0].w as u32, wins[0].h as u32);
        assert_eq!(target_screen.x, 700);
        assert_eq!(target_screen.y, 300);

        // 3. Unbounded mode round-trip (using GEOMETRY or REL)
        let mut wins = base_windows.clone();
        compute_relative_window_coordinates(&mut wins, Some(&layout), None, None, Area::Unbounded);
        let rel = wins[0].rel.as_ref().unwrap();
        assert_eq!(rel.x, wins[0].x);
        assert_eq!(rel.y, wins[0].y);
        let params_unbounded = PlacementParams {
            pos: Some((wins[0].x, wins[0].y)),
            area: Some(Area::Unbounded),
            clamp: false,
            ..Default::default()
        };
        let target_unbounded =
            calculate_rect_from_placement(&params_unbounded, wins[0].w as u32, wins[0].h as u32);
        assert_eq!(target_unbounded.x, 700);
        assert_eq!(target_unbounded.y, 300);
    }
}
