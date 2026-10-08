//! # Internal GNOME Compositor Mechanics
//!
//! Internal instruction synthesis and low-level Clutter/Mutter payload generation
//! for the GNOME Wayland Shell extension.

use serde::{Deserialize, Serialize};
use spawn_at_core::geometry::{Anchor, Area, Pivot, PlacementParams};
use spawn_at_core::driver::Entry;

/// Raw serialization payload expected by the GNOME Shell extension for anchored positioning.
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
    #[serde(default)]
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

/// Low-level micro-instructions dispatched to the GNOME Shell extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    Snapshot,
    Cloak,
    SetSize { w: u32, h: u32 },
    WaitForCommit { timeout_ms: u32 },
    SetPositionAnchored(PlacementPayload),
    Uncloak,
    DestroySnapshot,
}

/// Translates declarative `PlacementParams` into a low-level `PlacementPayload`.
pub fn calculate_placement(params: &PlacementParams, current_w: u32, current_h: u32) -> PlacementPayload {
    let intended_w = params.size.map(|s| s.0).unwrap_or(current_w);
    let intended_h = params.size.map(|s| s.1).unwrap_or(current_h);

    let wa = params.workarea;
    let margin_top = params.margin_top.unwrap_or(params.margin);
    let margin_bottom = params.margin_bottom.unwrap_or(params.margin);
    let margin_left = params.margin_left.unwrap_or(params.margin);
    let margin_right = params.margin_right.unwrap_or(params.margin);

    let offset_x = params.offset.map(|o| o.0).unwrap_or(0);
    let offset_y = params.offset.map(|o| o.1).unwrap_or(0);

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

    let (screen_anchor_x, screen_anchor_y) = if let Some(anchor) = params.anchor {
        match anchor {
            Anchor::Center => (
                wa.x + (wa.width as i32) / 2,
                wa.y + (wa.height as i32) / 2,
            ),
            Anchor::TopLeft => (
                wa.x + margin_left,
                wa.y + margin_top,
            ),
            Anchor::TopRight => (
                wa.x + (wa.width as i32) - margin_right,
                wa.y + margin_top,
            ),
            Anchor::BottomLeft => (
                wa.x + margin_left,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            Anchor::BottomRight => (
                wa.x + (wa.width as i32) - margin_right,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            Anchor::Top => (
                wa.x + (wa.width as i32) / 2,
                wa.y + margin_top,
            ),
            Anchor::Bottom => (
                wa.x + (wa.width as i32) / 2,
                wa.y + (wa.height as i32) - margin_bottom,
            ),
            Anchor::Left => (
                wa.x + margin_left,
                wa.y + (wa.height as i32) / 2,
            ),
            Anchor::Right => (
                wa.x + (wa.width as i32) - margin_right,
                wa.y + (wa.height as i32) / 2,
            ),
            Anchor::Cursor => {
                if let Some(cursor) = params.cursor_pos {
                    (cursor.0, cursor.1)
                } else {
                    (wa.x + margin_left, wa.y + margin_top)
                }
            }
        }
    } else {
        if let Some(pos) = params.pos {
            (pos.0, pos.1)
        } else if let Some(cursor) = params.cursor_pos {
            (cursor.0, cursor.1)
        } else {
            (wa.x + margin_left, wa.y + margin_top)
        }
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
        area: params.area.map(|a| match a {
            Area::Workarea => "workarea".to_string(),
            Area::Screen => "screen".to_string(),
        }),
        margin_top,
        margin_bottom,
        margin_left,
        margin_right,
        clamp: params.clamp,
    }
}

/// Constructs the cold-start instruction pipeline for a given declarative entry.
pub fn build_instructions_for_entry(entry: &Entry) -> Vec<Instruction> {
    let payload = calculate_placement(&entry.placement, 0, 0);
    let mut instructions = vec![Instruction::Cloak];

    if let Some(size) = entry.placement.size {
        instructions.push(Instruction::SetSize { w: size.0, h: size.1 });
    }

    instructions.push(Instruction::WaitForCommit { timeout_ms: 300 });
    instructions.push(Instruction::SetPositionAnchored(payload));
    instructions.push(Instruction::Uncloak);

    instructions
}

#[cfg(test)]
mod tests {
    use super::*;
    use spawn_at_core::geometry::{Anchor, Area, Pivot, Rect};

    #[test]
    fn test_golden_build_instructions_for_entry() {
        let entry = Entry {
            key: "test-token-123".to_string(),
            app_hint: "org.gnome.TextEditor".to_string(),
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
            assert_eq!(payload.screen_anchor_y, 40 + 20);   // 60
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
            Instruction::WaitForCommit { timeout_ms: 500 },
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
}
