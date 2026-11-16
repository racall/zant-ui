//! `zant update`: a source checkout pulls (fast-forward only), fetches the
//! pinned dependencies again, checks the Rust toolchain and rebuilds the CLI;
//! an installed release runs the install script of the latest release again.

use crate::{COLOR_CYAN, COLOR_RESET, IS_WINDOWS, dim_line, fail, ok, path_exists, read_file};
use crate::{read_text, run_capture, run_inherit, run_ok, step, write_file};
use std::path::Path;

/// `zant update`: pulls Zant UI (a clean checkout that tracks a remote,
/// fast-forward only), fetches the pinned dependencies again when they
/// changed, checks the Rust toolchain and rebuilds the CLI.
pub fn cmd_update() -> std::io::Result<()> {
    if let Some(home) = crate::release_home() {
        return update_release(home);
    }
    let root = crate::zant_root().to_string_lossy().into_owned();
    eprintln!(
        "{}",
        tr!(en: "\n{COLOR_CYAN}Updating Zant UI{COLOR_RESET}: {root}", zh: "\n{COLOR_CYAN}更新 Zant UI{COLOR_RESET}：{root}", COLOR_CYAN = COLOR_CYAN, COLOR_RESET = COLOR_RESET, root = root)
    );

    let git_dir = Path::new(&root).join(".git");
    if path_exists(&git_dir) {
        let before = git_output(&root, &["rev-parse", "--short", "HEAD"]);
        let dirty = git_output(&root, &["status", "--porcelain", "--untracked-files=no"]);
        let upstream = git_output(
            &root,
            &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        );
        if !dirty.is_empty() {
            dim_line(
                tr!(en: "Uncommitted changes; rebuilding without pulling", zh: "本地有未提交的修改，不拉取新代码，只重新编译"),
            );
        } else if upstream.is_empty() {
            dim_line(
                tr!(en: "No upstream branch; rebuilding without pulling", zh: "当前分支没有对应的远程分支，不拉取新代码，只重新编译"),
            );
        } else {
            step(
                &tr!(en: "Pulling latest code ({upstream})", zh: "拉取最新代码（{upstream}）", upstream = upstream),
            );
            let pulled = run_ok(
                &[
                    "git".to_string(),
                    "-C".to_string(),
                    root.clone(),
                    "pull".to_string(),
                    "--ff-only".to_string(),
                    "--quiet".to_string(),
                ],
                &root,
            );
            let after = git_output(&root, &["rev-parse", "--short", "HEAD"]);
            if !pulled {
                dim_line(
                    tr!(en: "Pull failed (local commits or network error); rebuilding existing code", zh: "拉取失败（本地有远程没有的提交，或网络问题），用现有代码重新编译"),
                );
            } else if before == after {
                ok(
                    &tr!(en: "Already up to date ({after})", zh: "已经是最新（{after}）", after = after),
                );
            } else {
                ok(
                    &tr!(en: "Updated: {before} → {after}", zh: "已更新：{before} → {after}", after = after, before = before),
                );
            }
        }
    } else {
        dim_line(
            tr!(en: "Not a git repository; rebuilding without pulling", zh: "不是 git 仓库，不拉取新代码，只重新编译"),
        );
    }

    // Rust version the framework needs (workspace Cargo.toml) vs the one zant uses
    let cargo_toml = read_text(&Path::new(&root).join("Cargo.toml")).unwrap_or_default();
    let needed = toml_string(&cargo_toml, "rust-version").unwrap_or("");
    let have = rustc_version();
    if !needed.is_empty() && !version_at_least(&have, needed) {
        fail(
            tr!(en: "Zant UI requires Rust {needed} (zant uses {}): rerun the installer to install the required toolchain; see Installation in the Zant UI README", zh: "Zant UI 现在需要 Rust {needed}（zant 用的是 {}）：重新运行安装脚本即可（见 Zant UI README「安装」），它会安装对应版本的 Rust",
            if have.is_empty() {
                tr!(en: "Rust not found", zh: "找不到的 Rust")
            } else {
                &have
            }
        , needed = needed),
        );
    }

    // pinned Skia / PDFium / fonts: the scripts download only what changed
    step(tr!(en: "Checking dependencies (Skia, etc.)", zh: "检查依赖（Skia 等）"));
    let skia_script = Path::new(&root).join(if IS_WINDOWS {
        "scripts/fetch_skia_windows.ps1"
    } else {
        "scripts/fetch_skia.sh"
    });
    if IS_WINDOWS {
        run_inherit(
            &[
                "powershell".to_string(),
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                skia_script.to_string_lossy().into_owned(),
            ],
            &root,
        );
    } else {
        run_inherit(
            &[
                "/bin/bash".to_string(),
                skia_script.to_string_lossy().into_owned(),
            ],
            &root,
        );
    }

    step(tr!(en: "Rebuilding zant", zh: "重新编译 zant"));
    let exe =
        Path::new(&root)
            .join("target/debug")
            .join(if IS_WINDOWS { "zant.exe" } else { "zant" });
    let old = Path::new(&root).join("target/debug/zant.old.exe");
    if IS_WINDOWS {
        // a running .exe cannot be overwritten, but it can be renamed
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&exe, &old);
    }
    if !run_ok(
        &[
            crate::cargo_exe().to_string(),
            "build".to_string(),
            "-p".to_string(),
            "zant-cli".to_string(),
        ],
        &root,
    ) {
        if IS_WINDOWS {
            let _ = std::fs::rename(&old, &exe);
        }
        fail(
            tr!(en: "Build failed; keeping the existing zant version", zh: "编译失败，zant 保持原来的版本"),
        );
    }
    ok(&tr!(en: "Updated: {}", zh: "已更新：{}", exe.to_string_lossy()));
    Ok(())
}

/// An installed release updates by running the install script of the latest
/// release again (it checks the version and the Rust toolchain and replaces
/// `zant-ui/` and `bin/zant`); ZANT_DOWNLOAD points it elsewhere.
fn update_release(home: &Path) -> std::io::Result<()> {
    let base = match std::env::var("ZANT_DOWNLOAD") {
        Ok(value) if !value.is_empty() => value,
        _ => RELEASE_DOWNLOAD.to_string(),
    };
    let home_string = home.to_string_lossy().into_owned();
    if !IS_WINDOWS {
        let script = format!("curl -fsSL '{base}/install.sh' | bash");
        if !run_ok_with_env(
            &["/bin/bash".to_string(), "-c".to_string(), script],
            &home_string,
            &[("ZANT_HOME", home_string.clone())],
        ) {
            fail(
                tr!(en: "Update did not complete; see the messages above", zh: "更新没有完成（见上面的说明）"),
            );
        }
        retag_current_project(home);
        return Ok(());
    }
    // Windows: download the script and run it as a file with a UTF-8 BOM
    // (release assets come as application/octet-stream, which `irm` would
    // decode as Latin-1, and Windows PowerShell reads BOM-less files as ANSI)
    let dir = home.join("cache");
    std::fs::create_dir_all(&dir)?;
    let script = dir.join("install.ps1");
    let url = format!("{base}/install.ps1");
    if !run_ok_with_env(
        &[
            "curl.exe".to_string(),
            "-fsSL".to_string(),
            "--retry".to_string(),
            "3".to_string(),
            "-o".to_string(),
            script.to_string_lossy().into_owned(),
            url.clone(),
        ],
        &home_string,
        &[("ZANT_HOME", home_string.clone())],
    ) {
        fail(
            tr!(en: "Could not download update script: {url}", zh: "下载更新脚本失败：{url}", url = url),
        );
    }
    let text = read_file(&script)?;
    if !text.starts_with(b"\xEF\xBB\xBF") {
        let mut with_bom = b"\xEF\xBB\xBF".to_vec();
        with_bom.extend_from_slice(&text);
        write_file(&script, &with_bom)?;
    }
    if !run_ok_with_env(
        &[
            "powershell".to_string(),
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            script.to_string_lossy().into_owned(),
        ],
        &home_string,
        &[("ZANT_HOME", home_string.clone())],
    ) {
        fail(
            tr!(en: "Update did not complete; see the messages above", zh: "更新没有完成（见上面的说明）"),
        );
    }
    retag_current_project(home);
    Ok(())
}

/// Run inside a project: its Zant UI git dependency follows the version the
/// install script just put in place (`zant-ui/.zant-version`).
fn retag_current_project(home: &Path) {
    let Ok(version) = std::fs::read_to_string(home.join("zant-ui/.zant-version")) else {
        return;
    };
    let tag = version.trim();
    if tag.is_empty() {
        return;
    }
    if let Ok(dir) = std::env::current_dir() {
        crate::framework::retag_project(&dir, tag);
    }
}

/// Runs a command with extra environment variables and the terminal; `false`
/// when it fails (no exit).
fn run_ok_with_env(argv: &[String], cwd: &str, envs: &[(&str, String)]) -> bool {
    let mut command = std::process::Command::new(&argv[0]);
    command.args(&argv[1..]).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    #[cfg(windows)]
    command.env_remove("PSModulePath");
    matches!(command.status(), Ok(status) if status.success())
}

/// Where release archives and the install scripts are published.
pub const RELEASE_DOWNLOAD: &str = "https://github.com/racall/zant-ui/releases/latest/download";

/// Trimmed stdout of a git command in `root`, "" on failure.
fn git_output(root: &str, args: &[&str]) -> String {
    let mut argv = vec!["git".to_string(), "-C".to_string(), root.to_string()];
    argv.extend(args.iter().map(|arg| arg.to_string()));
    match run_capture(&argv) {
        Ok(captured) if captured.success => {
            captured.stdout.trim_matches([' ', '\r', '\n']).to_string()
        }
        _ => String::new(),
    }
}

/// `rustc --version`'s number ("1.94.1"), "" on failure.
fn rustc_version() -> String {
    let rustc = std::env::var("ZANT_RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let Ok(captured) = run_capture(&[rustc, "--version".to_string()]) else {
        return String::new();
    };
    if !captured.success {
        return String::new();
    }
    captured
        .stdout
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string()
}

/// True when `have` ("1.94.1") is at least `needed` ("1.94"): `rust-version`
/// is a minimum, not an exact match.
fn version_at_least(have: &str, needed: &str) -> bool {
    fn parse(text: &str) -> Option<(u64, u64, u64)> {
        let mut parts = text.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts
            .next()
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or(0);
        Some((major, minor, patch))
    }
    match (parse(have), parse(needed)) {
        (Some(have), Some(needed)) => have >= needed,
        _ => false,
    }
}

/// The string value of the first `field = "value"` in a TOML file (a plain
/// text scan, enough for the workspace `rust-version`).
pub fn toml_string<'a>(text: &'a str, field: &str) -> Option<&'a str> {
    let at = text.find(field)?;
    let open = text[at..].find('"')? + at;
    let close = text[open + 1..].find('"')? + open + 1;
    Some(&text[open + 1..close])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_string_field() {
        assert_eq!(
            toml_string(
                "[workspace.package]\nrust-version = \"1.94\"\n",
                "rust-version"
            ),
            Some("1.94")
        );
        assert_eq!(
            toml_string(
                "[package]\nname = \"demo\"\nedition = \"2024\"\n",
                "edition"
            ),
            Some("2024")
        );
        assert_eq!(toml_string("[package]\n", "rust-version"), None);
    }

    #[test]
    fn rust_versions_are_compared_as_a_minimum() {
        assert!(version_at_least("1.94.1", "1.94"));
        assert!(version_at_least("1.94.0", "1.94"));
        assert!(version_at_least("2.0.0", "1.94"));
        assert!(!version_at_least("1.93.4", "1.94"));
        assert!(!version_at_least("", "1.94"));
        assert!(!version_at_least("1.94.1", ""));
    }
}
