//! Decoded frames kept for scrubbing back and forth: least recently used first out, within a
//! byte budget (a 1080p RGBA frame is 8 MB: 64 of them is half a gigabyte).

use std::collections::VecDeque;

use crate::video::Frame;

/// Frames by index, newest use last.
#[derive(Debug)]
pub struct FrameCache {
    frames: VecDeque<Frame>,
    budget: usize,
    used: usize,
}

impl FrameCache {
    /// A cache of up to `budget` bytes.
    pub fn new(budget: usize) -> Self {
        Self { frames: VecDeque::new(), budget, used: 0 }
    }

    /// Frame `index`, when kept (it becomes the most recently used).
    pub fn get(&mut self, index: usize) -> Option<Frame> {
        let at = self.frames.iter().position(|f| f.index == index)?;
        let frame = self.frames.remove(at)?;
        self.frames.push_back(frame.clone());
        Some(frame)
    }

    /// Whether frame `index` is kept.
    pub fn contains(&self, index: usize) -> bool {
        self.frames.iter().any(|f| f.index == index)
    }

    /// Keeps `frame`, dropping the least recently used to stay within the budget (a frame larger
    /// than the budget is kept alone).
    pub fn put(&mut self, frame: Frame) {
        if let Some(at) = self.frames.iter().position(|f| f.index == frame.index) {
            let old = self.frames.remove(at).expect("found");
            self.used -= old.bytes();
        }
        self.used += frame.bytes();
        self.frames.push_back(frame);
        while self.used > self.budget && self.frames.len() > 1 {
            let old = self.frames.pop_front().expect("more than one");
            self.used -= old.bytes();
        }
    }

    /// Bytes held.
    pub fn used(&self) -> usize {
        self.used
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.used = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn frame(index: usize) -> Frame {
        Frame { index, time: index as f64, width: 2, height: 2, rgba: Arc::new(vec![0; 16]) }
    }

    #[test]
    fn keeps_the_recently_used_within_its_budget() {
        let mut c = FrameCache::new(48);
        for i in 0..3 {
            c.put(frame(i));
        }
        assert!(c.get(0).is_some(), "0 is now the most recent");
        c.put(frame(3));
        assert!(!c.contains(1), "the least recently used went");
        assert!(c.contains(0) && c.contains(2) && c.contains(3));
        assert_eq!(c.used(), 48);
    }
}
