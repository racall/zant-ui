//! Reconciler tests: matching, reuse, freeing and the root rules.

use super::*;
use crate::core::node::NodeData;
use crate::theme::tokens::ant_light;
use crate::ui::builders::{column, text};
use crate::ui::options::{BoxOptions, TextOptions};
use crate::ui::{Node, RunOptions, app};

struct Keyed {
    names: Vec<&'static str>,
}

impl app::App for Keyed {
    fn render(&mut self) -> Node {
        column(BoxOptions {
            children: self
                .names
                .iter()
                .map(|name| {
                    text(
                        *name,
                        TextOptions {
                            key: Some((*name).into()),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            ..Default::default()
        })
    }
}

fn runtime_with(names: Vec<&'static str>) -> Runtime {
    Runtime::init(Keyed { names }, RunOptions::default())
}

fn texts(runtime: &Runtime) -> Vec<String> {
    runtime
        .tree
        .nodes
        .iter()
        .filter_map(|node| match &node.data {
            NodeData::Text(data) => Some(data.content.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn keyed_children_are_reused_and_unkeyed_ones_follow_order() {
    let mut runtime = runtime_with(vec!["a", "b"]);
    runtime.rebuild_for_test();
    let first_ids: Vec<NodeId> = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && node.key.is_some())
        .map(|node| node.id)
        .collect();
    assert_eq!(texts(&runtime), ["a", "b"]);
    // same keys: the retained nodes are reused (same ids), content patched
    runtime.app_state_mut::<Keyed>().names = vec!["a", "b"];
    runtime.rebuild_for_test();
    let second_ids: Vec<NodeId> = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && node.key.is_some())
        .map(|node| node.id)
        .collect();
    assert_eq!(first_ids.len(), 2);
    assert_eq!(second_ids, first_ids);
    // reordered keys are matched by key, not position
    runtime.app_state_mut::<Keyed>().names = vec!["b", "a"];
    runtime.rebuild_for_test();
    let reordered: Vec<(String, NodeId)> = runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && node.key.is_some())
        .map(|node| (node.key.clone().unwrap().to_string(), node.id))
        .collect();
    let ids: std::collections::HashMap<String, NodeId> = reordered.into_iter().collect();
    assert_eq!(ids["a"], first_ids[0]);
    assert_eq!(ids["b"], first_ids[1]);
}

fn ids_by_key(runtime: &Runtime) -> std::collections::HashMap<String, NodeId> {
    runtime
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive && node.key.is_some())
        .map(|node| (node.key.clone().unwrap().to_string(), node.id))
        .collect()
}

#[test]
fn keyed_moves_removals_and_inserts_keep_every_surviving_node() {
    let names: Vec<&'static str> = vec!["a", "b", "c", "d", "e", "f"];
    let mut runtime = runtime_with(names.clone());
    runtime.rebuild_for_test();
    let before = ids_by_key(&runtime);
    // reversed, "c" removed, "x" inserted in the middle, "a" moved last
    runtime.app_state_mut::<Keyed>().names = vec!["f", "e", "x", "d", "b", "a"];
    runtime.rebuild_for_test();
    assert_eq!(texts_in_order(&runtime), ["f", "e", "x", "d", "b", "a"]);
    let after = ids_by_key(&runtime);
    for name in ["a", "b", "d", "e", "f"] {
        assert_eq!(after[name], before[name], "{name} keeps its node");
    }
    assert!(!after.contains_key("c"));
    // duplicate keys still match one node each
    runtime.app_state_mut::<Keyed>().names = vec!["a", "a", "b"];
    runtime.rebuild_for_test();
    assert_eq!(texts_in_order(&runtime), ["a", "a", "b"]);
}

/// The root's children in tree order (not slot order).
fn texts_in_order(runtime: &Runtime) -> Vec<String> {
    let root = runtime.tree.root;
    runtime
        .tree
        .get(root)
        .children
        .iter()
        .filter_map(|&id| match &runtime.tree.get(id).data {
            NodeData::Text(data) => Some(data.content.to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn removed_children_are_freed() {
    let mut runtime = runtime_with(vec!["a", "b", "c"]);
    runtime.rebuild_for_test();
    assert_eq!(texts(&runtime), ["a", "b", "c"]);
    runtime.app_state_mut::<Keyed>().names = vec!["a"];
    runtime.rebuild_for_test();
    assert_eq!(texts(&runtime), ["a"]);
    // freed slots are recycled tombstones
    assert_eq!(runtime.tree.free_ids.len(), 2);
}

#[test]
fn a_non_container_root_keeps_the_previous_tree() {
    struct TextRoot;
    impl app::App for TextRoot {
        fn render(&mut self) -> Node {
            text("not a container", TextOptions::default())
        }
    }
    let mut runtime = Runtime::init(TextRoot, RunOptions::default());
    runtime.rebuild_for_test();
    assert!(!runtime.needs_build, "the build finished");
    assert_eq!(runtime.tree.nodes.len(), 1, "only the root exists");
}

#[test]
fn set_theme_replaces_the_tree_theme_and_selection_colour() {
    let mut runtime = runtime_with(vec!["a"]);
    runtime.rebuild_for_test();
    let dark = crate::theme::tokens::ant_dark();
    runtime.set_theme(dark);
    assert_eq!(runtime.theme, dark);
    assert_eq!(runtime.tree.theme, dark);
    assert_eq!(
        runtime.tree.selection_color,
        crate::theme::tokens::with_alpha(dark.color_primary, 0x4d)
    );
    assert!(runtime.tree.needs_layout());
    // setting the same theme again is a no-op
    runtime.tree.clear_paint();
    runtime.tree.get_mut(runtime.tree.root).dirty.layout = false;
    runtime.set_theme(dark);
    assert!(!runtime.tree.needs_layout());
}

#[test]
fn the_default_tree_root_fills_the_window() {
    let mut runtime = runtime_with(vec![]);
    runtime.rebuild_for_test();
    let root = runtime.tree.get(runtime.tree.root);
    assert_eq!(root.style.width, None);
    assert_eq!(root.style.height, None);
    assert_eq!(root.style.grow, 0.0);
    assert_eq!(root.visual.background, Some(ant_light().color_bg_base));
    assert_eq!(
        runtime.clear_color,
        ant_light().color_bg_base,
        "the frame clears to the app background"
    );
    assert_eq!(runtime.tree.root, 0);
}
