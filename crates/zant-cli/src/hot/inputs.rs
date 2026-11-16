//! Content snapshots make notifications a prompt to check, never a rebuild reason.
use super::classify::{Change, classify};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub(super) const CONFIG: [&str; 5] = [
    "Cargo.toml",
    "Cargo.lock",
    "app.config.json",
    "rust-toolchain.toml",
    "native/build.rs",
];

#[derive(Default)]
pub(super) struct Inputs {
    files: BTreeMap<PathBuf, u64>,
    pub assets: BTreeSet<PathBuf>,
}

impl Inputs {
    pub fn scan(root: &Path) -> Self {
        let mut inputs = Self::default();
        let mut sources = Vec::new();
        collect(&root.join("src"), &mut sources);
        collect(&root.join("native"), &mut sources);
        for path in &sources {
            if !matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("tsx" | "ts")
            ) {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(path) else {
                continue;
            };
            let mut parser = zant_tsx::parser::Parser::init(&source);
            let Ok(module) = parser.parse_module() else {
                continue;
            };
            for import in module.imports {
                let import = String::from_utf8_lossy(&import.source);
                if zant_tsx::assets::is_asset(&import) {
                    let importer = path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let asset =
                        zant_tsx::assets::normalize(&zant_tsx::assets::path(&importer, &import));
                    inputs.assets.insert(asset.into());
                }
            }
        }
        sources.extend(CONFIG.map(|path| root.join(path)));
        if let Ok(entries) = std::fs::read_dir(root.join("locales")) {
            sources.extend(entries.flatten().filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_file())
                    .map(|_| entry.path())
            }));
        }
        collect(&root.join("assets/icons"), &mut sources);
        sources.extend(inputs.assets.iter().map(|path| root.join(path)));
        for path in sources {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if classify(&relative, &inputs.assets) == Change::Ignore {
                continue;
            }
            if let Ok(contents) = std::fs::read(&path) {
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                contents.hash(&mut hash);
                inputs.files.insert(relative, hash.finish());
            }
        }
        inputs
    }

    pub fn changes(&self, previous: &Self) -> Vec<(PathBuf, Change)> {
        let assets = self.assets.union(&previous.assets).cloned().collect();
        self.files
            .keys()
            .chain(previous.files.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|path| self.files.get(*path) != previous.files.get(*path))
            .map(|path| (path.clone(), classify(path, &assets)))
            .collect()
    }

    /// Watch parents of imported assets too (including files outside assets/).
    pub fn asset_parents(&self, root: &Path) -> BTreeSet<PathBuf> {
        self.assets
            .iter()
            .filter_map(|path| root.join(path).parent().map(Path::to_path_buf))
            .collect()
    }
}

fn collect(directory: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            {
                collect(&path, paths);
            }
        } else if kind.is_file() {
            paths.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_content_changes_include_additions_and_deletions() {
        let root = std::env::temp_dir().join(format!("zant-hot-locales-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let before = Inputs::scan(&root);
        std::fs::create_dir_all(root.join("locales/sub")).unwrap();
        for path in [
            "sub/en.json",
            "README.md",
            ".en.json",
            "en.json~",
            "en.json.swp",
        ] {
            std::fs::write(root.join("locales").join(path), "ignored").unwrap();
        }
        assert!(Inputs::scan(&root).changes(&before).is_empty());

        let locale = root.join("locales/en.json");
        let content = r#"{"hello":"Hello"}"#;
        std::fs::write(&locale, content).unwrap();
        let added = Inputs::scan(&root);
        let expected = vec![(PathBuf::from("locales/en.json"), Change::HotReload)];
        assert_eq!(added.changes(&before), expected);
        std::fs::write(&locale, content).unwrap();
        assert!(Inputs::scan(&root).changes(&added).is_empty());
        std::fs::write(&locale, r#"{"hello":"Hi"}"#).unwrap();
        let modified = Inputs::scan(&root);
        assert_eq!(modified.changes(&added), expected);
        std::fs::remove_file(&locale).unwrap();
        assert_eq!(Inputs::scan(&root).changes(&modified), expected);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn content_changes_include_additions_deletions_assets_and_batch_priority() {
        let root = std::env::temp_dir().join(format!("zant-hot-inputs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("assets/icons")).unwrap();
        let source = "import badge from '../assets/x.png'; export default function App() { return <View/>; }";
        std::fs::write(root.join("src/App.tsx"), source).unwrap();
        std::fs::write(root.join("assets/x.png"), b"one").unwrap();
        let before = Inputs::scan(&root);
        std::fs::write(root.join("src/App.tsx"), source).unwrap();
        std::fs::write(root.join("README.md"), "ignored").unwrap();
        assert!(Inputs::scan(&root).changes(&before).is_empty());
        std::fs::write(root.join("assets/x.png"), b"two").unwrap();
        assert_eq!(
            Inputs::scan(&root).changes(&before),
            vec![("assets/x.png".into(), Change::HotReload)]
        );
        std::fs::write(root.join("Cargo.lock"), "new").unwrap();
        let after = Inputs::scan(&root);
        let changes = after.changes(&before);
        assert_eq!(
            changes.iter().map(|(_, change)| *change).max(),
            Some(Change::Restart)
        );
        std::fs::remove_file(root.join("Cargo.lock")).unwrap();
        assert_eq!(
            Inputs::scan(&root).changes(&after),
            vec![("Cargo.lock".into(), Change::Restart)]
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
