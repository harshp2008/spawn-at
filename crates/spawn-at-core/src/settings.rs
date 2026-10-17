//! # User settings: schema, validation, defaults, precedence and the `config` commands
//!
//! Pure and platform-neutral: nothing here touches the file system, the environment or a
//! terminal. Where the file lives and how it is read and written is a [`SettingsStore`] that a
//! platform supplies; the commands of `spawn-at config` are [`run`] against any store, so they
//! can be tested with an in-memory one. Callers pass environment values in (see
//! [`config_location`]); this module never reads them.
//!
//! The file is TOML and may hold tables this module does not own: it reads and edits only the keys
//! it knows ([`Key::ALL`]) and keeps every other key and table, though not their comments or
//! their order, because the file is parsed and written again ([`edit_text`]). The `update` table
//! is skipped without a warning when the file is read ([`load_text`]); any other unknown table or
//! key gets a warning.
//!
//! Precedence, highest first: a command-line flag, the config file, the built-in default
//! ([`effective`]). A setting that is not set anywhere sends nothing to the extension, so the
//! bytes of the arming instruction are those of a build without this module (see
//! [`wait_override`]).

// The Linux store is src/platform/linux/config_store.rs, and src/commands/config.rs and
// src/commands/spawn.rs are the callers. The `update` table belongs to the update checker in
// src/config.rs (not verified beyond the FOREIGN_TABLES constant below).

use crate::ready_wait::Policy;
use crate::wait_override::{WaitOverride, WaitStrategy};
use std::path::PathBuf;

/// Name of the environment variable that replaces the config file location with the path it
/// holds ([`config_location`] uses it as given). A [`SettingsStore`] should treat a path that does
/// not exist as "no config file": defaults only.
// The Linux store reads it (src/platform/linux/config_store.rs), and
// crates/spawn-at-verify/src/no_config.rs sets it to a path that does not exist (it keeps its own
// copy of the name).
pub const ENV_CONFIG_PATH: &str = "SPAWN_AT_CONFIG";

/// Smallest accepted `spawn.window_timeout_ms`, in milliseconds.
pub const WINDOW_TIMEOUT_MIN_MS: u32 = 200;
/// Largest accepted `spawn.window_timeout_ms`, in milliseconds.
pub const WINDOW_TIMEOUT_MAX_MS: u32 = 60_000;
/// Default of `spawn.window_timeout_ms`, in milliseconds: how long `spawn` waits for the window of
/// the command it started.
// src/commands/spawn.rs passes the value to `wait_claim` and also to `wait_for_spawn`, the poll
// that runs when the claim gave no window (grep -n "window_timeout" src/commands/spawn.rs). When
// the claim wait times out, both run, so the total wait can be up to twice the setting.
pub const WINDOW_TIMEOUT_DEFAULT_MS: u32 = 2000;

/// A setting the user can change. The dotted names (`wait.cap_ms`, ...) are what the user types
/// and what the file holds, so they are a contract; see [`Key::name`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// `wait.cap_ms`: cap of the ready-wait, in milliseconds.
    WaitCapMs,
    /// `wait.strategy`: which rule waits for the resized window.
    WaitStrategy,
    /// `spawn.window_timeout_ms`: how long `spawn` waits for the window to appear, in
    /// milliseconds.
    SpawnWindowTimeoutMs,
}

impl Key {
    /// Every key, in the order `config show` lists them.
    pub const ALL: [Key; 3] = [Key::WaitCapMs, Key::WaitStrategy, Key::SpawnWindowTimeoutMs];

    /// The dotted name used on the command line and as `[table] field` in the file.
    pub fn name(self) -> &'static str {
        match self {
            Key::WaitCapMs => "wait.cap_ms",
            Key::WaitStrategy => "wait.strategy",
            Key::SpawnWindowTimeoutMs => "spawn.window_timeout_ms",
        }
    }

    // The part of the dotted name before the first '.': the TOML table.
    fn table(self) -> &'static str {
        self.name().split('.').next().unwrap_or("")
    }

    // The part of the dotted name after the first '.': the field inside the table.
    fn field(self) -> &'static str {
        self.name().split('.').nth(1).unwrap_or("")
    }

    /// Reads a dotted key name (exact match, case-sensitive). Errors: the message names every
    /// valid key.
    pub fn parse(text: &str) -> Result<Key, String> {
        Key::ALL
            .into_iter()
            .find(|k| k.name() == text)
            .ok_or_else(|| {
                let all: Vec<&str> = Key::ALL.iter().map(|k| k.name()).collect();
                format!("unknown setting '{text}' (settings: {})", all.join(", "))
            })
    }

    /// What the setting does, in one line.
    // `grep -rn "\.about()" src/ crates/` finds no caller (`config show` prints `allowed`, not
    // this text).
    pub fn about(self) -> &'static str {
        match self {
            Key::WaitCapMs => "longest the extension waits for the app to answer a size request",
            Key::WaitStrategy => "which rule waits for the resized window",
            Key::SpawnWindowTimeoutMs => "how long spawn waits for the new window to appear",
        }
    }

    /// The allowed values in words, for error messages and the last column of `show`. The upper
    /// bound of `wait.cap_ms` is the hard limit of [`Policy::default`] (milliseconds).
    pub fn allowed(self) -> String {
        match self {
            Key::WaitCapMs => format!("whole milliseconds from 1 to {}", Policy::default().hard_ms),
            Key::WaitStrategy => "auto, legacy or ready-wait".to_string(),
            Key::SpawnWindowTimeoutMs => format!(
                "whole milliseconds from {WINDOW_TIMEOUT_MIN_MS} to {WINDOW_TIMEOUT_MAX_MS}"
            ),
        }
    }

    /// The built-in default: `wait.cap_ms` the cap of [`Policy::default`], `wait.strategy` `auto`,
    /// `spawn.window_timeout_ms` [`WINDOW_TIMEOUT_DEFAULT_MS`].
    pub fn default_value(self) -> Value {
        match self {
            Key::WaitCapMs => Value::Ms(Policy::default().cap_ms as u32),
            Key::WaitStrategy => Value::Strategy(WaitStrategy::Auto),
            Key::SpawnWindowTimeoutMs => Value::Ms(WINDOW_TIMEOUT_DEFAULT_MS),
        }
    }

    /// True for the settings that are sent to the extension (`wait.cap_ms`, `wait.strategy`): when
    /// they are not set, nothing is sent and the extension's own default applies.
    // Only render_show in this file calls it, to add the "(not sent...)" note.
    pub fn sent_to_extension(self) -> bool {
        matches!(self, Key::WaitCapMs | Key::WaitStrategy)
    }
}

/// A value of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// A number of milliseconds.
    Ms(u32),
    /// A wait rule.
    Strategy(WaitStrategy),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Ms(n) => write!(f, "{n}"),
            Value::Strategy(s) => f.write_str(s.as_str()),
        }
    }
}

/// Reads the text of a value for `key`: a strategy by [`WaitStrategy::parse`], or a number of
/// milliseconds written as decimal digits only (no sign, fraction or space) that fits `u32` and
/// the range of the key. Errors: the message is `<key> must be <allowed> (got '<text>')`.
pub fn parse_value(key: Key, text: &str) -> Result<Value, String> {
    let bad = || format!("{} must be {} (got '{text}')", key.name(), key.allowed());
    match key {
        Key::WaitStrategy => WaitStrategy::parse(text)
            .map(Value::Strategy)
            .ok_or_else(bad),
        Key::WaitCapMs | Key::SpawnWindowTimeoutMs => {
            // Digits only: no sign, no fraction, no spaces.
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            let n: u32 = text.parse().map_err(|_| bad())?;
            in_range(key, n).then_some(Value::Ms(n)).ok_or_else(bad)
        }
    }
}

// Range check for the number settings, inclusive on both ends. Always false for WaitStrategy,
// which has no number.
fn in_range(key: Key, n: u32) -> bool {
    match key {
        Key::WaitCapMs => n >= 1 && u64::from(n) <= Policy::default().hard_ms,
        Key::SpawnWindowTimeoutMs => (WINDOW_TIMEOUT_MIN_MS..=WINDOW_TIMEOUT_MAX_MS).contains(&n),
        Key::WaitStrategy => false,
    }
}

/// Where an effective value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Built in; nothing set it.
    Default,
    /// The config file.
    File,
    /// A command-line flag.
    Flag,
}

impl Source {
    /// The word `show` prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Default => "default",
            Source::File => "file",
            Source::Flag => "flag",
        }
    }
}

/// The settings one layer (the file, or the flags) sets. `None` is "not set by this layer".
/// One field per [`Key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    /// `wait.cap_ms`.
    pub wait_cap_ms: Option<u32>,
    /// `wait.strategy`.
    pub wait_strategy: Option<WaitStrategy>,
    /// `spawn.window_timeout_ms`.
    pub window_timeout_ms: Option<u32>,
}

impl Settings {
    /// The value this layer sets for `key`, if any.
    pub fn get(&self, key: Key) -> Option<Value> {
        match key {
            Key::WaitCapMs => self.wait_cap_ms.map(Value::Ms),
            Key::WaitStrategy => self.wait_strategy.map(Value::Strategy),
            Key::SpawnWindowTimeoutMs => self.window_timeout_ms.map(Value::Ms),
        }
    }

    // Stores `value` for `key`. A value of the wrong kind for the key (a strategy for a number
    // key, or the reverse) is silently ignored; the callers in this file always pair them right.
    fn put(&mut self, key: Key, value: Value) {
        match (key, value) {
            (Key::WaitCapMs, Value::Ms(n)) => self.wait_cap_ms = Some(n),
            (Key::WaitStrategy, Value::Strategy(s)) => self.wait_strategy = Some(s),
            (Key::SpawnWindowTimeoutMs, Value::Ms(n)) => self.window_timeout_ms = Some(n),
            _ => {}
        }
    }
}

/// The effective value of `key` and where it came from: flag, else file, else default.
pub fn effective(key: Key, flags: &Settings, file: &Settings) -> (Value, Source) {
    if let Some(v) = flags.get(key) {
        (v, Source::Flag)
    } else if let Some(v) = file.get(key) {
        (v, Source::File)
    } else {
        (key.default_value(), Source::Default)
    }
}

/// The flag layer for the wait settings, from the raw text of `--wait-strategy` and
/// `--wait-cap-ms`. A value that is not valid is dropped (the file value then applies) and one
/// sentence per dropped value is returned. The checks are those of [`WaitOverride::from_raw`].
/// `window_timeout_ms` is never set by flags here.
pub fn wait_flags(strategy: Option<&str>, cap_ms: Option<&str>) -> (Settings, Vec<String>) {
    let (parsed, problems) = WaitOverride::from_raw(strategy, cap_ms);
    let flags = Settings {
        wait_cap_ms: parsed.cap_ms,
        wait_strategy: strategy.and_then(WaitStrategy::parse),
        window_timeout_ms: None,
    };
    (flags, problems)
}

/// The override one spawn sends to the extension: per field the flag value, else the file value.
/// A setting that neither flag nor file sets stays out of it (`strategy` `Auto`, `cap_ms` `None`),
/// and `auto` is the same as nothing, so with no flags and no file the result is
/// [`WaitOverride::default`] and the instruction carries no extra field.
pub fn wait_override(flags: &Settings, file: &Settings) -> WaitOverride {
    WaitOverride {
        strategy: flags
            .wait_strategy
            .or(file.wait_strategy)
            .unwrap_or(WaitStrategy::Auto),
        cap_ms: flags.wait_cap_ms.or(file.wait_cap_ms),
    }
}

/// What reading a config file produced.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Loaded {
    /// The valid settings found; everything else is at its default.
    pub settings: Settings,
    /// One sentence per problem, ready to print as a warning. Never fatal.
    pub warnings: Vec<String>,
}

/// Line number (1-based) of byte offset `at` in `text`.
fn line_of(text: &str, at: usize) -> usize {
    text[..at.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

// Parses `text` as a TOML table. The error reads `<label>: line <n>: <message>`; the line is 1
// when the parser gives no position.
fn parse_table(text: &str, label: &str) -> Result<toml::Table, String> {
    text.parse::<toml::Table>().map_err(|e| {
        let line = e.span().map_or(1, |s| line_of(text, s.start));
        format!("{label}: line {line}: {}", e.message())
    })
}

/// Tables of the shared file that other parts of the program own; they are not unknown keys.
const FOREIGN_TABLES: [&str; 1] = ["update"];

/// Reads the settings from the text of a config file. `label` names the file in messages.
/// A file that is not valid TOML gives the defaults and one warning with the line number;
/// a wrong value (out of range, or the wrong TOML type: a string for a number key, or the reverse)
/// gives a warning and that key keeps its default; an unknown table or key gives a warning; a
/// known table that is not a table gives a warning. The `update` table is skipped silently.
/// Never fails.
pub fn load_text(text: &str, label: &str) -> Loaded {
    let mut out = Loaded::default();
    let table = match parse_table(text, label) {
        Ok(t) => t,
        Err(e) => {
            out.warnings
                .push(format!("{e} (not valid TOML); using the built-in defaults"));
            return out;
        }
    };
    for (name, item) in &table {
        let known_table = Key::ALL.iter().any(|k| k.table() == name);
        if FOREIGN_TABLES.contains(&name.as_str()) {
            continue;
        }
        if !known_table {
            out.warnings
                .push(format!("{label}: unknown key '{name}' ignored"));
            continue;
        }
        let Some(fields) = item.as_table() else {
            out.warnings
                .push(format!("{label}: '{name}' should be a table, ignored"));
            continue;
        };
        for (field, value) in fields {
            let dotted = format!("{name}.{field}");
            let Ok(key) = Key::parse(&dotted) else {
                out.warnings
                    .push(format!("{label}: unknown key '{dotted}' ignored"));
                continue;
            };
            let text = match value {
                toml::Value::Integer(n) => n.to_string(),
                toml::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            // A TOML string "250" is not a number setting; only the right type is accepted.
            let right_type = match key {
                Key::WaitStrategy => value.is_str(),
                _ => value.is_integer(),
            };
            match parse_value(key, &text) {
                Ok(v) if right_type => out.settings.put(key, v),
                _ => out.warnings.push(format!(
                    "{label}: ignoring {dotted} = {value} (expected {})",
                    key.allowed()
                )),
            }
        }
    }
    out
}

/// A file location and the means to read and write it. The platform supplies one.
pub trait SettingsStore {
    /// The place, for messages and `config path`; used as the `label` of parse warnings.
    fn location(&self) -> String;
    /// The whole text, or `None` when there is no file. A file that cannot be read is an error.
    fn read(&self) -> Result<Option<String>, String>;
    /// Replaces the whole text. An implementation should make the replacement all-or-nothing, so
    /// that a reader sees the old or the new file and never half of one.
    // The Linux store writes a temporary file and renames it over the target
    // (src/platform/linux/config_store.rs).
    fn write(&self, text: &str) -> Result<(), String>;
}

/// Reads the settings from `store` for a command that must not fail because of the file (a
/// spawn): a file that cannot be read or parsed gives the defaults and a warning. A missing file
/// gives the defaults and no warning.
pub fn load(store: &dyn SettingsStore) -> Loaded {
    match store.read() {
        Ok(Some(text)) => load_text(&text, &store.location()),
        Ok(None) => Loaded::default(),
        Err(e) => Loaded {
            settings: Settings::default(),
            warnings: vec![format!("{e}; using the built-in defaults")],
        },
    }
}

/// The path of the config file on a system that follows the XDG base directories, from the three
/// environment values. `override_path` (see [`ENV_CONFIG_PATH`]) wins and is used as given;
/// then `xdg_config_home` if it starts with `/`, giving `<xdg>/spawn-at/config.toml`; then
/// `<home>/.config/spawn-at/config.toml`. An empty string counts as not set. Returns `None` when
/// none of the three yields a path. Does not check that the path exists.
pub fn config_location(
    override_path: Option<&str>,
    xdg_config_home: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    fn set(v: Option<&str>) -> Option<&str> {
        v.filter(|s| !s.is_empty())
    }
    if let Some(p) = set(override_path) {
        return Some(PathBuf::from(p));
    }
    if let Some(x) = set(xdg_config_home).filter(|x| x.starts_with('/')) {
        return Some(PathBuf::from(x).join("spawn-at/config.toml"));
    }
    set(home).map(|h| PathBuf::from(h).join(".config/spawn-at/config.toml"))
}

/// Sets (`Some`) or removes (`None`) `key` in the file text and returns the new text. Every other
/// key and table is kept (comments and order are not). A file that is not valid TOML is an error:
/// it is never overwritten. After an unset, a table left with no field is removed. If the
/// key's table exists but is not a table (`wait = 3`), a set replaces it with a new table and an
/// unset leaves it as it is. Errors: the message ends "fix or remove the file before changing
/// settings". An empty result gives an empty string.
pub fn edit_text(
    text: &str,
    label: &str,
    key: Key,
    value: Option<Value>,
) -> Result<String, String> {
    let mut table = parse_table(text, label).map_err(|e| {
        format!("{e} (not valid TOML); fix or remove the file before changing settings")
    })?;
    let existing = table.remove(key.table());
    let mut fields = match existing {
        Some(toml::Value::Table(t)) => t,
        Some(other) => {
            // A wrong-typed `wait = 3` is replaced only by a set, never silently dropped by unset.
            if value.is_none() {
                table.insert(key.table().to_string(), other);
                return Ok(render(&table));
            }
            toml::Table::new()
        }
        None => toml::Table::new(),
    };
    match value {
        Some(Value::Ms(n)) => {
            fields.insert(key.field().to_string(), toml::Value::Integer(i64::from(n)));
        }
        Some(Value::Strategy(s)) => {
            fields.insert(
                key.field().to_string(),
                toml::Value::String(s.as_str().to_string()),
            );
        }
        None => {
            fields.remove(key.field());
        }
    }
    if !fields.is_empty() {
        table.insert(key.table().to_string(), toml::Value::Table(fields));
    }
    Ok(render(&table))
}

// The TOML text of `table`; an empty table gives an empty string (no file content), and a
// serialisation error also gives an empty string, because of `unwrap_or_default`.
fn render(table: &toml::Table) -> String {
    if table.is_empty() {
        String::new()
    } else {
        toml::to_string_pretty(table).unwrap_or_default()
    }
}

/// A `spawn-at config` action. The strings are the raw texts the user typed; [`run`] parses them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Every setting with its effective value and source.
    Show,
    /// The effective value of one setting (its dotted name).
    Get(String),
    /// Writes one setting to the file (dotted name, value text).
    Set(String, String),
    /// Removes one setting from the file (dotted name).
    Unset(String),
    /// The path of the file.
    Path,
    /// Removes every setting this module knows from the file.
    Reset,
}

/// What an action printed and what it wants said on stderr.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Output {
    /// Text for stdout, newline-terminated.
    pub stdout: String,
    /// Warnings for stderr; the caller prints them (this module prints nothing).
    pub warnings: Vec<String>,
}

/// The `show` table as text: a header row `KEY VALUE SOURCE ALLOWED`, then one row per [`Key`] in
/// [`Key::ALL`] order with its effective value, its source (with " (not sent: the extension
/// decides)" appended for a default of a setting sent to the extension) and the allowed values.
/// The first three columns are padded to their widest cell and separated by two spaces.
pub fn render_show(flags: &Settings, file: &Settings) -> String {
    let mut rows: Vec<[String; 4]> = vec![[
        "KEY".into(),
        "VALUE".into(),
        "SOURCE".into(),
        "ALLOWED".into(),
    ]];
    for key in Key::ALL {
        let (v, src) = effective(key, flags, file);
        let note = if src == Source::Default && key.sent_to_extension() {
            " (not sent: the extension decides)"
        } else {
            ""
        };
        rows.push([
            key.name().into(),
            v.to_string(),
            format!("{}{note}", src.as_str()),
            key.allowed(),
        ]);
    }
    let w: Vec<usize> = (0..3)
        .map(|c| rows.iter().map(|r| r[c].len()).max().unwrap_or(0))
        .collect();
    let mut s = String::new();
    for r in &rows {
        s.push_str(&format!(
            "{:<w0$}  {:<w1$}  {:<w2$}  {}\n",
            r[0],
            r[1],
            r[2],
            r[3],
            w0 = w[0],
            w1 = w[1],
            w2 = w[2]
        ));
    }
    s
}

/// Runs `action` against `store`. `flags` is the flag layer used by `show` and `get` (empty for
/// the plain commands). Errors are for the user: they name the allowed values.
///
/// Side effects: `Set`, `Unset` and `Reset` call [`SettingsStore::write`]. `Unset` writes only
/// when the text changed; `Reset` writes whenever a file exists, even if nothing was set.
/// `Show` and `Get` read through [`load`], so a bad file gives warnings, not an error; `Set`,
/// `Unset` and `Reset` fail on a file that is not valid TOML and leave it untouched.
pub fn run(store: &dyn SettingsStore, action: &Action, flags: &Settings) -> Result<Output, String> {
    let label = store.location();
    match action {
        Action::Path => Ok(Output {
            stdout: format!("{label}\n"),
            warnings: vec![],
        }),
        Action::Show | Action::Get(_) => {
            let key = match action {
                Action::Get(k) => Some(Key::parse(k)?),
                _ => None,
            };
            let loaded = load(store);
            let out = match key {
                Some(k) => format!("{}\n", effective(k, flags, &loaded.settings).0),
                None => render_show(flags, &loaded.settings),
            };
            Ok(Output {
                stdout: out,
                warnings: loaded.warnings,
            })
        }
        Action::Set(k, v) => {
            let key = Key::parse(k)?;
            let value = parse_value(key, v)?;
            let text = store.read()?.unwrap_or_default();
            let new = edit_text(&text, &label, key, Some(value))?;
            store.write(&new)?;
            Ok(Output {
                stdout: format!("{} = {} ({label})\n", key.name(), value),
                warnings: vec![],
            })
        }
        Action::Unset(k) => {
            let key = Key::parse(k)?;
            let Some(text) = store.read()? else {
                return Ok(Output {
                    stdout: format!("{} was not set (no config file)\n", key.name()),
                    warnings: vec![],
                });
            };
            let was_set = load_text(&text, &label).settings.get(key).is_some();
            let new = edit_text(&text, &label, key, None)?;
            if new != text {
                store.write(&new)?;
            }
            Ok(Output {
                stdout: if was_set {
                    format!("{} removed; the default applies\n", key.name())
                } else {
                    format!("{} was not set\n", key.name())
                },
                warnings: vec![],
            })
        }
        Action::Reset => {
            let Some(mut text) = store.read()? else {
                return Ok(Output {
                    stdout: "no config file; nothing to reset\n".into(),
                    warnings: vec![],
                });
            };
            let mut removed = Vec::new();
            for key in Key::ALL {
                let was = parse_table(&text, &label)
                    .ok()
                    .and_then(|t| t.get(key.table())?.as_table()?.get(key.field()).cloned())
                    .is_some();
                text = edit_text(&text, &label, key, None)?;
                if was {
                    removed.push(key.name());
                }
            }
            store.write(&text)?;
            Ok(Output {
                stdout: if removed.is_empty() {
                    "no settings were set\n".into()
                } else {
                    format!("removed: {}\n", removed.join(", "))
                },
                warnings: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests;
