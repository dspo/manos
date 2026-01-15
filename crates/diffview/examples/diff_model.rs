use diffview::{DiffOptions, DiffRowKind, DiffSegmentKind, Document, diff_documents};

fn main() {
    let old = r#"fn main() {
    let x = 1;
    println!("x = {}", x);
}
"#;

    let new = r#"fn main() {
    let   x = 2;
    println!( "x = {}", x);
    println!("done");
}
"#;

    let old_doc = Document::from_str(old);
    let new_doc = Document::from_str(new);

    for ignore_whitespace in [false, true] {
        let options = DiffOptions {
            context_lines: 3,
            ignore_whitespace,
        };
        let model = diff_documents(&old_doc, &new_doc, options);

        println!("== ignore_whitespace={ignore_whitespace} ==");
        println!("hunks: {}", model.hunks.len());
        for (index, hunk) in model.hunks.iter().enumerate() {
            println!(
                "-- hunk {} @@ -{},{} +{},{} @@",
                index + 1,
                hunk.old_start + 1,
                hunk.old_len,
                hunk.new_start + 1,
                hunk.new_len
            );

            for row in &hunk.rows {
                let kind = row.kind();
                let old_no = row
                    .old
                    .as_ref()
                    .map(|l| (l.line_index + 1).to_string())
                    .unwrap_or_else(|| "".to_string());
                let new_no = row
                    .new
                    .as_ref()
                    .map(|l| (l.line_index + 1).to_string())
                    .unwrap_or_else(|| "".to_string());

                let left = row
                    .old
                    .as_ref()
                    .map(|l| render_segments(&l.segments))
                    .unwrap_or_default();
                let right = row
                    .new
                    .as_ref()
                    .map(|l| render_segments(&l.segments))
                    .unwrap_or_default();

                let marker = match kind {
                    DiffRowKind::Unchanged => ' ',
                    DiffRowKind::Added => '+',
                    DiffRowKind::Removed => '-',
                    DiffRowKind::Modified => '~',
                };

                println!("{marker} {old_no:>4} | {new_no:>4} | {left} || {right}");
            }
        }
        println!();
    }
}

fn render_segments(segments: &[diffview::DiffSegment]) -> String {
    let mut out = String::new();
    for seg in segments {
        match seg.kind {
            DiffSegmentKind::Unchanged => out.push_str(&seg.text),
            DiffSegmentKind::Added => {
                out.push_str("{+");
                out.push_str(&seg.text);
                out.push_str("+}");
            }
            DiffSegmentKind::Removed => {
                out.push_str("[-");
                out.push_str(&seg.text);
                out.push_str("-]");
            }
        }
    }
    out
}
