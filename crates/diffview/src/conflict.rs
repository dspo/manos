use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictRegion {
    pub ours_branch_name: String,
    pub theirs_branch_name: String,
    pub range: Range<usize>,
    pub ours: Range<usize>,
    pub theirs: Range<usize>,
    pub base: Option<Range<usize>>,
}

pub fn parse_conflicts(text: &str) -> Vec<ConflictRegion> {
    let mut conflicts = Vec::new();

    let mut conflict_start: Option<usize> = None;
    let mut ours_start: Option<usize> = None;
    let mut ours_end: Option<usize> = None;
    let mut ours_branch_name: Option<String> = None;
    let mut base_start: Option<usize> = None;
    let mut base_end: Option<usize> = None;
    let mut theirs_start: Option<usize> = None;
    let mut theirs_branch_name: Option<String> = None;

    let bytes = text.as_bytes();
    let len = bytes.len();

    let mut line_start = 0usize;
    while line_start <= len {
        let mut line_end = line_start;
        while line_end < len && bytes[line_end] != b'\n' {
            line_end += 1;
        }

        let mut line = &text[line_start..line_end];
        if let Some(stripped) = line.strip_suffix('\r') {
            line = stripped;
        }

        let has_newline = line_end < len;
        let line_end_including_newline = line_end + usize::from(has_newline);

        if let Some(branch_name) = line.strip_prefix("<<<<<<< ") {
            conflict_start = Some(line_start);
            ours_start = Some(line_end_including_newline);

            let branch_name = branch_name.trim();
            if !branch_name.is_empty() {
                ours_branch_name = Some(branch_name.to_string());
            }
        } else if line.starts_with("||||||| ") && conflict_start.is_some() && ours_start.is_some() {
            ours_end = Some(line_start);
            base_start = Some(line_end_including_newline);
        } else if line.starts_with("=======") && conflict_start.is_some() && ours_start.is_some() {
            if ours_end.is_none() {
                ours_end = Some(line_start);
            } else if base_start.is_some() {
                base_end = Some(line_start);
            }
            theirs_start = Some(line_end_including_newline);
        } else if let Some(branch_name) = line.strip_prefix(">>>>>>> ")
            && conflict_start.is_some()
            && ours_start.is_some()
            && ours_end.is_some()
            && theirs_start.is_some()
        {
            let branch_name = branch_name.trim();
            if !branch_name.is_empty() {
                theirs_branch_name = Some(branch_name.to_string());
            }

            let theirs_end = line_start;
            let conflict_end = line_end_including_newline.min(len);

            let base = base_start.zip(base_end).map(|(start, end)| start..end);

            conflicts.push(ConflictRegion {
                ours_branch_name: ours_branch_name
                    .take()
                    .unwrap_or_else(|| "HEAD".to_string()),
                theirs_branch_name: theirs_branch_name
                    .take()
                    .unwrap_or_else(|| "Origin".to_string()),
                range: conflict_start.unwrap()..conflict_end,
                ours: ours_start.unwrap()..ours_end.unwrap(),
                theirs: theirs_start.unwrap()..theirs_end,
                base,
            });

            conflict_start = None;
            ours_start = None;
            ours_end = None;
            base_start = None;
            base_end = None;
            theirs_start = None;
        }

        line_start = line_end_including_newline;
        if line_start == len {
            break;
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice<'a>(text: &'a str, range: &Range<usize>) -> &'a str {
        &text[range.clone()]
    }

    #[test]
    fn parses_conflicts_without_base() {
        let text = "before\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> branch\nafter\n";
        let conflicts = parse_conflicts(text);
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert_eq!(conflict.ours_branch_name, "HEAD");
        assert_eq!(conflict.theirs_branch_name, "branch");
        assert_eq!(slice(text, &conflict.ours), "ours\n");
        assert_eq!(slice(text, &conflict.theirs), "theirs\n");
        assert!(conflict.base.is_none());
    }

    #[test]
    fn parses_conflicts_with_base() {
        let text = "before\n<<<<<<< ours\none\n||||||| base\nbase line\n=======\ntwo\n>>>>>>> theirs\nafter\n";
        let conflicts = parse_conflicts(text);
        assert_eq!(conflicts.len(), 1);

        let conflict = &conflicts[0];
        assert_eq!(conflict.ours_branch_name, "ours");
        assert_eq!(conflict.theirs_branch_name, "theirs");
        assert_eq!(slice(text, &conflict.ours), "one\n");
        assert_eq!(slice(text, &conflict.theirs), "two\n");
        assert_eq!(slice(text, conflict.base.as_ref().unwrap()), "base line\n");
    }

    #[test]
    fn prefers_nested_conflict() {
        let text = "before\n<<<<<<< HEAD\nouter ours\n<<<<<<< HEAD\ninner ours\n=======\ninner theirs\n>>>>>>> inner\n=======\nouter theirs\n>>>>>>> outer\nafter\n";
        let conflicts = parse_conflicts(text);
        assert_eq!(conflicts.len(), 1);
        let conflict = &conflicts[0];
        assert_eq!(conflict.ours_branch_name, "HEAD");
        assert_eq!(conflict.theirs_branch_name, "inner");
        assert_eq!(slice(text, &conflict.ours), "inner ours\n");
        assert_eq!(slice(text, &conflict.theirs), "inner theirs\n");
    }

    #[test]
    fn handles_conflict_markers_at_eof() {
        let text = "<<<<<<< ours\n=======\ntheirs\n>>>>>>> ";
        let conflicts = parse_conflicts(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].ours_branch_name, "ours");
        assert_eq!(conflicts[0].theirs_branch_name, "Origin");
    }
}
