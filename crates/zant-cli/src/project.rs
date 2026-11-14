//! Port of `tools/zant/project.zig`: project language conventions shared by
//! create, sync and the CLI.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Zig,
    Ts,
}

impl Language {
    pub fn parse(value: &str) -> Option<Language> {
        match value {
            "zig" => Some(Language::Zig),
            "ts" => Some(Language::Ts),
            _ => None,
        }
    }
}

/// A mixed project with a handwritten entry remains a Zig project.
///
/// Deviation: the Rust port keeps the Zig rule (no `src/main.zig`) verbatim;
/// a Rust TSX app uses `src/main.rs` (`include!` of the generated code), so
/// `src/App.tsx` alone is what marks a TS project once WP-6.3 replaces the
/// templates. See the WP-6.1 report.
pub fn detect(root: &str) -> std::io::Result<Language> {
    let app = Path::new(root).join("src").join("App.tsx");
    let main = Path::new(root).join("src").join("main.zig");
    Ok(if exists(&app)? && !exists(&main)? {
        Language::Ts
    } else {
        Language::Zig
    })
}

fn exists(path: &Path) -> std::io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/zant-cli-test")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn language_choices() {
        assert_eq!(Language::parse("zig"), Some(Language::Zig));
        assert_eq!(Language::parse("ts"), Some(Language::Ts));
        assert_eq!(Language::parse("javascript"), None);
    }

    #[test]
    fn detect_ts_only_when_app_tsx_exists_without_a_handwritten_main_zig() {
        let root = temp_dir("project-detect");
        let root_str = root.to_str().expect("utf8 path");
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        assert_eq!(detect(root_str).expect("detect"), Language::Zig);
        std::fs::write(root.join("src/App.tsx"), "").expect("write App.tsx");
        assert_eq!(detect(root_str).expect("detect"), Language::Ts);
        std::fs::write(root.join("src/main.zig"), "").expect("write main.zig");
        assert_eq!(detect(root_str).expect("detect"), Language::Zig);
    }
}
