//! The loading indicator of a `loading` button: antd `LoadingOutlined`, a
//! quarter arc turning once per second (`loadingCircle 1s linear infinite`).
//! `button::animate` samples the turn; with motion off it stays still.
#![forbid(unsafe_code)]

use crate::motion::motion;
use crate::renderer::canvas::Canvas;

/// One turn, in milliseconds.
pub const TURN_MS: i64 = 1000;

const SEGMENTS: usize = 12;

/// The rotation at `now` (0..1), 0 while motion is off.
pub fn turn(now: i64) -> f32 {
    if motion::enabled() { turn_at(now) } else { 0.0 }
}

fn turn_at(now: i64) -> f32 {
    now.rem_euclid(TURN_MS) as f32 / TURN_MS as f32
}

/// Paints the arc centred on `(cx, cy)` in a `size`-wide glyph box.
pub fn paint(canvas: Canvas, cx: f32, cy: f32, size: f32, turn: f32, color: u32) {
    // LoadingOutlined: radius 476 and stroke 72 of its 1024 box
    let radius = size * 0.465;
    let stroke = (size * 0.07).max(1.25);
    let start = -std::f32::consts::FRAC_PI_2 + turn * std::f32::consts::TAU;
    let sweep = std::f32::consts::FRAC_PI_2;
    let mut points = [[0.0f32; 2]; SEGMENTS + 1];
    for (i, point) in points.iter_mut().enumerate() {
        let angle = start + sweep * i as f32 / SEGMENTS as f32;
        *point = [cx + radius * angle.cos(), cy + radius * angle.sin()];
    }
    canvas.stroke_path(&points, color, stroke, false);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_turn_wraps_every_second() {
        assert_eq!(super::turn_at(0), 0.0);
        assert!((super::turn_at(250) - 0.25).abs() < 1e-6);
        assert!((super::turn_at(1_750) - 0.75).abs() < 1e-6);
    }
}
