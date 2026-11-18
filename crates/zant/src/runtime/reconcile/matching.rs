//! Reconciling a node's children: matching by key or position.

use super::*;

impl Runtime {
    /// Reconciles `specs` into `parent_id`'s children.
    /// Returns true when the layout has to run again.
    pub(crate) fn reconcile_children(&mut self, parent_id: NodeId, specs: &[Node]) -> bool {
        let old: Vec<NodeId> = self.tree.get(parent_id).children.clone();
        let old_len = old.len();
        let mut used = vec![false; old_len];
        let mut new_children: Vec<NodeId> = Vec::with_capacity(specs.len());
        let mut changed_layout = false;
        let mut changed_children = false;
        // walks `old` for unkeyed specs
        let mut cursor = 0usize;
        let mut keyed = KeyedLookup::default();

        for spec in specs {
            let found = self.find_match(&old, &used, &mut cursor, &mut keyed, spec);
            let Some(index) = found else {
                if let Some(id) = self.create_memo(spec.clone()) {
                    self.tree.get_mut(id).parent = Some(parent_id);
                    self.begin_enter(id);
                    new_children.push(id);
                    changed_layout = true;
                    changed_children = true;
                }
                continue;
            };

            used[index] = true;
            let id = old[index];
            let exiting = self.tree.get(id).exiting;
            if exiting {
                // re-added while leaving: play the enter transition again
                self.revive_node(id);
                changed_layout = true;
                changed_children = true;
            }
            if let Some(hash) = spec.element().memo_hash {
                let node_hash = self.tree.get(id).memo_hash;
                if node_hash.is_some() && node_hash == Some(hash) {
                    let states = self.tree.get(id).memo_states.clone();
                    self.local_states.borrow_mut().touch(&states);
                    new_children.push(id);
                    continue;
                }
            }
            if spec.element().memo_build.is_none() {
                if self.patch_node(id, spec.clone()) {
                    changed_layout = true;
                }
                self.set_memo(id, None, Vec::new());
            } else {
                self.local_states.borrow_mut().record();
                let real = resolve_memo(spec.clone());
                if self.patch_node(id, real) {
                    changed_layout = true;
                }
                let states = self.local_states.borrow_mut().end_record();
                self.set_memo(id, spec.element().memo_hash, states);
            }
            new_children.push(id);
        }

        // unmatched nodes leave: with a presence transition they stay (out of
        // flow) until it ends, anchored after their previous live sibling
        let mut leaving: Vec<(NodeId, Option<NodeId>)> = Vec::new(); // (id, anchor)
        let mut last_live: Option<NodeId> = None;
        for (index, &id) in old.iter().enumerate() {
            if used[index] {
                last_live = Some(id);
                continue;
            }
            let (exiting, presence, motion_on) = {
                let node = self.tree.get(id);
                (node.exiting, node.motion.presence, motion::enabled())
            };
            if !exiting {
                if !motion_on || presence == motion::Presence::None {
                    self.remove_subtree(id);
                    changed_layout = true;
                    changed_children = true;
                    continue;
                }
                self.begin_exit(id);
                changed_layout = true;
                changed_children = true;
            }
            leaving.push((id, last_live));
        }
        if !leaving.is_empty() {
            let mut merged: Vec<NodeId> = Vec::new();
            for (id, anchor) in &leaving {
                if anchor.is_none() {
                    merged.push(*id);
                }
            }
            for &id in &new_children {
                merged.push(id);
                for (leaving_id, anchor) in &leaving {
                    if *anchor == Some(id) {
                        merged.push(*leaving_id);
                    }
                }
            }
            // anchors that were removed themselves: keep those nodes at the end
            for (id, _) in &leaving {
                if !merged.contains(id) {
                    merged.push(*id);
                }
            }
            new_children = merged;
        }

        if old_len != new_children.len() {
            changed_children = true;
        } else if old != new_children {
            changed_children = true;
            changed_layout = true;
        }

        if changed_children {
            self.tree.get_mut(parent_id).children = new_children.clone();
            for &id in &new_children {
                self.tree.get_mut(id).parent = Some(parent_id);
            }
            self.tree.mark_dirty(parent_id, true, true);
        }
        if changed_layout {
            self.tree.mark_dirty(parent_id, true, true);
        }
        changed_layout
    }

    /// The match search: the index of the old child `spec` reuses.
    pub(super) fn find_match(
        &self,
        old: &[NodeId],
        used: &[bool],
        cursor: &mut usize,
        keyed: &mut KeyedLookup,
        spec: &Node,
    ) -> Option<usize> {
        let element = spec.element();
        if let Some(key) = element.key.as_deref() {
            // keyed specs only ever match a node with the same key
            let found = self.find_keyed(old, used, keyed, key, element.kind);
            if let Some(index) = found {
                keyed.next = index + 1;
            }
            return found;
        }
        // unkeyed specs match the next unused unkeyed node in order
        // (nodes playing an exit transition are never reused)
        while *cursor < old.len() {
            let index = *cursor;
            let candidate = self.tree.get(old[index]);
            if used[index] || candidate.key.is_some() || candidate.exiting {
                *cursor += 1;
                continue;
            }
            *cursor += 1;
            if kind_compatible(candidate.kind(), element.kind) {
                return Some(index);
            }
            return None;
        }
        None
    }

    pub(super) fn find_keyed(
        &self,
        old: &[NodeId],
        used: &[bool],
        keyed: &mut KeyedLookup,
        key: &str,
        kind: NodeKind,
    ) -> Option<usize> {
        let matches = |index: usize| {
            let node = self.tree.get(old[index]);
            !used[index] && node.key.as_deref() == Some(key) && kind_compatible(node.kind(), kind)
        };
        // in order: the old child after the previous keyed match
        if keyed.next < old.len() && matches(keyed.next) {
            return Some(keyed.next);
        }
        let by_key = keyed.by_key.get_or_insert_with(|| {
            let mut map = HashMap::with_capacity(old.len());
            for (index, &id) in old.iter().enumerate() {
                if let Some(old_key) = &self.tree.get(id).key {
                    map.entry(old_key.clone()).or_insert(index);
                }
            }
            map
        });
        match by_key.get(key) {
            Some(&index) if matches(index) => Some(index),
            // a duplicate key already used, or another kind: scan
            Some(_) => (0..old.len()).find(|&index| matches(index)),
            None => None,
        }
    }
}
