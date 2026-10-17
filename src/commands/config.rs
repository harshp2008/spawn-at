//! `spawn-at config`: show, get, set, unset, path and reset for the user's settings. It needs no
//! compositor; the rules live in `spawn_at_core::settings`, the file in `LinuxConfigStore`.
//!
//! This module only maps the parsed arguments to a `settings::Action` and prints the result. It
//! must not hold setting names, defaults or validation rules.

use crate::cli::args::{ConfigArgs, ConfigCommands};
use crate::platform::linux::config_store::LinuxConfigStore;
use spawn_at_core::settings::{self, Action, SettingsStore};

/// Runs the command against `store` and returns the text for stdout (not printed here).
///
/// Warnings go to stderr as they arise: first any dropped `--wait-strategy` / `--wait-cap-ms`
/// value, then those returned by `settings::run`. No subcommand means `show`. An error is the
/// message from `settings::run`; `main` prints it and exits 1.
///
/// Side effects: `set`, `unset` and `reset` write through `store`; see `settings::run`.
pub fn run_config_with(store: &dyn SettingsStore, args: ConfigArgs) -> Result<String, String> {
    let (flags, problems) =
        settings::wait_flags(args.wait_strategy.as_deref(), args.wait_cap_ms.as_deref());
    for p in &problems {
        crate::diagnostics::render_warning(p);
    }
    let action = match args.action.unwrap_or(ConfigCommands::Show) {
        ConfigCommands::Show => Action::Show,
        ConfigCommands::Get { key } => Action::Get(key),
        ConfigCommands::Set { key, value } => Action::Set(key, value),
        ConfigCommands::Unset { key } => Action::Unset(key),
        ConfigCommands::Path => Action::Path,
        ConfigCommands::Reset => Action::Reset,
    };
    let out = settings::run(store, &action, &flags)?;
    for w in &out.warnings {
        crate::diagnostics::render_warning(w);
    }
    Ok(out.stdout)
}

/// [`run_config_with`] on the user's config file, printing its output to stdout.
///
/// Fails with "cannot find the config directory: ..." before any subcommand runs (`path`
/// included) when no config file location can be derived.
pub fn run_config(args: ConfigArgs) -> Result<(), String> {
    let store = LinuxConfigStore::from_env();
    // The check applies to every subcommand, not only `path`. `SPAWN_AT_CONFIG` naming a file
    // makes it pass without HOME. The message here differs from the one in
    // `LinuxConfigStore::path` ("HOME is not set").
    if store.path().is_err() {
        return Err("cannot find the config directory: set HOME or XDG_CONFIG_HOME".into());
    }
    print!("{}", run_config_with(&store, args)?);
    Ok(())
}
