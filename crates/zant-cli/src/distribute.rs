//! `zant build [config]`, the release
//! build. zant reads the build settings (build.config.json by default, see
//! docs/build.md), asks in a terminal for what is missing (and offers to save
//! the answers), compiles, builds and signs the app, then hands the platform
//! steps to the scripts in `scripts/package/` (macos.sh: DMG, notarization,
//! App Store; windows.ps1: signing, installer, MSIX, Microsoft Store) through
//! ZANT_* environment variables.
//!
//! Secrets are never written to the config file: a value `$NAME` (or
//! `${NAME}`) reads the environment variable NAME, notarization credentials
//! can live in the keychain (notarytool profiles), and passwords typed in the
//! terminal are used for this run only.

mod context;
pub use context::*;

pub mod macos;
pub mod windows;

use crate::generate::Need;
use crate::{COLOR_CYAN, COLOR_RESET, fail, ok};
use std::io::Write as _;
use std::path::Path;
use std::sync::Mutex;

pub const DEFAULT_FILE: &str = "build.config.json";

/// Cleanup on exit and on `fail` (temporary key files, the CI keychain).
struct Cleanup {
    root: String,
    temp_files: Vec<String>,
    temp_keychain: Option<String>,
}

static CLEANUP: Mutex<Option<Cleanup>> = Mutex::new(None);

fn cleanup() {
    let state = CLEANUP.lock().expect("cleanup lock").take();
    let Some(state) = state else {
        return;
    };
    for path in state.temp_files {
        let _ = std::fs::remove_file(path);
    }
    if let Some(keychain) = state.temp_keychain {
        let argv = script_argv("remove-keychain");
        let mut command = std::process::Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .current_dir(&state.root)
            .env("ZANT_KEYCHAIN", keychain);
        #[cfg(windows)]
        command.env_remove("PSModulePath");
        let _ = command.status();
    }
}

fn register_cleanup(root: &str) {
    *CLEANUP.lock().expect("cleanup lock") = Some(Cleanup {
        root: root.to_string(),
        temp_files: Vec::new(),
        temp_keychain: None,
    });
}

fn register_temp_file(path: &str) {
    if let Some(state) = CLEANUP.lock().expect("cleanup lock").as_mut() {
        state.temp_files.push(path.to_string());
    }
}

fn register_temp_keychain(path: &str) {
    if let Some(state) = CLEANUP.lock().expect("cleanup lock").as_mut() {
        state.temp_keychain = Some(path.to_string());
    }
}

/// `zant build [config] [--yes]`.
pub fn cmd_build(file_arg: Option<String>, yes: bool) -> std::io::Result<()> {
    let root = std::env::current_dir()?.to_string_lossy().into_owned();
    let mut cfg = crate::load_config(&root)?;
    crate::generate::complete_platform_config(&root, &mut cfg, Need::Run)?;
    crate::generate::sync_project(&root, &cfg)?;

    let path = match &file_arg {
        Some(file) => {
            let file = Path::new(file);
            if file.is_absolute() {
                file.to_string_lossy().into_owned()
            } else {
                Path::new(&root).join(file).to_string_lossy().into_owned()
            }
        }
        None => Path::new(&root)
            .join(DEFAULT_FILE)
            .to_string_lossy()
            .into_owned(),
    };
    let data = crate::read_file(Path::new(&path)).ok();
    if data.is_none() && file_arg.is_some() {
        fail(
            tr!(en: "Configuration file not found: {path}", zh: "找不到配置文件 {path}", path = path),
        );
    }

    let ci = std::env::var_os("CI").is_some();
    let interactive = !yes && !ci && crate::is_tty();
    let mut c = Ctx {
        root: root.clone(),
        cfg,
        file: BuildFile::default(),
        path: path.clone(),
        exists: data.is_some(),
        interactive,
        changed: false,
        saved: false,
        env: std::env::vars().collect(),
        asc: None,
    };
    if let Some(text) = data {
        let text = String::from_utf8_lossy(&text);
        c.file = serde_json::from_str(&text).unwrap_or_else(|err| {
            fail(tr!(en: "Could not parse {path}: {err} (check for misspelled or unknown fields; see Zant UI docs/build.md)", zh: "{path} 解析失败：{err}（未知的字段名也会报错，检查拼写；格式见 Zant UI 的 docs/build.md）",
             err = err, path = path))
        });
    }
    register_cleanup(&root);
    crate::set_on_fail(Box::new(cleanup));

    eprintln!(
        "{}",
        tr!(en: "\n{COLOR_CYAN}Zant release build{COLOR_RESET} ({})", zh: "\n{COLOR_CYAN}Zant 发布构建{COLOR_RESET}（{}）",
        if crate::IS_WINDOWS {
            "Windows"
        } else {
            "macOS"
        }
    , COLOR_CYAN = COLOR_CYAN, COLOR_RESET = COLOR_RESET)
    );
    if c.exists {
        eprintln!(
            "{}",
            tr!(en: "  Configuration: {path}", zh: "  配置文件：{path}", path = path)
        );
    } else {
        eprintln!(
            "{}",
            tr!(en: "  No {}: {}", zh: "  没有 {}：{}",
                Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                if c.interactive {
                    tr!(en: "Missing settings will be prompted for; you can save them afterward", zh: "缺的设置会逐项询问，最后可以保存")
                } else {
                    tr!(en: "Using default settings", zh: "使用默认设置")
                }
            )
        );
    }
    if !c.interactive {
        eprintln!(
            "{}",
            tr!(en: "  Non-interactive mode (--yes / CI): required settings must be configured in advance", zh: "  非交互模式（--yes / CI）：不提问，缺少必需的设置时停止",)
        );
    }

    let result = if crate::IS_WINDOWS {
        windows::windows_flow(&mut c)
    } else {
        macos::mac_flow(&mut c)
    };
    if result.is_ok() {
        save(&mut c);
    }
    cleanup();
    crate::clear_on_fail();
    result
}

// ---- values, questions, saving ----

/// `$NAME` / `${NAME}` → NAME.
pub fn env_name(value: &str) -> Option<&str> {
    if value.len() < 2 || !value.starts_with('$') {
        return None;
    }
    let name = &value[1..];
    if name.len() > 2 && name.starts_with('{') && name.ends_with('}') {
        return Some(&name[1..name.len() - 1]);
    }
    Some(name)
}

pub fn expand_path(c: &Ctx, path: &str) -> String {
    if path.len() >= 2
        && path.starts_with('~')
        && (path[1..].starts_with('/') || path[1..].starts_with('\\'))
    {
        let home = c
            .get_env(if crate::IS_WINDOWS {
                "USERPROFILE"
            } else {
                "HOME"
            })
            .unwrap_or_default();
        return Path::new(&home)
            .join(&path[2..])
            .to_string_lossy()
            .into_owned();
    }
    if Path::new(path).is_absolute() {
        return path.to_string();
    }
    Path::new(&c.root).join(path).to_string_lossy().into_owned()
}

pub fn file_exists(_c: &Ctx, path: &str) -> bool {
    crate::path_exists(Path::new(path))
}

pub fn in_project(c: &Ctx, path: &str) -> bool {
    path.starts_with(&c.root)
        && path.len() > c.root.len()
        && matches!(path.as_bytes()[c.root.len()], b'/' | b'\\')
}

/// Writes a secret (key file contents from an environment variable) to
/// target/build, readable only by the user; deleted when zant exits.
pub fn write_secret_file(c: &Ctx, name: &str, data: &[u8]) -> std::io::Result<String> {
    let dir = Path::new(&c.root).join("target/build");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    let path_string = path.to_string_lossy().into_owned();
    register_temp_file(&path_string);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(data)?;
    }
    #[cfg(not(unix))]
    {
        crate::write_file(&path, data)?;
    }
    Ok(path_string)
}

/// A file given as a path or as base64 contents (CI secrets).
pub fn file_or_base64(c: &Ctx, value: &str, key: &str, name: &str) -> std::io::Result<String> {
    let path = expand_path(c, value);
    if file_exists(c, &path) {
        return Ok(path);
    }
    let decoded = base64_decode(value)
        .unwrap_or_else(|| fail(tr!(en: "{key}: expected an existing file or base64 content", zh: "{key}：既不是存在的文件，也不是 base64 内容", key = key)));
    write_secret_file(c, name, &decoded)
}

/// Standard base64 with the CI line breaks ignored; `None` on invalid input.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    let mut padding = 0;
    for byte in text.bytes() {
        if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
            continue;
        }
        if byte == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return None;
        }
        let index = TABLE.iter().position(|candidate| *candidate == byte)?;
        buffer = (buffer << 6) | index as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    let letters = text
        .bytes()
        .filter(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        .count();
    if letters % 4 != 0 || padding > 2 || bits >= 6 || buffer != 0 {
        return None;
    }
    Some(out)
}

/// Offers once to write the answers to the config file (no passwords or key
/// contents are ever in it: only names, options and `$VARIABLE` references).
pub fn save(c: &mut Ctx) {
    if !c.changed || c.saved || !c.interactive {
        return;
    }
    c.saved = true;
    eprintln!();
    let label = tr!(en: "Save these settings to {} for next time? Only certificate names and options are saved, without passwords or private keys", zh: "把这些设置保存到 {}？下次直接使用；文件里只有证书名称和选项，不含密码和密钥内容",
        Path::new(&c.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    if !crate::prompt_yes_no(&label, true) {
        return;
    }
    let Ok(text) = serde_json::to_string_pretty(&c.file) else {
        return;
    };
    let contents = format!("{text}\n");
    if let Err(err) = crate::write_file(Path::new(&c.path), contents.as_bytes()) {
        eprintln!(
            "{}",
            tr!(en: "  Could not save: {err}", zh: "  保存失败：{err}", err = err)
        );
        return;
    }
    ok(&tr!(en: "Saved {}", zh: "已保存 {}", c.path));
}

// ---- scripts ----

fn script_path() -> String {
    let name = if crate::IS_WINDOWS {
        "windows.ps1"
    } else {
        "macos.sh"
    };
    crate::zant_root()
        .join("scripts/package")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn script_argv(action: &str) -> Vec<String> {
    let script = script_path();
    if crate::IS_WINDOWS {
        vec![
            "powershell".to_string(),
            "-NoProfile".to_string(),
            "-NonInteractive".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            script,
            action.to_string(),
        ]
    } else {
        vec!["/bin/bash".to_string(), script, action.to_string()]
    }
}

/// "Name (TEAMID)" → TEAMID
pub fn team_of(identity: &str) -> Option<&str> {
    if identity.len() < 12 || !identity.ends_with(')') {
        return None;
    }
    let open = identity.rfind('(')?;
    Some(&identity[open + 1..identity.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msix_version_and_identity_helpers() {
        assert_eq!(windows::msix_version("0.1.0"), "0.1.0.0");
        assert_eq!(windows::msix_version("2.5"), "2.5.0.0");
        assert_eq!(
            team_of("Developer ID Application: Some One (ABCDE12345)"),
            Some("ABCDE12345")
        );
        assert_eq!(team_of("none"), None);
        assert_eq!(
            windows::common_name("CN=Some One, O=Acme, C=CN").as_deref(),
            Some("Some One")
        );
        assert!(windows::is_thumbprint(
            "0123456789abcdef0123456789ABCDEF01234567"
        ));
        assert_eq!(env_name("${NAME}"), Some("NAME"));
        assert_eq!(env_name("$NAME"), Some("NAME"));
        assert_eq!(env_name("plain"), None);
    }

    #[test]
    fn base64_decodes_padded_input_and_skips_line_breaks() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGVs\nbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("YQ==").unwrap(), b"a");
        assert!(base64_decode("YQ").is_none());
        assert!(base64_decode("a=").is_none());
        assert!(base64_decode("!!!!").is_none());
    }

    #[test]
    fn build_config_parses_the_documented_fields_and_rejects_typos() {
        let file: BuildFile = serde_json::from_str(
            "{ \"build_number\": \"$BUILD_NUMBER\",
               \"macos\": { \"distribution\": \"developer-id\", \"signing_identity\": \"Developer ID Application: Me (ABCDE12345)\",
                          \"dmg_background\": \"default\", \"notarize\": true, \"notary\": { \"profile\": \"zant-notary\" } },
               \"windows\": { \"distribution\": \"installer\", \"publisher\": \"Me\", \"certificate\": \"$PFX\" },
               \"app_store_connect\": { \"issuer_id\": \"I\", \"key_id\": \"K\", \"key\": \"$P8\" } }",
        )
        .expect("parse build.config.json");
        assert_eq!(file.macos.unwrap().format, None);
        assert!(serde_json::from_str::<BuildFile>("{ \"bogus\": 1 }").is_err());
        assert!(serde_json::from_str::<BuildFile>("{ \"macos\": { \"Bogus\": 1 } }").is_err());
        // Saving omits unset optionals (emit_null_optional_fields = false).
        let text = serde_json::to_string_pretty(&BuildFile::default()).expect("serialize");
        assert!(!text.contains("null"), "{text}");
    }
}
