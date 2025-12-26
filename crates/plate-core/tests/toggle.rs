use gpui_plate_core::{
    Attrs, Document, Editor, ElementNode, Marks, Node, PluginRegistry, Point, Selection, TextNode,
};
use serde_json::Value;

fn list_item(text: &str) -> Node {
    Node::Element(ElementNode {
        kind: "list_item".to_string(),
        attrs: {
            let mut attrs = Attrs::default();
            attrs.insert(
                "list_type".to_string(),
                Value::String("bulleted".to_string()),
            );
            attrs
        },
        children: vec![Node::Text(TextNode {
            text: text.to_string(),
            marks: Marks::default(),
        })],
    })
}

#[test]
fn wrap_selection_creates_toggle_and_remaps_selection() {
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

    editor.run_command("toggle.wrap_selection", None).unwrap();

    assert_eq!(editor.doc().children.len(), 1);
    let Node::Element(toggle) = &editor.doc().children[0] else {
        panic!("expected toggle element");
    };
    assert_eq!(toggle.kind, "toggle");
    assert_eq!(
        toggle
            .attrs
            .get("collapsed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        false
    );
    assert_eq!(toggle.children.len(), 3);

    assert_eq!(editor.selection().anchor.path, vec![0, 0, 0]);
    assert_eq!(editor.selection().focus.path, vec![0, 2, 0]);
    assert_eq!(editor.selection().focus.offset, 1);

    assert!(editor.run_query::<bool>("toggle.is_active", None).unwrap());
    assert!(
        !editor
            .run_query::<bool>("toggle.is_collapsed", None)
            .unwrap()
    );
}

#[test]
fn wrap_selection_inserts_title_when_first_block_is_not_paragraph_or_heading() {
    let doc = Document {
        children: vec![list_item("a"), Node::paragraph("b")],
    };
    let selection = Selection {
        anchor: Point::new(vec![0, 0], 0),
        focus: Point::new(vec![1, 0], 1),
    };
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("toggle.wrap_selection", None).unwrap();

    let Node::Element(toggle) = &editor.doc().children[0] else {
        panic!("expected toggle element");
    };
    assert_eq!(toggle.kind, "toggle");
    assert_eq!(toggle.children.len(), 3);
    let Node::Element(title) = &toggle.children[0] else {
        panic!("expected paragraph title");
    };
    assert_eq!(title.kind, "paragraph");

    assert_eq!(editor.selection().anchor.path, vec![0, 1, 0]);
    assert_eq!(editor.selection().focus.path, vec![0, 2, 0]);
    assert_eq!(editor.selection().focus.offset, 1);
}

#[test]
fn unwrap_toggle_restores_children_and_remaps_selection() {
    let toggle = Node::Element(ElementNode {
        kind: "toggle".to_string(),
        attrs: {
            let mut attrs = Attrs::default();
            attrs.insert("collapsed".to_string(), Value::Bool(false));
            attrs
        },
        children: vec![Node::paragraph("title"), Node::paragraph("content")],
    });
    let doc = Document {
        children: vec![toggle, Node::paragraph("after")],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 1, 0], 3));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("toggle.unwrap", None).unwrap();

    let doc = editor.doc();
    assert_eq!(doc.children.len(), 3);
    assert_eq!(
        doc.children
            .iter()
            .filter_map(|n| match n {
                Node::Element(el) => Some(el.kind.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec!["paragraph", "paragraph", "paragraph"]
    );

    assert_eq!(editor.selection().anchor.path, vec![1, 0]);
    assert_eq!(editor.selection().focus.path, vec![1, 0]);
    assert_eq!(editor.selection().focus.offset, 3);
}

#[test]
fn toggle_collapsed_moves_selection_out_of_hidden_content() {
    let toggle = Node::Element(ElementNode {
        kind: "toggle".to_string(),
        attrs: {
            let mut attrs = Attrs::default();
            attrs.insert("collapsed".to_string(), Value::Bool(false));
            attrs
        },
        children: vec![Node::paragraph("title"), Node::paragraph("content")],
    });
    let doc = Document {
        children: vec![toggle],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 1, 0], 2));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("toggle.toggle_collapsed", None).unwrap();

    let Node::Element(toggle) = &editor.doc().children[0] else {
        panic!("expected toggle element");
    };
    assert_eq!(
        toggle
            .attrs
            .get("collapsed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        true
    );

    assert_eq!(editor.selection().focus.path, vec![0, 0, 0]);
    assert_eq!(editor.selection().focus.offset, "title".len());
    assert!(
        editor
            .run_query::<bool>("toggle.is_collapsed", None)
            .unwrap()
    );
}

#[test]
fn normalize_toggle_inserts_title_and_collapsed_default() {
    let toggle = Node::Element(ElementNode {
        kind: "toggle".to_string(),
        attrs: Attrs::default(),
        children: Vec::new(),
    });
    let doc = Document {
        children: vec![toggle],
    };
    let selection = Selection::collapsed(Point::new(vec![0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let Node::Element(toggle) = &editor.doc().children[0] else {
        panic!("expected toggle element");
    };
    assert_eq!(toggle.kind, "toggle");
    assert_eq!(
        toggle
            .attrs
            .get("collapsed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        false
    );
    assert_eq!(toggle.children.len(), 1);
    let Node::Element(title) = &toggle.children[0] else {
        panic!("expected paragraph title");
    };
    assert_eq!(title.kind, "paragraph");
}
