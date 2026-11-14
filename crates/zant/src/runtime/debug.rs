//! Frame statistics (`FrameStats`), the `ZANT_LAYOUT_FILE` writer and the
//! `ZANT_*` debug switches.
//!
//! Frame timing: prints one `[frames]` line per second (frames drawn, slowest
//! frame and its phases, longest gap between frames, resize latency) and one
//! `[slow-frame]` line for frames over half the 60 Hz budget. The
//! `ZANT_LAYOUT_FILE` writer dumps `key\tx\ty\twidth\theight` of every keyed
//! in-flow node after each frame so automation can find controls.
#![forbid(unsafe_code)]

use crate::core::tree::Tree;
use crate::platform::platform;

/// Phases timed inside one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Rebuild,
    Layout,
    Paint,
}

impl Phase {
    fn index(self) -> usize {
        match self {
            Phase::Rebuild => 0,
            Phase::Layout => 1,
            Phase::Paint => 2,
        }
    }
}

/// Frame timing for `ZANT_FRAME_STATS`.
///
/// `label` is `""` for the main window (`[frames]`); later windows use
/// `":2"` (`[frames:2]`), `":3"`, …
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    /// "" for the main window ("[frames]"), ":2", ":3"… for windows opened later.
    pub label: &'static str,
    window_start: i64,
    last_end: i64,
    frames: u32,
    max_frame_us: i64,
    max_gap_us: i64,
    max_phase_us: [i64; 3],
    resize_frames: u32,
    pending_resize: i64,
    max_resize_latency_us: i64,
    /// Phases of the frame being drawn (for the slow-frame line).
    cur_phase_us: [i64; 3],
}

impl FrameStats {
    /// Statistics of a window opened after the main one: the main window
    /// enabled the counters, this one only needs its `ZANT_FRAME_STATS` label
    /// (`":2"`, `":3"`, …).
    pub fn for_window(label: &'static str) -> FrameStats {
        FrameStats {
            label,
            ..Default::default()
        }
    }

    /// Records the duration of one phase of the current frame.
    pub fn phase(&mut self, which: Phase, start_us: i64) {
        let index = which.index();
        let us = platform::monotonic_us() - start_us;
        self.cur_phase_us[index] = us;
        self.max_phase_us[index] = self.max_phase_us[index].max(us);
    }

    /// A window resize was requested (`window_resize` event).
    pub fn resize_requested(&mut self, now_us: i64) {
        if self.pending_resize == 0 {
            self.pending_resize = now_us;
        }
    }

    /// A frame of a new size was drawn.
    pub fn resized(&mut self, now_us: i64) {
        self.resize_frames += 1;
        if self.pending_resize != 0 {
            self.max_resize_latency_us =
                self.max_resize_latency_us.max(now_us - self.pending_resize);
            self.pending_resize = 0;
        }
    }

    fn ms(us: i64) -> f64 {
        us as f64 / 1000.0
    }

    /// Ends one frame: prints the slow-frame line and, once a second, the
    /// per-second summary.
    pub fn frame(&mut self, start: i64, end: i64, width: f32, height: f32) {
        if self.window_start == 0 {
            self.window_start = start;
        }
        if self.last_end != 0 {
            self.max_gap_us = self.max_gap_us.max(start - self.last_end);
        }
        self.last_end = end;
        self.frames += 1;
        self.max_frame_us = self.max_frame_us.max(end - start);
        // frames over half the 60 Hz budget, one line each
        let (text_layouts, text_us) = crate::renderer::text::LAYOUT_STATS.take();
        if end - start > 8_000 {
            eprintln!(
                "[slow-frame{}] ms={:.2} rebuild={:.2} layout={:.2} paint={:.2} text_layouts={} text_ms={:.2}",
                self.label,
                Self::ms(end - start),
                Self::ms(self.cur_phase_us[0]),
                Self::ms(self.cur_phase_us[1]),
                Self::ms(self.cur_phase_us[2]),
                text_layouts,
                Self::ms(text_us),
            );
        }
        self.cur_phase_us = [0; 3];
        let elapsed = end - self.window_start;
        if elapsed < 1_000_000 {
            return;
        }
        let fps = f64::from(self.frames) * 1_000_000.0 / elapsed as f64;
        eprintln!(
            "[frames{}] fps={:.1} max_frame_ms={:.2} (rebuild={:.2} layout={:.2} paint={:.2}) max_gap_ms={:.2} resize_frames={} max_resize_latency_ms={:.2} size={:.0}x{:.0}",
            self.label,
            fps,
            Self::ms(self.max_frame_us),
            Self::ms(self.max_phase_us[0]),
            Self::ms(self.max_phase_us[1]),
            Self::ms(self.max_phase_us[2]),
            Self::ms(self.max_gap_us),
            self.resize_frames,
            Self::ms(self.max_resize_latency_us),
            width,
            height,
        );
        let label = self.label;
        let pending_resize = self.pending_resize;
        *self = FrameStats {
            label,
            last_end: end,
            pending_resize,
            ..FrameStats::default()
        };
    }
}

/// The contents written for `ZANT_LAYOUT_FILE`, one
/// line per keyed in-flow node: `key\tx\ty\twidth\theight` with one decimal
/// (logical px).
pub fn layout_file_contents(tree: &Tree) -> String {
    let mut out = String::new();
    for node in &tree.nodes {
        if !node.alive || !node.in_flow() {
            continue;
        }
        let Some(key) = node.key.as_deref() else {
            continue;
        };
        let b = node.bounds;
        out.push_str(&format!(
            "{key}\t{:.1}\t{:.1}\t{:.1}\t{:.1}\n",
            b.x, b.y, b.width, b.height
        ));
    }
    out
}

/// Dumps the boxes of all keyed nodes (UI automation).
/// Write errors are ignored.
pub fn write_layout_file(tree: &Tree, path: &str) {
    let contents = layout_file_contents(tree);
    let _ = crate::platform::dialog::write_file(path, contents.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;
    use crate::core::node::FlexStyle;
    use crate::core::tree::Tree;
    use crate::theme::tokens::ant_light;

    #[test]
    fn frame_stats_start_zeroed_and_unlabelled() {
        let stats = FrameStats::default();
        assert_eq!(stats.label, "");
        assert_eq!(stats.window_start, 0);
        assert_eq!(stats.frames, 0);
        assert_eq!(stats.max_phase_us, [0; 3]);
        assert_eq!(stats.resize_frames, 0);
        assert_eq!(stats.cur_phase_us, [0; 3]);
        assert_eq!(Phase::Rebuild.index(), 0);
        assert_eq!(Phase::Layout.index(), 1);
        assert_eq!(Phase::Paint.index(), 2);
        assert_eq!(FrameStats::ms(1500), 1.5);
    }

    #[test]
    fn resize_requests_measure_the_latency_of_the_next_frame() {
        let mut stats = FrameStats::default();
        stats.resize_requested(1000);
        // a second request while one is pending keeps the first timestamp
        stats.resize_requested(5000);
        stats.resized(7000);
        assert_eq!(stats.resize_frames, 1);
        assert_eq!(stats.pending_resize, 0);
        assert_eq!(stats.max_resize_latency_us, 6000);
        stats.resized(9000);
        assert_eq!(stats.resize_frames, 2);
        assert_eq!(stats.max_resize_latency_us, 6000);
    }

    /// The `ZANT_LAYOUT_FILE` writer: one tab-separated line per keyed in-flow
    /// node, with one decimal of precision.
    #[test]
    fn layout_file_writes_keyed_in_flow_nodes() {
        let mut tree = Tree::init(ant_light());
        let root = tree.root;
        let keyed = tree.add_container(
            Some(root),
            FlexStyle {
                width: Some(80.0),
                height: Some(20.0),
                ..Default::default()
            },
        );
        tree.set_key(keyed, Some("panel"));
        let unkeyed = tree.add_container(
            Some(root),
            FlexStyle {
                width: Some(10.0),
                height: Some(10.0),
                ..Default::default()
            },
        );
        let hidden = tree.add_container(
            Some(root),
            FlexStyle {
                width: Some(10.0),
                height: Some(10.0),
                ..Default::default()
            },
        );
        tree.set_key(hidden, Some("hidden"));
        tree.get_mut(hidden).visible = false;
        tree.layout(Size {
            width: 200.0,
            height: 100.0,
        });
        let contents = layout_file_contents(&tree);
        assert_eq!(contents, "panel\t0.0\t0.0\t80.0\t20.0\n");
        assert!(!contents.contains("hidden"), "invisible nodes are skipped");
        assert!(!contents.contains("unkeyed"));
        let _ = unkeyed;
        // the writer itself goes through the platform file helper
        let path = std::env::temp_dir().join("zant-wp14-layout.txt");
        let path = path.to_string_lossy().into_owned();
        write_layout_file(&tree, &path);
        let written =
            std::fs::read_to_string(&path).expect("ZANT_LAYOUT_FILE exists after the frame");
        assert_eq!(written, contents);
        let _ = std::fs::remove_file(&path);
    }
}
