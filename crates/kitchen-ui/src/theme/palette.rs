//! Accent colours. The neutrals are shared ([`super::color`]); each app supplies a default
//! [`Palette`] (dsp-kitchen [`Palette::LIME`], lucem [`Palette::TEAL`], garnish
//! [`Palette::PAPRIKA`], kitchen-orchestrate [`Palette::AMBER`], kitchen [`Palette::COBALT`]),
//! and the user may pick any
//! colour instead ([`Rgb`], stored in the app's preferences): [`Palette::from_rgb`] turns it into
//! light and dark accents that pass WCAG contrast.

use std::fmt;

use gpui_kit::Hsla;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::color::{contrast, neutrals, rgb_hsla};

/// A colour as `0xRRGGBB`: what the user picked as accent, stored as is in the app's preferences
/// (as `"#rrggbb"` in JSON).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u32);

impl Rgb {
    /// Colours an accent picker offers besides the app's own (any other is a custom pick).
    pub const SUGGESTED: [Rgb; 8] = [
        Rgb(0xc6f26b), // lime
        Rgb(0x52b7be), // teal
        Rgb(0xff9a5c), // paprika
        Rgb(0xf2b544), // amber
        Rgb(0x93a6ff), // cobalt
        Rgb(0x7451b8), // violet
        Rgb(0xe5484d), // red
        Rgb(0x8a8f98), // grey
    ];

    /// `"#c6f26b"`, `"c6f26b"` or `"#c6f"` as a colour; `None` for anything else.
    pub fn parse(text: &str) -> Option<Rgb> {
        let hex = text.trim().trim_start_matches('#');
        let full = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 => hex.to_string(),
            _ => return None,
        };
        u32::from_str_radix(&full, 16).ok().map(Rgb)
    }

    /// As a colour to draw.
    pub fn hsla(self) -> Hsla {
        rgb_hsla(self.0)
    }
}

impl From<Hsla> for Rgb {
    fn from(c: Hsla) -> Self {
        Rgb(hsla_rgb(c))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:06x}", self.0 & 0xffffff)
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Rgb::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("{text:?} is not a colour (#rrggbb)")))
    }
}

/// The accent colours of one theme (light or dark), as RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    /// Fills: primary buttons, the selected item's background (at a low opacity), switches on.
    pub fill: u32,
    /// The fill under the pointer.
    pub fill_hover: u32,
    /// Text and icons on the fill (≥ 4.5:1 on it and on its hover).
    pub on_fill: u32,
    /// Accent text and thin marks: links, focus rings, a slider's filled part, the selected item's
    /// border (≥ 4.5:1 on panels as text, ≥ 3:1 on controls as a mark).
    pub mark: u32,
}

/// An app's accent colours, one set per theme. Neutrals, spacing and type are shared.
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
    /// them in both themes; in the light theme accent text and marks are a deep olive, since lime
    /// on a light panel is barely visible.
    pub const LIME: Palette = Palette {
        name: "Lime",
        dark: Accent { fill: 0xc6f26b, fill_hover: 0xe2faa8, on_fill: 0x0a0e17, mark: 0xc6f26b },
        light: Accent { fill: 0xc6f26b, fill_hover: 0xb4e350, on_fill: 0x0a0e17, mark: 0x3f6212 },
    };

    /// lucem's teal (from lucem's own site: the stain's complement, so selections and outlines
    /// stay visible on H&E): a light teal in the dark theme (near-black text on it), a deep teal in
    /// the light theme (white text on it).
    pub const TEAL: Palette = Palette {
        name: "Teal",
        dark: Accent { fill: 0x52b7be, fill_hover: 0x74c9cf, on_fill: 0x0b1416, mark: 0x6fd0c8 },
        light: Accent { fill: 0x1f6b71, fill_hover: 0x185a5f, on_fill: 0xffffff, mark: 0x1f6b71 },
    };

    /// garnish's paprika (Okabe–Ito's vermillion, the palette its figures use).
    pub const PAPRIKA: Palette = Palette {
        name: "Paprika",
        dark: Accent { fill: 0xff9a5c, fill_hover: 0xffb383, on_fill: 0x1a0d05, mark: 0xff9a5c },
        light: Accent { fill: 0xb54a00, fill_hover: 0x9a3f00, on_fill: 0xffffff, mark: 0xb54a00 },
    };

    /// kitchen-orchestrate's amber (the colour of a running run).
    pub const AMBER: Palette = Palette {
        name: "Amber",
        dark: Accent { fill: 0xf2b544, fill_hover: 0xf6c86e, on_fill: 0x161106, mark: 0xf2b544 },
        light: Accent { fill: 0x8a5a00, fill_hover: 0x744b00, on_fill: 0xffffff, mark: 0x8a5a00 },
    };

    /// kitchen's cobalt: the family's house colour (kitchen's own galleries and tools).
    pub const COBALT: Palette = Palette {
        name: "Cobalt",
        dark: Accent { fill: 0x93a6ff, fill_hover: 0xb0bfff, on_fill: 0x0b0e1c, mark: 0x93a6ff },
        light: Accent { fill: 0x3d55c9, fill_hover: 0x3347ad, on_fill: 0xffffff, mark: 0x3d55c9 },
    };

    /// lucem's former rose, now [`Palette::TEAL`].
    #[deprecated(since = "0.0.9", note = "lucem's accent is Teal: use Palette::TEAL")]
    pub const EOSIN: Palette = Palette::TEAL;

    /// The apps' default palettes.
    pub const APPS: [Palette; 5] = [Palette::LIME, Palette::TEAL, Palette::PAPRIKA, Palette::AMBER, Palette::COBALT];

    /// The accent of the dark (`true`) or light theme.
    pub fn accent(&self, dark: bool) -> Accent {
        if dark { self.dark } else { self.light }
    }

    /// The palette of a colour the user picked ([`Palette::from_rgb`], named "Custom").
    pub fn custom(rgb: Rgb) -> Palette {
        Palette::from_rgb("Custom", rgb.0)
    }

    /// A palette made from any colour, one accent per theme. The hue is kept; in the dark theme
    /// the colour is first made 25 % less colourful, so it does not glare (designparser
    /// `dark-mode-color-adaptation`). Then only its lightness moves (lighter in the dark theme,
    /// darker in the light one) until:
    ///
    /// - the fill stands apart from the panel (≥ 2:1) and white or near-black text reads on it
    ///   (≥ 4.5:1; whichever reads better is the text colour);
    /// - the hover steps away from that text, so it reads at least as well;
    /// - the mark reads as text on panels (≥ 4.5:1) and as a mark on controls (≥ 3:1).
    pub fn from_rgb(name: &'static str, rgb: u32) -> Palette {
        let base = rgb_hsla(rgb);
        Palette { name, dark: derive(base, true), light: derive(base, false) }
    }
}

/// One theme's accent from `base` ([`Palette::from_rgb`]).
fn derive(base: Hsla, dark: bool) -> Accent {
    let pick = |pair: (u32, u32)| rgb_hsla(if dark { pair.0 } else { pair.1 });
    let (panel, raised, control, hover) = (pick(neutrals::PANEL), pick(neutrals::RAISED), pick(neutrals::CONTROL), pick(neutrals::HOVER));
    // Lighter on dark panels, darker on light ones: the direction that always converges
    let step = if dark { 0.01 } else { -0.01 };
    let start = Hsla { s: if dark { base.s * 0.75 } else { base.s }, a: 1.0, ..base };
    // Checked on the colour as stored (8-bit RGB), so rounding cannot undo a pass
    let stored = |c: Hsla| rgb_hsla(hsla_rgb(c));
    let reach = |ok: &dyn Fn(Hsla) -> bool| {
        let mut c = start;
        for _ in 0..100 {
            if ok(stored(c)) {
                break;
            }
            c.l = (c.l + step).clamp(0.0, 1.0);
        }
        stored(c)
    };
    let fill = reach(&|c| contrast(c, panel) >= 2.0 && contrast(best_text_on(c), c) >= 4.5);
    let on_fill = best_text_on(fill);
    let fill_hover = Hsla { l: (fill.l + if on_fill.l > 0.5 { -0.06 } else { 0.06 }).clamp(0.0, 1.0), ..fill };
    let mark = reach(&|c| [panel, raised].iter().all(|b| contrast(c, *b) >= 4.5) && [control, hover].iter().all(|b| contrast(c, *b) >= 3.0));
    Accent { fill: hsla_rgb(fill), fill_hover: hsla_rgb(fill_hover), on_fill: hsla_rgb(on_fill), mark: hsla_rgb(mark) }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_reads_and_writes_hex() {
        assert_eq!(Rgb::parse("#c6f26b"), Some(Rgb(0xc6f26b)));
        assert_eq!(Rgb::parse(" C6F26B "), Some(Rgb(0xc6f26b)));
        assert_eq!(Rgb::parse("#fa0"), Some(Rgb(0xffaa00)));
        assert_eq!(Rgb::parse("#12345"), None);
        assert_eq!(Rgb::parse("#zzzzzz"), None);
        assert_eq!(serde_json::to_string(&Rgb(0x0a1030)).unwrap(), "\"#0a1030\"");
        assert_eq!(serde_json::from_str::<Option<Rgb>>("\"#1f6b71\"").unwrap(), Some(Rgb(0x1f6b71)));
        assert!(serde_json::from_str::<Rgb>("\"rose\"").is_err());
    }

    /// Any colour the user picks gives accents that pass in both themes (the pairs themselves are
    /// checked for every look in `color::tests`); here also: hue kept, fill apart from the panel.
    #[test]
    fn arbitrary_colours_pass_in_both_themes() {
        let picks = [("pale yellow", 0xfffacd), ("near-black navy", 0x0a1030), ("pure red", 0xff0000), ("white", 0xffffff), ("black", 0x000000), ("lime", 0xc6f26b), ("mid grey", 0x808080)];
        for (name, rgb) in picks {
            let p = Palette::from_rgb(name, rgb);
            for dark in [true, false] {
                let a = p.accent(dark);
                let panel = rgb_hsla(if dark { neutrals::PANEL.0 } else { neutrals::PANEL.1 });
                let (fill, mark) = (rgb_hsla(a.fill), rgb_hsla(a.mark));
                assert!(contrast(fill, panel) >= 2.0, "{name} dark={dark}: fill on panel {:.2}:1", contrast(fill, panel));
                assert!(contrast(rgb_hsla(a.on_fill), fill) >= 4.5, "{name} dark={dark}: text on fill");
                assert!(contrast(rgb_hsla(a.on_fill), rgb_hsla(a.fill_hover)) >= 4.5, "{name} dark={dark}: text on hover");
                assert!(contrast(mark, panel) >= 4.5, "{name} dark={dark}: mark on panel {:.2}:1", contrast(mark, panel));
                // Hue kept (when the colour has one)
                let src = rgb_hsla(rgb);
                if src.s > 0.2 && (0.1..0.9).contains(&fill.l) {
                    let d = (fill.h - src.h).abs().min(1.0 - (fill.h - src.h).abs());
                    assert!(d < 0.02, "{name} dark={dark}: hue {} → {}", src.h, fill.h);
                }
            }
        }
    }

    #[test]
    fn dark_accents_are_less_colourful() {
        let p = Palette::from_rgb("red", 0xff0000);
        let (dark, light) = (rgb_hsla(p.dark.fill), rgb_hsla(p.light.fill));
        assert!(dark.s < light.s, "dark {} vs light {}", dark.s, light.s);
    }
}
