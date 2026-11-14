//! `zant-demo`: port of the Zig `zant-demo` example, `examples/declarative.zig`
//! (the `zant-demo` executable target of `build.zig`).
//!
//! The demo is the declarative M1 proof: a settings form (input, checkbox,
//! switch, radio group, select, buttons inside a card), a preview card with
//! tabs and an alert, an image and a tooltip — the same window title and size,
//! the same state and callbacks and the same rendered tree as the Zig version.
//!
//! Deviations from the Zig source (see the WP-1.5 report):
//!
//! * The Rust runtime does not load `ui.image` sources given as file paths yet
//!   (`runtime/patch.rs` records the path but never calls
//!   `components::image::load`; that wiring is still missing after WP-2.2), so
//!   this demo decodes the same asset on the first rebuild with the ported
//!   `zant::image::Image` and passes `ImageOptions::image`, which the builder
//!   documents as the in-memory alternative to `src`. The loaded bytes and the
//!   `zant_canvas_draw_image_rect` paint call are the same ones the Zig app
//!   uses, so the rendered pixels match. Like the Zig demo, the asset is
//!   resolved relative to the working directory.
//! * Zig keeps the name in a 96-byte buffer and truncates raw bytes; the port
//!   keeps the same `[u8; 96]` buffer (non-UTF-8 leftovers render through
//!   `String::from_utf8_lossy`, Rust needs valid UTF-8 text).
//! * `ui.textFmt(...)` has no Rust counterpart (see `ui/builders/typography.rs`);
//!   the call site formats with `format!` and passes the string.
#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::rc::Rc;

use zant::Str;
use zant::core::geometry::Edges;
use zant::core::node::{ButtonType, ImageFit};
use zant::image::Image;
use zant::layout::flex;
use zant::platform::platform::WindowOptions;
use zant::renderer::text::Weight;
use zant::theme::tokens::Theme;
use zant::ui::{
    self, AlertOptions, AlertType, BoxOptions, ButtonOptions, CardOptions, DividerOptions,
    ImageOptions, InputOptions, ParagraphOptions, RadioGroupOptions, RadioOption, RunOptions,
    SelectOptions, SpaceOptions, TabItem, TabsOptions, TextOptions, ToggleOptions, TooltipOptions,
};

const PLANS: [&str; 3] = ["Starter", "Pro", "Enterprise"];
const THEMES: [&str; 3] = ["Light", "System", "Dark"];
/// Zig `ImageOptions.src` of the demo's image.
const IMAGE_SRC: &str = "crates/zant-examples/assets/zant.png";
/// Zig `State.name_buf` length.
const NAME_CAPACITY: usize = 96;

/// Zig `State`: the whole application state of the demo.
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
    /// Zig `State.init`.
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

    /// Zig `State.name`.
    fn name(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.name_buf[..self.name_len])
    }

    fn set_name(&mut self, value: &str) {
        let length = value.len().min(NAME_CAPACITY);
        self.name_buf[..length].copy_from_slice(&value.as_bytes()[..length]);
        self.name_len = length;
    }

    /// Zig `State.onName`: truncate to the buffer (raw bytes, like Zig).
    fn on_name(&mut self, value: &str) {
        self.set_name(value);
    }

    /// Zig `State.setNotifications`.
    fn set_notifications(&mut self, checked: bool) {
        self.notifications = checked;
    }

    /// Zig `State.setBeta`.
    fn set_beta(&mut self, checked: bool) {
        self.beta = checked;
    }

    /// Zig `State.setTheme`.
    fn set_theme(&mut self, index: usize) {
        self.theme_choice = index;
    }

    /// Zig `State.setPlan`.
    fn set_plan(&mut self, index: usize) {
        self.plan = index;
    }

    /// Zig `State.setTab`.
    fn set_tab(&mut self, index: usize) {
        self.tab = index;
    }

    /// Zig `State.save`.
    fn save(&mut self) {
        self.saved += 1;
    }

    /// The demo image, decoded on the first rebuild (Zig `components/image.zig`
    /// loaded it during reconcile; the Rust runtime's file loader is not wired
    /// yet, see the module docs).
    fn image(&mut self) -> Option<Rc<Image>> {
        if !self.image_loaded {
            self.image_loaded = true;
            self.image = load_image(IMAGE_SRC).map(Rc::new);
        }
        self.image.clone()
    }

    /// Zig `State.render`.
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
                    "Declarative native components powered by Zig + Skia",
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

/// Zig `Runtime.Config` of the demo.
fn run_options() -> RunOptions {
    RunOptions {
        title: String::from("Zant UI — Declarative macOS"),
        window: WindowOptions {
            width: 900.0,
            height: 700.0,
            ..Default::default()
        },
        theme: Theme::default(),
    }
}

/// Decodes the demo image from its working-directory-relative path, like the
/// Zig demo (`zant_image_load_file`). The parity capture runs from the
/// repository root, so the relative path resolves there for both binaries.
fn load_image(src: &str) -> Option<Image> {
    let bytes = std::fs::read(src).ok()?;
    Image::from_encoded(&bytes)
}

fn main() -> zant::Result<()> {
    ui::run(State::init(), run_options())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zant::core::events::{Event, PointerEvent};
    use zant::core::node::{NodeData, NodeId};
    use zant::renderer::renderer::Renderer;
    use zant::runtime::Runtime;

    const WIDTH: f32 = 900.0;
    const HEIGHT: f32 = 700.0;

    /// The keyed in-flow boxes the Zig `zant-demo` writes to
    /// `ZANT_LAYOUT_FILE` at every window size (anchored to the top-left, so
    /// the keys are the stable part of the comparison).
    const ZIG_KEYS: [&str; 8] = [
        "theme",
        "theme-0",
        "theme-1",
        "theme-2",
        "preview-tabs",
        "preview-tabs-0",
        "preview-tabs-1",
        "preview-tabs-ink",
    ];

    fn frame(runtime: &mut Runtime, renderer: &mut Renderer) {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        runtime.render_frame(canvas, WIDTH, HEIGHT, 1.0);
        renderer.end();
    }

    /// `State::init` normally decodes `crates/zant-examples/assets/zant.png`
    /// relative to
    /// the working directory (the repository root when the demo runs). Unit
    /// tests run from the crate directory, so they decode the same file from
    /// the manifest path explicitly.
    fn demo_state() -> State {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/zant.png");
        let image = std::fs::read(path)
            .ok()
            .and_then(|bytes| Image::from_encoded(&bytes))
            .map(Rc::new);
        State {
            image,
            image_loaded: true,
            ..State::init()
        }
    }

    fn demo() -> (Runtime, Renderer) {
        let mut runtime = Runtime::init(demo_state(), run_options());
        let mut renderer =
            Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
        frame(&mut runtime, &mut renderer);
        (runtime, renderer)
    }

    fn bounds(runtime: &Runtime, key: &str) -> zant::core::geometry::Rect {
        runtime.tree.get(runtime.find_key(key).expect(key)).bounds
    }

    fn find_button(runtime: &Runtime, label: &str) -> NodeId {
        for node in &runtime.tree.nodes {
            if node.alive
                && let NodeData::Button(data) = &node.data
                && data.label == label
            {
                return node.id;
            }
        }
        panic!("no button {label:?}");
    }

    fn find_input(runtime: &Runtime) -> NodeId {
        for node in &runtime.tree.nodes {
            if node.alive && matches!(node.data, NodeData::Input(_)) {
                return node.id;
            }
        }
        panic!("no input");
    }

    fn click(runtime: &mut Runtime, id: NodeId) {
        let box_ = runtime.tree.get(id).bounds;
        click_at(
            runtime,
            box_.x + box_.width * 0.5,
            box_.y + box_.height * 0.5,
        );
    }

    fn click_at(runtime: &mut Runtime, x: f32, y: f32) {
        let point = PointerEvent {
            x,
            y,
            ..Default::default()
        };
        runtime.dispatch(Event::PointerDown(point));
        runtime.dispatch(Event::PointerUp(point));
    }

    #[test]
    fn demo_has_the_zig_layout_keys_and_fixed_boxes() {
        let (runtime, _renderer) = demo();
        let mut keys = Vec::new();
        for node in &runtime.tree.nodes {
            if node.alive
                && node.in_flow()
                && let Some(key) = node.key.as_deref()
            {
                keys.push(key.to_string());
            }
        }
        assert_eq!(keys, ZIG_KEYS, "same keyed boxes as the Zig layout dump");

        // card 486 + padding 20 → the radio group starts at 32 + 20 and spans
        // the card's content width (Zig dump: theme 52 319 446 24).
        let theme = bounds(&runtime, "theme");
        assert_eq!((theme.x, theme.width, theme.height), (52.0, 446.0, 24.0));
        // preview card starts at 32 + 486 + 18 and is 326 wide (Zig dump:
        // preview-tabs 556 168 286 377).
        let tabs = bounds(&runtime, "preview-tabs");
        assert_eq!((tabs.x, tabs.width), (556.0, 286.0));
        let first_tab = bounds(&runtime, "preview-tabs-0");
        assert_eq!(first_tab.x, 556.0);
        // the ink bar sits on the active tab and is as wide as it
        let ink = bounds(&runtime, "preview-tabs-ink");
        assert_eq!(
            (ink.x, ink.width, ink.height),
            (556.0, first_tab.width, 2.0)
        );
        assert_eq!(
            ink.y + ink.height,
            tabs.y + 32.0,
            "ink bar at the bottom of the 32px tab strip"
        );
    }

    #[test]
    fn demo_decodes_and_attaches_the_image() {
        let (runtime, _renderer) = demo();
        let mut found = false;
        for node in &runtime.tree.nodes {
            if node.alive
                && let NodeData::Image(data) = &node.data
            {
                assert!(data.handle.is_some(), "the image asset decoded");
                // the node box is fixed at 280x140, so the intrinsic size is
                // only informational here
                assert_eq!(data.fit, ImageFit::Cover);
                found = true;
            }
        }
        assert!(found, "the preview card contains the image");
    }

    #[test]
    fn demo_toggle_callbacks_are_declared_like_the_zig_tree() {
        let (runtime, _renderer) = demo();
        let mut checkbox = 0;
        let mut switch = 0;
        let mut radio = 0;
        for node in &runtime.tree.nodes {
            if !node.alive {
                continue;
            }
            let NodeData::Toggle(data) = &node.data else {
                continue;
            };
            match (data.kind, data.index_action.is_some()) {
                (zant::core::node::ToggleKind::Checkbox, _) => {
                    assert!(data.action.is_some(), "checkbox on_change");
                    checkbox += 1;
                }
                (zant::core::node::ToggleKind::SwitchControl, _) => {
                    assert!(data.action.is_some(), "switch on_change");
                    switch += 1;
                }
                (zant::core::node::ToggleKind::Radio, true) => radio += 1,
                (zant::core::node::ToggleKind::Radio, false) => panic!("radio without on_change"),
            }
        }
        assert_eq!((checkbox, switch, radio), (1, 1, 3));
    }

    #[test]
    fn demo_save_button_updates_the_state() {
        let (mut runtime, mut renderer) = demo();
        let save = find_button(&runtime, "Save Changes");
        click(&mut runtime, save);
        frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.app_state_mut::<State>().saved, 1);
        // the Details tab is not built, so the counter text stays out of the
        // tree; the state is the single source of truth
        let save = find_button(&runtime, "Save Changes");
        click(&mut runtime, save);
        frame(&mut runtime, &mut renderer);
        assert_eq!(runtime.app_state_mut::<State>().saved, 2);
    }

    #[test]
    fn demo_typing_updates_the_project_name() {
        let (mut runtime, mut renderer) = demo();
        let input = find_input(&runtime);
        let input_box = runtime.tree.get(input).bounds;
        // click at the right edge so the caret lands at the end of the value
        click_at(
            &mut runtime,
            input_box.x + input_box.width - 2.0,
            input_box.y + 12.0,
        );
        frame(&mut runtime, &mut renderer);
        runtime.dispatch(Event::TextInput(String::from("!")));
        frame(&mut runtime, &mut renderer);
        let state = runtime.app_state_mut::<State>();
        assert_eq!(state.name_len, 8, "one byte typed");
        assert!(state.name().contains('!'));
        let typed = state.name().into_owned();
        // the controlled input shows the new value
        let node = runtime.tree.get(find_input(&runtime));
        let NodeData::Input(data) = &node.data else {
            panic!("input node");
        };
        assert_eq!(data.text.as_str(), typed);
    }
}
