//! Command-line behavior of the Rust `zant`: help / version / usage text and
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
fn no_arguments_prints_the_zig_usage_text() {
    let output = run(&mut zant());
    assert!(output.status.success());
    let text = stderr(&output);
    assert!(text.starts_with("用法: zant <命令>"), "{text}");
    for line in [
        "create <项目名>",
        "run [--watch]",
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
    // `zant build --release` keeps the Zig CLI's hint instead of compiling.
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
    // Default language is TS: a Rust Cargo app with TSX sources and Rust
    // native modules.
    for file in [
        "app.config.json",
        "Cargo.toml",
        "build.rs",
        "rust-toolchain.toml",
        "src/main.rs",
        "src/App.tsx",
        "src/components/Dashboard.tsx",
        "src/app_config.rs",
        "native/lib.rs",
        "native/system.rs",
        "native/system.ts",
        "assets/icons/app-macos.png",
        "zant.d.ts",
        "tsconfig.json",
        "DESIGN.md",
    ] {
        assert!(project.join(file).is_file(), "{file} missing");
    }
    for file in [
        "build.zig",
        "build.zig.zon",
        "src/main.zig",
        "native/lib.zig",
        "native/system.zig",
    ] {
        assert!(!project.join(file).exists(), "{file} should not be created");
    }
    let config = std::fs::read_to_string(project.join("app.config.json")).expect("config");
    // Same text `zant create` wrote before (window defaults included).
    assert!(config.contains("\"title\": \"Demo App\""), "{config}");
    assert!(
        config.contains("\"bundle_id\": \"com.acme.demo\""),
        "{config}"
    );
    assert!(config.contains("\"width\": 680"), "{config}");
    assert!(config.contains("\"min_height\": 620"), "{config}");
    assert!(!config.contains("\"macos\""), "{config}");
    let manifest = std::fs::read_to_string(project.join("Cargo.toml")).expect("Cargo.toml");
    assert!(manifest.contains("name = \"demo\""), "{manifest}");
    assert!(
        manifest.contains("/crates/zant\""),
        "the zant dependency path is relative to the project:\n{manifest}"
    );

    // `zant sync` generates the per-platform files; no cargo build involved.
    let output = run(zant().current_dir(&project).arg("sync"));
    assert!(output.status.success(), "{}", stderr(&output));
    let generated = std::fs::read_to_string(project.join("src/app_config.rs")).expect("config rs");
    assert!(
        generated.contains("pub const title: &str = \"Demo App\";"),
        "{generated}"
    );
    assert!(
        generated.contains("style: zant::WindowStyle::Native,"),
        "{generated}"
    );
    if cfg!(target_os = "macos") {
        assert!(project.join("Info.plist").is_file());
        let config = std::fs::read_to_string(project.join("app.config.json")).expect("config");
        assert!(config.contains("\"macos\""), "{config}");
        assert!(config.contains("\"sandbox\": false"), "{config}");
    }
    // Running sync again does not rewrite the generated file (watch mode
    // must not see its own output as an edit).
    let before = std::fs::metadata(project.join("src/app_config.rs"))
        .and_then(|meta| meta.modified())
        .expect("mtime");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let output = run(zant().current_dir(&project).arg("sync"));
    assert!(output.status.success());
    let after = std::fs::metadata(project.join("src/app_config.rs"))
        .and_then(|meta| meta.modified())
        .expect("mtime");
    assert_eq!(before, after);
}

#[test]
fn create_lang_zig_still_copies_the_legacy_template() {
    // Kept selectable until M8 deletes `scaffold/template/`; the CLI cannot
    // build it any more (`cargo build` needs Cargo.toml), so only the file
    // layout is checked here.
    let dir = temp_dir("create-zig");
    let output = run(zant()
        .current_dir(&dir)
        .args(["create", "demo-zig", "--yes", "--lang", "zig"]));
    assert!(output.status.success(), "{}", stderr(&output));
    let project = dir.join("demo-zig");
    for file in [
        "build.zig",
        "build.zig.zon",
        "src/main.zig",
        "src/app_config.rs",
    ] {
        assert!(project.join(file).is_file(), "{file} missing");
    }
    assert!(!project.join("src/App.tsx").exists());
}

/// A minimal plain-Rust application project: cargo build / run must work now
/// (only the TSX pipeline waits for `zant_build::tsx_app()`).
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
    assert!(project.join("target/debug/demo-app").is_file());
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
