//! `[expect.resize]`: the ready-wait outcome a spec declares, read from the CLI's `--json`.

use super::validation::{printed_resize, validate_resize};
use crate::schema::TestDefinition;

/// A spec with the given `[expect.resize]` body.
fn spec(resize_body: &str) -> TestDefinition {
    toml::from_str(&format!(
        r#"
id = "hostile/t"
category = "hostile"
title = "t"
description = "d"
kind = "auto"
required = false
tags = []
[requires]
session = ["wayland"]
backend = ["gnome"]
gnome = "any"
monitors_min = 1
scale = "any"
apps = []
[action]
command = "spawn"
args = []
[expect]
exit_code = 0
[expect.resize]
{resize_body}
"#
    ))
    .unwrap()
}

/// The printed JSON of a spawn with a `resize` object.
fn printed(outcome: &str, reason: &str, waited: u32, w: u32, h: u32) -> String {
    format!(
        r#"{{"window_id":7,"x":1,"y":2,"w":{w},"h":{h},"resize":{{"outcome":"{outcome}","requested":{{"w":800,"h":600}},"final":{{"w":{w},"h":{h}}},"waited_ms":{waited},"reason":"{reason}"}}}}"#
    )
}

/// The section parses with its fields and defaults (2 px tolerance, optional fields absent), and a
/// spec without the section has none.
#[test]
fn test_spec_parsing() {
    let t = spec("outcome = \"timed-out\"\nreason = \"cap\"\nwaited_ms_min = 240\nwaited_ms_max = 330\nfinal_size = [400, 300]");
    let r = t.expect.resize.unwrap();
    assert_eq!(
        (r.outcome.as_str(), r.reason.as_deref()),
        ("timed-out", Some("cap"))
    );
    assert_eq!((r.waited_ms_min, r.waited_ms_max), (Some(240), Some(330)));
    assert_eq!(r.final_size, Some(vec![400, 300]));
    assert_eq!(r.tolerance_px, 2);
    let bare = spec("outcome = \"reached\"").expect.resize.unwrap();
    assert_eq!(
        (bare.reason, bare.waited_ms_min, bare.final_size),
        (None, None, None)
    );
    let none: TestDefinition = toml::from_str(
        "id=\"a/b\"\ncategory=\"placement\"\ntitle=\"t\"\ndescription=\"d\"\nkind=\"auto\"\nrequired=true\ntags=[]\n[requires]\nsession=[\"any\"]\nbackend=[\"any\"]\ngnome=\"any\"\nmonitors_min=1\nscale=\"any\"\napps=[]\n[action]\ncommand=\"spawn\"\nargs=[]\n",
    )
    .unwrap();
    assert!(none.expect.resize.is_none());
}

/// The `resize` object is found in the printed JSON; null, absent and non-JSON give none.
#[test]
fn test_printed_resize() {
    let r = printed_resize(&printed("adjusted", "quiet", 70, 777, 574)).unwrap();
    assert_eq!(
        (r.outcome.as_str(), r.reason.as_str(), r.waited_ms),
        ("adjusted", "quiet", 70)
    );
    assert_eq!((r.final_size.w, r.final_size.h), (777, 574));
    assert!(printed_resize(r#"{"window_id":7,"resize":null}"#).is_none());
    assert!(printed_resize(r#"{"window_id":7}"#).is_none());
    assert!(printed_resize("[spawn-at] INFO: mapped").is_none());
}

/// A run that matches every declared field passes; no section means nothing is judged.
#[test]
fn test_match_passes() {
    let t = spec("outcome = \"timed-out\"\nreason = \"cap\"\nwaited_ms_min = 240\nwaited_ms_max = 330\nfinal_size = [400, 300]");
    assert_eq!(
        validate_resize(&t, &printed("timed-out", "cap", 251, 400, 300), false),
        Ok(())
    );
    let bare = spec("outcome = \"reached\"");
    assert_eq!(
        validate_resize(&bare, &printed("reached", "at-target", 5, 1, 1), false),
        Ok(())
    );
}

/// Each mismatch is named with the value seen and the value wanted, all in one message.
#[test]
fn test_mismatches_are_named() {
    let t = spec("outcome = \"reached\"\nreason = \"at-target\"\nwaited_ms_min = 90\nwaited_ms_max = 249\nfinal_size = [800, 600]");
    let e = validate_resize(&t, &printed("timed-out", "cap", 251, 400, 300), false).unwrap_err();
    for part in [
        "outcome timed-out (expected reached)",
        "reason cap (expected at-target)",
        "waited_ms 251 (expected at most 249)",
        "final size 400x300 (expected 800x600 within 2 px)",
    ] {
        assert!(e.contains(part), "{e}");
    }
    let e = validate_resize(&t, &printed("reached", "at-target", 40, 800, 600), false).unwrap_err();
    assert!(e.contains("waited_ms 40 (expected at least 90)"), "{e}");
}

/// The final size is judged within the tolerance: 2 px passes by default, 3 px fails; a custom
/// tolerance is honoured.
#[test]
fn test_final_size_tolerance() {
    let t = spec("outcome = \"adjusted\"\nfinal_size = [777, 574]");
    assert!(validate_resize(&t, &printed("adjusted", "quiet", 60, 779, 572), false).is_ok());
    assert!(validate_resize(&t, &printed("adjusted", "quiet", 60, 780, 574), false).is_err());
    let wide = spec("outcome = \"adjusted\"\nfinal_size = [777, 574]\ntolerance_px = 5");
    assert!(validate_resize(&wide, &printed("adjusted", "quiet", 60, 780, 574), false).is_ok());
}

/// A legacy-rule run reports no outcome and is not judged; a ready-wait run that printed none fails.
#[test]
fn test_legacy_is_not_judged_and_a_missing_outcome_fails() {
    let t = spec("outcome = \"reached\"");
    assert_eq!(
        validate_resize(&t, r#"{"window_id":7,"resize":null}"#, true),
        Ok(())
    );
    let e = validate_resize(&t, r#"{"window_id":7,"resize":null}"#, false).unwrap_err();
    assert!(e.contains("printed none"), "{e}");
}
