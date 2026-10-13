//! # Per-spawn override of the resize wait
//!
//! A developer switch for same-login A/B runs (`docs/DESIGN_READY_WAIT.md` section 10): which
//! wait rule one spawn uses, and the cap of the ready-wait. It is carried in the instruction that
//! arms the extension, so it applies to that spawn only and the next spawn starts from the
//! defaults again. Without an override nothing is serialised and the instruction bytes are the
//! ones of a build that does not know this module.

use crate::ready_wait::Policy;
use serde::{Deserialize, Serialize};

/// Which rule waits for the resized window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WaitStrategy {
    /// The extension decides: the ready-wait machine for a native Wayland window with a size
    /// request, the original quiet rule for everything else.
    #[default]
    Auto,
    /// The original 30 ms quiet rule, also for a native Wayland window.
    Legacy,
    /// The ready-wait machine where it applies (native Wayland, size request). Any other window
    /// keeps the original rule, as under `Auto`.
    ReadyWait,
    /// Experimental, XWayland and X11 windows only (`docs/DESIGN_READY_WAIT.md` section 13): keep
    /// the window cloaked, ignore the first `damaged` after the size request, reveal on the next
    /// one or at the cap. A native Wayland window behaves as under `Auto`. Selectable only by the
    /// hidden `--wait-strategy` flag, never by the config file, never the default.
    X11HoldDamage,
}

impl WaitStrategy {
    /// True for a value that exists for experiments: the config file does not accept it.
    pub fn is_experimental(self) -> bool {
        matches!(self, WaitStrategy::X11HoldDamage)
    }

    /// The spelling used on the command line, in the instruction JSON and in the logs.
    pub fn as_str(self) -> &'static str {
        match self {
            WaitStrategy::Auto => "auto",
            WaitStrategy::Legacy => "legacy",
            WaitStrategy::ReadyWait => "ready-wait",
            WaitStrategy::X11HoldDamage => "x11-hold-damage",
        }
    }

    /// Exact, lower-case match of [`WaitStrategy::as_str`]; anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "auto" => Some(WaitStrategy::Auto),
            "legacy" => Some(WaitStrategy::Legacy),
            "ready-wait" => Some(WaitStrategy::ReadyWait),
            "x11-hold-damage" => Some(WaitStrategy::X11HoldDamage),
            _ => None,
        }
    }
}

/// The override of one spawn. `Default` is "no override".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WaitOverride {
    /// The wait rule; `Auto` leaves the choice to the extension.
    pub strategy: WaitStrategy,
    /// Cap of the ready-wait in milliseconds, `None` for the policy default (250).
    pub cap_ms: Option<u32>,
}

impl WaitOverride {
    /// True when nothing is overridden; such a value is never serialised.
    pub fn is_default(&self) -> bool {
        *self == WaitOverride::default()
    }

    /// Reads the raw values of `--wait-strategy` and `--wait-cap-ms`. A value that is not valid
    /// is dropped, the other one is still used, and one sentence per dropped value is returned so
    /// the caller can say so. Valid: a strategy spelled `auto`, `legacy` or `ready-wait`; a cap
    /// written as decimal digits between 1 and the hard limit of [`Policy`] (a longer cap could
    /// not end the wait before the hard limit does).
    pub fn from_raw(strategy: Option<&str>, cap_ms: Option<&str>) -> (Self, Vec<String>) {
        let mut out = WaitOverride::default();
        let mut problems = Vec::new();
        if let Some(text) = strategy {
            match WaitStrategy::parse(text) {
                Some(parsed) => out.strategy = parsed,
                None => problems.push(format!(
                    "ignoring --wait-strategy '{text}' (expected auto, legacy or ready-wait)"
                )),
            }
        }
        if let Some(text) = cap_ms {
            let hard = Policy::default().hard_ms;
            let digits = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
            match text.parse::<u32>() {
                Ok(n) if digits && n >= 1 && u64::from(n) <= hard => out.cap_ms = Some(n),
                _ => problems.push(format!(
                    "ignoring --wait-cap-ms '{text}' (expected whole milliseconds from 1 to {hard})"
                )),
            }
        }
        (out, problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_no_override() {
        assert!(WaitOverride::default().is_default());
        assert_eq!(WaitOverride::default().strategy, WaitStrategy::Auto);
        assert_eq!(WaitOverride::default().cap_ms, None);
    }

    #[test]
    fn test_no_raw_values_is_the_default_without_problems() {
        let (o, problems) = WaitOverride::from_raw(None, None);
        assert!(o.is_default());
        assert!(problems.is_empty());
    }

    #[test]
    fn test_auto_spelled_out_is_still_the_default() {
        // `--wait-strategy auto` must send nothing, so the arm instruction stays byte-identical.
        let (o, problems) = WaitOverride::from_raw(Some("auto"), None);
        assert!(o.is_default());
        assert!(problems.is_empty());
    }

    #[test]
    fn test_valid_values_are_taken() {
        let (o, problems) = WaitOverride::from_raw(Some("legacy"), Some("200"));
        assert_eq!(o.strategy, WaitStrategy::Legacy);
        assert_eq!(o.cap_ms, Some(200));
        assert!(problems.is_empty());
        let (o, _) = WaitOverride::from_raw(Some("ready-wait"), Some("1500"));
        assert_eq!(o.strategy, WaitStrategy::ReadyWait);
        assert_eq!(o.cap_ms, Some(1500));
        let (o, _) = WaitOverride::from_raw(None, Some("1"));
        assert_eq!(o.cap_ms, Some(1));
    }

    #[test]
    fn test_invalid_strategy_is_dropped_and_named() {
        for bad in [
            "",
            "Legacy",
            "ready_wait",
            "readywait",
            " legacy",
            "legacy ",
            "fast",
        ] {
            let (o, problems) = WaitOverride::from_raw(Some(bad), None);
            assert!(o.is_default(), "'{bad}' must not select a strategy");
            assert_eq!(problems.len(), 1, "'{bad}'");
            assert!(problems[0].contains("--wait-strategy"), "{}", problems[0]);
            assert!(problems[0].contains(&format!("'{bad}'")), "{}", problems[0]);
        }
    }

    #[test]
    fn test_invalid_cap_is_dropped_and_named() {
        // 0 (would end the wait before it starts), above the 1500 ms hard limit, a sign, a
        // fraction, text, empty and u32 overflow.
        for bad in ["0", "1501", "-5", "+5", "1.5", "abc", "", "99999999999"] {
            let (o, problems) = WaitOverride::from_raw(None, Some(bad));
            assert_eq!(o.cap_ms, None, "'{bad}' must not set a cap");
            assert_eq!(problems.len(), 1, "'{bad}'");
            assert!(problems[0].contains("--wait-cap-ms"), "{}", problems[0]);
            assert!(problems[0].contains("1 to 1500"), "{}", problems[0]);
        }
    }

    #[test]
    fn test_one_bad_value_does_not_drop_the_good_one() {
        let (o, problems) = WaitOverride::from_raw(Some("legacy"), Some("0"));
        assert_eq!(o.strategy, WaitStrategy::Legacy);
        assert_eq!(o.cap_ms, None);
        assert_eq!(problems.len(), 1);
        let (o, problems) = WaitOverride::from_raw(Some("nope"), Some("120"));
        assert_eq!(o.strategy, WaitStrategy::Auto);
        assert_eq!(o.cap_ms, Some(120));
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn test_strategy_spelling_round_trips() {
        for s in [
            WaitStrategy::Auto,
            WaitStrategy::Legacy,
            WaitStrategy::ReadyWait,
            WaitStrategy::X11HoldDamage,
        ] {
            assert_eq!(WaitStrategy::parse(s.as_str()), Some(s));
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.as_str())
            );
        }
    }

    /// The experimental value is taken from the flag, is not the default, and is the only
    /// experimental one; the three public values are not.
    #[test]
    fn test_x11_hold_damage_is_an_opt_in_flag_value() {
        let (o, problems) = WaitOverride::from_raw(Some("x11-hold-damage"), Some("400"));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(o.strategy, WaitStrategy::X11HoldDamage);
        assert_eq!(o.cap_ms, Some(400));
        assert!(!o.is_default());
        assert!(WaitStrategy::X11HoldDamage.is_experimental());
        for s in [
            WaitStrategy::Auto,
            WaitStrategy::Legacy,
            WaitStrategy::ReadyWait,
        ] {
            assert!(!s.is_experimental());
        }
        assert_eq!(WaitOverride::default().strategy, WaitStrategy::Auto);
    }

    #[test]
    fn test_cap_limit_is_the_policy_hard_limit() {
        let hard = Policy::default().hard_ms;
        assert_eq!(hard, 1500);
        assert!(WaitOverride::from_raw(None, Some(&hard.to_string()))
            .1
            .is_empty());
        assert!(!WaitOverride::from_raw(None, Some(&(hard + 1).to_string()))
            .1
            .is_empty());
    }
}
