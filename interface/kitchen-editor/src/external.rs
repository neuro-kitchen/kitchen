//! Writing code wherever the user writes code: the open script in an external editor (VS Code,
//! Zed, PyCharm, Sublime…), and the workspace following the file while it is edited there.
//!
//! Which editor ([`resolve`]), first found wins:
//! 1. the app's setting (a command line with `{file}` and `{line}`: `code -g {file}:{line}`);
//! 2. `KITCHEN_EDITOR`, then `VISUAL`, then `EDITOR`, when they name an editor with a window
//!    (a terminal editor, `vim` or `nano`, has nowhere to draw: it is skipped unless set in the
//!    app's setting with a terminal around it, `kitty nvim {file}`);
//! 3. a known editor on the `PATH` (`code`, `cursor`, `zed`, `subl`, `pycharm`, `codium`);
//! 4. the system's opener (`open`, `xdg-open`, `start`): the editor the system gives `.py` files.
//!
//! The workspace then watches the file ([`Watched`]): changed outside and not edited here, it is
//! read again; edited in both places, the workspace asks which to keep.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

/// Editors known by name: the program, and how it is told the line (`{file}`, `{line}`).
const KNOWN: &[(&str, &str, &[&str])] = &[
    ("code", "VS Code", &["-g", "{file}:{line}"]),
    ("cursor", "Cursor", &["-g", "{file}:{line}"]),
    ("codium", "VSCodium", &["-g", "{file}:{line}"]),
    ("zed", "Zed", &["{file}:{line}"]),
    ("subl", "Sublime Text", &["{file}:{line}"]),
    ("pycharm", "PyCharm", &["--line", "{line}", "{file}"]),
    ("idea", "IntelliJ IDEA", &["--line", "{line}", "{file}"]),
    ("rustrover", "RustRover", &["--line", "{line}", "{file}"]),
    ("gedit", "gedit", &["+{line}", "{file}"]),
    ("kate", "Kate", &["--line", "{line}", "{file}"]),
];

/// Editors that draw in a terminal: they need one around them.
const TERMINAL: &[&str] = &["vi", "vim", "nvim", "nano", "emacs", "hx", "helix", "micro", "kak", "joe", "ed"];

/// An editor and how to start it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalEditor {
    /// As the tooltip names it (`VS Code`).
    pub name: String,
    pub program: String,
    /// Its arguments, with `{file}` and `{line}` filled in when it starts.
    pub args: Vec<String>,
}

impl ExternalEditor {
    /// From a command line (`code -g {file}:{line}`); without `{file}`, the file goes last.
    pub fn from_command(command: &str) -> Option<ExternalEditor> {
        let mut words = split_words(command).into_iter();
        let program = words.next()?;
        let mut args: Vec<String> = words.collect();
        if !args.iter().any(|a| a.contains("{file}")) {
            args.push("{file}".into());
        }
        Some(ExternalEditor { name: display_name(&program), program, args })
    }

    /// A known editor by its program's name, with its line argument.
    fn known(program: &str) -> Option<ExternalEditor> {
        let stem = program_stem(program);
        KNOWN.iter().find(|(p, ..)| *p == stem).map(|(_, name, args)| ExternalEditor {
            name: name.to_string(),
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        })
    }

    /// The system's opener for `file`'s type.
    fn system() -> ExternalEditor {
        let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
            ("open", &["{file}"])
        } else if cfg!(windows) {
            ("cmd", &["/C", "start", "", "{file}"])
        } else {
            ("xdg-open", &["{file}"])
        };
        ExternalEditor { name: "the system's editor".into(), program: program.into(), args: args.iter().map(|a| a.to_string()).collect() }
    }

    /// The arguments for `file` at `line` (1-based).
    pub fn args_for(&self, file: &Path, line: usize) -> Vec<String> {
        let file = file.display().to_string();
        self.args.iter().map(|a| a.replace("{file}", &file).replace("{line}", &line.max(1).to_string())).collect()
    }

    /// Starts the editor on `file` at `line` and lets it run on its own (it outlives the app).
    pub fn open(&self, file: &Path, line: usize) -> Result<(), String> {
        Command::new(&self.program)
            .args(self.args_for(file, line))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("{} ({}): {e}. Choose the editor in Settings, or set KITCHEN_EDITOR.", self.name, self.program))
    }
}

/// The editor to use: the app's setting, the environment, a known editor on the `PATH`, the
/// system's opener (module docs).
pub fn resolve(setting: Option<&str>) -> ExternalEditor {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    resolve_with(setting, &|name| env(name), &on_path)
}

/// [`resolve`] with the environment and the `PATH` given (tests).
pub fn resolve_with(setting: Option<&str>, env: &dyn Fn(&str) -> Option<String>, on_path: &dyn Fn(&str) -> bool) -> ExternalEditor {
    if let Some(e) = setting.and_then(ExternalEditor::from_command) {
        return e;
    }
    for var in ["KITCHEN_EDITOR", "VISUAL", "EDITOR"] {
        let Some(command) = env(var) else { continue };
        let Some(e) = ExternalEditor::from_command(&command) else { continue };
        if TERMINAL.contains(&program_stem(&e.program).as_str()) {
            continue;
        }
        // A known editor gets its line argument even when the variable names only the program
        let only_program = split_words(&command).len() == 1;
        return if only_program { ExternalEditor::known(&e.program).unwrap_or(e) } else { e };
    }
    KNOWN.iter().find(|(p, ..)| on_path(p)).and_then(|(p, ..)| ExternalEditor::known(p)).unwrap_or_else(ExternalEditor::system)
}

/// Whether `program` is on the `PATH`.
pub fn on_path(program: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else { return false };
    let exts: Vec<String> = if cfg!(windows) { vec![".exe".into(), ".cmd".into(), ".bat".into()] } else { vec![String::new()] };
    std::env::split_paths(&paths).any(|dir| exts.iter().any(|ext| dir.join(format!("{program}{ext}")).is_file()))
}

fn program_stem(program: &str) -> String {
    Path::new(program).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn display_name(program: &str) -> String {
    let stem = program_stem(program);
    KNOWN.iter().find(|(p, ..)| *p == stem).map(|(_, n, _)| n.to_string()).unwrap_or(stem)
}

/// Words of a command line: spaces split, double quotes keep a word whole.
fn split_words(command: &str) -> Vec<String> {
    let (mut words, mut word, mut quoted) = (Vec::new(), String::new(), false);
    for c in command.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// What happened to a watched file since the last look.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileChange {
    /// Nothing.
    Same,
    /// Written outside the workspace: its new text.
    Changed(String),
    /// Gone (deleted or moved away).
    Gone,
}

/// A file the workspace has open, watched for writes from outside (an external editor, git).
#[derive(Clone, Debug)]
pub struct Watched {
    path: PathBuf,
    modified: Option<SystemTime>,
    /// The text last read or written here: a write that leaves it the same is not a change.
    text: String,
}

impl Watched {
    /// Watches `path`, whose text is `text` now.
    pub fn new(path: &Path, text: &str) -> Self {
        Self { path: path.to_path_buf(), modified: modified(path), text: text.to_string() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The workspace wrote `text` itself: not a change from outside.
    pub fn wrote(&mut self, text: &str) {
        self.text = text.to_string();
        self.modified = modified(&self.path);
    }

    /// Looks at the file: written since (by someone else, to another text), gone, or the same.
    pub fn check(&mut self) -> FileChange {
        let now = modified(&self.path);
        if now.is_none() {
            return if self.path.exists() { FileChange::Same } else { FileChange::Gone };
        }
        if now == self.modified {
            return FileChange::Same;
        }
        self.modified = now;
        match std::fs::read_to_string(&self.path) {
            Ok(text) if text != self.text => {
                self.text = text.clone();
                FileChange::Changed(text)
            }
            _ => FileChange::Same,
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editors_resolve_in_order() {
        let none = |_: &str| None;
        let nothing = |_: &str| false;
        // The setting wins, its line placeholder kept
        let e = resolve_with(Some("zed {file}:{line}"), &none, &nothing);
        assert_eq!((e.name.as_str(), e.args_for(Path::new("/s/a.py"), 7)), ("Zed", vec!["/s/a.py:7".to_string()]));
        // VISUAL naming a known program gets its line argument
        let env = |v: &str| (v == "VISUAL").then(|| "code".to_string());
        assert_eq!(resolve_with(None, &env, &nothing).args_for(Path::new("a.py"), 3), vec!["-g", "a.py:3"]);
        // A terminal editor is skipped for a known one on the PATH
        let env = |v: &str| (v == "EDITOR").then(|| "nvim".to_string());
        let path = |p: &str| p == "subl";
        assert_eq!(resolve_with(None, &env, &path).name, "Sublime Text");
        // Nothing found: the system's opener
        assert_eq!(resolve_with(None, &none, &nothing).name, "the system's editor");
        // Quoted program paths stay whole; a command without {file} gets it last
        let e = ExternalEditor::from_command("\"/Applications/My Editor\" --wait").unwrap();
        assert_eq!((e.program.as_str(), e.args_for(Path::new("x.rs"), 1)), ("/Applications/My Editor", vec!["--wait".to_string(), "x.rs".into()]));
    }

    #[test]
    fn watched_files_report_writes_from_outside_only() {
        let dir = std::env::temp_dir().join(format!("kitchen-editor-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.py");
        std::fs::write(&file, "a = 1\n").unwrap();
        let mut w = Watched::new(&file, "a = 1\n");
        assert_eq!(w.check(), FileChange::Same);
        // Our own write is not a change; a later one from outside is
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&file, "a = 2\n").unwrap();
        w.wrote("a = 2\n");
        assert_eq!(w.check(), FileChange::Same);
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&file, "a = 3\n").unwrap();
        assert_eq!(w.check(), FileChange::Changed("a = 3\n".into()));
        std::fs::remove_file(&file).unwrap();
        assert_eq!(w.check(), FileChange::Gone);
    }
}
