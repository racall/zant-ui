//! Text selection inside `selectable` containers.
//!
//! The hit testing / copying helpers are `core::text_select`; this
//! module owns the runtime state (anchor, focus, drag, double / triple click
//! word / block expansion), marks the selected byte ranges on the text nodes
//! (`RetainedNode::text_sel`) and copies through the platform clipboard.
#![forbid(unsafe_code)]

use crate::core::events::{PointerButton, PointerEvent};
use crate::core::node::{NodeData, NodeId, NodeKind};
use crate::core::text_select::{self, Point};
use crate::platform::platform;

use super::{Runtime, SelMode};

impl Runtime {
    /// Drops the marks and the selection state.
    pub fn clear_text_selection(&mut self) {
        for &id in &self.sel_marked {
            if (id as usize) < self.tree.nodes.len() {
                self.tree.get_mut(id).text_sel = None;
            }
        }
        if !self.sel_marked.is_empty() {
            self.tree.mark_paint_dirty();
        }
        self.sel_marked.clear();
        self.sel_root = None;
        self.sel_anchor = None;
        self.sel_focus = None;
        self.sel_dragging = false;
    }

    /// The selected text (empty when nothing is selected).
    pub fn selected_text(&mut self) -> String {
        let Some(root) = self.sel_root else {
            return String::new();
        };
        let (Some(anchor), Some(focus)) = (self.sel_anchor, self.sel_focus) else {
            return String::new();
        };
        let mut nodes: Vec<NodeId> = Vec::new();
        text_select::collect(&self.tree, root, &mut nodes);
        text_select::copy_text(&self.tree, &nodes, anchor, focus)
    }

    /// Marks the selected range on each text node
    /// (painted behind the text).
    fn apply_text_selection(&mut self) {
        for &id in &self.sel_marked {
            if (id as usize) < self.tree.nodes.len() {
                self.tree.get_mut(id).text_sel = None;
            }
        }
        self.sel_marked.clear();
        self.tree.mark_paint_dirty();
        let Some(root) = self.sel_root else {
            return;
        };
        let (Some(anchor), Some(focus)) = (self.sel_anchor, self.sel_focus) else {
            return;
        };
        let mut nodes: Vec<NodeId> = Vec::new();
        text_select::collect(&self.tree, root, &mut nodes);
        let Some(range) = text_select::ordered(&nodes, anchor, focus) else {
            return;
        };
        let mut index = range.si;
        while index <= range.ei {
            let node_id = nodes[index];
            let (from, to) = {
                let node = self.tree.get(node_id);
                let from = if index == range.si {
                    range.start.byte
                } else {
                    0
                };
                let to = if index == range.ei {
                    range.end.byte
                } else {
                    text_select::text_len(node)
                };
                (from, to)
            };
            if from < to {
                self.tree.get_mut(node_id).text_sel = Some([from as u32, to as u32]);
                self.sel_marked.push(node_id);
            }
            index += 1;
        }
    }

    /// The word / whole node a multi-click selection grows by.
    fn unit_at(&mut self, point: Point) -> [Point; 2] {
        match self.sel_mode {
            SelMode::Char => [point, point],
            SelMode::Word => {
                let word = text_select::word_at(self.tree.get(point.node), point.byte);
                [
                    Point {
                        node: point.node,
                        byte: word[0],
                    },
                    Point {
                        node: point.node,
                        byte: word[1],
                    },
                ]
            }
            SelMode::Block => {
                let len = text_select::text_len(self.tree.get(point.node));
                [
                    Point {
                        node: point.node,
                        byte: 0,
                    },
                    Point {
                        node: point.node,
                        byte: len,
                    },
                ]
            }
        }
    }

    /// Starts a text selection when the press lands in a `selectable`
    /// container on something that does not take clicks itself.
    pub(crate) fn begin_text_selection(&mut self, target: Option<NodeId>, p: PointerEvent) -> bool {
        if p.button != PointerButton::Left {
            return false;
        }
        if let Some(id) = target {
            match self.tree.get(id).kind() {
                NodeKind::RichText => {
                    if self.link_at(id, p.x, p.y).is_some() {
                        return false;
                    }
                }
                NodeKind::Container => {
                    let clicks = match &self.tree.get(id).data {
                        NodeData::Container(data) => {
                            data.pointer.as_ref().is_some_and(|area| area.clicks)
                        }
                        _ => false,
                    };
                    if clicks {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        let Some(root) = text_select::root_at(&self.tree, p.x, p.y) else {
            if target.is_none() {
                self.clear_text_selection();
            }
            return false;
        };
        // a blocking panel (modal, popover) over selectable text: only text
        // inside the panel itself starts a selection
        if let Some(id) = target
            && !self.is_ancestor(root, id)
            && !self.is_ancestor(id, root)
        {
            return false;
        }
        let mut nodes: Vec<NodeId> = Vec::new();
        text_select::collect(&self.tree, root, &mut nodes);
        let Some(point) = text_select::point_at(&mut self.tree, &nodes, p.x, p.y) else {
            return false;
        };
        self.focus_node(None);
        self.count_click(p);
        if p.modifiers.shift && self.sel_root == Some(root) && self.sel_anchor.is_some() {
            self.sel_focus = Some(point);
        } else {
            self.sel_root = Some(root);
            self.sel_mode = match self.click_count {
                0 | 1 => SelMode::Char,
                2 => SelMode::Word,
                _ => SelMode::Block,
            };
            self.sel_unit = self.unit_at(point);
            self.sel_anchor = Some(self.sel_unit[0]);
            self.sel_focus = Some(self.sel_unit[1]);
        }
        self.sel_dragging = true;
        self.apply_text_selection();
        true
    }

    /// Extends the selection (auto-scrolls the
    /// enclosing scroll container past its edges).
    pub(crate) fn drag_text_selection(&mut self, p: PointerEvent) {
        let Some(root) = self.sel_root else {
            return;
        };
        let mut nodes: Vec<NodeId> = Vec::new();
        text_select::collect(&self.tree, root, &mut nodes);
        // auto-scroll the enclosing scroll container past its edges
        let mut current = self.tree.get(root).parent;
        while let Some(pid) = current {
            let (is_scroll, top, bottom) = {
                let container = self.tree.get(pid);
                (
                    container.style.scroll,
                    container.bounds.y,
                    container.bounds.bottom(),
                )
            };
            current = self.tree.get(pid).parent;
            if !is_scroll {
                continue;
            }
            if p.y < top {
                self.tree.scroll_by(pid, (p.y - top) * 0.5);
            } else if p.y > bottom {
                self.tree.scroll_by(pid, (p.y - bottom) * 0.5);
            }
            break;
        }
        let Some(point) = text_select::point_at(&mut self.tree, &nodes, p.x, p.y) else {
            return;
        };
        if self.sel_mode == SelMode::Char {
            self.sel_focus = Some(point);
        } else {
            let unit = self.unit_at(point);
            let Some(first) = text_select::ordered(&nodes, unit[0], self.sel_unit[0]) else {
                return;
            };
            if first.start.node == unit[0].node
                && first.start.byte == unit[0].byte
                && !(unit[0].node == self.sel_unit[0].node && unit[0].byte == self.sel_unit[0].byte)
            {
                // dragging backwards: from the end of the first unit
                self.sel_anchor = Some(self.sel_unit[1]);
                self.sel_focus = Some(unit[0]);
            } else {
                self.sel_anchor = Some(self.sel_unit[0]);
                self.sel_focus = Some(unit[1]);
            }
        }
        self.apply_text_selection();
    }

    /// Selects every text node of `root`.
    pub(crate) fn select_all_text(&mut self, root: NodeId) {
        let mut nodes: Vec<NodeId> = Vec::new();
        text_select::collect(&self.tree, root, &mut nodes);
        if nodes.is_empty() {
            return;
        }
        let last = *nodes.last().expect("checked non-empty");
        self.sel_root = Some(root);
        self.sel_anchor = Some(Point {
            node: nodes[0],
            byte: 0,
        });
        self.sel_focus = Some(Point {
            node: last,
            byte: text_select::text_len(self.tree.get(last)),
        });
        self.apply_text_selection();
    }

    /// Copies the selected text to the clipboard.
    pub(crate) fn copy_text_selection(&mut self) -> bool {
        let value = self.selected_text();
        if value.is_empty() {
            return false;
        }
        platform::set_clipboard(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::events::{Event, Key, KeyEvent, Modifiers, PointerButton, PointerEvent};
    use crate::renderer::renderer::Renderer;
    use crate::ui::app::App;
    use crate::ui::builders::{column, text};
    use crate::ui::options::{BoxOptions, TextOptions};
    use crate::ui::{Node, RunOptions};

    struct Doc;

    impl App for Doc {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                // `ui::paragraph` is not needed: plain text nodes join
                // into a multi-node selection.
                selectable: true,
                width: Some(400.0),
                height: Some(300.0),
                children: crate::nodes![
                    text(
                        "Hello world",
                        TextOptions {
                            wrap: false,
                            ..Default::default()
                        }
                    ),
                    text(
                        "Second line",
                        TextOptions {
                            wrap: false,
                            ..Default::default()
                        }
                    ),
                ],
                ..Default::default()
            })
        }
    }

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 400.0, 300.0, 1.0);
        renderer.end();
    }

    fn pointer(x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            x,
            y,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
        }
    }

    fn text_bounds(
        runtime: &Runtime,
    ) -> (crate::core::geometry::Rect, crate::core::geometry::Rect) {
        let mut bounds = runtime
            .tree
            .nodes
            .iter()
            .filter(|node| node.alive && matches!(node.data, NodeData::Text(_)))
            .map(|node| node.bounds);
        (
            bounds.next().expect("first"),
            bounds.next().expect("second"),
        )
    }

    #[test]
    fn dragging_across_text_nodes_selects_and_copies_them() {
        let mut runtime = Runtime::init(Doc, RunOptions::default());
        let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let (first, second) = text_bounds(&runtime);
        let start = pointer(first.x + 1.0, first.y + first.height * 0.5);
        let end = pointer(
            second.x + second.width - 1.0,
            second.y + second.height * 0.5,
        );
        runtime.dispatch(Event::PointerDown(start));
        assert!(runtime.sel_dragging, "the press starts a selection");
        runtime.dispatch(Event::PointerMove(end));
        runtime.dispatch(Event::PointerUp(end));
        assert!(!runtime.sel_dragging);
        let selected = runtime.selected_text();
        assert!(selected.contains("Hello world"), "selected: {selected:?}");
        assert!(selected.contains("Second line"), "selected: {selected:?}");
        assert!(selected.contains('\n'), "stacked nodes join with a newline");
        // the marks are set on the text nodes so paint highlights them
        let marked: Vec<[u32; 2]> = runtime
            .tree
            .nodes
            .iter()
            .filter(|node| node.alive)
            .filter_map(|node| node.text_sel)
            .collect();
        assert_eq!(marked.len(), 2);
    }

    #[test]
    fn a_plain_click_selects_nothing() {
        let mut runtime = Runtime::init(Doc, RunOptions::default());
        let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
        frame(&mut runtime, &mut renderer);
        let (first, _) = text_bounds(&runtime);
        let point = pointer(first.x + 20.0, first.y + first.height * 0.5);
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
        assert!(runtime.selected_text().is_empty());
        assert!(runtime.sel_root.is_none());
    }

    #[test]
    fn select_all_and_escape_clear_the_selection() {
        let mut runtime = Runtime::init(Doc, RunOptions::default());
        let mut renderer = Renderer::create_raster(400, 300).expect("renderer");
        frame(&mut runtime, &mut renderer);
        // a selection must exist first: cmd/ctrl+A only applies to selectable
        // content when a selection root is set (a click in the text sets it)
        let (first, _) = text_bounds(&runtime);
        let point = pointer(first.x + 5.0, first.y + first.height * 0.5);
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
        runtime.select_all_text(runtime.tree.root);
        let all = runtime.selected_text();
        assert!(all.contains("Hello world") && all.contains("Second line"));
        assert_eq!(
            runtime.dispatch(Event::KeyDown(KeyEvent {
                key: Key::Escape,
                modifiers: Modifiers::default(),
                repeat: false,
            })),
            1
        );
        assert!(runtime.selected_text().is_empty());
        assert!(runtime.sel_anchor.is_none());
        assert!(
            runtime
                .tree
                .nodes
                .iter()
                .all(|node| node.text_sel.is_none() || !node.alive)
        );
    }
}
