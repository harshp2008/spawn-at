//! Comparison of an observed placement against the oracle's expected rectangle.
//!
//! One comparison is shared by the runner (which decides pass/fail) and the drawer (which shows
//! the Expected / Actual / Result table), so the two cannot disagree. Expected never comes from
//! the observed result: it is the oracle applied to the test spec and the workarea.

use super::calculator::{calculate_expected_rect, OracleParams, Rect};
use crate::schema::ExpectRect;
use regex::Regex;

/// Outcome of comparing the observed rectangle with the expected one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryStatus {
    /// There is no observed rectangle (the test did not run or printed none), so nothing was
    /// compared.
    NotMeasured,
    /// A rectangle was observed but no workarea is available from the probe or the log, so the
    /// oracle cannot say where it should be. The geometry is NOT verified.
    Unchecked,
    /// Every component is within the tolerance.
    Pass,
    /// At least one component differs by more than the tolerance.
    Fail,
}

/// Where the workarea used to compute the expected rectangle came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkareaSource {
    /// Read from the window system (mutter's `_NET_WORKAREA`), independent of the extension under
    /// test. Extension-derived values only cross-check it: they either agree or produce a
    /// conflict (see [`GeometryCheck::conflict`]); judging is always on this value.
    Independent,
    /// `<bin> query layout`, which on GNOME is the extension under test, so a pass is not
    /// independent evidence.
    ExtensionLayout,
    /// Only the bounds the extension logged were available; also extension-derived.
    ExtensionLog,
    /// No workarea was available.
    None,
}

impl WorkareaSource {
    /// True when the extension under test supplied the workarea, so the result is not
    /// independent evidence.
    pub fn is_extension_derived(self) -> bool {
        matches!(self, Self::ExtensionLayout | Self::ExtensionLog)
    }
}

/// Workarea measurements available when a result is judged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkareaProbe {
    /// Read from the window system, independent of the extension. Primary when present.
    pub independent: Option<Rect>,
    /// From `query layout` (the extension on GNOME). A cross-check, or the fallback when there
    /// is no independent value.
    pub extension: Option<Rect>,
}

/// Expected vs actual placement for one result, with per-row verdicts.
#[derive(Debug, Clone)]
pub struct GeometryCheck {
    /// Oracle rectangle, if it could be computed (needs a workarea and a computable spec).
    pub expected: Option<Rect>,
    /// Size the spec asks for; `None` when the spec states no size, in which case size is not
    /// compared and the window's own size is used as an oracle input for the position.
    pub spec_size: Option<(u32, u32)>,
    /// Rectangle the window was observed at (from the run's stdout).
    pub actual: Option<Rect>,
    /// Allowed difference in pixels per component.
    pub tolerance_px: i32,
    /// Why Expected is unavailable, when it is.
    pub expected_error: Option<String>,
    /// Workarea the expected rectangle was computed in.
    pub workarea: Option<Rect>,
    /// Where `workarea` came from.
    pub workarea_source: WorkareaSource,
    /// `(independent, extension-derived)` when an independent workarea exists and an
    /// extension-derived one (layout or log) differs. Judging still uses the independent one.
    pub conflict: Option<(Rect, Rect)>,
}

/// Formats a rectangle as `x,y WxH` for messages.
fn fmt_rect(r: Rect) -> String {
    format!("{},{} {}×{}", r.x, r.y, r.w, r.h)
}

impl GeometryCheck {
    /// Warning to show next to the verdict, if any: a probe/extension workarea disagreement.
    pub fn warning(&self) -> Option<String> {
        self.conflict.map(|(probe, ext)| {
            format!(
                "workarea differs: probe {} vs extension {}",
                fmt_rect(probe),
                fmt_rect(ext)
            )
        })
    }

    /// Signed differences `actual - expected` as `(dx, dy, dw, dh)`, when both rectangles exist.
    pub fn deltas(&self) -> Option<(i32, i32, i32, i32)> {
        let (e, a) = (self.expected?, self.actual?);
        Some((
            a.x - e.x,
            a.y - e.y,
            a.w as i32 - e.w as i32,
            a.h as i32 - e.h as i32,
        ))
    }

    /// Whether the position matches within tolerance; `None` if it cannot be compared.
    pub fn position_ok(&self) -> Option<bool> {
        let (dx, dy, _, _) = self.deltas()?;
        Some(dx.abs() <= self.tolerance_px && dy.abs() <= self.tolerance_px)
    }

    /// Whether the size matches within tolerance; `None` if the spec states no size or the
    /// rectangles are unavailable.
    pub fn size_ok(&self) -> Option<bool> {
        self.spec_size?;
        let (_, _, dw, dh) = self.deltas()?;
        Some(dw.abs() <= self.tolerance_px && dh.abs() <= self.tolerance_px)
    }

    /// Overall verdict: `NotMeasured` without an observed rectangle, `Unchecked` with one but no
    /// expected rectangle, otherwise `Fail` if any compared row fails and `Pass` if not.
    pub fn status(&self) -> GeometryStatus {
        if self.actual.is_none() {
            return GeometryStatus::NotMeasured;
        }
        match (self.position_ok(), self.size_ok()) {
            (None, _) => GeometryStatus::Unchecked,
            (Some(false), _) | (_, Some(false)) => GeometryStatus::Fail,
            _ => GeometryStatus::Pass,
        }
    }

    /// `Err` describing the mismatch when the status is `Fail`, `Ok` otherwise.
    pub fn verdict(&self) -> Result<(), String> {
        if self.status() != GeometryStatus::Fail {
            return Ok(());
        }
        let (e, a) = (self.expected.unwrap(), self.actual.unwrap());
        let (dx, dy, dw, dh) = self.deltas().unwrap();
        Err(format!(
            "Geometry mismatch (tolerance {}px): expected [{}, {}, {}x{}], actual [{}, {}, {}x{}] (delta actual-expected: dx={:+}, dy={:+}, dw={:+}, dh={:+})",
            self.tolerance_px, e.x, e.y, e.w, e.h, a.x, a.y, a.w, a.h, dx, dy, dw, dh
        ))
    }
}

/// Reads the observed rectangle from a spawn result's stdout JSON (`x`, `y`, `w`, `h`).
pub fn actual_rect_from_stdout(stdout: &str) -> Option<Rect> {
    let val: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    Some(Rect::new(
        i32::try_from(val.get("x")?.as_i64()?).ok()?,
        i32::try_from(val.get("y")?.as_i64()?).ok()?,
        u32::try_from(val.get("w")?.as_u64()?).ok()?,
        u32::try_from(val.get("h")?.as_u64()?).ok()?,
    ))
}

/// Reads the workarea the compositor used from the first `bounds=(x,y,WxH)` in a trace slice
/// (logged by `APPLY_ANCHOR`). It is an input to placement, not an output.
pub fn workarea_from_log(slice: &str) -> Option<Rect> {
    let re = Regex::new(r"bounds=\((-?\d+),(-?\d+),(\d+)x(\d+)\)").ok()?;
    let c = re.captures(slice)?;
    Some(Rect::new(
        c[1].parse().ok()?,
        c[2].parse().ok()?,
        c[3].parse().ok()?,
        c[4].parse().ok()?,
    ))
}

/// Computes the oracle rectangle for `expect` inside `workarea`.
///
/// `size` is the window size used as an oracle input: the spec's size when it has one, otherwise
/// the observed size supplied by the caller.
pub fn expected_rect(
    expect: &ExpectRect,
    workarea: Rect,
    size: (u32, u32),
) -> Result<Rect, String> {
    let params = OracleParams {
        anchor: expect.anchor.clone(),
        pivot: expect.pivot.clone(),
        explicit_pos: None,
        offset: None,
        area: None,
        cursor_pos: None,
        size,
        margin: expect.margin.unwrap_or(0),
        margin_top: expect.margin_top,
        margin_bottom: expect.margin_bottom,
        margin_left: expect.margin_left,
        margin_right: expect.margin_right,
        clamp: true,
    };
    calculate_expected_rect(workarea, &params)
}

/// Builds the shared geometry comparison for one result.
///
/// The workarea is `probe.independent` (read from the window system, independent of the
/// extension under test) whenever it exists. The extension-derived values (`probe.extension`
/// from `query layout`, and the bounds the extension logged) are only cross-checks: if one
/// differs from the independent value the check records a conflict and still judges on the
/// independent one. Without an independent value the extension's layout, then its log bounds,
/// are used and the check is marked extension-derived because the extension supplied its own
/// reference.
pub fn check_geometry(
    expect: &ExpectRect,
    stdout: &str,
    log_slice: Option<&str>,
    probe: WorkareaProbe,
) -> GeometryCheck {
    let actual = actual_rect_from_stdout(stdout);
    let spec_size = match &expect.size {
        Some(s) if s.len() >= 2 => Some((s[0], s[1])),
        _ => None,
    };
    let log_workarea = log_slice.and_then(workarea_from_log);
    let (workarea, workarea_source, conflict) =
        match (probe.independent, probe.extension, log_workarea) {
            (Some(p), ext, log) => {
                let differing = [ext, log].into_iter().flatten().find(|e| *e != p);
                (
                    Some(p),
                    WorkareaSource::Independent,
                    differing.map(|e| (p, e)),
                )
            }
            (None, Some(e), _) => (Some(e), WorkareaSource::ExtensionLayout, None),
            (None, None, Some(l)) => (Some(l), WorkareaSource::ExtensionLog, None),
            (None, None, None) => (None, WorkareaSource::None, None),
        };

    let (expected, expected_error) = match (workarea, spec_size.or(actual.map(|a| (a.w, a.h)))) {
        (Some(wa), Some(size)) => match expected_rect(expect, wa, size) {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e)),
        },
        (None, _) => (None, Some("no workarea recorded or probed".to_string())),
        (_, None) => (None, Some("no window size to place".to_string())),
    };
    // With no size in the spec the oracle's size is the observed size; report it as expected so
    // the position comparison is meaningful, but size itself is never judged (see `size_ok`).
    GeometryCheck {
        expected,
        spec_size,
        actual,
        tolerance_px: expect.tolerance_px,
        expected_error,
        workarea,
        workarea_source,
        conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec used by the tests: bottom-right, 400x300, margin 16, tolerance 1.
    fn spec() -> ExpectRect {
        ExpectRect {
            anchor: Some("bottom-right".into()),
            pivot: None,
            size: Some(vec![400, 300]),
            margin: Some(16),
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            tolerance_px: 1,
        }
    }

    /// Workarea 1920x1040 at (0, 40), as in the real recordings.
    fn wa() -> Rect {
        Rect::new(0, 40, 1920, 1040)
    }

    /// A probe with only an independent (window-system) workarea.
    fn indep(r: Rect) -> WorkareaProbe {
        WorkareaProbe {
            independent: Some(r),
            extension: None,
        }
    }

    /// Independent workarea wins over both extension-derived values; a differing `query layout`
    /// value (here 0,0 1920x1000) warns even when the log agrees with the independent one.
    #[test]
    fn test_independent_beats_extension_layout_and_layout_conflict_warns() {
        let probe = WorkareaProbe {
            independent: Some(wa()),
            extension: Some(Rect::new(0, 0, 1920, 1000)),
        };
        let c = check_geometry(&spec(), &out(1504, 764, 400, 300), Some(LOG_AGREES), probe);
        assert_eq!(c.workarea_source, WorkareaSource::Independent);
        assert_eq!(c.status(), GeometryStatus::Pass);
        assert_eq!(
            c.warning().as_deref(),
            Some("workarea differs: probe 0,40 1920×1040 vs extension 0,0 1920×1000")
        );
    }

    /// No independent value: the extension's `query layout` is used (and marked extension-derived)
    /// ahead of the log; a layout/log difference is not a conflict because both are the extension.
    #[test]
    fn test_extension_layout_is_the_fallback_and_marked_extension_derived() {
        let probe = WorkareaProbe {
            independent: None,
            extension: Some(wa()),
        };
        let c = check_geometry(&spec(), &out(1504, 764, 400, 300), Some(LOG_DIFFERS), probe);
        assert_eq!(c.workarea_source, WorkareaSource::ExtensionLayout);
        assert!(c.workarea_source.is_extension_derived());
        assert_eq!(c.conflict, None);
        assert_eq!(c.status(), GeometryStatus::Pass);
        assert!(!WorkareaSource::Independent.is_extension_derived());
    }

    /// Stdout JSON for a window at `x, y` with size `w`x`h`.
    fn out(x: i32, y: i32, w: u32, h: u32) -> String {
        format!(
            r#"{{"window_id":1,"x":{},"y":{},"w":{},"h":{}}}"#,
            x, y, w, h
        )
    }

    /// Bottom-right of a 1920x1040 workarea at (0,40): x = 1920-400-16, y = 40+1040-300-16.
    #[test]
    fn test_expected_comes_from_oracle_not_from_actual() {
        let c = check_geometry(&spec(), &out(0, 0, 1, 1), None, indep(wa()));
        assert_eq!(c.expected, Some(Rect::new(1504, 764, 400, 300)));
    }

    /// A window exactly at the expected rectangle passes.
    #[test]
    fn test_match_passes() {
        let c = check_geometry(&spec(), &out(1504, 764, 400, 300), None, indep(wa()));
        assert_eq!(c.status(), GeometryStatus::Pass);
        assert_eq!(c.position_ok(), Some(true));
        assert_eq!(c.size_ok(), Some(true));
        assert!(c.verdict().is_ok());
    }

    /// Off by exactly the tolerance (1 px) in x and y is still a pass.
    #[test]
    fn test_exactly_tolerance_passes() {
        let c = check_geometry(&spec(), &out(1505, 763, 401, 299), None, indep(wa()));
        assert_eq!(c.status(), GeometryStatus::Pass);
    }

    /// One pixel more than the tolerance fails, and the message names the deltas.
    #[test]
    fn test_beyond_tolerance_fails() {
        let c = check_geometry(&spec(), &out(1506, 764, 400, 300), None, indep(wa()));
        assert_eq!(c.status(), GeometryStatus::Fail);
        assert_eq!(c.position_ok(), Some(false));
        assert_eq!(c.size_ok(), Some(true));
        let err = c.verdict().unwrap_err();
        assert!(err.contains("dx=+2") && err.contains("dy=+0"), "{}", err);
    }

    /// A wrong size fails even when the position is right.
    #[test]
    fn test_wrong_size_fails() {
        let c = check_geometry(&spec(), &out(1504, 764, 420, 300), None, indep(wa()));
        assert_eq!(c.position_ok(), Some(true));
        assert_eq!(c.size_ok(), Some(false));
        assert_eq!(c.status(), GeometryStatus::Fail);
    }

    /// No stdout rectangle (test not run) is "not measured", never a pass.
    #[test]
    fn test_no_actual_is_not_measured() {
        let c = check_geometry(&spec(), "", None, indep(wa()));
        assert_eq!(c.status(), GeometryStatus::NotMeasured);
        assert!(c.expected.is_some());
        assert!(c.verdict().is_ok());
    }

    /// Log line recording the extension's workarea bounds `0,40 1920x1040`.
    const LOG_AGREES: &str =
        "x APPLY_ANCHOR eff=(400x300) bounds=(0,40,1920x1040) final=(1504,764)";
    /// Log line recording different bounds, `0,0 1920x1000` (a shorter workarea: bottom-right is 1504, 684).
    const LOG_DIFFERS: &str =
        "x APPLY_ANCHOR eff=(400x300) bounds=(0,0,1920x1000) final=(1504,684)";

    /// Probe and log agree: judged on the probe, no conflict, no warning.
    #[test]
    fn test_probe_and_log_agree() {
        let c = check_geometry(
            &spec(),
            &out(1504, 764, 400, 300),
            Some(LOG_AGREES),
            indep(wa()),
        );
        assert_eq!(c.workarea_source, WorkareaSource::Independent);
        assert_eq!(c.conflict, None);
        assert_eq!(c.warning(), None);
        assert_eq!(c.status(), GeometryStatus::Pass);
    }

    /// Probe and log disagree and the window matches the oracle under the PROBE workarea:
    /// still a pass (judged on the probe) but with the warning naming both rectangles.
    #[test]
    fn test_probe_and_log_disagree_judged_on_probe_with_warning() {
        let c = check_geometry(
            &spec(),
            &out(1504, 764, 400, 300),
            Some(LOG_DIFFERS),
            indep(wa()),
        );
        assert_eq!(c.workarea, Some(wa()));
        assert_eq!(c.expected, Some(Rect::new(1504, 764, 400, 300)));
        assert_eq!(
            c.warning().as_deref(),
            Some("workarea differs: probe 0,40 1920×1040 vs extension 0,0 1920×1000")
        );
        assert_eq!(c.status(), GeometryStatus::Pass);
    }

    /// Probe and log disagree and the window sits where the EXTENSION'S workarea says (bottom-right
    /// of 0,0 1920x1000 is 1504, 684) but not where the probe says: the check fails, so a wrong
    /// extension cannot agree with itself.
    #[test]
    fn test_probe_and_log_disagree_window_at_extension_position_fails() {
        let c = check_geometry(
            &spec(),
            &out(1504, 684, 400, 300),
            Some(LOG_DIFFERS),
            indep(wa()),
        );
        assert_eq!(c.status(), GeometryStatus::Fail);
        assert!(c.warning().is_some());
        assert!(c.verdict().is_err());
    }

    /// Only the extension's log bounds exist: they are used, but the check is marked as
    /// extension-derived and the pass is not independent evidence.
    #[test]
    fn test_only_log_is_used_and_marked_extension_derived() {
        let c = check_geometry(
            &spec(),
            &out(1504, 764, 400, 300),
            Some(LOG_AGREES),
            WorkareaProbe::default(),
        );
        assert_eq!(c.workarea_source, WorkareaSource::ExtensionLog);
        assert_eq!(c.status(), GeometryStatus::Pass);
        assert_eq!(c.warning(), None);
        let c = check_geometry(
            &spec(),
            &out(1500, 764, 400, 300),
            Some(LOG_AGREES),
            WorkareaProbe::default(),
        );
        assert_eq!(c.status(), GeometryStatus::Fail);
    }

    /// No workarea from either source: a rectangle was observed but cannot be verified, which is
    /// `Unchecked`, never `Pass`, and the runner's verdict does not fail.
    #[test]
    fn test_no_workarea_is_unchecked_not_pass() {
        let c = check_geometry(
            &spec(),
            &out(1504, 764, 400, 300),
            None,
            WorkareaProbe::default(),
        );
        assert_eq!(c.expected, None);
        assert_eq!(c.workarea_source, WorkareaSource::None);
        assert!(c.expected_error.is_some());
        assert_eq!(c.status(), GeometryStatus::Unchecked);
        assert!(c.verdict().is_ok());
    }

    /// A spec without a size compares the position (using the observed size as oracle input)
    /// but never judges the size.
    #[test]
    fn test_spec_without_size_judges_position_only() {
        let mut s = spec();
        s.size = None;
        let c = check_geometry(&s, &out(1504, 764, 123, 77), None, indep(wa()));
        assert_eq!(c.size_ok(), None);
        assert_eq!(
            c.position_ok(),
            Some(false),
            "bottom-right of a 123x77 window differs"
        );
        let c = check_geometry(
            &s,
            &out(1920 - 123 - 16, 40 + 1040 - 77 - 16, 123, 77),
            None,
            indep(wa()),
        );
        assert_eq!(c.status(), GeometryStatus::Pass);
    }
}
