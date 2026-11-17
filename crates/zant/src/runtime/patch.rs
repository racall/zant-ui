//! Patching a retained node with the element of the same position
//! (`apply_common`, `patch_node`) and the small equality / selection helpers
//! it needs.
//!
//! Patching only writes what changed: layout is marked dirty when a box /
//! visual / content affecting layout changed, paint only for colours and
//! component state. Actions are cloned out of the element into the node — the
//! runtime clones them out of the node again before calling.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::node::{InputData, NodeData, NodeId, RichSpan, SelectData};
use crate::motion::motion::Animated;
use crate::renderer::text;
use crate::ui::element::{Element, Node};

use super::Runtime;

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

    /// Updates the retained node `id` from `spec` and
    /// reconciles its children for containers. Returns true when the layout
    /// must run.
    pub(crate) fn patch_node(&mut self, id: NodeId, spec: Node) -> bool {
        let element = spec.element();
        let mut layout_changed = self.apply_common(id, element);
        let mut paint_changed = layout_changed;
        let kind = self.tree.get(id).kind();

        match kind {
            crate::core::node::NodeKind::Container => {
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Container(data) = &mut node.data else {
                        return layout_changed;
                    };
                    // a pointer area appearing / disappearing changes hit testing
                    if data.pointer.is_none() != element.pointer.is_none() {
                        paint_changed = true;
                    }
                    data.pointer = element.pointer.clone();
                    data.key_action = element.key_action.clone();
                    data.cursor = element.cursor;
                    data.resize_action = element.on_resize.clone();
                    if data.hover_visual != element.hover_visual {
                        paint_changed = true;
                    }
                    data.hover_visual = element.hover_visual;
                }
                if element.pointer.is_none() && self.pointer_capture == Some(id) {
                    self.pointer_capture = None;
                }
            }
            crate::core::node::NodeKind::Text => {
                let (content_changed, style_changed) = {
                    let NodeData::Text(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    (
                        data.content.as_str() != element.content.as_str(),
                        !text_style_equal(&data.style, &element.text_style),
                    )
                };
                if content_changed {
                    self.tree.set_text(id, &element.content);
                    layout_changed = true;
                    paint_changed = true;
                }
                if style_changed {
                    if let NodeData::Text(data) = &mut self.tree.get_mut(id).data {
                        data.style = element.text_style;
                        data.invalidate();
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
            }
            crate::core::node::NodeKind::Button => {
                let label_changed = {
                    let NodeData::Button(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    data.label != element.content
                };
                if label_changed {
                    if let NodeData::Button(data) = &mut self.tree.get_mut(id).data {
                        data.label = Str::copy(&element.content);
                        data.invalidate_layout();
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Button(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.button_type != element.button_type
                        || data.size != element.button_size
                        || data.font_size != element.text_style.font_size
                    {
                        data.button_type = element.button_type;
                        data.size = element.button_size;
                        data.font_size = element.text_style.font_size;
                        data.invalidate_layout();
                        layout_changed = true;
                        paint_changed = true;
                    }
                    data.action = element.on_click.clone();
                    data.index_action = element.index_action.clone();
                    data.index = element.index;
                    node.focusable = element.focusable;
                    data.hover_text_color = element.hover_text_color;
                    if data.icon != element.icon
                        || data.icon_size != element.icon_size
                        || data.text_color != element.text_color
                        || data.text_align != element.text_style.text_align
                        || data.label_padding != element.label_padding
                    {
                        data.icon = element.icon;
                        data.icon_size = element.icon_size;
                        data.text_color = element.text_color;
                        data.text_align = element.text_style.text_align;
                        data.label_padding = element.label_padding;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    if data.danger != element.danger {
                        data.danger = element.danger;
                        paint_changed = true;
                    }
                    // the spinner widens the button and starts its frames
                    if data.loading != element.loading {
                        data.loading = element.loading;
                        layout_changed = true;
                        paint_changed = true;
                        self.tree.poke(id);
                    }
                }
            }
            crate::core::node::NodeKind::Input => {
                if element.controlled {
                    let value_changed = {
                        let NodeData::Input(data) = &self.tree.get(id).data else {
                            return layout_changed;
                        };
                        data.text.as_str() != element.content.as_str()
                    };
                    if value_changed {
                        self.tree.input_set_value(id, &element.content);
                        paint_changed = true;
                    }
                }
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Input(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.placeholder != element.placeholder {
                        data.placeholder = Str::copy(&element.placeholder);
                        data.placeholder_layout = None;
                        paint_changed = true;
                    }
                    if data.read_only != element.read_only {
                        data.read_only = element.read_only;
                        paint_changed = true;
                    }
                    if data.password != element.password {
                        data.password = element.password;
                        data.rebuild_display();
                        paint_changed = true;
                    }
                    if data.font_size != element.text_style.font_size
                        || data.line_height != element.text_style.line_height
                    {
                        data.font_size = element.text_style.font_size;
                        data.line_height = element.text_style.line_height;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    data.selection_action = element.on_selection.clone();
                    data.key_action = element.on_key.clone();
                    if apply_selection(data, element.selection) {
                        paint_changed = true;
                    }
                    if data.content_max_width != element.content_max_width {
                        data.content_max_width = element.content_max_width;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    data.text_action = element.on_text.clone();
                    data.submit_action = element.on_click.clone();
                    data.undo_action = element.on_undo.clone();
                    data.redo_action = element.on_redo.clone();
                    data.focus_ring = element.focus_ring;
                    if data.multiline != element.multiline {
                        data.multiline = element.multiline;
                        data.invalidate_layout();
                        paint_changed = true;
                    }
                    node.focusable = true;
                }
            }
            crate::core::node::NodeKind::Vector => {
                {
                    let NodeData::Vector(data) = &mut self.tree.get_mut(id).data else {
                        return layout_changed;
                    };
                    if data.copy != element.content {
                        data.copy = Str::copy(&element.content);
                    }
                }
                if let Some(drawing) = element.drawing.as_deref() {
                    let before = match &self.tree.get(id).data {
                        NodeData::Vector(data) => data.hash,
                        _ => 0,
                    };
                    self.tree.vector_set(id, drawing);
                    let after = match &self.tree.get(id).data {
                        NodeData::Vector(data) => data.hash,
                        _ => before,
                    };
                    if before != after {
                        layout_changed = true;
                        paint_changed = true;
                    }
                }
            }
            crate::core::node::NodeKind::RichText => {
                let spans_changed = {
                    let NodeData::RichText(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    !rich_spans_equal(&data.spans, &element.rich_spans)
                };
                if spans_changed {
                    self.tree.rich_text_set(id, &element.rich_spans);
                    layout_changed = true;
                    paint_changed = true;
                }
                {
                    let NodeData::RichText(data) = &mut self.tree.get_mut(id).data else {
                        return layout_changed;
                    };
                    if data.line_height != element.rich_line_height {
                        data.line_height = element.rich_line_height;
                        data.invalidate_layout();
                        layout_changed = true;
                        paint_changed = true;
                    }
                    data.link_action = element.on_link.clone();
                }
            }
            crate::core::node::NodeKind::Select => {
                let options_changed = {
                    let NodeData::Select(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    !select_options_equal(data, &element.options)
                };
                if options_changed {
                    {
                        let NodeData::Select(data) = &mut self.tree.get_mut(id).data else {
                            return layout_changed;
                        };
                        data.clear_options();
                        for label in &element.options {
                            data.options.push(crate::core::node::SelectOption {
                                label: label.clone(),
                            });
                        }
                        if data.selected >= data.options.len() {
                            data.selected = 0;
                        }
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Select(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.selected != element.selected {
                        let last = data.options.len().saturating_sub(1);
                        data.selected = element.selected.min(last);
                        data.layout_dirty = true;
                        paint_changed = true;
                    }
                    data.action = element.index_action.clone();
                    node.focusable = true;
                }
            }
            crate::core::node::NodeKind::Image => {
                let node = self.tree.get_mut(id);
                let NodeData::Image(data) = &mut node.data else {
                    return layout_changed;
                };
                if element.image_id != 0 {
                    if data.source_id != element.image_id {
                        // compare the intrinsic size before / after
                        // `set_source`: a differently sized source changes
                        // the measured box.
                        let old_width = data.intrinsic_width;
                        let old_height = data.intrinsic_height;
                        crate::components::image::set_source(
                            data,
                            element.image_handle.as_ref(),
                            element.image_id,
                        );
                        if old_width != data.intrinsic_width || old_height != data.intrinsic_height
                        {
                            layout_changed = true;
                        }
                        paint_changed = true;
                    }
                } else if data.source_id != 0 || data.path != element.content {
                    // the file is loaded synchronously here; this runs
                    // during reconcile, never from `render`.
                    data.path = Str::copy(&element.content);
                    data.source_id = 0;
                    crate::components::image::load(data);
                    layout_changed = true;
                    paint_changed = true;
                }
                if data.fit != element.image_fit {
                    data.fit = element.image_fit;
                    paint_changed = true;
                }
            }
            crate::core::node::NodeKind::Toggle => {
                let label_changed = {
                    let NodeData::Toggle(data) = &self.tree.get(id).data else {
                        return layout_changed;
                    };
                    data.label != element.content
                };
                if label_changed {
                    if let NodeData::Toggle(data) = &mut self.tree.get_mut(id).data {
                        data.label = Str::copy(&element.content);
                        data.invalidate_layout();
                    }
                    layout_changed = true;
                    paint_changed = true;
                }
                let checked_changed;
                {
                    let node = self.tree.get_mut(id);
                    let NodeData::Toggle(data) = &mut node.data else {
                        return layout_changed;
                    };
                    if data.kind != element.toggle_kind {
                        data.kind = element.toggle_kind;
                        layout_changed = true;
                        paint_changed = true;
                    }
                    if data.font_size != element.text_style.font_size {
                        data.font_size = element.text_style.font_size;
                        data.invalidate_layout();
                        layout_changed = true;
                        paint_changed = true;
                    }
                    checked_changed = data.checked != element.checked;
                    if checked_changed {
                        data.checked = element.checked;
                        paint_changed = true;
                    }
                    data.action = element.on_bool.clone();
                    data.index_action = element.index_action.clone();
                    data.index = element.index;
                    node.focusable = true;
                }
                if checked_changed {
                    self.tree.poke(id);
                }
            }
            crate::core::node::NodeKind::Root => {}
        }

        if kind == crate::core::node::NodeKind::Container
            && self.reconcile_children(id, &element.children)
        {
            layout_changed = true;
        }
        if layout_changed {
            self.tree.mark_dirty(id, true, true);
        } else if paint_changed {
            self.tree.mark_dirty(id, false, true);
        }
        layout_changed
    }
}

/// Applies a controlled input selection when it changed.
/// Returns true when the input must repaint.
pub(crate) fn apply_selection(data: &mut InputData, selection: Option<[usize; 2]>) -> bool {
    let Some(selection) = selection else {
        return false;
    };
    if let Some(previous) = data.applied_selection
        && previous[0] == selection[0]
        && previous[1] == selection[1]
    {
        return false;
    }
    data.applied_selection = Some(selection);
    let len = data.text.len();
    data.anchor = clamp_boundary(&data.text, selection[0].min(len));
    data.cursor = clamp_boundary(&data.text, selection[1].min(len));
    data.reported_selection = [data.anchor, data.cursor];
    data.blink_on = true;
    data.preferred_x = None;
    data.reveal_caret = true;
    true
}

/// The largest char boundary `<= index`.
pub(crate) fn clamp_boundary(bytes: &str, index: usize) -> usize {
    let mut index = index.min(bytes.len());
    while index > 0 && !bytes.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// `renderer::text::Style` has no `PartialEq` (it is a plain option struct);
/// compare the fields the patch pass cares about.
fn text_style_equal(a: &text::Style, b: &text::Style) -> bool {
    a.font_size == b.font_size
        && a.weight == b.weight
        && a.text_align == b.text_align
        && a.wrap == b.wrap
        && a.line_height == b.line_height
        && a.color == b.color
        && a.max_lines == b.max_lines
        && a.family == b.family
}

/// Field-by-field, embed drawings by content hash.
fn rich_spans_equal(a: &[RichSpan], b: &[RichSpan]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (x, y) in a.iter().zip(b) {
        if x.font_size != y.font_size
            || x.weight != y.weight
            || x.color != y.color
            || x.italic != y.italic
            || x.family != y.family
            || x.background != y.background
            || x.underline != y.underline
            || x.strike != y.strike
            || x.text != y.text
            || x.link != y.link
        {
            return false;
        }
        match (x.copy.as_deref(), y.copy.as_deref()) {
            (Some(left), Some(right)) => {
                if left != right {
                    return false;
                }
            }
            (None, None) => {}
            _ => return false,
        }
        match (&x.embed, &y.embed) {
            (Some(left), Some(right)) => {
                if left.content_hash() != right.content_hash() {
                    return false;
                }
            }
            (None, None) => {}
            _ => return false,
        }
    }
    true
}

/// Whether the select already holds exactly `options`.
fn select_options_equal(data: &SelectData, options: &[Str]) -> bool {
    if data.options.len() != options.len() {
        return false;
    }
    data.options
        .iter()
        .zip(options)
        .all(|(old, new)| old.label.as_str() == new.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app;
    use crate::ui::builders::{column, image, text};
    use crate::ui::options::{BoxOptions, ImageOptions, TextOptions};
    use crate::ui::{Node, RunOptions};
    use std::rc::Rc;

    struct Doc {
        title: &'static str,
    }

    impl app::App for Doc {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![text(
                    self.title,
                    TextOptions {
                        key: Some("title".into()),
                        ..Default::default()
                    }
                )],
                ..Default::default()
            })
        }
    }

    fn title_node(runtime: &Runtime) -> &crate::core::node::RetainedNode {
        let id = runtime.tree.id_of_key("title").expect("the title node");
        runtime.tree.get(id)
    }

    #[test]
    fn patch_updates_text_content_in_place() {
        let mut runtime = Runtime::init(Doc { title: "one" }, RunOptions::default());
        runtime.rebuild_for_test();
        let id = runtime.tree.id_of_key("title").expect("title");
        runtime.app_state_mut::<Doc>().title = "two";
        runtime.rebuild_for_test();
        assert_eq!(runtime.tree.id_of_key("title"), Some(id), "reused node");
        match &title_node(&runtime).data {
            NodeData::Text(data) => assert_eq!(data.content, "two"),
            _ => panic!("expected a text node"),
        }
        // unchanged content on the next build does not dirty the layout again
        runtime.tree.clear_paint();
        runtime.tree.get_mut(runtime.tree.root).dirty.layout = false;
        runtime.rebuild_for_test();
        assert!(!runtime.tree.needs_layout(), "content did not change");
    }

    #[test]
    fn apply_selection_clamps_to_char_boundaries_and_skips_unchanged_values() {
        let mut data = InputData {
            text: String::from("héllo"),
            ..Default::default()
        };
        assert!(apply_selection(&mut data, Some([0, 6])));
        assert_eq!(data.cursor, 6, "len is 6 bytes");
        // "é" is two bytes: a selection in the middle of it moves back
        assert!(!apply_selection(&mut data, Some([0, 6])), "unchanged");
        assert!(apply_selection(&mut data, Some([2, 3])));
        // "héllo": é occupies bytes 1-2, so byte 2 clamps back to 1 while
        // byte 3 (the "l") is a boundary
        assert_eq!(data.anchor, 1, "byte 2 is inside é");
        assert_eq!(data.cursor, 3);
        assert!(data.reveal_caret);
        assert!(data.blink_on);
        assert!(!apply_selection(&mut data, None));
    }

    #[test]
    fn rich_spans_compare_field_by_field() {
        let base = RichSpan {
            text: Str::from("hello"),
            ..Default::default()
        };
        assert!(rich_spans_equal(
            std::slice::from_ref(&base),
            std::slice::from_ref(&base)
        ));
        let mut changed = base.clone();
        changed.font_size = 99.0;
        assert!(!rich_spans_equal(
            std::slice::from_ref(&base),
            std::slice::from_ref(&changed)
        ));
        let mut linked = base.clone();
        linked.link = Str::from("https://example.com");
        assert!(!rich_spans_equal(
            std::slice::from_ref(&base),
            std::slice::from_ref(&linked)
        ));
        assert!(!rich_spans_equal(std::slice::from_ref(&base), &[]));
    }

    // ── image loading in the reconcile / patch arms ─────────────────────────

    struct Picture {
        path: String,
        image: Option<Rc<crate::image::Image>>,
    }

    impl app::App for Picture {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![image(ImageOptions {
                    key: Some("pic".into()),
                    src: Str::copy(&self.path),
                    image: self.image.clone(),
                    ..Default::default()
                })],
                ..Default::default()
            })
        }
    }

    fn image_state(runtime: &Runtime) -> &crate::core::node::ImageData {
        let id = runtime.find_key("pic").expect("image node");
        match &runtime.tree.get(id).data {
            NodeData::Image(data) => data,
            _ => panic!("expected an image node"),
        }
    }

    #[test]
    fn the_image_arms_load_paths_and_in_memory_sources() {
        // a 40x20 PNG on disk
        let pixels = vec![255u8; (40 * 20 * 4) as usize];
        let source =
            crate::image::Image::from_pixels(&pixels, 40, 20, 40 * 4).expect("image pixels");
        let png = source.encode_png().expect("png");
        let path =
            std::env::temp_dir().join(format!("zant_wp21_patch_image_{}.png", std::process::id()));
        std::fs::write(&path, &png).expect("write png");
        let real_path = path.to_str().expect("utf-8 path").to_string();
        let mut runtime = Runtime::init(
            Picture {
                path: String::from("this/image/does/not/exist.png"),
                image: None,
            },
            RunOptions::default(),
        );
        // a created image node with an unreadable path stays a placeholder
        runtime.rebuild_for_test();
        {
            let data = image_state(&runtime);
            assert!(data.handle.is_none());
            assert_eq!((data.intrinsic_width, data.intrinsic_height), (0, 0));
        }
        // the path changes: the patch arm loads it and
        // its intrinsic size re-lays out the box
        runtime.app_state_mut::<Picture>().path = real_path.clone();
        runtime.rebuild_for_test();
        {
            let data = image_state(&runtime);
            assert!(data.handle.is_some(), "the png was decoded");
            assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
        }
        assert!(
            runtime.tree.needs_layout(),
            "a new intrinsic size dirties the layout"
        );
        // an in-memory source goes through `set_source`
        runtime.app_state_mut::<Picture>().image = Some(Rc::new(source));
        runtime.rebuild_for_test();
        {
            let data = image_state(&runtime);
            assert!(data.handle.is_some());
            assert_eq!(
                data.source_id,
                data.handle.as_ref().map_or(0, |image| image.id)
            );
            assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
        }
        // switching back to the path loads the file again
        runtime.app_state_mut::<Picture>().image = None;
        runtime.rebuild_for_test();
        {
            let data = image_state(&runtime);
            assert_eq!(data.source_id, 0);
            assert!(data.handle.is_some());
            assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
        }
        std::fs::remove_file(&path).expect("remove png");
    }
}
