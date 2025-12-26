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

    assert_eq!(
        editor
            .run_query::<bool>("marks.is_italic_active", None)
            .unwrap(),
        false
    );
    editor.run_command("marks.toggle_italic", None).unwrap();
    assert_eq!(
        editor
            .run_query::<bool>("marks.is_italic_active", None)
            .unwrap(),
        true
    );

    let active = editor
        .run_query::<gpui_plate_core::Marks>("marks.get_active", None)
        .unwrap();
    assert!(active.bold);
    assert!(active.italic);

    editor
        .run_command(
            "marks.set_text_color",
            Some(serde_json::json!({ "color": "#ff0000ff" })),
        )
        .unwrap();
    let active = editor
        .run_query::<gpui_plate_core::Marks>("marks.get_active", None)
        .unwrap();
    assert_eq!(active.text_color.as_deref(), Some("#ff0000ff"));
    editor.run_command("marks.unset_text_color", None).unwrap();
    let active = editor
        .run_query::<gpui_plate_core::Marks>("marks.get_active", None)
        .unwrap();
    assert_eq!(active.text_color, None);
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
