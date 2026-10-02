use crate::config::{Config, WindowMode};
use crate::geometry::{self, Rect};
use zbus::{proxy, Connection};
use futures_util::stream::StreamExt;

#[proxy(
    default_service = "org.gnome.Shell",
    default_path = "/org/gnome/Shell/Extensions/SpawnAt",
    interface = "org.gnome.Shell.Extensions.SpawnAt"
)]
pub trait SpawnAt {
    fn arm(&self, identifier: &str, x: i32, y: i32, w: i32, h: i32) -> zbus::Result<()>;
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

pub async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load().unwrap_or_default();
    let conn = Connection::session().await?;
    let proxy = SpawnAtProxy::new(&conn).await?;

    let mut stream = proxy.receive_workarea_changed().await?;
    
    // Initial sync
    sync_windows(&proxy, &config).await?;

    while let Some(_) = stream.next().await {
        sync_windows(&proxy, &config).await?;
    }

    Ok(())
}

async fn sync_windows(proxy: &SpawnAtProxy<'_>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let json_layout = proxy.get_workareas().await?;
    
    #[derive(serde::Deserialize, Debug)]
    struct LayoutRect { x: i32, y: i32, w: u32, h: u32 }
    
    let areas: Vec<LayoutRect> = serde_json::from_str(&json_layout).unwrap_or_default();
    let monitors: Vec<Rect> = areas.into_iter().map(|a| Rect { x: a.x, y: a.y, w: a.w, h: a.h }).collect();

    if monitors.is_empty() { return Ok(()); }

    let json_windows = proxy.get_windows().await?;
    #[derive(serde::Deserialize, Debug)]
    struct WindowRect {
        #[serde(alias = "class")]
        app_id: Option<String>,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    }
    
    let windows: Vec<WindowRect> = serde_json::from_str(&json_windows).unwrap_or_default();

    for win in windows {
        if let Some(app_id) = win.app_id {
            if let Some(rule) = config.apps.get(&app_id) {
                if rule.mode == WindowMode::ClampOnChange || rule.mode == WindowMode::PinnedBounds {
                    let win_rect = Rect { x: win.x, y: win.y, w: win.w, h: win.h };
                    let center_x = win_rect.x + (win_rect.w as i32 / 2);
                    let center_y = win_rect.y + (win_rect.h as i32 / 2);
                    let active_monitor = geometry::find_active_monitor(center_x, center_y, &monitors);
                    
                    let clamped = win_rect.clamp_to_bounds(active_monitor, rule.margin.unwrap_or(0));
                    
                    if clamped.x != win_rect.x || clamped.y != win_rect.y {
                        proxy.move_window(&app_id, clamped.x, clamped.y).await?;
                    }
                }
            }
        }
    }

    Ok(())
}
