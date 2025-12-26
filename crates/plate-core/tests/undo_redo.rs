use gpui_plate_core::{Document, Editor, Node, Op, PluginRegistry, Point, Selection, Transaction};

fn editor_with_text(text: &str) -> Editor {
    let doc = Document {
        children: vec![Node::paragraph(text)],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    Editor::new(doc, selection, PluginRegistry::core())
}

#[test]
fn undo_redo_handles_multi_op_insert_order() {
    let mut editor = editor_with_text("");

    let tx = Transaction::new(vec![
        Op::InsertText {
            path: vec![0, 0],
            offset: 0,
            text: "a".to_string(),
        },
        Op::InsertText {
            path: vec![0, 0],
            offset: 1,
            text: "b".to_string(),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(vec![0, 0], 2)))
    .source("test:multi_insert");

    editor.apply(tx).unwrap();
    assert_eq!(editor.doc().children, vec![Node::paragraph("ab")]);
    assert_eq!(editor.selection().focus.offset, 2);

    assert!(editor.undo());
    assert_eq!(editor.doc().children, vec![Node::paragraph("")]);
    assert_eq!(editor.selection().focus.offset, 0);

    assert!(editor.redo());
    assert_eq!(editor.doc().children, vec![Node::paragraph("ab")]);
    assert_eq!(editor.selection().focus.offset, 2);
}

#[test]
fn undo_redo_handles_multi_op_paste_newline_shape() {
    let mut editor = editor_with_text("XYZ");
    let selection_before = editor.selection().clone();

    let tx = Transaction::new(vec![
        Op::RemoveText {
            path: vec![0, 0],
            range: 0..3,
        },
        Op::InsertText {
            path: vec![0, 0],
            offset: 0,
            text: "a".to_string(),
        },
        Op::InsertNode {
            path: vec![1],
            node: Node::paragraph("bXYZ"),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(vec![1, 0], 1)))
    .source("test:paste_newline");

    editor.apply(tx).unwrap();
    let doc_after = editor.doc().clone();
    let selection_after = editor.selection().clone();

    assert_eq!(doc_after.children.len(), 2);
    assert_eq!(selection_after.focus.path, vec![1, 0]);
    assert_eq!(selection_after.focus.offset, 1);

    assert!(editor.undo());
    assert_eq!(editor.doc().children, vec![Node::paragraph("XYZ")]);
    assert_eq!(editor.selection(), &selection_before);

    assert!(editor.redo());
    assert_eq!(editor.doc(), &doc_after);
    assert_eq!(editor.selection(), &selection_after);
}
