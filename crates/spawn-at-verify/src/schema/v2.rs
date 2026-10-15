//! Load-time rules for version 2 specs (`docs/DESIGN_RUNNER_SETUP.md` section 2.3).
//!
//! A version 2 spec may launch its own fixture windows (`[[setup]]`) and act on them. The rules
//! here are the first half of the safety model: they make it impossible for a spec to name any
//! window other than one the runner recorded itself. The runner re-checks at run time (section 3.3
//! of the design); these rules keep the mistake out of the catalog in the first place.

use super::types::{TestAction, TestDefinition, SCHEMA_VERSION_EXISTING_WINDOW};
use regex::Regex;
use std::collections::HashSet;

/// Most `[[setup]]` windows one spec may launch.
pub const MAX_SETUPS: usize = 3;

/// Longest `timeout_ms` a setup may ask for.
pub const MAX_SETUP_TIMEOUT_MS: u64 = 60_000;

/// The name that stands for an id the runner has just proved is not a window (design 3.5). The only
/// way a spec can name an id it did not record, and it is never a real window.
pub const NONEXISTENT: &str = "nonexistent";

/// Commands a version 2 `[action]` or `[[then]]` may run.
pub const V2_COMMANDS: &[&str] = &[
    "focus",
    "defocus",
    "transform",
    "maximize",
    "minimize",
    "restore",
    "close",
    "query",
];

/// The commands that act on one existing window and so need `--id ${name}`.
pub const WINDOW_COMMANDS: &[&str] = &[
    "focus",
    "defocus",
    "transform",
    "maximize",
    "minimize",
    "restore",
    "close",
];

/// Fixture flags a `[[setup]]` may pass, and whether each takes a value. Anything else, in
/// particular `--title`, `--app-id`, `--exit-after-ms` and `--protocol`, belongs to the runner.
pub const SETUP_FLAGS: &[(&str, bool)] = &[
    ("--width", true),
    ("--height", true),
    ("--min-width", true),
    ("--min-height", true),
    ("--decorations", true),
    ("--paint-delay-ms", true),
    ("--expect", true),
    ("--state-colors", false),
    ("--resize-delay-ms", true),
    ("--min-size", true),
    ("--max-size", true),
    ("--resize-increment", true),
    ("--fixed-size", false),
    ("--stall-after-map-ms", true),
    ("--flap-ms", true),
];

/// One `${name}` or `${name.title}` found in a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placeholder {
    /// The setup name (or [`NONEXISTENT`]).
    pub name: String,
    /// True for `${name.title}`, the window's unique title.
    pub title: bool,
}

fn placeholder_regex() -> Regex {
    Regex::new(r"\$\{([a-z][a-z0-9_]*)(\.title)?\}").expect("static regex")
}

fn name_is_valid(name: &str) -> bool {
    Regex::new(r"^[a-z][a-z0-9_]*$")
        .expect("static regex")
        .is_match(name)
}

/// Every placeholder in `text`, or an error if `text` holds a `${` that is not a well-formed one.
pub fn placeholders(text: &str) -> Result<Vec<Placeholder>, String> {
    let re = placeholder_regex();
    let found: Vec<Placeholder> = re
        .captures_iter(text)
        .map(|c| Placeholder {
            name: c[1].to_string(),
            title: c.get(2).is_some(),
        })
        .collect();
    if text.matches("${").count() != found.len() {
        return Err(format!(
            "malformed placeholder in '{text}': write ${{name}} or ${{name.title}} with a lower-case name"
        ));
    }
    Ok(found)
}

/// `Some(placeholder)` when the whole of `text` is exactly one placeholder.
pub fn whole_placeholder(text: &str) -> Option<Placeholder> {
    let re = placeholder_regex();
    let c = re.captures(text)?;
    if c.get(0)?.as_str() != text {
        return None;
    }
    Some(Placeholder {
        name: c[1].to_string(),
        title: c.get(2).is_some(),
    })
}

/// True for a token that selects a window by anything other than a recorded id.
fn is_forbidden_selector(token: &str) -> bool {
    const EXACT: &[&str] = &["-c", "--class", "-t", "--title", "--pid", "--focused"];
    if EXACT.contains(&token) {
        return true;
    }
    if ["--class=", "--title=", "--pid=", "--focused="]
        .iter()
        .any(|p| token.starts_with(p))
    {
        return true;
    }
    // `-cfoo`, `-tfoo`, `-c=foo`: a short option with its value attached.
    !token.starts_with("--")
        && token.len() > 2
        && (token.starts_with("-c") || token.starts_with("-t"))
}

/// Checks the arguments of one version 2 action.
fn validate_action(
    label: &str,
    action: &TestAction,
    declared: &HashSet<&str>,
) -> Result<(), String> {
    let cmd = action.command.as_str();
    if !V2_COMMANDS.contains(&cmd) {
        return Err(format!(
            "{label}: command '{cmd}' is not allowed in a version 2 spec (allowed: {V2_COMMANDS:?})"
        ));
    }
    let args = &action.args;
    if args.iter().any(|a| a == "--") {
        return Err(format!("{label}: '--' is not allowed in the arguments"));
    }
    if let Some(bad) = args.iter().find(|a| is_forbidden_selector(a)) {
        return Err(format!(
            "{label}: '{bad}' selects a window by something other than a recorded id; use --id ${{name}}"
        ));
    }

    // Where each placeholder is used: the value after --id may be ${name}; the value after
    // --to-window / --to may be ${name.title}; nowhere else.
    let id_positions: Vec<usize> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == "--id")
        .map(|(i, _)| i)
        .collect();
    if args.iter().any(|a| a.starts_with("--id=")) {
        return Err(format!(
            "{label}: write '--id' and its value as two arguments, not '--id=...'"
        ));
    }
    if WINDOW_COMMANDS.contains(&cmd) {
        let [pos] = id_positions[..] else {
            return Err(format!(
                "{label}: '{cmd}' needs exactly one '--id ${{name}}' (found {})",
                id_positions.len()
            ));
        };
        let value = args.get(pos + 1).map(String::as_str).unwrap_or("");
        match whole_placeholder(value) {
            Some(p)
                if !p.title && (declared.contains(p.name.as_str()) || p.name == NONEXISTENT) => {}
            _ => {
                return Err(format!(
                    "{label}: the value of --id must be ${{name}} of a declared [[setup]] (or ${{{NONEXISTENT}}}), got '{value}'"
                ));
            }
        }
    } else if !id_positions.is_empty() {
        return Err(format!("{label}: '{cmd}' takes no --id"));
    }

    for (i, arg) in args.iter().enumerate() {
        let found = placeholders(arg).map_err(|e| format!("{label}: {e}"))?;
        for p in found {
            let prev = if i > 0 { args[i - 1].as_str() } else { "" };
            let is_id_value = prev == "--id";
            let is_window_value = cmd == "defocus" && (prev == "--to-window" || prev == "--to");
            let name_ok = declared.contains(p.name.as_str()) || (p.name == NONEXISTENT && !p.title);
            if !name_ok {
                return Err(format!(
                    "{label}: placeholder '{}' names no [[setup]] of this spec",
                    p.name
                ));
            }
            let place_ok = if p.title {
                is_window_value
            } else {
                is_id_value
            };
            if !place_ok {
                return Err(format!(
                    "{label}: ${{{}{}}} may only be the value of {}",
                    p.name,
                    if p.title { ".title" } else { "" },
                    if p.title {
                        "--to-window / --to of defocus"
                    } else {
                        "--id"
                    }
                ));
            }
        }
    }
    // `defocus --to-window X` hands focus to a window chosen by title: only our own unique title.
    if cmd == "defocus" {
        for (i, arg) in args.iter().enumerate() {
            if arg == "--to-window" || arg == "--to" {
                let value = args.get(i + 1).map(String::as_str).unwrap_or("");
                match whole_placeholder(value) {
                    Some(p) if p.title && declared.contains(p.name.as_str()) => {}
                    _ => {
                        return Err(format!(
                            "{label}: the value of {arg} must be ${{name.title}} of a declared [[setup]], got '{value}'"
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Checks the flags of one `[[setup]]` against [`SETUP_FLAGS`].
pub fn validate_setup_args(label: &str, args: &[String]) -> Result<(), String> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let Some((_, takes_value)) = SETUP_FLAGS.iter().find(|(f, _)| f == arg) else {
            return Err(format!(
                "{label}: fixture flag '{arg}' is not allowed in a setup (allowed: {:?}); the runner sets --title, --app-id, --exit-after-ms and --protocol itself",
                SETUP_FLAGS.iter().map(|(f, _)| *f).collect::<Vec<_>>()
            ));
        };
        if *takes_value {
            match args.get(i + 1) {
                Some(v) if !v.starts_with("--") => i += 1,
                _ => return Err(format!("{label}: {arg} needs a value")),
            }
        }
        i += 1;
    }
    Ok(())
}

/// Applies the load-time rules to a parsed spec. A version 1 spec passes if it uses none of the
/// version 2 keys; a version 2 spec must satisfy every rule of design section 2.3.
pub fn validate(t: &TestDefinition) -> Result<(), String> {
    let id = &t.id;
    let version = t.v2.schema_version;
    let uses_v2_expect =
        !t.expect.stdout_present.is_empty() || t.expect.stdout_lists_window.is_some();

    if version == 0 || version > SCHEMA_VERSION_EXISTING_WINDOW {
        return Err(format!(
            "Spec '{id}': schema_version {version} is not supported (use 1, or 2 for specs with [[setup]])"
        ));
    }
    if version == 1 {
        if !t.v2.is_empty() || uses_v2_expect {
            return Err(format!(
                "Spec '{id}': [[setup]], [[then]], [[check]], stdout_present and stdout_lists_window require schema_version = 2"
            ));
        }
        return Ok(());
    }

    let e = |m: String| format!("Spec '{id}': {m}");
    if t.subject.is_some() {
        return Err(e(
            "a version 2 spec launches its windows with [[setup]]; remove [subject]".into(),
        ));
    }

    // Setups.
    if t.v2.setup.len() > MAX_SETUPS {
        return Err(e(format!(
            "{} [[setup]] windows; at most {MAX_SETUPS} are allowed",
            t.v2.setup.len()
        )));
    }
    let matrix_keys: HashSet<&str> = t
        .matrix
        .as_ref()
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let mut declared: HashSet<&str> = HashSet::new();
    for s in &t.v2.setup {
        if !name_is_valid(&s.name) {
            return Err(e(format!(
                "setup name '{}' must be lower-case letters, digits and '_', starting with a letter",
                s.name
            )));
        }
        if s.name == NONEXISTENT {
            return Err(e(format!(
                "'{NONEXISTENT}' is reserved and cannot name a setup"
            )));
        }
        if matrix_keys.contains(s.name.as_str()) {
            return Err(e(format!(
                "setup name '{}' is also a [matrix] key; matrix substitution would rewrite ${{{}}}",
                s.name, s.name
            )));
        }
        if !declared.insert(s.name.as_str()) {
            return Err(e(format!("setup name '{}' is used twice", s.name)));
        }
        if let Some(ms) = s.timeout_ms {
            if ms == 0 || ms > MAX_SETUP_TIMEOUT_MS {
                return Err(e(format!(
                    "setup '{}': timeout_ms must be 1..={MAX_SETUP_TIMEOUT_MS}",
                    s.name
                )));
            }
        }
        validate_setup_args(&format!("Spec '{id}': setup '{}'", s.name), &s.args)?;
    }

    // Actions.
    validate_action(&format!("Spec '{id}': [action]"), &t.action, &declared)?;
    for (i, a) in t.v2.then.iter().enumerate() {
        validate_action(&format!("Spec '{id}': [[then]] #{}", i + 1), a, &declared)?;
    }

    // Checks.
    let last_action = t.v2.then.len();
    for (i, c) in t.v2.check.iter().enumerate() {
        let label = format!("Spec '{id}': [[check]] #{}", i + 1);
        if !declared.contains(c.window.as_str()) {
            return Err(format!(
                "{label}: window '{}' names no [[setup]] of this spec",
                c.window
            ));
        }
        let kinds = usize::from(c.state.is_some())
            + usize::from(c.same_rect_as.is_some())
            + usize::from(c.process_exited);
        if kinds != 1 {
            return Err(format!(
                "{label}: give exactly one of state, same_rect_as, process_exited (found {kinds})"
            ));
        }
        if let Some(r) = &c.same_rect_as {
            if r != "before" {
                return Err(format!(
                    "{label}: same_rect_as must be \"before\", got '{r}'"
                ));
            }
        }
        if let Some(after) = c.after {
            if after > last_action {
                return Err(format!(
                    "{label}: after = {after} but the spec has {} action(s) (0 to {last_action})",
                    last_action + 1
                ));
            }
        }
    }

    // Placeholders in expectations may name any declared setup.
    let mut strings: Vec<&str> = Vec::new();
    strings.extend(t.expect.stdout_present.iter().map(String::as_str));
    strings.extend(t.expect.stderr_present.iter().map(String::as_str));
    strings.extend(t.expect.stderr_absent.iter().map(String::as_str));
    strings.extend(t.expect.diagnostics.iter().map(String::as_str));
    for s in strings {
        for p in placeholders(s).map_err(&e)? {
            if !declared.contains(p.name.as_str()) && p.name != NONEXISTENT {
                return Err(e(format!(
                    "expectation '{s}': placeholder '{}' names no [[setup]]",
                    p.name
                )));
            }
        }
    }
    if let Some(w) = &t.expect.stdout_lists_window {
        if !declared.contains(w.as_str()) {
            return Err(e(format!(
                "stdout_lists_window '{w}' names no [[setup]] of this spec"
            )));
        }
    }
    Ok(())
}

/// What `${...}` placeholders stand for while one test runs: only values the runner recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    /// Recorded window id per setup name.
    pub ids: std::collections::HashMap<String, u64>,
    /// Unique title per setup name.
    pub titles: std::collections::HashMap<String, String>,
    /// An id the runner has just proved is not a window, once it has chosen one.
    pub nonexistent: Option<u64>,
}

/// Replaces every placeholder in `text` from `b`. A placeholder with no binding is an error: a
/// literal `${w}` must never reach a command line, where `spawn-at` would at best refuse it.
pub fn substitute(text: &str, b: &Bindings) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let re = placeholder_regex();
    // `placeholders` rejects a stray `${` first, so the scan below sees only well-formed ones.
    placeholders(text)?;
    while let Some(m) = re.find(rest) {
        out.push_str(&rest[..m.start()]);
        let c = re.captures(&rest[m.start()..]).expect("matched above");
        let name = &c[1];
        let want_title = c.get(2).is_some();
        let value = if want_title {
            b.titles.get(name).cloned()
        } else if name == NONEXISTENT {
            b.nonexistent.map(|n| n.to_string())
        } else {
            b.ids.get(name).map(|n| n.to_string())
        };
        match value {
            Some(v) => out.push_str(&v),
            None => return Err(format!("placeholder {} has no recorded value", m.as_str())),
        }
        rest = &rest[m.end()..];
    }
    out.push_str(rest);
    Ok(out)
}
