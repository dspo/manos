#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffOptions {
    pub context_lines: usize,
    pub ignore_whitespace: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            context_lines: 3,
            ignore_whitespace: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffModel {
    pub hunks: Vec<DiffHunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
    pub rows: Vec<DiffRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffRow {
    pub old: Option<SideLine>,
    pub new: Option<SideLine>,
}

impl DiffRow {
    pub fn kind(&self) -> DiffRowKind {
        match (&self.old, &self.new) {
            (Some(old), Some(new)) => {
                if old.segments.len() == 1
                    && new.segments.len() == 1
                    && old.segments[0].kind == DiffSegmentKind::Unchanged
                    && new.segments[0].kind == DiffSegmentKind::Unchanged
                {
                    DiffRowKind::Unchanged
                } else {
                    DiffRowKind::Modified
                }
            }
            (Some(_), None) => DiffRowKind::Removed,
            (None, Some(_)) => DiffRowKind::Added,
            (None, None) => DiffRowKind::Unchanged,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffRowKind {
    Unchanged,
    Added,
    Removed,
    Modified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideLine {
    pub line_index: usize,
    pub text: String,
    pub segments: Vec<DiffSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffSegment {
    pub kind: DiffSegmentKind,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffSegmentKind {
    Unchanged,
    Added,
    Removed,
}
