//! Port of `src/components/segmented.zig` — WP-2.5: the antd Segmented control.
//!
//! A `bg_layout` track (2 px padding, 6 px radius) with a raised `bg_elevated`
//! thumb that slides to the selected option (antd: 300 ms ease-in-out). The
//! thumb follows the selected item through the layout `anchor`; the runtime's
//! FLIP transition (WP-2.7) animates the slide from the old to the new box.
#![forbid(unsafe_code)]

use crate::Str;
use crate::core::geometry::Edges;
use crate::core::node::{ButtonType, Icon};
use crate::layout::flex;
use crate::motion::easing::Easing;
use crate::motion::motion;
use crate::ui::actions::IndexAction;
use crate::ui::builders::{button, column, icon, row, text, tooltip};
use crate::ui::context::current_theme;
use crate::ui::element::Node;
use crate::ui::options::{BoxOptions, ButtonOptions, IconOptions, TextOptions, TooltipOptions};
use crate::ui::styled::{Style, inset, styled};

/// One option of [`segmented`] (`SegmentedOption`).
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct SegmentedOption {
    pub label: Str,
    pub icon: Icon,
    /// Hover hint (useful for icon-only options).
    pub tooltip: Option<Str>,
}

/// Options of [`segmented`] (`SegmentedOptions`).
#[derive(Clone, zant_macros::TsxProps)]
pub struct SegmentedOptions {
    pub key: Option<Str>,
    pub options: Vec<SegmentedOption>,
    pub selected: usize,
    /// Receives the chosen option index.
    pub on_change: Option<IndexAction>,
    /// Item height (antd: 28 small, 32 middle).
    pub height: f32,
    pub font_size: f32,
    pub item_width: Option<f32>,
    /// Fill the parent's width (antd `block`). Otherwise the control is as
    /// wide as its options, even in a column that stretches its children
    /// (antd's default `inline-block`).
    pub block: bool,
}

impl Default for SegmentedOptions {
    fn default() -> Self {
        SegmentedOptions {
            key: Some(Str::Static("segmented")),
            options: Vec::new(),
            selected: 0,
            on_change: None,
            height: 28.0,
            font_size: 13.0,
            item_width: None,
            block: false,
        }
    }
}

/// Zig `segmented`: the antd Segmented control.
pub fn segmented(o: SegmentedOptions) -> Node {
    let theme = current_theme();
    let base = o.key.clone().unwrap_or(Str::Static("segmented"));
    let mut items: Vec<Node> = Vec::with_capacity(o.options.len() + 1);
    // the thumb is the first child (painted below the options) and follows
    // the selected option through `anchor`; FLIP animates the slide
    if !o.options.is_empty() {
        items.push(styled(
            column(BoxOptions {
                key: Some(Str::from(format!("{}-thumb", base.as_str()))),
                children: Vec::new(),
                ..Default::default()
            }),
            Style {
                absolute: Some(true),
                anchor: Some(u16::try_from(o.selected.min(o.options.len() - 1)).expect("int cast")),
                background: Some(theme.color_bg_elevated),
                radius: Some(4.0),
                shadow_color: Some(0x1400_0000),
                shadow_blur: Some(4.0),
                shadow_offset_y: Some(1.0),
                layout_transition: Some(motion::Transition {
                    duration_ms: 300,
                    easing: Easing::EaseInOut,
                    delay_ms: 0,
                }),
                ..Default::default()
            },
        ));
    }
    for (i, option) in o.options.iter().enumerate() {
        let active = i == o.selected;
        let color = if active {
            theme.color_text
        } else {
            theme.color_text_secondary
        };
        let mut content: Vec<Node> = Vec::new();
        let mut hit = styled(
            button(ButtonOptions {
                key: Some(Str::from(format!("{}-{}-hit", base.as_str(), i))),
                variant: if active {
                    ButtonType::Plain
                } else {
                    ButtonType::Text
                },
                focusable: false,
                index_action: o.on_change.clone(),
                index: i,
                ..Default::default()
            }),
            Style {
                absolute: Some(true),
                left: Some(0.0),
                top: Some(0.0),
                width_percent: Some(100.0),
                height_percent: Some(100.0),
                radius: Some(4.0),
                ..Default::default()
            },
        );
        if let Some(tip) = &option.tooltip {
            hit = tooltip(TooltipOptions {
                text: tip.clone(),
                child: hit,
            });
        }
        content.push(hit);
        if option.icon != Icon::None {
            content.push(icon(IconOptions {
                name: option.icon,
                size: o.font_size + 1.0,
                color: Some(color),
                ..Default::default()
            }));
        }
        if !option.label.is_empty() {
            content.push(text(
                option.label.clone(),
                TextOptions {
                    size: o.font_size,
                    color: Some(color),
                    ..Default::default()
                },
            ));
        }
        let item = row(BoxOptions {
            key: Some(Str::from(format!("{}-{}", base.as_str(), i))),
            height: Some(o.height - 4.0),
            width: o.item_width,
            padding: Edges {
                left: 11.0,
                right: 11.0,
                ..Edges::default()
            },
            gap: 6.0,
            justify: flex::Justify::Center,
            align_items: flex::Align::Center,
            children: content,
            ..Default::default()
        });
        items.push(item);
    }
    let track = row(BoxOptions {
        key: Some(base),
        height: Some(o.height),
        padding: inset(2.0),
        gap: 2.0,
        radius: 6.0,
        background: Some(theme.color_bg_layout),
        align_items: flex::Align::Center,
        children: items,
        ..Default::default()
    });
    if o.block {
        return track;
    }
    // inline: a transparent row takes the stretch, the track keeps its width
    row(BoxOptions {
        children: vec![track],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::node::NodeKind;
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;
    use crate::ui::actions::{ActionCx, index_action};
    use crate::ui::app::App;
    use crate::ui::builders::column;
    use crate::ui::{BoxOptions, RunOptions};

    fn options() -> SegmentedOptions {
        SegmentedOptions {
            options: vec![
                SegmentedOption {
                    label: "Day".into(),
                    ..Default::default()
                },
                SegmentedOption {
                    label: "Week".into(),
                    ..Default::default()
                },
                SegmentedOption {
                    label: "Month".into(),
                    ..Default::default()
                },
            ],
            selected: 1,
            on_change: Some(index_action(|s: &mut u32, i| *s = i as u32)),
            block: true,
            ..Default::default()
        }
    }

    #[test]
    fn segmented_defaults_match_the_zig_struct() {
        let defaults = SegmentedOptions::default();
        assert_eq!(defaults.key.as_deref(), Some("segmented"));
        assert!(defaults.options.is_empty());
        assert_eq!(defaults.selected, 0);
        assert!(defaults.on_change.is_none());
        assert_eq!(defaults.height, 28.0);
        assert_eq!(defaults.font_size, 13.0);
        assert!(defaults.item_width.is_none());
        assert!(!defaults.block);
        let option = SegmentedOption::default();
        assert!(option.label.is_empty());
        assert_eq!(option.icon, Icon::None);
        assert!(option.tooltip.is_none());
    }

    #[test]
    fn segmented_builds_the_track_thumb_and_items() {
        let theme = ant_light();
        let node = segmented(options());
        let track = node.element();
        assert_eq!(track.kind, NodeKind::Container);
        assert_eq!(track.key.as_deref(), Some("segmented"));
        assert_eq!(track.layout.height, Some(28.0));
        assert_eq!(track.layout.padding, inset(2.0));
        assert_eq!(track.layout.gap, 2.0);
        assert_eq!(track.visual.radius, 6.0);
        assert_eq!(track.visual.background, Some(theme.color_bg_layout));
        assert_eq!(track.layout.align_items, flex::Align::Center);
        assert_eq!(track.children.len(), 4, "thumb + three options");

        let thumb = track.children[0].element();
        assert_eq!(thumb.key.as_deref(), Some("segmented-thumb"));
        assert!(thumb.layout.absolute);
        assert_eq!(
            thumb.layout.anchor,
            Some(1),
            "the thumb follows the selected option"
        );
        assert_eq!(thumb.visual.background, Some(theme.color_bg_elevated));
        assert_eq!(thumb.visual.radius, 4.0);
        assert_eq!(thumb.visual.shadow_color, Some(0x1400_0000));
        assert_eq!(thumb.visual.shadow_blur, 4.0);
        assert_eq!(thumb.visual.shadow_offset_y, 1.0);
        let transition = thumb.motion.layout_transition.expect("thumb transition");
        assert_eq!(transition.duration_ms, 300);
        assert_eq!(transition.easing, Easing::EaseInOut);
        assert!(thumb.children.is_empty());

        let first = track.children[1].element();
        assert_eq!(first.key.as_deref(), Some("segmented-0"));
        assert_eq!(first.layout.height, Some(24.0));
        assert_eq!(first.layout.padding.left, 11.0);
        assert_eq!(first.layout.padding.right, 11.0);
        assert_eq!(first.layout.gap, 6.0);
        assert_eq!(first.layout.justify, flex::Justify::Center);
        assert_eq!(first.layout.align_items, flex::Align::Center);
        assert_eq!(first.children.len(), 2, "hit target + label");
        let hit = first.children[0].element();
        assert_eq!(hit.key.as_deref(), Some("segmented-0-hit"));
        assert_eq!(hit.kind, NodeKind::Button);
        assert_eq!(hit.button_type, ButtonType::Text);
        assert!(!hit.focusable);
        assert!(hit.layout.absolute);
        assert_eq!(hit.layout.width_percent, Some(100.0));
        assert_eq!(hit.layout.height_percent, Some(100.0));
        assert_eq!(hit.visual.radius, 4.0);
        assert_eq!(hit.index, 0);
        assert!(hit.index_action.is_some());
        let label = first.children[1].element();
        assert_eq!(label.content, "Day");
        assert_eq!(label.text_style.font_size, 13.0);
        assert_eq!(label.text_style.color, theme.color_text_secondary);

        // the selected item uses the plain variant and the primary text colour
        let selected = track.children[2].element();
        let selected_hit = selected.children[0].element();
        assert_eq!(selected_hit.button_type, ButtonType::Plain);
        assert_eq!(
            selected.children[1].element().text_style.color,
            theme.color_text
        );
    }

    #[test]
    fn segmented_icons_tooltips_and_item_width() {
        let node = segmented(SegmentedOptions {
            options: vec![
                SegmentedOption {
                    icon: Icon::List,
                    tooltip: Some("List view".into()),
                    ..Default::default()
                },
                SegmentedOption {
                    label: "Grid".into(),
                    icon: Icon::Menu,
                    ..Default::default()
                },
            ],
            item_width: Some(64.0),
            block: true,
            ..Default::default()
        });
        let track = node.element();
        let first = track.children[1].element();
        assert_eq!(first.layout.width, Some(64.0));
        // hit (wrapped in a tooltip) + icon, no label
        assert_eq!(first.children.len(), 2);
        let hit = first.children[0].element();
        assert_eq!(hit.kind, NodeKind::Button, "tooltip wraps the same node");
        assert_eq!(hit.tooltip.as_deref(), Some("List view"));
        let view_icon = first.children[1].element();
        assert_eq!(view_icon.kind, NodeKind::Vector);
        let second = track.children[2].element();
        assert_eq!(second.children.len(), 3, "hit + icon + label");
        assert_eq!(
            second.children[1].element().layout.width,
            Some(14.0),
            "the drawing is `font_size + 1` px wide"
        );
        assert_eq!(second.children[2].element().content, "Grid");
    }

    #[test]
    fn segmented_inline_mode_wraps_the_track_in_a_transparent_row() {
        let node = segmented(SegmentedOptions {
            options: vec![SegmentedOption {
                label: "One".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        let outer = node.element();
        assert!(outer.key.is_none(), "the wrapper row has no key (Zig)");
        assert_eq!(outer.kind, NodeKind::Container);
        assert_eq!(outer.layout.direction, flex::Direction::Row);
        assert_eq!(outer.children.len(), 1);
        assert_eq!(
            outer.children[0].element().key.as_deref(),
            Some("segmented")
        );
        // `block` returns the track itself
        let block = segmented(SegmentedOptions {
            options: vec![SegmentedOption {
                label: "One".into(),
                ..Default::default()
            }],
            block: true,
            ..Default::default()
        });
        assert_eq!(block.element().key.as_deref(), Some("segmented"));
        // an empty control keeps the track and paints no thumb
        let empty = segmented(SegmentedOptions {
            block: true,
            ..Default::default()
        });
        assert!(empty.element().children.is_empty());
    }

    #[test]
    fn segmented_clamps_the_selected_index_for_the_thumb() {
        let node = segmented(SegmentedOptions {
            options: vec![
                SegmentedOption {
                    label: "A".into(),
                    ..Default::default()
                },
                SegmentedOption {
                    label: "B".into(),
                    ..Default::default()
                },
            ],
            selected: 99,
            block: true,
            ..Default::default()
        });
        let track = node.element();
        assert_eq!(track.children[0].element().layout.anchor, Some(1));
        // no option is active when `selected` is out of range (Zig compares
        // the raw index), so every hit uses the text variant
        assert_eq!(
            track.children[1].element().children[0]
                .element()
                .button_type,
            ButtonType::Text
        );
    }

    #[test]
    fn segmented_change_callback_receives_the_index() {
        let mut state = 7u32;
        let node = segmented(options());
        let track = node.element();
        let hit = track.children[2].element().children[0].element();
        {
            let mut cx = ActionCx::new(&mut state);
            if let Some(callback) = hit.index_action.clone() {
                callback.call(&mut cx, hit.index);
            }
        }
        assert_eq!(state, 1);
    }

    struct SegmentedApp;

    impl App for SegmentedApp {
        fn render(&mut self) -> Node {
            column(BoxOptions {
                children: crate::nodes![segmented(options())],
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
    fn segmented_paints_the_thumb_over_the_track() {
        let theme = ant_light();
        let (runtime, renderer) = paint(SegmentedApp, 320, 60);
        let track = runtime.find_key("segmented").expect("track");
        let track_bounds = runtime.tree.bounds(track);
        assert_eq!(track_bounds.height, 28.0);
        let thumb = runtime.find_key("segmented-thumb").expect("thumb");
        let thumb_bounds = runtime.tree.bounds(thumb);
        // the thumb sits on the second option's box
        let second = runtime.find_key("segmented-1").expect("second item");
        let second_bounds = runtime.tree.bounds(second);
        assert!((thumb_bounds.x - second_bounds.x).abs() <= 1.0);
        let surface = renderer
            .pixel_at(
                thumb_bounds.x as usize + 2,
                (thumb_bounds.y + thumb_bounds.height * 0.5) as usize,
            )
            .expect("thumb pixel");
        assert_eq!(surface, theme.color_bg_elevated);
    }
}
