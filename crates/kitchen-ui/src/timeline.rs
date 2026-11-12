//! An episodic memory of the work: what the person and the agent did, over time, as a story one
//! can read back (after Scott Jenson's "Episodic Memory" prototype). It shows attention, not
//! content: which kind of work, when, for how long and how busily.
//!
//! - **Input**: [`Activity`]: one thing that happened (a command run, time in a workspace, a job),
//!   its lane (who or what: you, the agent, jobs), its kind (the colour), what it was, where it
//!   came from (its provenance: the workspace, how it was started) and what it produced (assets).
//! - **Synthesis**: [`episodes`] turns many activities into a few episodes: neighbours of the same
//!   kind merge, then the shortest episodes fold into their neighbours until a handful is left
//!   (50 browser visits become "Research").
//! - **View**: [`Timeline`]: one row per lane, a bar per episode (its width the time, its height
//!   how busy it was, its colour its kind), the hours under it, a card on hover, and, for the
//!   episode clicked, context cards below: each thing done in it, where it came from and what it
//!   produced. Summary shows the episodes, All every activity.

use std::collections::HashMap;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Context, EventEmitter, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _, Window, div, px, relative,
};

use crate::theme::{self, color};
use crate::widgets::MenuSelect;

/// One thing that happened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activity {
    /// When it started (seconds since 1970, UTC).
    pub start: f64,
    /// When it ended (the same as `start` for an instant: it then lasts until the next one).
    pub end: f64,
    /// Who or what did it: its row ("You", "Agent", "Jobs", "Focus").
    pub lane: SharedString,
    /// Its kind: the colour and the episode it joins ("Analyze", "Pipeline", "histology").
    pub kind: SharedString,
    /// What it was ("Find cells").
    pub title: SharedString,
    /// A short line about it (its arguments, its result).
    pub detail: SharedString,
    /// Where it came from: its provenance ("Explore · menu", "agent · lumen MCP").
    pub source: SharedString,
    /// What it produced or used (files, ids).
    pub assets: Vec<SharedString>,
}

/// Several activities read as one: a stretch of one kind of work.
#[derive(Clone, Debug, PartialEq)]
pub struct Episode {
    /// When it started (seconds since 1970).
    pub start: f64,
    /// When it ended.
    pub end: f64,
    /// Its row.
    pub lane: SharedString,
    /// Its kind (the colour).
    pub kind: SharedString,
    /// What it was mostly ("Find cells ×3").
    pub title: SharedString,
    /// The activities in it, in time order.
    pub items: Vec<Activity>,
}

impl Episode {
    /// How long it lasted (s).
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }

    /// How busy it was: activities per minute.
    pub fn intensity(&self) -> f64 {
        self.items.len() as f64 / (self.duration() / 60.0).max(1.0)
    }

    fn of(items: Vec<Activity>) -> Episode {
        let start = items.iter().map(|a| a.start).fold(f64::INFINITY, f64::min);
        let end = items.iter().map(|a| a.end).fold(f64::NEG_INFINITY, f64::max);
        // The kind and the title that cover the most time
        let mut by_kind: HashMap<SharedString, f64> = HashMap::new();
        let mut by_title: HashMap<SharedString, (usize, f64)> = HashMap::new();
        for a in &items {
            *by_kind.entry(a.kind.clone()).or_default() += (a.end - a.start).max(1.0);
            let t = by_title.entry(a.title.clone()).or_default();
            t.0 += 1;
            t.1 += (a.end - a.start).max(1.0);
        }
        let kind = by_kind.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|k| k.0).unwrap_or_default();
        let (title, (n, _)) = by_title.into_iter().max_by(|a, b| (a.1.0, a.1.1).partial_cmp(&(b.1.0, b.1.1)).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or_default();
        let title = if n > 1 { format!("{title} ×{n}").into() } else { title };
        Episode { start, end, lane: items.first().map(|a| a.lane.clone()).unwrap_or_default(), kind, title, items }
    }
}

/// The activities with their ends filled in: an instant lasts until the next activity in its lane,
/// at most `idle` seconds (then the person was away), and at least one second.
pub fn fill_ends(items: &[Activity], idle: f64, now: f64) -> Vec<Activity> {
    let mut out: Vec<Activity> = items.to_vec();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut next_in_lane: HashMap<SharedString, f64> = HashMap::new();
    for a in out.iter_mut().rev() {
        if a.end <= a.start {
            let next = next_in_lane.get(&a.lane).copied().unwrap_or(now);
            a.end = (a.start + idle).min(next).max(a.start + 1.0);
        }
        next_in_lane.insert(a.lane.clone(), a.start);
    }
    out
}

/// Every activity as an episode of its own (the "All" view).
pub fn each(items: &[Activity]) -> Vec<Episode> {
    let mut out: Vec<Episode> = items.iter().map(|a| Episode::of(vec![a.clone()])).collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// The story in a few episodes: per lane, neighbours of the same kind less than `gap` seconds
/// apart merge; then, while there are more than `target` episodes, the shortest one folds into
/// the neighbour in its lane it is closer to (in time), until `target` are left or nothing can merge.
pub fn episodes(items: &[Activity], target: usize, gap: f64) -> Vec<Episode> {
    let mut lanes: Vec<SharedString> = Vec::new();
    for a in items {
        if !lanes.contains(&a.lane) {
            lanes.push(a.lane.clone());
        }
    }
    let mut per_lane: Vec<Vec<Vec<Activity>>> = lanes
        .iter()
        .map(|lane| {
            let mut mine: Vec<Activity> = items.iter().filter(|a| &a.lane == lane).cloned().collect();
            mine.sort_by(|a, b| a.start.total_cmp(&b.start));
            let mut groups: Vec<Vec<Activity>> = Vec::new();
            for a in mine {
                match groups.last_mut() {
                    Some(g) if g.last().is_some_and(|l| l.kind == a.kind && a.start - l.end <= gap) => g.push(a),
                    _ => groups.push(vec![a]),
                }
            }
            groups
        })
        .collect();
    let span = |g: &[Activity]| g.last().map_or(0.0, |l| l.end) - g.first().map_or(0.0, |f| f.start);
    loop {
        let count: usize = per_lane.iter().map(Vec::len).sum();
        if count <= target.max(1) {
            break;
        }
        // The shortest episode that has a neighbour in its lane
        let mut best: Option<(usize, usize, f64)> = None;
        for (l, groups) in per_lane.iter().enumerate() {
            if groups.len() < 2 {
                continue;
            }
            for (i, g) in groups.iter().enumerate() {
                let s = span(g);
                if best.is_none_or(|b| s < b.2) {
                    best = Some((l, i, s));
                }
            }
        }
        let Some((l, i, _)) = best else { break };
        let groups = &mut per_lane[l];
        // Into the neighbour closer in time
        let gap_before = (i > 0).then(|| groups[i][0].start - groups[i - 1].last().map_or(0.0, |a| a.end));
        let gap_after = (i + 1 < groups.len()).then(|| groups[i + 1][0].start - groups[i].last().map_or(0.0, |a| a.end));
        let into = match (gap_before, gap_after) {
            (Some(b), Some(a)) => if b <= a { i - 1 } else { i + 1 },
            (Some(_), None) => i - 1,
            _ => i + 1,
        };
        let g = groups.remove(i);
        let into = if into > i { into - 1 } else { into };
        groups[into].extend(g);
        groups[into].sort_by(|a, b| a.start.total_cmp(&b.start));
    }
    let mut out: Vec<Episode> = per_lane.into_iter().flatten().map(Episode::of).collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// The colours of the kinds: a set of hues readable on both themes, given out in order of first
/// appearance so a kind keeps its colour while the view lives.
const KIND_HUES: [u32; 10] = [0x5b8def, 0xf2b134, 0x2fb67c, 0xd9479b, 0x1fb5c9, 0x8b6cf0, 0xef6f4a, 0x8a96a8, 0x9bc53d, 0xc98bdb];

/// The time of day of `t` (seconds since 1970) in the local time zone: "09:19".
pub fn clock(t: f64) -> String {
    jiff::Timestamp::from_millisecond((t * 1000.0) as i64)
        .map(|ts| ts.to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M").to_string())
        .unwrap_or_default()
}

/// A duration as people say it: "45 s", "26 min", "1 h 05".
pub fn duration(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    match s {
        0..60 => format!("{s} s"),
        60..3600 => format!("{} min", s / 60),
        _ => format!("{} h {:02}", s / 3600, (s % 3600) / 60),
    }
}

/// A tick step for a span of `span` seconds: about six ticks, on round times.
fn tick_step(span: f64) -> f64 {
    const STEPS: [f64; 12] = [10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0, 7200.0, 10800.0, 21600.0];
    STEPS.into_iter().find(|s| span / s <= 7.0).unwrap_or(43200.0)
}

/// What the timeline tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum TimelineEvent {
    /// An episode was clicked (its index in [`Timeline::shown`]), or the selection cleared.
    Selected(Option<usize>),
}

/// The view.
pub struct Timeline {
    title: SharedString,
    activities: Vec<Activity>,
    shown: Vec<Episode>,
    summary: bool,
    target: usize,
    gap: f64,
    idle: f64,
    hovered: Option<usize>,
    selected: Option<usize>,
    colors: HashMap<SharedString, Hsla>,
}

impl EventEmitter<TimelineEvent> for Timeline {}

impl Timeline {
    /// An empty timeline titled `title`, summarising to about eight episodes.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self { title: title.into(), activities: Vec::new(), shown: Vec::new(), summary: true, target: 8, gap: 120.0, idle: 300.0, hovered: None, selected: None, colors: HashMap::new() }
    }

    /// Sets what happened (instants last until the next activity in their lane, at most five
    /// minutes) and draws it again; the selection is kept when its episode still starts the same.
    pub fn set_activities(&mut self, items: Vec<Activity>, now: f64, cx: &mut Context<Self>) {
        let keep = self.selected.and_then(|i| self.shown.get(i)).map(|e| (e.lane.clone(), e.start));
        self.activities = fill_ends(&items, self.idle, now);
        self.rebuild();
        self.selected = keep.and_then(|(lane, start)| self.shown.iter().position(|e| e.lane == lane && e.start == start));
        cx.notify();
    }

    /// Shows the episodes (`true`) or every activity.
    pub fn set_summary(&mut self, summary: bool, cx: &mut Context<Self>) {
        self.summary = summary;
        self.selected = None;
        self.hovered = None;
        self.rebuild();
        cx.notify();
    }

    /// How many episodes the summary aims at.
    pub fn set_target(&mut self, target: usize, cx: &mut Context<Self>) {
        self.target = target.max(1);
        self.rebuild();
        cx.notify();
    }

    /// The episodes shown.
    pub fn shown(&self) -> &[Episode] {
        &self.shown
    }

    /// Every activity.
    pub fn activities(&self) -> &[Activity] {
        &self.activities
    }

    /// The episode clicked.
    pub fn selected(&self) -> Option<&Episode> {
        self.selected.and_then(|i| self.shown.get(i))
    }

    /// Selects the episode `index` (`None`: none), as a click does.
    pub fn select(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.selected = index.filter(|i| *i < self.shown.len());
        cx.emit(TimelineEvent::Selected(self.selected));
        cx.notify();
    }

    fn rebuild(&mut self) {
        self.shown = if self.summary { episodes(&self.activities, self.target, self.gap) } else { each(&self.activities) };
        for a in &self.activities {
            if !self.colors.contains_key(&a.kind) {
                let hue = KIND_HUES[self.colors.len() % KIND_HUES.len()];
                self.colors.insert(a.kind.clone(), theme::rgb_hsla(hue));
            }
        }
    }

    fn color(&self, kind: &SharedString) -> Hsla {
        self.colors.get(kind).copied().unwrap_or_else(color::text_muted)
    }

    fn lanes(&self) -> Vec<SharedString> {
        let mut out: Vec<SharedString> = Vec::new();
        for a in &self.activities {
            if !out.contains(&a.lane) {
                out.push(a.lane.clone());
            }
        }
        out
    }

    fn range(&self) -> (f64, f64) {
        let start = self.activities.iter().map(|a| a.start).fold(f64::INFINITY, f64::min);
        let end = self.activities.iter().map(|a| a.end).fold(f64::NEG_INFINITY, f64::max);
        if start.is_finite() && end > start { (start, end) } else { (0.0, 1.0) }
    }

    fn context_cards(&self, e: &Episode) -> impl IntoElement {
        let dot = self.color(&e.kind);
        let head = h_flex()
            .gap(px(theme::SM))
            .items_center()
            .child(div().size(px(10.)).rounded_full().bg(dot))
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(e.title.clone()))
            .child(div().text_xs().text_color(color::text_muted()).child(format!("{} · {} · {} – {} · {}", e.lane, e.kind, clock(e.start), clock(e.end), duration(e.duration()))));
        let cards = e.items.iter().enumerate().map(|(i, a)| {
            v_flex()
                .id(SharedString::from(format!("timeline-card-{i}")))
                .w(px(260.))
                .flex_none()
                .gap(px(theme::XS))
                .p(px(theme::SM))
                .rounded(px(theme::RADIUS))
                .border_1()
                .border_color(color::border())
                .bg(color::raised())
                .child(h_flex().gap(px(theme::XS)).items_center().child(div().size(px(8.)).rounded_full().bg(self.color(&a.kind))).child(div().flex_1().min_w_0().truncate().text_sm().font_weight(FontWeight::SEMIBOLD).child(a.title.clone())).child(div().text_xs().text_color(color::text_muted()).child(clock(a.start))))
                .when(!a.source.is_empty(), |d| d.child(div().text_xs().text_color(color::text_muted()).child(format!("From {}", a.source))))
                .when(!a.detail.is_empty(), |d| d.child(div().text_xs().font_family("monospace").text_color(color::text_muted()).line_clamp(3).child(a.detail.clone())))
                .when(!a.assets.is_empty(), |d| {
                    d.child(h_flex().flex_wrap().gap(px(theme::XS)).children(a.assets.iter().map(|s| div().px(px(theme::XS)).rounded(px(theme::RADIUS_SM)).bg(color::control()).text_xs().truncate().max_w(px(240.)).child(s.clone()))))
                })
        });
        v_flex().gap(px(theme::SM)).child(head).child(h_flex().id("timeline-cards").gap(px(theme::SM)).overflow_x_scroll().pb(px(theme::XS)).children(cards))
    }
}

/// Height of a lane (px): the tallest bar and its label.
const LANE_H: f32 = 64.;
/// Width of the lane names (px).
const LANE_LABEL_W: f32 = 72.;

impl Render for Timeline {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (t0, t1) = self.range();
        let span = (t1 - t0).max(1.0);
        let x = move |t: f64| ((t - t0) / span).clamp(0.0, 1.0) as f32;
        let busiest = self.shown.iter().map(Episode::intensity).fold(0.0f64, f64::max).max(1e-9);
        let header = h_flex()
            .gap(px(theme::MD))
            .items_center()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(self.title.clone()))
            .child(div().flex_1().text_xs().text_color(color::text_muted()).child(if self.activities.is_empty() {
                "Nothing yet: what you and the agent do shows here".to_string()
            } else {
                format!("{} episodes from {} actions · {} – {}", self.shown.len(), self.activities.len(), clock(t0), clock(t1))
            }))
            .child(div().w(px(150.)).child({
                let me = cx.entity().downgrade();
                MenuSelect::new("timeline-detail", vec!["Summary".into(), "Every action".into()], Some(if self.summary { 0 } else { 1 }), move |i, _, cx| {
                    let _ = me.update(cx, |t, cx| t.set_summary(i == 0, cx));
                })
            }));
        let lanes = self.lanes().into_iter().map(|lane| {
            let bars = self.shown.iter().enumerate().filter(|(_, e)| e.lane == lane).map(|(i, e)| {
                let (left, width) = (x(e.start), (x(e.end) - x(e.start)).max(0.002));
                // Height: how busy it was (at least a third, so a quiet stretch still reads)
                let h = 0.35 + 0.65 * (e.intensity() / busiest).sqrt() as f32;
                let (hovered, selected) = (self.hovered == Some(i), self.selected == Some(i));
                let fill = self.color(&e.kind);
                let label = (width > 0.07).then(|| {
                    div().absolute().left(relative(left)).w(relative(width)).top_0().px(px(2.)).truncate().text_xs().text_color(fill).child(e.title.clone())
                });
                let bar = div()
                    .id(SharedString::from(format!("timeline-bar-{i}")))
                    .absolute()
                    .left(relative(left))
                    .w(relative(width))
                    .bottom_0()
                    .h(px((LANE_H - 18.) * h))
                    .rounded(px(2.))
                    .bg(if hovered || selected { fill } else { fill.opacity(0.85) })
                    .when(selected, |d| d.border_2().border_color(color::text()))
                    .cursor_pointer()
                    .on_hover(cx.listener(move |t, on: &bool, _, cx| {
                        t.hovered = if *on { Some(i) } else if t.hovered == Some(i) { None } else { t.hovered };
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |t, _, _, cx| {
                        let next = if t.selected == Some(i) { None } else { Some(i) };
                        t.select(next, cx);
                    }))
                    .test_support();
                [label.map(|l| l.into_any_element()), Some(bar.into_any_element())]
            });
            h_flex()
                .h(px(LANE_H))
                .items_end()
                .child(div().w(px(LANE_LABEL_W)).flex_none().pb(px(theme::XS)).text_xs().text_color(color::text_muted()).child(lane.clone()))
                .child(div().relative().flex_1().h_full().border_b_1().border_color(color::border()).children(bars.flatten().flatten()))
        });
        // The hours (or minutes) under the bars
        let step = tick_step(span);
        let first = (t0 / step).ceil() * step;
        let ticks = (0..12).map(|k| first + k as f64 * step).take_while(|t| *t <= t1).map(|t| {
            div().absolute().left(relative(x(t))).top_0().pl(px(2.)).border_l_1().border_color(color::border()).text_xs().text_color(color::text_muted()).child(clock(t))
        });
        let axis = h_flex().h(px(18.)).child(div().w(px(LANE_LABEL_W)).flex_none()).child(div().relative().flex_1().h_full().children(ticks));
        // The card of the episode under the pointer
        let hover = self.hovered.and_then(|i| self.shown.get(i)).map(|e| {
            let left = x(e.start).min(0.75);
            h_flex().h(px(0.)).child(div().w(px(LANE_LABEL_W)).flex_none()).child(
                div().relative().flex_1().child(
                    v_flex()
                        .absolute()
                        .left(relative(left))
                        .top(px(theme::XS))
                        .w(px(260.))
                        .gap(px(2.))
                        .p(px(theme::SM))
                        .rounded(px(theme::RADIUS))
                        .border_1()
                        .border_color(color::border())
                        .bg(color::raised())
                        .shadow_lg()
                        .child(h_flex().gap(px(theme::XS)).items_center().child(div().size(px(8.)).rounded_full().bg(self.color(&e.kind))).child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(e.kind.clone())).child(div().text_xs().text_color(color::text_muted()).child(format!("{} · {}", clock(e.start), duration(e.duration())))))
                        .child(div().text_sm().truncate().child(e.title.clone()))
                        .child(div().text_xs().text_color(color::text_muted()).child(format!("{} action{} · {}", e.items.len(), if e.items.len() == 1 { "" } else { "s" }, e.lane)))
                        .children(e.items.first().filter(|a| !a.detail.is_empty()).map(|a| div().text_xs().font_family("monospace").text_color(color::text_muted()).line_clamp(2).child(a.detail.clone()))),
                ),
            )
        });
        let context = self.selected.and_then(|i| self.shown.get(i)).map(|e| self.context_cards(e));
        v_flex()
            .id("timeline")
            .size_full()
            .gap(px(theme::SM))
            .p(px(theme::MD))
            .overflow_y_scroll()
            .child(header)
            .child(v_flex().children(lanes).child(axis).children(hover))
            .children(context)
            .when(self.selected.is_none() && !self.shown.is_empty(), |d| d.child(div().pt(px(theme::LG)).text_xs().text_color(color::text_muted()).child("Click a bar to see what was done in it, where it came from and what it produced.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(start: f64, lane: &str, kind: &str, title: &str) -> Activity {
        Activity { start, end: start, lane: lane.into(), kind: kind.into(), title: title.into(), ..Default::default() }
    }

    /// Instants last until the next activity in their lane, and never past the idle limit.
    #[test]
    fn instants_last_until_the_next_one() {
        let filled = fill_ends(&[act(0.0, "You", "A", "a"), act(60.0, "You", "B", "b"), act(10.0, "Agent", "C", "c")], 300.0, 1000.0);
        let end = |t: &str| filled.iter().find(|a| a.title == t).unwrap().end;
        assert_eq!(end("a"), 60.0);
        assert_eq!(end("b"), 360.0, "the idle limit");
        assert_eq!(end("c"), 310.0, "lanes are apart");
    }

    /// Fifty actions of a few kinds become the few episodes asked for, each keeping its actions.
    #[test]
    fn many_actions_become_a_few_episodes() {
        let mut items = Vec::new();
        let kinds = ["Research", "Draft", "Research", "Chat", "Draft"];
        for k in 0..50 {
            let kind = kinds[(k / 10) % kinds.len()];
            // A stray action of another kind now and then
            let kind = if k % 7 == 3 { "System" } else { kind };
            items.push(act(k as f64 * 30.0, "You", kind, &format!("step {}", k % 4)));
        }
        let items = fill_ends(&items, 300.0, 1500.0);
        let raw = each(&items);
        assert_eq!(raw.len(), 50);
        let eps = episodes(&items, 7, 120.0);
        assert!(eps.len() <= 7, "{} episodes", eps.len());
        assert_eq!(eps.iter().map(|e| e.items.len()).sum::<usize>(), 50, "nothing lost");
        assert!(eps.windows(2).all(|w| w[0].start <= w[1].start));
        assert!(eps.iter().any(|e| e.kind == "Research") && eps.iter().any(|e| e.kind == "Draft"));
    }

    #[test]
    fn durations_and_ticks_read_well() {
        assert_eq!(duration(45.0), "45 s");
        assert_eq!(duration(26.0 * 60.0), "26 min");
        assert_eq!(duration(3900.0), "1 h 05");
        assert_eq!(tick_step(3.0 * 3600.0), 1800.0);
        assert_eq!(tick_step(600.0), 120.0);
    }
}
