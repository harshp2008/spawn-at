import re

with open("src/commands/transform.rs", "r") as f:
    content = f.read()

content = content.replace("use crate::core::geometry::{self, Rect};", "use crate::core::geometry::{self, Rect, PlacementParams};\nuse crate::core::types::Instruction;")

transform_code = """    // 4. Calculate target position and size via geometry solver
    let target_w = args.size.as_ref().map(|s| s[0] as u32).unwrap_or(target_win.w as u32);
    let target_h = args.size.as_ref().map(|s| s[1] as u32).unwrap_or(target_win.h as u32);

    let pos = if let Some(p) = args.pos {
        Some((p[0], p[1]))
    } else {
        None
    };
    
    let params = PlacementParams {
        pos,
        offset: None,
        anchor: args.anchor,
        pivot: args.pivot,
        size: Some((target_w, target_h)),
        margin: args.margin,
        cursor_pos: Some((cursor_x, cursor_y)),
        workarea: target_workarea,
    };

    let payload = geometry::calculate_placement(params, target_win.w as u32, target_win.h as u32);

    // 5. Generate execution plan
    let instructions = vec![
        Instruction::Snapshot,
        Instruction::Cloak,
        Instruction::SetSize { w: payload.intended_w, h: payload.intended_h },
        Instruction::WaitForCommit { timeout_ms: 500 },
        Instruction::SetPositionAnchored(payload),
        Instruction::Uncloak,
        Instruction::DestroySnapshot,
    ];

    // 6. Execute batch via backend
    let target_id = target::resolve_target_id(target_win);

    backend
        .execute_batch(&target_id, &instructions)
        .await?;

    apply_focus_policy(backend, &target_id, &args.focus_modifiers).await?;

    Ok(())
}
"""

old_transform = re.search(r"    // 4\. Determine target size.*?Ok\(\(\)\)\n}", content, re.DOTALL)
if old_transform:
    content = content[:old_transform.start()] + transform_code

with open("src/commands/transform.rs", "w") as f:
    f.write(content)
