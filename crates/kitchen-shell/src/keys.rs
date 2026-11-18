//! Key bindings the user edits: `keybindings.toml` in the app's configuration folder, a key or a
//! list of keys per command, in sections (`[commands]` for the registry's commands, and one per
//! part of an app that binds its own keys, as a plot's `[plot]`). Read once at start
//! ([`load_keybindings`]); each section is resolved against its defaults ([`resolve`]); a
//! missing file is written with every default ([`write_default_keybindings`]) so the user sees
//! what can be changed. A key that cannot be read keeps its default and is named in a notice
//! ([`take_notices`]).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use gpui_kit::{App, SharedString};
use toml_edit::DocumentMut;

use crate::command::Registry;

/// A section's commands with their keys.
pub type Keys = Vec<(String, Vec<String>)>;

#[derive(Default)]
struct Keymap {
    /// The file: section → command → keys.
    file: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// The defaults of every section resolved so far, in order (what the file is written from).
    defaults: Vec<(String, Keys)>,
    notices: Vec<String>,
}

static KEYMAP: Mutex<Option<Keymap>> = Mutex::new(None);

fn with_keymap<R>(f: impl FnOnce(&mut Keymap) -> R) -> R {
    let mut guard = KEYMAP.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Keymap::default))
}

/// Reads `keybindings.toml` (a missing file: every default). Call once at start, before any
/// section is resolved.
pub fn load_keybindings(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let (file, notices) = parse_keybindings(&text);
    with_keymap(|k| {
        k.file = file;
        k.notices.extend(notices);
    });
}

fn parse_keybindings(text: &str) -> (BTreeMap<String, BTreeMap<String, Vec<String>>>, Vec<String>) {
    let mut notices = Vec::new();
    let mut file = BTreeMap::new();
    let doc: DocumentMut = match text.parse() {
        Ok(d) => d,
        Err(e) => return (file, vec![format!("keybindings.toml: {e}")]),
    };
    for (section, item) in doc.iter() {
        let mut out = BTreeMap::new();
        match item.as_table_like() {
            Some(table) => flatten(section, "", table, &mut out, &mut notices),
            None => notices.push(format!("keybindings.toml: {section} must be a section ([{section}])")),
        }
        file.insert(section.to_string(), out);
    }
    (file, notices)
}

/// `a.b = "key"` is read as the command `a.b` (TOML makes it a table inside a table).
fn flatten(section: &str, prefix: &str, table: &dyn toml_edit::TableLike, out: &mut BTreeMap<String, Vec<String>>, notices: &mut Vec<String>) {
    for (key, item) in table.iter() {
        let name = if prefix.is_empty() { key.to_string() } else { format!("{prefix}.{key}") };
        if let Some(text) = item.as_str() {
            out.insert(name, if text.is_empty() { Vec::new() } else { vec![text.to_string()] });
        } else if let Some(list) = item.as_array() {
            let keys: Vec<String> = list.iter().filter_map(|v| v.as_str()).filter(|t| !t.is_empty()).map(str::to_string).collect();
            if keys.len() != list.iter().filter(|v| v.as_str() != Some("")).count() {
                notices.push(format!("keybindings.toml: [{section}] {name}: every key must be text in quotes"));
            } else {
                out.insert(name, keys);
            }
        } else if let Some(inner) = item.as_table_like() {
            flatten(section, &name, inner, out, notices);
        } else {
            notices.push(format!("keybindings.toml: [{section}] {name} must be a key in quotes, or a list of keys"));
        }
    }
}

/// The keys of `section`'s commands: the file's where it names the command, else `defaults`.
/// `valid` says whether a key can be bound (one that cannot keeps the default). Commands the file
/// names that `defaults` lacks, and one key on two commands, are noticed.
pub fn resolve(section: &str, defaults: &[(&str, &[&str])], valid: impl Fn(&str) -> bool) -> Keys {
    with_keymap(|k| {
        let defaults: Keys = defaults.iter().map(|(name, keys)| (name.to_string(), keys.iter().map(|s| s.to_string()).collect())).collect();
        let empty = BTreeMap::new();
        let file = k.file.get(section).unwrap_or(&empty);
        for name in file.keys() {
            if !defaults.iter().any(|(n, _)| n == name) {
                k.notices.push(format!("keybindings.toml: [{section}] has no command {name:?}"));
            }
        }
        let mut out = Keys::new();
        for (name, default) in &defaults {
            let keys = match file.get(name) {
                Some(keys) => match keys.iter().find(|key| !valid(key)) {
                    Some(bad) => {
                        k.notices.push(format!("keybindings.toml: [{section}] {name}: {bad:?} is not a key; the default is kept"));
                        default.clone()
                    }
                    None => keys.clone(),
                },
                None => default.clone(),
            };
            out.push((name.clone(), keys));
        }
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for (name, keys) in &out {
            for key in keys {
                if let Some(other) = seen.insert(key, name)
                    && other != name
                {
                    k.notices.push(format!("keybindings.toml: [{section}] {key:?} is bound to both {other} and {name}"));
                }
            }
        }
        k.defaults.retain(|(s, _)| s != section);
        k.defaults.push((section.to_string(), defaults));
        out
    })
}

/// Writes `keybindings.toml` with every default of the sections resolved so far, when the file
/// does not exist yet.
pub fn write_default_keybindings(path: &Path, app: &str) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    let text = with_keymap(|k| default_keybindings(app, &k.defaults));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

fn default_keybindings(app: &str, sections: &[(String, Keys)]) -> String {
    let mut text = format!(
        "# {app} key bindings: command = \"key\", or a list of keys; \"\" removes a binding.\n# Keys are written as ctrl-shift-o, alt-g, space, left, f3, +. Read when the app starts.\n# Delete a line to go back to its default.\n",
    );
    for (section, keys) in sections {
        text.push_str(&format!("\n[{section}]\n"));
        // A command id that is also the start of another (`a.b` and `a.b.c`) cannot be a dotted key
        let bare = |name: &str| name.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) && !keys.iter().any(|(other, _)| other.starts_with(&format!("{name}.")));
        for (name, bound) in keys {
            let name = if bare(name) { name.clone() } else { format!("{name:?}") };
            let value = match bound.as_slice() {
                [one] => format!("{one:?}"),
                many => format!("[{}]", many.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(", ")),
            };
            text.push_str(&format!("{name} = {value}\n"));
        }
    }
    text
}

/// The notices collected so far (each is given once).
pub fn take_notices() -> Vec<String> {
    with_keymap(|k| std::mem::take(&mut k.notices))
}

/// A key as the files write it, for matching by hand: (control or command, alt, shift, key).
/// `None`: not a key.
pub fn parse_key(text: &str) -> Option<(bool, bool, bool, String)> {
    let (mut cmd, mut alt, mut shift) = (false, false, false);
    let mut rest = text.trim();
    loop {
        let Some((head, tail)) = rest.split_once('-').filter(|(_, tail)| !tail.is_empty()) else { break };
        match head {
            "ctrl" | "cmd" | "secondary" | "super" => cmd = true,
            "alt" => alt = true,
            "shift" => shift = true,
            _ => return None,
        }
        rest = tail;
    }
    (!rest.is_empty() && !rest.contains(' ')).then(|| (cmd, alt, shift, rest.to_lowercase()))
}

/// Gives the registry's commands their keys from `[commands]` (the first key the file names; the
/// defaults are the commands' own) and binds them ([`Registry::bind_keys`]). Returns what
/// could not be bound.
pub fn bind_commands(registry: &Registry, cx: &mut App) -> Vec<String> {
    let commands = registry.all();
    let defaults: Vec<(String, Vec<String>)> = commands.iter().map(|c| (c.id.to_string(), c.key.iter().map(|k| k.to_string()).collect())).collect();
    let borrowed: Vec<(&str, Vec<&str>)> = defaults.iter().map(|(id, keys)| (id.as_str(), keys.iter().map(String::as_str).collect())).collect();
    let slices: Vec<(&str, &[&str])> = borrowed.iter().map(|(id, keys)| (*id, keys.as_slice())).collect();
    let probe = |key: &str| gpui_kit::KeyBinding::load(key, Box::new(gpui_kit::NoAction), None, false, None, cx).is_ok();
    for ((id, keys), mut command) in resolve("commands", &slices, probe).into_iter().zip(commands) {
        let key = keys.first().map(|k| SharedString::from(k.clone()));
        if command.id.as_ref() == id && command.key != key {
            command.key = key;
            registry.add(command);
        }
    }
    registry.bind_keys(None, cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_bindings_override_defaults_by_command() {
        let (file, notices) = parse_keybindings("[plot]\nplay_pause = \"p\"\nzoom_in = [\"+\", \"=\"]\npan_back = \"\"\n[commands]\nfile.open = \"ctrl-shift-o\"\n\"view.dark\" = \"f9\"\nbad = 3\n");
        assert_eq!(notices.len(), 1, "{notices:?}");
        assert_eq!(file["plot"]["play_pause"], vec!["p"]);
        assert_eq!(file["plot"]["zoom_in"], vec!["+", "="]);
        assert!(file["plot"]["pan_back"].is_empty(), "an empty key removes the binding");
        assert_eq!(file["commands"]["file.open"], vec!["ctrl-shift-o"], "a dotted key is the command's id");
        assert_eq!(file["commands"]["view.dark"], vec!["f9"]);
    }

    #[test]
    fn the_default_file_reads_back_as_the_defaults() {
        let sections = vec![("commands".to_string(), vec![("file.open".to_string(), vec!["ctrl-o".to_string()]), ("a.b".to_string(), vec![]), ("a.b.c".to_string(), vec!["f2".to_string(), "f3".to_string()])])];
        let text = default_keybindings("test-app", &sections);
        let (file, notices) = parse_keybindings(&text);
        assert!(notices.is_empty(), "{notices:?}\n{text}");
        assert_eq!(file["commands"]["file.open"], vec!["ctrl-o"]);
        assert!(file["commands"]["a.b"].is_empty());
        assert_eq!(file["commands"]["a.b.c"], vec!["f2", "f3"]);
    }

    #[test]
    fn keys_parse_for_matching_by_hand() {
        assert_eq!(parse_key("alt-g"), Some((false, true, false, "g".into())));
        assert_eq!(parse_key("ctrl-shift-z"), Some((true, false, true, "z".into())));
        assert_eq!(parse_key("shift-space"), Some((false, false, true, "space".into())));
        assert_eq!(parse_key("-"), Some((false, false, false, "-".into())));
        assert_eq!(parse_key("ctrl--"), Some((true, false, false, "-".into())));
        assert_eq!(parse_key("hyper-g"), None);
        assert_eq!(parse_key(""), None);
    }
}
