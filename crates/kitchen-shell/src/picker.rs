//! The file picker's model: one folder at a time, what is in it, and what the app can open there.
//! No window: [`crate::picker_view::FilePickerView`] draws it, and the tests drive it directly.
//!
//! File → Open… (and only that item) brings the picker up. The app says what it opens
//! ([`FileKind`]: a name and the extensions; some "files" are folders, a Zarr store or an image
//! sequence, and open as one). In a folder the picker lists the subfolders first, then what the app
//! opens, and hides the rest unless "Show all files" is on (shown greyed, not openable). Hidden
//! entries (a leading dot) are left out.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui_kit::assets::IconName;
use kitchen_ui::icons::standard;

/// What an app opens: a name for people and the extensions that are one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileKind {
    /// Shown in the picker's list and its footer ("Recording", "Zarr store").
    pub name: String,
    /// Extensions without the dot, lowercase (`nwb`, `zarr`, `ome.tiff`).
    pub extensions: Vec<String>,
    /// Whether these are folders that open as one (a Zarr store, an image sequence).
    pub folder: bool,
    /// Its icon in the list.
    pub icon: IconName,
}

impl FileKind {
    /// Files named `*.<ext>` for each of `extensions`.
    pub fn new(name: impl Into<String>, extensions: &[&str]) -> Self {
        Self { name: name.into(), extensions: extensions.iter().map(|e| e.trim_start_matches('.').to_lowercase()).collect(), folder: false, icon: IconName::File }
    }

    /// Folders named `*.<ext>` that open as one (a Zarr store).
    pub fn folder(name: impl Into<String>, extensions: &[&str]) -> Self {
        Self { folder: true, icon: IconName::Folder, ..Self::new(name, extensions) }
    }

    /// The same kind with another icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// Whether `name` (a file's or a folder's, `is_dir` saying which) is one of these. Longer
    /// extensions count (`scan.ome.tiff` is `ome.tiff`), and case does not matter.
    pub fn matches(&self, name: &str, is_dir: bool) -> bool {
        if is_dir != self.folder {
            return false;
        }
        let lower = name.to_lowercase();
        self.extensions.iter().any(|e| lower.len() > e.len() + 1 && lower.ends_with(e.as_str()) && lower.as_bytes()[lower.len() - e.len() - 1] == b'.')
    }
}

/// What an entry of the folder is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// A folder to go into.
    Folder,
    /// Something the app opens: the index of its [`FileKind`].
    Opens(usize),
    /// Anything else (listed only with "Show all files", and not openable).
    Other,
}

/// One entry of the folder shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Its file name.
    pub name: String,
    /// Its full path.
    pub path: PathBuf,
    /// What it is.
    pub kind: EntryKind,
    /// Its size in bytes (files only).
    pub size: Option<u64>,
    /// When it last changed.
    pub modified: Option<SystemTime>,
}

/// A place in the picker's left column: home, the desktop, downloads, a recent file's folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// Its name.
    pub name: String,
    /// Where it is.
    pub path: PathBuf,
    /// Its icon.
    pub icon: IconName,
}

/// The places on this computer that exist: home, Desktop, Documents, Downloads, then the file
/// system's root.
pub fn places() -> Vec<Place> {
    let mut out = Vec::new();
    if let Some(home) = home_dir() {
        out.push(Place { name: "Home".into(), path: home.clone(), icon: standard::HOME });
        for (name, icon) in [("Desktop", standard::DESKTOP), ("Documents", IconName::FileText), ("Downloads", standard::DOWNLOADS)] {
            let path = home.join(name);
            if path.is_dir() {
                out.push(Place { name: name.into(), path, icon });
            }
        }
    }
    let root = if cfg!(windows) { PathBuf::from("C:\\") } else { PathBuf::from("/") };
    out.push(Place { name: "Computer".into(), path: root, icon: IconName::HardDrive });
    out
}

/// The user's home folder, if the environment names one.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).filter(|p| p.is_dir())
}

/// `1.2 MB`, `340 kB`, `12 B` (powers of 1000, as file managers show them).
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["kB", "MB", "GB", "TB", "PB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64 / 1000.0;
    let mut unit = 0;
    while v >= 1000.0 && unit < UNITS.len() - 1 {
        v /= 1000.0;
        unit += 1;
    }
    if v < 10.0 { format!("{v:.1} {}", UNITS[unit]) } else { format!("{v:.0} {}", UNITS[unit]) }
}

/// The picker's state: the folder shown, its entries, the selection.
#[derive(Clone, Debug)]
pub struct Picker {
    kinds: Vec<FileKind>,
    dir: PathBuf,
    entries: Vec<Entry>,
    error: Option<String>,
    show_all: bool,
    selected: Option<usize>,
    recent: Vec<PathBuf>,
}

impl Picker {
    /// A picker for `kinds` showing `start` (or, if it cannot be read, home, then the root).
    pub fn new(kinds: Vec<FileKind>, start: impl Into<PathBuf>) -> Self {
        let mut picker = Picker { kinds, dir: PathBuf::new(), entries: Vec::new(), error: None, show_all: false, selected: None, recent: Vec::new() };
        let start = start.into();
        let fallbacks = [Some(start.clone()), home_dir(), places().last().map(|p| p.path.clone())];
        let mut first_error = None;
        for dir in fallbacks.into_iter().flatten() {
            match picker.enter(&dir) {
                Ok(()) => break,
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        if picker.dir != start {
            picker.error = first_error;
        }
        picker
    }

    /// The same picker listing `recent` files under Places (the app's recent files, newest first).
    pub fn with_recent(mut self, recent: Vec<PathBuf>) -> Self {
        self.recent = recent;
        self
    }

    /// What the app opens.
    pub fn kinds(&self) -> &[FileKind] {
        &self.kinds
    }

    /// The folder shown.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// What is shown of the folder: folders first, then the rest, each by name (case ignored).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The app's recent files that still exist.
    pub fn recent(&self) -> Vec<PathBuf> {
        self.recent.iter().filter(|p| p.exists()).cloned().collect()
    }

    /// Why the last folder could not be shown (the picker stays where it was).
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether files the app does not open are listed (greyed).
    pub fn show_all(&self) -> bool {
        self.show_all
    }

    /// Lists, or hides, the files the app does not open.
    pub fn set_show_all(&mut self, on: bool) {
        if on != self.show_all {
            self.show_all = on;
            self.refresh();
        }
    }

    /// The selected entry's index.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The selected entry.
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.selected.and_then(|i| self.entries.get(i))
    }

    /// Selects entry `i` (none if out of range).
    pub fn select(&mut self, i: Option<usize>) {
        self.selected = i.filter(|i| *i < self.entries.len());
    }

    /// What the Open button would open now: the selected entry, if the app opens it.
    pub fn openable(&self) -> Option<PathBuf> {
        self.selected_entry().filter(|e| matches!(e.kind, EntryKind::Opens(_))).map(|e| e.path.clone())
    }

    /// The breadcrumb: each folder from the root down to the one shown, with its path.
    pub fn crumbs(&self) -> Vec<(String, PathBuf)> {
        let mut out: Vec<(String, PathBuf)> = self
            .dir
            .ancestors()
            .map(|p| {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
                (name, p.to_path_buf())
            })
            .collect();
        out.reverse();
        out
    }

    /// Shows the folder `dir`. When it cannot be read the picker stays where it was and says why.
    pub fn enter(&mut self, dir: &Path) -> Result<(), String> {
        match self.read(dir) {
            Ok(entries) => {
                self.dir = dir.to_path_buf();
                self.entries = entries;
                self.error = None;
                self.selected = None;
                Ok(())
            }
            Err(e) => {
                self.error = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Shows the folder above; whether there is one.
    pub fn up(&mut self) -> bool {
        let Some(parent) = self.dir.parent().map(Path::to_path_buf) else { return false };
        let from = self.dir.clone();
        if self.enter(&parent).is_err() {
            return false;
        }
        // Keep the folder just left selected, so going back down is one step
        self.selected = self.entries.iter().position(|e| e.path == from);
        true
    }

    /// Reads the folder again (after a change on disk), keeping the selection by path.
    pub fn refresh(&mut self) {
        let keep = self.selected_entry().map(|e| e.path.clone());
        let dir = self.dir.clone();
        if self.enter(&dir).is_ok() {
            self.selected = keep.and_then(|p| self.entries.iter().position(|e| e.path == p));
        }
    }

    /// What a double click (or Enter) on entry `i` does: a folder is entered (`None`); something
    /// the app opens is returned, to open; anything else does nothing.
    pub fn activate(&mut self, i: usize) -> Option<PathBuf> {
        let entry = self.entries.get(i)?.clone();
        match entry.kind {
            EntryKind::Folder => {
                let _ = self.enter(&entry.path);
                None
            }
            EntryKind::Opens(_) => Some(entry.path),
            EntryKind::Other => None,
        }
    }

    /// The kind of `name`, given whether it is a folder.
    pub fn kind_of(&self, name: &str, is_dir: bool) -> EntryKind {
        if let Some(k) = self.kinds.iter().position(|k| k.matches(name, is_dir)) {
            return EntryKind::Opens(k);
        }
        if is_dir { EntryKind::Folder } else { EntryKind::Other }
    }

    /// A line for the footer: what the app opens here ("Opens: Recording (.nwb), Zarr store (.zarr)").
    pub fn opens_line(&self) -> String {
        let kinds: Vec<String> = self.kinds.iter().map(|k| format!("{} ({})", k.name, k.extensions.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(", "))).collect();
        if kinds.is_empty() { "Opens any file".into() } else { format!("Opens: {}", kinds.join(", ")) }
    }

    fn read(&self, dir: &Path) -> Result<Vec<Entry>, String> {
        let read = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut out = Vec::new();
        for item in read.flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = item.path();
            // Follow links: a linked folder is a folder
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let kind = if self.kinds.is_empty() && !is_dir { EntryKind::Opens(usize::MAX) } else { self.kind_of(&name, is_dir) };
            if kind == EntryKind::Other && !self.show_all {
                continue;
            }
            let size = meta.as_ref().filter(|m| m.is_file()).map(|m| m.len());
            let modified = meta.as_ref().and_then(|m| m.modified().ok());
            out.push(Entry { name, path, kind, size, modified });
        }
        out.sort_by(|a, b| match (a.kind == EntryKind::Folder, b.kind == EntryKind::Folder) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.name.cmp(&b.name)),
        });
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kitchen-picker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn kinds() -> Vec<FileKind> {
        vec![FileKind::new("Recording", &["nwb", "bin"]), FileKind::folder("Zarr store", &["zarr"]), FileKind::new("Slide", &["ome.tiff"])]
    }

    /// A project folder: two subfolders, a Zarr store, files the app opens and one it does not.
    fn project(name: &str) -> PathBuf {
        let dir = scratch(name);
        std::fs::create_dir_all(dir.join("sessions/day1")).unwrap();
        std::fs::create_dir_all(dir.join("Notes")).unwrap();
        std::fs::create_dir_all(dir.join("probe.zarr")).unwrap();
        std::fs::write(dir.join("b.nwb"), vec![0u8; 1500]).unwrap();
        std::fs::write(dir.join("A.BIN"), b"12").unwrap();
        std::fs::write(dir.join("scan.ome.tiff"), b"x").unwrap();
        std::fs::write(dir.join("readme.txt"), b"hi").unwrap();
        std::fs::write(dir.join(".hidden.nwb"), b"x").unwrap();
        dir
    }

    fn names(p: &Picker) -> Vec<&str> {
        p.entries().iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn lists_folders_first_then_what_the_app_opens() {
        let dir = project("list");
        let p = Picker::new(kinds(), &dir);
        assert_eq!(p.dir(), dir);
        assert_eq!(names(&p), ["Notes", "sessions", "A.BIN", "b.nwb", "probe.zarr", "scan.ome.tiff"]);
        let kinds: Vec<EntryKind> = p.entries().iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EntryKind::Folder, EntryKind::Folder, EntryKind::Opens(0), EntryKind::Opens(0), EntryKind::Opens(1), EntryKind::Opens(2)]);
        assert_eq!(p.entries()[3].size, Some(1500));
        assert_eq!(p.entries()[0].size, None, "folders have no size");
        assert!(p.error().is_none());
    }

    #[test]
    fn show_all_lists_the_rest_greyed() {
        let dir = project("all");
        let mut p = Picker::new(kinds(), &dir);
        p.set_show_all(true);
        let txt = p.entries().iter().find(|e| e.name == "readme.txt").expect("listed");
        assert_eq!(txt.kind, EntryKind::Other);
        let i = p.entries().iter().position(|e| e.name == "readme.txt").unwrap();
        p.select(Some(i));
        assert_eq!(p.openable(), None, "not openable");
        assert_eq!(p.activate(i), None);
        assert!(!names(&p).contains(&".hidden.nwb"), "hidden entries stay hidden");
    }

    #[test]
    fn enters_goes_up_and_keeps_the_way_back() {
        let dir = project("walk");
        let mut p = Picker::new(kinds(), &dir);
        let sessions = p.entries().iter().position(|e| e.name == "sessions").unwrap();
        assert_eq!(p.activate(sessions), None, "a folder is entered, not opened");
        assert_eq!(p.dir(), dir.join("sessions"));
        assert_eq!(names(&p), ["day1"]);
        assert!(p.up());
        assert_eq!(p.dir(), dir);
        assert_eq!(p.selected_entry().map(|e| e.name.as_str()), Some("sessions"), "the folder just left is selected");
        let crumbs = p.crumbs();
        assert_eq!(crumbs.last().unwrap().1, dir);
        assert!(crumbs.first().unwrap().1.parent().is_none(), "from the root");
    }

    #[test]
    fn a_zarr_store_opens_as_one() {
        let dir = project("zarr");
        let mut p = Picker::new(kinds(), &dir);
        let i = p.entries().iter().position(|e| e.name == "probe.zarr").unwrap();
        p.select(Some(i));
        assert_eq!(p.openable(), Some(dir.join("probe.zarr")));
        assert_eq!(p.activate(i), Some(dir.join("probe.zarr")));
        assert_eq!(p.dir(), dir, "not entered");
    }

    #[test]
    fn an_empty_folder_and_one_that_cannot_be_read() {
        let dir = scratch("empty");
        let mut p = Picker::new(kinds(), &dir);
        assert!(p.entries().is_empty());
        assert!(p.error().is_none());
        let missing = dir.join("gone");
        assert!(p.enter(&missing).is_err());
        assert_eq!(p.dir(), dir, "stays where it was");
        assert!(p.error().unwrap().contains("gone"), "says which folder");
        // Starting in a folder that cannot be read falls back, and says why
        let q = Picker::new(kinds(), &missing);
        assert_ne!(q.dir(), missing);
        assert!(q.error().is_some());
    }

    #[test]
    fn kinds_match_by_extension_and_shape() {
        let k = kinds();
        assert!(k[0].matches("rec.NWB", false));
        assert!(!k[0].matches("rec.nwb", true), "a folder named .nwb is not a recording");
        assert!(!k[0].matches(".nwb", false), "a name needs a stem");
        assert!(k[1].matches("probe.zarr", true));
        assert!(!k[1].matches("probe.zarr", false));
        assert!(k[2].matches("scan.ome.tiff", false));
        assert!(!k[2].matches("scan.tiff", false));
        assert!(!k[2].matches("scanome.tiff", false));
        let p = Picker::new(k, std::env::temp_dir());
        assert_eq!(p.opens_line(), "Opens: Recording (.nwb, .bin), Zarr store (.zarr), Slide (.ome.tiff)");
    }

    #[test]
    fn with_no_kinds_any_file_opens() {
        let dir = project("any");
        let p = Picker::new(Vec::new(), &dir);
        assert!(names(&p).contains(&"readme.txt"));
        assert!(p.entries().iter().filter(|e| e.kind != EntryKind::Folder).all(|e| matches!(e.kind, EntryKind::Opens(_))));
    }

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(human_size(12), "12 B");
        assert_eq!(human_size(1500), "1.5 kB");
        assert_eq!(human_size(340_000), "340 kB");
        assert_eq!(human_size(1_200_000), "1.2 MB");
        assert_eq!(human_size(25_000_000_000), "25 GB");
    }
}
