//! The "log out to finish" follow-up of a headless update (#87).
//!
//! `spawn-at update --headless` (the login entry) rewrites the extension files after the Shell
//! has started, and on Wayland the Shell only loads them at the next log-in. When that happens
//! the update sends one desktop notification and writes a marker file,
//! `$XDG_STATE_HOME/spawn-at/needs-logout` (default `~/.local/state/spawn-at/needs-logout`).
//! `spawn` reads the marker: while the loaded extension's protocol is lower than the protocol in
//! the marker it prints one hint line, and once the loaded protocol has caught up it removes the
//! marker.
//!
//! The extension only reports a protocol number over D-Bus (no content hash), so a change that
//! keeps the protocol (for example a new Shell version in `metadata.json`) is announced once by
//! the notification but never produces the `spawn` hint: the marker is removed by the first
//! `spawn` that finds a loaded protocol equal to the marker's.
//!
//! Everything that decides something is pure over its arguments ([`classify`], [`announce`],
//! [`hint_action`]); only [`marker_path`], [`read_marker`], [`write_marker`], [`check_marker`] and
//! [`send_notification`] touch the machine.

use super::logout::DeployedFile;
use crate::platform::update_hooks::ChangedExtension;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The text of the notification and of the line printed to the update's own output.
pub const NOTICE_TEXT: &str = "spawn-at updated its GNOME extension; log out and back in to finish";

/// Upper bound for the `Notify` call, so a login entry never hangs on a silent bus.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(5);

/// What the marker file records: the protocol the deployed extension reports and a fingerprint
/// of the deployed files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub protocol: u32,
    pub fingerprint: String,
}

/// `<state home>/spawn-at/needs-logout` from the two environment values. `xdg_state_home` counts
/// only when it is a non-empty absolute path (the XDG rule: a relative value is ignored);
/// otherwise it is `<home>/.local/state`. `None` when neither gives a base.
pub fn marker_path_from(
    xdg_state_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let base = match xdg_state_home.map(PathBuf::from) {
        Some(p) if p.is_absolute() => p,
        _ => PathBuf::from(home.filter(|h| !h.is_empty())?).join(".local/state"),
    };
    Some(base.join("spawn-at").join("needs-logout"))
}

/// [`marker_path_from`] with `$XDG_STATE_HOME` and `$HOME`.
pub fn marker_path() -> Option<PathBuf> {
    marker_path_from(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// Two lines, `protocol=<n>` and `fingerprint=<16 hex digits>`.
fn render_marker(m: &Marker) -> String {
    format!("protocol={}\nfingerprint={}\n", m.protocol, m.fingerprint)
}

/// Reads [`render_marker`]'s text. Unknown keys are ignored; `None` when either key is missing
/// or the protocol is not a number.
fn parse_marker(text: &str) -> Option<Marker> {
    let mut protocol = None;
    let mut fingerprint = None;
    for line in text.lines() {
        match line.split_once('=') {
            Some(("protocol", v)) => protocol = v.trim().parse().ok(),
            Some(("fingerprint", v)) => fingerprint = Some(v.trim().to_string()),
            _ => {}
        }
    }
    Some(Marker {
        protocol: protocol?,
        fingerprint: fingerprint.filter(|f| !f.is_empty())?,
    })
}

/// The marker at `path`; `None` when the file is missing, unreadable or not in the format above.
pub fn read_marker(path: &Path) -> Option<Marker> {
    parse_marker(&std::fs::read_to_string(path).ok()?)
}

/// Writes the marker through `<path>.tmp` and a rename, creating the directory. The rename keeps
/// a concurrent `spawn` from reading half a file.
pub fn write_marker(path: &Path, marker: &Marker) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, render_marker(marker))?;
    std::fs::rename(&tmp, path)
}

/// FNV-1a (64 bit) over the entries sorted by name, each as name, a NUL, the length as 8 little
/// endian bytes and the bytes, as 16 lower-case hex digits. Used only to tell two deployments
/// apart, not for security; it is spelled out here (not `DefaultHasher`) so the value stays the
/// same between builds of spawn-at.
pub fn fingerprint(files: &[DeployedFile]) -> String {
    let mut sorted: Vec<&DeployedFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for f in sorted {
        feed(f.name.as_bytes());
        feed(&[0]);
        feed(&(f.bytes.len() as u64).to_le_bytes());
        feed(&f.bytes);
    }
    format!("{h:016x}")
}

/// The protocol the deployed `extension.js` reports: the first number after `return` that
/// follows `get ProtocolVersion()`. `None` when there is no such file or getter.
pub fn deployed_protocol(files: &[DeployedFile]) -> Option<u32> {
    let js = files.iter().find(|f| f.name == "extension.js")?;
    let text = String::from_utf8_lossy(&js.bytes);
    let after_getter = &text[text.find("get ProtocolVersion()")?..];
    let after_return = &after_getter[after_getter.find("return")? + "return".len()..];
    let digits: String = after_return
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Whether an update changed the deployed extension, from the files read before and after the
/// new binary's `install --skip-bin --headless`. `Some` only when both reads found a directory
/// and the files differ in name or bytes. `None` for a first install (nothing was deployed
/// before: the Shell has not loaded anything yet), when nothing changed, and when either read
/// failed (a change that cannot be shown is not claimed).
pub fn classify(
    before: Result<Option<Vec<DeployedFile>>, String>,
    after: Result<Option<Vec<DeployedFile>>, String>,
) -> Option<ChangedExtension> {
    let (Ok(Some(before)), Ok(Some(after))) = (before, after) else {
        return None;
    };
    let new_fingerprint = fingerprint(&after);
    if fingerprint(&before) == new_fingerprint {
        return None;
    }
    Some(ChangedExtension {
        protocol: deployed_protocol(&after),
        fingerprint: new_fingerprint,
    })
}

/// How an [`announce`] call ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Announced {
    /// X11 (the Shell reloads in place): no marker, no notification.
    NotWayland,
    /// The marker already records this deployed version: nothing sent again.
    AlreadyFlagged,
    /// The notification was sent.
    Sent,
    /// The notification could not be sent (the text says why).
    SendFailed(String),
}

/// The result of [`announce`]: what happened to the notification and, separately, whether the
/// marker could be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub announced: Announced,
    /// Why the marker was not written; `None` when it was written or nothing was to be written.
    pub marker_error: Option<String>,
}

/// Flags a changed extension: on Wayland, unless the marker at `marker` already records this
/// fingerprint, writes the marker (only when the protocol is known) and calls `notify` once with
/// [`NOTICE_TEXT`]. A marker that cannot be written does not stop the notification.
pub fn announce(
    changed: &ChangedExtension,
    is_wayland: bool,
    marker: Option<&Path>,
    notify: &mut dyn FnMut(&str) -> Result<(), String>,
) -> Announcement {
    let done = |announced| Announcement {
        announced,
        marker_error: None,
    };
    if !is_wayland {
        return done(Announced::NotWayland);
    }
    if marker
        .and_then(read_marker)
        .is_some_and(|m| m.fingerprint == changed.fingerprint)
    {
        return done(Announced::AlreadyFlagged);
    }
    let marker_error = match (marker, changed.protocol) {
        (Some(path), Some(protocol)) => write_marker(
            path,
            &Marker {
                protocol,
                fingerprint: changed.fingerprint.clone(),
            },
        )
        .err()
        .map(|e| format!("cannot write {}: {e}", path.display())),
        (None, _) => Some("neither $XDG_STATE_HOME nor $HOME is set".to_string()),
        (Some(_), None) => Some("the deployed extension's protocol could not be read".to_string()),
    };
    let announced = match notify(NOTICE_TEXT) {
        Ok(()) => Announced::Sent,
        Err(e) => Announced::SendFailed(e),
    };
    Announcement {
        announced,
        marker_error,
    }
}

/// What `spawn` does with the marker, given the protocol the loaded extension reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintAction {
    /// No marker, or the loaded protocol could not be read (the missing-extension diagnosis
    /// covers that case).
    Nothing,
    /// The loaded protocol is lower than the deployed one: print the hint.
    Hint,
    /// The loaded protocol has caught up: remove the marker.
    Clear,
}

/// The rule: a marker with a lower loaded protocol hints, a loaded protocol equal or higher
/// clears, an unreadable loaded protocol does nothing.
pub fn hint_action(marker: Option<&Marker>, loaded: Option<u32>) -> HintAction {
    match (marker, loaded) {
        (Some(m), Some(l)) if l < m.protocol => HintAction::Hint,
        (Some(_), Some(_)) => HintAction::Clear,
        _ => HintAction::Nothing,
    }
}

/// The one hint line for a deployed protocol and a loaded one.
pub fn hint_text(deployed: u32, loaded: u32) -> String {
    format!(
        "The updated GNOME extension (protocol {deployed}) is not loaded yet; the Shell still \
         runs protocol {loaded}. Log out and back in to finish."
    )
}

/// Applies [`hint_action`] to the marker file at `path`: removes it on `Clear` (a failure to
/// remove is ignored; the next `spawn` tries again) and returns the hint line on `Hint`.
pub fn check_marker(path: &Path, loaded: Option<u32>) -> Option<String> {
    let marker = read_marker(path);
    match hint_action(marker.as_ref(), loaded) {
        HintAction::Hint => Some(hint_text(marker?.protocol, loaded?)),
        HintAction::Clear => {
            let _ = std::fs::remove_file(path);
            None
        }
        HintAction::Nothing => None,
    }
}

/// Sends one `org.freedesktop.Notifications.Notify` call on the session bus with `summary` as
/// the whole message (no body, no actions, default timeout) and waits up to [`NOTIFY_TIMEOUT`].
///
/// zbus here runs on tokio, and the caller may or may not be inside a runtime, so the call runs
/// on its own thread with its own current-thread runtime.
pub fn send_notification(summary: &str) -> Result<(), String> {
    let summary = summary.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        rt.block_on(async {
            let call = async {
                let conn = zbus::Connection::session()
                    .await
                    .map_err(|e| format!("no session bus: {e}"))?;
                let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
                conn.call_method(
                    Some("org.freedesktop.Notifications"),
                    "/org/freedesktop/Notifications",
                    Some("org.freedesktop.Notifications"),
                    "Notify",
                    // app_name, replaces_id, icon, summary, body, actions, hints, timeout (-1 = server default)
                    &(
                        "spawn-at",
                        0u32,
                        "",
                        summary.as_str(),
                        "",
                        Vec::<&str>::new(),
                        hints,
                        -1i32,
                    ),
                )
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
            };
            tokio::time::timeout(NOTIFY_TIMEOUT, call)
                .await
                .map_err(|_| "the notification service did not answer in 5 s".to_string())?
        })
    })
    .join()
    .map_err(|_| "the notification thread panicked".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux::gnome::deploy::{
        EMBEDDED_EXTENSION_FILES_LEGACY, EMBEDDED_EXTENSION_FILES_MODERN,
    };
    use std::time::UNIX_EPOCH;

    fn file(name: &str, text: &str) -> DeployedFile {
        DeployedFile {
            name: name.to_string(),
            bytes: text.as_bytes().to_vec(),
            modified: UNIX_EPOCH,
        }
    }

    fn files_of(set: &[(&str, &str)]) -> Vec<DeployedFile> {
        set.iter().map(|(n, c)| file(n, c)).collect()
    }

    fn changed(protocol: Option<u32>, fingerprint: &str) -> ChangedExtension {
        ChangedExtension {
            protocol,
            fingerprint: fingerprint.to_string(),
        }
    }

    /// A unique, absolute, not-yet-created marker path under the system temp dir; removed by
    /// the test's guard. Nothing is created under `$HOME`.
    struct TempMarker(PathBuf);

    impl TempMarker {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "spawnat-test-needs-logout-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            TempMarker(dir.join("spawn-at").join("needs-logout"))
        }
    }

    impl Drop for TempMarker {
        fn drop(&mut self) {
            // Two levels up: <dir>/spawn-at/needs-logout.
            let root = self
                .0
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf);
            if let Some(root) = root {
                let _ = std::fs::remove_dir_all(root);
            }
        }
    }

    /// `$XDG_STATE_HOME` wins when absolute; an empty or relative one is ignored (the XDG rule)
    /// and `~/.local/state` is used; with no `$HOME` either there is no path.
    #[test]
    fn test_marker_path_follows_the_xdg_state_rule() {
        let p = |x: Option<&str>, h: Option<&str>| {
            marker_path_from(x.map(Into::into), h.map(Into::into))
        };
        assert_eq!(
            p(Some("/s"), Some("/h")),
            Some(PathBuf::from("/s/spawn-at/needs-logout"))
        );
        assert_eq!(
            p(None, Some("/h")),
            Some(PathBuf::from("/h/.local/state/spawn-at/needs-logout"))
        );
        assert_eq!(
            p(Some(""), Some("/h")),
            Some(PathBuf::from("/h/.local/state/spawn-at/needs-logout"))
        );
        assert_eq!(
            p(Some("relative/dir"), Some("/h")),
            Some(PathBuf::from("/h/.local/state/spawn-at/needs-logout"))
        );
        assert_eq!(p(None, None), None);
        assert_eq!(p(None, Some("")), None);
    }

    /// The text written is exactly two lines and reads back to the same value; a file missing a
    /// key, or with a non-numeric protocol, reads as no marker.
    #[test]
    fn test_marker_text_round_trips() {
        let m = Marker {
            protocol: 7,
            fingerprint: "00ff00ff00ff00ff".into(),
        };
        assert_eq!(
            render_marker(&m),
            "protocol=7\nfingerprint=00ff00ff00ff00ff\n"
        );
        assert_eq!(parse_marker(&render_marker(&m)), Some(m));
        assert_eq!(parse_marker("protocol=7\n"), None);
        assert_eq!(parse_marker("fingerprint=ab\n"), None);
        assert_eq!(parse_marker("protocol=seven\nfingerprint=ab\n"), None);
        assert_eq!(parse_marker(""), None);
    }

    /// Writing creates the missing directories, leaves no `.tmp` file behind, and replaces an
    /// older marker.
    #[test]
    fn test_write_marker_creates_dirs_and_replaces() {
        let t = TempMarker::new("write");
        let first = Marker {
            protocol: 6,
            fingerprint: "aaaaaaaaaaaaaaaa".into(),
        };
        let second = Marker {
            protocol: 7,
            fingerprint: "bbbbbbbbbbbbbbbb".into(),
        };
        write_marker(&t.0, &first).unwrap();
        assert_eq!(read_marker(&t.0), Some(first));
        write_marker(&t.0, &second).unwrap();
        assert_eq!(read_marker(&t.0), Some(second));
        assert!(!t.0.with_extension("tmp").exists());
    }

    /// The fingerprint ignores listing order and modification time, and changes with one byte,
    /// one name, or an extra file. 16 hex digits.
    #[test]
    fn test_fingerprint_tracks_names_and_bytes_only() {
        let a = vec![file("a.js", "one"), file("b.js", "two")];
        let mut reordered = vec![file("b.js", "two"), file("a.js", "one")];
        reordered[0].modified = later();
        assert_eq!(fingerprint(&a), fingerprint(&reordered));
        assert_eq!(fingerprint(&a).len(), 16);
        assert_ne!(
            fingerprint(&a),
            fingerprint(&[file("a.js", "one"), file("b.js", "twp")])
        );
        assert_ne!(
            fingerprint(&a),
            fingerprint(&[file("a.js", "one"), file("c.js", "two")])
        );
        assert_ne!(
            fingerprint(&a),
            fingerprint(&[file("a.js", "one"), file("b.js", "two"), file("c.js", "")])
        );
        // "ab" + "c" must not equal "a" + "bc": the name, a NUL and the length are hashed.
        assert_ne!(
            fingerprint(&[file("ab", "c")]),
            fingerprint(&[file("a", "bc")])
        );
        // Pinned value: the marker outlives the binary that wrote it, so this must not drift.
        assert_eq!(fingerprint(&[file("a", "b")]), "426361459c52a885");
    }

    /// A modification time different from the `UNIX_EPOCH` that `file` uses.
    fn later() -> std::time::SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_000_000)
    }

    /// The getter is read from the embedded extensions, so a reformatted `extension.js` that
    /// breaks the parser fails here: modern reports 7, legacy 5 (the numbers in
    /// `assets/gnome/*/extension.js`, `get ProtocolVersion()`).
    #[test]
    fn test_deployed_protocol_reads_the_embedded_extensions() {
        assert_eq!(
            deployed_protocol(&files_of(EMBEDDED_EXTENSION_FILES_MODERN)),
            Some(7)
        );
        assert_eq!(
            deployed_protocol(&files_of(EMBEDDED_EXTENSION_FILES_LEGACY)),
            Some(5)
        );
        assert_eq!(deployed_protocol(&[file("other.js", "return 9")]), None);
        assert_eq!(
            deployed_protocol(&[file("extension.js", "return 9;")]),
            None
        );
        assert_eq!(
            deployed_protocol(&[file(
                "extension.js",
                "get ProtocolVersion() {\n  return 12;\n}"
            )]),
            Some(12)
        );
    }

    /// The changed / unchanged decision: only a directory that existed before and holds
    /// different bytes (or names) afterwards counts. First install, identical files, a missing
    /// directory afterwards and any unreadable side give `None`.
    #[test]
    fn test_classify_changed_unchanged_first_install_and_errors() {
        let old = files_of(&[("extension.js", "get ProtocolVersion() { return 6; }")]);
        let new = files_of(&[("extension.js", "get ProtocolVersion() { return 7; }")]);
        let c = classify(Ok(Some(old.clone())), Ok(Some(new.clone()))).unwrap();
        assert_eq!(c.protocol, Some(7));
        assert_eq!(c.fingerprint, fingerprint(&new));
        // Same bytes, different modification time: not a change.
        let mut touched = old.clone();
        touched[0].modified = later();
        assert_eq!(classify(Ok(Some(old.clone())), Ok(Some(touched))), None);
        // One more file with the same extension.js is a change.
        let mut extra = old.clone();
        extra.push(file("metadata.json", "{}"));
        assert!(classify(Ok(Some(old.clone())), Ok(Some(extra))).is_some());
        // First install: nothing deployed before.
        assert_eq!(classify(Ok(None), Ok(Some(new.clone()))), None);
        // The directory is gone afterwards, or a side could not be read.
        assert_eq!(classify(Ok(Some(old.clone())), Ok(None)), None);
        assert_eq!(classify(Err("denied".into()), Ok(Some(new.clone()))), None);
        assert_eq!(classify(Ok(Some(old)), Err("denied".into())), None);
    }

    /// A change on Wayland writes the marker (protocol and fingerprint) and calls the notifier
    /// exactly once with the notice text.
    #[test]
    fn test_announce_on_wayland_writes_the_marker_and_notifies_once() {
        let t = TempMarker::new("announce");
        let mut sent = Vec::new();
        let a = announce(&changed(Some(7), "f1"), true, Some(&t.0), &mut |s| {
            sent.push(s.to_string());
            Ok(())
        });
        assert_eq!(a.announced, Announced::Sent);
        assert_eq!(a.marker_error, None);
        assert_eq!(sent, vec![NOTICE_TEXT.to_string()]);
        assert_eq!(
            read_marker(&t.0),
            Some(Marker {
                protocol: 7,
                fingerprint: "f1".into()
            })
        );
    }

    /// The same deployed version again sends nothing and leaves the marker as it was; a
    /// different version sends again and replaces the marker.
    #[test]
    fn test_announce_does_not_repeat_for_the_same_version() {
        let t = TempMarker::new("repeat");
        let mut count = 0;
        let mut notify = |_: &str| {
            count += 1;
            Ok(())
        };
        announce(&changed(Some(7), "f1"), true, Some(&t.0), &mut notify);
        let again = announce(&changed(Some(7), "f1"), true, Some(&t.0), &mut notify);
        assert_eq!(again.announced, Announced::AlreadyFlagged);
        announce(&changed(Some(8), "f2"), true, Some(&t.0), &mut notify);
        assert_eq!(count, 2);
        assert_eq!(read_marker(&t.0).unwrap().fingerprint, "f2");
    }

    /// X11 gets no marker and no notification.
    #[test]
    fn test_announce_does_nothing_on_x11() {
        let t = TempMarker::new("x11");
        let a = announce(&changed(Some(7), "f1"), false, Some(&t.0), &mut |_| {
            panic!("no notification on X11")
        });
        assert_eq!(a.announced, Announced::NotWayland);
        assert!(!t.0.exists());
    }

    /// A failing notifier is reported and the marker is still written; an unwritable marker
    /// path is reported and the notification is still sent.
    #[test]
    fn test_announce_keeps_the_two_effects_independent() {
        let t = TempMarker::new("indep");
        let a = announce(&changed(Some(7), "f1"), true, Some(&t.0), &mut |_| {
            Err("no daemon".into())
        });
        assert_eq!(a.announced, Announced::SendFailed("no daemon".into()));
        assert!(t.0.exists());

        let mut sent = 0;
        let a = announce(&changed(Some(7), "f1"), true, None, &mut |_| {
            sent += 1;
            Ok(())
        });
        assert_eq!(a.announced, Announced::Sent);
        assert!(a.marker_error.unwrap().contains("$HOME"));
        assert_eq!(sent, 1);

        let a = announce(&changed(None, "f1"), true, Some(&t.0), &mut |_| Ok(()));
        assert_eq!(
            a.announced,
            Announced::AlreadyFlagged,
            "t.0 already holds f1"
        );
        let t2 = TempMarker::new("noproto");
        let a = announce(&changed(None, "f9"), true, Some(&t2.0), &mut |_| Ok(()));
        assert_eq!(a.announced, Announced::Sent);
        assert!(a.marker_error.unwrap().contains("protocol"));
        assert!(!t2.0.exists());
    }

    /// The hint rule: lower loaded protocol hints, equal or higher clears, an unreadable
    /// loaded protocol or no marker does nothing.
    #[test]
    fn test_hint_action_rule() {
        let m = Marker {
            protocol: 7,
            fingerprint: "f".into(),
        };
        assert_eq!(hint_action(Some(&m), Some(6)), HintAction::Hint);
        assert_eq!(hint_action(Some(&m), Some(7)), HintAction::Clear);
        assert_eq!(hint_action(Some(&m), Some(8)), HintAction::Clear);
        assert_eq!(hint_action(Some(&m), None), HintAction::Nothing);
        assert_eq!(hint_action(None, Some(6)), HintAction::Nothing);
        assert_eq!(hint_action(None, None), HintAction::Nothing);
    }

    /// `check_marker` on a real file: it hints while the loaded protocol is lower and keeps the
    /// file, removes the file once the protocol has caught up, and stays silent without a file.
    #[test]
    fn test_check_marker_hints_then_clears() {
        let t = TempMarker::new("check");
        assert_eq!(check_marker(&t.0, Some(6)), None, "no marker, no hint");
        write_marker(
            &t.0,
            &Marker {
                protocol: 7,
                fingerprint: "f".into(),
            },
        )
        .unwrap();
        let hint = check_marker(&t.0, Some(6)).unwrap();
        assert!(
            hint.contains("protocol 7") && hint.contains("protocol 6"),
            "{hint}"
        );
        assert!(!hint.contains('\n'), "one line");
        assert!(t.0.exists(), "hinting keeps the marker");
        assert_eq!(check_marker(&t.0, None), None);
        assert!(t.0.exists(), "an unreadable protocol keeps the marker");
        assert_eq!(check_marker(&t.0, Some(7)), None);
        assert!(!t.0.exists(), "caught up: the marker is removed");
    }
}
