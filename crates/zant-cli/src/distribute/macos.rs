//! Port of the macOS half of `tools/zant/distribute.zig`: `zant build` on a
//! Mac (Developer ID / DMG / notarization, or Mac App Store).

use super::{
    Asc, Ctx, Macos, Notary, expand_path, file_exists, file_or_base64, in_project, save, team_of,
    write_secret_file,
};
use crate::bundle::BundleOptions;
use crate::generate::{self, PlistExtras};
use crate::{COLOR_DIM, COLOR_RED, COLOR_RESET, dim_line, fail, ok, run_inherit};

pub fn mac_flow(c: &mut Ctx) -> std::io::Result<()> {
    let mut mac = c.file.macos.take().unwrap_or_default();
    if mac.distribution.is_none() {
        if c.interactive {
            let index = c.choose(
                "发布方式（macOS）",
                &[
                    "自己分发：Developer ID 签名 → DMG → Apple 公证（推荐）",
                    "Mac App Store：Apple Distribution 签名 → .pkg → 提交 App Store Connect",
                ],
                0,
            );
            mac.distribution = Some(if index == 0 {
                "developer-id".to_string()
            } else {
                "app-store".to_string()
            });
            c.changed = true;
        } else {
            mac.distribution = Some("developer-id".to_string());
        }
    }
    let dist = c.value(mac.distribution.as_deref().unwrap(), "macos.distribution");
    let store = dist == "app-store";
    if !store && dist != "developer-id" {
        fail(format!(
            "macos.distribution 只能是 developer-id 或 app-store（现在是 {dist}）"
        ));
    }

    // CI: the certificate goes into a temporary keychain
    let mut keychain: Option<String> = None;
    if let Some(cert) = mac.certificate.clone() {
        let kc = format!("{}/target/build/zant-signing.keychain-db", c.root);
        if let Some(parent) = std::path::Path::new(&kc).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cert_path = file_or_base64(
            c,
            &c.value(&cert, "macos.certificate"),
            "macos.certificate",
            "signing.p12",
        )?;
        c.set_env("ZANT_CERT", &cert_path);
        let password = match &mac.certificate_password {
            Some(password) => c.secret(password, "macos.certificate_password", "证书密码"),
            None => String::new(),
        };
        c.set_env("ZANT_CERT_PASSWORD", &password);
        c.set_env("ZANT_KEYCHAIN", &kc);
        super::register_temp_keychain(&kc);
        c.run_script("import-cert");
        keychain = Some(kc);
    }

    if mac.signing_identity.is_none() {
        if let Some(kc) = &keychain {
            mac.signing_identity = Some(first_identity(c, kc, store)?);
        } else if c.interactive {
            mac.signing_identity = Some(ask_identity(c, store)?);
            c.changed = true;
        } else {
            c.need(
                "macos.signing_identity",
                "签名证书，如 Developer ID Application: 名字 (TEAMID)",
            );
        }
    }
    let identity = c.value(
        mac.signing_identity.as_deref().unwrap(),
        "macos.signing_identity",
    );
    let adhoc = identity == "none";
    if store && adhoc {
        fail("Mac App Store 需要 Apple Distribution 证书，不能用 Ad-hoc 签名");
    }
    if !adhoc && keychain.is_none() {
        check_identity(c, &identity, "macos.signing_identity", false)?;
    }

    // App Store: every question before the long build
    let mut profile: Option<String> = None;
    let mut team: Option<String> = None;
    let mut category: Option<String> = mac
        .category
        .as_ref()
        .map(|value| c.value(value, "macos.category"));
    if store {
        if category.is_none() {
            if !c.interactive {
                c.need(
                    "macos.category",
                    "App Store 分类，如 public.app-category.utilities",
                );
            }
            eprintln!(
                "\nApp Store 分类（LSApplicationCategoryType），常用：utilities 工具、productivity 效率、developer-tools 开发者工具、graphics-design 图形和设计"
            );
            let value = crate::prompt("分类", "public.app-category.utilities");
            mac.category = Some(if value.starts_with("public.app-category.") {
                value
            } else {
                format!("public.app-category.{value}")
            });
            category = mac.category.clone();
            c.changed = true;
        }
        if mac.provisioning_profile.is_none() {
            if !c.interactive {
                c.need(
                    "macos.provisioning_profile",
                    "Mac App Store 描述文件（.provisionprofile）的路径",
                );
            }
            mac.provisioning_profile = Some(ask_profile(c)?);
            c.changed = true;
        }
        let profile_path = expand_path(
            c,
            &c.value(
                mac.provisioning_profile.as_deref().unwrap(),
                "macos.provisioning_profile",
            ),
        );
        if !file_exists(c, &profile_path) {
            fail(format!("找不到描述文件 {profile_path}"));
        }
        profile = Some(profile_path);
        let info = profile_info(c, profile.as_deref().unwrap())?;
        team = match info.team {
            Some(team) => Some(team),
            None => match team_of(&identity) {
                Some(team) => Some(team.to_string()),
                None => fail("无法从描述文件或证书得到团队 ID"),
            },
        };
        if mac.installer_identity.is_none() {
            if let Some(kc) = &keychain {
                mac.installer_identity = Some(first_installer(c, kc)?);
            } else if c.interactive {
                mac.installer_identity = Some(ask_installer(c)?);
                c.changed = true;
            } else {
                c.need(
                    "macos.installer_identity",
                    "安装包证书，如 3rd Party Mac Developer Installer: 名字 (TEAMID)",
                );
            }
        }
        if keychain.is_none() {
            check_identity(
                c,
                &c.value(
                    mac.installer_identity.as_deref().unwrap(),
                    "macos.installer_identity",
                ),
                "macos.installer_identity",
                true,
            )?;
        }
        if mac.upload.is_none() {
            if c.interactive {
                eprintln!();
                mac.upload = Some(crate::prompt_yes_no(
                    "打包后提交到 App Store Connect？",
                    true,
                ));
                c.changed = true;
            } else {
                mac.upload = Some(false);
            }
        }
        if mac.upload.unwrap_or(false) {
            let _ = asc_key(c)?;
        }
        c.file.macos = Some(mac.clone());
        save(c);
        if !c.cfg.sandbox {
            dim_line(
                "Mac App Store 要求 App Sandbox：这次自动开启。上架前在 app.config.json 设 macos.sandbox: true，用 zant run 检查沙盒里各功能是否正常",
            );
        }
    }

    // the signed app
    let build_dir = format!("{}/target/build", c.root);
    std::fs::create_dir_all(&build_dir)?;
    let mut ent: Option<String> = None;
    if store || c.cfg.sandbox || c.cfg.entitlements.is_some() {
        let path = format!("{build_dir}/app.entitlements");
        let plist =
            generate::entitlements_plist(&c.cfg, if store { team.as_deref() } else { None });
        crate::write_file(std::path::Path::new(&path), plist.as_bytes())?;
        ent = Some(path);
    }
    eprintln!();
    crate::bundle::compile_app(&c.root, true);
    let exe = crate::app_exe_path(&c.root, true);
    let app = crate::bundle::build_mac_bundle(
        &c.root,
        &c.cfg,
        &exe,
        &BundleOptions {
            identity: identity.clone(),
            entitlements: ent,
            provisioning_profile: profile,
            keychain: keychain.clone(),
            plist: PlistExtras {
                build_number: c.build_number(),
                category: category.clone(),
                ..PlistExtras::default()
            },
        },
    );
    if !adhoc {
        run_inherit(
            &[
                "codesign".to_string(),
                "--verify".to_string(),
                "--strict".to_string(),
                "--deep".to_string(),
                app.clone(),
            ],
            &c.root,
        );
        ok("签名有效");
    }

    let out = format!("{}/target/dist", c.root);
    c.set_env("ZANT_APP", &app);
    let title = c.cfg.title.clone();
    let version = c.cfg.version.clone();
    c.set_env("ZANT_TITLE", &title);
    c.set_env("ZANT_VERSION", &version);
    c.set_env("ZANT_OUT", &out);
    c.set_env("ZANT_IDENTITY", &identity);
    if store {
        c.set_env(
            "ZANT_INSTALLER_IDENTITY",
            &c.value(
                mac.installer_identity.as_deref().unwrap(),
                "macos.installer_identity",
            ),
        );
        c.set_env(
            "ZANT_UPLOAD",
            if mac.upload.unwrap_or(false) {
                "1"
            } else {
                "0"
            },
        );
        if mac.upload.unwrap_or(false) {
            let key = asc_key(c)?;
            c.set_env("ZANT_ASC_KEY", &key.path);
            c.set_env("ZANT_ASC_KEY_ID", &key.id);
            c.set_env("ZANT_ASC_ISSUER", &key.issuer);
        }
        c.file.macos = Some(mac);
        c.run_script("app-store");
        c.print_result(&format!("{out}/{}-{}.pkg", c.cfg.title, c.cfg.version));
        return Ok(());
    }

    let format = mac
        .format
        .as_ref()
        .map(|format| c.value(format, "macos.format"))
        .unwrap_or_else(|| "dmg".to_string());
    if format != "dmg" && format != "zip" {
        fail(format!("macos.format 只能是 dmg 或 zip（现在是 {format}）"));
    }
    let can_notarize = identity.starts_with("Developer ID Application");
    if mac.notarize.is_none() {
        if !can_notarize {
            dim_line("不是 Developer ID Application 证书，跳过公证（只有它签名的应用能公证）");
            mac.notarize = Some(false);
        } else if c.interactive {
            eprintln!();
            mac.notarize = Some(crate::prompt_yes_no(
                "签名完成。要提交 Apple 公证吗？（公证后别人下载打开时不会被系统拦截，需要几分钟）",
                true,
            ));
            c.changed = true;
        } else {
            dim_line("没有设置 macos.notarize，跳过公证");
            mac.notarize = Some(false);
        }
    }
    if format == "dmg" {
        let background = dmg_background(c, &mut mac)?;
        c.set_env("ZANT_DMG_BACKGROUND", &background);
    }
    if mac.notarize.unwrap_or(false) && !can_notarize {
        fail(format!(
            "只有 Developer ID Application 证书签名的应用能公证（现在是 {identity}）"
        ));
    }
    if mac.notarize.unwrap_or(false) {
        notary_env(c, &mut mac, &identity)?;
    }
    c.file.macos = Some(mac.clone());
    save(c);

    c.set_env("ZANT_FORMAT", &format);
    c.set_env(
        "ZANT_NOTARIZE",
        if mac.notarize.unwrap_or(false) {
            "1"
        } else {
            "0"
        },
    );
    if let Some(kc) = &keychain {
        c.set_env("ZANT_KEYCHAIN", kc);
    }
    c.run_script("developer-id");
    c.print_result(&format!(
        "{out}/{}-{}.{}",
        c.cfg.title, c.cfg.version, format
    ));
    Ok(())
}

/// The DMG background PNG for the script ("" = plain window); asks once.
fn dmg_background(c: &mut Ctx, mac: &mut Macos) -> std::io::Result<String> {
    if mac.dmg_background.is_none() {
        if c.interactive {
            eprintln!();
            loop {
                let value = crate::prompt(
                    "DMG 背景图片（PNG，建议 640×400，同目录可放 @2x 版本；回车用默认背景，none 不用背景）",
                    "默认",
                );
                if value == "默认" || value == "default" {
                    mac.dmg_background = Some("default".to_string());
                } else if value == "none" {
                    mac.dmg_background = Some("none".to_string());
                } else if file_exists(c, &expand_path(c, &value)) {
                    mac.dmg_background = Some(value);
                } else {
                    eprintln!("  找不到这个文件");
                    continue;
                }
                break;
            }
            c.changed = true;
        } else {
            mac.dmg_background = Some("default".to_string());
        }
    }
    let value = c.value(
        mac.dmg_background.as_deref().unwrap(),
        "macos.dmg_background",
    );
    if value == "none" {
        return Ok(String::new());
    }
    let path = if value == "default" {
        crate::zant_root()
            .join("tools/zant/package/dmg-background.png")
            .to_string_lossy()
            .into_owned()
    } else {
        expand_path(c, &value)
    };
    let data = crate::read_file(std::path::Path::new(&path))
        .unwrap_or_else(|_| fail(format!("找不到 DMG 背景图片 {path}")));
    let (width, height) = crate::png::info(&data)
        .unwrap_or_else(|_| fail(format!("{path} 不是可用的 PNG（需要 8 位、非隔行扫描）")));
    if !(320..=2000).contains(&width) || !(240..=1400).contains(&height) {
        fail(format!(
            "DMG 背景图片 {path} 是 {width}×{height}：它的尺寸就是窗口大小（点），建议 640×400，Retina 版本另存为 @2x"
        ));
    }
    Ok(path)
}

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

fn ask_identity(c: &Ctx, store: bool) -> std::io::Result<String> {
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
    eprintln!("\n签名证书（来自钥匙串）:");
    if !store {
        eprintln!("  0) Ad-hoc（不用证书，只能在本机测试，不能公证）");
    }
    for (index, id) in list.iter().enumerate() {
        eprintln!(
            "  {}) {id}{}",
            index + 1,
            if index < n_rec { "（推荐）" } else { "" }
        );
    }
    if n_rec == 0 {
        eprintln!(
            "  {COLOR_DIM}钥匙串里没有「{kind}」证书：在 Xcode → 设置 → 帐户 → 管理证书 里添加，或从 developer.apple.com 下载后双击安装{COLOR_RESET}"
        );
        if store && list.is_empty() {
            fail("没有可用的签名证书");
        }
    }
    let default = if n_rec > 0 || store { "1" } else { "0" };
    loop {
        let value = crate::prompt("选择", default);
        let Ok(number) = value.parse::<usize>() else {
            continue;
        };
        if number == 0 && !store {
            return Ok("none".to_string());
        }
        if (1..=list.len()).contains(&number) {
            if number > n_rec {
                eprintln!(
                    "  {COLOR_DIM}提示：这个证书不是「{kind}」，{}{COLOR_RESET}",
                    if store {
                        "App Store 不会接受"
                    } else {
                        "签名的应用不能公证，别人的 Mac 会拦截"
                    }
                );
            }
            return Ok(list[number - 1].clone());
        }
    }
}

fn first_identity(c: &Ctx, keychain: &str, store: bool) -> std::io::Result<String> {
    let all = identities(c, Some(keychain), false);
    for id in &all {
        if recommended(store, id) {
            return Ok(id.clone());
        }
    }
    if let Some(first) = all.first() {
        return Ok(first.clone());
    }
    fail("导入的证书里没有可用的签名身份")
}

fn first_installer(c: &Ctx, keychain: &str) -> std::io::Result<String> {
    let all = identities(c, Some(keychain), true);
    if let Some(first) = all.first() {
        return Ok(first.clone());
    }
    fail(
        "导入的证书里没有安装包证书（3rd Party Mac Developer Installer / Mac Installer Distribution）：macos.installer_identity 要另外设置",
    )
}

fn check_identity(c: &Ctx, identity: &str, key: &str, installer: bool) -> std::io::Result<()> {
    let all = identities(c, None, installer);
    if all.iter().any(|id| id == identity || id.contains(identity)) {
        return Ok(());
    }
    eprintln!(
        "\n  钥匙串里可用的{}证书：",
        if installer { "安装包" } else { "签名" }
    );
    for id in &all {
        eprintln!("    {id}");
    }
    fail(format!("{key}：钥匙串里没有可用的证书「{identity}」"))
}

fn ask_installer(c: &Ctx) -> std::io::Result<String> {
    let all = identities(c, None, true);
    if all.is_empty() {
        fail(
            "钥匙串里没有安装包证书：在 Xcode → 设置 → 帐户 → 管理证书 里添加「Mac Installer Distribution」",
        );
    }
    let options: Vec<&str> = all.iter().map(String::as_str).collect();
    let index = c.choose("安装包证书（签 .pkg）", &options, 0);
    Ok(all[index].clone())
}

#[derive(Debug, Default)]
struct ProfileInfo {
    name: Option<String>,
    team: Option<String>,
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

fn profile_info(c: &Ctx, path: &str) -> std::io::Result<ProfileInfo> {
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
fn ask_profile(c: &mut Ctx) -> std::io::Result<String> {
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
    labels.push("输入 .provisionprofile 文件路径".to_string());
    eprintln!(
        "\n  {COLOR_DIM}描述文件：developer.apple.com → Certificates, IDs & Profiles → Profiles 新建「Mac App Store Connect」类型，选 {}，下载{COLOR_RESET}",
        c.cfg.bundle_id
    );
    let options: Vec<&str> = labels.iter().map(String::as_str).collect();
    let index = c.choose("Mac App Store 描述文件", &options, 0);
    if index < found.len() {
        return Ok(found[index].clone());
    }
    loop {
        let path = c.ask_required(".provisionprofile 文件路径", "");
        if file_exists(c, &expand_path(c, &path)) {
            return Ok(path);
        }
        eprintln!("  找不到这个文件");
    }
}

/// Notarization credentials as ZANT_NOTARY_* variables for the script.
fn notary_env(c: &mut Ctx, mac: &mut Macos, identity: &str) -> std::io::Result<()> {
    if mac.notary.is_none() {
        if !c.interactive {
            c.need(
                "macos.notary",
                "公证凭据：{\"profile\": \"钥匙串配置名\"}、{\"api_key\": true} 或 {\"apple_id\", \"password\", \"team_id\"}",
            );
        }
        mac.notary = Some(ask_notary(c, identity)?);
        c.changed = true;
    }
    let notary = mac.notary.clone().unwrap_or_default();
    if let Some(profile) = &notary.profile {
        c.set_env(
            "ZANT_NOTARY_PROFILE",
            &c.value(profile, "macos.notary.profile"),
        );
        return Ok(());
    }
    if notary.api_key.unwrap_or(false) {
        let key = asc_key(c)?;
        c.set_env("ZANT_NOTARY_KEY", &key.path);
        c.set_env("ZANT_NOTARY_KEY_ID", &key.id);
        c.set_env("ZANT_NOTARY_ISSUER", &key.issuer);
        return Ok(());
    }
    if let Some(apple_id) = &notary.apple_id {
        let password = match &notary.password {
            Some(password) => password.clone(),
            None => c.need("macos.notary.password", "App 专用密码，写成 $环境变量名"),
        };
        c.set_env(
            "ZANT_NOTARY_APPLE_ID",
            &c.value(apple_id, "macos.notary.apple_id"),
        );
        let secret = c.secret(&password, "macos.notary.password", "App 专用密码");
        c.set_env("ZANT_NOTARY_PASSWORD", &secret);
        let team = match &notary.team_id {
            Some(team) => c.value(team, "macos.notary.team_id"),
            None => match team_of(identity) {
                Some(team) => team.to_string(),
                None => c.need("macos.notary.team_id", "团队 ID"),
            },
        };
        c.set_env("ZANT_NOTARY_TEAM", &team);
        return Ok(());
    }
    fail("macos.notary 要设置 profile、api_key 或 apple_id 其中一种")
}

fn ask_notary(c: &mut Ctx, identity: &str) -> std::io::Result<Notary> {
    let index = c.choose(
        "公证用的苹果帐户",
        &[
            "已经用 notarytool 存在钥匙串里的配置（输入名称）",
            "Apple ID + App 专用密码（notarytool 验证后存进钥匙串，不写进配置文件）",
            "App Store Connect API key（Issuer ID、Key ID、.p8 文件）",
        ],
        1,
    );
    match index {
        0 => Ok(Notary {
            profile: Some(c.ask_required("钥匙串配置名称", "zant-notary")),
            ..Notary::default()
        }),
        1 => {
            let apple_id = c.ask_required("Apple ID（邮箱）", "");
            let team = c.ask_required("团队 ID", team_of(identity).unwrap_or(""));
            let name = c.ask_required("存进钥匙串的配置名称", "zant-notary");
            eprintln!(
                "  {COLOR_DIM}App 专用密码在 account.apple.com → 登录与安全 → App 专用密码 生成；下面由 notarytool 询问，zant 看不到它{COLOR_RESET}"
            );
            run_inherit(
                &[
                    "xcrun".to_string(),
                    "notarytool".to_string(),
                    "store-credentials".to_string(),
                    name.clone(),
                    "--apple-id".to_string(),
                    apple_id,
                    "--team-id".to_string(),
                    team,
                ],
                &c.root.clone(),
            );
            Ok(Notary {
                profile: Some(name),
                ..Notary::default()
            })
        }
        _ => {
            let _ = asc_key(c)?;
            Ok(Notary {
                api_key: Some(true),
                ..Notary::default()
            })
        }
    }
}

/// The App Store Connect API key, asked once (Issuer ID, Key ID, .p8 path).
fn asc_key(c: &mut Ctx) -> std::io::Result<Asc> {
    if let Some(key) = &c.asc {
        return Ok(key.clone());
    }
    let mut key = c.file.app_store_connect.take().unwrap_or_default();
    if key.issuer_id.is_none() || key.key_id.is_none() || key.key.is_none() {
        if !c.interactive {
            c.need(
                "app_store_connect",
                "issuer_id、key_id、key（.p8 文件路径或内容，写成 $环境变量名）",
            );
        }
        eprintln!(
            "\nApp Store Connect API key（App Store Connect → 用户和访问 → 集成 → App Store Connect API，权限「App 管理」）"
        );
        if key.issuer_id.is_none() {
            key.issuer_id = Some(c.ask_required("Issuer ID", ""));
        }
        if key.key_id.is_none() {
            key.key_id = Some(c.ask_required("Key ID", ""));
        }
        if key.key.is_none() {
            let home = c.get_env("HOME").unwrap_or_default();
            let key_id = key.key_id.clone().unwrap_or_default();
            let mut default = String::new();
            for sub in [
                ".appstoreconnect/private_keys",
                "private_keys",
                ".private_keys",
            ] {
                let path = format!("{home}/{sub}/AuthKey_{key_id}.p8");
                if file_exists(c, &path) {
                    default = format!("~/{sub}/AuthKey_{key_id}.p8");
                }
            }
            loop {
                let path = c.ask_required(".p8 文件路径", &default);
                let full = expand_path(c, &path);
                if !file_exists(c, &full) {
                    eprintln!("  找不到这个文件");
                    continue;
                }
                if in_project(c, &full) {
                    eprintln!(
                        "  {COLOR_RED}注意{COLOR_RESET}：这个 .p8 在项目目录里，千万不要提交到仓库（建议移到 ~/.appstoreconnect/private_keys/）"
                    );
                }
                key.key = Some(path);
                break;
            }
        }
        c.changed = true;
    }
    let issuer = c.value(
        key.issuer_id.as_deref().unwrap(),
        "app_store_connect.issuer_id",
    );
    let id = c.value(key.key_id.as_deref().unwrap(), "app_store_connect.key_id");
    let source = c.value(key.key.as_deref().unwrap(), "app_store_connect.key");
    let name = format!("AuthKey_{id}.p8");
    let path = if source.contains("-----BEGIN") {
        write_secret_file(c, &name, source.as_bytes())?
    } else {
        file_or_base64(c, &source, "app_store_connect.key", &name)?
    };
    let asc = Asc { issuer, id, path };
    c.asc = Some(asc.clone());
    c.file.app_store_connect = Some(key);
    Ok(asc)
}
