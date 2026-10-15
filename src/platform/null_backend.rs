//! The backend of an operating system spawn-at does not support.
//!
//! `main` stops at `platform::ensure_supported_os()` before a backend is asked for, so on a
//! supported system this is never reached; it exists so `init_backend` has a value of the right
//! type on every OS and every operation says plainly what is wrong.

use super::{Armed, Batch, CompositorBackend, Driver, DriverError};

/// A backend that does nothing and refuses everything. Only `init_backend` off Linux builds it
/// (and the tests), so on Linux the compiler sees no use.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub struct NullBackend;

#[cfg_attr(target_os = "linux", allow(dead_code))]
const WHY: &str = "Unsupported operating system";

#[async_trait::async_trait]
impl Driver for NullBackend {
    async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
        Err(DriverError::UnsupportedCapability(WHY))
    }
}

#[async_trait::async_trait]
impl CompositorBackend for NullBackend {
    fn name(&self) -> &'static str {
        "unsupported"
    }

    fn resolve_id(&self, command: &[String], explicit_class: Option<&str>) -> String {
        explicit_class
            .map(str::to_string)
            .or_else(|| command.first().cloned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything the null backend is asked refuses with the same words the old non-Linux
    /// `init_backend` error had; it needs nothing installed and has no capability.
    #[tokio::test]
    async fn test_null_backend_refuses_everything() {
        let b = NullBackend;
        assert!(b.provisioner().is_none());
        assert_eq!(
            b.capabilities(),
            crate::platform::capability::Capabilities::default()
        );
        let e = b.get_windows().await.unwrap_err().to_string();
        assert!(e.contains("get_windows"), "{e}");
        let e = b
            .arm(Batch {
                id: 0,
                entries: vec![],
                reveal: spawn_at_core::driver::Reveal::Independent,
                focus: spawn_at_core::driver::FocusIntent::Leave,
                urgency: spawn_at_core::driver::Urgency::Normal,
                deadline: std::time::Duration::from_secs(1),
            })
            .await
            .unwrap_err()
            .to_string();
        assert!(e.contains(WHY), "{e}");
        assert_eq!(b.resolve_id(&["x".into()], None), "x");
        assert_eq!(b.resolve_id(&["x".into()], Some("c")), "c");
    }
}
