//! The colours: shared neutrals (plain greys: surfaces from the deepest, text, borders), status
//! colours and the accent ([`super::Look::active_palette`]), resolved for the current look.
//!
//! Every function here reads the look set with [`super::set_look`], so code can ask for a colour
//! without a context (`color::text_muted()`). [`Colors::resolve`] gives the same values for any
//! look, for tests and for drawing a theme that is not the current one.
//!
//! Surfaces step clearly apart; borders are darker than the surfaces they divide in the dark
//! theme (thin lines, as in Affinity); text passes WCAG AA (≥ 4.5:1) on every surface, marks
//! ≥ 3:1 (designparser `wcag-contrast`). The tests check each pair.

use gpui_kit::{Hsla, Rgba, rgb};

use super::{Look, look};

/// The shared neutrals as `(dark, light)` RGB pairs: plain greys (no tint), so any accent sits on
/// them.
pub mod neutrals {
    /// Title bar, tool columns, tab strips, and the window background panes sit on.
    pub const CHROME: (u32, u32) = (0x222222, 0xe4e4e4);
    /// Panels and panes.
    pub const PANEL: (u32, u32) = (0x2a2a2a, 0xf2f2f2);
    /// Raised blocks inside panels, popovers, panel group headers.
    pub const RAISED: (u32, u32) = (0x313131, 0xfafafa);
    /// Controls: buttons, inputs, slider tracks.
    pub const CONTROL: (u32, u32) = (0x3a3a3a, 0xe0e0e0);
    /// A control or row under the pointer.
    pub const HOVER: (u32, u32) = (0x454545, 0xd4d4d4);
    /// Separators and outlines.
    pub const BORDER: (u32, u32) = (0x171717, 0xc6c6c6);
    /// Body text.
    pub const TEXT: (u32, u32) = (0xe6e6e6, 0x1c1c1c);
    /// Secondary text: labels, captions, hints.
    pub const TEXT_MUTED: (u32, u32) = (0xb8b8b8, 0x555555);
    /// Text of disabled controls only (no contrast requirement).
    pub const TEXT_DISABLED: (u32, u32) = (0x6e6e6e, 0x9a9a9a);
    /// Errors (text and fills).
    pub const DANGER: (u32, u32) = (0xef6b6b, 0xb3261e);
    /// Success (text and fills).
    pub const SUCCESS: (u32, u32) = (0x4caf7a, 0x1e7a46);
    /// Warnings (text and fills).
    pub const WARNING: (u32, u32) = (0xe0a030, 0x8a5a00);
}

/// Every colour of one look, resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    /// Title bar, tool columns, tab strips; the window background between panes.
    pub chrome: Hsla,
    /// Panels and panes.
    pub panel: Hsla,
    /// Raised blocks inside panels, popovers.
    pub raised: Hsla,
    /// Controls: buttons, inputs, slider tracks.
    pub control: Hsla,
    /// A control or row under the pointer.
    pub hover: Hsla,
    /// Separators and outlines.
    pub border: Hsla,
    /// Body text.
    pub text: Hsla,
    /// Secondary text.
    pub text_muted: Hsla,
    /// Disabled text only.
    pub text_disabled: Hsla,
    /// The accent fill: primary buttons, selection backgrounds (at a low opacity).
    pub accent: Hsla,
    /// The accent fill under the pointer.
    pub accent_hover: Hsla,
    /// Text and icons on the accent fill.
    pub on_accent: Hsla,
    /// Thin accent marks and accent text: slider fills, focus rings, links, selected borders.
    pub accent_mark: Hsla,
    /// Errors.
    pub danger: Hsla,
    /// Success.
    pub success: Hsla,
    /// Warnings.
    pub warning: Hsla,
}

impl Colors {
    /// The colours of `look`.
    pub fn resolve(look: &Look) -> Colors {
        let dark = look.dark;
        let n = |pair: (u32, u32)| rgb_hsla(if dark { pair.0 } else { pair.1 });
        let a = look.active_palette().accent(dark);
        Colors {
            chrome: n(neutrals::CHROME),
            panel: n(neutrals::PANEL),
            raised: n(neutrals::RAISED),
            control: n(neutrals::CONTROL),
            hover: n(neutrals::HOVER),
            border: n(neutrals::BORDER),
            text: n(neutrals::TEXT),
            text_muted: n(neutrals::TEXT_MUTED),
            text_disabled: n(neutrals::TEXT_DISABLED),
            accent: rgb_hsla(a.fill),
            accent_hover: rgb_hsla(a.fill_hover),
            on_accent: rgb_hsla(a.on_fill),
            accent_mark: rgb_hsla(a.mark),
            danger: n(neutrals::DANGER),
            success: n(neutrals::SUCCESS),
            warning: n(neutrals::WARNING),
        }
    }
}

/// The colours of the current look.
pub fn colors() -> Colors {
    Colors::resolve(&look())
}

/// Title bar, tool columns, tab strips; the window background between panes.
pub fn chrome() -> Hsla {
    colors().chrome
}
/// Panels and panes.
pub fn panel() -> Hsla {
    colors().panel
}
/// Raised blocks inside panels, popovers, panel group headers.
pub fn raised() -> Hsla {
    colors().raised
}
/// Controls: buttons, inputs, slider tracks.
pub fn control() -> Hsla {
    colors().control
}
/// A control or row under the pointer.
pub fn hover() -> Hsla {
    colors().hover
}
/// Separators and outlines.
pub fn border() -> Hsla {
    colors().border
}
/// Body text.
pub fn text() -> Hsla {
    colors().text
}
/// Secondary text: ≥ 4.5:1 on every surface in both themes.
pub fn text_muted() -> Hsla {
    colors().text_muted
}
/// Disabled text only.
pub fn text_disabled() -> Hsla {
    colors().text_disabled
}
/// The accent fill: selection, the active tool, primary actions ([`on_accent`] text on it).
pub fn accent() -> Hsla {
    colors().accent
}
/// The accent fill under the pointer.
pub fn accent_hover() -> Hsla {
    colors().accent_hover
}
/// Text and icons on the accent fill.
pub fn on_accent() -> Hsla {
    colors().on_accent
}
/// Thin accent marks and accent text (≥ 3:1 on panels and controls).
pub fn accent_mark() -> Hsla {
    colors().accent_mark
}
/// Errors.
pub fn danger() -> Hsla {
    colors().danger
}
/// Success.
pub fn success() -> Hsla {
    colors().success
}
/// Warnings.
pub fn warning() -> Hsla {
    colors().warning
}

/// A colour from `0xRRGGBB`.
pub fn rgb_hsla(c: u32) -> Hsla {
    rgb(c).into()
}

/// A colour from 0–255 RGB.
pub fn rgb8(c: [u8; 3]) -> Hsla {
    rgb_hsla(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32)
}

/// Relative luminance (WCAG), alpha ignored.
pub fn luminance(c: Hsla) -> f32 {
    let Rgba { r, g, b, .. } = c.to_rgb();
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio of two colours (1 to 21).
pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Palette;

    use crate::theme::Rgb;

    /// Every app palette and every suggested accent, plus a few arbitrary picks, in both themes.
    fn looks() -> Vec<Look> {
        let mut accents: Vec<Option<Rgb>> = vec![None];
        accents.extend(Rgb::SUGGESTED.map(Some));
        accents.extend([0xfffacd, 0x0a1030, 0xff0000, 0xffffff, 0x000000].map(|c| Some(Rgb(c))));
        let mut out = Vec::new();
        for p in Palette::APPS {
            for a in &accents {
                for dark in [true, false] {
                    out.push(Look::new(p).with_accent(*a).with_dark(dark));
                }
            }
        }
        out
    }

    #[test]
    fn neutrals_are_plain_greys() {
        use neutrals::*;
        for (name, pair) in [("chrome", CHROME), ("panel", PANEL), ("raised", RAISED), ("control", CONTROL), ("hover", HOVER), ("border", BORDER), ("text", TEXT), ("text_muted", TEXT_MUTED), ("text_disabled", TEXT_DISABLED)] {
            for c in [pair.0, pair.1] {
                let (r, g, b) = (c >> 16 & 0xff, c >> 8 & 0xff, c & 0xff);
                assert!(r == g && g == b, "{name}: #{c:06x} is tinted");
            }
        }
    }

    #[test]
    fn text_passes_aa_on_every_surface() {
        for l in looks() {
            let c = Colors::resolve(&l);
            for (sname, s) in [("chrome", c.chrome), ("panel", c.panel), ("raised", c.raised), ("control", c.control), ("hover", c.hover)] {
                for (tname, t) in [("text", c.text), ("text_muted", c.text_muted)] {
                    let r = contrast(t, s);
                    assert!(r >= 4.5, "{} dark={}: {tname} on {sname} is {r:.2}:1", l.palette.name, l.dark);
                }
            }
            for (name, t) in [("danger", c.danger), ("success", c.success), ("warning", c.warning)] {
                let r = contrast(t, c.panel);
                assert!(r >= 4.5, "dark={}: {name} on panel is {r:.2}:1", l.dark);
            }
        }
    }

    #[test]
    fn accents_pass_contrast() {
        for l in looks() {
            let c = Colors::resolve(&l);
            let name = format!("{} {:?} dark={}", l.palette.name, l.accent, l.dark);
            for (fname, f) in [("fill", c.accent), ("fill_hover", c.accent_hover)] {
                let r = contrast(c.on_accent, f);
                assert!(r >= 4.5, "{name}: text on {fname} is {r:.2}:1");
            }
            // Accent text (links) on panels; marks on controls
            for (sname, s, min) in [("panel", c.panel, 4.5), ("raised", c.raised, 4.5), ("control", c.control, 3.0), ("hover", c.hover, 3.0)] {
                let r = contrast(c.accent_mark, s);
                assert!(r >= min, "{name}: mark on {sname} is {r:.2}:1");
            }
        }
    }

    #[test]
    fn surfaces_step_apart() {
        for dark in [true, false] {
            let c = Colors::resolve(&Look { dark, ..Look::new(Palette::LIME) });
            let order = [c.chrome, c.panel, c.raised];
            let l: Vec<f32> = order.iter().map(|c| luminance(*c)).collect();
            assert!(l[0] < l[1] && l[1] < l[2], "dark={dark}: chrome < panel < raised in lightness");
        }
    }

    #[test]
    fn contrast_of_known_pairs() {
        assert!((contrast(rgb_hsla(0x000000), rgb_hsla(0xffffff)) - 21.0).abs() < 0.01);
        // The usual reference pairs: #767676 on white is 4.54:1 (the lightest grey passing AA),
        // #777777 4.48:1 (fails). designparser `wcag-contrast` quotes 4.48 for #767676.
        let r = contrast(rgb_hsla(0x767676), rgb_hsla(0xffffff));
        assert!((r - 4.54).abs() < 0.01, "{r}");
        let r = contrast(rgb_hsla(0x777777), rgb_hsla(0xffffff));
        assert!((r - 4.48).abs() < 0.01, "{r}");
        assert_eq!(rgb8([0x12, 0x34, 0x56]), rgb_hsla(0x123456));
    }
}
