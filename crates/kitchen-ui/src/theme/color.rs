//! The colours: shared neutrals (greys with a slight cool bias: surfaces from the deepest, text,
//! borders), the data ground ([`canvas`], [`grid`]), status colours, the selection, and the accent
//! ([`super::Look::active_palette`]), resolved for the current look. The values are the Design
//! System's tokens (`tokens.json`), one for one.
//!
//! Every function here reads the look set with [`super::set_look`], so code can ask for a colour
//! without a context (`color::text_muted()`). [`Colors::resolve`] gives the same values for any
//! look, for tests and for drawing a theme that is not the current one.
//!
//! Surfaces step clearly apart; in the dark theme a border is a step lighter than the surfaces it
//! divides (an edge, not a ridge); text passes WCAG AA (≥ 4.5:1) on every surface and on the
//! canvas, marks ≥ 3:1 (designparser `wcag-contrast`). The tests check each pair.

use gpui_kit::{Hsla, Rgba, rgb};

use super::{Look, look};

/// The shared neutrals as `(dark, light)` RGB pairs: greys with a slight cool bias (blue a little
/// above red), so warm data and any accent stand off them.
pub mod neutrals {
    /// Title bar, menu row, status bar, and the window ground panes sit on.
    pub const CHROME: (u32, u32) = (0x16181b, 0xe9ebee);
    /// Panels and panes: where content sits.
    pub const PANEL: (u32, u32) = (0x1d1f23, 0xf6f7f9);
    /// Raised blocks inside panels, popovers, panel group headers, the code editor ground.
    pub const RAISED: (u32, u32) = (0x24272b, 0xffffff);
    /// Controls at rest: buttons, inputs, slider tracks.
    pub const CONTROL: (u32, u32) = (0x2c2f34, 0xe4e7eb);
    /// A control or row under the pointer; the editor's current line at half opacity.
    pub const HOVER: (u32, u32) = (0x363a40, 0xd8dce1);
    /// Separators and outlines, one pixel.
    pub const BORDER: (u32, u32) = (0x33373d, 0xc9ced5);
    /// Body text and icons.
    pub const TEXT: (u32, u32) = (0xe8eaed, 0x16191d);
    /// Secondary text: labels, captions, hints, axis text.
    pub const TEXT_MUTED: (u32, u32) = (0xa9aeb6, 0x4f5660);
    /// Text of disabled controls only (no contrast requirement).
    pub const TEXT_DISABLED: (u32, u32) = (0x646a73, 0x9aa1aa);
    /// The ground data is drawn on: traces, slides, figures, the pipeline graph. Deeper than the
    /// panel in the dark theme, white in the light one.
    pub const CANVAS: (u32, u32) = (0x0e0f11, 0xffffff);
    /// Gridlines and the pipeline's dot grid on the canvas. Never for text.
    pub const GRID: (u32, u32) = (0x23262b, 0xe9ecef);
    /// Errors and failed runs (text, marks and fills; always with a word or an icon).
    pub const DANGER: (u32, u32) = (0xff8577, 0xc22f22);
    /// Success and finished runs (text, marks and fills; always with a word or an icon).
    pub const SUCCESS: (u32, u32) = (0x46b583, 0x1b7448);
    /// Warnings and running jobs (text, marks and fills).
    pub const WARNING: (u32, u32) = (0xe8a838, 0x8a5a00);
    /// Neutral notices and reused runs (a result taken from an earlier run).
    pub const INFO: (u32, u32) = (0x7fa8ff, 0x2f5fb3);
}

/// The data colours (Okabe & Ito 2008), in the order to use them: groups in figures, overlays on
/// video, channels. Told apart with every common colour blindness, the same in both themes. The
/// seventh (yellow) reads poorly on light grounds: use it only when six are not enough.
pub const DATA: [u32; 7] = [0x0072b2, 0xe69f00, 0x009e73, 0xcc79a7, 0x56b4e9, 0xd55e00, 0xf0e442];

/// Data colour `i` (wrapping after [`DATA`]'s seven).
pub fn data(i: usize) -> Hsla {
    rgb_hsla(DATA[i % DATA.len()])
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
    /// Neutral notices, reused runs.
    pub info: Hsla,
    /// The ground data is drawn on.
    pub canvas: Hsla,
    /// Gridlines on the canvas.
    pub grid: Hsla,
    /// A selected row, button or toggle: the accent at [`SELECTION_TINT`] over its surface (drawn
    /// with a 1 px [`Colors::selection_edge`], 2 px on the left in lists).
    pub selection: Hsla,
    /// The edge of a selected item ([`Colors::accent_mark`]).
    pub selection_edge: Hsla,
    /// The focus ring: the accent mark, 2 px, outside the control.
    pub focus: Hsla,
    /// Background of an added line in a diff: [`Colors::success`] mixed into the panel.
    pub diff_added: Hsla,
    /// Background of a removed line in a diff: [`Colors::danger`] mixed into the panel.
    pub diff_removed: Hsla,
}

/// How much of the status colour a diff line's background takes (the rest is the panel).
pub const DIFF_TINT: f32 = 0.16;

/// How much of the accent a selected item's background takes ([`Colors::selection`]), at most:
/// less for an accent where this much would leave text unreadable ([`selection_tint`]).
pub const SELECTION_TINT: f32 = 0.16;

/// The tint of `accent` over `panel` for a selected item: [`SELECTION_TINT`], or the largest step
/// below it at which each of `texts` still reads (≥ 4.5:1) and the selected edge `edge` still
/// shows (≥ 3:1). A very light accent in the dark theme (a user's white, say) gets a fainter tint.
pub fn selection_tint(panel: Hsla, accent: Hsla, texts: [Hsla; 2], edge: Hsla) -> f32 {
    let mut t = SELECTION_TINT;
    while t > 0.04 {
        let bg = mix(panel, accent, t);
        if texts.iter().all(|x| contrast(*x, bg) >= 4.5) && contrast(edge, bg) >= 3.0 {
            break;
        }
        t -= 0.02;
    }
    t
}

/// Width of the focus ring (px).
pub const FOCUS_RING_W: f32 = 2.;

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
            info: n(neutrals::INFO),
            canvas: n(neutrals::CANVAS),
            grid: n(neutrals::GRID),
            selection: rgb_hsla(a.fill).opacity(selection_tint(n(neutrals::PANEL), rgb_hsla(a.fill), [n(neutrals::TEXT), n(neutrals::TEXT_MUTED)], rgb_hsla(a.mark))),
            selection_edge: rgb_hsla(a.mark),
            focus: rgb_hsla(a.mark),
            diff_added: mix(n(neutrals::PANEL), n(neutrals::SUCCESS), DIFF_TINT),
            diff_removed: mix(n(neutrals::PANEL), n(neutrals::DANGER), DIFF_TINT),
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

/// Neutral notices, reused runs.
pub fn info() -> Hsla {
    colors().info
}
/// The ground data is drawn on: traces, slides, figures, the pipeline graph.
pub fn canvas() -> Hsla {
    colors().canvas
}
/// Gridlines and the dot grid on the canvas. Never for text.
pub fn grid() -> Hsla {
    colors().grid
}
/// A selected item's background: the accent at 16 % (draw it with [`selection_edge`]).
pub fn selection() -> Hsla {
    colors().selection
}
/// The 1 px edge of a selected item.
pub fn selection_edge() -> Hsla {
    colors().selection_edge
}
/// The focus ring.
pub fn focus() -> Hsla {
    colors().focus
}

/// Background of an added line in a diff (text, muted text and the success mark pass on it).
pub fn diff_added() -> Hsla {
    colors().diff_added
}
/// Background of a removed line in a diff (text, muted text and the danger mark pass on it).
pub fn diff_removed() -> Hsla {
    colors().diff_removed
}

/// `a` with `t` (0–1) of `b` mixed in, in sRGB, opaque.
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b) = (a.to_rgb(), b.to_rgb());
    let m = |x: f32, y: f32| x + (y - x) * t;
    Rgba { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: 1.0 }.into()
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
    fn neutrals_have_a_slight_cool_bias() {
        use neutrals::*;
        let all = [("chrome", CHROME), ("panel", PANEL), ("raised", RAISED), ("control", CONTROL), ("hover", HOVER), ("border", BORDER), ("text", TEXT), ("text_muted", TEXT_MUTED), ("text_disabled", TEXT_DISABLED), ("canvas", CANVAS), ("grid", GRID)];
        for (name, pair) in all {
            for c in [pair.0, pair.1] {
                let (r, g, b) = (c >> 16 & 0xff, c >> 8 & 0xff, c & 0xff);
                assert!(r <= g && g <= b, "{name}: #{c:06x} leans warm");
                assert!(b - r <= 20, "{name}: #{c:06x} is tinted, not neutral");
            }
        }
    }

    #[test]
    fn data_ground_and_notices_pass() {
        for dark in [true, false] {
            let c = Colors::resolve(&Look::new(Palette::LIME).with_dark(dark));
            for (tname, t) in [("text", c.text), ("text_muted", c.text_muted)] {
                let r = contrast(t, c.canvas);
                assert!(r >= 4.5, "dark={dark}: {tname} on canvas is {r:.2}:1");
            }
            assert!(c.grid != c.canvas, "dark={dark}: the grid shows on the canvas");
            for (sname, s) in [("chrome", c.chrome), ("panel", c.panel), ("raised", c.raised)] {
                let r = contrast(c.info, s);
                assert!(r >= 4.5, "dark={dark}: info on {sname} is {r:.2}:1");
            }
            for (sname, s) in [("chrome", c.chrome), ("panel", c.panel), ("raised", c.raised), ("control", c.control), ("hover", c.hover)] {
                let r = contrast(c.warning, s);
                assert!(r >= 3.0, "dark={dark}: warning as a mark on {sname} is {r:.2}:1");
            }
        }
    }

    /// A selected row is the accent at up to 16 % over the panel: its text still reads, its edge
    /// shows. The app palettes keep the full 16 %.
    #[test]
    fn selected_rows_pass() {
        for l in looks() {
            let c = Colors::resolve(&l);
            let tint = c.selection.a;
            assert!(tint <= SELECTION_TINT + 1e-6 && tint > 0.04, "tint {tint}");
            if l.accent.is_none() {
                assert!((tint - SELECTION_TINT).abs() < 1e-6, "{}: the app palette keeps 16 %", l.palette.name);
            }
            let bg = mix(c.panel, c.accent, tint);
            let name = format!("{} accent={:?} dark={}", l.palette.name, l.accent, l.dark);
            for (tname, t) in [("text", c.text), ("text_muted", c.text_muted)] {
                let r = contrast(t, bg);
                assert!(r >= 4.5, "{name}: {tname} on a selected row is {r:.2}:1");
            }
            let r = contrast(c.selection_edge, bg);
            assert!(r >= 3.0, "{name}: the selected edge is {r:.2}:1");
        }
    }

    #[test]
    fn data_colours_are_okabe_ito() {
        assert_eq!(DATA.len(), 7);
        assert_eq!(data(0), data(7));
        let set: std::collections::BTreeSet<u32> = DATA.into_iter().collect();
        assert_eq!(set.len(), 7, "seven distinct colours");
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
    fn diff_lines_pass_contrast() {
        for dark in [true, false] {
            let c = Colors::resolve(&Look::new(Palette::LIME).with_dark(dark));
            for (bname, bg, mark) in [("added", c.diff_added, c.success), ("removed", c.diff_removed, c.danger)] {
                // The line's text and its line numbers
                for (tname, t) in [("text", c.text), ("text_muted", c.text_muted)] {
                    let r = contrast(t, bg);
                    assert!(r >= 4.5, "dark={dark}: {tname} on the {bname} line is {r:.2}:1");
                }
                // The + / − sign is a mark
                let r = contrast(mark, bg);
                assert!(r >= 3.0, "dark={dark}: the sign on the {bname} line is {r:.2}:1");
                // The tint is visible against the panel
                assert!(bg != c.panel, "dark={dark}: the {bname} line is not tinted");
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
