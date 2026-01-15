use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Marks, Node, PluginRegistry, Point, Selection, TextNode,
};

#[test]
fn indent_commands_update_indent_attr_and_query() {
    let mut editor = Editor::with_richtext_plugins();

    assert_eq!(
        editor.run_query::<u64>("block.indent_level", None).unwrap(),
        0
    );

    editor.run_command("block.indent_increase", None).unwrap();

    assert_eq!(
        editor.run_query::<u64>("block.indent_level", None).unwrap(),
        1
    );
    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "paragraph");
    assert_eq!(block.attrs.get("indent").and_then(|v| v.as_u64()), Some(1));

    editor.run_command("block.indent_decrease", None).unwrap();
    assert_eq!(
        editor.run_query::<u64>("block.indent_level", None).unwrap(),
        0
    );
    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert!(block.attrs.get("indent").is_none());
}

#[test]
fn indent_commands_adjust_list_level_for_list_items() {
    let mut editor = Editor::with_richtext_plugins();
    editor.run_command("list.toggle_bulleted", None).unwrap();

    assert_eq!(
        editor.run_query::<u64>("block.indent_level", None).unwrap(),
        0
    );

    editor.run_command("block.indent_increase", None).unwrap();
    assert_eq!(
        editor.run_query::<u64>("block.indent_level", None).unwrap(),
        1
    );

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "list_item");
    assert_eq!(
        block.attrs.get("list_level").and_then(|v| v.as_u64()),
        Some(1)
    );

    editor.run_command("block.indent_decrease", None).unwrap();
    assert_eq!(
        editor.run_query::<u64>("block.indent_level", None).unwrap(),
        0
    );
    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert!(block.attrs.get("list_level").is_none());
}

#[test]
fn indent_applies_across_multi_block_selection() {
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

    editor.run_command("block.indent_increase", None).unwrap();

    for node in &editor.doc().children {
        let Node::Element(el) = node else {
            panic!("expected element");
        };
        assert_eq!(el.attrs.get("indent").and_then(|v| v.as_u64()), Some(1));
    }
}

#[test]
fn indent_normalize_clamps_large_indent_values() {
    let mut attrs = Attrs::default();
    attrs.insert("indent".to_string(), serde_json::json!(999));
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
    assert_eq!(block.attrs.get("indent").and_then(|v| v.as_u64()), Some(8));
}
