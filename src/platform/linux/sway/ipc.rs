//! The Sway (i3) IPC wire format: framing only, no sockets.
//!
//! A frame is the 6 bytes `i3-ipc`, a u32 payload length and a u32 message type (both in the
//! machine's native byte order, as `man sway-ipc` says), then the payload. Requests and replies
//! use the same frame. Event frames have the high bit of the type set (`0x80000003` is a
//! `window` event). The bytes in the tests were read from a private headless Sway 1.9
//! (`scripts/sway-headless.sh`) with `socat`.
//!
//! Used by `client.rs` (sockets) and the fake server in `fake.rs`. It does no I/O of its own
//! beyond the `Read` and `Write` it is handed, and it does not know what any message means.
//! The Sway backend is experimental and was tested only on that private Sway 1.9; frames from
//! other Sway or i3 versions are not verified.

use std::io::{Read, Write};

/// The first 6 bytes of every frame.
pub const MAGIC: &[u8; 6] = b"i3-ipc";
/// Magic + length + type.
pub const HEADER_LEN: usize = 14;
/// Largest payload accepted from the socket: 64 MiB (67,108,864 bytes). A larger announced
/// length is treated as a corrupt stream instead of allocating for it. The size of a real
/// `GET_TREE` reply on a busy desktop is not verified here.
pub const MAX_PAYLOAD: u32 = 64 * 1024 * 1024;

/// The request types spawn-at sends, with their numbers on the wire. `GET_MARKS` and
/// `GET_BAR_CONFIG` exist in Sway but are not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    RunCommand = 0,
    GetWorkspaces = 1,
    Subscribe = 2,
    GetOutputs = 3,
    GetTree = 4,
}

/// Type of a `window` event frame.
pub const EVENT_WINDOW: u32 = 0x8000_0003;

/// Why a frame could not be read or written.
#[derive(Debug)]
pub enum FrameError {
    /// The first 6 bytes were not `i3-ipc`: not a Sway/i3 socket, or the stream is out of step.
    BadMagic([u8; 6]),
    /// The header announced a payload larger than [`MAX_PAYLOAD`].
    TooLarge(u32),
    /// The socket failed or closed in the middle of a frame.
    Io(std::io::Error),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::BadMagic(got) => write!(
                f,
                "the reply does not start with \"i3-ipc\" (got {:02x?}); is $SWAYSOCK a Sway socket?",
                got
            ),
            FrameError::TooLarge(len) => write!(
                f,
                "the reply announces {} bytes, more than the {} byte limit",
                len, MAX_PAYLOAD
            ),
            FrameError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl From<std::io::Error> for FrameError {
    fn from(e: std::io::Error) -> Self {
        FrameError::Io(e)
    }
}

/// One decoded frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The raw type: a [`MessageType`] for a reply, `0x80000000 | n` for an event.
    pub kind: u32,
    pub payload: Vec<u8>,
}

/// The bytes of a request frame: magic, payload length, type, payload. The length is cast to
/// `u32` with `as`, so a payload over 4 GiB would wrap (no caller sends one).
pub fn encode_frame(kind: MessageType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
    out.extend_from_slice(&(kind as u32).to_ne_bytes());
    out.extend_from_slice(payload);
    out
}

/// Splits a 14-byte header into (payload length, type), both in bytes / raw numbers.
///
/// Errors: [`FrameError::BadMagic`] when the first 6 bytes are not `i3-ipc`;
/// [`FrameError::TooLarge`] when the length is over [`MAX_PAYLOAD`]. The type is not checked.
pub fn decode_header(header: &[u8; HEADER_LEN]) -> Result<(u32, u32), FrameError> {
    let mut magic = [0u8; 6];
    magic.copy_from_slice(&header[..6]);
    if &magic != MAGIC {
        return Err(FrameError::BadMagic(magic));
    }
    let len = u32::from_ne_bytes([header[6], header[7], header[8], header[9]]);
    let kind = u32::from_ne_bytes([header[10], header[11], header[12], header[13]]);
    if len > MAX_PAYLOAD {
        return Err(FrameError::TooLarge(len));
    }
    Ok((len, kind))
}

/// Reads one frame, blocking until it is complete. How long that can take is up to the reader's
/// own timeout (`client.rs` sets one on the socket).
///
/// Errors: any of [`FrameError`]; a short read (closed socket) is `Io`. A payload is allocated
/// at its announced length, after the [`MAX_PAYLOAD`] check.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Frame, FrameError> {
    let mut header = [0u8; HEADER_LEN];
    reader.read_exact(&mut header)?;
    let (len, kind) = decode_header(&header)?;
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;
    Ok(Frame { kind, payload })
}

/// Writes one request frame and flushes. Errors: `Io` when the write fails.
pub fn write_frame<W: Write>(
    writer: &mut W,
    kind: MessageType,
    payload: &[u8],
) -> Result<(), FrameError> {
    writer.write_all(&encode_frame(kind, payload))?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // The golden frames are written in little-endian order, the byte order of the machine they
    // were read on (x86-64); on a big-endian host the same frames differ in the two u32 fields,
    // so those tests are compiled only on little-endian targets.

    /// `GET_WORKSPACES` with an empty payload, as sent with `socat` to Sway 1.9.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_encode_get_workspaces_matches_the_bytes_sent_to_sway() {
        let want: &[u8] = b"i3-ipc\x00\x00\x00\x00\x01\x00\x00\x00";
        assert_eq!(encode_frame(MessageType::GetWorkspaces, b""), want);
        assert_eq!(want.len(), HEADER_LEN);
    }

    /// `SUBSCRIBE ["window"]`: length 10 (`0a 00 00 00`), type 2.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_encode_subscribe_window_matches_the_bytes_sent_to_sway() {
        let want: &[u8] = b"i3-ipc\x0a\x00\x00\x00\x02\x00\x00\x00[\"window\"]";
        assert_eq!(encode_frame(MessageType::Subscribe, b"[\"window\"]"), want);
    }

    /// The reply Sway 1.9 gave to the subscribe frame above: 17 bytes of `{"success": true}`.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_decode_the_subscribe_reply_read_from_sway() {
        let bytes: &[u8] = b"i3-ipc\x11\x00\x00\x00\x02\x00\x00\x00{\"success\": true}";
        let frame = read_frame(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(frame.kind, MessageType::Subscribe as u32);
        assert_eq!(frame.payload, b"{\"success\": true}");
        assert_eq!(
            frame.kind & 0x8000_0000,
            0,
            "a reply has the high bit clear"
        );
    }

    /// The reply to a `nop` command: `[ { "success": true } ]` is 23 bytes (`17 00 00 00`).
    #[cfg(target_endian = "little")]
    #[test]
    fn test_decode_the_run_command_reply_read_from_sway() {
        let bytes: &[u8] = b"i3-ipc\x17\x00\x00\x00\x00\x00\x00\x00[ { \"success\": true } ]";
        let frame = read_frame(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(frame.kind, MessageType::RunCommand as u32);
        assert_eq!(frame.payload.len(), 23);
    }

    /// The header of the `GET_WORKSPACES` reply seen on the wire: length `6f 02 00 00` = 623.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_decode_header_of_the_workspaces_reply() {
        let header: [u8; HEADER_LEN] = *b"i3-ipco\x02\x00\x00\x01\x00\x00\x00";
        assert_eq!(decode_header(&header).unwrap(), (623, 1));
    }

    /// A window event has type `0x80000003`, which is `03 00 00 80` in little-endian bytes.
    #[cfg(target_endian = "little")]
    #[test]
    fn test_decode_a_window_event_frame() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"i3-ipc\x02\x00\x00\x00\x03\x00\x00\x80{}");
        let frame = read_frame(&mut Cursor::new(bytes)).unwrap();
        assert_eq!(frame.kind, EVENT_WINDOW);
        assert_ne!(frame.kind & 0x8000_0000, 0, "an event has the high bit set");
        assert_eq!(frame.payload, b"{}");
    }

    /// Every request type survives encode, then decode, on any byte order.
    #[test]
    fn test_round_trip_for_every_request_type() {
        for kind in [
            MessageType::RunCommand,
            MessageType::GetWorkspaces,
            MessageType::Subscribe,
            MessageType::GetOutputs,
            MessageType::GetTree,
        ] {
            let bytes = encode_frame(kind, b"payload \xc3\xa9");
            let frame = read_frame(&mut Cursor::new(bytes)).unwrap();
            assert_eq!(frame.kind, kind as u32);
            assert_eq!(frame.payload, b"payload \xc3\xa9");
        }
    }

    /// The numbers of the request types are the ones in `man sway-ipc`.
    #[test]
    fn test_message_type_numbers() {
        assert_eq!(MessageType::RunCommand as u32, 0);
        assert_eq!(MessageType::GetWorkspaces as u32, 1);
        assert_eq!(MessageType::Subscribe as u32, 2);
        assert_eq!(MessageType::GetOutputs as u32, 3);
        assert_eq!(MessageType::GetTree as u32, 4);
        assert_eq!(EVENT_WINDOW, 0x8000_0003);
    }

    /// A stream that does not start with `i3-ipc` (an X11 or HTTP socket, say) is refused with a
    /// message naming `i3-ipc`, not read as a length.
    #[test]
    fn test_bad_magic_is_refused() {
        let bytes = b"HTTP/1.1 400 Bad Request\r\n".to_vec();
        match read_frame(&mut Cursor::new(bytes)) {
            Err(FrameError::BadMagic(m)) => assert_eq!(&m, b"HTTP/1"),
            other => panic!("expected BadMagic, got {other:?}"),
        }
        let msg = FrameError::BadMagic(*b"HTTP/1").to_string();
        assert!(msg.contains("i3-ipc") && msg.contains("$SWAYSOCK"), "{msg}");
    }

    /// A header that announces more than `MAX_PAYLOAD` bytes is refused before allocating.
    #[test]
    fn test_oversized_length_is_refused() {
        let mut header = Vec::new();
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&(MAX_PAYLOAD + 1).to_ne_bytes());
        header.extend_from_slice(&0u32.to_ne_bytes());
        match read_frame(&mut Cursor::new(header)) {
            Err(FrameError::TooLarge(n)) => assert_eq!(n, MAX_PAYLOAD + 1),
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    /// A socket that closes before the whole header or payload arrived is an I/O error
    /// (`UnexpectedEof`), never a short payload returned as if complete.
    #[test]
    fn test_truncated_frames_are_io_errors() {
        let full = encode_frame(MessageType::RunCommand, b"nop nop nop");
        for cut in [3, HEADER_LEN - 1, HEADER_LEN + 4] {
            match read_frame(&mut Cursor::new(full[..cut].to_vec())) {
                Err(FrameError::Io(e)) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::UnexpectedEof, "cut {cut}")
                }
                other => panic!("cut {cut}: expected Io, got {other:?}"),
            }
        }
    }

    /// `write_frame` puts exactly `encode_frame` on the writer.
    #[test]
    fn test_write_frame_writes_the_encoded_bytes() {
        let mut sink = Vec::new();
        write_frame(&mut sink, MessageType::GetTree, b"").unwrap();
        assert_eq!(sink, encode_frame(MessageType::GetTree, b""));
    }
}
