use gpui_plate_core::{Document, Editor, Node, PluginRegistry, Point, Selection};

#[test]
fn insert_image_inserts_void_block_and_paragraph_after() {
    let doc = Document {
        children: vec![Node::paragraph("hello")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 2));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor
        .run_command(
            "image.insert",
            Some(serde_json::json!({
                "src": "https://example.com/a.png",
                "alt": "A"
            })),
        )
        .unwrap();

    assert_eq!(editor.doc().children.len(), 3);
    assert!(matches!(
        editor.doc().children.get(1),
        Some(Node::Void(v)) if v.kind == "image"
            && v.attrs.get("src").and_then(|v| v.as_str()) == Some("https://example.com/a.png")
            && v.attrs.get("alt").and_then(|v| v.as_str()) == Some("A")
    ));

    assert_eq!(editor.selection().focus.path, vec![2, 0]);
    assert_eq!(editor.selection().focus.offset, 0);
}

#[test]
fn insert_image_requires_src() {
    let mut editor = Editor::with_richtext_plugins();
    let err = editor
        .run_command("image.insert", Some(serde_json::json!({})))
        .unwrap_err();
    assert!(err.message().contains("src"));
}
