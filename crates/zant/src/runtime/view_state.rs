//! Keyed, version-tolerant view snapshots for replacement runtimes.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

use serde_json::{Value, json};

use super::Runtime;
use crate::components::list::ListState;
use crate::core::node::{NodeData, NodeId};
use crate::core::text_select::{self, Point};
use crate::motion::motion::Animated;

pub(crate) type Lists = Rc<RefCell<HashMap<String, Weak<RefCell<ListState>>>>>;
thread_local! {
    // Only routes registrations during a build; all state belongs to a runtime.
    static BUILD_LISTS: RefCell<Option<ListBuild>> = const { RefCell::new(None) };
}

struct ListBuild {
    lists: Lists,
    seen: HashSet<String>,
}

pub(crate) struct ListBuildGuard(Option<ListBuild>);
impl ListBuildGuard {
    pub(crate) fn enter(lists: Lists) -> Self {
        Self(BUILD_LISTS.with(|slot| {
            slot.replace(Some(ListBuild {
                lists,
                seen: HashSet::new(),
            }))
        }))
    }
}
impl Drop for ListBuildGuard {
    fn drop(&mut self) {
        BUILD_LISTS.with(|slot| slot.replace(self.0.take()));
    }
}

pub(crate) fn register_list(viewport: &str, state: &Rc<RefCell<ListState>>) {
    BUILD_LISTS.with(|slot| {
        if let Some(build) = slot.borrow_mut().as_mut()
            && build.seen.insert(viewport.into())
        {
            build
                .lists
                .borrow_mut()
                .insert(viewport.into(), Rc::downgrade(state));
        }
    });
}

pub(crate) struct Pending {
    nodes: serde_json::Map<String, Value>,
    selection: Option<Value>,
    frames: u8,
}

fn byte(text: &str, value: &Value) -> Option<usize> {
    let mut pos = usize::try_from(value.as_u64()?)
        .unwrap_or(usize::MAX)
        .min(text.len());
    while !text.is_char_boundary(pos) {
        pos -= 1;
    }
    Some(pos)
}

impl Runtime {
    // Traverse children rather than the arena: recycled slots are not tree order.
    fn view_keys(&self) -> Vec<(String, NodeId)> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![self.tree.root];
        while let Some(id) = stack.pop() {
            let node = self.tree.get(id);
            if !node.alive {
                continue;
            }
            if let Some(key) = &node.key
                && seen.insert(key.to_string())
            {
                out.push((key.to_string(), id));
            }
            stack.extend(node.children.iter().rev().copied());
        }
        out
    }

    fn view_lists(&self, keys: &[(String, NodeId)]) -> HashMap<NodeId, Rc<RefCell<ListState>>> {
        let lists = self.view_lists.borrow();
        keys.iter()
            .filter_map(|(key, id)| {
                let state = lists.get(key)?.upgrade()?;
                let parent = self.tree.get(*id).parent?;
                Some((parent, state))
            })
            .collect()
    }

    /// What the next runtime on this window should restore.
    pub fn view_state(&self) -> Value {
        let keys = self.view_keys();
        let lists = self.view_lists(&keys);
        let mut nodes = serde_json::Map::new();
        for (key, id) in &keys {
            let node = self.tree.get(*id);
            let mut state = serde_json::Map::new();
            if node.style.scroll
                && let NodeData::Container(data) = &node.data
            {
                state.insert("scroll".into(), json!(data.scroll_offset));
            }
            if let Some(list) = lists.get(id)
                && let Ok(list) = list.try_borrow()
            {
                state.insert("list".into(), json!({"anchor": list.anchor, "offset": list.offset, "following": list.following}));
            }
            if self.tree.focused == Some(*id) {
                state.insert("focus".into(), json!(true));
                if let NodeData::Input(data) = &node.data {
                    state.insert(
                        "input".into(),
                        json!({"anchor": data.anchor, "cursor": data.cursor}),
                    );
                }
            }
            if !state.is_empty() {
                nodes.insert(key.clone(), state.into());
            }
        }
        let key_of = |id| {
            keys.iter()
                .find(|(_, node)| *node == id)
                .map(|(key, _)| key)
        };
        let selection = (|| {
            let root = key_of(self.sel_root?)?;
            let anchor = self.sel_anchor?;
            let focus = self.sel_focus?;
            Some(
                json!({"root": root, "anchor": {"key": key_of(anchor.node)?, "byte": anchor.byte}, "focus": {"key": key_of(focus.node)?, "byte": focus.byte}}),
            )
        })();
        json!({"version": 1, "nodes": nodes, "selection": selection})
    }

    /// Restores a snapshot after layout as its keyed nodes become available.
    /// Missing keys expire after ten frames; malformed entries are ignored.
    pub fn restore_view_state(&mut self, state: Value) {
        let nodes = state
            .get("nodes")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let selection = state.get("selection").filter(|v| v.is_object()).cloned();
        self.pending_view_state = Some(Pending {
            nodes,
            selection,
            frames: 0,
        });
        self.request_redraw();
    }

    pub(crate) fn apply_view_state(&mut self) {
        let Some(mut pending) = self.pending_view_state.take() else {
            return;
        };
        pending.frames += 1;
        let keys = self.view_keys();
        let lists = self.view_lists(&keys);
        for (key, id) in &keys {
            let Some(state) = pending.nodes.remove(key) else {
                continue;
            };
            if let Some(offset) = state.get("scroll").and_then(Value::as_f64)
                && offset.is_finite()
                && (offset as f32).is_finite()
            {
                let node = self.tree.get_mut(*id);
                if node.style.scroll
                    && let NodeData::Container(data) = &mut node.data
                {
                    data.scroll_animating = false;
                    let delta = offset as f32 - data.scroll_offset;
                    self.tree.scroll_by(*id, delta);
                }
            }
            if let Some(saved) = state.get("list")
                && let Some(list) = lists.get(id)
                && let Ok(mut list) = list.try_borrow_mut()
                && let (Some(anchor), Some(offset)) =
                    (saved["anchor"].as_u64(), saved["offset"].as_f64())
                && offset.is_finite()
                && (offset as f32).is_finite()
            {
                list.anchor = usize::try_from(anchor)
                    .unwrap_or(usize::MAX)
                    .min(list.count().saturating_sub(1));
                list.offset = offset as f32;
                list.pending_scroll = None;
                list.scroll_by(0.0);
                list.following = saved["following"].as_bool().unwrap_or(false);
                self.needs_build = true;
            }
            let node = self.tree.get(*id);
            if state["focus"].as_bool() == Some(true)
                && node.focusable
                && node.enabled
                && node.in_flow()
            {
                self.focus_node(Some(*id));
                match &mut self.tree.get_mut(*id).data {
                    NodeData::Input(data) => {
                        data.focus_anim = Animated::at(1.0);
                        data.focus_t = 1.0;
                        if let Some(input) = state.get("input")
                            && let (Some(anchor), Some(cursor)) = (
                                byte(&data.text, &input["anchor"]),
                                byte(&data.text, &input["cursor"]),
                            )
                        {
                            data.anchor = anchor;
                            data.cursor = cursor;
                            data.reveal_caret = true;
                        }
                    }
                    NodeData::Select(data) => {
                        data.focus_anim = Animated::at(1.0);
                        data.focus_t = 1.0;
                    }
                    _ => {}
                }
            }
        }
        if let Some(selection) = &pending.selection {
            let find = |value: &Value| {
                let key = value.as_str()?;
                keys.iter().find(|(k, _)| k == key).map(|(_, id)| *id)
            };
            if let (Some(root), Some(anchor), Some(focus)) = (
                find(&selection["root"]),
                find(&selection["anchor"]["key"]),
                find(&selection["focus"]["key"]),
            ) {
                let mut text = Vec::new();
                text_select::collect(&self.tree, root, &mut text);
                if self.tree.get(root).visual.selectable
                    && text.contains(&anchor)
                    && text.contains(&focus)
                    && let (Some(a), Some(f)) = (
                        byte(
                            &text_select::node_text(self.tree.get(anchor)),
                            &selection["anchor"]["byte"],
                        ),
                        byte(
                            &text_select::node_text(self.tree.get(focus)),
                            &selection["focus"]["byte"],
                        ),
                    )
                {
                    self.sel_root = Some(root);
                    self.sel_anchor = Some(Point {
                        node: anchor,
                        byte: a,
                    });
                    self.sel_focus = Some(Point {
                        node: focus,
                        byte: f,
                    });
                    self.apply_text_selection();
                }
                pending.selection = None;
            }
        }
        self.tree.mark_paint_dirty();
        if pending.frames < 10 && (!pending.nodes.is_empty() || pending.selection.is_some()) {
            self.pending_view_state = Some(pending);
        }
        self.request_redraw();
    }
}
