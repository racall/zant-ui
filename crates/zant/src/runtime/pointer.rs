//! Port of `src/ui_runtime.zig:1808-2215`: pointer dispatch.
//!
//! Hover / press state of buttons, toggles and inputs, the container hover look,
//! pointer-area capture (`ui::pointer_area`: down / drag / up / hover / leave),
//! cursor changes, multi-click counting, the click into an input (caret
//! placement), and the press that starts a text selection (see `selection`).
//!
//! Zig called `components/select.zig:optionAt` for the open select popup; that
//! component is not ported yet (WP-2.2), so while a select is open the runtime
//! only keeps its open / close bookkeeping.
#![forbid(unsafe_code)]

use crate::components::button;
use crate::core::events::PointerEvent;
use crate::core::node::{ButtonType, NodeData, NodeId, NodeKind, RetainedNode};
use crate::platform::platform::Cursor;
use crate::ui::actions::{PointerInfo, PointerPhase};

use super::Runtime;

impl Runtime {
    /// The link of the rich-text span under a window point (Zig `linkAt`).
    pub(crate) fn link_at(&mut self, id: NodeId, x: f32, y: f32) -> Option<String> {
        let bounds = self.tree.get(id).bounds;
        let NodeData::RichText(data) = &mut self.tree.get_mut(id).data else {
            return None;
        };
        data.link_action.as_ref()?;
        let _ = data.ensure_layout(bounds.width)?;
        let byte = {
            let layout = data.layout.as_ref()?;
            layout.index_at(x - bounds.x, y - bounds.y)
        };
        // indexAt returns the nearest caret; probe the glyph on both sides.
        if let Some(link) = data.link_at_byte(byte) {
            return Some(link.to_string());
        }
        if byte > 0 {
            return data.link_at_byte(byte - 1).map(str::to_string);
        }
        None
    }

    /// The pointer area handler of a container (`pointerAreaNode`).
    fn pointer_area_node(&self, id: NodeId) -> Option<crate::core::node::PointerArea> {
        let node = self.tree.get(id);
        if !node.alive {
            return None;
        }
        match &node.data {
            NodeData::Container(data) => data.pointer.clone(),
            _ => None,
        }
    }

    /// Zig `sendPointer`: calls a pointer area's handler with coordinates
    /// relative to it.
    pub(crate) fn send_pointer(
        &mut self,
        id: NodeId,
        phase: PointerPhase,
        p: PointerEvent,
        dx: f32,
        dy: f32,
    ) {
        let Some(area) = self.pointer_area_node(id) else {
            return;
        };
        let bounds = self.tree.get(id).bounds;
        let info = PointerInfo {
            phase,
            x: p.x - bounds.x,
            y: p.y - bounds.y,
            window_x: p.x,
            window_y: p.y,
            width: bounds.width,
            height: bounds.height,
            button: p.button,
            modifiers: p.modifiers,
            click_count: if matches!(phase, PointerPhase::Down | PointerPhase::Up) {
                self.click_count.max(1)
            } else {
                1
            },
            dx,
            dy,
        };
        let changed = {
            let mut cx = super::action_cx(&mut self.app);
            area.action.call(&mut cx, info)
        };
        if changed {
            self.needs_build = true;
        }
    }

    /// Hover / leave for pointer areas under the pointer (no button held).
    fn update_pointer_hover(&mut self, target: Option<NodeId>, p: PointerEvent) {
        let area = target.and_then(|id| self.pointer_area_node(id).map(|_| id));
        if let Some(previous) = self.pointer_hover_area
            && area != Some(previous)
        {
            self.pointer_hover_area = None;
            self.send_pointer(previous, PointerPhase::Leave, p, 0.0, 0.0);
        }
        if let Some(id) = area {
            self.pointer_hover_area = Some(id);
            self.send_pointer(id, PointerPhase::Hover, p, 0.0, 0.0);
        }
    }

    /// Ends a drag that lost its pointer (window deactivated, area removed).
    pub(crate) fn release_pointer_capture(&mut self, p: PointerEvent) {
        let Some(id) = self.pointer_capture else {
            return;
        };
        self.pointer_capture = None;
        self.send_pointer(id, PointerPhase::Up, p, 0.0, 0.0);
    }

    /// Zig `handlePointerMove`.
    pub(crate) fn handle_pointer_move(&mut self, p: PointerEvent) {
        if self.sel_dragging {
            self.drag_text_selection(p);
            self.set_cursor(Cursor::Ibeam);
            return;
        }
        if let Some(id) = self.pointer_capture {
            // dragging: the capturing area gets every move, even outside it
            self.send_pointer(id, PointerPhase::Drag, p, 0.0, 0.0);
            self.update_cursor(Some(id));
            return;
        }
        if let Some(select_id) = self.open_select {
            let node = self.tree.get(select_id);
            if matches!(node.data, NodeData::Select(_)) {
                // TODO(WP-2.2): components::select::option_at hover highlight.
                let open = matches!(&node.data, NodeData::Select(data) if data.open);
                self.set_cursor(if open {
                    Cursor::PointingHand
                } else {
                    Cursor::Arrow
                });
                return;
            }
        }
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        self.update_pointer_hover(target, p);
        let hover_box = self.hover_box_for(target, p.x, p.y);
        self.set_hover_box(hover_box);
        if let Some(id) = self.pressed_button {
            let inside = self.tree.get(id).bounds.contains(p.x, p.y);
            let changed = {
                let node = self.tree.get_mut(id);
                match &mut node.data {
                    NodeData::Button(data) => {
                        if data.pressed != inside {
                            data.pressed = inside;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            };
            if changed {
                self.tree.poke(id);
            }
        }
        if let Some(id) = self.pressed_toggle {
            let inside = self.tree.get(id).bounds.contains(p.x, p.y);
            let changed = {
                let node = self.tree.get_mut(id);
                match &mut node.data {
                    NodeData::Toggle(data) => {
                        if data.pressed != inside {
                            data.pressed = inside;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            };
            if changed {
                self.tree.poke(id);
            }
        }
        if let Some(id) = self.drag_input {
            self.set_input_cursor(id, p.x, p.y, true);
        }
        self.update_cursor_at(target, Some([p.x, p.y]));
    }

    /// Zig `setHover`: moves the hover state of the widget under the pointer and
    /// arms its tooltip timer.
    pub(crate) fn set_hover(&mut self, target: Option<NodeId>) {
        if self.hovered == target {
            return;
        }
        self.clear_tooltip();
        if let Some(previous) = self.hovered {
            let node = self.tree.get_mut(previous);
            match &mut node.data {
                NodeData::Button(data) => data.hovered = false,
                NodeData::Input(data) => data.hovered = false,
                NodeData::Toggle(data) => data.hovered = false,
                NodeData::Select(data) => data.hovered = false,
                _ => {}
            }
            self.tree.poke(previous);
        }
        self.hovered = target;
        if let Some(id) = target {
            let has_tooltip = self.tree.get(id).tooltip.is_some();
            if has_tooltip {
                self.tooltip_target = Some(id);
                self.tooltip_show_at = self.now_ms + 450;
            }
            let node = self.tree.get_mut(id);
            match &mut node.data {
                NodeData::Button(data) => data.hovered = node.enabled,
                NodeData::Input(data) => data.hovered = node.enabled,
                NodeData::Toggle(data) => data.hovered = node.enabled,
                NodeData::Select(data) => data.hovered = node.enabled,
                _ => {}
            }
            self.tree.poke(id);
        }
    }

    /// Zig `clearPressed`: releases the pressed button / toggle without
    /// activating it.
    pub(crate) fn clear_pressed(&mut self) {
        if let Some(id) = self.pressed_button {
            if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                data.pressed = false;
            }
            self.tree.poke(id);
        }
        if let Some(id) = self.pressed_toggle {
            if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                data.pressed = false;
            }
            self.tree.poke(id);
        }
        self.pressed_button = None;
        self.pressed_toggle = None;
    }

    /// Sets the platform cursor of this runtime's window.
    fn set_cursor(&self, cursor: Cursor) {
        if let Some(window) = &self.window {
            window.set_cursor(cursor);
        }
    }

    pub(crate) fn update_cursor(&mut self, target: Option<NodeId>) {
        self.update_cursor_at(target, None);
    }

    /// Zig `updateCursorAt`: the cursor of the node under the pointer
    /// (`point` lets containers / rich text probe text and links).
    fn update_cursor_at(&mut self, target: Option<NodeId>, point: Option<[f32; 2]>) {
        let cursor = match target {
            Some(id) => match self.tree.get(id).kind() {
                NodeKind::Input => Cursor::Ibeam,
                NodeKind::Button => {
                    let button_type = match &self.tree.get(id).data {
                        NodeData::Button(data) => data.button_type,
                        _ => ButtonType::Default,
                    };
                    match button_type {
                        ButtonType::Plain | ButtonType::Caption | ButtonType::CaptionClose => {
                            Cursor::Arrow
                        }
                        _ => Cursor::PointingHand,
                    }
                }
                NodeKind::Toggle => Cursor::PointingHand,
                NodeKind::Container => {
                    let custom = match &self.tree.get(id).data {
                        NodeData::Container(data) => data.cursor,
                        _ => None,
                    };
                    custom.unwrap_or_else(|| self.default_cursor(point))
                }
                NodeKind::RichText => match point {
                    Some(point) => {
                        if self.link_at(id, point[0], point[1]).is_some() {
                            Cursor::PointingHand
                        } else {
                            Cursor::Arrow
                        }
                    }
                    None => Cursor::Arrow,
                },
                _ => Cursor::Arrow,
            },
            None => self.default_cursor(point),
        };
        self.set_cursor(cursor);
    }

    fn default_cursor(&self, point: Option<[f32; 2]>) -> Cursor {
        match point {
            Some(point) if self.selectable_text_at(point[0], point[1]) => Cursor::Ibeam,
            _ => Cursor::Arrow,
        }
    }

    /// Zig `handlePointerDown`.
    pub(crate) fn handle_pointer_down(&mut self, p: PointerEvent) {
        if let Some(select_id) = self.open_select {
            let node = self.tree.get(select_id);
            if matches!(node.data, NodeData::Select(_)) {
                // TODO(WP-2.2): components::select::option_at selects an item.
                let inside = node.bounds.contains(p.x, p.y);
                if inside {
                    self.close_select();
                    return;
                }
            }
            self.close_select();
            return;
        }
        let target = self.tree.hit_test(p.x, p.y);
        self.set_hover(target);
        if self.begin_text_selection(target, p) {
            return;
        }
        let Some(id) = target else {
            self.focus_node(None);
            return;
        };
        let (kind, enabled, focusable) = {
            let node = self.tree.get(id);
            (node.kind(), node.enabled, node.focusable)
        };
        match kind {
            NodeKind::Button => {
                if focusable {
                    self.focus_node(Some(id));
                }
                if !enabled {
                    return;
                }
                if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = true;
                    data.hovered = true;
                }
                self.pressed_button = Some(id);
                self.tree.poke(id);
            }
            NodeKind::Toggle => {
                self.focus_node(Some(id));
                if !enabled {
                    return;
                }
                if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = true;
                    data.hovered = true;
                }
                self.pressed_toggle = Some(id);
                self.tree.poke(id);
            }
            NodeKind::Input => {
                if !enabled {
                    return;
                }
                self.focus_node(Some(id));
                self.drag_input = Some(id);
                self.set_input_cursor(id, p.x, p.y, p.modifiers.shift);
            }
            NodeKind::RichText => {
                if self.link_at(id, p.x, p.y).is_some() {
                    self.pressed_link = Some(id);
                }
            }
            NodeKind::Select => {
                if !enabled {
                    return;
                }
                self.focus_node(Some(id));
                if let NodeData::Select(data) = &mut self.tree.get_mut(id).data {
                    data.open = true;
                    data.hovered_option = None;
                }
                self.open_select = Some(id);
                self.tree.poke(id);
            }
            NodeKind::Container => {
                let (has_pointer, clicks) = match &self.tree.get(id).data {
                    NodeData::Container(data) => (
                        data.pointer.is_some(),
                        data.pointer.as_ref().is_some_and(|area| area.clicks),
                    ),
                    _ => (false, false),
                };
                if has_pointer && clicks && enabled {
                    self.focus_node(if focusable { Some(id) } else { None });
                    self.count_click(p);
                    self.pointer_capture = Some(id);
                    self.send_pointer(id, PointerPhase::Down, p, 0.0, 0.0);
                } else {
                    // blocking panels (block_pointer) still blur inputs; a
                    // scroll-only area (a list) takes the focus for its keys
                    self.focus_node(if has_pointer && focusable {
                        Some(id)
                    } else {
                        None
                    });
                }
            }
            _ => {}
        }
        self.update_cursor(Some(id));
    }

    /// Zig `countClick`: double / triple click detection.
    pub(crate) fn count_click(&mut self, p: PointerEvent) {
        let near = (p.x - self.last_click_x).abs() <= 4.0 && (p.y - self.last_click_y).abs() <= 4.0;
        if near && self.now_ms - self.last_click_ms <= 500 && self.click_count > 0 {
            self.click_count = if self.click_count >= 3 {
                1
            } else {
                self.click_count + 1
            };
        } else {
            self.click_count = 1;
        }
        self.last_click_ms = self.now_ms;
        self.last_click_x = p.x;
        self.last_click_y = p.y;
    }

    /// Zig `handlePointerUp`: activates buttons / toggles / links, finishes a
    /// pointer-area drag and a text selection.
    pub(crate) fn handle_pointer_up(&mut self, p: PointerEvent) {
        self.drag_input = None;
        if self.sel_dragging {
            self.sel_dragging = false;
            // a plain click selects nothing
            if self.sel_mode == super::SelMode::Char
                && let Some(anchor) = self.sel_anchor
                && let Some(focus) = self.sel_focus
                && anchor.node == focus.node
                && anchor.byte == focus.byte
            {
                self.clear_text_selection();
            }
        }
        // a drag is not a click: the next press starts a new click count
        if (p.x - self.last_click_x).abs() > 4.0 || (p.y - self.last_click_y).abs() > 4.0 {
            self.click_count = 0;
        }
        if let Some(id) = self.pointer_capture {
            self.pointer_capture = None;
            self.send_pointer(id, PointerPhase::Up, p, 0.0, 0.0);
        }
        if let Some(id) = self.pressed_button {
            self.pressed_button = None;
            let (alive, inside, enabled) = {
                let node = self.tree.get(id);
                (node.alive, node.bounds.contains(p.x, p.y), node.enabled)
            };
            if alive {
                if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = false;
                }
                self.tree.poke(id);
                if inside && enabled {
                    let now = self.now_ms;
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        button::start_wave(data, now);
                    }
                    // copy the callbacks first: an action may change the UI
                    // (or run a modal dialog), after which the node must not be
                    // read again
                    let (index_action, index, action) = {
                        let node = self.tree.get(id);
                        match &node.data {
                            NodeData::Button(data) => {
                                (data.index_action.clone(), data.index, data.action.clone())
                            }
                            _ => (None, 0, None),
                        }
                    };
                    let mut cx = super::action_cx(&mut self.app);
                    if let Some(index_action) = index_action {
                        index_action.call(&mut cx, index);
                    } else if let Some(action) = action {
                        action.call(&mut cx);
                    }
                }
            }
        }
        if let Some(id) = self.pressed_toggle {
            let (alive, inside, enabled) = {
                let node = self.tree.get(id);
                (node.alive, node.bounds.contains(p.x, p.y), node.enabled)
            };
            if alive {
                let (kind, checked) = {
                    let node = self.tree.get(id);
                    match &node.data {
                        NodeData::Toggle(data) => (data.kind, data.checked),
                        _ => (crate::core::node::ToggleKind::Checkbox, false),
                    }
                };
                if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                    data.pressed = false;
                }
                if inside && enabled {
                    if kind == crate::core::node::ToggleKind::Radio {
                        if !checked {
                            self.tree.toggle_set(id, true);
                        }
                    } else {
                        self.tree.toggle_flip(id);
                    }
                }
                self.tree.poke(id);
            }
            self.pressed_toggle = None;
        }
        if let Some(id) = self.pressed_link {
            self.pressed_link = None;
            let inside = self.tree.get(id).bounds.contains(p.x, p.y);
            if inside && let Some(link) = self.link_at(id, p.x, p.y) {
                let action = match &self.tree.get(id).data {
                    NodeData::RichText(data) => data.link_action.clone(),
                    _ => None,
                };
                if let Some(action) = action {
                    // The span memory may be replaced by the callback's
                    // rebuild; hand the callback a stable copy (owned String).
                    let mut cx = super::action_cx(&mut self.app);
                    action.call(&mut cx, &link);
                }
            }
        }
        self.update_cursor_at(self.tree.hit_test(p.x, p.y), Some([p.x, p.y]));
    }

    /// Zig `setInputCursor`: places the input caret at a window point (a click
    /// or a drag inside the input).
    fn set_input_cursor(&mut self, id: NodeId, x: f32, y: f32, extend: bool) {
        let bounds = self.tree.get(id).bounds;
        let NodeData::Input(data) = &mut self.tree.get_mut(id).data else {
            return;
        };
        if data.has_composition {
            return;
        }
        let multiline = data.multiline;
        let scroll_x = data.scroll_x;
        let scroll_y = data.scroll_y;
        let text_len = data.text.len();
        let text_left = super::ime::input_text_left(data, bounds);
        let Some(byte) = ({
            let Some(layout) = super::ime::ensure_input_layout(data) else {
                return;
            };
            let origin_x = text_left - scroll_x;
            let origin_y = if multiline {
                bounds.y + 8.0 - scroll_y
            } else {
                bounds.y + (bounds.height - layout.metrics().height) * 0.5
            };
            Some(layout.index_at(x - origin_x, y - origin_y))
        }) else {
            return;
        };
        data.cursor = byte.min(text_len);
        if !extend {
            data.anchor = data.cursor;
        }
        self.tree.mark_dirty(id, false, true);
    }

    /// A text node of a selectable container under (x, y) (I-beam cursor).
    fn selectable_text_at(&self, x: f32, y: f32) -> bool {
        let Some(root) = crate::core::text_select::root_at(&self.tree, x, y) else {
            return false;
        };
        self.text_node_under(self.tree.get(root), x, y)
    }

    fn text_node_under(&self, node: &RetainedNode, x: f32, y: f32) -> bool {
        if !node.in_flow() || !node.bounds.contains(x, y) {
            return false;
        }
        if crate::core::text_select::is_text(node) {
            return true;
        }
        node.children
            .iter()
            .any(|child| self.text_node_under(self.tree.get(*child), x, y))
    }

    /// Zig `hoverBoxFor`: the container whose hover look applies: the nearest
    /// one around the widget under the pointer, or the innermost one containing
    /// the point.
    fn hover_box_for(&self, target: Option<NodeId>, x: f32, y: f32) -> Option<NodeId> {
        let Some(mut current) = target else {
            return self.tree.hover_box_at(x, y);
        };
        loop {
            let node = self.tree.get(current);
            if let NodeData::Container(data) = &node.data
                && data.hover_visual.is_some()
            {
                return Some(current);
            }
            current = node.parent?;
        }
    }

    /// Zig `setHoverBox`: moves the container hover look (paint-only, animated).
    pub(crate) fn set_hover_box(&mut self, id: Option<NodeId>) {
        if self.hover_box == id {
            return;
        }
        if let Some(old) = self.hover_box
            && old < self.tree.nodes.len() as NodeId
        {
            if let NodeData::Container(data) = &mut self.tree.get_mut(old).data {
                data.hovered = false;
            }
            self.tree.poke(old);
        }
        self.hover_box = id;
        if let Some(new) = id {
            if let NodeData::Container(data) = &mut self.tree.get_mut(new).data {
                data.hovered = true;
            }
            self.tree.poke(new);
        }
    }

    /// Zig `isAncestor`: `ancestor` is `id` or one of its ancestors.
    pub(crate) fn is_ancestor(&self, ancestor: NodeId, id: NodeId) -> bool {
        let mut current = Some(id);
        while let Some(node) = current {
            if node == ancestor {
                return true;
            }
            current = self.tree.get(node).parent;
        }
        false
    }
}
