//! One container for every workspace's views (lumen's planes, tables and plots; dsp-kitchen's
//! traces and rasters): a dock whose views are tabs dragged into splits, the "Add view or
//! container" dialog (after Rerun's: arrange every view, or add one), and the buttons in a view's
//! tab. A workspace only lists its view kinds ([`ViewOption`]) with their flags: whether each can
//! be added now (and why not), which one a view shows.

use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::base::dock::{DockArea, DockLayout, DockPlacement, InsertTarget, PanelId};
use gpui_kit::base::Placement;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dock::{panel_handle, DockSkin, Panel};
use gpui_kit::component::{h_flex, v_flex, Disableable as _, Icon, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Context, Entity, EntityId, EventEmitter, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window,
};

use kitchen_ui::theme::{self, color};
use serde::{Deserialize, Serialize};

/// How the views are laid out together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrangement {
    /// Every view as a tab of one group.
    Tabs,
    /// Every view side by side.
    Horizontal,
    /// Every view stacked.
    Vertical,
    /// Every view in a grid (as square as the count allows).
    Grid,
}

impl Arrangement {
    /// Every arrangement, in the dialog's order.
    pub const ALL: [Arrangement; 4] = [Arrangement::Tabs, Arrangement::Horizontal, Arrangement::Vertical, Arrangement::Grid];

    /// Its name ("Side by side").
    pub fn title(self) -> &'static str {
        match self {
            Arrangement::Tabs => "Tabs",
            Arrangement::Horizontal => "Side by side",
            Arrangement::Vertical => "Stacked",
            Arrangement::Grid => "Grid",
        }
    }
}

/// A kind of view a container offers.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewOption {
    /// Its name ("Plane", "Table").
    pub title: String,
    /// Its icon (the tab button, the dialog's tile).
    pub icon: IconName,
    /// What it shows (the dialog's line, the tab button's tooltip).
    pub hint: String,
    /// Whether it can be added now; when not, `hint` says why.
    pub enabled: bool,
}

impl ViewOption {
    /// A kind of view titled `title` showing what `hint` says.
    pub fn new(title: impl Into<String>, icon: IconName, hint: impl Into<String>) -> Self {
        Self { title: title.into(), icon, hint: hint.into(), enabled: true }
    }

    /// Not available now, for `why`.
    pub fn unavailable(mut self, why: impl Into<String>) -> Self {
        self.enabled = false;
        self.hint = why.into();
        self
    }
}

/// A dock for views, with the look every workspace shares (no dock toggle buttons).
pub fn new_dock(id: &'static str, window: &mut Window, cx: &mut App) -> Entity<DockArea> {
    // The skin hides the toggles once the dock exists (it updates the dock)
    let mut skin = None;
    let dock = cx.new(|cx| {
        let s = DockSkin::new(cx);
        skin = Some(s.clone());
        DockArea::new(id, Some(2), window, cx).with_renderer(s)
    });
    if let Some(skin) = skin {
        skin.set_toggle_button_visible(false, cx);
    }
    dock
}

/// Adds `panel` to the dock's centre, split off `beside` (another view's entity) at `placement`
/// when it is there.
pub fn add_beside<P: Panel>(dock: &Entity<DockArea>, panel: Entity<P>, beside: Option<(EntityId, Placement)>, window: &mut Window, cx: &mut App) {
    let id = PanelId::from(panel.entity_id());
    dock.update(cx, |d, cx| {
        let target = beside.and_then(|(e, at)| Some((d.layout(DockPlacement::Center)?.find_panel_node(PanelId::from(e))?, at)));
        d.add_panel_view(panel_handle(panel), DockPlacement::Center, None, window, cx);
        if let Some((node, placement)) = target {
            d.move_panel(id, InsertTarget::Split { node, placement, size: None }, window, cx);
        }
    });
}

/// Lays out `panels` in `dock`'s centre as `how`.
pub fn arrange<P: Panel>(dock: &Entity<DockArea>, panels: &[Entity<P>], how: Arrangement, window: &mut Window, cx: &mut App) {
    let layout = arranged(panels, how, cx);
    dock.update(cx, |d, cx| d.set_center(layout, window, cx));
}

/// The views laid out as `how`, in order.
pub fn arranged<P: Panel>(panels: &[Entity<P>], how: Arrangement, cx: &App) -> DockLayout {
    let group = |p: &Entity<P>, cx: &App| DockLayout::tabs().panel_view(panel_handle(p.clone()), cx);
    match how {
        Arrangement::Tabs => panels.iter().fold(DockLayout::tabs(), |l, p| l.panel_view(panel_handle(p.clone()), cx)),
        Arrangement::Horizontal => panels.iter().fold(DockLayout::h_split(), |l, p| l.child(group(p, cx), None)),
        Arrangement::Vertical => panels.iter().fold(DockLayout::v_split(), |l, p| l.child(group(p, cx), None)),
        Arrangement::Grid => {
            let cols = (panels.len() as f64).sqrt().ceil().max(1.0) as usize;
            panels.chunks(cols).fold(DockLayout::v_split(), |l, row| l.child(row.iter().fold(DockLayout::h_split(), |r, p| r.child(group(p, cx), None)), None))
        }
    }
}

type Pick = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The buttons in a view's tab: one per kind it can show (`current` selected; a click shows that
/// kind in this view), then "+" (add a view, or arrange them). Ids: `{key}-kind-{i}`,
/// `{key}-add`. gpui-kit's small size (about 24 px: designparser `touch-target`).
pub fn tab_buttons(key: &str, options: &[ViewOption], current: Option<usize>, show: impl Fn(usize, &mut Window, &mut App) + 'static, add: impl Fn(&mut Window, &mut App) + 'static) -> Vec<Button> {
    let show: Pick = Rc::new(show);
    let mut out: Vec<Button> = options
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let show = show.clone();
            Button::new(SharedString::from(format!("{key}-kind-{i}")))
                .ghost()
                .small()
                .icon(Icon::new(o.icon))
                .tooltip(format!("{}: {}", o.title, o.hint))
                .selected(current == Some(i))
                .disabled(!o.enabled && current != Some(i))
                .on_click(move |_, window, cx| show(i, window, cx))
        })
        .collect();
    out.push(Button::new(SharedString::from(format!("{key}-add"))).ghost().small().icon(Icon::new(IconName::Plus)).tooltip("Add a view, or arrange the views").on_click(move |_, window, cx| add(window, cx)));
    out
}

/// What the dialog asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogEvent {
    /// Arrange every view so.
    Arrange(Arrangement),
    /// Add the view of option `i`.
    Add(usize),
    /// Close it.
    Close,
}

/// "Add view or container": the arrangements, then the workspace's views; ↑ ↓ move between the
/// rows that can run, Enter runs one, Esc or a click outside closes.
pub struct AddViewDialog {
    options: Vec<ViewOption>,
    /// Arranging needs views to arrange.
    can_arrange: bool,
    cursor: usize,
    focus: FocusHandle,
}

impl EventEmitter<DialogEvent> for AddViewDialog {}

/// The arrangements the dialog offers: icon and line.
const ARRANGEMENTS: [(Arrangement, IconName, &str); 4] = [
    (Arrangement::Tabs, IconName::AppWindow, "Every view as a tab of one group"),
    (Arrangement::Horizontal, IconName::Columns2, "Every view side by side"),
    (Arrangement::Vertical, IconName::Rows2, "Every view stacked"),
    (Arrangement::Grid, IconName::Grid2x2, "Every view in a grid"),
];

impl AddViewDialog {
    /// The dialog offering `options` (and the arrangements, when `can_arrange`); focuses it.
    pub fn new(options: Vec<ViewOption>, can_arrange: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut d = Self { options, can_arrange, cursor: 0, focus };
        d.cursor = d.enabled().first().copied().unwrap_or(0);
        d
    }

    /// The rows that can run (arrangements first, then the views).
    fn enabled(&self) -> Vec<usize> {
        let arr = (0..ARRANGEMENTS.len()).filter(|_| self.can_arrange);
        arr.chain(self.options.iter().enumerate().filter(|(_, o)| o.enabled).map(|(i, _)| ARRANGEMENTS.len() + i)).collect()
    }

    /// Runs row `row` (arrangements first, then the views), as a click does.
    pub fn run(&mut self, row: usize, cx: &mut Context<Self>) {
        cx.emit(if row < ARRANGEMENTS.len() { DialogEvent::Arrange(ARRANGEMENTS[row].0) } else { DialogEvent::Add(row - ARRANGEMENTS.len()) });
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let enabled = self.enabled();
        let at = enabled.iter().position(|&i| i == self.cursor);
        match e.keystroke.key.as_str() {
            "escape" => cx.emit(DialogEvent::Close),
            "down" | "up" if !enabled.is_empty() => {
                let n = enabled.len();
                let next = match (at, e.keystroke.key.as_str()) {
                    (Some(i), "down") => (i + 1) % n,
                    (Some(i), _) => (i + n - 1) % n,
                    (None, "down") => 0,
                    (None, _) => n - 1,
                };
                self.cursor = enabled[next];
                cx.notify();
            }
            "enter" => {
                if enabled.contains(&self.cursor) {
                    self.run(self.cursor, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for AddViewDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity().downgrade();
        let cursor = self.cursor;
        let row = |ix: usize, icon: IconName, title: String, text: String, enabled: bool| {
            let me = me.clone();
            h_flex()
                .id(SharedString::from(format!("add-view-row-{ix}")))
                .min_h(px(48.))
                .px(px(theme::SM))
                .gap(px(theme::MD))
                .rounded(px(theme::RADIUS))
                .when(enabled && ix == cursor, |d| d.bg(color::control()))
                .when(enabled, |d| {
                    d.cursor_pointer().hover(|d| d.bg(color::control())).on_click(move |_, _, cx| {
                        let _ = me.update(cx, |d, cx| d.run(ix, cx));
                    })
                })
                .when(!enabled, |d| d.opacity(0.5))
                .child(div().flex_none().size(px(32.)).rounded(px(theme::RADIUS)).border_1().border_color(color::hover()).flex().items_center().justify_center().child(Icon::new(icon)))
                .child(v_flex().min_w_0().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title)).child(div().text_xs().text_color(color::text_muted()).truncate().child(text)))
        };
        let heading = |text: &'static str, hint: &'static str| v_flex().px(px(theme::SM)).pt(px(theme::MD)).pb(px(theme::XS)).child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(text)).child(div().text_xs().text_color(color::text_muted()).child(hint));
        let close = me.clone();
        let n = ARRANGEMENTS.len();
        let card = v_flex()
            .id("add-view-dialog")
            .occlude()
            .w(px(460.))
            .max_h(px(620.))
            .overflow_y_scroll()
            .p(px(theme::SM))
            .rounded(px(theme::RADIUS_LG))
            .border_1()
            .border_color(color::border())
            .bg(color::raised())
            .shadow_lg()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .px(px(theme::SM))
                    .py(px(theme::XS))
                    .justify_between()
                    .child(h_flex().gap(px(theme::SM)).items_baseline().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Add view or container")).child(div().text_xs().text_color(color::text_muted()).child("↑ ↓ Enter · Esc")))
                    .child(Button::new("add-view-close").ghost().small().icon(Icon::new(IconName::X)).tooltip("Close").on_click(move |_, _, cx| {
                        let _ = close.update(cx, |_, cx| cx.emit(DialogEvent::Close));
                    })),
            )
            .child(heading("Containers", "Arrange the views"))
            .children(ARRANGEMENTS.iter().enumerate().map(|(i, (how, icon, text))| row(i, *icon, how.title().to_string(), text.to_string(), self.can_arrange)))
            .child(div().mx(px(theme::SM)).my(px(theme::SM)).h(px(1.)).bg(color::border()))
            .child(heading("Views", "Added beside the focused view"))
            .children(self.options.iter().enumerate().map(|(i, o)| row(n + i, o.icon, o.title.clone(), o.hint.clone(), o.enabled)));
        let back = me.clone();
        // The backdrop over the views closes it
        div()
            .id("add-view-backdrop")
            .absolute()
            .inset_0()
            .bg(gpui_kit::black().opacity(0.35))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(48.))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = back.update(cx, |_, cx| cx.emit(DialogEvent::Close));
            })
            .child(card)
    }
}
