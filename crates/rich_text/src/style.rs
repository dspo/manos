use gpui::Hsla;
use std::ops::Range;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub fg: Option<Hsla>,
    pub bg: Option<Hsla>,
}

impl InlineStyle {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StyleRun {
    pub(crate) len: usize,
    pub(crate) style: InlineStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StyleRuns {
    runs: Vec<StyleRun>,
}

impl StyleRuns {
    pub(crate) fn new(total_len: usize) -> Self {
        Self {
            runs: vec![StyleRun {
                len: total_len,
                style: InlineStyle::default(),
            }],
        }
    }

    pub(crate) fn total_len(&self) -> usize {
        self.runs.iter().map(|r| r.len).sum()
    }

    pub(crate) fn style_at(&self, mut offset: usize) -> InlineStyle {
        let total_len = self.total_len();
        if total_len == 0 {
            return InlineStyle::default();
        }
        if offset >= total_len {
            offset = total_len.saturating_sub(1);
        }

        let mut cursor = 0;
        for run in &self.runs {
            if offset < cursor + run.len {
                return run.style.clone();
            }
            cursor += run.len;
        }

        self.runs
            .last()
            .map(|r| r.style.clone())
            .unwrap_or_default()
    }

    pub(crate) fn set_total_len(&mut self, total_len: usize) {
        let current = self.total_len();
        if current == total_len {
            return;
        }

        if total_len == 0 {
            self.runs = vec![StyleRun {
                len: 0,
                style: InlineStyle::default(),
            }];
            return;
        }

        if self.runs.is_empty() {
            self.runs.push(StyleRun {
                len: total_len,
                style: InlineStyle::default(),
            });
            return;
        }

        if current < total_len {
            let add_len = total_len - current;
            let tail_style = self
                .runs
                .last()
                .map(|r| r.style.clone())
                .unwrap_or_default();
            self.runs.push(StyleRun {
                len: add_len,
                style: tail_style,
            });
            self.normalize();
            return;
        }

        // current > total_len: truncate
        let mut keep = total_len;
        let mut new_runs = Vec::with_capacity(self.runs.len());
        for run in &self.runs {
            if keep == 0 {
                break;
            }
            let len = run.len.min(keep);
            new_runs.push(StyleRun {
                len,
                style: run.style.clone(),
            });
            keep -= len;
        }

        self.runs = new_runs;
        self.normalize();
    }

    pub(crate) fn delete_range(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }

        let start_ix = self.split_at(range.start);
        let end_ix = self.split_at(range.end);
        if start_ix < end_ix {
            self.runs.drain(start_ix..end_ix);
        }
        self.normalize();
    }

    pub(crate) fn insert_range(&mut self, offset: usize, len: usize, style: InlineStyle) {
        if len == 0 {
            return;
        }
        let ix = self.split_at(offset);
        self.runs.insert(ix, StyleRun { len, style });
        self.normalize();
    }

    pub(crate) fn update_range(
        &mut self,
        range: Range<usize>,
        mut update: impl FnMut(&mut InlineStyle),
    ) {
        if range.is_empty() {
            return;
        }
        let start_ix = self.split_at(range.start);
        let end_ix = self.split_at(range.end);
        for run in &mut self.runs[start_ix..end_ix] {
            update(&mut run.style);
        }
        self.normalize();
    }

    pub(crate) fn iter_runs_in_range(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = (Range<usize>, &InlineStyle)> {
        let mut cursor = 0usize;
        self.runs.iter().filter_map(move |run| {
            let run_start = cursor;
            let run_end = cursor + run.len;
            cursor = run_end;

            let start = run_start.max(range.start);
            let end = run_end.min(range.end);
            if start < end {
                Some((start..end, &run.style))
            } else {
                None
            }
        })
    }

    fn split_at(&mut self, offset: usize) -> usize {
        let total_len = self.total_len();
        let offset = offset.min(total_len);

        let mut cursor = 0usize;
        for ix in 0..self.runs.len() {
            let run_len = self.runs[ix].len;
            if offset == cursor {
                return ix;
            }
            if offset < cursor + run_len {
                let left_len = offset - cursor;
                let right_len = run_len - left_len;
                let style = self.runs[ix].style.clone();
                self.runs[ix].len = left_len;
                self.runs.insert(
                    ix + 1,
                    StyleRun {
                        len: right_len,
                        style,
                    },
                );
                return ix + 1;
            }
            cursor += run_len;
        }
        self.runs.len()
    }

    fn normalize(&mut self) {
        let keep_zero = self.runs.len() == 1;
        self.runs.retain(|r| r.len > 0 || keep_zero);

        if self.runs.is_empty() {
            self.runs.push(StyleRun {
                len: 0,
                style: InlineStyle::default(),
            });
            return;
        }

        let mut merged: Vec<StyleRun> = Vec::with_capacity(self.runs.len());
        for run in self.runs.drain(..) {
            if let Some(prev) = merged.last_mut() {
                if prev.style == run.style {
                    prev.len += run.len;
                    continue;
                }
            }
            merged.push(run);
        }

        self.runs = merged;
        if self.runs.is_empty() {
            self.runs.push(StyleRun {
                len: 0,
                style: InlineStyle::default(),
            });
        }
    }
}
