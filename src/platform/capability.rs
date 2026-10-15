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

/// What a command needs from the backend's helper (for GNOME: the Shell extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperNeed {
    /// Placing a window against the whole screen (`--area screen`): extension protocol 4.
    ScreenArea,
    /// Monitor and state columns of `query windows`: extension protocol 5.
    MonitorAndState,
}

impl HelperNeed {
    /// The lowest helper protocol that provides it.
    pub fn min_protocol(self) -> u32 {
        match self {
            HelperNeed::ScreenArea => 4,
            HelperNeed::MonitorAndState => 5,
        }
    }
}

/// Whether the backend's helper is new enough for a [`HelperNeed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperStatus {
    /// New enough, or the backend does not report a protocol (no helper to be outdated).
    Ok,
    /// The helper reports `found`, the command needs `needed`.
    Outdated { found: u32, needed: u32 },
}

impl HelperStatus {
    /// The status for the protocol a backend reported (`None`: it reports none).
    pub fn from_protocol(reported: Option<u32>, need: HelperNeed) -> Self {
        match reported {
            Some(found) if found < need.min_protocol() => HelperStatus::Outdated {
                found,
                needed: need.min_protocol(),
            },
            _ => HelperStatus::Ok,
        }
    }

    /// The message for an outdated helper, word for word what the commands printed before this
    /// type existed; `None` when it is not outdated.
    pub fn message(&self, need: HelperNeed) -> Option<String> {
        match (*self, need) {
            (HelperStatus::Ok, _) => None,
            (HelperStatus::Outdated { found, needed }, HelperNeed::ScreenArea) => Some(format!(
                "extension is older than the CLI expects (protocol {}, need {}): run `spawn-at install` and log out and back in",
                found, needed
            )),
            (HelperStatus::Outdated { found, needed }, HelperNeed::MonitorAndState) => Some(format!(
                "extension is older than the CLI expects (protocol {}, need {}): MON and STATE are unavailable; run `spawn-at install` and log out and back in",
                found, needed
            )),
        }
    }
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

#[cfg(test)]
mod helper_tests {
    use super::*;
    use crate::platform::{Armed, Batch, Driver};

    /// A backend that reports a protocol number (or none).
    struct Reports(Option<u32>);

    #[async_trait::async_trait]
    impl Driver for Reports {
        async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
            Ok(Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for Reports {
        fn name(&self) -> &'static str {
            "reports"
        }
        fn resolve_id(&self, _c: &[String], _k: Option<&str>) -> String {
            String::new()
        }
        async fn protocol_version(&self) -> Option<u32> {
            self.0
        }
    }

    /// The thresholds are the ones the commands used: 4 for the screen area, 5 for monitor and
    /// state; a backend that reports no protocol is never outdated.
    #[test]
    fn test_helper_status_thresholds() {
        use HelperNeed::*;
        assert_eq!(ScreenArea.min_protocol(), 4);
        assert_eq!(MonitorAndState.min_protocol(), 5);
        assert_eq!(
            HelperStatus::from_protocol(None, ScreenArea),
            HelperStatus::Ok
        );
        assert_eq!(
            HelperStatus::from_protocol(Some(4), ScreenArea),
            HelperStatus::Ok
        );
        assert_eq!(
            HelperStatus::from_protocol(Some(3), ScreenArea),
            HelperStatus::Outdated {
                found: 3,
                needed: 4
            }
        );
        assert_eq!(
            HelperStatus::from_protocol(Some(5), MonitorAndState),
            HelperStatus::Ok
        );
        assert_eq!(
            HelperStatus::from_protocol(Some(4), MonitorAndState),
            HelperStatus::Outdated {
                found: 4,
                needed: 5
            }
        );
    }

    /// The messages are byte for byte the ones the commands printed before.
    #[test]
    fn test_helper_messages_are_unchanged() {
        let screen = HelperStatus::Outdated {
            found: 3,
            needed: 4,
        };
        assert_eq!(
            screen.message(HelperNeed::ScreenArea).unwrap(),
            "extension is older than the CLI expects (protocol 3, need 4): run `spawn-at install` and log out and back in"
        );
        let mon = HelperStatus::Outdated {
            found: 4,
            needed: 5,
        };
        assert_eq!(
            mon.message(HelperNeed::MonitorAndState).unwrap(),
            "extension is older than the CLI expects (protocol 4, need 5): MON and STATE are unavailable; run `spawn-at install` and log out and back in"
        );
        assert_eq!(HelperStatus::Ok.message(HelperNeed::ScreenArea), None);
    }

    /// The trait default asks `protocol_version`, so every backend and test double that reports a
    /// protocol gets the check without implementing anything.
    #[tokio::test]
    async fn test_default_helper_status_reads_the_reported_protocol() {
        assert_eq!(
            Reports(Some(2)).helper_status(HelperNeed::ScreenArea).await,
            HelperStatus::Outdated {
                found: 2,
                needed: 4
            }
        );
        assert_eq!(
            Reports(None).helper_status(HelperNeed::ScreenArea).await,
            HelperStatus::Ok
        );
        assert_eq!(
            Reports(Some(6))
                .helper_status(HelperNeed::MonitorAndState)
                .await,
            HelperStatus::Ok
        );
    }
}
