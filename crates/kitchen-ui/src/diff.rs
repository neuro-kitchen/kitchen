//! Line diffs of two texts, git style, for [`crate::widgets::DiffView`]: added and removed lines
//! with their old and new line numbers, grouped into hunks with a few unchanged lines of context
//! around each change; the unchanged runs between hunks fold away.
//!
//! [`Diff::json`] compares two JSON documents by key instead of by text: both are printed with
//! sorted keys first, so reordered keys are not noise, and a long array (cells, tiles, labels)
//! prints as one line, so a changed array reads as one summary line (`cells: 2,539 → 2,712`)
//! instead of thousands of numbers.
//!
//! Plain data: no UI here, so an app (or its tests) can compute a diff without a window.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};
use std::ops::Range;

use serde_json::Value;
use similar::{Algorithm, DiffOp, capture_diff_slices};

/// Unchanged lines shown around each change by default.
pub const CONTEXT: usize = 3;
/// Arrays longer than this print as one summary line in [`Diff::json`] by default.
pub const ARRAY_SUMMARY: usize = 16;

/// What a line of a diff is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// Unchanged.
    Context,
    /// Only in the new text.
    Added,
    /// Only in the old text.
    Removed,
    /// A long JSON array that changed, as one line (`cells: 2,539 → 2,712`).
    Summary,
}

/// One line of a diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    /// What the line is.
    pub kind: LineKind,
    /// Its line number in the old text (1-based), if it is there.
    pub old: Option<usize>,
    /// Its line number in the new text (1-based), if it is there.
    pub new: Option<usize>,
    /// The line, without its newline.
    pub text: String,
}

/// A run of changes with its context: [`Diff::lines`]`[lines]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// Its lines in [`Diff::lines`].
    pub lines: Range<usize>,
    /// First old line it covers (1-based; the line before it when it covers none).
    pub old_start: usize,
    /// Old lines it covers.
    pub old_len: usize,
    /// First new line it covers (1-based; the line before it when it covers none).
    pub new_start: usize,
    /// New lines it covers.
    pub new_len: usize,
}

impl Hunk {
    /// The unified-diff header: `@@ -12,7 +12,9 @@`.
    pub fn header(&self) -> String {
        format!("@@ -{},{} +{},{} @@", self.old_start, self.old_len, self.new_start, self.new_len)
    }
}

/// The diff of two texts: every line, and the hunks over them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Every line of both texts in order: unchanged, removed and added (and, for JSON, summary)
    /// lines.
    pub lines: Vec<DiffLine>,
    /// The changes with their context, in order. Lines outside every hunk are unchanged
    /// ([`Diff::gaps`]).
    pub hunks: Vec<Hunk>,
    /// Compared as JSON ([`Diff::json`]).
    pub json: bool,
    /// Why a JSON comparison fell back to text (a side did not parse).
    pub note: Option<String>,
}

impl Diff {
    /// The line diff of `old` and `new`, with `context` unchanged lines around each change.
    pub fn text(old: &str, new: &str, context: usize) -> Diff {
        let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
        let lines = walk(&a, &b, |i| a[i].to_string(), |i| b[i].to_string());
        let hunks = hunks(&lines, context);
        Diff { lines, hunks, json: false, note: None }
    }

    /// The diff of two JSON documents compared by key ([`Diff::json_values`]); when a side does
    /// not parse, the text diff with a [`Diff::note`] saying why.
    pub fn json(old: &str, new: &str, context: usize, array_summary: usize) -> Diff {
        let parse = |s: &str| if s.trim().is_empty() { Ok(Value::Null) } else { serde_json::from_str::<Value>(s) };
        match (parse(old), parse(new)) {
            (Ok(a), Ok(b)) => Diff::json_values(&a, &b, context, array_summary),
            (a, b) => {
                let which = if a.is_err() { "old" } else { "new" };
                let e = a.err().or(b.err()).map(|e| e.to_string()).unwrap_or_default();
                Diff { note: Some(format!("The {which} side is not JSON ({e}); compared as text.")), ..Diff::text(old, new, context) }
            }
        }
    }

    /// The diff of two JSON values: both printed with sorted keys (two-space indent), arrays
    /// longer than `array_summary` as one line each, then compared line by line. A changed long
    /// array becomes one [`LineKind::Summary`] line. Trailing commas are not compared, so adding
    /// a key at the end does not mark the line before it as changed.
    pub fn json_values(old: &Value, new: &Value, context: usize, array_summary: usize) -> Diff {
        let (a, b) = (print_json(old, array_summary), print_json(new, array_summary));
        let (ka, kb): (Vec<&str>, Vec<&str>) = (a.iter().map(|l| l.key.as_str()).collect(), b.iter().map(|l| l.key.as_str()).collect());
        let mut lines = walk(&ka, &kb, |i| a[i].display.clone(), |i| b[i].display.clone());
        merge_summaries(&mut lines, &a, &b);
        let hunks = hunks(&lines, context);
        Diff { lines, hunks, json: true, note: None }
    }

    /// Whether the texts are the same.
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    /// Lines added (summary lines count as one added and one removed).
    pub fn added(&self) -> usize {
        self.lines.iter().filter(|l| matches!(l.kind, LineKind::Added | LineKind::Summary)).count()
    }

    /// Lines removed (summary lines count as one added and one removed).
    pub fn removed(&self) -> usize {
        self.lines.iter().filter(|l| matches!(l.kind, LineKind::Removed | LineKind::Summary)).count()
    }

    /// The unchanged runs outside the hunks, as ranges of [`Diff::lines`]: before the first hunk,
    /// between hunks and after the last (empty runs left out). `gaps()[i]` comes before
    /// `hunks[i]` when it ends where that hunk starts.
    pub fn gaps(&self) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut at = 0;
        for h in &self.hunks {
            if h.lines.start > at {
                out.push(at..h.lines.start);
            }
            at = h.lines.end;
        }
        if at < self.lines.len() {
            out.push(at..self.lines.len());
        }
        out
    }

    /// The diff as unified-diff text (hunk headers, then ` `, `-`, `+` lines; a summary line as
    /// `~`).
    pub fn unified(&self) -> String {
        let mut out = String::new();
        for h in &self.hunks {
            out.push_str(&h.header());
            out.push('\n');
            for l in &self.lines[h.lines.clone()] {
                out.push(match l.kind {
                    LineKind::Context => ' ',
                    LineKind::Added => '+',
                    LineKind::Removed => '-',
                    LineKind::Summary => '~',
                });
                out.push_str(&l.text);
                out.push('\n');
            }
        }
        out
    }

    /// The old text with the accepted hunks applied (`accepted[i]` for `hunks[i]`; missing
    /// entries are rejected): an agent's proposed change, reviewed. `None` for a JSON diff (its
    /// lines are a printout, not the document).
    pub fn apply(&self, accepted: &[bool]) -> Option<String> {
        if self.json {
            return None;
        }
        let hunk_of = |i: usize| self.hunks.iter().position(|h| h.lines.contains(&i));
        let mut out: Vec<&str> = Vec::new();
        for (i, l) in self.lines.iter().enumerate() {
            let take = match l.kind {
                LineKind::Context | LineKind::Summary => true,
                LineKind::Added => hunk_of(i).is_some_and(|h| accepted.get(h).copied().unwrap_or(false)),
                LineKind::Removed => !hunk_of(i).is_some_and(|h| accepted.get(h).copied().unwrap_or(false)),
            };
            if take {
                out.push(&l.text);
            }
        }
        let mut s = out.join("\n");
        if !s.is_empty() {
            s.push('\n');
        }
        Some(s)
    }
}

/// Every line of both sides in order, from the line ops of `a` and `b`.
fn walk<K: Eq + std::hash::Hash>(a: &[K], b: &[K], old_text: impl Fn(usize) -> String, new_text: impl Fn(usize) -> String) -> Vec<DiffLine> {
    let mut lines = Vec::with_capacity(a.len().max(b.len()));
    let removed = |lines: &mut Vec<DiffLine>, from: usize, len: usize| {
        lines.extend((from..from + len).map(|i| DiffLine { kind: LineKind::Removed, old: Some(i + 1), new: None, text: old_text(i) }));
    };
    let added = |lines: &mut Vec<DiffLine>, from: usize, len: usize| {
        lines.extend((from..from + len).map(|i| DiffLine { kind: LineKind::Added, old: None, new: Some(i + 1), text: new_text(i) }));
    };
    for op in capture_diff_slices(Algorithm::Myers, a, b) {
        match op {
            DiffOp::Equal { old_index, new_index, len } => {
                // The new side's text: a trailing comma that came or went shows as it is now
                lines.extend((0..len).map(|k| DiffLine { kind: LineKind::Context, old: Some(old_index + k + 1), new: Some(new_index + k + 1), text: new_text(new_index + k) }));
            }
            DiffOp::Delete { old_index, old_len, .. } => removed(&mut lines, old_index, old_len),
            DiffOp::Insert { new_index, new_len, .. } => added(&mut lines, new_index, new_len),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                removed(&mut lines, old_index, old_len);
                added(&mut lines, new_index, new_len);
            }
        }
    }
    lines
}

/// The hunks over `lines`: each change with up to `context` unchanged lines on either side;
/// changes whose context touches or overlaps join into one hunk.
fn hunks(lines: &[DiffLine], context: usize) -> Vec<Hunk> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if l.kind == LineKind::Context {
            continue;
        }
        let r = i.saturating_sub(context)..(i + 1 + context).min(lines.len());
        match ranges.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => ranges.push(r),
        }
    }
    ranges
        .into_iter()
        .map(|r| {
            let before = &lines[..r.start];
            let old_before = before.iter().filter(|l| l.old.is_some()).count();
            let new_before = before.iter().filter(|l| l.new.is_some()).count();
            let inside = &lines[r.clone()];
            let old_len = inside.iter().filter(|l| l.old.is_some()).count();
            let new_len = inside.iter().filter(|l| l.new.is_some()).count();
            Hunk {
                lines: r,
                old_start: if old_len > 0 { old_before + 1 } else { old_before },
                old_len,
                new_start: if new_len > 0 { new_before + 1 } else { new_before },
                new_len,
            }
        })
        .collect()
}

/// A printed JSON line: what is compared, what is shown, and the long array it stands for.
struct JsonLine {
    key: String,
    display: String,
    array: Option<(String, usize)>,
}

/// `v` printed with sorted keys, two-space indent, long arrays as one line.
fn print_json(v: &Value, array_summary: usize) -> Vec<JsonLine> {
    let mut out = Vec::new();
    print_value(v, 0, "", None, false, array_summary, &mut out);
    out
}

fn print_value(v: &Value, depth: usize, prefix: &str, label: Option<&str>, comma: bool, max: usize, out: &mut Vec<JsonLine>) {
    let pad = "  ".repeat(depth);
    let c = if comma { "," } else { "" };
    let mut line = |body: String, array: Option<(String, usize)>, hash: Option<u64>| {
        let key = match hash {
            Some(h) => format!("{pad}{prefix}{body}#{h:016x}"),
            None => format!("{pad}{prefix}{body}"),
        };
        out.push(JsonLine { display: format!("{pad}{prefix}{body}{c}"), key, array });
    };
    match v {
        Value::Object(map) if !map.is_empty() => {
            line("{".into(), None, None);
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let n = keys.len();
            for (i, k) in keys.into_iter().enumerate() {
                let p = format!("{}: ", Value::String(k.clone()));
                print_value(&map[k], depth + 1, &p, Some(k), i + 1 < n, max, out);
            }
            out.push(JsonLine { key: format!("{pad}}}"), display: format!("{pad}}}{c}"), array: None });
        }
        Value::Array(items) if items.len() > max => {
            let mut h = DefaultHasher::new();
            serde_json::to_string(v).unwrap_or_default().hash(&mut h);
            let name = label.unwrap_or("items").to_string();
            line(format!("[{} items]", group_digits(items.len())), Some((name, items.len())), Some(h.finish()));
        }
        Value::Array(items) if !items.is_empty() => {
            line("[".into(), None, None);
            let n = items.len();
            for (i, item) in items.iter().enumerate() {
                print_value(item, depth + 1, "", label, i + 1 < n, max, out);
            }
            out.push(JsonLine { key: format!("{pad}]"), display: format!("{pad}]{c}"), array: None });
        }
        _ => line(serde_json::to_string(v).unwrap_or_default(), None, None),
    }
}

/// In each run of changes, a removed long array and an added one of the same name become one
/// summary line (`cells: 2,539 → 2,712`).
fn merge_summaries(lines: &mut Vec<DiffLine>, a: &[JsonLine], b: &[JsonLine]) {
    let array_of = |l: &DiffLine| match l.kind {
        LineKind::Removed => l.old.and_then(|n| a[n - 1].array.clone()),
        LineKind::Added => l.new.and_then(|n| b[n - 1].array.clone()),
        _ => None,
    };
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind == LineKind::Context {
            i += 1;
            continue;
        }
        let end = (i..lines.len()).find(|&j| lines[j].kind == LineKind::Context).unwrap_or(lines.len());
        let mut drop = Vec::new();
        for r in i..end {
            if lines[r].kind != LineKind::Removed {
                continue;
            }
            let Some((name, old_len)) = array_of(&lines[r]) else { continue };
            let found = (i..end).find(|&j| !drop.contains(&j) && lines[j].kind == LineKind::Added && array_of(&lines[j]).is_some_and(|(n, _)| n == name));
            if let Some(j) = found {
                let new_len = array_of(&lines[j]).map(|(_, n)| n).unwrap_or(0);
                let pad: String = lines[r].text.chars().take_while(|c| *c == ' ').collect();
                let text = if old_len == new_len {
                    format!("{pad}{name}: {} items, changed", group_digits(new_len))
                } else {
                    format!("{pad}{name}: {} → {}", group_digits(old_len), group_digits(new_len))
                };
                lines[r] = DiffLine { kind: LineKind::Summary, old: lines[r].old, new: lines[j].new, text };
                drop.push(j);
            }
        }
        drop.sort_unstable();
        for j in drop.iter().rev() {
            lines.remove(*j);
        }
        i = end - drop.len();
    }
}

/// `2539` → `"2,539"`.
pub fn group_digits(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kinds(d: &Diff) -> String {
        d.lines
            .iter()
            .map(|l| match l.kind {
                LineKind::Context => ' ',
                LineKind::Added => '+',
                LineKind::Removed => '-',
                LineKind::Summary => '~',
            })
            .collect()
    }

    #[test]
    fn text_diff_numbers_lines_and_folds_context() {
        let old: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        let new = old.replace("line 5\n", "line five\n").replace("line 18\n", "line 18\nline 18b\n");
        let d = Diff::text(&old, &new, 2);
        assert_eq!(d.added(), 2);
        assert_eq!(d.removed(), 1);
        assert_eq!(d.hunks.len(), 2, "two changes far apart: two hunks");
        let h = &d.hunks[0];
        assert_eq!(h.header(), "@@ -3,5 +3,5 @@");
        let first: Vec<&DiffLine> = d.lines[h.lines.clone()].iter().collect();
        assert_eq!(first[2].kind, LineKind::Removed);
        assert_eq!((first[2].old, first[2].new), (Some(5), None));
        assert_eq!(first[3].kind, LineKind::Added);
        assert_eq!((first[3].old, first[3].new), (None, Some(5)));
        assert_eq!(d.hunks[1].header(), "@@ -17,4 +17,5 @@");
        // Unchanged runs: before the first hunk, between, none after (the change is near the end)
        let gaps = d.gaps();
        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0], 0..2);
        assert_eq!(gaps[1].len(), 9);
        assert!(d.gaps().iter().flat_map(|g| d.lines[g.clone()].iter()).all(|l| l.kind == LineKind::Context));
    }

    #[test]
    fn near_changes_share_a_hunk() {
        let d = Diff::text("a\nb\nc\nd\ne\n", "a\nB\nc\nD\ne\n", 3);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(kinds(&d), " -+ -+ ");
        assert!(Diff::text("same\n", "same\n", 3).is_empty());
    }

    #[test]
    fn apply_takes_only_accepted_hunks() {
        let old: String = (1..=20).map(|i| format!("l{i}\n")).collect();
        let new = old.replace("l2\n", "two\n").replace("l19\n", "nineteen\n");
        let d = Diff::text(&old, &new, 1);
        assert_eq!(d.hunks.len(), 2);
        assert_eq!(d.apply(&[true, true]).unwrap(), new);
        assert_eq!(d.apply(&[false, false]).unwrap(), old);
        assert_eq!(d.apply(&[false, true]).unwrap(), old.replace("l19\n", "nineteen\n"));
        assert!(d.unified().starts_with("@@ -1,3 +1,3 @@\n l1\n-l2\n+two\n"), "{}", d.unified());
    }

    #[test]
    fn json_ignores_key_order_and_trailing_commas() {
        let old = r#"{"b": 1, "a": {"y": 2, "x": 1}}"#;
        let new = r#"{"a": {"x": 1, "y": 2}, "b": 1}"#;
        assert!(Diff::json(old, new, CONTEXT, ARRAY_SUMMARY).is_empty());
        // A key added at the end changes one line only
        let d = Diff::json(r#"{"a": 1}"#, r#"{"a": 1, "b": 2}"#, CONTEXT, ARRAY_SUMMARY);
        assert_eq!(kinds(&d), "  + ");
        assert_eq!(d.lines[1].text, r#"  "a": 1,"#);
        assert_eq!(d.lines[2].text, r#"  "b": 2"#);
    }

    #[test]
    fn json_long_arrays_are_one_summary_line() {
        let old = json!({"cells": (0..2539).collect::<Vec<_>>(), "image": "slide-01", "tiles": (0..40).collect::<Vec<_>>()});
        let new = json!({"tiles": (0..40).collect::<Vec<_>>(), "image": "slide-01", "cells": (0..2712).collect::<Vec<_>>()});
        let d = Diff::json_values(&old, &new, CONTEXT, ARRAY_SUMMARY);
        assert_eq!(d.lines.len(), 5, "{:#?}", d.lines);
        assert_eq!(kinds(&d), " ~   ");
        assert_eq!(d.lines[1].text, "  cells: 2,539 → 2,712");
        assert_eq!(d.lines[3].text, r#"  "tiles": [40 items]"#);
        // Same length, other values: still a change
        let d = Diff::json_values(&json!({"cells": (0..100).collect::<Vec<_>>()}), &json!({"cells": (1..101).collect::<Vec<_>>()}), CONTEXT, ARRAY_SUMMARY);
        assert_eq!(d.lines[1].text, "  cells: 100 items, changed");
        // Short arrays print element by element
        let d = Diff::json_values(&json!({"steps": [1, 2]}), &json!({"steps": [1, 3]}), CONTEXT, ARRAY_SUMMARY);
        assert_eq!(kinds(&d), "   -+  ");
    }

    #[test]
    fn json_that_does_not_parse_falls_back_to_text() {
        let d = Diff::json("{\"a\": 1}", "{oops", CONTEXT, ARRAY_SUMMARY);
        assert!(!d.json);
        assert!(d.note.as_deref().is_some_and(|n| n.starts_with("The new side is not JSON")));
        assert_eq!(d.added(), 1);
    }

    #[test]
    fn digits_are_grouped() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(2539), "2,539");
        assert_eq!(group_digits(1234567), "1,234,567");
    }
}
