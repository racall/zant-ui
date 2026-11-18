//! Tells the build scripts of the apps that depend on `zant` which optional
//! parts are compiled in (`links = "zant"`):
//!
//! - `DEP_ZANT_FEATURES`: the optional features that are on, comma separated
//!   (`zant_build` passes them to the TSX compiler, which reports a
//!   `<Markdown>` without `markdown` at the tag instead of failing in rustc);
//! - `DEP_ZANT_PDF`: `1` with the `pdf` feature, `0` without (`zant_build`
//!   only ships `pdfium.dll` for the former).

const OPTIONAL: [&str; 5] = ["fetch", "markdown", "math", "pdf", "db"];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let on: Vec<&str> = OPTIONAL
        .iter()
        .copied()
        .filter(|feature| {
            std::env::var_os(format!("CARGO_FEATURE_{}", feature.to_ascii_uppercase())).is_some()
        })
        .collect();
    println!("cargo:features={}", on.join(","));
    println!("cargo:pdf={}", u8::from(on.contains(&"pdf")));
}
