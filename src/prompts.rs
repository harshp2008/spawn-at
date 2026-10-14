//! Every dialoguer prompt and item list of the program, in one place, with a guard test.
//!
//! dialoguer redraws a prompt by counting its unwrapped lines. A prompt that wraps on a narrow
//! terminal therefore leaves a copy of itself behind (the question appeared twice). So a
//! prompt must stay short enough to render on one line of 70 columns: "? " + prompt + a
//! " [Y/n] " suffix is at most 70 characters, an item with its "❯ " marker too. The longer
//! explanation is printed as plain text in the lines above the question. dialoguer adds its
//! own punctuation after a prompt, so a prompt never ends with ':'.

/// Longest rendered line, in characters.
#[cfg(test)]
pub const MAX_LINE: usize = 70;
/// Characters dialoguer adds around a prompt ("? " in front, " [Y/n] " or " › " behind).
#[cfg(test)]
const PROMPT_OVERHEAD: usize = 10;
/// Characters dialoguer adds in front of an item ("❯ ").
#[cfg(test)]
const ITEM_OVERHEAD: usize = 2;

/// Why a prompt or its items do not fit, if they do not.
#[cfg(test)]
pub fn check(prompt: &str, items: &[&str]) -> Result<(), String> {
    let n = prompt.chars().count();
    if n + PROMPT_OVERHEAD > MAX_LINE {
        return Err(format!("prompt too long ({n} characters): {prompt}"));
    }
    if prompt.ends_with(':') {
        return Err(format!("prompt ends with ':': {prompt}"));
    }
    for item in items {
        let n = item.chars().count();
        if n + ITEM_OVERHEAD > MAX_LINE {
            return Err(format!("item too long ({n} characters): {item}"));
        }
        if item.ends_with(':') {
            return Err(format!("item ends with ':': {item}"));
        }
    }
    Ok(())
}

// --- install ---
pub const PROMPT_INSTALL_ANYWAY: &str = "Install anyway?";
pub const PROMPT_SCOPE: &str = "Where should the 'spawn-at' executable be installed?";
pub const ITEMS_SCOPE: &[&str] = &[
    "Current user only (~/.local/bin) [Recommended]",
    "System-wide for all users (/usr/local/bin - requires sudo)",
    "Skip binary installation (compositor setup only)",
];
pub const PROMPT_FALLBACK_USER: &str = "Install to ~/.local/bin (your user directory) instead?";
pub const PROMPT_ENABLE_EXTENSION: &str =
    "Automatically enable the spawn-at GNOME Shell extension?";
pub const PROMPT_UPDATE_CHANNEL: &str = "Select the update channel";
pub const ITEMS_CHANNEL: &[&str] = &[
    "Official releases only (stable) [Recommended]",
    "Include beta/pre-releases (beta)",
];
pub const PROMPT_UPDATE_NOTIFY: &str = "Show update notifications in the terminal?";
/// Printed above the notifications question.
pub const NOTE_UPDATE_NOTIFY: &str =
    "spawn-at can print a notice in the terminal when a newer version is available.";

// --- uninstall ---
pub const PROMPT_REMOVE_BINARY: &str = "Do you want to remove the global 'spawn-at' executable?";
pub const ITEMS_REMOVE_BINARY: &[&str] = &[
    "Remove from your user directory (~/.local/bin) [Recommended]",
    "Remove from /usr/local/bin (system-wide, requires sudo)",
    "Skip binary removal (compositor cleanup only)",
];
pub const PROMPT_DELETE_EXTENSION_FILES: &str = "Delete the extension files?";
/// Printed above the delete question.
pub const NOTE_DELETE_EXTENSION_FILES: &str =
    "The extension is installed in ~/.local/share/gnome-shell/extensions.";

// --- update ---
pub const PROMPT_UPDATE_MENU: &str = "What would you like to do?";
pub const PROMPT_RELEASE_CHANNEL: &str = "Select the release channel";

// --- existing install (platform/upgrade.rs) ---
pub const PROMPT_EXISTING: &str = "What do you want to do?";

// --- end of run (session.rs) ---
pub const PROMPT_LOGOUT: &str = "Log out of your session now?";
pub const NOTE_LOGOUT: &str =
    "Logging out closes all your apps. The extension loads at your next log-in.";
pub const NOTE_LOGOUT_UPDATE: &str =
    "Logging out closes all your apps. It completes the reload of the extension.";
pub const ITEMS_LOGOUT: &[&str] = &[
    "No - Keep session running [Default]",
    "Yes - Log out now (closes all apps)",
];
pub const PROMPT_RELOAD: &str = "Reload GNOME Shell now?";
pub const NOTE_RELOAD: &str = "Reloading keeps your apps open (X11 only).";
pub const ITEMS_RELOAD: &[&str] = &[
    "Yes - Reload GNOME Shell now (apps stay open) [Default]",
    "No - Keep session running",
];

/// Every prompt with its items, for the guard test. The test also checks that every
/// `PROMPT_*` and `ITEMS_*` constant above is listed here.
#[cfg(test)]
const ALL: &[(&str, &[&str])] = &[
    (PROMPT_INSTALL_ANYWAY, &[]),
    (PROMPT_SCOPE, ITEMS_SCOPE),
    (PROMPT_FALLBACK_USER, &[]),
    (PROMPT_ENABLE_EXTENSION, &[]),
    (PROMPT_UPDATE_CHANNEL, ITEMS_CHANNEL),
    (PROMPT_UPDATE_NOTIFY, &[]),
    (PROMPT_REMOVE_BINARY, ITEMS_REMOVE_BINARY),
    (PROMPT_DELETE_EXTENSION_FILES, &[]),
    (PROMPT_UPDATE_MENU, &[]),
    (PROMPT_RELEASE_CHANNEL, ITEMS_CHANNEL),
    (PROMPT_EXISTING, &[]),
    (PROMPT_LOGOUT, ITEMS_LOGOUT),
    (PROMPT_RELOAD, ITEMS_RELOAD),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every registered prompt and item fits on one 70-column line and has no ':' at the end.
    #[test]
    fn test_every_prompt_fits_and_has_no_trailing_colon() {
        for (prompt, items) in ALL {
            check(prompt, items).unwrap();
        }
    }

    /// The check itself: a wrapped prompt (the 88-character notifications question), a long item
    /// and a trailing colon are all refused.
    #[test]
    fn test_check_refuses_long_and_colon() {
        let long =
            "Enable visual update notifications in terminal when a newer version is available?";
        assert_eq!(long.chars().count(), 81);
        assert!(check(long, &[]).is_err());
        assert!(check("Select update channel:", &[]).is_err());
        assert!(check("Fine?", &["a".repeat(69).as_str()]).is_err());
        assert!(check("Fine?", &["a".repeat(68).as_str()]).is_ok());
        assert!(check(&"p".repeat(61), &[]).is_err());
        assert!(check(&"p".repeat(60), &[]).is_ok());
    }

    /// No constant was left out of ALL, and the notes (plain text above a question) are not
    /// mistaken for prompts.
    #[test]
    fn test_registry_lists_every_prompt_constant() {
        let src = include_str!("prompts.rs");
        let body = src.split("#[cfg(test)]\nconst ALL").nth(1).unwrap();
        let body = body.split("#[cfg(test)]\nmod tests").next().unwrap();
        for line in src.lines() {
            for prefix in ["pub const PROMPT_", "pub const ITEMS_"] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let name = rest.split(':').next().unwrap();
                    let full = format!("{}{name}", &prefix["pub const ".len()..]);
                    assert!(body.contains(&full), "{full} is missing from ALL");
                }
            }
        }
    }

    /// No prompt in the program is a string literal outside this file: every `with_prompt(`
    /// takes a constant from here (or the `prompt` of a Question, tested separately), so the
    /// checks above cover them all; and no `.items(&[` literal list is left.
    #[test]
    fn test_no_prompt_literals_elsewhere() {
        fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push((
                        p.display().to_string(),
                        std::fs::read_to_string(&p).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        for (path, src) in &files {
            if path.ends_with("prompts.rs") {
                continue;
            }
            let src = src.split("#[cfg(test)]").next().unwrap();
            let squashed: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
            for call in squashed.split(".with_prompt(").skip(1) {
                assert!(
                    !call.trim_start().starts_with('"'),
                    "{path}: a prompt literal; put it in prompts.rs"
                );
            }
            for call in squashed.split("Select::new()").skip(1) {
                let chain = call.split(".interact").next().unwrap();
                assert!(
                    !chain.contains(".items(&["),
                    "{path}: an item list literal; put it in prompts.rs"
                );
            }
        }
    }
}
