//! Pointer-area callbacks, hover and leave events, and capture release.

use super::Runtime;
use crate::core::events::PointerEvent;
use crate::core::node::{NodeData, NodeId};
use crate::ui::actions::{PointerInfo, PointerPhase};

impl Runtime {
    /// The pointer area handler of a container.
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

    /// Calls a pointer area's handler with coordinates
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
            let mut app = self.app.borrow_mut();
            let mut cx = super::action_cx(&mut app);
            area.action.call(&mut cx, info)
        };
        if changed {
            self.needs_build = true;
        }
    }

    /// Hover / leave for pointer areas under the pointer (no button held).
    pub(super) fn update_pointer_hover(&mut self, target: Option<NodeId>, p: PointerEvent) {
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
}
