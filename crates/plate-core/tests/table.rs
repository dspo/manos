use gpui_plate_core::{Document, Editor, ElementNode, Node, PluginRegistry, Point, Selection};

#[test]
fn table_insert_creates_rectangular_table_and_moves_selection() {
    let mut editor = Editor::with_richtext_plugins();

    editor
        .run_command(
            "table.insert",
            Some(serde_json::json!({ "rows": 2, "cols": 2 })),
        )
        .unwrap();

    assert_eq!(editor.doc().children.len(), 3);
    assert!(matches!(
        editor.doc().children.get(1),
        Some(Node::Element(el)) if el.kind == "table"
    ));

    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(table.children.len(), 2);
    for row in &table.children {
        let Node::Element(row) = row else {
            panic!("Expected table_row element");
        };
        assert_eq!(row.kind, "table_row");
        assert_eq!(row.children.len(), 2);
        for cell in &row.children {
            let Node::Element(cell) = cell else {
                panic!("Expected table_cell element");
            };
            assert_eq!(cell.kind, "table_cell");
            assert!(!cell.children.is_empty());
        }
    }

    assert_eq!(editor.selection().focus.path, vec![1, 0, 0, 0, 0]);
}

#[test]
fn table_row_and_col_commands_keep_table_rectangular() {
    let mut editor = Editor::with_richtext_plugins();
    editor
        .run_command(
            "table.insert",
            Some(serde_json::json!({ "rows": 2, "cols": 2 })),
        )
        .unwrap();

    editor.run_command("table.insert_row_below", None).unwrap();
    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(table.children.len(), 3);
    assert_eq!(editor.selection().focus.path, vec![1, 1, 0, 0, 0]);

    editor.run_command("table.insert_col_right", None).unwrap();
    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    for row in &table.children {
        let Node::Element(row) = row else {
            panic!("Expected row");
        };
        assert_eq!(row.children.len(), 3);
    }
    assert_eq!(editor.selection().focus.path, vec![1, 1, 1, 0, 0]);

    editor.run_command("table.delete_col", None).unwrap();
    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    for row in &table.children {
        let Node::Element(row) = row else {
            panic!("Expected row");
        };
        assert_eq!(row.children.len(), 2);
    }

    editor.run_command("table.delete_row", None).unwrap();
    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(table.children.len(), 2);
}

#[test]
fn table_pro_commands_support_row_above_col_left_and_delete_table() {
    let mut editor = Editor::with_richtext_plugins();
    editor
        .run_command(
            "table.insert",
            Some(serde_json::json!({ "rows": 2, "cols": 2 })),
        )
        .unwrap();

    editor.run_command("table.insert_row_above", None).unwrap();
    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(table.kind, "table");
    assert_eq!(table.children.len(), 3);
    for row in &table.children {
        let Node::Element(row) = row else {
            panic!("Expected row");
        };
        assert_eq!(row.kind, "table_row");
        assert_eq!(row.children.len(), 2);
    }

    editor.run_command("table.insert_col_left", None).unwrap();
    let table = match editor.doc().children.get(1).unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert_eq!(table.kind, "table");
    for row in &table.children {
        let Node::Element(row) = row else {
            panic!("Expected row");
        };
        assert_eq!(row.children.len(), 3);
    }

    editor.run_command("table.delete_table", None).unwrap();
    assert!(matches!(
        editor.doc().children.get(1),
        Some(Node::Element(el)) if el.kind == "paragraph"
    ));
    assert_eq!(editor.selection().focus.path, vec![1, 0]);
}

#[test]
fn table_normalize_fills_missing_structure() {
    let doc = Document {
        children: vec![Node::Element(ElementNode {
            kind: "table".to_string(),
            attrs: Default::default(),
            children: vec![Node::Element(ElementNode {
                kind: "table_row".to_string(),
                attrs: Default::default(),
                children: vec![Node::Element(ElementNode {
                    kind: "table_cell".to_string(),
                    attrs: Default::default(),
                    children: vec![],
                })],
            })],
        })],
    };
    let selection = Selection::collapsed(Point::new(vec![0], 0));
    let editor = Editor::new(doc, selection, PluginRegistry::richtext());

    let table = match editor.doc().children.first().unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    let row = match table.children.first().unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    let cell = match row.children.first().unwrap() {
        Node::Element(el) => el,
        _ => unreachable!(),
    };
    assert!(!cell.children.is_empty());
    assert!(matches!(
        cell.children.first().unwrap(),
        Node::Element(el) if el.kind == "paragraph"
    ));
}
