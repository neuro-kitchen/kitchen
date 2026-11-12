//! Headless UI tests: the gallery (every widget) drawn with each app palette and a few custom
//! accents, dark and light; the accent picker; a slider that follows a click; the code, diff and
//! chat pages in both themes, a hunk accepted, a prompt sent from the chat pane.

#[path = "../examples/gallery/view.rs"]
mod view;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, point, px, size};
use kitchen_ui::chat::ChatMessage;
use kitchen_ui::theme::{self, Palette, Rgb};
use kitchen_ui::widgets::HunkDecision;

/// Draws a frame and lets the UI settle.
fn frame(cx: &mut TestAppContext, window: AnyWindowHandle) {
    cx.update_window(window, |_, window, cx| window.render_frame(cx)).unwrap();
    cx.run_until_parked();
}

/// Opens the gallery in a test window, in the light theme.
fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, gpui_kit::Entity<view::Gallery>) {
    let (window, gallery) = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::use_look(view::initial_look(false), cx);
        let bounds = Bounds { origin: Point::default(), size: size(px(1440.), px(860.)) };
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| view::Gallery::new(window, cx))).expect("open test window")
    });
    frame(cx, window);
    (window.into(), gallery)
}

#[gpui_kit::test]
fn gallery_draws_in_every_look_and_the_slider_follows_a_click(cx: &mut TestAppContext) {
    let (window, gallery) = open(cx);
    let accents = [None, Some(Rgb(0xfffacd)), Some(Rgb(0x0a1030)), Some(Rgb(0xff0000))];
    for p in Palette::APPS {
        for accent in accents {
            for dark in [true, false] {
                cx.update(|cx| theme::use_look(theme::look().with_palette(p).with_accent(accent).with_dark(dark), cx));
                frame(cx, window);
            }
        }
    }

    // Picking a suggested accent applies it; "App" goes back to the app's palette
    cx.update_window(window, |_, window, cx| window.click("accent-2b7f86", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|_| theme::look().accent), Some(Rgb(0x2b7f86)));
    cx.update_window(window, |_, window, cx| window.click("accent-app", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|_| theme::look().accent), None);

    // A click at three quarters of the track sets the value to about 0.75
    cx.update_window(window, |_, window, cx| {
        let b = window.find("gallery-level-track").bounds();
        window.click_at("gallery-level-track", point(b.size.width * 0.75, b.size.height / 2.), cx);
    })
    .unwrap();
    frame(cx, window);
    let level = cx.update(|cx| gallery.read(cx).level);
    assert!((level - 0.75).abs() < 0.03, "slider value after the click: {level}");
}

#[gpui_kit::test]
fn code_diff_and_chat_pages_draw_in_both_themes(cx: &mut TestAppContext) {
    let (window, gallery) = open(cx);
    for page in view::Page::ALL {
        cx.update_window(window, |_, window, cx| window.click(page_id(page), cx)).unwrap();
        frame(cx, window);
        assert_eq!(cx.update(|cx| gallery.read(cx).page), page);
        for dark in [true, false, true] {
            cx.update(|cx| theme::use_look(theme::look().with_dark(dark), cx));
            frame(cx, window);
        }
    }

    // The code page: the error is listed; the session diff has hunks to review
    cx.update_window(window, |_, window, cx| window.click("page-code", cx)).unwrap();
    frame(cx, window);
    let errors = cx.update(|cx| gallery.read(cx).code.read(cx).error_list().to_vec());
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].0, 6);
    let hunks = cx.update(|cx| gallery.read(cx).session_diff.read(cx).diff().hunks.len());
    assert!(hunks >= 2, "the session diff has {hunks} hunks");
    let summary = cx.update(|cx| gallery.read(cx).session_diff.read(cx).diff().lines.iter().any(|l| l.text.trim() == "cells: 2,539 → 2,712"));
    assert!(summary, "the cell list is one summary line");
    cx.update_window(window, |_, window, cx| window.click("session-diff-accept-0", cx)).unwrap();
    frame(cx, window);
    cx.update_window(window, |_, window, cx| window.click("session-diff-reject-1", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| gallery.read(cx).decisions.clone()), vec![HunkDecision { hunk: 0, accept: true }, HunkDecision { hunk: 1, accept: false }]);
    // A folded run of unchanged lines opens
    cx.update_window(window, |_, window, cx| window.click("session-diff-gap-0", cx)).unwrap();
    frame(cx, window);

    // The chat page: the Send button sends the prompt
    cx.update_window(window, |_, window, cx| window.click("page-chat", cx)).unwrap();
    frame(cx, window);
    let chat = cx.update(|cx| gallery.read(cx).chat.clone());
    let before = cx.update(|cx| chat.read(cx).messages().len());
    cx.update_window(window, |_, window, cx| chat.update(cx, |c, cx| c.set_prompt("Count the cells by class", window, cx))).unwrap();
    frame(cx, window);
    cx.update_window(window, |_, window, cx| window.click("chat-send", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| gallery.read(cx).chat_sent.clone()), vec!["Count the cells by class".to_string()]);
    assert_eq!(cx.update(|cx| chat.read(cx).messages().len()), before + 1);
    assert_eq!(cx.update(|cx| chat.read(cx).messages().last().cloned()), Some(ChatMessage::User { text: "Count the cells by class".into() }));
    assert!(cx.update(|cx| chat.read(cx).prompt_text(cx).is_empty()), "the prompt is cleared");
    assert!(cx.update(|cx| chat.read(cx).is_running()));

    // While the agent runs, the button is Stop
    cx.update_window(window, |_, window, cx| window.click("chat-stop", cx)).unwrap();
    frame(cx, window);
    assert_eq!(cx.update(|cx| gallery.read(cx).chat_stops), 1);
    assert!(!cx.update(|cx| chat.read(cx).is_running()));

    // Enter sends; Shift+Enter is a newline
    cx.update_window(window, |_, window, cx| chat.update(cx, |c, cx| c.focus_prompt(window, cx))).unwrap();
    frame(cx, window);
    cx.simulate_input(window, "Now the tumor");
    cx.simulate_keystrokes(window, "shift-enter");
    cx.simulate_input(window, "cells only");
    frame(cx, window);
    assert_eq!(cx.update(|cx| chat.read(cx).prompt_text(cx)), "Now the tumor\ncells only");
    assert_eq!(cx.update(|cx| gallery.read(cx).chat_sent.len()), 1, "Shift+Enter does not send");
    cx.simulate_keystrokes(window, "enter");
    frame(cx, window);
    assert_eq!(cx.update(|cx| gallery.read(cx).chat_sent.last().cloned()), Some("Now the tumor\ncells only".to_string()));

    // A tool call opens to its arguments and result
    cx.update(|cx| chat.update(cx, |c, cx| c.toggle_tool(2, cx)));
    frame(cx, window);
}

fn page_id(page: view::Page) -> &'static str {
    match page {
        view::Page::Widgets => "page-widgets",
        view::Page::Code => "page-code",
        view::Page::Chat => "page-chat",
        view::Page::Timeline => "page-timeline",
    }
}

/// The timeline page: the morning's actions summarised to a few episodes, every action on demand,
/// a click selects an episode and shows its context cards, a second click clears it.
#[gpui_kit::test]
fn the_timeline_summarises_and_shows_where_things_came_from(cx: &mut TestAppContext) {
    let (window, gallery) = open(cx);
    cx.update_window(window, |_, window, cx| window.click("page-timeline", cx)).unwrap();
    frame(cx, window);
    let timeline = cx.update(|cx| gallery.read(cx).timeline.clone());
    let (episodes, actions) = cx.update(|cx| (timeline.read(cx).shown().len(), timeline.read(cx).activities().len()));
    assert!(actions > 40, "{actions} actions");
    assert!(episodes < actions / 3, "{episodes} episodes from {actions} actions");
    // A click on a bar selects its episode; its actions keep their provenance
    cx.update_window(window, |_, window, cx| window.click("timeline-bar-1", cx)).unwrap();
    frame(cx, window);
    let picked = cx.update(|cx| timeline.read(cx).selected().cloned()).expect("an episode is selected");
    assert!(!picked.items.is_empty());
    assert_eq!(cx.update(|cx| gallery.read(cx).timeline_picks.clone()), vec![Some(1)]);
    cx.update_window(window, |_, window, cx| window.click("timeline-bar-1", cx)).unwrap();
    frame(cx, window);
    assert!(cx.update(|cx| timeline.read(cx).selected().is_none()), "a second click clears it");
    // Every action
    cx.update(|cx| timeline.update(cx, |t, cx| t.set_summary(false, cx)));
    frame(cx, window);
    assert_eq!(cx.update(|cx| timeline.read(cx).shown().len()), actions);
    // Both themes
    cx.update(|cx| kitchen_ui::theme::use_look(kitchen_ui::theme::look().with_dark(false), cx));
    frame(cx, window);
}
