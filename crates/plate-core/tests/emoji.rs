use gpui_plate_core::{Document, Editor, Node, PluginRegistry, Point, Selection};

#[test]
fn emoji_insert_splits_text_and_moves_selection() {
    let doc = Document {
        children: vec![Node::paragraph("hello")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 2));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor
        .run_command(
            "emoji.insert",
            Some(serde_json::json!({
                "emoji": "😀",
            })),
        )
        .unwrap();

    let Some(Node::Element(p)) = editor.doc().children.first() else {
        panic!("Expected paragraph element");
    };

    assert_eq!(p.children.len(), 3);
    assert!(matches!(
        p.children.get(1),
        Some(Node::Void(v)) if v.kind == "emoji"
            && v.attrs.get("emoji").and_then(|v| v.as_str()) == Some("😀")
    ));

    assert_eq!(editor.selection().focus.path, vec![0, 2]);
    assert_eq!(editor.selection().focus.offset, 0);
}

#[test]
fn emoji_insert_defaults_to_grinning_face() {
    let mut editor = Editor::with_richtext_plugins();
    editor
        .run_command("emoji.insert", Some(serde_json::json!({})))
        .unwrap();

    let Some(Node::Element(p)) = editor.doc().children.first() else {
        panic!("Expected paragraph element");
    };

    let emoji = p
        .children
        .iter()
        .find_map(|node| match node {
            Node::Void(v) if v.kind == "emoji" => Some(v),
            _ => None,
        })
        .expect("Expected emoji void node");

    assert_eq!(
        emoji.attrs.get("emoji").and_then(|v| v.as_str()),
        Some("😀")
    );
}
