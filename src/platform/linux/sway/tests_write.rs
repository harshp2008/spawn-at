//! Placement, matching and the single-window commands against the scripted fake IPC server
//! (`fake.rs`). No Sway is involved; what Sway really does with these commands was measured on
//! a private headless Sway and is in `docs/DESIGN_SWAY.md`.

use super::fake::{FakeSway, Script};
use super::ipc::MessageType;
use super::matching::Matcher;
use super::*;
use crate::platform::{Batch, FocusIntent, Reveal, Urgency};
use serde_json::json;
use spawn_at_core::driver::Entry;
use spawn_at_core::geometry::Anchor;
use std::collections::HashSet;
use std::time::Duration;

const OUTPUTS_ONE: &[u8] = include_bytes!("../../../../tests/fixtures/sway/outputs_one.json");
const TREE_EMPTY: &[u8] = include_bytes!("../../../../tests/fixtures/sway/tree_empty.json");
const TREE_FLOATING: &[u8] = include_bytes!("../../../../tests/fixtures/sway/tree_floating.json");
const OK: &[u8] = br#"[{"success":true}]"#;

/// A tree with one floating window `id` (pid 4242, app id `demo`).
fn tree_with(id: u64, rect: (i32, i32, i32, i32), content: (i32, i32)) -> Vec<u8> {
    let window = json!({
        "id": id, "type": "floating_con", "pid": 4242, "app_id": "demo", "name": "w",
        "rect": {"x": rect.0, "y": rect.1, "width": rect.2, "height": rect.3},
        "window_rect": {"x": 0, "y": 0, "width": content.0, "height": content.1},
        "nodes": [], "floating_nodes": []
    });
    let ws =
        json!({"id": 4, "type": "workspace", "num": 1, "nodes": [], "floating_nodes": [window]});
    let output = json!({"id": 3, "type": "output", "name": "HEADLESS-1", "nodes": [ws], "floating_nodes": []});
    serde_json::to_vec(&json!({"id": 1, "type": "root", "nodes": [output], "floating_nodes": []}))
        .unwrap()
}

/// A `window` event of change `change` for window `id` with the given pid and app id.
fn event(change: &str, id: u64, pid: u32, app_id: &str) -> String {
    json!({"change": change, "container": {
        "id": id, "type": "con", "pid": pid, "app_id": app_id, "name": null,
        "rect": {"x": 0, "y": 0, "width": 0, "height": 0}, "nodes": [], "floating_nodes": []
    }})
    .to_string()
}

fn batch_with(params: PlacementParams, app_hint: &str, focus: FocusIntent) -> Batch {
    Batch {
        id: 1,
        entries: vec![Entry {
            key: "k".into(),
            app_hint: app_hint.into(),
            placement: params,
            wait: Default::default(),
        }],
        reveal: Reveal::Together,
        focus,
        urgency: Urgency::Normal,
        deadline: Duration::from_secs(15),
    }
}

fn params(size: Option<(u32, u32)>, anchor: Option<Anchor>, margin: i32) -> PlacementParams {
    PlacementParams {
        size,
        anchor,
        margin,
        workarea: Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        ..PlacementParams::default()
    }
}

fn base_script() -> Script {
    Script::default()
        .reply(MessageType::GetOutputs, OUTPUTS_ONE)
        .reply(MessageType::Subscribe, br#"{"success": true}"#)
        .reply(MessageType::RunCommand, OK)
}

/// The full path: `arm` subscribes and snapshots, the `new` event for the launched pid arrives,
/// and one chained command places the window: top-left anchor, 800x500, margin 20 gives
/// position 20,20. The requests show the order: subscribe first, then the snapshot.
#[tokio::test]
async fn test_arm_then_post_spawn_places_the_window_with_one_chained_command() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (20, 20, 800, 500), (800, 500)),
            )
            .event(Duration::from_millis(30), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(Some((800, 500)), Some(Anchor::TopLeft), 20),
        "demo",
        FocusIntent::Leave,
    );
    let armed = driver.arm(batch.clone()).await.unwrap();
    assert!(armed.launch_env.is_empty() && armed.token.is_none());
    driver.post_spawn(4242, &batch).await.unwrap();

    let kinds: Vec<u32> = fake.requests().iter().map(|(k, _)| *k).collect();
    let subscribe = kinds
        .iter()
        .position(|k| *k == MessageType::Subscribe as u32)
        .unwrap();
    let snapshot = kinds
        .iter()
        .position(|k| *k == MessageType::GetTree as u32)
        .unwrap();
    assert!(
        subscribe < snapshot,
        "subscribe before the snapshot: {kinds:?}"
    );
    assert_eq!(
        fake.commands(),
        ["[con_id=9] floating enable, resize set 800 500, move absolute position 20 20"]
    );
}

/// The anchors reuse the shared placement math: bottom-right with margin 30 on the 1920x1080
/// area puts an 800x500 window at (1920-30-800, 1080-30-500) = (1090, 550).
#[tokio::test]
async fn test_bottom_right_anchor_uses_the_shared_math() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (1090, 550, 800, 500), (800, 500)),
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(Some((800, 500)), Some(Anchor::BottomRight), 30),
        "demo",
        FocusIntent::Leave,
    );
    driver.arm(batch.clone()).await.unwrap();
    driver.post_spawn(4242, &batch).await.unwrap();
    assert_eq!(
        fake.commands(),
        ["[con_id=9] floating enable, resize set 800 500, move absolute position 1090 550"]
    );
}

/// With a work area that is not at the origin (second output, or below a bar) the position is
/// global: centre of the 1280x690 area at 960,30, for a 400x300 window, is (1400, 225), sent
/// with `move absolute`.
#[tokio::test]
async fn test_position_is_global_for_a_work_area_away_from_the_origin() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (1400, 225, 400, 300), (400, 300)),
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let mut p = params(Some((400, 300)), Some(Anchor::Center), 0);
    p.workarea = Rect {
        x: 960,
        y: 30,
        width: 1280,
        height: 690,
    };
    let batch = batch_with(p, "demo", FocusIntent::Leave);
    driver.arm(batch.clone()).await.unwrap();
    driver.post_spawn(4242, &batch).await.unwrap();
    assert_eq!(
        fake.commands(),
        ["[con_id=9] floating enable, resize set 400 300, move absolute position 1400 225"]
    );
}

/// Without `--size` the window keeps the size its client chose: no `resize`, and the anchor is
/// computed from the size the tree reports (640x400): bottom-right, margin 10 -> (1270, 670).
#[tokio::test]
async fn test_without_a_size_the_window_is_not_resized() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (640, 340, 640, 400), (640, 400)),
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(None, Some(Anchor::BottomRight), 10),
        "demo",
        FocusIntent::Leave,
    );
    driver.arm(batch.clone()).await.unwrap();
    driver.post_spawn(4242, &batch).await.unwrap();
    assert_eq!(
        fake.commands(),
        [
            "[con_id=9] floating enable",
            "[con_id=9] move absolute position 1270 670"
        ]
    );
}

/// A window whose container came out 4 px wider and 24 px taller than asked (border and title
/// bar) is corrected once: the content size is reduced by exactly that.
#[tokio::test]
async fn test_decoration_is_corrected_once() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (20, 20, 804, 524), (800, 500)),
            )
            // after the correction the container is 800x500
            .reply(
                MessageType::GetTree,
                &tree_with(9, (20, 20, 800, 500), (796, 476)),
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(Some((800, 500)), Some(Anchor::TopLeft), 20),
        "demo",
        FocusIntent::Leave,
    );
    driver.arm(batch.clone()).await.unwrap();
    driver.post_spawn(4242, &batch).await.unwrap();
    assert_eq!(
        fake.commands(),
        [
            "[con_id=9] floating enable, resize set 800 500, move absolute position 20 20",
            "[con_id=9] resize set 796 476, move absolute position 20 20"
        ]
    );
}

/// `FocusIntent::Exclusive` adds a `focus` command; `Leave` does not.
#[tokio::test]
async fn test_exclusive_focus_is_requested() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (0, 0, 100, 100), (100, 100)),
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(Some((100, 100)), Some(Anchor::TopLeft), 0),
        "demo",
        FocusIntent::Exclusive,
    );
    driver.arm(batch.clone()).await.unwrap();
    driver.post_spawn(4242, &batch).await.unwrap();
    assert_eq!(fake.commands().last().unwrap(), "[con_id=9] focus");
}

/// Sway refusing the command is an error carrying Sway's own words, so the standard
/// "post-spawn hook failed" warning says why.
#[tokio::test]
async fn test_a_refused_command_is_an_error_with_sways_text() {
    let fake = FakeSway::start(
        Script::default()
            .reply(MessageType::GetOutputs, OUTPUTS_ONE)
            .reply(MessageType::Subscribe, br#"{"success": true}"#)
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::RunCommand,
                br#"[{"success":false,"parse_error":true,"error":"Unknown/invalid command"}]"#,
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(Some((100, 100)), Some(Anchor::TopLeft), 0),
        "demo",
        FocusIntent::Leave,
    );
    driver.arm(batch.clone()).await.unwrap();
    let err = driver
        .post_spawn(4242, &batch)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Unknown/invalid command") && err.contains("[con_id=9]"),
        "{err}"
    );
}

/// `post_spawn` without a preceding `arm` is an error and sends no command: it never guesses.
#[tokio::test]
async fn test_post_spawn_without_arm_is_an_error() {
    let fake = FakeSway::start(base_script());
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(params(None, None, 0), "demo", FocusIntent::Leave);
    assert!(driver.post_spawn(1, &batch).await.is_err());
    assert!(fake.commands().is_empty());
}

/// `disarm` drops the subscription once and reports whether there was one.
#[tokio::test]
async fn test_disarm_drops_the_subscription() {
    let fake = FakeSway::start(base_script().reply(MessageType::GetTree, TREE_EMPTY));
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(params(None, None, 0), "demo", FocusIntent::Leave);
    driver.arm(batch).await.unwrap();
    assert!(driver.disarm("t").await.unwrap());
    assert!(!driver.disarm("t").await.unwrap());
}

fn matcher_for(snapshot: &[u64], pid: u32, hint: &str) -> Matcher {
    Matcher::new(snapshot.iter().copied().collect::<HashSet<_>>(), pid, hint)
}

fn subscribed(fake: &FakeSway) -> client::Client {
    let mut c = client::Client::connect(&fake.path).unwrap();
    c.request(MessageType::Subscribe, br#"["window"]"#).unwrap();
    c
}

fn no_descendants(_: u32, _: u32) -> bool {
    false
}

/// A window that was in the snapshot is never chosen: the wait ends with the "started but not
/// placed" error instead of moving the old window.
#[test]
fn test_an_old_window_is_never_moved_and_the_wait_reports_it() {
    let fake = FakeSway::start(
        base_script().event(Duration::from_millis(5), &event("title", 7, 4242, "demo")),
    );
    let mut events = subscribed(&fake);
    let mut matcher = matcher_for(&[7], 4242, "demo");
    let err = place::wait_for_window(
        &mut events,
        &mut matcher,
        Duration::from_millis(400),
        &no_descendants,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("started but not placed"), "{err}");
}

/// Other windows opening meanwhile are skipped: only the launched pid's window is returned.
#[test]
fn test_other_windows_are_skipped_until_the_launched_one_appears() {
    let fake = FakeSway::start(
        base_script()
            .event(Duration::from_millis(5), &event("new", 11, 1, "other"))
            .event(Duration::from_millis(5), &event("focus", 12, 4242, "demo"))
            .event(Duration::from_millis(5), &event("new", 13, 4242, "demo")),
    );
    let mut events = subscribed(&fake);
    let mut matcher = matcher_for(&[], 4242, "demo");
    let id = place::wait_for_window(
        &mut events,
        &mut matcher,
        Duration::from_secs(2),
        &no_descendants,
    )
    .unwrap();
    assert_eq!(id, 13);
}

/// A window whose pid descends from the launched process matches (wrapper scripts).
#[test]
fn test_descendant_pid_matches() {
    let fake = FakeSway::start(
        base_script().event(Duration::from_millis(5), &event("new", 21, 5000, "x")),
    );
    let mut events = subscribed(&fake);
    let mut matcher = matcher_for(&[], 4242, "");
    let id = place::wait_for_window(
        &mut events,
        &mut matcher,
        Duration::from_secs(2),
        &|w, p| w == 5000 && p == 4242,
    )
    .unwrap();
    assert_eq!(id, 21);
}

/// A window that matches only by app id (the launcher handed over to a running server) is taken
/// after the 300 ms grace, not before.
#[test]
fn test_app_id_match_is_taken_after_the_grace() {
    let fake = FakeSway::start(
        base_script().event(Duration::from_millis(5), &event("new", 31, 9999, "demo")),
    );
    let mut events = subscribed(&fake);
    let mut matcher = matcher_for(&[], 4242, "demo");
    let start = std::time::Instant::now();
    let id = place::wait_for_window(
        &mut events,
        &mut matcher,
        Duration::from_secs(2),
        &no_descendants,
    )
    .unwrap();
    assert_eq!(id, 31);
    assert!(
        start.elapsed() >= Duration::from_millis(290),
        "{:?}",
        start.elapsed()
    );
}

/// Events that are not JSON for a window are skipped, not fatal.
#[test]
fn test_garbage_events_are_skipped() {
    let fake = FakeSway::start(
        base_script()
            .event(Duration::from_millis(5), "not json")
            .event(Duration::from_millis(5), &event("new", 41, 4242, "demo")),
    );
    let mut events = subscribed(&fake);
    let mut matcher = matcher_for(&[], 4242, "demo");
    assert_eq!(
        place::wait_for_window(
            &mut events,
            &mut matcher,
            Duration::from_secs(2),
            &no_descendants
        )
        .unwrap(),
        41
    );
}

/// Closing sends `kill` for the container with that id; focusing sends `focus`; neither touches
/// any other window.
#[tokio::test]
async fn test_close_and_focus_send_one_command_for_the_container() {
    let fake = FakeSway::start(base_script().reply(MessageType::GetTree, TREE_FLOATING));
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    driver.close_window("5").await.unwrap();
    driver.focus_window("5").await.unwrap();
    assert_eq!(fake.commands(), ["[con_id=5] kill", "[con_id=5] focus"]);
}

/// A target that names no window is `TargetNotFound`, and no command is sent.
#[tokio::test]
async fn test_unknown_target_sends_nothing() {
    let fake = FakeSway::start(base_script().reply(MessageType::GetTree, TREE_FLOATING));
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let err = driver.close_window("999").await.unwrap_err();
    assert!(matches!(err, DriverError::TargetNotFound(_)), "{err}");
    assert!(fake.commands().is_empty());
}

/// A runtime transform moves and resizes the window found by class: the window of the fixture
/// tree (class `dev.spawnat.swaycap`) to the centre of the work area at 400x300.
#[tokio::test]
async fn test_transform_by_class() {
    let fake = FakeSway::start(
        base_script()
            // 1st read: the window list that resolves the class; 2nd: the check after the command
            .reply(MessageType::GetTree, TREE_FLOATING)
            .reply(
                MessageType::GetTree,
                &tree_with(5, (760, 390, 400, 300), (400, 300)),
            ),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let p = params(Some((400, 300)), Some(Anchor::Center), 0);
    driver
        .transform_window("dev.spawnat.swaycap", p, 800, 500)
        .await
        .unwrap();
    assert_eq!(
        fake.commands(),
        ["[con_id=5] floating enable, resize set 400 300, move absolute position 760 390"]
    );
}

/// Window ids win over pids and classes; a pid or class is resolved through the shared selector.
#[test]
fn test_resolve_window_id() {
    let tree = types::parse_tree(TREE_FLOATING).unwrap();
    let outputs = types::parse_outputs(OUTPUTS_ONE).unwrap();
    let windows = types::windows_from_tree(&tree, &outputs);
    assert_eq!(place::resolve_window_id(&windows, "5").unwrap(), 5);
    let pid = windows[0].pid.unwrap().to_string();
    assert_eq!(place::resolve_window_id(&windows, &pid).unwrap(), 5);
    assert_eq!(place::resolve_window_id(&windows, "swaycap").unwrap(), 5);
    assert!(place::resolve_window_id(&windows, "nothing-like-it").is_err());
}

/// What the driver declares: runtime transform and close, nothing else.
#[test]
fn test_declared_capabilities() {
    let fake = FakeSway::start(base_script());
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    assert_eq!(
        driver.capabilities(),
        crate::platform::capability::Capabilities {
            runtime_transform: true,
            claim_wait: false,
            close: true,
            reveal_control: false,
            needs_helper: false,
        }
    );
    assert!(driver.supports_runtime_transform() && driver.supports_close());
    assert!(!driver.supports_claim_wait());
}

/// What Sway cannot do is reported as unsupported, not pretended: maximize/minimize, defocus
/// and the pointer position.
#[tokio::test]
async fn test_unsupported_operations_say_so() {
    let fake = FakeSway::start(base_script());
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    use crate::platform::WindowState;
    for state in [
        WindowState::Maximize,
        WindowState::Minimize,
        WindowState::Restore,
    ] {
        assert!(matches!(
            driver.set_window_state("5", state).await,
            Err(DriverError::UnsupportedCapability(_))
        ));
    }
    assert!(matches!(
        driver.defocus_window("5", "desktop", "").await,
        Err(DriverError::UnsupportedCapability(_))
    ));
    assert!(matches!(
        driver.get_cursor_position().await,
        Err(DriverError::UnsupportedCapability(_))
    ));
    assert!(fake.commands().is_empty());
}

/// A window with a minimum size is raised (`--size 30 20` came out 75x50 on the private Sway):
/// the position is computed again for the size it really has, so bottom-right with margin 16
/// puts a 75x50 window at (1920-16-75, 1080-16-50) = (1829, 1014), inside the area, the same
/// position `verify_and_report_placement` expects. No second `resize`: there is no border to
/// subtract, and the size would be refused again.
#[tokio::test]
async fn test_a_raised_window_is_anchored_with_its_real_size() {
    let fake = FakeSway::start(
        base_script()
            .reply(MessageType::GetTree, TREE_EMPTY)
            .reply(
                MessageType::GetTree,
                &tree_with(9, (1874, 1044, 75, 50), (75, 50)),
            )
            .event(Duration::from_millis(10), &event("new", 9, 4242, "demo")),
    );
    let driver = SwayDriver::connect(fake.path.clone()).unwrap();
    let batch = batch_with(
        params(Some((30, 20)), Some(Anchor::BottomRight), 16),
        "demo",
        FocusIntent::Leave,
    );
    driver.arm(batch.clone()).await.unwrap();
    driver.post_spawn(4242, &batch).await.unwrap();
    assert_eq!(
        fake.commands(),
        [
            "[con_id=9] floating enable, resize set 30 20, move absolute position 1874 1044",
            "[con_id=9] move absolute position 1829 1014"
        ]
    );
}
