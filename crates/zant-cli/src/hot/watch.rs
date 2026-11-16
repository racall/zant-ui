//! Native notifications by default, explicit content polling for restricted hosts.
use super::{Event, inputs::Inputs};
use notify::{RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(super) struct Watches {
    watcher: Box<dyn Watcher>,
    paths: BTreeSet<PathBuf>,
}
impl Watches {
    pub fn start(root: &Path, inputs: &Inputs, tx: mpsc::Sender<Event>) -> Self {
        let handler = move |event: notify::Result<notify::Event>| match event {
            Ok(_) => {
                let _ = tx.send(Event::Changed(Instant::now()));
            }
            Err(error) => eprintln!("[zant] 文件监听失败：{error}"),
        };
        let watcher: Box<dyn Watcher> = if std::env::var_os("ZANT_HOT_POLL").is_some() {
            eprintln!("[zant] 使用 100 ms 内容轮询（ZANT_HOT_POLL）");
            Box::new(
                notify::PollWatcher::new(
                    handler,
                    notify::Config::default()
                        .with_poll_interval(Duration::from_millis(100))
                        .with_compare_contents(true),
                )
                .unwrap_or_else(|error| crate::fail(error.to_string())),
            )
        } else {
            Box::new(
                notify::recommended_watcher(handler)
                    .unwrap_or_else(|error| crate::fail(error.to_string())),
            )
        };
        let mut watches = Self {
            watcher,
            paths: BTreeSet::new(),
        };
        // The root catches creation/removal and atomic replacement of config files,
        // without watching target/, .git/ or docs recursively.
        watches.add(root, RecursiveMode::NonRecursive);
        watches.assets(root, inputs);
        watches
    }

    pub fn assets(&mut self, root: &Path, inputs: &Inputs) {
        // Optional directories may be created after the session starts.
        for (path, mode) in directories(root) {
            self.add(&path, mode);
        }
        for path in inputs.asset_parents(root) {
            if ["src", "native", "assets"]
                .iter()
                .any(|dir| path.starts_with(root.join(dir)))
            {
                continue;
            }
            self.add(&path, RecursiveMode::NonRecursive);
        }
    }

    fn add(&mut self, path: &Path, mode: RecursiveMode) {
        if self.paths.contains(path) {
            return;
        }
        match self.watcher.watch(path, mode) {
            Ok(()) => {
                self.paths.insert(path.to_path_buf());
            }
            Err(error) => eprintln!("[zant] {}: {error}", path.display()),
        }
    }
}

/// Recomputed on each scan so optional directories can appear during a session.
fn directories(root: &Path) -> Vec<(PathBuf, RecursiveMode)> {
    [
        ("src", RecursiveMode::Recursive),
        ("native", RecursiveMode::Recursive),
        ("assets", RecursiveMode::Recursive),
        ("locales", RecursiveMode::NonRecursive),
    ]
    .into_iter()
    .map(|(directory, mode)| (root.join(directory), mode))
    .filter(|(path, _)| path.is_dir())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_include_locales_created_after_start() {
        let root = std::env::temp_dir().join(format!("zant-hot-watches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for directory in ["src", "native", "assets"] {
            std::fs::create_dir_all(root.join(directory)).unwrap();
        }
        let existing: Vec<_> = ["src", "native", "assets"]
            .map(|directory| (root.join(directory), RecursiveMode::Recursive))
            .into();
        assert_eq!(directories(&root), existing);
        std::fs::create_dir_all(root.join("locales/sub")).unwrap();
        let mut expected = existing;
        expected.push((root.join("locales"), RecursiveMode::NonRecursive));
        assert_eq!(directories(&root), expected);
        std::fs::remove_dir_all(root.join("locales")).unwrap();
        expected.pop();
        assert_eq!(directories(&root), expected);
        std::fs::remove_dir_all(root).unwrap();
    }
}
