//! Asset imports (docs/tsx.md, "资源文件"):
//!
//! ```tsx
//! import trayIcon from "../assets/tray.png";   // Uint8Array, embedded at compile time
//! ```
//!
//! Translation of `tools/tsx/assets.zig`. A relative import with an extension
//! other than `.tsx` / `.ts` / `.rs` / `.jsx` / `.js` names a file whose bytes
//! are embedded (`include_bytes!`). The compiler copies it next to the
//! generated Rust as `_assets/<hash>-<name>` (the generated module's
//! `include_bytes!` is resolved relative to the generated file) and lists it in
//! the build's dep file, so editing the asset rebuilds.
//!
//! Generated code:
//!
//! ```ignore
//! const tray_icon: &'static [u8] = include_bytes!("_assets/1a2b3c4d-tray.png");
//! ```
//!
//! The `&'static [u8]` spelling matches what the runtime takes
//! (`tsx::platform::asset_image`, `tsx::desktop::TrayOptions::icon`).

use crate::codegen::app_state;
use crate::codegen::rust_imports;

/// Whether an import names an asset (Zig `assets.isAsset`): a relative import
/// with an extension that is not code.
pub fn is_asset(source: &str) -> bool {
    let relative = source.starts_with("./") || source.starts_with("../");
    if !relative {
        return false;
    }
    let base = source.rsplit(['/', '\\']).next().unwrap_or(source);
    let Some(index) = base.rfind('.') else {
        return false;
    };
    // `.hidden` has no extension (Zig `std.fs.path.extension`)
    if index == 0 {
        return false;
    }
    let extension = &base[index..];
    ![".tsx", ".ts", ".zig", ".rs", ".jsx", ".js"]
        .iter()
        .any(|code| extension.eq_ignore_ascii_case(code))
}

/// The asset's path on disk: relative to the importing file.
pub fn path(importer: &str, source: &str) -> String {
    let directory = match importer.rfind('/') {
        Some(index) => &importer[..index],
        None => ".",
    };
    join(directory, source)
}

/// Where the copy lives in the output directory: one name per file on disk
/// (two imports of it share it), readable in generated code.
pub fn key(importer: &str, source: &str) -> String {
    let full = path(importer, source);
    let normalized = normalize(&full.replace('\\', "/"));
    let hash = zant::core::hash::Wyhash::hash(0, normalized.as_bytes());
    let hash = hash as u32;
    let base = source.rsplit(['/', '\\']).next().unwrap_or(source);
    format!("_assets/{hash:08x}-{base}")
}

/// `std.fs.path.join` for `/`-separated paths.
fn join(directory: &str, source: &str) -> String {
    if directory.is_empty() || directory == "." {
        return source.to_owned();
    }
    if directory.ends_with('/') {
        format!("{directory}{source}")
    } else {
        format!("{directory}/{source}")
    }
}

/// Resolves `.` / `..` components textually (Zig `std.fs.path.resolvePosix`
/// with one relative component): `src/../assets/x.png` → `assets/x.png`.
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => match parts.last() {
                Some(last) if *last != ".." => {
                    parts.pop();
                }
                _ => parts.push(".."),
            },
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// An asset import of one `.tsx` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    /// The importing `.tsx` file and the import's position in it (errors).
    pub importer: String,
    pub importer_src: String,
    pub loc: u32,
    /// Path on disk and the copy's name in the output directory.
    pub from: String,
    pub key: String,
}

/// The asset imports of one module: emits the file-level `const`s and records
/// the copies (Zig `globals.assets`).
pub fn process(
    g: &mut crate::codegen::Generator<'_>,
    module: &crate::ast::Module,
    source_path: Option<&str>,
) -> crate::codegen::Result<String> {
    use crate::codegen::{rust_text, scope, types};

    let mut code = String::new();
    let mut assets: Vec<Asset> = Vec::new();
    for import in &module.imports {
        let source = String::from_utf8_lossy(&import.source).into_owned();
        if !is_asset(&source) {
            continue;
        }
        let name = import.default_name.clone();
        let Some(name) = name else {
            return crate::codegen::fail(
                g,
                import.source_loc,
                "an asset import is a default import: import icon from \"./icon.png\"",
            );
        };
        let Some(importer) = source_path else {
            return crate::codegen::fail(
                g,
                import.source_loc,
                "asset imports need a project build (zant_build::tsx_app, or zant-tsx --out)",
            );
        };
        if scope::global_name(g, &name.name) {
            return crate::codegen::fail(
                g,
                name.loc,
                format!(
                    "duplicate or reserved name `{}`",
                    String::from_utf8_lossy(&name.name)
                ),
            );
        }
        let id = rust_text::id_at(g, &name.name, name.loc)?;
        let key = key(importer, &source);
        let asset = Asset {
            importer: importer.to_owned(),
            importer_src: String::new(),
            loc: import.source_loc,
            from: path(importer, &source),
            key: key.clone(),
        };
        assets.push(asset);
        code.push_str(&format!(
            "const {id}: &'static [u8] = include_bytes!(\"{key}\");\n\n"
        ));
        let ty = rust_imports::bytes(g)?;
        g.constants.push(crate::codegen::Constant {
            name: String::from_utf8_lossy(&name.name).into_owned(),
            value: types::Value {
                ty,
                code: id,
                ..types::Value::default()
            },
        });
    }
    if let Some(project) = app_state::project() {
        for mut asset in assets {
            if let Some(module) = project.module_by_path(&asset.importer) {
                asset.importer_src = module.source.clone();
            }
            project.assets.borrow_mut().push(asset);
        }
    }
    Ok(code)
}

/// A Makefile-style dep file for the build (`addDepFileOutputArg`): every
/// source read (`sources`) and asset embedded; editing one reruns `zant-tsx`,
/// and a deleted one is only a cache miss.
pub fn dep_file(target: &str, sources: &[String], assets: &[Asset]) -> String {
    let mut out = escape(target);
    out.push(':');
    for source in sources {
        out.push_str(" \\\n  ");
        out.push_str(&escape(source));
    }
    for asset in assets {
        out.push_str(" \\\n  ");
        out.push_str(&escape(&asset.from));
    }
    out.push('\n');
    out
}

fn escape(value: &str) -> String {
    value.replace(' ', "\\ ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from `tools/tsx/assets.zig`.

    #[test]
    fn asset_imports_by_extension() {
        assert!(is_asset("../assets/tray.png"));
        assert!(is_asset("./data/words.txt"));
        assert!(!is_asset("./components/Row"));
        assert!(!is_asset("./Row.tsx"));
        assert!(!is_asset("./sync.zig"));
        assert!(!is_asset("./format.rs"));
        assert!(!is_asset("zant"));
        assert!(!is_asset("./.hidden"));
    }

    #[test]
    fn one_key_per_file_on_disk() {
        let from_app = key("src/App.tsx", "../assets/tray.png");
        let from_child = key("src/components/Tray.tsx", "../../assets/tray.png");
        assert_eq!(from_app, from_child);
        assert!(from_app.ends_with("-tray.png"), "{from_app}");
        assert_ne!(from_app, key("src/App.tsx", "./tray.png"));
        assert_eq!("assets/tray.png", normalize("src/../assets/tray.png"));
        assert_eq!(
            "assets/tray.png",
            normalize("src/components/../../assets/tray.png")
        );
    }

    #[test]
    fn dep_file_lists_the_assets() {
        let assets = [Asset {
            importer: String::new(),
            importer_src: String::new(),
            loc: 0,
            from: "my app/tray.png".to_owned(),
            key: String::new(),
        }];
        assert_eq!(
            "out: \\\n  src/App.tsx \\\n  my\\ app/tray.png\n",
            dep_file("out", &["src/App.tsx".to_owned()], &assets)
        );
    }
}
