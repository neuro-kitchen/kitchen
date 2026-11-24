//! The video view: the frame at the clock's moment, keypoints over it, and what a researcher needs
//! under it (the frame number, the time on both clocks, dropped frames, how well the clocks are
//! aligned).
//!
//! It follows the workspace's [`Clock`]: on every move it maps the reference time onto the video's
//! clock ([`Alignment`]), finds the frame shown then ([`crate::video::FrameTimes::frame_at`]), and
//! shows it, from the cache or decoded off the UI thread (only the latest frame asked for is
//! decoded: scrubbing fast does not queue up). Opening a video sets the clock's step to its frame
//! interval, so frame steps land on frames. The playback keys work over it.

use std::sync::{Arc, Mutex};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    App, Context, Entity, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, RenderImage, SharedString, Styled as _, StyledImage as _,
    Subscription, Window, canvas, div, img, px,
};
use kitchen_ui::theme::{self, color};
use smallvec::SmallVec;

use crate::clock::{Clock, ClockEvent, Faster, JumpBack, JumpForward, Slower, StepBack, StepForward, TogglePlay};
use crate::pose::{Pose, PoseStyle};
use crate::sync::Alignment;
use crate::video::{Frame, FrameCache, VideoError, VideoInfo, VideoSource};

/// Decoded frames kept for scrubbing (bytes).
const CACHE_BYTES: usize = 512 << 20;

/// A video in a view, following a clock.
pub struct VideoView {
    source: Arc<Mutex<Box<dyn VideoSource>>>,
    info: VideoInfo,
    clock: Entity<Clock>,
    alignment: Alignment,
    cache: Arc<Mutex<FrameCache>>,
    shown: Option<(usize, Arc<RenderImage>)>,
    /// A decode is running.
    busy: bool,
    /// The frame to show next (asked for while a decode ran).
    wanted: Option<usize>,
    pose: Option<Arc<Pose>>,
    style: PoseStyle,
    error: Option<String>,
    _clock: Subscription,
}

impl VideoView {
    /// `source` in a view following `clock`, its time mapped onto the clock's by `alignment`.
    pub fn new(source: Box<dyn VideoSource>, clock: Entity<Clock>, alignment: Alignment, cx: &mut Context<Self>) -> Self {
        let info = source.info().clone();
        let sub = cx.subscribe(&clock, |this, _, e: &ClockEvent, cx| {
            if let ClockEvent::Moved(t) = e {
                this.show_time(*t, cx);
            }
        });
        // The clock steps by this video's frames, and runs over it
        let step = info.times.median_interval();
        let span = info.times.span().map(|(a, b)| (alignment.to_reference(a), alignment.to_reference(b)));
        clock.update(cx, |c, cx| {
            if let Some(s) = step {
                c.set_step(s * alignment.slope);
            }
            if let Some((a, b)) = span {
                let (a0, b0) = c.range().unwrap_or((a, b));
                c.set_range(a.min(a0), b.max(b0), cx);
            }
        });
        let mut view = Self {
            source: Arc::new(Mutex::new(source)),
            info,
            clock: clock.clone(),
            alignment,
            cache: Arc::new(Mutex::new(FrameCache::new(CACHE_BYTES))),
            shown: None,
            busy: false,
            wanted: None,
            pose: None,
            style: PoseStyle::default(),
            error: None,
            _clock: sub,
        };
        let t = clock.read(cx).position();
        view.show_time(t, cx);
        view
    }

    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    /// Maps the video's clock onto the reference with `alignment` (a sync-pulse fit, or a shift
    /// set by hand); the frame shown follows.
    pub fn set_alignment(&mut self, alignment: Alignment, cx: &mut Context<Self>) {
        self.alignment = alignment;
        let t = self.clock.read(cx).position();
        self.show_time(t, cx);
    }

    /// Draws `pose` over the frames (none: no keypoints).
    pub fn set_pose(&mut self, pose: Option<Arc<Pose>>, cx: &mut Context<Self>) {
        self.pose = pose;
        cx.notify();
    }

    pub fn set_pose_style(&mut self, style: PoseStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    /// The frame index shown, when one is.
    pub fn shown_frame(&self) -> Option<usize> {
        self.shown.as_ref().map(|(i, _)| *i)
    }

    /// Shows the frame at reference time `t`.
    fn show_time(&mut self, t: f64, cx: &mut Context<Self>) {
        let Some(index) = self.info.times.frame_at(self.alignment.to_source(t)) else { return };
        self.show(index, cx);
    }

    /// Shows frame `index`: from the cache at once, else decoded off the UI thread.
    fn show(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.shown_frame() == Some(index) {
            return;
        }
        if let Some(frame) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(index) {
            self.set_shown(frame, cx);
            return;
        }
        if self.busy {
            self.wanted = Some(index);
            return;
        }
        self.busy = true;
        let (source, cache) = (self.source.clone(), self.cache.clone());
        cx.spawn(async move |this, cx| {
            let decoded = cx.background_executor().spawn(async move { source.lock().unwrap_or_else(|e| e.into_inner()).frame(index) }).await;
            let _ = this.update(cx, |v, cx| {
                v.busy = false;
                match decoded {
                    Ok(frame) => {
                        cache.lock().unwrap_or_else(|e| e.into_inner()).put(frame.clone());
                        v.error = None;
                        v.set_shown(frame, cx);
                    }
                    Err(e) => v.error = Some(e.to_string()),
                }
                if let Some(next) = v.wanted.take() {
                    v.show(next, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn set_shown(&mut self, frame: Frame, cx: &mut Context<Self>) {
        match render_image(&frame) {
            Ok(image) => self.shown = Some((frame.index, image)),
            Err(e) => self.error = Some(e.to_string()),
        }
        cx.notify();
    }

    fn status(&self, cx: &App) -> String {
        let clock = self.clock.read(cx);
        let frames = self.info.frames();
        let index = self.shown_frame().map_or("–".to_string(), |i| (i + 1).to_string());
        let t_video = self.shown_frame().and_then(|i| self.info.times.time(i)).unwrap_or(0.0);
        let gaps = self.info.times.gaps();
        let dropped: usize = gaps.iter().map(|g| g.missing).sum();
        let mut parts = vec![
            format!("frame {index} / {frames}"),
            format!("video {t_video:.3} s"),
            format!("reference {:.3} s", clock.position()),
        ];
        if let Some(rate) = self.info.times.rate() {
            parts.push(format!("{rate:.2} fps"));
        }
        if dropped > 0 {
            parts.push(format!("{dropped} dropped in {} gaps", gaps.len()));
        }
        if self.alignment.pulses > 0 {
            parts.push(format!("aligned on {} pulses, {:.2} ms rms", self.alignment.pulses, self.alignment.rms * 1e3));
        }
        if (clock.rate() - 1.0).abs() > 1e-9 {
            parts.push(format!("{}×", clock.rate()));
        }
        parts.join(" · ")
    }
}

/// `frame` as an image GPUI draws (BGRA).
fn render_image(frame: &Frame) -> Result<Arc<RenderImage>, VideoError> {
    let mut bgra = frame.rgba.as_ref().clone();
    for px in bgra.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let buffer = image::RgbaImage::from_raw(frame.width, frame.height, bgra).ok_or_else(|| VideoError::Decode(format!("frame {}: wrong size", frame.index)))?;
    let frames: SmallVec<[image::Frame; 1]> = SmallVec::from_elem(image::Frame::new(buffer), 1);
    Ok(Arc::new(RenderImage::new(frames)))
}

impl Render for VideoView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (w, h) = (self.info.width, self.info.height);
        let overlay = self.pose.clone().zip(self.shown_frame()).map(|(pose, frame)| {
            let style = self.style.clone();
            canvas(|_, _, _| (), move |bounds, _, window, _| crate::pose::paint(window, bounds, w, h, &pose, frame, &style)).absolute().top_0().left_0().size_full()
        });
        let picture = match &self.shown {
            Some((_, image)) => div().relative().size_full().child(img(image.clone()).size_full().object_fit(ObjectFit::Contain)).children(overlay).into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(color::text_muted())
                .child(SharedString::from(self.error.clone().unwrap_or_else(|| "Decoding…".into())))
                .into_any_element(),
        };
        let clock = self.clock.clone();
        let step = move |frames: i64, cx: &mut App| clock.update(cx, |c, cx| c.step_by(frames, cx));
        let (s1, s2, s3, s4) = (step.clone(), step.clone(), step.clone(), step);
        let (c1, c2, c3) = (self.clock.clone(), self.clock.clone(), self.clock.clone());
        v_flex()
            .size_full()
            .bg(gpui_kit::black())
            .on_action(move |_: &StepForward, _, cx| s1(1, cx))
            .on_action(move |_: &StepBack, _, cx| s2(-1, cx))
            .on_action(move |_: &JumpForward, _, cx| s3(10, cx))
            .on_action(move |_: &JumpBack, _, cx| s4(-10, cx))
            .on_action(move |_: &TogglePlay, _, cx| c1.update(cx, |c, cx| c.toggle_play(cx)))
            .on_action(move |_: &Faster, _, cx| c2.update(cx, |c, cx| c.change_rate(true, cx)))
            .on_action(move |_: &Slower, _, cx| c3.update(cx, |c, cx| c.change_rate(false, cx)))
            .child(div().flex_1().min_h_0().child(picture))
            .child(
                h_flex()
                    .flex_none()
                    .px(px(theme::SM))
                    .py(px(theme::XS))
                    .gap(px(theme::SM))
                    .bg(color::panel())
                    .border_t_1()
                    .border_color(color::border())
                    .text_xs()
                    .font_family("monospace")
                    .text_color(color::text_muted())
                    .child(div().flex_1().min_w_0().truncate().child(self.status(cx)))
                    .child(div().flex_none().child(SharedString::from(self.info.format.clone()))),
            )
            .children(self.error.clone().filter(|_| self.shown.is_some()).map(|e| div().px(px(theme::SM)).text_xs().text_color(color::danger()).child(e)))
    }
}
