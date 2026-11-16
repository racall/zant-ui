//! `zant run`: the app with hot reload.
//!
//! The window belongs to `zant-hot-host`, a small process that loads the
//! app's UI as a dynamic library. On every saved change the CLI rebuilds only
//! that library and tells the host to switch to it; the host carries the
//! state over and the window stays open (like React Native / Flutter). A build
//! error is shown over the running UI and disappears with the next good build.
//!
//! Changes the library swap cannot apply (`Cargo.toml`, `app.config.json`:
//! dependencies, window options) replace the CLI itself after a state handoff.

mod classify;
mod inputs;
mod process;
mod project;
mod restart;
mod watch;
mod workspace;

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

use crate::{IS_WINDOWS, fail, generate, load_config, ok, step};
use classify::Change;
use generate::Need;
use project::Paths;

/// What wakes the session loop.
enum Event {
    Changed(Instant),
    HostExited(u32),
}

/// Hot reload applies to TSX apps (`src/App.tsx`); other projects just run.
pub fn available() -> bool {
    std::env::current_dir().is_ok_and(|dir| crate::project::is_tsx_app(&dir.to_string_lossy()))
}

pub fn cmd_hot(release: bool) -> std::io::Result<()> {
    let root = std::env::current_dir()?.to_string_lossy().into_owned();
    let session = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    eprintln!("[zant] cli_pid={} session={session}", std::process::id());
    let mut config = load_config(&root)?;
    generate::complete_platform_config(&root, &mut config, Need::Run)?;
    generate::sync_project(&root, &config)?;
    let zant_root = crate::zant_root().to_path_buf();
    let paths = Paths::new(&root, &zant_root);

    step(tr!(en: "Preparing hot reload host", zh: "准备热刷新宿主"));
    if let Err(message) = build_host(&zant_root, &paths) {
        fail(message);
    }
    if let Err(message) = project::write(&root, &zant_root, &paths) {
        fail(message);
    }
    let (tx, rx) = mpsc::channel();
    let mut last = inputs::Inputs::scan(Path::new(&root));
    let mut watches = watch::Watches::start(Path::new(&root), &last, tx.clone());
    step(tr!(en: "Initial build", zh: "首次编译"));
    let mut generation = 0u32;
    let library = match build_library(&paths, release, &mut generation) {
        Ok(library) => library,
        Err(_) => fail(tr!(en: "Build failed", zh: "编译失败")),
    };
    ok(tr!(en: "Build complete", zh: "编译完成"));

    let mut host = Host::spawn(&paths, &library, &root, release, tx.clone())?;
    eprintln!(
        "{}",
        tr!(en: "\nHot reload enabled: save files in src/ or native/ to refresh the UI in place; press Ctrl+C to exit", zh: "\n热刷新已开启：保存 src/、native/ 下的文件后界面原地刷新；Ctrl+C 退出",)
    );

    loop {
        let event_at = match rx.recv() {
            Ok(Event::HostExited(id)) if id == host.id => return Ok(()),
            Ok(Event::HostExited(_)) => continue,
            Err(_) => return Ok(()),
            Ok(Event::Changed(at)) => at,
        };
        // coalesce the burst of events one save produces
        std::thread::sleep(std::time::Duration::from_millis(60));
        let mut exited = false;
        while let Ok(event) = rx.try_recv() {
            exited |= matches!(event, Event::HostExited(id) if id == host.id);
        }
        if exited {
            return Ok(());
        }
        let now = inputs::Inputs::scan(Path::new(&root));
        let changes = now.changes(&last);
        watches.assets(Path::new(&root), &now);
        last = now;
        let action = changes
            .iter()
            .map(|(_, action)| *action)
            .max()
            .unwrap_or(Change::Ignore);
        if action == Change::Ignore {
            continue;
        }
        if changes.iter().any(|(path, _)| path.starts_with("locales")) {
            // the editor's key types follow the catalogs
            crate::declarations::sync(&root);
        }
        let begin = Instant::now();
        if std::env::var_os("ZANT_HOT_TRACE").is_some() {
            eprintln!(
                "[zant-hot] event_debounce_ms={:.3}",
                event_at.elapsed().as_secs_f64() * 1000.0
            );
        }
        if action == Change::Restart {
            let file = changes
                .iter()
                .find(|(_, action)| *action == Change::Restart)
                .expect("restart path");
            eprintln!(
                "{}",
                tr!(en: "{} changed, restarting zant run…", zh: "{} 改动，重启 zant run…", file.0.display())
            );
            let state = paths
                .crate_dir
                .parent()
                .expect("hot directory")
                .join("restart-state.json");
            // Remove an earlier failed handoff; a failed save starts fresh.
            let _ = std::fs::remove_file(&state);
            host.send(&format!("checkpoint {}", state.display()));
            host.stop();
            drop(watches);
            return restart::execute(&state);
        }
        eprintln!(
            "{}",
            tr!(en: "\nChanges detected, rebuilding...", zh: "\n检测到改动，重新编译...",)
        );
        match build_library(&paths, release, &mut generation) {
            Ok(library) => {
                host.send("clear");
                host.send(&format!("load {}", library.display()));
                eprintln!(
                    "{}",
                    tr!(en: "Reloaded (build took {} ms)", zh: "已刷新（编译 {} ms）", begin.elapsed().as_millis())
                );
            }
            Err(errors) => {
                host.send(&format!("notice {}", escape(&errors)));
                eprintln!(
                    "{}",
                    tr!(en: "Build failed; keeping the previous UI and waiting for changes", zh: "编译失败，界面保持上一版本，等待下一次改动",)
                );
            }
        }
    }
}

/// Builds `zant-hot-host` and the shared native library (once per checkout;
/// cargo makes later runs a no-op).
fn build_host(zant_root: &Path, paths: &Paths) -> Result<(), String> {
    let status = process::cargo()
        .args(["build", "-p", "zant-hot-host", "--manifest-path"])
        .arg(zant_root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &paths.zant_dir)
        .env("ZANT_NATIVE_SHARED", paths.native_dir())
        .status()
        .map_err(|error| tr!(en: "Could not run cargo: {error}", zh: "无法运行 cargo：{error}", error = error))?;
    if !status.success() {
        return Err(tr!(en: "Hot reload host build failed", zh: "热刷新宿主编译失败").into());
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
    let mut command = process::cargo();
    command
        .args(["rustc", "--lib", "--manifest-path"])
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
    // Apply only to the UI crate, never via RUSTFLAGS (which rebuilds deps).
    command.args(["--", "-C", "prefer-dynamic"]);
    let mut child = command
        .spawn()
        .map_err(|error| tr!(en: "Could not run cargo: {error}", zh: "无法运行 cargo：{error}", error = error))?;
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
    let started = Instant::now();
    // Each generation must keep its inode: never overwrite a mapped library.
    // Cargo atomically replaces its output on the next build. A hard link
    // retains the linker's signature cache; cross-device copies still work.
    if copy.exists() {
        std::fs::remove_file(&copy).map_err(|error| error.to_string())?;
    }
    if std::fs::hard_link(paths.built_library(release), &copy).is_err() {
        std::fs::copy(paths.built_library(release), &copy).map_err(|error| error.to_string())?;
    }
    if std::env::var_os("ZANT_HOT_TRACE").is_some() {
        eprintln!(
            "[zant-hot] copy_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
    }
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
        tr!(en: "Build failed (see terminal output)", zh: "编译失败（详见终端输出）").into()
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

/// The running `zant-hot-host`. Its waiter blocks in the OS, without polling.
struct Host {
    id: u32,
    stdin: Option<ChildStdin>,
    waiter: Option<std::thread::JoinHandle<()>>,
}

impl Host {
    fn spawn(
        paths: &Paths,
        library: &Path,
        root: &str,
        release: bool,
        tx: mpsc::Sender<Event>,
    ) -> std::io::Result<Host> {
        let mut command = Command::new(paths.host_exe());
        command
            .arg(library)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit());
        if let Some(state) = std::env::var_os("ZANT_HOT_RESTART_STATE") {
            command.arg("--state").arg(state);
        }
        command.arg("--deps").arg(paths.deps_dir(release));
        if !(crate::raw_logs() || IS_WINDOWS) {
            command.stderr(Stdio::piped());
        }
        let mut child = command.spawn()?;
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || crate::filter_stderr(stderr));
        }
        let stdin = child.stdin.take();
        let id = child.id();
        eprintln!("[zant] host_pid={id}");
        let waiter = std::thread::spawn(move || {
            let _ = child.wait();
            let _ = tx.send(Event::HostExited(id));
        });
        Ok(Host {
            id,
            stdin,
            waiter: Some(waiter),
        })
    }

    fn send(&mut self, line: &str) {
        if let Some(stdin) = self.stdin.as_mut()
            && (writeln!(stdin, "{line}").is_err() || stdin.flush().is_err())
        {
            self.stdin = None;
        }
    }

    fn stop(mut self) {
        // EOF asks the host to quit. The same mechanism handles a CLI killed
        // by Ctrl+C, even when only the parent receives the signal.
        self.stdin = None;
        if let Some(waiter) = self.waiter.take() {
            let _ = waiter.join();
        }
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
