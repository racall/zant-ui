//! Pointer-target and overlay builders: `pointer_area`, `clickable`,
//! `tooltip`, `dismiss_layer`.
//!
//! `pointer_area` / `clickable` are ordinary `column` containers plus a
//! [`PointerArea`] handler, so the runtime can deliver raw pointer input
//! without the app touching the tree. `clickable` wraps `pointer_area` with
//! the "released over the area" rule. `tooltip` / `dismiss_layer` are the
//! small declarative wrappers popovers are built from.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::events::PointerButton;
use crate::core::node::{ButtonType, NodeKind, PointerArea};
use crate::layout::flex;

use super::super::actions::{PointerAction, PointerPhase};
use super::super::element::{Element, Node};
use super::super::options::{
    ButtonOptions, ClickableOptions, DismissLayerOptions, PointerAreaOptions, TooltipOptions,
};
use super::super::styled::{Style, styled};
use super::{button, hover_from, layout_from, visual_from};

/// `ui::pointer_area`: a column container that handles raw pointer input.
pub fn pointer_area(options: PointerAreaOptions) -> Node {
    let direction = options
        .r#box
        .flex_direction
        .unwrap_or(flex::Direction::Column);
    let layout = layout_from(&options.r#box, direction);
    let visual = visual_from(&options.r#box);
    let hover_visual = hover_from(&options.r#box);
    Node::new(Element {
        kind: NodeKind::Container,
        key: options.r#box.key,
        layout,
        visual,
        children: options.r#box.children,
        focusable: options.focusable,
        pointer: Some(PointerArea {
            action: options.on_pointer,
            capture_scroll: options.capture_scroll,
            clicks: options.clicks,
        }),
        key_action: options.on_key,
        cursor: options.cursor,
        on_resize: options.r#box.on_resize,
        hover_visual,
        ..Element::default()
    })
}

/// `ui::clickable`: a container clicked as a whole (list row, card).
///
/// A press that is dragged away cancels the click; `index_action` wins over
/// `on_click`.
pub fn clickable(options: ClickableOptions) -> Node {
    let on_click = options.on_click;
    let index_action = options.index_action;
    let index = options.index;
    pointer_area(PointerAreaOptions {
        r#box: options.r#box,
        cursor: Some(options.cursor),
        on_pointer: PointerAction::new(move |cx, info| {
            if info.phase != PointerPhase::Up || info.button != PointerButton::Left {
                return false;
            }
            // released over the area (a press dragged away is cancelled)
            if info.x < 0.0 || info.y < 0.0 || info.x > info.width || info.y > info.height {
                return false;
            }
            if let Some(callback) = &index_action {
                callback.call(cx, index);
                return true;
            }
            if let Some(callback) = &on_click {
                callback.call(cx);
                return true;
            }
            false
        }),
        ..Default::default()
    })
}

/// `ui::tooltip`: the same node with a tooltip attached.
pub fn tooltip(options: TooltipOptions) -> Node {
    let mut element = options.child.element_clone();
    element.tooltip = Some(options.text);
    Node::new(element)
}

/// `ui::dismiss_layer`: a full-size invisible click catcher placed right before
/// a popover / menu; `on_dismiss` fires for clicks outside the popover.
pub fn dismiss_layer(options: DismissLayerOptions) -> Node {
    styled(
        button(ButtonOptions {
            key: options.key,
            text: Str::default(),
            variant: ButtonType::Plain,
            focusable: false,
            on_click: options.on_dismiss,
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            radius: Some(0.0),
            background: options.background,
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::super::text;
    use super::*;
    use crate::core::geometry::Edges;
    use crate::platform::platform::Cursor;
    use crate::ui::actions::{Action, ActionCx, PointerInfo, action, index_action};
    use crate::ui::options::BoxOptions;

    fn inside() -> PointerInfo {
        PointerInfo {
            phase: PointerPhase::Up,
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 20.0,
            ..Default::default()
        }
    }

    #[test]
    fn pointer_area_is_a_column_container_by_default() {
        let node = pointer_area(PointerAreaOptions {
            r#box: BoxOptions {
                key: Some("split".into()),
                width: Some(6.0),
                height_percent: Some(100.0),
                padding: Edges::all(2.0),
                background: Some(0xff00_0000),
                hover_background: Some(0x1100_0000),
                ..Default::default()
            },
            cursor: Some(Cursor::ResizeHorizontal),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.key.as_deref(), Some("split"));
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.layout.width, Some(6.0));
        assert_eq!(element.layout.height_percent, Some(100.0));
        assert_eq!(element.layout.padding, Edges::all(2.0));
        assert_eq!(element.visual.background, Some(0xff00_0000));
        assert_eq!(element.cursor, Some(Cursor::ResizeHorizontal));
        assert!(!element.focusable);
        assert!(element.key_action.is_none());
        assert!(element.on_resize.is_none());
        let hover = element.hover_visual.expect("hover look from BoxOptions");
        assert_eq!(hover.background, Some(0x1100_0000));
        let pointer = element.pointer.as_ref().expect("pointer handler");
        assert!(pointer.clicks);
        assert!(!pointer.capture_scroll);
    }

    #[test]
    fn pointer_area_honours_row_direction_focus_and_scroll_capture() {
        let node = pointer_area(PointerAreaOptions {
            r#box: BoxOptions {
                flex_direction: Some(flex::Direction::Row),
                gap: 2.0,
                children: vec![text("x", Default::default())],
                on_resize: Some(action(|s: &mut u32| *s += 1)),
                ..Default::default()
            },
            capture_scroll: true,
            clicks: false,
            focusable: true,
            on_key: Some(crate::ui::actions::KeyAction::new(|_, _| true)),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.layout.direction, flex::Direction::Row);
        assert_eq!(element.layout.gap, 2.0);
        assert_eq!(element.children.len(), 1);
        assert!(element.focusable);
        assert!(element.key_action.is_some());
        assert!(element.on_resize.is_some());
        let pointer = element.pointer.as_ref().expect("pointer handler");
        assert!(pointer.capture_scroll);
        assert!(!pointer.clicks);
    }

    #[test]
    fn pointer_area_keeps_the_box_direction_when_given() {
        let node = pointer_area(PointerAreaOptions {
            r#box: BoxOptions {
                flex_direction: Some(flex::Direction::Row),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(node.element().layout.direction, flex::Direction::Row);
    }

    #[test]
    fn clickable_is_a_pointing_hand_area_with_hover() {
        let node = clickable(ClickableOptions {
            r#box: BoxOptions {
                padding: Edges::all(8.0),
                radius: 6.0,
                hover_background: Some(0x0f00_0000),
                children: vec![text("row", Default::default())],
                ..Default::default()
            },
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Container);
        assert_eq!(element.layout.direction, flex::Direction::Column);
        assert_eq!(element.cursor, Some(Cursor::PointingHand));
        let hover = element.hover_visual.expect("hover look from BoxOptions");
        assert_eq!(hover.background, Some(0x0f00_0000));
        assert_eq!(element.layout.padding, Edges::all(8.0));
        assert_eq!(element.visual.radius, 6.0);
        assert_eq!(element.children.len(), 1);
        let pointer = element.pointer.as_ref().expect("pointer handler");
        assert!(pointer.clicks);
        assert!(!pointer.capture_scroll);
    }

    #[test]
    fn clickable_fires_on_click_only_when_released_inside() {
        let clicks = Rc::new(Cell::new(0u32));
        let captured = Rc::clone(&clicks);
        let node = clickable(ClickableOptions {
            on_click: Some(Action::new(move |_| captured.set(captured.get() + 1))),
            ..Default::default()
        });
        let pointer = node.element().pointer.clone().expect("pointer handler");
        let mut state = 0u32;
        let mut cx = ActionCx::new(&mut state);
        // a press alone does nothing
        assert!(!pointer.action.call(
            &mut cx,
            PointerInfo {
                phase: PointerPhase::Down,
                ..inside()
            }
        ));
        assert_eq!(clicks.get(), 0);
        // a non-left release does nothing
        assert!(!pointer.action.call(
            &mut cx,
            PointerInfo {
                button: PointerButton::Right,
                ..inside()
            }
        ));
        assert_eq!(clicks.get(), 0);
        // released outside the area: the press was dragged away
        assert!(!pointer.action.call(
            &mut cx,
            PointerInfo {
                x: 200.0,
                ..inside()
            }
        ));
        assert_eq!(clicks.get(), 0);
        // released inside: the click fires
        assert!(pointer.action.call(&mut cx, inside()));
        assert_eq!(clicks.get(), 1);
    }

    #[test]
    fn clickable_prefers_the_index_action() {
        let node = clickable(ClickableOptions {
            on_click: Some(action(|s: &mut u32| *s += 1)),
            index_action: Some(index_action(|s: &mut u32, index| *s = 100 + index as u32)),
            index: 3,
            ..Default::default()
        });
        let pointer = node.element().pointer.clone().expect("pointer handler");
        let mut state = 0u32;
        let mut cx = ActionCx::new(&mut state);
        assert!(pointer.action.call(&mut cx, inside()));
        assert_eq!(state, 103, "the index action wins over on_click");
    }

    #[test]
    fn tooltip_attaches_to_a_copy_of_the_child() {
        let child = button(ButtonOptions {
            text: "Save".into(),
            ..Default::default()
        });
        let node = tooltip(TooltipOptions {
            text: "Save the file".into(),
            child: child.clone(),
        });
        assert_eq!(node.element().tooltip.as_deref(), Some("Save the file"));
        assert_eq!(node.element().kind, NodeKind::Button);
        assert_eq!(node.element().content, "Save");
        assert!(!node.ptr_eq(&child), "the child is copied, not mutated");
        assert!(child.element().tooltip.is_none());
    }

    #[test]
    fn dismiss_layer_covers_the_parent_and_dismisses() {
        let mut state = 0u32;
        let node = dismiss_layer(DismissLayerOptions {
            on_dismiss: Some(action(|s: &mut u32| *s += 1)),
            ..Default::default()
        });
        let element = node.element();
        assert_eq!(element.kind, NodeKind::Button);
        assert_eq!(element.button_type, ButtonType::Plain);
        assert_eq!(element.key.as_deref(), Some("dismiss-layer"));
        assert!(!element.focusable);
        assert!(element.layout.absolute);
        assert_eq!(element.layout.left, Some(0.0));
        assert_eq!(element.layout.top, Some(0.0));
        assert_eq!(element.layout.width_percent, Some(100.0));
        assert_eq!(element.layout.height_percent, Some(100.0));
        assert_eq!(element.visual.radius, 0.0);
        assert_eq!(element.visual.background, None, "paints nothing by default");
        {
            let mut cx = ActionCx::new(&mut state);
            if let Some(callback) = element.on_click.clone() {
                callback.call(&mut cx);
            }
        }
        assert_eq!(state, 1);

        let dimmed = dismiss_layer(DismissLayerOptions {
            background: Some(0x8000_0000),
            ..Default::default()
        });
        assert_eq!(dimmed.element().visual.background, Some(0x8000_0000));
    }
}
