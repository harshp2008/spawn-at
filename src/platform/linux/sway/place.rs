//! The write side over blocking connections: waiting for the new window, placing it, and the
//! single-window commands. The text of the commands is in `command`, the choice of window in
//! `matching`; this module only sequences them over a `Client`.
//!
//! Everything here blocks: it is called from `SwayDriver` inside `spawn_blocking`, never from
//! async code directly. Each function takes a connection (or a socket path) and keeps no state.
//! Placement is a short series of `RUN_COMMAND` requests (the first one chains `floating enable`,
//! the optional resize and the move; `fit_and_anchor` and the focus command are further
//! requests), not a single request. Experimental: checked only on a private headless Sway 1.9.

use super::client::Client;
use super::command;
use super::ipc::{MessageType, EVENT_WINDOW};
use super::matching::Matcher;
use super::types::{self, Node};
use crate::platform::{DriverError, PlacementParams, WindowMetadata};
use crate::target::{self, WindowSelector};
use spawn_at_core::geometry::calculate_rect_from_placement;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long `post_spawn` waits for the launched application's window: 5 s. The X11 driver's
/// wait is also 5000 ms (`grep -n "5000" src/platform/linux/x11/interception.rs`).
pub const WINDOW_WAIT: Duration = Duration::from_secs(5);

/// How long the placement of a window without `--size` waits for it to report a size: 1 s, in
/// `settled_frame`.
const SIZE_WAIT: Duration = Duration::from_secs(1);

/// What `arm` prepares before the application starts: a connection subscribed to `window`
/// events (it buffers them until `post_spawn` reads them) and the ids of the windows that
/// already exist.
pub struct Pending {
    pub events: Client,
    pub snapshot: HashSet<u64>,
}

/// Subscribes to `window` events, then records the existing windows. Subscribing first means no
/// window can appear unseen between the two.
///
/// Opens two connections: one that stays subscribed (kept in the result) and one used only for
/// `GET_TREE`. Errors: `IpcError` when a connection, the subscription or the tree fails.
pub fn subscribe_and_snapshot(socket: &Path) -> Result<Pending, DriverError> {
    let mut events = Client::connect(socket)?;
    let reply = events.request(MessageType::Subscribe, br#"["window"]"#)?;
    types::check_subscribe_reply(&reply).map_err(DriverError::IpcError)?;
    let mut request = Client::connect(socket)?;
    let tree = fetch_tree(&mut request)?;
    Ok(Pending {
        events,
        snapshot: types::window_ids(&tree).into_iter().collect(),
    })
}

/// `GET_TREE` parsed. Errors: `IpcError` for the request or for JSON that does not parse.
pub fn fetch_tree(client: &mut Client) -> Result<Node, DriverError> {
    types::parse_tree(&client.request(MessageType::GetTree, b"")?).map_err(DriverError::IpcError)
}

/// Sends one command string and fails with Sway's own error text if any part is refused.
pub fn run(client: &mut Client, command: &str) -> Result<(), DriverError> {
    let reply = client.request(MessageType::RunCommand, command.as_bytes())?;
    types::check_command_reply(&reply)
        .map_err(|e| DriverError::IpcError(format!("Sway refused `{command}`: {e}")))
}

/// Reads `window` events until one is the application's window; its container id.
///
/// `is_descendant(window_pid, launched_pid)` is the process-tree test. A window that matches
/// only by application id is returned after a short grace (see `matching`), or when `timeout`
/// ends. No match is a `TargetNotFound` error saying the application was started but not placed.
///
/// Polls the event connection in slices of at most 50 ms until `timeout`. Only `new` and
/// `title` events are looked at; a payload that does not parse is skipped without a message.
/// Errors from the connection (closed, malformed frame) end the wait with `IpcError`.
pub fn wait_for_window(
    events: &mut Client,
    matcher: &mut Matcher,
    timeout: Duration,
    is_descendant: &dyn Fn(u32, u32) -> bool,
) -> Result<u64, DriverError> {
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed();
        let last_chance = elapsed >= timeout;
        if let Some(id) = matcher.app_id_fallback(elapsed, last_chance) {
            return Ok(id);
        }
        if last_chance {
            return Err(DriverError::TargetNotFound(format!(
                "no window of the application appeared in Sway within {} ms; it was started but not placed",
                timeout.as_millis()
            )));
        }
        let wait = (timeout - elapsed).min(Duration::from_millis(50));
        if let Some(frame) = events.next_frame(wait)? {
            if frame.kind != EVENT_WINDOW {
                continue;
            }
            // A payload that does not parse is skipped: one odd event must not stop the wait.
            let Ok(event) = types::parse_window_event(&frame.payload) else {
                continue;
            };
            // `new` carries the app id and pid; `title` re-offers an XWayland window whose
            // class was not known yet at `new`.
            if event.change == "new" || event.change == "title" {
                if let Some(id) = matcher.offer(&event.container, start.elapsed(), is_descendant) {
                    return Ok(id);
                }
            }
        }
    }
}

/// The container `id` as the tree has it now.
fn current_node(client: &mut Client, id: u64) -> Result<Node, DriverError> {
    let tree = fetch_tree(client)?;
    types::find_node(&tree, id)
        .cloned()
        .ok_or_else(|| DriverError::TargetNotFound(format!("Window ID {id} is gone")))
}

/// The container's size once it has one and it stopped changing (two equal reads 40 ms apart),
/// or the last nonzero size when `SIZE_WAIT` ends.
fn settled_frame(client: &mut Client, id: u64) -> Result<(u32, u32), DriverError> {
    let start = Instant::now();
    let mut last: Option<(u32, u32)> = None;
    while start.elapsed() < SIZE_WAIT {
        let node = current_node(client, id)?;
        let size = (
            node.rect.width.max(0) as u32,
            node.rect.height.max(0) as u32,
        );
        if size.0 > 0 && size.1 > 0 {
            if last == Some(size) {
                return Ok(size);
            }
            last = Some(size);
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    last.ok_or_else(|| {
        DriverError::Execution(
            format!(
                "Window ID {id} did not report a size within {} ms",
                SIZE_WAIT.as_millis()
            )
            .into(),
        )
    })
}

/// After a sized placement: if the container is not `frame` large because of its border and
/// title bar (which `resize set` does not count), asks once for the content size that makes the
/// whole container `frame`. If the size still is not `frame` (the window has a minimum size and
/// was raised, as the GNOME extension reports it), the position is computed again for the size
/// the window really has, so an anchor such as bottom-right keeps the window inside the area.
fn fit_and_anchor(
    client: &mut Client,
    id: u64,
    params: &PlacementParams,
    frame: (u32, u32),
    at: (i32, i32),
) -> Result<(), DriverError> {
    let size_of = |node: &Node| {
        (
            node.rect.width.max(0) as u32,
            node.rect.height.max(0) as u32,
        )
    };
    let mut node = current_node(client, id)?;
    let mut actual = size_of(&node);
    if actual != frame {
        let content = (
            node.window_rect.width.max(0) as u32,
            node.window_rect.height.max(0) as u32,
        );
        let wanted = command::content_size_for_frame(frame, actual, content);
        // Only ask when there is a border or title bar to subtract: without one the content
        // size just requested was already `frame`, and a window held at its minimum size would
        // be refused again.
        if actual != content && wanted != content {
            run(client, &command::resize_and_move(id, wanted, at.0, at.1))?;
            node = current_node(client, id)?;
            actual = size_of(&node);
        }
    }
    if actual != frame && actual.0 > 0 && actual.1 > 0 {
        let mut raised = params.clone();
        raised.size = Some(actual);
        let rect = calculate_rect_from_placement(&raised, actual.0, actual.1);
        run(client, &command::move_to(id, rect.x, rect.y))?;
    }
    Ok(())
}

/// Makes window `id` floating and puts it where `params` say. With a size the window is
/// resized so the whole container (what `query windows` reports) has that size, the meaning
/// `--size` has on GNOME; without one it keeps the size its client chose.
///
/// With a size: one chained command, then `fit_and_anchor` (up to two more requests and some
/// `GET_TREE` reads). Without: `floating enable`, a wait of up to 1 s for a size, then a move.
/// `exclusive_focus` adds a `focus` command. Errors: `IpcError` when Sway refuses a command
/// (with Sway's own text), `Execution` when the window never reports a size, `TargetNotFound`
/// when it disappeared. A command that fails midway leaves the window as far as it got.
pub fn place_window(
    client: &mut Client,
    id: u64,
    params: &PlacementParams,
    exclusive_focus: bool,
) -> Result<(), DriverError> {
    match params.size {
        Some((w, h)) => {
            let rect = calculate_rect_from_placement(params, w, h);
            run(client, &command::place(id, Some((w, h)), rect.x, rect.y))?;
            fit_and_anchor(client, id, params, (w, h), (rect.x, rect.y))?;
        }
        None => {
            run(client, &command::floating_enable(id))?;
            let (w, h) = settled_frame(client, id)?;
            let rect = calculate_rect_from_placement(params, w, h);
            run(client, &command::move_to(id, rect.x, rect.y))?;
        }
    }
    if exclusive_focus {
        run(client, &command::focus(id))?;
    }
    Ok(())
}

/// Moves and resizes an existing window to the rectangle `params` give for a window of the
/// current size `current` (pixels). Always sends a resize to the rectangle's size, even when
/// `params` have no size; then `fit_and_anchor` as in `place_window`.
pub fn transform(
    client: &mut Client,
    id: u64,
    params: &PlacementParams,
    current: (u32, u32),
) -> Result<(), DriverError> {
    let rect = calculate_rect_from_placement(params, current.0, current.1);
    let frame = (rect.width, rect.height);
    run(client, &command::place(id, Some(frame), rect.x, rect.y))?;
    fit_and_anchor(client, id, params, frame, (rect.x, rect.y))
}

/// The container id for a target given as a window id, a pid or a class, the way the X11 driver
/// resolves it: an id that exists wins, then a pid or class through the shared selector.
/// A string that parses as a number is tried as an id first, then as a pid; anything else is a
/// class. Errors: `TargetNotFound` with the selector's message when nothing matches. Which window
/// wins when several match is decided by `target::resolve_target`, not here.
pub fn resolve_window_id(windows: &[WindowMetadata], target_id: &str) -> Result<u64, DriverError> {
    if let Ok(n) = target_id.parse::<u64>() {
        if windows.iter().any(|w| w.id == Some(n)) {
            return Ok(n);
        }
    }
    let pid = target_id.parse::<u32>().ok();
    let selector = WindowSelector {
        id: None,
        class: if pid.is_none() {
            Some(target_id.to_string())
        } else {
            None
        },
        title: None,
        pid,
        focused: false,
    };
    let found = target::resolve_target(windows, &selector).map_err(DriverError::TargetNotFound)?;
    found
        .id
        .ok_or_else(|| DriverError::TargetNotFound("Window missing ID".into()))
}
