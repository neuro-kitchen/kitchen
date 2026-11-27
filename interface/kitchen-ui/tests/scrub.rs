//! Headless UI test of the scrub field: dragged, it steps; pressed and let go without moving, it
//! asks to be typed.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Point, Render, Styled as _, TestAppContext, Window, WindowBounds,
    WindowOptions, div, point, px, size,
};
use kitchen_ui::theme::{self, Look, Palette};
use kitchen_ui::widgets::scrub_field;

/// One scrub field; counts its steps and its clicks.
struct Field {
    steps: Rc<Cell<i32>>,
    clicks: Rc<Cell<u32>>,
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (steps, clicks) = (self.steps.clone(), self.clicks.clone());
        div().size_full().p_8().child(scrub_field(
            "cutoff",
            format!("{} Hz", 300 + self.steps.get() * 10),
            move |d, _, _| steps.set(steps.get() + d as i32),
            move |_, _| clicks.set(clicks.get() + 1),
        ))
    }
}

fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_scrub_field_steps_when_dragged_and_asks_to_be_typed_when_clicked(cx: &mut TestAppContext) {
    let (steps, clicks) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (s, c) = (steps.clone(), clicks.clone());
    let (window, _field) = cx.update(|cx| {
            gpui_kit::init(cx);
            theme::use_look(Look::new(Palette::LIME), cx);
            let bounds = Bounds { origin: Point::default(), size: size(px(400.), px(200.)) };
            let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
            gpui_kit::open_window(options, cx, move |_, cx| cx.new(|_| Field { steps: s, clicks: c })).expect("open test window")
        });
    let window: AnyWindowHandle = window.into();
    frame(cx, window);

    // Pressed and let go where it was pressed: typed, not stepped
    cx.update_window(window, |_, window, cx| window.click("cutoff", cx)).unwrap();
    frame(cx, window);
    assert_eq!((steps.get(), clicks.get()), (0, 1), "a click asks to type");

    // Dragged to the right: steps up, and no click
    let centre = cx.update_window(window, |_, window, _| window.find("cutoff").bounds().center()).unwrap();
    cx.update_window(window, |_, window, cx| window.drag(centre, point(centre.x + px(60.), centre.y), cx)).unwrap();
    frame(cx, window);
    assert!(steps.get() > 0, "a drag to the right steps up: {}", steps.get());
    assert_eq!(clicks.get(), 1, "a drag is not a click");
}
