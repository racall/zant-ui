//! `ui::portal` — the React portal builder.
//!
//! The node is moved out of the current position into the build context's
//! portal queue; the runtime appends the queued nodes to the window root
//! ([`super::context::take_portals`]).
//! The returned placeholder keeps the position in the app's layout: an empty
//! container with `width = 0`, `height = 0`, `shrink = 0`.
#![forbid(unsafe_code)]

use crate::core::node::{FlexStyle, NodeKind};

use super::context;
use super::element::{Element, Node};

/// Renders `node` as the last child of the window root instead of here (a
/// React portal): overlays such as `ui::modal` and `ui::message` cover the
/// window wherever they are built. Returns an empty placeholder for this
/// position. Give the node a key.
pub fn portal(node: Node) -> Node {
    context::push_portal(node);
    Node::new(Element {
        kind: NodeKind::Container,
        layout: FlexStyle {
            width: Some(0.0),
            height: Some(0.0),
            shrink: 0.0,
            ..FlexStyle::default()
        },
        ..Element::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Str;
    use crate::ui::builders::text;
    use crate::ui::context::take_portals;
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

    /// Portals are thread-local build state; a test starts from an empty queue.
    fn reset() {
        let _ = take_portals();
    }

    #[test]
    fn portal_queues_the_node_and_returns_a_zero_size_placeholder() {
        reset();
        let overlay = keyed("overlay");
        let placeholder = portal(overlay.clone());

        let element = placeholder.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.layout.width, Some(0.0));
        assert_eq!(element.layout.height, Some(0.0));
        assert_eq!(element.layout.shrink, 0.0);
        assert!(element.children.is_empty());
        assert!(element.key.is_none());
        assert_eq!(element.layout.direction, FlexStyle::default().direction);
        assert_eq!(element.visual.background, None);

        let queued = take_portals();
        assert_eq!(queued.len(), 1);
        assert!(queued[0].ptr_eq(&overlay), "the exact node is queued");
        assert!(take_portals().is_empty(), "taking drains the queue");
        reset();
    }

    #[test]
    fn portal_queue_keeps_the_build_order() {
        reset();
        let first = keyed("first");
        let second = keyed("second");
        let _ = portal(first.clone());
        let _ = portal(second.clone());

        let queued = take_portals();
        assert_eq!(queued.len(), 2);
        assert!(queued[0].ptr_eq(&first));
        assert!(queued[1].ptr_eq(&second));
        reset();
    }

    #[test]
    fn each_portal_returns_its_own_placeholder() {
        reset();
        let a = portal(keyed("a"));
        let b = portal(keyed("b"));
        assert!(!a.ptr_eq(&b), "every portal builds a fresh placeholder");
        assert_eq!(
            b.element().layout.width,
            Some(0.0),
            "the placeholder is not the portalled node"
        );
        assert_eq!(take_portals().len(), 2);
        reset();
    }

    #[test]
    fn portal_keeps_runtime_strings_alive_in_the_queue() {
        reset();
        let runtime_text = String::from("runtime portal");
        let node = text(Str::copy(&runtime_text), TextOptions::default());
        let _ = portal(node);
        let queued = take_portals();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].element().content, "runtime portal");
        reset();
    }
}
