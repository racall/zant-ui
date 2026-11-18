//! `apply_common`: the props every element kind shares (box, visual, motion, actions).

use super::*;

impl Runtime {
    /// Style, visual, motion, enabled and tooltip of any
    /// node kind. Returns true when the layout is affected.
    pub(crate) fn apply_common(&mut self, id: NodeId, element: &Element) -> bool {
        let mut layout_changed;
        let mut disabled_now = false;
        let mut needs_key = false;
        let needs_tooltip;
        {
            let node = self.tree.get_mut(id);
            layout_changed = node.style != element.layout || node.visual != element.visual;
            node.style = element.layout;
            node.visual = element.visual;
            node.motion = element.motion;
            if node.enabled != element.enabled {
                layout_changed = true;
                node.enabled = element.enabled;
                disabled_now = !element.enabled;
            }
            if disabled_now {
                match &mut node.data {
                    NodeData::Button(data) => {
                        data.hovered = false;
                        data.pressed = false;
                        data.hover = Animated::at(0.0);
                        data.press = Animated::at(0.0);
                        data.wave = Animated::at(1.0);
                        data.hover_t = 0.0;
                        data.press_t = 0.0;
                        data.wave_t = 1.0;
                    }
                    NodeData::Input(data) => {
                        data.hovered = false;
                        data.focused = false;
                        data.blink_on = true;
                        data.hover = Animated::at(0.0);
                        data.focus_anim = Animated::at(0.0);
                        data.hover_t = 0.0;
                        data.focus_t = 0.0;
                    }
                    NodeData::Toggle(data) => {
                        data.hovered = false;
                        data.pressed = false;
                        data.hover = Animated::at(0.0);
                        data.press = Animated::at(0.0);
                        data.hover_t = 0.0;
                        data.press_t = 0.0;
                    }
                    NodeData::Select(data) => {
                        data.hovered = false;
                        data.hover = Animated::at(0.0);
                        data.focus_anim = Animated::at(0.0);
                        data.hover_t = 0.0;
                        data.focus_t = 0.0;
                    }
                    _ => {}
                }
            }
            node.focusable = element.focusable;
            if node.key.is_none() && element.key.is_some() {
                needs_key = true;
            }
            let new_tooltip = element.tooltip.as_deref();
            needs_tooltip = match node.tooltip.as_deref() {
                Some(old) => new_tooltip.is_none_or(|new| new != old),
                None => new_tooltip.is_some(),
            };
        }
        if needs_key {
            self.tree.set_key(id, element.key.as_deref());
        }
        if needs_tooltip {
            self.tree.set_tooltip(id, element.tooltip.as_deref());
        }
        if disabled_now && self.tree.focused == Some(id) {
            self.tree.set_focus(None);
        }
        layout_changed
    }
}
