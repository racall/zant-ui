//! The `zant` command line tool.
//!
//! Apps are built with `cargo build`; their binaries are
//! `target/{debug,release}/<name>`.
//!
//! File layout: this file has the shared helpers and command dispatch;
//! `create.rs` / `run.rs` / `update.rs` / `bundle.rs` / `generate.rs` hold the
//! commands, split to stay under ~500 lines each.

mod app_config;
mod bundle;
mod create;
mod distribute;
mod generate;
#[allow(dead_code)]
mod icon;
mod png;
mod project;
mod run;
mod scaffold;
mod sources;
mod update;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command as Proc;
use std::sync::{Mutex, OnceLock};

use clap::{Parser, Subcommand};

pub const COLOR_CYAN: &str = "\x1b[36m";
pub const COLOR_GREEN: &str = "\x1b[32m";
pub const COLOR_RED: &str = "\x1b[31m";
pub const COLOR_DIM: &str = "\x1b[2m";
pub const COLOR_RESET: &str = "\x1b[0m";

/// The platform this zant runs on.
pub const IS_WINDOWS: bool = cfg!(windows);

/// `zant-icon`'s exit code 2 usage print is in `icon.rs`; the CLI usage text
/// prints when zant is called without a command.
pub const USAGE: &str = "用法: zant <命令>
  create <项目名>       生成项目（TSX + Rust）
  run [--watch]         编译并运行（--watch 热更新）
  compile [--release]   只编译
  package               生成本机测试用的 .app / 程序目录
  build [配置文件]      发布构建：签名、公证、DMG / 安装包，或提交商店
                        （默认读 build.config.json；--yes 不提问，CI 用）
  sync                  重新生成配置代码
  update                更新 Zant UI（git pull、依赖）并重新编译 zant

";

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
        read_file(&path).unwrap_or_else(|_| fail("找不到 app.config.json（请先运行 zant create）"));
    let text = String::from_utf8_lossy(&data);
    match app_config::parse(&text, app_config::Os::current()) {
        Ok(cfg) => Ok(cfg),
        Err(app_config::ParseError::InvalidJson(why)) => {
            fail(format!("app.config.json 解析失败: {why}"))
        }
        Err(app_config::ParseError::InvalidEntitlements) => fail(
            "app.config.json 的 macos.entitlements 要是对象，如 { \"com.apple.security.network.client\": true }",
        ),
        Err(app_config::ParseError::InvalidBundleId(why)) => fail(format!(
            "app.config.json 的 bundle_id 不符合规范：{why}（如 com.xx.test）"
        )),
    }
}

// ---- child processes ----

fn base_command(program: &str) -> Proc {
    let command = Proc::new(program);
    // PowerShell 7 puts its own module folders in PSModulePath; a Windows
    // PowerShell 5.1 started from here would then load PowerShell 7's
    // Microsoft.PowerShell.Utility and miss Get-FileHash. Without the
    // variable each PowerShell uses its own default.
    #[cfg(windows)]
    {
        let mut command = command;
        command.env_remove("PSModulePath");
        return command;
    }
    #[cfg(not(windows))]
    command
}

/// Runs a command with the terminal, exiting on spawn / wait / exit failure.
pub fn run_inherit(argv: &[String], cwd: &str) {
    let mut command = base_command(&argv[0]);
    command.args(&argv[1..]).current_dir(cwd);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => fail(format!("无法启动 {}: {err}", argv[0])),
    };
    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(_) => fail(format!("{} 执行失败", argv[0])),
        Err(err) => fail(format!("等待 {} 失败: {err}", argv[0])),
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
        _ => fail(format!(
            "找不到 {cargo}：请先安装 Rust 工具链（https://rustup.rs），或设置 ZANT_CARGO"
        )),
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
        fail(format!(
            "{root} 没有 Cargo.toml：zant run/compile/package 用 cargo build 构建 Cargo 应用；请用 zant create 生成 TSX 项目"
        ));
    }
    if project::is_tsx_app(root) && !Path::new(root).join("build.rs").is_file() {
        fail(format!(
            "{root} 是 TSX 应用（src/App.tsx）但没有 build.rs：TSX → Rust 的编译由应用的 build.rs 调用 `zant_build::tsx_app()` 完成（把 src/**/*.tsx 生成到 OUT_DIR，由 src/main.rs include!）；请用 `zant create` 生成模板，或参照 scaffold/template-ts/build.rs 补上 build.rs"
        ));
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
        fail(format!(
            "无法从 {manifest_arg} 读取应用名：需要 [package] name 或 [[bin]] name"
        ))
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

// ---- console / prompts ----

pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// One line of input (without the line break), "" on error.
pub fn read_line() -> String {
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return String::new();
    }
    line.trim_end_matches(['\n', '\r']).to_string()
}

/// A line typed without echo (a password); used for this run only. The echo
/// is turned off with `stty` on Unix (no unsafe) and PowerShell's secure
/// prompt on Windows.
pub fn read_secret() -> String {
    if !is_tty() {
        return read_line();
    }
    read_secret_tty()
}

#[cfg(unix)]
fn read_secret_tty() -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let cwd = cwd.to_string_lossy().into_owned();
    let _ = run_ok(&["stty".to_string(), "-echo".to_string()], &cwd);
    let line = read_line();
    let _ = run_ok(&["stty".to_string(), "echo".to_string()], &cwd);
    eprintln!();
    line
}

#[cfg(windows)]
fn read_secret_tty() -> String {
    let script = "$s = Read-Host -AsSecureString; [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($s))";
    if let Ok(captured) = run_capture(&[
        "powershell".to_string(),
        "-NoProfile".to_string(),
        "-Command".to_string(),
        script.to_string(),
    ]) && captured.success
    {
        eprintln!();
        return captured.stdout.trim_end_matches(['\n', '\r']).to_string();
    }
    read_line()
}

#[cfg(not(any(unix, windows)))]
fn read_secret_tty() -> String {
    read_line()
}

/// Prints `label [default]: ` and returns the trimmed answer (default when
/// empty).
pub fn prompt(label: &str, default: &str) -> String {
    eprint!("{label} [{default}]: ");
    let _ = std::io::stderr().flush();
    let value = read_line();
    let value = value.trim_matches(' ');
    if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    }
}

pub fn prompt_yes_no(label: &str, default: bool) -> bool {
    let def = if default { "Y/n" } else { "y/N" };
    loop {
        eprint!("{label} [{def}]: ");
        let _ = std::io::stderr().flush();
        let value = read_line();
        let value = value.trim_matches([' ', '\r', '\n']);
        if value.is_empty() {
            return default;
        }
        if value.eq_ignore_ascii_case("y") || value.eq_ignore_ascii_case("yes") {
            return true;
        }
        if value.eq_ignore_ascii_case("n") || value.eq_ignore_ascii_case("no") {
            return false;
        }
    }
}

/// Harmless macOS framework log lines (InputMethodKit / Text Services
/// Manager) that unbundled apps launched from a terminal print when an input
/// method attaches. They do not affect input; `zant run` hides them unless
/// ZANT_RAW_LOGS=1.
const BENIGN_LOG_LINES: &[&str] = &[
    "IMKCFRunLoopWakeUpReliable",
    "IMKClient Stall detected",
    "TSM AdjustCapsLockLEDForKeyTransitionHandling",
];

pub fn raw_logs() -> bool {
    match std::env::var("ZANT_RAW_LOGS") {
        Ok(value) => value.as_bytes().first() == Some(&b'1'),
        Err(_) => false,
    }
}

/// Whether a stderr line is one of the harmless macOS framework messages.
fn is_benign_log(line: &str) -> bool {
    BENIGN_LOG_LINES.iter().any(|benign| line.contains(benign))
}

/// Forwards the app's stderr, dropping the benign framework lines.
fn filter_stderr(stderr: std::process::ChildStderr) {
    use std::io::BufRead as _;
    let lines = std::io::BufReader::new(stderr).lines();
    for line in lines.map_while(Result::ok) {
        if !is_benign_log(&line) {
            eprintln!("{line}");
        }
    }
}

/// Spawns the app the same way `zant run` does.
pub fn spawn_app(exe: &Path, root: &str) -> std::io::Result<std::process::Child> {
    let mut command = base_command(&exe.to_string_lossy());
    command
        .current_dir(root)
        .stdout(std::process::Stdio::inherit());
    if raw_logs() || IS_WINDOWS {
        return command.spawn();
    }
    command.stderr(std::process::Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || filter_stderr(stderr));
    }
    Ok(child)
}

/// Runs the app to completion (interactive `zant run`).
pub fn run_app(exe: &Path, root: &str) {
    let mut child = spawn_app(exe, root)
        .unwrap_or_else(|err| fail(format!("无法启动 {}: {err}", exe.display())));
    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(_) => fail(format!("{} 执行失败", exe.display())),
        Err(err) => fail(format!("等待 {} 失败: {err}", exe.display())),
    }
}

// ---- Zant UI's own folder ----

struct RootInfo {
    root: PathBuf,
    release_home: Option<PathBuf>,
}

static ROOT_INFO: OnceLock<RootInfo> = OnceLock::new();

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
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let Ok(executable) = std::env::current_exe() else {
        return RootInfo {
            root: checkout,
            release_home: None,
        };
    };
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

// ---- command line ----

#[derive(Parser)]
#[command(
    name = "zant",
    version,
    about = "Zant UI 命令行工具：创建、运行、打包应用",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 生成项目
    Create {
        /// 项目名（只能以字母开头，包含字母、数字、-、_）
        name: String,
        /// 不提问，使用默认值
        #[arg(long)]
        yes: bool,
        /// 覆盖已存在的目录
        #[arg(long)]
        force: bool,
        /// 应用标题
        #[arg(long)]
        title: Option<String>,
        /// 窗口风格：native、hidden-titlebar 或 borderless
        #[arg(long)]
        style: Option<String>,
        /// 应用标识（反向域名，如 com.xx.test）
        #[arg(long = "bundle-id")]
        bundle_id: Option<String>,
        /// macOS 沙盒：true 或 false
        #[arg(long, value_name = "true|false")]
        sandbox: Option<String>,
        /// macOS 签名证书
        #[arg(long)]
        sign: Option<String>,
    },
    /// 编译并运行
    Run {
        /// 热更新：改动后自动重新编译并重启
        #[arg(long)]
        watch: bool,
        /// Release 构建
        #[arg(long)]
        release: bool,
    },
    /// 只编译
    Compile {
        /// Release 构建
        #[arg(long)]
        release: bool,
    },
    /// 生成本机测试用的 .app / 程序目录
    Package,
    /// 发布构建：签名、公证、DMG / 安装包，或提交商店
    Build {
        /// 配置文件（默认 build.config.json）
        file: Option<String>,
        /// 不提问（CI 用）
        #[arg(long, short = 'y')]
        yes: bool,
        /// 旧用法（zant build --release），现在会提示改用 zant compile
        #[arg(long, hide = true)]
        release: bool,
    },
    /// 重新生成配置代码
    Sync,
    /// 更新 Zant UI（git pull、依赖）并重新编译 zant
    Update,
}

fn main() {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        eprint!("{USAGE}");
        return;
    };
    let result: std::io::Result<()> = match command {
        Command::Create {
            name,
            yes,
            force,
            title,
            style,
            bundle_id,
            sandbox,
            sign,
        } => create::cmd_create(create::CreateOptions {
            name,
            yes,
            force,
            title,
            style,
            bundle_id,
            sandbox,
            sign,
        }),
        Command::Run { watch, release } => {
            if watch {
                run::cmd_watch(release)
            } else {
                run::cmd_run(release)
            }
        }
        Command::Compile { release } => run::cmd_compile(release),
        Command::Package => run::cmd_package(),
        Command::Build { file, yes, release } => {
            if release {
                fail(
                    "zant build 现在是发布构建（签名、打包、公证或提交商店）；只编译请用 zant compile [--release]",
                );
            }
            distribute::cmd_build(file, yes)
        }
        Command::Sync => run::cmd_sync(),
        Command::Update => update::cmd_update(),
    };
    if let Err(err) = result {
        eprintln!("zant: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod log_filter_tests {
    use super::is_benign_log;

    #[test]
    fn benign_macos_framework_lines_are_dropped() {
        assert!(is_benign_log(
            "IMKCFRunLoopWakeUpReliable: calling into InputMethodKit"
        ));
        assert!(is_benign_log(
            "TSM AdjustCapsLockLEDForKeyTransitionHandling"
        ));
        assert!(is_benign_log("IMKClient Stall detected, please report"));
        assert!(!is_benign_log(
            "Zant UI app — renderer: Metal (Skia Ganesh)"
        ));
        assert!(!is_benign_log("error: failed to open the file"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escaping_covers_quotes_and_backslashes() {
        assert_eq!(escape_json("plain"), "plain");
        assert_eq!(escape_json("a\"b"), "a\\\"b");
        assert_eq!(escape_json("a\\b"), "a\\\\b");
        assert_eq!(escape_json("a\"\\"), "a\\\"\\\\");
    }

    #[test]
    fn usage_lists_every_command() {
        for needle in [
            "create <项目名>",
            "run [--watch]",
            "compile [--release]",
            "package",
            "build [配置文件]",
            "sync",
            "update",
        ] {
            assert!(USAGE.contains(needle), "{needle} missing from usage");
        }
    }
}
