//! Signing certificates and provisioning profiles.

use crate::distribute::{Ctx, expand_path, file_exists};
use crate::{COLOR_DIM, COLOR_RESET, fail};

/// Valid identities in the keychains: codesigning ones, or (installer) the
/// installer certificates, which `security` lists only without a policy.
fn identities(c: &Ctx, keychain: Option<&str>, installer: bool) -> Vec<String> {
    let mut argv = vec![
        "security".to_string(),
        "find-identity".to_string(),
        "-v".to_string(),
    ];
    if !installer {
        argv.push("-p".to_string());
        argv.push("codesigning".to_string());
    }
    if let Some(kc) = keychain {
        argv.push(kc.to_string());
    }
    let _ = c;
    let Ok(captured) = crate::run_capture(&argv) else {
        return Vec::new();
    };
    let mut list: Vec<String> = Vec::new();
    for line in captured.stdout.split('\n') {
        let Some(start) = line.find('"') else {
            continue;
        };
        let Some(end) = line[start + 1..].find('"') else {
            continue;
        };
        let name = line[start + 1..start + 1 + end].to_string();
        if installer != name.contains("Installer") {
            continue;
        }
        if !list.contains(&name) {
            list.push(name);
        }
    }
    list
}

fn recommended(store: bool, name: &str) -> bool {
    if store {
        name.starts_with("Apple Distribution")
            || name.starts_with("3rd Party Mac Developer Application")
    } else {
        name.starts_with("Developer ID Application")
    }
}

pub(super) fn ask_identity(c: &Ctx, store: bool) -> std::io::Result<String> {
    let all = identities(c, None, false);
    let mut list: Vec<String> = Vec::new();
    for id in &all {
        if recommended(store, id) {
            list.push(id.clone());
        }
    }
    let n_rec = list.len();
    for id in &all {
        if !recommended(store, id) {
            list.push(id.clone());
        }
    }
    let kind = if store {
        "Apple Distribution"
    } else {
        "Developer ID Application"
    };
    eprintln!(
        "{}",
        tr!(en: "\nSigning certificates (from Keychain):", zh: "\n签名证书（来自钥匙串）:",)
    );
    if !store {
        eprintln!(
            "{}",
            tr!(en: "  0) Ad-hoc (no certificate; local testing only, cannot be notarized)", zh: "  0) Ad-hoc（不用证书，只能在本机测试，不能公证）",)
        );
    }
    for (index, id) in list.iter().enumerate() {
        eprintln!(
            "  {}) {id}{}",
            index + 1,
            if index < n_rec {
                tr!(en: " (recommended)", zh: "（推荐）")
            } else {
                ""
            }
        );
    }
    if n_rec == 0 {
        eprintln!(
            "{}",
            tr!(en: "  {COLOR_DIM}No “{kind}” certificate in Keychain: add one in Xcode → Settings → Accounts → Manage Certificates, or download from developer.apple.com and double-click to install{COLOR_RESET}", zh: "  {COLOR_DIM}钥匙串里没有「{kind}」证书：在 Xcode → 设置 → 帐户 → 管理证书 里添加，或从 developer.apple.com 下载后双击安装{COLOR_RESET}",
         COLOR_DIM = COLOR_DIM, COLOR_RESET = COLOR_RESET, kind = kind)
        );
        if store && list.is_empty() {
            fail(tr!(en: "No usable signing certificate", zh: "没有可用的签名证书"));
        }
    }
    let default = if n_rec > 0 || store { "1" } else { "0" };
    loop {
        let value = crate::prompt(tr!(en: "Choose", zh: "选择"), default);
        let Ok(number) = value.parse::<usize>() else {
            continue;
        };
        if number == 0 && !store {
            return Ok("none".to_string());
        }
        if (1..=list.len()).contains(&number) {
            if number > n_rec {
                eprintln!(
                    "{}",
                    tr!(en: "  {COLOR_DIM}Hint: this certificate is not “{kind}”; {}{COLOR_RESET}", zh: "  {COLOR_DIM}提示：这个证书不是「{kind}」，{}{COLOR_RESET}",
                    if store {
                        tr!(en: "App Store will reject it", zh: "App Store 不会接受")
                    } else {
                        tr!(en: "Signed apps cannot be notarized and will be blocked on other Macs", zh: "签名的应用不能公证，别人的 Mac 会拦截")
                    }
                , COLOR_DIM = COLOR_DIM, COLOR_RESET = COLOR_RESET, kind = kind)
                );
            }
            return Ok(list[number - 1].clone());
        }
    }
}

pub(super) fn first_identity(c: &Ctx, keychain: &str, store: bool) -> std::io::Result<String> {
    let all = identities(c, Some(keychain), false);
    for id in &all {
        if recommended(store, id) {
            return Ok(id.clone());
        }
    }
    if let Some(first) = all.first() {
        return Ok(first.clone());
    }
    fail(
        tr!(en: "No usable signing identity in imported certificates", zh: "导入的证书里没有可用的签名身份"),
    )
}

pub(super) fn first_installer(c: &Ctx, keychain: &str) -> std::io::Result<String> {
    let all = identities(c, Some(keychain), true);
    if let Some(first) = all.first() {
        return Ok(first.clone());
    }
    fail(
        tr!(en: "Imported certificates contain no installer certificate (3rd Party Mac Developer Installer / Mac Installer Distribution); set macos.installer_identity separately", zh: "导入的证书里没有安装包证书（3rd Party Mac Developer Installer / Mac Installer Distribution）：macos.installer_identity 要另外设置"),
    )
}

pub(super) fn check_identity(
    c: &Ctx,
    identity: &str,
    key: &str,
    installer: bool,
) -> std::io::Result<()> {
    let all = identities(c, None, installer);
    if all.iter().any(|id| id == identity || id.contains(identity)) {
        return Ok(());
    }
    eprintln!(
        "{}",
        tr!(en: "\n  Available {} certificates in Keychain:", zh: "\n  钥匙串里可用的{}证书：",
            if installer { tr!(en: "installer", zh: "安装包") } else { tr!(en: "signing", zh: "签名") }
        )
    );
    for id in &all {
        eprintln!("    {id}");
    }
    fail(
        tr!(en: "{key}: no usable certificate “{identity}” in Keychain", zh: "{key}：钥匙串里没有可用的证书「{identity}」", identity = identity, key = key),
    )
}

pub(super) fn ask_installer(c: &Ctx) -> std::io::Result<String> {
    let all = identities(c, None, true);
    if all.is_empty() {
        fail(
            tr!(en: "No installer certificate in Keychain: add “Mac Installer Distribution” in Xcode → Settings → Accounts → Manage Certificates", zh: "钥匙串里没有安装包证书：在 Xcode → 设置 → 帐户 → 管理证书 里添加「Mac Installer Distribution」"),
        );
    }
    let options: Vec<&str> = all.iter().map(String::as_str).collect();
    let index = c.choose(
        tr!(en: "Installer certificate (signs .pkg)", zh: "安装包证书（签 .pkg）"),
        &options,
        0,
    );
    Ok(all[index].clone())
}

#[derive(Debug, Default)]
pub(super) struct ProfileInfo {
    name: Option<String>,
    pub(super) team: Option<String>,
    app_id: Option<String>,
    mac: bool,
}

/// The `<string>` after `<key>key</key>` in a decoded profile.
fn plist_string(xml: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let at = xml.find(&needle)?;
    let start = xml[at..].find("<string>")? + at;
    let end = xml[start..].find("</string>")? + start;
    Some(xml[start + "<string>".len()..end].to_string())
}

pub(super) fn profile_info(c: &Ctx, path: &str) -> std::io::Result<ProfileInfo> {
    let _ = c;
    let Ok(captured) = crate::run_capture(&[
        "security".to_string(),
        "cms".to_string(),
        "-D".to_string(),
        "-i".to_string(),
        path.to_string(),
    ]) else {
        return Ok(ProfileInfo::default());
    };
    let xml = captured.stdout;
    Ok(ProfileInfo {
        name: plist_string(&xml, "Name"),
        team: plist_string(&xml, "TeamIdentifier"),
        app_id: plist_string(&xml, "com.apple.application-identifier"),
        mac: xml.contains("<string>OSX</string>"),
    })
}

/// Installed Mac provisioning profiles for this bundle id (Xcode keeps them
/// in these folders), or a path typed in.
pub(super) fn ask_profile(c: &mut Ctx) -> std::io::Result<String> {
    let home = c.get_env("HOME").unwrap_or_default();
    let mut found: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for sub in [
        "Library/Developer/Xcode/UserData/Provisioning Profiles",
        "Library/MobileDevice/Provisioning Profiles",
    ] {
        let dir_path = std::path::Path::new(&home).join(sub);
        let Ok(entries) = std::fs::read_dir(&dir_path) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".provisionprofile") {
                continue;
            }
            let path = dir_path.join(&name).to_string_lossy().into_owned();
            let info = profile_info(c, &path)?;
            let Some(app_id) = info.app_id else {
                continue;
            };
            let Some(dot) = app_id.find('.') else {
                continue;
            };
            let bundle = &app_id[dot + 1..];
            if !info.mac || !(bundle == c.cfg.bundle_id || bundle == "*") {
                continue;
            }
            found.push(path);
            labels.push(format!("{}（{}）", info.name.unwrap_or(name), app_id));
        }
    }
    labels.push(
        tr!(en: "Enter .provisionprofile file path", zh: "输入 .provisionprofile 文件路径")
            .to_string(),
    );
    eprintln!(
        "{}",
        tr!(en: "\n  {COLOR_DIM}Provisioning profile: developer.apple.com → Certificates, IDs & Profiles → Profiles; create a “Mac App Store Connect” profile, select {}, and download{COLOR_RESET}", zh: "\n  {COLOR_DIM}描述文件：developer.apple.com → Certificates, IDs & Profiles → Profiles 新建「Mac App Store Connect」类型，选 {}，下载{COLOR_RESET}",
        c.cfg.bundle_id
    , COLOR_DIM = COLOR_DIM, COLOR_RESET = COLOR_RESET)
    );
    let options: Vec<&str> = labels.iter().map(String::as_str).collect();
    let index = c.choose(
        tr!(en: "Mac App Store provisioning profile", zh: "Mac App Store 描述文件"),
        &options,
        0,
    );
    if index < found.len() {
        return Ok(found[index].clone());
    }
    loop {
        let path = c.ask_required(
            tr!(en: ".provisionprofile file path", zh: ".provisionprofile 文件路径"),
            "",
        );
        if file_exists(c, &expand_path(c, &path)) {
            return Ok(path);
        }
        eprintln!("{}", tr!(en: "  File not found", zh: "  找不到这个文件",));
    }
}
