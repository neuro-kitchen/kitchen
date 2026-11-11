//! The gallery's views page: a dock of demo views in the shared container. Each view's tab has
//! its kind buttons and "+"; "+" opens "Add view or container" (arrange every view, or add one).

#![allow(dead_code)]

use gpui_kit::assets::IconName;
use gpui_kit::base::Placement;
use gpui_kit::base::dock::{DockArea, Panel as BasePanel, PanelEvent};
use gpui_kit::component::button::Button;
use gpui_kit::component::dock::{Panel, PanelControl};
use gpui_kit::component::{Icon, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, WeakEntity, Window, div, px,
};
use kitchen_shell::container::{self, AddViewDialog, Arrangement, DialogEvent, ViewOption};
use kitchen_ui::theme::{self, color};

/// The kinds of view the demo offers (the last is unavailable, to show why).
pub fn options() -> Vec<ViewOption> {
    vec![
        ViewOption::new("Plane", IconName::Image, "One plane of the image"),
        ViewOption::new("Table", IconName::Table, "The measurements, one row per object"),
        ViewOption::new("Plot", IconName::ChartScatter, "Two measurements against each other").unavailable("Needs a table with two numeric columns"),
    ]
}

/// One view in the dock.
pub struct DemoPanel {
    pub n: usize,
    pub kind: usize,
    page: WeakEntity<ViewsPage>,
    focus: FocusHandle,
}

impl DemoPanel {
    fn new(n: usize, kind: usize, page: WeakEntity<ViewsPage>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| DemoPanel { n, kind, page, focus: cx.focus_handle() })
    }
}

impl Render for DemoPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let o = &options()[self.kind];
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(theme::SM))
            .bg(color::panel())
            .child(Icon::new(o.icon).size(px(32.)).text_color(color::text_muted()))
            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(format!("{} {}", o.title, self.n)))
            .child(div().text_sm().text_color(color::text_muted()).child(o.hint.clone()))
    }
}

impl EventEmitter<PanelEvent> for DemoPanel {}

impl Focusable for DemoPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl BasePanel for DemoPanel {
    fn panel_name(&self) -> &'static str {
        "kitchen-gallery.view"
    }
}

impl Panel for DemoPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        SharedString::from(format!("{} {}", options()[self.kind].title, self.n))
    }

    fn zoom_control(&self, _: &App) -> Option<PanelControl> {
        Some(PanelControl::Toolbar)
    }

    fn toolbar_buttons(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Option<Vec<Button>> {
        let (me, page) = (cx.entity().downgrade(), self.page.clone());
        Some(container::tab_buttons(
            &format!("view-{}", self.n),
            &options(),
            Some(self.kind),
            move |i, _, cx| {
                let _ = me.update(cx, |p, cx| {
                    p.kind = i;
                    cx.notify();
                });
            },
            move |window, cx| {
                let _ = page.update(cx, |p, cx| p.open_dialog(window, cx));
            },
        ))
    }
}

/// The views page.
pub struct ViewsPage {
    pub dock: Entity<DockArea>,
    pub panels: Vec<Entity<DemoPanel>>,
    pub dialog: Option<(Entity<AddViewDialog>, Subscription)>,
    pub arrangement: Option<Arrangement>,
    next: usize,
}

impl ViewsPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock = container::new_dock("gallery-views", window, cx);
        let mut page = ViewsPage { dock, panels: Vec::new(), dialog: None, arrangement: None, next: 1 };
        page.add(0, window, cx);
        page.add(1, window, cx);
        page
    }

    /// Adds a view of kind `kind` beside the last one.
    pub fn add(&mut self, kind: usize, window: &mut Window, cx: &mut Context<Self>) {
        let panel = DemoPanel::new(self.next, kind, cx.entity().downgrade(), cx);
        self.next += 1;
        let beside = self.panels.last().map(|p| (p.entity_id(), Placement::Right));
        container::add_beside(&self.dock, panel.clone(), beside, window, cx);
        self.panels.push(panel);
        cx.notify();
    }

    /// Opens "Add view or container".
    pub fn open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let can_arrange = self.panels.len() > 1;
        let dialog = cx.new(|cx| AddViewDialog::new(options(), can_arrange, window, cx));
        let sub = cx.subscribe_in(&dialog, window, |this, _, e: &DialogEvent, window, cx| {
            this.dialog = None;
            match *e {
                DialogEvent::Add(i) => this.add(i, window, cx),
                DialogEvent::Arrange(how) => {
                    container::arrange(&this.dock, &this.panels, how, window, cx);
                    this.arrangement = Some(how);
                }
                DialogEvent::Close => {}
            }
            cx.notify();
        });
        self.dialog = Some((dialog, sub));
        cx.notify();
    }
}

impl Render for ViewsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().relative().child(self.dock.clone()).children(self.dialog.as_ref().map(|(d, _)| d.clone()))
    }
}
