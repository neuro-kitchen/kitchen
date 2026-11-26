//! Any video `ffmpeg` reads (AVI with MJPEG, MKV, HEVC in MP4, Norpix `.seq`…), through the
//! system's `ffprobe` and `ffmpeg` run as separate processes: their licence (LGPL or GPL) stays
//! with them, outside this crate, as kitchen's isolated-licence rule asks.
//!
//! - The timestamps come from the packets' presentation times (`ffprobe`), sorted: every frame
//!   at the time it was stamped, variable rates and gaps kept.
//! - Frames are decoded by an `ffmpeg` reading from a time and writing raw RGBA to a pipe. While
//!   frames are asked for in order (playback), the same process goes on; a jump starts another
//!   at the new time (input seeking: from the nearest keyframe, decoded up to the frame, exact
//!   to the frame).

use std::io::Read as _;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, OnceLock};

use crate::video::{Frame, FrameTimes, VideoError, VideoInfo, VideoResult, VideoSource};

/// Whether `ffmpeg` and `ffprobe` are on the `PATH` (asked once).
pub fn available() -> bool {
    static FOUND: OnceLock<bool> = OnceLock::new();
    *FOUND.get_or_init(|| {
        let runs = |p: &str| Command::new(p).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
        runs("ffmpeg") && runs("ffprobe")
    })
}

/// A video decoded by `ffmpeg`.
pub struct Ffmpeg {
    info: VideoInfo,
    /// The first frame's presentation time (seeking is from the file's start).
    start: f64,
    reader: Option<Reader>,
}

/// An `ffmpeg` writing frames from `next` on.
struct Reader {
    child: Child,
    out: ChildStdout,
    next: usize,
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Ffmpeg {
    pub fn open(path: &Path) -> VideoResult<Self> {
        let stream = probe(path, &["-show_entries", "stream=width,height,codec_long_name,avg_frame_rate,nb_frames", "-of", "default=nw=1"])?;
        let field = |name: &str| stream.lines().find_map(|l| l.strip_prefix(&format!("{name}=")).map(str::to_string));
        let width: u32 = field("width").and_then(|v| v.parse().ok()).ok_or_else(|| unsupported(path, "no video stream"))?;
        let height: u32 = field("height").and_then(|v| v.parse().ok()).ok_or_else(|| unsupported(path, "no video stream"))?;
        let packets = probe(path, &["-show_entries", "packet=pts_time", "-of", "csv=p=0"])?;
        let mut stamps: Vec<f64> = packets.lines().filter_map(|l| l.trim().trim_end_matches(',').parse().ok()).collect();
        if stamps.is_empty() {
            // No packet times (some raw streams): the stream's rate and count
            let rate = field("avg_frame_rate").and_then(|r| parse_rate(&r)).unwrap_or(30.0);
            let n: usize = field("nb_frames").and_then(|v| v.parse().ok()).unwrap_or(0);
            stamps = (0..n).map(|i| i as f64 / rate).collect();
        }
        let times = FrameTimes::from_times(stamps);
        let start = times.time(0).unwrap_or(0.0);
        let codec = field("codec_long_name").unwrap_or_else(|| "video".into());
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
        let format = format!("{codec} in {ext} (ffmpeg)");
        Ok(Self { info: VideoInfo { path: path.to_path_buf(), width, height, times, format }, start, reader: None })
    }

    /// An `ffmpeg` writing raw RGBA frames from frame `index` on.
    fn start_at(&self, index: usize) -> VideoResult<Reader> {
        let t = self.info.times.time(index).ok_or(VideoError::NoFrame { index, frames: self.info.frames() })?;
        // A quarter frame early: the frame stamped at t is the first written
        let quarter = self.info.times.median_interval().unwrap_or(1.0 / 30.0) / 4.0;
        let seek = (t - self.start - quarter).max(0.0);
        let path = &self.info.path;
        let mut child = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", &format!("{seek:.6}"), "-i"])
            .arg(path)
            .args(["-map", "0:v:0", "-vsync", "0", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| VideoError::io(path, format!("ffmpeg: {e}")))?;
        let out = child.stdout.take().expect("piped");
        Ok(Reader { child, out, next: index })
    }
}

impl VideoSource for Ffmpeg {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn frame(&mut self, index: usize) -> VideoResult<Frame> {
        if index >= self.info.frames() {
            return Err(VideoError::NoFrame { index, frames: self.info.frames() });
        }
        // Going on in order; a jump back, or far ahead, starts another reader
        let reuse = self.reader.as_ref().is_some_and(|r| r.next <= index && index - r.next < 30);
        if !reuse {
            self.reader = Some(self.start_at(index)?);
        }
        let (w, h) = (self.info.width, self.info.height);
        let reader = self.reader.as_mut().expect("started");
        let mut rgba = vec![0u8; w as usize * h as usize * 4];
        while reader.next <= index {
            if let Err(e) = reader.out.read_exact(&mut rgba) {
                self.reader = None;
                return Err(VideoError::Decode(format!("ffmpeg stopped before frame {index}: {e}")));
            }
            reader.next += 1;
        }
        Ok(Frame { index, time: self.info.times.time(index).unwrap_or(0.0), width: w, height: h, rgba: Arc::new(rgba) })
    }
}

fn probe(path: &Path, entries: &[&str]) -> VideoResult<String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0"])
        .args(entries)
        .arg(path)
        .output()
        .map_err(|e| VideoError::io(path, format!("ffprobe: {e}")))?;
    if !out.status.success() {
        return Err(unsupported(path, &format!("ffprobe cannot read it: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn unsupported(path: &Path, hint: &str) -> VideoError {
    VideoError::Unsupported { path: path.to_path_buf(), hint: hint.to_string() }
}

/// `30000/1001` as 29.97.
fn parse_rate(text: &str) -> Option<f64> {
    let (n, d) = text.split_once('/').unwrap_or((text, "1"));
    let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
    (d != 0.0 && n > 0.0).then(|| n / d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_read_as_ffprobe_writes_them() {
        assert!((parse_rate("30000/1001").unwrap() - 29.97).abs() < 1e-3);
        assert_eq!(parse_rate("25"), Some(25.0));
        assert_eq!(parse_rate("0/0"), None);
    }
}
