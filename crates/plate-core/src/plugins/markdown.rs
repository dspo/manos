//! Markdown import/export plugin.

use serde_json::Value;

use crate::core::{Attrs, Document, ElementNode, Marks, Node, Point, Selection, TextNode};
use crate::ops::{Op, Transaction};
use crate::plugin::{CommandError, CommandSpec, PlatePlugin, QuerySpec};

pub struct MarkdownPlugin;

impl PlatePlugin for MarkdownPlugin {
    fn id(&self) -> &'static str {
        "markdown"
    }

    fn commands(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec::new("markdown.export", "Export to Markdown", |editor, _args| {
                let _md = document_to_markdown(editor.doc());
                Ok(())
            })
            .description("Export the document to Markdown string.")
            .keywords(["markdown", "export", "md"]),
            CommandSpec::new("markdown.import", "Import from Markdown", |editor, args| {
                let md = args
                    .as_ref()
                    .and_then(|v| v.get("markdown"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| CommandError::new("missing args.markdown"))?;

                let doc = markdown_to_document(md);
                import_document(editor, doc)
                    .map_err(CommandError::new)
                    .and_then(|tx| {
                        editor
                            .apply(tx)
                            .map_err(|e| CommandError::new(format!("failed to import: {e:?}")))
                    })
            })
            .description("Import a Markdown string into the document.")
            .keywords(["markdown", "import", "md"])
            .args_example(serde_json::json!({ "markdown": "# Hello\n\nWorld" })),
        ]
    }

    fn queries(&self) -> Vec<QuerySpec> {
        vec![QuerySpec {
            id: "markdown.export".to_string(),
            handler: std::sync::Arc::new(|editor, _args| {
                let md = document_to_markdown(editor.doc());
                Ok(Value::String(md))
            }),
        }]
    }
}

fn document_to_markdown(doc: &Document) -> String {
    let mut out = String::new();
    for node in &doc.children {
        node_to_markdown(node, &mut out, 0);
    }
    out.trim_end().to_string()
}

fn node_to_markdown(node: &Node, out: &mut String, depth: usize) {
    match node {
        Node::Element(el) => element_to_markdown(el, out, depth),
        Node::Text(t) => text_to_markdown(t, out),
        Node::Void(v) => void_to_markdown(v, out),
    }
}

fn element_to_markdown(el: &ElementNode, out: &mut String, depth: usize) {
    match el.kind.as_str() {
        "paragraph" => {
            inline_children_to_markdown(&el.children, out);
            out.push_str("\n\n");
        }
        "heading" => {
            let level = el
                .attrs
                .get("level")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                .clamp(1, 6) as usize;
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
            inline_children_to_markdown(&el.children, out);
            out.push_str("\n\n");
        }
        "code_block" => {
            let lang = el
                .attrs
                .get("language")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            out.push_str("```");
            out.push_str(lang);
            out.push('\n');
            for child in &el.children {
                if let Node::Text(t) = child {
                    out.push_str(&t.text);
                }
            }
            out.push_str("\n```\n\n");
        }
        "blockquote" => {
            let mut inner = String::new();
            for child in &el.children {
                node_to_markdown(child, &mut inner, depth + 1);
            }
            for line in inner.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        "list_item" => {
            let list_type = el
                .attrs
                .get("list_type")
                .and_then(|v| v.as_str())
                .unwrap_or("bulleted");
            let list_level = el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let list_index = el
                .attrs
                .get("list_index")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);

            let indent = "  ".repeat(list_level);
            out.push_str(&indent);
            if list_type == "ordered" {
                out.push_str(&format!("{}. ", list_index));
            } else {
                out.push_str("- ");
            }
            inline_children_to_markdown(&el.children, out);
            out.push('\n');
        }
        "todo_item" => {
            let checked = el
                .attrs
                .get("checked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if checked {
                out.push_str("- [x] ");
            } else {
                out.push_str("- [ ] ");
            }
            inline_children_to_markdown(&el.children, out);
            out.push('\n');
        }
        "table" => {
            md_table_to_markdown(el, out);
        }
        "toggle" => {
            if let Some(Node::Element(title)) = el.children.first() {
                out.push_str("> **");
                inline_children_to_markdown(&title.children, out);
                out.push_str("**\n");
            }
            for child in el.children.iter().skip(1) {
                let mut inner = String::new();
                node_to_markdown(child, &mut inner, depth + 1);
                for line in inner.lines() {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push('\n');
        }
        "columns" | "column" => {
            for child in &el.children {
                node_to_markdown(child, out, depth);
            }
        }
        _ => {
            inline_children_to_markdown(&el.children, out);
            out.push_str("\n\n");
        }
    }
}

fn md_table_to_markdown(el: &ElementNode, out: &mut String) {
    let rows: Vec<&ElementNode> = el
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(row) if row.kind == "table_row" => Some(row),
            _ => None,
        })
        .collect();

    if rows.is_empty() {
        return;
    }

    if let Some(header) = rows.first() {
        out.push('|');
        for cell in &header.children {
            if let Node::Element(cell_el) = cell {
                out.push(' ');
                for para in &cell_el.children {
                    if let Node::Element(p) = para {
                        inline_children_to_markdown(&p.children, out);
                    }
                }
                out.push_str(" |");
            }
        }
        out.push('\n');

        out.push('|');
        for _ in &header.children {
            out.push_str(" --- |");
        }
        out.push('\n');
    }

    for row in rows.iter().skip(1) {
        out.push('|');
        for cell in &row.children {
            if let Node::Element(cell_el) = cell {
                out.push(' ');
                for para in &cell_el.children {
                    if let Node::Element(p) = para {
                        inline_children_to_markdown(&p.children, out);
                    }
                }
                out.push_str(" |");
            }
        }
        out.push('\n');
    }
    out.push('\n');
}

fn inline_children_to_markdown(children: &[Node], out: &mut String) {
    for child in children {
        match child {
            Node::Text(t) => text_to_markdown(t, out),
            Node::Void(v) => void_to_markdown(v, out),
            Node::Element(_) => {}
        }
    }
}

fn text_to_markdown(t: &TextNode, out: &mut String) {
    let mut text = t.text.clone();

    if t.marks.code {
        out.push('`');
        out.push_str(&text);
        out.push('`');
        return;
    }

    let link = t.marks.link.clone();

    if t.marks.bold {
        text = format!("**{}**", text);
    }
    if t.marks.italic {
        text = format!("*{}*", text);
    }
    if t.marks.strikethrough {
        text = format!("~~{}~~", text);
    }

    if let Some(url) = link {
        out.push('[');
        out.push_str(&text);
        out.push_str("](");
        out.push_str(&url);
        out.push(')');
    } else {
        out.push_str(&text);
    }
}

fn void_to_markdown(v: &crate::core::VoidNode, out: &mut String) {
    match v.kind.as_str() {
        "divider" => {
            out.push_str("\n---\n\n");
        }
        "image" => {
            let src = v.attrs.get("src").and_then(|val| val.as_str()).unwrap_or("");
            let alt = v.attrs.get("alt").and_then(|val| val.as_str()).unwrap_or("");
            out.push_str("![");
            out.push_str(alt);
            out.push_str("](");
            out.push_str(src);
            out.push_str(")\n\n");
        }
        "mention" => {
            out.push_str(&v.inline_text());
        }
        "emoji" => {
            out.push_str(&v.inline_text());
        }
        _ => {
            out.push_str(&v.inline_text());
        }
    }
}

fn markdown_to_document(md: &str) -> Document {
    use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(md, options);

    let mut doc = Document {
        children: Vec::new(),
    };

    enum MdStackFrame {
        Paragraph { children: Vec<Node> },
        Heading { level: u64, children: Vec<Node> },
        CodeBlock { lang: String, text: String },
        Blockquote { children: Vec<Node> },
        List { ordered: bool, items: Vec<Node> },
        ListItem { children: Vec<Node>, checked: Option<bool> },
        Table { rows: Vec<Node> },
        TableHead { cells: Vec<Node> },
        TableRow { cells: Vec<Node> },
        TableCell { children: Vec<Node> },
        Emphasis,
        Strong,
        Strikethrough,
        Link { _url: String },
        Image { url: String, alt: String },
    }

    let mut stack: Vec<MdStackFrame> = Vec::new();
    let mut current_marks = Marks::default();

    fn md_push_text_to_stack(stack: &mut Vec<MdStackFrame>, text: &str, marks: &Marks) {
        let node = Node::Text(TextNode {
            text: text.to_string(),
            marks: marks.clone(),
        });
        md_push_node_to_stack(stack, node);
    }

    fn md_push_node_to_stack(stack: &mut [MdStackFrame], node: Node) {
        for frame in stack.iter_mut().rev() {
            match frame {
                MdStackFrame::Paragraph { children }
                | MdStackFrame::Heading { children, .. }
                | MdStackFrame::Blockquote { children }
                | MdStackFrame::ListItem { children, .. }
                | MdStackFrame::TableCell { children } => {
                    children.push(node);
                    return;
                }
                MdStackFrame::CodeBlock { text, .. } => {
                    if let Node::Text(t) = &node {
                        text.push_str(&t.text);
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    fn md_push_to_parent(stack: &mut Vec<MdStackFrame>, doc: &mut Document, node: Node) {
        for frame in stack.iter_mut().rev() {
            match frame {
                MdStackFrame::Blockquote { children }
                | MdStackFrame::ListItem { children, .. } => {
                    children.push(node);
                    return;
                }
                _ => {}
            }
        }
        doc.children.push(node);
    }

    fn md_ensure_text_children(children: Vec<Node>) -> Vec<Node> {
        if children.is_empty() {
            vec![Node::Text(TextNode {
                text: String::new(),
                marks: Marks::default(),
            })]
        } else {
            children
        }
    }

    fn md_flatten_to_inline(children: Vec<Node>) -> Vec<Node> {
        let mut out = Vec::new();
        for child in children {
            match child {
                Node::Element(el) if el.kind == "paragraph" => {
                    out.extend(el.children);
                }
                Node::Text(_) | Node::Void(_) => {
                    out.push(child);
                }
                _ => {}
            }
        }
        if out.is_empty() {
            out.push(Node::Text(TextNode {
                text: String::new(),
                marks: Marks::default(),
            }));
        }
        out
    }

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    stack.push(MdStackFrame::Paragraph {
                        children: Vec::new(),
                    });
                }
                Tag::Heading { level, .. } => {
                    let lvl = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    stack.push(MdStackFrame::Heading {
                        level: lvl,
                        children: Vec::new(),
                    });
                }
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    stack.push(MdStackFrame::CodeBlock {
                        lang,
                        text: String::new(),
                    });
                }
                Tag::BlockQuote(_) => {
                    stack.push(MdStackFrame::Blockquote {
                        children: Vec::new(),
                    });
                }
                Tag::List(start) => {
                    stack.push(MdStackFrame::List {
                        ordered: start.is_some(),
                        items: Vec::new(),
                    });
                }
                Tag::Item => {
                    stack.push(MdStackFrame::ListItem {
                        children: Vec::new(),
                        checked: None,
                    });
                }
                Tag::Table(_) => {
                    stack.push(MdStackFrame::Table { rows: Vec::new() });
                }
                Tag::TableHead => {
                    stack.push(MdStackFrame::TableHead { cells: Vec::new() });
                }
                Tag::TableRow => {
                    stack.push(MdStackFrame::TableRow { cells: Vec::new() });
                }
                Tag::TableCell => {
                    stack.push(MdStackFrame::TableCell {
                        children: Vec::new(),
                    });
                }
                Tag::Emphasis => {
                    stack.push(MdStackFrame::Emphasis);
                    current_marks.italic = true;
                }
                Tag::Strong => {
                    stack.push(MdStackFrame::Strong);
                    current_marks.bold = true;
                }
                Tag::Strikethrough => {
                    stack.push(MdStackFrame::Strikethrough);
                    current_marks.strikethrough = true;
                }
                Tag::Link { dest_url, .. } => {
                    stack.push(MdStackFrame::Link {
                        _url: dest_url.to_string(),
                    });
                    current_marks.link = Some(dest_url.to_string());
                }
                Tag::Image { dest_url, title, .. } => {
                    stack.push(MdStackFrame::Image {
                        url: dest_url.to_string(),
                        alt: title.to_string(),
                    });
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Paragraph => {
                    if let Some(MdStackFrame::Paragraph { children }) = stack.pop() {
                        let node = Node::Element(ElementNode {
                            kind: "paragraph".to_string(),
                            attrs: Attrs::default(),
                            children: md_ensure_text_children(children),
                        });
                        md_push_to_parent(&mut stack, &mut doc, node);
                    }
                }
                TagEnd::Heading(_) => {
                    if let Some(MdStackFrame::Heading { level, children }) = stack.pop() {
                        let mut attrs = Attrs::default();
                        attrs.insert(
                            "level".to_string(),
                            Value::Number(serde_json::Number::from(level)),
                        );
                        let node = Node::Element(ElementNode {
                            kind: "heading".to_string(),
                            attrs,
                            children: md_ensure_text_children(children),
                        });
                        md_push_to_parent(&mut stack, &mut doc, node);
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some(MdStackFrame::CodeBlock { lang, text }) = stack.pop() {
                        let mut attrs = Attrs::default();
                        if !lang.is_empty() {
                            attrs.insert("language".to_string(), Value::String(lang));
                        }
                        let node = Node::Element(ElementNode {
                            kind: "code_block".to_string(),
                            attrs,
                            children: vec![Node::Text(TextNode {
                                text,
                                marks: Marks::default(),
                            })],
                        });
                        md_push_to_parent(&mut stack, &mut doc, node);
                    }
                }
                TagEnd::BlockQuote(_) => {
                    if let Some(MdStackFrame::Blockquote { children }) = stack.pop() {
                        let node = Node::Element(ElementNode {
                            kind: "blockquote".to_string(),
                            attrs: Attrs::default(),
                            children,
                        });
                        md_push_to_parent(&mut stack, &mut doc, node);
                    }
                }
                TagEnd::List(_) => {
                    if let Some(MdStackFrame::List { items, .. }) = stack.pop() {
                        for item in items {
                            md_push_to_parent(&mut stack, &mut doc, item);
                        }
                    }
                }
                TagEnd::Item => {
                    if let Some(MdStackFrame::ListItem { children, checked }) = stack.pop() {
                        let (ordered, index) = stack
                            .iter()
                            .rev()
                            .find_map(|f| match f {
                                MdStackFrame::List { ordered, items } => {
                                    Some((*ordered, items.len() + 1))
                                }
                                _ => None,
                            })
                            .unwrap_or((false, 1));

                        let node = if let Some(is_checked) = checked {
                            let mut attrs = Attrs::default();
                            attrs.insert("checked".to_string(), Value::Bool(is_checked));
                            Node::Element(ElementNode {
                                kind: "todo_item".to_string(),
                                attrs,
                                children: md_flatten_to_inline(children),
                            })
                        } else {
                            let mut attrs = Attrs::default();
                            if ordered {
                                attrs.insert(
                                    "list_type".to_string(),
                                    Value::String("ordered".to_string()),
                                );
                                attrs.insert(
                                    "list_index".to_string(),
                                    Value::Number(serde_json::Number::from(index)),
                                );
                            } else {
                                attrs.insert(
                                    "list_type".to_string(),
                                    Value::String("bulleted".to_string()),
                                );
                            }
                            Node::Element(ElementNode {
                                kind: "list_item".to_string(),
                                attrs,
                                children: md_flatten_to_inline(children),
                            })
                        };

                        if let Some(MdStackFrame::List { items, .. }) = stack.last_mut() {
                            items.push(node);
                        } else {
                            md_push_to_parent(&mut stack, &mut doc, node);
                        }
                    }
                }
                TagEnd::Table => {
                    if let Some(MdStackFrame::Table { rows }) = stack.pop() {
                        let node = Node::Element(ElementNode {
                            kind: "table".to_string(),
                            attrs: Attrs::default(),
                            children: rows,
                        });
                        md_push_to_parent(&mut stack, &mut doc, node);
                    }
                }
                TagEnd::TableHead => {
                    if let Some(MdStackFrame::TableHead { cells }) = stack.pop() {
                        let row = Node::Element(ElementNode {
                            kind: "table_row".to_string(),
                            attrs: Attrs::default(),
                            children: cells,
                        });
                        if let Some(MdStackFrame::Table { rows }) = stack.last_mut() {
                            rows.push(row);
                        }
                    }
                }
                TagEnd::TableRow => {
                    if let Some(MdStackFrame::TableRow { cells }) = stack.pop() {
                        let row = Node::Element(ElementNode {
                            kind: "table_row".to_string(),
                            attrs: Attrs::default(),
                            children: cells,
                        });
                        if let Some(MdStackFrame::Table { rows }) = stack.last_mut() {
                            rows.push(row);
                        }
                    }
                }
                TagEnd::TableCell => {
                    if let Some(MdStackFrame::TableCell { children }) = stack.pop() {
                        let para = Node::Element(ElementNode {
                            kind: "paragraph".to_string(),
                            attrs: Attrs::default(),
                            children: md_ensure_text_children(children),
                        });
                        let cell = Node::Element(ElementNode {
                            kind: "table_cell".to_string(),
                            attrs: Attrs::default(),
                            children: vec![para],
                        });
                        match stack.last_mut() {
                            Some(MdStackFrame::TableHead { cells }) => cells.push(cell),
                            Some(MdStackFrame::TableRow { cells }) => cells.push(cell),
                            _ => {}
                        }
                    }
                }
                TagEnd::Emphasis => {
                    if let Some(MdStackFrame::Emphasis) = stack.pop() {
                        current_marks.italic = false;
                    }
                }
                TagEnd::Strong => {
                    if let Some(MdStackFrame::Strong) = stack.pop() {
                        current_marks.bold = false;
                    }
                }
                TagEnd::Strikethrough => {
                    if let Some(MdStackFrame::Strikethrough) = stack.pop() {
                        current_marks.strikethrough = false;
                    }
                }
                TagEnd::Link => {
                    if let Some(MdStackFrame::Link { .. }) = stack.pop() {
                        current_marks.link = None;
                    }
                }
                TagEnd::Image => {
                    if let Some(MdStackFrame::Image { url, alt }) = stack.pop() {
                        let node = Node::image(url, Some(alt).filter(|s| !s.is_empty()));
                        md_push_to_parent(&mut stack, &mut doc, node);
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                md_push_text_to_stack(&mut stack, &text, &current_marks);
            }
            Event::Code(code) => {
                let node = Node::Text(TextNode {
                    text: code.to_string(),
                    marks: Marks {
                        code: true,
                        ..current_marks.clone()
                    },
                });
                md_push_node_to_stack(&mut stack, node);
            }
            Event::SoftBreak | Event::HardBreak => {
                md_push_text_to_stack(&mut stack, "\n", &current_marks);
            }
            Event::Rule => {
                md_push_to_parent(&mut stack, &mut doc, Node::divider());
            }
            Event::TaskListMarker(checked) => {
                if let Some(MdStackFrame::ListItem { checked: c, .. }) = stack.last_mut() {
                    *c = Some(checked);
                }
            }
            _ => {}
        }
    }

    if doc.children.is_empty() {
        doc.children.push(Node::paragraph(""));
    }

    doc
}

fn import_document(editor: &crate::core::Editor, new_doc: Document) -> Result<Transaction, String> {
    let mut ops = Vec::new();

    for i in (0..editor.doc().children.len()).rev() {
        ops.push(Op::RemoveNode { path: vec![i] });
    }

    for (i, node) in new_doc.children.into_iter().enumerate() {
        ops.push(Op::InsertNode {
            path: vec![i],
            node,
        });
    }

    Ok(Transaction::new(ops)
        .selection_after(Selection::collapsed(Point::new(vec![0, 0], 0)))
        .source("command:markdown.import"))
}
