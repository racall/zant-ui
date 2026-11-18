//! Unit tests.

use super::*;
use crate::renderer::renderer::Renderer;

#[test]
fn every_icon_traces() {
    for icon in Icon::ALL {
        let drawing = drawing(icon, 18.0, 0xff00_0000, None);
        if icon != Icon::None {
            assert!(
                !drawing.commands.is_empty(),
                "{icon:?} produced no commands"
            );
        }
    }
}

#[test]
fn painting_moves_pixels() {
    let mut renderer = Renderer::create_raster(64, 64).expect("raster renderer");
    {
        let canvas = renderer.begin(1.0).expect("frame canvas");
        canvas.clear(0xffff_ffff);
        paint(canvas, Icon::Check, 32.0, 32.0, 32.0, 0xff00_0000, None);
    }
    renderer.end();
    let mut changed = 0;
    for y in 0..64 {
        for x in 0..64 {
            if renderer.pixel_at(x, y) != Some(0xffff_ffff) {
                changed += 1;
            }
        }
    }
    assert!(
        changed > 0,
        "painting the icon must move at least one pixel"
    );
}
