//! Project language conventions shared by create, sync and the CLI.
//!
//! The pure-Zig template (`zant create --lang zig`) was removed together with
//! the Zig toolchain at M8 (docs/rust-port/m8-cutover.md); TSX is the only
//! application language.

use std::path::Path;

/// Whether `root` is a TSX application, i.e. has `src/App.tsx`. Such a project
/// needs the `build.rs` that calls `zant_build::tsx_app()`.
pub fn is_tsx_app(root: &str) -> bool {
    Path::new(root).join("src").join("App.tsx").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_tsx_app_by_its_app_tsx() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/zant-cli-test/project-detect");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        let root_str = root.to_str().expect("utf8 path");
        assert!(!is_tsx_app(root_str));
        std::fs::write(root.join("src/App.tsx"), "").expect("write App.tsx");
        assert!(is_tsx_app(root_str));
    }
}
