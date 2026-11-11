//! The gallery page: the demo app's menus, built from its command registry, the search over the
//! same registry (Ctrl+K), and a log of what ran. Also drawn by the headless tests.

#![allow(dead_code)]

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled as _,
    Subscription, Window, div, px,
};
use kitchen_shell::{Registry, RunCommand, Search, SearchEvent};
use kitchen_ui::theme::{self, color};

use crate::demo::{self, DemoState, DemoStateHolder};

/// The commands page.
pub struct Gallery {
    pub state: Entity<DemoStateHolder>,
    pub registry: Registry,
    pub search: Option<(Entity<Search>, Subscription)>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Gallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_| DemoStateHolder(DemoState::default()));
        let registry = demo::registry(&state);
        // The search command only flags the wish; this view opens the search (it needs a window)
        let subs = vec![cx.observe_in(&state, window, |this, state, window, cx| {
            if state.read(cx).0.search_open {
                state.update(cx, |s, _| s.0.search_open = false);
                this.open_search(window, cx);
            }
            cx.notify();
        })];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self { state, registry, search: None, focus, _subs: subs }
    }

    /// Opens the search over the registry.
    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let registry = self.registry.clone();
        let search = cx.new(|cx| Search::new(registry, window, cx));
        let sub = cx.subscribe_in(&search, window, |this, _, e: &SearchEvent, window, cx| {
            this.search = None;
            window.focus(&this.focus, cx);
            if let SearchEvent::Run(id) = e {
                let r = this.registry.run(id, serde_json::Value::Null, window, cx);
                this.note_result(id, r, cx);
            }
            cx.notify();
        });
        self.search = Some((search, sub));
        cx.notify();
    }

    fn note_result(&mut self, id: &str, r: Result<serde_json::Value, String>, cx: &mut Context<Self>) {
        if let Err(e) = r {
            self.state.update(cx, |s, cx| {
                s.0.note(format!("{id}: {e}"));
                cx.notify();
            });
        }
    }

    fn menu_row(&self) -> impl IntoElement {
        h_flex()
            .flex_none()
            .w_full()
            .h(px(32.))
            .px(px(theme::XS))
            .gap(px(2.))
            .items_center()
            .bg(color::panel())
            .border_b_1()
            .border_color(color::border())
            .children(demo::menus().iter().enumerate().map(|(i, m)| self.registry.menu_button(format!("menu-{i}"), m)))
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let log = self.state.read(cx).0.log.clone();
        let lines = if log.is_empty() { vec!["Nothing ran yet. Try the menus, Ctrl+K, or Ctrl+O.".to_string()] } else { log };
        v_flex()
            .id("shell-gallery")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .bg(color::chrome())
            .text_color(color::text())
            .on_action(cx.listener(|this, a: &RunCommand, window, cx| {
                let r = this.registry.run(&a.id, serde_json::Value::Null, window, cx);
                this.note_result(&a.id, r, cx);
            }))
            .child(self.menu_row())
            .child(
                v_flex()
                    .flex_1()
                    .m(px(theme::PANE_GAP))
                    .p(px(theme::LG))
                    .gap(px(theme::SM))
                    .rounded(px(theme::RADIUS_LG))
                    .bg(color::panel())
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Commands"))
                    .child(div().text_sm().text_color(color::text_muted()).child("One registry: the menus above, the search (Ctrl+K) and the keys come from the same list."))
                    .children(lines.into_iter().map(|l| div().text_sm().font_family("monospace").child(l))),
            )
            .children(self.search.as_ref().map(|(s, _)| s.clone()))
    }
}
