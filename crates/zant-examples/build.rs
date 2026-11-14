//! Generates the TSX side of the TSX frame benchmark
//! (`zant-tsx-frame-bench`) into `$OUT_DIR`.
//!
//! Zant applications get their TSX → Rust step from their own `build.rs`
//! (`zant_build::tsx_app()` / `try_tsx_app()`, the `scaffold/template-ts`
//! shape). The benchmark binary lives inside the `zant-examples` crate, which
//! is not a TSX app, so this build script creates one throwaway app root per
//! benchmark case under `$OUT_DIR` — `src/App.tsx` copied from the real
//! `tsx/corpus/*.tsx` input (or the one adapted page, see [`CASES`]), plus a
//! view file where the case needs one — and compiles it with the same
//! [`zant_build::tsx_app_in`] the template's build script uses. The generated
//! modules are included by `src/bin/zant_tsx_frame_bench/mod.rs`:
//!
//! ```ignore
//! #[allow(clippy::all)] // generated TSX code
//! mod counter_case {
//!     include!(concat!(env!("OUT_DIR"), "/tsx-frame-bench/cases/counter/main.rs"));
//! }
//! ```
//!
//! The cases and the scenarios they are measured with:
//!
//! | case | `src/App.tsx` | view | benchmark scenario |
//! |---|---|---|---|
//! | counter | `counter.tsx` | — | clicks / typing |
//! | todos | `todos.tsx` | — | add / toggle / filter |
//! | list | `ListBench.tsx` (`list_bench.tsx` + same-file rows) | — | virtual list |
//! | local | `local_bench.tsx` | — | local-state page |
//! | markdown | `markdown_bench.tsx` | `MarkdownView.tsx` | streaming markdown |
//! | settings | `settings.tsx` | — | 40-switch settings page |
//!
//! Two cases are adapted: the benchmark driver has to provide the list rows /
//! the streamed Markdown source before a timed frame, but the generated root
//! struct keeps its state fields private, so:
//!
//! - `markdown` keeps `markdown_bench.tsx` as the app root and drives the same
//!   `<Markdown>` through a prop-taking `MarkdownView` component;
//! - `list` builds the same 10 000 rows in the page itself (`ListBench.tsx`,
//!   `list_bench.tsx` with a `makeRows()` initialiser), because the prop-taking
//!   view version hits a `zant-tsx` codegen bug for `<List>` in non-root
//!   components.
//!
//! The generated `<List>` / `<Markdown>` JSX and the code paths are the ones
//! of `list_bench.tsx` / `markdown_bench.tsx`. See `docs/tsx.md` (performance
//! section).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// One benchmark case: the TSX input plus optional view helper files
/// (checked into this crate) that mirror the case with a prop.
struct Case {
    /// Directory name under `$OUT_DIR/tsx-frame-bench/cases`.
    name: &'static str,
    /// The benchmark input (a path from the repository root), copied to
    /// `src/App.tsx`.
    input: &'static str,
    /// Extra files copied into `src/` of the generated app.
    views: &'static [&'static str],
}

/// The benchmark cases and their TSX inputs.
const CASES: &[Case] = &[
    Case {
        name: "counter",
        input: "crates/zant-examples/tsx/corpus/counter.tsx",
        views: &[],
    },
    Case {
        name: "todos",
        input: "crates/zant-examples/tsx/corpus/todos.tsx",
        views: &[],
    },
    Case {
        name: "list",
        // `list_bench.tsx` with the rows built by a same-file function: the
        // driver cannot inject them into the root state (private fields).
        // TODO: use a prop-taking view once `zant-tsx` generates `<List>`
        // correctly in non-root components. The JSX below `makeRows` is the
        // original page's. See the file header.
        input: "crates/zant-examples/src/bin/zant_tsx_frame_bench/tsx_views/ListBench.tsx",
        views: &[],
    },
    Case {
        name: "local",
        input: "crates/zant-examples/tsx/corpus/local_bench.tsx",
        views: &[],
    },
    Case {
        name: "markdown",
        input: "crates/zant-examples/tsx/corpus/markdown_bench.tsx",
        views: &["MarkdownView.tsx"],
    },
    Case {
        name: "settings",
        input: "crates/zant-examples/tsx/corpus/settings.tsx",
        views: &[],
    },
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    // `crates/zant-examples` → repository root (the case inputs are relative to it).
    let repo = manifest
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives at <repo>/crates/zant-examples")
        .to_path_buf();
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("tsx-frame-bench");
    let views = manifest.join("src/bin/zant_tsx_frame_bench/tsx_views");

    for case in CASES {
        // A TSX app root is `<root>/src/App.tsx`; the compiler also picks up
        // the view files next to it. Creating it under `$OUT_DIR` keeps the
        // crate a normal (non-TSX) workspace member.
        let app_root = out.join("apps").join(case.name);
        let src = app_root.join("src");
        // Recreate the app root so a removed view file does not linger in
        // `$OUT_DIR` from an earlier build.
        let _ = std::fs::remove_dir_all(&app_root);
        std::fs::create_dir_all(&src).expect("create the generated app root");
        let input = repo.join(case.input);
        copy_if_changed(&input, &src.join("App.tsx"));
        println!("cargo:rerun-if-changed={}", input.display());
        for view in case.views {
            let source = views.join(view);
            copy_if_changed(&source, &src.join(view));
            println!("cargo:rerun-if-changed={}", source.display());
        }

        // The same TSX → Rust step a Zant app runs in its own build script.
        let generated = out.join("cases").join(case.name);
        if let Err(error) = zant_build::tsx_app_in(&app_root, &generated) {
            panic!("zant-tsx-frame-bench: {}: {error}", case.input);
        }
    }
}

/// Writes `source` to `destination` only when the content differs (the input
/// files are unchanged most builds, and the generated modules are only
/// regenerated when this build script runs).
fn copy_if_changed(source: &Path, destination: &Path) {
    let data =
        std::fs::read(source).unwrap_or_else(|error| panic!("read {}: {error}", source.display()));
    if std::fs::read(destination).is_ok_and(|existing| existing == data) {
        return;
    }
    std::fs::write(destination, data)
        .unwrap_or_else(|error| panic!("write {}: {error}", destination.display()));
}
