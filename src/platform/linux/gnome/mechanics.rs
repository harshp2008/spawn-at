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
