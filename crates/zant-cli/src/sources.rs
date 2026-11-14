//! Inputs watched by `zant run --watch`. `stamp` is the fingerprint the watch
//! loop uses; the `notify` watcher only triggers a re-check, so regenerated
//! files that end up unchanged do not restart the app.

use std::hash::{Hash, Hasher};
use std::path::Path;

/// Config files that affect the app build (`app.config.json`, `build.rs`,
/// `Cargo.toml`).
pub const WATCH_FILES: [&str; 3] = ["app.config.json", "build.rs", "Cargo.toml"];

/// Directories hashed recursively.
pub const WATCH_DIRS: [&str; 2] = ["src", "native"];

/// Source files under the watched directories (`.rs` / `.tsx` / `.ts`).
pub const SOURCE_EXTENSIONS: [&str; 3] = ["rs", "tsx", "ts"];

fn hash_file_stamp(hasher: &mut impl Hasher, path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return;
    };
    path.to_string_lossy().hash(hasher);
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    modified.hash(hasher);
    metadata.len().hash(hasher);
}

fn hash_tree(hasher: &mut impl Hasher, dir_path: &Path) {
    let Ok(entries) = std::fs::read_dir(dir_path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            hash_tree(hasher, &path);
        } else if file_type.is_file() {
            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if SOURCE_EXTENSIONS.contains(&extension) {
                hash_file_stamp(hasher, &path);
            }
        }
    }
}

/// Fingerprint of everything that affects the app build: the config files
/// and every `.rs` / `.tsx` / `.ts` file under `src/` and `native/` (TS apps'
/// native modules and their specs), any depth.
pub fn stamp(root: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for rel in WATCH_FILES {
        hash_file_stamp(&mut hasher, &Path::new(root).join(rel));
    }
    for sub in WATCH_DIRS {
        hash_tree(&mut hasher, &Path::new(root).join(sub));
    }
    hasher.finish()
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
    fn watch_nested_tsx_edits_additions_removals_and_native_config_inputs() {
        let root = temp_dir("sources-watch");
        let root_str = root.to_str().expect("utf8 path");
        // More than the old limit of eight levels; nested component edits must restart.
        let nested = "src/a/b/c/d/e/f/g/h/i/j/Child.tsx";
        std::fs::create_dir_all(root.join("src/a/b/c/d/e/f/g/h/i/j")).expect("nested dir");
        std::fs::create_dir_all(root.join("native/engine")).expect("native dir");
        let mut previous = stamp(root_str);
        let inputs = [
            "src/App.tsx",
            nested,
            "src/native.rs",
            "app.config.json",
            "build.rs",
            "Cargo.toml",
            "native/clips.ts",
            "native/clips.rs",
            "native/lib.rs",
            "native/engine/store.rs",
        ];
        for file in inputs {
            std::fs::write(root.join(file), "first").expect("write");
            let added = stamp(root_str);
            assert_ne!(added, previous, "{file} addition must change the stamp");
            std::fs::write(root.join(file), "edited and longer").expect("rewrite");
            let edited = stamp(root_str);
            assert_ne!(edited, added, "{file} edit must change the stamp");
            previous = edited;
        }
        std::fs::remove_file(root.join(nested)).expect("remove");
        assert_ne!(stamp(root_str), previous);
    }
}
