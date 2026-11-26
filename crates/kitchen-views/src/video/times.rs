//! When each frame was taken. Cameras are not metronomes: a frame is dropped under load, a
//! variable-rate encoder spaces frames unevenly, a trigger starts late. Frames are kept with
//! their own timestamps, and the frame shown at a time is the last one taken at or before it.

/// Each frame's timestamp, in seconds of the video's own clock, increasing.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameTimes {
    times: Vec<f64>,
}

/// A run of frames missing between two that were kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    /// The frame after the gap.
    pub after: usize,
    /// About how many frames are missing.
    pub missing: usize,
    /// The gap's length, in seconds.
    pub seconds: f64,
}

impl FrameTimes {
    /// `frames` frames at a constant `rate` (frames per second), from `start`.
    pub fn constant(frames: usize, rate: f64, start: f64) -> Self {
        let rate = if rate.is_finite() && rate > 0.0 { rate } else { 30.0 };
        Self { times: (0..frames).map(|i| start + i as f64 / rate).collect() }
    }

    /// Timestamps as given (sorted when they are not: an encoder's presentation order).
    pub fn from_times(mut times: Vec<f64>) -> Self {
        times.retain(|t| t.is_finite());
        times.sort_by(f64::total_cmp);
        Self { times }
    }

    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Frame `index`'s timestamp.
    pub fn time(&self, index: usize) -> Option<f64> {
        self.times.get(index).copied()
    }

    /// The first and last timestamps.
    pub fn span(&self) -> Option<(f64, f64)> {
        Some((*self.times.first()?, *self.times.last()?))
    }

    /// The frame shown at `t`: the last taken at or before it (the first before the video
    /// starts, the last after it ends); none for an empty video.
    pub fn frame_at(&self, t: f64) -> Option<usize> {
        if self.times.is_empty() {
            return None;
        }
        Some(self.times.partition_point(|&x| x <= t).saturating_sub(1))
    }

    /// The typical interval between frames (the median), in seconds.
    pub fn median_interval(&self) -> Option<f64> {
        let mut d: Vec<f64> = self.times.windows(2).map(|w| w[1] - w[0]).filter(|d| *d > 0.0).collect();
        if d.is_empty() {
            return None;
        }
        d.sort_by(f64::total_cmp);
        Some(d[d.len() / 2])
    }

    /// The frame rate from the median interval.
    pub fn rate(&self) -> Option<f64> {
        self.median_interval().map(|d| 1.0 / d)
    }

    /// Where frames are missing: intervals over 1.5 times the median.
    pub fn gaps(&self) -> Vec<Gap> {
        let Some(m) = self.median_interval() else { return Vec::new() };
        self.times
            .windows(2)
            .enumerate()
            .filter_map(|(i, w)| {
                let d = w[1] - w[0];
                (d > 1.5 * m).then(|| Gap { after: i + 1, missing: ((d / m).round() as usize).saturating_sub(1).max(1), seconds: d })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_found_by_time_and_gaps_are_seen() {
        // 100 fps with frames 5 and 6 dropped
        let times: Vec<f64> = (0..20).filter(|i| *i != 5 && *i != 6).map(|i| i as f64 * 0.01).collect();
        let t = FrameTimes::from_times(times);
        assert_eq!(t.len(), 18);
        assert_eq!(t.frame_at(-1.0), Some(0));
        assert_eq!(t.frame_at(0.045), Some(4));
        assert_eq!(t.frame_at(0.065), Some(4), "frames 5 and 6 are missing: frame 4 is still shown");
        assert_eq!(t.frame_at(0.07), Some(5));
        assert_eq!(t.frame_at(99.0), Some(17));
        assert!((t.rate().unwrap() - 100.0).abs() < 1e-6);
        assert_eq!(t.gaps(), vec![Gap { after: 5, missing: 2, seconds: t.time(5).unwrap() - t.time(4).unwrap() }]);
        assert_eq!(FrameTimes::constant(3, 50.0, 1.0).span(), Some((1.0, 1.04)));
    }
}
