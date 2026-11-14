//! `zant-pdf`: the PDF reader example. The reader app lives in
//! [`zant_examples::pdf_app`] so the headless smoke test can open the fixture
//! and build the same tree the window paints.

fn main() -> zant::Result<()> {
    zant_examples::pdf_app::run()
}
