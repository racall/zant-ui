//! `zant-md-stream`: the Markdown streaming benchmark. It lives in
//! [`zant_examples::stream_app`] so the headless smoke test can build the same
//! chat the window paints.

fn main() -> zant::Result<()> {
    zant_examples::stream_app::run()
}
