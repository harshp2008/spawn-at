//! A blocking connection to the Sway IPC socket. The framing is in `ipc`; this adds the socket,
//! the timeouts and the error messages that name `$SWAYSOCK`.

use super::ipc::{self, Frame, FrameError, MessageType};
use crate::platform::DriverError;
use std::io::{Chain, Cursor, Read};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long a request waits for its reply. Sway answers in milliseconds; this only bounds a
/// socket that accepts and then says nothing.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Where the rest of a frame may take once its first byte has arrived.
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

/// The socket path from the value of `$SWAYSOCK` (`None` when the variable is unset).
///
/// The messages are shown when the user forced Sway with `SPAWN_AT_BACKEND=sway`, so each says
/// what to do. There is no fallback to another backend in any of these cases.
pub fn socket_path(value: Option<std::ffi::OsString>) -> Result<PathBuf, DriverError> {
    match value {
        None => Err(DriverError::IpcError(
            "SPAWN_AT_BACKEND=sway, but $SWAYSOCK is not set. Run spawn-at inside a Sway session, or unset SPAWN_AT_BACKEND."
                .to_string(),
        )),
        Some(v) if v.is_empty() => Err(DriverError::IpcError(
            "SPAWN_AT_BACKEND=sway, but $SWAYSOCK is empty. Run spawn-at inside a Sway session, or unset SPAWN_AT_BACKEND."
                .to_string(),
        )),
        Some(v) => Ok(PathBuf::from(v)),
    }
}

fn frame_error(path: &Path, what: &str, e: FrameError) -> DriverError {
    DriverError::IpcError(format!("$SWAYSOCK ({}): {}: {}", path.display(), what, e))
}

/// One connection. Requests are answered in order; a connection that subscribed to events also
/// receives event frames, so use a separate connection for requests and for events.
pub struct Client {
    stream: UnixStream,
    path: PathBuf,
}

impl Client {
    /// Connects to the socket at `path`.
    pub fn connect(path: &Path) -> Result<Client, DriverError> {
        let stream = UnixStream::connect(path).map_err(|e| {
            DriverError::IpcError(format!(
                "SPAWN_AT_BACKEND=sway, but cannot connect to $SWAYSOCK ({}): {}. Is Sway running, and is this its socket?",
                path.display(),
                e
            ))
        })?;
        let io = |e: std::io::Error| frame_error(path, "cannot set a timeout", FrameError::Io(e));
        stream.set_read_timeout(Some(REPLY_TIMEOUT)).map_err(io)?;
        stream.set_write_timeout(Some(REPLY_TIMEOUT)).map_err(io)?;
        Ok(Client {
            stream,
            path: path.to_path_buf(),
        })
    }

    /// Sends one request and returns the payload of its reply. A reply of another type than the
    /// request is a protocol error (it means the stream is out of step).
    pub fn request(&mut self, kind: MessageType, payload: &[u8]) -> Result<Vec<u8>, DriverError> {
        ipc::write_frame(&mut self.stream, kind, payload)
            .map_err(|e| frame_error(&self.path, "cannot send the request", e))?;
        let frame = ipc::read_frame(&mut self.stream)
            .map_err(|e| frame_error(&self.path, "cannot read the reply", e))?;
        if frame.kind != kind as u32 {
            return Err(DriverError::IpcError(format!(
                "$SWAYSOCK ({}): asked for message type {} and got type {:#x}",
                self.path.display(),
                kind as u32,
                frame.kind
            )));
        }
        Ok(frame.payload)
    }

    /// Waits up to `wait` for the next frame (an event, on a subscribed connection). `None` when
    /// nothing arrived in time. Once the first byte of a frame has arrived the rest is read with
    /// the longer frame timeout, so a timeout never leaves half a frame behind.
    pub fn next_frame(&mut self, wait: Duration) -> Result<Option<Frame>, DriverError> {
        let wait = wait.max(Duration::from_millis(1));
        let io =
            |e: std::io::Error| frame_error(&self.path, "cannot set a timeout", FrameError::Io(e));
        self.stream.set_read_timeout(Some(wait)).map_err(io)?;
        let mut first = [0u8; 1];
        let got = match self.stream.read(&mut first) {
            Ok(0) => {
                return Err(DriverError::IpcError(format!(
                    "$SWAYSOCK ({}): Sway closed the connection",
                    self.path.display()
                )))
            }
            Ok(_) => true,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                false
            }
            Err(e) => {
                return Err(frame_error(
                    &self.path,
                    "cannot read an event",
                    FrameError::Io(e),
                ))
            }
        };
        if !got {
            return Ok(None);
        }
        self.stream
            .set_read_timeout(Some(FRAME_TIMEOUT))
            .map_err(io)?;
        let mut rest: Chain<Cursor<[u8; 1]>, &mut UnixStream> =
            Cursor::new(first).chain(&mut self.stream);
        let frame = ipc::read_frame(&mut rest)
            .map_err(|e| frame_error(&self.path, "cannot read an event", e))?;
        Ok(Some(frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    /// The three ways `$SWAYSOCK` can be unusable before any connection is tried each give an
    /// error that names `$SWAYSOCK` and `SPAWN_AT_BACKEND=sway`; a set value is the path.
    #[test]
    fn test_socket_path_messages() {
        let unset = socket_path(None).unwrap_err().to_string();
        assert!(
            unset.contains("$SWAYSOCK is not set") && unset.contains("SPAWN_AT_BACKEND=sway"),
            "{unset}"
        );
        let empty = socket_path(Some(OsString::new())).unwrap_err().to_string();
        assert!(empty.contains("$SWAYSOCK is empty"), "{empty}");
        assert_eq!(
            socket_path(Some(OsString::from("/run/user/1000/sway-ipc.sock"))).unwrap(),
            PathBuf::from("/run/user/1000/sway-ipc.sock")
        );
    }

    /// A path with nothing listening is an error naming `$SWAYSOCK` and the path, not a panic.
    #[test]
    fn test_connect_to_a_missing_socket_names_the_variable_and_the_path() {
        let path =
            std::env::temp_dir().join(format!("spawn-at-sway-none-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let err = Client::connect(&path).err().expect("must fail").to_string();
        assert!(
            err.contains("$SWAYSOCK") && err.contains(&*path.to_string_lossy()),
            "{err}"
        );
    }
}
