use ropey::{LineType, Rope, RopeSlice};
use sum_tree::Bias;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub(crate) struct TextPoint {
    pub(crate) row: usize,
    pub(crate) column: usize,
}

impl TextPoint {
    pub(crate) fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}

pub(crate) trait RopeExt {
    fn line_start_offset(&self, row: usize) -> usize;
    fn line_end_offset(&self, row: usize) -> usize;
    fn slice_line(&self, row: usize) -> RopeSlice<'_>;
    fn lines_len(&self) -> usize;
    fn char_at(&self, offset: usize) -> Option<char>;

    fn offset_to_point(&self, offset: usize) -> TextPoint;
    fn point_to_offset(&self, point: TextPoint) -> usize;

    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize;
    fn offset_to_offset_utf16(&self, offset: usize) -> usize;

    fn clip_offset(&self, offset: usize, bias: Bias) -> usize;
}

impl RopeExt for Rope {
    fn slice_line(&self, row: usize) -> RopeSlice<'_> {
        let total_lines = self.lines_len();
        if row >= total_lines {
            return self.slice(0..0);
        }

        let line = self.line(row, LineType::LF);
        if line.len() > 0 {
            let line_end = line.len() - 1;
            if line.is_char_boundary(line_end) && line.char(line_end) == '\n' {
                return line.slice(..line_end);
            }
        }

        line
    }

    fn line_start_offset(&self, row: usize) -> usize {
        self.point_to_offset(TextPoint::new(row, 0))
    }

    fn line_end_offset(&self, row: usize) -> usize {
        if row > self.lines_len() {
            return self.len();
        }

        self.line_start_offset(row) + self.slice_line(row).len()
    }

    fn lines_len(&self) -> usize {
        self.len_lines(LineType::LF)
    }

    fn char_at(&self, offset: usize) -> Option<char> {
        if offset > self.len() {
            return None;
        }

        self.get_char(offset).ok()
    }

    fn offset_to_point(&self, offset: usize) -> TextPoint {
        let offset = self.clip_offset(offset, Bias::Left);
        let row = self.byte_to_line_idx(offset, LineType::LF);
        let line_start = self.line_to_byte_idx(row, LineType::LF);
        let column = offset.saturating_sub(line_start);
        TextPoint::new(row, column)
    }

    fn point_to_offset(&self, point: TextPoint) -> usize {
        if point.row >= self.lines_len() {
            return self.len();
        }

        let line_start = self.line_to_byte_idx(point.row, LineType::LF);
        (line_start + point.column).min(self.len())
    }

    #[inline]
    fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize {
        if offset_utf16 > self.len_utf16() {
            return self.len();
        }

        self.utf16_to_byte_idx(offset_utf16)
    }

    #[inline]
    fn offset_to_offset_utf16(&self, offset: usize) -> usize {
        if offset > self.len() {
            return self.len_utf16();
        }

        self.byte_to_utf16_idx(offset)
    }

    fn clip_offset(&self, offset: usize, bias: Bias) -> usize {
        if offset > self.len() {
            return self.len();
        }

        if self.is_char_boundary(offset) {
            return offset;
        }

        if bias == Bias::Left {
            self.floor_char_boundary(offset)
        } else {
            self.ceil_char_boundary(offset)
        }
    }
}
