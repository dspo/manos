use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use gpui::actions;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{ActiveTheme as _, WindowExt as _, notification::Notification};
use gpui_plate_core::{
    Attrs, ChildConstraint, Document, Editor, ElementNode, Marks, Node, NodeRole, PlateValue,
    PluginRegistry, Point as CorePoint, Selection, TextNode,
};
use serde::{Deserialize, Serialize};

use crate::{
    BlockAlign, BundleAsset, BundleExportReport, CollectAssetsReport, EmbedLocalImagesReport,
    PortabilityIssue, PortabilityIssueLevel, PortabilityReport,
};

pub const CONTEXT: &str = "RichText";

actions!(
    rich_text,
    [
        Backspace,
        Delete,
        Enter,
        MoveLeft,
        MoveRight,
        SelectLeft,
        SelectRight,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        InsertDivider,
        InsertMention,
        ToggleBold,
        ToggleItalic,
        ToggleUnderline,
        ToggleStrikethrough,
        ToggleCode,
        ToggleBulletedList,
        ToggleOrderedList,
    ]
);

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
        KeyBinding::new("delete", Delete, Some(CONTEXT)),
        KeyBinding::new("enter", Enter, Some(CONTEXT)),
        KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
        KeyBinding::new("right", MoveRight, Some(CONTEXT)),
        KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-z", Redo, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-d", InsertDivider, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-d", InsertDivider, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-m", InsertMention, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-m", InsertMention, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-b", ToggleBold, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-b", ToggleBold, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-i", ToggleItalic, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-i", ToggleItalic, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-u", ToggleUnderline, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-u", ToggleUnderline, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-x", ToggleStrikethrough, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-x", ToggleStrikethrough, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-e", ToggleCode, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-e", ToggleCode, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-8", ToggleBulletedList, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-8", ToggleBulletedList, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-7", ToggleOrderedList, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-7", ToggleOrderedList, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
    ]);
}

fn node_at_path<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a Node> {
    if path.is_empty() {
        return None;
    }

    let mut node = doc.children.get(path[0])?;
    for &ix in path.iter().skip(1) {
        node = match node {
            Node::Element(el) => el.children.get(ix)?,
            Node::Text(_) | Node::Void(_) => return None,
        };
    }
    Some(node)
}

fn element_at_path<'a>(doc: &'a Document, path: &[usize]) -> Option<&'a ElementNode> {
    match node_at_path(doc, path)? {
        Node::Element(el) => Some(el),
        _ => None,
    }
}

fn is_text_block_kind(registry: &PluginRegistry, kind: &str) -> bool {
    registry
        .node_specs()
        .get(kind)
        .map(|spec| {
            spec.role == NodeRole::Block
                && !spec.is_void
                && spec.children == ChildConstraint::InlineOnly
        })
        .unwrap_or(false)
}

fn text_block_paths(doc: &Document, registry: &PluginRegistry) -> Vec<Vec<usize>> {
    fn walk(
        nodes: &[Node],
        path: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
        registry: &PluginRegistry,
    ) {
        for (ix, node) in nodes.iter().enumerate() {
            let Node::Element(el) = node else {
                continue;
            };

            path.push(ix);

            if is_text_block_kind(registry, &el.kind) {
                out.push(path.clone());
            } else {
                if el.kind == "toggle"
                    && el
                        .attrs
                        .get("collapsed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                {
                    if let Some(first) = el.children.first() {
                        walk(std::slice::from_ref(first), path, out, registry);
                    }
                } else {
                    walk(&el.children, path, out, registry);
                }
            }

            path.pop();
        }
    }

    let mut out = Vec::new();
    walk(&doc.children, &mut Vec::new(), &mut out, registry);
    out
}

#[derive(Clone)]
pub enum InlineTextSegmentKind {
    Text,
    Void { kind: SharedString },
}

#[derive(Clone)]
pub struct InlineTextSegment {
    pub child_ix: usize,
    pub start: usize,
    pub len: usize,
    pub marks: Marks,
    pub kind: InlineTextSegmentKind,
}

#[derive(Clone)]
pub struct LineLayoutCache {
    pub bounds: Bounds<Pixels>,
    pub text_layout: gpui::TextLayout,
    pub text_align: TextAlign,
    pub text: SharedString,
    pub segments: Vec<InlineTextSegment>,
}

#[derive(Clone)]
pub struct BlockBoundsCache {
    pub bounds: Bounds<Pixels>,
    pub kind: SharedString,
}

#[derive(Clone)]
struct ColumnsResizeState {
    columns_path: Vec<usize>,
    boundary_ix: usize,
    start_mouse_x: Pixels,
    total_width: Pixels,
    start_widths: Vec<Pixels>,
    start_fractions: Vec<f32>,
    preview_fractions: Vec<f32>,
}

#[derive(Clone, Debug)]
struct FindActiveMatch {
    block_path: Vec<usize>,
    range_index: usize,
}

pub struct RichTextState {
    focus_handle: FocusHandle,
    scroll_handle: ScrollHandle,
    editor: Editor,
    selecting: bool,
    selection_anchor: Option<CorePoint>,
    viewport_bounds: Bounds<Pixels>,
    layout_cache: HashMap<Vec<usize>, LineLayoutCache>,
    block_bounds_cache: HashMap<Vec<usize>, BlockBoundsCache>,
    embedded_image_cache: HashMap<String, Arc<Image>>,
    document_base_dir: Option<PathBuf>,
    text_block_order: Vec<Vec<usize>>,
    find_query: Option<String>,
    find_matches: HashMap<Vec<usize>, Vec<Range<usize>>>,
    find_active: Option<FindActiveMatch>,
    ime_marked_range: Option<Range<usize>>,
    selected_block_path: Option<Vec<usize>>,
    columns_resizing: Option<ColumnsResizeState>,
    did_auto_focus: bool,
    pending_notifications: Vec<String>,
}

const PLATE_CLIPBOARD_FRAGMENT_KIND: &str = "gpui-manos-plate/fragment";
const PLATE_CLIPBOARD_FRAGMENT_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlateClipboardFragmentMode {
    Block,
    Range,
}

impl Default for PlateClipboardFragmentMode {
    fn default() -> Self {
        Self::Block
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlateClipboardFragment {
    kind: String,
    version: u64,
    #[serde(default)]
    mode: PlateClipboardFragmentMode,
    nodes: Vec<Node>,
}

impl PlateClipboardFragment {
    fn new(nodes: Vec<Node>) -> Self {
        Self {
            kind: PLATE_CLIPBOARD_FRAGMENT_KIND.to_string(),
            version: PLATE_CLIPBOARD_FRAGMENT_VERSION,
            mode: PlateClipboardFragmentMode::Block,
            nodes,
        }
    }

    fn new_range(nodes: Vec<Node>) -> Self {
        Self {
            kind: PLATE_CLIPBOARD_FRAGMENT_KIND.to_string(),
            version: PLATE_CLIPBOARD_FRAGMENT_VERSION,
            mode: PlateClipboardFragmentMode::Range,
            nodes,
        }
    }

    fn from_clipboard(item: &ClipboardItem) -> Option<Self> {
        for entry in item.entries() {
            let ClipboardEntry::String(s) = entry else {
                continue;
            };
            let payload: PlateClipboardFragment = s.metadata_json()?;
            if payload.kind == PLATE_CLIPBOARD_FRAGMENT_KIND
                && payload.version == PLATE_CLIPBOARD_FRAGMENT_VERSION
            {
                return Some(payload);
            }
        }
        None
    }
}

impl RichTextState {
    fn queue_notification(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.pending_notifications.push(message.into());
        cx.notify();
    }

    fn data_url_for_image(image: &Image) -> String {
        format!(
            "data:{};base64,{}",
            image.format.mime_type(),
            base64::engine::general_purpose::STANDARD.encode(&image.bytes)
        )
    }

    fn decode_data_url_bytes(src: &str) -> Option<(String, Vec<u8>)> {
        let src = src.strip_prefix("data:")?;
        let (meta, data) = src.split_once(',')?;

        let is_base64 = meta
            .split(';')
            .any(|part| part.trim().eq_ignore_ascii_case("base64"));
        if !is_base64 {
            return None;
        }

        let mime = meta.split(';').next().unwrap_or("").trim().to_string();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .ok()?;
        Some((mime, bytes))
    }

    fn parse_data_url_image(src: &str) -> Option<Image> {
        let (mime, bytes) = Self::decode_data_url_bytes(src)?;
        let format = ImageFormat::from_mime_type(&mime)?;
        Some(Image::from_bytes(format, bytes))
    }

    fn get_or_decode_data_url_image(
        cache: &mut HashMap<String, Arc<Image>>,
        src: &str,
    ) -> Option<Arc<Image>> {
        if !src.starts_with("data:") {
            return None;
        }
        if let Some(img) = cache.get(src) {
            return Some(img.clone());
        }
        let img = Arc::new(Self::parse_data_url_image(src)?);
        cache.insert(src.to_string(), img.clone());
        Some(img)
    }

    fn mime_and_format_for_image_path(path: &Path) -> Option<(&'static str, ImageFormat)> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "png" => Some(("image/png", ImageFormat::Png)),
            "jpg" | "jpeg" => Some(("image/jpeg", ImageFormat::Jpeg)),
            "webp" => Some(("image/webp", ImageFormat::Webp)),
            "gif" => Some(("image/gif", ImageFormat::Gif)),
            "svg" => Some(("image/svg+xml", ImageFormat::Svg)),
            "bmp" => Some(("image/bmp", ImageFormat::Bmp)),
            "tif" | "tiff" => Some(("image/tiff", ImageFormat::Tiff)),
            _ => None,
        }
    }

    fn image_ext_for_mime(mime: &str) -> Option<&'static str> {
        match mime.trim().to_lowercase().as_str() {
            "image/png" => Some("png"),
            "image/jpeg" | "image/jpg" => Some("jpg"),
            "image/webp" => Some("webp"),
            "image/gif" => Some("gif"),
            "image/svg+xml" => Some("svg"),
            "image/bmp" => Some("bmp"),
            "image/tiff" => Some("tiff"),
            _ => None,
        }
    }

    fn local_image_path_for_src(src: &str, document_base_dir: Option<&Path>) -> Option<PathBuf> {
        let src = src.trim();
        if src.is_empty() {
            return None;
        }
        if src.starts_with("data:")
            || src.starts_with("http://")
            || src.starts_with("https://")
            || src.starts_with("resource://")
        {
            return None;
        }

        let mut path = if let Some(rest) = src.strip_prefix("file://") {
            PathBuf::from(rest)
        } else {
            PathBuf::from(Self::normalize_image_src(src))
        };

        if !path.is_absolute() {
            let base = document_base_dir?;
            path = base.join(path);
        }

        if !path.exists() || !path.is_file() {
            return None;
        }

        Some(std::fs::canonicalize(&path).unwrap_or(path))
    }

    fn embed_local_images_in_document(
        doc: &mut Document,
        document_base_dir: Option<&Path>,
    ) -> EmbedLocalImagesReport {
        fn walk_nodes(
            nodes: &mut [Node],
            report: &mut EmbedLocalImagesReport,
            document_base_dir: Option<&Path>,
        ) {
            for node in nodes {
                match node {
                    Node::Element(el) => walk_nodes(&mut el.children, report, document_base_dir),
                    Node::Void(v) if v.kind == "image" => {
                        let Some(src) = v.attrs.get("src").and_then(|v| v.as_str()) else {
                            report.skipped += 1;
                            continue;
                        };
                        let Some(local_path) =
                            RichTextState::local_image_path_for_src(src, document_base_dir)
                        else {
                            report.skipped += 1;
                            continue;
                        };
                        let Some((mime, _format)) =
                            RichTextState::mime_and_format_for_image_path(&local_path)
                        else {
                            report.skipped += 1;
                            continue;
                        };
                        let bytes = match std::fs::read(&local_path) {
                            Ok(bytes) => bytes,
                            Err(_) => {
                                report.failed += 1;
                                continue;
                            }
                        };
                        let data_url = format!(
                            "data:{mime};base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(&bytes)
                        );
                        v.attrs
                            .insert("src".to_string(), serde_json::Value::String(data_url));
                        report.embedded += 1;
                    }
                    Node::Void(_) | Node::Text(_) => {}
                }
            }
        }

        let mut report = EmbedLocalImagesReport::default();
        walk_nodes(&mut doc.children, &mut report, document_base_dir);
        report
    }

    pub fn make_plate_value_portable(
        value: PlateValue,
        document_base_dir: Option<PathBuf>,
    ) -> (PlateValue, EmbedLocalImagesReport) {
        let mut doc = value.into_document();
        let report = Self::embed_local_images_in_document(&mut doc, document_base_dir.as_deref());
        (PlateValue::from_document(doc), report)
    }

    fn bundle_local_images_in_document(
        doc: &mut Document,
        document_base_dir: Option<&Path>,
    ) -> (Vec<BundleAsset>, BundleExportReport) {
        fn walk_nodes(
            nodes: &mut [Node],
            document_base_dir: Option<&Path>,
            assets: &mut Vec<BundleAsset>,
            asset_paths: &mut HashMap<String, ()>,
            report: &mut BundleExportReport,
        ) {
            for node in nodes {
                match node {
                    Node::Element(el) => walk_nodes(
                        &mut el.children,
                        document_base_dir,
                        assets,
                        asset_paths,
                        report,
                    ),
                    Node::Void(v) if v.kind == "image" => {
                        let Some(src) = v.attrs.get("src").and_then(|v| v.as_str()) else {
                            report.skipped += 1;
                            continue;
                        };

                        let (bytes, ext, is_data_url) = if src.trim().starts_with("data:") {
                            let Some((mime, bytes)) = RichTextState::decode_data_url_bytes(src)
                            else {
                                report.failed += 1;
                                continue;
                            };
                            (
                                bytes,
                                RichTextState::image_ext_for_mime(&mime)
                                    .map(|s| s.to_string())
                                    .unwrap_or_default(),
                                true,
                            )
                        } else {
                            let Some(local_path) =
                                RichTextState::local_image_path_for_src(src, document_base_dir)
                            else {
                                report.skipped += 1;
                                continue;
                            };
                            let bytes = match std::fs::read(&local_path) {
                                Ok(bytes) => bytes,
                                Err(_) => {
                                    report.failed += 1;
                                    continue;
                                }
                            };
                            let ext = local_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            (bytes, ext, false)
                        };

                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        bytes.hash(&mut hasher);
                        let digest = hasher.finish();

                        let file_name = if ext.is_empty() {
                            format!("{digest:016x}")
                        } else {
                            format!("{digest:016x}.{ext}")
                        };
                        let relative_path = format!("assets/{file_name}");

                        v.attrs.insert(
                            "src".to_string(),
                            serde_json::Value::String(relative_path.clone()),
                        );
                        report.bundled += 1;
                        if is_data_url {
                            report.bundled_data_url += 1;
                        }

                        if asset_paths.contains_key(&relative_path) {
                            continue;
                        }
                        asset_paths.insert(relative_path.clone(), ());
                        assets.push(BundleAsset {
                            relative_path,
                            bytes,
                        });
                    }
                    Node::Void(_) | Node::Text(_) => {}
                }
            }
        }

        let mut assets: Vec<BundleAsset> = Vec::new();
        let mut asset_paths: HashMap<String, ()> = HashMap::new();
        let mut report = BundleExportReport::default();
        walk_nodes(
            &mut doc.children,
            document_base_dir,
            &mut assets,
            &mut asset_paths,
            &mut report,
        );
        (assets, report)
    }

    pub fn make_plate_value_bundle(
        value: PlateValue,
        document_base_dir: Option<PathBuf>,
    ) -> (PlateValue, Vec<BundleAsset>, BundleExportReport) {
        let mut doc = value.into_document();
        let (assets, report) =
            Self::bundle_local_images_in_document(&mut doc, document_base_dir.as_deref());
        (PlateValue::from_document(doc), assets, report)
    }

    fn normalize_image_src(src: &str) -> String {
        let src = src.trim();
        if src.is_empty() {
            return String::new();
        }
        if src.contains("://") || src.starts_with("data:") {
            return src.to_string();
        }

        let expanded = if let Some(rest) = src.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                format!("{home}/{rest}")
            } else {
                src.to_string()
            }
        } else {
            src.to_string()
        };

        let path = Path::new(&expanded);
        if !path.is_absolute() {
            return expanded;
        }
        if !path.exists() {
            return expanded;
        }
        if let Ok(canon) = std::fs::canonicalize(path) {
            return canon.to_string_lossy().to_string();
        }

        expanded
    }

    fn relative_path_to_slash_string(path: &Path) -> String {
        path.iter()
            .map(|c| c.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    fn normalize_image_src_for_document(&self, src: &str) -> String {
        let src = src.trim();
        if src.is_empty() {
            return String::new();
        }

        if src.starts_with("data:")
            || src.starts_with("http://")
            || src.starts_with("https://")
            || src.starts_with("resource://")
        {
            return src.to_string();
        }

        let normalized = if let Some(rest) = src.strip_prefix("file://") {
            Self::normalize_image_src(rest)
        } else {
            Self::normalize_image_src(src)
        };
        if normalized.is_empty() {
            return String::new();
        }

        let Some(document_base_dir) = self.document_base_dir.as_deref() else {
            return normalized;
        };

        let base_dir = std::fs::canonicalize(document_base_dir)
            .unwrap_or_else(|_| document_base_dir.to_path_buf());

        let path = PathBuf::from(&normalized);
        if !path.is_absolute() || !path.exists() {
            return normalized;
        }

        let canonical_path = std::fs::canonicalize(&path).unwrap_or(path);
        let Ok(rel) = canonical_path.strip_prefix(&base_dir) else {
            return normalized;
        };

        let rel = Self::relative_path_to_slash_string(rel);
        if rel.is_empty() { normalized } else { rel }
    }

    pub fn referenced_relative_image_paths(&self) -> Vec<String> {
        fn walk(nodes: &[Node], out: &mut HashSet<String>) {
            for node in nodes {
                match node {
                    Node::Element(el) => walk(&el.children, out),
                    Node::Void(v) if v.kind == "image" => {
                        let Some(src) = v.attrs.get("src").and_then(|v| v.as_str()) else {
                            continue;
                        };
                        let src = src.trim();
                        if src.is_empty()
                            || src.starts_with("data:")
                            || src.contains("://")
                            || Path::new(src).is_absolute()
                        {
                            continue;
                        }
                        out.insert(src.to_string());
                    }
                    Node::Void(_) | Node::Text(_) => {}
                }
            }
        }

        let mut out: HashSet<String> = HashSet::new();
        walk(&self.editor.doc().children, &mut out);
        let mut out: Vec<String> = out.into_iter().collect();
        out.sort();
        out
    }

    pub fn document_base_dir(&self) -> Option<PathBuf> {
        self.document_base_dir.clone()
    }

    pub fn portability_report(&self) -> PortabilityReport {
        fn walk(nodes: &[Node], out: &mut Vec<String>) {
            for node in nodes {
                match node {
                    Node::Element(el) => walk(&el.children, out),
                    Node::Void(v) if v.kind == "image" => {
                        if let Some(src) = v.attrs.get("src").and_then(|v| v.as_str()) {
                            out.push(src.to_string());
                        }
                    }
                    Node::Void(_) | Node::Text(_) => {}
                }
            }
        }

        let base_dir = self.document_base_dir.clone();
        let base_dir_canon = base_dir
            .as_deref()
            .and_then(|dir| std::fs::canonicalize(dir).ok())
            .or_else(|| base_dir.clone());

        let mut image_srcs: Vec<String> = Vec::new();
        walk(&self.editor.doc().children, &mut image_srcs);

        let mut report = PortabilityReport {
            base_dir,
            ..PortabilityReport::default()
        };

        for src in image_srcs {
            let src = src.trim().to_string();
            report.images_total += 1;

            if src.is_empty() {
                report.issues.push(PortabilityIssue {
                    level: PortabilityIssueLevel::Error,
                    src,
                    message: "Empty image src".to_string(),
                });
                continue;
            }

            if src.starts_with("data:") {
                report.images_data_url += 1;
                report.issues.push(PortabilityIssue {
                    level: PortabilityIssueLevel::Warning,
                    src,
                    message:
                        "Embedded data URL (large). Consider Collect Assets or Export Plate Bundle."
                            .to_string(),
                });
                continue;
            }

            if src.starts_with("http://") || src.starts_with("https://") {
                report.images_http += 1;
                report.issues.push(PortabilityIssue {
                    level: PortabilityIssueLevel::Warning,
                    src,
                    message: "External URL (not self-contained). Consider Collect Assets or Export Plate Bundle.".to_string(),
                });
                continue;
            }

            if src.starts_with("resource://") {
                report.images_resource += 1;
                continue;
            }

            let normalized = if let Some(rest) = src.strip_prefix("file://") {
                Self::normalize_image_src(rest)
            } else {
                Self::normalize_image_src(&src)
            };
            let path = PathBuf::from(&normalized);

            if path.is_absolute() {
                if path.exists() && path.is_file() {
                    report.images_absolute_ok += 1;
                    report.issues.push(PortabilityIssue {
                        level: PortabilityIssueLevel::Warning,
                        src,
                        message: "Absolute local path (not portable). Consider Collect Assets or Export Plate Bundle."
                            .to_string(),
                    });
                } else {
                    report.images_absolute_missing += 1;
                    report.issues.push(PortabilityIssue {
                        level: PortabilityIssueLevel::Error,
                        src,
                        message: "Missing absolute local file".to_string(),
                    });
                }
                continue;
            }

            let Some(base_dir_canon) = base_dir_canon.as_ref() else {
                report.images_relative_missing += 1;
                report.issues.push(PortabilityIssue {
                    level: PortabilityIssueLevel::Error,
                    src,
                    message: "Relative src but document has no base directory".to_string(),
                });
                continue;
            };

            let resolved = base_dir_canon.join(&path);
            if resolved.exists() && resolved.is_file() {
                report.images_relative_ok += 1;
            } else {
                report.images_relative_missing += 1;
                report.issues.push(PortabilityIssue {
                    level: PortabilityIssueLevel::Error,
                    src,
                    message: format!("Missing relative file at {}", resolved.to_string_lossy()),
                });
            }
        }

        report
    }

    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle().tab_stop(true);
        let editor = Editor::with_richtext_plugins();
        let text_block_order = text_block_paths(editor.doc(), editor.registry());
        Self {
            focus_handle,
            scroll_handle: ScrollHandle::new(),
            editor,
            selecting: false,
            selection_anchor: None,
            viewport_bounds: Bounds::default(),
            layout_cache: HashMap::new(),
            block_bounds_cache: HashMap::new(),
            embedded_image_cache: HashMap::new(),
            document_base_dir: None,
            text_block_order,
            find_query: None,
            find_matches: HashMap::new(),
            find_active: None,
            ime_marked_range: None,
            selected_block_path: None,
            columns_resizing: None,
            did_auto_focus: false,
            pending_notifications: Vec::new(),
        }
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn can_undo(&self) -> bool {
        self.editor.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.editor.can_redo()
    }

    pub fn has_selected_block(&self) -> bool {
        self.selected_block_sibling_info().is_some()
    }

    pub fn can_move_selected_block_up(&self) -> bool {
        self.selected_block_sibling_info()
            .map(|(_, ix, _)| ix > 0)
            .unwrap_or(false)
    }

    pub fn can_move_selected_block_down(&self) -> bool {
        self.selected_block_sibling_info()
            .map(|(_, ix, len)| ix + 1 < len)
            .unwrap_or(false)
    }

    pub fn plate_value(&self) -> PlateValue {
        PlateValue::from_document(self.editor.doc().clone())
    }

    pub fn load_plate_value(
        &mut self,
        value: PlateValue,
        document_base_dir: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let selection = Selection::collapsed(CorePoint::new(vec![0, 0], 0));
        self.editor = Editor::new(
            value.into_document(),
            selection,
            gpui_plate_core::PluginRegistry::richtext(),
        );
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
        self.layout_cache.clear();
        self.block_bounds_cache.clear();
        self.embedded_image_cache.clear();
        self.document_base_dir = document_base_dir;
        self.selected_block_path = None;
        self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
        self.find_active = None;
        self.recompute_find_matches();
        self.ime_marked_range = None;
        cx.notify();
    }

    pub fn set_document_base_dir(
        &mut self,
        document_base_dir: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.document_base_dir = document_base_dir;
        cx.notify();
    }

    pub fn set_find_query(&mut self, query: String, cx: &mut Context<Self>) {
        let query = query.trim().to_string();
        if query.is_empty() {
            self.find_query = None;
        } else {
            self.find_query = Some(query);
        }
        self.find_active = None;
        self.recompute_find_matches();
        cx.notify();
    }

    fn recompute_find_matches(&mut self) {
        self.find_matches.clear();
        let Some(query) = self.find_query.as_ref().map(|s| s.as_str()) else {
            self.find_active = None;
            return;
        };
        if query.is_empty() {
            self.find_active = None;
            return;
        }

        for block_path in &self.text_block_order {
            let Some(text) = self.text_block_text(block_path) else {
                continue;
            };
            let text = text.as_ref();
            if text.is_empty() {
                continue;
            }

            let mut ranges: Vec<Range<usize>> = Vec::new();
            let mut cursor = 0usize;
            while cursor <= text.len() {
                let Some(found) = text.get(cursor..).and_then(|s| s.find(query)) else {
                    break;
                };
                let start = cursor + found;
                let end = start + query.len();
                if start >= end || end > text.len() {
                    break;
                }
                ranges.push(start..end);
                cursor = end;
            }

            if !ranges.is_empty() {
                self.find_matches.insert(block_path.clone(), ranges);
            }
        }

        if let Some(active) = self.find_active.as_ref() {
            if self
                .find_matches
                .get(&active.block_path)
                .and_then(|ranges| ranges.get(active.range_index))
                .is_none()
            {
                self.find_active = None;
            }
        }
    }

    pub fn find_stats(&self) -> (usize, usize) {
        let total = self.find_matches.values().map(|ranges| ranges.len()).sum();
        let Some(active) = self.find_active.as_ref() else {
            return (0, total);
        };

        let mut cursor = 0usize;
        for block_path in &self.text_block_order {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if &active.block_path == block_path {
                if active.range_index < ranges.len() {
                    return (cursor + active.range_index + 1, total);
                }
                return (0, total);
            }
            cursor += ranges.len();
        }

        (0, total)
    }

    pub fn find_next(&mut self, cx: &mut Context<Self>) {
        let has_query = self.find_query.as_deref().is_some_and(|q| !q.is_empty());
        if !has_query || self.find_matches.is_empty() {
            return;
        }

        let next = self
            .find_active
            .as_ref()
            .and_then(|active| self.find_next_after_active(active))
            .or_else(|| self.find_next_from_focus());

        if let Some(next) = next {
            self.jump_to_find_match(next, cx);
        }
    }

    pub fn find_prev(&mut self, cx: &mut Context<Self>) {
        let has_query = self.find_query.as_deref().is_some_and(|q| !q.is_empty());
        if !has_query || self.find_matches.is_empty() {
            return;
        }

        let prev = self
            .find_active
            .as_ref()
            .and_then(|active| self.find_prev_before_active(active))
            .or_else(|| self.find_prev_from_focus());

        if let Some(prev) = prev {
            self.jump_to_find_match(prev, cx);
        }
    }

    fn jump_to_find_match(&mut self, target: FindActiveMatch, cx: &mut Context<Self>) {
        let Some(range) = self
            .find_matches
            .get(&target.block_path)
            .and_then(|ranges| ranges.get(target.range_index))
            .cloned()
        else {
            return;
        };
        let Some(point) = self.point_for_block_offset(&target.block_path, range.start) else {
            return;
        };
        self.find_active = Some(target);
        self.set_selection_points(point.clone(), point, cx);
    }

    fn find_next_after_active(&self, active: &FindActiveMatch) -> Option<FindActiveMatch> {
        let Some(block_pos) = self
            .text_block_order
            .iter()
            .position(|path| path == &active.block_path)
        else {
            return None;
        };
        let ranges = self.find_matches.get(&active.block_path)?;
        if active.range_index >= ranges.len() {
            return None;
        }
        if active.range_index + 1 < ranges.len() {
            return Some(FindActiveMatch {
                block_path: active.block_path.clone(),
                range_index: active.range_index + 1,
            });
        }

        for block_path in self.text_block_order.iter().skip(block_pos + 1) {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: block_path.clone(),
                    range_index: 0,
                });
            }
        }

        for block_path in &self.text_block_order {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: block_path.clone(),
                    range_index: 0,
                });
            }
        }

        None
    }

    fn find_prev_before_active(&self, active: &FindActiveMatch) -> Option<FindActiveMatch> {
        let Some(block_pos) = self
            .text_block_order
            .iter()
            .position(|path| path == &active.block_path)
        else {
            return None;
        };
        let ranges = self.find_matches.get(&active.block_path)?;
        if active.range_index >= ranges.len() {
            return None;
        }
        if active.range_index > 0 {
            return Some(FindActiveMatch {
                block_path: active.block_path.clone(),
                range_index: active.range_index - 1,
            });
        }

        for block_path in self.text_block_order.iter().take(block_pos).rev() {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: block_path.clone(),
                    range_index: ranges.len() - 1,
                });
            }
        }

        for block_path in self.text_block_order.iter().rev() {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: block_path.clone(),
                    range_index: ranges.len() - 1,
                });
            }
        }

        None
    }

    fn find_next_from_focus(&self) -> Option<FindActiveMatch> {
        let focus = &self.editor.selection().focus;
        let (_, block_path) = focus.path.split_last()?;
        let block_path: Vec<usize> = block_path.to_vec();
        let focus_offset = self.row_offset_for_point(focus).unwrap_or(0);
        let anchor_pos = self
            .text_block_order
            .iter()
            .position(|path| path == &block_path)
            .unwrap_or(0);

        if let Some(ranges) = self.find_matches.get(&block_path) {
            for (ix, range) in ranges.iter().enumerate() {
                if range.start <= focus_offset && focus_offset < range.end {
                    return Some(FindActiveMatch {
                        block_path,
                        range_index: ix,
                    });
                }
            }
            for (ix, range) in ranges.iter().enumerate() {
                if range.start >= focus_offset {
                    return Some(FindActiveMatch {
                        block_path,
                        range_index: ix,
                    });
                }
            }
        }

        for next_block in self.text_block_order.iter().skip(anchor_pos + 1) {
            let Some(ranges) = self.find_matches.get(next_block) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: next_block.clone(),
                    range_index: 0,
                });
            }
        }

        for block_path in &self.text_block_order {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: block_path.clone(),
                    range_index: 0,
                });
            }
        }

        None
    }

    fn find_prev_from_focus(&self) -> Option<FindActiveMatch> {
        let focus = &self.editor.selection().focus;
        let (_, block_path) = focus.path.split_last()?;
        let block_path: Vec<usize> = block_path.to_vec();
        let focus_offset = self.row_offset_for_point(focus).unwrap_or(0);
        let anchor_pos = self
            .text_block_order
            .iter()
            .position(|path| path == &block_path)
            .unwrap_or(0);

        if let Some(ranges) = self.find_matches.get(&block_path) {
            for (ix, range) in ranges.iter().enumerate() {
                if range.start <= focus_offset && focus_offset < range.end {
                    return Some(FindActiveMatch {
                        block_path,
                        range_index: ix,
                    });
                }
            }

            for (ix, range) in ranges.iter().enumerate().rev() {
                if range.end <= focus_offset {
                    return Some(FindActiveMatch {
                        block_path,
                        range_index: ix,
                    });
                }
            }
        }

        for prev_block in self.text_block_order.iter().take(anchor_pos).rev() {
            let Some(ranges) = self.find_matches.get(prev_block) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: prev_block.clone(),
                    range_index: ranges.len() - 1,
                });
            }
        }

        for block_path in self.text_block_order.iter().rev() {
            let Some(ranges) = self.find_matches.get(block_path) else {
                continue;
            };
            if !ranges.is_empty() {
                return Some(FindActiveMatch {
                    block_path: block_path.clone(),
                    range_index: ranges.len() - 1,
                });
            }
        }

        None
    }

    fn active_text_block_path(&self) -> Option<Vec<usize>> {
        self.editor
            .selection()
            .focus
            .path
            .split_last()
            .map(|(_, p)| p.to_vec())
    }

    fn set_selection_points(
        &mut self,
        anchor: CorePoint,
        focus: CorePoint,
        cx: &mut Context<Self>,
    ) {
        self.editor.set_selection(Selection { anchor, focus });
        _ = self.scroll_cursor_into_view();
        self.ime_marked_range = None;
        self.selected_block_path = None;
        cx.notify();
    }

    fn active_text(&self) -> SharedString {
        let Some(block_path) = self.active_text_block_path() else {
            return SharedString::default();
        };
        if let Some(cache) = self.layout_cache.get(&block_path) {
            return cache.text.clone();
        }
        self.text_block_text(&block_path).unwrap_or_default()
    }

    pub fn active_marks(&self) -> Marks {
        let path = &self.editor.selection().focus.path;
        match node_at_path(self.editor.doc(), path) {
            Some(Node::Text(t)) => t.marks.clone(),
            _ => Marks::default(),
        }
    }

    fn row_offset_for_point(&self, point: &CorePoint) -> Option<usize> {
        let (child_ix, block_path) = point.path.split_last()?;
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return None;
        };
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        let mut global = 0usize;
        for (ix, child) in el.children.iter().enumerate() {
            match child {
                Node::Text(t) => {
                    if ix == *child_ix {
                        return Some(global + point.offset.min(t.text.len()));
                    }
                    global += t.text.len();
                }
                Node::Void(v) => {
                    global += v.inline_text_len();
                }
                Node::Element(_) => {}
            }
        }

        Some(global)
    }

    fn point_for_block_offset(&self, block_path: &[usize], offset: usize) -> Option<CorePoint> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        Some(Self::point_for_block_offset_in_children(
            block_path,
            &el.children,
            offset,
        ))
    }

    fn active_list_type(&self) -> Option<SharedString> {
        self.editor
            .run_query::<Option<String>>("list.active_type", None)
            .ok()
            .flatten()
            .map(SharedString::new)
    }

    fn set_selection_in_active_block(
        &mut self,
        anchor: usize,
        focus: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let anchor_point = self
            .point_for_block_offset(&block_path, anchor)
            .unwrap_or_else(|| {
                let mut path = block_path.clone();
                path.push(0);
                CorePoint::new(path, 0)
            });
        let focus_point = self
            .point_for_block_offset(&block_path, focus)
            .unwrap_or_else(|| {
                let mut path = block_path.clone();
                path.push(0);
                CorePoint::new(path, 0)
            });
        let sel = Selection {
            anchor: anchor_point,
            focus: focus_point,
        };
        self.editor.set_selection(sel);
        _ = self.scroll_cursor_into_view();
        self.ime_marked_range = None;
        self.selected_block_path = None;
        cx.notify();
    }

    fn ordered_active_range(&self) -> Range<usize> {
        let sel = self.editor.selection();
        let Some(active_block) = self.active_text_block_path() else {
            return 0..0;
        };
        let anchor_block = sel.anchor.path.split_last().map(|(_, p)| p);
        let focus_block = sel.focus.path.split_last().map(|(_, p)| p);
        if anchor_block != Some(active_block.as_slice())
            || focus_block != Some(active_block.as_slice())
        {
            let focus = self.row_offset_for_point(&sel.focus).unwrap_or(0);
            return focus..focus;
        }
        let mut a = self.row_offset_for_point(&sel.anchor).unwrap_or(0);
        let mut b = self.row_offset_for_point(&sel.focus).unwrap_or(0);
        if b < a {
            std::mem::swap(&mut a, &mut b);
        }
        a..b
    }

    fn text_block_text(&self, block_path: &[usize]) -> Option<SharedString> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }
        let mut out = String::new();
        for child in &el.children {
            match child {
                Node::Text(t) => out.push_str(&t.text),
                Node::Void(v) => out.push_str(&v.inline_text()),
                Node::Element(_) => {}
            }
        }
        Some(out.into())
    }

    fn prev_boundary(text: &str, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let mut ix = (offset - 1).min(text.len());
        while ix > 0 && !text.is_char_boundary(ix) {
            ix -= 1;
        }
        ix
    }

    fn next_boundary(text: &str, offset: usize) -> usize {
        if offset >= text.len() {
            return text.len();
        }
        let mut ix = (offset + 1).min(text.len());
        while ix < text.len() && !text.is_char_boundary(ix) {
            ix += 1;
        }
        ix
    }

    fn inline_void_ranges_in_block(&self, block_path: &[usize]) -> Vec<Range<usize>> {
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return Vec::new();
        };
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return Vec::new();
        }

        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut cursor = 0usize;
        for child in &el.children {
            match child {
                Node::Text(t) => cursor += t.text.len(),
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    if len > 0 {
                        ranges.push(cursor..cursor + len);
                        cursor += len;
                    }
                }
                Node::Element(_) => {}
            }
        }
        ranges
    }

    fn prev_cursor_offset_in_block(
        &self,
        block_path: &[usize],
        text: &str,
        offset: usize,
    ) -> usize {
        if offset == 0 {
            return 0;
        }

        let void_ranges = self.inline_void_ranges_in_block(block_path);
        for range in &void_ranges {
            if offset > range.start && offset <= range.end {
                return range.start;
            }
        }

        let mut prev = Self::prev_boundary(text, offset);
        for range in &void_ranges {
            if prev >= range.start && prev < range.end {
                prev = range.start;
                break;
            }
        }
        prev
    }

    fn next_cursor_offset_in_block(
        &self,
        block_path: &[usize],
        text: &str,
        offset: usize,
    ) -> usize {
        if offset >= text.len() {
            return text.len();
        }

        let void_ranges = self.inline_void_ranges_in_block(block_path);
        for range in &void_ranges {
            if offset >= range.start && offset < range.end {
                return range.end;
            }
        }

        let mut next = Self::next_boundary(text, offset);
        for range in &void_ranges {
            if next > range.start && next <= range.end {
                next = range.end;
                break;
            }
        }
        next
    }

    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn word_range(text: &str, offset: usize) -> Range<usize> {
        if text.is_empty() {
            return 0..0;
        }

        let offset = offset.min(text.len());
        let mut start = offset;
        while start > 0 && !text.is_char_boundary(start) {
            start -= 1;
        }
        let mut end = offset;
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }

        let probe_ix = if start == text.len() {
            Self::prev_boundary(text, start)
        } else if start == end && start > 0 {
            Self::prev_boundary(text, start)
        } else {
            start
        };
        let probe = text.get(probe_ix..).and_then(|s| s.chars().next());
        let Some(probe) = probe else {
            return 0..0;
        };

        if probe.is_whitespace() {
            let mut a = probe_ix;
            while a > 0 {
                let prev = Self::prev_boundary(text, a);
                let Some(ch) = text.get(prev..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                a = prev;
            }
            let mut b = probe_ix + probe.len_utf8();
            while b < text.len() {
                let next = Self::next_boundary(text, b);
                let Some(ch) = text.get(b..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                b = next;
            }
            return a..b;
        }

        if Self::is_word_char(probe) {
            let mut a = probe_ix;
            while a > 0 {
                let prev = Self::prev_boundary(text, a);
                let Some(ch) = text.get(prev..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !Self::is_word_char(ch) {
                    break;
                }
                a = prev;
            }
            let mut b = probe_ix + probe.len_utf8();
            while b < text.len() {
                let next = Self::next_boundary(text, b);
                let Some(ch) = text.get(b..).and_then(|s| s.chars().next()) else {
                    break;
                };
                if !Self::is_word_char(ch) {
                    break;
                }
                b = next;
            }
            return a..b;
        }

        probe_ix..probe_ix + probe.len_utf8()
    }

    fn word_or_void_range_in_block(
        &self,
        block_path: &[usize],
        text: &str,
        offset: usize,
    ) -> Range<usize> {
        for range in self.inline_void_ranges_in_block(block_path) {
            if offset >= range.start && offset < range.end {
                return range;
            }
        }
        Self::word_range(text, offset)
    }

    fn clamp_to_char_boundary(s: &str, mut ix: usize) -> usize {
        ix = ix.min(s.len());
        while ix > 0 && !s.is_char_boundary(ix) {
            ix -= 1;
        }
        ix
    }

    fn split_inline_children_at_offset(children: &[Node], offset: usize) -> (Vec<Node>, Vec<Node>) {
        let mut left: Vec<Node> = Vec::new();
        let mut right: Vec<Node> = Vec::new();

        let mut cursor = 0usize;
        let mut splitting = false;

        for child in children {
            if splitting {
                right.push(child.clone());
                continue;
            }

            match child {
                Node::Text(text) => {
                    let len = text.text.len();
                    if cursor + len <= offset {
                        left.push(Node::Text(text.clone()));
                        cursor += len;
                        continue;
                    }

                    if cursor >= offset {
                        splitting = true;
                        right.push(Node::Text(text.clone()));
                        continue;
                    }

                    let local = offset.saturating_sub(cursor).min(len);
                    let local = Self::clamp_to_char_boundary(&text.text, local);

                    let prefix = text.text.get(..local).unwrap_or("").to_string();
                    let suffix = text.text.get(local..).unwrap_or("").to_string();

                    if !prefix.is_empty() {
                        left.push(Node::Text(TextNode {
                            text: prefix,
                            marks: text.marks.clone(),
                        }));
                    }
                    if !suffix.is_empty() {
                        right.push(Node::Text(TextNode {
                            text: suffix,
                            marks: text.marks.clone(),
                        }));
                    }
                    splitting = true;
                }
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    if cursor + len <= offset {
                        left.push(child.clone());
                        cursor += len;
                        continue;
                    }

                    if cursor >= offset {
                        splitting = true;
                        right.push(child.clone());
                        continue;
                    }

                    // Offset fell "inside" a void segment. Snap to the nearest boundary so the
                    // void stays atomic.
                    let local = offset.saturating_sub(cursor).min(len);
                    if local <= len / 2 {
                        splitting = true;
                        right.push(child.clone());
                    } else {
                        left.push(child.clone());
                        cursor += len;
                        splitting = true;
                    }
                }
                Node::Element(_) => {
                    left.push(child.clone());
                }
            }
        }

        (left, right)
    }

    fn merge_adjacent_text_nodes(nodes: Vec<Node>) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        for node in nodes {
            match (&mut out.last_mut(), &node) {
                (Some(Node::Text(prev)), Node::Text(next)) if prev.marks == next.marks => {
                    prev.text.push_str(&next.text);
                }
                _ => out.push(node),
            }
        }
        out
    }

    fn ensure_has_text_leaf(mut children: Vec<Node>, marks: &Marks) -> Vec<Node> {
        let has_text = children.iter().any(|n| matches!(n, Node::Text(_)));
        if !has_text {
            children.push(Node::Text(TextNode {
                text: String::new(),
                marks: marks.clone(),
            }));
        }
        children
    }

    fn point_for_block_offset_in_children(
        block_path: &[usize],
        children: &[Node],
        offset: usize,
    ) -> CorePoint {
        let mut text_nodes: Vec<(usize, usize)> = Vec::new();
        for (child_ix, child) in children.iter().enumerate() {
            if let Node::Text(t) = child {
                text_nodes.push((child_ix, t.text.len()));
            }
        }
        if text_nodes.is_empty() {
            let mut path = block_path.to_vec();
            path.push(0);
            return CorePoint::new(path, 0);
        }

        let total: usize = children
            .iter()
            .map(|node| match node {
                Node::Text(t) => t.text.len(),
                Node::Void(v) => v.inline_text_len(),
                Node::Element(_) => 0,
            })
            .sum();

        let mut remaining = offset.min(total);
        for (child_ix, child) in children.iter().enumerate() {
            match child {
                Node::Text(t) => {
                    let len = t.text.len();
                    if remaining < len {
                        let mut path = block_path.to_vec();
                        path.push(child_ix);
                        return CorePoint::new(path, remaining);
                    }
                    if remaining == len {
                        if matches!(children.get(child_ix + 1), Some(Node::Text(_))) {
                            let mut path = block_path.to_vec();
                            path.push(child_ix + 1);
                            return CorePoint::new(path, 0);
                        }
                        let mut path = block_path.to_vec();
                        path.push(child_ix);
                        return CorePoint::new(path, len);
                    }
                    remaining = remaining.saturating_sub(len);
                }
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    if len == 0 {
                        continue;
                    }
                    if remaining < len {
                        let before = remaining;
                        let after = len - remaining;
                        if before <= after {
                            for (prev_ix, prev) in children.iter().enumerate().take(child_ix).rev()
                            {
                                if let Node::Text(t) = prev {
                                    let mut path = block_path.to_vec();
                                    path.push(prev_ix);
                                    return CorePoint::new(path, t.text.len());
                                }
                            }
                        }

                        for (next_ix, next) in children.iter().enumerate().skip(child_ix + 1) {
                            if matches!(next, Node::Text(_)) {
                                let mut path = block_path.to_vec();
                                path.push(next_ix);
                                return CorePoint::new(path, 0);
                            }
                        }

                        let (last_ix, last_len) = text_nodes.last().copied().unwrap();
                        let mut path = block_path.to_vec();
                        path.push(last_ix);
                        return CorePoint::new(path, last_len);
                    }

                    if remaining == len {
                        for (next_ix, next) in children.iter().enumerate().skip(child_ix + 1) {
                            if matches!(next, Node::Text(_)) {
                                let mut path = block_path.to_vec();
                                path.push(next_ix);
                                return CorePoint::new(path, 0);
                            }
                        }
                        let (last_ix, last_len) = text_nodes.last().copied().unwrap();
                        let mut path = block_path.to_vec();
                        path.push(last_ix);
                        return CorePoint::new(path, last_len);
                    }

                    remaining = remaining.saturating_sub(len);
                }
                Node::Element(_) => {}
            }
        }

        let (last_ix, last_len) = text_nodes.last().copied().unwrap();
        let mut path = block_path.to_vec();
        path.push(last_ix);
        CorePoint::new(path, last_len)
    }

    fn tx_replace_range_in_block(
        &self,
        block_path: &[usize],
        range: Range<usize>,
        inserted: &str,
        inserted_marks: &Marks,
        selection_after: Option<(usize, usize)>,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        if inserted.contains('\n') {
            return None;
        }

        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        let old_children = el.children.clone();
        let row_len = self.block_text_len_in_block(block_path);
        let start = range.start.min(row_len);
        let end = range.end.min(row_len).max(start);

        let (left, rest) = Self::split_inline_children_at_offset(&old_children, start);
        let (_removed, right) = Self::split_inline_children_at_offset(&rest, end - start);

        let mut new_children = left;
        if !inserted.is_empty() {
            new_children.push(Node::Text(TextNode {
                text: inserted.to_string(),
                marks: inserted_marks.clone(),
            }));
        }
        new_children.extend(right);
        new_children = Self::merge_adjacent_text_nodes(new_children);
        new_children = Self::ensure_has_text_leaf(new_children, inserted_marks);

        let (anchor, focus) = if let Some((a, b)) = selection_after {
            (
                Self::point_for_block_offset_in_children(block_path, &new_children, a),
                Self::point_for_block_offset_in_children(block_path, &new_children, b),
            )
        } else {
            let caret = start + inserted.len();
            let point = Self::point_for_block_offset_in_children(block_path, &new_children, caret);
            (point.clone(), point)
        };
        let selection_after = Selection { anchor, focus };

        let mut ops = Vec::new();
        for child_ix in (0..old_children.len()).rev() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in new_children.into_iter().enumerate() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn tx_replace_range_with_multiline_text_in_block(
        &self,
        block_path: &[usize],
        range: Range<usize>,
        inserted: &str,
        inserted_marks: &Marks,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        let parts: Vec<&str> = inserted.split('\n').collect();
        if parts.len() <= 1 {
            return self.tx_replace_range_in_block(
                block_path,
                range,
                inserted,
                inserted_marks,
                None,
                source,
            );
        }

        let is_list_item = el.kind == "list_item";
        let is_todo_item = el.kind == "todo_item";
        let block_attrs = el.attrs.clone();

        let old_children = el.children.clone();
        let row_len = self.block_text_len_in_block(block_path);
        let start = range.start.min(row_len);
        let end = range.end.min(row_len).max(start);

        let (left, rest) = Self::split_inline_children_at_offset(&old_children, start);
        let (_removed, right) = Self::split_inline_children_at_offset(&rest, end - start);

        let mut first_children = left;
        if !parts[0].is_empty() {
            first_children.push(Node::Text(TextNode {
                text: parts[0].to_string(),
                marks: inserted_marks.clone(),
            }));
        }
        first_children = Self::merge_adjacent_text_nodes(first_children);
        first_children = Self::ensure_has_text_leaf(first_children, inserted_marks);

        let last_ix = parts.len() - 1;
        let mut inserted_blocks: Vec<Node> = Vec::new();
        let mut last_children: Vec<Node> = Vec::new();

        for (i, part) in parts.iter().enumerate().skip(1) {
            let mut children: Vec<Node> = Vec::new();
            if !part.is_empty() {
                children.push(Node::Text(TextNode {
                    text: (*part).to_string(),
                    marks: inserted_marks.clone(),
                }));
            }
            if i == last_ix {
                children.extend(right.clone());
                children = Self::merge_adjacent_text_nodes(children);
                children = Self::ensure_has_text_leaf(children, inserted_marks);
                last_children = children.clone();
            } else {
                children = Self::merge_adjacent_text_nodes(children);
                children = Self::ensure_has_text_leaf(children, inserted_marks);
            }

            let node = if is_list_item {
                Node::Element(ElementNode {
                    kind: "list_item".to_string(),
                    attrs: block_attrs.clone(),
                    children,
                })
            } else if is_todo_item {
                let mut attrs = Attrs::default();
                if let Some(indent) = block_attrs.get("indent").cloned() {
                    attrs.insert("indent".to_string(), indent);
                }
                attrs.insert("checked".to_string(), serde_json::Value::Bool(false));
                Node::Element(ElementNode {
                    kind: "todo_item".to_string(),
                    attrs,
                    children,
                })
            } else {
                let mut attrs = Attrs::default();
                if let Some(indent) = block_attrs.get("indent").cloned() {
                    attrs.insert("indent".to_string(), indent);
                }
                Node::Element(ElementNode {
                    kind: "paragraph".to_string(),
                    attrs,
                    children,
                })
            };
            inserted_blocks.push(node);
        }

        let mut ops = Vec::new();
        for child_ix in (0..old_children.len()).rev() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in first_children.into_iter().enumerate() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        let (block_ix, parent_path) = block_path.split_last()?;
        for (i, node) in inserted_blocks.into_iter().enumerate() {
            let mut path = parent_path.to_vec();
            path.push(block_ix + i + 1);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        let mut last_block_path = parent_path.to_vec();
        last_block_path.push(block_ix + last_ix);
        let caret_offset = parts[last_ix].len();
        let caret_point = Self::point_for_block_offset_in_children(
            &last_block_path,
            &last_children,
            caret_offset,
        );

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(Selection::collapsed(caret_point))
                .source(source),
        )
    }

    fn tx_insert_multiline_text_at_block_offset(
        &self,
        block_path: &[usize],
        offset: usize,
        inserted: &str,
        inserted_marks: &Marks,
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        self.tx_replace_range_with_multiline_text_in_block(
            block_path,
            offset..offset,
            inserted,
            inserted_marks,
            source,
        )
    }

    fn delete_selection_transaction(&self) -> Option<gpui_plate_core::Transaction> {
        let selection = self.editor.selection().clone();
        if selection.is_collapsed() {
            return None;
        }

        let mut start = selection.anchor.clone();
        let mut end = selection.focus.clone();
        if start.path == end.path {
            if end.offset < start.offset {
                std::mem::swap(&mut start, &mut end);
            }
        } else if end.path < start.path {
            std::mem::swap(&mut start, &mut end);
        }

        let start_offset = self.row_offset_for_point(&start)?;
        let end_offset = self.row_offset_for_point(&end)?;

        let start_block: Vec<usize> = start.path.split_last().map(|(_, p)| p.to_vec())?;
        let end_block: Vec<usize> = end.path.split_last().map(|(_, p)| p.to_vec())?;

        let marks = self.active_marks();

        if start_block == end_block {
            return self.tx_replace_range_in_block(
                &start_block,
                start_offset..end_offset,
                "",
                &marks,
                None,
                "selection:delete",
            );
        }

        let start_el = element_at_path(self.editor.doc(), &start_block)?;
        let end_el = element_at_path(self.editor.doc(), &end_block)?;
        if !is_text_block_kind(self.editor.registry(), &start_el.kind)
            || !is_text_block_kind(self.editor.registry(), &end_el.kind)
        {
            return None;
        }

        let (start_ix, start_parent) = start_block.split_last()?;
        let (end_ix, end_parent) = end_block.split_last()?;

        let list_compatible = if start_el.kind == "list_item" {
            if end_el.kind != "list_item" {
                false
            } else {
                let start_type = start_el.attrs.get("list_type").and_then(|v| v.as_str());
                let end_type = end_el.attrs.get("list_type").and_then(|v| v.as_str());
                let start_level = start_el
                    .attrs
                    .get("list_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let end_level = end_el
                    .attrs
                    .get("list_level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                start_type == end_type && start_level == end_level
            }
        } else {
            start_el.kind == end_el.kind
        };

        let same_parent = start_parent == end_parent && *start_ix <= *end_ix;
        if list_compatible && same_parent {
            let start_children = start_el.children.clone();
            let end_children = end_el.children.clone();

            let (start_left, _start_right) =
                Self::split_inline_children_at_offset(&start_children, start_offset);
            let (_end_left, end_right) =
                Self::split_inline_children_at_offset(&end_children, end_offset);

            let mut merged_children = start_left;
            merged_children.extend(end_right);
            merged_children = Self::merge_adjacent_text_nodes(merged_children);
            merged_children = Self::ensure_has_text_leaf(merged_children, &marks);

            let selection_after = Selection::collapsed(Self::point_for_block_offset_in_children(
                &start_block,
                &merged_children,
                start_offset,
            ));

            let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
            for child_ix in (0..start_children.len()).rev() {
                let mut path = start_block.clone();
                path.push(child_ix);
                ops.push(gpui_plate_core::Op::RemoveNode { path });
            }
            for (child_ix, node) in merged_children.into_iter().enumerate() {
                let mut path = start_block.clone();
                path.push(child_ix);
                ops.push(gpui_plate_core::Op::InsertNode { path, node });
            }

            for ix in (*start_ix + 1..=*end_ix).rev() {
                let mut path = start_parent.to_vec();
                path.push(ix);
                ops.push(gpui_plate_core::Op::RemoveNode { path });
            }

            return Some(
                gpui_plate_core::Transaction::new(ops)
                    .selection_after(selection_after)
                    .source("selection:delete_multi_block_merge"),
            );
        }

        let blocks = text_block_paths(self.editor.doc(), self.editor.registry());
        let start_pos = blocks.iter().position(|p| p == &start_block)?;
        let end_pos = blocks.iter().position(|p| p == &end_block)?;
        let (start_pos, end_pos) = if start_pos <= end_pos {
            (start_pos, end_pos)
        } else {
            (end_pos, start_pos)
        };

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        let mut selection_after: Option<Selection> = None;

        for (ix, block_path) in blocks.iter().enumerate().take(end_pos + 1).skip(start_pos) {
            let len = self.block_text_len_in_block(block_path);
            let range = if ix == start_pos {
                start_offset..len
            } else if ix == end_pos {
                0..end_offset.min(len)
            } else {
                0..len
            };
            if range.start >= range.end {
                continue;
            }

            let Some(tx) = self.tx_replace_range_in_block(
                block_path,
                range,
                "",
                &marks,
                None,
                "selection:delete:block",
            ) else {
                continue;
            };

            if selection_after.is_none() && ix == start_pos {
                selection_after = tx.selection_after.clone();
            }
            ops.extend(tx.ops);
        }

        let selection_after = selection_after.unwrap_or_else(|| {
            Selection::collapsed(
                self.point_for_block_offset(&start_block, start_offset)
                    .unwrap_or_else(|| CorePoint::new(start.path.clone(), start.offset)),
            )
        });

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source("selection:delete_multi_block_clear"),
        )
    }

    fn delete_selection_if_any(&mut self, cx: &mut Context<Self>) -> Option<CorePoint> {
        let tx = self.delete_selection_transaction()?;
        self.push_tx(tx, cx);
        Some(self.editor.selection().focus.clone())
    }

    fn tx_join_block_with_previous_in_block(
        &self,
        block_path: &[usize],
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        let (block_ix, parent_path) = block_path.split_last()?;
        if *block_ix == 0 {
            return None;
        }

        let mut prev_path = parent_path.to_vec();
        prev_path.push(block_ix.saturating_sub(1));

        let doc = self.editor.doc();
        let prev_el = element_at_path(doc, &prev_path)?;
        let cur_el = element_at_path(doc, block_path)?;

        if !is_text_block_kind(self.editor.registry(), &prev_el.kind)
            || !is_text_block_kind(self.editor.registry(), &cur_el.kind)
        {
            return None;
        }

        if cur_el.kind == "list_item" {
            if prev_el.kind != "list_item" {
                return None;
            }
            let cur_type = cur_el.attrs.get("list_type").and_then(|v| v.as_str());
            let prev_type = prev_el.attrs.get("list_type").and_then(|v| v.as_str());
            let cur_level = cur_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let prev_level = prev_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if cur_type != prev_type || cur_level != prev_level {
                return None;
            }
        }

        let caret_offset = self.block_text_len_in_block(&prev_path);
        let marks = self.active_marks();

        let mut merged_children = prev_el.children.clone();
        merged_children.extend(cur_el.children.clone());
        merged_children = Self::merge_adjacent_text_nodes(merged_children);
        merged_children = Self::ensure_has_text_leaf(merged_children, &marks);

        let selection_after = Selection::collapsed(Self::point_for_block_offset_in_children(
            &prev_path,
            &merged_children,
            caret_offset,
        ));

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        for child_ix in (0..prev_el.children.len()).rev() {
            let mut path = prev_path.clone();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in merged_children.into_iter().enumerate() {
            let mut path = prev_path.clone();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }
        ops.push(gpui_plate_core::Op::RemoveNode {
            path: block_path.to_vec(),
        });

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn tx_join_block_with_next_in_block(
        &self,
        block_path: &[usize],
        source: &'static str,
    ) -> Option<gpui_plate_core::Transaction> {
        let (block_ix, parent_path) = block_path.split_last()?;
        let mut next_path = parent_path.to_vec();
        next_path.push(block_ix + 1);

        let doc = self.editor.doc();
        let cur_el = element_at_path(doc, block_path)?;
        let next_el = element_at_path(doc, &next_path)?;

        if !is_text_block_kind(self.editor.registry(), &cur_el.kind)
            || !is_text_block_kind(self.editor.registry(), &next_el.kind)
        {
            return None;
        }

        if cur_el.kind == "list_item" {
            if next_el.kind != "list_item" {
                return None;
            }
            let cur_type = cur_el.attrs.get("list_type").and_then(|v| v.as_str());
            let next_type = next_el.attrs.get("list_type").and_then(|v| v.as_str());
            let cur_level = cur_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let next_level = next_el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            if cur_type != next_type || cur_level != next_level {
                return None;
            }
        }

        let caret_offset = self.block_text_len_in_block(block_path);
        let marks = self.active_marks();

        let mut merged_children = cur_el.children.clone();
        merged_children.extend(next_el.children.clone());
        merged_children = Self::merge_adjacent_text_nodes(merged_children);
        merged_children = Self::ensure_has_text_leaf(merged_children, &marks);

        let selection_after = Selection::collapsed(Self::point_for_block_offset_in_children(
            block_path,
            &merged_children,
            caret_offset,
        ));

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        for child_ix in (0..cur_el.children.len()).rev() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::RemoveNode { path });
        }
        for (child_ix, node) in merged_children.into_iter().enumerate() {
            let mut path = block_path.to_vec();
            path.push(child_ix);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }
        ops.push(gpui_plate_core::Op::RemoveNode { path: next_path });

        Some(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source(source),
        )
    }

    fn prev_text_block_path(&self, block_path: &[usize]) -> Option<Vec<usize>> {
        let ix = self
            .text_block_order
            .iter()
            .position(|p| p.as_slice() == block_path)?;
        ix.checked_sub(1)
            .and_then(|prev| self.text_block_order.get(prev).cloned())
    }

    fn next_text_block_path(&self, block_path: &[usize]) -> Option<Vec<usize>> {
        let ix = self
            .text_block_order
            .iter()
            .position(|p| p.as_slice() == block_path)?;
        self.text_block_order.get(ix + 1).cloned()
    }

    fn offset_for_point_in_block(
        &self,
        block_path: &[usize],
        position: gpui::Point<Pixels>,
    ) -> Option<usize> {
        let cache = self.layout_cache.get(block_path)?;
        let offset_x = RichTextLineElement::align_offset_x_for_position(
            &cache.text_layout,
            &cache.bounds,
            position,
            cache.text_align,
        );
        let position = gpui::point(position.x - offset_x, position.y);
        let mut local = match cache.text_layout.index_for_position(position) {
            Ok(ix) | Err(ix) => ix,
        };
        local = local.min(cache.text.len());
        Some(local)
    }

    fn link_at_offset(&self, block_path: &[usize], offset: usize) -> Option<String> {
        let cache = self.layout_cache.get(block_path)?;

        let candidates = [offset, offset.saturating_sub(1)];
        for candidate in candidates {
            for seg in &cache.segments {
                if seg.len == 0 {
                    continue;
                }
                if candidate >= seg.start && candidate < seg.start + seg.len {
                    return seg.marks.link.clone();
                }
            }
        }

        None
    }

    fn link_run_range_in_block(&self, block_path: &[usize], offset: usize) -> Option<Range<usize>> {
        let el = element_at_path(self.editor.doc(), block_path)?;
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return None;
        }

        #[derive(Clone)]
        struct Seg {
            start: usize,
            len: usize,
            url: Option<String>,
        }

        let mut segs: Vec<Seg> = Vec::new();
        let mut global = 0usize;
        for child in &el.children {
            match child {
                Node::Text(t) => {
                    let len = t.text.len();
                    segs.push(Seg {
                        start: global,
                        len,
                        url: t.marks.link.clone(),
                    });
                    global += len;
                }
                Node::Void(v) => {
                    let len = v.inline_text_len();
                    segs.push(Seg {
                        start: global,
                        len,
                        url: None,
                    });
                    global += len;
                }
                Node::Element(_) => {}
            }
        }

        let candidates = [offset, offset.saturating_sub(1)];
        for candidate in candidates {
            for (i, seg) in segs.iter().enumerate() {
                if seg.len == 0 {
                    continue;
                }
                if candidate < seg.start || candidate >= seg.start + seg.len {
                    continue;
                }

                let url = seg.url.clone()?;
                let mut start = seg.start;
                let mut end = seg.start + seg.len;

                // Expand left: include adjacent segments with the same URL (even if other marks
                // differ), so editing a link updates the whole link run.
                let mut j = i;
                while j > 0 {
                    let prev = &segs[j - 1];
                    if prev.len == 0 {
                        j -= 1;
                        continue;
                    }
                    if prev.url.as_deref() == Some(url.as_str()) && prev.start + prev.len == start {
                        start = prev.start;
                        j -= 1;
                        continue;
                    }
                    break;
                }

                // Expand right.
                let mut j = i;
                while j + 1 < segs.len() {
                    let next = &segs[j + 1];
                    if next.len == 0 {
                        j += 1;
                        continue;
                    }
                    if next.url.as_deref() == Some(url.as_str()) && end == next.start {
                        end = next.start + next.len;
                        j += 1;
                        continue;
                    }
                    break;
                }

                if start < end {
                    return Some(start..end);
                }
                return None;
            }
        }

        None
    }

    fn block_text_len_in_block(&self, block_path: &[usize]) -> usize {
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return 0;
        };
        if !is_text_block_kind(self.editor.registry(), &el.kind) {
            return 0;
        }

        el.children
            .iter()
            .map(|child| match child {
                Node::Text(t) => t.text.len(),
                Node::Void(v) => v.inline_text_len(),
                Node::Element(_) => 0,
            })
            .sum()
    }

    fn text_align_for_block(&self, block_path: &[usize]) -> TextAlign {
        let Some(el) = element_at_path(self.editor.doc(), block_path) else {
            return TextAlign::Left;
        };
        match el.attrs.get("align").and_then(|v| v.as_str()) {
            Some("center") => TextAlign::Center,
            Some("right") => TextAlign::Right,
            _ => TextAlign::Left,
        }
    }

    fn text_block_path_for_point(&self, point: gpui::Point<Pixels>) -> Option<Vec<usize>> {
        for (path, cache) in &self.layout_cache {
            if cache.bounds.contains(&point) {
                return Some(path.clone());
            }
        }

        let mut best_containing: Option<Vec<usize>> = None;
        for (path, cache) in &self.block_bounds_cache {
            if !cache.bounds.contains(&point) {
                continue;
            }
            if !is_text_block_kind(self.editor.registry(), cache.kind.as_ref()) {
                continue;
            }

            match &best_containing {
                None => best_containing = Some(path.clone()),
                Some(best_path) if path.len() > best_path.len() => {
                    best_containing = Some(path.clone())
                }
                _ => {}
            }
        }
        if best_containing.is_some() {
            return best_containing;
        }

        let mut best: Option<(Pixels, Vec<usize>)> = None;
        for (path, cache) in &self.layout_cache {
            let dist = if point.y < cache.bounds.top() {
                cache.bounds.top() - point.y
            } else if point.y > cache.bounds.bottom() {
                point.y - cache.bounds.bottom()
            } else {
                px(0.)
            };
            match &best {
                None => best = Some((dist, path.clone())),
                Some((best_dist, _)) if dist < *best_dist => best = Some((dist, path.clone())),
                _ => {}
            }
        }
        best.map(|(_, path)| path)
    }

    fn void_block_path_for_point(&self, point: gpui::Point<Pixels>) -> Option<Vec<usize>> {
        let mut best: Option<(usize, Vec<usize>)> = None;
        for (path, cache) in &self.block_bounds_cache {
            if !cache.bounds.contains(&point) {
                continue;
            }
            let kind = cache.kind.as_ref();
            if kind != "divider" && kind != "image" {
                continue;
            }
            if !matches!(node_at_path(self.editor.doc(), path), Some(Node::Void(_))) {
                continue;
            }
            let depth = path.len();
            match &best {
                None => best = Some((depth, path.clone())),
                Some((best_depth, _)) if depth >= *best_depth => best = Some((depth, path.clone())),
                _ => {}
            }
        }
        best.map(|(_, path)| path)
    }

    fn columns_resize_target_for_point(
        &self,
        point: gpui::Point<Pixels>,
    ) -> Option<(Vec<usize>, usize)> {
        let mut best: Option<(usize, Vec<usize>, usize)> = None;

        for (columns_path, cache) in &self.block_bounds_cache {
            if cache.kind.as_ref() != "columns" {
                continue;
            }
            if !cache.bounds.contains(&point) {
                continue;
            }

            let Some(columns_el) = element_at_path(self.editor.doc(), columns_path) else {
                continue;
            };
            if columns_el.kind != "columns" {
                continue;
            }

            let col_count = columns_el.children.len();
            if col_count < 2 {
                continue;
            }

            for boundary_ix in 0..col_count.saturating_sub(1) {
                let mut left_path = columns_path.clone();
                left_path.push(boundary_ix);
                let Some(left_bounds) = self.block_bounds_cache.get(&left_path).map(|c| c.bounds)
                else {
                    continue;
                };

                let mut right_path = columns_path.clone();
                right_path.push(boundary_ix + 1);
                let Some(right_bounds) = self.block_bounds_cache.get(&right_path).map(|c| c.bounds)
                else {
                    continue;
                };

                let left = left_bounds.right();
                let right = right_bounds.left();
                let mid = left + (right - left) * 0.5;
                let half = px(6.);

                let x_ok = point.x >= mid - half && point.x <= mid + half;
                let y_ok = point.y >= cache.bounds.top() && point.y <= cache.bounds.bottom();
                if !x_ok || !y_ok {
                    continue;
                }

                let depth = columns_path.len();
                match &best {
                    None => best = Some((depth, columns_path.clone(), boundary_ix)),
                    Some((best_depth, _, _)) if depth > *best_depth => {
                        best = Some((depth, columns_path.clone(), boundary_ix))
                    }
                    _ => {}
                }
            }
        }

        best.map(|(_, path, ix)| (path, ix))
    }

    fn begin_columns_resize(
        &mut self,
        columns_path: Vec<usize>,
        boundary_ix: usize,
        start_mouse_x: Pixels,
        cx: &mut Context<Self>,
    ) {
        let Some(columns_el) = element_at_path(self.editor.doc(), &columns_path) else {
            return;
        };
        if columns_el.kind != "columns" {
            return;
        }

        let col_count = columns_el.children.len();
        if col_count < 2 || boundary_ix + 1 >= col_count {
            return;
        }

        let mut start_widths: Vec<Pixels> = Vec::with_capacity(col_count);
        for col_ix in 0..col_count {
            let mut col_path = columns_path.clone();
            col_path.push(col_ix);
            let Some(bounds) = self.block_bounds_cache.get(&col_path).map(|c| c.bounds) else {
                return;
            };
            start_widths.push(bounds.size.width);
        }

        let mut total_width = px(0.);
        for width in &start_widths {
            total_width += *width;
        }
        if total_width <= px(0.) {
            return;
        }

        let start_fractions: Vec<f32> = start_widths.iter().map(|w| *w / total_width).collect();

        self.columns_resizing = Some(ColumnsResizeState {
            columns_path,
            boundary_ix,
            start_mouse_x,
            total_width,
            start_widths: start_widths.clone(),
            start_fractions: start_fractions.clone(),
            preview_fractions: start_fractions,
        });
        self.selecting = false;
        self.selection_anchor = None;
        cx.notify();
    }

    fn update_columns_resize_preview(&mut self, mouse_x: Pixels, cx: &mut Context<Self>) {
        let Some(resize) = self.columns_resizing.as_mut() else {
            return;
        };
        if resize.boundary_ix + 1 >= resize.start_widths.len() {
            return;
        }

        let min_width = px(80.);
        let left_start = resize.start_widths[resize.boundary_ix];
        let right_start = resize.start_widths[resize.boundary_ix + 1];

        let mut delta_f = (mouse_x - resize.start_mouse_x) / px(1.);
        let max_increase_f = ((right_start - min_width) / px(1.)).max(0.0);
        let max_decrease_f = ((left_start - min_width) / px(1.)).max(0.0);

        if delta_f > max_increase_f {
            delta_f = max_increase_f;
        }
        if delta_f < -max_decrease_f {
            delta_f = -max_decrease_f;
        }

        let delta = px(delta_f);
        let new_left = left_start + delta;
        let new_right = right_start - delta;

        let mut preview = resize.start_fractions.clone();
        preview[resize.boundary_ix] = new_left / resize.total_width;
        preview[resize.boundary_ix + 1] = new_right / resize.total_width;
        resize.preview_fractions = preview;
        cx.notify();
    }

    fn commit_columns_resize(&mut self, cx: &mut Context<Self>) {
        let Some(resize) = self.columns_resizing.take() else {
            return;
        };
        let args = serde_json::json!({
            "path": resize.columns_path,
            "widths": resize.preview_fractions,
        });
        if !self.run_command_and_refresh("columns.set_widths", Some(args), cx) {
            cx.notify();
        }
    }

    fn selectable_block_paths_for_point(&self, point: gpui::Point<Pixels>) -> Vec<Vec<usize>> {
        let mut candidates: Vec<(u8, usize, Vec<usize>)> = Vec::new();
        for (path, cache) in &self.block_bounds_cache {
            if !cache.bounds.contains(&point) {
                continue;
            }

            let node = match node_at_path(self.editor.doc(), path) {
                Some(node) => node,
                None => continue,
            };

            let (score, kind) = match node {
                Node::Void(v) => (2u8, v.kind.as_str()),
                Node::Element(el) => {
                    let is_text = is_text_block_kind(self.editor.registry(), &el.kind);
                    (u8::from(!is_text), el.kind.as_str())
                }
                Node::Text(_) => continue,
            };

            let score = if kind == "divider" || kind == "image" {
                score
            } else {
                score.min(1)
            };

            candidates.push((score, path.len(), path.clone()));
        }

        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        candidates.into_iter().map(|(_, _, path)| path).collect()
    }

    fn refresh_after_doc_change(
        &mut self,
        selected_block_path_after: Option<Vec<usize>>,
        cx: &mut Context<Self>,
    ) {
        _ = self.scroll_cursor_into_view();
        self.layout_cache.clear();
        self.block_bounds_cache.clear();
        self.selected_block_path = selected_block_path_after;
        self.columns_resizing = None;
        self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
        self.recompute_find_matches();
        cx.notify();
    }

    fn push_tx_with_selected_block_path(
        &mut self,
        tx: gpui_plate_core::Transaction,
        selected_block_path_after: Option<Vec<usize>>,
        cx: &mut Context<Self>,
    ) {
        let source = tx.meta.source.as_deref().unwrap_or("tx:apply").to_string();
        match self.editor.apply(tx) {
            Ok(()) => self.refresh_after_doc_change(selected_block_path_after, cx),
            Err(err) => self.queue_notification(format!("Apply failed ({source}): {err:?}"), cx),
        }
    }

    fn push_tx(&mut self, tx: gpui_plate_core::Transaction, cx: &mut Context<Self>) {
        self.push_tx_with_selected_block_path(tx, None, cx);
    }

    fn run_command_and_refresh(
        &mut self,
        id: &str,
        args: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        match self.editor.run_command(id, args) {
            Ok(()) => {
                self.refresh_after_doc_change(None, cx);
                true
            }
            Err(err) => {
                self.queue_notification(format!("Command failed ({id}): {}", err.message()), cx);
                false
            }
        }
    }

    pub fn command_list(&self) -> Vec<crate::CommandInfo> {
        let mut out: Vec<crate::CommandInfo> = self
            .editor
            .registry()
            .commands()
            .iter()
            .filter_map(|(id, spec)| {
                if spec.hidden {
                    return None;
                }
                Some(crate::CommandInfo {
                    id: id.clone(),
                    label: spec.label.clone(),
                    description: spec.description.clone(),
                    keywords: spec.keywords.clone(),
                    args_example: spec.args_example.clone(),
                })
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn command_run(
        &mut self,
        id: &str,
        args: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.editor
            .run_command(id, args)
            .map_err(|e| e.message().to_string())?;
        self.refresh_after_doc_change(None, cx);
        Ok(())
    }

    fn scroll_cursor_into_view(&mut self) -> bool {
        let Some(block_path) = self.active_text_block_path() else {
            return false;
        };
        let Some(cache) = self.layout_cache.get(block_path.as_slice()) else {
            return false;
        };
        let viewport = self.viewport_bounds;
        if viewport.size.width == px(0.) && viewport.size.height == px(0.) {
            return false;
        }

        let caret_ix = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(cache.text_layout.len());
        let Some(pos) = cache.text_layout.position_for_index(caret_ix).or_else(|| {
            cache
                .text_layout
                .position_for_index(cache.text_layout.len())
        }) else {
            return false;
        };
        let offset_x = RichTextLineElement::align_offset_x_for_index(
            &cache.text_layout,
            &cache.bounds,
            caret_ix,
            cache.text_align,
        );
        let pos = point(pos.x + offset_x, pos.y);

        let line_height = cache.text_layout.line_height();
        let caret_bounds = Bounds::from_corners(pos, point(pos.x + px(1.5), pos.y + line_height));

        let prev_offset = self.scroll_handle.offset();
        let mut offset = prev_offset;

        let margin = px(12.);
        if caret_bounds.top() < viewport.top() + margin {
            offset.y += (viewport.top() + margin) - caret_bounds.top();
        } else if caret_bounds.bottom() > viewport.bottom() - margin {
            offset.y -= caret_bounds.bottom() - (viewport.bottom() - margin);
        }

        if offset == prev_offset {
            return false;
        }
        self.scroll_handle.set_offset(offset);
        true
    }

    pub(super) fn backspace(
        &mut self,
        _: &Backspace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(path) = self.selected_block_path.take() {
            if matches!(
                node_at_path(self.editor.doc(), &path),
                Some(Node::Void(_)) | Some(Node::Element(_))
            ) {
                self.push_tx(
                    gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveNode {
                        path,
                    }])
                    .source("key:backspace:remove_selected_block"),
                    cx,
                );
            }
            return;
        }

        if self.delete_selection_if_any(cx).is_some() {
            return;
        }

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(text.len());
        let start = self.prev_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        if start == cursor {
            if cursor == 0 {
                if let Some((block_ix, parent_path)) = block_path.split_last()
                    && *block_ix > 0
                {
                    let mut prev_path = parent_path.to_vec();
                    prev_path.push(block_ix.saturating_sub(1));

                    if matches!(
                        node_at_path(self.editor.doc(), &prev_path),
                        Some(Node::Void(v)) if v.kind == "divider" || v.kind == "image"
                    ) {
                        let source = match node_at_path(self.editor.doc(), &prev_path) {
                            Some(Node::Void(v)) if v.kind == "image" => {
                                "key:backspace:remove_image"
                            }
                            _ => "key:backspace:remove_divider",
                        };
                        self.push_tx(
                            gpui_plate_core::Transaction::new(vec![
                                gpui_plate_core::Op::RemoveNode { path: prev_path },
                            ])
                            .source(source),
                            cx,
                        );
                        return;
                    }

                    if let Some(tx) =
                        self.tx_join_block_with_previous_in_block(&block_path, "key:backspace:join")
                    {
                        self.push_tx(tx, cx);
                        return;
                    }
                }

                if self.active_list_type().is_some() {
                    _ = self.run_command_and_refresh("list.unwrap", None, cx);
                }
            }
            return;
        }
        let marks = self.active_marks();
        if let Some(tx) = self.tx_replace_range_in_block(
            &block_path,
            start..cursor,
            "",
            &marks,
            None,
            "key:backspace",
        ) {
            self.push_tx(tx, cx);
        }
    }

    pub(super) fn delete(&mut self, _: &Delete, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.selected_block_path.take() {
            if matches!(
                node_at_path(self.editor.doc(), &path),
                Some(Node::Void(_)) | Some(Node::Element(_))
            ) {
                self.push_tx(
                    gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveNode {
                        path,
                    }])
                    .source("key:delete:remove_selected_block"),
                    cx,
                );
            }
            return;
        }

        if self.delete_selection_if_any(cx).is_some() {
            return;
        }

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(text.len());
        let end = self.next_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        if end == cursor {
            if cursor == text.len()
                && let Some((block_ix, parent_path)) = block_path.split_last()
            {
                let mut next_path = parent_path.to_vec();
                next_path.push(block_ix + 1);

                if matches!(
                    node_at_path(self.editor.doc(), &next_path),
                    Some(Node::Void(v)) if v.kind == "divider" || v.kind == "image"
                ) {
                    let source = match node_at_path(self.editor.doc(), &next_path) {
                        Some(Node::Void(v)) if v.kind == "image" => "key:delete:remove_image",
                        _ => "key:delete:remove_divider",
                    };
                    self.push_tx(
                        gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveNode {
                            path: next_path,
                        }])
                        .source(source),
                        cx,
                    );
                    return;
                }

                if let Some(tx) =
                    self.tx_join_block_with_next_in_block(&block_path, "key:delete:join")
                {
                    self.push_tx(tx, cx);
                }
            }
            return;
        }
        let marks = self.active_marks();
        if let Some(tx) =
            self.tx_replace_range_in_block(&block_path, cursor..end, "", &marks, None, "key:delete")
        {
            self.push_tx(tx, cx);
        }
    }

    pub(super) fn enter(&mut self, _: &Enter, _window: &mut Window, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let Some(el) = element_at_path(self.editor.doc(), &block_path) else {
            return;
        };
        let block_kind = el.kind.as_str();

        let text_len = self.block_text_len_in_block(&block_path);
        let cursor = self
            .row_offset_for_point(&self.editor.selection().focus)
            .unwrap_or(0)
            .min(text_len);

        if text_len == 0 && cursor == 0 {
            if block_kind == "list_item" {
                _ = self.run_command_and_refresh("list.unwrap", None, cx);
                return;
            }
            if block_kind == "todo_item" {
                _ = self.run_command_and_refresh("todo.toggle", None, cx);
                return;
            }
        }

        let marks = self.active_marks();
        if let Some(tx) = self.tx_insert_multiline_text_at_block_offset(
            &block_path,
            cursor,
            "\n",
            &marks,
            if block_kind == "list_item" {
                "key:enter:split_list_item"
            } else {
                "key:enter:split_paragraph"
            },
        ) {
            self.push_tx(tx, cx);
        }
    }

    pub(super) fn left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        let sel = self.editor.selection().clone();
        if !sel.is_collapsed() {
            let mut start = sel.anchor.clone();
            let mut end = sel.focus.clone();
            if start.path == end.path {
                if end.offset < start.offset {
                    std::mem::swap(&mut start, &mut end);
                }
            } else if end.path < start.path {
                std::mem::swap(&mut start, &mut end);
            }
            self.set_selection_points(start.clone(), start, cx);
            return;
        }
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == 0 {
            if let Some(prev_path) = self.prev_text_block_path(&block_path) {
                let offset = self.block_text_len_in_block(&prev_path);
                let point = self
                    .point_for_block_offset(&prev_path, offset)
                    .unwrap_or_else(|| {
                        let mut path = prev_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                self.set_selection_points(point.clone(), point, cx);
                return;
            }
        }
        let new_cursor = self.prev_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        self.set_selection_in_active_block(new_cursor, new_cursor, cx);
    }

    pub(super) fn right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        let sel = self.editor.selection().clone();
        if !sel.is_collapsed() {
            let mut start = sel.anchor.clone();
            let mut end = sel.focus.clone();
            if start.path == end.path {
                if end.offset < start.offset {
                    std::mem::swap(&mut start, &mut end);
                }
            } else if end.path < start.path {
                std::mem::swap(&mut start, &mut end);
            }
            self.set_selection_points(end.clone(), end, cx);
            return;
        }
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == text.len() {
            if let Some(next_path) = self.next_text_block_path(&block_path) {
                let point = self
                    .point_for_block_offset(&next_path, 0)
                    .unwrap_or_else(|| {
                        let mut path = next_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                self.set_selection_points(point.clone(), point, cx);
                return;
            }
        }
        let new_cursor = self.next_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        self.set_selection_in_active_block(new_cursor, new_cursor, cx);
    }

    pub(super) fn select_left(
        &mut self,
        _: &SelectLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sel = self.editor.selection().clone();
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == 0 {
            if let Some(prev_path) = self.prev_text_block_path(&block_path) {
                let focus = self
                    .point_for_block_offset(&prev_path, self.block_text_len_in_block(&prev_path))
                    .unwrap_or_else(|| {
                        let mut path = prev_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                let anchor = if sel.is_collapsed() {
                    self.point_for_block_offset(&block_path, cursor)
                        .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
                } else {
                    sel.anchor.clone()
                };
                self.set_selection_points(anchor, focus, cx);
                return;
            }
        }
        let new_cursor = self.prev_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        let focus = self
            .point_for_block_offset(&block_path, new_cursor)
            .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset));
        let anchor = if sel.is_collapsed() {
            self.point_for_block_offset(&block_path, cursor)
                .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
        } else {
            sel.anchor.clone()
        };
        self.set_selection_points(anchor, focus, cx);
    }

    pub(super) fn select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sel = self.editor.selection().clone();
        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let text = self.text_block_text(&block_path).unwrap_or_default();
        let cursor = self
            .row_offset_for_point(&sel.focus)
            .unwrap_or(0)
            .min(text.len());
        if cursor == text.len() {
            if let Some(next_path) = self.next_text_block_path(&block_path) {
                let focus = self
                    .point_for_block_offset(&next_path, 0)
                    .unwrap_or_else(|| {
                        let mut path = next_path.clone();
                        path.push(0);
                        CorePoint::new(path, 0)
                    });
                let anchor = if sel.is_collapsed() {
                    self.point_for_block_offset(&block_path, cursor)
                        .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
                } else {
                    sel.anchor.clone()
                };
                self.set_selection_points(anchor, focus, cx);
                return;
            }
        }
        let new_cursor = self.next_cursor_offset_in_block(&block_path, text.as_str(), cursor);
        let focus = self
            .point_for_block_offset(&block_path, new_cursor)
            .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset));
        let anchor = if sel.is_collapsed() {
            self.point_for_block_offset(&block_path, cursor)
                .unwrap_or_else(|| CorePoint::new(sel.focus.path.clone(), sel.focus.offset))
        } else {
            sel.anchor.clone()
        };
        self.set_selection_points(anchor, focus, cx);
    }

    pub(super) fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.undo() {
            self.layout_cache.clear();
            self.block_bounds_cache.clear();
            self.selected_block_path = None;
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.redo() {
            self.layout_cache.clear();
            self.block_bounds_cache.clear();
            self.selected_block_path = None;
            self.text_block_order = text_block_paths(self.editor.doc(), self.editor.registry());
            cx.notify();
        }
    }

    pub(super) fn copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_copy(cx);
    }

    pub(super) fn cut(&mut self, _: &Cut, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_cut(cx);
    }

    pub(super) fn paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_paste(cx);
    }

    pub(super) fn select_all(
        &mut self,
        _: &SelectAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_select_all(cx);
    }

    pub(super) fn insert_divider(
        &mut self,
        _: &InsertDivider,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("core.insert_divider", None, cx);
    }

    pub(super) fn insert_mention(
        &mut self,
        _: &InsertMention,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "label": "Alice" });
        _ = self.run_command_and_refresh("mention.insert", Some(args), cx);
    }

    pub(super) fn toggle_bold(
        &mut self,
        _: &ToggleBold,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_bold", None, cx);
    }

    pub(super) fn toggle_italic(
        &mut self,
        _: &ToggleItalic,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_italic", None, cx);
    }

    pub(super) fn toggle_underline(
        &mut self,
        _: &ToggleUnderline,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_underline", None, cx);
    }

    pub(super) fn toggle_strikethrough(
        &mut self,
        _: &ToggleStrikethrough,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_strikethrough", None, cx);
    }

    pub(super) fn toggle_code(
        &mut self,
        _: &ToggleCode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("marks.toggle_code", None, cx);
    }

    pub(super) fn toggle_bulleted_list(
        &mut self,
        _: &ToggleBulletedList,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("list.toggle_bulleted", None, cx);
    }

    pub(super) fn toggle_ordered_list(
        &mut self,
        _: &ToggleOrderedList,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        _ = self.run_command_and_refresh("list.toggle_ordered", None, cx);
    }

    pub fn command_undo(&mut self, cx: &mut Context<Self>) {
        if self.editor.undo() {
            self.refresh_after_doc_change(None, cx);
        }
    }

    pub fn command_redo(&mut self, cx: &mut Context<Self>) {
        if self.editor.redo() {
            self.refresh_after_doc_change(None, cx);
        }
    }

    pub fn command_duplicate_selected_block(&mut self, cx: &mut Context<Self>) {
        let Some((parent_path, ix, _len)) = self.selected_block_sibling_info() else {
            return;
        };
        let Some(selected_path) = self.selected_block_path.clone() else {
            return;
        };
        let Some(node) = node_at_path(self.editor.doc(), &selected_path) else {
            return;
        };
        let node = match node {
            Node::Element(el) => Node::Element(el.clone()),
            Node::Void(v) => Node::Void(v.clone()),
            Node::Text(_) => return,
        };

        let mut inserted_path = parent_path;
        inserted_path.push(ix + 1);
        let mut tx = gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::InsertNode {
            path: inserted_path.clone(),
            node: node.clone(),
        }])
        .source("command:block.duplicate");
        if matches!(node, Node::Element(_)) {
            tx = tx.selection_after(Selection::collapsed(CorePoint::new(
                inserted_path.clone(),
                0,
            )));
        }

        self.push_tx_with_selected_block_path(tx, Some(inserted_path), cx);
    }

    pub fn command_move_selected_block_up(&mut self, cx: &mut Context<Self>) {
        let Some((parent_path, ix, _len)) = self.selected_block_sibling_info() else {
            return;
        };
        if ix == 0 {
            return;
        }
        let Some(selected_path) = self.selected_block_path.clone() else {
            return;
        };
        let Some(node) = node_at_path(self.editor.doc(), &selected_path) else {
            return;
        };
        let node = match node {
            Node::Element(el) => Node::Element(el.clone()),
            Node::Void(v) => Node::Void(v.clone()),
            Node::Text(_) => return,
        };

        let mut new_path = parent_path;
        new_path.push(ix - 1);
        let mut tx = gpui_plate_core::Transaction::new(vec![
            gpui_plate_core::Op::RemoveNode {
                path: selected_path,
            },
            gpui_plate_core::Op::InsertNode {
                path: new_path.clone(),
                node: node.clone(),
            },
        ])
        .source("command:block.move_up");
        if matches!(node, Node::Element(_)) {
            tx = tx.selection_after(Selection::collapsed(CorePoint::new(new_path.clone(), 0)));
        }

        self.push_tx_with_selected_block_path(tx, Some(new_path), cx);
    }

    pub fn command_move_selected_block_down(&mut self, cx: &mut Context<Self>) {
        let Some((parent_path, ix, len)) = self.selected_block_sibling_info() else {
            return;
        };
        if ix + 1 >= len {
            return;
        }
        let Some(selected_path) = self.selected_block_path.clone() else {
            return;
        };
        let Some(node) = node_at_path(self.editor.doc(), &selected_path) else {
            return;
        };
        let node = match node {
            Node::Element(el) => Node::Element(el.clone()),
            Node::Void(v) => Node::Void(v.clone()),
            Node::Text(_) => return,
        };

        let mut new_path = parent_path;
        new_path.push(ix + 1);
        let mut tx = gpui_plate_core::Transaction::new(vec![
            gpui_plate_core::Op::RemoveNode {
                path: selected_path,
            },
            gpui_plate_core::Op::InsertNode {
                path: new_path.clone(),
                node: node.clone(),
            },
        ])
        .source("command:block.move_down");
        if matches!(node, Node::Element(_)) {
            tx = tx.selection_after(Selection::collapsed(CorePoint::new(new_path.clone(), 0)));
        }

        self.push_tx_with_selected_block_path(tx, Some(new_path), cx);
    }

    pub fn command_delete_selected_block(&mut self, cx: &mut Context<Self>) {
        let Some(selected_path) = self.selected_block_path.clone() else {
            return;
        };
        if !matches!(
            node_at_path(self.editor.doc(), &selected_path),
            Some(Node::Element(_)) | Some(Node::Void(_))
        ) {
            return;
        }
        self.push_tx(
            gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveNode {
                path: selected_path,
            }])
            .source("command:block.delete"),
            cx,
        );
    }

    pub fn command_insert_divider(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("core.insert_divider", None, cx);
    }

    pub fn command_insert_image(
        &mut self,
        src: String,
        alt: Option<String>,
        cx: &mut Context<Self>,
    ) {
        _ = self.delete_selection_if_any(cx);
        let src = self.normalize_image_src_for_document(&src);
        let args = if let Some(alt) = alt {
            serde_json::json!({ "src": src, "alt": alt })
        } else {
            serde_json::json!({ "src": src })
        };
        _ = self.run_command_and_refresh("image.insert", Some(args), cx);
    }

    pub fn command_insert_images(&mut self, srcs: Vec<String>, cx: &mut Context<Self>) {
        let srcs: Vec<String> = srcs
            .into_iter()
            .map(|src| self.normalize_image_src_for_document(&src))
            .filter(|src| !src.is_empty())
            .collect();
        if srcs.is_empty() {
            return;
        }

        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "srcs": srcs });
        _ = self.run_command_and_refresh("image.insert_many", Some(args), cx);
    }

    pub fn command_embed_local_images(&mut self, cx: &mut Context<Self>) -> EmbedLocalImagesReport {
        fn walk(nodes: &[Node], path: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, String)>) {
            for (ix, node) in nodes.iter().enumerate() {
                path.push(ix);
                match node {
                    Node::Void(v) if v.kind == "image" => {
                        if let Some(src) = v.attrs.get("src").and_then(|v| v.as_str()) {
                            out.push((path.clone(), src.to_string()));
                        }
                    }
                    Node::Element(el) => walk(&el.children, path, out),
                    Node::Void(_) | Node::Text(_) => {}
                }
                path.pop();
            }
        }

        let mut images: Vec<(Vec<usize>, String)> = Vec::new();
        walk(&self.editor.doc().children, &mut Vec::new(), &mut images);

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        let mut report = EmbedLocalImagesReport::default();

        for (path, src) in images {
            let Some(local_path) =
                Self::local_image_path_for_src(&src, self.document_base_dir.as_deref())
            else {
                report.skipped += 1;
                continue;
            };
            let Some((mime, format)) = Self::mime_and_format_for_image_path(&local_path) else {
                report.skipped += 1;
                continue;
            };

            let bytes = match std::fs::read(&local_path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    report.failed += 1;
                    continue;
                }
            };

            let data_url = format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            );

            let mut set = Attrs::default();
            set.insert(
                "src".to_string(),
                serde_json::Value::String(data_url.clone()),
            );
            let patch = gpui_plate_core::AttrPatch {
                set,
                remove: Vec::new(),
            };
            ops.push(gpui_plate_core::Op::SetNodeAttrs { path, patch });

            self.embedded_image_cache
                .insert(data_url, Arc::new(Image::from_bytes(format, bytes)));
            report.embedded += 1;
        }

        if report.embedded > 0 {
            self.push_tx(
                gpui_plate_core::Transaction::new(ops).source("command:image.embed_local_images"),
                cx,
            );
        }

        report
    }

    pub fn command_collect_assets_into_assets_dir(
        &mut self,
        cx: &mut Context<Self>,
    ) -> CollectAssetsReport {
        fn walk(nodes: &[Node], path: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, String)>) {
            for (ix, node) in nodes.iter().enumerate() {
                path.push(ix);
                match node {
                    Node::Void(v) if v.kind == "image" => {
                        if let Some(src) = v.attrs.get("src").and_then(|v| v.as_str()) {
                            out.push((path.clone(), src.to_string()));
                        }
                    }
                    Node::Element(el) => walk(&el.children, path, out),
                    Node::Void(_) | Node::Text(_) => {}
                }
                path.pop();
            }
        }

        let mut report = CollectAssetsReport::default();
        let Some(document_base_dir) = self.document_base_dir.clone() else {
            report.failed += 1;
            return report;
        };

        let base_dir = if document_base_dir.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            document_base_dir
        };
        let base_dir = std::fs::canonicalize(&base_dir).unwrap_or(base_dir);
        let assets_dir = base_dir.join("assets");
        if std::fs::create_dir_all(&assets_dir).is_err() {
            report.failed += 1;
            return report;
        }

        let mut images: Vec<(Vec<usize>, String)> = Vec::new();
        walk(&self.editor.doc().children, &mut Vec::new(), &mut images);

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        let mut touched_assets: HashSet<String> = HashSet::new();

        for (path, src) in images {
            let src = src.trim();
            if src.is_empty() {
                report.skipped += 1;
                continue;
            }
            if src.starts_with("http://")
                || src.starts_with("https://")
                || src.starts_with("resource://")
            {
                report.skipped += 1;
                continue;
            }

            let (bytes, ext, is_data_url) = if src.starts_with("data:") {
                let Some((mime, bytes)) = Self::decode_data_url_bytes(src) else {
                    report.failed += 1;
                    continue;
                };
                let Some(ext) = Self::image_ext_for_mime(&mime).map(|s| s.to_string()) else {
                    report.failed += 1;
                    continue;
                };
                (bytes, ext, true)
            } else {
                let Some(local_path) = Self::local_image_path_for_src(src, Some(&base_dir)) else {
                    report.skipped += 1;
                    continue;
                };
                let ext = local_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext.is_empty() {
                    report.skipped += 1;
                    continue;
                }
                let bytes = match std::fs::read(&local_path) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        report.failed += 1;
                        continue;
                    }
                };
                (bytes, ext, false)
            };

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut hasher);
            let digest = hasher.finish();

            let file_name = format!("{digest:016x}.{ext}");
            let relative_path = format!("assets/{file_name}");
            let dest_path = base_dir.join(&relative_path);
            if dest_path.exists() && dest_path.is_dir() {
                report.failed += 1;
                continue;
            }

            if !dest_path.exists() && !touched_assets.contains(&relative_path) {
                if let Some(parent) = dest_path.parent() {
                    if std::fs::create_dir_all(parent).is_err() {
                        report.failed += 1;
                        continue;
                    }
                }
                if std::fs::write(&dest_path, &bytes).is_ok() {
                    report.assets_written += 1;
                } else {
                    report.failed += 1;
                    continue;
                }
                touched_assets.insert(relative_path.clone());
            }

            if src != relative_path {
                let mut set = Attrs::default();
                set.insert("src".to_string(), serde_json::Value::String(relative_path));
                let patch = gpui_plate_core::AttrPatch {
                    set,
                    remove: Vec::new(),
                };
                ops.push(gpui_plate_core::Op::SetNodeAttrs { path, patch });
                report.rewritten += 1;
                if is_data_url {
                    report.rewritten_data_url += 1;
                }
            } else {
                report.skipped += 1;
            }
        }

        if !ops.is_empty() {
            self.push_tx(
                gpui_plate_core::Transaction::new(ops).source("command:image.collect_assets"),
                cx,
            );
        }

        report
    }

    pub fn command_insert_mention(&mut self, label: String, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "label": label });
        _ = self.run_command_and_refresh("mention.insert", Some(args), cx);
    }

    pub fn command_insert_emoji(&mut self, emoji: String, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "emoji": emoji });
        _ = self.run_command_and_refresh("emoji.insert", Some(args), cx);
    }

    pub fn command_set_heading(&mut self, level: u64, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "level": level });
        _ = self.run_command_and_refresh("block.set_heading", Some(args), cx);
    }

    pub fn command_unset_heading(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("block.unset_heading", None, cx);
    }

    pub fn command_toggle_code_block(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("code_block.toggle", None, cx);
    }

    pub fn command_set_align(&mut self, align: BlockAlign, cx: &mut Context<Self>) {
        let align = match align {
            BlockAlign::Left => "left",
            BlockAlign::Center => "center",
            BlockAlign::Right => "right",
        };
        let args = serde_json::json!({ "align": align });
        _ = self.run_command_and_refresh("block.set_align", Some(args), cx);
    }

    pub fn command_set_font_size(&mut self, size: u64, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "size": size });
        _ = self.run_command_and_refresh("block.set_font_size", Some(args), cx);
    }

    pub fn command_unset_font_size(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("block.unset_font_size", None, cx);
    }

    pub fn command_toggle_blockquote(&mut self, cx: &mut Context<Self>) {
        if self.is_blockquote_active() {
            _ = self.run_command_and_refresh("blockquote.unwrap", None, cx);
        } else {
            _ = self.run_command_and_refresh("blockquote.wrap_selection", None, cx);
        }
    }

    pub fn command_toggle_toggle(&mut self, cx: &mut Context<Self>) {
        if self.is_toggle_active() {
            _ = self.run_command_and_refresh("toggle.unwrap", None, cx);
        } else {
            _ = self.run_command_and_refresh("toggle.wrap_selection", None, cx);
        }
    }

    pub fn command_toggle_collapsed(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("toggle.toggle_collapsed", None, cx);
    }

    pub fn command_toggle_todo(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("todo.toggle", None, cx);
    }

    pub fn command_indent_increase(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("block.indent_increase", None, cx);
    }

    pub fn command_indent_decrease(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("block.indent_decrease", None, cx);
    }

    pub fn command_toggle_bold(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_bold", None, cx);
    }

    pub fn command_toggle_italic(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_italic", None, cx);
    }

    pub fn command_toggle_underline(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_underline", None, cx);
    }

    pub fn command_toggle_strikethrough(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_strikethrough", None, cx);
    }

    pub fn command_toggle_code(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.toggle_code", None, cx);
    }

    pub fn command_set_text_color(&mut self, color: String, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "color": color });
        _ = self.run_command_and_refresh("marks.set_text_color", Some(args), cx);
    }

    pub fn command_unset_text_color(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.unset_text_color", None, cx);
    }

    pub fn command_set_highlight_color(&mut self, color: String, cx: &mut Context<Self>) {
        let args = serde_json::json!({ "color": color });
        _ = self.run_command_and_refresh("marks.set_highlight_color", Some(args), cx);
    }

    pub fn command_unset_highlight_color(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.unset_highlight_color", None, cx);
    }

    pub fn command_toggle_bulleted_list(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("list.toggle_bulleted", None, cx);
    }

    pub fn command_toggle_ordered_list(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("list.toggle_ordered", None, cx);
    }

    pub fn command_insert_table(&mut self, rows: usize, cols: usize, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "rows": rows, "cols": cols });
        _ = self.run_command_and_refresh("table.insert", Some(args), cx);
    }

    pub fn command_insert_table_row_above(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.insert_row_above", None, cx);
    }

    pub fn command_insert_table_row_below(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.insert_row_below", None, cx);
    }

    pub fn command_insert_table_col_left(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.insert_col_left", None, cx);
    }

    pub fn command_insert_table_col_right(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.insert_col_right", None, cx);
    }

    pub fn command_delete_table_row(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.delete_row", None, cx);
    }

    pub fn command_delete_table_col(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.delete_col", None, cx);
    }

    pub fn command_delete_table(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("table.delete_table", None, cx);
    }

    pub fn command_insert_columns(&mut self, columns: usize, cx: &mut Context<Self>) {
        _ = self.delete_selection_if_any(cx);
        let args = serde_json::json!({ "columns": columns });
        _ = self.run_command_and_refresh("columns.insert", Some(args), cx);
    }

    pub fn command_unwrap_columns(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("columns.unwrap", None, cx);
    }

    pub fn command_set_link(&mut self, url: String, cx: &mut Context<Self>) {
        // UX goals:
        // - If selection exists: apply link to selection (core semantics).
        // - If caret is in an existing link: edit the whole link run (not just one segment).
        // - Otherwise: try to apply link to a nearby word; if nothing meaningful exists (empty/
        //   whitespace), insert the URL as linked text so users see an immediate result.
        if self.editor.selection().is_collapsed() {
            if let Some(block_path) = self.active_text_block_path()
                && let Some(text) = self.text_block_text(&block_path)
            {
                let cursor = self
                    .row_offset_for_point(&self.editor.selection().focus)
                    .unwrap_or(0)
                    .min(text.len());
                let text = text.as_str();

                if let Some(range) = self.link_run_range_in_block(&block_path, cursor) {
                    self.set_selection_in_active_block(range.start, range.end, cx);
                    let args = serde_json::json!({ "url": url });
                    _ = self.run_command_and_refresh("marks.set_link", Some(args), cx);
                    return;
                }

                let mut range = Self::word_range(text, cursor);
                let mut selected = text.get(range.clone()).unwrap_or("");
                if range.start >= range.end || selected.chars().all(|ch| ch.is_whitespace()) {
                    // If we landed on whitespace, try the next non-whitespace run to the right.
                    let mut ix = range.end.min(text.len());
                    while ix < text.len() {
                        let Some(ch) = text.get(ix..).and_then(|s| s.chars().next()) else {
                            break;
                        };
                        if !ch.is_whitespace() {
                            range = Self::word_range(text, ix);
                            selected = text.get(range.clone()).unwrap_or("");
                            break;
                        }
                        ix = Self::next_boundary(text, ix);
                    }
                }

                if range.start >= range.end || selected.chars().all(|ch| ch.is_whitespace()) {
                    // If there's nothing meaningful to the right, try the left.
                    let mut ix = range.start.min(text.len());
                    while ix > 0 {
                        let prev = Self::prev_boundary(text, ix);
                        let Some(ch) = text.get(prev..).and_then(|s| s.chars().next()) else {
                            break;
                        };
                        if !ch.is_whitespace() {
                            range = Self::word_range(text, prev);
                            selected = text.get(range.clone()).unwrap_or("");
                            break;
                        }
                        ix = prev;
                    }
                }

                if range.start < range.end && !selected.chars().all(|ch| ch.is_whitespace()) {
                    self.set_selection_in_active_block(range.start, range.end, cx);
                    let args = serde_json::json!({ "url": url });
                    _ = self.run_command_and_refresh("marks.set_link", Some(args), cx);
                    return;
                }

                let mut marks = self.active_marks();
                marks.link = Some(url.clone());
                if let Some(tx) = self.tx_replace_range_in_block(
                    &block_path,
                    cursor..cursor,
                    &url,
                    &marks,
                    None,
                    "command:marks.insert_link",
                ) {
                    self.push_tx(tx, cx);
                }
                return;
            }
        }

        let args = serde_json::json!({ "url": url });
        _ = self.run_command_and_refresh("marks.set_link", Some(args), cx);
    }

    pub fn command_unset_link(&mut self, cx: &mut Context<Self>) {
        _ = self.run_command_and_refresh("marks.unset_link", None, cx);
    }

    pub fn is_bold_active(&self) -> bool {
        self.editor
            .run_query::<bool>("marks.is_bold_active", None)
            .unwrap_or(false)
    }

    pub fn has_link_active(&self) -> bool {
        self.editor
            .run_query::<bool>("marks.has_link_active", None)
            .unwrap_or(false)
    }

    pub fn active_link_url(&self) -> Option<String> {
        self.editor
            .run_query::<Marks>("marks.get_active", None)
            .ok()
            .and_then(|m| m.link)
    }

    pub fn is_bulleted_list_active(&self) -> bool {
        self.editor
            .run_query::<bool>(
                "list.is_active",
                Some(serde_json::json!({ "type": "bulleted" })),
            )
            .unwrap_or(false)
    }

    pub fn is_ordered_list_active(&self) -> bool {
        self.editor
            .run_query::<bool>(
                "list.is_active",
                Some(serde_json::json!({ "type": "ordered" })),
            )
            .unwrap_or(false)
    }

    pub fn is_table_active(&self) -> bool {
        self.editor
            .run_query::<bool>("table.is_active", None)
            .unwrap_or(false)
    }

    pub fn is_columns_active(&self) -> bool {
        self.editor
            .run_query::<bool>("columns.is_active", None)
            .unwrap_or(false)
    }

    pub fn is_code_block_active(&self) -> bool {
        self.editor
            .run_query::<bool>("code_block.is_active", None)
            .unwrap_or(false)
    }

    pub fn block_align(&self) -> BlockAlign {
        match self
            .editor
            .run_query::<Option<String>>("block.align", None)
            .ok()
            .flatten()
            .as_deref()
        {
            Some("center") => BlockAlign::Center,
            Some("right") => BlockAlign::Right,
            _ => BlockAlign::Left,
        }
    }

    pub fn block_font_size(&self) -> Option<u64> {
        self.editor
            .run_query::<Option<u64>>("block.font_size", None)
            .ok()
            .flatten()
    }

    pub fn heading_level(&self) -> Option<u64> {
        self.editor
            .run_query::<Option<u64>>("block.heading_level", None)
            .ok()
            .flatten()
    }

    pub fn indent_level(&self) -> u64 {
        self.editor
            .run_query::<u64>("block.indent_level", None)
            .unwrap_or(0)
    }

    pub fn is_blockquote_active(&self) -> bool {
        self.editor
            .run_query::<bool>("blockquote.is_active", None)
            .unwrap_or(false)
    }

    pub fn is_toggle_active(&self) -> bool {
        self.editor
            .run_query::<bool>("toggle.is_active", None)
            .unwrap_or(false)
    }

    pub fn is_toggle_collapsed(&self) -> bool {
        self.editor
            .run_query::<bool>("toggle.is_collapsed", None)
            .unwrap_or(false)
    }

    pub fn is_todo_active(&self) -> bool {
        self.editor
            .run_query::<bool>("todo.is_active", None)
            .unwrap_or(false)
    }

    pub fn is_todo_checked(&self) -> bool {
        self.editor
            .run_query::<bool>("todo.is_checked", None)
            .unwrap_or(false)
    }

    fn selected_block_sibling_info(&self) -> Option<(Vec<usize>, usize, usize)> {
        let path = self.selected_block_path.as_ref()?;
        let node = node_at_path(self.editor.doc(), path)?;
        if !matches!(node, Node::Element(_) | Node::Void(_)) {
            return None;
        }

        let (ix, parent_path) = path.split_last()?;
        let parent_path = parent_path.to_vec();
        let len = if parent_path.is_empty() {
            self.editor.doc().children.len()
        } else {
            element_at_path(self.editor.doc(), &parent_path)?
                .children
                .len()
        };
        Some((parent_path, *ix, len))
    }

    fn plain_text_for_node(&self, node: &Node) -> String {
        fn void_fallback(v: &gpui_plate_core::VoidNode) -> String {
            match v.kind.as_str() {
                "divider" => "---".to_string(),
                "image" => {
                    let src = v.attrs.get("src").and_then(|v| v.as_str()).unwrap_or("");
                    let alt = v.attrs.get("alt").and_then(|v| v.as_str()).unwrap_or("");
                    if src.is_empty() {
                        "![image]()".to_string()
                    } else {
                        format!("![{alt}]({src})")
                    }
                }
                _ => v.inline_text(),
            }
        }

        fn collect(
            node: &Node,
            registry: &PluginRegistry,
            out: &mut Vec<String>,
            buf: &mut String,
        ) {
            match node {
                Node::Text(t) => buf.push_str(&t.text),
                Node::Void(v) => {
                    if !buf.is_empty() {
                        out.push(std::mem::take(buf));
                    }
                    out.push(void_fallback(v));
                }
                Node::Element(el) => {
                    if is_text_block_kind(registry, &el.kind) {
                        for child in &el.children {
                            match child {
                                Node::Text(t) => buf.push_str(&t.text),
                                Node::Void(v) => buf.push_str(&v.inline_text()),
                                Node::Element(_) => {}
                            }
                        }
                        out.push(std::mem::take(buf));
                    } else {
                        for child in &el.children {
                            collect(child, registry, out, buf);
                        }
                    }
                }
            }
        }

        let mut lines: Vec<String> = Vec::new();
        let mut buf = String::new();
        collect(node, self.editor.registry(), &mut lines, &mut buf);
        if !buf.is_empty() {
            lines.push(buf);
        }
        lines.join("\n")
    }

    fn copy_selected_block_to_clipboard(&self, cx: &mut Context<Self>) -> bool {
        let Some(path) = self.selected_block_path.as_ref() else {
            return false;
        };
        let Some(node) = node_at_path(self.editor.doc(), path) else {
            return false;
        };
        let node = match node {
            Node::Text(_) => return false,
            Node::Void(v) => Node::Void(v.clone()),
            Node::Element(el) => Node::Element(el.clone()),
        };

        let payload = PlateClipboardFragment::new(vec![node.clone()]);
        let text = self.plain_text_for_node(&node);
        cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(text, payload));
        true
    }

    fn copy_selected_range_to_clipboard(&self, cx: &mut Context<Self>) -> bool {
        let selection = self.editor.selection().clone();
        if selection.is_collapsed() {
            return false;
        }

        let anchor_block: Vec<usize> = selection
            .anchor
            .path
            .split_last()
            .map(|(_, p)| p.to_vec())
            .unwrap_or_default();
        let focus_block: Vec<usize> = selection
            .focus
            .path
            .split_last()
            .map(|(_, p)| p.to_vec())
            .unwrap_or_default();

        let blocks = text_block_paths(self.editor.doc(), self.editor.registry());
        let Some(anchor_pos) = blocks.iter().position(|p| p == &anchor_block) else {
            return false;
        };
        let Some(focus_pos) = blocks.iter().position(|p| p == &focus_block) else {
            return false;
        };

        let anchor_offset = self.row_offset_for_point(&selection.anchor).unwrap_or(0);
        let focus_offset = self.row_offset_for_point(&selection.focus).unwrap_or(0);

        let (start_pos, start_offset, end_pos, end_offset) = if anchor_pos == focus_pos {
            if focus_offset < anchor_offset {
                (focus_pos, focus_offset, anchor_pos, anchor_offset)
            } else {
                (anchor_pos, anchor_offset, focus_pos, focus_offset)
            }
        } else if focus_pos < anchor_pos {
            (focus_pos, focus_offset, anchor_pos, anchor_offset)
        } else {
            (anchor_pos, anchor_offset, focus_pos, focus_offset)
        };

        let mut fragment_blocks: Vec<Node> = Vec::new();
        let mut fallback_text = String::new();

        for (pos, block_path) in blocks.iter().enumerate().take(end_pos + 1).skip(start_pos) {
            let Some(el) = element_at_path(self.editor.doc(), block_path) else {
                continue;
            };
            if !is_text_block_kind(self.editor.registry(), &el.kind) {
                continue;
            }

            let len = self.block_text_len_in_block(block_path);
            let (range_start, range_end) = if start_pos == end_pos {
                (
                    start_offset.min(len),
                    end_offset.min(len).max(start_offset.min(len)),
                )
            } else if pos == start_pos {
                (start_offset.min(len), len)
            } else if pos == end_pos {
                (0, end_offset.min(len))
            } else {
                (0, len)
            };

            let old_children = el.children.clone();
            let selected_children = if range_end > range_start {
                let (_left, rest) =
                    Self::split_inline_children_at_offset(&old_children, range_start);
                let (selected, _right) =
                    Self::split_inline_children_at_offset(&rest, range_end - range_start);
                Self::merge_adjacent_text_nodes(selected)
            } else {
                Vec::new()
            };

            fragment_blocks.push(Node::Element(ElementNode {
                kind: el.kind.clone(),
                attrs: el.attrs.clone(),
                children: selected_children,
            }));

            let text = self.text_block_text(block_path).unwrap_or_default();
            if start_pos == end_pos {
                let start = range_start.min(text.len());
                let end = range_end.min(text.len()).max(start);
                fallback_text.push_str(text.as_str().get(start..end).unwrap_or(""));
                break;
            }

            if pos == start_pos {
                let start = range_start.min(text.len());
                fallback_text.push_str(text.as_str().get(start..).unwrap_or(""));
                fallback_text.push('\n');
            } else if pos == end_pos {
                let end = range_end.min(text.len());
                fallback_text.push_str(text.as_str().get(..end).unwrap_or(""));
            } else {
                fallback_text.push_str(text.as_str());
                fallback_text.push('\n');
            }
        }

        if fragment_blocks.is_empty() {
            return false;
        }

        let payload = PlateClipboardFragment::new_range(fragment_blocks);
        cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(
            fallback_text,
            payload,
        ));
        true
    }

    fn inline_len(nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|n| match n {
                Node::Text(t) => t.text.len(),
                Node::Void(v) => v.inline_text_len(),
                Node::Element(_) => 0,
            })
            .sum()
    }

    fn inline_children_is_effectively_empty(children: &[Node]) -> bool {
        children.iter().all(|n| match n {
            Node::Text(t) => t.text.is_empty(),
            Node::Void(_) | Node::Element(_) => false,
        })
    }

    fn normalize_inline_children_for_insert(children: Vec<Node>, marks: &Marks) -> Vec<Node> {
        let children = Self::merge_adjacent_text_nodes(children);
        Self::ensure_has_text_leaf(children, marks)
    }

    fn paste_range_fragment_blocks_at_insertion_point(
        &mut self,
        fragment_blocks: Vec<ElementNode>,
        cx: &mut Context<Self>,
    ) -> bool {
        let insert_at = self
            .delete_selection_if_any(cx)
            .unwrap_or_else(|| self.editor.selection().focus.clone());
        let Some(block_path) = insert_at.path.split_last().map(|(_, p)| p.to_vec()) else {
            return false;
        };

        let Some(context_el) = element_at_path(self.editor.doc(), &block_path) else {
            return false;
        };
        if !is_text_block_kind(self.editor.registry(), &context_el.kind) {
            return false;
        }

        let offset = self.row_offset_for_point(&insert_at).unwrap_or(0);
        let marks = self.active_marks();
        let (prefix, suffix) = Self::split_inline_children_at_offset(&context_el.children, offset);

        if fragment_blocks.is_empty() {
            return false;
        }

        if fragment_blocks.len() == 1 && fragment_blocks[0].kind == context_el.kind {
            let inserted_children = fragment_blocks[0].children.clone();
            let caret_offset = Self::inline_len(&prefix) + Self::inline_len(&inserted_children);

            let mut new_children = prefix;
            new_children.extend(inserted_children);
            new_children.extend(suffix);
            let new_children = Self::normalize_inline_children_for_insert(new_children, &marks);

            let caret_point =
                Self::point_for_block_offset_in_children(&block_path, &new_children, caret_offset);

            let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
            for child_ix in (0..context_el.children.len()).rev() {
                let mut path = block_path.clone();
                path.push(child_ix);
                ops.push(gpui_plate_core::Op::RemoveNode { path });
            }
            for (child_ix, node) in new_children.into_iter().enumerate() {
                let mut path = block_path.clone();
                path.push(child_ix);
                ops.push(gpui_plate_core::Op::InsertNode { path, node });
            }

            self.push_tx(
                gpui_plate_core::Transaction::new(ops)
                    .selection_after(Selection::collapsed(caret_point))
                    .source("command:paste_range_fragment_inline"),
                cx,
            );
            return true;
        }

        let Some((block_ix, parent_path)) = block_path.split_last() else {
            return false;
        };
        let block_ix = *block_ix;
        let parent_path = parent_path.to_vec();

        let context_kind = context_el.kind.clone();
        let context_attrs = context_el.attrs.clone();

        let prefix_is_empty = Self::inline_children_is_effectively_empty(&prefix);
        let suffix_is_empty = Self::inline_children_is_effectively_empty(&suffix);
        let default_marks = Marks::default();

        if fragment_blocks.len() == 1 {
            let mut out_blocks: Vec<Node> = Vec::new();
            let mut caret_rel_ix = 0usize;

            if !prefix_is_empty {
                let children = Self::normalize_inline_children_for_insert(prefix, &marks);
                out_blocks.push(Node::Element(ElementNode {
                    kind: context_kind.clone(),
                    attrs: context_attrs.clone(),
                    children,
                }));
                caret_rel_ix = 1;
            }

            let mut frag = fragment_blocks[0].clone();
            frag.children =
                Self::normalize_inline_children_for_insert(frag.children, &default_marks);
            let caret_inline_offset = Self::inline_len(&frag.children);
            out_blocks.push(Node::Element(frag));

            if !suffix_is_empty {
                let children = Self::normalize_inline_children_for_insert(suffix, &marks);
                out_blocks.push(Node::Element(ElementNode {
                    kind: context_kind.clone(),
                    attrs: context_attrs.clone(),
                    children,
                }));
            }

            let caret_block_ix = block_ix + caret_rel_ix;
            let mut caret_block_path = parent_path.clone();
            caret_block_path.push(caret_block_ix);
            let caret_block_children = match out_blocks.get(caret_rel_ix) {
                Some(Node::Element(el)) => &el.children,
                _ => return false,
            };
            let caret_point = Self::point_for_block_offset_in_children(
                &caret_block_path,
                caret_block_children,
                caret_inline_offset,
            );

            let mut ops: Vec<gpui_plate_core::Op> = vec![gpui_plate_core::Op::RemoveNode {
                path: block_path.clone(),
            }];
            for (i, node) in out_blocks.into_iter().enumerate() {
                let mut path = parent_path.clone();
                path.push(block_ix + i);
                ops.push(gpui_plate_core::Op::InsertNode { path, node });
            }

            self.push_tx(
                gpui_plate_core::Transaction::new(ops)
                    .selection_after(Selection::collapsed(caret_point))
                    .source("command:paste_range_fragment_block"),
                cx,
            );
            return true;
        }

        let mut out_blocks: Vec<Node> = Vec::new();
        let caret_rel_ix: usize;
        let caret_inline_offset: usize;

        let last_frag_ix = fragment_blocks.len() - 1;
        let merge_first = fragment_blocks
            .first()
            .is_some_and(|el| el.kind == context_kind);
        let merge_last = fragment_blocks
            .last()
            .is_some_and(|el| el.kind == context_kind);

        // Prefix + first fragment block
        if merge_first {
            let first_children = fragment_blocks[0].children.clone();
            let mut children = prefix;
            children.extend(first_children);
            let children = Self::normalize_inline_children_for_insert(children, &marks);
            out_blocks.push(Node::Element(ElementNode {
                kind: context_kind.clone(),
                attrs: context_attrs.clone(),
                children,
            }));
        } else {
            if !prefix_is_empty {
                let children = Self::normalize_inline_children_for_insert(prefix, &marks);
                out_blocks.push(Node::Element(ElementNode {
                    kind: context_kind.clone(),
                    attrs: context_attrs.clone(),
                    children,
                }));
            }

            let mut first = fragment_blocks[0].clone();
            first.children =
                Self::normalize_inline_children_for_insert(first.children, &default_marks);
            out_blocks.push(Node::Element(first));
        }

        // Middle fragment blocks (excluding first & last)
        if fragment_blocks.len() > 2 {
            for frag in fragment_blocks.iter().take(last_frag_ix).skip(1) {
                let mut node = frag.clone();
                node.children =
                    Self::normalize_inline_children_for_insert(node.children, &default_marks);
                out_blocks.push(Node::Element(node));
            }
        }

        // Last fragment block + suffix
        let last_children = fragment_blocks[last_frag_ix].children.clone();
        if merge_last {
            let caret_at = Self::inline_len(&last_children);
            let mut children = last_children;
            children.extend(suffix);
            let children = Self::normalize_inline_children_for_insert(children, &marks);
            caret_rel_ix = out_blocks.len();
            caret_inline_offset = caret_at;
            out_blocks.push(Node::Element(ElementNode {
                kind: context_kind.clone(),
                attrs: context_attrs.clone(),
                children,
            }));
        } else {
            let mut last = fragment_blocks[last_frag_ix].clone();
            last.children =
                Self::normalize_inline_children_for_insert(last.children, &default_marks);
            caret_rel_ix = out_blocks.len();
            caret_inline_offset = Self::inline_len(&last.children);
            out_blocks.push(Node::Element(last));

            if !suffix_is_empty {
                let children = Self::normalize_inline_children_for_insert(suffix, &marks);
                out_blocks.push(Node::Element(ElementNode {
                    kind: context_kind.clone(),
                    attrs: context_attrs.clone(),
                    children,
                }));
            }
        }

        let mut ops: Vec<gpui_plate_core::Op> = vec![gpui_plate_core::Op::RemoveNode {
            path: block_path.clone(),
        }];
        for (i, node) in out_blocks.iter().cloned().enumerate() {
            let mut path = parent_path.clone();
            path.push(block_ix + i);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        let caret_block_ix = block_ix + caret_rel_ix;
        let mut caret_block_path = parent_path.clone();
        caret_block_path.push(caret_block_ix);
        let caret_block_children = match out_blocks.get(caret_rel_ix) {
            Some(Node::Element(el)) => &el.children,
            _ => return false,
        };
        let caret_point = Self::point_for_block_offset_in_children(
            &caret_block_path,
            caret_block_children,
            caret_inline_offset,
        );

        self.push_tx(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(Selection::collapsed(caret_point))
                .source("command:paste_range_fragment_multiblock"),
            cx,
        );
        true
    }

    fn paste_block_fragment_nodes(&mut self, nodes: Vec<Node>, cx: &mut Context<Self>) -> bool {
        fn has_text_descendant(node: &Node) -> bool {
            match node {
                Node::Text(_) => true,
                Node::Void(_) => false,
                Node::Element(el) => el.children.iter().any(has_text_descendant),
            }
        }

        let nodes: Vec<Node> = nodes
            .into_iter()
            .filter(|n| matches!(n, Node::Void(_) | Node::Element(_)))
            .collect();
        if nodes.is_empty() {
            return false;
        }
        let nodes_len = nodes.len();

        if let Some(selected_path) = self.selected_block_path.clone()
            && let Some((selected_ix, parent_path)) = selected_path.split_last()
        {
            let selected_ix = *selected_ix;
            let parent_path = parent_path.to_vec();

            let after_sibling_has_text = {
                let mut path = parent_path.clone();
                path.push(selected_ix + 1);
                node_at_path(self.editor.doc(), &path)
                    .map(has_text_descendant)
                    .unwrap_or(false)
            };

            let mut ops: Vec<gpui_plate_core::Op> = vec![gpui_plate_core::Op::RemoveNode {
                path: selected_path,
            }];

            for (i, node) in nodes.into_iter().enumerate() {
                let mut path = parent_path.clone();
                path.push(selected_ix + i);
                ops.push(gpui_plate_core::Op::InsertNode { path, node });
            }

            let after_index = selected_ix + nodes_len;
            let selection_after = if after_sibling_has_text {
                let mut path = parent_path.clone();
                path.push(after_index);
                Selection::collapsed(CorePoint::new(path, 0))
            } else {
                let mut paragraph_path = parent_path.clone();
                paragraph_path.push(after_index);
                ops.push(gpui_plate_core::Op::InsertNode {
                    path: paragraph_path.clone(),
                    node: Node::paragraph(""),
                });
                Selection::collapsed(CorePoint::new(paragraph_path, 0))
            };

            self.push_tx(
                gpui_plate_core::Transaction::new(ops)
                    .selection_after(selection_after)
                    .source("command:paste_block_fragment"),
                cx,
            );
            return true;
        }

        let insert_at = self
            .delete_selection_if_any(cx)
            .unwrap_or_else(|| self.editor.selection().focus.clone());
        let Some((_, block_path)) = insert_at.path.split_last() else {
            return false;
        };
        let Some((block_ix, parent_path)) = block_path.split_last() else {
            return false;
        };
        let insert_ix = block_ix + 1;
        let parent_path = parent_path.to_vec();

        let after_sibling_has_text = {
            let mut path = parent_path.clone();
            path.push(insert_ix);
            node_at_path(self.editor.doc(), &path)
                .map(has_text_descendant)
                .unwrap_or(false)
        };

        let mut ops: Vec<gpui_plate_core::Op> = Vec::new();
        for (i, node) in nodes.into_iter().enumerate() {
            let mut path = parent_path.clone();
            path.push(insert_ix + i);
            ops.push(gpui_plate_core::Op::InsertNode { path, node });
        }

        let after_index = insert_ix + nodes_len;
        let selection_after = if after_sibling_has_text {
            let mut path = parent_path.clone();
            path.push(after_index);
            Selection::collapsed(CorePoint::new(path, 0))
        } else {
            let mut paragraph_path = parent_path.clone();
            paragraph_path.push(after_index);
            ops.push(gpui_plate_core::Op::InsertNode {
                path: paragraph_path.clone(),
                node: Node::paragraph(""),
            });
            Selection::collapsed(CorePoint::new(paragraph_path, 0))
        };

        self.push_tx(
            gpui_plate_core::Transaction::new(ops)
                .selection_after(selection_after)
                .source("command:paste_block_fragment"),
            cx,
        );
        true
    }

    pub fn command_copy(&mut self, cx: &mut Context<Self>) {
        if self.copy_selected_block_to_clipboard(cx) {
            return;
        }

        _ = self.copy_selected_range_to_clipboard(cx);
    }

    pub fn command_cut(&mut self, cx: &mut Context<Self>) {
        if self.copy_selected_block_to_clipboard(cx)
            && let Some(path) = self.selected_block_path.clone()
            && matches!(
                node_at_path(self.editor.doc(), &path),
                Some(Node::Void(_)) | Some(Node::Element(_))
            )
        {
            self.selected_block_path = None;
            self.push_tx(
                gpui_plate_core::Transaction::new(vec![gpui_plate_core::Op::RemoveNode { path }])
                    .source("command:cut_selected_block"),
                cx,
            );
            return;
        }

        self.command_copy(cx);
        _ = self.delete_selection_if_any(cx);
    }

    pub fn command_paste(&mut self, cx: &mut Context<Self>) {
        let Some(clipboard) = cx.read_from_clipboard() else {
            return;
        };

        if let Some(fragment) = PlateClipboardFragment::from_clipboard(&clipboard) {
            match fragment.mode {
                PlateClipboardFragmentMode::Range => {
                    if self.selected_block_path.is_some() {
                        if self.paste_block_fragment_nodes(fragment.nodes, cx) {
                            return;
                        }
                    } else {
                        let blocks: Vec<ElementNode> = fragment
                            .nodes
                            .into_iter()
                            .filter_map(|node| match node {
                                Node::Element(el) => Some(el),
                                _ => None,
                            })
                            .collect();
                        if !blocks.is_empty()
                            && self.paste_range_fragment_blocks_at_insertion_point(blocks, cx)
                        {
                            return;
                        }
                    }
                }
                PlateClipboardFragmentMode::Block => {
                    if self.paste_block_fragment_nodes(fragment.nodes, cx) {
                        return;
                    }
                }
            }
        }

        let mut pasted_image_srcs: Vec<String> = Vec::new();
        for entry in clipboard.entries() {
            let ClipboardEntry::Image(image) = entry else {
                continue;
            };
            let src = Self::data_url_for_image(image);
            if src.is_empty() {
                continue;
            }
            self.embedded_image_cache
                .insert(src.clone(), Arc::new(image.clone()));
            pasted_image_srcs.push(src);
        }

        if !pasted_image_srcs.is_empty() {
            if self.selected_block_path.is_some() {
                let nodes = pasted_image_srcs
                    .iter()
                    .cloned()
                    .map(|src| Node::image(src, None))
                    .collect();
                if self.paste_block_fragment_nodes(nodes, cx) {
                    return;
                }
            }

            _ = self.delete_selection_if_any(cx);
            let args = serde_json::json!({ "srcs": pasted_image_srcs });
            if !self.run_command_and_refresh("image.insert_many", Some(args), cx) {
                cx.notify();
            }
            return;
        }

        let mut new_text = clipboard.text().unwrap_or_default();
        new_text = new_text.replace("\r\n", "\n").replace('\r', "\n");
        if new_text.is_empty() {
            return;
        }

        let mut image_srcs: Vec<String> = Vec::new();
        let mut ok = true;
        for line in new_text.lines().map(str::trim).filter(|s| !s.is_empty()) {
            let src = Self::normalize_image_src(line);
            let path = Path::new(&src);
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            let is_image = matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "tif" | "tiff"
            );
            if !(is_image && path.is_absolute() && path.exists()) {
                ok = false;
                break;
            }
            image_srcs.push(src);
        }

        if ok && !image_srcs.is_empty() {
            _ = self.delete_selection_if_any(cx);
            let args = serde_json::json!({ "srcs": image_srcs });
            if !self.run_command_and_refresh("image.insert_many", Some(args), cx) {
                cx.notify();
            }
            return;
        }

        let insert_at = self
            .delete_selection_if_any(cx)
            .unwrap_or_else(|| self.editor.selection().focus.clone());
        let Some((_, block_path)) = insert_at.path.split_last() else {
            return;
        };
        let offset = self.row_offset_for_point(&insert_at).unwrap_or(0);
        let marks = self.active_marks();

        if let Some(tx) = self.tx_insert_multiline_text_at_block_offset(
            block_path,
            offset,
            &new_text,
            &marks,
            "command:paste",
        ) {
            self.push_tx(tx, cx);
        }
    }

    pub fn command_select_all(&mut self, cx: &mut Context<Self>) {
        let blocks = text_block_paths(self.editor.doc(), self.editor.registry());
        let (Some(first), Some(last)) = (blocks.first(), blocks.last()) else {
            return;
        };

        let anchor = self.point_for_block_offset(first, 0).unwrap_or_else(|| {
            let mut path = first.clone();
            path.push(0);
            CorePoint::new(path, 0)
        });
        let focus = self
            .point_for_block_offset(last, self.block_text_len_in_block(last))
            .unwrap_or_else(|| {
                let mut path = last.clone();
                path.push(0);
                CorePoint::new(path, 0)
            });
        self.set_selection_points(anchor, focus, cx);
    }
}

impl EntityInputHandler for RichTextState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.active_text();
        let start = utf16_to_byte(text.as_str(), range_utf16.start);
        let end = utf16_to_byte(text.as_str(), range_utf16.end);
        adjusted_range.replace(byte_to_utf16_range(text.as_str(), start..end));
        Some(text.as_str().get(start..end).unwrap_or("").to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.active_text();
        let range = self.ordered_active_range();
        let utf16 = byte_to_utf16_range(text.as_str(), range.clone());

        let sel = self.editor.selection();
        let anchor_offset = self.row_offset_for_point(&sel.anchor).unwrap_or(0);
        let focus_offset = self.row_offset_for_point(&sel.focus).unwrap_or(0);
        let reversed = if let Some(active_block) = self.active_text_block_path() {
            let anchor_block = sel.anchor.path.split_last().map(|(_, p)| p);
            let focus_block = sel.focus.path.split_last().map(|(_, p)| p);
            anchor_block == Some(active_block.as_slice())
                && focus_block == Some(active_block.as_slice())
                && focus_offset < anchor_offset
        } else {
            false
        };

        Some(UTF16Selection {
            range: utf16,
            reversed,
        })
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.active_text();
        let range_bytes = range_utf16
            .as_ref()
            .map(|r| utf16_to_byte(text.as_str(), r.start)..utf16_to_byte(text.as_str(), r.end))
            .or_else(|| self.ime_marked_range.clone())
            .unwrap_or_else(|| self.ordered_active_range());

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let marks = self.active_marks();
        let inserted = new_text.replace("\r\n", "\n").replace('\r', "\n");

        let tx = if inserted.contains('\n') {
            self.tx_replace_range_with_multiline_text_in_block(
                &block_path,
                range_bytes.clone(),
                &inserted,
                &marks,
                "ime:replace_text",
            )
        } else {
            self.tx_replace_range_in_block(
                &block_path,
                range_bytes.clone(),
                &inserted,
                &marks,
                None,
                "ime:replace_text",
            )
        };

        if let Some(tx) = tx {
            self.push_tx(tx, cx);
        }
        // Committed input ends any active preedit range.
        self.ime_marked_range = None;
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.active_text();
        self.ime_marked_range
            .as_ref()
            .map(|r| byte_to_utf16_range(text.as_str(), r.clone()))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.ime_marked_range = None;
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.active_text();
        let range_bytes = range_utf16
            .as_ref()
            .map(|r| utf16_to_byte(text.as_str(), r.start)..utf16_to_byte(text.as_str(), r.end))
            .or_else(|| self.ime_marked_range.clone())
            .unwrap_or_else(|| self.ordered_active_range());

        let Some(block_path) = self.active_text_block_path() else {
            return;
        };
        let marks = self.active_marks();
        let inserted = new_text.replace("\r\n", "\n").replace('\r', "\n");
        if inserted.contains('\n') {
            return;
        }

        let inserted_len = inserted.len();
        let marked = if inserted_len == 0 {
            None
        } else {
            Some(range_bytes.start..range_bytes.start + inserted_len)
        };
        self.ime_marked_range = marked.clone();

        let selection_after_offsets =
            if let (Some(marked), Some(sel_utf16)) = (marked, new_selected_range_utf16) {
                let rel_start = utf16_to_byte(inserted.as_str(), sel_utf16.start);
                let rel_end = utf16_to_byte(inserted.as_str(), sel_utf16.end);
                Some((marked.start + rel_start, marked.start + rel_end))
            } else {
                None
            };

        if let Some(tx) = self.tx_replace_range_in_block(
            &block_path,
            range_bytes.clone(),
            &inserted,
            &marks,
            selection_after_offsets,
            "ime:replace_and_mark_text",
        ) {
            self.push_tx(tx, cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let block_path = self.active_text_block_path()?;
        let cache = self.layout_cache.get(block_path.as_slice())?;
        let text = cache.text.as_str();
        let start = utf16_to_byte(text, range_utf16.start);
        let pos = cache.text_layout.position_for_index(start)?;
        let line_height = cache.text_layout.line_height();
        Some(Bounds::from_corners(
            pos,
            point(pos.x + px(1.0), pos.y + line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let block_path = self.active_text_block_path()?;
        let byte_ix = self.offset_for_point_in_block(&block_path, point)?;
        let text = self.active_text();
        Some(byte_to_utf16(text.as_str(), byte_ix))
    }
}

impl gpui::Focusable for RichTextState {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub struct RichTextLineElement {
    state: Entity<RichTextState>,
    block_path: Vec<usize>,
    styled_text: StyledText,
    text: SharedString,
    segments: Vec<InlineTextSegment>,
}

impl RichTextLineElement {
    pub fn new(state: Entity<RichTextState>, block_path: Vec<usize>) -> Self {
        Self {
            state,
            block_path,
            styled_text: StyledText::new(SharedString::default()),
            text: SharedString::default(),
            segments: Vec::new(),
        }
    }

    fn paint_selection_range(
        layout: &gpui::TextLayout,
        range: Range<usize>,
        bounds: &Bounds<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        selection_color: gpui::Hsla,
        text_align: TextAlign,
    ) {
        let start_ix = range.start.min(layout.len());
        let end_ix = range.end.min(layout.len());
        if start_ix >= end_ix {
            return;
        }

        let Some(first_pos) = layout.position_for_index(0).or_else(|| {
            layout
                .position_for_index(layout.len())
                .or_else(|| Some(bounds.origin))
        }) else {
            return;
        };
        let origin = first_pos;

        let Some(line_layout) = layout.line_layout_for_index(start_ix) else {
            return;
        };

        let mut boundaries: Vec<usize> = line_layout
            .wrap_boundaries()
            .iter()
            .filter_map(|boundary| {
                let run = line_layout.unwrapped_layout.runs.get(boundary.run_ix)?;
                run.glyphs.get(boundary.glyph_ix).map(|g| g.index)
            })
            .collect();

        // `wrap_boundaries` is empty for non-wrapping text; treat it as a single visual line.
        boundaries.push(line_layout.len());
        boundaries.dedup();

        let mut seg_start = 0usize;
        for (seg_ix, seg_end) in boundaries.into_iter().enumerate() {
            let seg_start_ix = seg_start;
            seg_start = seg_end;

            let a = start_ix.max(seg_start_ix);
            let b = end_ix.min(seg_end);
            if a >= b {
                continue;
            }

            let seg_origin_x = line_layout.unwrapped_layout.x_for_index(seg_start_ix);
            let start_x = line_layout.unwrapped_layout.x_for_index(a) - seg_origin_x;
            let end_x = line_layout.unwrapped_layout.x_for_index(b) - seg_origin_x;
            let seg_width = line_layout.unwrapped_layout.x_for_index(seg_end) - seg_origin_x;
            let offset_x = match text_align {
                TextAlign::Left => px(0.),
                TextAlign::Center => (bounds.size.width - seg_width) / 2.0,
                TextAlign::Right => bounds.size.width - seg_width,
            };

            let y = origin.y + line_height * seg_ix as f32;
            window.paint_quad(gpui::quad(
                Bounds::from_corners(
                    point(origin.x + offset_x + start_x, y),
                    point(origin.x + offset_x + end_x, y + line_height),
                ),
                px(0.),
                selection_color,
                gpui::Edges::default(),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
        }
    }

    fn align_offset_x_for_position(
        layout: &gpui::TextLayout,
        bounds: &Bounds<Pixels>,
        position: gpui::Point<Pixels>,
        text_align: TextAlign,
    ) -> Pixels {
        if text_align == TextAlign::Left {
            return px(0.);
        }

        let Some(line_layout) = layout.line_layout_for_index(0).or_else(|| {
            layout
                .line_layout_for_index(layout.len())
                .or_else(|| layout.line_layout_for_index(layout.len().saturating_sub(1)))
        }) else {
            return px(0.);
        };

        let line_height = layout.line_height();
        if line_height == px(0.) {
            return px(0.);
        }

        let mut seg_ix = ((position.y - bounds.origin.y) / line_height) as usize;
        seg_ix = seg_ix.min(line_layout.wrap_boundaries().len());

        Self::align_offset_x_for_segment(&line_layout, bounds, seg_ix, text_align)
    }

    fn align_offset_x_for_index(
        layout: &gpui::TextLayout,
        bounds: &Bounds<Pixels>,
        index: usize,
        text_align: TextAlign,
    ) -> Pixels {
        if text_align == TextAlign::Left {
            return px(0.);
        }

        let Some(line_layout) = layout.line_layout_for_index(0).or_else(|| {
            layout
                .line_layout_for_index(layout.len())
                .or_else(|| layout.line_layout_for_index(layout.len().saturating_sub(1)))
        }) else {
            return px(0.);
        };

        let mut seg_ix = 0usize;
        let index = index.min(line_layout.len());
        for (boundary_ix, boundary) in line_layout.wrap_boundaries().iter().enumerate() {
            let Some(run) = line_layout.unwrapped_layout.runs.get(boundary.run_ix) else {
                continue;
            };
            let Some(glyph) = run.glyphs.get(boundary.glyph_ix) else {
                continue;
            };
            if index < glyph.index {
                break;
            }
            seg_ix = boundary_ix + 1;
        }

        Self::align_offset_x_for_segment(&line_layout, bounds, seg_ix, text_align)
    }

    fn align_offset_x_for_segment(
        line_layout: &gpui::WrappedLineLayout,
        bounds: &Bounds<Pixels>,
        seg_ix: usize,
        text_align: TextAlign,
    ) -> Pixels {
        if text_align == TextAlign::Left {
            return px(0.);
        }

        let seg_ix = seg_ix.min(line_layout.wrap_boundaries().len());
        let seg_start_ix = if seg_ix == 0 {
            0
        } else {
            Self::wrap_boundary_index(line_layout, seg_ix.saturating_sub(1)).unwrap_or(0)
        };
        let seg_end_ix = if seg_ix < line_layout.wrap_boundaries().len() {
            Self::wrap_boundary_index(line_layout, seg_ix).unwrap_or(line_layout.len())
        } else {
            line_layout.len()
        };

        let start_x = line_layout.unwrapped_layout.x_for_index(seg_start_ix);
        let end_x = line_layout.unwrapped_layout.x_for_index(seg_end_ix);
        let seg_width = end_x - start_x;

        match text_align {
            TextAlign::Left => px(0.),
            TextAlign::Center => (bounds.size.width - seg_width) / 2.0,
            TextAlign::Right => bounds.size.width - seg_width,
        }
    }

    fn wrap_boundary_index(
        line_layout: &gpui::WrappedLineLayout,
        boundary_ix: usize,
    ) -> Option<usize> {
        let boundary = line_layout.wrap_boundaries().get(boundary_ix)?;
        let run = line_layout.unwrapped_layout.runs.get(boundary.run_ix)?;
        run.glyphs.get(boundary.glyph_ix).map(|g| g.index)
    }
}

impl IntoElement for RichTextLineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub struct RichTextBlockBoundsElement {
    state: Entity<RichTextState>,
    block_path: Vec<usize>,
    kind: SharedString,
    child: AnyElement,
}

impl RichTextBlockBoundsElement {
    pub fn new(
        state: Entity<RichTextState>,
        block_path: Vec<usize>,
        kind: SharedString,
        child: AnyElement,
    ) -> Self {
        Self {
            state,
            block_path,
            kind,
            child,
        }
    }
}

impl IntoElement for RichTextBlockBoundsElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextBlockBoundsElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let block_path = self.block_path.clone();
        let kind = self.kind.clone();
        self.state.update(cx, |state, _| {
            state.block_bounds_cache.insert(
                block_path,
                BlockBoundsCache {
                    bounds,
                    kind: kind.clone(),
                },
            );
        });

        _ = self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);

        let is_selected = self
            .state
            .read(cx)
            .selected_block_path
            .as_deref()
            .is_some_and(|p| p == self.block_path.as_slice());
        if is_selected {
            let theme = cx.theme();
            window.paint_quad(gpui::quad(
                bounds,
                theme.radius / 2.,
                gpui::transparent_black(),
                gpui::Edges::all(px(2.)),
                theme.ring,
                gpui::BorderStyle::default(),
            ));
        }
    }
}

impl Element for RichTextLineElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_element_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let (text, segments, marked_range) = {
            let state = self.state.read(cx);
            let Some(el) = element_at_path(state.editor.doc(), &self.block_path) else {
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            };
            if !is_text_block_kind(state.editor.registry(), &el.kind) {
                self.text = SharedString::default();
                return (window.request_layout(gpui::Style::default(), [], cx), ());
            }

            let mut combined = String::new();
            let mut segments = Vec::new();
            for (child_ix, child) in el.children.iter().enumerate() {
                match child {
                    Node::Text(t) => {
                        let start = combined.len();
                        combined.push_str(&t.text);
                        segments.push(InlineTextSegment {
                            child_ix,
                            start,
                            len: t.text.len(),
                            marks: t.marks.clone(),
                            kind: InlineTextSegmentKind::Text,
                        });
                    }
                    Node::Void(v) => {
                        let display = v.inline_text();
                        let start = combined.len();
                        combined.push_str(&display);
                        segments.push(InlineTextSegment {
                            child_ix,
                            start,
                            len: display.len(),
                            marks: Marks::default(),
                            kind: InlineTextSegmentKind::Void {
                                kind: v.kind.clone().into(),
                            },
                        });
                    }
                    Node::Element(_) => {}
                }
            }
            if segments.is_empty() {
                segments.push(InlineTextSegment {
                    child_ix: 0,
                    start: 0,
                    len: 0,
                    marks: Marks::default(),
                    kind: InlineTextSegmentKind::Text,
                });
            }

            let marked = (state.active_text_block_path().as_ref() == Some(&self.block_path))
                .then(|| state.ime_marked_range.clone())
                .flatten();
            (SharedString::new(combined), segments, marked)
        };

        self.text = text.clone();
        self.segments = segments.clone();

        let render_text: SharedString = if text.is_empty() { " ".into() } else { text };

        let marked_range = marked_range
            .filter(|_| !self.text.is_empty())
            .map(|r| r.start.min(self.text.len())..r.end.min(self.text.len()))
            .filter(|r| r.start < r.end);

        let mut runs = Vec::new();

        let theme = cx.theme();
        let base_style = window.text_style();
        let apply_marks = |mut style: TextStyle,
                           marks: &Marks,
                           kind: &InlineTextSegmentKind,
                           theme: &gpui_component::Theme|
         -> TextStyle {
            if marks.bold {
                style.font_weight = FontWeight::BOLD;
            }
            if marks.italic {
                style.font_style = FontStyle::Italic;
            }
            if marks.code {
                style.font_family = theme.mono_font_family.clone();
                if style.background_color.is_none() {
                    style.background_color = Some(theme.muted);
                }
            }

            if let Some(color) = marks.highlight_color.as_deref().and_then(parse_hex_color) {
                style.background_color = Some(color);
            }
            if let Some(color) = marks.text_color.as_deref().and_then(parse_hex_color) {
                style.color = color;
            }

            let is_link = marks.link.is_some();
            if is_link {
                style.color = theme.blue;
            }
            if marks.underline || is_link {
                style.underline = Some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(style.color),
                    wavy: false,
                });
            }
            if marks.strikethrough {
                style.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.),
                    color: Some(style.color),
                });
            }

            if matches!(
                kind,
                InlineTextSegmentKind::Void { kind } if kind.as_ref() == "mention"
            ) {
                style.color = theme.foreground;
                style.background_color = Some(theme.muted);
                style.underline = None;
                style.strikethrough = None;
                style.font_weight = FontWeight::MEDIUM;
            }

            style
        };

        if self.text.is_empty() {
            let marks = segments
                .first()
                .map(|s| s.marks.clone())
                .unwrap_or_default();
            let kind = InlineTextSegmentKind::Text;
            let style = apply_marks(base_style.clone(), &marks, &kind, theme);
            runs.push(style.to_run(1));
        } else {
            for seg in segments.iter().filter(|s| s.len > 0) {
                let style = apply_marks(base_style.clone(), &seg.marks, &seg.kind, theme);

                if let Some(marked) = marked_range.clone() {
                    let seg_start = seg.start;
                    let seg_end = seg.start + seg.len;
                    let sel_start = marked.start.max(seg_start);
                    let sel_end = marked.end.min(seg_end);
                    if sel_start >= sel_end {
                        runs.push(style.to_run(seg.len));
                        continue;
                    }

                    let before = sel_start - seg_start;
                    let inside = sel_end - sel_start;
                    let after = seg_end - sel_end;

                    if before > 0 {
                        runs.push(style.clone().to_run(before));
                    }

                    let mut marked_style = style.clone();
                    marked_style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(window.text_style().color),
                        wavy: false,
                    });
                    runs.push(marked_style.to_run(inside));

                    if after > 0 {
                        runs.push(style.to_run(after));
                    }
                } else {
                    runs.push(style.to_run(seg.len));
                }
            }
        }

        self.styled_text = StyledText::new(render_text.clone()).with_runs(runs);
        let (layout_id, _) =
            self.styled_text
                .request_layout(global_element_id, inspector_id, window, cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text
            .prepaint(id, inspector_id, bounds, &mut (), window, cx);

        let text_layout = self.styled_text.layout().clone();
        let block_path = self.block_path.clone();
        let text = self.text.clone();
        let segments = self.segments.clone();
        let text_align = {
            let state = self.state.read(cx);
            state.text_align_for_block(&block_path)
        };
        self.state.update(cx, |state, _| {
            state.layout_cache.insert(
                block_path,
                LineLayoutCache {
                    bounds,
                    text_layout: text_layout.clone(),
                    text_align,
                    text,
                    segments,
                },
            );
        });

        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let hitbox = prepaint.clone();
        window.set_cursor_style(CursorStyle::IBeam, &hitbox);

        self.styled_text
            .paint(global_id, None, bounds, &mut (), &mut (), window, cx);

        let layout = self.styled_text.layout().clone();
        let line_height = layout.line_height();
        let text_align = window.text_style().text_align;
        // Paint find highlights and selection/caret for this row.
        let (selection, is_focused) = {
            let state = self.state.read(cx);

            if let Some(find_ranges) = state.find_matches.get(&self.block_path) {
                let active_ix = state.find_active.as_ref().and_then(|active| {
                    if active.block_path == self.block_path {
                        Some(active.range_index)
                    } else {
                        None
                    }
                });
                let mut base_color = cx.theme().warning;
                base_color.a = (base_color.a * 0.35).clamp(0.0, 1.0);
                let mut active_color = cx.theme().warning;
                active_color.a = (active_color.a * 0.65).clamp(0.0, 1.0);

                for (ix, range) in find_ranges.iter().enumerate() {
                    let color = if active_ix == Some(ix) {
                        active_color
                    } else {
                        base_color
                    };
                    Self::paint_selection_range(
                        &layout,
                        range.clone(),
                        &bounds,
                        line_height,
                        window,
                        color,
                        text_align,
                    );
                }
            }

            (
                state.editor.selection().clone(),
                state.focus_handle.is_focused(window),
            )
        };

        if selection.is_collapsed() {
            if selection
                .focus
                .path
                .split_last()
                .map(|(_, p)| p == self.block_path.as_slice())
                .unwrap_or(false)
                && is_focused
            {
                let caret_ix = {
                    let state = self.state.read(cx);
                    state.row_offset_for_point(&selection.focus).unwrap_or(0)
                }
                .min(layout.len());
                if let Some(pos) = layout
                    .position_for_index(caret_ix)
                    .or_else(|| layout.position_for_index(layout.len()))
                {
                    let offset_x =
                        Self::align_offset_x_for_index(&layout, &bounds, caret_ix, text_align);
                    let pos = point(pos.x + offset_x, pos.y);
                    window.paint_quad(gpui::quad(
                        Bounds::from_corners(pos, point(pos.x + px(1.5), pos.y + line_height)),
                        px(0.),
                        window.text_style().color,
                        gpui::Edges::default(),
                        gpui::transparent_black(),
                        gpui::BorderStyle::default(),
                    ));
                }
            }
            return;
        }

        let mut start = selection.anchor.clone();
        let mut end = selection.focus.clone();
        if start.path == end.path {
            if end.offset < start.offset {
                std::mem::swap(&mut start, &mut end);
            }
        } else if end.path < start.path {
            std::mem::swap(&mut start, &mut end);
        }
        if start.path.len() < 2 || end.path.len() < 2 {
            return;
        }

        let Some(start_block) = start.path.split_last().map(|(_, p)| p) else {
            return;
        };
        let Some(end_block) = end.path.split_last().map(|(_, p)| p) else {
            return;
        };
        if self.block_path.as_slice() < start_block || self.block_path.as_slice() > end_block {
            return;
        }

        let row_len = self.text.len();
        let (a, b) = if start_block == end_block {
            let state = self.state.read(cx);
            let a = state.row_offset_for_point(&start).unwrap_or(0).min(row_len);
            let b = state.row_offset_for_point(&end).unwrap_or(0).min(row_len);
            (a, b)
        } else if self.block_path.as_slice() == start_block {
            let state = self.state.read(cx);
            let a = state.row_offset_for_point(&start).unwrap_or(0).min(row_len);
            (a, row_len)
        } else if self.block_path.as_slice() == end_block {
            let state = self.state.read(cx);
            let b = state.row_offset_for_point(&end).unwrap_or(0).min(row_len);
            (0, b)
        } else {
            (0, row_len)
        };

        if a >= b {
            return;
        }

        Self::paint_selection_range(
            &layout,
            a..b,
            &bounds,
            line_height,
            window,
            cx.theme().selection,
            text_align,
        );
    }
}

pub(crate) struct RichTextInputHandlerElement {
    state: Entity<RichTextState>,
}

impl RichTextInputHandlerElement {
    pub(crate) fn new(state: Entity<RichTextState>) -> Self {
        Self { state }
    }
}

impl IntoElement for RichTextInputHandlerElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichTextInputHandlerElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.state.update(cx, |state, _| {
            state.viewport_bounds = bounds;
        });
        // Mirror the existing rich text editor behavior: capture mouse events for selection,
        // but allow scrolling to propagate to the underlying scroll area.
        window.insert_hitbox(bounds, HitboxBehavior::BlockMouseExceptScroll)
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.state.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );

        window.set_cursor_style(CursorStyle::IBeam, prepaint);

        // Mouse down focuses and sets caret.
        window.on_mouse_event({
            let state = self.state.clone();
            let hitbox = prepaint.clone();
            move |event: &MouseDownEvent, phase, window, cx| {
                if !phase.bubble() || event.button != MouseButton::Left {
                    return;
                }
                if !hitbox.is_hovered(window) {
                    return;
                }

                // Dragging the columns resize handle enters "column resizing" mode.
                if event.click_count == 1
                    && !event.modifiers.shift
                    && !event.modifiers.alt
                    && !event.modifiers.secondary()
                {
                    let target = {
                        let this = state.read(cx);
                        this.columns_resize_target_for_point(event.position)
                    };
                    if let Some((columns_path, boundary_ix)) = target {
                        let focus_handle = { state.read(cx).focus_handle.clone() };
                        window.focus(&focus_handle);

                        let start_x = event.position.x;
                        state.update(cx, |this, cx| {
                            this.begin_columns_resize(columns_path, boundary_ix, start_x, cx);
                        });
                        return;
                    }
                }

                // Clicking a block-void node (image/divider) selects it (and does not enter
                // "drag selecting" mode).
                if event.click_count == 1 && !event.modifiers.shift && !event.modifiers.secondary()
                {
                    let void_path: Option<Vec<usize>> = {
                        let this = state.read(cx);
                        this.void_block_path_for_point(event.position)
                    };
                    if let Some(void_path) = void_path {
                        let focus_handle = { state.read(cx).focus_handle.clone() };
                        window.focus(&focus_handle);

                        state.update(cx, |this, cx| {
                            if this.selected_block_path.as_deref() == Some(void_path.as_slice()) {
                                this.selected_block_path = None;
                            } else {
                                this.selected_block_path = Some(void_path);
                            }
                            this.selecting = false;
                            this.selection_anchor = None;
                            cx.notify();
                        });
                        return;
                    }
                }

                // Alt-click selects a block under the pointer (and does not enter "drag selecting"
                // mode). Repeated Alt-click cycles through nested blocks at the same location,
                // from container → inner block → ... → none.
                if event.click_count == 1
                    && event.modifiers.alt
                    && !event.modifiers.shift
                    && !event.modifiers.secondary()
                {
                    let block_paths: Vec<Vec<usize>> = {
                        let this = state.read(cx);
                        this.selectable_block_paths_for_point(event.position)
                    };
                    if !block_paths.is_empty() {
                        let focus_handle = { state.read(cx).focus_handle.clone() };
                        window.focus(&focus_handle);

                        state.update(cx, move |this, cx| {
                            match &this.selected_block_path {
                                None => {
                                    this.selected_block_path = block_paths.first().cloned();
                                }
                                Some(selected) => {
                                    if let Some(pos) =
                                        block_paths.iter().position(|p| p == selected)
                                    {
                                        this.selected_block_path =
                                            block_paths.get(pos + 1).cloned();
                                    } else {
                                        this.selected_block_path = block_paths.first().cloned();
                                    }
                                }
                            }
                            this.selecting = false;
                            this.selection_anchor = None;
                            cx.notify();
                        });
                        return;
                    }
                }

                // Clicking the toggle chevron toggles the collapsed state without entering
                // "drag selecting" mode.
                if event.click_count == 1 && !event.modifiers.shift && !event.modifiers.secondary()
                {
                    let toggle_path: Option<Vec<usize>> = (|| {
                        let this = state.read(cx);
                        let block_path = this.text_block_path_for_point(event.position)?;
                        let (child_ix, toggle_path) = block_path.split_last()?;
                        if *child_ix != 0 {
                            return None;
                        }
                        let toggle_path = toggle_path.to_vec();
                        let toggle_el = element_at_path(this.editor.doc(), &toggle_path)?;
                        if toggle_el.kind != "toggle" {
                            return None;
                        }

                        let cache = this.layout_cache.get(&block_path)?;
                        let y_ok = event.position.y >= cache.bounds.top()
                            && event.position.y <= cache.bounds.bottom();
                        if !y_ok {
                            return None;
                        }

                        let chevron_left = cache.bounds.left() - px(26.);
                        let chevron_right = cache.bounds.left() - px(2.);
                        let x_ok =
                            event.position.x >= chevron_left && event.position.x <= chevron_right;
                        x_ok.then_some(toggle_path)
                    })();

                    if let Some(toggle_path) = toggle_path {
                        let focus_handle = { state.read(cx).focus_handle.clone() };
                        window.focus(&focus_handle);

                        state.update(cx, |this, cx| {
                            let mut title_path = toggle_path.clone();
                            title_path.push(0);
                            let focus =
                                this.point_for_block_offset(&title_path, 0)
                                    .unwrap_or_else(|| {
                                        let mut path = title_path.clone();
                                        path.push(0);
                                        CorePoint::new(path, 0)
                                    });
                            this.set_selection_points(focus.clone(), focus, cx);
                            this.selecting = false;
                            this.selection_anchor = None;

                            let args = serde_json::json!({ "path": toggle_path });
                            _ = this.run_command_and_refresh(
                                "toggle.toggle_collapsed",
                                Some(args),
                                cx,
                            );
                        });
                        return;
                    }
                }

                // Clicking the todo checkbox toggles the checked state without entering
                // "drag selecting" mode.
                if event.click_count == 1 && !event.modifiers.shift && !event.modifiers.secondary()
                {
                    let toggle_path: Option<Vec<usize>> = (|| {
                        let this = state.read(cx);
                        let block_path = this.text_block_path_for_point(event.position)?;
                        let el = element_at_path(this.editor.doc(), &block_path)?;
                        if el.kind != "todo_item" {
                            return None;
                        }
                        let cache = this.layout_cache.get(&block_path)?;

                        let y_ok = event.position.y >= cache.bounds.top()
                            && event.position.y <= cache.bounds.bottom();
                        if !y_ok {
                            return None;
                        }

                        let checkbox_left = cache.bounds.left() - px(26.);
                        let checkbox_right = cache.bounds.left() - px(2.);
                        let x_ok =
                            event.position.x >= checkbox_left && event.position.x <= checkbox_right;
                        x_ok.then_some(block_path)
                    })();

                    if let Some(block_path) = toggle_path {
                        let focus_handle = { state.read(cx).focus_handle.clone() };
                        window.focus(&focus_handle);

                        state.update(cx, |this, cx| {
                            let focus =
                                this.point_for_block_offset(&block_path, 0)
                                    .unwrap_or_else(|| {
                                        let mut path = block_path.clone();
                                        path.push(0);
                                        CorePoint::new(path, 0)
                                    });
                            this.set_selection_points(focus.clone(), focus, cx);
                            this.selecting = false;
                            this.selection_anchor = None;

                            let args = serde_json::json!({ "path": block_path });
                            _ = this.run_command_and_refresh("todo.toggle_checked", Some(args), cx);
                        });
                        return;
                    }
                }

                // Cmd/Ctrl-click opens a link instead of moving the caret.
                if event.modifiers.secondary() && !event.modifiers.shift {
                    let (image_src, document_base_dir) = {
                        let this = state.read(cx);
                        let image_src =
                            if let Some(path) = this.void_block_path_for_point(event.position) {
                                match node_at_path(this.editor.doc(), &path) {
                                    Some(Node::Void(v)) if v.kind == "image" => v
                                        .attrs
                                        .get("src")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    _ => None,
                                }
                            } else {
                                None
                            };
                        (image_src, this.document_base_dir.clone())
                    };
                    if let Some(src) = image_src {
                        let src = src.trim();
                        let open_target =
                            if src.starts_with("http://") || src.starts_with("https://") {
                                Some(src.to_string())
                            } else if src.starts_with("file://") {
                                Some(src.to_string())
                            } else if let Some(path) = RichTextState::local_image_path_for_src(
                                src,
                                document_base_dir.as_deref(),
                            ) {
                                Some(format!("file://{}", path.to_string_lossy()))
                            } else {
                                None
                            };

                        if let Some(open_target) = open_target {
                            cx.open_url(&open_target);
                            return;
                        }
                    }

                    let url = {
                        let this = state.read(cx);
                        if let Some(block_path) = this.text_block_path_for_point(event.position) {
                            let offset = this
                                .offset_for_point_in_block(&block_path, event.position)
                                .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                            this.link_at_offset(&block_path, offset)
                        } else {
                            None
                        }
                    };
                    if let Some(url) = url {
                        cx.open_url(&url);
                        return;
                    }
                }

                let focus_handle = { state.read(cx).focus_handle.clone() };
                window.focus(&focus_handle);

                state.update(cx, |this, cx| {
                    let Some(block_path) = this.text_block_path_for_point(event.position) else {
                        return;
                    };

                    let offset = this
                        .offset_for_point_in_block(&block_path, event.position)
                        .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                    let focus = this
                        .point_for_block_offset(&block_path, offset)
                        .unwrap_or_else(|| {
                            let mut path = block_path.clone();
                            path.push(0);
                            CorePoint::new(path, 0)
                        });

                    if event.click_count >= 3 {
                        let range = 0..this.block_text_len_in_block(&block_path);
                        let anchor = this
                            .point_for_block_offset(&block_path, range.start)
                            .unwrap_or_else(|| focus.clone());
                        let focus = this
                            .point_for_block_offset(&block_path, range.end)
                            .unwrap_or_else(|| focus.clone());
                        this.set_selection_points(anchor, focus, cx);
                        // Do not enter "drag selecting" mode on multi-click. This avoids tiny mouse
                        // movements between down/up being interpreted as a drag that expands the
                        // selection across paragraphs (which can look like "select all").
                        this.selecting = false;
                        this.selection_anchor = None;
                        return;
                    }

                    if event.click_count == 2 {
                        let text = this.text_block_text(&block_path).unwrap_or_default();
                        let range =
                            this.word_or_void_range_in_block(&block_path, text.as_str(), offset);
                        let anchor = this
                            .point_for_block_offset(&block_path, range.start)
                            .unwrap_or_else(|| focus.clone());
                        let focus = this
                            .point_for_block_offset(&block_path, range.end)
                            .unwrap_or_else(|| focus.clone());
                        this.set_selection_points(anchor.clone(), focus, cx);
                        this.selecting = false;
                        this.selection_anchor = None;
                        return;
                    }

                    let anchor = if event.modifiers.shift {
                        this.editor.selection().anchor.clone()
                    } else {
                        focus.clone()
                    };

                    this.set_selection_points(anchor.clone(), focus, cx);
                    this.selecting = true;
                    this.selection_anchor = Some(anchor);
                });
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseMoveEvent, _phase, _window, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                state.update(cx, |this, cx| {
                    if this.columns_resizing.is_some() {
                        this.update_columns_resize_preview(event.position.x, cx);
                        return;
                    }
                    if !this.selecting {
                        return;
                    }
                    let Some(block_path) = this.text_block_path_for_point(event.position) else {
                        return;
                    };
                    let offset = this
                        .offset_for_point_in_block(&block_path, event.position)
                        .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                    let focus = this
                        .point_for_block_offset(&block_path, offset)
                        .unwrap_or_else(|| {
                            let mut path = block_path.clone();
                            path.push(0);
                            CorePoint::new(path, 0)
                        });
                    let anchor = this
                        .selection_anchor
                        .clone()
                        .unwrap_or_else(|| focus.clone());
                    this.set_selection_points(anchor, focus, cx);
                });
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            move |event: &MouseUpEvent, _phase, _window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                state.update(cx, |this, cx| {
                    if this.columns_resizing.is_some() {
                        this.commit_columns_resize(cx);
                        this.selecting = false;
                        this.selection_anchor = None;
                        return;
                    }
                    this.selecting = false;
                    this.selection_anchor = None;
                    cx.notify();
                });
            }
        });
    }
}

impl Render for RichTextState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.pending_notifications.is_empty() {
            for message in std::mem::take(&mut self.pending_notifications) {
                window.push_notification(Notification::new().message(message).autohide(true), cx);
            }
        }

        let theme = cx.theme().clone();
        let state = cx.entity().clone();
        let registry = self.editor.registry();
        let columns_resizing = self.columns_resizing.clone();

        if !self.did_auto_focus && !window.is_inspector_picking(cx) {
            window.focus(&self.focus_handle);
            self.did_auto_focus = true;
        }

        fn render_text_block(
            el: &ElementNode,
            path: Vec<usize>,
            _window: &mut Window,
            theme: &gpui_component::Theme,
            state: &Entity<RichTextState>,
        ) -> AnyElement {
            let line = RichTextLineElement::new(state.clone(), path);

            let align = el.attrs.get("align").and_then(|v| v.as_str());
            let font_size = el.attrs.get("font_size").and_then(|v| v.as_u64());
            let mut text = div().w_full().min_w(px(0.));
            match align {
                Some("center") => text = text.text_center(),
                Some("right") => text = text.text_right(),
                _ => {}
            }

            if el.kind == "heading" {
                let level = el
                    .attrs
                    .get("level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, 6);
                let default_size: u64 = match level {
                    1 => 28,
                    2 => 22,
                    3 => 18,
                    4 => 16,
                    5 => 14,
                    _ => 13,
                };
                let size = font_size.unwrap_or(default_size) as f32;
                text = text
                    .text_size(px(size))
                    .line_height(px(size * 1.25))
                    .font_weight(FontWeight::SEMIBOLD);
            } else if let Some(size) = font_size {
                text = text.text_size(px(size as f32));
            }

            if el.kind == "code_block" {
                text = text.font_family(theme.mono_font_family.clone());
            }

            let text = text.child(line);

            let block = if el.kind == "code_block" {
                div()
                    .w_full()
                    .bg(theme.muted)
                    .border_1()
                    .border_color(theme.border)
                    .rounded(theme.radius)
                    .p(px(8.))
                    .child(text)
                    .into_any_element()
            } else {
                text.into_any_element()
            };

            let indent_level = el.attrs.get("indent").and_then(|v| v.as_u64()).unwrap_or(0);
            if indent_level == 0 {
                return block;
            }

            div()
                .pl(px(16. * indent_level as f32))
                .child(block)
                .into_any_element()
        }

        fn render_list_item(
            el: &ElementNode,
            path: Vec<usize>,
            theme: &gpui_component::Theme,
            state: &Entity<RichTextState>,
        ) -> AnyElement {
            let list_type = el
                .attrs
                .get("list_type")
                .and_then(|v| v.as_str())
                .unwrap_or("bulleted");
            let marker = if list_type == "ordered" {
                let ix = el
                    .attrs
                    .get("list_index")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                format!("{ix}.")
            } else {
                "•".to_string()
            };
            let level = el
                .attrs
                .get("list_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let indent = px(16. * level as f32);
            let align = el.attrs.get("align").and_then(|v| v.as_str());
            let font_size = el.attrs.get("font_size").and_then(|v| v.as_u64());
            let mut content = div().flex_1().min_w(px(0.));
            match align {
                Some("center") => content = content.text_center(),
                Some("right") => content = content.text_right(),
                _ => {}
            }
            if let Some(size) = font_size {
                content = content.text_size(px(size as f32));
            }
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.))
                .pl(indent)
                .child(
                    div()
                        .w(px(24.))
                        .flex()
                        .justify_end()
                        .text_color(theme.muted_foreground)
                        .child(marker),
                )
                .child(content.child(RichTextLineElement::new(state.clone(), path)))
                .into_any_element()
        }

        fn render_todo_item(
            el: &ElementNode,
            path: Vec<usize>,
            _window: &mut Window,
            theme: &gpui_component::Theme,
            state: &Entity<RichTextState>,
        ) -> AnyElement {
            let checked = el
                .attrs
                .get("checked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let indent_level = el.attrs.get("indent").and_then(|v| v.as_u64()).unwrap_or(0);
            let font_size = el.attrs.get("font_size").and_then(|v| v.as_u64());

            let checkbox = div()
                .w(px(18.))
                .h(px(18.))
                .mt(px(2.))
                .rounded(px(4.))
                .border_1()
                .border_color(if checked { theme.blue } else { theme.border })
                .flex()
                .items_center()
                .justify_center()
                .text_color(if checked {
                    theme.blue
                } else {
                    theme.muted_foreground
                })
                .child(if checked { "✓" } else { "" });

            let align = el.attrs.get("align").and_then(|v| v.as_str());
            let mut content = div().flex_1().min_w(px(0.));
            match align {
                Some("center") => content = content.text_center(),
                Some("right") => content = content.text_right(),
                _ => {}
            }
            if let Some(size) = font_size {
                content = content.text_size(px(size as f32));
            }
            if checked {
                content = content.text_color(theme.muted_foreground).line_through();
            }
            content = content.child(RichTextLineElement::new(state.clone(), path));

            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.))
                .when(indent_level > 0, |this| {
                    this.pl(px(16. * indent_level as f32))
                })
                .child(checkbox)
                .child(content)
                .into_any_element()
        }

        fn render_toggle(
            el: &ElementNode,
            path: Vec<usize>,
            window: &mut Window,
            theme: &gpui_component::Theme,
            state: &Entity<RichTextState>,
            registry: &PluginRegistry,
            columns_resizing: Option<&ColumnsResizeState>,
            embedded_image_cache: &mut HashMap<String, Arc<Image>>,
            document_base_dir: Option<&Path>,
        ) -> AnyElement {
            let collapsed = el
                .attrs
                .get("collapsed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let Some(Node::Element(title_el)) = el.children.first() else {
                return div()
                    .text_color(theme.muted_foreground)
                    .italic()
                    .child(format!("<invalid toggle at {:?}>", path))
                    .into_any_element();
            };

            let mut title_path = path.clone();
            title_path.push(0);

            let line = RichTextLineElement::new(state.clone(), title_path);

            let align = title_el.attrs.get("align").and_then(|v| v.as_str());
            let font_size = title_el.attrs.get("font_size").and_then(|v| v.as_u64());
            let mut title = div().flex_1().min_w(px(0.));
            match align {
                Some("center") => title = title.text_center(),
                Some("right") => title = title.text_right(),
                _ => {}
            }
            if title_el.kind == "heading" {
                let level = title_el
                    .attrs
                    .get("level")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1)
                    .clamp(1, 6);
                let default_size: u64 = match level {
                    1 => 28,
                    2 => 22,
                    3 => 18,
                    4 => 16,
                    5 => 14,
                    _ => 13,
                };
                let size = font_size.unwrap_or(default_size) as f32;
                title = title
                    .text_size(px(size))
                    .line_height(px(size * 1.25))
                    .font_weight(FontWeight::SEMIBOLD);
            } else if let Some(size) = font_size {
                title = title.text_size(px(size as f32));
            }
            let title = title.child(line);

            let indent_level = title_el
                .attrs
                .get("indent")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let chevron = if collapsed { "▸" } else { "▾" };

            let title_row = div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.))
                .when(indent_level > 0, |this| {
                    this.pl(px(16. * indent_level as f32))
                })
                .child(
                    div()
                        .w(px(18.))
                        .h(px(18.))
                        .mt(px(2.))
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(theme.muted_foreground)
                        .child(chevron),
                )
                .child(title)
                .into_any_element();

            if collapsed {
                return div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(title_row)
                    .into_any_element();
            }

            let mut content: Vec<AnyElement> = Vec::new();
            for (ix, child) in el.children.iter().enumerate().skip(1) {
                let mut child_path = path.clone();
                child_path.push(ix);
                content.push(render_block(
                    child,
                    child_path,
                    window,
                    theme,
                    state,
                    registry,
                    columns_resizing,
                    embedded_image_cache,
                    document_base_dir,
                ));
            }

            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(title_row)
                .when(!content.is_empty(), |this| {
                    this.child(
                        div()
                            .pl(px(26.))
                            .child(div().flex_col().gap(px(6.)).children(content)),
                    )
                })
                .into_any_element()
        }

        fn render_columns(
            el: &ElementNode,
            path: Vec<usize>,
            window: &mut Window,
            theme: &gpui_component::Theme,
            state: &Entity<RichTextState>,
            registry: &PluginRegistry,
            columns_resizing: Option<&ColumnsResizeState>,
            embedded_image_cache: &mut HashMap<String, Arc<Image>>,
            document_base_dir: Option<&Path>,
        ) -> AnyElement {
            let col_count = el.children.len();
            if col_count < 2 {
                return div()
                    .text_color(theme.muted_foreground)
                    .italic()
                    .child(format!("<invalid columns at {:?}>", path))
                    .into_any_element();
            }

            let mut widths: Vec<f32> =
                if columns_resizing.is_some_and(|r| r.columns_path.as_slice() == path.as_slice()) {
                    columns_resizing
                        .map(|r| r.preview_fractions.clone())
                        .unwrap_or_default()
                } else {
                    el.attrs
                        .get("widths")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_f64())
                                .map(|v| v as f32)
                                .collect()
                        })
                        .unwrap_or_default()
                };
            if widths.len() != col_count {
                widths = vec![1.0 / col_count as f32; col_count];
            }

            let mut row: Vec<AnyElement> = Vec::new();
            for col_ix in 0..col_count {
                let width = widths
                    .get(col_ix)
                    .copied()
                    .unwrap_or(1.0 / col_count as f32)
                    .clamp(0.05, 0.95);

                let mut col_path = path.clone();
                col_path.push(col_ix);

                let blocks: Vec<AnyElement> = match el.children.get(col_ix) {
                    Some(Node::Element(col_el)) if col_el.kind == "column" => col_el
                        .children
                        .iter()
                        .enumerate()
                        .map(|(block_ix, block_node)| {
                            let mut block_path = col_path.clone();
                            block_path.push(block_ix);
                            render_block(
                                block_node,
                                block_path,
                                window,
                                theme,
                                state,
                                registry,
                                columns_resizing,
                                embedded_image_cache,
                                document_base_dir,
                            )
                        })
                        .collect(),
                    Some(other) => vec![
                        div()
                            .text_color(theme.muted_foreground)
                            .italic()
                            .child(format!(
                                "<invalid column child {:?}: {:?}>",
                                col_path,
                                match other {
                                    Node::Element(el) => el.kind.as_str(),
                                    Node::Void(v) => v.kind.as_str(),
                                    Node::Text(_) => "text",
                                }
                            ))
                            .into_any_element(),
                    ],
                    None => Vec::new(),
                };

                let content = div().flex().flex_col().gap(px(6.)).children(blocks);

                row.push(
                    RichTextBlockBoundsElement::new(
                        state.clone(),
                        col_path,
                        SharedString::from("column"),
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .flex_shrink()
                            .flex_basis(relative(width))
                            .min_w(px(0.))
                            .p(px(8.))
                            .child(content.into_any_element())
                            .into_any_element(),
                    )
                    .into_any_element(),
                );

                if col_ix + 1 < col_count {
                    row.push(
                        div()
                            .w(px(12.))
                            .flex_none()
                            .cursor_col_resize()
                            .hover(|this| this.bg(theme.muted))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(div().w(px(2.)).h_full().bg(theme.border).mx_auto())
                            .into_any_element(),
                    );
                }
            }

            div()
                .flex()
                .flex_row()
                .border_1()
                .border_color(theme.border)
                .rounded(theme.radius / 2.)
                .children(row)
                .into_any_element()
        }

        fn render_block(
            node: &Node,
            path: Vec<usize>,
            window: &mut Window,
            theme: &gpui_component::Theme,
            state: &Entity<RichTextState>,
            registry: &PluginRegistry,
            columns_resizing: Option<&ColumnsResizeState>,
            embedded_image_cache: &mut HashMap<String, Arc<Image>>,
            document_base_dir: Option<&Path>,
        ) -> AnyElement {
            let kind: SharedString = match node {
                Node::Element(el) => el.kind.clone().into(),
                Node::Void(v) => v.kind.clone().into(),
                Node::Text(_) => "text".into(),
            };

            let inner = match node {
                Node::Element(el) if el.kind == "todo_item" => {
                    render_todo_item(el, path.clone(), window, theme, state)
                }
                Node::Element(el) if el.kind == "list_item" => {
                    render_list_item(el, path.clone(), theme, state)
                }
                Node::Element(el) if is_text_block_kind(registry, &el.kind) => {
                    render_text_block(el, path.clone(), window, theme, state)
                }
                Node::Element(el) if el.kind == "toggle" => render_toggle(
                    el,
                    path.clone(),
                    window,
                    theme,
                    state,
                    registry,
                    columns_resizing,
                    embedded_image_cache,
                    document_base_dir,
                ),
                Node::Element(el) if el.kind == "columns" => render_columns(
                    el,
                    path.clone(),
                    window,
                    theme,
                    state,
                    registry,
                    columns_resizing,
                    embedded_image_cache,
                    document_base_dir,
                ),
                Node::Element(el) if el.kind == "blockquote" => {
                    let mut children: Vec<AnyElement> = Vec::new();
                    for (ix, child) in el.children.iter().enumerate() {
                        let mut child_path = path.clone();
                        child_path.push(ix);
                        children.push(render_block(
                            child,
                            child_path,
                            window,
                            theme,
                            state,
                            registry,
                            columns_resizing,
                            embedded_image_cache,
                            document_base_dir,
                        ));
                    }

                    div()
                        .flex()
                        .flex_row()
                        .gap(px(10.))
                        .child(div().w(px(3.)).rounded(px(99.)).bg(theme.border))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .child(div().flex_col().gap(px(6.)).children(children)),
                        )
                        .into_any_element()
                }
                Node::Element(el) if el.kind == "table" => {
                    let mut rows: Vec<AnyElement> = Vec::new();
                    for (row_ix, row_node) in el.children.iter().enumerate() {
                        let Node::Element(row_el) = row_node else {
                            continue;
                        };
                        if row_el.kind != "table_row" {
                            continue;
                        }

                        let mut cells: Vec<AnyElement> = Vec::new();
                        for (cell_ix, cell_node) in row_el.children.iter().enumerate() {
                            let Node::Element(cell_el) = cell_node else {
                                continue;
                            };
                            if cell_el.kind != "table_cell" {
                                continue;
                            }

                            let mut cell_blocks: Vec<AnyElement> = Vec::new();
                            for (block_ix, block_node) in cell_el.children.iter().enumerate() {
                                let mut block_path = path.clone();
                                block_path.push(row_ix);
                                block_path.push(cell_ix);
                                block_path.push(block_ix);
                                cell_blocks.push(render_block(
                                    block_node,
                                    block_path,
                                    window,
                                    theme,
                                    state,
                                    registry,
                                    columns_resizing,
                                    embedded_image_cache,
                                    document_base_dir,
                                ));
                            }

                            cells.push(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .p(px(6.))
                                    .child(div().flex_col().gap(px(6.)).children(cell_blocks))
                                    .into_any_element(),
                            );
                        }

                        rows.push(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(6.))
                                .children(cells)
                                .into_any_element(),
                        );
                    }

                    div()
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius / 2.)
                        .p(px(6.))
                        .child(div().flex_col().gap(px(6.)).children(rows))
                        .into_any_element()
                }
                Node::Void(v) if v.kind == "image" => {
                    let src = v
                        .attrs
                        .get("src")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let alt = v.attrs.get("alt").and_then(|v| v.as_str()).unwrap_or("");
                    let is_data_url = src.starts_with("data:");
                    let caption = if alt.is_empty() {
                        if is_data_url {
                            "Embedded image".to_string()
                        } else {
                            src.to_string()
                        }
                    } else if is_data_url {
                        alt.to_string()
                    } else {
                        format!("{alt} · {src}")
                    };

                    div()
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius / 2.)
                        .overflow_hidden()
                        .child(
                            div()
                                .h(px(140.))
                                .w_full()
                                .bg(theme.muted)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(theme.muted_foreground)
                                .child(if src.is_empty() {
                                    div().child("Image").into_any_element()
                                } else {
                                    let source = if let Some(embedded) =
                                        RichTextState::get_or_decode_data_url_image(
                                            embedded_image_cache,
                                            src,
                                        ) {
                                        img(embedded)
                                    } else if let Some(local_path) =
                                        RichTextState::local_image_path_for_src(
                                            src,
                                            document_base_dir,
                                        )
                                    {
                                        img(local_path)
                                    } else {
                                        img(src.to_string())
                                    };
                                    source
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Contain)
                                        .with_loading(|| {
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child("Loading…")
                                                .into_any_element()
                                        })
                                        .with_fallback(|| {
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child("Image")
                                                .into_any_element()
                                        })
                                        .into_any_element()
                                }),
                        )
                        .child(
                            div()
                                .p(px(8.))
                                .text_color(theme.muted_foreground)
                                .text_size(px(12.))
                                .child(caption),
                        )
                        .into_any_element()
                }
                Node::Void(v) if v.kind == "divider" => div()
                    .py(px(8.))
                    .child(div().w_full().h(px(1.)).bg(theme.border))
                    .into_any_element(),
                _ => div()
                    .text_color(theme.muted_foreground)
                    .italic()
                    .child(format!(
                        "<unknown node at {:?}: {:?}>",
                        path,
                        match node {
                            Node::Element(el) => el.kind.as_str(),
                            Node::Void(v) => v.kind.as_str(),
                            Node::Text(_) => "text",
                        }
                    ))
                    .into_any_element(),
            };

            RichTextBlockBoundsElement::new(state.clone(), path, kind, inner).into_any_element()
        }

        let mut blocks: Vec<AnyElement> = Vec::new();
        for (ix, node) in self.editor.doc().children.iter().enumerate() {
            blocks.push(render_block(
                node,
                vec![ix],
                window,
                &theme,
                &state,
                registry,
                columns_resizing.as_ref(),
                &mut self.embedded_image_cache,
                self.document_base_dir.as_deref(),
            ));
        }

        div()
            .id(("richtext-next", cx.entity_id()))
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .w_full()
            .h_full()
            .relative()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius)
            .when(!window.is_inspector_picking(cx), |this| {
                this.on_action(window.listener_for(&state, RichTextState::backspace))
                    .on_action(window.listener_for(&state, RichTextState::delete))
                    .on_action(window.listener_for(&state, RichTextState::enter))
                    .on_action(window.listener_for(&state, RichTextState::left))
                    .on_action(window.listener_for(&state, RichTextState::right))
                    .on_action(window.listener_for(&state, RichTextState::select_left))
                    .on_action(window.listener_for(&state, RichTextState::select_right))
                    .on_action(window.listener_for(&state, RichTextState::undo))
                    .on_action(window.listener_for(&state, RichTextState::redo))
                    .on_action(window.listener_for(&state, RichTextState::copy))
                    .on_action(window.listener_for(&state, RichTextState::cut))
                    .on_action(window.listener_for(&state, RichTextState::paste))
                    .on_action(window.listener_for(&state, RichTextState::select_all))
                    .on_action(window.listener_for(&state, RichTextState::insert_divider))
                    .on_action(window.listener_for(&state, RichTextState::insert_mention))
                    .on_action(window.listener_for(&state, RichTextState::toggle_bold))
                    .on_action(window.listener_for(&state, RichTextState::toggle_italic))
                    .on_action(window.listener_for(&state, RichTextState::toggle_underline))
                    .on_action(window.listener_for(&state, RichTextState::toggle_strikethrough))
                    .on_action(window.listener_for(&state, RichTextState::toggle_code))
                    .on_action(window.listener_for(&state, RichTextState::toggle_bulleted_list))
                    .on_action(window.listener_for(&state, RichTextState::toggle_ordered_list))
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        window.focus(&this.focus_handle);

                        let drop_pos = window.mouse_position();
                        if let Some(block_path) = this.text_block_path_for_point(drop_pos) {
                            let offset = this
                                .offset_for_point_in_block(&block_path, drop_pos)
                                .unwrap_or_else(|| this.block_text_len_in_block(&block_path));
                            let focus = this
                                .point_for_block_offset(&block_path, offset)
                                .unwrap_or_else(|| {
                                    let mut path = block_path.clone();
                                    path.push(0);
                                    CorePoint::new(path, 0)
                                });
                            this.set_selection_points(focus.clone(), focus, cx);
                        }

                        let mut srcs: Vec<String> = Vec::new();
                        for path in paths.paths() {
                            let ext = path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let is_image = matches!(
                                ext.as_str(),
                                "png"
                                    | "jpg"
                                    | "jpeg"
                                    | "gif"
                                    | "webp"
                                    | "bmp"
                                    | "svg"
                                    | "tif"
                                    | "tiff"
                            );
                            if !is_image {
                                continue;
                            }
                            srcs.push(path.to_string_lossy().to_string());
                        }

                        if srcs.is_empty() {
                            return;
                        }
                        this.command_insert_images(srcs, cx);
                    }))
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(RichTextInputHandlerElement::new(state.clone())),
            )
            .child(
                div()
                    .id("scroll-area")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(div().p(px(12.)).flex_col().gap(px(6.)).children(blocks)),
            )
    }
}

fn utf16_to_byte(s: &str, utf16_ix: usize) -> usize {
    if utf16_ix == 0 {
        return 0;
    }
    let mut utf16_count = 0usize;
    for (byte_ix, ch) in s.char_indices() {
        if utf16_count >= utf16_ix {
            return byte_ix;
        }
        utf16_count += ch.len_utf16();
    }
    s.len()
}

fn byte_to_utf16(s: &str, byte_ix: usize) -> usize {
    let byte_ix = byte_ix.min(s.len());
    let mut utf16_count = 0usize;
    for (ix, ch) in s.char_indices() {
        if ix >= byte_ix {
            break;
        }
        utf16_count += ch.len_utf16();
    }
    utf16_count
}

fn byte_to_utf16_range(s: &str, range: Range<usize>) -> Range<usize> {
    byte_to_utf16(s, range.start)..byte_to_utf16(s, range.end)
}

fn parse_hex_color(value: &str) -> Option<gpui::Hsla> {
    gpui::Rgba::try_from(value).ok().map(gpui::Hsla::from)
}
