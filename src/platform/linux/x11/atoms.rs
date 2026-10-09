//! # X11 Atoms and Diagnostic Logging

#[macro_export]
macro_rules! x11_debug {
    ($($arg:tt)*) => {
        if std::env::var("SPAWN_AT_DEBUG").map(|v| v == "1" || v == "true").unwrap_or(false) {
            eprintln!($($arg)*);
        }
    };
}

pub(crate) use x11_debug;

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
