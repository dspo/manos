use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Marks, Node, PluginRegistry, Point, Selection, TextNode,
};

#[test]
fn toggle_todo_and_checked_commands_update_doc_and_queries() {
    let mut editor = Editor::with_richtext_plugins();

    assert!(!editor.run_query::<bool>("todo.is_active", None).unwrap());
    assert!(!editor.run_query::<bool>("todo.is_checked", None).unwrap());

    editor.run_command("todo.toggle", None).unwrap();

    assert!(editor.run_query::<bool>("todo.is_active", None).unwrap());
    assert!(!editor.run_query::<bool>("todo.is_checked", None).unwrap());

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "todo_item");
    assert_eq!(
        block.attrs.get("checked").and_then(|v| v.as_bool()),
        Some(false)
    );

    editor.run_command("todo.toggle_checked", None).unwrap();
    assert!(editor.run_query::<bool>("todo.is_checked", None).unwrap());

    editor.run_command("todo.toggle", None).unwrap();
    assert!(!editor.run_query::<bool>("todo.is_active", None).unwrap());
    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "paragraph");
}

#[test]
fn toggle_todo_applies_across_multi_block_selection_when_in_same_parent() {
    let doc = Document {
        children: vec![
            Node::paragraph("a"),
            Node::paragraph("b"),
            Node::paragraph("c"),
        ],
    };
    let selection = Selection {
        anchor: Point::new(vec![0, 0], 0),
        focus: Point::new(vec![1, 0], 1),
    };
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("todo.toggle", None).unwrap();

    let kinds: Vec<_> = editor
        .doc()
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(el) => Some(el.kind.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, vec!["todo_item", "todo_item", "paragraph"]);
}

#[test]
fn todo_normalize_fills_missing_checked_attr() {
    let todo = Node::Element(ElementNode {
        kind: "todo_item".to_string(),
        attrs: Attrs::default(),
        children: vec![Node::Text(TextNode {
            text: "x".to_string(),
            marks: Marks::default(),
        })],
    });
    let doc = Document {
        children: vec![todo],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "todo_item");
    assert_eq!(
        block.attrs.get("checked").and_then(|v| v.as_bool()),
        Some(false)
    );
}
