use serde::{Deserialize, Serialize};

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
}

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
