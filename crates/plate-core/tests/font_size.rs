use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Marks, Node, PluginRegistry, Point, Selection, TextNode,
};

#[test]
fn font_size_commands_update_attrs_and_query() {
    let mut editor = Editor::with_richtext_plugins();

    assert_eq!(
        editor
            .run_query::<Option<u64>>("block.font_size", None)
            .unwrap(),
        None
    );

    editor
        .run_command(
            "block.set_font_size",
            Some(serde_json::json!({ "size": 20 })),
        )
        .unwrap();

    assert_eq!(
        editor
            .run_query::<Option<u64>>("block.font_size", None)
            .unwrap(),
        Some(20)
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(
        block.attrs.get("font_size").and_then(|v| v.as_u64()),
        Some(20)
    );

    editor.run_command("block.unset_font_size", None).unwrap();

    assert_eq!(
        editor
            .run_query::<Option<u64>>("block.font_size", None)
            .unwrap(),
        None
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert!(block.attrs.get("font_size").is_none());
}

#[test]
fn font_size_applies_across_multi_block_selection() {
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
            "block.set_font_size",
            Some(serde_json::json!({ "size": 24 })),
        )
        .unwrap();

    for node in &editor.doc().children {
        let Node::Element(el) = node else {
            panic!("expected element");
        };
        assert_eq!(el.attrs.get("font_size").and_then(|v| v.as_u64()), Some(24));
    }
}

#[test]
fn font_size_normalize_clamps_invalid_values() {
    let mut attrs = Attrs::default();
    attrs.insert("font_size".to_string(), serde_json::json!(999));
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
    assert_eq!(
        block.attrs.get("font_size").and_then(|v| v.as_u64()),
        Some(72)
    );

    let mut attrs = Attrs::default();
    attrs.insert("font_size".to_string(), serde_json::json!(true));
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
    assert!(block.attrs.get("font_size").is_none());
}
