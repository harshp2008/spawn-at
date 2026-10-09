//! # Process Tree Introspection Helpers

/// Reads `/proc/{pid}/stat` on Linux to extract the Parent Process ID (PPID).
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

/// Checks whether `pid` is a descendant of `target_parent` by walking the process tree.
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
