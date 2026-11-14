//! Port of `tests/text_align_test.zig` — centred / right-aligned text that
//! does not wrap stays inside its box (it used to be centred within a 1e9px
//! layout width, i.e. off screen).

use zant::core::geometry::Rect;
use zant::core::node::NodeId;
use zant::renderer::renderer::Renderer;
use zant::renderer::text;
use zant::runtime::Runtime;
use zant::ui::app::{App, RunOptions};
use zant::ui::options::{AvatarOptions, BoxOptions, TextOptions};
use zant::ui::{Style, avatar, column, inset, row, styled, text};

struct Scene;

impl App for Scene {
    fn render(&mut self) -> zant::ui::Node {
        column(BoxOptions {
            padding: inset(16.0),
            gap: 8.0,
            children: zant::nodes![
                row(BoxOptions {
                    gap: 8.0,
                    children: zant::nodes![
                        avatar(AvatarOptions {
                            key: Some("avatar".into()),
                            text: "Z".into(),
                            size: 32.0,
                            ..Default::default()
                        }),
                        avatar(AvatarOptions {
                            key: Some("avatar-solid".into()),
                            text: "UI".into(),
                            size: 40.0,
                            background: Some(0xff1677ff),
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                }),
                styled(
                    text(
                        "Right",
                        TextOptions {
                            key: Some("right".into()),
                            size: 16.0,
                            text_align: text::Align::Right,
                            color: Some(0xff000000),
                            ..Default::default()
                        },
                    ),
                    Style {
                        width: Some(160.0),
                        ..Default::default()
                    },
                ),
            ],
            ..Default::default()
        })
    }
}

fn find(tree: &zant::core::tree::Tree, key: &str) -> NodeId {
    for node in &tree.nodes {
        if node.alive && node.key.as_deref() == Some(key) {
            return node.id;
        }
    }
    panic!("no node with key {key}");
}

/// Lowest red channel inside a rect: text pixels are darker than the fills.
fn min_red(renderer: &Renderer, b: Rect) -> u32 {
    let mut min = 255u32;
    let mut y = b.y as usize;
    while y < (b.y + b.height) as usize {
        let mut x = b.x as usize;
        while x < (b.x + b.width) as usize {
            if let Some(px) = renderer.pixel_at(x, y) {
                min = min.min((px >> 16) & 0xff);
            }
            x += 1;
        }
        y += 1;
    }
    min
}

/// Highest red channel inside a rect (white text on a dark fill).
fn max_red(renderer: &Renderer, b: Rect) -> u32 {
    let mut max = 0u32;
    let mut y = (b.y + b.height * 0.25) as usize;
    while y < (b.y + b.height * 0.75) as usize {
        let mut x = (b.x + b.width * 0.25) as usize;
        while x < (b.x + b.width * 0.75) as usize {
            if let Some(px) = renderer.pixel_at(x, y) {
                max = max.max((px >> 16) & 0xff);
            }
            x += 1;
        }
        y += 1;
    }
    max
}

#[test]
fn unwrapped_centred_and_right_aligned_text_is_painted_inside_its_box() {
    let mut runtime = Runtime::init(
        Scene,
        RunOptions {
            window: zant::platform::platform::WindowOptions {
                width: 240.0,
                height: 120.0,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let mut renderer = Renderer::create_raster(240, 120).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("canvas");
        runtime.render_frame(canvas, 240.0, 120.0, 1.0);
        renderer.end();
    }

    let avatar_bounds = runtime.tree.get(find(&runtime.tree, "avatar")).bounds;
    let solid_bounds = runtime.tree.get(find(&runtime.tree, "avatar-solid")).bounds;
    let right_bounds = runtime.tree.get(find(&runtime.tree, "right")).bounds;
    // primary (0x16) "Z" on the light primary tint (0xe6)
    assert!(min_red(&renderer, avatar_bounds) < 0x80);
    // white "UI" on the primary fill (red 0x16)
    assert!(max_red(&renderer, solid_bounds) > 0xc0);
    // right-aligned black text inside its 160px box
    assert!(min_red(&renderer, right_bounds) < 0x40);
}
