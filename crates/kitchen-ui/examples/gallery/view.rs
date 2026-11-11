//! The gallery page: every widget and token of kitchen-ui, in the current look. The header
//! switches dark and light, the palette and the text size. Also drawn by the headless test
//! (`tests/gallery.rs`).

#![allow(dead_code)]

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use kitchen_ui::theme::{self, color, palette::hsla_rgb};
use kitchen_ui::widgets::{AccentChanged, AccentPicker, EmptyState, MenuSelect, Section, icon_button, row, scrub_label, slider, stepper, swatch};
use kitchen_ui::{Commit, Look, Palette, ParamEditor, Params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A band filter, standing in for an app's op in the parameter editor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DemoFilter {
    pub band: String,
    pub cutoff: Vec<f64>,
    pub order: u32,
    pub zero_phase: bool,
    pub taps: Vec<f64>,
}

impl Default for DemoFilter {
    fn default() -> Self {
        DemoFilter { band: "bandpass".into(), cutoff: vec![300.0, 6000.0], order: 4, zero_phase: true, taps: vec![0.1, 0.2, 0.4, 0.2, 0.1] }
    }
}

impl Params for DemoFilter {
    fn choices(field: &str) -> Option<&'static [&'static str]> {
        (field == "band").then_some(&["lowpass", "highpass", "bandpass"][..])
    }

    fn element_label(field: &str, len: usize, i: usize) -> String {
        match (field, len, i) {
            ("cutoff", 2, 0) => "Low (Hz)".into(),
            ("cutoff", 2, 1) => "High (Hz)".into(),
            ("cutoff", _, _) => "Cutoff (Hz)".into(),
            _ => format!("{} {}", kitchen_ui::param_editor::label_of(field), i + 1),
        }
    }

    fn on_choice(value: &mut Value, field: &str, choice: &str) {
        if field != "band" {
            return;
        }
        let cut: Vec<f64> = value["cutoff"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        let (lo, hi) = (cut.first().copied().unwrap_or(300.0), cut.last().copied().unwrap_or(6000.0));
        value["cutoff"] = match choice {
            "lowpass" => json!([hi]),
            "highpass" => json!([lo]),
            _ if cut.len() == 2 => json!(cut),
            _ => json!([lo, if hi > lo { hi } else { lo * 10.0 }]),
        };
    }
}

/// The gallery page.
pub struct Gallery {
    pub level: f32,
    pub choice: Option<usize>,
    pub count: i32,
    pub gain: f32,
    pub filter: DemoFilter,
    params: Entity<ParamEditor<DemoFilter>>,
    accent: Entity<AccentPicker>,
    _subs: Vec<Subscription>,
}

impl Gallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let accent = cx.new(|cx| AccentPicker::new(theme::look().accent, window, cx));
        // An app would also store the pick in its preferences
        let sub = cx.subscribe(&accent, |_, _, e: &AccentChanged, cx| {
            theme::use_look(theme::look().with_accent(e.0), cx);
            cx.notify();
        });
        let params = cx.new(|_| ParamEditor::new());
        Gallery { level: 0.4, choice: Some(1), count: 3, gain: 1.0, filter: DemoFilter::default(), params, accent, _subs: vec![sub] }
    }

    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let look = theme::look();
        let current = Palette::APPS.iter().position(|p| p.name == look.palette.name);
        let names: Vec<SharedString> = Palette::APPS.iter().map(|p| SharedString::from(p.name)).collect();
        let this = cx.entity().downgrade();
        let pick_palette = MenuSelect::new("gallery-palette", names, current, move |i, _, cx| {
            theme::use_look(theme::look().with_palette(Palette::APPS[i]), cx);
            let _ = this.update(cx, |_, cx| cx.notify());
        });
        let this = cx.entity().downgrade();
        let font = stepper("gallery-font", format!("{} px", look.font_size), move |step, _, cx| {
            let l = theme::look();
            theme::use_look(l.with_font_size(l.font_size + step), cx);
            let _ = this.update(cx, |_, cx| cx.notify());
        });
        let (icon, tip) = if look.dark { (IconName::Sun, "Light theme") } else { (IconName::Moon, "Dark theme") };
        let toggle = icon_button("gallery-theme", icon, tip).on_click(cx.listener(|_, _, _, cx| {
            let l = theme::look();
            theme::use_look(l.with_dark(!l.dark), cx);
            cx.notify();
        }));
        h_flex()
            .flex_none()
            .w_full()
            .px(px(theme::LG))
            .py(px(theme::SM))
            .flex_wrap()
            .gap_y(px(theme::SM))
            .gap_x(px(theme::LG))
            .bg(color::chrome())
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("kitchen-ui gallery"))
            .child(div().flex_1())
            .child(h_flex().gap(px(theme::SM)).child(div().text_sm().text_color(color::text_muted()).child("App")).child(div().w(px(140.)).child(pick_palette)))
            .child(h_flex().gap(px(theme::SM)).child(div().text_sm().text_color(color::text_muted()).child("Accent")).child(self.accent.clone()))
            .child(h_flex().gap(px(theme::SM)).child(div().text_sm().text_color(color::text_muted()).child("Text")).child(font))
            .child(toggle)
    }

    /// The labelled rows, in whatever width the pane gives them.
    fn rows(&self, prefix: &str, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let this = cx.entity().downgrade();
        let level = slider(format!("{prefix}-level"), self.level, (0.0, 1.0), |v| format!("{v:.2}"), move |v, _, _, cx| {
            let _ = this.update(cx, |g, cx| {
                g.level = v;
                cx.notify();
            });
        });
        let this = cx.entity().downgrade();
        let options: Vec<SharedString> = ["Nearest", "Bilinear", "A much longer option that cannot fit and ends in an ellipsis"].map(SharedString::from).to_vec();
        let pick = MenuSelect::new(SharedString::from(format!("{prefix}-pick")), options, self.choice, move |i, _, cx| {
            let _ = this.update(cx, |g, cx| {
                g.choice = Some(i);
                cx.notify();
            });
        });
        let this = cx.entity().downgrade();
        let count = stepper(format!("{prefix}-count"), self.count.to_string(), move |s, _, cx| {
            let _ = this.update(cx, |g, cx| {
                g.count = (g.count + s as i32).clamp(0, 99);
                cx.notify();
            });
        });
        let this = cx.entity().downgrade();
        let gain = scrub_label(format!("{prefix}-gain"), format!("{:.1}×", self.gain), move |s, _, cx| {
            let _ = this.update(cx, |g, cx| {
                g.gain = (g.gain + s * 0.1).clamp(0.0, 10.0);
                cx.notify();
            });
        });
        vec![
            row("Level", level).into_any_element(),
            row("Resampling", pick).into_any_element(),
            row("Count", count).into_any_element(),
            row("Gain (drag the number)", gain).into_any_element(),
            row("A label long enough to wrap onto two lines", div().text_sm().child("Value")).into_any_element(),
        ]
    }

    fn tokens(&self) -> impl IntoElement {
        let c = theme::colors();
        let colours: [(&str, Hsla); 16] = [
            ("chrome", c.chrome),
            ("panel", c.panel),
            ("raised", c.raised),
            ("control", c.control),
            ("hover", c.hover),
            ("border", c.border),
            ("text", c.text),
            ("text_muted", c.text_muted),
            ("text_disabled", c.text_disabled),
            ("accent", c.accent),
            ("accent_hover", c.accent_hover),
            ("on_accent", c.on_accent),
            ("accent_mark", c.accent_mark),
            ("danger", c.danger),
            ("success", c.success),
            ("warning", c.warning),
        ];
        let swatches = v_flex().gap(px(theme::XS)).children(colours.map(|(name, col)| {
            h_flex()
                .gap(px(theme::SM))
                .child(swatch(col, 16.))
                .child(div().flex_1().text_sm().child(name))
                .child(div().text_xs().font_family("monospace").text_color(color::text_muted()).child(format!("#{:06x}", hsla_rgb(col))))
        }));
        let type_scale = v_flex()
            .gap(px(theme::XS))
            .child(div().text_xs().text_color(color::text_muted()).child("Caption · text_xs"))
            .child(div().text_sm().child("Body · text_sm"))
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Title · text_base"))
            .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Heading · text_lg"));
        let spacing = v_flex().gap(px(theme::XS)).children(
            [("XS", theme::XS), ("SM", theme::SM), ("MD", theme::MD), ("LG", theme::LG), ("XL", theme::XL), ("XXL", theme::XXL)].map(|(name, v)| {
                h_flex()
                    .gap(px(theme::SM))
                    .child(div().w(px(36.)).text_xs().text_color(color::text_muted()).child(name))
                    .child(div().w(px(v)).h(px(8.)).rounded(px(2.)).bg(color::accent_mark()))
                    .child(div().text_xs().font_family("monospace").text_color(color::text_muted()).child(format!("{v} px")))
            }),
        );
        let accent_button = div()
            .px(px(theme::MD))
            .py(px(theme::XS))
            .rounded(px(theme::RADIUS))
            .bg(color::accent())
            .text_color(color::on_accent())
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .child("Text on the accent");
        v_flex()
            .gap(px(theme::LG))
            .child(Section::new("Colours").child(swatches).child(accent_button))
            .child(Section::new("Type scale").child(type_scale))
            .child(Section::new("Spacing").child(spacing))
    }
}

/// A pane: a rounded card on the chrome, scrolling when its content is taller.
fn pane(id: &'static str, title: &str, body: impl IntoElement) -> impl IntoElement {
    v_flex()
        .id(id)
        .h_full()
        .min_w_0()
        .overflow_y_scroll()
        .p(px(theme::LG))
        .gap(px(theme::LG))
        .rounded(px(theme::RADIUS_LG))
        .bg(color::panel())
        .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
        .child(body)
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let commit: Commit<DemoFilter> = Rc::new(move |f: DemoFilter, cx: &mut App| {
            if f.cutoff.iter().any(|c| *c <= 0.0) {
                return Err("Cutoffs must be above 0 Hz.".into());
            }
            this.update(cx, |g, cx| {
                g.filter = f;
                cx.notify();
            })
            .map_err(|e| e.to_string())
        });
        let filter = self.filter.clone();
        self.params.update(cx, |p, cx| p.set_target("demo", &filter, commit, window, cx));

        let wide = self.rows("gallery", cx);
        let narrow = self.rows("narrow", cx);
        let this = cx.entity().downgrade();
        let reset = icon_button("gallery-reset", IconName::Undo2, "Reset the values").on_click(move |_, _, cx| {
            let _ = this.update(cx, |g, cx| {
                g.level = 0.4;
                g.count = 3;
                g.gain = 1.0;
                cx.notify();
            });
        });
        let controls = v_flex()
            .gap(px(theme::XL))
            .child(Section::new("Rows").extra(reset).children(wide))
            .child(
                Section::new("Boxed section")
                    .boxed()
                    .child(row("Swatches", h_flex().gap(px(theme::XS)).children([0xe15759u32, 0x59a14f, 0x4e79a7, 0xedc948].map(|c| swatch(theme::rgb_hsla(c), 16.)))))
                    .child(row("Disabled text", div().text_sm().text_color(color::text_disabled()).child("Not available"))),
            )
            .child(
                Section::new("Icon buttons").child(
                    h_flex()
                        .gap(px(theme::SM))
                        .child(icon_button("gallery-plus", IconName::Plus, "Add"))
                        .child(icon_button("gallery-search", IconName::Search, "Search"))
                        .child(icon_button("gallery-settings", IconName::Settings, "Settings"))
                        .child(icon_button("gallery-close", IconName::Close, "Close")),
                ),
            );
        let narrow_pane = v_flex().gap(px(theme::SM)).child(div().text_xs().text_color(color::text_muted()).child("The same rows in a narrow pane: controls drop under their labels.")).child(Section::new("Rows").children(narrow));
        let empty = div().h(px(280.)).rounded(px(theme::RADIUS_LG)).border_1().border_color(color::border()).child(
            EmptyState::new(IconName::Microscope, "No image open", "Open an image or a project to start. Drop a file here, or use File › Open.")
                .child(gpui_kit::component::button::Button::new("gallery-open").child("Open…").map(|b| {
                    use gpui_kit::component::button::ButtonVariants as _;
                    b.primary()
                })),
        );
        let right = v_flex()
            .gap(px(theme::XL))
            .child(Section::new("Parameter editor").child(self.params.clone()))
            .child(Section::new("Empty state").child(empty));
        v_flex()
            .size_full()
            .bg(color::chrome())
            .text_color(color::text())
            .child(self.header(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_start()
                    .p(px(theme::PANE_GAP))
                    .gap(px(theme::PANE_GAP))
                    .child(div().flex_1().h_full().min_w_0().child(pane("pane-controls", "Controls", controls)))
                    .child(div().w(px(232.)).flex_none().h_full().child(pane("pane-narrow", "Narrow", narrow_pane)))
                    .child(div().flex_1().h_full().min_w_0().child(pane("pane-editor", "Editor and empty state", right)))
                    .child(div().w(px(300.)).flex_none().h_full().child(pane("pane-tokens", "Tokens", self.tokens()))),
            )
    }
}

/// The look the gallery opens with.
pub fn initial_look(light: bool) -> Look {
    Look::new(Palette::LIME).with_dark(!light)
}
