//! # Safe Process Execution Pipeline
//!
//! Provides timeout-bounded process execution and pipe handling that prevents hangs
//! caused by orphaned background processes retaining open inherited file descriptors.

use std::io::{self, Read};
use std::os::unix::io::AsRawFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Structured output from executing a process with a timeout.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Sets a file descriptor to non-blocking mode.
fn set_nonblocking(fd: i32) -> io::Result<()> {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL, 0);
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Drains all currently available bytes from a non-blocking reader into a buffer.
fn drain_nonblocking<R: Read>(reader: &mut R, buf: &mut Vec<u8>) {
    let mut temp = [0u8; 4096];
    loop {
        match reader.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&temp[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
}

/// Executes a `Command` with a strict timeout, never hanging on pipes held by background processes.
pub fn execute_with_timeout(mut cmd: Command, timeout: Duration) -> Result<ProcessOutput, String> {
    use std::os::unix::process::CommandExt;

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Put child in its own process group so we can kill all descendants on timeout
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    let child_pid = child.id() as i32;
    let mut stdout_pipe = child.stdout.take().unwrap();
    let mut stderr_pipe = child.stderr.take().unwrap();

    let _ = set_nonblocking(stdout_pipe.as_raw_fd());
    let _ = set_nonblocking(stderr_pipe.as_raw_fd());

    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();

    let start = Instant::now();
    let poll_interval = Duration::from_millis(20);

    let exit_code = loop {
        drain_nonblocking(&mut stdout_pipe, &mut stdout_bytes);
        drain_nonblocking(&mut stderr_pipe, &mut stderr_bytes);

        match child.try_wait() {
            Ok(Some(status)) => {
                // Target child process has terminated!
                // Drain any final buffered data in the pipe
                drain_nonblocking(&mut stdout_pipe, &mut stdout_bytes);
                drain_nonblocking(&mut stderr_pipe, &mut stderr_bytes);
                break status.code();
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    // Timed out: kill the child's process group
                    unsafe {
                        libc::kill(-child_pid, libc::SIGKILL);
                        libc::kill(child_pid, libc::SIGKILL);
                    }
                    let _ = child.wait();
                    return Err(format!(
                        "Command timed out after {}ms (killed process group {})",
                        timeout.as_millis(),
                        child_pid
                    ));
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(format!("Error polling child status: {}", e));
            }
        }
    };

    Ok(ProcessOutput {
        exit_code,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fake_spawn_at_leaving_background_process_does_not_hang() {
        // Creates a fake script that outputs valid JSON and leaves a background sleep 999 holding stdout
        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "echo '{\"window_id\": 1234, \"size_raised\": false}'; sleep 999 >&1 & exit 0",
        ]);

        let start = Instant::now();
        let result =
            execute_with_timeout(cmd, Duration::from_secs(5)).expect("execute_with_timeout failed");

        // Must complete almost immediately (< 500ms), NOT hanging on sleep 999
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "Command hung waiting for pipes: elapsed {:?}",
            start.elapsed()
        );
        assert_eq!(result.exit_code, Some(0));

        let stdout_str = String::from_utf8_lossy(&result.stdout);
        assert!(stdout_str.contains("1234"));
    }

    #[test]
    fn test_hung_process_exceeds_timeout_and_is_killed() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30"]);

        let start = Instant::now();
        let err = execute_with_timeout(cmd, Duration::from_millis(150))
            .expect_err("Expected command to time out");

        assert!(start.elapsed() < Duration::from_millis(600));
        assert!(err.contains("Command timed out"));
    }
}
