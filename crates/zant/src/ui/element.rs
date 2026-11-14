//! Declaration nodes: `ui::Node` / `ui::Element` (port of `src/ui.zig:14-87`).
//!
//! A node owns its data in an `Rc<Element>`: cheap to clone for `styled()` and
//! `with_key()`, no frame arena and no lifetimes (plan §4). Every `ui::*`
//! builder returns a fresh `Element`, so no two nodes share mutable state.
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::Str;
use crate::core::node::{
    ButtonIcon, ButtonSize, ButtonType, FlexStyle, HoverVisual, ImageFit, MotionStyle, NodeKind,
    PointerArea, RichSpan, ToggleKind, Visual,
};
use crate::core::vector::DrawingRef;
use crate::image::Image;
use crate::platform::platform::Cursor;
use crate::renderer::text;

use super::actions::{
    Action, BoolAction, IndexAction, InputKeyAction, KeyAction, RangeAction, TextAction,
};

/// A declarative node: `Rc<Element>` so cloning is cheap (Zig `*const Element`
/// pointing into the frame arena; the arena is gone in Rust).
#[derive(Clone)]
pub struct Node(Rc<Element>);

impl PartialEq for Node {
    /// Node identity: generated TSX props derive `PartialEq` and compare the
    /// node fields (a rebuilt tree makes a new node, like React's element
    /// identity).
    fn eq(&self, other: &Node) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Node {}

/// A node that is a `MemoBuild` subtree (`ui::memo`, WP-2.4).
#[derive(Clone)]
pub struct MemoBuild(Rc<dyn Fn() -> Node>);

impl MemoBuild {
    /// Builds the memoized subtree. The caller must not hold the build-context
    /// `RefCell` borrow (the builder calls `ui::*`, which borrows it again).
    pub fn new(build: impl Fn() -> Node + 'static) -> MemoBuild {
        MemoBuild(Rc::new(build))
    }

    pub fn build(&self) -> Node {
        (self.0)()
    }
}

/// Everything a declared node carries into the retained tree.
///
/// Field names and order match the Zig `Element` (`src/ui.zig:18-87`) so the
/// two can be reviewed side by side. Rust ownership changes a few types:
/// `[]const u8` → [`Str`], slices → `Vec`, `?*anyopaque` image handle →
/// `Option<Rc<Image>>`, `?*const vector.Drawing` → `Option<DrawingRef>`.
#[derive(Clone)]
pub struct Element {
    pub kind: NodeKind,
    pub key: Option<Str>,
    pub layout: FlexStyle,
    pub visual: Visual,
    pub enabled: bool,
    pub focusable: bool,
    pub content: Str,
    pub placeholder: Str,
    pub text_style: text::Style,
    pub button_type: ButtonType,
    pub button_size: ButtonSize,
    pub toggle_kind: ToggleKind,
    pub checked: bool,
    pub controlled: bool,
    pub read_only: bool,
    pub password: bool,
    pub multiline: bool,
    pub focus_ring: bool,
    pub text_color: Option<u32>,
    pub label_padding: f32,
    pub icon: ButtonIcon,
    pub icon_size: f32,
    pub on_click: Option<Action>,
    pub on_bool: Option<BoolAction>,
    pub on_text: Option<TextAction>,
    pub on_undo: Option<Action>,
    pub on_redo: Option<Action>,
    pub index_action: Option<IndexAction>,
    pub index: usize,
    pub tooltip: Option<Str>,
    pub image_fit: ImageFit,
    pub options: Vec<Str>,
    pub selected: usize,
    pub children: Vec<Node>,
    pub rich_spans: Vec<RichSpan>,
    /// Rich text line height multiplier (0 = font default).
    pub rich_line_height: f32,
    /// Rich text link activation; receives the clicked span's `link`.
    pub on_link: Option<TextAction>,
    /// Controlled input selection `(anchor, cursor)`; applied when it changes.
    pub selection: Option<[usize; 2]>,
    /// Input selection change notifications `(anchor, cursor)`.
    pub on_selection: Option<RangeAction>,
    /// Input key hook (runs before the input's own editing keys).
    pub on_key: Option<InputKeyAction>,
    /// Centred text column width for wide multiline inputs (0 = full width).
    pub content_max_width: f32,
    /// Vector drawing (`ui::vector`).
    pub drawing: Option<DrawingRef>,
    /// Memoized subtree: when the hash equals the mounted node's, the
    /// reconciler keeps that subtree without calling `memo_build`.
    pub memo_hash: Option<u64>,
    pub memo_build: Option<MemoBuild>,
    /// Enter / exit and layout transitions (`ui::styled(.., .{ .motion = .. })`).
    pub motion: MotionStyle,
    /// Pointer area handler (`ui::pointer_area`).
    pub pointer: Option<PointerArea>,
    /// Key handler of a focused pointer area.
    pub key_action: Option<KeyAction>,
    /// Cursor over a pointer area.
    pub cursor: Option<Cursor>,
    /// Container size changes (`BoxOptions::on_resize`).
    pub on_resize: Option<Action>,
    /// Container hover look (`BoxOptions::hover_*`).
    pub hover_visual: Option<HoverVisual>,
    /// Image already decoded in memory (`ui::image(.{ .image = .. })`); the
    /// node keeps it alive, so `ui::image` can return a node that outlives the
    /// caller's `Image` (Zig kept a retained raw handle + id).
    pub image_handle: Option<Rc<Image>>,
    pub image_id: u64,
}

impl Default for Element {
    /// The field defaults of the Zig `Element` struct. Hand-written because
    /// several are non-zero (`enabled`, `focus_ring`, `icon_size`,
    /// `button_size`, `toggle_kind`, `image_fit`, `rich_line_height`);
    /// `kind` has no Zig default and starts as `NodeKind::Root`.
    fn default() -> Self {
        Element {
            kind: NodeKind::Root,
            key: None,
            layout: FlexStyle::default(),
            visual: Visual::default(),
            enabled: true,
            focusable: false,
            content: Str::default(),
            placeholder: Str::default(),
            text_style: text::Style::default(),
            button_type: ButtonType::Default,
            button_size: ButtonSize::Middle,
            toggle_kind: ToggleKind::Checkbox,
            checked: false,
            controlled: false,
            read_only: false,
            password: false,
            multiline: false,
            focus_ring: true,
            text_color: None,
            label_padding: 0.0,
            icon: ButtonIcon::None,
            icon_size: 18.0,
            on_click: None,
            on_bool: None,
            on_text: None,
            on_undo: None,
            on_redo: None,
            index_action: None,
            index: 0,
            tooltip: None,
            image_fit: ImageFit::Cover,
            options: Vec::new(),
            selected: 0,
            children: Vec::new(),
            rich_spans: Vec::new(),
            rich_line_height: 2.0,
            on_link: None,
            selection: None,
            on_selection: None,
            on_key: None,
            content_max_width: 0.0,
            drawing: None,
            memo_hash: None,
            memo_build: None,
            motion: MotionStyle::default(),
            pointer: None,
            key_action: None,
            cursor: None,
            on_resize: None,
            hover_visual: None,
            image_handle: None,
            image_id: 0,
        }
    }
}

impl Node {
    /// Wraps a freshly built element (Zig `make`).
    pub fn new(element: Element) -> Node {
        Node(Rc::new(element))
    }

    pub fn element(&self) -> &Element {
        &self.0
    }

    /// A deep-enough copy of the element: `styled()` / `with_key()` /
    /// `append_children()` copy the `Rc` contents and change a few fields.
    pub(crate) fn element_clone(&self) -> Element {
        (*self.0).clone()
    }

    /// Whether both nodes share one `Element` (the reconciler's identity test;
    /// cloning a node keeps the pointer, building one makes a new element).
    pub fn ptr_eq(&self, other: &Node) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for Node {
    /// An empty `Root` element; lets option structs derive `Default` (the Zig
    /// `Node` has no default, but nothing paints a lone root node).
    fn default() -> Self {
        Node::new(Element::default())
    }
}

/// A copy of `node` with `extra` appended to its children (the runtime uses it
/// for overlays such as the Windows caption buttons).
pub fn append_children(node: Node, extra: &[Node]) -> Node {
    if extra.is_empty() {
        return node;
    }
    let mut element = node.element_clone();
    element.children.extend_from_slice(extra);
    Node::new(element)
}

/// Returns `node` with `key` set (used to key memoized subtrees).
pub fn with_key(node: Node, key: impl Into<Str>) -> Node {
    let mut element = node.element_clone();
    element.key = Some(key.into());
    Node::new(element)
}

/// Flattens the arguments of [`nodes!`] into a `Vec<Node>`.
///
/// `children: nodes![title, Some(subtitle), list]` accepts `Node`,
/// `Option<Node>` and `Vec<Node>` (`None` / empty lists are skipped).
pub trait IntoNodes {
    fn into_nodes(self) -> Vec<Node>;
}

impl IntoNodes for Node {
    fn into_nodes(self) -> Vec<Node> {
        vec![self]
    }
}

impl IntoNodes for Option<Node> {
    fn into_nodes(self) -> Vec<Node> {
        self.into_iter().collect()
    }
}

impl IntoNodes for Vec<Node> {
    fn into_nodes(self) -> Vec<Node> {
        self
    }
}

/// Builds a `Vec<Node>` from `Node`, `Option<Node>` and `Vec<Node>` values.
///
/// ```ignore
/// ui::column(ui::BoxOptions {
///     children: nodes![ui::text("a", Default::default()), self.button(), self.extra],
///     ..Default::default()
/// })
/// ```
#[macro_export]
macro_rules! nodes {
    () => {
        ::std::vec::Vec::<$crate::ui::Node>::new()
    };
    ($($node:expr),+ $(,)?) => {{
        let mut nodes: ::std::vec::Vec<$crate::ui::Node> = ::std::vec::Vec::new();
        $(
            nodes.extend($crate::ui::IntoNodes::into_nodes($node));
        )+
        nodes
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::builders::text;
    use crate::ui::options::TextOptions;

    fn keyed(key: &'static str) -> Node {
        text(
            key,
            TextOptions {
                key: Some(key.into()),
                ..Default::default()
            },
        )
    }

    #[test]
    fn element_defaults_match_the_zig_struct() {
        let element = Element::default();
        assert_eq!(element.kind, NodeKind::Root);
        assert!(element.enabled);
        assert!(element.focus_ring);
        assert_eq!(element.icon_size, 18.0);
        assert_eq!(element.button_size, ButtonSize::Middle);
        assert_eq!(element.toggle_kind, ToggleKind::Checkbox);
        assert_eq!(element.image_fit, ImageFit::Cover);
        assert_eq!(element.rich_line_height, 2.0);
        assert_eq!(element.layout.shrink, 1.0, "FlexStyle default shrink");
        assert!(!element.layout.absolute);
        assert_eq!(element.content, "");
        assert!(element.children.is_empty());
        assert!(element.options.is_empty());
        assert!(element.rich_spans.is_empty());
        assert!(element.key.is_none());
    }

    #[test]
    fn nodes_macro_flattens_nodes_options_and_lists() {
        let nodes = nodes![
            keyed("a"),
            None::<Node>,
            Some(keyed("b")),
            vec![keyed("c"), keyed("d")]
        ];
        let keys: Vec<&str> = nodes
            .iter()
            .map(|node| node.element().key.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(keys, ["a", "b", "c", "d"]);
        assert!(nodes![].is_empty());
        let single = nodes![keyed("only"),];
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn into_nodes_accepts_the_three_shapes() {
        assert_eq!(IntoNodes::into_nodes(keyed("x")).len(), 1);
        assert!(IntoNodes::into_nodes(None::<Node>).is_empty());
        assert_eq!(IntoNodes::into_nodes(vec![keyed("x"), keyed("y")]).len(), 2);
    }

    #[test]
    fn append_children_keeps_the_original_node() {
        let node = keyed("root");
        let same = append_children(node.clone(), &[]);
        assert!(node.ptr_eq(&same), "no copy when nothing to add");
        let appended = append_children(node.clone(), &[keyed("extra")]);
        assert_eq!(appended.element().children.len(), 1);
        assert!(
            node.element().children.is_empty(),
            "the original is unchanged"
        );
    }

    #[test]
    fn with_key_replaces_the_key_of_a_copy() {
        let node = keyed("old");
        let keyed_node = with_key(node.clone(), "new");
        assert_eq!(keyed_node.element().key.as_deref(), Some("new"));
        assert_eq!(node.element().key.as_deref(), Some("old"));
    }
}
