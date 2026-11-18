//! The tag order of the table and the builders the table skips, with the reason.

use super::*;

/// Every `ui.*` builder in the order of the generated table. Reflected tags
/// and [`pending`] tags together fill it.
pub(super) const TAG_ORDER: &[&str] = &[
    "column",
    "row",
    "clickable",
    "space",
    "text",
    "heading",
    "title",
    "paragraph",
    "button",
    "link",
    "input",
    "checkbox",
    "switch",
    "radio_group",
    "tabs",
    "tooltip",
    "image",
    "select",
    "icon",
    "icon_button",
    "dismiss_layer",
    "rich_text",
    "pdf_preview",
    "divider",
    "tag",
    "card",
    "alert",
    "avatar",
    "progress",
    "dropdown_menu",
    "segmented",
    "modal",
    "shortcut_list",
    "message",
    "hotkey_input",
    "view",
    "markdown",
    "pdf_viewer",
];

/// Builders intentionally excluded from the generated table: adapters,
/// runtime-only builders and the composite / viewer builders that take
/// non-option types.
pub(crate) const SKIPPED: &[Skipped] = &[
    Skipped {
        builder: "ui.portal",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.appendChildren",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.styled",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.pointerArea",
        reason: "options have unsupported required fields (see omitted props)",
    },
    Skipped {
        builder: "ui.textFmt",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.withKey",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.vector",
        reason: "options have unsupported required fields (see omitted props)",
    },
    Skipped {
        builder: "ui.memo",
        reason: "adapter / generic helper, not an options builder",
    },
    Skipped {
        builder: "ui.windowControls",
        reason: "runtime-only window controls",
    },
    Skipped {
        builder: "ui.list",
        reason: "List uses *ListState and a compiler special case",
    },
];

pub(super) fn skipped() -> &'static [Skipped] {
    SKIPPED
}
