//! The video view: the frame at the clock's moment, an app's overlays over it, and what a
//! researcher needs under it (the frame number, the time on both clocks, dropped frames, how well the clocks are
//! aligned).
//!
//! It follows the workspace's [`Clock`]: on every move it maps the reference time onto the video's
//! clock ([`Alignment`]), finds the frame shown then ([`crate::video::FrameTimes::frame_at`]), and
//! shows it, from the cache or decoded off the UI thread (only the latest frame asked for is
//! decoded: scrubbing fast does not queue up). Opening a video sets the clock's step to its frame
//! interval, so frame steps land on frames. The playback keys work over it.
//!
//! Overlays ([`Overlay`]) are what an app paints over the frame without the view knowing what
//! they are: segmentation outlines on a time-lapse (lucem), ROIs of calcium imaging or spike-time
//! markers (dspk). Each is told where the frame is drawn and which frame it is ([`OverlayFrame`]).

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    App, Bounds, Context, Entity, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _, Render, RenderImage, SharedString, Styled as _, StyledImage as _,
    Pixels, Point, Subscription, Window, canvas, div, img, point, px,
};
use kitchen_ui::theme::{self, color};
use smallvec::SmallVec;

use crate::clock::{Clock, ClockEvent, Faster, JumpBack, JumpForward, Slower, StepBack, StepForward, TogglePlay};
use crate::sync::Alignment;
use crate::video::{Frame, FrameCache, VideoError, VideoInfo, VideoSource};

/// Where the shown frame is drawn and which it is, for an overlay to place what it paints:
/// a point `(x, y)` in the video's pixels is at `origin + (x, y) · scale` in the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayFrame {
    pub index: usize,
    /// On the video's own clock, in seconds.
    pub time: f64,
    pub width: u32,
    pub height: u32,
    pub origin: Point<Pixels>,
    pub scale: f32,
}

impl OverlayFrame {
    /// A point in the video's pixels, in the window.
    pub fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        point(self.origin.x + px(x * self.scale), self.origin.y + px(y * self.scale))
    }
}

/// Something an app paints over the video at the shown frame.
pub type Overlay = Rc<dyn Fn(&mut Window, OverlayFrame)>;

/// Where a video of `width` × `height` pixels is drawn inside `bounds` (scaled to fit, centred):
/// its origin and scale.
pub fn fit(bounds: Bounds<Pixels>, width: u32, height: u32) -> (Point<Pixels>, f32) {
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let scale = (bw / width.max(1) as f32).min(bh / height.max(1) as f32);
    let (w, h) = (width as f32 * scale, height as f32 * scale);
    (point(bounds.origin.x + px((bw - w) / 2.0), bounds.origin.y + px((bh - h) / 2.0)), scale)
}

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
    overlays: Vec<Overlay>,
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
            overlays: Vec::new(),
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

    /// Paints `overlay` over every frame shown, after the ones added before.
    pub fn add_overlay(&mut self, overlay: Overlay, cx: &mut Context<Self>) {
        self.overlays.push(overlay);
        cx.notify();
    }

    /// Removes every overlay.
    pub fn clear_overlays(&mut self, cx: &mut Context<Self>) {
        self.overlays.clear();
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
        let overlay = self.shown_frame().filter(|_| !self.overlays.is_empty()).map(|index| {
            let (overlays, time) = (self.overlays.clone(), self.info.times.time(index).unwrap_or(0.0));
            canvas(|_, _, _| (), move |bounds, _, window, _| {
                let (origin, scale) = fit(bounds, w, h);
                let frame = OverlayFrame { index, time, width: w, height: h, origin, scale };
                for paint in &overlays {
                    paint(window, frame);
                }
            })
            .absolute()
            .top_0()
            .left_0()
            .size_full()
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

#[cfg(test)]
mod tests {
    use gpui_kit::size;

    use super::*;

    #[test]
    fn videos_fit_their_bounds_centred() {
        let bounds = Bounds { origin: point(px(0.), px(0.)), size: size(px(200.), px(100.)) };
        let (origin, scale) = fit(bounds, 100, 100);
        assert_eq!((f32::from(origin.x), f32::from(origin.y), scale), (50.0, 0.0, 1.0));
        let frame = OverlayFrame { index: 0, time: 0.0, width: 100, height: 100, origin, scale };
        assert_eq!(f32::from(frame.at(10.0, 20.0).x), 60.0);
    }
}
