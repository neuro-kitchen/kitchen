//! The Code workspace's own keys: the `[code]` section of `keybindings.toml`, bound in the
//! workspace's key context only. Each space of an app binds its own keys this way (lucem's canvas
//! has `[canvas]`), so the same key can mean one thing over the code and another over an image.
//!
//! | Command | Default | What it does |
//! |---|---|---|
//! | `toggle_changes` | Ctrl/⌘ + Alt + G | change marks on or off |
//! | `open_external` | Ctrl/⌘ + Shift + E | open the script in the external editor |
//! | `run` | Ctrl/⌘ + Enter | run the script |
//! | `save` | Ctrl/⌘ + S | save the script |
//!
//! `secondary` is ⌘ on macOS and Ctrl elsewhere. The user's file wins; a key that cannot be read
//! keeps its default and is named in kitchen-shell's notices.

use std::rc::Rc;
use std::sync::Mutex;

use gpui_kit::{Action, App, KeyBinding, KeyBindingContextPredicate, actions};
use kitchen_shell::keys::{Keys, resolve};

actions!(
    kitchen_code,
    [
        /// Change marks on or off.
        ToggleChanges,
        /// The open script in the external editor.
        OpenExternal,
        /// Runs the open script.
        RunScript,
        /// Saves the open script.
        SaveScript,
    ]
);

/// The workspace's key context: its keys only work there.
pub const CONTEXT: &str = "CodeWorkspace";

/// The section of `keybindings.toml`.
pub const SECTION: &str = "code";

/// The workspace's commands and their default keys.
pub const CODE_KEYS: &[(&str, &[&str])] = &[
    ("toggle_changes", &["secondary-alt-g"]),
    ("open_external", &["secondary-shift-e"]),
    ("run", &["secondary-enter"]),
    ("save", &["secondary-s"]),
];

static RESOLVED: Mutex<Option<Keys>> = Mutex::new(None);

fn action(name: &str) -> Option<Box<dyn Action>> {
    Some(match name {
        "toggle_changes" => Box::new(ToggleChanges),
        "open_external" => Box::new(OpenExternal),
        "run" => Box::new(RunScript),
        "save" => Box::new(SaveScript),
        _ => return None,
    })
}

/// Binds the workspace's keys: the defaults, or what `[code]` says. Call once at start, after
/// kitchen-shell's `load_keybindings` (an app binding its spaces' keys does it with the others).
pub fn bind_keys(cx: &mut App) -> Keys {
    let context = KeyBindingContextPredicate::parse(CONTEXT).ok().map(Rc::new);
    let load = |key: &str, name: &str, cx: &App| action(name).and_then(|a| KeyBinding::load(key, a, context.clone(), false, None, cx).ok());
    let resolved = resolve(SECTION, CODE_KEYS, |key| load(key, "save", cx).is_some());
    let bindings: Vec<KeyBinding> = resolved.iter().flat_map(|(name, keys)| keys.iter().filter_map(|key| load(key, name, cx))).collect();
    cx.bind_keys(bindings);
    *RESOLVED.lock().unwrap_or_else(|e| e.into_inner()) = Some(resolved.clone());
    resolved
}

/// The first key of `command` as people read it (`Ctrl+Alt+G`, `⌘⌥G` on macOS), for tooltips;
/// the default when the keys were not bound yet, none when it has no key.
pub fn key_label(command: &str) -> Option<String> {
    let bound = RESOLVED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|keys| keys.iter().find(|(n, _)| n == command).and_then(|(_, k)| k.first().cloned()));
    let key = bound.or_else(|| CODE_KEYS.iter().find(|(n, _)| *n == command).and_then(|(_, k)| k.first().map(|s| s.to_string())))?;
    Some(readable(&key, cfg!(target_os = "macos")))
}

/// `secondary-alt-g` as `Ctrl+Alt+G`, or `⌘⌥G` on macOS.
pub fn readable(key: &str, mac: bool) -> String {
    let parts: Vec<&str> = key.split('-').collect();
    let (mods, last) = parts.split_at(parts.len().saturating_sub(1));
    let name = match last.first().copied().unwrap_or("") {
        "enter" => if mac { "↩".to_string() } else { "Enter".to_string() },
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    };
    let mods: Vec<&str> = mods
        .iter()
        .map(|m| match (*m, mac) {
            ("secondary" | "cmd" | "super", true) => "⌘",
            ("ctrl", true) => "⌃",
            ("alt", true) => "⌥",
            ("shift", true) => "⇧",
            ("secondary" | "ctrl", false) => "Ctrl",
            ("cmd" | "super", false) => "Super",
            ("alt", false) => "Alt",
            ("shift", false) => "Shift",
            (other, _) => other,
        })
        .collect();
    if mac { format!("{}{name}", mods.concat()) } else { mods.iter().copied().chain([name.as_str()]).collect::<Vec<_>>().join("+") }
}

/// `what (key)` for a tooltip, or `what` when the command has no key.
pub fn with_key(what: &str, command: &str) -> String {
    match key_label(command) {
        Some(key) => format!("{what} ({key})"),
        None => what.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_read_as_people_write_them() {
        assert_eq!(readable("secondary-alt-g", false), "Ctrl+Alt+G");
        assert_eq!(readable("secondary-alt-g", true), "⌘⌥G");
        assert_eq!(readable("secondary-enter", false), "Ctrl+Enter");
        assert_eq!(readable("secondary-shift-e", true), "⌘⇧E");
        assert_eq!(readable("f3", false), "F3");
        // Every default names a command the workspace handles
        assert!(CODE_KEYS.iter().all(|(name, _)| action(name).is_some()));
    }
}
