use super::*;
use std::cell::RefCell;

/// A store that keeps the text in memory and counts writes.
struct Mem {
    text: RefCell<Option<String>>,
    writes: RefCell<usize>,
}

impl Mem {
    fn new(text: Option<&str>) -> Self {
        Mem {
            text: RefCell::new(text.map(str::to_string)),
            writes: RefCell::new(0),
        }
    }
}

impl SettingsStore for Mem {
    fn location(&self) -> String {
        "mem.toml".into()
    }
    fn read(&self) -> Result<Option<String>, String> {
        Ok(self.text.borrow().clone())
    }
    fn write(&self, text: &str) -> Result<(), String> {
        *self.text.borrow_mut() = Some(text.to_string());
        *self.writes.borrow_mut() += 1;
        Ok(())
    }
}

fn none() -> Settings {
    Settings::default()
}

/// The defaults are the documented ones: cap 250 (the policy default), strategy auto, 2000 ms.
#[test]
fn test_defaults() {
    assert_eq!(Key::WaitCapMs.default_value(), Value::Ms(250));
    assert_eq!(
        Key::WaitCapMs.default_value(),
        Value::Ms(Policy::default().cap_ms as u32)
    );
    assert_eq!(
        Key::WaitStrategy.default_value(),
        Value::Strategy(WaitStrategy::Auto)
    );
    assert_eq!(Key::SpawnWindowTimeoutMs.default_value(), Value::Ms(2000));
}

/// Values at both ends of each range are accepted, one step outside is refused with a message
/// that names the key, the allowed range or words, and the text given.
#[test]
fn test_ranges_and_messages() {
    for (k, ok, bad) in [
        (Key::WaitCapMs, ["1", "1500"], ["0", "1501"]),
        (
            Key::SpawnWindowTimeoutMs,
            ["200", "60000"],
            ["199", "60001"],
        ),
    ] {
        for t in ok {
            assert!(parse_value(k, t).is_ok(), "{} {t}", k.name());
        }
        for t in bad {
            let e = parse_value(k, t).unwrap_err();
            assert!(e.contains(k.name()) && e.contains(&format!("'{t}'")), "{e}");
            assert!(e.contains("whole milliseconds from"), "{e}");
        }
    }
    for bad in ["", "-5", "+5", "1.5", "abc", " 5", "99999999999"] {
        assert!(parse_value(Key::WaitCapMs, bad).is_err(), "'{bad}'");
    }
    assert_eq!(
        parse_value(Key::WaitStrategy, "legacy"),
        Ok(Value::Strategy(WaitStrategy::Legacy))
    );
    // The experimental value is for the hidden flag only: `config set` and the file refuse it.
    for experimental in ["x11-hold-damage", "x11-premap", "x11-pixel-probe"] {
        let e = parse_value(Key::WaitStrategy, experimental).unwrap_err();
        assert!(e.contains("auto, legacy or ready-wait"), "{e}");
    }
    let e = parse_value(Key::WaitStrategy, "fast").unwrap_err();
    assert!(
        e.contains("auto, legacy or ready-wait") && e.contains("'fast'"),
        "{e}"
    );
}

/// An unknown key name lists the valid ones.
#[test]
fn test_unknown_key_lists_the_valid_ones() {
    let e = Key::parse("wait.cap").unwrap_err();
    assert!(e.contains("unknown setting 'wait.cap'"));
    for k in Key::ALL {
        assert!(e.contains(k.name()), "{e}");
    }
}

/// Flag beats file beats default, and the source says which.
#[test]
fn test_precedence_flag_over_file_over_default() {
    let file = Settings {
        wait_cap_ms: Some(100),
        ..none()
    };
    let flags = Settings {
        wait_cap_ms: Some(300),
        ..none()
    };
    assert_eq!(
        effective(Key::WaitCapMs, &flags, &file),
        (Value::Ms(300), Source::Flag)
    );
    assert_eq!(
        effective(Key::WaitCapMs, &none(), &file),
        (Value::Ms(100), Source::File)
    );
    assert_eq!(
        effective(Key::WaitCapMs, &none(), &none()),
        (Value::Ms(250), Source::Default)
    );
    // A flag of `auto` beats a file `legacy` (it is a value, not "unset").
    let (f, problems) = wait_flags(Some("auto"), None);
    assert!(problems.is_empty());
    let file = Settings {
        wait_strategy: Some(WaitStrategy::Legacy),
        ..none()
    };
    assert_eq!(wait_override(&f, &file).strategy, WaitStrategy::Auto);
}

/// With no flags and no file the override is the default one, which is never serialised: the
/// bytes of the arming instruction are those of a build without the config.
#[test]
fn test_nothing_set_means_no_override() {
    assert!(wait_override(&none(), &none()).is_default());
    // `auto` in the file is the same as nothing.
    let file = Settings {
        wait_strategy: Some(WaitStrategy::Auto),
        ..none()
    };
    assert!(wait_override(&none(), &file).is_default());
    let file = Settings {
        wait_cap_ms: Some(250),
        ..none()
    };
    assert_eq!(
        wait_override(&none(), &file).cap_ms,
        Some(250),
        "an explicit 250 is sent"
    );
}

/// A bad flag is dropped with a sentence and the file value applies.
#[test]
fn test_bad_flag_falls_back_to_the_file() {
    let (flags, problems) = wait_flags(Some("fast"), Some("0"));
    assert_eq!(problems.len(), 2);
    assert_eq!(flags, none());
    let file = Settings {
        wait_cap_ms: Some(90),
        ..none()
    };
    assert_eq!(wait_override(&flags, &file).cap_ms, Some(90));
}

/// A valid file is read; the `[update]` table of the update checker is not an unknown key.
#[test]
fn test_load_valid_file_with_foreign_table() {
    let l = load_text(
        "[wait]\ncap_ms = 120\nstrategy = \"ready-wait\"\n[spawn]\nwindow_timeout_ms = 5000\n[update]\nchannel = \"beta\"\n",
        "f",
    );
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(
        l.settings,
        Settings {
            wait_cap_ms: Some(120),
            wait_strategy: Some(WaitStrategy::ReadyWait),
            window_timeout_ms: Some(5000)
        }
    );
}

/// Unknown keys warn and the known ones are still used.
#[test]
fn test_unknown_keys_warn_and_never_fail() {
    let l = load_text(
        "top = 2\n[wait]\ncap_ms = 120\nbogus = 1\n[nope]\nx = 1\n",
        "f",
    );
    assert_eq!(l.settings.wait_cap_ms, Some(120));
    assert_eq!(l.warnings.len(), 3, "{:?}", l.warnings);
    assert!(l.warnings.iter().any(|w| w.contains("'wait.bogus'")));
    assert!(l.warnings.iter().any(|w| w.contains("'nope'")));
    assert!(l.warnings.iter().any(|w| w.contains("'top'")));
}

/// A wrong value or a wrong type is ignored with a warning that names the allowed values; the
/// key keeps its default.
#[test]
fn test_bad_values_warn_and_keep_the_default() {
    let l = load_text(
        "[wait]\ncap_ms = 5000\nstrategy = 3\n[spawn]\nwindow_timeout_ms = \"2000\"\n",
        "f",
    );
    assert_eq!(l.settings, none());
    assert_eq!(l.warnings.len(), 3, "{:?}", l.warnings);
    assert!(l
        .warnings
        .iter()
        .any(|w| w.contains("wait.cap_ms") && w.contains("1 to 1500")));
}

/// A malformed file is one warning with the line number; the defaults apply.
#[test]
fn test_malformed_file_is_one_warning_with_the_line() {
    let l = load_text("[wait]\ncap_ms = 120\n\n[spawn\nwindow = 1\n", "cfg.toml");
    assert_eq!(l.settings, none());
    assert_eq!(l.warnings.len(), 1);
    assert!(
        l.warnings[0].starts_with("cfg.toml: line 4:"),
        "{}",
        l.warnings[0]
    );
    assert!(l.warnings[0].contains("using the built-in defaults"));
}

/// set keeps unknown keys, the update table and other settings; unset removes only its key and
/// an emptied table; round trip returns the original value.
#[test]
fn test_set_unset_round_trip_keeps_everything_else() {
    let start = "[update]\nchannel = \"beta\"\n\n[wait]\nstrategy = \"legacy\"\nodd = 7\n\n[zzz]\nk = \"v\"\n";
    let s = edit_text(start, "f", Key::WaitCapMs, Some(Value::Ms(90))).unwrap();
    let l = load_text(&s, "f");
    assert_eq!(l.settings.wait_cap_ms, Some(90));
    assert_eq!(l.settings.wait_strategy, Some(WaitStrategy::Legacy));
    for kept in ["channel = \"beta\"", "odd = 7", "[zzz]", "k = \"v\""] {
        assert!(s.contains(kept), "{kept} lost in:\n{s}");
    }
    let back = edit_text(&s, "f", Key::WaitCapMs, None).unwrap();
    assert_eq!(load_text(&back, "f").settings.wait_cap_ms, None);
    assert!(back.contains("odd = 7") && back.contains("strategy"));
    // Emptying a table removes it.
    let only = edit_text("", "f", Key::SpawnWindowTimeoutMs, Some(Value::Ms(3000))).unwrap();
    assert!(only.contains("[spawn]"));
    assert_eq!(
        edit_text(&only, "f", Key::SpawnWindowTimeoutMs, None).unwrap(),
        ""
    );
}

/// A malformed file is never overwritten by set.
#[test]
fn test_set_refuses_a_malformed_file() {
    let m = Mem::new(Some("[wait\n"));
    let e = run(
        &m,
        &Action::Set("wait.cap_ms".into(), "100".into()),
        &none(),
    )
    .unwrap_err();
    assert!(e.contains("line 1") && e.contains("not valid TOML"), "{e}");
    assert_eq!(*m.writes.borrow(), 0);
    assert_eq!(m.text.borrow().as_deref(), Some("[wait\n"));
}

/// The commands against a store: set, get, show with sources, unset, reset, path.
#[test]
fn test_commands_end_to_end() {
    let m = Mem::new(None);
    let a = |a: Action| run(&m, &a, &none()).unwrap();
    assert_eq!(a(Action::Get("wait.cap_ms".into())).stdout, "250\n");
    assert!(a(Action::Set("wait.cap_ms".into(), "120".into()))
        .stdout
        .contains("wait.cap_ms = 120"));
    a(Action::Set("spawn.window_timeout_ms".into(), "4000".into()));
    assert_eq!(a(Action::Get("wait.cap_ms".into())).stdout, "120\n");
    let show = a(Action::Show).stdout;
    assert!(
        show.contains("wait.cap_ms") && show.contains("120") && show.contains("file"),
        "{show}"
    );
    let line = show
        .lines()
        .find(|l| l.starts_with("wait.strategy"))
        .unwrap();
    assert!(
        line.contains("auto") && line.contains("default (not sent: the extension decides)"),
        "{line}"
    );
    let line = show
        .lines()
        .find(|l| l.starts_with("spawn.window_timeout_ms"))
        .unwrap();
    assert!(
        line.contains("4000") && line.contains("file") && !line.contains("not sent"),
        "{line}"
    );
    assert!(a(Action::Unset("wait.cap_ms".into()))
        .stdout
        .contains("removed"));
    assert!(a(Action::Unset("wait.cap_ms".into()))
        .stdout
        .contains("was not set"));
    assert_eq!(a(Action::Path).stdout, "mem.toml\n");
    assert!(a(Action::Reset).stdout.contains("spawn.window_timeout_ms"));
    assert_eq!(m.text.borrow().as_deref(), Some(""));
    assert!(a(Action::Reset).stdout.contains("no settings were set"));
}

/// show with a flag layer names the flag as the source and the flag wins over the file.
#[test]
fn test_show_reports_a_flag_as_the_source() {
    let m = Mem::new(Some("[wait]\ncap_ms = 100\n"));
    let (flags, _) = wait_flags(None, Some("300"));
    let show = run(&m, &Action::Show, &flags).unwrap().stdout;
    let line = show.lines().find(|l| l.starts_with("wait.cap_ms")).unwrap();
    assert!(line.contains("300") && line.contains("flag"), "{line}");
}

/// Reset leaves the update table and unknown keys alone.
#[test]
fn test_reset_keeps_foreign_and_unknown_keys() {
    let m = Mem::new(Some(
        "[update]\nchannel = \"beta\"\n[wait]\ncap_ms = 9\nodd = 1\n",
    ));
    run(&m, &Action::Reset, &none()).unwrap();
    let t = m.text.borrow().clone().unwrap();
    assert!(
        t.contains("channel = \"beta\"") && t.contains("odd = 1") && !t.contains("cap_ms"),
        "{t}"
    );
}

/// A file that can be read as text but breaks load (not valid TOML) never fails a load.
#[test]
fn test_load_through_a_store_never_fails() {
    let m = Mem::new(Some("= = ="));
    let l = load(&m);
    assert_eq!(l.settings, none());
    assert_eq!(l.warnings.len(), 1);
    assert!(load(&Mem::new(None)).warnings.is_empty());
}

/// Location: override first (used as given), then an absolute XDG_CONFIG_HOME, then ~/.config.
#[test]
fn test_config_location_precedence() {
    let p = |o, x, h| config_location(o, x, h).map(|p| p.to_string_lossy().into_owned());
    assert_eq!(
        p(Some("/x/c.toml"), Some("/xdg"), Some("/h")),
        Some("/x/c.toml".into())
    );
    assert_eq!(
        p(None, Some("/xdg"), Some("/h")),
        Some("/xdg/spawn-at/config.toml".into())
    );
    assert_eq!(
        p(None, Some("rel"), Some("/h")),
        Some("/h/.config/spawn-at/config.toml".into())
    );
    assert_eq!(
        p(Some(""), Some(""), Some("/h")),
        Some("/h/.config/spawn-at/config.toml".into())
    );
    assert_eq!(p(None, None, None), None);
}
