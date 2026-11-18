//! `zant-demo`: the declarative API demo.
//!
//! A settings form (input, checkbox, switch, radio group, select, buttons
//! inside a card), a preview card with tabs and an alert, an image and a
//! tooltip.
//!
//! * The image is decoded by the app on the first rebuild
//!   (`zant::image::Image`) and passed as `ImageOptions::image`, the in-memory
//!   alternative to `src`. The asset is resolved relative to the working
//!   directory.
//! * The name lives in a 96-byte buffer and is truncated by raw bytes; a cut
//!   multi-byte character renders through `String::from_utf8_lossy`.
#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::rc::Rc;

use zant::Str;
use zant::core::geometry::Edges;
use zant::core::node::{ButtonType, ImageFit};
use zant::flex;
use zant::image::Image;
use zant::platform::platform::WindowOptions;
use zant::renderer::text::Weight;
use zant::theme::tokens::Theme;
use zant::ui::{
    self, AlertOptions, AlertType, BoxOptions, ButtonOptions, CardOptions, DividerOptions,
    ImageOptions, InputOptions, ParagraphOptions, RadioGroupOptions, RadioOption, RunOptions,
    SelectOptions, SpaceOptions, TabItem, TabsOptions, TextOptions, ToggleOptions, TooltipOptions,
};

#[cfg(test)]
mod tests;

const PLANS: [&str; 3] = ["Starter", "Pro", "Enterprise"];

const THEMES: [&str; 3] = ["Light", "System", "Dark"];

/// Path of the demo's image, relative to the working directory.
const IMAGE_SRC: &str = "crates/zant-examples/assets/zant.png";

/// Capacity of the name buffer in bytes.
const NAME_CAPACITY: usize = 96;

/// The whole application state of the demo.
struct State {
    name_buf: [u8; NAME_CAPACITY],
    name_len: usize,
    notifications: bool,
    beta: bool,
    theme_choice: usize,
    plan: usize,
    tab: usize,
    saved: u32,
    /// `ImageOptions.image` of the image node (see the module docs).
    image: Option<Rc<Image>>,
    image_loaded: bool,
}

impl State {
    fn init() -> State {
        let mut state = State {
            name_buf: [0; NAME_CAPACITY],
            name_len: 0,
            notifications: true,
            beta: false,
            theme_choice: 0,
            plan: 1,
            tab: 0,
            saved: 0,
            image: None,
            image_loaded: false,
        };
        state.set_name("Zant UI");
        state
    }

    /// The name as text (lossy when the truncation cut a character).
    fn name(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.name_buf[..self.name_len])
    }

    fn set_name(&mut self, value: &str) {
        let length = value.len().min(NAME_CAPACITY);
        self.name_buf[..length].copy_from_slice(&value.as_bytes()[..length]);
        self.name_len = length;
    }

    /// Input callback: truncates to the buffer (raw bytes).
    fn on_name(&mut self, value: &str) {
        self.set_name(value);
    }

    fn set_notifications(&mut self, checked: bool) {
        self.notifications = checked;
    }

    fn set_beta(&mut self, checked: bool) {
        self.beta = checked;
    }

    fn set_theme(&mut self, index: usize) {
        self.theme_choice = index;
    }

    fn set_plan(&mut self, index: usize) {
        self.plan = index;
    }

    fn set_tab(&mut self, index: usize) {
        self.tab = index;
    }

    fn save(&mut self) {
        self.saved += 1;
    }

    /// The demo image, decoded on the first rebuild (see the module docs).
    fn image(&mut self) -> Option<Rc<Image>> {
        if !self.image_loaded {
            self.image_loaded = true;
            self.image = load_image(IMAGE_SRC).map(Rc::new);
        }
        self.image.clone()
    }

    /// The whole window tree.
    fn render(&mut self) -> ui::Node {
        let plan_options: Vec<Str> = PLANS.iter().map(|plan| Str::from(*plan)).collect();
        let theme_options: Vec<RadioOption> = THEMES
            .iter()
            .map(|label| RadioOption {
                label: Str::from(*label),
                disabled: false,
            })
            .collect();

        let overview = ui::column(BoxOptions {
            gap: 14.0,
            align_items: flex::Align::Start,
            children: vec![
                ui::tooltip(TooltipOptions {
                    text: "Tooltip is rendered by Skia above the UI tree".into(),
                    child: ui::button(ButtonOptions {
                        text: "Hover me".into(),
                        variant: ButtonType::Default,
                        ..Default::default()
                    }),
                }),
                ui::image(ImageOptions {
                    src: Str::from(IMAGE_SRC),
                    image: self.image(),
                    width: Some(280.0),
                    height: Some(140.0),
                    fit: ImageFit::Cover,
                    radius: 10.0,
                    ..Default::default()
                }),
                ui::text(
                    "Live preview",
                    TextOptions {
                        size: 16.0,
                        weight: Weight::Semibold,
                        ..Default::default()
                    },
                ),
                ui::text(
                    Str::copy(&self.name()),
                    TextOptions {
                        size: 22.0,
                        weight: Weight::Semibold,
                        color: Some(0xff1677ff),
                        ..Default::default()
                    },
                ),
                ui::paragraph(
                    "A declarative UI tree built with ui.column, ui.input, ui.radioGroup, ui.select, ui.tabs and ui.image.",
                    ParagraphOptions::default(),
                ),
            ],
            ..Default::default()
        });

        let details = ui::column(BoxOptions {
            gap: 10.0,
            children: vec![
                ui::text(
                    "Component state",
                    TextOptions {
                        size: 16.0,
                        weight: Weight::Semibold,
                        ..Default::default()
                    },
                ),
                ui::text(
                    "Notifications",
                    TextOptions {
                        size: 13.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::text(
                    if self.notifications { "On" } else { "Off" },
                    TextOptions {
                        size: 13.0,
                        ..Default::default()
                    },
                ),
                ui::text(
                    "Beta updates",
                    TextOptions {
                        size: 13.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::text(
                    if self.beta { "Enabled" } else { "Disabled" },
                    TextOptions {
                        size: 13.0,
                        ..Default::default()
                    },
                ),
                ui::text(
                    "Theme",
                    TextOptions {
                        size: 13.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::text(
                    THEMES[self.theme_choice],
                    TextOptions {
                        size: 13.0,
                        ..Default::default()
                    },
                ),
                ui::text(
                    "Plan",
                    TextOptions {
                        size: 13.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::text(
                    PLANS[self.plan],
                    TextOptions {
                        size: 13.0,
                        ..Default::default()
                    },
                ),
                ui::text(
                    "Saves",
                    TextOptions {
                        size: 13.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::text(
                    format!("{}", self.saved),
                    TextOptions {
                        size: 13.0,
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        });

        let tab_items = vec![
            TabItem {
                label: "Overview".into(),
                content: overview,
            },
            TabItem {
                label: "Details".into(),
                content: details,
            },
        ];

        let left_card = ui::card(CardOptions {
            title: Some("Settings".into()),
            width: Some(486.0),
            children: vec![
                ui::text(
                    "Project name",
                    TextOptions {
                        size: 12.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::input(InputOptions {
                    value: Some(Str::copy(&self.name())),
                    placeholder: "Project name".into(),
                    on_change: Some(ui::text_action(|state: &mut State, value| {
                        state.on_name(value)
                    })),
                    ..Default::default()
                }),
                ui::space(SpaceOptions {
                    gap: 20.0,
                    children: vec![
                        ui::checkbox(ToggleOptions {
                            label: "Notifications".into(),
                            checked: self.notifications,
                            on_change: Some(ui::bool_action(|state: &mut State, checked| {
                                state.set_notifications(checked)
                            })),
                            ..Default::default()
                        }),
                        ui::r#switch(ToggleOptions {
                            label: "Beta updates".into(),
                            checked: self.beta,
                            on_change: Some(ui::bool_action(|state: &mut State, checked| {
                                state.set_beta(checked)
                            })),
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                }),
                ui::divider(DividerOptions {
                    margin: ui::inset_xy(0.0, 4.0),
                    ..Default::default()
                }),
                ui::text(
                    "Theme",
                    TextOptions {
                        size: 13.0,
                        weight: Weight::Medium,
                        ..Default::default()
                    },
                ),
                ui::radio_group(RadioGroupOptions {
                    key: Some("theme".into()),
                    options: theme_options,
                    selected: self.theme_choice,
                    direction: flex::Direction::Row,
                    gap: 20.0,
                    on_change: Some(ui::index_action(|state: &mut State, index| {
                        state.set_theme(index)
                    })),
                }),
                ui::text(
                    "Plan",
                    TextOptions {
                        size: 13.0,
                        weight: Weight::Medium,
                        ..Default::default()
                    },
                ),
                ui::select(SelectOptions {
                    options: plan_options,
                    selected: self.plan,
                    width: Some(220.0),
                    on_change: Some(ui::index_action(|state: &mut State, index| {
                        state.set_plan(index)
                    })),
                    ..Default::default()
                }),
                ui::space(SpaceOptions {
                    gap: 10.0,
                    children: vec![
                        ui::button(ButtonOptions {
                            text: "Save Changes".into(),
                            variant: ButtonType::Primary,
                            on_click: Some(ui::action(|state: &mut State| state.save())),
                            ..Default::default()
                        }),
                        ui::button(ButtonOptions {
                            text: "Cancel".into(),
                            variant: ButtonType::Default,
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });

        let right_card = ui::card(CardOptions {
            title: Some("Preview".into()),
            width: Some(326.0),
            children: vec![
                ui::tabs(TabsOptions {
                    key: Some("preview-tabs".into()),
                    items: tab_items,
                    active: self.tab,
                    on_change: Some(ui::index_action(|state: &mut State, index| {
                        state.set_tab(index)
                    })),
                }),
                ui::alert(AlertOptions {
                    r#type: AlertType::Info,
                    title: "Declarative components".into(),
                    description: "State changes reconcile only the affected nodes; editing an input does not rebuild layout.".into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        });

        ui::column(BoxOptions {
            padding: Edges {
                left: 32.0,
                right: 32.0,
                top: 28.0,
                bottom: 28.0,
            },
            gap: 18.0,
            children: vec![
                ui::text(
                    "Zant UI",
                    TextOptions {
                        size: 30.0,
                        weight: Weight::Semibold,
                        ..Default::default()
                    },
                ),
                ui::text(
                    "Declarative native components powered by Rust + Skia",
                    TextOptions {
                        size: 14.0,
                        color: Some(0xa6000000),
                        ..Default::default()
                    },
                ),
                ui::row(BoxOptions {
                    align_items: flex::Align::Start,
                    gap: 18.0,
                    children: vec![left_card, right_card],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        })
    }
}

impl ui::App for State {
    fn render(&mut self) -> ui::Node {
        State::render(self)
    }
}

/// Window title, size and theme of the demo.
fn run_options() -> RunOptions {
    RunOptions {
        title: String::from("Zant UI — Declarative macOS"),
        window: WindowOptions {
            width: 900.0,
            height: 700.0,
            ..Default::default()
        },
        theme: Theme::default(),
        ..Default::default()
    }
}

/// Decodes the demo image from its working-directory-relative path (run the
/// demo from the repository root so it resolves).
fn load_image(src: &str) -> Option<Image> {
    let bytes = std::fs::read(src).ok()?;
    Image::from_encoded(&bytes)
}

fn main() -> zant::Result<()> {
    ui::run(State::init(), run_options())
}
