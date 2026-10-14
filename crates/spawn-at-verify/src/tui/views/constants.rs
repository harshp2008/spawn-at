//! Drawer layout sizing constants and column offsets.

/// Number of indentation columns preceding drawer section keys.
pub const DRAWER_INDENT: usize = 2;
/// Minimum width allocated for key labels in drawer tabular layouts.
pub const DRAWER_LABEL_W: usize = 14;
/// Maximum width the shared label column may grow to for long flag labels.
pub const DRAWER_LABEL_W_MAX: usize = 24;
/// Default horizontal column offset where drawer values begin (indent + minimum label width).
pub const DRAWER_VALUE_X: usize = DRAWER_INDENT + DRAWER_LABEL_W; // 16
/// Padding columns reserved on the right margin of the drawer.
pub const DRAWER_RIGHT_PAD: usize = 2;
