//! MP4 with H.264 decoded in Rust (feature `h264`): the `mp4` crate reads the container, Cisco's
//! OpenH264 (BSD) decodes. No ffmpeg needed for the most common camera recordings.
//!
//! Timestamps come from the container's tables (no frame is read to know them). Frames are
//! decoded from the nearest keyframe before the one asked for; while frames are asked for in
//! order, the decoder goes on. Streams with B-frames (frames stored out of display order) are
//! left to ffmpeg: [`Mp4H264::open`] says `Unsupported` and [`crate::video::open`] falls back.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use ::mp4::{MediaType, Mp4Reader, TrackType};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource as _;

use crate::video::{Frame, FrameTimes, VideoError, VideoInfo, VideoResult, VideoSource};

pub struct Mp4H264 {
    info: VideoInfo,
    reader: Mp4Reader<BufReader<File>>,
    track: u32,
    /// The parameter sets, Annex B, fed to a new decoder before its first keyframe.
    params: Vec<u8>,
    /// Bytes before each NAL unit's length (1, 2 or 4).
    length_size: usize,
    /// Sample numbers (1-based) of the keyframes, increasing.
    keyframes: Vec<u32>,
    decoder: Option<Decoder>,
    /// The last sample the decoder took.
    decoded: Option<u32>,
}

impl Mp4H264 {
    pub fn open(path: &Path) -> VideoResult<Self> {
        let file = File::open(path).map_err(|e| VideoError::io(path, e))?;
        let size = file.metadata().map_err(|e| VideoError::io(path, e))?.len();
        let reader = Mp4Reader::read_header(BufReader::new(file), size).map_err(|e| VideoError::io(path, e))?;
        let unsupported = |hint: &str| VideoError::Unsupported { path: path.to_path_buf(), hint: hint.into() };
        let (track_id, track) = reader
            .tracks()
            .iter()
            .find(|(_, t)| t.track_type().is_ok_and(|k| k == TrackType::Video))
            .map(|(id, t)| (*id, t))
            .ok_or_else(|| unsupported("no video track"))?;
        if !track.media_type().is_ok_and(|m| m == MediaType::H264) {
            return Err(unsupported("not H.264"));
        }
        let stbl = &track.trak.mdia.minf.stbl;
        if stbl.ctts.as_ref().is_some_and(|c| c.entries.iter().any(|e| e.sample_offset != 0)) {
            return Err(unsupported("H.264 with B-frames: read with ffmpeg"));
        }
        let avc = stbl.stsd.avc1.as_ref().ok_or_else(|| unsupported("no avc1 sample description"))?;
        let mut params = Vec::new();
        for nal in avc.avcc.sequence_parameter_sets.iter().chain(&avc.avcc.picture_parameter_sets) {
            params.extend_from_slice(&[0, 0, 0, 1]);
            params.extend_from_slice(&nal.bytes);
        }
        let length_size = avc.avcc.length_size_minus_one as usize % 4 + 1;
        // Times from the time-to-sample table: deltas in the track's timescale
        let timescale = f64::from(track.timescale().max(1));
        let mut times = Vec::with_capacity(track.sample_count() as usize);
        let mut t = 0u64;
        for e in &stbl.stts.entries {
            for _ in 0..e.sample_count {
                times.push(t as f64 / timescale);
                t += u64::from(e.sample_delta);
            }
        }
        let count = track.sample_count();
        let keyframes = match &stbl.stss {
            Some(s) => s.entries.clone(),
            None => (1..=count).collect(),
        };
        let info = VideoInfo {
            path: path.to_path_buf(),
            width: u32::from(track.width()),
            height: u32::from(track.height()),
            times: FrameTimes::from_times(times),
            format: "H.264 in MP4".into(),
        };
        Ok(Self { info, reader, track: track_id, params, length_size, keyframes, decoder: None, decoded: None })
    }

    /// Sample `n` (1-based) as Annex B: each NAL unit with a start code instead of its length.
    fn annex_b(&mut self, n: u32) -> VideoResult<Vec<u8>> {
        let sample = self.reader.read_sample(self.track, n).map_err(|e| VideoError::Decode(e.to_string()))?.ok_or(VideoError::NoFrame { index: n as usize - 1, frames: self.info.frames() })?;
        let (data, ls) = (&sample.bytes[..], self.length_size);
        let mut out = Vec::with_capacity(data.len() + 16);
        let mut i = 0;
        while i + ls <= data.len() {
            let len = data[i..i + ls].iter().fold(0usize, |acc, b| (acc << 8) | *b as usize);
            i += ls;
            let end = (i + len).min(data.len());
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(&data[i..end]);
            i = end;
        }
        Ok(out)
    }
}

impl VideoSource for Mp4H264 {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn frame(&mut self, index: usize) -> VideoResult<Frame> {
        let frames = self.info.frames();
        if index >= frames {
            return Err(VideoError::NoFrame { index, frames });
        }
        let target = index as u32 + 1;
        let key = self.keyframes[..self.keyframes.partition_point(|&k| k <= target)].last().copied().unwrap_or(1);
        // Going on from where the decoder is, when that is between the keyframe and the target
        let from = match self.decoded {
            Some(d) if self.decoder.is_some() && d >= key && d < target => d + 1,
            _ => {
                let mut decoder = Decoder::new().map_err(|e| VideoError::Decode(e.to_string()))?;
                let _ = decoder.decode(&self.params).map_err(|e| VideoError::Decode(e.to_string()))?;
                self.decoder = Some(decoder);
                key
            }
        };
        let mut rgba = Vec::new();
        let (mut width, mut height) = (self.info.width, self.info.height);
        for n in from..=target {
            let nal = self.annex_b(n)?;
            let decoder = self.decoder.as_mut().expect("made above");
            let picture = decoder.decode(&nal).map_err(|e| VideoError::Decode(format!("sample {n}: {e}")))?;
            self.decoded = Some(n);
            if n == target {
                let yuv = picture.ok_or_else(|| VideoError::Decode(format!("sample {n} gave no picture")))?;
                let (w, h) = yuv.dimensions();
                (width, height) = (w as u32, h as u32);
                rgba = vec![0u8; w * h * 4];
                yuv.write_rgba8(&mut rgba);
            }
        }
        Ok(Frame { index, time: self.info.times.time(index).unwrap_or(0.0), width, height, rgba: Arc::new(rgba) })
    }
}
