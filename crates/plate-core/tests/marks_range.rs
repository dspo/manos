use gpui_plate_core::{Document, Editor, Node, PluginRegistry, Point, Selection};

fn row_offset(doc: &Document, point: &Point) -> usize {
    let row = point.path.first().copied().unwrap_or(0);
    let child_ix = point.path.get(1).copied().unwrap_or(0);
    let Some(Node::Element(el)) = doc.children.get(row) else {
        return 0;
    };

    let mut offset = 0usize;
    for (ix, node) in el.children.iter().enumerate() {
        let Node::Text(t) = node else { continue };
        if ix < child_ix {
            offset += t.text.len();
            continue;
        }
        if ix == child_ix {
            offset += point.offset.min(t.text.len());
            break;
        }
    }
    offset
}

#[test]
fn toggle_bold_only_affects_selection_range() {
    let doc = Document {
        children: vec![Node::paragraph("abcde")],
    };
    let selection = Selection {
        anchor: Point::new(vec![0, 0], 1),
        focus: Point::new(vec![0, 0], 3),
    };
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("marks.toggle_bold", None).unwrap();

    let doc = editor.doc();
    let Node::Element(paragraph) = &doc.children[0] else {
        panic!("expected paragraph element");
    };
    assert_eq!(paragraph.kind, "paragraph");
    assert_eq!(paragraph.children.len(), 3);

    let texts: Vec<_> = paragraph
        .children
        .iter()
        .map(|n| match n {
            Node::Text(t) => (t.text.clone(), t.marks.bold),
            _ => ("".to_string(), false),
        })
        .collect();
    assert_eq!(
        texts,
        vec![
            ("a".to_string(), false),
            ("bc".to_string(), true),
            ("de".to_string(), false),
        ]
    );

    let (a, b) = (
        editor.selection().anchor.clone(),
        editor.selection().focus.clone(),
    );
    let a_off = row_offset(doc, &a);
    let b_off = row_offset(doc, &b);
    let start = a_off.min(b_off);
    let end = a_off.max(b_off);
    assert_eq!((start, end), (1, 3));

    editor.run_command("marks.toggle_bold", None).unwrap();
    let doc = editor.doc();
    let Node::Element(paragraph) = &doc.children[0] else {
        panic!("expected paragraph element");
    };
    assert_eq!(paragraph.kind, "paragraph");
    assert_eq!(paragraph.children.len(), 1);

    let Node::Text(t) = &paragraph.children[0] else {
        panic!("expected paragraph text");
    };
    assert_eq!(t.text, "abcde");
    assert!(!t.marks.bold);
}
