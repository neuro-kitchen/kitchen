//! The accent picker: the app's accent, a few suggested colours, or any colour.

use gpui_kit::component::button::Button;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Window, div, px,
};

use crate::theme::{self, Rgb, color};
use crate::widgets::swatch;

/// What the picker reports: the accent picked (`None`: the app's own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentChanged(pub Option<Rgb>);

/// A row to choose the accent: "App" (the app's palette), the suggested colours
/// ([`Rgb::SUGGESTED`]) as swatches, and gpui-kit's colour picker for any other colour (hex,
/// hue, saturation, lightness). The current choice is ringed, not only coloured (WCAG 1.4.1).
/// Each swatch is a 24 px target (designparser `touch-target`). It emits [`AccentChanged`]; the
/// owner stores the value and applies it (`theme::use_look(look.with_accent(…))`).
pub struct AccentPicker {
    current: Option<Rgb>,
    suggested: Vec<Rgb>,
    custom: Entity<ColorPickerState>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<AccentChanged> for AccentPicker {}

impl AccentPicker {
    /// A picker showing `current` (`None`: the app's accent) and the suggested colours.
    pub fn new(current: Option<Rgb>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let custom = cx.new(|cx| {
            let state = ColorPickerState::new(window, cx);
            match current {
                Some(rgb) => state.default_value(rgb.hsla()),
                None => state,
            }
        });
        let sub = cx.subscribe(&custom, |this, _, e: &ColorPickerEvent, cx| {
            if let ColorPickerEvent::Change(Some(c)) = e {
                this.pick(Some(Rgb::from(*c)), cx);
            }
        });
        Self { current, suggested: Rgb::SUGGESTED.to_vec(), custom, _subs: vec![sub] }
    }

    /// The colours offered as swatches (by default [`Rgb::SUGGESTED`]).
    pub fn set_suggested(&mut self, colours: Vec<Rgb>, cx: &mut Context<Self>) {
        self.suggested = colours;
        cx.notify();
    }

    /// The accent shown as chosen (when it changed elsewhere, such as in the preferences).
    pub fn set_current(&mut self, current: Option<Rgb>, cx: &mut Context<Self>) {
        self.current = current;
        cx.notify();
    }

    /// The accent chosen (`None`: the app's).
    pub fn current(&self) -> Option<Rgb> {
        self.current
    }

    /// Chooses `accent` and tells the owner.
    pub fn pick(&mut self, accent: Option<Rgb>, cx: &mut Context<Self>) {
        if self.current != accent {
            self.current = accent;
            cx.emit(AccentChanged(accent));
            cx.notify();
        }
    }
}

impl Render for AccentPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current;
        let app = Button::new("accent-app").small().outline().label("App").selected(current.is_none()).tooltip("The app's own accent").on_click(cx.listener(|this, _, _, cx| this.pick(None, cx)));
        let swatches = self.suggested.iter().copied().map(|rgb| {
            let chosen = current == Some(rgb);
            let tip = SharedString::from(rgb.to_string());
            div()
                .id(SharedString::from(format!("accent-{:06x}", rgb.0)))
                .flex_none()
                .size(px(theme::TARGET_MIN))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::RADIUS))
                .border_2()
                .border_color(if chosen { color::text() } else { gpui_kit::transparent_black() })
                .when(!chosen, |d| d.hover(|d| d.bg(color::hover())))
                .cursor_pointer()
                .child(swatch(rgb.hsla(), 16.))
                .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                .on_click(cx.listener(move |this, _, _, cx| this.pick(Some(rgb), cx)))
                .test_support()
        });
        let custom_chosen = current.is_some_and(|c| !self.suggested.contains(&c));
        let featured = self.suggested.iter().map(|c| c.hsla()).collect();
        let custom = div()
            .flex_none()
            .rounded(px(theme::RADIUS))
            .border_2()
            .border_color(if custom_chosen { color::text() } else { gpui_kit::transparent_black() })
            .child(ColorPicker::new(&self.custom).small().featured_colors(featured).label("Custom"));
        h_flex().gap(px(theme::XS)).items_center().flex_wrap().child(app).children(swatches).child(custom)
    }
}
