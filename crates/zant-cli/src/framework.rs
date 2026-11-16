//! The project's Zant UI dependency (`git` + `tag` on GitHub) against the
//! installed release: warn when they differ, retag after `zant update`.

use std::path::Path;

/// The Zant UI repository generated projects depend on.
pub const FRAMEWORK_GIT: &str = "https://github.com/racall/zant-ui";

/// The installed release's tag (`v` + this CLI's version).
pub fn installed_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn is_framework_git(url: &str) -> bool {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .ends_with("racall/zant-ui")
}

/// `tag` of the manifest's `zant` dependency when it is the Zant UI git
/// repository; `None` for path / registry dependencies.
pub fn project_tag(manifest: &str) -> Option<String> {
    let table: toml::Table = manifest.parse().ok()?;
    let zant = table.get("dependencies")?.get("zant")?.as_table()?;
    if !is_framework_git(zant.get("git")?.as_str()?) {
        return None;
    }
    zant.get("tag")?.as_str().map(str::to_owned)
}

/// The manifest with every Zant UI git dependency's `tag` set to `tag`
/// (`zant`, `zant-build`); `None` when nothing changed. Line-based, so the
/// rest of the file keeps its formatting and comments.
pub fn retag(manifest: &str, tag: &str) -> Option<String> {
    let mut changed = false;
    let mut out = String::with_capacity(manifest.len());
    for line in manifest.split_inclusive('\n') {
        let replaced = (line.contains("racall/zant-ui") && line.contains("git"))
            .then(|| replace_tag(line, tag))
            .flatten();
        match replaced {
            Some(new) if new != line => {
                changed = true;
                out.push_str(&new);
            }
            _ => out.push_str(line),
        }
    }
    changed.then_some(out)
}

fn replace_tag(line: &str, tag: &str) -> Option<String> {
    let key = line.find("tag")?;
    let open = key + line[key..].find('"')? + 1;
    let close = open + line[open..].find('"')?;
    Some(format!("{}{tag}{}", &line[..open], &line[close..]))
}

/// Once per command: tells the user when the project builds against another
/// Zant UI version than the installed one (hot reload always uses the
/// installed framework, and the prebuilt native library only matches its own
/// version, so the native layer would be compiled from source).
pub fn warn_if_stale(root: &str) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if crate::release_home().is_none() {
        return;
    }
    ONCE.call_once(|| {
        let manifest = std::fs::read_to_string(Path::new(root).join("Cargo.toml")).unwrap_or_default();
        let installed = installed_tag();
        if let Some(tag) = project_tag(&manifest).filter(|tag| *tag != installed) {
            crate::dim_line(&tr!(en: "Project requires Zant UI {tag}, but {installed} is installed: version mismatches rebuild the native layer from source; hot reload uses the installed version. Run zant update or set tag = \"{installed}\" in Cargo.toml", zh: "项目依赖 Zant UI {tag}，已安装 {installed}：版本不一致时原生层要从源码编译，热刷新也会用已安装的版本。运行 zant update 或把 Cargo.toml 里的 tag 改成 {installed}",
             installed = installed, tag = tag));
        }
    });
}

/// After an update: points the project in `dir` (if it is one with a Zant
/// UI git dependency) at the newly installed release `tag`.
pub fn retag_project(dir: &Path, tag: &str) {
    let path = dir.join("Cargo.toml");
    let Ok(manifest) = std::fs::read_to_string(&path) else {
        return;
    };
    let Some(old) = project_tag(&manifest) else {
        return;
    };
    if let Some(updated) = retag(&manifest, tag) {
        match std::fs::write(&path, updated) {
            Ok(()) => crate::ok(
                &tr!(en: "Project Zant UI dependency: {old} → {tag}", zh: "项目的 Zant UI 依赖：{old} → {tag}", old = old, tag = tag),
            ),
            Err(error) => crate::dim_line(
                &tr!(en: "Could not update {}: {error}", zh: "没能更新 {}：{error}", path.display(), error = error),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"[package]
name = "demo"

[dependencies]
zant = { git = "https://github.com/racall/zant-ui", tag = "v0.2.4" }
serde = "1"

[build-dependencies]
# the TSX compiler
zant-build = { git = "https://github.com/racall/zant-ui", tag = "v0.2.4" }
"#;

    #[test]
    fn reads_the_framework_tag() {
        assert_eq!(project_tag(MANIFEST).as_deref(), Some("v0.2.4"));
        assert_eq!(
            project_tag("[dependencies]\nzant = { path = \"../zant/crates/zant\" }\n"),
            None
        );
        assert_eq!(
            project_tag(
                "[dependencies]\nzant = { git = \"https://github.com/other/zant\", tag = \"v1\" }\n"
            ),
            None
        );
    }

    #[test]
    fn retags_both_dependencies_and_keeps_the_rest() {
        let updated = retag(MANIFEST, "v0.3.0").expect("changed");
        assert_eq!(updated.matches("tag = \"v0.3.0\"").count(), 2, "{updated}");
        assert!(!updated.contains("v0.2.4"), "{updated}");
        assert!(updated.contains("# the TSX compiler\n"), "{updated}");
        assert!(updated.contains("serde = \"1\"\n"), "{updated}");
        assert_eq!(retag(&updated, "v0.3.0"), None);
    }
}
