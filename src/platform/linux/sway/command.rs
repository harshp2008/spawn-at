//! The text of the commands sent with `RUN_COMMAND`. Pure: every number is an integer, and the
//! container id is a number, so nothing the user typed can reach the command line of Sway.
//!
//! A leading `[con_id=N]` selects the container; Sway keeps the criteria across the commas of
//! one command string (`man 5 sway`, "COMMAND CONVENTIONS"), so one `RUN_COMMAND` can change
//! several things on the same window.

/// Makes the window floating, optionally resizes it, and moves it so its top-left corner is at
/// (`x`, `y`) in the global layout (`move absolute position`; `move position` would be relative
/// to the usable area of the workspace's output). The size comes first so the position is the
/// last thing applied.
///
/// `size` is the *content* size: `resize set` sets the part inside the border and title bar.
pub fn place(id: u64, size: Option<(u32, u32)>, x: i32, y: i32) -> String {
    match size {
        Some((w, h)) => format!(
            "[con_id={id}] floating enable, resize set {} {}, move absolute position {x} {y}",
            w.max(1),
            h.max(1)
        ),
        None => format!("[con_id={id}] floating enable, move absolute position {x} {y}"),
    }
}

/// Only makes the window floating; it then has the size its client asked for.
pub fn floating_enable(id: u64) -> String {
    format!("[con_id={id}] floating enable")
}

/// Sets the content size and moves the window (used to correct a size that came out larger
/// because of the border and title bar).
pub fn resize_and_move(id: u64, size: (u32, u32), x: i32, y: i32) -> String {
    format!(
        "[con_id={id}] resize set {} {}, move absolute position {x} {y}",
        size.0.max(1),
        size.1.max(1)
    )
}

/// Moves a floating window to (`x`, `y`) in the global layout.
pub fn move_to(id: u64, x: i32, y: i32) -> String {
    format!("[con_id={id}] move absolute position {x} {y}")
}

pub fn focus(id: u64) -> String {
    format!("[con_id={id}] focus")
}

/// Asks the client to close (an `xdg_toplevel` close request, or `WM_DELETE_WINDOW` for an
/// XWayland window), as `kill` does for a container.
pub fn kill(id: u64) -> String {
    format!("[con_id={id}] kill")
}

/// The content size to ask for so that the whole container (border and title bar included) ends
/// up `frame` large, given the container's current `rect` size and its current `window_rect`
/// size. At least 1x1.
pub fn content_size_for_frame(
    frame: (u32, u32),
    current_rect: (u32, u32),
    current_content: (u32, u32),
) -> (u32, u32) {
    let deco_w = current_rect.0.saturating_sub(current_content.0);
    let deco_h = current_rect.1.saturating_sub(current_content.1);
    (
        frame.0.saturating_sub(deco_w).max(1),
        frame.1.saturating_sub(deco_h).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact string the live runs sent (it is what produced `rect` 800x500 at 100,150 on
    /// the private Sway, see `tests/fixtures/sway/tree_floating.json`).
    #[test]
    fn test_place_with_a_size() {
        assert_eq!(
            place(5, Some((800, 500)), 100, 150),
            "[con_id=5] floating enable, resize set 800 500, move absolute position 100 150"
        );
    }

    /// Without a size the window is not resized: no `resize` word at all.
    #[test]
    fn test_place_without_a_size_does_not_resize() {
        let cmd = place(12, None, 10, 20);
        assert_eq!(
            cmd,
            "[con_id=12] floating enable, move absolute position 10 20"
        );
        assert!(!cmd.contains("resize"));
    }

    /// Negative coordinates (a second output to the left, or a window partly off screen) are
    /// printed with their sign; a size of 0 is raised to 1 because `resize set 0 0` is refused.
    #[test]
    fn test_negative_coordinates_and_zero_sizes() {
        assert_eq!(
            place(3, Some((0, 0)), -1920, -40),
            "[con_id=3] floating enable, resize set 1 1, move absolute position -1920 -40"
        );
    }

    /// The other builders name the container by id and nothing else.
    #[test]
    fn test_small_commands() {
        assert_eq!(floating_enable(7), "[con_id=7] floating enable");
        assert_eq!(move_to(7, -5, 9), "[con_id=7] move absolute position -5 9");
        assert_eq!(focus(7), "[con_id=7] focus");
        assert_eq!(kill(7), "[con_id=7] kill");
        assert_eq!(
            resize_and_move(7, (790, 470), 1, 2),
            "[con_id=7] resize set 790 470, move absolute position 1 2"
        );
    }

    /// A container 804x524 with a 800x500 content has 4 px of border across and 24 px (border and
    /// title bar) down; to get a 800x500 container the content must be 796x476.
    #[test]
    fn test_content_size_for_frame_subtracts_the_decoration() {
        assert_eq!(
            content_size_for_frame((800, 500), (804, 524), (800, 500)),
            (796, 476)
        );
        // no decoration: unchanged
        assert_eq!(
            content_size_for_frame((800, 500), (800, 500), (800, 500)),
            (800, 500)
        );
        // a decoration larger than the frame never gives 0
        assert_eq!(content_size_for_frame((3, 3), (100, 100), (10, 10)), (1, 1));
    }
}
