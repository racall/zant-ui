//! The antd composite components:
//! icons, dropdown menu, segmented, dismiss layer, modal, shortcut list,
//! message, focus scope, clickable rows, hotkey recorder, image thumbnails and
//! button icon metrics.

use zant::components::hotkey_input::{
    HotkeyAction, HotkeyInputOptions, Shortcut as HotkeyShortcut, hotkey_input,
};
use zant::components::menu::{MenuItem, MenuOptions, dropdown_menu};
use zant::components::message::{MessageKind, MessageOptions, message};
use zant::components::modal::{ModalOptions, modal};
use zant::components::segmented::{SegmentedOption, SegmentedOptions, segmented};
use zant::components::shortcut_list::shortcut_list;
use zant::components::shortcut_list::{
    Shortcut as ShortcutItem, ShortcutGroup, ShortcutListOptions,
};
use zant::core::events::{Event, Key, KeyEvent, Modifiers};
use zant::core::node::{ButtonType, FlexStyle, NodeId};
use zant::core::tree::Tree;
use zant::image::{Image, NATIVE_ORDER, PixelOrder};
use zant::motion::motion::{self, Presence};
use zant::renderer::renderer::Renderer;
use zant::runtime::Runtime;
use zant::theme::tokens::ant_light;
use zant::ui::actions::{action, index_action};
use zant::ui::app::{App, RunOptions};
use zant::ui::element::Node;
use zant::ui::options::{
    BoxOptions, ButtonOptions, ClickableOptions, DismissLayerOptions, IconButtonOptions,
    IconOptions, TextOptions, ToggleOptions,
};
use zant::ui::{Icon, button, clickable, column, dismiss_layer, icon, icon_button, row};

mod dialogs;
mod menus;
mod rows;

use self::dialogs::*;
