use crate::config::{Config, WindowMode};
use crate::core::geometry::{self, Rect};
use crate::platform::DriverError;
use futures_util::stream::StreamExt;
use zbus::proxy;

#[proxy(
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/SpawnAt",
    interface = "org.gnome.Shell.Extensions.SpawnAt"
)]
pub trait SpawnAt {
    fn arm(
        &self,
        identifier: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        min_x: i32,
        max_x: i32,
        min_y: i32,
        max_y: i32,
    ) -> zbus::Result<()>;
    fn get_cursor(&self) -> zbus::Result<(i32, i32)>;
    fn get_pointer(&self) -> zbus::Result<(i32, i32)>;
    fn get_workareas(&self) -> zbus::Result<String>;
    fn get_windows(&self) -> zbus::Result<String>;
    fn move_window(&self, app_id: &str, x: i32, y: i32) -> zbus::Result<()>;
    fn move_resize_window(&self, target: &str, x: i32, y: i32, w: i32, h: i32) -> zbus::Result<bool>;
    fn focus_window(&self, target: &str) -> zbus::Result<bool>;
    fn defocus_window(&self, target: &str, to_target: &str) -> zbus::Result<bool>;
    fn set_window_state(&self, target: &str, state: &str) -> zbus::Result<bool>;
    #[zbus(signal)]
    fn workarea_changed(&self) -> zbus::Result<()>;
}

/// Runs the background daemon listening for workarea changes and enforcing window clamping.
pub async fn run_daemon(proxy: &SpawnAtProxy<'_>) -> Result<(), DriverError> {
    let config = Config::load().unwrap_or_default();
    let mut stream = proxy
        .receive_workarea_changed()
        .await
        .map_err(|e| DriverError::IpcError(e.to_string()))?;

    // Initial sync
    sync_windows(proxy, &config)
        .await
        .map_err(|e| DriverError::Execution(e.to_string().into()))?;

    while let Some(_) = stream.next().await {
        sync_windows(proxy, &config)
            .await
            .map_err(|e| DriverError::Execution(e.to_string().into()))?;
    }

    Ok(())
}

async fn sync_windows(
    proxy: &SpawnAtProxy<'_>,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let json_layout = proxy.get_workareas().await?;
    let monitors: Vec<Rect> = serde_json::from_str(&json_layout).unwrap_or_default();

    if monitors.is_empty() {
        return Ok(());
    }

    let json_windows = proxy.get_windows().await?;
    #[derive(serde::Deserialize, Debug)]
    struct WindowRect {
        #[serde(alias = "class")]
        app_id: Option<String>,
        x: i32,
        y: i32,
        #[serde(alias = "w")]
        width: u32,
        #[serde(alias = "h")]
        height: u32,
    }

    let windows: Vec<WindowRect> = serde_json::from_str(&json_windows).unwrap_or_default();

    for win in windows {
        if let Some(app_id) = win.app_id {
            if let Some(rule) = config.apps.get(&app_id) {
                if rule.mode == WindowMode::ClampOnChange || rule.mode == WindowMode::PinnedBounds {
                    let win_rect = Rect {
                        x: win.x,
                        y: win.y,
                        width: win.width,
                        height: win.height,
                    };
                    let center_x = win_rect.x + (win_rect.width as i32 / 2);
                    let center_y = win_rect.y + (win_rect.height as i32 / 2);
                    let active_monitor =
                        geometry::resolve_workarea(&monitors, (center_x, center_y), "cursor")
                            .unwrap_or(monitors[0]);

                    let clamped = geometry::clamp_to_bounds(
                        win_rect,
                        active_monitor,
                        0,
                        0,
                        0,
                        0,
                        rule.margin.unwrap_or(0) as i32,
                    );

                    if clamped.x != win_rect.x || clamped.y != win_rect.y {
                        proxy.move_window(&app_id, clamped.x, clamped.y).await?;
                    }
                }
            }
        }
    }

    Ok(())
}
