//! `zant create`'s scaffold: copies the application template, substituting
//! project metadata only in text files.

use std::path::Path;

/// The `zant create` template: a Rust application (`Cargo.toml` + `build.rs` +
/// `rust-toolchain.toml`). `src/` holds only TS/TSX, `native/` the Rust native
/// modules, and `zant_build::tsx_app()` compiles the TSX in `build.rs`.
pub const TEMPLATE_TS: &str = "scaffold/template-ts";

pub struct Options {
    pub framework: String,
    pub target: String,
    pub title: String,
    pub bundle: String,
    /// The Cargo package name (snake_case).
    pub crate_name: String,
    pub dependency: String,
    pub hidden_titlebar: bool,
}

pub fn write(options: &Options) -> std::io::Result<()> {
    let source = Path::new(&options.framework).join(TEMPLATE_TS);
    copy_tree(&source, Path::new(&options.target), options, true)?;
    // `zant.d.ts` is generated from the Rust tag table (`zant-tsx-dts`), so
    // it is copied from the compiler crate instead of being kept in the
    // template (a template copy would go stale).
    let types = Path::new(&options.framework).join("crates/zant-tsx/zant.d.ts");
    let dest_types = Path::new(&options.target).join("zant.d.ts");
    let declarations = crate::read_file(&types)?;
    crate::write_file(&dest_types, &declarations)?;
    Ok(())
}

fn copy_tree(
    source: &Path,
    dest: &Path,
    options: &Options,
    substitute: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // app.config.json is written by create after validating all options.
        if name == "app.config.json" {
            continue;
        }
        let from = source.join(&*name);
        let to = dest.join(&*name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let text = substitute && name != "assets";
            copy_tree(&from, &to, options, text)?;
        } else if file_type.is_file() {
            let data = crate::read_file(&from)?;
            let output = if substitute {
                expand(
                    &String::from_utf8_lossy(&data),
                    options,
                    name.ends_with(".tsx"),
                )
            } else {
                data
            };
            crate::write_file(&to, &output)?;
        }
    }
    Ok(())
}

fn expand(source: &str, options: &Options, is_tsx: bool) -> Vec<u8> {
    let fingerprint = (u64::from(crc32(options.crate_name.as_bytes())) << 32) | 0x9e37_79b9;
    let hex = format!("0x{fingerprint:016x}");
    let top = if options.hidden_titlebar {
        "top: 56"
    } else {
        "top: 24"
    };
    let pairs: [(&str, &str); 7] = [
        ("__APP_NAME_SNAKE__", &options.crate_name),
        ("__APP_FINGERPRINT__", &hex),
        ("__ZANT_PATH__", &options.dependency),
        ("__APP_TITLE__", &options.title),
        ("__BUNDLE_ID__", &options.bundle),
        ("top: 24 /* titlebar */", top),
        (
            "__TOP_PADDING__",
            if options.hidden_titlebar { "56" } else { "24" },
        ),
    ];
    let mut output = source.to_string();
    for (needle, value) in pairs {
        // Titles in source must be escaped as string literals, unlike Markdown.
        let value = if is_tsx && needle == "__APP_TITLE__" {
            crate::escape_json(value)
        } else {
            value.to_string()
        };
        output = output.replace(needle, &value);
    }
    output.into_bytes()
}

/// CRC-32 (IEEE) of the crate name, used for the project fingerprint.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("repo root")
            .to_string_lossy()
            .into_owned()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("target/zant-cli-test")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn options(target: &std::path::Path) -> Options {
        Options {
            framework: repo_root(),
            target: target.to_string_lossy().into_owned(),
            title: "My \"App\"".to_string(),
            bundle: "com.acme.myapp".to_string(),
            crate_name: "my_app".to_string(),
            dependency: "../zant-ui".to_string(),
            hidden_titlebar: true,
        }
    }

    #[test]
    fn template_directory_exists() {
        assert!(Path::new(&repo_root()).join(TEMPLATE_TS).is_dir());
    }

    #[test]
    fn copies_the_rust_ts_template_and_substitutes_metadata() {
        let dir = temp_dir("scaffold-ts");
        let target = dir.join("my-ts-app");
        write(&options(&target)).expect("scaffold");
        // A complete Rust application: Cargo manifest + build script, the TSX
        // sources, the Rust native modules and the shared icons / types.
        for file in [
            "Cargo.toml",
            "native/build.rs",
            "rust-toolchain.toml",
            ".gitignore",
            "native/main.rs",
            "src/App.tsx",
            "src/components/Dashboard.tsx",
            "native/system.rs",
            "native/system.ts",
            "assets/icons/app-macos.png",
            "assets/icons/tray-dark.png",
            "zant.d.ts",
            "DESIGN.md",
        ] {
            assert!(target.join(file).is_file(), "{file} missing");
        }
        // app.config.json is written by create, not copied from the template
        assert!(!target.join("app.config.json").exists());
        let manifest = std::fs::read_to_string(target.join("Cargo.toml")).expect("Cargo.toml");
        assert!(manifest.contains("name = \"my_app\""), "{manifest}");
        assert!(
            manifest.contains("path = \"../zant-ui/crates/zant\""),
            "{manifest}"
        );
        assert!(
            manifest.contains("path = \"../zant-ui/crates/zant-build\""),
            "{manifest}"
        );
        assert!(manifest.contains("[workspace]"), "{manifest}");
        let build_rs =
            std::fs::read_to_string(target.join("native/build.rs")).expect("native/build.rs");
        assert!(build_rs.contains("zant_build::tsx_app()"), "{build_rs}");
        let main = std::fs::read_to_string(target.join("native/main.rs")).expect("main.rs");
        assert_eq!(main, "zant::app!();\n");
        let readme = std::fs::read_to_string(target.join("README.md")).expect("readme");
        assert!(readme.contains("com.acme.myapp"), "{readme}");
        assert!(readme.contains("My \"App\""), "{readme}");
        // Only substitutes in text files, not in the binary icon assets.
        let icon = std::fs::read(target.join("assets/icons/app-macos.png")).expect("icon");
        let original = std::fs::read(
            Path::new(&repo_root()).join("scaffold/template-ts/assets/icons/app-macos.png"),
        )
        .expect("original icon");
        assert_eq!(icon, original);
        // TSX string literals get JSON-escaped titles; hidden-titlebar
        // projects get the title-bar padding.
        let app = std::fs::read_to_string(target.join("src/App.tsx")).expect("App.tsx");
        assert!(app.contains("My \\\"App\\\""), "{app}");
        assert!(app.contains("top: 56"), "{app}");
        assert_no_placeholders(&target);
    }

    /// No `zant create` placeholder may survive in a generated project.
    fn assert_no_placeholders(dir: &Path) {
        let mut pending = vec![dir.to_path_buf()];
        while let Some(path) = pending.pop() {
            for entry in std::fs::read_dir(&path).expect("read dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                let data = std::fs::read(&path).expect("read file");
                for needle in [
                    "__APP_NAME_SNAKE__",
                    "__APP_FINGERPRINT__",
                    "__ZANT_PATH__",
                    "__APP_TITLE__",
                    "__BUNDLE_ID__",
                    "__TOP_PADDING__",
                ] {
                    let found = data
                        .windows(needle.len())
                        .any(|window| window == needle.as_bytes());
                    assert!(!found, "{} still contains {needle}", path.display());
                }
            }
        }
    }
}
