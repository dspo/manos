use gpui_plate_core::{Document, Editor, ElementNode, Node, PluginRegistry, Point, Selection};

fn columns_widths(el: &ElementNode) -> Vec<f64> {
    el.attrs
        .get("widths")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
        .unwrap_or_default()
}

#[test]
fn columns_insert_creates_columns_and_moves_selection() {
    let mut editor = Editor::with_richtext_plugins();

    editor
        .run_command("columns.insert", Some(serde_json::json!({ "columns": 2 })))
        .unwrap();

    assert_eq!(editor.doc().children.len(), 3);
    assert!(matches!(
        editor.doc().children.get(1),
        Some(Node::Element(el)) if el.kind == "columns"
    ));

    let columns = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(columns.children.len(), 2);
    for col in &columns.children {
        let Node::Element(col_el) = col else {
            panic!("Expected column element");
        };
        assert_eq!(col_el.kind, "column");
        assert!(!col_el.children.is_empty());
        assert!(matches!(
            col_el.children.first().unwrap(),
            Node::Element(el) if el.kind == "paragraph"
        ));
    }

    let widths = columns_widths(columns);
    assert_eq!(widths.len(), 2);
    let sum: f64 = widths.iter().sum();
    assert!((sum - 1.0).abs() <= 0.01);

    assert_eq!(editor.selection().focus.path, vec![1, 0, 0, 0]);
}

#[test]
fn columns_normalize_inserts_missing_widths_and_paragraphs() {
    let doc = Document {
        children: vec![Node::Element(ElementNode {
            kind: "columns".to_string(),
            attrs: Default::default(),
            children: vec![
                Node::Element(ElementNode {
                    kind: "column".to_string(),
                    attrs: Default::default(),
                    children: vec![],
                }),
                Node::Element(ElementNode {
                    kind: "column".to_string(),
                    attrs: Default::default(),
                    children: vec![Node::paragraph("ok")],
                }),
            ],
        })],
    };
    let selection = Selection::collapsed(Point::new(vec![0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let columns = match editor.doc().children.first().unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(columns.kind, "columns");
    assert!(columns.children.len() >= 2);

    let widths = columns_widths(columns);
    assert_eq!(widths.len(), columns.children.len());
    let sum: f64 = widths.iter().sum();
    assert!((sum - 1.0).abs() <= 0.01);

    let first_col = match columns.children.first().unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(first_col.kind, "column");
    assert!(!first_col.children.is_empty());
}

#[test]
fn columns_unwrap_flattens_children_and_remaps_selection() {
    let mut attrs = gpui_plate_core::Attrs::default();
    attrs.insert("widths".to_string(), serde_json::json!([0.5, 0.5]));

    let doc = Document {
        children: vec![Node::Element(ElementNode {
            kind: "columns".to_string(),
            attrs,
            children: vec![
                Node::Element(ElementNode {
                    kind: "column".to_string(),
                    attrs: Default::default(),
                    children: vec![Node::paragraph("a")],
                }),
                Node::Element(ElementNode {
                    kind: "column".to_string(),
                    attrs: Default::default(),
                    children: vec![Node::paragraph("b"), Node::paragraph("c")],
                }),
            ],
        })],
    };
    let selection = Selection::collapsed(Point::new(vec![0, 1, 0, 0], 1));
    let mut editor = Editor::new(doc, selection, PluginRegistry::richtext());

    editor.run_command("columns.unwrap", None).unwrap();

    assert_eq!(editor.doc().children.len(), 3);
    assert!(matches!(
        editor.doc().children.get(0),
        Some(Node::Element(el)) if el.kind == "paragraph"
    ));
    assert!(matches!(
        editor.doc().children.get(1),
        Some(Node::Element(el)) if el.kind == "paragraph"
    ));
    assert!(matches!(
        editor.doc().children.get(2),
        Some(Node::Element(el)) if el.kind == "paragraph"
    ));

    assert_eq!(editor.selection().focus.path, vec![1, 0]);
}

#[test]
fn columns_set_widths_updates_attrs() {
    let mut editor = Editor::with_richtext_plugins();
    editor
        .run_command("columns.insert", Some(serde_json::json!({ "columns": 2 })))
        .unwrap();

    editor
        .run_command(
            "columns.set_widths",
            Some(serde_json::json!({ "path": [1], "widths": [0.2, 0.8] })),
        )
        .unwrap();

    let columns = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };

    let widths = columns_widths(columns);
    assert_eq!(widths.len(), 2);
    assert!((widths[0] - 0.2).abs() <= 0.01);
    assert!((widths[1] - 0.8).abs() <= 0.01);

    assert!(editor.run_query::<bool>("columns.is_active", None).unwrap());
}
