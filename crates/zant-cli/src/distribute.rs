//! `zant build [配置文件]`, the release
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

pub mod macos;
pub mod windows;

use crate::app_config::AppConfig;
use crate::generate::Need;
use crate::{COLOR_CYAN, COLOR_GREEN, COLOR_RESET, fail, ok};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;
use std::sync::Mutex;

pub const DEFAULT_FILE: &str = "build.config.json";

/// build.config.json. Every field is optional; a missing one is asked in a
/// terminal or (with --yes / in CI) uses its default or stops with a message
/// naming the key.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct BuildFile {
    /// CFBundleVersion: App Store Connect needs a new one for every upload of
    /// the same version (default: `version` from app.config.json)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macos: Option<Macos>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<Windows>,
    /// API key for App Store uploads (and optionally notarization)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_store_connect: Option<AscKey>,
}

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Macos {
    /// "developer-id" (DMG, notarized) | "app-store"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
    /// codesign identity as `security find-identity` shows it; "none" = ad-hoc
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_identity: Option<String>,
    /// CI: a .p12 (path, or base64 contents) imported into a temporary keychain
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_password: Option<String>,
    /// developer-id: "dmg" (default) | "zip"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// DMG window background: a PNG, "default" or "none"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dmg_background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notarize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notary: Option<Notary>,
    /// app-store: LSApplicationCategoryType
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// app-store: "3rd Party Mac Developer Installer: …" / "Mac Installer Distribution: …"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer_identity: Option<String>,
    /// app-store: the Mac App Store provisioning profile (.provisionprofile)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioning_profile: Option<String>,
    /// app-store: upload the .pkg to App Store Connect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<bool>,
}

/// One of: a notarytool keychain profile, the App Store Connect API key, or
/// an Apple ID with an app-specific password.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apple_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
}

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AscKey {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    /// the .p8 file: a path, or its contents (CI secret)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Windows {
    /// "installer" (Inno Setup) | "store" (MSIX) | "folder" (zip)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
    /// shown by the installer and in Settings → Apps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// a certificate in the user's certificate store: thumbprint or subject
    /// name; "none" = not signed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_identity: Option<String>,
    /// a .pfx (path, or base64 contents) instead of the certificate store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<Store>,
}

/// Values from Partner Center → the app → Product management → Product identity.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Store {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher_display_name: Option<String>,
    /// submit with the Microsoft Store Developer CLI (msstore)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

/// An App Store Connect API key resolved to a readable .p8 file.
#[derive(Debug, Clone)]
pub struct Asc {
    pub issuer: String,
    pub id: String,
    pub path: String,
}

pub struct Ctx {
    pub root: String,
    pub cfg: AppConfig,
    pub file: BuildFile,
    pub path: String,
    pub exists: bool,
    pub interactive: bool,
    pub changed: bool,
    pub saved: bool,
    /// ZANT_* variables / the inherited environment for the scripts
    pub env: HashMap<String, String>,
    pub asc: Option<Asc>,
}

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

impl Ctx {
    pub fn get_env(&self, name: &str) -> Option<String> {
        match self.env.get(name) {
            Some(value) if !value.is_empty() => Some(value.clone()),
            _ => None,
        }
    }

    /// A config value: the literal, or `$NAME` from the environment.
    pub fn value(&self, value: &str, key: &str) -> String {
        if let Some(name) = env_name(value) {
            return self
                .get_env(name)
                .unwrap_or_else(|| fail(format!("{key} 是环境变量 {name}，但它没有设置")));
        }
        value.to_string()
    }

    /// Like `value`, but an unset variable is typed (without echo) in a
    /// terminal, for this run only.
    pub fn secret(&self, value: &str, key: &str, label: &str) -> String {
        if let Some(name) = env_name(value) {
            if let Some(found) = self.get_env(name) {
                return found;
            }
            if !self.interactive {
                fail(format!("{key} 是环境变量 {name}，但它没有设置"));
            }
            eprint!("{label}（不显示、不保存；也可以设置环境变量 {name}）: ");
            let _ = std::io::stderr().flush();
            return crate::read_secret();
        }
        value.to_string()
    }

    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());
    }

    pub fn need(&self, key: &str, what: &str) -> ! {
        fail(format!(
            "{} 缺少 {key}（{what}）。在终端里运行 zant build 按提示选择，或写进配置文件",
            Path::new(&self.path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.clone())
        ))
    }

    pub fn ask_required(&self, label: &str, default: &str) -> String {
        loop {
            let value = if !default.is_empty() {
                crate::prompt(label, default)
            } else {
                eprint!("{label}: ");
                let _ = std::io::stderr().flush();
                crate::read_line().trim_matches(' ').to_string()
            };
            if !value.is_empty() {
                return value;
            }
        }
    }

    /// Numbered choice; returns the index.
    pub fn choose(&self, title: &str, options: &[&str], default: usize) -> usize {
        eprintln!("\n{title}:");
        for (index, option) in options.iter().enumerate() {
            eprintln!("  {}) {option}", index + 1);
        }
        loop {
            let value = crate::prompt("选择", &(default + 1).to_string());
            let Ok(number) = value.parse::<usize>() else {
                continue;
            };
            if (1..=options.len()).contains(&number) {
                return number - 1;
            }
        }
    }

    pub fn print_result(&self, artifact: &str) {
        eprintln!("\n{COLOR_GREEN}完成{COLOR_RESET}：{artifact}\n");
    }

    pub fn build_number(&self) -> Option<String> {
        self.file
            .build_number
            .as_ref()
            .map(|build| self.value(build, "build_number"))
    }

    pub fn capture_script(&self, action: &str) -> String {
        let argv = script_argv(action);
        let mut command = std::process::Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .current_dir(&self.root)
            .envs(&self.env);
        #[cfg(windows)]
        command.env_remove("PSModulePath");
        match command.output() {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                .trim_matches([' ', '\t', '\r', '\n'])
                .to_string(),
            _ => String::new(),
        }
    }

    pub fn run_script(&self, action: &str) {
        let _ = self.run_script_env(&self.env, action, true);
    }

    pub fn run_script_env(&self, env: &HashMap<String, String>, action: &str, must: bool) -> bool {
        let argv = script_argv(action);
        let mut command = std::process::Command::new(&argv[0]);
        command.args(&argv[1..]).current_dir(&self.root).envs(env);
        #[cfg(windows)]
        command.env_remove("PSModulePath");
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                if must {
                    fail(format!("无法运行打包脚本：{err}"));
                }
                return false;
            }
        };
        match child.wait() {
            Ok(status) if status.success() => true,
            _ => {
                if must {
                    fail("打包没有完成（见上面的说明）");
                }
                false
            }
        }
    }
}

/// `zant build [配置文件] [--yes]`.
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
        fail(format!("找不到配置文件 {path}"));
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
            fail(format!(
                "{path} 解析失败：{err}（未知的字段名也会报错，检查拼写；格式见 Zant UI 的 docs/build.md）"
            ))
        });
    }
    register_cleanup(&root);
    crate::set_on_fail(Box::new(cleanup));

    eprintln!(
        "\n{COLOR_CYAN}Zant 发布构建{COLOR_RESET}（{}）",
        if crate::IS_WINDOWS {
            "Windows"
        } else {
            "macOS"
        }
    );
    if c.exists {
        eprintln!("  配置文件：{path}");
    } else {
        eprintln!(
            "  没有 {}：{}",
            Path::new(&path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            if c.interactive {
                "缺的设置会逐项询问，最后可以保存"
            } else {
                "使用默认设置"
            }
        );
    }
    if !c.interactive {
        eprintln!("  非交互模式（--yes / CI）：不提问，缺少必需的设置时停止");
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
        .unwrap_or_else(|| fail(format!("{key}：既不是存在的文件，也不是 base64 内容")));
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
    let label = format!(
        "把这些设置保存到 {}？下次直接使用；文件里只有证书名称和选项，不含密码和密钥内容",
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
        eprintln!("  保存失败：{err}");
        return;
    }
    ok(&format!("已保存 {}", c.path));
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
