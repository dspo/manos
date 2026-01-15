use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Marks, Node, PluginRegistry, Point, Selection, TextNode,
};

#[test]
fn heading_commands_update_block_and_query() {
    let mut editor = Editor::with_richtext_plugins();

    assert_eq!(
        editor
            .run_query::<Option<u64>>("block.heading_level", None)
            .unwrap(),
        None
    );

    editor
        .run_command("block.set_heading", Some(serde_json::json!({ "level": 2 })))
        .unwrap();

    assert_eq!(
        editor
            .run_query::<Option<u64>>("block.heading_level", None)
            .unwrap(),
        Some(2)
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "heading");
    assert_eq!(block.attrs.get("level").and_then(|v| v.as_u64()), Some(2));

    editor.run_command("block.unset_heading", None).unwrap();

    assert_eq!(
        editor
            .run_query::<Option<u64>>("block.heading_level", None)
            .unwrap(),
        None
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "paragraph");
}

#[test]
fn heading_normalize_clamps_level_attr() {
    let mut attrs = Attrs::default();
    attrs.insert("level".to_string(), serde_json::json!(42));
    let doc = Document {
        children: vec![Node::Element(ElementNode {
            kind: "heading".to_string(),
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
    assert_eq!(block.kind, "heading");
    assert_eq!(block.attrs.get("level").and_then(|v| v.as_u64()), Some(6));
}
