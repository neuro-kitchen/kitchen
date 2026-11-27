//! One clock every view follows. A workspace holds one [`Clock`]; the video, the traces, the
//! raster, the keypoints all draw what is at its position, and any of them moves it (a click on
//! the timeline, a frame step in the video). The position is in seconds of the **reference**
//! clock (usually the ephys recording's); each source maps its own time onto it with an
//! [`crate::sync::Alignment`].
//!
//! Playback advances the position with the wall clock times the rate, on the UI thread at
//! display rate; views draw the frame or sample at the new position. Frame steps move by the
//! clock's step (the video's frame interval when a video is open).
//!
//! The playback keys are their own section (`[playback]`), bound only where a workspace says
//! (its key context): [`PLAYBACK_KEYS`], [`binding`].

use std::time::{Duration, Instant};

use gpui_kit::{Action, App, Context, EventEmitter, KeyBinding, KeyBindingContextPredicate, Task, actions};

/// What the clock tells the views.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClockEvent {
    /// The position changed (seek, step, or playback).
    Moved(f64),
    /// Playback started or stopped.
    Playing(bool),
}

/// The shared time cursor of a workspace.
pub struct Clock {
    position: f64,
    /// Where time runs (the longest source); none until a source is open.
    range: Option<(f64, f64)>,
    playing: bool,
    /// Seconds of the reference clock per second of wall time.
    rate: f64,
    looping: bool,
    /// A frame step, in seconds.
    step: f64,
    ticker: Option<Task<()>>,
}

impl EventEmitter<ClockEvent> for Clock {}

/// How often playback moves the clock (about the display's rate).
const TICK: Duration = Duration::from_millis(16);

/// The rates playback steps through (`[` and `]`).
pub const RATES: [f64; 9] = [0.1, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];

impl Default for Clock {
    fn default() -> Self {
        Self { position: 0.0, range: None, playing: false, rate: 1.0, looping: false, step: 1.0 / 30.0, ticker: None }
    }
}

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }

    /// The position, in seconds of the reference clock.
    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn range(&self) -> Option<(f64, f64)> {
        self.range
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn step(&self) -> f64 {
        self.step
    }

    /// Where time runs; the position is kept inside.
    pub fn set_range(&mut self, start: f64, end: f64, cx: &mut Context<Self>) {
        self.range = Some((start.min(end), start.max(end)));
        let p = self.clamp(self.position);
        if p != self.position {
            self.seek(p, cx);
        }
        cx.notify();
    }

    /// A frame step of `seconds` (a video's frame interval).
    pub fn set_step(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.step = seconds;
        }
    }

    /// Playback wraps to the start at the end, instead of stopping.
    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    fn clamp(&self, t: f64) -> f64 {
        match self.range {
            Some((a, b)) => t.clamp(a, b),
            None => t.max(0.0),
        }
    }

    /// Moves to `t` (kept in the range).
    pub fn seek(&mut self, t: f64, cx: &mut Context<Self>) {
        let t = self.clamp(t);
        if t != self.position {
            self.position = t;
            cx.emit(ClockEvent::Moved(t));
            cx.notify();
        }
    }

    /// Moves by `frames` steps (negative: back). Pauses playback: a step is a precise look.
    pub fn step_by(&mut self, frames: i64, cx: &mut Context<Self>) {
        self.pause(cx);
        // Onto the step grid, so repeated steps land on frames and not between them
        let index = (self.position / self.step).round() + frames as f64;
        self.seek(index * self.step, cx);
    }

    /// Plays at the clock's rate.
    pub fn play(&mut self, cx: &mut Context<Self>) {
        if self.playing {
            return;
        }
        if let Some((a, b)) = self.range
            && self.position >= b
        {
            self.seek(a, cx);
        }
        self.playing = true;
        cx.emit(ClockEvent::Playing(true));
        self.ticker = Some(cx.spawn(async move |this, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(TICK).await;
                let now = Instant::now();
                let dt = now.duration_since(last).as_secs_f64();
                last = now;
                if this.update(cx, |c, cx| c.advance(dt, cx)).unwrap_or(false) {
                    continue;
                }
                break;
            }
        }));
        cx.notify();
    }

    pub fn pause(&mut self, cx: &mut Context<Self>) {
        if !self.playing {
            return;
        }
        self.playing = false;
        self.ticker = None;
        cx.emit(ClockEvent::Playing(false));
        cx.notify();
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        if self.playing { self.pause(cx) } else { self.play(cx) }
    }

    /// Playback at `rate` (seconds of recording per second).
    pub fn set_rate(&mut self, rate: f64, cx: &mut Context<Self>) {
        if rate.is_finite() && rate > 0.0 {
            self.rate = rate;
            cx.notify();
        }
    }

    /// The next rate of [`RATES`] up (`faster`) or down.
    pub fn change_rate(&mut self, faster: bool, cx: &mut Context<Self>) {
        let i = RATES.iter().position(|r| (*r - self.rate).abs() < 1e-9).unwrap_or(3);
        let j = if faster { (i + 1).min(RATES.len() - 1) } else { i.saturating_sub(1) };
        self.set_rate(RATES[j], cx);
    }

    /// Playback moves by `dt` seconds of wall time; whether it goes on.
    fn advance(&mut self, dt: f64, cx: &mut Context<Self>) -> bool {
        if !self.playing {
            return false;
        }
        let next = self.position + dt * self.rate;
        match self.range {
            Some((a, b)) if next > b => {
                if self.looping {
                    self.seek(a + (next - b) % (b - a).max(f64::EPSILON), cx);
                } else {
                    self.seek(b, cx);
                    self.pause(cx);
                    return false;
                }
            }
            _ => self.seek(next, cx),
        }
        true
    }
}

actions!(
    kitchen_views,
    [
        /// Play or pause.
        TogglePlay,
        /// One frame forward.
        StepForward,
        /// One frame back.
        StepBack,
        /// Ten frames forward.
        JumpForward,
        /// Ten frames back.
        JumpBack,
        /// Play faster.
        Faster,
        /// Play slower.
        Slower,
    ]
);

/// The `[playback]` section of `keybindings.toml`: commands and their default keys.
pub const PLAYBACK_KEYS: &[(&str, &[&str])] = &[
    ("toggle_play", &["space"]),
    ("step_forward", &["right", "."]),
    ("step_back", &["left", ","]),
    ("jump_forward", &["shift-right"]),
    ("jump_back", &["shift-left"]),
    ("faster", &["]"]),
    ("slower", &["["]),
];

/// The playback section's name.
pub const SECTION: &str = "playback";

fn action(name: &str) -> Option<Box<dyn Action>> {
    Some(match name {
        "toggle_play" => Box::new(TogglePlay),
        "step_forward" => Box::new(StepForward),
        "step_back" => Box::new(StepBack),
        "jump_forward" => Box::new(JumpForward),
        "jump_back" => Box::new(JumpBack),
        "faster" => Box::new(Faster),
        "slower" => Box::new(Slower),
        _ => return None,
    })
}

/// The binding of `command` to `key` in the key context `context` (a workspace's, so the space
/// bar plays the video there and does something else elsewhere). An app resolves `[playback]`
/// with kitchen-shell's `keys::resolve` and binds what this gives.
pub fn binding(command: &str, key: &str, context: &str, cx: &App) -> Option<KeyBinding> {
    let context = KeyBindingContextPredicate::parse(context).ok().map(std::rc::Rc::new);
    KeyBinding::load(key, action(command)?, context, false, None, cx).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_playback_key_names_an_action() {
        assert!(PLAYBACK_KEYS.iter().all(|(name, _)| action(name).is_some()));
        assert_eq!(RATES[3], 1.0, "the default rate is in the list");
    }
}
