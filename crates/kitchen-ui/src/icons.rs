//! Icons: what an app ships ([`AppAssets`]: gpui-kit's component icons, the few Lucide icons the
//! app adds, and its own files), and the check that every icon its code names is shipped
//! ([`assert_icons_bundled`]): an icon missing from the assets draws nothing and logs an error.
//!
//! An app lists the icons it adds with gpui-kit's macro, so the binary carries those alone rather
//! than the whole catalog:
//!
//! ```ignore
//! gpui_kit::assets::icon_assets!(pub ExtraIcons, [AudioWaveform, Workflow, Microscope]);
//!
//! let assets = kitchen_ui::icons::AppAssets::new().icons(ExtraIcons).file(LOGO, LOGO_PNG);
//! gpui_kit::application().with_assets(assets).run(|cx| { /* … */ });
//!
//! #[test]
//! fn every_icon_used_is_bundled() {
//!     let assets = kitchen_ui::icons::AppAssets::new().icons(ExtraIcons);
//!     kitchen_ui::icons::assert_icons_bundled(&assets, &[concat!(env!("CARGO_MANIFEST_DIR"), "/src")]);
//! }
//! ```

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gpui_kit::assets::IconName;
use gpui_kit::{AssetSource, SharedString};

/// The assets an app ships: its own files (a logo), then the icon sets it adds, then gpui-kit's
/// component icons (or the whole Lucide catalog, [`AppAssets::all_icons`]).
#[derive(Default)]
pub struct AppAssets {
    files: Vec<(&'static str, &'static [u8])>,
    extra: Vec<Box<dyn AssetSource>>,
    all_icons: bool,
}

impl AppAssets {
    /// gpui-kit's component icons alone.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a set of icons (one made with `gpui_kit::assets::icon_assets!`), or any other source.
    pub fn icons(mut self, source: impl AssetSource) -> Self {
        self.extra.push(Box::new(source));
        self
    }

    /// Adds a file served at `path` (for `img(path)`: a logo, a picture in the about dialog).
    pub fn file(mut self, path: &'static str, bytes: &'static [u8]) -> Self {
        self.files.push((path, bytes));
        self
    }

    /// Ships the whole Lucide catalog instead of the component icons (a larger binary; for an app
    /// that has not listed its icons yet).
    pub fn all_icons(mut self) -> Self {
        self.all_icons = true;
        self
    }

    fn base(&self) -> &dyn AssetSource {
        if self.all_icons { &gpui_kit::assets::AllAssets } else { &gpui_kit::assets::Assets }
    }
}

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = self.files.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        for source in &self.extra {
            if let Some(bytes) = source.load(path)? {
                return Ok(Some(bytes));
            }
        }
        self.base().load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut paths = self.base().list(path)?;
        for source in &self.extra {
            paths.extend(source.list(path)?);
        }
        paths.extend(self.files.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)));
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

/// The names of the icons the Rust sources under `roots` name: `IconName::Server`, and the same
/// through an alias (`use gpui_kit::assets::IconName as I;` then `I::Server`, or
/// `type Icons = IconName;`). Aliases are collected from every file first, so an alias declared
/// in one module and used in another is found too. Only real icon names are kept, so another
/// type behind the same alias adds nothing. A glob import (`use IconName::*`) is not seen.
pub fn icon_names_used(roots: &[impl AsRef<Path>]) -> BTreeSet<String> {
    let texts: Vec<String> = rust_files(roots).iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect();
    let mut markers: BTreeSet<String> = BTreeSet::from(["IconName".to_string()]);
    for text in &texts {
        markers.extend(aliases_in(text));
    }
    let known: BTreeSet<String> = IconName::ALL.iter().map(|i| format!("{i:?}")).collect();
    let mut used = BTreeSet::new();
    for text in &texts {
        used.extend(names_after(text, &markers).into_iter().filter(|n| known.contains(n)));
    }
    used
}

/// The icons named in `used` that `assets` cannot load.
pub fn missing_icons(assets: &dyn AssetSource, used: &BTreeSet<String>) -> Vec<String> {
    let mut missing = Vec::new();
    for name in used {
        let Some(icon) = IconName::ALL.iter().find(|i| format!("{i:?}") == *name) else { continue };
        if assets.load(&icon.path()).ok().flatten().is_none() {
            missing.push(name.clone());
        }
    }
    missing
}

/// Panics unless every icon the sources under `roots` name is in `assets` (and at least one icon
/// is found, so a wrong root fails instead of passing empty). For an app's test.
pub fn assert_icons_bundled(assets: &dyn AssetSource, roots: &[impl AsRef<Path>]) {
    let used = icon_names_used(roots);
    assert!(!used.is_empty(), "no icons found in the sources: are the roots right? {:?}", roots.iter().map(|r| r.as_ref().display().to_string()).collect::<Vec<_>>());
    let missing = missing_icons(assets, &used);
    assert!(missing.is_empty(), "icons used but not bundled (add them to the app's icon set): {missing:?}");
}

/// Every `.rs` file under `roots` (roots that do not exist are skipped).
fn rust_files(roots: &[impl AsRef<Path>]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = roots.iter().map(|r| r.as_ref().to_path_buf()).collect();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let Ok(entries) = std::fs::read_dir(&path) else { continue };
            stack.extend(entries.flatten().map(|e| e.path()));
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Names `IconName` goes by in `text`: `IconName as I` (in a `use`, grouped or not) and
/// `type I = …IconName;`.
fn aliases_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("IconName") {
        let rest = text[at + "IconName".len()..].trim_start();
        if let Some(after) = rest.strip_prefix("as") {
            if after.starts_with(char::is_whitespace) {
                let name = ident_at(after.trim_start());
                if !name.is_empty() && name != "_" {
                    out.push(name);
                }
            }
        }
    }
    for (at, _) in text.match_indices("type ") {
        if at > 0 && is_ident_char(text[..at].chars().next_back().unwrap_or(' ')) {
            continue;
        }
        let rest = &text[at + 5..];
        let name = ident_at(rest.trim_start());
        let Some(eq) = rest.find('=') else { continue };
        let Some(end) = rest[eq..].find(';') else { continue };
        let target = rest[eq + 1..eq + end].trim();
        if !name.is_empty() && (target == "IconName" || target.ends_with("::IconName")) {
            out.push(name);
        }
    }
    out
}

/// The capitalised names after `marker::` for each marker, where the marker is a whole word.
fn names_after(text: &str, markers: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for m in markers {
        let pat = format!("{m}::");
        for (at, _) in text.match_indices(&pat) {
            if text[..at].chars().next_back().is_some_and(is_ident_char) {
                continue;
            }
            let name = ident_at(&text[at + pat.len()..]);
            if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                out.push(name);
            }
        }
    }
    out
}

fn ident_at(s: &str) -> String {
    s.chars().take_while(|c| is_ident_char(*c)).collect()
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markers(text: &str) -> BTreeSet<String> {
        let mut m = BTreeSet::from(["IconName".to_string()]);
        m.extend(aliases_in(text));
        m
    }

    #[test]
    fn finds_names_and_aliases() {
        let text = r#"
            use gpui_kit::assets::IconName as I;
            use gpui_kit::{assets::{IconName as Lucide, Assets}, px};
            type Icons = gpui_kit::assets::IconName;
            fn f() { (IconName::Plus, I::Server, Lucide::Columns2, Icons::Flame, UI::Nope, I::new()) }
        "#;
        let m = markers(text);
        assert_eq!(m, BTreeSet::from(["IconName", "I", "Lucide", "Icons"].map(String::from)));
        let mut names = names_after(text, &m);
        names.sort();
        // `UI::Nope` is not `I::Nope`; `I::new` is not a name
        assert_eq!(names, ["Columns2", "Flame", "Plus", "Server"]);
    }

    #[test]
    fn an_alias_named_like_a_type_is_filtered_by_real_names() {
        // dsp-kitchen aliases the enum as `Assets` while `Assets` is also gpui-kit's asset source
        let text = "use gpui_kit::assets::IconName as Assets; let _ = (Assets::AppWindow, Assets::load);";
        let names = names_after(text, &markers(text));
        assert_eq!(names, ["AppWindow"]);
    }

    /// kitchen-ui's widgets name only icons gpui-kit's component set ships: the check run here as
    /// an app runs it (over the widget sources; this file's test texts name icons on purpose).
    #[test]
    fn every_icon_used_is_bundled() {
        let root = env!("CARGO_MANIFEST_DIR");
        assert_icons_bundled(&AppAssets::new(), &[format!("{root}/src/widgets"), format!("{root}/src/param_editor.rs")]);
    }

    #[test]
    fn a_missing_icon_is_reported() {
        let used = BTreeSet::from(["Microscope".to_string(), "Plus".to_string()]);
        assert_eq!(missing_icons(&AppAssets::new(), &used), ["Microscope"]);
        gpui_kit::assets::icon_assets!(Extra, [Microscope]);
        assert!(missing_icons(&AppAssets::new().icons(Extra), &used).is_empty());
    }
}
