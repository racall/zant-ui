//! Console prompts, Unicode input, and app log forwarding.

use crate::{IS_WINDOWS, base_command, fail};
use std::io::Write as _;
use std::path::Path;

// ---- console / prompts ----

pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// One line of input (without the line break), "" on error.
// Windows console stdin uses Rust std ReadConsoleW, preserving Unicode.
// Redirected stdin must be UTF-8; GBK-encoded pipes are not decoded.
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
    let _ = crate::run_ok(&["stty".to_string(), "-echo".to_string()], &cwd);
    let line = read_line();
    let _ = crate::run_ok(&["stty".to_string(), "echo".to_string()], &cwd);
    eprintln!();
    line
}

#[cfg(windows)]
fn read_secret_tty() -> String {
    let script = "$s = Read-Host -AsSecureString; [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($s))";
    if let Ok(captured) = crate::run_capture(&[
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

/// Accept English, Chinese, and full-width Latin console answers.
pub fn parse_yes_no(value: &str) -> Option<bool> {
    let normalized: String = value
        .trim()
        .chars()
        .map(|ch| match ch {
            '！'..='～' => char::from_u32(ch as u32 - 0xfee0).unwrap(),
            _ => ch,
        })
        .collect();
    match normalized.to_lowercase().as_str() {
        "y" | "yes" | "是" | "好" | "对" | "要" => Some(true),
        "n" | "no" | "否" | "不" | "不要" => Some(false),
        _ => None,
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
        if let Some(answer) = parse_yes_no(value) {
            return answer;
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
pub(crate) fn filter_stderr(stderr: std::process::ChildStderr) {
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
        .unwrap_or_else(|err| fail(tr!(en: "Could not start {}: {err}", zh: "无法启动 {}: {err}", exe.display(), err = err)));
    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(_) => fail(tr!(en: "{} failed", zh: "{} 执行失败", exe.display())),
        Err(err) => fail(
            tr!(en: "Could not wait for {}: {err}", zh: "等待 {} 失败: {err}", exe.display(), err = err),
        ),
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
mod input_tests {
    use super::*;
    #[test]
    fn yes_no_accepts_unicode_and_english_answers() {
        for answer in ["y", "YES", "是", "好", "对", "要", "ｙ", "ＹＥＳ", "　是　"] {
            assert_eq!(parse_yes_no(answer), Some(true), "{answer}");
        }
        for answer in ["n", "NO", "否", "不", "不要", "ｎ", "ＮＯ", " 不要 "] {
            assert_eq!(parse_yes_no(answer), Some(false), "{answer}");
        }
        for answer in ["", "maybe", "yes please", "不确定"] {
            assert_eq!(parse_yes_no(answer), None, "{answer}");
        }
    }
}
