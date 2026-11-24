//! Video, for research: every frame where it was, at the time it was taken. What matters is not
//! smooth playback but **which frame** is shown at a given moment of the recording, so frames
//! are addressed by index and carry their timestamp ([`FrameTimes`]), dropped frames are found,
//! and the time is mapped onto the reference clock ([`crate::sync::Alignment`]).
//!
//! Sources ([`VideoSource`]), picked by [`open`]:
//! - an image sequence (a folder of PNG, JPEG or TIFF frames: high-speed cameras) — [`sequence`];
//! - MP4 with H.264, decoded in Rust (feature `h264`, OpenH264) — `mp4`;
//! - anything else (AVI with MJPEG, Norpix, MKV…) through the system's `ffmpeg`, run as a separate
//!   process: its licence stays outside this crate — [`ffmpeg`].
//!
//! In a browser the page decodes with WebCodecs (the hardware's decoder) and hands frames over;
//! that source lives with the web app's loader.

pub mod cache;
pub mod sequence;
pub mod times;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use cache::FrameCache;
pub use times::FrameTimes;

/// One decoded frame: RGBA, 8 bits a channel, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub index: usize,
    /// On the video's own clock, in seconds.
    pub time: f64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
}

impl Frame {
    /// Bytes it holds (for the cache's budget).
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }
}

/// What a video is.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    /// Each frame's timestamp, on the video's own clock.
    pub times: FrameTimes,
    /// The format, as people say it (`H.264 in MP4`, `PNG sequence`).
    pub format: String,
}

impl VideoInfo {
    pub fn frames(&self) -> usize {
        self.times.len()
    }
}

/// Why a video could not be read.
#[derive(Clone, Debug, PartialEq)]
pub enum VideoError {
    /// A file could not be opened or read.
    Io { path: PathBuf, message: String },
    /// The file is not a video this build reads; how to read it.
    Unsupported { path: PathBuf, hint: String },
    /// A frame index past the end.
    NoFrame { index: usize, frames: usize },
    /// The decoder failed on a frame.
    Decode(String),
}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VideoError::Io { path, message } => write!(f, "{}: {message}", path.display()),
            VideoError::Unsupported { path, hint } => write!(f, "{}: {hint}", path.display()),
            VideoError::NoFrame { index, frames } => write!(f, "frame {index} is past the end ({frames} frames)"),
            VideoError::Decode(m) => write!(f, "decoding: {m}"),
        }
    }
}

impl std::error::Error for VideoError {}

impl VideoError {
    pub(crate) fn io(path: &Path, e: impl std::fmt::Display) -> Self {
        Self::Io { path: path.to_path_buf(), message: e.to_string() }
    }
}

pub type VideoResult<T> = Result<T, VideoError>;

/// A video's frames, by index. Decoding is blocking: views call it off the UI thread.
pub trait VideoSource: Send {
    fn info(&self) -> &VideoInfo;

    /// Frame `index`, decoded.
    fn frame(&mut self, index: usize) -> VideoResult<Frame>;
}

/// Opens the video at `path`: a folder of images, an MP4 (decoded in Rust when built with
/// `h264`), or anything `ffmpeg` reads. `rate` is the frame rate of an image sequence without
/// timestamps (ignored for files, which carry theirs).
pub fn open(path: &Path, rate: Option<f64>) -> VideoResult<Box<dyn VideoSource>> {
    if path.is_dir() {
        return Ok(Box::new(sequence::Sequence::open(path, rate)?));
    }
    Err(VideoError::Unsupported { path: path.to_path_buf(), hint: "only image sequences (a folder of frames) are read so far".into() })
}

/// RGB (3 bytes a pixel) as RGBA, opaque.
pub(crate) fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
    for px in rgb.chunks_exact(3) {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}
