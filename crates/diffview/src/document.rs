use ropey::LineType;
use ropey::Rope;

#[derive(Clone, Debug)]
pub struct Document {
    rope: Rope,
}

impl Document {
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len() == 0
    }

    pub fn line_count(&self) -> usize {
        if self.is_empty() {
            return 0;
        }

        let mut count = self.rope.len_lines(LineType::LF);
        if count == 0 {
            return 0;
        }

        if self.rope.byte(self.rope.len() - 1) == b'\n' {
            count = count.saturating_sub(1);
        }

        count
    }

    pub fn line(&self, index: usize) -> Option<String> {
        if index >= self.line_count() {
            return None;
        }
        let slice = self.rope.line(index, LineType::LF);
        Some(trim_line_ending(slice.as_str().unwrap_or_default()).to_string())
    }

    pub fn lines(&self) -> Vec<String> {
        let count = self.line_count();
        (0..count)
            .map(|index| {
                let slice = self.rope.line(index, LineType::LF);
                trim_line_ending(slice.as_str().unwrap_or_default()).to_string()
            })
            .collect()
    }

    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }
}

fn trim_line_ending(line: &str) -> &str {
    if let Some(stripped) = line.strip_suffix("\r\n") {
        return stripped;
    }
    let without_lf = line.strip_suffix('\n').unwrap_or(line);
    without_lf.strip_suffix('\r').unwrap_or(without_lf)
}
