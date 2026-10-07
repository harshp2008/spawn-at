//! # Command Handlers
//!
//! This module contains platform-agnostic implementations of the CLI commands.
//! It isolates CLI routing logic from the compositor-specific execution logic.

pub mod focus;
pub mod lifecycle;
pub mod query;
pub mod transform;

pub use focus::{apply_focus_policy, run_defocus, run_focus};
pub use lifecycle::{run_maximize, run_minimize, run_restore, run_unminimize};
pub use query::run_query;
pub use transform::run_transform;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{MaximizeArgs, TransformArgs, WindowTargetArgs};
    use crate::platform::linux::x11::X11Driver;

    #[tokio::test]
    async fn test_transform_supported_on_x11() {
        let driver = X11Driver;
        let args = TransformArgs {
            class: Some("alacritty".into()),
            ..Default::default()
        };
        let err = run_transform(&driver, args).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("The active compositor does not support this feature"));
    }

    #[tokio::test]
    async fn test_focus_supported_on_x11() {
        let driver = X11Driver;
        let args = WindowTargetArgs {
            class: Some("alacritty".into()),
            ..Default::default()
        };
        let err = run_focus(&driver, args).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("The active compositor does not support this feature"));
    }

    #[tokio::test]
    async fn test_lifecycle_supported_on_x11() {
        let driver = X11Driver;
        let args = MaximizeArgs {
            target: WindowTargetArgs {
                class: Some("alacritty".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let err = run_maximize(&driver, args).await.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("The active compositor does not support this feature"));
    }
}
