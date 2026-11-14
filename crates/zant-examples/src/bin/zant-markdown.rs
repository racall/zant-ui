//! `zant-markdown`: port of the Zig `zant-markdown` example,
//! `examples/markdown.zig` (WP-3.11). The application lives in
//! [`zant_examples::markdown_app`] so the headless smoke test can build the
//! same tree the window paints. `ZANT_DARK=1` for dark.

fn main() -> zant::Result<()> {
    let dark = std::env::var_os("ZANT_DARK").is_some();
    zant::ui::run(
        zant_examples::markdown_app::MarkdownApp::new(dark),
        zant_examples::markdown_app::run_options(dark),
    )
}
