//! Granularity of `on_resize` reports.

/// How coarse a container's `on_resize` reports are: a size is reported when
/// it moves into another `width` / `height` step (`floor(size / step)`), and
/// a step of 0 ignores that axis. Without one, every change of half a point
/// is reported.
///
/// Size-dependent UI that only changes at steps (blocks fitted to a width in
/// 16 pt steps, a column count) uses it so a live window resize or content
/// that grows taller does not rebuild the app on every frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeStep {
    pub width: f32,
    pub height: f32,
}

impl ResizeStep {
    /// Reports width changes across `step` points only; height is ignored.
    pub const fn width(step: f32) -> ResizeStep {
        ResizeStep {
            width: step,
            height: 0.0,
        }
    }

    /// Whether `now` is in another step than `reported` (`(width, height)`).
    pub fn changed(self, reported: (f32, f32), now: (f32, f32)) -> bool {
        fn bucket(size: f32, step: f32) -> f32 {
            (size / step).floor()
        }
        (self.width > 0.0 && bucket(reported.0, self.width) != bucket(now.0, self.width))
            || (self.height > 0.0 && bucket(reported.1, self.height) != bucket(now.1, self.height))
    }
}

#[cfg(test)]
mod tests {
    use super::ResizeStep;

    #[test]
    fn reports_only_step_crossings_of_the_watched_axes() {
        let step = ResizeStep::width(16.0);
        // the first layout (nothing reported yet: -1) always reports
        assert!(step.changed((-1.0, -1.0), (300.0, 40.0)));
        // within one 16 pt step, and any height change: no report
        assert!(!step.changed((300.0, 40.0), (303.9, 900.0)));
        assert!(step.changed((300.0, 40.0), (304.0, 40.0)));
        assert!(step.changed((300.0, 40.0), (287.0, 40.0)));
        let both = ResizeStep {
            width: 10.0,
            height: 10.0,
        };
        assert!(both.changed((5.0, 5.0), (5.0, 12.0)));
        assert!(!both.changed((5.0, 5.0), (9.0, 9.0)));
    }
}
