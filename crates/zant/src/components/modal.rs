//! Port of `src/components/modal.zig` — WP-2.5: the antd `Modal`.
//!
//! A dimmed mask over the window with a centred card — title, close button,
//! content and an optional footer. The mask fades and the card zooms (antd
//! motion). Render it as the last child of the window root while it is open;
//! give each modal its own `key`. Esc handling stays with the app (call the
//! same close action from its key handler), like Zig.
//!
//! ```ignore
//! if state.help_open {
//!     children.push(ui::modal(ModalOptions {
//!         title: "Keyboard shortcuts".into(),
//!         on_close: Some(ui::action(|s: &mut State| s.close_help())),
//!         content: ui::shortcut_list(ShortcutListOptions { groups, ..Default::default() }),
//!         ..Default::default()
//!     }));
//! }
//! ```
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::Icon;
use crate::layout::flex;
use crate::motion::motion;
use crate::renderer::text::Weight;
use crate::ui::actions::Action;
use crate::ui::builders::{column, dismiss_layer, icon_button, row, text};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, DismissLayerOptions, IconButtonOptions, TextOptions};
use crate::ui::styled::{Style, inset, styled};

/// Options of [`modal`] (`ModalOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct ModalOptions {
    pub key: Str,
    pub title: Str,
    pub content: Node,
    /// Buttons row (antd: right-aligned, primary last). `None` for none.
    pub footer: Option<Node>,
    /// Card width (antd default 520). `None` sizes it to the content.
    pub width: Option<f32>,
    /// Close button, mask click (when `mask_closable`) and the app's Esc.
    pub on_close: Option<Action>,
    pub closable: bool,
    pub mask_closable: bool,
    /// Mask colour (antd colorBgMask: rgba(0,0,0,.45)).
    pub mask: u32,
}

impl Default for ModalOptions {
    fn default() -> Self {
        ModalOptions {
            key: Str::Static("modal"),
            title: Str::Static(""),
            content: Node::default(),
            footer: None,
            // Zig: `.width = 520`
            width: Some(520.0),
            on_close: None,
            closable: true,
            mask_closable: true,
            // Zig: `.mask = 0x73000000`
            mask: 0x7300_0000,
        }
    }
}

/// Zig `modal`: the mask + centred card overlay.
pub fn modal(o: ModalOptions) -> Node {
    let theme = current_theme();
    let base = o.key.clone();
    let k = |suffix: &str| format!("{}-{suffix}", base.as_str());

    let mut body: Vec<Node> = Vec::new();
    if !o.title.is_empty() || o.closable {
        let mut head: Vec<Node> = Vec::new();
        head.push(styled(
            text(
                o.title.clone(),
                TextOptions {
                    size: 16.0,
                    weight: Weight::Semibold,
                    color: Some(theme.color_text),
                    max_lines: 1,
                    ..Default::default()
                },
            ),
            Style {
                grow: Some(1.0),
                shrink: Some(1.0),
                min_width: Some(0.0),
                ..Default::default()
            },
        ));
        if o.closable {
            head.push(icon_button(IconButtonOptions {
                key: Some(k("close").into()),
                icon: Icon::Close,
                size: 16.0,
                r#box: 28.0,
                color: Some(theme.color_text_secondary),
                tooltip: Some(Str::Static("Esc")),
                on_click: o.on_close.clone(),
                ..Default::default()
            }));
        }
        body.push(row(BoxOptions {
            key: Some(k("head").into()),
            align_items: flex::Align::Center,
            gap: 12.0,
            children: head,
            ..Default::default()
        }));
    }
    body.push(o.content.clone());
    if let Some(footer) = &o.footer {
        body.push(row(BoxOptions {
            key: Some(k("foot").into()),
            justify: flex::Justify::End,
            gap: 8.0,
            children: vec![footer.clone()],
            ..Default::default()
        }));
    }

    // antd: 8 px radius, 20 / 24 px padding, elevated surface, soft shadow
    let card = styled(
        column(BoxOptions {
            key: Some(k("card").into()),
            width: o.width,
            padding: Edges {
                left: 24.0,
                right: 24.0,
                top: 20.0,
                bottom: 24.0,
            },
            gap: 16.0,
            align_items: flex::Align::Stretch,
            children: body,
            ..Default::default()
        }),
        Style {
            background: Some(theme.color_bg_elevated),
            radius: Some(theme.border_radius_lg),
            shadow_color: Some(0x2900_0000),
            shadow_blur: Some(24.0),
            shadow_offset_y: Some(8.0),
            block_pointer: Some(true),
            motion: Some(motion::Presence::ZoomBig),
            ..Default::default()
        },
    );
    let mask = dismiss_layer(DismissLayerOptions {
        key: Some(k("mask").into()),
        on_dismiss: if o.mask_closable {
            o.on_close.clone()
        } else {
            None
        },
        background: Some(o.mask),
    });
    // the centring layer holds nothing clickable: clicks beside the card
    // reach the mask
    let center = styled(
        column(BoxOptions {
            key: Some(k("center").into()),
            justify: flex::Justify::Center,
            align_items: flex::Align::Center,
            padding: inset(24.0),
            children: vec![card],
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            ..Default::default()
        },
    );
    styled(
        column(BoxOptions {
            key: Some(o.key),
            // Tab stays inside the dialog while it is open
            focus_scope: true,
            children: vec![mask, center],
            ..Default::default()
        }),
        Style {
            absolute: Some(true),
            left: Some(0.0),
            top: Some(0.0),
            width_percent: Some(100.0),
            height_percent: Some(100.0),
            motion: Some(motion::Presence::Fade),
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
    use crate::ui::actions::{ActionCx, action};
    use crate::ui::app::App;
    use crate::ui::builders::{button, column};
    use crate::ui::options::ButtonOptions;
    use crate::ui::{BoxOptions, RunOptions};

    fn content() -> Node {
        text(
            "Body",
            TextOptions {
                key: Some("body".into()),
                ..Default::default()
            },
        )
    }

    fn options() -> ModalOptions {
        ModalOptions {
            title: "Help".into(),
            content: content(),
            footer: Some(button(ButtonOptions {
                key: Some("ok".into()),
                text: "OK".into(),
                ..Default::default()
            })),
            on_close: Some(action(|s: &mut u32| *s += 1)),
            ..Default::default()
        }
    }

    #[test]
    fn modal_defaults_match_the_zig_struct() {
        let defaults = ModalOptions::default();
        assert_eq!(defaults.key.as_str(), "modal");
        assert!(defaults.title.is_empty());
        assert_eq!(defaults.content.element().kind, NodeKind::Root);
        assert!(defaults.footer.is_none());
        assert_eq!(defaults.width, Some(520.0));
        assert!(defaults.on_close.is_none());
        assert!(defaults.closable);
        assert!(defaults.mask_closable);
        assert_eq!(defaults.mask, 0x7300_0000);
    }

    #[test]
    fn modal_builds_the_head_content_footer_card_and_mask() {
        let theme = ant_light();
        let node = modal(options());
        let overlay = node.element();
        assert_eq!(overlay.kind, NodeKind::Container);
        assert_eq!(overlay.key.as_deref(), Some("modal"));
        assert!(overlay.visual.focus_scope);
        assert!(overlay.layout.absolute);
        assert_eq!(overlay.layout.left, Some(0.0));
        assert_eq!(overlay.layout.top, Some(0.0));
        assert_eq!(overlay.layout.width_percent, Some(100.0));
        assert_eq!(overlay.layout.height_percent, Some(100.0));
        assert_eq!(overlay.motion.presence, motion::Presence::Fade);
        assert_eq!(overlay.children.len(), 2, "mask then centre layer");

        let mask = overlay.children[0].element();
        assert_eq!(mask.key.as_deref(), Some("modal-mask"));
        assert_eq!(mask.kind, NodeKind::Button);
        assert_eq!(mask.button_type, crate::core::node::ButtonType::Plain);
        assert!(mask.layout.absolute);
        assert_eq!(mask.layout.width_percent, Some(100.0));
        assert_eq!(mask.layout.height_percent, Some(100.0));
        assert_eq!(mask.visual.background, Some(0x7300_0000));
        assert!(mask.on_click.is_some(), "the mask closes the modal");

        let center = overlay.children[1].element();
        assert_eq!(center.key.as_deref(), Some("modal-center"));
        assert!(center.layout.absolute);
        assert_eq!(center.layout.justify, flex::Justify::Center);
        assert_eq!(center.layout.align_items, flex::Align::Center);
        assert_eq!(center.layout.padding, inset(24.0));
        assert_eq!(center.children.len(), 1);
        let card = center.children[0].element();
        assert_eq!(card.key.as_deref(), Some("modal-card"));
        assert_eq!(card.layout.width, Some(520.0));
        assert_eq!(card.layout.gap, 16.0);
        assert_eq!(card.layout.align_items, flex::Align::Stretch);
        assert_eq!(
            card.layout.padding,
            Edges {
                left: 24.0,
                right: 24.0,
                top: 20.0,
                bottom: 24.0
            }
        );
        assert_eq!(card.visual.background, Some(theme.color_bg_elevated));
        assert_eq!(card.visual.radius, theme.border_radius_lg);
        assert_eq!(card.visual.shadow_color, Some(0x2900_0000));
        assert_eq!(card.visual.shadow_blur, 24.0);
        assert_eq!(card.visual.shadow_offset_y, 8.0);
        assert!(card.visual.block_pointer);
        assert_eq!(card.motion.presence, motion::Presence::ZoomBig);

        // head: title then the close button
        let head = card.children[0].element();
        assert_eq!(head.key.as_deref(), Some("modal-head"));
        assert_eq!(head.layout.gap, 12.0);
        assert_eq!(head.layout.align_items, flex::Align::Center);
        let title = head.children[0].element();
        assert_eq!(title.content, "Help");
        assert_eq!(title.text_style.font_size, 16.0);
        assert_eq!(title.text_style.weight, Weight::Semibold);
        assert_eq!(title.text_style.color, theme.color_text);
        assert_eq!(title.text_style.max_lines, 1);
        assert_eq!(title.layout.grow, 1.0);
        assert_eq!(title.layout.shrink, 1.0);
        assert_eq!(title.layout.min_width, Some(0.0));
        let close = head.children[1].element();
        assert_eq!(close.key.as_deref(), Some("modal-close"));
        assert_eq!(close.kind, NodeKind::Button);
        assert_eq!(close.icon, Icon::Close);
        assert_eq!(close.icon_size, 16.0);
        assert_eq!(close.tooltip.as_deref(), Some("Esc"));
        assert!(close.on_click.is_some());

        // content, then the right-aligned footer row
        assert_eq!(card.children[1].element().content, "Body");
        let foot = card.children[2].element();
        assert_eq!(foot.key.as_deref(), Some("modal-foot"));
        assert_eq!(foot.layout.justify, flex::Justify::End);
        assert_eq!(foot.layout.gap, 8.0);
        assert_eq!(foot.children.len(), 1);
    }

    #[test]
    fn modal_without_title_or_close_has_no_head_row() {
        let node = modal(ModalOptions {
            closable: false,
            content: content(),
            ..Default::default()
        });
        let card = node.element().children[1].element().children[0].element();
        assert_eq!(card.children.len(), 1, "content only");
        assert_eq!(card.children[0].element().content, "Body");
        // the mask no longer closes the dialog
        let mask = node.element().children[0].element();
        assert!(mask.on_click.is_none());
        // a title without the close button still gets a head row
        let titled = modal(ModalOptions {
            title: "About".into(),
            closable: false,
            content: content(),
            ..Default::default()
        });
        let card = titled.element().children[1].element().children[0].element();
        let head = card.children[0].element();
        assert_eq!(head.children.len(), 1, "title only, no close button");
    }

    #[test]
    fn modal_content_width_none_sizes_the_card_to_its_content() {
        let node = modal(ModalOptions {
            width: None,
            ..Default::default()
        });
        let card = node.element().children[1].element().children[0].element();
        assert_eq!(card.layout.width, None);
    }

    #[test]
    fn modal_close_callbacks_run_against_the_root_state() {
        let mut state = 0u32;
        let node = modal(options());
        let overlay = node.element();
        let mask = overlay.children[0].element();
        let card = overlay.children[1].element().children[0].element();
        let close = card.children[0].element().children[1].element();
        {
            let mut cx = ActionCx::new(&mut state);
            if let Some(callback) = mask.on_click.clone() {
                callback.call(&mut cx);
            }
            if let Some(callback) = close.on_click.clone() {
                callback.call(&mut cx);
            }
        }
        assert_eq!(state, 2);
    }

    struct ModalApp;

    impl App for ModalApp {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![modal(options())],
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
    fn modal_paints_the_mask_and_the_centred_card() {
        let theme = ant_light();
        let (runtime, renderer) = paint(ModalApp, 600, 400);
        let card = runtime.find_key("modal-card").expect("card");
        let bounds = runtime.tree.bounds(card);
        // the card is centred horizontally
        let centre = bounds.x + bounds.width * 0.5;
        assert!((centre - 300.0).abs() <= 1.0, "card centre {centre}");
        // padding of 24 px: a point just inside the left edge is the surface
        let surface = renderer
            .pixel_at(
                bounds.x as usize + 2,
                (bounds.y + bounds.height * 0.5) as usize,
            )
            .expect("card pixel");
        assert_eq!(surface, theme.color_bg_elevated);
        // outside the card the 45 % black mask dims the page background
        let masked = renderer.pixel_at(4, 4).expect("mask pixel");
        assert_ne!(masked, theme.color_bg_base);
        assert_ne!(masked, theme.color_bg_elevated);
        // the top half of the window is above the card: dim everywhere
        let above = renderer.pixel_at(300, 3);
        assert_eq!(above, Some(masked));
    }
}
