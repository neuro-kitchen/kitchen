//! The search (Ctrl+K, F3; as Spotlight or Blender's F3): every registered command by name,
//! filtered as you type, under its group: an icon tile, the title, a line saying what it does,
//! its key at the right. A command that cannot run now stays listed, greyed, with the reason in
//! place of its line. ↑ ↓ move between the commands that can run, Enter runs the highlighted
//! one, a click runs any, Escape or a click outside closes.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use kitchen_ui::theme::{self, color};

use crate::command::{CommandSpec, Registry};

/// What the search asks of its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchEvent {
    /// Close it.
    Close,
    /// Close it, then run the command `id` (the owner runs it once the search is gone, so what it
    /// opens keeps the focus).
    Run(SharedString),
}

/// Which commands a search lists (all of them by default).
pub type Filter = Rc<dyn Fn(&CommandSpec) -> bool>;

/// A row of the search: a command, and why it cannot run now (if it cannot).
#[derive(Clone)]
pub struct Row {
    /// The command.
    pub command: CommandSpec,
    /// Why it cannot run now.
    pub disabled: Option<String>,
}

/// The search over a [`Registry`].
pub struct Search {
    registry: Registry,
    filter: Option<Filter>,
    input: Entity<InputState>,
    title: SharedString,
    /// The highlighted row (of the filtered list).
    cursor: usize,
    _subs: Vec<Subscription>,
}

impl EventEmitter<SearchEvent> for Search {}

/// Rows listed at most.
const SHOWN: usize = 50;
/// Height of a row (px): two lines of text and the icon tile.
const ROW_H: f32 = 48.;

impl Search {
    /// A search over every command of `registry`; focuses its input.
    pub fn new(registry: Registry, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search commands…"));
        let subs = vec![cx.subscribe_in(&input, window, |this, _, e: &InputEvent, _, cx| match e {
            InputEvent::PressEnter { .. } => this.run_cursor(cx),
            InputEvent::Change => {
                this.cursor = this.first_enabled(cx);
                cx.notify()
            }
            _ => {}
        })];
        let focus = gpui_kit::Focusable::focus_handle(input.read(cx), cx);
        window.focus(&focus, cx);
        let mut s = Self { registry, filter: None, input, title: "Search".into(), cursor: 0, _subs: subs };
        s.cursor = s.first_enabled(cx);
        s
    }

    /// Lists only the commands `filter` accepts (Process ▸ "Find an operation…": one group).
    pub fn filter(mut self, filter: impl Fn(&CommandSpec) -> bool + 'static) -> Self {
        self.filter = Some(Rc::new(filter));
        self
    }

    /// Sets the heading ("Search", "Find an operation").
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the text shown in the empty input.
    pub fn placeholder(self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text: SharedString = text.into();
        self.input.update(cx, |i, cx| i.set_placeholder(text, window, cx));
        self
    }

    /// Types `query` into the search (as a person would).
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        let q = query.to_string();
        self.input.update(cx, |i, cx| i.set_value(q, window, cx));
        self.cursor = self.first_enabled(cx);
        cx.notify();
    }

    /// The rows matching what is typed: every word typed is in the group, title, hint or key.
    pub fn rows(&self, cx: &App) -> Vec<Row> {
        let q = self.input.read(cx).value().to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut rows: Vec<Row> = self
            .registry
            .all()
            .into_iter()
            .filter(|c| self.filter.as_ref().is_none_or(|f| f(c)))
            .filter(|c| {
                let hay = format!("{} {} {} {}", c.group, c.title, c.hint, c.key_label().unwrap_or_default()).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .map(|c| Row { disabled: (c.enabled)(cx).err(), command: c })
            .collect();
        // Grouped, each group where its first command was registered
        let order: Vec<SharedString> = rows.iter().map(|r| r.command.group.clone()).fold(Vec::new(), |mut v, g| {
            if !v.contains(&g) {
                v.push(g);
            }
            v
        });
        rows.sort_by_key(|r| order.iter().position(|g| *g == r.command.group));
        rows.truncate(SHOWN);
        rows
    }

    /// The highlighted row's index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn first_enabled(&self, cx: &App) -> usize {
        self.rows(cx).iter().position(|r| r.disabled.is_none()).unwrap_or(0)
    }

    fn run_cursor(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.rows(cx).into_iter().nth(self.cursor).filter(|r| r.disabled.is_none()) {
            cx.emit(SearchEvent::Run(r.command.id));
        }
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows(cx);
        let enabled: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| r.disabled.is_none()).map(|(i, _)| i).collect();
        let at = enabled.iter().position(|&i| i == self.cursor);
        match e.keystroke.key.as_str() {
            "down" if !enabled.is_empty() => self.cursor = enabled[at.map_or(0, |i| (i + 1).min(enabled.len() - 1))],
            "up" if !enabled.is_empty() => self.cursor = enabled[at.map_or(0, |i| i.saturating_sub(1))],
            "escape" => cx.emit(SearchEvent::Close),
            "enter" => {
                self.run_cursor(cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}

impl Render for Search {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows(cx);
        let cursor = self.cursor;
        let mut list = v_flex().id("search-rows").flex_1().min_h_0().overflow_y_scroll();
        let mut last_group = SharedString::default();
        if rows.is_empty() {
            list = list.child(div().p(px(theme::MD)).text_sm().text_color(color::text_muted()).child("No command matches."));
        }
        for (i, r) in rows.into_iter().enumerate() {
            let c = r.command;
            if c.group != last_group {
                last_group = c.group.clone();
                list = list.child(
                    div().px(px(theme::SM)).pt(px(if i == 0 { theme::XS } else { theme::MD })).pb(px(theme::XS)).text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color::text_muted()).child(c.group.clone()),
                );
            }
            let enabled = r.disabled.is_none();
            let line: SharedString = r.disabled.map(SharedString::from).unwrap_or(c.hint.clone());
            let me = cx.entity().downgrade();
            let id = c.id.clone();
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("search-row-{}", c.id)))
                    .min_h(px(ROW_H))
                    .px(px(theme::SM))
                    .gap(px(theme::MD))
                    .rounded(px(theme::RADIUS))
                    .when(enabled && i == cursor, |d| d.bg(color::control()))
                    .when(enabled, |d| {
                        d.cursor_pointer().hover(|d| d.bg(color::control())).on_click(move |_, _, cx| {
                            let id = id.clone();
                            let _ = me.update(cx, |_, cx| cx.emit(SearchEvent::Run(id)));
                        })
                    })
                    .child(
                        div()
                            .flex_none()
                            .size(px(32.))
                            .rounded(px(theme::RADIUS))
                            .border_1()
                            .border_color(color::hover())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(c.icon).text_color(if enabled { color::text() } else { color::text_disabled() })),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).truncate().text_color(if enabled { color::text() } else { color::text_disabled() }).child(c.title.clone()))
                            .child(div().text_xs().text_color(color::text_muted()).truncate().child(line)),
                    )
                    .children(c.key_label().map(|k| div().flex_none().text_xs().font_family("monospace").text_color(color::text_muted()).child(k))),
            );
        }
        let (back, close) = (cx.entity().downgrade(), cx.entity().downgrade());
        div()
            .id("search-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(72.))
            .px(px(theme::LG))
            .bg(gpui_kit::black().opacity(0.35))
            .on_click(move |_, _, cx| {
                let _ = back.update(cx, |_, cx| cx.emit(SearchEvent::Close));
            })
            .child(
                v_flex()
                    .id("search")
                    .occlude()
                    // Narrows with the window, never wider than reads well
                    .w_full()
                    .max_w(px(560.))
                    .max_h(gpui_kit::relative(0.8))
                    .p(px(theme::SM))
                    .gap(px(theme::XS))
                    .rounded(px(theme::RADIUS_LG))
                    .border_1()
                    .border_color(color::border())
                    .bg(color::raised())
                    .text_color(color::text())
                    .shadow_lg()
                    .capture_key_down(cx.listener(Self::on_key))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .px(px(theme::SM))
                            .py(px(theme::XS))
                            .gap(px(theme::SM))
                            .items_center()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(self.title.clone()))
                            .child(div().text_xs().text_color(color::text_muted()).child("↑ ↓ Enter · Esc"))
                            .child(div().flex_1())
                            .child(kitchen_ui::widgets::icon_button("search-close", IconName::Close, "Close (Esc)").on_click(move |_, _, cx| {
                                let _ = close.update(cx, |_, cx| cx.emit(SearchEvent::Close));
                            })),
                    )
                    .child(Input::new(&self.input))
                    .child(list),
            )
    }
}
