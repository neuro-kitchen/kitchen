//! The dropdown of fixed options.

use std::rc::Rc;

use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex};
use gpui_kit::{App, ElementId, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px};

type OnPick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// Tallest the list of options gets before it scrolls (px).
const MENU_MAX_H: f32 = 360.;

/// A button showing the current option; pressing it lists the options, the current one checked.
/// It fills its place, so a long option ends in "…" instead of being cut or pushing the panel
/// wider.
#[derive(IntoElement)]
pub struct MenuSelect {
    id: ElementId,
    options: Vec<SharedString>,
    selected: Option<usize>,
    placeholder: SharedString,
    on_pick: OnPick,
}

impl MenuSelect {
    /// A dropdown of `options` with `selected` current; `on_pick` gets the index picked.
    pub fn new(id: impl Into<ElementId>, options: Vec<SharedString>, selected: Option<usize>, on_pick: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        Self { id: id.into(), options, selected, placeholder: "Choose…".into(), on_pick: Rc::new(on_pick) }
    }

    /// What the button shows while nothing is selected ("Choose…" by default).
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }
}

impl RenderOnce for MenuSelect {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { id, options, selected, placeholder, on_pick } = self;
        let current = selected.and_then(|i| options.get(i).cloned()).unwrap_or(placeholder);
        let face = h_flex().w_full().min_w_0().gap_1().child(div().flex_1().min_w_0().truncate().text_left().child(current)).child(Icon::new(IconName::ChevronDown).xsmall());
        Button::new(id).outline().small().min_w_0().w_full().child(face).cursor_pointer().dropdown_menu(move |menu, _, _| {
            let mut menu = menu.scrollable(true).max_h(px(MENU_MAX_H));
            for (i, label) in options.iter().enumerate() {
                let on_pick = on_pick.clone();
                menu = menu.item(PopupMenuItem::new(label.clone()).checked(selected == Some(i)).on_click(move |_, window, cx| on_pick(i, window, cx)));
            }
            menu
        })
    }
}
