//! A folder of frames: what high-speed and machine-vision cameras write (one PNG, JPEG or TIFF
//! per frame). Frames are ordered by the number in their names (`frame_9` before `frame_10`).
//! Their times come from `timestamps.csv` in the folder when there is one (one time in seconds
//! per line, or `index,time`), else from a frame rate.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::video::{Frame, FrameTimes, VideoError, VideoInfo, VideoResult, VideoSource};

const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "tif", "tiff", "bmp"];

/// The timestamp file read beside the frames.
pub const TIMESTAMPS: &str = "timestamps.csv";

/// An image sequence.
pub struct Sequence {
    info: VideoInfo,
    files: Vec<PathBuf>,
}

impl Sequence {
    /// The frames in `dir`; `rate` is used when the folder has no timestamps (30 when none is
    /// given either).
    pub fn open(dir: &Path, rate: Option<f64>) -> VideoResult<Self> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| VideoError::io(dir, e))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str())))
            .collect();
        files.sort_by(|a, b| natural_key(a).cmp(&natural_key(b)));
        let first = files.first().ok_or_else(|| VideoError::Unsupported { path: dir.to_path_buf(), hint: "no PNG, JPEG or TIFF frames in this folder".into() })?;
        let (width, height) = image::image_dimensions(first).map_err(|e| VideoError::io(first, e))?;
        let times = match read_timestamps(&dir.join(TIMESTAMPS))? {
            Some(t) if t.len() == files.len() => FrameTimes::from_times(t),
            Some(t) => {
                return Err(VideoError::Unsupported {
                    path: dir.join(TIMESTAMPS),
                    hint: format!("{} timestamps for {} frames: one per frame", t.len(), files.len()),
                });
            }
            None => FrameTimes::constant(files.len(), rate.unwrap_or(30.0), 0.0),
        };
        let ext = first.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
        Ok(Self { info: VideoInfo { path: dir.to_path_buf(), width, height, times, format: format!("{ext} sequence") }, files })
    }
}

impl VideoSource for Sequence {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn frame(&mut self, index: usize) -> VideoResult<Frame> {
        let path = self.files.get(index).ok_or(VideoError::NoFrame { index, frames: self.files.len() })?;
        let image = image::open(path).map_err(|e| VideoError::io(path, e))?.into_rgba8();
        let (width, height) = image.dimensions();
        Ok(Frame { index, time: self.info.times.time(index).unwrap_or(0.0), width, height, rgba: Arc::new(image.into_raw()) })
    }
}

/// Timestamps from `path`: one number a line, or `index,time` (a header line is skipped). None
/// when there is no file.
fn read_timestamps(path: &Path) -> VideoResult<Option<Vec<f64>>> {
    let Ok(text) = std::fs::read_to_string(path) else { return Ok(None) };
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let last = line.split([',', '\t', ' ']).filter(|f| !f.is_empty()).last().unwrap_or("");
        match last.parse::<f64>() {
            Ok(t) => out.push(t),
            Err(_) if i == 0 || line.trim().is_empty() => {}
            Err(_) => return Err(VideoError::Unsupported { path: path.to_path_buf(), hint: format!("line {}: not a time in seconds", i + 1) }),
        }
    }
    Ok(Some(out))
}

/// A name's sort key: its text with every run of digits compared as a number.
fn natural_key(path: &Path) -> Vec<(String, u64)> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut key = Vec::new();
    let (mut text, mut digits) = (String::new(), String::new());
    for c in name.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            if !digits.is_empty() {
                key.push((std::mem::take(&mut text), digits.parse().unwrap_or(u64::MAX)));
                digits.clear();
            }
            text.push(c);
        }
    }
    key.push((text, digits.parse().unwrap_or(0)));
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_sort_by_their_numbers_and_take_their_timestamps() {
        let dir = std::env::temp_dir().join(format!("kitchen-views-seq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in [1, 2, 10] {
            image::RgbImage::from_pixel(4, 3, image::Rgb([i as u8, 0, 0])).save(dir.join(format!("frame_{i}.png"))).unwrap();
        }
        let mut s = Sequence::open(&dir, Some(100.0)).unwrap();
        assert_eq!((s.info().width, s.info().height, s.info().frames()), (4, 3, 3));
        assert_eq!(s.frame(2).unwrap().rgba[0], 10, "frame_10 is last");
        assert!((s.info().times.time(1).unwrap() - 0.01).abs() < 1e-12);
        std::fs::write(dir.join(TIMESTAMPS), "index,time\n0,5.0\n1,5.02\n2,5.05\n").unwrap();
        let s = Sequence::open(&dir, None).unwrap();
        assert_eq!(s.info().times.time(2), Some(5.05));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
