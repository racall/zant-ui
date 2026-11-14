//! `zant-md-stream`: port of the Zig `zant-md-stream` example,
//! `examples/markdown_stream.zig` (WP-3.11). The streaming benchmark lives in
//! [`zant_examples::stream_app`] so the headless smoke test can build the same
//! chat the window paints.

fn main() -> zant::Result<()> {
    zant_examples::stream_app::run()
}
