//! # Internal GNOME Compositor Mechanics
//!
//! Internal instruction synthesis and low-level Clutter/Mutter payload generation
//! for the GNOME Wayland Shell extension.
//!
//! Turns the neutral placement types of `spawn-at-core` into the list of instructions the
//! extension runs (`ArmSpawn` and `ExecuteBatch` take it as JSON). Pure: no D-Bus, no I/O.
//! The driver (`driver.rs`) is the caller. The JSON shape is fixed by serde (a unit variant is
//! a string such as `"Cloak"`, a variant with data is a one-key object such as
//! `{"SetSize":{"w":500,"h":400}}`) and is read by `validateInstructions` in
//! `assets/gnome/*/validator.js`, which rejects names and ranges it does not know. Renaming a
//! variant or field here therefore breaks the extension, not the Rust build.

use serde::{Deserialize, Serialize};
use spawn_at_core::driver::Entry;
use spawn_at_core::geometry::{Anchor, Area, Pivot, PlacementParams};
use spawn_at_core::wait_override::{WaitOverride, WaitStrategy};

/// The argument object of the `SetPositionAnchored` instruction. All lengths are pixels.
///
/// `screen_anchor_x/y` is the point on the screen the window's pivot is placed on;
/// `pivot_u/v` says which point of the window that is, as a fraction of its size (0.0 is the
/// left or top edge, 1.0 the right or bottom edge, 0.5 the middle); `offset_x/y` is added
/// on top. `area` is `"workarea"` or `"screen"`, or absent. The extension's validator keeps only
/// the fields it names, and `intended_w` and `intended_h` are not among them (no use of them
/// in `assets/gnome/modern/*.js`; `grep -n intended_w assets/gnome/modern/*.js`), so they
/// are sent but dropped; the window size comes from the `SetSize` instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementPayload {
    pub intended_w: u32,
    pub intended_h: u32,
    pub screen_anchor_x: i32,
    pub screen_anchor_y: i32,
    pub pivot_u: f64,
    pub pivot_v: f64,
    pub offset_x: i32,
    pub offset_y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(default)]
    pub margin_top: i32,
    #[serde(default)]
    pub margin_bottom: i32,
    #[serde(default)]
    pub margin_left: i32,
    #[serde(default)]
    pub margin_right: i32,
    #[serde(default = "default_true")]
    pub clamp: bool,
}

fn default_true() -> bool {
    true
}

/// One step of the list sent to the extension. The names are the allow-list of the extension's
/// validator (`ALLOWED_OPS`); the extension refuses a list longer than 64 steps, empty, or
/// with a step it does not know, and then runs none of it.
///
/// `Snapshot` and `DestroySnapshot` create and remove a snapshot of the window in the
/// extension's cloak manager (`transform_window` uses them; the cold-start list from
/// `build_instructions_for_entry` does not). The extension skips the snapshot for a minimised,
/// covered or hidden window. `Cloak` and `Uncloak` hide and reveal the window; `SetSize` is in
/// pixels; `WaitForCommit` waits for a commit, at most `timeout_ms` milliseconds (the validator
/// accepts 0 to 60000). Beyond these names, the steps are not described here (not verified).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    Snapshot,
    Cloak,
    SetSize {
        w: u32,
        h: u32,
    },
    WaitForCommit {
        timeout_ms: u32,
        /// Developer override of the wait rule (`legacy` or `ready-wait`); left out of the JSON
        /// when not set, so an unchanged spawn sends the bytes it always sent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        strategy: Option<WaitStrategy>,
        /// Developer override of the ready-wait cap in milliseconds; left out when not set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cap_ms: Option<u32>,
    },
    SetPositionAnchored(PlacementPayload),
    Uncloak,
    DestroySnapshot,
}

impl Instruction {
    /// A `WaitForCommit` with `timeout_ms` (milliseconds) and the developer override `wait`. The
    /// `strategy` field is set only when `wait.strategy` is not `Auto`, and `cap_ms` only when
    /// the override has one; unset fields are left out of the JSON.
    pub fn wait_for_commit(timeout_ms: u32, wait: &WaitOverride) -> Self {
        Instruction::WaitForCommit {
            timeout_ms,
            strategy: match wait.strategy {
                WaitStrategy::Auto => None,
                other => Some(other),
            },
            cap_ms: wait.cap_ms,
        }
    }
}

/// Turns `params` into the argument of `SetPositionAnchored`. Pure. `current_w` and `current_h`
/// (pixels) fill `intended_w/h` when `params.size` is unset.
///
/// How the point is chosen, in order: with an anchor, `compute_screen_anchor` on the work area
/// with the four margins; else with `pos` (or, if that is unset, `offset`), the work-area
/// origin plus that point (the origin is `(0, 0)` for `Area::Unbounded`); else the work-area
/// origin plus the left and top margins. Each side's margin is its own value if set, else
/// `params.margin`. `offset_x/y` is passed on only when an anchor is set. The pivot is the
/// explicit `pivot` if set, else the one that matches the anchor (0.0 to 1.0; `Cursor` gives
/// the top-left), else the top-left. `Area::Unbounded` sends no `area`, zero margins and
/// `clamp` false; otherwise `clamp` is copied.
// The callers in this folder (`transform_window`, `build_instructions_for_entry`) call it only
// when `anchor` or `pos` is set, so the `offset` fallback is reached only by direct calls
// (grep -rn calculate_placement src/).
pub fn calculate_placement(
    params: &PlacementParams,
    current_w: u32,
    current_h: u32,
) -> PlacementPayload {
    let intended_w = params.size.map(|s| s.0).unwrap_or(current_w);
    let intended_h = params.size.map(|s| s.1).unwrap_or(current_h);

    let wa = params.workarea;
    let margin_top = params.margin_top.unwrap_or(params.margin);
    let margin_bottom = params.margin_bottom.unwrap_or(params.margin);
    let margin_left = params.margin_left.unwrap_or(params.margin);
    let margin_right = params.margin_right.unwrap_or(params.margin);

    let offset_x = if params.anchor.is_some() {
        params.offset.map(|o| o.0).unwrap_or(0)
    } else {
        0
    };
    let offset_y = if params.anchor.is_some() {
        params.offset.map(|o| o.1).unwrap_or(0)
    } else {
        0
    };

    let (pivot_u, pivot_v) = if let Some(pivot) = params.pivot {
        match pivot {
            Pivot::TopLeft => (0.0, 0.0),
            Pivot::TopRight => (1.0, 0.0),
            Pivot::BottomLeft => (0.0, 1.0),
            Pivot::BottomRight => (1.0, 1.0),
            Pivot::Center => (0.5, 0.5),
        }
    } else if let Some(anchor) = params.anchor {
        match anchor {
            Anchor::Center => (0.5, 0.5),
            Anchor::TopLeft => (0.0, 0.0),
            Anchor::TopRight => (1.0, 0.0),
            Anchor::BottomLeft => (0.0, 1.0),
            Anchor::BottomRight => (1.0, 1.0),
            Anchor::Top => (0.5, 0.0),
            Anchor::Bottom => (0.5, 1.0),
            Anchor::Left => (0.0, 0.5),
            Anchor::Right => (1.0, 0.5),
            Anchor::Cursor => (0.0, 0.0),
        }
    } else {
        (0.0, 0.0)
    };

    let is_unbounded = params.area == Some(Area::Unbounded);

    let (screen_anchor_x, screen_anchor_y) = if let Some(anchor) = params.anchor {
        spawn_at_core::geometry::compute_screen_anchor(
            wa,
            anchor,
            margin_top,
            margin_bottom,
            margin_left,
            margin_right,
            params.cursor_pos,
        )
    } else if let Some(pos) = params.pos.or(params.offset) {
        let (ref_x, ref_y) = if is_unbounded { (0, 0) } else { (wa.x, wa.y) };
        (ref_x + pos.0, ref_y + pos.1)
    } else {
        (wa.x + margin_left, wa.y + margin_top)
    };

    PlacementPayload {
        intended_w,
        intended_h,
        screen_anchor_x,
        screen_anchor_y,
        pivot_u,
        pivot_v,
        offset_x,
        offset_y,
        area: match params.area {
            Some(Area::Workarea) => Some("workarea".to_string()),
            Some(Area::Screen) => Some("screen".to_string()),
            Some(Area::Unbounded) | None => None,
        },
        margin_top: if is_unbounded { 0 } else { margin_top },
        margin_bottom: if is_unbounded { 0 } else { margin_bottom },
        margin_left: if is_unbounded { 0 } else { margin_left },
        margin_right: if is_unbounded { 0 } else { margin_right },
        clamp: if is_unbounded { false } else { params.clamp },
    }
}

/// The instruction list armed for a window that does not exist yet: `Cloak`; if a size is
/// requested, `SetSize` and `WaitForCommit` with a 300 ms timeout (plus the entry's wait
/// override); if an anchor or `pos` is requested, `SetPositionAnchored` computed with a current
/// size of 0 by 0; then `Uncloak`. There is no `Snapshot`: the window has nothing to snapshot.
// Compare `transform_window` in driver.rs, which starts with `Snapshot` and waits 500 ms.
pub fn build_instructions_for_entry(entry: &Entry) -> Vec<Instruction> {
    let has_move = entry.placement.anchor.is_some() || entry.placement.pos.is_some();
    let mut instructions = vec![Instruction::Cloak];

    if let Some(size) = entry.placement.size {
        instructions.push(Instruction::SetSize {
            w: size.0,
            h: size.1,
        });
        instructions.push(Instruction::wait_for_commit(300, &entry.wait));
    }

    if has_move {
        let payload = calculate_placement(&entry.placement, 0, 0);
        instructions.push(Instruction::SetPositionAnchored(payload));
    }

    instructions.push(Instruction::Uncloak);

    instructions
}

#[cfg(test)]
mod tests {
    use super::*;
    use spawn_at_core::geometry::{Anchor, Area, Pivot, Rect};

    #[test]
    fn test_golden_instruction_batch_resize_only() {
        let entry = Entry {
            key: "test-resize-only".to_string(),
            app_hint: "org.gnome.Calculator".to_string(),
            wait: Default::default(),
            placement: PlacementParams {
                size: Some((500, 400)),
                ..Default::default()
            },
        };

        let instructions = build_instructions_for_entry(&entry);
        assert_eq!(instructions.len(), 4);

        let json = serde_json::to_string(&instructions).unwrap();
        assert_eq!(
            json,
            r#"["Cloak",{"SetSize":{"w":500,"h":400}},{"WaitForCommit":{"timeout_ms":300}},"Uncloak"]"#
        );
    }

    #[test]
    fn test_golden_instruction_batch_pos_only() {
        let entry = Entry {
            key: "test-pos-only".to_string(),
            app_hint: "org.gnome.Calculator".to_string(),
            wait: Default::default(),
            placement: PlacementParams {
                pos: Some((100, 200)),
                area: Some(Area::Workarea),
                workarea: Rect {
                    x: 0,
                    y: 40,
                    width: 1920,
                    height: 1040,
                },
                margin: 16,
                clamp: true,
                ..Default::default()
            },
        };

        let instructions = build_instructions_for_entry(&entry);
        assert_eq!(instructions.len(), 3);

        let json = serde_json::to_string(&instructions).unwrap();
        let deserialized: Vec<Instruction> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 3);

        if let Instruction::SetPositionAnchored(ref payload) = deserialized[1] {
            assert_eq!(payload.screen_anchor_x, 100);
            assert_eq!(payload.screen_anchor_y, 240);
            assert_eq!(payload.pivot_u, 0.0);
            assert_eq!(payload.pivot_v, 0.0);
            assert_eq!(payload.offset_x, 0);
            assert_eq!(payload.offset_y, 0);
            assert!(payload.clamp);
        } else {
            panic!("Expected SetPositionAnchored at index 1");
        }
    }

    #[test]
    fn test_golden_instruction_batch_anchor_only() {
        let entry = Entry {
            key: "test-anchor-only".to_string(),
            app_hint: "org.gnome.Calculator".to_string(),
            wait: Default::default(),
            placement: PlacementParams {
                anchor: Some(Anchor::Center),
                area: Some(Area::Workarea),
                workarea: Rect {
                    x: 0,
                    y: 40,
                    width: 1920,
                    height: 1040,
                },
                margin: 16,
                clamp: true,
                ..Default::default()
            },
        };

        let instructions = build_instructions_for_entry(&entry);
        assert_eq!(instructions.len(), 3);

        let json = serde_json::to_string(&instructions).unwrap();
        let deserialized: Vec<Instruction> = serde_json::from_str(&json).unwrap();
        if let Instruction::SetPositionAnchored(ref payload) = deserialized[1] {
            assert_eq!(payload.screen_anchor_x, 1920 / 2);
            assert_eq!(payload.screen_anchor_y, 40 + 1040 / 2);
            assert_eq!(payload.pivot_u, 0.5);
            assert_eq!(payload.pivot_v, 0.5);
            assert_eq!(payload.offset_x, 0);
            assert_eq!(payload.offset_y, 0);
            assert!(payload.clamp);
        } else {
            panic!("Expected SetPositionAnchored at index 1");
        }
    }

    #[test]
    fn test_golden_instruction_batch_anchor_and_offset() {
        let entry = Entry {
            key: "test-anchor-offset".to_string(),
            app_hint: "org.gnome.Calculator".to_string(),
            wait: Default::default(),
            placement: PlacementParams {
                anchor: Some(Anchor::TopRight),
                offset: Some((15, -10)),
                area: Some(Area::Workarea),
                workarea: Rect {
                    x: 0,
                    y: 40,
                    width: 1920,
                    height: 1040,
                },
                margin: 16,
                clamp: true,
                ..Default::default()
            },
        };

        let instructions = build_instructions_for_entry(&entry);
        assert_eq!(instructions.len(), 3);

        let json = serde_json::to_string(&instructions).unwrap();
        let deserialized: Vec<Instruction> = serde_json::from_str(&json).unwrap();
        if let Instruction::SetPositionAnchored(ref payload) = deserialized[1] {
            assert_eq!(payload.screen_anchor_x, 1920 - 16);
            assert_eq!(payload.screen_anchor_y, 40 + 16);
            assert_eq!(payload.pivot_u, 1.0);
            assert_eq!(payload.pivot_v, 0.0);
            assert_eq!(payload.offset_x, 15);
            assert_eq!(payload.offset_y, -10);
            assert!(payload.clamp);
        } else {
            panic!("Expected SetPositionAnchored at index 1");
        }
    }

    #[test]
    fn test_golden_instruction_batch_cursor_no_pivot() {
        let entry = Entry {
            key: "test-cursor-no-pivot".to_string(),
            app_hint: "org.gnome.Calculator".to_string(),
            wait: Default::default(),
            placement: PlacementParams {
                anchor: Some(Anchor::Cursor),
                cursor_pos: Some((500, 300)),
                area: Some(Area::Workarea),
                workarea: Rect {
                    x: 0,
                    y: 40,
                    width: 1920,
                    height: 1040,
                },
                margin: 16,
                clamp: true,
                ..Default::default()
            },
        };

        let instructions = build_instructions_for_entry(&entry);
        assert_eq!(instructions.len(), 3);

        let json = serde_json::to_string(&instructions).unwrap();
        let deserialized: Vec<Instruction> = serde_json::from_str(&json).unwrap();
        if let Instruction::SetPositionAnchored(ref payload) = deserialized[1] {
            assert_eq!(payload.screen_anchor_x, 500);
            assert_eq!(payload.screen_anchor_y, 300);
            // Must default to top-left (0.0, 0.0)
            assert_eq!(payload.pivot_u, 0.0);
            assert_eq!(payload.pivot_v, 0.0);
            assert_eq!(payload.offset_x, 0);
            assert_eq!(payload.offset_y, 0);
            assert!(payload.clamp);
        } else {
            panic!("Expected SetPositionAnchored at index 1");
        }
    }

    #[test]
    fn test_golden_build_instructions_for_entry() {
        let entry = Entry {
            key: "test-token-123".to_string(),
            app_hint: "org.gnome.TextEditor".to_string(),
            wait: Default::default(),
            placement: PlacementParams {
                size: Some((800, 600)),
                anchor: Some(Anchor::TopRight),
                pivot: Some(Pivot::TopRight),
                margin: 16,
                margin_top: Some(20),
                margin_right: Some(25),
                offset: Some((10, -5)),
                area: Some(Area::Workarea),
                workarea: Rect {
                    x: 0,
                    y: 40,
                    width: 1920,
                    height: 1040,
                },
                clamp: true,
                ..Default::default()
            },
        };

        let instructions = build_instructions_for_entry(&entry);
        assert_eq!(instructions.len(), 5);

        let json = serde_json::to_string(&instructions).unwrap();
        // Assert exact serialization contract
        assert!(json.starts_with(r#"["Cloak",{"SetSize":{"w":800,"h":600}},{"WaitForCommit":{"timeout_ms":300}},{"SetPositionAnchored":"#));
        assert!(json.ends_with(r#"},"Uncloak"]"#));

        // Verify deserialization back to Vec<Instruction>
        let deserialized: Vec<Instruction> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 5);

        // Verify payload values inside SetPositionAnchored
        if let Instruction::SetPositionAnchored(ref payload) = deserialized[3] {
            assert_eq!(payload.intended_w, 800);
            assert_eq!(payload.intended_h, 600);
            assert_eq!(payload.screen_anchor_x, 1920 - 25); // 1895
            assert_eq!(payload.screen_anchor_y, 40 + 20); // 60
            assert_eq!(payload.pivot_u, 1.0);
            assert_eq!(payload.pivot_v, 0.0);
            assert_eq!(payload.offset_x, 10);
            assert_eq!(payload.offset_y, -5);
            assert_eq!(payload.area.as_deref(), Some("workarea"));
            assert_eq!(payload.margin_top, 20);
            assert_eq!(payload.margin_right, 25);
            assert!(payload.clamp);
        } else {
            panic!("Expected SetPositionAnchored at index 3");
        }
    }

    #[test]
    fn test_golden_transform_batch_instructions() {
        let payload = PlacementPayload {
            intended_w: 500,
            intended_h: 400,
            screen_anchor_x: 960,
            screen_anchor_y: 540,
            pivot_u: 0.5,
            pivot_v: 0.5,
            offset_x: 0,
            offset_y: 0,
            area: Some("screen".to_string()),
            margin_top: 16,
            margin_bottom: 16,
            margin_left: 16,
            margin_right: 16,
            clamp: true,
        };

        let instructions = vec![
            Instruction::Snapshot,
            Instruction::Cloak,
            Instruction::SetSize { w: 500, h: 400 },
            Instruction::wait_for_commit(500, &WaitOverride::default()),
            Instruction::SetPositionAnchored(payload),
            Instruction::Uncloak,
            Instruction::DestroySnapshot,
        ];

        let json = serde_json::to_string(&instructions).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 7);

        assert_eq!(arr[0], "Snapshot");
        assert_eq!(arr[1], "Cloak");
        assert_eq!(arr[2]["SetSize"]["w"], 500);
        assert_eq!(arr[2]["SetSize"]["h"], 400);
        assert_eq!(arr[3]["WaitForCommit"]["timeout_ms"], 500);
        assert_eq!(arr[4]["SetPositionAnchored"]["intended_w"], 500);
        assert_eq!(arr[4]["SetPositionAnchored"]["area"], "screen");
        assert_eq!(arr[5], "Uncloak");
        assert_eq!(arr[6], "DestroySnapshot");
    }

    #[test]
    fn test_golden_transform_batch_pos_only_instructions() {
        let payload = PlacementPayload {
            intended_w: 500,
            intended_h: 400,
            screen_anchor_x: 960,
            screen_anchor_y: 540,
            pivot_u: 0.5,
            pivot_v: 0.5,
            offset_x: 0,
            offset_y: 0,
            area: Some("screen".to_string()),
            margin_top: 16,
            margin_bottom: 16,
            margin_left: 16,
            margin_right: 16,
            clamp: true,
        };

        // Position-only transform skips SetSize and WaitForCommit
        let instructions = vec![
            Instruction::Snapshot,
            Instruction::Cloak,
            Instruction::SetPositionAnchored(payload),
            Instruction::Uncloak,
            Instruction::DestroySnapshot,
        ];

        let json = serde_json::to_string(&instructions).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 5);

        assert_eq!(arr[0], "Snapshot");
        assert_eq!(arr[1], "Cloak");
        assert_eq!(arr[2]["SetPositionAnchored"]["intended_w"], 500);
        assert_eq!(arr[3], "Uncloak");
        assert_eq!(arr[4], "DestroySnapshot");
    }

    #[test]
    fn test_golden_transform_batch_unbounded_instructions() {
        let params = PlacementParams {
            pos: Some((-50, 100)),
            area: Some(Area::Unbounded),
            clamp: false,
            ..Default::default()
        };
        let payload = calculate_placement(&params, 367, 514);
        assert_eq!(payload.screen_anchor_x, -50);
        assert_eq!(payload.screen_anchor_y, 100);
        assert_eq!(payload.area, None);
        assert!(!payload.clamp);

        let instructions = vec![
            Instruction::Snapshot,
            Instruction::Cloak,
            Instruction::SetPositionAnchored(payload),
            Instruction::Uncloak,
            Instruction::DestroySnapshot,
        ];

        let json = serde_json::to_string(&instructions).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 5);

        assert_eq!(arr[0], "Snapshot");
        assert_eq!(arr[1], "Cloak");
        assert_eq!(arr[2]["SetPositionAnchored"]["intended_w"], 367);
        assert_eq!(arr[2]["SetPositionAnchored"]["intended_h"], 514);
        assert_eq!(arr[2]["SetPositionAnchored"]["screen_anchor_x"], -50);
        assert_eq!(arr[2]["SetPositionAnchored"]["screen_anchor_y"], 100);
        assert!(arr[2]["SetPositionAnchored"].get("area").is_none());
        assert_eq!(arr[2]["SetPositionAnchored"]["clamp"], false);
        assert_eq!(arr[3], "Uncloak");
        assert_eq!(arr[4], "DestroySnapshot");
    }

    fn resize_entry(wait: WaitOverride) -> Entry {
        Entry {
            key: "test-wait".to_string(),
            app_hint: "org.gnome.Calculator".to_string(),
            placement: PlacementParams {
                size: Some((500, 400)),
                ..Default::default()
            },
            wait,
        }
    }

    #[test]
    fn test_default_wait_override_leaves_the_instruction_bytes_unchanged() {
        // The exact string a build without the override sent: no strategy, no cap_ms, key order
        // timeout_ms only.
        let json = serde_json::to_string(&build_instructions_for_entry(&resize_entry(
            Default::default(),
        )))
        .unwrap();
        assert_eq!(
            json,
            r#"["Cloak",{"SetSize":{"w":500,"h":400}},{"WaitForCommit":{"timeout_ms":300}},"Uncloak"]"#
        );
        // `auto` with no cap is the default too.
        let (auto, _) = WaitOverride::from_raw(Some("auto"), None);
        assert_eq!(
            serde_json::to_string(&build_instructions_for_entry(&resize_entry(auto))).unwrap(),
            json
        );
    }

    #[test]
    fn test_wait_override_is_carried_in_the_wait_instruction_only() {
        let (wait, problems) = WaitOverride::from_raw(Some("legacy"), Some("200"));
        assert!(problems.is_empty());
        let json =
            serde_json::to_string(&build_instructions_for_entry(&resize_entry(wait))).unwrap();
        assert_eq!(
            json,
            r#"["Cloak",{"SetSize":{"w":500,"h":400}},{"WaitForCommit":{"timeout_ms":300,"strategy":"legacy","cap_ms":200}},"Uncloak"]"#
        );
    }

    #[test]
    fn test_wait_override_strategy_and_cap_are_independent() {
        let (only_cap, _) = WaitOverride::from_raw(None, Some("90"));
        let json = serde_json::to_string(&Instruction::wait_for_commit(300, &only_cap)).unwrap();
        assert_eq!(json, r#"{"WaitForCommit":{"timeout_ms":300,"cap_ms":90}}"#);
        let (only_strategy, _) = WaitOverride::from_raw(Some("ready-wait"), None);
        let json =
            serde_json::to_string(&Instruction::wait_for_commit(300, &only_strategy)).unwrap();
        assert_eq!(
            json,
            r#"{"WaitForCommit":{"timeout_ms":300,"strategy":"ready-wait"}}"#
        );
    }

    #[test]
    fn test_wait_instruction_without_override_fields_still_deserialises() {
        let parsed: Instruction =
            serde_json::from_str(r#"{"WaitForCommit":{"timeout_ms":300}}"#).unwrap();
        match parsed {
            Instruction::WaitForCommit {
                timeout_ms,
                strategy,
                cap_ms,
            } => {
                assert_eq!(timeout_ms, 300);
                assert_eq!(strategy, None);
                assert_eq!(cap_ms, None);
            }
            other => panic!("expected WaitForCommit, got {other:?}"),
        }
    }
}
