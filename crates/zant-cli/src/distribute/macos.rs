//! The macOS half of the release build: `zant build` on a
//! Mac (Developer ID / DMG / notarization, or Mac App Store).

use super::{Ctx, Macos, expand_path, file_exists, file_or_base64, save, team_of};
use crate::bundle::BundleOptions;
use crate::generate::{self, PlistExtras};
use crate::{dim_line, fail, ok, run_inherit};

mod notary;
mod signing;
use notary::{asc_key, notary_env};
use signing::{
    ask_identity, ask_installer, ask_profile, check_identity, first_identity, first_installer,
    profile_info,
};

pub fn mac_flow(c: &mut Ctx) -> std::io::Result<()> {
    let mut mac = c.file.macos.take().unwrap_or_default();
    if mac.distribution.is_none() {
        if c.interactive {
            let index = c.choose(
                tr!(en: "Distribution method (macOS)", zh: "发布方式（macOS）"),
                &[
                    tr!(en: "Direct distribution: Developer ID signing → DMG → Apple notarization (recommended)", zh: "自己分发：Developer ID 签名 → DMG → Apple 公证（推荐）"),
                    tr!(en: "Mac App Store: Apple Distribution signing → .pkg → App Store Connect submission", zh: "Mac App Store：Apple Distribution 签名 → .pkg → 提交 App Store Connect"),
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
        fail(
            tr!(en: "macos.distribution must be developer-id or app-store (got {dist})", zh: "macos.distribution 只能是 developer-id 或 app-store（现在是 {dist}）",
         dist = dist),
        );
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
            Some(password) => c.secret(
                password,
                "macos.certificate_password",
                tr!(en: "Certificate password", zh: "证书密码"),
            ),
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
                tr!(en: "Signing certificate, e.g. Developer ID Application: Name (TEAMID)", zh: "签名证书，如 Developer ID Application: 名字 (TEAMID)"),
            );
        }
    }
    let identity = c.value(
        mac.signing_identity.as_deref().unwrap(),
        "macos.signing_identity",
    );
    let adhoc = identity == "none";
    if store && adhoc {
        fail(
            tr!(en: "Mac App Store requires an Apple Distribution certificate; Ad-hoc signing is not supported", zh: "Mac App Store 需要 Apple Distribution 证书，不能用 Ad-hoc 签名"),
        );
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
                    tr!(en: "App Store category, e.g. public.app-category.utilities", zh: "App Store 分类，如 public.app-category.utilities"),
                );
            }
            eprintln!(
                "{}",
                tr!(en: "\nApp Store category (LSApplicationCategoryType): utilities, productivity, developer-tools, graphics-design", zh: "\nApp Store 分类（LSApplicationCategoryType），常用：utilities 工具、productivity 效率、developer-tools 开发者工具、graphics-design 图形和设计",
                )
            );
            let value = crate::prompt(
                tr!(en: "Category", zh: "分类"),
                "public.app-category.utilities",
            );
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
                    tr!(en: "Path to Mac App Store provisioning profile (.provisionprofile)", zh: "Mac App Store 描述文件（.provisionprofile）的路径"),
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
            fail(
                tr!(en: "Provisioning profile not found: {profile_path}", zh: "找不到描述文件 {profile_path}", profile_path = profile_path),
            );
        }
        profile = Some(profile_path);
        let info = profile_info(c, profile.as_deref().unwrap())?;
        team = match info.team {
            Some(team) => Some(team),
            None => match team_of(&identity) {
                Some(team) => Some(team.to_string()),
                None => fail(
                    tr!(en: "Could not determine team ID from provisioning profile or certificate", zh: "无法从描述文件或证书得到团队 ID"),
                ),
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
                    tr!(en: "Installer certificate, e.g. 3rd Party Mac Developer Installer: Name (TEAMID)", zh: "安装包证书，如 3rd Party Mac Developer Installer: 名字 (TEAMID)"),
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
                    tr!(en: "Submit to App Store Connect after packaging?", zh: "打包后提交到 App Store Connect？"),
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
                tr!(en: "Mac App Store requires App Sandbox; enabling it for this build. Set macos.sandbox: true in app.config.json and run zant run to check sandbox functionality before submission", zh: "Mac App Store 要求 App Sandbox：这次自动开启。上架前在 app.config.json 设 macos.sandbox: true，用 zant run 检查沙盒里各功能是否正常"),
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
        ok(tr!(en: "Signature valid", zh: "签名有效"));
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
        fail(
            tr!(en: "macos.format must be dmg or zip (got {format})", zh: "macos.format 只能是 dmg 或 zip（现在是 {format}）", format = format),
        );
    }
    let can_notarize = identity.starts_with("Developer ID Application");
    if mac.notarize.is_none() {
        if !can_notarize {
            dim_line(
                tr!(en: "Skipping notarization: requires a Developer ID Application certificate", zh: "不是 Developer ID Application 证书，跳过公证（只有它签名的应用能公证）"),
            );
            mac.notarize = Some(false);
        } else if c.interactive {
            eprintln!();
            mac.notarize = Some(crate::prompt_yes_no(
                tr!(en: "Signing complete. Submit for Apple notarization? (Prevents download warnings; takes a few minutes)", zh: "签名完成。要提交 Apple 公证吗？（公证后别人下载打开时不会被系统拦截，需要几分钟）"),
                true,
            ));
            c.changed = true;
        } else {
            dim_line(
                tr!(en: "macos.notarize is not set; skipping notarization", zh: "没有设置 macos.notarize，跳过公证"),
            );
            mac.notarize = Some(false);
        }
    }
    if format == "dmg" {
        let background = dmg_background(c, &mut mac)?;
        c.set_env("ZANT_DMG_BACKGROUND", &background);
    }
    if mac.notarize.unwrap_or(false) && !can_notarize {
        fail(
            tr!(en: "Notarization requires a Developer ID Application certificate (got {identity})", zh: "只有 Developer ID Application 证书签名的应用能公证（现在是 {identity}）",
         identity = identity),
        );
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
                    tr!(en: "DMG background (PNG, recommended 640×400; optional @2x version alongside it; Enter for default, none for no background)", zh: "DMG 背景图片（PNG，建议 640×400，同目录可放 @2x 版本；回车用默认背景，none 不用背景）"),
                    tr!(en: "default", zh: "默认"),
                );
                if value == "默认" || value == "default" {
                    mac.dmg_background = Some("default".to_string());
                } else if value == "none" {
                    mac.dmg_background = Some("none".to_string());
                } else if file_exists(c, &expand_path(c, &value)) {
                    mac.dmg_background = Some(value);
                } else {
                    eprintln!("{}", tr!(en: "  File not found", zh: "  找不到这个文件",));
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
            .join("scripts/package/dmg-background.png")
            .to_string_lossy()
            .into_owned()
    } else {
        expand_path(c, &value)
    };
    let data = crate::read_file(std::path::Path::new(&path))
        .unwrap_or_else(|_| fail(tr!(en: "DMG background not found: {path}", zh: "找不到 DMG 背景图片 {path}", path = path)));
    let (width, height) = crate::png::info(&data)
        .unwrap_or_else(|_| fail(tr!(en: "{path} is not a supported PNG (expected 8-bit, non-interlaced)", zh: "{path} 不是可用的 PNG（需要 8 位、非隔行扫描）", path = path)));
    if !(320..=2000).contains(&width) || !(240..=1400).contains(&height) {
        fail(
            tr!(en: "DMG background {path} is {width}×{height}: image size sets the window size in points; recommended 640×400, with a separate @2x Retina version", zh: "DMG 背景图片 {path} 是 {width}×{height}：它的尺寸就是窗口大小（点），建议 640×400，Retina 版本另存为 @2x",
         height = height, path = path, width = width),
        );
    }
    Ok(path)
}
