use gpui_plate_core::{Editor, Node};

#[test]
fn code_block_toggle_updates_block_and_query() {
    let mut editor = Editor::with_richtext_plugins();

    assert!(
        !editor
            .run_query::<bool>("code_block.is_active", None)
            .unwrap()
    );

    editor.run_command("code_block.toggle", None).unwrap();

    assert!(
        editor
            .run_query::<bool>("code_block.is_active", None)
            .unwrap()
    );
    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "code_block");

    editor.run_command("code_block.toggle", None).unwrap();

    assert!(
        !editor
            .run_query::<bool>("code_block.is_active", None)
            .unwrap()
    );
    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "paragraph");
}

#[test]
fn code_block_toggle_drops_heading_attrs() {
    let mut editor = Editor::with_richtext_plugins();
    editor
        .run_command("block.set_heading", Some(serde_json::json!({ "level": 3 })))
        .unwrap();

    editor.run_command("code_block.toggle", None).unwrap();

    let Node::Element(block) = &editor.doc().children[0] else {
        panic!("expected element block");
    };
    assert_eq!(block.kind, "code_block");
    assert!(block.attrs.get("level").is_none());
}
