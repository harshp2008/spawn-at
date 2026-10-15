//! What a backend can do, and narrow views of the backend trait.
//!
//! [`Capabilities`] is plain data a backend returns from `CompositorBackend::capabilities`; it
//! replaces asking three separate `supports_*` methods. [`WindowQuery`], [`WindowControl`] and
//! [`ReadySignal`] are the narrow parts of [`CompositorBackend`]; every `CompositorBackend` is
//! all three through the blanket impls below, so a backend implements one trait and the test
//! doubles of the commands keep working. Code that only reads windows can ask for
//! `WindowQuery` and a test needs no more than that.
//!
//! The methods of the narrow traits have the names of the `CompositorBackend` methods they
//! forward to; do not `use` them next to `CompositorBackend` (the call would be ambiguous), name
//! them in a bound instead.

use super::{
    ClaimSubscription, CompositorBackend, DriverError, MonitorLayout, Rect, WindowMetadata,
    WindowState,
};
use spawn_at_core::geometry::PlacementParams;

/// What a backend can do. Returned by `CompositorBackend::capabilities`; the default is built
/// from the `supports_*` methods, so a backend that only has those keeps its behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
// `reveal_control` and `needs_helper` are declared by the GNOME driver and checked in tests; no
// command reads them yet (the first reader comes with the next backend).
#[allow(dead_code)]
pub struct Capabilities {
    /// `transform_window` works on an existing window.
    pub runtime_transform: bool,
    /// The backend can tell when an armed window was placed (`prepare_claim_wait`).
    pub claim_wait: bool,
    /// `close_window` works.
    pub close: bool,
    /// A new window can be held invisible until it is placed (GNOME "cloak").
    pub reveal_control: bool,
    /// The backend needs a helper installed in the compositor (the GNOME extension).
    pub needs_helper: bool,
}

/// Reading windows, monitors and the pointer.
// Only `get_windows` and `get_window_rect` are asked through this view so far (the wait helpers
// in `commands/mod.rs`); the rest completes the view for the next backend.
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait WindowQuery: Send + Sync {
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError>;
    async fn get_window_rect(&self, id: u64) -> Result<Rect, DriverError>;
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError>;
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError>;
    async fn get_layout(&self) -> Result<MonitorLayout, DriverError>;
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError>;
    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String;
}

/// Acting on windows: placing, state, focus, close.
// No command asks for this narrow view yet; see the module docs.
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait WindowControl: Send + Sync {
    async fn transform_window(
        &self,
        target_id: &str,
        params: PlacementParams,
        current_w: u32,
        current_h: u32,
    ) -> Result<(), DriverError>;
    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError>;
    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError>;
    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError>;
    async fn close_window(&self, target_id: &str) -> Result<(), DriverError>;
}

/// Knowing when an armed window was placed.
// No command asks for this narrow view yet; see the module docs.
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait ReadySignal: Send + Sync {
    async fn prepare_claim_wait(
        &self,
        target_id: &str,
    ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError>;
}

#[async_trait::async_trait]
impl<T: CompositorBackend + ?Sized> WindowQuery for T {
    async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
        CompositorBackend::get_windows(self).await
    }
    async fn get_window_rect(&self, id: u64) -> Result<Rect, DriverError> {
        CompositorBackend::get_window_rect(self, id).await
    }
    async fn get_monitors(&self) -> Result<Vec<Rect>, DriverError> {
        CompositorBackend::get_monitors(self).await
    }
    async fn get_workareas(&self) -> Result<Vec<Rect>, DriverError> {
        CompositorBackend::get_workareas(self).await
    }
    async fn get_layout(&self) -> Result<MonitorLayout, DriverError> {
        CompositorBackend::get_layout(self).await
    }
    async fn get_cursor_position(&self) -> Result<(i32, i32), DriverError> {
        CompositorBackend::get_cursor_position(self).await
    }
    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        CompositorBackend::resolve_id(self, command, explicit_class)
    }
}

#[async_trait::async_trait]
impl<T: CompositorBackend + ?Sized> WindowControl for T {
    async fn transform_window(
        &self,
        target_id: &str,
        params: PlacementParams,
        current_w: u32,
        current_h: u32,
    ) -> Result<(), DriverError> {
        CompositorBackend::transform_window(self, target_id, params, current_w, current_h).await
    }
    async fn set_window_state(
        &self,
        target_id: &str,
        state: WindowState,
    ) -> Result<(), DriverError> {
        CompositorBackend::set_window_state(self, target_id, state).await
    }
    async fn focus_window(&self, target_id: &str) -> Result<(), DriverError> {
        CompositorBackend::focus_window(self, target_id).await
    }
    async fn defocus_window(
        &self,
        target_id: &str,
        mode: &str,
        destination: &str,
    ) -> Result<(), DriverError> {
        CompositorBackend::defocus_window(self, target_id, mode, destination).await
    }
    async fn close_window(&self, target_id: &str) -> Result<(), DriverError> {
        CompositorBackend::close_window(self, target_id).await
    }
}

#[async_trait::async_trait]
impl<T: CompositorBackend + ?Sized> ReadySignal for T {
    async fn prepare_claim_wait(
        &self,
        target_id: &str,
    ) -> Result<Option<Box<dyn ClaimSubscription>>, DriverError> {
        CompositorBackend::prepare_claim_wait(self, target_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux::x11::X11Driver;
    use crate::platform::{Armed, Batch, Driver};

    /// A backend that only answers `get_windows` and says nothing about capabilities.
    struct Minimal;

    #[async_trait::async_trait]
    impl Driver for Minimal {
        async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
            Ok(Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for Minimal {
        fn name(&self) -> &'static str {
            "minimal"
        }
        fn resolve_id(&self, _command: &[String], _class: Option<&str>) -> String {
            "resolved".into()
        }
        fn supports_close(&self) -> bool {
            true
        }
        async fn get_windows(&self) -> Result<Vec<WindowMetadata>, DriverError> {
            Ok(vec![WindowMetadata {
                id: Some(7),
                ..Default::default()
            }])
        }
    }

    /// The default `capabilities()` is built from the `supports_*` methods: a backend that
    /// overrides only those (as every existing test double does) reports exactly them.
    #[test]
    fn test_default_capabilities_follow_the_supports_methods() {
        let caps = Minimal.capabilities();
        assert_eq!(
            caps,
            Capabilities {
                runtime_transform: false,
                claim_wait: false,
                close: true,
                reveal_control: false,
                needs_helper: false,
            }
        );
    }

    /// The X11 driver reports what its `supports_*` methods say, and needs no helper.
    #[test]
    fn test_x11_capabilities_equal_its_supports_methods() {
        let d = X11Driver;
        let c = d.capabilities();
        assert_eq!(c.runtime_transform, d.supports_runtime_transform());
        assert_eq!(c.claim_wait, d.supports_claim_wait());
        assert_eq!(c.close, d.supports_close());
        assert!(!c.reveal_control && !c.needs_helper);
    }

    /// `LinuxBackend` forwards the capabilities of the driver it wraps.
    #[test]
    fn test_linux_backend_forwards_capabilities() {
        let b = crate::platform::linux::LinuxBackend::new(Box::new(X11Driver));
        assert_eq!(b.capabilities(), X11Driver.capabilities());
    }

    /// The narrow views forward to the backend: a bound on `WindowQuery` is enough to read
    /// windows, and the same value is a `WindowControl` and a `ReadySignal`.
    #[tokio::test]
    async fn test_narrow_views_forward_to_the_backend() {
        async fn ids<Q: WindowQuery + ?Sized>(q: &Q) -> Vec<Option<u64>> {
            q.get_windows()
                .await
                .unwrap()
                .iter()
                .map(|w| w.id)
                .collect()
        }
        async fn unsupported<C: WindowControl + ?Sized>(c: &C) -> bool {
            c.close_window("x").await.is_err()
        }
        async fn no_claim<R: ReadySignal + ?Sized>(r: &R) -> bool {
            r.prepare_claim_wait("x").await.unwrap().is_none()
        }
        assert_eq!(ids(&Minimal).await, vec![Some(7)]);
        let as_dyn: &dyn CompositorBackend = &Minimal;
        assert_eq!(ids(as_dyn).await, vec![Some(7)]);
        assert_eq!(WindowQuery::resolve_id(&Minimal, &[], None), "resolved");
        assert!(unsupported(&Minimal).await);
        assert!(no_claim(&Minimal).await);
    }
}
