//! Which `spawn-at` backend a run is aimed at, and what follows from it
//! (`docs/DESIGN_RUNNER_SETUP.md` section 4).
//!
//! The verify crate cannot ask the binary which backend it picked (that would be a product
//! change), so it mirrors the binary's rule from the same two variables: `SPAWN_AT_BACKEND`
//! (compared after lowercasing, nothing trimmed; only `x11` and `sway` count, any other value is
//! ignored) and `XDG_CURRENT_DESKTOP`. Mirror, not truth: it cannot know that the GNOME driver
//! failed to reach the session bus and the binary fell back to X11.
//!
//! Three things hang on the answer: a spec's `[requires] backend` list (including the new value
//! `sway`), which log source the runner uses (the GNOME journal and `SetLogging` over D-Bus belong
//! to the GNOME extension and must never be touched when another backend is selected), and the
//! refusal to run against a Sway that is not the private headless one.

use crate::probe::EnvironmentInfo;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Name of the marker file `scripts/sway-headless.sh` puts in its private directory.
pub const SWAY_HARNESS_MARKER: &str = ".spawn-at-sway-harness";

/// Prefix of the private directory's name (`spawn-at-sway.XXXXXX`).
pub const SWAY_HARNESS_DIR_PREFIX: &str = "spawn-at-sway.";

/// The backend `SPAWN_AT_BACKEND` forces, if it forces one.
pub fn forced_backend(var: &str) -> Option<&'static str> {
    match var.to_lowercase().as_str() {
        "x11" => Some("x11"),
        "sway" => Some("sway"),
        _ => None,
    }
}

/// The backend the product would pick: a forced `x11`/`sway`, else `gnome` for a desktop whose
/// name contains `gnome`, else `x11` for an X11 session, else `none` (the product refuses it).
pub fn selected_backend(desktop: &str, session_type: &str, forced_var: &str) -> &'static str {
    if let Some(b) = forced_backend(forced_var) {
        return b;
    }
    if desktop.to_lowercase().contains("gnome") {
        "gnome"
    } else if session_type.eq_ignore_ascii_case("x11") {
        "x11"
    } else {
        "none"
    }
}

/// True when the GNOME journal log source and the extension's `SetLogging` control apply: the
/// desktop is GNOME **and** no other backend is forced. With `SPAWN_AT_BACKEND=sway` set in a
/// GNOME user's shell they must not be used, or a "Sway" run would call `gdbus` on the live
/// GNOME session's extension.
pub fn gnome_journal_applies(desktop: &str, forced_var: &str) -> bool {
    desktop.to_lowercase().contains("gnome") && forced_backend(forced_var).is_none()
}

/// Whether a spec's `[requires] backend` list is met by `backend` (and `is_x11_session`, which
/// keeps the old meaning of `x11`).
pub fn requirement_met(required: &[String], backend: &str, is_x11_session: bool) -> bool {
    required.iter().any(|b| match b.as_str() {
        "gnome" => backend == "gnome",
        "x11" => is_x11_session || backend == "x11",
        "sway" => backend == "sway",
        _ => false,
    })
}

impl EnvironmentInfo {
    /// The backend this environment selects, given the value of `SPAWN_AT_BACKEND`.
    pub fn backend_with(&self, forced_var: &str) -> &'static str {
        selected_backend(&self.desktop, &self.session_type, forced_var)
    }

    /// [`EnvironmentInfo::backend_with`] with the process's `SPAWN_AT_BACKEND`.
    pub fn backend(&self) -> &'static str {
        self.backend_with(&std::env::var("SPAWN_AT_BACKEND").unwrap_or_default())
    }

    /// The requirement check of a version 2 spec, with the process's `SPAWN_AT_BACKEND`.
    pub fn check_requirements_v2(&self, req: &crate::schema::TestRequires) -> Result<(), String> {
        self.check_requirements_v2_with(req, &std::env::var("SPAWN_AT_BACKEND").unwrap_or_default())
    }

    /// Like [`EnvironmentInfo::check_requirements`], but a backend that does not meet the spec
    /// gives the fixed text `unsupported on this backend (<backend>): spec requires <list>` (the
    /// result is `Skipped`, not a failure).
    pub fn check_requirements_v2_with(
        &self,
        req: &crate::schema::TestRequires,
        forced_var: &str,
    ) -> Result<(), String> {
        let backend = self.backend_with(forced_var);
        let any = req.backend.iter().any(|b| b == "any");
        if !any && !requirement_met(&req.backend, backend, self.session_type == "x11") {
            return Err(format!(
                "unsupported on this backend ({backend}): spec requires {}",
                req.backend.join(" or ")
            ));
        }
        // Session, GNOME version and monitors are judged as for any spec.
        let mut rest = req.clone();
        rest.backend = vec!["any".to_string()];
        self.check_requirements(&rest)
    }
}

/// Refuses a Sway that is not the private headless session of `scripts/sway-headless.sh`.
///
/// `swaysock` is `$SWAYSOCK`, `runtime_dir` is `$XDG_RUNTIME_DIR`, `uid` the effective user id.
/// The socket must sit directly in a directory that is named `spawn-at-sway.*`, carries the
/// harness marker file, belongs to `uid`, and is not the real runtime directory. These are the
/// checks of `check_dir` in `scripts/sway-headless.sh`, so "never point the runner at the
/// owner's real Sway session" is a property of the runner and not only of the wrapper script.
pub fn private_sway_guard(
    swaysock: Option<&str>,
    runtime_dir: Option<&str>,
    uid: u32,
) -> Result<(), String> {
    let refuse = |why: String| {
        Err(format!(
            "refusing to run against a Sway that is not the private headless session of scripts/sway-headless.sh: {why}"
        ))
    };
    let Some(sock) = swaysock.filter(|s| !s.is_empty()) else {
        return refuse("SWAYSOCK is not set".into());
    };
    let Some(dir) = Path::new(sock).parent().filter(|d| d.is_absolute()) else {
        return refuse(format!("SWAYSOCK '{sock}' is not an absolute path"));
    };
    let dir = match dir.canonicalize() {
        Ok(d) => d,
        Err(e) => return refuse(format!("cannot read '{}': {e}", dir.display())),
    };
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !name.starts_with(SWAY_HARNESS_DIR_PREFIX) {
        return refuse(format!(
            "'{}' is not named {SWAY_HARNESS_DIR_PREFIX}*",
            dir.display()
        ));
    }
    if !dir.join(SWAY_HARNESS_MARKER).is_file() {
        return refuse(format!(
            "'{}' has no {SWAY_HARNESS_MARKER} marker",
            dir.display()
        ));
    }
    match std::fs::metadata(&dir) {
        Ok(m) if m.uid() == uid => {}
        Ok(_) => return refuse(format!("'{}' is not owned by you", dir.display())),
        Err(e) => return refuse(format!("cannot read '{}': {e}", dir.display())),
    }
    if let Some(rt) = runtime_dir.filter(|r| !r.is_empty()) {
        if Path::new(rt).canonicalize().ok().as_deref() == Some(dir.as_path()) {
            return refuse(format!("'{}' is the real XDG_RUNTIME_DIR", dir.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::TestRequires;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    fn env(desktop: &str, session: &str) -> EnvironmentInfo {
        EnvironmentInfo {
            session_type: session.into(),
            desktop: desktop.into(),
            gnome_version: None,
            monitors_count: 1,
            primary_workarea: None,
            independent_workarea: None,
        }
    }

    fn requires(backend: &[&str]) -> TestRequires {
        TestRequires {
            backend: backend.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    /// Only `x11` and `sway` force a backend, compared after lowercasing and nothing trimmed, as in
    /// the binary's registry (`src/platform/linux/registry.rs`).
    #[test]
    fn test_forced_backend_follows_the_registry_rule() {
        assert_eq!(forced_backend("x11"), Some("x11"));
        assert_eq!(forced_backend("X11"), Some("x11"));
        assert_eq!(forced_backend("sway"), Some("sway"));
        assert_eq!(forced_backend("SWAY"), Some("sway"));
        for ignored in ["", " sway", "sway ", "swayx", "gnome", "wayland"] {
            assert_eq!(forced_backend(ignored), None, "{ignored:?}");
        }
    }

    #[test]
    fn test_selected_backend() {
        assert_eq!(selected_backend("ubuntu:gnome", "wayland", ""), "gnome");
        assert_eq!(selected_backend("gnome", "x11", ""), "gnome");
        assert_eq!(selected_backend("kde", "x11", ""), "x11");
        assert_eq!(selected_backend("kde", "wayland", ""), "none");
        assert_eq!(selected_backend("gnome", "wayland", "sway"), "sway");
        assert_eq!(selected_backend("gnome", "wayland", "x11"), "x11");
        assert_eq!(selected_backend("gnome", "wayland", "swayx"), "gnome");
        assert_eq!(selected_backend("", "", "SWAY"), "sway");
    }

    /// The journal fix: a forced non-GNOME backend turns the GNOME log source and `SetLogging`
    /// off even in a GNOME session, so a Sway run never calls the live extension.
    #[test]
    fn test_gnome_journal_is_not_used_when_another_backend_is_forced() {
        assert!(gnome_journal_applies("ubuntu:GNOME", ""));
        assert!(
            gnome_journal_applies("GNOME", "swayx"),
            "an unknown value is ignored"
        );
        assert!(!gnome_journal_applies("GNOME", "sway"));
        assert!(!gnome_journal_applies("GNOME", "SWAY"));
        assert!(!gnome_journal_applies("GNOME", "x11"));
        assert!(!gnome_journal_applies("KDE", ""));
        assert!(!gnome_journal_applies("", ""));
    }

    #[test]
    fn test_requirement_met() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(requirement_met(&s(&["gnome"]), "gnome", false));
        assert!(!requirement_met(&s(&["gnome"]), "sway", false));
        assert!(requirement_met(&s(&["gnome", "sway"]), "sway", false));
        assert!(
            requirement_met(&s(&["x11"]), "gnome", true),
            "the old x11 meaning: an X11 session"
        );
        assert!(requirement_met(&s(&["x11"]), "x11", false));
        assert!(!requirement_met(&s(&["sway"]), "gnome", false));
        assert!(!requirement_met(&s(&["mir"]), "gnome", false));
    }

    #[test]
    fn test_v2_requirement_mismatch_is_the_fixed_unsupported_message() {
        let gnome = env("gnome", "wayland");
        assert_eq!(
            gnome
                .check_requirements_v2_with(&requires(&["sway"]), "")
                .unwrap_err(),
            "unsupported on this backend (gnome): spec requires sway"
        );
        assert_eq!(
            gnome
                .check_requirements_v2_with(&requires(&["gnome"]), "sway")
                .unwrap_err(),
            "unsupported on this backend (sway): spec requires gnome"
        );
        assert_eq!(
            gnome
                .check_requirements_v2_with(&requires(&["gnome", "x11"]), "sway")
                .unwrap_err(),
            "unsupported on this backend (sway): spec requires gnome or x11"
        );
        assert!(gnome
            .check_requirements_v2_with(&requires(&["gnome", "sway"]), "sway")
            .is_ok());
        assert!(gnome
            .check_requirements_v2_with(&requires(&["any"]), "sway")
            .is_ok());
        assert!(gnome
            .check_requirements_v2_with(&requires(&["gnome"]), "")
            .is_ok());
    }

    #[test]
    fn test_v2_requirement_still_judges_the_session() {
        let mut req = requires(&["any"]);
        req.session = vec!["x11".into()];
        let e = env("gnome", "wayland")
            .check_requirements_v2_with(&req, "")
            .unwrap_err();
        assert!(e.contains("does not match requirement"), "{e}");
    }

    // ---- the private Sway guard ----

    struct Dirs {
        root: PathBuf,
    }

    impl Dirs {
        fn new() -> Dirs {
            let root = std::env::temp_dir().join(format!(
                "spawn-at-guard-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&root).unwrap();
            Dirs { root }
        }
        fn harness(&self, name: &str, marker: bool) -> PathBuf {
            let d = self.root.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
            if marker {
                std::fs::write(d.join(SWAY_HARNESS_MARKER), "").unwrap();
            }
            d
        }
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn uid() -> u32 {
        // SAFETY: geteuid has no preconditions.
        unsafe { libc::geteuid() }
    }

    fn sock(dir: &Path) -> String {
        dir.join("sway-ipc.1.1.sock").to_string_lossy().to_string()
    }

    #[test]
    fn test_guard_accepts_the_private_harness_directory() {
        let t = Dirs::new();
        let d = t.harness("spawn-at-sway.AbC123", true);
        assert!(private_sway_guard(Some(&sock(&d)), Some("/run/user/1000"), uid()).is_ok());
        assert!(private_sway_guard(Some(&sock(&d)), None, uid()).is_ok());
    }

    #[test]
    fn test_guard_refuses_everything_else() {
        let t = Dirs::new();
        let good = t.harness("spawn-at-sway.AbC123", true);
        let r =
            |s: Option<&str>, rt: Option<&str>, u: u32| private_sway_guard(s, rt, u).unwrap_err();

        assert!(r(None, None, uid()).contains("SWAYSOCK is not set"));
        assert!(r(Some(""), None, uid()).contains("SWAYSOCK is not set"));
        assert!(r(Some("sway-ipc.sock"), None, uid()).contains("not an absolute path"));
        assert!(r(Some("/nonexistent-dir/sway-ipc.sock"), None, uid()).contains("cannot read"));

        // The real session's socket lives in the user's runtime directory.
        let real = t.harness("run-user-1000", true);
        assert!(r(Some(&sock(&real)), None, uid()).contains("is not named spawn-at-sway."));

        // Named like the harness but without the marker the script writes.
        let unmarked = t.harness("spawn-at-sway.nomark", false);
        assert!(r(Some(&sock(&unmarked)), None, uid()).contains("marker"));

        // Someone else's directory.
        assert!(r(Some(&sock(&good)), None, uid() + 1).contains("not owned by you"));

        // The real runtime directory, even if it looks like the harness.
        assert!(r(Some(&sock(&good)), Some(good.to_str().unwrap()), uid())
            .contains("real XDG_RUNTIME_DIR"));
    }
}
