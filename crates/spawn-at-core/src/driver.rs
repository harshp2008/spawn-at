//! # Declarative Compositor Driver Abstraction
//!
//! Defines the high-level declarative `Batch` intent and the asynchronous `Driver` trait.

use crate::geometry::PlacementParams;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Defines the visual presentation strategy for batched window reveals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reveal {
    /// All windows in the batch are held cloaked until all are mapped, then uncloaked together.
    Together,
    /// Each window is uncloaked independently as soon as it completes its commit phase.
    Independent,
}

/// Defines the keyboard focus allocation policy upon batch settlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusIntent {
    /// Leave current desktop focus untouched.
    Leave,
    /// Direct focus to the entry identified by its key.
    Entry(String),
    /// Forcibly claim exclusive focus for the leader of the batch.
    Exclusive,
}

/// Scheduling urgency of the batch transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Urgency {
    /// Standard scheduling through FIFO / priority queues.
    Normal,
    /// Urgent out-of-band execution (--bypass-fifo) skipping barriers.
    Express,
}

/// A declarative placement entry describing a single window within a batch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// Unique identifier or token for deterministic identity matching.
    pub key: String,
    /// Application identifier or binary hint (e.g. "alacritty", "org.gnome.TextEditor").
    pub app_hint: String,
    /// Declarative spatial geometry and alignment constraints.
    pub placement: PlacementParams,
}

/// A declarative batch of windows to be orchestrated and placed by the compositor driver.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Batch {
    /// Unique batch identifier.
    pub id: u64,
    /// Ordered list of window entries in this batch.
    pub entries: Vec<Entry>,
    /// Visual reveal policy (simultaneous vs independent).
    pub reveal: Reveal,
    /// Focus arbitration policy.
    pub focus: FocusIntent,
    /// Scheduling urgency level.
    pub urgency: Urgency,
    /// Maximum allowable time for windows to map before degraded reveal.
    pub deadline: Duration,
}

/// Environment tokens and startup context returned after arming the compositor.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Armed {
    /// Key-value environment variables to inject into spawned child processes
    /// (e.g. `XDG_ACTIVATION_TOKEN`, `DESKTOP_STARTUP_ID`).
    pub launch_env: Vec<(String, String)>,
    /// Optional identifier or token corresponding to this armed registration.
    pub token: Option<String>,
}

/// Error type produced by driver arming and execution.
#[derive(Debug)]
pub enum DriverError {
    /// IPC failure (D-Bus, socket, protocol violation).
    IpcError(String),
    /// Target window or capability not supported by the active driver.
    UnsupportedCapability(&'static str),
    /// Window target not found during lookup.
    TargetNotFound(String),
    /// Execution error.
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

/// Declarative compositor driver interface.
#[async_trait::async_trait]
pub trait Driver: Send + Sync {
    /// Arms the compositor with the declarative batch intent before processes are launched.
    async fn arm(&self, batch: Batch) -> Result<Armed, DriverError>;

    /// Disarms a previously armed token (best-effort, idempotent).
    async fn disarm(&self, _token: &str) -> Result<bool, DriverError> {
        Ok(false)
    }
}
