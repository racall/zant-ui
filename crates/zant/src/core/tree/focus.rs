//! Focus (focused node, Tab order,
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

    /// The topmost (last painted) dismiss layer that Esc dismisses, if any.
    pub fn escape_target(&self) -> Option<NodeId> {
        self.topmost(self.root, &|node| node.visual.escape_dismiss)
    }

    fn topmost(
        &self,
        id: NodeId,
        wanted: &dyn Fn(&crate::core::node::RetainedNode) -> bool,
    ) -> Option<NodeId> {
        let node = &self.nodes[id as usize];
        if !node.in_flow() {
            return None;
        }
        let mut i = node.children.len();
        while i > 0 {
            i -= 1;
            if let Some(hit) = self.topmost(node.children[i], wanted) {
                return Some(hit);
            }
        }
        wanted(node).then_some(id)
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

#[cfg(test)]
mod tests {
    use crate::core::geometry::Size;
    use crate::core::node::{FlexStyle, NodeData};
    use crate::core::tree::Tree;
    use crate::theme::tokens::ant_light;

    fn focusable_tree() -> (Tree, [u32; 3]) {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let a = tree.add_button(
            Some(root),
            "a",
            Default::default(),
            Default::default(),
            None,
        );
        let b = tree.add_button(
            Some(root),
            "b",
            Default::default(),
            Default::default(),
            None,
        );
        let toggle = tree.add_toggle(Some(root), "t", Default::default(), false);
        tree.layout(Size {
            width: 200.0,
            height: 200.0,
        });
        (tree, [a, b, toggle])
    }

    #[test]
    fn tab_focus_cycles_through_focusable_nodes() {
        let (mut tree, [a, b, toggle]) = focusable_tree();
        assert_eq!(tree.focused, None);
        tree.tab_focus(false);
        assert_eq!(tree.focused, Some(a));
        tree.tab_focus(false);
        assert_eq!(tree.focused, Some(b));
        tree.tab_focus(false);
        assert_eq!(tree.focused, Some(toggle));
        tree.tab_focus(false);
        assert_eq!(tree.focused, Some(a), "wraps around");
        tree.tab_focus(true);
        assert_eq!(tree.focused, Some(toggle), "backwards wraps");
        // only the focused node reports focused state
        let focused_count = tree
            .nodes
            .iter()
            .filter(|node| match &node.data {
                NodeData::Button(data) => data.focused,
                NodeData::Toggle(data) => data.focused,
                _ => false,
            })
            .count();
        assert_eq!(focused_count, 1);
    }

    #[test]
    fn set_focus_ignores_unfocusable_targets() {
        let (mut tree, [a, _b, _toggle]) = focusable_tree();
        tree.set_focus(Some(tree.root));
        assert_eq!(tree.focused, None, "the root is not focusable");
        tree.set_focus(Some(a));
        assert_eq!(tree.focused, Some(a));
        tree.set_focus(None);
        assert_eq!(tree.focused, None);
        assert!(tree.focused_node().is_none());
    }

    #[test]
    fn focus_scope_limits_tab_order() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let outside = tree.add_button(
            Some(root),
            "out",
            Default::default(),
            Default::default(),
            None,
        );
        let panel = tree.add_container(Some(root), FlexStyle::default());
        tree.get_mut(panel).visual.focus_scope = true;
        let inside = tree.add_button(
            Some(panel),
            "in",
            Default::default(),
            Default::default(),
            None,
        );
        tree.layout(Size {
            width: 200.0,
            height: 200.0,
        });
        tree.tab_focus(false);
        assert_eq!(tree.focused, Some(inside), "the topmost focus scope wins");
        tree.tab_focus(false);
        assert_eq!(tree.focused, Some(inside), "Tab stays inside the scope");
        assert_ne!(tree.focused, Some(outside));
    }
}
