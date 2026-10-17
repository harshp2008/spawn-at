//! # Declarative Compositor Driver Abstraction
//!
//! The [`Batch`] a spawn asks for and the [`Driver`] trait a compositor backend implements to
//! receive it.
//!
//! `src/commands/spawn.rs` builds one `Batch` per spawn and calls [`Driver::arm`] before it
//! launches the child; the backends under `src/platform/` (GNOME, Sway, X11, null) implement
//! `Driver`. This module only describes the data and the error type. It does no I/O and must not
//! name a display server.

use crate::geometry::PlacementParams;
use crate::wait_override::WaitOverride;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// When the windows of a batch become visible.
///
/// `spawn` builds `Together`; the null backend uses `Independent`. No backend under `src/` reads
/// the field (grep of `.reveal`, not verified beyond that), so the variants below state the
/// intended meaning only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reveal {
    /// Intended: every window stays hidden until all are mapped, then all appear together.
    Together,
    /// Intended: each window appears as soon as it has been placed.
    Independent,
}

/// Who gets keyboard focus once the batch is placed.
///
/// The Sway and X11 drivers compare the field with `Exclusive`; no code under `src/` matches on
/// `Entry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusIntent {
    /// Leave current desktop focus untouched.
    Leave,
    /// Intended: focus the entry with this [`Entry::key`].
    Entry(String),
    /// Take focus for the spawned window. `spawn` builds every batch with this value.
    Exclusive,
}

/// Scheduling urgency of the batch.
///
/// Only `Normal` is constructed in this tree, and no code reads the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Urgency {
    /// Standard scheduling.
    Normal,
    /// Intended: run out of band. No `--bypass-fifo` flag exists in the repository (grep).
    Express,
}

/// A declarative placement entry describing a single window within a batch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// Identifies the entry. The GNOME and X11 drivers hand it to the child as the value of
    /// `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`; GNOME also uses it as the target id when
    /// the batch has more than one entry.
    pub key: String,
    /// Application identifier or binary hint (e.g. "alacritty", "org.gnome.TextEditor"). GNOME
    /// uses it as the target id of a single-entry batch, or `"*"` when it is empty.
    pub app_hint: String,
    /// Position, size and anchor to apply.
    pub placement: PlacementParams,
    /// Developer override of the resize wait for this entry only; the default changes nothing and
    /// is not serialised.
    #[serde(default, skip_serializing_if = "WaitOverride::is_default")]
    pub wait: WaitOverride,
}

/// The windows one spawn asks the compositor driver to place, passed to [`Driver::arm`].
///
/// `spawn` currently builds a batch of exactly one entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Batch {
    /// Batch identifier; `spawn` always sets 1 and no code under `src/` reads it.
    pub id: u64,
    /// The window entries, in the order given.
    pub entries: Vec<Entry>,
    /// When the windows become visible.
    pub reveal: Reveal,
    /// Who gets focus afterwards.
    pub focus: FocusIntent,
    /// Scheduling urgency.
    pub urgency: Urgency,
    /// Intended: how long to wait for the windows to map before revealing anyway. `spawn` sets
    /// 15000 ms; no driver under `src/` reads it (not verified beyond grep).
    pub deadline: Duration,
}

/// What [`Driver::arm`] hands back to the caller before it launches the child.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Armed {
    /// Environment variables `spawn` sets on the child process (the GNOME and X11 drivers return
    /// `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`).
    pub launch_env: Vec<(String, String)>,
    /// Identifies the registration. `spawn` passes it to [`Driver::disarm`] if the child cannot
    /// be started; the GNOME driver sets it to the target id of the last entry.
    pub token: Option<String>,
}

/// Error returned by drivers and by the backend methods built on them.
///
/// A `String` converts into `IpcError` through `From`.
#[derive(Debug)]
pub enum DriverError {
    /// Talking to the compositor or helper failed (D-Bus, socket, protocol violation).
    IpcError(String),
    /// The active backend lacks this operation. The payload names it (for example
    /// `"get_windows"`); `Display` special-cases `"close_window"` and any text starting with
    /// `"this backend"`.
    UnsupportedCapability(&'static str),
    /// No window matches the id or target; the payload is the message shown to the user.
    TargetNotFound(String),
    /// Any other failure; [`std::error::Error::source`] returns the wrapped error.
    Execution(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverError::IpcError(err) => write!(f, "IPC Error: {}", err),
            DriverError::UnsupportedCapability(feature) => {
                if *feature == "close_window" || feature.starts_with("this backend") {
                    write!(f, "this backend cannot close windows")
                } else {
                    write!(
                        f,
                        "The active compositor does not support this feature: {}",
                        feature
                    )
                }
            }
            DriverError::TargetNotFound(target) => write!(f, "Target window not found: {}", target),
            DriverError::Execution(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for DriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DriverError::Execution(err) => Some(&**err as &(dyn std::error::Error + 'static)),
            _ => None,
        }
    }
}

impl From<String> for DriverError {
    fn from(s: String) -> Self {
        DriverError::IpcError(s)
    }
}

/// What a compositor backend offers `spawn`: register the placement before the child exists.
///
/// `CompositorBackend` in `src/platform/mod.rs` has this trait as a supertrait.
#[async_trait::async_trait]
pub trait Driver: Send + Sync {
    /// Registers the batch with the compositor side so the window is placed when it appears, and
    /// returns the environment for the child. Call it before launching the child.
    ///
    /// Errors: [`DriverError::IpcError`] when the compositor or helper cannot be reached; the
    /// null backend returns [`DriverError::UnsupportedCapability`].
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError>;

    /// Withdraws a registration made by [`Driver::arm`], given [`Armed::token`]. The default does
    /// nothing and returns `Ok(false)`; the GNOME driver forwards the call to the extension and
    /// returns its answer.
    async fn disarm(&self, _token: &str) -> Result<bool, DriverError> {
        Ok(false)
    }
}
