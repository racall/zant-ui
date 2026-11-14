//! The Windows caption buttons (minimize, maximize / restore, close) for
//! app-drawn title bars.
//!
//! The runtime adds them in the top-right corner on Windows for
//! `WindowStyle::HiddenTitlebar`, so apps only keep the space free
//! (`ui::titlebar_insets()`); macOS keeps its native traffic lights.
//!
//! Sizes follow Windows 11: 46 px wide buttons filling the title bar height,
//! 10 px glyphs with 1 px strokes, a faint hover fill and the red close
//! button. The maximize button reports itself to the window (key
//! [`CAPTION_MAXIMIZE_KEY`]) so Windows 11 shows its snap layouts flyout.
//!
//! Runtime integration points (not in this module):
//! - `Runtime::build_tree` appends `window_controls(WindowControlsOptions {
//!   height: caption_height, maximized: platform::is_maximized(window),
//!   active: self.active, on_minimize / on_maximize / on_close })` when the
//!   window is Windows + hidden title bar.
//! - `Runtime::titlebar_hit` maps the node keyed [`CAPTION_MAXIMIZE_KEY`] to
//!   `platform::hit::MAXIMIZE` so the native snap-layouts hit test works.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonType, Icon};
use crate::platform::platform::CAPTION_BUTTON_WIDTH;
use crate::ui::actions::Action;
use crate::ui::builders::{button, row};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, ButtonOptions};
use crate::ui::styled::{Style, styled};

/// Key of the maximize button; the runtime's title-bar hit test looks it up.
pub const CAPTION_MAXIMIZE_KEY: &str = "zant-caption-max";

/// Options of [`window_controls`] (`WindowControlsOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct WindowControlsOptions {
    /// Title bar height (the buttons fill it).
    pub height: f32,
    pub maximized: bool,
    /// Inactive windows draw dimmer glyphs.
    pub active: bool,
    pub on_minimize: Option<Action>,
    pub on_maximize: Option<Action>,
    pub on_close: Option<Action>,
}

impl Default for WindowControlsOptions {
    fn default() -> Self {
        WindowControlsOptions {
            height: 32.0,
            maximized: false,
            active: true,
            on_minimize: None,
            on_maximize: None,
            on_close: None,
        }
    }
}

/// One square caption button filling the title bar.
fn caption_button(
    key: &str,
    icon: Icon,
    variant: ButtonType,
    color: u32,
    height: f32,
    action: Option<Action>,
) -> Node {
    styled(
        button(ButtonOptions {
            key: Some(Str::copy(key)),
            variant,
            icon,
            icon_size: 10.0,
            text_color: Some(color),
            focusable: false,
            on_click: action,
            ..Default::default()
        }),
        Style {
            width: Some(CAPTION_BUTTON_WIDTH),
            height: Some(height),
            radius: Some(0.0),
            padding: Some(Edges::default()),
            ..Default::default()
        },
    )
}

/// Minimize / maximize / close in the top-right corner.
pub fn window_controls(options: WindowControlsOptions) -> Node {
    let theme = current_theme();
    let color = if options.active {
        theme.color_text
    } else {
        theme.color_text_secondary
    };
    let maximize_icon = if options.maximized {
        Icon::WindowRestore
    } else {
        Icon::WindowMaximize
    };
    styled(
        row(BoxOptions {
            key: Some(Str::Static("zant-caption")),
            children: vec![
                caption_button(
                    "zant-caption-min",
                    Icon::WindowMinimize,
                    ButtonType::Caption,
                    color,
                    options.height,
                    options.on_minimize.clone(),
                ),
                caption_button(
                    CAPTION_MAXIMIZE_KEY,
                    maximize_icon,
                    ButtonType::Caption,
                    color,
                    options.height,
                    options.on_maximize.clone(),
                ),
                caption_button(
                    "zant-caption-close",
                    Icon::WindowClose,
                    ButtonType::CaptionClose,
                    color,
                    options.height,
                    options.on_close.clone(),
                ),
            ],
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            top: Some(0.0),
            right: Some(0.0),
            height: Some(options.height),
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeKind;
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;
    use crate::ui::RunOptions;
    use crate::ui::actions::{ActionCx, action};
    use crate::ui::app::App;
    use crate::ui::builders::column;
    use crate::ui::options::BoxOptions;
    use crate::ui::styled::styled;

    fn options() -> WindowControlsOptions {
        WindowControlsOptions {
            on_minimize: Some(action(|s: &mut u32| *s += 1)),
            on_maximize: Some(action(|s: &mut u32| *s += 10)),
            on_close: Some(action(|s: &mut u32| *s += 100)),
            ..Default::default()
        }
    }

    #[test]
    fn window_controls_defaults() {
        let defaults = WindowControlsOptions::default();
        assert_eq!(defaults.height, 32.0);
        assert!(!defaults.maximized);
        assert!(defaults.active);
        assert!(defaults.on_minimize.is_none());
        assert!(defaults.on_maximize.is_none());
        assert!(defaults.on_close.is_none());
        assert_eq!(CAPTION_MAXIMIZE_KEY, "zant-caption-max");
        assert_eq!(CAPTION_BUTTON_WIDTH, 46.0);
    }

    #[test]
    fn window_controls_build_three_caption_buttons() {
        let theme = ant_light();
        let node = window_controls(options());
        let bar = node.element();
        assert_eq!(bar.kind, NodeKind::Container);
        assert_eq!(bar.key.as_deref(), Some("zant-caption"));
        assert!(bar.layout.absolute);
        assert_eq!(bar.layout.top, Some(0.0));
        assert_eq!(bar.layout.right, Some(0.0));
        assert_eq!(bar.layout.height, Some(32.0));
        assert_eq!(bar.children.len(), 3);

        let expected = [
            (
                "zant-caption-min",
                Icon::WindowMinimize,
                ButtonType::Caption,
            ),
            (
                CAPTION_MAXIMIZE_KEY,
                Icon::WindowMaximize,
                ButtonType::Caption,
            ),
            (
                "zant-caption-close",
                Icon::WindowClose,
                ButtonType::CaptionClose,
            ),
        ];
        for (button_node, (key, icon, variant)) in bar.children.iter().zip(expected) {
            let element = button_node.element();
            assert_eq!(element.key.as_deref(), Some(key));
            assert_eq!(element.kind, NodeKind::Button);
            assert_eq!(element.icon, icon);
            assert_eq!(element.button_type, variant);
            assert_eq!(element.icon_size, 10.0);
            assert!(!element.focusable);
            assert_eq!(element.text_color, Some(theme.color_text));
            assert_eq!(element.layout.width, Some(46.0));
            assert_eq!(element.layout.height, Some(32.0));
            assert_eq!(element.layout.shrink, 0.0);
            assert_eq!(element.visual.radius, 0.0);
            assert_eq!(element.layout.padding, Edges::default());
            assert!(element.on_click.is_some());
        }
    }

    #[test]
    fn window_controls_switch_the_restore_icon_and_inactive_colour() {
        let theme = ant_light();
        let maximized = window_controls(WindowControlsOptions {
            maximized: true,
            active: false,
            height: 40.0,
            ..Default::default()
        });
        let bar = maximized.element();
        assert_eq!(bar.layout.height, Some(40.0));
        let maximize = bar.children[1].element();
        assert_eq!(maximize.icon, Icon::WindowRestore);
        assert_eq!(maximize.layout.height, Some(40.0));
        assert_eq!(maximize.text_color, Some(theme.color_text_secondary));
        // the other buttons use the inactive colour as well
        assert_eq!(
            bar.children[0].element().text_color,
            Some(theme.color_text_secondary)
        );
        assert_eq!(
            bar.children[2].element().text_color,
            Some(theme.color_text_secondary)
        );
    }

    #[test]
    fn window_controls_callbacks_run_against_the_root_state() {
        let mut state = 0u32;
        let node = window_controls(options());
        let bar = node.element();
        {
            let mut cx = ActionCx::new(&mut state);
            for child in &bar.children {
                if let Some(callback) = child.element().on_click.clone() {
                    callback.call(&mut cx);
                }
            }
        }
        assert_eq!(state, 111);
    }

    struct ControlsApp;

    impl App for ControlsApp {
        fn render(&mut self) -> Node {
            styled(
                column(BoxOptions {
                    children: crate::nodes![window_controls(options())],
                    ..Default::default()
                }),
                Style {
                    background: Some(ant_light().color_bg_base),
                    ..Default::default()
                },
            )
        }
    }

    fn settle(runtime: &mut crate::runtime::Runtime) {
        for node in &mut runtime.tree.nodes {
            if !node.alive {
                continue;
            }
            if let Some(anim) = node.anim.as_mut() {
                anim.presence = crate::motion::motion::Animated::at(1.0);
                anim.fx = crate::motion::motion::Fx::default();
            }
        }
        runtime.tree.mark_paint_dirty();
    }

    fn paint(app: impl App, width: usize, height: usize) -> (crate::runtime::Runtime, Renderer) {
        let mut runtime = crate::runtime::Runtime::init(app, RunOptions::default());
        let mut renderer =
            Renderer::create_raster(width as u32, height as u32).expect("raster renderer");
        for _ in 0..2 {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            runtime.render_frame(canvas, width as f32, height as f32, 1.0);
            renderer.end();
            settle(&mut runtime);
        }
        (runtime, renderer)
    }

    #[test]
    fn window_controls_paint_in_the_top_right_corner() {
        let theme = ant_light();
        let width = 400.0;
        let (runtime, renderer) = paint(ControlsApp, 400, 200);
        let bar = runtime.find_key("zant-caption").expect("caption bar");
        let bounds = runtime.tree.bounds(bar);
        assert_eq!(bounds.width, 138.0, "three 46 px buttons");
        assert_eq!(bounds.height, 32.0);
        assert_eq!(bounds.y, 0.0);
        assert!((bounds.x + bounds.width - width).abs() <= 0.01);
        // the three buttons sit side by side
        let min = runtime.find_key("zant-caption-min").expect("minimize");
        let mid = runtime.find_key(CAPTION_MAXIMIZE_KEY).expect("maximize");
        let close = runtime.find_key("zant-caption-close").expect("close");
        let min_bounds = runtime.tree.bounds(min);
        let mid_bounds = runtime.tree.bounds(mid);
        let close_bounds = runtime.tree.bounds(close);
        assert!((min_bounds.x - bounds.x).abs() <= 0.01);
        assert!((mid_bounds.x - (min_bounds.x + 46.0)).abs() <= 0.01);
        assert!((close_bounds.x - (mid_bounds.x + 46.0)).abs() <= 0.01);
        // the close glyph paints inside its button over the title bar
        let mut painted = 0usize;
        for y in close_bounds.y as usize..(close_bounds.y + close_bounds.height) as usize {
            for x in close_bounds.x as usize..(close_bounds.x + close_bounds.width) as usize {
                if renderer.pixel_at(x, y) != Some(theme.color_bg_base) {
                    painted += 1;
                }
            }
        }
        assert!(painted > 0, "the caption glyph is painted");
    }
}
