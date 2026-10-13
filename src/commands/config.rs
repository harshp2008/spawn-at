//! `spawn-at config`: show, get, set, unset, path and reset for the user's settings. It needs no
//! compositor; the rules live in `spawn_at_core::settings`, the file in `LinuxConfigStore`.

use crate::cli::args::{ConfigArgs, ConfigCommands};
use crate::platform::linux::config_store::LinuxConfigStore;
use spawn_at_core::settings::{self, Action, SettingsStore};

/// Runs the command against `store` and returns what to print on stdout; warnings are rendered
/// on stderr. An error is a message for the user and exit code 1.
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

/// [`run_config_with`] on the user's config file.
pub fn run_config(args: ConfigArgs) -> Result<(), String> {
    let store = LinuxConfigStore::from_env();
    // `path` works even when HOME is unset only if SPAWN_AT_CONFIG names a file.
    if store.path().is_err() {
        return Err("cannot find the config directory: set HOME or XDG_CONFIG_HOME".into());
    }
    print!("{}", run_config_with(&store, args)?);
    Ok(())
}
