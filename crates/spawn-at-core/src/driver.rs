//! # Declarative Compositor Driver Abstraction
//!
//! The [`Batch`] a spawn asks for and the [`Driver`] trait a compositor backend implements to
//! receive it.
//!
//! A caller builds a `Batch` and calls [`Driver::arm`] before it launches the child; each backend
//! implements `Driver`. This module only describes the data and the error type. It does no I/O
//! and must not name a display server.

// Callers at this commit: src/commands/spawn.rs builds the Batch and calls arm; the backends under
// src/platform/ (GNOME, Sway, X11, null) implement Driver.

use crate::geometry::PlacementParams;
use crate::wait_override::WaitOverride;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// When the windows of a batch become visible.
///
/// The variants state the intended meaning.
// At this commit src/commands/spawn.rs builds `Together` and src/platform/null_backend.rs builds
// `Independent`. A grep of `.reveal` under src/ finds no reader; not verified beyond that grep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reveal {
    /// Intended: every window stays hidden until all are mapped, then all appear together.
    Together,
    /// Intended: each window appears as soon as it has been placed.
    Independent,
}

/// Who gets keyboard focus once the batch is placed.
///
// At this commit src/platform/linux/sway/mod.rs and src/platform/linux/x11/interception.rs compare
// the field with `Exclusive`; a grep finds no code under src/ that matches on `Entry`.
// src/commands/spawn.rs builds `Exclusive`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusIntent {
    /// Leave current desktop focus untouched.
    Leave,
    /// Intended: focus the entry with this [`Entry::key`].
    Entry(String),
    /// Take focus for the spawned window.
    Exclusive,
}

/// Scheduling urgency of the batch.
///
// At this commit only `Normal` is constructed under src/ and a grep finds no reader of the field.
// No `--bypass-fifo` flag exists in the repository (grep).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Urgency {
    /// Standard scheduling.
    Normal,
    /// Intended: run out of band, without waiting behind other batches.
    Express,
}

/// A declarative placement entry describing a single window within a batch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// Identifies the entry within its batch.
    // At this commit the GNOME and X11 drivers return it as the value of XDG_ACTIVATION_TOKEN and
    // DESKTOP_STARTUP_ID, and GNOME uses it as the target id when the batch has several entries.
    pub key: String,
    /// Application identifier or binary hint (e.g. "alacritty", "org.gnome.TextEditor").
    // At this commit GNOME uses it as the target id of a single-entry batch, or "*" when empty.
    pub app_hint: String,
    /// Position, size and anchor to apply.
    pub placement: PlacementParams,
    /// Developer override of the resize wait for this entry only; the default changes nothing and
    /// is not serialised.
    #[serde(default, skip_serializing_if = "WaitOverride::is_default")]
    pub wait: WaitOverride,
}

/// The windows one spawn asks the compositor driver to place, passed to [`Driver::arm`].
// At this commit src/commands/spawn.rs builds a batch of exactly one entry with id 1 and a grep
// finds no reader of `id` under src/.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Batch {
    /// Batch identifier.
    pub id: u64,
    /// The window entries, in the order given.
    pub entries: Vec<Entry>,
    /// When the windows become visible.
    pub reveal: Reveal,
    /// Who gets focus afterwards.
    pub focus: FocusIntent,
    /// Scheduling urgency.
    pub urgency: Urgency,
    /// Intended: how long to wait for the windows to map before revealing anyway.
    // At this commit spawn.rs sets 15000 ms and a grep finds no driver under src/ reading it;
    // not verified beyond the grep.
    pub deadline: Duration,
}

/// What [`Driver::arm`] hands back to the caller before it launches the child.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Armed {
    /// Environment variables the caller sets on the child process.
    // At this commit the GNOME and X11 drivers return XDG_ACTIVATION_TOKEN and
    // DESKTOP_STARTUP_ID, and spawn.rs applies them with `cmd.env`.
    pub launch_env: Vec<(String, String)>,
    /// Identifies the registration; the value to pass to [`Driver::disarm`].
    // At this commit the GNOME driver sets it to the target id of the last entry and spawn.rs
    // passes it to disarm when the child cannot be started.
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

/// What a compositor backend offers a caller: register the placement before the child exists.
// At this commit `CompositorBackend` in src/platform/mod.rs has this trait as a supertrait.
#[async_trait::async_trait]
pub trait Driver: Send + Sync {
    /// Registers the batch with the compositor side so the window is placed when it appears, and
    /// returns the environment for the child. Call it before launching the child.
    ///
    /// Errors: [`DriverError`]; which variant depends on the implementation.
    // At this commit the GNOME driver returns IpcError when the D-Bus call fails and the null
    // backend returns UnsupportedCapability.
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError>;

    /// Withdraws a registration made by [`Driver::arm`], given [`Armed::token`]. The default does
    /// nothing and returns `Ok(false)`.
    // At this commit the GNOME driver overrides it and returns the answer of the extension's
    // DisarmSpawn call.
    async fn disarm(&self, _token: &str) -> Result<bool, DriverError> {
        Ok(false)
    }
}
