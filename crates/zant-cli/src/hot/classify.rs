//! One policy for app-relative changes. Asset membership comes from TSX imports.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Change {
    Ignore,
    HotReload,
    Restart,
}

pub(super) fn classify(path: &Path, imported_assets: &BTreeSet<PathBuf>) -> Change {
    let parts: Vec<_> = path.components().collect();
    if path.starts_with("target")
        || path.starts_with("docs")
        || path == Path::new("tsconfig.json")
        || path == Path::new("README.md")
    {
        return Change::Ignore;
    }
    if parts.iter().any(|part| match part {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            name.starts_with('.')
                || name.ends_with('~')
                || [".swp", ".swo", ".tmp", ".bak"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
        }
        _ => true,
    }) {
        return Change::Ignore;
    }
    if [
        "Cargo.toml",
        "Cargo.lock",
        "app.config.json",
        "native/build.rs",
        "rust-toolchain.toml",
    ]
    .iter()
    .any(|file| path == Path::new(file))
        || path.starts_with("assets/icons")
    {
        return Change::Restart;
    }
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    if path.starts_with("locales") {
        return if parts.len() == 2 && extension == "json" {
            Change::HotReload
        } else {
            Change::Ignore
        };
    }
    if (path.starts_with("src") && matches!(extension, "tsx" | "ts"))
        || (path.starts_with("native") && matches!(extension, "rs" | "ts"))
        || imported_assets.contains(path)
    {
        Change::HotReload
    } else {
        Change::Ignore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_kind_has_one_policy() {
        let assets = BTreeSet::from([
            PathBuf::from("assets/x.png"),
            PathBuf::from("src/badge.svg"),
        ]);
        for (expected, paths) in [
            (
                Change::HotReload,
                vec![
                    "src/App.tsx",
                    "src/components/deep/Child.tsx",
                    "src/lib/data.ts",
                    "native/main.rs",
                    "native/engine/x.rs",
                    "native/system.ts",
                    "assets/x.png",
                    "src/badge.svg",
                    "locales/en.json",
                    "locales/zh-CN.json",
                ],
            ),
            (
                Change::Restart,
                vec![
                    "Cargo.toml",
                    "Cargo.lock",
                    "app.config.json",
                    "native/build.rs",
                    "rust-toolchain.toml",
                    "assets/icons/app-macos.png",
                    "assets/icons/app-windows.png",
                ],
            ),
            (
                Change::Ignore,
                vec![
                    "tsconfig.json",
                    "README.md",
                    "docs/tsx.md",
                    "target/debug/main.rs",
                    ".git/config",
                    ".DS_Store",
                    "src/.DS_Store",
                    "src/App.tsx.swp",
                    "native/x.rs.swo",
                    "src/App.tsx~",
                    "assets/icons/app-macos.png~",
                    "assets/x.png.tmp",
                    "assets/unimported.png",
                    "src/legacy.rs",
                    "native/data.json",
                    "assets/icons/.icon.png.swp",
                    "locales/sub/en.json",
                    "locales/README.md",
                    "locales/.en.json",
                    "locales/en.json~",
                    "locales/en.json.swp",
                ],
            ),
        ] {
            for path in paths {
                assert_eq!(classify(Path::new(path), &assets), expected, "{path}");
            }
        }
        // Even imported editor/ignored files never trigger a build.
        for path in [
            "target/x.png",
            "docs/x.png",
            "assets/.DS_Store",
            "README.md",
            "tsconfig.json",
            "locales/sub/en.json",
            "locales/README.md",
        ] {
            let assets = BTreeSet::from([PathBuf::from(path)]);
            assert_eq!(classify(Path::new(path), &assets), Change::Ignore);
        }
    }
}
