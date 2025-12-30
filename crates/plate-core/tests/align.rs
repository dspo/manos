use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Marks, Node, PluginRegistry, Point, Selection, TextNode,
};

#[test]
fn align_command_updates_attrs_and_query() {
    let mut editor = Editor::with_richtext_plugins();

    assert_eq!(
        editor
            .run_query::<Option<String>>("block.align", None)
            .unwrap(),
        None
    );

    editor
        .run_command(
            "block.set_align",
            Some(serde_json::json!({ "align": "center" })),
        )
        .unwrap();

    assert_eq!(
        editor
            .run_query::<Option<String>>("block.align", None)
            .unwrap(),
        Some("center".to_string())
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(
        block.attrs.get("align").and_then(|v| v.as_str()),
        Some("center")
    );

    editor
        .run_command(
            "block.set_align",
            Some(serde_json::json!({ "align": "left" })),
        )
        .unwrap();

    assert_eq!(
        editor
            .run_query::<Option<String>>("block.align", None)
            .unwrap(),
        None
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert!(block.attrs.get("align").is_none());
}

#[test]
fn align_applies_across_multi_block_selection() {
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
        .run_command(
            "block.set_align",
            Some(serde_json::json!({ "align": "right" })),
        )
        .unwrap();

    for node in &editor.doc().children {
        let Node::Element(el) = node else {
            panic!("expected element");
        };
        assert_eq!(
            el.attrs.get("align").and_then(|v| v.as_str()),
            Some("right")
        );
    }
}

#[test]
fn align_normalize_removes_left_and_invalid_values() {
    let mut attrs = Attrs::default();
    attrs.insert("align".to_string(), serde_json::json!("left"));
    let doc = Document {
        children: vec![Node::Element(ElementNode {
            kind: "paragraph".to_string(),
            attrs,
            children: vec![Node::Text(TextNode {
                text: "x".to_string(),
                marks: Marks::default(),
            })],
        })],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert!(block.attrs.get("align").is_none());

    let mut attrs = Attrs::default();
    attrs.insert("align".to_string(), serde_json::json!(true));
    let doc = Document {
        children: vec![Node::Element(ElementNode {
            kind: "paragraph".to_string(),
            attrs,
            children: vec![Node::Text(TextNode {
                text: "x".to_string(),
                marks: Marks::default(),
            })],
        })],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert!(block.attrs.get("align").is_none());
}
