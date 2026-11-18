//! Unit tests.

use crate::core::geometry::Size;
use crate::core::node::{FlexStyle, NodeData};
use crate::core::tree::Tree;
use crate::theme::tokens::ant_light;

#[test]
fn freed_slots_are_tombstoned_and_reused() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let first = tree.add_text(Some(root), "a", Default::default());
    let second = tree.add_text(Some(root), "b", Default::default());
    let live = tree.live_count();
    assert_eq!(live, 3);
    tree.free_subtree(first);
    assert!(!tree.get(first).alive, "freed slot is a tombstone");
    assert_eq!(tree.live_count(), live - 1);
    let reused = tree.add_text(Some(root), "c", Default::default());
    assert_eq!(reused, first, "the freed id is recycled");
    assert!(tree.get(reused).alive);
    assert_eq!(tree.get(reused).id, reused);
    assert!(matches!(tree.get(reused).data, NodeData::Text(_)));
    assert!(tree.get(second).alive);
}

#[test]
fn key_index_resolves_ids_and_bounds() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let id = tree.add_container(
        Some(root),
        FlexStyle {
            width: Some(80.0),
            height: Some(20.0),
            ..Default::default()
        },
    );
    tree.set_key(id, Some("panel"));
    assert_eq!(tree.id_of_key("panel"), Some(id));
    assert_eq!(tree.id_of_key("missing"), None);
    tree.layout(Size {
        width: 200.0,
        height: 100.0,
    });
    let bounds = tree.bounds_of_key("panel").expect("keyed bounds");
    assert_eq!(
        (bounds.x, bounds.y, bounds.width, bounds.height),
        (0.0, 0.0, 80.0, 20.0)
    );
    tree.set_key(id, Some("renamed"));
    assert_eq!(tree.id_of_key("panel"), None);
    assert_eq!(tree.id_of_key("renamed"), Some(id));
    tree.set_key(id, None);
    assert_eq!(tree.id_of_key("renamed"), None);
    // freeing removes the index entry
    tree.set_key(id, Some("gone"));
    tree.free_subtree(id);
    assert_eq!(tree.id_of_key("gone"), None);
}

#[test]
fn dirty_flags_propagate_to_the_root() {
    let mut tree = Tree::init(ant_light());
    let root = tree.root;
    let container = tree.add_container(Some(root), FlexStyle::default());
    let text = tree.add_text(Some(container), "x", Default::default());
    tree.clear_paint();
    assert!(!tree.needs_paint() || tree.needs_layout());
    tree.mark_dirty(text, false, true);
    assert!(tree.needs_paint());
    tree.mark_dirty(text, true, true);
    assert!(tree.needs_layout());
    tree.clear_paint();
    assert!(!tree.get(root).dirty.paint);
    let extra = tree.add_container(None, FlexStyle::default());
    tree.clear_paint();
    tree.get_mut(root).dirty.layout = false;
    tree.add_child(root, extra);
    assert!(tree.needs_layout(), "adding a child marks layout dirty");
    // detach the extra node again so the tree stays consistent
    tree.free_subtree(extra);
}

#[test]
fn poke_queues_animation_once() {
    let mut tree = Tree::init(ant_light());
    let id = tree.add_text(Some(tree.root), "x", Default::default());
    assert!(tree.animating.is_empty());
    tree.poke(id);
    assert_eq!(tree.animating, vec![id]);
    assert!(tree.get(id).anim_queued);
    tree.poke(id);
    assert_eq!(tree.animating.len(), 1, "already queued");
    tree.free_subtree(id);
    assert!(tree.animating.is_empty(), "freed nodes leave the queue");
}
