//! Port of `src/core/tree.zig` — WP-1.3: focus (focused node, Tab order,
//! focus scopes).
#![forbid(unsafe_code)]

use crate::core::node::{NodeData, NodeId, RetainedNode};
use crate::core::tree::Tree;

impl Tree {
    pub fn focused_node(&self) -> Option<&RetainedNode> {
        let id = self.focused?;
        let node = &self.nodes[id as usize];
        if node.enabled && node.in_flow() && node.focusable {
            Some(node)
        } else {
            None
        }
    }

    pub fn set_focus(&mut self, id: Option<NodeId>) {
        for index in 0..self.nodes.len() {
            if !self.nodes[index].alive {
                continue;
            }
            let node_id = index as NodeId;
            let mut poke = false;
            match &mut self.nodes[index].data {
                NodeData::Button(data) => {
                    data.focused = false;
                    data.focus_visible = false;
                }
                NodeData::Input(data) => {
                    if data.focused {
                        poke = true;
                    }
                    data.focused = false;
                    data.blink_on = true;
                    data.next_blink_ms = 0;
                }
                NodeData::Toggle(data) => {
                    data.focused = false;
                    data.focus_visible = false;
                }
                NodeData::Select(data) => {
                    if data.focused || data.open {
                        poke = true;
                    }
                    data.focused = false;
                    data.open = false;
                    data.hovered_option = None;
                }
                _ => {}
            }
            if poke {
                self.poke(node_id);
            }
        }
        self.focused = None;
        let Some(target_id) = id else {
            return;
        };
        {
            let target = &self.nodes[target_id as usize];
            if !target.enabled || !target.in_flow() || !target.focusable {
                return;
            }
        }
        self.focused = Some(target_id);
        match &mut self.nodes[target_id as usize].data {
            NodeData::Button(data) => {
                data.focused = true;
                data.focus_visible = self.keyboard_modality;
            }
            NodeData::Input(data) => {
                data.focused = true;
                data.blink_on = true;
                data.next_blink_ms = 0;
            }
            NodeData::Toggle(data) => {
                data.focused = true;
                data.focus_visible = self.keyboard_modality;
            }
            NodeData::Select(data) => data.focused = true,
            _ => {}
        }
        self.poke(target_id);
    }

    fn collect_focusables(&self, id: NodeId, out: &mut Vec<NodeId>) {
        let node = &self.nodes[id as usize];
        if !node.in_flow() {
            return;
        }
        if node.focusable && node.enabled {
            out.push(node.id);
        }
        for &child in &node.children {
            self.collect_focusables(child, out);
        }
    }

    /// The topmost (last painted) focus scope shown, if any.
    fn active_focus_scope(&self, id: NodeId) -> Option<NodeId> {
        let node = &self.nodes[id as usize];
        if !node.in_flow() {
            return None;
        }
        let mut i = node.children.len();
        while i > 0 {
            i -= 1;
            if let Some(hit) = self.active_focus_scope(node.children[i]) {
                return Some(hit);
            }
        }
        if node.visual.focus_scope {
            Some(id)
        } else {
            None
        }
    }

    pub fn tab_focus(&mut self, backwards: bool) {
        // inside an open dialog Tab cycles through the dialog only
        let scope = self.active_focus_scope(self.root).unwrap_or(self.root);
        let mut focusables: Vec<NodeId> = Vec::new();
        self.collect_focusables(scope, &mut focusables);
        if focusables.is_empty() {
            return;
        }
        let mut index = 0usize;
        let mut found = false;
        if let Some(focused_id) = self.focused {
            for (i, &id) in focusables.iter().enumerate() {
                if id == focused_id {
                    index = i;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            // focus outside the scope (or none): enter at its first / last
            let entry = if backwards {
                focusables[focusables.len() - 1]
            } else {
                focusables[0]
            };
            self.set_focus(Some(entry));
            return;
        }
        if self.focused.is_some() {
            if backwards {
                index = if index == 0 {
                    focusables.len() - 1
                } else {
                    index - 1
                };
            } else {
                index = (index + 1) % focusables.len();
            }
        } else if backwards {
            index = focusables.len() - 1;
        }
        self.set_focus(Some(focusables[index]));
    }
}
