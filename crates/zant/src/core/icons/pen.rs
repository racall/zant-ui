//! `Pen`: the stroke primitives icon outlines are drawn with (lines, arcs, circles, rounded rects).

use super::*;

/// Sugar over [`Sink`] used by [`trace`].
pub(super) struct Pen<'a> {
    pub(super) sink: &'a mut dyn Sink,
}

impl Pen<'_> {
    pub(super) fn line(&mut self, points: &[Point]) {
        self.sink.stroke(points, false);
    }

    pub(super) fn closed(&mut self, points: &[Point]) {
        self.sink.stroke(points, true);
    }

    pub(super) fn dot(&mut self, x: f32, y: f32, r: f32) {
        self.sink.dot(x, y, r);
    }

    /// 32 steps, 33 points (both ends included).
    pub(super) fn arc(&mut self, cx: f32, cy: f32, r: f32, a0_deg: f32, a1_deg: f32) {
        let mut points: [Point; 33] = [[0.0; 2]; 33];
        let steps: usize = 32;
        for (i, point) in points.iter_mut().enumerate() {
            let t = i as f32 / steps as f32;
            let a = (a0_deg + (a1_deg - a0_deg) * t) * RAD_PER_DEG;
            *point = [cx + a.cos() * r, cy + a.sin() * r];
        }
        self.line(&points);
    }

    /// 32 points, closed.
    pub(super) fn circle(&mut self, cx: f32, cy: f32, r: f32) {
        let mut points: [Point; 32] = [[0.0; 2]; 32];
        let len = points.len();
        for (i, point) in points.iter_mut().enumerate() {
            let a = i as f32 / len as f32 * std::f32::consts::TAU;
            *point = [cx + a.cos() * r, cy + a.sin() * r];
        }
        self.closed(&points);
    }

    /// Rounded rectangle outline: 7 points per corner (15° steps), closed.
    pub(super) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let mut points: [Point; 28] = [[0.0; 2]; 28];
        let mut n: usize = 0;
        let corners = [
            [x + w - r, y + r, -90.0],
            [x + w - r, y + h - r, 0.0],
            [x + r, y + h - r, 90.0],
            [x + r, y + r, 180.0],
        ];
        for corner in corners {
            for k in 0..7 {
                let a = (corner[2] + k as f32 * 15.0) * RAD_PER_DEG;
                points[n] = [corner[0] + a.cos() * r, corner[1] + a.sin() * r];
                n += 1;
            }
        }
        self.closed(&points[..n]);
    }
}
