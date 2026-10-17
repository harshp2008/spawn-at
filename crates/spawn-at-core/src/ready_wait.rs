//! # Ready-wait state machine
//!
//! Decides when a window has answered a size request, from size samples and time alone. It does
//! no I/O and reads no clock: the caller passes the current time in milliseconds with every call
//! and schedules a timer for the deadline the machine returns. That makes it testable with a
//! virtual clock and identical in a port to another language that follows the same scripts.
//!
//! Rules (native Wayland strategy, `docs/DESIGN_READY_WAIT.md` section 4.3):
//! - A non-resizable, fullscreen or tiled window is [`Outcome::Skipped`]; this is checked first.
//! - A target equal to the current size is [`Outcome::Reached`] at once.
//! - Otherwise wait for a sample equal to the target: [`Outcome::Reached`].
//! - The first sample that differs from the previous size is the window's answer. If it is not
//!   the target, the quiet rule takes over: the wait ends with [`Outcome::AdjustedTo`] once
//!   `quiet_ms` pass without a further change, and any further change restarts that clock.
//! - No answer within `cap_ms` of the start is [`Outcome::TimedOut`] with phase `Cap`.
//! - `hard_ms` after the start ends the wait in any phase: `TimedOut` with phase `Hard`.
//!
//! A sample equal to the previous size is not an answer and is ignored.
//!
//! Ordering: a sample arriving at the same millisecond as a deadline is applied before the timer
//! fires ([`ReadyWait::on_sample`] expires only when `now` is later than the deadline, while
//! [`ReadyWait::on_tick`] expires at `now >= deadline`); a sample arriving after a deadline is
//! handled as if the timer had fired on time, and the outcome reports the deadline, not the late
//! arrival. When the quiet or cap deadline and the hard deadline coincide, or the cap is longer
//! than the hard limit, the outcome is the `Hard` timeout.
//!
//! The machine never checks that time moves forward; see [`ReadyWait::on_sample`].

// The JS port is assets/gnome/modern/ready_wait.js; the Rust and the JS machine are both run
// against the scripts in tests/ready_wait_vectors/ (tests/ready_wait_test.js and
// crates/spawn-at-core/tests/ready_wait_vectors.rs). Why an equal sample is not an answer: in the
// measured run in the table of docs/TEST_PLAN.md section 2.1 a first `allocation` sample still
// showed the pre-resize frame (400x300) before the real size arrived. Not verified beyond reading
// that table.

use serde::{Deserialize, Serialize};

/// Milliseconds on the caller's clock. Only differences matter.
pub type Millis = u64;

/// Width and height in pixels.
pub type Size = (u32, u32);

/// Timing constants of the wait, all in milliseconds from the start of the request unless noted.
/// `#[serde(default)]`: a missing field takes the value of [`Policy::default`].
// This is the Rust copy of the numbers; the JS port carries another one and
// tests/ready_wait_test.js checks that tests/ready_wait_vectors/policy.json equals the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(default)]
pub struct Policy {
    /// Longest wait for any answer, from the start of the request. A cap longer than `hard_ms`
    /// never fires: the hard limit ends the wait first.
    pub cap_ms: Millis,
    /// Time without a further size change that ends the quiet phase (counted from the latest
    /// change, not from the start).
    pub quiet_ms: Millis,
    /// Unconditional end of the wait, from the start of the request.
    pub hard_ms: Millis,
}

impl Default for Policy {
    /// Cap 250 ms, quiet 30 ms, hard limit 1500 ms.
    // The 30 and 1500 equal COMMIT_QUIET_MS and COMMIT_HARD_CAP_MS in
    // assets/gnome/modern/commit.js.
    fn default() -> Self {
        Self {
            cap_ms: 250,
            quiet_ms: 30,
            hard_ms: 1500,
        }
    }
}

/// What the wait is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// The size that was requested.
    pub target: Size,
    /// The window's size when the request was made.
    pub current: Size,
    /// False when the window cannot be resized. A missing field in a script means true.
    // The extension fills it from `window.allows_resize()` (assets/gnome/modern/extension.js).
    #[serde(default = "default_true")]
    pub resizable: bool,
    /// True for a fullscreen window.
    #[serde(default)]
    pub fullscreen: bool,
    /// True for a tiled window.
    #[serde(default)]
    pub tiled: bool,
}

/// serde default for [`Request::resizable`].
fn default_true() -> bool {
    true
}

impl Request {
    /// A request for a resizable window that is neither fullscreen nor tiled.
    pub fn new(target: Size, current: Size) -> Self {
        Self {
            target,
            current,
            resizable: true,
            fullscreen: false,
            tiled: false,
        }
    }
}

/// Why no wait was attempted. Serialises in kebab-case (`not-resizable`). When several apply the
/// order of checking in [`ReadyWait::start`] is not resizable, fullscreen, tiled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SkipReason {
    /// The window does not allow resizing.
    NotResizable,
    /// The window is fullscreen.
    Fullscreen,
    /// The window is tiled.
    Tiled,
}

/// Which limit ended a timed-out wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeoutPhase {
    /// `cap_ms` passed without any answer.
    Cap,
    /// `hard_ms` passed.
    Hard,
}

/// How a wait ended. Serialises as `{"outcome": "reached" | "adjusted" | "timed-out" | "skipped", ...}`,
/// the shape the golden scripts expect (`AdjustedTo` is spelled `adjusted`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Outcome {
    /// The window reached exactly the requested size.
    Reached {
        /// The size, equal to the target.
        size: Size,
        /// Milliseconds from the start of the request.
        waited_ms: Millis,
    },
    /// The window answered with a different size and stayed there for `quiet_ms`.
    #[serde(rename = "adjusted")]
    AdjustedTo {
        /// The size the window settled at.
        size: Size,
        /// Milliseconds from the start of the request to the end of the quiet window (the
        /// deadline, not the time the call arrived).
        waited_ms: Millis,
    },
    /// The cap or the hard limit ended the wait.
    TimedOut {
        /// The last size seen (the starting size if there was no answer).
        last_size: Size,
        /// Milliseconds from the start of the request to the deadline that fired: `cap_ms` for
        /// `Cap`, `hard_ms` for `Hard`.
        waited_ms: Millis,
        /// Which limit fired.
        phase: TimeoutPhase,
    },
    /// No wait was attempted.
    Skipped {
        /// Why.
        reason: SkipReason,
    },
}

/// What the caller does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Keep waiting. Call [`ReadyWait::on_tick`] at `deadline_ms` unless a sample arrives first.
    Wait {
        /// The next time the machine can end the wait by itself, on the caller's clock.
        deadline_ms: Millis,
    },
    /// The wait is over.
    Done(Outcome),
}

/// Where the machine is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No answer yet.
    Exact,
    /// The window answered; `since` is the time of its latest size change.
    Quiet { since: Millis },
    /// Finished.
    Done(Outcome),
}

/// The ready-wait state machine for one size request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadyWait {
    /// Timing constants.
    policy: Policy,
    /// The requested size.
    target: Size,
    /// Time of the request.
    started: Millis,
    /// The latest size seen; starts as the size at the request.
    last: Size,
    /// Current phase.
    phase: Phase,
}

impl ReadyWait {
    /// Starts a wait at time `now`. Returns the machine and its first action: `Done(Skipped)` when
    /// the window is not resizable, fullscreen or tiled (checked in that order, before anything
    /// else), `Done(Reached)` with `waited_ms` 0 when the current size already equals the target,
    /// else `Wait` until the cap (or the hard limit, if that is earlier).
    pub fn start(policy: Policy, request: Request, now: Millis) -> (ReadyWait, Action) {
        let machine = ReadyWait {
            policy,
            target: request.target,
            started: now,
            last: request.current,
            phase: Phase::Exact,
        };
        let skip = if !request.resizable {
            Some(SkipReason::NotResizable)
        } else if request.fullscreen {
            Some(SkipReason::Fullscreen)
        } else if request.tiled {
            Some(SkipReason::Tiled)
        } else {
            None
        };
        if let Some(reason) = skip {
            return machine.finish(Outcome::Skipped { reason });
        }
        if request.current == request.target {
            return machine.finish(Outcome::Reached {
                size: request.target,
                waited_ms: 0,
            });
        }
        let action = machine.waiting();
        (machine, action)
    }

    /// A size sample at time `now`. After the wait is over it returns the same `Done` again.
    ///
    /// Order of checks: a `now` later than the current deadline ends the wait as if the timer had
    /// fired (the sample is not looked at); a sample equal to the target is `Reached`; a sample
    /// different from the latest size is the window's answer (it starts or restarts the quiet
    /// clock); a sample equal to the latest size changes nothing.
    ///
    /// Panics: `now` must not be earlier than the `now` given to [`ReadyWait::start`]. This is
    /// not checked; a sample equal to the target would compute `now - started` on `u64` and
    /// overflow (a panic in a debug build, a wrapped value in a release build).
    pub fn on_sample(&mut self, size: Size, now: Millis) -> Action {
        if let Phase::Done(outcome) = self.phase {
            return Action::Done(outcome);
        }
        if now > self.deadline() {
            return self.expire();
        }
        if size == self.target {
            return self.finish_here(Outcome::Reached {
                size,
                waited_ms: now - self.started,
            });
        }
        if size != self.last {
            self.last = size;
            self.phase = Phase::Quiet { since: now };
        }
        self.waiting()
    }

    /// The caller's timer: `now` was reached with no new sample. Ends the wait when `now` is at or
    /// after the current deadline, else returns `Wait` for that deadline again. After the wait is
    /// over it returns the same `Done` again.
    pub fn on_tick(&mut self, now: Millis) -> Action {
        if let Phase::Done(outcome) = self.phase {
            return Action::Done(outcome);
        }
        if now >= self.deadline() {
            return self.expire();
        }
        self.waiting()
    }

    /// The moment the machine can end the wait without further input: the earlier of the hard
    /// limit and the phase's own limit (cap before an answer, quiet window after one).
    fn deadline(&self) -> Millis {
        let hard = self.started.saturating_add(self.policy.hard_ms);
        let own = match self.phase {
            Phase::Exact => self.started.saturating_add(self.policy.cap_ms),
            Phase::Quiet { since } => since.saturating_add(self.policy.quiet_ms),
            Phase::Done(_) => hard,
        };
        own.min(hard)
    }

    /// `Wait` until the current deadline.
    fn waiting(&self) -> Action {
        Action::Wait {
            deadline_ms: self.deadline(),
        }
    }

    /// Ends the wait because the current deadline passed. The outcome reports the deadline, not
    /// the time of the call. The hard limit wins whenever the deadline reaches it (`>=`): then the
    /// result is `TimedOut` with phase `Hard` in every phase. Otherwise: no answer yet is
    /// `TimedOut` with phase `Cap`, and an answer followed by quiet is `AdjustedTo`.
    fn expire(&mut self) -> Action {
        let deadline = self.deadline();
        let waited_ms = deadline - self.started;
        let hard_fired = deadline >= self.started.saturating_add(self.policy.hard_ms);
        let outcome = match self.phase {
            _ if hard_fired => Outcome::TimedOut {
                last_size: self.last,
                waited_ms,
                phase: TimeoutPhase::Hard,
            },
            Phase::Exact => Outcome::TimedOut {
                last_size: self.last,
                waited_ms,
                phase: TimeoutPhase::Cap,
            },
            _ => Outcome::AdjustedTo {
                size: self.last,
                waited_ms,
            },
        };
        self.finish_here(outcome)
    }

    /// Consumes the machine, marks it `Done(outcome)` and returns it with `Action::Done(outcome)`.
    /// Used by [`ReadyWait::start`], which has no machine to mutate yet.
    fn finish(self, outcome: Outcome) -> (ReadyWait, Action) {
        let mut machine = self;
        let action = machine.finish_here(outcome);
        (machine, action)
    }

    /// Marks the machine `Done(outcome)` in place and returns `Action::Done(outcome)`.
    fn finish_here(&mut self, outcome: Outcome) -> Action {
        self.phase = Phase::Done(outcome);
        Action::Done(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the machine as the golden-script driver does, on a virtual clock: before each event,
    /// timers whose deadline is strictly earlier than the event fire; after the last event the
    /// remaining timers run to completion. Returns the outcome.
    fn run(policy: Policy, request: Request, events: &[(Millis, Size)]) -> Outcome {
        let (mut m, mut action) = ReadyWait::start(policy, request, 0);
        for &(at, size) in events {
            while let Action::Wait { deadline_ms } = action {
                if deadline_ms >= at {
                    break;
                }
                action = m.on_tick(deadline_ms);
            }
            if let Action::Done(o) = action {
                return o;
            }
            action = m.on_sample(size, at);
        }
        loop {
            match action {
                Action::Done(o) => return o,
                Action::Wait { deadline_ms } => action = m.on_tick(deadline_ms),
            }
        }
    }

    const BIG: Size = (800, 600);
    const SMALL: Size = (400, 300);

    /// Skip checks come before the equal-size check: a non-resizable window already at the target
    /// is `Skipped`, not `Reached`, and the order of the three skip reasons is fixed.
    #[test]
    fn test_skip_is_checked_before_equal_size_and_in_a_fixed_order() {
        let mut r = Request::new(SMALL, SMALL);
        r.resizable = false;
        r.fullscreen = true;
        r.tiled = true;
        let (_, a) = ReadyWait::start(Policy::default(), r, 0);
        assert_eq!(
            a,
            Action::Done(Outcome::Skipped {
                reason: SkipReason::NotResizable
            })
        );
        r.resizable = true;
        let (_, a) = ReadyWait::start(Policy::default(), r, 0);
        assert_eq!(
            a,
            Action::Done(Outcome::Skipped {
                reason: SkipReason::Fullscreen
            })
        );
        r.fullscreen = false;
        let (_, a) = ReadyWait::start(Policy::default(), r, 0);
        assert_eq!(
            a,
            Action::Done(Outcome::Skipped {
                reason: SkipReason::Tiled
            })
        );
    }

    /// The first action of a real wait is `Wait` until the cap (250 ms after the start).
    #[test]
    fn test_first_deadline_is_the_cap() {
        let (_, a) = ReadyWait::start(Policy::default(), Request::new(BIG, SMALL), 1000);
        assert_eq!(a, Action::Wait { deadline_ms: 1250 });
    }

    /// A sample equal to the size at the request is not an answer: the cap clock is untouched and
    /// the wait still times out at the cap.
    #[test]
    fn test_unchanged_sample_is_not_an_answer() {
        let o = run(
            Policy::default(),
            Request::new(BIG, SMALL),
            &[(5, SMALL), (60, SMALL)],
        );
        assert_eq!(
            o,
            Outcome::TimedOut {
                last_size: SMALL,
                waited_ms: 250,
                phase: TimeoutPhase::Cap
            }
        );
    }

    /// The machine keeps answering `Done` with the same outcome after it finished, whatever the
    /// input, so a caller that races its own timer is safe.
    #[test]
    fn test_done_is_sticky() {
        let (mut m, _) = ReadyWait::start(Policy::default(), Request::new(BIG, SMALL), 0);
        let first = m.on_sample(BIG, 10);
        assert_eq!(
            first,
            Action::Done(Outcome::Reached {
                size: BIG,
                waited_ms: 10
            })
        );
        assert_eq!(m.on_sample((1, 1), 20), first);
        assert_eq!(m.on_tick(5000), first);
    }

    /// A sample that arrives after the cap without any tick in between is handled as if the timer
    /// had fired on time: `TimedOut` at 250 ms, not a late `Reached`.
    #[test]
    fn test_late_sample_does_not_beat_an_expired_cap() {
        let (mut m, _) = ReadyWait::start(Policy::default(), Request::new(BIG, SMALL), 0);
        assert_eq!(
            m.on_sample(BIG, 400),
            Action::Done(Outcome::TimedOut {
                last_size: SMALL,
                waited_ms: 250,
                phase: TimeoutPhase::Cap
            })
        );
    }

    /// When the quiet deadline lands on the hard limit the hard limit wins. A window that changes
    /// size every 20 ms from 10 ms to 1470 ms (never quiet for 30 ms) has its last change at
    /// 1470 ms, whose quiet deadline is exactly 1500 ms: the outcome is `TimedOut{Hard}`, not
    /// `AdjustedTo`. The 74th change (index 73, odd) is the second of the two alternating sizes.
    #[test]
    fn test_hard_limit_wins_a_tie_with_the_quiet_deadline() {
        let events: Vec<(Millis, Size)> = (0..74)
            .map(|k| {
                (
                    10 + 20 * k,
                    if k % 2 == 0 { (410, 310) } else { (420, 320) },
                )
            })
            .collect();
        assert_eq!(events.last().unwrap().0, 1470);
        let o = run(Policy::default(), Request::new(BIG, SMALL), &events);
        assert_eq!(
            o,
            Outcome::TimedOut {
                last_size: (420, 320),
                waited_ms: 1500,
                phase: TimeoutPhase::Hard
            }
        );
    }

    /// A cap larger than the hard limit never gets to fire: a hung app is ended by the hard limit.
    #[test]
    fn test_cap_beyond_hard_limit_reports_the_hard_limit() {
        let p = Policy {
            cap_ms: 5000,
            ..Policy::default()
        };
        let o = run(p, Request::new(BIG, SMALL), &[]);
        assert_eq!(
            o,
            Outcome::TimedOut {
                last_size: SMALL,
                waited_ms: 1500,
                phase: TimeoutPhase::Hard
            }
        );
    }

    /// The start time is arbitrary: the same script started at 10 000 ms gives the same
    /// `waited_ms` (it is relative to the start).
    #[test]
    fn test_waited_is_relative_to_the_start() {
        let (mut m, a) = ReadyWait::start(Policy::default(), Request::new(BIG, SMALL), 10_000);
        assert_eq!(
            a,
            Action::Wait {
                deadline_ms: 10_250
            }
        );
        assert_eq!(
            m.on_sample(BIG, 10_040),
            Action::Done(Outcome::Reached {
                size: BIG,
                waited_ms: 40
            })
        );
    }

    /// The outcome serialises in the shape the golden scripts and the later JSON use.
    #[test]
    fn test_outcome_serialises_as_a_tagged_object() {
        let v = serde_json::to_value(Outcome::AdjustedTo {
            size: (300, 200),
            waited_ms: 42,
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"outcome":"adjusted","size":[300,200],"waited_ms":42})
        );
        let v = serde_json::to_value(Outcome::TimedOut {
            last_size: SMALL,
            waited_ms: 150,
            phase: TimeoutPhase::Cap,
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"outcome":"timed-out","last_size":[400,300],"waited_ms":150,"phase":"cap"})
        );
        let v = serde_json::to_value(Outcome::Skipped {
            reason: SkipReason::NotResizable,
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"outcome":"skipped","reason":"not-resizable"})
        );
    }
}
