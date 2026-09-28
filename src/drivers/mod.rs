pub mod gnome;
pub mod x11;

use crate::geometry::{Rect, WindowInfo};
use std::env;
use std::error::Error;

pub enum CompositorDriver {
    Gnome(gnome::GnomeDriver),
    X11(x11::X11Driver),
}

impl CompositorDriver {
    pub async fn get_cursor(&self) -> Result<(i32, i32), Box<dyn Error>> {
        match self {
            Self::Gnome(d) => d.get_cursor().await,
            Self::X11(d) => d.get_cursor().await,
        }
    }

    pub async fn get_monitors(&self) -> Result<Vec<Rect>, Box<dyn Error>> {
        match self {
            Self::Gnome(d) => d.get_monitors().await,
            Self::X11(d) => d.get_monitors().await,
        }
    }

    pub async fn move_window(
        &self,
        win_id: &str,
        target_x: i32,
        target_y: i32,
        w: Option<i32>,
        h: Option<i32>,
        monitor: &Rect,
        bound_top: i32,
        bound_bottom: i32,
        bound_left: i32,
        bound_right: i32,
        global_margin: i32,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            Self::Gnome(d) => {
                d.move_window(
                    win_id,
                    target_x,
                    target_y,
                    w,
                    h,
                    monitor,
                    bound_top,
                    bound_bottom,
                    bound_left,
                    bound_right,
                    global_margin,
                )
                .await
            }
            Self::X11(d) => {
                d.move_window(
                    win_id,
                    target_x,
                    target_y,
                    w,
                    h,
                    monitor,
                    bound_top,
                    bound_bottom,
                    bound_left,
                    bound_right,
                    global_margin,
                )
                .await
            }
        }
    }

    pub async fn get_windows(&self) -> Result<Vec<WindowInfo>, Box<dyn Error>> {
        match self {
            Self::Gnome(d) => d.get_windows().await,
            Self::X11(d) => d.get_windows().await,
        }
    }
}

pub async fn detect_driver() -> Result<CompositorDriver, Box<dyn Error>> {
    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    let session = env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase();

    if desktop.contains("gnome") && session == "wayland" {
        Ok(CompositorDriver::Gnome(gnome::GnomeDriver::new().await?))
    } else {
        Ok(CompositorDriver::X11(x11::X11Driver::new()))
    }
}
