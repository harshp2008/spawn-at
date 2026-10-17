//! # X11 Atoms and Diagnostic Logging
//!
//! The atoms the X11 driver interns once per connection (`X11Session::connect`) and the
//! `x11_debug!` macro the X11 files log with. The macro prints to stderr only when the
//! environment variable `SPAWN_AT_DEBUG` is `1` or `true`; it reads the variable on every call
//! and has no other effect.

/// `eprintln!` of its arguments when `SPAWN_AT_DEBUG` is `1` or `true`, otherwise nothing. The
/// arguments are not evaluated when the variable is off.
#[macro_export]
macro_rules! x11_debug {
    ($($arg:tt)*) => {
        if std::env::var("SPAWN_AT_DEBUG").map(|v| v == "1" || v == "true").unwrap_or(false) {
            eprintln!($($arg)*);
        }
    };
}

pub(crate) use x11_debug;

// Interned but not read as `atoms.<name>` anywhere under src/ (grep -rn "atoms\._NET_CURRENT_DESKTOP"
// src/): `_NET_CURRENT_DESKTOP`, `WM_NORMAL_HINTS` (the code uses the built-in
// `AtomEnum::WM_NORMAL_HINTS` instead) and `WM_STATE`.
x11rb::atom_manager! {
    /// EWMH and ICCCM atoms used for window management, state control, and placement hints.
    pub Atoms: AtomsCookie {
        _NET_CLIENT_LIST,
        _NET_ACTIVE_WINDOW,
        _NET_WORKAREA,
        _NET_CURRENT_DESKTOP,
        _NET_WM_PID,
        _NET_WM_NAME,
        _NET_MOVERESIZE_WINDOW,
        _NET_CLOSE_WINDOW,
        _NET_WM_STATE,
        _NET_WM_STATE_MAXIMIZED_VERT,
        _NET_WM_STATE_MAXIMIZED_HORZ,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_USER_TIME,
        _NET_STARTUP_ID,
        UTF8_STRING,
        WM_NORMAL_HINTS,
        WM_SIZE_HINTS,
        WM_CHANGE_STATE,
        WM_STATE,
    }
}
