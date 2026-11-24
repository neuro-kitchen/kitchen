//! The file picker's popup ([`FilePickerView`]), over the frame when File → Open… runs. It draws
//! [`crate::picker::Picker`]:
//!
//! - **Left**: the places (home, Desktop, Documents, Downloads, the computer) and the app's
//!   recent files.
//! - **Top**: up one folder, then the breadcrumb (each folder a button).
//! - **List**: folders first, then what the app opens, with kind, size and date; the rest only
//!   with "Show all files", greyed. A click selects, a double click enters a folder or opens.
//! - **Footer**: what the app opens, "Show all files", Cancel and Open.
//!
//! Keys: ↑ ↓ move, Enter opens or enters, Backspace goes up, Esc cancels. On a narrow window the
//! places fold away (they are one Home button in the top row).

use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    ClickEvent, Context, EventEmitter, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use kitchen_ui::icons::standard;
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::icon_button;

use crate::picker::{EntryKind, Picker, Place, human_size, places};

/// What the picker asks of its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerEvent {
    /// Close it and open this.
    Open(PathBuf),
    /// Close it, opening nothing.
    Cancel,
}

/// Below this width (px of the popup) the places column folds away.
const PLACES_MIN_W: f32 = 560.;
/// Height of a row of the list (px).
const ROW_H: f32 = 28.;

/// The file picker's popup.
pub struct FilePickerView {
    picker: Picker,
    places: Vec<Place>,
    title: SharedString,
    focus: FocusHandle,
}

impl EventEmitter<PickerEvent> for FilePickerView {}

impl FilePickerView {
    /// The popup over `picker`, focused.
    pub fn new(picker: Picker, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self { picker, places: places(), title: "Open".into(), focus }
    }

    /// Sets the heading ("Open", "Open a recording").
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The model (for tests).
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// The model, to change it (for tests).
    pub fn picker_mut(&mut self) -> &mut Picker {
        &mut self.picker
    }

    fn go(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let _ = self.picker.enter(&dir);
        cx.notify();
    }

    fn activate(&mut self, i: usize, cx: &mut Context<Self>) {
        if let Some(path) = self.picker.activate(i) {
            cx.emit(PickerEvent::Open(path));
        }
        cx.notify();
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(i) = self.picker.selected() {
            self.activate(i, cx);
        }
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let n = self.picker.entries().len();
        match e.keystroke.key.as_str() {
            "escape" => cx.emit(PickerEvent::Cancel),
            "enter" => self.open_selected(cx),
            "backspace" => {
                self.picker.up();
            }
            "down" if n > 0 => self.picker.select(Some(self.picker.selected().map_or(0, |i| (i + 1).min(n - 1)))),
            "up" if n > 0 => self.picker.select(Some(self.picker.selected().map_or(0, |i| i.saturating_sub(1)))),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn places_column(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let place = |id: String, name: String, icon: IconName, path: PathBuf, here: bool, cx: &mut Context<Self>| {
            h_flex()
                .id(SharedString::from(id))
                .h(px(ROW_H))
                .px(px(theme::SM))
                .gap(px(theme::SM))
                .items_center()
                .rounded(px(theme::RADIUS))
                .text_sm()
                .cursor_pointer()
                .hover(|d| d.bg(color::hover()))
                .when(here, |d| d.bg(color::selection()).border_l_2().border_color(color::selection_edge()))
                .child(Icon::new(icon).small().text_color(color::text_muted()))
                .child(div().min_w_0().truncate().child(name))
                .on_click(cx.listener(move |this, _, _, cx| this.go(path.clone(), cx)))
        };
        let mut col = v_flex().w(px(180.)).flex_none().gap(px(2.)).p(px(theme::SM)).border_r_1().border_color(color::border());
        col = col.child(section_label("Places"));
        for (i, p) in self.places.iter().enumerate() {
            let here = p.path == self.picker.dir();
            col = col.child(place(format!("picker-place-{i}"), p.name.clone(), p.icon, p.path.clone(), here, cx));
        }
        let recent = self.picker.recent();
        if !recent.is_empty() {
            col = col.child(div().h(px(theme::SM))).child(section_label("Recent"));
            for (i, path) in recent.into_iter().take(6).enumerate() {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let me = cx.entity().downgrade();
                let open = path.clone();
                col = col.child(
                    h_flex()
                        .id(SharedString::from(format!("picker-recent-{i}")))
                        .h(px(ROW_H))
                        .px(px(theme::SM))
                        .gap(px(theme::SM))
                        .items_center()
                        .rounded(px(theme::RADIUS))
                        .text_sm()
                        .cursor_pointer()
                        .hover(|d| d.bg(color::hover()))
                        .child(Icon::new(standard::RECENT).small().text_color(color::text_muted()))
                        .child(div().min_w_0().truncate().child(name))
                        .on_click(move |_, _, cx| {
                            let open = open.clone();
                            let _ = me.update(cx, |_, cx| cx.emit(PickerEvent::Open(open)));
                        }),
                );
            }
        }
        col
    }

    fn top_row(&self, wide: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let crumbs = self.picker.crumbs();
        let last = crumbs.len().saturating_sub(1);
        // Narrow: the last three folders, the rest behind "…"
        let skip = if wide { 0 } else { crumbs.len().saturating_sub(3) };
        let mut row = h_flex().id("picker-crumbs").flex_1().min_w_0().gap(px(2.)).items_center().overflow_x_scroll();
        if skip > 0 {
            row = row.child(div().text_sm().text_color(color::text_muted()).child("…"));
        }
        for (i, (name, path)) in crumbs.into_iter().enumerate().skip(skip) {
            if i > skip || skip > 0 {
                row = row.child(Icon::new(IconName::ChevronRight).xsmall().text_color(color::text_muted()));
            }
            row = row.child(
                Button::new(SharedString::from(format!("picker-crumb-{i}")))
                    .ghost()
                    .small()
                    .label(name)
                    .selected(i == last)
                    .on_click(cx.listener(move |this, _, _, cx| this.go(path.clone(), cx))),
            );
        }
        h_flex()
            .gap(px(theme::XS))
            .px(px(theme::SM))
            .py(px(theme::XS))
            .items_center()
            .border_b_1()
            .border_color(color::border())
            .child(icon_button("picker-up", standard::UP, "Up one folder (Backspace)").on_click(cx.listener(|this, _, _, cx| {
                this.picker.up();
                cx.notify();
            })))
            .when(!wide, |d| {
                let home = self.places.first().map(|p| p.path.clone());
                d.children(home.map(|h| icon_button("picker-home", standard::HOME, "Home").on_click(cx.listener(move |this, _, _, cx| this.go(h.clone(), cx)))))
            })
            .child(row)
    }

    fn list(&self, wide: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = v_flex().id("picker-list").flex_1().min_h_0().overflow_y_scroll().p(px(theme::XS)).gap(px(1.));
        if let Some(e) = self.picker.error() {
            list = list.child(div().p(px(theme::SM)).text_sm().text_color(color::danger()).child(format!("Cannot open that folder. {e}")));
        }
        if self.picker.entries().is_empty() {
            let line = if self.picker.show_all() { "This folder is empty." } else { "Nothing here the app opens. Turn on \"Show all files\" to see everything." };
            list = list.child(div().p(px(theme::MD)).text_sm().text_color(color::text_muted()).child(line));
        }
        let selected = self.picker.selected();
        for (i, e) in self.picker.entries().iter().enumerate() {
            let (icon, kind, dim) = match e.kind {
                EntryKind::Folder => (IconName::Folder, "Folder".to_string(), false),
                EntryKind::Opens(k) => match self.picker.kinds().get(k) {
                    Some(k) => (k.icon, k.name.clone(), false),
                    None => (IconName::File, "File".to_string(), false),
                },
                EntryKind::Other => (IconName::File, String::new(), true),
            };
            let date = e.modified.and_then(|m| jiff::Timestamp::try_from(m).ok()).map(|t| t.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d %H:%M").to_string());
            let text = if dim { color::text_disabled() } else { color::text() };
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("picker-entry-{i}")))
                    .h(px(ROW_H))
                    .px(px(theme::SM))
                    .gap(px(theme::SM))
                    .items_center()
                    .rounded(px(theme::RADIUS))
                    .text_sm()
                    .text_color(text)
                    .cursor_pointer()
                    .hover(|d| d.bg(color::hover()))
                    .when(selected == Some(i), |d| d.bg(color::selection()).border_l_2().border_color(color::selection_edge()))
                    .child(Icon::new(icon).small().text_color(if dim { color::text_disabled() } else { color::text_muted() }))
                    .child(div().flex_1().min_w_0().truncate().child(e.name.clone()))
                    .when(wide, |d| d.child(div().w(px(120.)).flex_none().truncate().text_xs().text_color(color::text_muted()).child(kind)))
                    .child(div().w(px(72.)).flex_none().text_right().text_xs().font_family(kitchen_ui::theme::fonts::MONO).text_color(color::text_muted()).child(e.size.map(human_size).unwrap_or_default()))
                    .when(wide, |d| d.child(div().w(px(128.)).flex_none().text_right().text_xs().font_family(kitchen_ui::theme::fonts::MONO).text_color(color::text_muted()).child(date.unwrap_or_default())))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.picker.select(Some(i));
                        if ev.click_count() >= 2 {
                            this.activate(i, cx);
                        }
                        cx.notify();
                    })),
            );
        }
        list
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_open = self.picker.openable().is_some() || self.picker.selected_entry().is_some_and(|e| e.kind == EntryKind::Folder);
        let label = if self.picker.selected_entry().is_some_and(|e| e.kind == EntryKind::Folder) { "Go into" } else { "Open" };
        h_flex()
            .flex_wrap()
            .gap(px(theme::SM))
            .px(px(theme::MD))
            .py(px(theme::SM))
            .items_center()
            .border_t_1()
            .border_color(color::border())
            .child(div().flex_1().min_w(px(160.)).text_xs().text_color(color::text_muted()).child(self.picker.opens_line()))
            .child(Checkbox::new("picker-show-all").label("Show all files").checked(self.picker.show_all()).on_click(cx.listener(|this, on: &bool, _, cx| {
                this.picker.set_show_all(*on);
                cx.notify();
            })))
            .child(Button::new("picker-cancel").label("Cancel").small().on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Cancel))))
            .child(Button::new("picker-open").primary().small().label(label).disabled(!can_open).on_click(cx.listener(|this, _, _, cx| this.open_selected(cx))))
    }
}

fn section_label(text: &'static str) -> impl IntoElement {
    div().px(px(theme::SM)).pb(px(2.)).text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color::text_muted()).child(text)
}

impl Render for FilePickerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = window.viewport_size().width.as_f32();
        let popup_w = (width - 2. * theme::LG).min(880.);
        let wide = popup_w >= PLACES_MIN_W;
        let back = cx.entity().downgrade();
        div()
            .id("picker-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(if wide { 56. } else { theme::LG }))
            .px(px(theme::LG))
            .bg(gpui_kit::black().opacity(0.35))
            .on_click(move |_, _, cx| {
                let _ = back.update(cx, |_, cx| cx.emit(PickerEvent::Cancel));
            })
            .child(
                v_flex()
                    .id("picker")
                    .track_focus(&self.focus)
                    .occlude()
                    .w(px(popup_w))
                    .h(gpui_kit::relative(if wide { 0.75 } else { 0.9 }))
                    .rounded(px(theme::RADIUS_LG))
                    .border_1()
                    .border_color(color::border())
                    .bg(color::raised())
                    .text_color(color::text())
                    .shadow_lg()
                    .overflow_hidden()
                    .capture_key_down(cx.listener(Self::on_key))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .px(px(theme::MD))
                            .py(px(theme::SM))
                            .gap(px(theme::SM))
                            .items_center()
                            .border_b_1()
                            .border_color(color::border())
                            .child(Icon::new(standard::OPEN).small().text_color(color::accent_mark()))
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(self.title.clone()))
                            .child(div().flex_1())
                            .child(div().text_xs().text_color(color::text_muted()).when(wide, |d| d.child("↑ ↓ Enter · Backspace up · Esc")))
                            .child(icon_button("picker-close", IconName::Close, "Close (Esc)").on_click(cx.listener(|_, _, _, cx| cx.emit(PickerEvent::Cancel)))),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .when(wide, |d| d.child(self.places_column(cx)))
                            .child(v_flex().flex_1().min_w_0().min_h_0().child(self.top_row(wide, cx)).child(self.list(wide, cx))),
                    )
                    .child(self.footer(cx)),
            )
    }
}
