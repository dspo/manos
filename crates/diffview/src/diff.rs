use similar::{DiffOp, TextDiff};

use crate::document::Document;
use crate::model::{
    DiffHunk, DiffModel, DiffOptions, DiffRow, DiffSegment, DiffSegmentKind, SideLine,
};

pub fn diff_documents(old: &Document, new: &Document, options: DiffOptions) -> DiffModel {
    let old_lines = old.lines();
    let new_lines = new.lines();

    let old_key_storage = if options.ignore_whitespace {
        old_lines
            .iter()
            .map(|line| normalize_line_for_diff(line))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let new_key_storage = if options.ignore_whitespace {
        new_lines
            .iter()
            .map(|line| normalize_line_for_diff(line))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let old_key_refs: Vec<&str> = if options.ignore_whitespace {
        old_key_storage.iter().map(|line| line.as_str()).collect()
    } else {
        old_lines.iter().map(|line| line.as_str()).collect()
    };
    let new_key_refs: Vec<&str> = if options.ignore_whitespace {
        new_key_storage.iter().map(|line| line.as_str()).collect()
    } else {
        new_lines.iter().map(|line| line.as_str()).collect()
    };

    let diff = TextDiff::from_slices(&old_key_refs, &new_key_refs);
    let mut hunks = Vec::new();

    for group in diff.grouped_ops(options.context_lines) {
        let mut rows = Vec::new();
        for op in group {
            rows.extend(rows_for_op(
                &op,
                &old_lines,
                &new_lines,
                options.ignore_whitespace,
            ));
        }

        if rows.is_empty() {
            continue;
        }

        let (old_start, old_len) = compute_side_range(&rows, Side::Old);
        let (new_start, new_len) = compute_side_range(&rows, Side::New);

        hunks.push(DiffHunk {
            old_start,
            old_len,
            new_start,
            new_len,
            rows,
        });
    }

    DiffModel { hunks }
}

fn normalize_line_for_diff(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for ch in line.chars() {
        if ch.is_whitespace() {
            continue;
        }
        out.push(ch);
    }
    out
}

fn rows_for_op(
    op: &DiffOp,
    old_lines: &[String],
    new_lines: &[String],
    ignore_whitespace: bool,
) -> Vec<DiffRow> {
    match op.tag() {
        similar::DiffTag::Equal => op
            .old_range()
            .zip(op.new_range())
            .filter_map(|(old_index, new_index)| {
                let old_text = old_lines.get(old_index)?.clone();
                let new_text = new_lines.get(new_index)?.clone();
                Some(DiffRow {
                    old: Some(side_line(old_index, old_text, DiffSegmentKind::Unchanged)),
                    new: Some(side_line(new_index, new_text, DiffSegmentKind::Unchanged)),
                })
            })
            .collect(),
        similar::DiffTag::Delete => op
            .old_range()
            .filter_map(|old_index| {
                let old_text = old_lines.get(old_index)?.clone();
                Some(DiffRow {
                    old: Some(side_line(old_index, old_text, DiffSegmentKind::Removed)),
                    new: None,
                })
            })
            .collect(),
        similar::DiffTag::Insert => op
            .new_range()
            .filter_map(|new_index| {
                let new_text = new_lines.get(new_index)?.clone();
                Some(DiffRow {
                    old: None,
                    new: Some(side_line(new_index, new_text, DiffSegmentKind::Added)),
                })
            })
            .collect(),
        similar::DiffTag::Replace => rows_for_replace(op, old_lines, new_lines, ignore_whitespace),
    }
}

fn rows_for_replace(
    op: &DiffOp,
    old_lines: &[String],
    new_lines: &[String],
    ignore_whitespace: bool,
) -> Vec<DiffRow> {
    let old_range = op.old_range();
    let new_range = op.new_range();

    let old_slice = &old_lines[old_range.clone()];
    let new_slice = &new_lines[new_range.clone()];
    if old_slice.is_empty() || new_slice.is_empty() {
        return rows_for_replace_by_index(
            old_range.start,
            old_range.len(),
            new_range.start,
            new_range.len(),
            old_lines,
            new_lines,
        );
    }

    let old_key_storage = if ignore_whitespace {
        old_slice
            .iter()
            .map(|line| normalize_line_for_diff(line))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let new_key_storage = if ignore_whitespace {
        new_slice
            .iter()
            .map(|line| normalize_line_for_diff(line))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let old_key_refs: Vec<&str> = if ignore_whitespace {
        old_key_storage.iter().map(|line| line.as_str()).collect()
    } else {
        old_slice.iter().map(|line| line.as_str()).collect()
    };
    let new_key_refs: Vec<&str> = if ignore_whitespace {
        new_key_storage.iter().map(|line| line.as_str()).collect()
    } else {
        new_slice.iter().map(|line| line.as_str()).collect()
    };

    let diff = TextDiff::from_slices(&old_key_refs, &new_key_refs);
    let ops = diff.ops();

    if ops.len() == 1 {
        let op = &ops[0];
        if op.tag() == similar::DiffTag::Replace
            && op.old_range().len() == old_slice.len()
            && op.new_range().len() == new_slice.len()
            && old_slice.len() > 1
            && new_slice.len() > 1
        {
            return rows_for_replace_by_index(
                old_range.start,
                old_range.len(),
                new_range.start,
                new_range.len(),
                old_lines,
                new_lines,
            );
        }
    }

    let mut rows = Vec::new();
    for op in ops {
        rows.extend(rows_for_op_with_offsets(
            op,
            old_range.start,
            new_range.start,
            old_lines,
            new_lines,
        ));
    }

    rows
}

fn rows_for_op_with_offsets(
    op: &DiffOp,
    old_offset: usize,
    new_offset: usize,
    old_lines: &[String],
    new_lines: &[String],
) -> Vec<DiffRow> {
    match op.tag() {
        similar::DiffTag::Equal => op
            .old_range()
            .zip(op.new_range())
            .filter_map(|(old_index, new_index)| {
                let old_index = old_offset + old_index;
                let new_index = new_offset + new_index;
                let old_text = old_lines.get(old_index)?.clone();
                let new_text = new_lines.get(new_index)?.clone();
                Some(DiffRow {
                    old: Some(side_line(old_index, old_text, DiffSegmentKind::Unchanged)),
                    new: Some(side_line(new_index, new_text, DiffSegmentKind::Unchanged)),
                })
            })
            .collect(),
        similar::DiffTag::Delete => op
            .old_range()
            .filter_map(|old_index| {
                let old_index = old_offset + old_index;
                let old_text = old_lines.get(old_index)?.clone();
                Some(DiffRow {
                    old: Some(side_line(old_index, old_text, DiffSegmentKind::Removed)),
                    new: None,
                })
            })
            .collect(),
        similar::DiffTag::Insert => op
            .new_range()
            .filter_map(|new_index| {
                let new_index = new_offset + new_index;
                let new_text = new_lines.get(new_index)?.clone();
                Some(DiffRow {
                    old: None,
                    new: Some(side_line(new_index, new_text, DiffSegmentKind::Added)),
                })
            })
            .collect(),
        similar::DiffTag::Replace => {
            let old_range = op.old_range();
            let new_range = op.new_range();
            rows_for_replace_by_index(
                old_offset + old_range.start,
                old_range.len(),
                new_offset + new_range.start,
                new_range.len(),
                old_lines,
                new_lines,
            )
        }
    }
}

fn rows_for_replace_by_index(
    old_start: usize,
    old_len: usize,
    new_start: usize,
    new_len: usize,
    old_lines: &[String],
    new_lines: &[String],
) -> Vec<DiffRow> {
    let row_len = old_len.max(new_len);
    let mut rows = Vec::with_capacity(row_len);

    for offset in 0..row_len {
        let old_index = old_start + offset;
        let new_index = new_start + offset;

        let old_text = (offset < old_len)
            .then(|| old_lines.get(old_index).cloned())
            .flatten();
        let new_text = (offset < new_len)
            .then(|| new_lines.get(new_index).cloned())
            .flatten();

        let (old, new) = match (old_text, new_text) {
            (Some(old_text), Some(new_text)) => {
                let (old_segments, new_segments) = intraline_segments(&old_text, &new_text);
                (
                    Some(SideLine {
                        line_index: old_index,
                        text: old_text,
                        segments: old_segments,
                    }),
                    Some(SideLine {
                        line_index: new_index,
                        text: new_text,
                        segments: new_segments,
                    }),
                )
            }
            (Some(old_text), None) => (
                Some(side_line(old_index, old_text, DiffSegmentKind::Removed)),
                None,
            ),
            (None, Some(new_text)) => (
                None,
                Some(side_line(new_index, new_text, DiffSegmentKind::Added)),
            ),
            (None, None) => (None, None),
        };

        rows.push(DiffRow { old, new });
    }

    rows
}

fn intraline_segments(old_text: &str, new_text: &str) -> (Vec<DiffSegment>, Vec<DiffSegment>) {
    let diff = TextDiff::from_chars(old_text, new_text);

    let mut old_segments = Vec::new();
    let mut new_segments = Vec::new();

    for change in diff.iter_all_changes() {
        let value = change.value().to_string();
        match change.tag() {
            similar::ChangeTag::Equal => {
                push_segment(&mut old_segments, DiffSegmentKind::Unchanged, value.clone());
                push_segment(&mut new_segments, DiffSegmentKind::Unchanged, value);
            }
            similar::ChangeTag::Delete => {
                push_segment(&mut old_segments, DiffSegmentKind::Removed, value);
            }
            similar::ChangeTag::Insert => {
                push_segment(&mut new_segments, DiffSegmentKind::Added, value);
            }
        }
    }

    if old_segments.is_empty() {
        old_segments.push(DiffSegment {
            kind: DiffSegmentKind::Removed,
            text: old_text.to_string(),
        });
    }

    if new_segments.is_empty() {
        new_segments.push(DiffSegment {
            kind: DiffSegmentKind::Added,
            text: new_text.to_string(),
        });
    }

    (old_segments, new_segments)
}

fn push_segment(segments: &mut Vec<DiffSegment>, kind: DiffSegmentKind, text: String) {
    if text.is_empty() {
        return;
    }

    if let Some(last) = segments.last_mut() {
        if last.kind == kind {
            last.text.push_str(&text);
            return;
        }
    }

    segments.push(DiffSegment { kind, text });
}

fn side_line(line_index: usize, text: String, kind: DiffSegmentKind) -> SideLine {
    SideLine {
        line_index,
        text: text.clone(),
        segments: vec![DiffSegment { kind, text }],
    }
}

#[derive(Clone, Copy, Debug)]
enum Side {
    Old,
    New,
}

fn compute_side_range(rows: &[DiffRow], side: Side) -> (usize, usize) {
    let mut min: Option<usize> = None;
    let mut max: Option<usize> = None;

    for row in rows {
        let line = match side {
            Side::Old => row.old.as_ref(),
            Side::New => row.new.as_ref(),
        };
        let Some(line) = line else { continue };

        min = Some(min.map_or(line.line_index, |m| m.min(line.line_index)));
        max = Some(max.map_or(line.line_index, |m| m.max(line.line_index)));
    }

    let Some(min) = min else { return (0, 0) };
    let Some(max) = max else { return (0, 0) };

    (min, max - min + 1)
}
