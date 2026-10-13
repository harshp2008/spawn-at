//! Flash detection: was the window shown before it was ready, or did it change after the reveal?
//!
//! The extension's own trace cannot answer that on every session. On X11 the window manager
//! applies a resize at once, so `get_frame_rect()` reads the requested size 0.2 ms after the
//! request whatever the client has painted. The fixture's own clock is the ground truth: it logs
//! `FIRST_PAINT WxH` for the first paint at each new size, plus `RESIZED` and `MOVED`
//! (`src/bin/fixture/trace.rs`). This module compares those times with the extension's `UNCLOAK`.
//!
//! A flash is, per run:
//! - the reveal (`UNCLOAK`) comes before the fixture's first paint at its final size, or
//! - the window's size or position changes after the reveal by more than
//!   [`GEOMETRY_TOLERANCE_PX`], within [`POST_REVEAL_WATCH_MS`].
//!
//! Size is judged on the fixture's paints, because a paint is the moment the content changes.
//! The toolkit's `Resized` and `Moved` events are times at which it *delivered* the news, which
//! lag the window manager's change, so they are logged but never taken as the time of one;
//! position is judged by the last position the client reports against the one the extension
//! logged at `UNCLOAK`.
//!
//! Both clocks are `CLOCK_MONOTONIC`, but the extension prints milliseconds since its own first
//! log line. [`ClockAnchor`] recovers that origin from `SetLogging` echoes bracketed by monotonic
//! reads; the comparison is only counted when it holds for every origin the anchor allows.

use super::models::TraceEvent;
use serde::{Deserialize, Serialize};

/// How long after the reveal a size or position change still counts as a flash.
pub const POST_REVEAL_WATCH_MS: f64 = 1500.0;

/// Largest per-axis difference, in pixels, that is not a size or position change.
pub const GEOMETRY_TOLERANCE_PX: u32 = 2;

/// Number of bracketed `SetLogging(true)` calls made per run to anchor the extension's clock.
pub const ANCHOR_SAMPLES: usize = 4;

/// Refresh rate assumed when the display's own cannot be read.
pub const DEFAULT_REFRESH_HZ: f64 = 60.0;

/// Per-screen and per-run settings of the flash check.
///
/// Whether a run is a flash does not depend on them except `threshold_ms`: lateness is a number
/// that is the same on every screen. `frame_ms` only labels how likely the user is to have seen
/// it: the compositor shows the revealed window at its next frame, up to one frame period after
/// `UNCLOAK`, so a paint that lands within that period may share the frame and never be seen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlashParams {
    /// Display refresh rate in Hz the visibility label is computed for.
    pub refresh_hz: f64,
    /// Lowest lead in milliseconds that counts as a flash, on top of the clock uncertainty
    /// (`--flash-threshold-ms`); 0 leaves only the uncertainty floor.
    pub threshold_ms: f64,
}

impl Default for FlashParams {
    /// 60 Hz and no extra threshold.
    fn default() -> Self {
        Self {
            refresh_hz: DEFAULT_REFRESH_HZ,
            threshold_ms: 0.0,
        }
    }
}

impl FlashParams {
    /// One frame period in milliseconds.
    pub fn frame_ms(&self) -> f64 {
        1000.0 / self.refresh_hz
    }
}

/// The extension prints `elapsed_ms` with two decimals (`toFixed(2)`), so an echo time is exact
/// to within 5 microseconds either way.
const ECHO_QUANTUM_NS: f64 = 5_000.0;

/// Current `CLOCK_MONOTONIC` in nanoseconds: the clock the fixture stamps its events with and
/// the clock behind the extension's `GLib.get_monotonic_time()`.
pub fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec for the duration of the call.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// What the fixture reported about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientEventKind {
    /// First paint of a buffer of this size (not written again for a repaint at the same size).
    FirstPaint {
        /// Painted width in pixels.
        w: u32,
        /// Painted height in pixels.
        h: u32,
    },
    /// The toolkit delivered a resize to this size.
    Resized {
        /// New width in pixels.
        w: u32,
        /// New height in pixels.
        h: u32,
    },
    /// The toolkit delivered a move to this outer position (X11 only; Wayland sends none).
    Moved {
        /// New horizontal position.
        x: i32,
        /// New vertical position.
        y: i32,
    },
}

/// One line of the fixture's `fixture-<pid>.events` file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientEvent {
    /// `CLOCK_MONOTONIC` nanoseconds when the fixture wrote the line.
    pub t_ns: u64,
    /// What happened.
    pub kind: ClientEventKind,
}

/// Parses `WxH` into a width and height.
fn parse_wh(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

/// Parses `X,Y` into a position.
fn parse_xy(s: &str) -> Option<(i32, i32)> {
    let (x, y) = s.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// Parses the fixture's event file (`<mono_ns> FIXTURE <TAG> <detail>` per line), ordered by
/// time. Lines of other tags (`TRACE_START`, `NEW_EVENTS`, ...) and malformed lines are skipped.
pub fn parse_client_events(text: &str) -> Vec<ClientEvent> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(ns), Some("FIXTURE"), Some(tag)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let Ok(t_ns) = ns.parse::<u64>() else {
            continue;
        };
        let detail = it.next().unwrap_or("");
        let kind = match tag {
            "FIRST_PAINT" => parse_wh(detail).map(|(w, h)| ClientEventKind::FirstPaint { w, h }),
            "RESIZED" => parse_wh(detail).map(|(w, h)| ClientEventKind::Resized { w, h }),
            "MOVED" => parse_xy(detail).map(|(x, y)| ClientEventKind::Moved { x, y }),
            _ => None,
        };
        if let Some(kind) = kind {
            out.push(ClientEvent { t_ns, kind });
        }
    }
    out.sort_by_key(|e| e.t_ns);
    out
}

/// `CLOCK_MONOTONIC` reads taken just before a `SetLogging(true)` call started and just after it
/// returned. The extension handled the call, and wrote its echo, somewhere in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonoBracket {
    /// Monotonic nanoseconds before the call was started.
    pub before_ns: u64,
    /// Monotonic nanoseconds after the call returned.
    pub after_ns: u64,
}

/// The `+N ms` values of the `SET_LOGGING enabled=true` echoes in `slice`, ascending, each value
/// once (the journal and `session.log` can both carry a line).
pub fn echo_times_ms(slice: &str) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for line in slice.lines() {
        let Some(idx) = line.find("ms | SET_LOGGING enabled=true") else {
            continue;
        };
        let before = &line[..idx];
        let start = before
            .rfind(|c: char| !c.is_ascii_digit() && c != '.')
            .map(|p| p + 1)
            .unwrap_or(0);
        if let Ok(ms) = before[start..].parse::<f64>() {
            if !out.contains(&ms) {
                out.push(ms);
            }
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

/// Where the extension's clock origin lies on the monotonic clock.
///
/// The extension prints `+N ms` counted from its first log line. A `SetLogging(true)` call made
/// between two monotonic reads `before` and `after` produced an echo at `X` ms, so the origin is
/// in `[before - X, after - X]`. With several calls the origin lies in the intersection of those
/// intervals; its half-width is the uncertainty of every cross-clock comparison.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockAnchor {
    /// Lowest monotonic nanosecond value the extension's origin can have.
    pub lo_ns: f64,
    /// Highest monotonic nanosecond value the extension's origin can have.
    pub hi_ns: f64,
    /// Number of bracket/echo pairs that were intersected.
    pub samples: usize,
}

impl ClockAnchor {
    /// Intersects the intervals of the last `min(brackets, echoes)` pairs: the `i`-th newest
    /// bracket belongs to the `i`-th newest echo. Fails when there is no pair or the intervals
    /// do not overlap (a bracket paired with a stale or foreign echo, or a reloaded extension).
    pub fn from_samples(brackets: &[MonoBracket], echo_ms: &[f64]) -> Result<ClockAnchor, String> {
        let n = brackets.len().min(echo_ms.len());
        if n == 0 {
            return Err(format!(
                "no clock anchor: {} bracketed call(s), {} echo line(s)",
                brackets.len(),
                echo_ms.len()
            ));
        }
        let b = &brackets[brackets.len() - n..];
        let e = &echo_ms[echo_ms.len() - n..];
        let mut lo = f64::MIN;
        let mut hi = f64::MAX;
        for (br, x) in b.iter().zip(e) {
            let x_ns = x * 1e6;
            lo = lo.max(br.before_ns as f64 - x_ns - ECHO_QUANTUM_NS);
            hi = hi.min(br.after_ns as f64 - x_ns + ECHO_QUANTUM_NS);
        }
        if lo > hi {
            return Err(format!(
                "no clock anchor: the {} bracket(s) disagree by {:.2} ms",
                n,
                (lo - hi) / 1e6
            ));
        }
        Ok(ClockAnchor {
            lo_ns: lo,
            hi_ns: hi,
            samples: n,
        })
    }

    /// Half the width of the origin's interval, in milliseconds.
    pub fn half_width_ms(&self) -> f64 {
        (self.hi_ns - self.lo_ns) / 2e6
    }

    /// Lowest and highest monotonic nanosecond value of an extension time of `ext_ms`.
    fn mono_range(&self, ext_ms: f64) -> (f64, f64) {
        (self.lo_ns + ext_ms * 1e6, self.hi_ns + ext_ms * 1e6)
    }
}

/// What kind of change made the run a flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlashKind {
    /// The window was revealed before the client had painted anything.
    PaintAfterReveal,
    /// The client painted a different size after the reveal than the one it showed at it (this
    /// includes reaching the final size late: reveal before the first paint at that size).
    SizeChangeAfterReveal,
    /// The window's position differs by more than the tolerance from the one logged at the reveal.
    PositionChangeAfterReveal,
}

/// Which log a flash event was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlashSource {
    /// The fixture's own event file (ground truth for what the client painted).
    Client,
    /// The extension's trace (`ANCHOR_CORRECTION` after `UNCLOAK`).
    Extension,
}

/// One observed flash, or a change too close to the reveal to order against it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlashEvent {
    /// What changed.
    pub kind: FlashKind,
    /// Where it was read from.
    pub source: FlashSource,
    /// Milliseconds after the reveal, taken at the middle of the clock anchor's interval; an
    /// event inside the uncertainty can be slightly negative.
    pub lead_ms: f64,
    /// True when the lead is above the clock uncertainty and the `--flash-threshold-ms` floor:
    /// the event is a flash on every screen. Other events are listed but not counted.
    pub numeric: bool,
    /// Per-screen label: `certain` when the lead is at least one frame period (a frame was
    /// drawn between the reveal and the paint), `possible` when it is shorter.
    pub visibility: Option<Visibility>,
    /// Probability that a frame was drawn in between, `min(1, lead / frame_ms)`, taking the
    /// reveal to fall at a random point of the frame period (HYPOTHESIS: it is not measured).
    pub visible_probability: f64,
    /// One line describing the change, with the values.
    pub detail: String,
}

/// How likely a numeric flash is to have been on screen, for the display's refresh rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// The lead is at least one frame period.
    Certain,
    /// The lead is shorter than one frame period.
    Possible,
}

/// What evidence the flash check had for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlashBasis {
    /// The fixture's paint times, compared with the reveal through a clock anchor.
    Client,
    /// Only the extension's trace: it sees a position correction after the reveal, nothing else.
    ExtensionOnly,
    /// No reveal was found, so nothing could be checked.
    None,
}

/// Flash result of one run; flattened into the test result and the report JSON, where the keys
/// are `flash`, `flash_events`, `flash_basis`, `flash_notes` and `flash_clock_uncertainty_ms`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlashInfo {
    /// True when at least one numeric flash event was found (screen-independent).
    pub flash: bool,
    /// Numeric events whose lead is at least one frame period.
    pub flash_certain: usize,
    /// Numeric events whose lead is shorter than one frame period.
    pub flash_possible: usize,
    /// Display refresh rate the labels were computed for (60 when it could not be read).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_hz: Option<f64>,
    /// One frame period in milliseconds at `refresh_hz`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_ms: Option<f64>,
    /// Probability that the run showed at least one of its numeric events,
    /// `1 - prod(1 - p)`; absent when there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_probability: Option<f64>,
    /// Every flash event found, numeric or not.
    pub flash_events: Vec<FlashEvent>,
    /// What evidence the check had; absent when the check did not run for this test.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flash_basis: Option<FlashBasis>,
    /// Why the check was limited (no fixture events, no clock anchor, ...).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flash_notes: Vec<String>,
    /// Half-width of the clock anchor in milliseconds, when one was built.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flash_clock_uncertainty_ms: Option<f64>,
}

impl FlashInfo {
    /// One-line description of the numeric events, for result messages.
    pub fn summary(&self) -> String {
        let certain: Vec<&str> = self
            .flash_events
            .iter()
            .filter(|e| e.numeric)
            .map(|e| e.detail.as_str())
            .collect();
        certain.join("; ")
    }
}

/// Inputs of [`detect_flash`] for one run.
pub struct FlashInputs<'a> {
    /// The extension's events for the run's window, in log order.
    pub trace_events: &'a [TraceEvent],
    /// The fixture's events, ordered by time; empty when the fixture wrote none.
    pub client_events: &'a [ClientEvent],
    /// Origin of the extension's clock on the monotonic clock, when one could be built.
    pub anchor: Option<&'a ClockAnchor>,
    /// Refresh rate for the visibility label and the threshold floor.
    pub params: FlashParams,
}

/// Where a client event falls against the reveal.
enum Rel {
    /// Before the reveal for every origin the anchor allows.
    Before,
    /// Before or after, depending on the origin within the anchor's interval.
    Ambiguous,
    /// After the reveal for every origin the anchor allows.
    After,
}

/// Returns true when the sizes `a` and `b` differ by no more than the tolerance on both axes.
fn same_size(a: (u32, u32), b: (u32, u32)) -> bool {
    a.0.abs_diff(b.0) <= GEOMETRY_TOLERANCE_PX && a.1.abs_diff(b.1) <= GEOMETRY_TOLERANCE_PX
}

/// Returns true when the positions `a` and `b` differ by no more than the tolerance on both axes.
fn same_pos(a: (i32, i32), b: (i32, i32)) -> bool {
    a.0.abs_diff(b.0) <= GEOMETRY_TOLERANCE_PX && a.1.abs_diff(b.1) <= GEOMETRY_TOLERANCE_PX
}

/// Parses `from=(x,y) to=(x,y)` of an `ANCHOR_CORRECTION` line.
fn correction_move(extra: &str) -> Option<((i32, i32), (i32, i32))> {
    let pair = |key: &str| -> Option<(i32, i32)> {
        let rest = &extra[extra.find(key)? + key.len()..];
        let inner = rest.strip_prefix('(')?.split(')').next()?;
        parse_xy(inner)
    };
    Some((pair("from=")?, pair("to=")?))
}

/// Finds the flashes of one run.
///
/// `basis` is [`FlashBasis::Client`] when the fixture's events and a clock anchor are both
/// present. The fixture's paints then decide what the user saw: content that changed size after
/// the reveal, or a window revealed before anything was painted. Only a paint is taken as the
/// time of a visible change; `Resized` and `Moved` are times at which the toolkit *delivered* an
/// event, which lag the window manager's change (on X11 the frame is resized and moved at once).
/// A client position counts only when the last one reported differs from the position the
/// extension logged at `UNCLOAK`. An `ANCHOR_CORRECTION` after `UNCLOAK` is always used, and is
/// the only signal left when the fixture's events or the clock anchor are missing; `flash_notes`
/// then says what was missing.
pub fn detect_flash(inp: &FlashInputs) -> FlashInfo {
    let mut info = FlashInfo::default();
    let Some(unc_idx) = inp.trace_events.iter().position(|e| e.tag == "UNCLOAK") else {
        info.flash_basis = Some(FlashBasis::None);
        info.flash_notes
            .push("no UNCLOAK in the trace: the reveal time is unknown".into());
        return info;
    };
    let reveal_ms = inp.trace_events[unc_idx].elapsed_ms;
    let revealed_pos = inp.trace_events[unc_idx].frame.map(|f| (f.x, f.y));

    let client_usable = match (inp.client_events.is_empty(), inp.anchor) {
        (false, Some(_)) => true,
        (true, _) => {
            info.flash_notes.push(
                "no fixture events: only the extension's post-reveal correction is checked".into(),
            );
            false
        }
        (false, None) => {
            info.flash_notes
                .push("no clock anchor: fixture times cannot be compared with the reveal".into());
            false
        }
    };
    info.flash_basis = Some(if client_usable {
        FlashBasis::Client
    } else {
        FlashBasis::ExtensionOnly
    });

    let correction = inp.trace_events[unc_idx + 1..]
        .iter()
        .find(|e| e.tag == "ANCHOR_CORRECTION");
    if let Some(corr) = correction {
        let moved = correction_move(&corr.extra);
        if moved.map_or(true, |(from, to)| !same_pos(from, to)) {
            let at = (corr.elapsed_ms - reveal_ms).max(0.0);
            let detail = match moved {
                Some((f, t)) => format!(
                    "position corrected {},{} -> {},{} {:.1} ms after the reveal",
                    f.0, f.1, t.0, t.1, at
                ),
                None => format!("position corrected {:.1} ms after the reveal", at),
            };
            info.flash_events.push(FlashEvent {
                kind: FlashKind::PositionChangeAfterReveal,
                source: FlashSource::Extension,
                lead_ms: at,
                // The extension's own clock has no uncertainty; only the floor applies.
                numeric: at > inp.params.threshold_ms,
                visibility: None,
                visible_probability: 0.0,
                detail,
            });
        }
    }

    if let (true, Some(anchor)) = (client_usable, inp.anchor) {
        info.flash_clock_uncertainty_ms = Some(anchor.half_width_ms());
        client_flashes(
            inp.client_events,
            anchor,
            reveal_ms,
            // The extension's correction already names the move; the client's position is only
            // consulted when there is none.
            if correction.is_some() {
                None
            } else {
                revealed_pos
            },
            inp.params.threshold_ms,
            &mut info,
        );
    }
    label_visibility(&mut info, inp.params);
    info
}

/// Fills in the per-screen labels and the counts. The headline `flash` is the numeric count
/// being above zero and does not look at the frame period.
fn label_visibility(info: &mut FlashInfo, params: FlashParams) {
    let frame_ms = params.frame_ms();
    info.refresh_hz = Some(params.refresh_hz);
    info.frame_ms = Some(frame_ms);
    let mut none_seen = 1.0;
    for e in &mut info.flash_events {
        e.visible_probability = (e.lead_ms / frame_ms).clamp(0.0, 1.0);
        e.visibility = Some(if e.lead_ms >= frame_ms {
            Visibility::Certain
        } else {
            Visibility::Possible
        });
        if e.numeric {
            none_seen *= 1.0 - e.visible_probability;
            if e.visibility == Some(Visibility::Certain) {
                info.flash_certain += 1;
            } else {
                info.flash_possible += 1;
            }
        }
    }
    info.flash = info.flash_certain + info.flash_possible > 0;
    info.visible_probability = info.flash.then_some(1.0 - none_seen);
}

/// Adds the flash events the fixture's own timeline shows, relative to the reveal at `reveal_ms`
/// on the extension's clock. `revealed_pos` is the window position the extension logged at the
/// reveal, or `None` to skip the position check.
fn client_flashes(
    client: &[ClientEvent],
    anchor: &ClockAnchor,
    reveal_ms: f64,
    revealed_pos: Option<(i32, i32)>,
    threshold_ms: f64,
    info: &mut FlashInfo,
) {
    let (r_lo, r_hi) = anchor.mono_range(reveal_ms);
    let r_mid = (r_lo + r_hi) / 2.0;
    let rel = |t_ns: u64| {
        let t = t_ns as f64;
        if t > r_hi {
            Rel::After
        } else if t < r_lo {
            Rel::Before
        } else {
            Rel::Ambiguous
        }
    };
    let after_ms = |t_ns: u64| (t_ns as f64 - r_mid) / 1e6;
    let watched = |t_ns: u64| after_ms(t_ns) <= POST_REVEAL_WATCH_MS;

    // The paints: `FIRST_PAINT` is written once per new size, so each one is a change of what the
    // window shows. The content at the reveal is the newest paint that is certainly before it.
    let paint_of = |e: &ClientEvent| match e.kind {
        ClientEventKind::FirstPaint { w, h } => Some((w, h)),
        _ => None,
    };
    let mut shown = client
        .iter()
        .filter(|e| matches!(rel(e.t_ns), Rel::Before))
        .filter_map(paint_of)
        .last();
    for e in client.iter().filter(|e| watched(e.t_ns)) {
        let (Some(sz), r) = (paint_of(e), rel(e.t_ns)) else {
            continue;
        };
        if matches!(r, Rel::Before) {
            continue;
        }
        let at = after_ms(e.t_ns);
        let (kind, detail) = match shown {
            // Nothing had been painted when the window was revealed.
            None => (
                FlashKind::PaintAfterReveal,
                format!(
                    "first paint at {}x{} came {:.1} ms after the reveal (nothing was painted at the reveal)",
                    sz.0, sz.1, at
                ),
            ),
            Some(cur) if same_size(sz, cur) => continue,
            Some(cur) => (
                FlashKind::SizeChangeAfterReveal,
                format!(
                    "first paint at {}x{} came {:.1} ms after the reveal (content was {}x{})",
                    sz.0, sz.1, at, cur.0, cur.1
                ),
            ),
        };
        info.flash_events.push(FlashEvent {
            kind,
            source: FlashSource::Client,
            lead_ms: at,
            numeric: matches!(r, Rel::After) && at > threshold_ms,
            visibility: None,
            visible_probability: 0.0,
            detail,
        });
        shown = Some(sz);
    }

    // The position the client ends up reporting, against the one the extension logged at the
    // reveal. A report delivered late for a move that happened before the reveal equals the
    // revealed position, so only a different final position is a move after it.
    let Some(rev) = revealed_pos else { return };
    let last_move = client
        .iter()
        .filter(|e| watched(e.t_ns))
        .filter_map(|e| match e.kind {
            ClientEventKind::Moved { x, y } => Some((e.t_ns, (x, y))),
            _ => None,
        })
        .last();
    if let Some((t_ns, pos)) = last_move {
        let r = rel(t_ns);
        if !matches!(r, Rel::Before) && !same_pos(pos, rev) {
            info.flash_events.push(FlashEvent {
                kind: FlashKind::PositionChangeAfterReveal,
                source: FlashSource::Client,
                lead_ms: after_ms(t_ns),
                numeric: matches!(r, Rel::After) && after_ms(t_ns) > threshold_ms,
                visibility: None,
                visible_probability: 0.0,
                detail: format!(
                    "position {},{} -> {},{} reported {:.1} ms after the reveal",
                    rev.0,
                    rev.1,
                    pos.0,
                    pos.1,
                    after_ms(t_ns)
                ),
            });
        }
    }
}
