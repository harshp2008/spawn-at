//! `spawn-at config` as a user runs it: the built binary, a temporary `XDG_CONFIG_HOME`, no
//! compositor needed. The user's real config is never touched: every run sets its own
//! `XDG_CONFIG_HOME` (or `HOME`) and clears `SPAWN_AT_CONFIG`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let d = std::env::temp_dir().join(format!("spawn-at-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Home(d)
    }
    fn file(&self) -> PathBuf {
        self.0.join("spawn-at/config.toml")
    }
    fn run(&self, args: &[&str]) -> Output {
        self.run_env(args, &[])
    }
    fn run_env(&self, args: &[&str], env: &[(&str, &Path)]) -> Output {
        let mut c = Command::new(env!("CARGO_BIN_EXE_spawn-at"));
        c.arg("config")
            .args(args)
            .env_remove("SPAWN_AT_CONFIG")
            .env("XDG_CONFIG_HOME", &self.0)
            .env("HOME", self.0.join("nohome"));
        for (k, v) in env {
            c.env(k, v);
        }
        c.output().unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// `config` alone is `config show`; with no file everything is the default and no file is made.
#[test]
fn test_bare_config_is_show_and_writes_nothing() {
    let h = Home::new("bare");
    let o = h.run(&[]);
    assert!(o.status.success(), "{}", err(&o));
    assert_eq!(out(&o), out(&h.run(&["show"])));
    let text = out(&o);
    for key in ["wait.cap_ms", "wait.strategy", "spawn.window_timeout_ms"] {
        assert!(text.contains(key), "{text}");
    }
    assert!(text.contains("250") && text.contains("2000"), "{text}");
    assert!(!h.file().exists());
    assert_eq!(err(&o), "");
}

/// set writes under XDG_CONFIG_HOME, get and show report the file as the source, unset and reset
/// take the values out again.
#[test]
fn test_set_get_show_unset_reset_round_trip() {
    let h = Home::new("round");
    assert!(h.run(&["set", "wait.cap_ms", "120"]).status.success());
    assert!(h
        .run(&["set", "wait.strategy", "ready-wait"])
        .status
        .success());
    assert!(h
        .run(&["set", "spawn.window_timeout_ms", "5000"])
        .status
        .success());
    assert!(h.file().exists());
    assert_eq!(out(&h.run(&["get", "wait.cap_ms"])), "120\n");
    assert_eq!(out(&h.run(&["get", "wait.strategy"])), "ready-wait\n");
    let show = out(&h.run(&["show"]));
    assert_eq!(show.matches(" file").count(), 3, "{show}");
    assert_eq!(out(&h.run(&["path"])).trim(), h.file().to_str().unwrap());
    assert!(h.run(&["unset", "wait.cap_ms"]).status.success());
    assert_eq!(out(&h.run(&["get", "wait.cap_ms"])), "250\n");
    assert!(out(&h.run(&["reset"])).contains("wait.strategy"));
    assert_eq!(out(&h.run(&["get", "spawn.window_timeout_ms"])), "2000\n");
}

/// Without XDG_CONFIG_HOME the file is under HOME/.config.
#[test]
fn test_falls_back_to_home_dot_config() {
    let h = Home::new("home");
    let o = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .args(["config", "path"])
        .env_remove("SPAWN_AT_CONFIG")
        .env_remove("XDG_CONFIG_HOME")
        .env("HOME", &h.0)
        .output()
        .unwrap();
    assert_eq!(
        out(&o).trim(),
        h.0.join(".config/spawn-at/config.toml").to_str().unwrap()
    );
}

/// Bad values and keys are refused with the allowed range or values, exit code 1, no file made.
#[test]
fn test_validation_errors_name_the_allowed_values() {
    let h = Home::new("bad");
    for (args, needle) in [
        (vec!["set", "wait.cap_ms", "1501"], "1 to 1500"),
        (vec!["set", "wait.cap_ms", "fast"], "whole milliseconds"),
        (
            vec!["set", "wait.strategy", "quick"],
            "auto, legacy or ready-wait",
        ),
        (vec!["set", "spawn.window_timeout_ms", "10"], "200 to 60000"),
        (vec!["get", "wait.cap"], "unknown setting"),
        (vec!["unset", "nope"], "wait.strategy"),
    ] {
        let o = h.run(&args);
        assert_eq!(o.status.code(), Some(1), "{args:?}");
        assert!(err(&o).contains(needle), "{args:?}: {}", err(&o));
    }
    assert!(!h.file().exists());
}

/// set keeps unknown keys and the update preferences that live in the same file.
#[test]
fn test_set_keeps_unknown_keys_and_the_update_table() {
    let h = Home::new("keep");
    std::fs::create_dir_all(h.file().parent().unwrap()).unwrap();
    std::fs::write(
        h.file(),
        "[update]\nchannel = \"beta\"\n[extra]\nk = \"v\"\n",
    )
    .unwrap();
    let o = h.run(&["set", "wait.cap_ms", "90"]);
    assert!(o.status.success(), "{}", err(&o));
    let text = std::fs::read_to_string(h.file()).unwrap();
    for kept in ["channel = \"beta\"", "[extra]", "k = \"v\"", "cap_ms = 90"] {
        assert!(text.contains(kept), "{kept} lost:\n{text}");
    }
}

/// An unknown key is a warning, never a failure; the known ones still apply.
#[test]
fn test_unknown_key_in_the_file_warns_and_succeeds() {
    let h = Home::new("unknown");
    std::fs::create_dir_all(h.file().parent().unwrap()).unwrap();
    std::fs::write(h.file(), "[wait]\ncap_ms = 120\ncolour = 1\n").unwrap();
    let o = h.run(&["show"]);
    assert!(o.status.success());
    assert!(
        err(&o).contains("WARNING") && err(&o).contains("wait.colour"),
        "{}",
        err(&o)
    );
    assert!(out(&o).contains("120"));
}

/// A malformed file is one warning with the line number and the defaults; set refuses to
/// overwrite it and says so.
#[test]
fn test_malformed_file_warns_with_the_line_and_set_refuses() {
    let h = Home::new("malformed");
    std::fs::create_dir_all(h.file().parent().unwrap()).unwrap();
    let bad = "[wait]\ncap_ms = 120\n\n[spawn\n";
    std::fs::write(h.file(), bad).unwrap();
    let o = h.run(&["show"]);
    assert!(o.status.success());
    assert_eq!(err(&o).matches("WARNING").count(), 1, "{}", err(&o));
    assert!(err(&o).contains("line 4"), "{}", err(&o));
    assert!(out(&o).contains("250"), "defaults apply");
    let o = h.run(&["set", "wait.cap_ms", "90"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(err(&o).contains("line 4"), "{}", err(&o));
    assert_eq!(std::fs::read_to_string(h.file()).unwrap(), bad);
}

/// The flag layer: a flag is the source and beats the file (hidden flags, as on `spawn`).
#[test]
fn test_flag_beats_the_file_in_show() {
    let h = Home::new("flag");
    h.run(&["set", "wait.cap_ms", "100"]);
    let o = h.run(&["--wait-cap-ms", "300", "show"]);
    let line = out(&o)
        .lines()
        .find(|l| l.starts_with("wait.cap_ms"))
        .unwrap()
        .to_string();
    assert!(line.contains("300") && line.contains("flag"), "{line}");
}

/// SPAWN_AT_CONFIG names the file to use; one that does not exist means "no config" even with an
/// XDG_CONFIG_HOME that holds a config with a different cap. This is how the verify harness keeps
/// a user's config out of a run.
#[test]
fn test_config_path_variable_overrides_xdg_and_a_missing_file_means_defaults() {
    let h = Home::new("env");
    h.run(&["set", "wait.cap_ms", "7"]);
    assert_eq!(out(&h.run(&["get", "wait.cap_ms"])), "7\n");
    let none = h.0.join("does/not/exist.toml");
    let o = h.run_env(&["get", "wait.cap_ms"], &[("SPAWN_AT_CONFIG", &none)]);
    assert_eq!(out(&o), "250\n");
    assert_eq!(err(&o), "");
    let o = h.run_env(&["path"], &[("SPAWN_AT_CONFIG", &none)]);
    assert_eq!(out(&o).trim(), none.to_str().unwrap());
}

/// `config --help` lists the commands and `spawn-at --help` lists `config`.
#[test]
fn test_help_lists_config() {
    let o = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(out(&o).contains("config"), "{}", out(&o));
    let o = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .args(["config", "--help"])
        .output()
        .unwrap();
    for w in ["show", "get", "set", "unset", "path", "reset"] {
        assert!(out(&o).contains(w), "{}", out(&o));
    }
    assert!(!out(&o).contains("wait-cap-ms"), "the flags stay hidden");
}
