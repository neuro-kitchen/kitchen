//! The stepper and the dragged number (scrub label).

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{IconName, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, CursorStyle, DispatchPhase, InteractiveElement as _, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _,
    SharedString, Styled as _, Window, canvas, div, px,
};

use crate::theme::{self, color};

/// What a stepped number calls with its step (−1 or +1; a fast drag calls it once per step).
pub type OnStep = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// Pointer px per step of a dragged number.
pub const SCRUB_PX: f32 = 6.0;

thread_local! {
    /// The number being dragged: its id and the pointer x of the last step.
    static SCRUBBING: RefCell<Option<(String, f32)>> = const { RefCell::new(None) };
}

/// `− value +`: a number changed in steps, by its buttons or by dragging the number (see
/// [`scrub_label`]). The caller holds the value and formats it. Element ids: `{id}-minus`,
/// `{id}-value`, `{id}-plus`. The buttons are gpui-kit's small size, [`theme::XS`] apart
/// (designparser `touch-target`: about 24 px each).
pub fn stepper(id: impl Into<String>, value: impl Into<SharedString>, on_step: impl Fn(f32, &mut Window, &mut App) + 'static) -> impl IntoElement {
    let id: String = id.into();
    let on_step: OnStep = Rc::new(on_step);
    let (a, b, c) = (on_step.clone(), on_step.clone(), on_step);
    h_flex()
        .gap(px(theme::XS))
        .child(Button::new(SharedString::from(format!("{id}-minus"))).ghost().small().icon(IconName::Minus).tooltip("Decrease").on_click(move |_, w, cx| a(-1.0, w, cx)))
        .child(scrub(format!("{id}-value"), value.into(), 52.0, c))
        .child(Button::new(SharedString::from(format!("{id}-plus"))).ghost().small().icon(IconName::Plus).tooltip("Increase").on_click(move |_, w, cx| b(1.0, w, cx)))
}

/// A number that changes as it is dragged (Affinity, Blender): press it and move left or right;
/// every [`SCRUB_PX`] px is one step down or up. The pointer may leave it while dragging. The
/// caller holds the value and formats it; `id` (owned, unique in the window) names the drag.
pub fn scrub_label(id: impl Into<String>, value: impl Into<SharedString>, on_step: impl Fn(f32, &mut Window, &mut App) + 'static) -> impl IntoElement {
    scrub(id.into(), value.into(), 0.0, Rc::new(on_step))
}

fn scrub(id: String, value: SharedString, min_w: f32, on_step: OnStep) -> impl IntoElement {
    let active = SCRUBBING.with(|s| s.borrow().as_ref().is_some_and(|(k, _)| *k == id));
    let down_id = id.clone();
    // While dragging, the window's pointer events drive it (the pointer leaves the number)
    let listener = canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            if !active {
                return;
            }
            let (id, step) = (id.clone(), on_step.clone());
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if e.pressed_button != Some(MouseButton::Left) {
                    SCRUBBING.with(|s| *s.borrow_mut() = None);
                    window.refresh();
                    return;
                }
                let x = e.position.x.as_f32();
                let steps = SCRUBBING.with(|s| match &mut *s.borrow_mut() {
                    Some((k, last)) if *k == id => {
                        let n = ((x - *last) / SCRUB_PX).trunc();
                        *last += n * SCRUB_PX;
                        n as i32
                    }
                    _ => 0,
                });
                for _ in 0..steps.unsigned_abs() {
                    step(steps.signum() as f32, window, cx);
                }
            });
            window.on_mouse_event(|_: &MouseUpEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble {
                    SCRUBBING.with(|s| *s.borrow_mut() = None);
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .size_full();
    div()
        .id(SharedString::from(down_id.clone()))
        .relative()
        .min_w(px(min_w))
        .min_h(px(theme::TARGET_MIN))
        .flex()
        .items_center()
        .justify_center()
        .px(px(theme::XS))
        .rounded(px(theme::RADIUS_SM))
        .text_sm()
        .font_family("monospace")
        .cursor(CursorStyle::ResizeLeftRight)
        .when(active, |d| d.bg(color::accent().opacity(0.35)))
        .when(!active, |d| d.hover(|d| d.bg(color::hover())))
        .on_mouse_down(MouseButton::Left, move |e, window, cx| {
            cx.stop_propagation();
            SCRUBBING.with(|s| *s.borrow_mut() = Some((down_id.clone(), e.position.x.as_f32())));
            window.refresh();
        })
        .child(value)
        .child(listener)
}
