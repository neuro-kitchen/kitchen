//! The type families: IBM Plex Sans for the interface, IBM Plex Mono for code, paths, ids and
//! numbers in columns (tabular digits). Both are under the SIL Open Font License, so every app
//! ships them: the files go in the app's assets at [`FILES`] (with
//! [`crate::icons::AppAssets::file`]), then [`load`] hands them to the text system before
//! [`super::apply`]. Until they load, or if the app ships none, the system's fonts are used and
//! nothing else changes.
//!
//! ```ignore
//! let assets = AppAssets::new()
//!     .file(fonts::FILES[0], include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"))
//!     /* … the other three … */;
//! gpui_kit::application().with_assets(assets).run(|cx| {
//!     kitchen_ui::theme::fonts::load(cx);
//!     kitchen_ui::theme::use_look(Look::new(Palette::LIME), cx);
//! });
//! ```

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui_kit::App;

/// The interface family.
pub const UI: &str = "IBM Plex Sans";
/// Code, paths, ids, units, numbers in columns.
pub const MONO: &str = "IBM Plex Mono";

/// Where an app's assets serve the font files: the two weights the design uses (400, 600) of each
/// family.
pub const FILES: [&str; 4] = [
    "fonts/IBMPlexSans-Regular.ttf",
    "fonts/IBMPlexSans-SemiBold.ttf",
    "fonts/IBMPlexMono-Regular.ttf",
    "fonts/IBMPlexMono-SemiBold.ttf",
];

static LOADED: AtomicBool = AtomicBool::new(false);

/// Loads the font files the app's assets serve at [`FILES`]. `true` when both families loaded
/// (at least their regular weight); then [`super::apply`] uses them. Files that are missing are
/// skipped: the system's fonts stand in.
pub fn load(cx: &mut App) -> bool {
    let source = cx.asset_source().clone();
    let mut fonts: Vec<Cow<'static, [u8]>> = Vec::new();
    let mut found = [false; 4];
    for (i, path) in FILES.iter().enumerate() {
        if let Ok(Some(bytes)) = source.load(path) {
            fonts.push(bytes);
            found[i] = true;
        }
    }
    let both = found[0] && found[2];
    if !fonts.is_empty() && cx.text_system().add_fonts(fonts).is_ok() && both {
        LOADED.store(true, Ordering::Relaxed);
    }
    loaded()
}

/// Whether [`load`] found both families.
pub fn loaded() -> bool {
    LOADED.load(Ordering::Relaxed)
}
