//! # Compositor State Query Command Handlers
//!
//! Provides handlers for inspecting active compositor workareas, pointer coordinates,
//! and mapped window metadata.

use crate::cli::QueryCommands;
use crate::platform::CompositorBackend;

/// Executes the `query` subcommand: fetches requested compositor state and prints to stdout.
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
        QueryCommands::Windows { json } => {
            let windows = backend.get_windows().await?;
            if json {
                println!("{}", serde_json::to_string(&windows)?);
            } else {
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
                let geom_w = windows
                    .iter()
                    .map(|w| format!("{}x{}@{},{}", w.w, w.h, w.x, w.y).len())
                    .max()
                    .unwrap_or(8)
                    .max(8);
                let focused_w = 7;

                println!(
                    "{:<id_w$}  {:<pid_w$}  {:<class_w$}  {:<geom_w$}  {:<focused_w$}  TITLE",
                    "ID", "PID", "CLASS", "GEOMETRY", "FOCUSED"
                );
                for w in &windows {
                    let id_str =
                        w.id.map(|id| id.to_string())
                            .unwrap_or_else(|| "-".to_string());
                    let pid_str = w
                        .pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let geom_str = format!("{}x{}@{},{}", w.w, w.h, w.x, w.y);
                    let focused_str = if w.focused { "*" } else { "" };
                    println!(
                        "{:<id_w$}  {:<pid_w$}  {:<class_w$}  {:<geom_w$}  {:<focused_w$}  {}",
                        id_str, pid_str, w.class, geom_str, focused_str, w.title
                    );
                }
            }
        }
    }
    Ok(())
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
    }

    #[tokio::test]
    async fn test_run_query_pointer() {
        let backend = MockQueryBackend {
            workareas: vec![],
            layout: None,
            windows: vec![],
            cursor: (100, 200),
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
                    focused: true,
                    maximized: false,
                    minimized: false,
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
                    focused: false,
                    maximized: false,
                    minimized: false,
                },
            ],
            cursor: (0, 0),
        };

        // Human readable table output
        assert!(run_query(&backend, QueryCommands::Windows { json: false })
            .await
            .is_ok());

        // JSON output
        assert!(run_query(&backend, QueryCommands::Windows { json: true })
            .await
            .is_ok());
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
                focused: true,
                maximized: false,
                minimized: false,
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
                focused: false,
                maximized: true,
                minimized: false,
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
        assert_eq!(arr[0]["focused"], true);
        assert_eq!(arr[0]["maximized"], false);
        assert_eq!(arr[0]["minimized"], false);

        assert!(arr[1]["id"].is_null());
        assert!(arr[1]["pid"].is_null());
        assert_eq!(arr[1]["maximized"], true);
    }
}
