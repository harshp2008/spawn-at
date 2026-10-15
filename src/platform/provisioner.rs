//! Setup and teardown of what a backend needs on the machine (for GNOME: the Shell extension).
//!
//! A backend that needs something installed implements [`Provisioner`] and returns itself from
//! `CompositorBackend::provisioner`; a backend that needs nothing (X11) returns `None`, and the
//! `CompositorBackend::install` / `uninstall` defaults then print "No installation required".
//! The three `CompositorBackend` methods stay as thin forwarders so `main` and the test doubles
//! keep their calls.

use super::{DriverError, InstallArgs, UninstallArgs};

/// What a backend does for `spawn-at install` and `spawn-at uninstall`.
pub trait Provisioner: Send + Sync {
    /// A short notice to print (and confirm) before installing when this environment has not been
    /// tested; `None` when it has.
    fn install_notice(&self) -> Option<String>;
    /// Installs what the backend needs (for GNOME: deploys and enables the extension).
    fn install(&self, args: &InstallArgs) -> Result<(), DriverError>;
    /// Removes it again.
    fn uninstall(&self, args: &UninstallArgs) -> Result<(), DriverError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux::x11::X11Driver;
    use crate::platform::{Armed, Batch, CompositorBackend, Driver};

    /// A backend that needs nothing installed.
    struct Bare;

    #[async_trait::async_trait]
    impl Driver for Bare {
        async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
            Ok(Armed::default())
        }
    }

    #[async_trait::async_trait]
    impl CompositorBackend for Bare {
        fn name(&self) -> &'static str {
            "bare"
        }
        fn resolve_id(&self, _command: &[String], _class: Option<&str>) -> String {
            String::new()
        }
    }

    /// A backend with no provisioner needs nothing: no notice, and install and uninstall succeed
    /// (printing "No installation required for ...", the text they always had).
    #[test]
    fn test_backend_without_a_provisioner_needs_nothing() {
        assert!(Bare.provisioner().is_none());
        assert_eq!(Bare.install_notice(), None);
        assert!(Bare.install(&InstallArgs::default()).is_ok());
        assert!(Bare.uninstall(&UninstallArgs::default()).is_ok());
    }

    /// X11 needs nothing installed; `LinuxBackend` forwards that.
    #[test]
    fn test_x11_has_no_provisioner() {
        assert!(X11Driver.provisioner().is_none());
        let b = crate::platform::linux::LinuxBackend::new(Box::new(X11Driver));
        assert!(b.provisioner().is_none());
    }

    /// A backend that provides one: the `CompositorBackend` methods forward to it.
    #[test]
    fn test_backend_methods_forward_to_the_provisioner() {
        use std::sync::atomic::{AtomicU32, Ordering};
        struct Rec(AtomicU32);
        impl Provisioner for Rec {
            fn install_notice(&self) -> Option<String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Some("notice".into())
            }
            fn install(&self, _args: &InstallArgs) -> Result<(), DriverError> {
                self.0.fetch_add(10, Ordering::SeqCst);
                Ok(())
            }
            fn uninstall(&self, _args: &UninstallArgs) -> Result<(), DriverError> {
                self.0.fetch_add(100, Ordering::SeqCst);
                Ok(())
            }
        }
        struct WithRec(Rec);
        #[async_trait::async_trait]
        impl Driver for WithRec {
            async fn arm(&self, _batch: Batch) -> Result<Armed, DriverError> {
                Ok(Armed::default())
            }
        }
        #[async_trait::async_trait]
        impl CompositorBackend for WithRec {
            fn name(&self) -> &'static str {
                "with-rec"
            }
            fn resolve_id(&self, _c: &[String], _k: Option<&str>) -> String {
                String::new()
            }
            fn provisioner(&self) -> Option<&dyn Provisioner> {
                Some(&self.0)
            }
        }
        let b = WithRec(Rec(AtomicU32::new(0)));
        assert_eq!(b.install_notice().as_deref(), Some("notice"));
        b.install(&InstallArgs::default()).unwrap();
        b.uninstall(&UninstallArgs::default()).unwrap();
        assert_eq!((b.0).0.load(Ordering::SeqCst), 111);
    }
}
