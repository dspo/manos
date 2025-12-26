use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Node, PluginRegistry, Point, Selection,
};

#[test]
fn wrap_selection_creates_blockquote_and_remaps_selection() {
    let doc = Document {
        children: vec![
            Node::paragraph("a"),
            Node::paragraph("b"),
            Node::paragraph("c"),
        ],
    };
    let selection = Selection {
        anchor: Point::new(vec![0, 0], 0),
        focus: Point::new(vec![2, 0], 1),
    };
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor
        .run_command("blockquote.wrap_selection", None)
        .unwrap();

    assert_eq!(editor.doc().children.len(), 1);
    let Node::Element(quote) = &editor.doc().children[0] else {
        panic!("expected blockquote element");
    };
    assert_eq!(quote.kind, "blockquote");
    assert_eq!(quote.children.len(), 3);

    let texts: Vec<_> = quote
        .children
        .iter()
        .map(|n| {
            let Node::Element(el) = n else {
                return String::new();
            };
            el.children
                .iter()
                .filter_map(|n| match n {
                    Node::Text(t) => Some(t.text.as_str()),
                    _ => None,
                })
                .fold(String::new(), |mut acc, part| {
                    acc.push_str(part);
                    acc
                })
        })
        .collect();
    assert_eq!(
        texts,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );

    assert_eq!(editor.selection().anchor.path, vec![0, 0, 0]);
    assert_eq!(editor.selection().focus.path, vec![0, 2, 0]);
    assert_eq!(editor.selection().focus.offset, 1);

    assert!(
        editor
            .run_query::<bool>("blockquote.is_active", None)
            .unwrap()
    );
}

#[test]
fn unwrap_blockquote_restores_children_and_remaps_selection() {
    let quote = Node::Element(ElementNode {
        kind: "blockquote".to_string(),
        attrs: Attrs::default(),
        children: vec![Node::paragraph("one"), Node::paragraph("two")],
    });
    let doc = Document {
        children: vec![quote, Node::paragraph("after")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 1, 0], 1));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("blockquote.unwrap", None).unwrap();

    let doc = editor.doc();
    assert_eq!(doc.children.len(), 3);
    assert_eq!(
        doc.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(el) => Some(el.kind.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec!["paragraph", "paragraph", "paragraph"]
    );

    assert_eq!(editor.selection().anchor.path, vec![1, 0]);
    assert_eq!(editor.selection().focus.path, vec![1, 0]);
    assert_eq!(editor.selection().focus.offset, 1);

    assert!(
        !editor
            .run_query::<bool>("blockquote.is_active", None)
            .unwrap()
    );
}

#[test]
fn normalize_blockquote_inserts_paragraph_when_empty() {
    let quote = Node::Element(ElementNode {
        kind: "blockquote".to_string(),
        attrs: Attrs::default(),
        children: Vec::new(),
    });
    let doc = Document {
        children: vec![quote],
    };
    let selection = Selection::collapsed(Point::new(vec![0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let Node::Element(quote) = &editor.doc().children[0] else {
        panic!("expected blockquote element");
    };
    assert_eq!(quote.kind, "blockquote");
    assert_eq!(quote.children.len(), 1);
    let Node::Element(child) = &quote.children[0] else {
        panic!("expected paragraph child");
    };
    assert_eq!(child.kind, "paragraph");
}
