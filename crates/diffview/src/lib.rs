pub mod conflict;
pub mod diff;
pub mod document;
pub mod model;

pub use conflict::{ConflictRegion, parse_conflicts};
pub use diff::diff_documents;
pub use document::Document;
pub use model::{
    DiffHunk, DiffModel, DiffOptions, DiffRow, DiffRowKind, DiffSegment, DiffSegmentKind, SideLine,
};
