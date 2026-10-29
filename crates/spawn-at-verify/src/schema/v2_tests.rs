//! Tests of the version 2 load-time rules (`docs/DESIGN_RUNNER_SETUP.md` section 2.3). Every
//! rule that keeps a spec from naming a window the runner did not record has a failing example
//! here: they are the first half of the safety model.

use super::types::TestDefinition;
use super::v2::{placeholders, validate, whole_placeholder, Placeholder};

const HEAD: &str = r#"
id = "transforms/example"
category = "transforms"
title = "example"
description = "example"
tags = ["moves-focus"]
kind = "auto"
"#;

/// Parses `head + body` the way the loader does and applies the version 2 rules.
fn load(version_line: &str, body: &str) -> Result<TestDefinition, String> {
    let text = format!("{version_line}\n{HEAD}\n{body}");
    let def: TestDefinition = toml::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    validate(&def)?;
    Ok(def)
}

fn v2(body: &str) -> Result<TestDefinition, String> {
    load("schema_version = 2", body)
}

/// A minimal valid version 2 body: one window, one resize by its recorded id.
const GOOD: &str = r#"
[[setup]]
name = "w"
args = ["--width", "400", "--height", "300"]

[action]
command = "transform"
args = ["--id", "${w}", "--size", "600", "450", "--no-focus"]
"#;

#[track_caller]
fn assert_err(body: &str, needle: &str) {
    match v2(body) {
        Ok(_) => panic!("expected an error containing '{needle}', but the spec loaded"),
        Err(e) => assert!(e.contains(needle), "error '{e}' lacks '{needle}'"),
    }
}

/// An action body whose only variable part is the argument list.
fn action(command: &str, args: &str) -> String {
    format!(
        "[[setup]]\nname = \"w\"\n\n[[setup]]\nname = \"v\"\n\n[action]\ncommand = \"{command}\"\nargs = [{args}]\n"
    )
}

#[test]
fn test_a_valid_version_2_spec_loads() {
    let d = v2(GOOD).unwrap();
    assert_eq!(d.v2.schema_version, 2);
    assert_eq!(d.v2.setup.len(), 1);
    assert_eq!(d.v2.setup[0].name, "w");
}

// ---- version rules ----

#[test]
fn test_version_1_spec_cannot_use_version_2_keys() {
    for body in [
        GOOD.to_string(),
        "[action]\ncommand = \"query\"\nargs = [\"layout\"]\n[[then]]\ncommand = \"query\"\nargs = [\"windows\"]\n".to_string(),
        "[action]\ncommand = \"query\"\nargs = [\"layout\"]\n[[check]]\nwindow = \"w\"\nstate = \"present\"\n".to_string(),
        "[action]\ncommand = \"query\"\nargs = [\"layout\"]\n[expect]\nstdout_present = [\"x\"]\n".to_string(),
        "[action]\ncommand = \"query\"\nargs = [\"layout\"]\n[expect]\nstdout_lists_window = \"w\"\n".to_string(),
    ] {
        let r = load("", &body);
        let e = r.expect_err("a version 1 spec must not use version 2 keys");
        assert!(e.contains("require schema_version = 2"), "{e}");
    }
}

#[test]
fn test_explicit_version_1_is_the_same_as_none() {
    let body = "[action]\ncommand = \"query\"\nargs = [\"layout\"]\n";
    assert!(load("schema_version = 1", body).is_ok());
    assert!(load("", body).is_ok());
}

#[test]
fn test_unknown_schema_versions_are_refused() {
    for v in ["0", "3", "99"] {
        let e = load(&format!("schema_version = {v}"), GOOD).unwrap_err();
        assert!(e.contains("is not supported"), "{e}");
    }
}

#[test]
fn test_version_1_spec_serializes_without_version_2_keys() {
    let d = load("", "[action]\ncommand = \"query\"\nargs = [\"layout\"]\n").unwrap();
    let json = serde_json::to_value(&d).unwrap();
    for key in ["schema_version", "setup", "then", "check"] {
        assert!(json.get(key).is_none(), "{key} leaked into a v1 spec");
    }
    let expect = json.get("expect").unwrap();
    assert!(expect.get("stdout_present").is_none());
    assert!(expect.get("stdout_lists_window").is_none());
}

// ---- commands ----

#[test]
fn test_spawn_and_other_commands_are_not_allowed_in_version_2() {
    for cmd in [
        "spawn",
        "install",
        "uninstall",
        "update",
        "config",
        "preflight-binary-check",
    ] {
        assert_err(&action(cmd, ""), "is not allowed in a version 2 spec");
    }
}

#[test]
fn test_version_2_spec_has_no_subject() {
    let body = format!("{GOOD}\n[subject]\napp = \"fixture\"\n");
    assert_err(&body, "remove [subject]");
}

// ---- the only selector is --id ${name} ----

#[test]
fn test_literal_id_is_a_load_error() {
    assert_err(
        &action("transform", r#""--id", "5", "--size", "1", "1""#),
        "value of --id must be",
    );
    assert_err(
        &action("close", r#""--id", "${w}x""#),
        "value of --id must be",
    );
}

#[test]
fn test_other_selectors_are_load_errors() {
    for sel in [
        r#""-c", "foo""#,
        r#""--class", "foo""#,
        r#""--class=foo""#,
        r#""-cfoo""#,
        r#""-c=foo""#,
        r#""-t", "x""#,
        r#""--title", "x""#,
        r#""--title=x""#,
        r#""-tx""#,
        r#""--pid", "5""#,
        r#""--pid=5""#,
        r#""--focused""#,
    ] {
        for cmd in [
            "focus",
            "transform",
            "maximize",
            "minimize",
            "restore",
            "close",
            "defocus",
        ] {
            let args = format!(r#""--id", "${{w}}", {sel}"#);
            assert_err(
                &action(cmd, &args),
                "selects a window by something other than a recorded id",
            );
        }
    }
}

#[test]
fn test_window_commands_need_exactly_one_id() {
    for cmd in [
        "focus",
        "defocus",
        "transform",
        "maximize",
        "minimize",
        "restore",
        "close",
    ] {
        assert_err(&action(cmd, ""), "needs exactly one '--id");
        assert_err(&action(cmd, r#""--no-focus""#), "needs exactly one '--id");
        assert_err(
            &action(cmd, r#""--id", "${w}", "--id", "${v}""#),
            "needs exactly one '--id",
        );
    }
}

#[test]
fn test_bare_defocus_is_a_load_error() {
    // Without a selector `defocus` targets the focused window, which may be the user's.
    assert_err(&action("defocus", ""), "needs exactly one '--id");
    assert_err(
        &action("defocus", r#""--to-desktop""#),
        "needs exactly one '--id",
    );
}

#[test]
fn test_id_equals_form_and_double_dash_are_refused() {
    assert_err(&action("close", r#""--id=${w}""#), "two arguments");
    assert_err(
        &action("close", r#""--id", "${w}", "--", "x""#),
        "'--' is not allowed",
    );
}

#[test]
fn test_id_value_must_name_a_declared_setup() {
    assert_err(
        &action("close", r#""--id", "${zzz}""#),
        "value of --id must be",
    );
    assert_err(
        &action("close", r#""--id", "${w.title}""#),
        "value of --id must be",
    );
    assert_err(
        &action("close", r#""--id", "${W}""#),
        "value of --id must be",
    );
}

#[test]
fn test_nonexistent_is_allowed_as_an_id_value_only() {
    assert!(v2(&action("close", r#""--id", "${nonexistent}""#)).is_ok());
    assert_err(
        "[[setup]]\nname = \"nonexistent\"\n[action]\ncommand = \"query\"\nargs = [\"layout\"]\n",
        "is reserved",
    );
}

#[test]
fn test_placeholders_are_only_allowed_where_they_belong() {
    assert_err(
        &action("transform", r#""--id", "${w}", "--size", "${w}", "300""#),
        "may only be the value of --id",
    );
    assert_err(
        &action("transform", r#""--id", "${w}", "--anchor", "${w.title}""#),
        "may only be the value of --to-window",
    );
    assert_err(
        &action("query", r#""windows", "${w}""#),
        "may only be the value of --id",
    );
}

#[test]
fn test_malformed_placeholders_are_load_errors() {
    assert_err(
        &action("close", r#""--id", "${w""#),
        "value of --id must be",
    );
    assert_err(
        &action("transform", r#""--id", "${w}", "--size", "$${w}", "1""#),
        "may only be the value of --id",
    );
    assert!(placeholders("${W}").is_err());
    assert!(placeholders("${w").is_err());
    assert!(placeholders("{w}").unwrap().is_empty());
}

#[test]
fn test_defocus_to_window_must_be_a_unique_title_of_a_setup() {
    assert!(v2(&action(
        "defocus",
        r#""--id", "${w}", "--to-window", "${v.title}""#
    ))
    .is_ok());
    assert!(v2(&action(
        "defocus",
        r#""--id", "${w}", "--to", "${v.title}""#
    ))
    .is_ok());
    assert!(v2(&action("defocus", r#""--id", "${w}", "--to-desktop""#)).is_ok());
    assert_err(
        &action("defocus", r#""--id", "${w}", "--to-window", "Terminal""#),
        "must be ${name.title}",
    );
    assert_err(
        &action("defocus", r#""--id", "${w}", "--to-window", "${v}""#),
        "may only be the value of --id",
    );
    assert_err(
        &action(
            "defocus",
            r#""--id", "${w}", "--to-window", "${zzz.title}""#,
        ),
        "names no [[setup]]",
    );
}

#[test]
fn test_query_takes_no_id_and_needs_no_setup() {
    let body = "[action]\ncommand = \"query\"\nargs = [\"layout\", \"--json\"]\n";
    assert!(v2(body).is_ok());
    assert_err(
        &action("query", r#""windows", "--id", "${w}""#),
        "takes no --id",
    );
}

#[test]
fn test_then_actions_follow_the_same_rules() {
    let body = format!("{GOOD}\n[[then]]\ncommand = \"close\"\nargs = [\"--id\", \"5\"]\n");
    assert_err(&body, "[[then]] #1");
    let body = format!("{GOOD}\n[[then]]\ncommand = \"spawn\"\n");
    assert_err(&body, "[[then]] #1");
    let body = format!("{GOOD}\n[[then]]\ncommand = \"close\"\nargs = [\"--id\", \"${{w}}\"]\n");
    assert!(v2(&body).is_ok());
}

// ---- setups ----

#[test]
fn test_at_most_three_setups() {
    let mut body = String::new();
    for n in ["a", "b", "c"] {
        body.push_str(&format!("[[setup]]\nname = \"{n}\"\n"));
    }
    body.push_str("[action]\ncommand = \"query\"\nargs = [\"windows\"]\n");
    assert!(v2(&body).is_ok());
    let four = body.replace("[action]", "[[setup]]\nname = \"d\"\n[action]");
    assert_err(&four, "at most 3");
}

#[test]
fn test_setup_names_are_checked() {
    for bad in ["W", "1a", "a-b", "", "a b", "a.b"] {
        let body = format!(
            "[[setup]]\nname = \"{bad}\"\n[action]\ncommand = \"query\"\nargs = [\"windows\"]\n"
        );
        assert_err(&body, "setup name");
    }
    let dup = "[[setup]]\nname = \"w\"\n[[setup]]\nname = \"w\"\n[action]\ncommand = \"query\"\nargs = [\"windows\"]\n";
    assert_err(dup, "used twice");
}

#[test]
fn test_setup_name_may_not_be_a_matrix_key() {
    let body = format!("{GOOD}\n[matrix]\nw = [1, 2]\n");
    assert_err(&body, "also a [matrix] key");
    let ok = format!("{GOOD}\n[matrix]\nanchor = [\"center\"]\n");
    assert!(v2(&ok).is_ok());
}

#[test]
fn test_setup_flags_are_whitelisted() {
    let with = |args: &str| {
        format!(
            "[[setup]]\nname = \"w\"\nargs = [{args}]\n[action]\ncommand = \"query\"\nargs = [\"windows\"]\n"
        )
    };
    for ok in [
        r#""--width", "400", "--height", "300""#,
        r#""--min-width", "100", "--state-colors""#,
        r#""--decorations", "on", "--paint-delay-ms", "50""#,
        r#""--fixed-size", "--min-size", "300x200""#,
        r#""--expect", "some text""#,
    ] {
        assert!(v2(&with(ok)).is_ok(), "{ok}");
    }
    for (bad, why) in [
        (r#""--title", "x""#, "not allowed in a setup"),
        (r#""--app-id", "x""#, "not allowed in a setup"),
        (r#""--exit-after-ms", "99999999""#, "not allowed in a setup"),
        (r#""--protocol", "x11""#, "not allowed in a setup"),
        (r#""--print-ready""#, "not allowed in a setup"),
        (r#""--remaximize-after-ms", "5""#, "not allowed in a setup"),
        (r#""--width=400""#, "not allowed in a setup"),
        (r#""rm""#, "not allowed in a setup"),
        (r#""--width""#, "needs a value"),
        (r#""--width", "--height""#, "needs a value"),
    ] {
        assert_err(&with(bad), why);
    }
}

#[test]
fn test_setup_timeout_is_bounded() {
    let with = |ms: &str| {
        format!(
            "[[setup]]\nname = \"w\"\ntimeout_ms = {ms}\n[action]\ncommand = \"query\"\nargs = [\"windows\"]\n"
        )
    };
    assert!(v2(&with("5000")).is_ok());
    assert_err(&with("0"), "timeout_ms must be");
    assert_err(&with("60001"), "timeout_ms must be");
}

#[test]
fn test_unknown_keys_in_setup_and_check_are_parse_errors() {
    let body = GOOD.replace("name = \"w\"", "name = \"w\"\ntitle = \"mine\"");
    assert!(v2(&body).unwrap_err().starts_with("parse:"));
    let body = format!("{GOOD}\n[[check]]\nwindow = \"w\"\nstate = \"present\"\nclass = \"x\"\n");
    assert!(v2(&body).unwrap_err().starts_with("parse:"));
}

// ---- checks ----

#[test]
fn test_checks_need_a_declared_window_and_exactly_one_assertion() {
    let ok = format!("{GOOD}\n[[check]]\nwindow = \"w\"\nstate = \"maximized\"\n");
    assert!(v2(&ok).is_ok());
    assert_err(
        &format!("{GOOD}\n[[check]]\nwindow = \"zzz\"\nstate = \"present\"\n"),
        "names no [[setup]]",
    );
    assert_err(
        &format!("{GOOD}\n[[check]]\nwindow = \"w\"\n"),
        "exactly one of",
    );
    assert_err(
        &format!("{GOOD}\n[[check]]\nwindow = \"w\"\nstate = \"gone\"\nprocess_exited = true\n"),
        "exactly one of",
    );
    assert_err(
        &format!("{GOOD}\n[[check]]\nwindow = \"w\"\nstate = \"flying\"\n"),
        "parse:",
    );
    assert_err(
        &format!("{GOOD}\n[[check]]\nwindow = \"w\"\nsame_rect_as = \"after\"\n"),
        "must be \"before\"",
    );
    assert!(v2(&format!(
        "{GOOD}\n[[check]]\nwindow = \"w\"\nprocess_exited = true\n"
    ))
    .is_ok());
}

#[test]
fn test_check_after_must_name_an_action() {
    let one = format!("{GOOD}\n[[check]]\nafter = 1\nwindow = \"w\"\nstate = \"present\"\n");
    assert_err(&one, "after = 1");
    let with_then = format!(
        "{GOOD}\n[[then]]\ncommand = \"close\"\nargs = [\"--id\", \"${{w}}\"]\n{}",
        "[[check]]\nafter = 1\nwindow = \"w\"\nstate = \"gone\"\n"
    );
    assert!(v2(&with_then).is_ok());
}

// ---- expectations ----

#[test]
fn test_placeholders_in_expectations_must_name_a_setup() {
    let ok = format!(
        "{GOOD}\n[expect]\nstdout_present = [\"\\\"id\\\": ${{w}}\"]\nstderr_present = [\"No window with id ${{nonexistent}}\"]\nstdout_lists_window = \"w\"\n"
    );
    assert!(v2(&ok).is_ok());
    let bad = format!("{GOOD}\n[expect]\nstdout_present = [\"${{zzz}}\"]\n");
    assert_err(&bad, "names no [[setup]]");
    let bad = format!("{GOOD}\n[expect]\nstdout_present = [\"${{w\"]\n");
    assert_err(&bad, "malformed placeholder");
    let bad = format!("{GOOD}\n[expect]\nstdout_lists_window = \"zzz\"\n");
    assert_err(&bad, "names no [[setup]]");
}

#[test]
fn test_whole_placeholder_parses_only_a_complete_one() {
    assert_eq!(
        whole_placeholder("${w}"),
        Some(Placeholder {
            name: "w".into(),
            title: false
        })
    );
    assert_eq!(
        whole_placeholder("${w.title}"),
        Some(Placeholder {
            name: "w".into(),
            title: true
        })
    );
    assert_eq!(whole_placeholder("x${w}"), None);
    assert_eq!(whole_placeholder("${w}x"), None);
    assert_eq!(whole_placeholder("5"), None);
}

// ---- run-time substitution ----

use super::v2::{substitute, Bindings};

fn bindings() -> Bindings {
    let mut b = Bindings::default();
    b.ids.insert("w".into(), 42);
    b.ids.insert("v".into(), 7);
    b.titles.insert("w".into(), "verify-1-0-w".into());
    b
}

#[test]
fn test_substitute_replaces_ids_titles_and_nonexistent() {
    let mut b = bindings();
    b.nonexistent = Some(1_000_042);
    assert_eq!(substitute("${w}", &b).unwrap(), "42");
    assert_eq!(
        substitute("id ${v} then ${w}!", &b).unwrap(),
        "id 7 then 42!"
    );
    assert_eq!(substitute("${w.title}", &b).unwrap(), "verify-1-0-w");
    assert_eq!(substitute("${nonexistent}", &b).unwrap(), "1000042");
    assert_eq!(
        substitute("no placeholder {w} $w", &b).unwrap(),
        "no placeholder {w} $w"
    );
}

#[test]
fn test_substitute_refuses_anything_not_recorded() {
    let b = bindings();
    assert!(substitute("${zzz}", &b)
        .unwrap_err()
        .contains("no recorded value"));
    assert!(substitute("${v.title}", &b)
        .unwrap_err()
        .contains("no recorded value"));
    // Until the runner has chosen a provably absent id, `${nonexistent}` has no value.
    assert!(substitute("${nonexistent}", &b).is_err());
    assert!(substitute("${w", &b).unwrap_err().contains("malformed"));
}

#[test]
fn test_substituted_text_is_not_scanned_again() {
    // A title that itself looks like a placeholder must not be expanded a second time.
    let mut b = bindings();
    b.titles.insert("w".into(), "${v}".into());
    assert_eq!(substitute("${w.title}", &b).unwrap(), "${v}");
}

// ---- matrix expansion reaches the version 2 fields ----

#[test]
fn test_matrix_substitutes_version_2_fields_and_keeps_placeholders() {
    let body = r#"
[matrix]
anchor = ["top-left", "center"]

[[setup]]
name = "w"
args = ["--expect", "move to {anchor}"]

[action]
command = "transform"
args = ["--id", "${w}", "--anchor", "{anchor}", "--no-focus"]

[[then]]
command = "transform"
args = ["--id", "${w}", "--anchor", "{anchor}"]

[expect]
stdout_present = ["{anchor}"]
"#;
    let def = v2(body).unwrap();
    let expanded = def.expand_matrix().unwrap();
    assert_eq!(expanded.len(), 2);
    let ids: Vec<&str> = expanded.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "transforms/example/anchor-top-left",
            "transforms/example/anchor-center"
        ]
    );
    let first = &expanded[1];
    assert_eq!(
        first.action.args,
        ["--id", "${w}", "--anchor", "center", "--no-focus"]
    );
    assert_eq!(first.v2.then[0].args[3], "center");
    assert_eq!(first.v2.setup[0].args[1], "move to center");
    assert_eq!(first.expect.stdout_present, ["center"]);
    // The expanded copies still pass the load rules.
    for t in &expanded {
        validate(t).unwrap();
    }
}

// ---- expectations a version 2 spec cannot have ----

#[test]
fn test_spawn_only_expectations_are_refused_in_version_2() {
    assert_err(
        &format!("{GOOD}\n[expect]\nnever_visible_off_target = true\n"),
        "never_visible_off_target",
    );
    assert_err(
        &format!("{GOOD}\n[expect]\ndiagnostics = [\"x\"]\n"),
        "diagnostics",
    );
    assert_err(
        &format!("{GOOD}\n[expect.resize]\noutcome = \"reached\"\n"),
        "[expect.resize]",
    );
}

#[test]
fn test_expect_rect_needs_a_recorded_window() {
    let rect = "[expect.rect]\nanchor = \"center\"\nsize = [10, 10]\nmargin = 0\n";
    let without_setup = format!("[action]\ncommand = \"query\"\nargs = [\"layout\"]\n{rect}");
    assert_err(&without_setup, "needs a [[setup]]");
    assert!(v2(&format!("{GOOD}\n{rect}")).is_ok());
}

// ---- the moves-focus tag ----

#[test]
fn test_a_spec_that_launches_a_window_must_carry_the_moves_focus_tag() {
    let head = |tags: &str| {
        format!(
            "schema_version = 2\nid = \"transforms/example\"\ncategory = \"transforms\"\ntitle = \"t\"\ndescription = \"d\"\nkind = \"auto\"\n{tags}\n"
        )
    };
    let setup = "[[setup]]\nname = \"w\"\n[action]\ncommand = \"query\"\nargs = [\"windows\"]\n";
    let check = |tags: &str| -> Result<(), String> {
        let def: TestDefinition = toml::from_str(&format!("{}{setup}", head(tags))).unwrap();
        validate(&def)
    };
    assert!(check("tags = [\"moves-focus\"]").is_ok());
    assert!(check("tags = [\"visual\", \"moves-focus\"]").is_ok());
    for bad in [
        "",
        "tags = []",
        "tags = [\"visual\"]",
        "tags = [\"moves_focus\"]",
    ] {
        let e = check(bad).unwrap_err();
        assert!(e.contains("add the tag \"moves-focus\""), "{bad}: {e}");
    }
    // No window launched, nothing that moves focus: no tag needed.
    let query_only = format!(
        "{}[action]\ncommand = \"query\"\nargs = [\"layout\"]\n",
        head("")
    );
    let def: TestDefinition = toml::from_str(&query_only).unwrap();
    assert!(validate(&def).is_ok());
}

#[test]
fn test_matrix_substitutes_the_anchor_the_oracle_expects() {
    let body = r#"
[matrix]
anchor = ["top-left", "center"]
[[setup]]
name = "w"
[action]
command = "transform"
args = ["--id", "${w}", "--size", "300", "200", "--anchor", "{anchor}", "--margin", "16"]
[expect.rect]
anchor = "{anchor}"
size = [300, 200]
margin = 16
"#;
    let expanded = v2(body).unwrap().expand_matrix().unwrap();
    let anchors: Vec<_> = expanded
        .iter()
        .map(|t| t.expect.rect.as_ref().unwrap().anchor.clone().unwrap())
        .collect();
    assert_eq!(anchors, ["top-left", "center"]);
}

// ---- a real program in [[setup]], and the shell_alive check ----

const REAL_HEAD: &str = r#"
id = "apps/example"
category = "apps"
title = "example"
description = "example"
tags = ["needs-real-app", "moves-focus", "manual-setup"]
kind = "auto"
"#;

fn real(body: &str) -> Result<TestDefinition, String> {
    let text = format!("schema_version = 2\n{REAL_HEAD}\n{body}");
    let def: TestDefinition = toml::from_str(&text).map_err(|e| format!("parse: {e}"))?;
    validate(&def)?;
    Ok(def)
}

#[track_caller]
fn assert_real_err(body: &str, needle: &str) {
    match real(body) {
        Ok(_) => panic!("expected an error containing '{needle}', but the spec loaded"),
        Err(e) => assert!(e.contains(needle), "error '{e}' lacks '{needle}'"),
    }
}

const REAL_GOOD: &str = r#"
[[setup]]
name = "w"
command = ["gnome-text-editor", "--new-window"]
env = { GDK_BACKEND = "x11" }

[action]
command = "close"
args = ["--id", "${w}"]

[[check]]
window = "w"
state = "gone"

[[check]]
shell_alive = true
"#;

#[test]
fn test_a_real_program_setup_and_a_shell_alive_check_load() {
    let def = real(REAL_GOOD).unwrap();
    assert_eq!(
        def.v2.setup[0].command,
        ["gnome-text-editor", "--new-window"]
    );
    assert_eq!(def.v2.setup[0].env["GDK_BACKEND"], "x11");
    assert!(def.v2.check[1].shell_alive && def.v2.check[1].window.is_empty());
}

#[test]
fn test_a_real_program_setup_needs_the_needs_real_app_tag() {
    let text = format!("schema_version = 2\n{HEAD}\n{REAL_GOOD}");
    let def: TestDefinition = toml::from_str(&text).unwrap();
    let e = validate(&def).unwrap_err();
    assert!(e.contains("needs-real-app"), "{e}");
}

#[test]
fn test_only_listed_programs_flags_and_environment_are_allowed() {
    let setup = |command: &str, env: &str| {
        format!(
            "[[setup]]\nname = \"w\"\ncommand = {command}\n{env}\n[action]\ncommand = \"close\"\nargs = [\"--id\", \"${{w}}\"]\n"
        )
    };
    assert_real_err(
        &setup(r#"["sh", "-c", "x"]"#, ""),
        "not in the allowed list",
    );
    assert_real_err(
        &setup(r#"["/usr/bin/gnome-text-editor"]"#, ""),
        "bare program name",
    );
    assert_real_err(
        &setup(r#"["gnome-text-editor", "/etc/passwd"]"#, ""),
        "must be a flag",
    );
    assert_real_err(
        &setup(r#"["gnome-text-editor", "-n"]"#, ""),
        "must be a flag",
    );
    assert_real_err(
        &setup(
            r#"["gnome-text-editor"]"#,
            r#"env = { LD_PRELOAD = "/tmp/x.so" }"#,
        ),
        "not allowed",
    );
    assert_real_err(
        &setup(
            r#"["gnome-text-editor"]"#,
            r#"env = { GDK_BACKEND = "mir" }"#,
        ),
        "allowed values",
    );
    // An empty command is not a real program: it is the fixture, as before.
    assert!(real(&setup("[]", "")).is_ok());
}

#[test]
fn test_command_and_fixture_flags_do_not_mix_and_env_needs_a_command() {
    assert_real_err(
        "[[setup]]\nname = \"w\"\ncommand = [\"gnome-text-editor\"]\nargs = [\"--width\", \"400\"]\n[action]\ncommand = \"close\"\nargs = [\"--id\", \"${w}\"]\n",
        "cannot be combined",
    );
    assert_real_err(
        "[[setup]]\nname = \"w\"\nenv = { GDK_BACKEND = \"x11\" }\n[action]\ncommand = \"close\"\nargs = [\"--id\", \"${w}\"]\n",
        "env needs a command",
    );
}

#[test]
fn test_shell_alive_takes_no_window_and_no_other_kind() {
    let with = |check: &str| {
        format!(
            "[[setup]]\nname = \"w\"\ncommand = [\"gnome-text-editor\"]\n[action]\ncommand = \"close\"\nargs = [\"--id\", \"${{w}}\"]\n{check}"
        )
    };
    assert_real_err(
        &with("[[check]]\nwindow = \"w\"\nshell_alive = true\n"),
        "not a window",
    );
    assert_real_err(
        &with("[[check]]\nshell_alive = true\nstate = \"gone\"\n"),
        "exactly one of",
    );
    // Every other kind still names a declared window.
    assert_real_err(&with("[[check]]\nstate = \"gone\"\n"), "names no [[setup]]");
    assert!(real(&with("[[check]]\nshell_alive = true\n")).is_ok());
}

#[test]
fn test_version_1_serialization_and_existing_setups_have_no_new_keys() {
    let def = v2(GOOD).unwrap();
    let text = toml::to_string(&def).unwrap();
    assert!(
        !text.contains("command = []") && !text.contains("shell_alive"),
        "{text}"
    );
    assert!(!text.contains("[setup.env]"), "{text}");
}
