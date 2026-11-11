//! The slider.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Bounds, DispatchPhase, InteractiveElement as _, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels,
    SharedString, Styled as _, TestSupportExt as _, Window, canvas, div, px, relative,
};

use crate::theme::{self, color};

/// What a slider calls: the new value and whether the drag ended (to commit one undo step).
pub type OnSlide = Rc<dyn Fn(f32, bool, &mut Window, &mut App)>;

thread_local! {
    /// The slider being dragged: its id and its track (window px: left, width).
    static SLIDING: RefCell<Option<(String, f32, f32)>> = const { RefCell::new(None) };
}

/// Height of the track's pointer target (px): [`theme::TARGET_MIN`], though the bar drawn is 4.
const TRACK_H: f32 = theme::TARGET_MIN;
/// Diameter of the thumb (px).
const THUMB: f32 = 12.;

/// A horizontal slider over `min..=max` with the value printed beside it (`fmt`). A click sets the
/// value; a drag follows the pointer (even off the track) and ends on release, when `on_change`
/// gets `true`. It keeps no state: the caller holds the value, and `id` (owned, unique in the
/// window) names the drag in progress. The track's element id is `{id}-track`.
pub fn slider(id: impl Into<String>, value: f32, (min, max): (f32, f32), fmt: impl Fn(f32) -> String, on_change: impl Fn(f32, bool, &mut Window, &mut App) + 'static) -> impl IntoElement {
    let on_change: OnSlide = Rc::new(on_change);
    let id: String = id.into();
    let t = ((value - min) / (max - min).max(1e-12)).clamp(0.0, 1.0);
    let active = SLIDING.with(|s| s.borrow().as_ref().is_some_and(|(k, _, _)| *k == id));
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let to_value = move |x: f32, left: f32, width: f32| min + ((x - left) / width.max(1.0)).clamp(0.0, 1.0) * (max - min);
    // While dragging, the window's pointer events drive it
    let (b1, id1, ch1) = (bounds.clone(), id.clone(), on_change.clone());
    let listener = canvas(
        move |b, _, _| b1.set(b),
        move |_, _, window, _| {
            if !active {
                return;
            }
            let (id, ch) = (id1.clone(), ch1.clone());
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                let Some((_, left, width)) = SLIDING.with(|s| s.borrow().clone()).filter(|(k, _, _)| *k == id) else { return };
                if e.pressed_button != Some(MouseButton::Left) {
                    SLIDING.with(|s| *s.borrow_mut() = None);
                    ch(to_value(e.position.x.as_f32(), left, width), true, window, cx);
                    window.refresh();
                    return;
                }
                ch(to_value(e.position.x.as_f32(), left, width), false, window, cx);
            });
            let (id, ch) = (id1.clone(), ch1.clone());
            window.on_mouse_event(move |e: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if let Some((_, left, width)) = SLIDING.with(|s| s.borrow_mut().take()).filter(|(k, _, _)| *k == id) {
                    ch(to_value(e.position.x.as_f32(), left, width), true, window, cx);
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .size_full();
    let (b2, id2, ch2) = (bounds, id.clone(), on_change);
    let bar_top = (TRACK_H - 4.) / 2.;
    let track = div()
        .id(SharedString::from(format!("{id}-track")))
        .relative()
        .flex_1()
        .min_w(px(60.))
        .h(px(TRACK_H))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |e, window, cx| {
            cx.stop_propagation();
            let b = b2.get();
            let (left, width) = (b.origin.x.as_f32(), b.size.width.as_f32());
            SLIDING.with(|s| *s.borrow_mut() = Some((id2.clone(), left, width)));
            ch2(to_value(e.position.x.as_f32(), left, width), false, window, cx);
            window.refresh();
        })
        // The track, its filled part (a mark: ≥ 3:1 on the panel), the thumb
        .child(div().absolute().left_0().right_0().top(px(bar_top)).h(px(4.)).rounded_full().bg(color::control()))
        .child(div().absolute().left_0().top(px(bar_top)).h(px(4.)).w(relative(t)).rounded_full().bg(color::accent_mark()))
        .child(
            div()
                .absolute()
                .top(px((TRACK_H - THUMB) / 2.))
                .left(relative(t))
                .ml(px(-THUMB / 2.))
                .size(px(THUMB))
                .rounded_full()
                .bg(gpui_kit::white())
                .border_color(color::accent_mark())
                .map(|d| if active { d.border_2() } else { d.border_1() }),
        )
        .child(listener)
        .test_support();
    h_flex()
        .w_full()
        .min_w_0()
        .gap(px(theme::SM))
        .child(track)
        .child(div().flex_none().w(px(44.)).text_right().text_xs().font_family("monospace").child(fmt(value)))
}
