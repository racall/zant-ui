//! Port of `src/components/message.zig` — WP-2.5: the antd global message.
//!
//! A small pill centred near the top of the window that drops in and fades
//! out. It holds nothing clickable, so clicks pass through to the page.
//!
//! The app decides how long it stays: render it while a deadline is ahead and
//! call `ui::wake_after(remaining)` so it disappears on time (the runtime's
//! `wake_at_ms` already handles the wake-up).
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::Icon;
use crate::layout::flex;
use crate::motion::motion;
use crate::ui::builders::{icon, row, text};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, IconOptions, TextOptions};
use crate::ui::styled::{Style, styled};

/// Status of a global message (`MessageKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MessageKind {
    #[default]
    Success,
    Info,
    Warning,
    Err,
}

/// Options of [`message`] (`MessageOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct MessageOptions {
    pub key: Str,
    pub text: Str,
    pub kind: MessageKind,
    /// Distance from the top of the window.
    pub top: f32,
    /// Icon colour override (default: the kind's token colour).
    pub icon_color: Option<u32>,
}

impl Default for MessageOptions {
    fn default() -> Self {
        MessageOptions {
            key: Str::Static("message"),
            text: Str::default(),
            kind: MessageKind::Success,
            top: 16.0,
            icon_color: None,
        }
    }
}

/// Zig `message`: the transient pill near the top of the window.
pub fn message(o: MessageOptions) -> Node {
    let theme = current_theme();
    let icon_name: Icon = match o.kind {
        MessageKind::Success => Icon::Check,
        MessageKind::Info => Icon::Info,
        MessageKind::Warning | MessageKind::Err => Icon::Alert,
    };
    let color = o.icon_color.unwrap_or(match o.kind {
        MessageKind::Success => theme.color_success,
        MessageKind::Info => theme.color_primary,
        MessageKind::Warning => theme.color_warning,
        MessageKind::Err => theme.color_error,
    });
    // antd: elevated surface, 8 px radius, 9 px × 12 px padding, soft shadow
    let pill = styled(
        row(BoxOptions {
            key: Some(Str::Static("message-pill")),
            gap: 8.0,
            padding: Edges {
                left: 12.0,
                right: 12.0,
                top: 9.0,
                bottom: 9.0,
            },
            align_items: flex::Align::Center,
            children: vec![
                icon(IconOptions {
                    name: icon_name,
                    size: 14.0,
                    color: Some(color),
                    ..Default::default()
                }),
                text(
                    o.text.clone(),
                    TextOptions {
                        size: 13.0,
                        color: Some(theme.color_text),
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        }),
        Style {
            background: Some(theme.color_bg_elevated),
            border_color: Some(theme.color_split),
            border_width: Some(1.0),
            radius: Some(theme.border_radius_lg),
            shadow_color: Some(0x1f00_0000),
            shadow_blur: Some(16.0),
            shadow_offset_y: Some(6.0),
            ..Default::default()
        },
    );
    styled(
        row(BoxOptions {
            key: Some(o.key),
            justify: flex::Justify::Center,
            children: vec![pill],
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(o.top),
            width_percent: Some(100.0),
            motion: Some(motion::Presence::MoveUp),
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
    use crate::ui::app::App;
    use crate::ui::builders::column;
    use crate::ui::{BoxOptions, RunOptions};

    fn options() -> MessageOptions {
        MessageOptions {
            text: "Saved".into(),
            ..Default::default()
        }
    }

    #[test]
    fn message_defaults_match_the_zig_struct() {
        let defaults = MessageOptions::default();
        assert_eq!(defaults.key.as_str(), "message");
        assert!(defaults.text.is_empty());
        assert_eq!(defaults.kind, MessageKind::Success);
        assert_eq!(defaults.top, 16.0);
        assert!(defaults.icon_color.is_none());
    }

    fn glyph_color(node: &Node) -> u32 {
        use crate::core::vector::Command;
        let drawing = node
            .element()
            .drawing
            .as_ref()
            .expect("the icon is a vector drawing");
        match &drawing.commands[0] {
            Command::Path(path) => path.color,
            Command::Line(line) => line.color,
            Command::Rect(rect) => rect.color,
            Command::Text(text) => text.color,
        }
    }

    #[test]
    fn message_kinds_pick_their_icon_and_token_colour() {
        let theme = ant_light();
        let expected = [
            (MessageKind::Success, theme.color_success),
            (MessageKind::Info, theme.color_primary),
            (MessageKind::Warning, theme.color_warning),
            (MessageKind::Err, theme.color_error),
        ];
        for (kind, color) in expected {
            let node = message(MessageOptions {
                kind,
                text: "x".into(),
                ..Default::default()
            });
            let pill = node.element().children[0].element();
            let glyph = pill.children[0].element();
            assert_eq!(glyph.kind, NodeKind::Vector, "icons build vector nodes");
            assert_eq!(glyph_color(&pill.children[0]), color);
            let label = pill.children[1].element();
            assert_eq!(label.content, "x");
            assert_eq!(label.text_style.color, theme.color_text);
            assert_eq!(glyph.layout.width, Some(14.0), "the drawing is 14 px wide");
        }
        let overridden = message(MessageOptions {
            kind: MessageKind::Err,
            text: "x".into(),
            icon_color: Some(0xff12_3456),
            ..Default::default()
        });
        let pill = overridden.element().children[0].element();
        assert_eq!(glyph_color(&pill.children[0]), 0xff12_3456);
    }

    #[test]
    fn message_builds_the_pill_and_the_top_centred_layer() {
        let theme = ant_light();
        let node = message(options());
        let layer = node.element();
        assert_eq!(layer.key.as_deref(), Some("message"));
        assert_eq!(layer.kind, NodeKind::Container);
        assert!(layer.layout.absolute);
        assert_eq!(layer.layout.left, Some(0.0));
        assert_eq!(layer.layout.top, Some(16.0));
        assert_eq!(layer.layout.width_percent, Some(100.0));
        assert_eq!(layer.layout.justify, flex::Justify::Center);
        assert_eq!(layer.motion.presence, motion::Presence::MoveUp);

        let pill = layer.children[0].element();
        assert_eq!(pill.key.as_deref(), Some("message-pill"));
        assert_eq!(pill.layout.gap, 8.0);
        assert_eq!(
            pill.layout.padding,
            Edges {
                left: 12.0,
                right: 12.0,
                top: 9.0,
                bottom: 9.0
            }
        );
        assert_eq!(pill.layout.align_items, flex::Align::Center);
        assert_eq!(pill.visual.background, Some(theme.color_bg_elevated));
        assert_eq!(pill.visual.border_color, Some(theme.color_split));
        assert_eq!(pill.visual.border_width, 1.0);
        assert_eq!(pill.visual.radius, theme.border_radius_lg);
        assert_eq!(pill.visual.shadow_color, Some(0x1f00_0000));
        assert_eq!(pill.visual.shadow_blur, 16.0);
        assert_eq!(pill.visual.shadow_offset_y, 6.0);
        assert_eq!(pill.children.len(), 2);
        assert_eq!(pill.children[1].element().text_style.font_size, 13.0);
    }

    struct MessageApp;

    impl App for MessageApp {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![message(options())],
                ..Default::default()
            })
        }
    }

    fn settle(runtime: &mut crate::runtime::Runtime) {
        for node in &mut runtime.tree.nodes {
            if !node.alive {
                continue;
            }
            if let Some(anim) = node.anim.as_mut() {
                anim.presence = motion::Animated::at(1.0);
                anim.fx = motion::Fx::default();
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
    fn message_paints_a_centred_pill_sixteen_pixels_down() {
        let theme = ant_light();
        let (runtime, renderer) = paint(MessageApp, 320, 120);
        let pill = runtime.find_key("message-pill").expect("pill");
        let bounds = runtime.tree.bounds(pill);
        assert_eq!(bounds.y, 16.0, "the pill sits `top` px below the top edge");
        // horizontally centred
        let centre = bounds.x + bounds.width * 0.5;
        assert!((centre - 160.0).abs() <= 1.0, "pill centre {centre}");
        let surface = renderer
            .pixel_at(
                bounds.x as usize + 3,
                (bounds.y + bounds.height * 0.5) as usize,
            )
            .expect("pill pixel");
        assert_eq!(surface, theme.color_bg_elevated);
    }
}
