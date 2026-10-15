//! The JSON Sway sends, parsed into plain types, and the conversions to what the neutral code
//! uses (`WindowMetadata`, `MonitorLayout`). No sockets here: every function takes bytes or
//! parsed values, so the unit tests run on JSON recorded from a private headless Sway 1.9
//! (`tests/fixtures/sway/`).
//!
//! All coordinates are Sway's *logical* pixels in one global layout (an output at scale 2 reports
//! its rect at half the mode size), the same space the placement engine works in, so no scaling
//! is done here.

use crate::platform::{MonitorLayout, Rect, WindowMetadata};
use serde::Deserialize;
use spawn_at_core::geometry::{LayoutRect, MonitorInfo};

/// Sway's `{x, y, width, height}`.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
pub struct SwayRect {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
}

impl From<SwayRect> for Rect {
    fn from(r: SwayRect) -> Self {
        Rect {
            x: r.x,
            y: r.y,
            width: r.width.max(0) as u32,
            height: r.height.max(0) as u32,
        }
    }
}

/// One entry of `GET_OUTPUTS`; only the fields spawn-at reads.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Output {
    pub name: String,
    /// False for an output that is connected but switched off; such outputs are not monitors.
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub rect: SwayRect,
}

/// One entry of `GET_WORKSPACES`; only the fields spawn-at reads.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Workspace {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub num: i32,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub visible: bool,
    /// The usable area: the output minus bars and layer-shell panels.
    #[serde(default)]
    pub rect: SwayRect,
}

/// The X11 properties of a window run by XWayland (`window_properties` in the tree).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct WindowProperties {
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub instance: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

/// A node of `GET_TREE` (root, output, workspace, split container or window) and the
/// `container` of a window event.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Node {
    #[serde(default)]
    pub id: u64,
    /// `root`, `output`, `workspace`, `con` or `floating_con`.
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub pid: Option<u32>,
    /// Wayland app id; null for XWayland windows.
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub window_properties: Option<WindowProperties>,
    /// Workspace number (workspace nodes only; `-1` for the scratchpad).
    #[serde(default)]
    pub num: Option<i32>,
    #[serde(default)]
    pub focused: bool,
    /// 0 none, 1 workspace-wide, 2 global; nonzero means the window is fullscreen.
    #[serde(default)]
    pub fullscreen_mode: Option<i32>,
    /// The container including its border and title bar, in global layout coordinates.
    #[serde(default)]
    pub rect: SwayRect,
    /// The part of `rect` inside the border and title bar.
    #[serde(default)]
    pub window_rect: SwayRect,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub floating_nodes: Vec<Node>,
}

/// A `window` event: `change` is `new`, `close`, `focus`, `title`, `floating`, ...
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WindowEvent {
    pub change: String,
    pub container: Node,
}

fn parse<T: serde::de::DeserializeOwned>(what: &str, bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes)
        .map_err(|e| format!("cannot parse the {what} reply from Sway: {e}"))
}

/// The reply to `GET_OUTPUTS`.
pub fn parse_outputs(bytes: &[u8]) -> Result<Vec<Output>, String> {
    parse("GET_OUTPUTS", bytes)
}

/// The reply to `GET_WORKSPACES`.
pub fn parse_workspaces(bytes: &[u8]) -> Result<Vec<Workspace>, String> {
    parse("GET_WORKSPACES", bytes)
}

/// The reply to `GET_TREE`.
pub fn parse_tree(bytes: &[u8]) -> Result<Node, String> {
    parse("GET_TREE", bytes)
}

/// The payload of a `window` event frame.
pub fn parse_window_event(bytes: &[u8]) -> Result<WindowEvent, String> {
    parse("window event", bytes)
}

/// Checks the reply to `SUBSCRIBE`, which is `{"success": true}`.
pub fn check_subscribe_reply(bytes: &[u8]) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Reply {
        #[serde(default)]
        success: bool,
    }
    let reply: Reply = parse("SUBSCRIBE", bytes)?;
    if reply.success {
        Ok(())
    } else {
        Err("Sway refused the window event subscription".to_string())
    }
}

/// Checks the reply to `RUN_COMMAND`: an array with one `{"success": true}` per command, or
/// `{"success": false, "parse_error": .., "error": ".."}` for the ones that failed. Returns the
/// first error text.
pub fn check_command_reply(bytes: &[u8]) -> Result<(), String> {
    #[derive(Deserialize)]
    struct Entry {
        #[serde(default)]
        success: bool,
        #[serde(default)]
        error: Option<String>,
    }
    let entries: Vec<Entry> = parse("RUN_COMMAND", bytes)?;
    for entry in entries {
        if !entry.success {
            return Err(entry
                .error
                .unwrap_or_else(|| "Sway rejected the command".to_string()));
        }
    }
    Ok(())
}

/// True for a client window: a `con` or `floating_con` leaf that Sway knows a process, an app id
/// or X11 properties for. A split container has none of these and has children.
pub fn is_window(node: &Node) -> bool {
    (node.kind == "con" || node.kind == "floating_con")
        && node.nodes.is_empty()
        && node.floating_nodes.is_empty()
        && (node.pid.is_some() || node.app_id.is_some() || node.window_properties.is_some())
}

/// The window class: the Wayland app id, else the X11 class (empty if neither).
pub fn node_class(node: &Node) -> String {
    node.app_id
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            node.window_properties
                .as_ref()
                .and_then(|p| p.class.clone())
        })
        .unwrap_or_default()
}

/// Every window under `root`, in tree order (outputs in order, tiled before floating). `outputs`
/// gives the monitor index of each window (position among the active outputs, the index
/// `get_layout` uses).
pub fn windows_from_tree(root: &Node, outputs: &[Output]) -> Vec<WindowMetadata> {
    let active: Vec<&Output> = outputs.iter().filter(|o| o.active).collect();
    let mut out = Vec::new();
    for output in &root.nodes {
        let monitor = active
            .iter()
            .position(|o| output.name.as_deref() == Some(o.name.as_str()))
            .map(|i| i as i32);
        // The pseudo output `__i3` holds `__i3_scratch`, the scratchpad: windows there are hidden.
        let hidden = output.name.as_deref() == Some("__i3");
        for ws in &output.nodes {
            let num = ws.num.filter(|n| *n >= 0);
            collect_windows(ws, monitor, num, hidden, &mut out);
        }
    }
    out
}

fn collect_windows(
    node: &Node,
    monitor: Option<i32>,
    workspace: Option<i32>,
    hidden: bool,
    out: &mut Vec<WindowMetadata>,
) {
    if is_window(node) {
        let rect: Rect = node.rect.into();
        let fullscreen = node.fullscreen_mode.unwrap_or(0) != 0;
        out.push(WindowMetadata {
            id: Some(node.id),
            pid: node.pid,
            title: node.name.clone().unwrap_or_default(),
            class: node_class(node),
            app_id: node.app_id.clone(),
            x: rect.x,
            y: rect.y,
            w: rect.width as i32,
            h: rect.height as i32,
            rel: None,
            area: None,
            focused: node.focused,
            maximized: false,
            minimized: hidden,
            monitor,
            // The words the GNOME extension uses (`normal`, `fullscreen`); Sway has no maximize.
            state: Some(if fullscreen { "fullscreen" } else { "normal" }.to_string()),
            workspace,
        });
        return;
    }
    for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
        collect_windows(child, monitor, workspace, hidden, out);
    }
}

/// The node with `id` under `root`.
pub fn find_node(root: &Node, id: u64) -> Option<&Node> {
    if root.id == id && (root.kind == "con" || root.kind == "floating_con") {
        return Some(root);
    }
    root.nodes
        .iter()
        .chain(root.floating_nodes.iter())
        .find_map(|child| find_node(child, id))
}

/// The ids of every window under `root` (what `arm` records so the new window can be told from
/// the old ones).
pub fn window_ids(root: &Node) -> Vec<u64> {
    fn walk(node: &Node, out: &mut Vec<u64>) {
        if is_window(node) {
            out.push(node.id);
            return;
        }
        for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
            walk(child, out);
        }
    }
    let mut ids = Vec::new();
    walk(root, &mut ids);
    ids
}

/// The monitors: the active outputs in the order Sway lists them. Sway has no primary monitor,
/// so the first one is flagged primary (the X11 driver does the same); `screen` is the output
/// rect and `workarea` the rect of the output's visible workspace, which excludes bars (equal to
/// `screen` when no workspace is visible there).
pub fn layout_from(outputs: &[Output], workspaces: &[Workspace]) -> MonitorLayout {
    let monitors = outputs
        .iter()
        .filter(|o| o.active)
        .enumerate()
        .map(|(index, o)| {
            let screen: LayoutRect = Rect::from(o.rect).into();
            let workarea: LayoutRect = workspaces
                .iter()
                .find(|ws| ws.output == o.name && ws.visible)
                .map(|ws| Rect::from(ws.rect).into())
                .unwrap_or(screen);
            MonitorInfo {
                index,
                name: Some(o.name.clone()),
                primary: index == 0,
                scale: o.scale,
                screen: Some(screen),
                workarea: Some(workarea),
                insets: MonitorInfo::compute_insets(Some(screen), Some(workarea)),
            }
        })
        .collect();
    MonitorLayout {
        schema_version: 1,
        monitors,
        note: Some(
            "Sway has no primary monitor: monitor 0 is the first output Sway lists. Coordinates are logical pixels."
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUTS_ONE: &[u8] = include_bytes!("../../../../tests/fixtures/sway/outputs_one.json");
    const WORKSPACES_ONE: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/workspaces_one.json");
    const OUTPUTS_SCALE2_BAR: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/outputs_scale2_bar.json");
    const WORKSPACES_SCALE2_BAR: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/workspaces_scale2_bar.json");
    const OUTPUTS_TWO: &[u8] = include_bytes!("../../../../tests/fixtures/sway/outputs_two.json");
    const WORKSPACES_TWO: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/workspaces_two.json");
    const TREE_EMPTY: &[u8] = include_bytes!("../../../../tests/fixtures/sway/tree_empty.json");
    const TREE_TILED: &[u8] = include_bytes!("../../../../tests/fixtures/sway/tree_tiled.json");
    const TREE_FLOATING: &[u8] =
        include_bytes!("../../../../tests/fixtures/sway/tree_floating.json");
    const WINDOW_EVENTS: &str = include_str!("../../../../tests/fixtures/sway/window_events.jsonl");

    fn r(x: i32, y: i32, w: u32, h: u32) -> LayoutRect {
        LayoutRect { x, y, w, h }
    }

    /// One 1920x1080 output, scale 1, no bar: screen and work area are the whole output.
    #[test]
    fn test_layout_of_one_plain_output() {
        let layout = layout_from(
            &parse_outputs(OUTPUTS_ONE).unwrap(),
            &parse_workspaces(WORKSPACES_ONE).unwrap(),
        );
        assert_eq!(layout.monitors.len(), 1);
        let m = &layout.monitors[0];
        assert_eq!(m.name.as_deref(), Some("HEADLESS-1"));
        assert!(m.primary);
        assert_eq!(m.index, 0);
        assert_eq!(m.scale, Some(1.0));
        assert_eq!(m.screen, Some(r(0, 0, 1920, 1080)));
        assert_eq!(m.workarea, Some(r(0, 0, 1920, 1080)));
    }

    /// Scale 2 with a 30 px top bar: the rects are logical (960x540, not the 1920x1080 mode) and
    /// the work area starts at y=30 and is 510 high, so the insets are 30 on top only.
    #[test]
    fn test_layout_of_a_scaled_output_with_a_bar() {
        let layout = layout_from(
            &parse_outputs(OUTPUTS_SCALE2_BAR).unwrap(),
            &parse_workspaces(WORKSPACES_SCALE2_BAR).unwrap(),
        );
        let m = &layout.monitors[0];
        assert_eq!(m.scale, Some(2.0));
        assert_eq!(m.screen, Some(r(0, 0, 960, 540)));
        assert_eq!(m.workarea, Some(r(0, 30, 960, 510)));
        let insets = m.insets.unwrap();
        assert_eq!(
            (insets.top, insets.right, insets.bottom, insets.left),
            (30, 0, 0, 0)
        );
    }

    /// Two outputs: the second sits at x=1920 in the global layout and its work area is its own
    /// workspace's rect, not the first output's; the first listed output is the primary.
    #[test]
    fn test_layout_of_two_outputs() {
        let layout = layout_from(
            &parse_outputs(OUTPUTS_TWO).unwrap(),
            &parse_workspaces(WORKSPACES_TWO).unwrap(),
        );
        assert_eq!(layout.monitors.len(), 2);
        assert_eq!(layout.monitors[0].name.as_deref(), Some("HEADLESS-1"));
        assert_eq!(layout.monitors[1].name.as_deref(), Some("HEADLESS-2"));
        assert_eq!(layout.monitors[1].index, 1);
        assert_eq!(layout.monitors[1].screen, Some(r(1920, 0, 1280, 720)));
        assert_eq!(layout.monitors[1].workarea, Some(r(1920, 0, 1280, 720)));
        assert!(layout.monitors[0].primary);
        assert!(!layout.monitors[1].primary);
    }

    /// An output that is switched off is not a monitor, and one without a visible workspace
    /// falls back to its own rect as the work area.
    #[test]
    fn test_inactive_outputs_are_skipped_and_work_area_falls_back_to_the_screen() {
        let outputs = vec![
            Output {
                name: "OFF-1".into(),
                active: false,
                focused: false,
                scale: None,
                rect: SwayRect::default(),
            },
            Output {
                name: "ON-1".into(),
                active: true,
                focused: true,
                scale: Some(1.5),
                rect: SwayRect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
            },
        ];
        let layout = layout_from(&outputs, &[]);
        assert_eq!(layout.monitors.len(), 1);
        assert_eq!(layout.monitors[0].name.as_deref(), Some("ON-1"));
        assert_eq!(layout.monitors[0].index, 0);
        assert_eq!(layout.monitors[0].workarea, Some(r(0, 0, 1280, 720)));
    }

    /// The empty tree has no windows (the workspace and the `__i3` scratchpad are not windows).
    #[test]
    fn test_empty_tree_has_no_windows() {
        let tree = parse_tree(TREE_EMPTY).unwrap();
        let outputs = parse_outputs(OUTPUTS_ONE).unwrap();
        assert!(windows_from_tree(&tree, &outputs).is_empty());
        assert!(window_ids(&tree).is_empty());
    }

    /// The tiled fixture window: pid and app id from Sway, filling the 1920x1080 output, on
    /// monitor 0 and workspace 1, state `normal`.
    #[test]
    fn test_tiled_window_maps_to_metadata() {
        let tree = parse_tree(TREE_TILED).unwrap();
        let outputs = parse_outputs(OUTPUTS_ONE).unwrap();
        let windows = windows_from_tree(&tree, &outputs);
        assert_eq!(windows.len(), 1);
        let w = &windows[0];
        assert_eq!(w.id, Some(5));
        assert_eq!(w.title, "sway-cap-1");
        assert_eq!(w.class, "dev.spawnat.swaycap");
        assert_eq!(w.app_id.as_deref(), Some("dev.spawnat.swaycap"));
        assert!(w.pid.is_some());
        assert_eq!((w.x, w.y, w.w, w.h), (0, 0, 1920, 1080));
        assert_eq!(w.monitor, Some(0));
        assert_eq!(w.workspace, Some(1));
        assert_eq!(w.state.as_deref(), Some("normal"));
        assert!(w.focused);
        assert!(!w.maximized && !w.minimized);
    }

    /// The window after `floating enable, resize set 800 500, move absolute position 100 150`
    /// is a `floating_con` in `floating_nodes`, and its rect is that placement.
    #[test]
    fn test_floating_window_maps_to_metadata() {
        let tree = parse_tree(TREE_FLOATING).unwrap();
        let outputs = parse_outputs(OUTPUTS_ONE).unwrap();
        let windows = windows_from_tree(&tree, &outputs);
        assert_eq!(windows.len(), 1);
        let w = &windows[0];
        assert_eq!((w.x, w.y, w.w, w.h), (100, 150, 800, 500));
        assert_eq!(w.workspace, Some(1));
        let node = find_node(&tree, 5).unwrap();
        assert_eq!(node.kind, "floating_con");
        assert_eq!(window_ids(&tree), vec![5]);
    }

    /// `find_node` finds only containers by id: id 3 is the output, not a window.
    #[test]
    fn test_find_node_ignores_non_window_nodes() {
        let tree = parse_tree(TREE_FLOATING).unwrap();
        assert!(find_node(&tree, 3).is_none());
        assert!(find_node(&tree, 999).is_none());
    }

    /// A window in the scratchpad (`__i3` output) is listed as minimized with no monitor and no
    /// workspace; an XWayland window has no app id, so its class is the X11 class; a fullscreen
    /// window says so.
    #[test]
    fn test_scratchpad_xwayland_and_fullscreen_windows() {
        let window = |id: u64, class: Option<&str>, fullscreen: i32| Node {
            id,
            kind: "con".into(),
            name: Some(format!("w{id}")),
            pid: Some(100 + id as u32),
            app_id: None,
            window_properties: class.map(|c| WindowProperties {
                class: Some(c.into()),
                instance: Some("inst".into()),
                title: Some("t".into()),
            }),
            fullscreen_mode: Some(fullscreen),
            rect: SwayRect {
                x: 1,
                y: 2,
                width: 30,
                height: 40,
            },
            ..Node::default()
        };
        let ws = |num: i32, nodes: Vec<Node>| Node {
            id: 50 + num.unsigned_abs() as u64,
            kind: "workspace".into(),
            num: Some(num),
            nodes,
            ..Node::default()
        };
        let output = |name: &str, nodes: Vec<Node>| Node {
            id: 40,
            kind: "output".into(),
            name: Some(name.into()),
            nodes,
            ..Node::default()
        };
        let root = Node {
            kind: "root".into(),
            nodes: vec![
                output("__i3", vec![ws(-1, vec![window(7, Some("Scratch"), 0)])]),
                output(
                    "HEADLESS-1",
                    vec![ws(3, vec![window(8, Some("XTerm"), 0), window(9, None, 1)])],
                ),
            ],
            ..Node::default()
        };
        let outputs = parse_outputs(OUTPUTS_ONE).unwrap();
        let windows = windows_from_tree(&root, &outputs);
        let by_id = |id: u64| windows.iter().find(|w| w.id == Some(id)).unwrap();
        let scratch = by_id(7);
        assert!(scratch.minimized);
        assert_eq!((scratch.monitor, scratch.workspace), (None, None));
        let xterm = by_id(8);
        assert_eq!(xterm.class, "XTerm");
        assert_eq!(xterm.app_id, None);
        assert_eq!((xterm.monitor, xterm.workspace), (Some(0), Some(3)));
        assert_eq!(by_id(9).state.as_deref(), Some("fullscreen"));
        assert_eq!(by_id(9).class, "");
    }

    /// The four events Sway sent for one fixture window: new (rect 0x0, name null), title,
    /// focus, close. The pid is the same in all of them.
    #[test]
    fn test_window_events_recorded_from_sway() {
        let events: Vec<WindowEvent> = WINDOW_EVENTS
            .lines()
            .map(|l| parse_window_event(l.as_bytes()).unwrap())
            .collect();
        let changes: Vec<&str> = events.iter().map(|e| e.change.as_str()).collect();
        assert_eq!(changes, ["new", "title", "focus", "close"]);
        assert_eq!(events[0].container.name, None);
        assert_eq!(events[0].container.rect, SwayRect::default());
        assert_eq!(events[1].container.name.as_deref(), Some("ev-1"));
        assert_eq!(
            events[0].container.app_id.as_deref(),
            Some("dev.spawnat.swaycap")
        );
        assert!(events
            .iter()
            .all(|e| e.container.pid == events[0].container.pid));
        assert!(is_window(&events[0].container));
    }

    /// The `RUN_COMMAND` reply: success for every command, or the first error text.
    #[test]
    fn test_command_reply_checking() {
        assert!(check_command_reply(b"[ { \"success\": true }, { \"success\": true } ]").is_ok());
        let bad = br#"[{"success":true},{"success":false,"parse_error":true,"error":"Unknown/invalid command 'florting'"}]"#;
        assert_eq!(
            check_command_reply(bad).unwrap_err(),
            "Unknown/invalid command 'florting'"
        );
        assert_eq!(
            check_command_reply(br#"[{"success":false}]"#).unwrap_err(),
            "Sway rejected the command"
        );
        assert!(check_command_reply(b"not json").is_err());
    }

    /// The `SUBSCRIBE` reply read from Sway is accepted; a refusal is an error.
    #[test]
    fn test_subscribe_reply_checking() {
        assert!(check_subscribe_reply(b"{\"success\": true}").is_ok());
        assert!(check_subscribe_reply(b"{\"success\": false}").is_err());
        assert!(check_subscribe_reply(b"[]").is_err());
    }

    /// Malformed JSON gives an error that says which reply it was.
    #[test]
    fn test_parse_errors_name_the_reply() {
        assert!(parse_outputs(b"{").unwrap_err().contains("GET_OUTPUTS"));
        assert!(parse_workspaces(b"{")
            .unwrap_err()
            .contains("GET_WORKSPACES"));
        assert!(parse_tree(b"not json").unwrap_err().contains("GET_TREE"));
        assert!(parse_window_event(b"{}")
            .unwrap_err()
            .contains("window event"));
    }
}
