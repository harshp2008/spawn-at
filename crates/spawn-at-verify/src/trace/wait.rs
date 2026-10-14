//! The wait rule each run used, for A/B runs inside one login (`docs/DESIGN_READY_WAIT.md`
//! section 10): what `run --wait-strategy / --wait-cap-ms` asked for, and what the extension says
//! it did, read from the run's own log slice.

use super::parser::TraceAnalyser;
use serde::{Deserialize, Serialize};

/// What `run` was asked to pass to every `spawn` (`--wait-strategy`, `--wait-cap-ms`). The values
/// are kept as typed: `spawn-at` is the one that judges them, so a typo shows up as its warning
/// (which fails a spec that forbids warnings) and not as a silent default arm.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitRequest {
    /// Value of `--wait-strategy`, if given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    /// Value of `--wait-cap-ms`, if given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_ms: Option<String>,
}

impl WaitRequest {
    /// True when neither option was given.
    pub fn is_empty(&self) -> bool {
        self.strategy.is_none() && self.cap_ms.is_none()
    }

    /// Appends the options to a `spawn` argument list. An option the spec already spells out in
    /// its own action arguments is left to the spec (a repeated flag would be a parse error).
    pub fn append_to(&self, args: &mut Vec<String>) {
        if let Some(strategy) = &self.strategy {
            if !args.iter().any(|a| a == "--wait-strategy") {
                args.push("--wait-strategy".to_string());
                args.push(strategy.clone());
            }
        }
        if let Some(cap_ms) = &self.cap_ms {
            if !args.iter().any(|a| a == "--wait-cap-ms") {
                args.push("--wait-cap-ms".to_string());
                args.push(cap_ms.clone());
            }
        }
    }

    /// True when the extension, if it honours the override, must have written a
    /// `READY_WAIT_OVERRIDE` line: `auto` alone sends nothing.
    fn expects_override_line(&self) -> bool {
        self.cap_ms.is_some()
            || matches!(
                self.strategy.as_deref(),
                Some(
                    "legacy" | "ready-wait" | "x11-hold-damage" | "x11-premap" | "x11-pixel-probe"
                )
            )
    }
}

/// The wait rule of one run; flattened into the test result and the report JSON, where the keys
/// are `wait_strategy`, `wait_cap_ms`, `wait_requested` and `wait_notes`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WaitInfo {
    /// The rule that waited, from the window's `READY_WAIT_START`: `ready-wait`, `legacy` or the experimental `x11-hold-damage`.
    /// Absent when the slice holds no such line (no trace, or an extension without ready-wait).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_strategy: Option<String>,
    /// The cap of the ready-wait in milliseconds as logged (`cap=`); absent for `legacy`, which
    /// has no cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_cap_ms: Option<u64>,
    /// What `run` asked for; absent when it asked for nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_requested: Option<WaitRequest>,
    /// Things worth knowing about the override: valid fields that had no effect, fields the
    /// extension dropped, or a requested override the extension never reported.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wait_notes: Vec<String>,
}

/// Value of `key=` in a log payload, up to the next space.
fn field<'a>(extra: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("{key}=");
    let start = if extra.starts_with(&marker) {
        0
    } else {
        extra.find(&format!(" {marker}"))? + 1
    };
    let rest = &extra[start + marker.len()..];
    Some(rest.split(' ').next().unwrap_or(rest))
}

impl WaitInfo {
    /// Reads the wait rule of `window` (or of the only window in the slice when `None`) from a
    /// log slice, and attaches what `requested` asked for.
    pub fn from_slice(slice: Option<&str>, window: Option<u64>, requested: &WaitRequest) -> Self {
        let mut info = WaitInfo {
            wait_requested: (!requested.is_empty()).then(|| requested.clone()),
            ..Default::default()
        };
        let Some(slice) = slice else {
            return info;
        };
        let mut analyser = TraceAnalyser::new();
        if let Some(id) = window {
            analyser = analyser.with_target_window_id(id);
        }
        let events = analyser.parse_events(slice);

        if let Some(start) = events.iter().find(|e| e.tag == "READY_WAIT_START") {
            info.wait_strategy = field(&start.extra, "strategy").map(str::to_string);
            if matches!(
                info.wait_strategy.as_deref(),
                Some("ready-wait" | "x11-hold-damage" | "x11-premap")
            ) {
                info.wait_cap_ms = field(&start.extra, "cap").and_then(|v| v.parse().ok());
            }
        }

        match events.iter().find(|e| e.tag == "READY_WAIT_OVERRIDE") {
            Some(line) => {
                if let Some(unused) = field(&line.extra, "unused").filter(|v| *v != "none") {
                    info.wait_notes
                        .push(format!("override had no effect: {unused}"));
                }
                // `ignored=` is last and may hold spaces: take everything after it.
                if let Some(at) = line.extra.find("ignored=") {
                    let ignored = &line.extra[at + "ignored=".len()..];
                    if ignored != "none" {
                        info.wait_notes
                            .push(format!("extension ignored: {ignored}"));
                    }
                }
            }
            None if requested.expects_override_line() && info.wait_strategy.is_some() => {
                info.wait_notes.push(
                    "override requested but the extension logged no READY_WAIT_OVERRIDE (extension \
                     without the override, or values spawn-at dropped)"
                        .to_string(),
                );
            }
            None => {}
        }
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: &str = r#"win=[id=7 pid=70 class="c" app="" type=NORMAL proto=Wayland]"#;

    fn line(ms: f64, tag: &str, rest: &str) -> String {
        format!("{ms}ms | {tag} {WIN} {rest}\n")
    }

    fn ready_wait_start(cap: u64) -> String {
        line(
            10.0,
            "READY_WAIT_START",
            &format!("strategy=ready-wait kind=NativeWayland target=(800x600) current=(400x300) cap={cap} quiet=30 hard=1500"),
        )
    }

    fn legacy_start() -> String {
        line(
            10.0,
            "READY_WAIT_START",
            "strategy=legacy kind=NativeWayland why=override-legacy timeoutMs=300",
        )
    }

    fn requested(strategy: Option<&str>, cap: Option<&str>) -> WaitRequest {
        WaitRequest {
            strategy: strategy.map(String::from),
            cap_ms: cap.map(String::from),
        }
    }

    #[test]
    fn test_default_run_records_the_effective_rule_and_no_request() {
        let slice = ready_wait_start(150);
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &WaitRequest::default());
        assert_eq!(info.wait_strategy.as_deref(), Some("ready-wait"));
        assert_eq!(info.wait_cap_ms, Some(150));
        assert_eq!(info.wait_requested, None);
        assert!(info.wait_notes.is_empty());
    }

    #[test]
    fn test_default_run_serialises_without_request_or_notes() {
        let slice = ready_wait_start(150);
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &WaitRequest::default());
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"wait_strategy": "ready-wait", "wait_cap_ms": 150})
        );
    }

    #[test]
    fn test_override_run_records_request_effective_cap_and_no_notes() {
        let slice = format!(
            "{}{}",
            line(
                9.0,
                "READY_WAIT_OVERRIDE",
                "strategy=none cap_ms=60 unused=none ignored=none"
            ),
            ready_wait_start(60)
        );
        let req = requested(None, Some("60"));
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &req);
        assert_eq!(info.wait_strategy.as_deref(), Some("ready-wait"));
        assert_eq!(info.wait_cap_ms, Some(60));
        assert_eq!(info.wait_requested, Some(req));
        assert!(info.wait_notes.is_empty(), "{:?}", info.wait_notes);
    }

    #[test]
    fn test_legacy_run_has_no_cap() {
        let slice = format!(
            "{}{}",
            line(
                9.0,
                "READY_WAIT_OVERRIDE",
                "strategy=legacy cap_ms=none unused=none ignored=none"
            ),
            legacy_start()
        );
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &requested(Some("legacy"), None));
        assert_eq!(info.wait_strategy.as_deref(), Some("legacy"));
        assert_eq!(info.wait_cap_ms, None);
        let json = serde_json::to_value(&info).unwrap();
        assert!(json.get("wait_cap_ms").is_none(), "{json}");
        assert_eq!(
            json["wait_requested"],
            serde_json::json!({"strategy": "legacy"})
        );
    }

    #[test]
    fn test_unused_and_ignored_parts_become_notes() {
        let slice = format!(
            "{}{}",
            line(
                9.0,
                "READY_WAIT_OVERRIDE",
                "strategy=legacy cap_ms=60 unused=cap_ms(legacy-has-no-cap) ignored=strategy=\"x y\";cap_ms=0"
            ),
            legacy_start()
        );
        let info = WaitInfo::from_slice(
            Some(&slice),
            Some(7),
            &requested(Some("legacy"), Some("60")),
        );
        assert_eq!(
            info.wait_notes,
            vec![
                "override had no effect: cap_ms(legacy-has-no-cap)".to_string(),
                "extension ignored: strategy=\"x y\";cap_ms=0".to_string(),
            ]
        );
    }

    #[test]
    fn test_requested_override_without_an_extension_line_is_flagged() {
        // An extension without the override ignores the fields silently; the arm would look like
        // an A/B run and be two default runs. The note is how that shows up.
        let slice = ready_wait_start(150);
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &requested(Some("legacy"), None));
        assert_eq!(info.wait_strategy.as_deref(), Some("ready-wait"));
        assert_eq!(info.wait_notes.len(), 1);
        assert!(info.wait_notes[0].contains("no READY_WAIT_OVERRIDE"));
        // `auto` alone sends nothing, so nothing is missing.
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &requested(Some("auto"), None));
        assert!(info.wait_notes.is_empty());
    }

    #[test]
    fn test_no_slice_or_no_start_line_records_nothing_about_the_extension() {
        let info = WaitInfo::from_slice(None, Some(7), &requested(Some("legacy"), Some("60")));
        assert_eq!(info.wait_strategy, None);
        assert_eq!(info.wait_cap_ms, None);
        assert!(info.wait_notes.is_empty());
        assert!(info.wait_requested.is_some());
        let other = line(10.0, "CLOAK", "opacity=0 frame=(0,0,10x10)");
        let info = WaitInfo::from_slice(Some(&other), Some(7), &WaitRequest::default());
        assert_eq!(info, WaitInfo::default());
    }

    #[test]
    fn test_another_windows_lines_are_not_read() {
        let slice = format!(
            "{}{}",
            ready_wait_start(150).replace("id=7", "id=8"),
            line(
                11.0,
                "READY_WAIT_START",
                "strategy=ready-wait kind=NativeWayland target=(1x1) current=(2x2) cap=90 quiet=30 hard=1500"
            )
        );
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &WaitRequest::default());
        assert_eq!(info.wait_cap_ms, Some(90));
    }

    #[test]
    fn test_append_to_adds_the_options_once_and_leaves_spec_flags_alone() {
        let mut args = vec!["spawn".to_string()];
        requested(Some("legacy"), Some("90")).append_to(&mut args);
        assert_eq!(
            args,
            ["spawn", "--wait-strategy", "legacy", "--wait-cap-ms", "90"]
        );
        let mut spelled = vec![
            "spawn".to_string(),
            "--wait-cap-ms".to_string(),
            "40".to_string(),
        ];
        requested(Some("legacy"), Some("90")).append_to(&mut spelled);
        assert_eq!(
            spelled,
            ["spawn", "--wait-cap-ms", "40", "--wait-strategy", "legacy"]
        );
        let mut none = vec!["spawn".to_string()];
        WaitRequest::default().append_to(&mut none);
        assert_eq!(none, ["spawn"]);
    }

    /// A run of the experimental `x11-hold-damage` wait: the strategy and its cap are recorded, the
    /// new `X11_HOLD_DAMAGED` and `x11-*` reason lines do not disturb the reading, and a requested
    /// override whose line is missing is noted like it is for `legacy` and `ready-wait`.
    #[test]
    fn test_x11_hold_damage_run_is_recorded_and_tolerated() {
        let slice = format!(
            "{}{}{}{}{}",
            line(
                9.0,
                "READY_WAIT_OVERRIDE",
                "strategy=x11-hold-damage cap_ms=none unused=none ignored=none"
            ),
            line(
                10.0,
                "READY_WAIT_START",
                "strategy=x11-hold-damage kind=XWayland target=(800x600) cap=250 hard=1500 reveal_on_damaged=2"
            ),
            line(14.0, "X11_HOLD_DAMAGED", "n=1 ignored=true since_start_ms=4.0"),
            line(40.0, "X11_HOLD_DAMAGED", "n=2 ignored=false since_start_ms=30.0"),
            line(
                40.1,
                "READY_WAIT_RESOLVED",
                "strategy=x11-hold-damage outcome=reached elapsed_ms=30.1 waited_ms=30 reason=x11-second-damaged damaged_seen=2 finalFrame=(800x600) target=(800x600)"
            ),
        );
        let req = requested(Some("x11-hold-damage"), None);
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &req);
        assert_eq!(info.wait_strategy.as_deref(), Some("x11-hold-damage"));
        assert_eq!(info.wait_cap_ms, Some(250));
        assert!(info.wait_notes.is_empty(), "{:?}", info.wait_notes);

        let without_override = line(
            10.0,
            "READY_WAIT_START",
            "strategy=legacy kind=XWayland why=not-native-wayland timeoutMs=300",
        );
        let info = WaitInfo::from_slice(Some(&without_override), Some(7), &req);
        assert_eq!(info.wait_notes.len(), 1, "{:?}", info.wait_notes);
    }

    /// A run of the experimental `x11-premap` wait records its strategy and cap like the other
    /// experiments, and a missing override line is noted for it and for the observe-only probe.
    #[test]
    fn test_x11_premap_run_is_recorded_and_tolerated() {
        let slice = format!(
            "{}{}{}",
            line(
                9.0,
                "READY_WAIT_OVERRIDE",
                "strategy=x11-premap cap_ms=none unused=none ignored=none"
            ),
            line(
                10.0,
                "READY_WAIT_START",
                "strategy=x11-premap kind=XWayland target=(800x600) cap=250 hard=1500 resized_ms_before_wait=21.0 first_frame_already=false"
            ),
            line(
                30.0,
                "READY_WAIT_RESOLVED",
                "strategy=x11-premap outcome=reached elapsed_ms=31.0 waited_ms=30 reason=x11-premap-first-frame first_frames=1 finalFrame=(800x600) target=(800x600)"
            ),
        );
        let req = requested(Some("x11-premap"), None);
        let info = WaitInfo::from_slice(Some(&slice), Some(7), &req);
        assert_eq!(info.wait_strategy.as_deref(), Some("x11-premap"));
        assert_eq!(info.wait_cap_ms, Some(250));
        assert!(info.wait_notes.is_empty(), "{:?}", info.wait_notes);
        let legacy = line(
            10.0,
            "READY_WAIT_START",
            "strategy=legacy kind=XWayland why=not-native-wayland timeoutMs=300",
        );
        for name in ["x11-premap", "x11-pixel-probe"] {
            let info = WaitInfo::from_slice(Some(&legacy), Some(7), &requested(Some(name), None));
            assert_eq!(info.wait_notes.len(), 1, "{name}: {:?}", info.wait_notes);
        }
    }
}
