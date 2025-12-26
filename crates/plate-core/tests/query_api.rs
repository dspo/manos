use gpui_plate_core::{Editor, Op, Transaction};

#[test]
fn marks_queries_reflect_active_marks_at_focus() {
    let mut editor = Editor::with_richtext_plugins();

    editor
        .apply(Transaction::new(vec![Op::InsertText {
            path: vec![0, 0],
            offset: 0,
            text: "hello".to_string(),
        }]))
        .unwrap();

    assert_eq!(
        editor
            .run_query::<bool>("marks.is_bold_active", None)
            .unwrap(),
        false
    );

    editor.run_command("marks.toggle_bold", None).unwrap();
    assert_eq!(
        editor
            .run_query::<bool>("marks.is_bold_active", None)
            .unwrap(),
        true
    );

    let active = editor
        .run_query::<gpui_plate_core::Marks>("marks.get_active", None)
        .unwrap();
    assert!(active.bold);
}

#[test]
fn list_queries_reflect_active_list_type() {
    let mut editor = Editor::with_richtext_plugins();

    editor
        .apply(Transaction::new(vec![Op::InsertText {
            path: vec![0, 0],
            offset: 0,
            text: "item".to_string(),
        }]))
        .unwrap();

    assert_eq!(
        editor
            .run_query::<bool>(
                "list.is_active",
                Some(serde_json::json!({ "type": "bulleted" })),
            )
            .unwrap(),
        false
    );

    editor.run_command("list.toggle_bulleted", None).unwrap();

    assert_eq!(
        editor
            .run_query::<Option<String>>("list.active_type", None)
            .unwrap()
            .as_deref(),
        Some("bulleted")
    );
    assert_eq!(
        editor
            .run_query::<bool>(
                "list.is_active",
                Some(serde_json::json!({ "type": "bulleted" })),
            )
            .unwrap(),
        true
    );
}
