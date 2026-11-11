//! An app's accent colours. The neutrals are shared ([`super::color`]); each app supplies one
//! [`Palette`]: dsp-kitchen [`Palette::LIME`], lumen [`Palette::PLACEHOLDER`] until its colour is
//! chosen. A palette made from one colour ([`Palette::from_rgb`]) is checked for contrast as it is
//! built, so any colour can be tried.

use super::color::{contrast, neutrals, rgb_hsla};
use gpui_kit::Hsla;

/// The accent colours of one theme (light or dark), as RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    /// Fills: primary buttons, the selected item's background (at a low opacity), switches on.
    pub fill: u32,
    /// The fill under the pointer.
    pub fill_hover: u32,
    /// Text and icons on the fill (≥ 4.5:1 on it).
    pub on_fill: u32,
    /// Thin marks and text in the accent: a slider's filled part, focus rings, links, the
    /// selected item's border (≥ 3:1 on panels and controls, WCAG non-text contrast).
    pub mark: u32,
}

/// The accent colours an app supplies, one set per theme. Neutrals, spacing and type are shared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Shown in Settings and in the gallery.
    pub name: &'static str,
    /// Accent of the dark theme.
    pub dark: Accent,
    /// Accent of the light theme.
    pub light: Accent,
}

impl Palette {
    /// dsp-kitchen's lime (the neuro-kitchen site's palette): lime fills with near-black text on
    /// them in both themes; in the light theme marks and links are a deep olive, since lime on a
    /// light panel is barely visible.
    pub const LIME: Palette = Palette {
        name: "Lime",
        dark: Accent { fill: 0xc6f26b, fill_hover: 0xe2faa8, on_fill: 0x0a0e17, mark: 0xc6f26b },
        light: Accent { fill: 0xc6f26b, fill_hover: 0xb4e350, on_fill: 0x0a0e17, mark: 0x3f6212 },
    };

    /// A stand-in for lumen until its accent is chosen: lumen's current blue (`0x3d65a8`), with
    /// a lighter blue for marks in the dark theme. Swap it with [`Palette::from_rgb`] or a
    /// hand-tuned palette like [`Palette::LIME`].
    pub const PLACEHOLDER: Palette = Palette {
        name: "Blue (placeholder)",
        dark: Accent { fill: 0x3d65a8, fill_hover: 0x4672b8, on_fill: 0xffffff, mark: 0x7fa6e6 },
        light: Accent { fill: 0x3d65a8, fill_hover: 0x335690, on_fill: 0xffffff, mark: 0x3d65a8 },
    };

    /// The accent of the dark (`true`) or light theme.
    pub fn accent(&self, dark: bool) -> Accent {
        if dark { self.dark } else { self.light }
    }

    /// A palette made from one colour (lumen's Settings offers several): the fill is the colour
    /// (lighter or darker when neither white nor near-black text reads on it at 4.5:1); the text on
    /// it is white or near-black, whichever reads better; its hover steps away from that text; the
    /// marks keep the hue and move in lightness
    /// until they reach 3:1 on every panel and control (in the dark theme also 25 % less
    /// colourful, so they do not glare: designparser `dark-mode-color-adaptation`).
    pub fn from_rgb(name: &'static str, rgb: u32) -> Palette {
        // The fill moves in lightness (away from its text) until its text reads at 4.5:1
        let mut fill: Hsla = rgb_hsla(rgb);
        for _ in 0..60 {
            let on = best_text_on(fill);
            if contrast(on, fill) >= 4.5 {
                break;
            }
            fill.l = (fill.l + if on.l > 0.5 { -0.02 } else { 0.02 }).clamp(0.0, 1.0);
        }
        let on_fill = best_text_on(fill);
        // Hover moves away from the text too, so it never reads worse than the fill
        let hover = Hsla { l: (fill.l + if on_fill.l > 0.5 { -0.06 } else { 0.06 }).clamp(0.0, 1.0), ..fill };
        let make = |dark: bool| {
            let mut mark = if dark { Hsla { s: fill.s * 0.75, ..fill } } else { fill };
            let backs = [neutrals::PANEL, neutrals::RAISED, neutrals::CONTROL, neutrals::HOVER].map(|n| rgb_hsla(if dark { n.0 } else { n.1 }));
            for _ in 0..60 {
                if backs.iter().all(|b| contrast(mark, *b) >= 3.0) {
                    break;
                }
                mark.l = (mark.l + if dark { 0.02 } else { -0.02 }).clamp(0.0, 1.0);
            }
            Accent { fill: hsla_rgb(fill), fill_hover: hsla_rgb(hover), on_fill: hsla_rgb(on_fill), mark: hsla_rgb(mark) }
        };
        Palette { name, dark: make(true), light: make(false) }
    }

    /// The palettes offered by default: the two app palettes and lumen's earlier accent choices
    /// (made with [`Palette::from_rgb`]).
    pub fn presets() -> Vec<Palette> {
        let mut out = vec![Palette::LIME, Palette::PLACEHOLDER];
        for (name, rgb) in [("Teal", 0x2b7f86), ("Green", 0x3f8a52), ("Purple", 0x7451b8), ("Orange", 0xb8622a), ("Pink", 0xb24470)] {
            out.push(Palette::from_rgb(name, rgb));
        }
        out
    }
}

/// White or near-black (`0x0a0e17`), whichever has more contrast on `c`.
pub fn best_text_on(c: Hsla) -> Hsla {
    let (white, ink) = (rgb_hsla(0xffffff), rgb_hsla(0x0a0e17));
    if contrast(white, c) >= contrast(ink, c) { white } else { ink }
}

/// `c` as `0xRRGGBB` (alpha dropped).
pub fn hsla_rgb(c: Hsla) -> u32 {
    let r = c.to_rgb();
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(r.r) << 16) | (b(r.g) << 8) | b(r.b)
}
