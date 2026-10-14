//! Phase gaps inside one window's placement trace, for diagnosing where cloak time goes.
//!
//! Diagnostic only: nothing here affects pass/fail.

use super::models::TraceEvent;

/// Time spent between consecutive landmarks of one placement, in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhaseGaps {
    /// `ACTOR_MAP` to `CLOAK` (the extension reacting to the new window).
    pub map_to_cloak: Option<f64>,
    /// `CLOAK` to `COMMIT_RESOLVED` (size request and commit wait).
    pub cloak_to_commit: Option<f64>,
    /// `COMMIT_RESOLVED` to the `BATCH_STEP inst=SetPositionAnchored` that follows it.
    pub commit_to_position: Option<f64>,
    /// The last `UNCLOAK_4C` to `UNCLOAK` (the one-frame wait before the reveal).
    pub uncloak_4c_to_uncloak: Option<f64>,
}

/// First event with `tag` at or after index `from`.
fn find(events: &[TraceEvent], from: usize, tag: &str, extra: Option<&str>) -> Option<usize> {
    (from..events.len())
        .find(|&i| events[i].tag == tag && extra.is_none_or(|x| events[i].extra.contains(x)))
}

/// Computes the phase gaps from one window's events in log order.
///
/// Each gap is measured between the first matching pair of landmarks in order, so a window with
/// several placements reports its first. A gap is `None` when either landmark is missing.
pub fn phase_gaps(events: &[TraceEvent]) -> PhaseGaps {
    let t = |i: Option<usize>| i.map(|i| events[i].elapsed_ms);
    let diff = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) => Some((b - a).max(0.0)),
        _ => None,
    };
    let map = find(events, 0, "ACTOR_MAP", None);
    let cloak = find(events, map.unwrap_or(0), "CLOAK", None);
    let commit = find(events, cloak.unwrap_or(0), "COMMIT_RESOLVED", None);
    let position = find(
        events,
        commit.unwrap_or(0),
        "BATCH_STEP",
        Some("inst=SetPositionAnchored"),
    );
    let last_4c = (0..events.len())
        .rev()
        .find(|&i| events[i].tag == "UNCLOAK_4C");
    let uncloak = last_4c.and_then(|c| find(events, c, "UNCLOAK", None));
    PhaseGaps {
        map_to_cloak: diff(t(map), t(cloak)),
        cloak_to_commit: diff(t(cloak), t(commit)),
        commit_to_position: if commit.is_some() {
            diff(t(commit), t(position))
        } else {
            None
        },
        uncloak_4c_to_uncloak: diff(t(last_4c), t(uncloak)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::TraceAnalyser;

    /// One log line for window 7.
    fn line(ms: f64, tag: &str, extra: &str) -> String {
        format!(
            "[spawn-at-time] +{:.2}ms | {} win=[id=7 pid=1 class=\"c\" app=\"\" type=NORMAL proto=Wayland] {}",
            ms, tag, extra
        )
    }

    /// The real delay-500 shape: gaps are read between the right landmarks and `UNCLOAK` is the
    /// one after the last `UNCLOAK_4C`, not the `UNCLOAK_4D_REVEAL` that follows it.
    #[test]
    fn test_phase_gaps_between_landmarks() {
        let log = [
            line(100.0, "ACTOR_MAP", ""),
            line(100.4, "CLOAK", "opacity=0"),
            line(100.5, "BATCH_STEP", "inst=SetSize"),
            line(100.6, "COMMIT_RESOLVED", "reason=AT_TARGET"),
            line(134.6, "BATCH_STEP", "inst=SetPositionAnchored"),
            line(134.9, "UNCLOAK_4C", "positioned=(1x1)"),
            line(139.9, "UNCLOAK", "opacity=255"),
            line(140.0, "UNCLOAK_4D_REVEAL", ""),
        ]
        .join("\n");
        let events = TraceAnalyser::new().parse_events(&log);
        let g = phase_gaps(&events);
        let near = |a: Option<f64>, b: f64| (a.unwrap() - b).abs() < 0.01;
        assert!(near(g.map_to_cloak, 0.4), "{:?}", g);
        assert!(near(g.cloak_to_commit, 0.2), "{:?}", g);
        assert!(near(g.commit_to_position, 34.0), "{:?}", g);
        assert!(near(g.uncloak_4c_to_uncloak, 5.0), "{:?}", g);
    }

    /// Missing landmarks give `None`, never a made-up number.
    #[test]
    fn test_phase_gaps_none_when_landmarks_missing() {
        let events = TraceAnalyser::new().parse_events(&line(1.0, "ACTOR_MAP", ""));
        assert_eq!(phase_gaps(&events), PhaseGaps::default());
        assert_eq!(phase_gaps(&[]), PhaseGaps::default());
    }
}
