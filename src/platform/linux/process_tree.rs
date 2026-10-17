//! # Process Tree Introspection Helpers
//!
//! One copy for everything that asks "is this window process a descendant of the process I
//! spawned" (the neutral commands and the X11 driver). Reads `/proc/{pid}/stat`.
//!
//! Only reads `/proc`; it signals nothing and keeps no state. A process that exits between two
//! reads makes the walk stop with `false`, not an error.
// Callers (grep -rn is_process_descendant src/): src/commands/mod.rs, the Sway driver, and the
// X11 session.

/// The parent pid of `pid` from field 4 of `/proc/<pid>/stat`, or `None` when the file cannot be
/// read or the text after the first `)` has fewer than two fields.
///
/// It cuts the text at the first `)`, not the last, so a command name that itself contains `)`
/// (the name is in parentheses in that file) gives a wrong field or `None`.
pub fn get_parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let ppid_str = stat.split(')').nth(1)?;
    let parts: Vec<&str> = ppid_str.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u32>().ok()
    } else {
        None
    }
}

/// True when `pid` equals `target_parent` or has it as an ancestor, found by following parent
/// pids upward. Looks at `pid` and at most nine ancestors (the loop runs 10 times), so a
/// descendant deeper than that is reported as `false`. The walk also stops at pid 1 or below, when
/// a parent cannot be read, and when a process is its own parent.
pub fn is_process_descendant(mut pid: u32, target_parent: u32) -> bool {
    for _ in 0..10 {
        if pid == target_parent {
            return true;
        }
        if pid <= 1 {
            break;
        }
        match get_parent_pid(pid) {
            Some(ppid) if ppid != pid => pid = ppid,
            _ => break,
        }
    }
    false
}
