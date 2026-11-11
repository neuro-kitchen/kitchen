//! Design tokens shared by the neuro-kitchen apps, and their application to gpui-kit's theme, so
//! every gpui-kit component (buttons, menus, inputs, tooltips) and the apps' own layout use one
//! set of values.
//!
//! - Spacing on the 8 px grid with a 4 px half-step: [`XS`] 4, [`SM`] 8, [`MD`] 12, [`LG`] 16,
//!   [`XL`] 24, [`XXL`] 32 (designparser `8pt-grid`, `4pt-system`).
//! - Type: base [`FONT_SIZE`] 15 px, so gpui's `text_xs` ≈ 11 (captions), `text_sm` ≈ 13 (body),
//!   `text_base` 15 (titles), `text_lg` ≈ 17 (headings); weights 400 and 600 only. The sizes are
//!   relative to the base, so a larger base in Settings scales every one of them.
//! - Radii by tier: [`RADIUS_SM`] 4 (chips, swatches), [`RADIUS`] 6 (controls), [`RADIUS_LG`] 8
//!   (sections, panes, dialogs) (designparser `corner-radius-tone`: neutral, one per tier).
//! - Panes are rounded cards [`PANE_GAP`] apart on the chrome, with [`PANE_PADDING`] inside
//!   (padding ≥ 1.5 × gap: designparser `padding-gap-ratio`).
//! - Neutrals are shared ([`color`]); the accent comes from the app ([`Palette`]).
//!
//! The look (dark or light, palette, text size) is process-wide: set it with [`set_look`], then
//! [`apply`] hands it to gpui-kit and redraws every window.

pub mod color;
pub mod palette;

use std::sync::RwLock;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Pixels, Window, px, rgba};

pub use color::{Colors, colors, contrast, rgb8, rgb_hsla};
pub use palette::{Accent, Palette};

/// Spacing: a hairline step, icon gaps (px).
pub const XS: f32 = 4.;
/// Spacing: the base unit; gaps between controls and rows (px).
pub const SM: f32 = 8.;
/// Spacing: padding inside sections and boxes (px).
pub const MD: f32 = 12.;
/// Spacing: padding of panels (px).
pub const LG: f32 = 16.;
/// Spacing: between groups (px).
pub const XL: f32 = 24.;
/// Spacing: around page-level blocks (px).
pub const XXL: f32 = 32.;

/// Base text size (px) by default.
pub const FONT_SIZE: f32 = 15.;
/// Smallest and largest base text size Settings may choose (px).
pub const FONT_RANGE: (f32, f32) = (12., 18.);

/// Corner radius of small things: chips, swatches, scrub labels (px).
pub const RADIUS_SM: f32 = 4.;
/// Corner radius of controls: buttons, inputs, menus (px).
pub const RADIUS: f32 = 6.;
/// Corner radius of large things: boxed sections, panes, dialogs (px).
pub const RADIUS_LG: f32 = 8.;

/// Gap between panes on the chrome (px).
pub const PANE_GAP: f32 = 4.;
/// Padding inside a pane (px).
pub const PANE_PADDING: f32 = 8.;

/// Width of a row's label ([`crate::widgets::row`]) (px).
pub const LABEL_W: f32 = 112.;
/// Narrowest a row's control gets before it drops under its label (px).
pub const CONTROL_MIN_W: f32 = 140.;
/// Smallest pointer target (px): WCAG 2.2 AA (designparser `touch-target`).
pub const TARGET_MIN: f32 = 24.;

/// What the user chose: dark or light, the app's palette, the base text size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The dark theme (`false`: light).
    pub dark: bool,
    /// The app's accent colours.
    pub palette: Palette,
    /// Base text size (px), within [`FONT_RANGE`].
    pub font_size: f32,
}

impl Look {
    /// Dark, `palette`, the default text size.
    pub const fn new(palette: Palette) -> Self {
        Look { dark: true, palette, font_size: FONT_SIZE }
    }

    /// The same look, dark or light.
    pub const fn with_dark(self, dark: bool) -> Self {
        Look { dark, ..self }
    }

    /// The same look with another palette.
    pub const fn with_palette(self, palette: Palette) -> Self {
        Look { palette, ..self }
    }

    /// The same look with another base text size (kept within [`FONT_RANGE`]).
    pub fn with_font_size(self, font_size: f32) -> Self {
        Look { font_size: font_size.round().clamp(FONT_RANGE.0, FONT_RANGE.1), ..self }
    }
}

impl Default for Look {
    fn default() -> Self {
        Look::new(Palette::PLACEHOLDER)
    }
}

static LOOK: RwLock<Look> = RwLock::new(Look::new(Palette::PLACEHOLDER));

/// Sets the look every colour reads. [`apply`] then hands it to gpui-kit.
pub fn set_look(look: Look) {
    let look = look.with_font_size(look.font_size);
    *LOOK.write().unwrap_or_else(|e| e.into_inner()) = look;
}

/// The current look.
pub fn look() -> Look {
    *LOOK.read().unwrap_or_else(|e| e.into_inner())
}

/// Whether the current look is dark.
pub fn is_dark() -> bool {
    look().dark
}

/// Whether the system shows `window` dark (for a theme that follows the system).
pub fn system_dark(window: &Window) -> bool {
    use gpui_kit::WindowAppearance as W;
    matches!(window.appearance(), W::Dark | W::VibrantDark)
}

/// Sets the look and applies it ([`set_look`], then [`apply`]).
pub fn use_look(look: Look, cx: &mut App) {
    set_look(look);
    apply(cx);
}

/// Applies the current look ([`set_look`]) and the tokens to gpui-kit's theme, and redraws every
/// window.
pub fn apply(cx: &mut App) {
    let l = look();
    Theme::change(if l.dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    let c = Colors::resolve(&l);
    let (on_danger, on_success, on_warning) = (palette::best_text_on(c.danger), palette::best_text_on(c.success), palette::best_text_on(c.warning));
    Theme::update(cx, |t| {
        t.font_size = px(l.font_size);
        t.radius = px(RADIUS);
        t.radius_lg = px(RADIUS_LG);
        t.shadow = true;
        let k = &mut t.colors;
        k.background = c.panel;
        k.foreground = c.text;
        k.muted = c.raised;
        k.muted_foreground = c.text_muted;
        k.border = c.border;
        k.input = c.hover;
        k.ring = c.accent_mark;
        k.caret = c.text;
        k.selection = c.accent.opacity(0.45);
        k.popover = c.raised;
        k.popover_foreground = c.text;
        k.sidebar = c.chrome;
        k.sidebar_foreground = c.text;
        k.sidebar_border = c.border;
        k.sidebar_accent = c.control;
        k.sidebar_accent_foreground = c.text;
        k.sidebar_primary = c.accent;
        k.sidebar_primary_foreground = c.on_accent;
        k.primary = c.accent;
        k.primary_hover = c.accent_hover;
        k.primary_active = c.accent;
        k.primary_foreground = c.on_accent;
        k.secondary = c.control;
        k.secondary_hover = c.hover;
        k.secondary_active = c.hover;
        k.secondary_foreground = c.text;
        k.accent = c.control;
        k.accent_foreground = c.text;
        k.title_bar = c.chrome;
        k.title_bar_border = c.border;
        k.tab_bar = c.chrome;
        k.tab = c.chrome;
        k.tab_active = c.panel;
        k.tab_foreground = c.text_muted;
        k.tab_active_foreground = c.text;
        k.list = c.panel;
        k.list_even = c.panel;
        k.list_hover = c.control;
        k.list_active = c.accent.opacity(0.25);
        k.list_active_border = c.accent_mark;
        k.list_head = c.raised;
        k.table = c.panel;
        k.table_even = c.raised;
        k.table_head = c.raised;
        k.table_head_foreground = c.text_muted;
        k.table_active = c.accent.opacity(0.25);
        k.table_active_border = c.accent_mark;
        k.danger = c.danger;
        k.danger_foreground = on_danger;
        k.success = c.success;
        k.success_foreground = on_success;
        k.warning = c.warning;
        k.warning_foreground = on_warning;
        k.info = c.accent_mark;
        k.link = c.accent_mark;
        k.scrollbar = rgba(0x00000000).into();
        k.scrollbar_thumb = if l.dark { rgba(0xffffff33).into() } else { rgba(0x00000033).into() };
        k.scrollbar_thumb_hover = if l.dark { rgba(0xffffff55).into() } else { rgba(0x00000055).into() };
        k.window_border = c.border;
        k.overlay = rgba(0x00000080).into();
        k.drag_border = c.accent_mark;
        k.drop_target = c.accent.opacity(0.2);
        k.chart_grid = c.border;
    });
}

/// `px` of a token.
pub fn sp(v: f32) -> Pixels {
    px(v)
}
