//! What an app provides so the engine can run a graph over its data: a [`Domain`].
//!
//! The apps differ in how they go through their data, so this is theirs:
//!
//! | | lucem | dsp-kitchen |
//! |---|---|---|
//! | Data on an edge | an image block on the GPU (or a mask, labels, a table) | a window of a recording on the device (or events, a table) |
//! | Pieces | tiles of a slide, z·y·x, with a margin of pixels | windows of time, with samples before and after |
//! | Reading | a Zarr level, a channel | a raw or Zarr recording, its channels |
//! | Writing | Zarr chunks; labels joined across tiles | Zarr or raw chunks; events appended |
//!
//! Both walks are here ([`windows`], [`tiles`]), so neither app rewrites them; the reading, the
//! device and the writing stay in the app.
//!
//! A [`Piece`] says what it owns (the part whose result is written) and what is read for it
//! (that part, with the context around it, clipped to the data). The engine reads the sources at
//! `read`, runs every node on that, and the domain trims back to `own` before writing.

use std::ops::Range;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::Graph;
use crate::types::{Context, PortType};

/// A box in the app's coordinates: up to three axes (lucem z, y, x; dsp-kitchen uses the last
/// for time and the others not at all).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Region {
    /// Where it starts, per axis.
    pub start: [usize; 3],
    /// Its size, per axis.
    pub size: [usize; 3],
}

impl Region {
    /// Its end (exclusive), per axis.
    pub fn end(&self) -> [usize; 3] {
        [0, 1, 2].map(|a| self.start[a] + self.size[a])
    }

    /// How many elements it holds.
    pub fn len(&self) -> usize {
        self.size.iter().product()
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One step of the walk: what it owns, and what is read for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Piece {
    /// Its number in the walk (0, 1, …).
    pub index: usize,
    /// The part whose result it writes.
    pub own: Region,
    /// The part read: `own` and the context around it, clipped to the data.
    pub read: Region,
}

impl Piece {
    /// A piece that is the whole of data of `shape`.
    pub fn whole(shape: [usize; 3]) -> Self {
        let r = Region { start: [0; 3], size: shape };
        Piece { index: 0, own: r, read: r }
    }

    /// Where `own` sits inside `read`, per axis (to trim a result back).
    pub fn inner(&self) -> [Range<usize>; 3] {
        [0, 1, 2].map(|a| {
            let from = self.own.start[a] - self.read.start[a];
            from..from + self.own.size[a]
        })
    }
}

/// What an app gives the engine. `Data` is what flows on an edge for one piece: lucem's image
/// block, dsp-kitchen's window; it is cloned when one output feeds several inputs (make it cheap
/// to clone: an `Arc`, a device handle).
pub trait Domain {
    /// What flows on an edge.
    type Data: Clone;

    /// The app's name (`lucem`), as graphs record it.
    fn name(&self) -> &str;

    /// The port types its data comes in.
    fn port_types(&self) -> Vec<PortType>;

    /// The walk over the graph's sources, each piece read with `context` around it. One piece is
    /// fine for small data or whole-table work.
    fn pieces(&self, graph: &Graph, context: Context) -> crate::Result<Vec<Piece>>;

    /// The source `name` (of type `ty`) over `piece.read`.
    fn read(&self, name: &str, ty: &PortType, piece: &Piece) -> crate::Result<Self::Data>;

    /// `data`, which covers `piece.read`, cut back to `piece.own`.
    fn trim(&self, data: Self::Data, ty: &PortType, piece: &Piece) -> crate::Result<Self::Data>;

    /// Writes the output `name`'s part for `piece` (already trimmed).
    fn write(&mut self, name: &str, ty: &PortType, piece: &Piece, data: Self::Data) -> crate::Result<()>;

    /// After the last piece: close the outputs (join labels, write metadata).
    fn finish(&mut self) -> crate::Result<()> {
        Ok(())
    }

    /// How scripts receive and return this data, if the app runs scripts ([`Exchange`]).
    fn exchange(&self) -> Option<&dyn Exchange<Self::Data>> {
        None
    }
}

/// A file handed to or taken from a script.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeFile {
    /// The file's name, in the call's folder (`image.npy`).
    pub file: String,
    /// `npy` (arrays: float32 images, uint32 masks and labels, float32 `[channels, samples]`
    /// windows), `csv` (tables) or `json` (anything else).
    pub format: String,
}

/// How an app's data travels to a script and back: as files in a folder in memory
/// (`/dev/shm`), so the script maps them without a copy.
pub trait Exchange<Data> {
    /// Writes `data` (of type `ty`) into `dir` as `name.<format>`.
    fn put(&self, data: &Data, ty: &PortType, dir: &Path, name: &str) -> crate::Result<ExchangeFile>;

    /// Reads what a script returned for an output of type `ty`. `like` is the call's first input
    /// (a script's array result takes its place and region).
    fn take(&self, file: &ExchangeFile, ty: &PortType, dir: &Path, like: Option<&Data>) -> crate::Result<Data>;
}

/// The windows of a recording `len` samples long, `size` each, with the time context (the last
/// axis of `context`) around each, clipped to the recording (dsp-kitchen; calcium imaging over
/// frames).
pub fn windows(len: usize, size: usize, context: Context) -> Vec<Piece> {
    let size = size.max(1);
    let (before, after) = (context.before[2], context.after[2]);
    let mut out = Vec::new();
    let mut start = 0;
    while start < len {
        let n = size.min(len - start);
        let read_start = start.saturating_sub(before);
        let read_end = (start + n + after).min(len);
        out.push(Piece {
            index: out.len(),
            own: Region { start: [0, 0, start], size: [1, 1, n] },
            read: Region { start: [0, 0, read_start], size: [1, 1, read_end - read_start] },
        });
        start += n;
    }
    out
}

/// The tiles of an image of `shape` (z, y, x), `block` each, with `context` around each, clipped to
/// the image (lucem). A 2-D image is `[1, h, w]` with a block of depth 1.
pub fn tiles(shape: [usize; 3], block: [usize; 3], context: Context) -> Vec<Piece> {
    let block = block.map(|b| b.max(1));
    let steps = |a: usize| -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        let mut s = 0;
        while s < shape[a] {
            let n = block[a].min(shape[a] - s);
            v.push((s, n));
            s += n;
        }
        v
    };
    let (zs, ys, xs) = (steps(0), steps(1), steps(2));
    let mut out = Vec::new();
    for &(z, dz) in &zs {
        for &(y, dy) in &ys {
            for &(x, dx) in &xs {
                let own = Region { start: [z, y, x], size: [dz, dy, dx] };
                let mut read = own;
                for a in 0..3 {
                    let s = own.start[a].saturating_sub(context.before[a]);
                    let e = (own.start[a] + own.size[a] + context.after[a]).min(shape[a]);
                    read.start[a] = s;
                    read.size[a] = e - s;
                }
                out.push(Piece { index: out.len(), own, read });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cover_the_recording_once_with_context_around() {
        let w = windows(1000, 300, Context::time(50, 20));
        assert_eq!(w.len(), 4);
        assert_eq!(w.iter().map(|p| p.own.size[2]).sum::<usize>(), 1000, "owned once");
        assert_eq!((w[0].read.start[2], w[0].read.size[2]), (0, 320), "clipped at the start");
        assert_eq!((w[1].read.start[2], w[1].read.end()[2]), (250, 620));
        assert_eq!((w[3].own.start[2], w[3].own.size[2], w[3].read.end()[2]), (900, 100, 1000), "clipped at the end");
        assert_eq!(w[1].inner()[2], 50..350);
        assert!(windows(0, 300, Context::NONE).is_empty());
    }

    #[test]
    fn tiles_cover_the_image_once_with_a_margin() {
        let t = tiles([1, 1000, 700], [1, 512, 512], Context::around(12, 2));
        assert_eq!(t.len(), 4);
        assert_eq!(t.iter().map(|p| p.own.len()).sum::<usize>(), 700_000, "owned once");
        let last = t[3];
        assert_eq!(last.own.start, [0, 512, 512]);
        assert_eq!(last.read.start, [0, 500, 500]);
        assert_eq!(last.read.end(), [1, 1000, 700], "clipped to the image");
        assert_eq!(last.inner(), [0..1, 12..500, 12..200]);
        // A margin in z on a single plane is clipped away
        let one = tiles([1, 10, 10], [1, 10, 10], Context::around(3, 3));
        assert_eq!(one[0].read, one[0].own);
    }
}
