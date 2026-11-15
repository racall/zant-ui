//! `zant run` / `compile` / `sync` / `package` and `run --watch`. Change
//! detection for `--watch` uses the workspace's `notify` crate but still goes
//! through `sources::stamp`, so a regenerated `src/app_config.rs` that ends
//! up unchanged does not restart the app.

use crate::IS_WINDOWS;
use crate::generate::{self, Need};
use crate::{app_exe_path, build_args, build_once, fail, load_config, ok, run_inherit, step};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

fn root() -> std::io::Result<String> {
    Ok(std::env::current_dir()?.to_string_lossy().into_owned())
}

pub fn cmd_sync() -> std::io::Result<()> {
    let root = root()?;
    sync(&root)?;
    // Windows has no bundle files to write (sync_project says so itself)
    if !IS_WINDOWS {
        crate::ok("配置代码已重新生成");
    }
    Ok(())
}

fn sync(root: &str) -> std::io::Result<()> {
    let mut config = load_config(root)?;
    generate::complete_platform_config(root, &mut config, Need::Run)?;
    generate::sync_project(root, &config)
}

pub fn cmd_compile(release: bool) -> std::io::Result<()> {
    let root = root()?;
    sync(&root)?;
    if let Err(err) = crate::ensure_rust_project(&root) {
        fail(format!("无法编译：{err}"));
    }
    run_inherit(&build_args(release), &root);
    Ok(())
}

pub fn cmd_run(release: bool) -> std::io::Result<()> {
    let root = root()?;
    let mut config = load_config(&root)?;
    generate::complete_platform_config(&root, &mut config, Need::Run)?;
    generate::sync_project(&root, &config)?;
    if config.sandbox && !IS_WINDOWS {
        let app = crate::bundle::package_app(&root, &config, true);
        // App Sandbox apps must be launched through LaunchServices, not by
        // executing the Mach-O inside the bundle directly.
        run_inherit(
            &["open".to_string(), "-n".to_string(), "-W".to_string(), app],
            &root,
        );
    } else {
        if let Err(err) = crate::ensure_rust_project(&root) {
            fail(format!("无法编译：{err}"));
        }
        run_inherit(&build_args(release), &root);
        let exe = app_exe_path(&root, release);
        crate::run_app(&exe, &root);
    }
    Ok(())
}

pub fn cmd_package() -> std::io::Result<()> {
    let root = root()?;
    let mut config = load_config(&root)?;
    generate::complete_platform_config(&root, &mut config, Need::Package)?;
    generate::sync_project(&root, &config)?;
    let app = crate::bundle::package_app(&root, &config, true);
    eprintln!("\n输出: {app}");
    Ok(())
}

/// `zant run --watch`: rebuild and restart on changes. Restart semantics:
/// macOS keeps the old window when the build fails; Windows cannot overwrite
/// the running .exe, so it stops the app first and reopens the last good build
/// when a build fails.
pub fn cmd_watch(release: bool) -> std::io::Result<()> {
    let root = root()?;
    let mut config = load_config(&root)?;
    generate::complete_platform_config(&root, &mut config, Need::Run)?;
    if config.sandbox && !IS_WINDOWS {
        fail("App Sandbox 下暂不支持热更新；请使用 zant run");
    }
    generate::sync_project(&root, &config)?;
    step("首次编译");
    if !build_once(&root, release) {
        fail("编译失败");
    }
    ok("编译完成");

    let mut last = crate::sources::stamp(&root);
    let exe = app_exe_path(&root, release);
    let mut child = crate::spawn_app(&exe, &root).unwrap_or_else(|err| {
        fail(format!("无法启动应用：{err}"));
    });
    eprintln!("\n热更新已开启：监听 src/、app.config.json；Ctrl+C 退出");

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                let _ = tx.send(());
            }
        })
        .unwrap_or_else(|err| fail(format!("无法监听文件变化：{err}")));
    let mut watched_any = false;
    for dir in crate::sources::WATCH_DIRS {
        let path = std::path::Path::new(&root).join(dir);
        if path.is_dir() && watcher.watch(&path, RecursiveMode::Recursive).is_ok() {
            watched_any = true;
        }
    }
    for file in crate::sources::WATCH_FILES {
        let path = std::path::Path::new(&root).join(file);
        if path.is_file() && watcher.watch(&path, RecursiveMode::NonRecursive).is_ok() {
            watched_any = true;
        }
    }
    if !watched_any {
        fail(format!("没有可监听的文件：{root}/src 不存在？"));
    }

    loop {
        // Block until notify reports something, then coalesce the burst.
        if rx.recv().is_err() {
            fail("文件监听已停止");
        }
        while rx.try_recv().is_ok() {}
        let now = crate::sources::stamp(&root);
        if now == last {
            continue;
        }
        last = now;
        eprintln!("\n检测到改动，重新编译...");
        if IS_WINDOWS {
            // a running .exe cannot be overwritten: stop it before linking,
            // and bring the last good build back if this one fails
            let _ = child.kill();
            let _ = child.wait();
            let built = build_once(&root, release);
            if !built {
                eprintln!("编译失败，重新打开上一次的版本，等待下一次改动");
            }
            generate::sync_project(&root, &config)?;
            child = crate::spawn_app(&exe, &root).unwrap_or_else(|err| {
                fail(format!("无法启动应用：{err}"));
            });
            if built {
                eprintln!("已重启应用");
            }
            continue;
        }
        if !build_once(&root, release) {
            eprintln!("编译失败，保持当前窗口，等待下一次改动");
            continue;
        }
        let _ = child.kill();
        let _ = child.wait();
        generate::sync_project(&root, &config)?;
        child = crate::spawn_app(&exe, &root).unwrap_or_else(|err| {
            fail(format!("无法启动应用：{err}"));
        });
        eprintln!("已重启应用");
    }
}
