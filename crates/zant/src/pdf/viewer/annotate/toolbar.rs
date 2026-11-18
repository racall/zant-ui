//! The annotation API on `Viewer` and the tool switch of the toolbar.

use super::*;

impl Viewer {
    /// Default colour of an annotation kind.
    pub fn default_color(kind: Kind) -> u32 {
        match kind {
            Kind::Highlight => annotations::colors::YELLOW,
            Kind::Underline | Kind::Squiggly => BLUE,
            Kind::Strikeout | Kind::Ink => annotations::colors::RED,
            Kind::Note => annotations::colors::NOTE,
        }
    }

    /// Node key of an annotation's box (for `ui::key_bounds`, e.g. to place
    /// a comment editor next to the selected annotation).
    pub fn annotation_key(key_prefix: &str, id: u64) -> String {
        format!("{key_prefix}-a{id}")
    }

    /// Switches the pointer tool (clears the text selection).
    pub fn set_tool(&self, tool: Tool) {
        self.borrow_mut().set_tool(tool);
    }

    /// The current pointer tool.
    pub fn tool(&self) -> Tool {
        self.borrow().tool
    }

    /// Colour for new annotations of the current tool (`None`: default).
    pub fn set_tool_color(&self, color: Option<u32>) {
        self.borrow_mut().tool_color = color;
    }

    /// Colour new annotations of the current tool get (`None`: the default).
    pub fn tool_color(&self) -> Option<u32> {
        self.borrow().tool_color
    }

    /// The selected annotation.
    pub fn selected_annotation(&self) -> Option<Annotation> {
        self.borrow().selected_annotation().cloned()
    }

    /// Id of the selected annotation.
    pub fn selected_annotation_id(&self) -> Option<u64> {
        self.borrow().selected_annotation
    }

    /// Selects an annotation (`None` clears the selection).
    pub fn select_annotation(&self, id: Option<u64>) {
        self.borrow_mut().select_annotation(id);
    }

    /// The annotation list (read access; it is the viewer's undo history).
    pub fn annotations(&self) -> std::cell::Ref<'_, annotations::Store> {
        std::cell::Ref::map(self.borrow(), |inner| &inner.annotations)
    }

    /// The annotation list (write access, e.g. `annotations_mut().load(list)`
    /// to restore a saved list).
    pub fn annotations_mut(&self) -> std::cell::RefMut<'_, annotations::Store> {
        std::cell::RefMut::map(self.borrow_mut(), |inner| &mut inner.annotations)
    }

    /// Turns the text selection into markup annotations (one per page);
    /// returns the id of the last one.
    pub fn annotate_selection(&self, kind: Kind, color: Option<u32>) -> Option<u64> {
        self.borrow_mut().annotate_selection(kind, color)
    }

    pub fn remove_annotation(&self, id: u64) {
        self.borrow_mut().remove_annotation(id);
    }

    pub fn set_annotation_contents(&self, id: u64, contents: &str) {
        self.borrow_mut().set_annotation_contents(id, contents);
    }

    pub fn set_annotation_color(&self, id: u64, color: u32) {
        self.borrow_mut().set_annotation_color(id, color);
    }

    pub fn undo(&self) -> bool {
        self.borrow_mut().undo()
    }

    pub fn redo(&self) -> bool {
        self.borrow_mut().redo()
    }

    /// Writes a copy of the open PDF with the annotations to `out_path`
    /// (standard PDF annotations other viewers show and edit).
    pub fn save_annotated(&self, out_path: &str) -> Result<(), SaveError> {
        self.borrow().save_annotated(out_path)
    }
}

/// Annotation tool switch of the toolbar: a segmented
/// control with the select / highlight / underline / pen / note tools, then
/// undo and redo.
pub(crate) fn tool_switch(inner: &Inner, viewer: &Viewer, options: &ViewOptions) -> Node {
    let labels = &options.labels;
    let selected: usize = match inner.tool {
        Tool::Select => 0,
        Tool::Highlight => 1,
        Tool::Underline | Tool::Strikeout | Tool::Squiggly => 2,
        Tool::Ink => 3,
        Tool::Note => 4,
    };
    let mut children: Vec<Node> = Vec::new();
    children.push(styled(
        space(SpaceOptions {
            width: Some(8.0),
            ..Default::default()
        }),
        Style::default(),
    ));
    let segmented_viewer = viewer.clone();
    children.push(ui::segmented(SegmentedOptions {
        key: Some(Str::from(format!("{}-tools", options.key))),
        options: vec![
            SegmentedOption {
                label: labels.select.clone(),
                ..Default::default()
            },
            SegmentedOption {
                label: labels.highlight.clone(),
                ..Default::default()
            },
            SegmentedOption {
                label: labels.underline.clone(),
                ..Default::default()
            },
            SegmentedOption {
                label: labels.pen.clone(),
                icon: Icon::Pencil,
                ..Default::default()
            },
            SegmentedOption {
                label: labels.note.clone(),
                icon: Icon::FileText,
                ..Default::default()
            },
        ],
        selected,
        on_change: Some(IndexAction::new(move |cx, index| {
            tb_tool(&segmented_viewer, index);
            segmented_viewer.flush_callbacks(cx);
        })),
        height: 24.0,
        font_size: 12.0,
        ..Default::default()
    }));
    let undo_viewer = viewer.clone();
    children.push(icon_button(IconButtonOptions {
        key: Some(Str::from(format!("{}-undo", options.key))),
        icon: Icon::Undo,
        size: 16.0,
        r#box: 28.0,
        disabled: !inner.annotations.can_undo(),
        on_click: Some(Action::new(move |cx| {
            tb_undo(&undo_viewer);
            undo_viewer.flush_callbacks(cx);
        })),
        ..Default::default()
    }));
    let redo_viewer = viewer.clone();
    children.push(icon_button(IconButtonOptions {
        key: Some(Str::from(format!("{}-redo", options.key))),
        icon: Icon::Redo,
        size: 16.0,
        r#box: 28.0,
        disabled: !inner.annotations.can_redo(),
        on_click: Some(Action::new(move |cx| {
            tb_redo(&redo_viewer);
            redo_viewer.flush_callbacks(cx);
        })),
        ..Default::default()
    }));
    row(BoxOptions {
        gap: 4.0,
        align_items: flex::Align::Center,
        children,
        ..Default::default()
    })
}

pub(super) fn tb_tool(viewer: &Viewer, index: usize) {
    let tools = [
        Tool::Select,
        Tool::Highlight,
        Tool::Underline,
        Tool::Ink,
        Tool::Note,
    ];
    let mut inner = viewer.borrow_mut();
    inner.set_tool(tools[index.min(tools.len() - 1)]);
    inner.changed();
}
