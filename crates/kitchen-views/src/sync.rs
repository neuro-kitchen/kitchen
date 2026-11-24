//! Aligning clocks. A camera, an ephys probe and a behaviour rig each count time on their own
//! oscillator: the same moment reads differently on each, and the difference drifts (tens of
//! parts per million is common). Rigs share **sync pulses**: one TTL line recorded by every
//! device. The times of the same pulses on two clocks give the map between them, a line:
//!
//! `t_reference = slope · t_source + offset`
//!
//! [`fit`] finds it from the two lists of pulse times, without assuming they start together or
//! that no pulse was lost: the pulses are paired by their intervals (irregular intervals, a
//! "barcode", pair unambiguously; perfectly regular pulses are paired from the first one), the
//! line is fitted by least squares, pulses that sit off it are dropped, and the line is fitted
//! again. The [`Alignment`] keeps how good it is (residuals), so a view can say "aligned to
//! 0.2 ms" or warn.
//!
//! Plain data (serde): an alignment is saved as an artifact beside the sources, with its
//! provenance, and every app and the orchestrator use the same one.

use serde::{Deserialize, Serialize};

/// A map from a source's clock to the reference clock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    pub slope: f64,
    pub offset: f64,
    /// Pulses the line was fitted on (0: set by hand).
    pub pulses: usize,
    /// Root mean square of the residuals of those pulses, in seconds.
    pub rms: f64,
    /// The largest residual, in seconds.
    pub max: f64,
    /// The pulses were regular, so they were paired from the first one: right only when both
    /// recordings saw that first pulse.
    pub ambiguous: bool,
}

impl Alignment {
    /// Same clock.
    pub fn identity() -> Self {
        Self::offset(0.0)
    }

    /// A constant shift (`t_reference = t_source + seconds`), set by hand.
    pub fn offset(seconds: f64) -> Self {
        Self { slope: 1.0, offset: seconds, pulses: 0, rms: 0.0, max: 0.0, ambiguous: false }
    }

    /// A source time on the reference clock.
    pub fn to_reference(&self, t: f64) -> f64 {
        self.slope * t + self.offset
    }

    /// A reference time on the source's clock.
    pub fn to_source(&self, t: f64) -> f64 {
        (t - self.offset) / self.slope
    }

    /// The drift, in parts per million (`+20`: the source's clock runs 20 ppm slow).
    pub fn drift_ppm(&self) -> f64 {
        (self.slope - 1.0) * 1e6
    }
}

/// Why clocks could not be aligned.
#[derive(Clone, Debug, PartialEq)]
pub enum SyncError {
    /// Fewer than two pulses in one of the lists.
    TooFewPulses { source: usize, reference: usize },
    /// No pairing of the pulses agrees with a line.
    NoMatch,
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::TooFewPulses { source, reference } => write!(f, "too few sync pulses to align: {source} in the source, {reference} in the reference (at least 2 each)"),
            SyncError::NoMatch => write!(f, "the sync pulses of the two recordings do not line up: are they the same TTL line?"),
        }
    }
}

impl std::error::Error for SyncError {}

/// The rising edges of a digital line sampled at `rate` Hz, in seconds: where it goes from
/// `false` to `true`. `start` is the time of the first sample.
pub fn rising_edges(line: &[bool], rate: f64, start: f64) -> Vec<f64> {
    line.windows(2).enumerate().filter(|(_, w)| !w[0] && w[1]).map(|(i, _)| start + (i + 1) as f64 / rate).collect()
}

/// The rising edges of an analog channel crossing `threshold` upwards.
pub fn threshold_edges(samples: &[f32], threshold: f32, rate: f64, start: f64) -> Vec<f64> {
    let line: Vec<bool> = samples.iter().map(|&v| v > threshold).collect();
    rising_edges(&line, rate, start)
}

/// The map from `source` pulse times to `reference` pulse times (both sorted, seconds).
pub fn fit(source: &[f64], reference: &[f64]) -> Result<Alignment, SyncError> {
    if source.len() < 2 || reference.len() < 2 {
        return Err(SyncError::TooFewPulses { source: source.len(), reference: reference.len() });
    }
    let (shift, ambiguous) = interval_shift(source, reference);
    // A first line from the pulses paired by the shift whose intervals agree (a pulse lost on
    // one side shifts the pairs after it: those intervals disagree and are left out)
    let paired = |i: usize| {
        let j = i as isize + shift;
        (j >= 0 && (j as usize) < reference.len()).then(|| j as usize)
    };
    let mut pairs = Vec::new();
    for i in 0..source.len() - 1 {
        let (Some(j), Some(k)) = (paired(i), paired(i + 1)) else { continue };
        let (da, db) = (source[i + 1] - source[i], reference[k] - reference[j]);
        if (da - db).abs() <= (0.01 * db.abs()).max(1e-3) {
            pairs.push((source[i], reference[j]));
            pairs.push((source[i + 1], reference[k]));
        }
    }
    pairs.dedup();
    let first = least_squares(&pairs).ok_or(SyncError::NoMatch)?;
    // Paired again by nearness on that line (lost pulses drop out), outliers dropped, refitted
    let tolerance = 0.25 * median_interval(reference);
    let near = pair_near(source, reference, &first, tolerance);
    if near.len() < 2 {
        return Err(SyncError::NoMatch);
    }
    let line = least_squares(&near).ok_or(SyncError::NoMatch)?;
    let kept = drop_outliers(&near, &line);
    let line = least_squares(&kept).unwrap_or(line);
    let residuals: Vec<f64> = kept.iter().map(|&(s, r)| (line.0 * s + line.1 - r).abs()).collect();
    let rms = (residuals.iter().map(|r| r * r).sum::<f64>() / residuals.len() as f64).sqrt();
    let max = residuals.iter().copied().fold(0.0, f64::max);
    Ok(Alignment { slope: line.0, offset: line.1, pulses: kept.len(), rms, max, ambiguous })
}

/// The index shift `k` pairing `source[i]` with `reference[i + k]` whose intervals agree best,
/// and whether the intervals were too regular to tell (then `k` pairs the first pulses).
fn interval_shift(source: &[f64], reference: &[f64]) -> (isize, bool) {
    let a: Vec<f64> = source.windows(2).map(|w| w[1] - w[0]).collect();
    let b: Vec<f64> = reference.windows(2).map(|w| w[1] - w[0]).collect();
    let regular = |v: &[f64]| {
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
        sd < 0.01 * mean.abs()
    };
    if a.len() < 3 || b.len() < 3 || (regular(&a) && regular(&b)) {
        return (0, true);
    }
    let min_overlap = a.len().min(b.len()).min(5).max(2);
    // The best agreement; among equal ones (a repeating pattern) the longest overlap
    let mut best = (0isize, f64::INFINITY, 0usize);
    for k in -(a.len() as isize - 1)..(b.len() as isize) {
        let (mut sum, mut n) = (0.0, 0usize);
        for (i, x) in a.iter().enumerate() {
            let j = i as isize + k;
            if j >= 0 && (j as usize) < b.len() {
                sum += (x - b[j as usize]).abs();
                n += 1;
            }
        }
        if n < min_overlap {
            continue;
        }
        let score = sum / n as f64;
        if score < best.1 - 1e-6 || (score <= best.1 + 1e-6 && n > best.2) {
            best = (k, score, n);
        }
    }
    (best.0, false)
}

/// For each source pulse, the reference pulse nearest where the line puts it, within `tolerance`.
fn pair_near(source: &[f64], reference: &[f64], line: &(f64, f64), tolerance: f64) -> Vec<(f64, f64)> {
    source
        .iter()
        .filter_map(|&s| {
            let t = line.0 * s + line.1;
            let i = reference.partition_point(|&r| r < t);
            let candidates = [i.checked_sub(1), Some(i)];
            let nearest = candidates.into_iter().flatten().filter_map(|j| reference.get(j)).min_by(|x, y| (*x - t).abs().total_cmp(&(*y - t).abs()))?;
            ((nearest - t).abs() <= tolerance).then_some((s, *nearest))
        })
        .collect()
}

/// The pairs within 5 median absolute deviations of the line (all of them when that leaves
/// fewer than 2).
fn drop_outliers(pairs: &[(f64, f64)], line: &(f64, f64)) -> Vec<(f64, f64)> {
    let mut res: Vec<f64> = pairs.iter().map(|&(s, r)| (line.0 * s + line.1 - r).abs()).collect();
    res.sort_by(f64::total_cmp);
    let mad = res[res.len() / 2].max(1e-9);
    let kept: Vec<(f64, f64)> = pairs.iter().copied().filter(|&(s, r)| (line.0 * s + line.1 - r).abs() <= 5.0 * mad).collect();
    if kept.len() >= 2 { kept } else { pairs.to_vec() }
}

/// Slope and offset of `r = slope · s + offset`, least squares (centred, for precision with
/// large times).
fn least_squares(pairs: &[(f64, f64)]) -> Option<(f64, f64)> {
    if pairs.len() < 2 {
        return None;
    }
    let n = pairs.len() as f64;
    let ms = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let mr = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pairs.iter().map(|p| (p.0 - ms).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = pairs.iter().map(|p| (p.0 - ms) * (p.1 - mr)).sum();
    let slope = sxy / sxx;
    Some((slope, mr - slope * ms))
}

fn median_interval(t: &[f64]) -> f64 {
    let mut d: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).collect();
    d.sort_by(f64::total_cmp);
    d[d.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pulses at irregular intervals (a barcode: 0.5 to 1.4 s, pseudo-random), seconds.
    fn barcode(n: usize) -> Vec<f64> {
        let (mut t, mut state) = (3.0, 12345u64);
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                t += 0.5 + ((state >> 33) % 1000) as f64 * 0.0009;
                t
            })
            .collect()
    }

    #[test]
    fn aligns_drifting_clocks_with_lost_pulses() {
        let src = barcode(60);
        // The reference runs 25 ppm faster and started 12.3 s earlier; it missed pulse 17, and
        // the source missed its first four
        let mut reference: Vec<f64> = src.iter().map(|&s| 1.000_025 * s + 12.3).collect();
        reference.remove(17);
        let a = fit(&src[4..], &reference).unwrap();
        assert!((a.slope - 1.000_025).abs() < 1e-9, "{a:?}");
        assert!((a.offset - 12.3).abs() < 1e-6, "{a:?}");
        assert!(!a.ambiguous && a.pulses >= 54 && a.max < 1e-9);
        assert!((a.drift_ppm() - 25.0).abs() < 1e-3);
        assert!((a.to_source(a.to_reference(100.0)) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn regular_pulses_pair_from_the_first_and_say_so() {
        let src: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let reference: Vec<f64> = src.iter().map(|s| s + 0.4).collect();
        let a = fit(&src, &reference).unwrap();
        assert!(a.ambiguous);
        assert!((a.offset - 0.4).abs() < 1e-9);
        assert!(matches!(fit(&[1.0], &reference), Err(SyncError::TooFewPulses { .. })));
    }

    #[test]
    fn edges_rise_where_the_line_goes_high() {
        let line = [false, false, true, true, false, true];
        assert_eq!(rising_edges(&line, 10.0, 1.0), vec![1.2, 1.5]);
        assert_eq!(threshold_edges(&[0.0, 3.0, 0.0], 1.5, 2.0, 0.0), vec![0.5]);
    }
}
