//! The `zant` command line tool.
//!
//! Apps are built with `cargo build`; their binaries are
//! `target/{debug,release}/<name>`.
//!
//! Shared build and filesystem helpers; command parsing lives in `command.rs`.

#[macro_use]
mod lang;

mod app_config;
mod bundle;
mod command;
mod console;
mod create;
pub(crate) use console::filter_stderr;
pub use console::{
    is_tty, prompt, prompt_yes_no, raw_logs, read_line, read_secret, run_app, spawn_app,
};
mod declarations;
mod distribute;
mod framework;
mod generate;
mod hot;
#[allow(dead_code)]
mod icon;
/// PNG decode / resize / `.ico`: the same code that gives the executable its
/// icon (`zant_build::png`), so both icons match.
use zant_build::png;
mod project;
mod run;
mod scaffold;
mod sources;
mod update;

use std::path::{Path, PathBuf};
use std::process::Command as Proc;
use std::sync::{Mutex, OnceLock};

pub const COLOR_CYAN: &str = "\x1b[36m";
pub const COLOR_GREEN: &str = "\x1b[32m";
pub const COLOR_RED: &str = "\x1b[31m";
pub const COLOR_DIM: &str = "\x1b[2m";
pub const COLOR_RESET: &str = "\x1b[0m";

/// The platform this zant runs on.
pub const IS_WINDOWS: bool = cfg!(windows);

pub fn step(text: &str) {
    eprintln!("  {COLOR_CYAN}→{COLOR_RESET} {text}");
}

pub fn ok(text: &str) {
    eprintln!("  {COLOR_GREEN}✓{COLOR_RESET} {text}");
}

/// A dim hint line.
pub fn dim_line(text: &str) {
    eprintln!("  {COLOR_DIM}{text}{COLOR_RESET}");
}

/// `zant build` removes temporary key files / keychains through this hook
/// when it fails.
static ON_FAIL: Mutex<Option<Box<dyn FnOnce() + Send>>> = Mutex::new(None);

pub fn set_on_fail(callback: Box<dyn FnOnce() + Send>) {
    *ON_FAIL.lock().expect("on_fail lock") = Some(callback);
}

pub fn clear_on_fail() {
    *ON_FAIL.lock().expect("on_fail lock") = None;
}

pub fn run_on_fail() {
    let callback = ON_FAIL.lock().expect("on_fail lock").take();
    if let Some(callback) = callback {
        callback();
    }
}

pub fn fail(message: impl std::fmt::Display) -> ! {
    run_on_fail();
    eprintln!("\n  {COLOR_RED}✗{COLOR_RESET} {message}\n");
    std::process::exit(1);
}

// ---- files ----

/// Creates the parent directory, then writes the bytes.
pub fn write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)
}

/// Reads a whole file; anything over 8 MiB is an error.
pub fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    if data.len() > 8 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file larger than 8 MiB",
        ));
    }
    Ok(data)
}

pub fn read_text(path: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&read_file(path)?).into_owned())
}

pub fn path_exists(path: &Path) -> bool {
    std::fs::metadata(path).is_ok()
}

/// Reads `app.config.json`, exiting with a readable message when it is
/// missing or invalid.
pub fn load_config(root: &str) -> std::io::Result<app_config::AppConfig> {
    let path = Path::new(root).join("app.config.json");
    let data =
        read_file(&path).unwrap_or_else(|_| fail(tr!(en: "app.config.json not found; run zant create first", zh: "找不到 app.config.json（请先运行 zant create）")));
    let text = String::from_utf8_lossy(&data);
    match app_config::parse(&text, app_config::Os::current()) {
        Ok(cfg) => Ok(cfg),
        Err(app_config::ParseError::InvalidJson(why)) => fail(
            tr!(en: "Could not parse app.config.json: {why}", zh: "app.config.json 解析失败: {why}", why = why),
        ),
        Err(app_config::ParseError::InvalidEntitlements) => fail(
            tr!(en: "app.config.json macos.entitlements must be an object, e.g. { \"com.apple.security.network.client\": true }", zh: "app.config.json 的 macos.entitlements 要是对象，如 { \"com.apple.security.network.client\": true }"),
        ),
        Err(app_config::ParseError::InvalidBundleId(why)) => fail(
            tr!(en: "Invalid bundle_id in app.config.json: {why} (e.g. com.xx.test)", zh: "app.config.json 的 bundle_id 不符合规范：{why}（如 com.xx.test）",
         why = app_config::bundle_reason(&why)),
        ),
    }
}

// ---- child processes ----

fn base_command(program: &str) -> Proc {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Proc::new(program);
    // PowerShell 7 puts its own module folders in PSModulePath; a Windows
    // PowerShell 5.1 started from here would then load PowerShell 7's
    // Microsoft.PowerShell.Utility and miss Get-FileHash. Without the
    // variable each PowerShell uses its own default.
    #[cfg(windows)]
    command.env_remove("PSModulePath");
    command
}

/// Runs a command with the terminal, exiting on spawn / wait / exit failure.
pub fn run_inherit(argv: &[String], cwd: &str) {
    let mut command = base_command(&argv[0]);
    command.args(&argv[1..]).current_dir(cwd);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            fail(tr!(en: "Could not start {}: {err}", zh: "无法启动 {}: {err}", argv[0], err = err))
        }
    };
    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(_) => fail(tr!(en: "{} failed", zh: "{} 执行失败", argv[0])),
        Err(err) => fail(
            tr!(en: "Could not wait for {}: {err}", zh: "等待 {} 失败: {err}", argv[0], err = err),
        ),
    }
}

#[derive(Debug, Default)]
pub struct Captured {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Runs a command and captures its stdout / stderr; no failure exit.
pub fn run_capture(argv: &[String]) -> std::io::Result<Captured> {
    let output = base_command(&argv[0]).args(&argv[1..]).output()?;
    Ok(Captured {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        success: output.status.success(),
    })
}

/// Runs a command with the terminal; `false` when it fails (no exit).
pub fn run_ok(argv: &[String], cwd: &str) -> bool {
    let mut command = base_command(&argv[0]);
    command.args(&argv[1..]).current_dir(cwd);
    match command.status() {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

// ---- cargo ----

static CARGO_EXE: OnceLock<String> = OnceLock::new();

/// The cargo to build apps with: `$ZANT_CARGO`, else `cargo` from PATH (it
/// always comes from the environment that ran zant).
pub fn cargo_exe() -> &'static str {
    CARGO_EXE.get_or_init(|| match std::env::var("ZANT_CARGO") {
        Ok(value) if !value.is_empty() => value,
        _ => "cargo".to_string(),
    })
}

/// Checks that cargo is usable, with a clear error instead of a spawn
/// failure.
pub fn require_cargo() -> &'static str {
    let cargo = cargo_exe();
    match run_capture(&[cargo.to_string(), "--version".to_string()]) {
        Ok(captured) if captured.success => cargo,
        _ => fail(
            tr!(en: "{cargo} not found: install the Rust toolchain (https://rustup.rs) or set ZANT_CARGO", zh: "找不到 {cargo}：请先安装 Rust 工具链（https://rustup.rs），或设置 ZANT_CARGO",
         cargo = cargo),
        ),
    }
}

/// `cargo build` / `cargo build --release` for the app.
pub fn build_args(release: bool) -> Vec<String> {
    let mut args = vec![require_cargo().to_string(), "build".to_string()];
    if release {
        args.push("--release".to_string());
    }
    args
}

/// Builds the app in `root` without exiting; `false` on failure.
pub fn build_once(root: &str, release: bool) -> bool {
    if ensure_rust_project(root).is_err() {
        return false;
    }
    run_ok(&build_args(release), root)
}

/// `zant run` / `compile` / `package` build with `cargo build`, so the project
/// needs the Rust application template (Cargo.toml + build.rs). A TSX
/// app's `build.rs` calls `zant_build::tsx_app()`, which compiles
/// `src/**/*.tsx` into `$OUT_DIR`; without that build script cargo would fail
/// with a confusing "file not found for module app_config" / missing `App`
/// error. Plain-Rust-app projects build normally.
pub fn ensure_rust_project(root: &str) -> std::io::Result<()> {
    if !Path::new(root).join("Cargo.toml").is_file() {
        fail(
            tr!(en: "{root} has no Cargo.toml: zant run/compile/package build Cargo apps. Run zant create to generate a TSX project", zh: "{root} 没有 Cargo.toml：zant run/compile/package 用 cargo build 构建 Cargo 应用；请用 zant create 生成 TSX 项目",
         root = root),
        );
    }
    if project::is_tsx_app(root) && !Path::new(root).join("native/build.rs").is_file() {
        fail(
            tr!(en: "{root} is a TSX app (src/App.tsx) without native/build.rs: build.rs must call `zant_build::tsx_app()` to generate Rust from src/**/*.tsx into OUT_DIR, included by zant::app!() in native/main.rs. Run `zant create` or copy scaffold/template-ts/native/build.rs", zh: "{root} 是 TSX 应用（src/App.tsx）但没有 native/build.rs：TSX → Rust 的编译由应用的 build.rs 调用 `zant_build::tsx_app()` 完成（把 src/**/*.tsx 生成到 OUT_DIR，由 native/main.rs 的 zant::app!() 引入）；请用 `zant create` 生成模板，或参照 scaffold/template-ts/native/build.rs 补上 build.rs",
         root = root),
        );
    }
    Ok(())
}

// ---- app executable ----

#[derive(Debug, Clone)]
pub struct AppTarget {
    /// The binary name (`target/{debug,release}/<name>`).
    pub name: String,
    pub target_dir: PathBuf,
}

/// The app's binary name and target directory, read from `cargo metadata`
/// (falling back to a small Cargo.toml scan when cargo cannot be asked).
pub fn app_target(root: &str) -> AppTarget {
    let manifest = Path::new(root).join("Cargo.toml");
    let manifest_arg = manifest.to_string_lossy().into_owned();
    if let Ok(captured) = run_capture(&[
        cargo_exe().to_string(),
        "metadata".to_string(),
        "--no-deps".to_string(),
        "--format-version".to_string(),
        "1".to_string(),
        "--manifest-path".to_string(),
        manifest_arg.clone(),
    ]) && captured.success
        && let Some(target) = parse_cargo_metadata(&captured.stdout, &manifest)
    {
        return target;
    }
    let fallback = fallback_app_name(&manifest_arg).unwrap_or_else(|| {
        fail(tr!(en: "Could not read app name from {manifest_arg}: expected [package] name or [[bin]] name", zh: "无法从 {manifest_arg} 读取应用名：需要 [package] name 或 [[bin]] name",
         manifest_arg = manifest_arg))
    });
    AppTarget {
        name: fallback,
        target_dir: Path::new(root).join("target"),
    }
}

fn parse_cargo_metadata(json: &str, manifest: &Path) -> Option<AppTarget> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let target_dir = value.get("target_directory")?.as_str()?;
    let packages = value.get("packages")?.as_array()?;
    let manifest = std::fs::canonicalize(manifest).ok();
    let package = packages
        .iter()
        .find(|package| {
            package
                .get("manifest_path")
                .and_then(|path| path.as_str())
                .map(|path| std::fs::canonicalize(path).ok() == manifest)
                .unwrap_or(false)
        })
        .or_else(|| packages.first())?;
    let package_name = package.get("name")?.as_str()?;
    let mut bins: Vec<&str> = Vec::new();
    for target in package.get("targets")?.as_array()? {
        let kind = target.get("kind")?.as_array()?;
        let is_bin = kind.iter().any(|k| k.as_str() == Some("bin"));
        if is_bin && let Some(name) = target.get("name").and_then(|n| n.as_str()) {
            bins.push(name);
        }
    }
    let name = if bins.contains(&package_name) {
        package_name.to_string()
    } else {
        bins.first()?.to_string()
    };
    Some(AppTarget {
        name,
        target_dir: PathBuf::from(target_dir),
    })
}

/// Tiny `[package] name` / `[[bin]] name` scan used when `cargo metadata`
/// cannot run.
fn fallback_app_name(manifest: &str) -> Option<String> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let mut section = "";
    let mut package = None;
    let mut bin = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line;
            continue;
        }
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start().strip_prefix('=')?.trim();
            let value = rest.strip_prefix('"')?;
            let value = value.split('"').next()?;
            match section {
                "[package]" => package = Some(value.to_string()),
                "[[bin]]" => bin = bin.or_else(|| Some(value.to_string())),
                _ => {}
            }
        }
    }
    bin.or(package)
}

pub fn app_exe_path(root: &str, release: bool) -> PathBuf {
    let target = app_target(root);
    let profile = if release { "release" } else { "debug" };
    let exe = if IS_WINDOWS {
        format!("{}.exe", target.name)
    } else {
        target.name.clone()
    };
    target.target_dir.join(profile).join(exe)
}

/// CLI-generated files live under `target/`.
pub fn out_dir(root: &str, rel: &str) -> PathBuf {
    Path::new(root).join("target").join(rel)
}

// ---- Zant UI's own folder ----

struct RootInfo {
    root: PathBuf,
    release_home: Option<PathBuf>,
}

static ROOT_INFO: OnceLock<RootInfo> = OnceLock::new();

/// `canonicalize` returns `\\?\C:\…` verbatim paths on Windows, which
/// PowerShell (`Join-Path`), cmd and the install script reject: a drive path
/// loses the prefix. Verbatim UNC paths (`\\?\UNC\…`) are kept.
fn plain_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with(r"UNC\") => PathBuf::from(rest.to_string()),
        _ => path,
    }
}

fn root_info() -> &'static RootInfo {
    ROOT_INFO.get_or_init(find_root)
}

/// Zant UI's folder. An installed release (install.sh / install.ps1) keeps it
/// as `zant-ui` next to `bin/zant`; a zant built from a checkout uses that
/// checkout (the workspace root this crate was built in).
fn find_root() -> RootInfo {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map(plain_path)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let Ok(executable) = std::env::current_exe() else {
        return RootInfo {
            root: checkout,
            release_home: None,
        };
    };
    // The installer links `zant` into ~/.local/bin; macOS reports the link's
    // path, not the installed binary's, so resolve it before looking for
    // the release next to it.
    let executable = executable
        .canonicalize()
        .map(plain_path)
        .unwrap_or(executable);
    let Some(home) = executable.parent().and_then(|dir| dir.parent()) else {
        return RootInfo {
            root: checkout,
            release_home: None,
        };
    };
    let root = home.join("zant-ui");
    // A release ships the Rust workspace (crates/, Cargo.toml) as zant-ui/.
    if root.join("Cargo.toml").is_file() {
        return RootInfo {
            root,
            release_home: Some(home.to_path_buf()),
        };
    }
    RootInfo {
        root: checkout,
        release_home: None,
    }
}

pub fn zant_root() -> &'static Path {
    &root_info().root
}

pub fn release_home() -> Option<&'static Path> {
    root_info().release_home.as_deref()
}

pub fn sep() -> &'static str {
    if IS_WINDOWS { "\\" } else { "/" }
}

// ---- JSON / XML escaping ----

pub fn escape_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out
}

fn main() {
    command::run();
}

#[cfg(test)]
mod tests {
    #[test]
    fn plain_path_drops_the_verbatim_drive_prefix() {
        use std::path::PathBuf;
        assert_eq!(
            super::plain_path(PathBuf::from(r"\\?\C:\Users\me\AppData\Local\zant")),
            PathBuf::from(r"C:\Users\me\AppData\Local\zant")
        );
        assert_eq!(
            super::plain_path(PathBuf::from(r"\\?\UNC\server\share")),
            PathBuf::from(r"\\?\UNC\server\share")
        );
        assert_eq!(
            super::plain_path(PathBuf::from("/Users/me/.zant")),
            PathBuf::from("/Users/me/.zant")
        );
    }

    use super::*;

    #[test]
    fn json_escaping_covers_quotes_and_backslashes() {
        assert_eq!(escape_json("plain"), "plain");
        assert_eq!(escape_json("a\"b"), "a\\\"b");
        assert_eq!(escape_json("a\\b"), "a\\\\b");
        assert_eq!(escape_json("a\"\\"), "a\\\"\\\\");
    }
}
