//! `zant run`: the app with hot reload.
//!
//! The window belongs to `zant-hot-host`, a small process that loads the
//! app's UI as a dynamic library. On every saved change the CLI rebuilds only
//! that library and tells the host to switch to it; the host carries the
//! state over and the window stays open (like React Native / Flutter). A build
//! error is shown over the running UI and disappears with the next good build.
//!
//! Changes the library swap cannot apply (`Cargo.toml`, `app.config.json`:
//! dependencies, window options) restart the host instead.

mod project;

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::{IS_WINDOWS, fail, generate, load_config, ok, step};
use generate::Need;
use project::Paths;

/// What wakes the session loop.
enum Event {
    Changed,
    HostExited,
}

/// Hot reload applies to TSX apps (`src/App.tsx`); other projects just run.
pub fn available() -> bool {
    std::env::current_dir().is_ok_and(|dir| crate::project::is_tsx_app(&dir.to_string_lossy()))
}

pub fn cmd_hot(release: bool) -> std::io::Result<()> {
    let root = std::env::current_dir()?.to_string_lossy().into_owned();
    let mut config = load_config(&root)?;
    generate::complete_platform_config(&root, &mut config, Need::Run)?;
    generate::sync_project(&root, &config)?;
    let zant_root = crate::zant_root().to_path_buf();
    let paths = Paths::new(&root, &zant_root);

    step("准备热刷新宿主");
    if let Err(message) = build_host(&zant_root, &paths) {
        fail(message);
    }
    if let Err(message) = project::write(&root, &zant_root, &paths) {
        fail(message);
    }
    step("首次编译");
    let mut generation = 0u32;
    let library = match build_library(&paths, release, &mut generation) {
        Ok(library) => library,
        Err(_) => fail("编译失败"),
    };
    ok("编译完成");

    let (tx, rx) = mpsc::channel();
    let _watcher = watch(&root, tx.clone());
    let mut host = Host::spawn(&paths, &library, &root, tx.clone())?;
    eprintln!("\n热刷新已开启：保存 src/、native/ 下的文件后界面原地刷新；Ctrl+C 退出");

    let mut last = crate::sources::stamp(&root);
    let mut restart_inputs = restart_stamp(&root);
    loop {
        match rx.recv() {
            Ok(Event::HostExited) | Err(_) => return Ok(()),
            Ok(Event::Changed) => {}
        }
        // coalesce the burst of events one save produces
        std::thread::sleep(std::time::Duration::from_millis(60));
        let mut exited = false;
        while let Ok(event) = rx.try_recv() {
            exited |= matches!(event, Event::HostExited);
        }
        if exited {
            return Ok(());
        }
        let now = crate::sources::stamp(&root);
        if now == last {
            continue;
        }
        last = now;
        let begin = Instant::now();
        let inputs = restart_stamp(&root);
        let restart = inputs != restart_inputs;
        restart_inputs = inputs;
        if restart {
            eprintln!("\nCargo.toml / app.config.json 有改动，重新编译并重启应用...");
            config = load_config(&root)?;
            generate::sync_project(&root, &config)?;
            if let Err(message) = project::write(&root, &zant_root, &paths) {
                eprintln!("{message}");
                continue;
            }
        } else {
            eprintln!("\n检测到改动，重新编译...");
        }
        match build_library(&paths, release, &mut generation) {
            Ok(library) if restart => {
                host.stop();
                host = Host::spawn(&paths, &library, &root, tx.clone())?;
                eprintln!("已重启应用（{} ms）", begin.elapsed().as_millis());
            }
            Ok(library) => {
                host.send("clear");
                host.send(&format!("load {}", library.display()));
                eprintln!("已刷新（编译 {} ms）", begin.elapsed().as_millis());
            }
            Err(errors) => {
                host.send(&format!("notice {}", escape(&errors)));
                eprintln!("编译失败，界面保持上一版本，等待下一次改动");
            }
        }
    }
}

/// Builds `zant-hot-host` and the shared native library (once per checkout;
/// cargo makes later runs a no-op).
fn build_host(zant_root: &Path, paths: &Paths) -> Result<(), String> {
    let status = Command::new(crate::cargo_exe())
        .args(["build", "-p", "zant-hot-host", "--manifest-path"])
        .arg(zant_root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &paths.zant_dir)
        .env("ZANT_NATIVE_SHARED", paths.native_dir())
        .status()
        .map_err(|error| format!("无法运行 cargo：{error}"))?;
    if !status.success() {
        return Err("热刷新宿主编译失败".into());
    }
    if IS_WINDOWS {
        // the host and every UI library load the DLLs from the host's folder
        let bin = paths
            .host_exe()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let dll = paths
            .native_dir()
            .join(project::library_file("zant_native_shared"));
        copy_if_newer(&dll, &bin.join(project::library_file("zant_native_shared")))?;
        let pdfium = zant_root.join("third_party/pdfium-win/bin/pdfium.dll");
        if pdfium.is_file() {
            copy_if_newer(&pdfium, &bin.join("pdfium.dll"))?;
        }
    }
    Ok(())
}

fn copy_if_newer(from: &Path, to: &Path) -> Result<(), String> {
    let modified = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    };
    if modified(to).is_some() && modified(to) >= modified(from) {
        return Ok(());
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("{} → {}: {error}", from.display(), to.display()))
}

/// Builds the UI library and copies it to a fresh numbered file. On failure
/// returns the compiler errors (also printed).
fn build_library(paths: &Paths, release: bool, generation: &mut u32) -> Result<PathBuf, String> {
    let mut command = Command::new(crate::cargo_exe());
    command
        .args(["build", "--lib", "--manifest-path"])
        .arg(paths.crate_dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &paths.target_dir)
        .env("ZANT_NATIVE_SHARED", paths.native_dir())
        .env(
            "CARGO_TERM_COLOR",
            if crate::is_tty() { "always" } else { "never" },
        )
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped());
    if release {
        command.arg("--release");
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法运行 cargo：{error}"))?;
    let mut errors = String::new();
    if let Some(stderr) = child.stderr.take() {
        for line in std::io::BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
        {
            eprintln!("{line}");
            errors.push_str(&strip_ansi(&line));
            errors.push('\n');
        }
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(summary(&errors));
    }
    std::fs::create_dir_all(&paths.gen_dir).map_err(|error| error.to_string())?;
    *generation += 1;
    let name = project::library_file(&format!("ui-{generation}"));
    let copy = paths.gen_dir.join(name);
    std::fs::copy(paths.built_library(release), &copy).map_err(|error| error.to_string())?;
    Ok(copy)
}

/// The compiler output worth showing in the window: from the first error on,
/// without cargo's progress lines, at most 40 lines.
fn summary(output: &str) -> String {
    let lines: Vec<&str> = output
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("error"))
        .filter(|line| {
            let line = line.trim_start();
            !(line.starts_with("Compiling") || line.starts_with("Building"))
        })
        .take(40)
        .collect();
    if lines.is_empty() {
        "编译失败（详见终端输出）".into()
    } else {
        lines.join("\n")
    }
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ … final byte in @..~
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// One line of the host protocol: `\` and newlines escaped.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n")
}

/// Content of the inputs a library swap cannot apply.
fn restart_stamp(root: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for file in ["Cargo.toml", "app.config.json"] {
        out.extend(std::fs::read(Path::new(root).join(file)).unwrap_or_default());
        out.push(0);
    }
    out
}

fn watch(root: &str, tx: mpsc::Sender<Event>) -> RecommendedWatcher {
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = tx.send(Event::Changed);
            }
        })
        .unwrap_or_else(|err| fail(format!("无法监听文件变化：{err}")));
    let mut watched_any = false;
    for dir in crate::sources::WATCH_DIRS {
        let path = Path::new(root).join(dir);
        if path.is_dir() && watcher.watch(&path, RecursiveMode::Recursive).is_ok() {
            watched_any = true;
        }
    }
    for file in crate::sources::WATCH_FILES {
        let path = Path::new(root).join(file);
        if path.is_file() && watcher.watch(&path, RecursiveMode::NonRecursive).is_ok() {
            watched_any = true;
        }
    }
    if !watched_any {
        fail(format!("没有可监听的文件：{root}/src 不存在？"));
    }
    watcher
}

/// The running `zant-hot-host`.
struct Host {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Host {
    fn spawn(
        paths: &Paths,
        library: &Path,
        root: &str,
        tx: mpsc::Sender<Event>,
    ) -> std::io::Result<Host> {
        let mut command = Command::new(paths.host_exe());
        command
            .arg(library)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit());
        let filter = !(crate::raw_logs() || IS_WINDOWS);
        if filter {
            command.stderr(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|error| {
            std::io::Error::other(format!("无法启动 {}: {error}", paths.host_exe().display()))
        })?;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || crate::filter_stderr(stderr));
        }
        let stdin = child.stdin.take();
        let id = child.id();
        std::thread::spawn(move || {
            wait_for(id);
            let _ = tx.send(Event::HostExited);
        });
        Ok(Host { child, stdin })
    }

    fn send(&mut self, line: &str) {
        if let Some(stdin) = self.stdin.as_mut()
            && (writeln!(stdin, "{line}").is_err() || stdin.flush().is_err())
        {
            self.stdin = None;
        }
    }

    fn stop(mut self) {
        self.stdin = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Blocks until the process `id` ended (polling; the `Child` stays with
/// [`Host`] so it can be killed for a restart).
fn wait_for(id: u32) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        if !process_alive(id) {
            return;
        }
    }
}

#[cfg(unix)]
fn process_alive(id: u32) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { kill(id as i32, 0) == 0 }
}

#[cfg(windows)]
fn process_alive(id: u32) -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, id: u32) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(process: *mut std::ffi::c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    // SAFETY: plain Win32 calls on a handle we own.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_are_one_protocol_line() {
        assert_eq!(escape("a\\b\nc"), "a\\\\b\\nc");
    }

    #[test]
    fn summaries_start_at_the_first_error() {
        let output = "   Compiling x v0.1.0\nwarning: unused\nerror: unknown tag `Foo`\n  --> src/App.tsx:3:5\n";
        assert_eq!(
            summary(output),
            "error: unknown tag `Foo`\n  --> src/App.tsx:3:5"
        );
        assert_eq!(strip_ansi("\u{1b}[1;31merror\u{1b}[0m: x"), "error: x");
    }
}
