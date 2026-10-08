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
            let layout = backend.get_workareas().await?;
            if json {
                println!("{}", serde_json::to_string(&layout)?);
            } else {
                println!(
                    "{:<7}  {:<15}  {:<6}  {:<6}  {:<6}  {}",
                    "MONITOR", "GEOMETRY", "X", "Y", "WIDTH", "HEIGHT"
                );
                for (i, area) in layout.iter().enumerate() {
                    let geom = format!("{}x{}@{},{}", area.width, area.height, area.x, area.y);
                    println!(
                        "{:<7}  {:<15}  {:<6}  {:<6}  {:<6}  {}",
                        format!("#{}", i),
                        geom,
                        area.x,
                        area.y,
                        area.width,
                        area.height
                    );
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
                    "{:<id_w$}  {:<pid_w$}  {:<class_w$}  {:<geom_w$}  {:<focused_w$}  {}",
                    "ID", "PID", "CLASS", "GEOMETRY", "FOCUSED", "TITLE"
                );
                for w in &windows {
                    let id_str = w
                        .id
                        .map(|id| id.to_string())
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
    use crate::core::geometry::Rect;
    use crate::platform::DriverError;
    use crate::target::WindowMetadata;

    struct MockQueryBackend {
        workareas: Vec<Rect>,
        windows: Vec<WindowMetadata>,
        cursor: (i32, i32),
    }

    #[async_trait::async_trait]
    impl crate::platform::Driver for MockQueryBackend {
        async fn arm(&self, _batch: crate::platform::Batch) -> Result<crate::platform::Armed, DriverError> {
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
        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            Ok(self.windows.clone())
        }
    }

    #[tokio::test]
    async fn test_run_query_pointer() {
        let backend = MockQueryBackend {
            workareas: vec![],
            windows: vec![],
            cursor: (100, 200),
        };
        assert!(run_query(&backend, QueryCommands::Pointer).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_query_windows_table_and_json() {
        let backend = MockQueryBackend {
            workareas: vec![],
            windows: vec![
                WindowMetadata {
                    id: Some(706192865),
                    pid: Some(75448),
                    title: "spawn-at - Antigravity IDE - cli.rs".to_string(),
                    class: "antigravity-ide".to_string(),
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
}
