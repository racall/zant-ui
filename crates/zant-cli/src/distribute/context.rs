//! Release configuration and interactive prompts.

use super::{env_name, script_argv};
use crate::app_config::AppConfig;
use crate::{COLOR_GREEN, COLOR_RESET, fail};
use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;

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
                .unwrap_or_else(|| fail(tr!(en: "{key} refers to environment variable {name}, which is not set", zh: "{key} 是环境变量 {name}，但它没有设置", key = key, name = name)));
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
                fail(
                    tr!(en: "{key} refers to environment variable {name}, which is not set", zh: "{key} 是环境变量 {name}，但它没有设置", key = key, name = name),
                );
            }
            eprint!(
                "{}",
                tr!(en: "{label} (hidden, not saved; or set environment variable {name}): ", zh: "{label}（不显示、不保存；也可以设置环境变量 {name}）: ", label = label, name = name)
            );
            let _ = std::io::stderr().flush();
            return crate::read_secret();
        }
        value.to_string()
    }

    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());
    }

    pub fn need(&self, key: &str, what: &str) -> ! {
        fail(
            tr!(en: "{} is missing {key} ({what}). Run zant build in a terminal to configure it, or edit the configuration file", zh: "{} 缺少 {key}（{what}）。在终端里运行 zant build 按提示选择，或写进配置文件",
            Path::new(&self.path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.path.clone())
        , key = key, what = what),
        )
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
            let value = crate::prompt(tr!(en: "Choose", zh: "选择"), &(default + 1).to_string());
            let Ok(number) = value.parse::<usize>() else {
                continue;
            };
            if (1..=options.len()).contains(&number) {
                return number - 1;
            }
        }
    }

    pub fn print_result(&self, artifact: &str) {
        eprintln!(
            "{}",
            tr!(en: "\n{COLOR_GREEN}Done{COLOR_RESET}: {artifact}\n", zh: "\n{COLOR_GREEN}完成{COLOR_RESET}：{artifact}\n", COLOR_GREEN = COLOR_GREEN, COLOR_RESET = COLOR_RESET, artifact = artifact)
        );
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
            .envs(&self.env)
            .env("ZANT_LANG", crate::lang::tag());
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
        command
            .args(&argv[1..])
            .current_dir(&self.root)
            .envs(env)
            .env("ZANT_LANG", crate::lang::tag());
        #[cfg(windows)]
        command.env_remove("PSModulePath");
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                if must {
                    fail(
                        tr!(en: "Could not run packaging script: {err}", zh: "无法运行打包脚本：{err}", err = err),
                    );
                }
                return false;
            }
        };
        match child.wait() {
            Ok(status) if status.success() => true,
            _ => {
                if must {
                    fail(
                        tr!(en: "Packaging did not complete; see the messages above", zh: "打包没有完成（见上面的说明）"),
                    );
                }
                false
            }
        }
    }
}
