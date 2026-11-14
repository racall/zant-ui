//! Declarative UI contract: elements, options, callbacks and builders.
//!
//! Port of `src/ui.zig` — WP-1.2 (the frozen API of docs/rust-port/api.md).
//! WP-2.4 added the memo / portal / command builders (`memo`, `portals`,
//! `commands`) and the local-state wrappers.
//!
//! Type ownership: the option structs live in [`options`], the node/element
//! pair and the [`nodes!`] macro in [`element`], callbacks in [`actions`],
//! `styled()` in [`styled`], the build context in [`context`] and the
//! `column` / `row` / `text` / `button` builders in [`builders`].

pub mod actions;
pub mod app;
pub mod builders;
pub mod commands;
pub mod context;
pub mod element;
pub mod local_state;
pub mod memo;
pub mod options;
pub mod portals;
pub mod styled;

pub use actions::{
    Action, ActionCx, BoolAction, IndexAction, InputKeyAction, KeyAction, PointerAction,
    PointerInfo, PointerPhase, RangeAction, TextAction, action, bool_action, call, call_bool,
    call_index, call_range, call_text, index_action, range_action, text_action,
};
pub use app::{App, RunOptions, run};
pub use builders::{button, column, row, text};
pub use commands::{
    Command, CommandKind, MAX_COMMANDS, focus, queue_command_for, requeue_command,
    scroll_into_view, scroll_to_end, set_command_owner, take_commands, take_commands_for,
};
pub use context::{
    BuildCx, begin_frame, build_states, current_theme, display_scale, document_edited, effect,
    end_frame, frame_string, is_frame_active, key_bounds, now_ms, post_wake, push_portal,
    request_rebuild, run_effects, set_build_states, set_build_theme, set_build_window_style,
    set_display_scale, set_key_bounds_hook, set_now, take_document_edited, take_portals, take_wake,
    titlebar_insets, wake_after, with_cx,
};
pub use element::{Element, IntoNodes, MemoBuild, Node, append_children, with_key};
pub use local_state::{child_id, local_slot, local_state};
pub use memo::{memo, memo_build, memo_hash, resolve_memo};
pub use portals::portal;

/// Zig `ui.LocalSlot` / `ui.LocalStateStore`: the local-state types the runtime
/// holds (`core::local_state`).
pub use crate::core::local_state::{Slot as LocalSlot, Store as LocalStateStore};
pub use options::{
    AlertOptions, AlertType, AvatarOptions, BoxOptions, ButtonOptions, CardOptions,
    ClickableOptions, DismissLayerOptions, DividerOptions, HeadingOptions, IconButtonOptions,
    IconOptions, ImageOptions, InputOptions, LinkOptions, MemoOptions, ParagraphOptions,
    PdfOptions, PointerAreaOptions, ProgressOptions, RadioGroupOptions, RadioOption,
    RichTextOptions, SelectOptions, SpaceOptions, TabItem, TabsOptions, TagOptions, TextOptions,
    ToggleOptions, TooltipOptions, VectorOptions,
};
pub use styled::{Style, inset, inset_xy, styled};

pub use crate::core::node::Icon;
pub use crate::platform::platform::Cursor;

#[cfg(test)]
mod contract_tests {
    //! The call shapes frozen by docs/rust-port/api.md §4, as the TSX codegen
    //! will emit them (state closures, `nodes!` children, `styled`).

    use super::*;
    use crate::Str;
    use crate::core::node::NodeKind;

    #[derive(Default)]
    struct Counter {
        count: u32,
        name: Str,
        notify: bool,
    }

    impl Counter {
        fn _set_name(&mut self, value: &str) {
            self.name = Str::copy(value);
        }

        fn _on_click1(&mut self) {
            self.count = 0;
        }

        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![
                    button(ButtonOptions {
                        key: Some("reset".into()),
                        text: "Reset".into(),
                        disabled: self.count == 0,
                        on_click: Some(action(|s: &mut Counter| s._on_click1())),
                        ..Default::default()
                    }),
                    row(BoxOptions {
                        gap: 8.0,
                        children: crate::nodes![
                            text("Notifications are on", TextOptions::default()),
                            if self.notify {
                                Some(text("on", TextOptions::default()))
                            } else {
                                None
                            },
                        ],
                        ..Default::default()
                    }),
                    styled(
                        text("Save", TextOptions::default()),
                        Style {
                            width: Some(120.0),
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            })
        }
    }

    #[test]
    fn the_plan_call_shapes_compile_and_dispatch() {
        let mut state = Counter::default();
        let node = state.render();
        assert_eq!(node.element().kind, NodeKind::Container);
        assert_eq!(node.element().children.len(), 3);
        // callbacks mutate the root state through `ActionCx`
        let mut cx = ActionCx::new(&mut state);
        text_action(|s: &mut Counter, value| s._set_name(value)).call(&mut cx, "Zant");
        action(|s: &mut Counter| s._on_click1()).call(&mut cx);
        assert_eq!(state.name, "Zant");
        assert_eq!(state.count, 0);
        assert!(!state.notify);
    }
}
