//! Keypoints over video: where each body part of each animal is in each frame, with how sure the
//! model was, and the skeleton that joins them. What markerless pose estimation gives (DeepLabCut,
//! SLEAP, Lightning Pose, dspk's pose plugin), in one shape every view draws the same way.
//!
//! - [`Skeleton`]: the body parts (nodes) and the lines between them (edges).
//! - [`Pose`]: for every frame, animal (track) and node, `x`, `y` in the video's pixels and a
//!   score in 0–1; a missing point is NaN.
//! - The long CSV every tool can write and read: `frame,track,node,x,y,score`, one point a line
//!   ([`Pose::from_long_csv`], [`Pose::to_long_csv`]). The pose plugin writes the same columns as
//!   Parquet (`table/parquet`).
//! - [`paint`]: the points and skeleton of one frame, painted into a view's bounds where the
//!   video is drawn.

use gpui_kit::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, point, px, size};
use serde::{Deserialize, Serialize};

/// The body parts and the lines between them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skeleton {
    pub nodes: Vec<String>,
    /// Pairs of node indices.
    pub edges: Vec<(usize, usize)>,
}

impl Skeleton {
    /// Nodes by name, edges by pairs of names (pairs naming unknown nodes are left out).
    pub fn new(nodes: &[&str], edges: &[(&str, &str)]) -> Self {
        let nodes: Vec<String> = nodes.iter().map(|n| n.to_string()).collect();
        let at = |n: &str| nodes.iter().position(|x| x == n);
        let edges = edges.iter().filter_map(|(a, b)| Some((at(a)?, at(b)?))).collect();
        Self { nodes, edges }
    }

    pub fn node(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n == name)
    }
}

/// Keypoints for every frame, track and node.
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub skeleton: Skeleton,
    pub frames: usize,
    pub tracks: usize,
    /// `[x, y, score]`, frame-major, then track, then node.
    points: Vec<[f32; 3]>,
}

impl Pose {
    /// Every point missing.
    pub fn new(skeleton: Skeleton, frames: usize, tracks: usize) -> Self {
        let n = frames * tracks.max(1) * skeleton.nodes.len();
        Self { frames, tracks: tracks.max(1), points: vec![[f32::NAN; 3]; n], skeleton }
    }

    fn at(&self, frame: usize, track: usize, node: usize) -> Option<usize> {
        (frame < self.frames && track < self.tracks && node < self.skeleton.nodes.len()).then(|| (frame * self.tracks + track) * self.skeleton.nodes.len() + node)
    }

    pub fn set(&mut self, frame: usize, track: usize, node: usize, x: f32, y: f32, score: f32) {
        if let Some(i) = self.at(frame, track, node) {
            self.points[i] = [x, y, score];
        }
    }

    /// The point, when there is one.
    pub fn point(&self, frame: usize, track: usize, node: usize) -> Option<(f32, f32, f32)> {
        let [x, y, s] = self.points[self.at(frame, track, node)?];
        (x.is_finite() && y.is_finite()).then_some((x, y, s))
    }

    /// Frame by frame, the mean score of one node (NaN where it is missing): how sure the model
    /// was over time, drawn as a trace beside the video.
    pub fn score_trace(&self, track: usize, node: usize) -> Vec<f32> {
        (0..self.frames).map(|f| self.point(f, track, node).map_or(f32::NAN, |p| p.2)).collect()
    }

    /// From the long CSV (`frame,track,node,x,y,score`, a header first). Nodes are taken in the
    /// order they first appear unless `skeleton` names them; tracks are numbers or names (named
    /// tracks are numbered in order of appearance).
    pub fn from_long_csv(text: &str, skeleton: Option<Skeleton>) -> Result<Pose, String> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let header: Vec<String> = lines.next().ok_or("empty file")?.split(',').map(|h| h.trim().to_ascii_lowercase()).collect();
        let col = |name: &str| header.iter().position(|h| h == name).ok_or_else(|| format!("no column {name:?} (need frame, track, node, x, y, score)"));
        let (cf, ct, cn, cx, cy, cs) = (col("frame")?, col("track")?, col("node")?, col("x")?, col("y")?, col("score")?);
        let mut rows = Vec::new();
        let (mut nodes, mut tracks): (Vec<String>, Vec<String>) = (skeleton.as_ref().map(|s| s.nodes.clone()).unwrap_or_default(), Vec::new());
        let mut frames = 0;
        for (i, line) in lines.enumerate() {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            let get = |c: usize| f.get(c).copied().ok_or_else(|| format!("line {}: too few columns", i + 2));
            let num = |c: usize| -> Result<f32, String> {
                let v = get(c)?;
                if v.is_empty() || v.eq_ignore_ascii_case("nan") { Ok(f32::NAN) } else { v.parse().map_err(|_| format!("line {}: {v:?} is not a number", i + 2)) }
            };
            let frame: usize = get(cf)?.parse().map_err(|_| format!("line {}: frame is not a whole number", i + 2))?;
            let (track, node) = (get(ct)?.to_string(), get(cn)?.to_string());
            if !tracks.contains(&track) {
                tracks.push(track.clone());
            }
            if !nodes.contains(&node) {
                if skeleton.is_some() {
                    continue;
                }
                nodes.push(node.clone());
            }
            frames = frames.max(frame + 1);
            rows.push((frame, track, node, num(cx)?, num(cy)?, num(cs)?));
        }
        // Numbered tracks keep their numbers' order
        if tracks.iter().all(|t| t.parse::<usize>().is_ok()) {
            tracks.sort_by_key(|t| t.parse::<usize>().unwrap_or(0));
        }
        let skeleton = skeleton.unwrap_or(Skeleton { nodes: nodes.clone(), edges: Vec::new() });
        let mut pose = Pose::new(skeleton, frames, tracks.len());
        for (frame, track, node, x, y, s) in rows {
            let (Some(t), Some(n)) = (tracks.iter().position(|x| *x == track), nodes.iter().position(|x| *x == node)) else { continue };
            pose.set(frame, t, n, x, y, s);
        }
        Ok(pose)
    }

    /// As the long CSV, missing points left out.
    pub fn to_long_csv(&self) -> String {
        let mut out = String::from("frame,track,node,x,y,score\n");
        for f in 0..self.frames {
            for t in 0..self.tracks {
                for (n, name) in self.skeleton.nodes.iter().enumerate() {
                    if let Some((x, y, s)) = self.point(f, t, n) {
                        out.push_str(&format!("{f},{t},{name},{x},{y},{s}\n"));
                    }
                }
            }
        }
        out
    }
}

/// How keypoints are drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct PoseStyle {
    /// A colour per node, cycled (Okabe–Ito by default: told apart with colour blindness).
    pub colours: Vec<Hsla>,
    /// Points under this score are drawn hollow, their lines left out.
    pub threshold: f32,
    pub radius: f32,
    pub line_width: f32,
}

impl Default for PoseStyle {
    fn default() -> Self {
        let rgb = |c: u32| kitchen_ui::theme::color::rgb_hsla(c);
        Self {
            colours: [0x0072B2, 0xE69F00, 0x009E73, 0xCC79A7, 0x56B4E9, 0xD55E00, 0xF0E442].into_iter().map(rgb).collect(),
            threshold: 0.6,
            radius: 3.5,
            line_width: 1.5,
        }
    }
}

/// Where a video of `width` × `height` pixels is drawn inside `bounds` (scaled to fit, centred):
/// its origin and scale, to place points.
pub fn fit(bounds: Bounds<Pixels>, width: u32, height: u32) -> (Point<Pixels>, f32) {
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let scale = (bw / width.max(1) as f32).min(bh / height.max(1) as f32);
    let (w, h) = (width as f32 * scale, height as f32 * scale);
    (point(bounds.origin.x + px((bw - w) / 2.0), bounds.origin.y + px((bh - h) / 2.0)), scale)
}

/// Paints frame `frame` of `pose` over a video of `width` × `height` drawn to fit `bounds`.
pub fn paint(window: &mut Window, bounds: Bounds<Pixels>, width: u32, height: u32, pose: &Pose, frame: usize, style: &PoseStyle) {
    let (origin, scale) = fit(bounds, width, height);
    let at = |x: f32, y: f32| point(origin.x + px(x * scale), origin.y + px(y * scale));
    let colour = |n: usize| style.colours.get(n % style.colours.len().max(1)).copied().unwrap_or_else(gpui_kit::white);
    for track in 0..pose.tracks {
        for &(a, b) in &pose.skeleton.edges {
            let (Some(p), Some(q)) = (pose.point(frame, track, a), pose.point(frame, track, b)) else { continue };
            if p.2 < style.threshold || q.2 < style.threshold {
                continue;
            }
            let mut line = PathBuilder::stroke(px(style.line_width));
            line.move_to(at(p.0, p.1));
            line.line_to(at(q.0, q.1));
            if let Ok(path) = line.build() {
                window.paint_path(path, colour(a).opacity(0.8));
            }
        }
        for n in 0..pose.skeleton.nodes.len() {
            let Some((x, y, s)) = pose.point(frame, track, n) else { continue };
            let r = px(style.radius);
            let dot = Bounds::centered_at(at(x, y), size(r * 2.0, r * 2.0));
            let quad = if s >= style.threshold {
                gpui_kit::fill(dot, colour(n)).corner_radii(r)
            } else {
                gpui_kit::outline(dot, colour(n).opacity(0.7), gpui_kit::BorderStyle::Solid).corner_radii(r)
            };
            window.paint_quad(quad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_long_csv_reads_back_what_it_wrote() {
        let csv = "frame,track,node,x,y,score\n0,mouse1,nose,10,20,0.9\n0,mouse1,tail_base,30,40,0.4\n2,mouse1,nose,11,21,0.95\n";
        let pose = Pose::from_long_csv(csv, None).unwrap();
        assert_eq!((pose.frames, pose.tracks, pose.skeleton.nodes.len()), (3, 1, 2));
        assert_eq!(pose.point(0, 0, 1), Some((30.0, 40.0, 0.4)));
        assert_eq!(pose.point(1, 0, 0), None, "frame 1 has no points");
        let again = Pose::from_long_csv(&pose.to_long_csv(), None).unwrap();
        assert_eq!(again.point(2, 0, 0), Some((11.0, 21.0, 0.95)));
        assert!(pose.score_trace(0, 0)[1].is_nan());
        // A skeleton names the nodes kept, and the edges
        let sk = Skeleton::new(&["nose"], &[("nose", "tail_base")]);
        let only = Pose::from_long_csv(csv, Some(sk)).unwrap();
        assert_eq!((only.skeleton.nodes.len(), only.skeleton.edges.len()), (1, 0));
    }

    #[test]
    fn videos_fit_their_bounds_centred() {
        let bounds = Bounds { origin: point(px(0.), px(0.)), size: size(px(200.), px(100.)) };
        let (origin, scale) = fit(bounds, 100, 100);
        assert_eq!((f32::from(origin.x), f32::from(origin.y), scale), (50.0, 0.0, 1.0));
    }
}
