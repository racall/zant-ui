//! Command-line behavior of `zant`: help / version / usage text and
//! an end-to-end `create` → `sync` run against the repository's templates.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn zant() -> Command {
    Command::new(env!("CARGO_BIN_EXE_zant"))
}

fn temp_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("zant-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn run(command: &mut Command) -> Output {
    command.output().expect("run zant")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn help_lists_every_command_and_version_prints_the_package_version() {
    let output = run(zant().arg("--help"));
    assert!(output.status.success());
    let text = stdout(&output);
    for command in [
        "create", "run", "compile", "package", "build", "sync", "update",
    ] {
        assert!(
            text.contains(command),
            "{command} missing from --help:\n{text}"
        );
    }
    let output = run(zant().arg("--version"));
    assert!(output.status.success());
    let text = stdout(&output);
    assert_eq!(text.trim(), format!("zant {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn no_arguments_prints_the_usage_text() {
    let output = run(&mut zant());
    assert!(output.status.success());
    let text = stderr(&output);
    assert!(text.starts_with("用法: zant <命令>"), "{text}");
    for line in [
        "create <项目名>",
        "run ",
        "compile [--release]",
        "package",
        "build [配置文件]",
        "sync",
        "update",
    ] {
        assert!(text.contains(line), "{line} missing from usage:\n{text}");
    }
}

#[test]
fn unknown_commands_and_arguments_are_rejected() {
    let output = run(zant().arg("bogus"));
    assert!(!output.status.success());
    assert!(stderr(&output).contains("bogus"), "{}", stderr(&output));
    let output = run(zant().args(["run", "--nope"]));
    assert!(!output.status.success());
    assert!(stderr(&output).contains("--nope"), "{}", stderr(&output));
    // `zant build --release` prints a hint instead of compiling.
    let dir = temp_dir("build-release");
    let output = run(zant().current_dir(&dir).args(["build", "--release"]));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("只编译请用 zant compile [--release]"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn create_then_sync_writes_the_ts_template_and_generated_config() {
    let dir = temp_dir("create");
    let output = run(zant().current_dir(&dir).args([
        "create",
        "demo",
        "--yes",
        "--title",
        "Demo App",
        "--bundle-id",
        "com.acme.demo",
    ]));
    assert!(output.status.success(), "{}", stderr(&output));
    let project = dir.join("demo");
    // A Cargo app with TSX sources and Rust native modules.
    for file in [
        "app.config.json",
        "Cargo.toml",
        "native/build.rs",
        "rust-toolchain.toml",
        "native/main.rs",
        "src/App.tsx",
        "src/components/Dashboard.tsx",
        "native/system.rs",
        "native/system.ts",
        "assets/icons/app-macos.png",
        "zant.d.ts",
        "tsconfig.json",
        "DESIGN.md",
    ] {
        assert!(project.join(file).is_file(), "{file} missing");
    }
    let config = std::fs::read_to_string(project.join("app.config.json")).expect("config");
    // The config text `zant create` writes (window defaults included).
    assert!(config.contains("\"title\": \"Demo App\""), "{config}");
    assert!(
        config.contains("\"bundle_id\": \"com.acme.demo\""),
        "{config}"
    );
    assert!(config.contains("\"width\": 680"), "{config}");
    assert!(config.contains("\"min_height\": 620"), "{config}");
    let template = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scaffold/template-ts/app.config.json"),
    )
    .expect("template");
    // Compare every key in source order, including nested window/macOS fields.
    let keys = |text: &str| {
        text.lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix('"')?
                    .split_once("\":")
                    .map(|(key, _)| key.to_string())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&config), keys(&template));
    let metadata: serde_json::Value = serde_json::from_str(&config).expect("valid JSON");
    assert_eq!(metadata["description"], "Demo App 桌面应用");
    assert_eq!(metadata["company"], "Acme");
    let copyright = metadata["copyright"].as_str().expect("copyright");
    let year = copyright
        .strip_prefix("© ")
        .and_then(|s| s.strip_suffix(" Acme"))
        .expect("copyright includes company and year");
    assert!(year.parse::<u16>().expect("calendar year") >= 1970);
    assert_eq!(
        config,
        template
            .replace("__APP_TITLE__", "Demo App")
            .replace("__BUNDLE_ID__", "com.acme.demo")
            .replace("__COMPANY__", "Acme")
            .replace("__COPYRIGHT_YEAR__", year)
    );
    assert_eq!(
        metadata["macos"]["category"],
        "public.app-category.productivity"
    );
    assert!(metadata["macos"]["sandbox"].is_null());
    assert!(!config.contains("__"), "unexpanded metadata: {config}");
    let manifest = std::fs::read_to_string(project.join("Cargo.toml")).expect("Cargo.toml");
    assert!(manifest.contains("name = \"demo\""), "{manifest}");
    assert!(
        manifest.contains("/crates/zant\""),
        "the zant dependency path is relative to the project:\n{manifest}"
    );

    // `zant sync` generates the per-platform files; no cargo build involved.
    let output = run(zant().current_dir(&project).arg("sync"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!project.join("src/app_config.rs").exists());
    if cfg!(target_os = "macos") {
        assert!(project.join("Info.plist").is_file());
        let config = std::fs::read_to_string(project.join("app.config.json")).expect("config");
        assert!(config.contains("\"macos\""), "{config}");
        assert!(config.contains("\"sandbox\": false"), "{config}");
    }
    let output = run(zant().current_dir(&project).arg("sync"));
    assert!(output.status.success());
    assert!(!project.join("src/app_config.rs").exists());
}

#[test]
fn create_preserves_template_formatting_with_explicit_options() {
    let dir = temp_dir("create-options");
    let sign = "Example \"Signing\" Identity";
    let output = run(zant().current_dir(&dir).args([
        "create",
        "demo",
        "--yes",
        "--bundle-id",
        "com.acme-labs.demo",
        "--style",
        "hidden-titlebar",
        "--sandbox",
        "true",
        "--sign",
        sign,
    ]));
    assert!(output.status.success(), "{}", stderr(&output));
    let config = std::fs::read_to_string(dir.join("demo/app.config.json")).expect("config");
    let metadata: serde_json::Value = serde_json::from_str(&config).expect("valid JSON");
    assert_eq!(metadata["company"], "Acme-labs");
    assert_eq!(metadata["window"]["style"], "hidden-titlebar");
    assert_eq!(metadata["macos"]["sandbox"], true);
    assert_eq!(metadata["macos"]["signing_identity"], sign);
    let year = metadata["copyright"]
        .as_str()
        .unwrap()
        .strip_prefix("© ")
        .unwrap()
        .strip_suffix(" Acme-labs")
        .unwrap();
    let template = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scaffold/template-ts/app.config.json"),
    )
    .expect("template");
    assert_eq!(config, template
        .replace("__APP_TITLE__", "Demo")
        .replace("__BUNDLE_ID__", "com.acme-labs.demo")
        .replace("__COMPANY__", "Acme-labs")
        .replace("__COPYRIGHT_YEAR__", year)
        .replace("\"style\": \"native\"", "\"style\": \"hidden-titlebar\"")
        .replace("\"category\": \"public.app-category.productivity\"", &format!(
            "\"category\": \"public.app-category.productivity\",\n    \"sandbox\": true,\n    \"signing_identity\": {}",
            serde_json::to_string(sign).unwrap()
        )));
}

/// A minimal plain-Rust application project (no TSX, no build script):
/// `zant compile` / `zant run` must build and run it with cargo.
fn write_plain_app(dir: &Path) -> PathBuf {
    let project = dir.join("plain");
    std::fs::create_dir_all(project.join("src")).expect("src dir");
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"demo-app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("Cargo.toml");
    std::fs::write(
        project.join("src/main.rs"),
        "fn main() { println!(\"hello from demo\"); }\n",
    )
    .expect("main.rs");
    std::fs::write(
        project.join("app.config.json"),
        "{\n  \"title\": \"Demo App\",\n  \"bundle_id\": \"com.acme.demo\",\n  \"version\": \"0.1.0\"\n}\n",
    )
    .expect("app.config.json");
    project
}

#[test]
fn compile_and_run_a_plain_rust_app() {
    let dir = temp_dir("plain-rust");
    let project = write_plain_app(&dir);
    let output = run(zant().current_dir(&project).arg("compile"));
    assert!(output.status.success(), "{}", stderr(&output));
    let exe = if cfg!(windows) {
        "target/debug/demo-app.exe"
    } else {
        "target/debug/demo-app"
    };
    assert!(project.join(exe).is_file());
    // ZANT_RAW_LOGS=1 skips the macOS log-filter wrapper: its bash process
    // substitution needs /dev/fd, which restricted sandboxes deny.
    let output = run(zant()
        .current_dir(&project)
        .env("ZANT_RAW_LOGS", "1")
        .arg("run"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("hello from demo"),
        "{}",
        stdout(&output)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn package_a_plain_rust_app_into_a_signed_app_bundle() {
    let dir = temp_dir("plain-package");
    let project = write_plain_app(&dir);
    let output = run(zant().current_dir(&project).arg("package"));
    assert!(output.status.success(), "{}", stderr(&output));
    let app = project.join("target/Demo App.app");
    assert!(app.join("Contents/MacOS/app").is_file());
    let plist = std::fs::read_to_string(app.join("Contents/Info.plist")).expect("Info.plist");
    assert!(plist.contains("<key>CFBundleIdentifier</key><string>com.acme.demo</string>"));
    assert!(stderr(&output).contains("输出: "), "{}", stderr(&output));
}
