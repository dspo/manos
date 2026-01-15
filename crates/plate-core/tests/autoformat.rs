use gpui_plate_core::{
    Document, Editor, Marks, Node, Op, PluginRegistry, Point, Selection, TextNode, Transaction,
};
use serde_json::Value;

#[test]
fn autoformat_dash_space_into_bulleted_list() {
    let doc = Document {
        children: vec![Node::paragraph("")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let tx = Transaction::new(vec![Op::InsertText {
        path: vec![0, 0],
        offset: 0,
        text: "- ".to_string(),
    }])
    .source("ime:replace_text");
    editor.apply(tx).unwrap();

    let Node::Element(el) = &editor.doc().children[0] else {
        panic!("expected element");
    };
    assert_eq!(el.kind, "list_item");
    assert_eq!(
        el.attrs.get("list_type").and_then(|v| v.as_str()),
        Some("bulleted")
    );
    assert_eq!(editor.selection().focus.path, vec![0, 0]);
    assert_eq!(editor.selection().focus.offset, 0);
}

#[test]
fn autoformat_greater_than_space_into_blockquote() {
    let doc = Document {
        children: vec![Node::paragraph("")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let tx = Transaction::new(vec![Op::InsertText {
        path: vec![0, 0],
        offset: 0,
        text: "> ".to_string(),
    }])
    .source("ime:replace_text");
    editor.apply(tx).unwrap();

    let Node::Element(quote) = &editor.doc().children[0] else {
        panic!("expected blockquote");
    };
    assert_eq!(quote.kind, "blockquote");
    assert_eq!(quote.children.len(), 1);
    assert_eq!(editor.selection().focus.path, vec![0, 0, 0]);
    assert_eq!(editor.selection().focus.offset, 0);
}

#[test]
fn autoformat_hash_space_into_heading() {
    let doc = Document {
        children: vec![Node::paragraph("")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let tx = Transaction::new(vec![Op::InsertText {
        path: vec![0, 0],
        offset: 0,
        text: "# ".to_string(),
    }])
    .source("ime:replace_text");
    editor.apply(tx).unwrap();

    let Node::Element(el) = &editor.doc().children[0] else {
        panic!("expected heading");
    };
    assert_eq!(el.kind, "heading");
    assert_eq!(el.attrs.get("level"), Some(&Value::Number(1u64.into())));
    assert_eq!(editor.selection().focus.path, vec![0, 0]);
    assert_eq!(editor.selection().focus.offset, 0);
}

#[test]
fn autoformat_brackets_into_todo() {
    let doc = Document {
        children: vec![Node::paragraph("")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let tx = Transaction::new(vec![Op::InsertText {
        path: vec![0, 0],
        offset: 0,
        text: "[ ] ".to_string(),
    }])
    .source("ime:replace_text");
    editor.apply(tx).unwrap();

    let Node::Element(el) = &editor.doc().children[0] else {
        panic!("expected todo item");
    };
    assert_eq!(el.kind, "todo_item");
    assert_eq!(
        el.attrs.get("checked").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(editor.selection().focus.path, vec![0, 0]);
    assert_eq!(editor.selection().focus.offset, 0);
}

#[test]
fn autoformat_does_not_run_for_marked_text_updates() {
    let doc = Document {
        children: vec![Node::paragraph("")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let tx = Transaction::new(vec![Op::InsertText {
        path: vec![0, 0],
        offset: 0,
        text: "- ".to_string(),
    }])
    .source("ime:replace_and_mark_text");
    editor.apply(tx).unwrap();

    let Node::Element(el) = &editor.doc().children[0] else {
        panic!("expected paragraph");
    };
    assert_eq!(el.kind, "paragraph");
    let Node::Text(t) = &el.children[0] else {
        panic!("expected text child");
    };
    assert_eq!(t.text, "- ");
}

#[test]
fn autoformat_handles_view_style_replace_ops() {
    let doc = Document {
        children: vec![Node::paragraph("")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 0], 0));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let tx = Transaction::new(vec![
        Op::RemoveNode { path: vec![0, 0] },
        Op::InsertNode {
            path: vec![0, 0],
            node: Node::Text(TextNode {
                text: "- ".to_string(),
                marks: Marks::default(),
            }),
        },
    ])
    .selection_after(Selection::collapsed(Point::new(vec![0, 0], 2)))
    .source("ime:replace_text");
    editor.apply(tx).unwrap();

    let Node::Element(el) = &editor.doc().children[0] else {
        panic!("expected list item");
    };
    assert_eq!(el.kind, "list_item");
    assert_eq!(
        el.attrs.get("list_type").and_then(|v| v.as_str()),
        Some("bulleted")
    );
    let Node::Text(t) = &el.children[0] else {
        panic!("expected text child");
    };
    assert_eq!(t.text, "");
    assert_eq!(editor.selection().focus.path, vec![0, 0]);
    assert_eq!(editor.selection().focus.offset, 0);
}
