//! Hostile-app behaviours of the fixture, as pure decisions (harness only; never in the product).
//!
//! Each flag models an app that is slow, snaps to a grid, hangs or never settles. The window loop
//! (`window.rs`) asks this module what to do and carries it out, so the effect of every flag is
//! tested without a display: [`Hostility::plan_resize`] says how long to wait, which size to
//! paint and which fixture events (`SNAP`, `RESIZE_DELAY`) to write.
//!
//! What each flag can and cannot do on Wayland (HYPOTHESIS until a live run shows the commit
//! times; TEST_PLAN 6.22 does):
//! - `--resize-delay-ms`: the app sleeps before it paints at a new size. winit has already
//!   acknowledged the configure, so the compositor waits for a commit that comes N ms late.
//! - `--min-size` / `--max-size` / `--fixed-size`: sent to the compositor as xdg-toplevel hints
//!   (`set_min_size`, `set_max_size`; fixed is min == max). The compositor enforces them, so
//!   "refusing" is the compositor clamping the request, not the app ignoring a configure.
//! - `--resize-increment`: there is no Wayland protocol for it and winit ignores it there, so the
//!   fixture does what a terminal does: it paints a buffer rounded down to a multiple of the step.
//! - `--stall-after-map-ms`: the event loop sleeps, so configures go unanswered (a hung app).
//! - `--flap-ms`: the app commits buffers of two alternating sizes every [`FLAP_TICK_MS`] for T ms
//!   and then settles at the configured size; a client may commit any buffer size on Wayland.

/// A width and a height in pixels.
pub type Size = (u32, u32);

/// Colour (`0xAARRGGBB`) of a buffer at the initial size under `--state-colors`: loud magenta,
/// "painted before the resize".
pub const STATE_COLOR_INITIAL: u32 = 0xFFFF00FF;

/// Colour of a buffer at any other size under `--state-colors`: loud green, "painted at the new size".
pub const STATE_COLOR_RESIZED: u32 = 0xFF00FF00;

/// The solid colour a `--state-colors` fixture paints a buffer of `size` with, given the size the
/// window started at.
pub fn state_color(size: Size, initial: Size) -> u32 {
    if size == initial {
        STATE_COLOR_INITIAL
    } else {
        STATE_COLOR_RESIZED
    }
}

/// The GNOME "not responding" dialog appears after about 5 s; a stall must stay under this.
pub const STALL_LIMIT_MS: u64 = 4000;

/// Interval of the flap, shorter than the 30 ms quiet window of the ready-wait so it never settles.
pub const FLAP_TICK_MS: u64 = 20;

/// Reads `WxH` (two positive integers).
pub fn parse_size(text: &str) -> Result<Size, String> {
    let bad = || format!("expected WxH with two positive whole numbers (got '{text}')");
    let (w, h) = text.split_once(['x', 'X']).ok_or_else(bad)?;
    let (w, h): (u32, u32) = (w.parse().map_err(|_| bad())?, h.parse().map_err(|_| bad())?);
    if w == 0 || h == 0 {
        return Err(bad());
    }
    Ok((w, h))
}

/// Reads a stall in milliseconds and refuses one that would make GNOME call the app not responding.
pub fn parse_stall_ms(text: &str) -> Result<u64, String> {
    let ms: u64 = text
        .parse()
        .map_err(|_| format!("expected whole milliseconds (got '{text}')"))?;
    if ms >= STALL_LIMIT_MS {
        return Err(format!(
            "a stall of {ms} ms would trigger GNOME's not-responding dialog; keep it under {STALL_LIMIT_MS}"
        ));
    }
    Ok(ms)
}

/// `size` rounded down to a multiple of `step` on each axis, never below one step.
pub fn snap(size: Size, step: Option<Size>) -> Size {
    let Some((sw, sh)) = step else { return size };
    let down = |v: u32, s: u32| if s == 0 || v < s { v } else { v - v % s };
    (down(size.0, sw), down(size.1, sh))
}

/// What to do when the compositor sizes the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizePlan {
    /// Milliseconds to sleep before painting (0 for none).
    pub delay_ms: u64,
    /// The buffer size to paint.
    pub draw: Size,
    /// Fixture events to write, in order: `(tag, detail)`.
    pub events: Vec<(&'static str, String)>,
}

/// The hostile behaviours switched on for this run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hostility {
    /// Sleep this long before painting at a new size.
    pub resize_delay_ms: u64,
    /// Paint sizes rounded down to a multiple of this step.
    pub resize_increment: Option<Size>,
    /// Block the event loop this long once, right after the first paint.
    pub stall_after_map_ms: Option<u64>,
    /// Flap between two sizes for this long once the first resize arrives.
    pub flap_ms: Option<u64>,
}

impl Hostility {
    /// True when no flag is set: the fixture behaves as before.
    pub fn is_default(&self) -> bool {
        *self == Hostility::default()
    }

    /// The plan for a resize to `configured`, with `last_painted` the size of the last buffer.
    /// The delay applies only when the buffer to paint differs from the last one, so a repeated
    /// redraw at the same size is not slowed.
    pub fn plan_resize(&self, configured: Size, last_painted: Option<Size>) -> ResizePlan {
        let draw = snap(configured, self.resize_increment);
        let mut events = Vec::new();
        if draw != configured {
            events.push((
                "SNAP",
                format!("{}x{}->{}x{}", configured.0, configured.1, draw.0, draw.1),
            ));
        }
        let delay_ms = if self.resize_delay_ms > 0 && last_painted != Some(draw) {
            events.push(("RESIZE_DELAY", self.resize_delay_ms.to_string()));
            self.resize_delay_ms
        } else {
            0
        };
        ResizePlan {
            delay_ms,
            draw,
            events,
        }
    }

    /// The buffer size of flap tick `tick` (1-based) around `base`: two sizes, both different
    /// from `base`, alternating, so the app is never at the size that was asked for while it flaps.
    pub fn flap_size(base: Size, tick: u64) -> Size {
        let shrink = if tick % 2 == 1 { 16 } else { 40 };
        (
            base.0.saturating_sub(shrink).max(1),
            base.1.saturating_sub(shrink).max(1),
        )
    }

    /// Number of flap ticks in the flap: `flap_ms / FLAP_TICK_MS`, at least one when set.
    pub fn flap_ticks(&self) -> Option<u64> {
        self.flap_ms.map(|ms| (ms / FLAP_TICK_MS).max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `WxH` is read with either case of x; zero, missing and non-numeric parts are refused.
    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("800x600"), Ok((800, 600)));
        assert_eq!(parse_size("40X30"), Ok((40, 30)));
        for bad in [
            "",
            "800",
            "800x",
            "x600",
            "0x600",
            "800x0",
            "axb",
            "800x600x1",
            "-1x5",
        ] {
            assert!(parse_size(bad).is_err(), "'{bad}'");
        }
    }

    /// A stall of 4000 ms or more is refused, 3999 is allowed.
    #[test]
    fn test_stall_stays_under_the_not_responding_limit() {
        assert_eq!(parse_stall_ms("2500"), Ok(2500));
        assert_eq!(parse_stall_ms("3999"), Ok(3999));
        assert!(parse_stall_ms("4000")
            .unwrap_err()
            .contains("not-responding"));
        assert!(parse_stall_ms("soon").is_err());
    }

    /// 800x600 on a 37x41 grid is 777x574; a size under one step, or no step, is unchanged.
    #[test]
    fn test_snap_rounds_down_to_the_grid() {
        assert_eq!(snap((800, 600), Some((37, 41))), (777, 574));
        assert_eq!(snap((800, 600), Some((40, 40))), (800, 600));
        assert_eq!(snap((20, 600), Some((37, 41))), (20, 574));
        assert_eq!(snap((800, 600), None), (800, 600));
    }

    /// With no flag the plan is "paint the configured size now" and writes no event.
    #[test]
    fn test_default_plan_changes_nothing() {
        let h = Hostility::default();
        assert!(h.is_default());
        let p = h.plan_resize((800, 600), Some((400, 300)));
        assert_eq!(
            p,
            ResizePlan {
                delay_ms: 0,
                draw: (800, 600),
                events: vec![]
            }
        );
        assert_eq!(h.flap_ticks(), None);
    }

    /// The delay is planned for a new size, with a `RESIZE_DELAY` event, and not for a repaint
    /// at the size already painted.
    #[test]
    fn test_delay_applies_to_a_new_size_only() {
        let h = Hostility {
            resize_delay_ms: 400,
            ..Default::default()
        };
        let p = h.plan_resize((800, 600), Some((400, 300)));
        assert_eq!(p.delay_ms, 400);
        assert_eq!(p.events, vec![("RESIZE_DELAY", "400".to_string())]);
        let again = h.plan_resize((800, 600), Some((800, 600)));
        assert_eq!(again.delay_ms, 0);
        assert!(again.events.is_empty());
        assert_eq!(
            h.plan_resize((800, 600), None).delay_ms,
            400,
            "the first paint is a new size"
        );
    }

    /// The grid step changes the painted size and writes a `SNAP` event with both sizes; the
    /// delay is judged on the snapped size.
    #[test]
    fn test_increment_changes_the_painted_size_and_says_so() {
        let h = Hostility {
            resize_increment: Some((37, 41)),
            resize_delay_ms: 50,
            ..Default::default()
        };
        let p = h.plan_resize((800, 600), Some((400, 300)));
        assert_eq!(p.draw, (777, 574));
        assert_eq!(
            p.events,
            vec![
                ("SNAP", "800x600->777x574".to_string()),
                ("RESIZE_DELAY", "50".to_string())
            ]
        );
        // Already painted at the snapped size: no delay.
        assert_eq!(h.plan_resize((800, 600), Some((777, 574))).delay_ms, 0);
    }

    /// The flap alternates between two sizes that are never the base, and runs T / 20 ticks
    /// (at least one).
    #[test]
    fn test_flap_alternates_and_never_visits_the_base() {
        let base = (800, 600);
        assert_eq!(Hostility::flap_size(base, 1), (784, 584));
        assert_eq!(Hostility::flap_size(base, 2), (760, 560));
        assert_eq!(Hostility::flap_size(base, 3), (784, 584));
        assert_eq!(
            Hostility::flap_size((10, 10), 2),
            (1, 1),
            "never below one pixel"
        );
        let h = Hostility {
            flap_ms: Some(2000),
            ..Default::default()
        };
        assert_eq!(h.flap_ticks(), Some(100));
        assert_eq!(
            Hostility {
                flap_ms: Some(5),
                ..Default::default()
            }
            .flap_ticks(),
            Some(1)
        );
    }

    /// The state colour is a pure function of the buffer size: magenta at the initial size, green
    /// at every other size (a shrink, a grow, a one-axis change); the two are opaque and differ.
    #[test]
    fn test_state_color_names_the_state() {
        let initial = (400, 300);
        assert_eq!(state_color((400, 300), initial), STATE_COLOR_INITIAL);
        for other in [(800, 600), (200, 150), (400, 301), (399, 300)] {
            assert_eq!(
                state_color(other, initial),
                STATE_COLOR_RESIZED,
                "{other:?}"
            );
        }
        assert_ne!(STATE_COLOR_INITIAL, STATE_COLOR_RESIZED);
        assert_eq!(STATE_COLOR_INITIAL >> 24, 0xFF, "opaque");
        assert_eq!(STATE_COLOR_RESIZED >> 24, 0xFF, "opaque");
    }
}
