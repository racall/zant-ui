//! `RetainedNode`: a node of the retained tree, with its kind data, layout, animation and actions.

use super::*;

/// What a retained node paints and lays out, by [`NodeKind`].
///
/// The variants deliberately differ a lot in size; boxing one would add an
/// allocation per node of that kind.
#[allow(clippy::large_enum_variant)]
pub enum NodeData {
    Root,
    Container(ContainerData),
    Text(TextData),
    Button(ButtonData),
    Input(InputData),
    Toggle(ToggleData),
    Image(ImageData),
    Select(SelectData),
    RichText(RichTextData),
    Vector(VectorData),
}

impl NodeData {
    pub fn kind(&self) -> NodeKind {
        match self {
            NodeData::Root => NodeKind::Root,
            NodeData::Container(_) => NodeKind::Container,
            NodeData::Text(_) => NodeKind::Text,
            NodeData::Button(_) => NodeKind::Button,
            NodeData::Input(_) => NodeKind::Input,
            NodeData::Toggle(_) => NodeKind::Toggle,
            NodeData::Image(_) => NodeKind::Image,
            NodeData::Select(_) => NodeKind::Select,
            NodeData::RichText(_) => NodeKind::RichText,
            NodeData::Vector(_) => NodeKind::Vector,
        }
    }
}

/// One slot of the retained tree.
pub struct RetainedNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub style: FlexStyle,
    pub visual: Visual,
    pub key: Option<Str>,
    pub tooltip: Option<Str>,
    /// Hash of the last memoized spec (see `ui::memo`); `None` when not memoized.
    pub memo_hash: Option<u64>,
    /// `ui::local_state` ids the memo build touched (kept alive when skipped).
    pub memo_states: Vec<u64>,
    pub bounds: Rect,
    pub dirty: DirtyFlags,
    /// False for a freed slot (tombstone) waiting in `Tree::free_ids`.
    pub alive: bool,
    /// Listed in `Tree::animating`.
    pub anim_queued: bool,
    pub motion: MotionStyle,
    pub anim: Option<NodeAnim>,
    /// Playing its exit transition: painted, but out of layout, hit testing and
    /// focus; freed when the transition ends.
    pub exiting: bool,
    /// Selected byte range of a text / rich text node (text selection).
    pub text_sel: Option<[u32; 2]>,
    pub visible: bool,
    pub enabled: bool,
    pub focusable: bool,
    pub data: NodeData,
}

impl RetainedNode {
    /// A live node slot with default field values.
    pub fn new(id: NodeId, data: NodeData) -> RetainedNode {
        RetainedNode {
            id,
            parent: None,
            children: Vec::new(),
            style: FlexStyle::default(),
            visual: Visual::default(),
            key: None,
            tooltip: None,
            memo_hash: None,
            memo_states: Vec::new(),
            bounds: Rect::default(),
            dirty: DirtyFlags::default(),
            alive: true,
            anim_queued: false,
            motion: MotionStyle::default(),
            anim: None,
            exiting: false,
            text_sel: None,
            visible: true,
            enabled: true,
            focusable: false,
            data,
        }
    }

    /// A freed slot: the whole node is reset to its defaults with
    /// `alive = false` and a container payload.
    pub fn tombstone(id: NodeId) -> RetainedNode {
        let mut node = RetainedNode::new(id, NodeData::Container(ContainerData::default()));
        node.visible = false;
        node.alive = false;
        node
    }

    pub fn kind(&self) -> NodeKind {
        self.data.kind()
    }

    /// Takes part in layout and input (visible, not exiting, not an empty
    /// placeholder).
    pub fn in_flow(&self) -> bool {
        self.visible && !self.exiting && !self.empty_placeholder()
    }

    /// An empty `Root` element inside another node (`ui::Node::default()`,
    /// e.g. a TSX component that returned `null`): like React's `null` it
    /// renders nothing and takes no space, not even a `gap`.
    pub(super) fn empty_placeholder(&self) -> bool {
        self.parent.is_some() && matches!(self.data, NodeData::Root) && self.children.is_empty()
    }

    pub fn is_interactive(&self) -> bool {
        if !self.visible || !self.enabled {
            return false;
        }
        match &self.data {
            NodeData::Button(_)
            | NodeData::Input(_)
            | NodeData::Toggle(_)
            | NodeData::Select(_) => true,
            NodeData::RichText(data) => data.link_action.is_some() && data.has_links(),
            NodeData::Container(data) => data.pointer.is_some(),
            _ => false,
        }
    }

    pub fn mark(&mut self, layout: bool, paint: bool) {
        if layout {
            self.dirty.layout = true;
        }
        if paint || layout {
            self.dirty.paint = true;
        }
    }

    pub fn container(&self) -> Option<&ContainerData> {
        match &self.data {
            NodeData::Container(data) => Some(data),
            _ => None,
        }
    }

    pub fn container_mut(&mut self) -> Option<&mut ContainerData> {
        match &mut self.data {
            NodeData::Container(data) => Some(data),
            _ => None,
        }
    }

    pub fn text_mut(&mut self) -> Option<&mut TextData> {
        match &mut self.data {
            NodeData::Text(data) => Some(data),
            _ => None,
        }
    }

    pub fn rich_text_mut(&mut self) -> Option<&mut RichTextData> {
        match &mut self.data {
            NodeData::RichText(data) => Some(data),
            _ => None,
        }
    }

    pub fn button_mut(&mut self) -> Option<&mut ButtonData> {
        match &mut self.data {
            NodeData::Button(data) => Some(data),
            _ => None,
        }
    }

    pub fn input_mut(&mut self) -> Option<&mut InputData> {
        match &mut self.data {
            NodeData::Input(data) => Some(data),
            _ => None,
        }
    }

    pub fn toggle_mut(&mut self) -> Option<&mut ToggleData> {
        match &mut self.data {
            NodeData::Toggle(data) => Some(data),
            _ => None,
        }
    }
}
