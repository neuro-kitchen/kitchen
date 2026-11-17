//! The frame's People: the Settings section that adds, picks and removes people, and what the
//! title bar's person button lists ([`menu_items`]). See [`crate::people`].

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _, Window,
    div, px,
};
use kitchen_ui::theme::{self, color};
use kitchen_ui::widgets::{icon_button, row};
use serde_json::json;

use crate::command::MenuItem;
use crate::people::{self, Person};

/// The title bar's person menu: each person (the one working checked), a guest, then People….
pub fn menu_items() -> Vec<MenuItem> {
    let current = people::current().map(|p| p.name);
    let mut items: Vec<MenuItem> = people::people()
        .into_iter()
        .map(|p| MenuItem::Run {
            title: p.name.clone().into(),
            icon: Some(IconName::CircleUser),
            command: "people.use".into(),
            args: json!({ "name": p.name }),
            checked: current.as_deref() == Some(p.name.as_str()),
        })
        .collect();
    items.push(MenuItem::Run {
        title: "Guest (no name recorded)".into(),
        icon: Some(IconName::CircleUser),
        command: "people.use".into(),
        args: json!({ "name": null }),
        checked: current.is_none(),
    });
    items.push(MenuItem::Separator);
    items.push(MenuItem::command("people.manage"));
    items
}

/// The People section of Settings.
pub struct PeopleSettings {
    name: Entity<InputState>,
    affiliation: Entity<InputState>,
    orcid: Entity<InputState>,
    /// What the last change said (a refusal).
    error: Option<SharedString>,
}

impl PeopleSettings {
    /// The section, its form empty.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            name: cx.new(|cx| InputState::new(window, cx).placeholder("Name, as it is recorded")),
            affiliation: cx.new(|cx| InputState::new(window, cx).placeholder("Lab or institution (optional)")),
            orcid: cx.new(|cx| InputState::new(window, cx).placeholder("0000-0000-0000-0000 (optional)")),
            error: None,
        }
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = |i: &Entity<InputState>, cx: &Context<Self>| i.read(cx).value().to_string();
        let person = Person {
            name: value(&self.name, cx),
            affiliation: value(&self.affiliation, cx),
            orcid: value(&self.orcid, cx),
        };
        match people::add(person, true) {
            Ok(()) => {
                self.error = None;
                for input in [&self.name, &self.affiliation, &self.orcid] {
                    input.update(cx, |i, cx| i.set_value("", window, cx));
                }
            }
            Err(e) => self.error = Some(e.into()),
        }
        cx.notify();
    }

    fn change(&mut self, done: Result<(), String>, cx: &mut Context<Self>) {
        self.error = done.err().map(Into::into);
        cx.notify();
    }

    /// The section's body.
    pub fn body(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let current = people::current().map(|p| p.name);
        let everyone = people::people();
        let list = v_flex().gap(px(theme::XS)).children(everyone.into_iter().enumerate().map(|(i, p)| {
            let working = current.as_deref() == Some(p.name.as_str());
            let about = [p.affiliation.as_str(), p.orcid.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let (use_name, remove_name) = (p.name.clone(), p.name.clone());
            h_flex()
                .gap(px(theme::SM))
                .p(px(theme::XS))
                .rounded(px(theme::RADIUS))
                .when(working, |d| d.bg(color::control()))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().truncate().child(p.name.clone()))
                        .when(!about.is_empty(), |d| {
                            d.child(div().text_xs().text_color(color::text_muted()).truncate().child(about))
                        }),
                )
                .child(
                    Button::new(SharedString::from(format!("people-use-{i}")))
                        .small()
                        .ghost()
                        .label(if working { "Working" } else { "Work as" })
                        .selected(working)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.change(people::use_person(Some(&use_name)), cx)),
                        ),
                )
                .child(
                    icon_button(
                        SharedString::from(format!("people-remove-{i}")),
                        IconName::Delete,
                        format!("Forget {} (what is recorded under the name stays)", p.name),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.change(people::remove(&remove_name), cx))),
                )
        }));
        let guest = Button::new("people-guest")
            .small()
            .ghost()
            .label(if current.is_none() { "Working as a guest" } else { "Work as a guest" })
            .selected(current.is_none())
            .on_click(cx.listener(|this, _, _, cx| this.change(people::use_person(None), cx)));
        // Two groups, further apart than their rows: who there is, and adding someone
        let add = v_flex()
            .gap(px(theme::SM))
            .pt(px(theme::LG))
            .border_t_1()
            .border_color(color::border())
            .child(div().text_sm().font_weight(gpui_kit::FontWeight::SEMIBOLD).child("Add a person"))
            .child(row("Name", div().w_full().child(Input::new(&self.name).small())))
            .child(row("Affiliation", div().w_full().child(Input::new(&self.affiliation).small())))
            .child(row("ORCID iD", div().w_full().child(Input::new(&self.orcid).small())))
            .children(self.error.clone().map(|e| div().text_xs().text_color(color::danger()).child(e)))
            .child(
                h_flex().justify_end().child(
                    Button::new("people-add")
                        .small()
                        .primary()
                        .icon(IconName::Plus)
                        .label("Add and work as")
                        .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                ),
            );
        v_flex()
            .gap(px(theme::LG))
            .child(div().max_w(px(560.)).text_xs().text_color(color::text_muted()).child("What you do is recorded under the name of the person working: in the app's activity log and in files you save (a curation, a run's record), which may be shared. A guest is recorded as \u{201c}guest\u{201d}, with no name."))
            .child(v_flex().gap(px(theme::SM)).child(list).child(h_flex().child(guest)))
            .child(add)
            .into_any_element()
    }
}
