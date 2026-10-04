import re

with open("src/core/geometry.rs", "r") as f:
    content = f.read()

# Add imports
content = content.replace("use serde::{Deserialize, Serialize};", "use serde::{Deserialize, Serialize};\nuse crate::core::types::PlacementPayload;")

# Add calculate_placement
calculate_placement_code = """
#[derive(Debug, Clone, Default)]
pub struct PlacementParams {
    pub pos: Option<(i32, i32)>,
    pub offset: Option<(i32, i32)>,
    pub anchor: Option<Anchor>,
    pub pivot: Pivot,
    pub size: Option<(u32, u32)>,
    pub margin: i32,
    pub cursor_pos: Option<(i32, i32)>,
    pub workarea: Rect,
}

pub fn calculate_placement(params: PlacementParams, current_w: u32, current_h: u32) -> PlacementPayload {
    let intended_w = params.size.map(|s| s.0).unwrap_or(current_w);
    let intended_h = params.size.map(|s| s.1).unwrap_or(current_h);

    let mut pivot_u = 0.0;
    let mut pivot_v = 0.0;
    let mut screen_anchor_x = 0;
    let mut screen_anchor_y = 0;
    let offset_x = params.offset.map(|o| o.0).unwrap_or(0);
    let offset_y = params.offset.map(|o| o.1).unwrap_or(0);

    let wa = params.workarea;
    let margin = params.margin;

    if let Some(anchor) = params.anchor {
        match anchor {
            Anchor::Center => {
                screen_anchor_x = wa.x + (wa.width as i32) / 2;
                screen_anchor_y = wa.y + (wa.height as i32) / 2;
                pivot_u = 0.5;
                pivot_v = 0.5;
            }
            Anchor::TopLeft => {
                screen_anchor_x = wa.x + margin;
                screen_anchor_y = wa.y + margin;
                pivot_u = 0.0;
                pivot_v = 0.0;
            }
            Anchor::TopRight => {
                screen_anchor_x = wa.x + (wa.width as i32) - margin;
                screen_anchor_y = wa.y + margin;
                pivot_u = 1.0;
                pivot_v = 0.0;
            }
            Anchor::BottomLeft => {
                screen_anchor_x = wa.x + margin;
                screen_anchor_y = wa.y + (wa.height as i32) - margin;
                pivot_u = 0.0;
                pivot_v = 1.0;
            }
            Anchor::BottomRight => {
                screen_anchor_x = wa.x + (wa.width as i32) - margin;
                screen_anchor_y = wa.y + (wa.height as i32) - margin;
                pivot_u = 1.0;
                pivot_v = 1.0;
            }
        }
    } else {
        match params.pivot {
            Pivot::TopLeft => { pivot_u = 0.0; pivot_v = 0.0; }
            Pivot::TopRight => { pivot_u = 1.0; pivot_v = 0.0; }
            Pivot::BottomLeft => { pivot_u = 0.0; pivot_v = 1.0; }
            Pivot::BottomRight => { pivot_u = 1.0; pivot_v = 1.0; }
            Pivot::Center => { pivot_u = 0.5; pivot_v = 0.5; }
        }

        if let Some(pos) = params.pos {
            screen_anchor_x = pos.0;
            screen_anchor_y = pos.1;
        } else if let Some(cursor) = params.cursor_pos {
            screen_anchor_x = cursor.0;
            screen_anchor_y = cursor.1;
        } else {
            screen_anchor_x = wa.x + margin;
            screen_anchor_y = wa.y + margin;
        }
    }

    PlacementPayload {
        intended_w,
        intended_h,
        screen_anchor_x,
        screen_anchor_y,
        pivot_u,
        pivot_v,
        offset_x,
        offset_y,
    }
}
"""

with open("src/core/geometry.rs", "w") as f:
    f.write(content + "\n" + calculate_placement_code)

