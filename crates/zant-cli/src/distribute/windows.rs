//! The Windows half of the release build: `zant build` on
//! Windows (installer / MSIX / zip, optional signtool signing).

use super::{Ctx, Windows, expand_path, file_exists, file_or_base64, in_project, save};
use crate::COLOR_RED;
use crate::COLOR_RESET;
use crate::{dim_line, fail};
use crate::{png, write_file};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WinKind {
    Installer,
    Store,
    Folder,
}

impl WinKind {
    fn parse(value: &str) -> Option<WinKind> {
        match value {
            "installer" => Some(WinKind::Installer),
            "store" => Some(WinKind::Store),
            "folder" => Some(WinKind::Folder),
            _ => None,
        }
    }
}

pub fn windows_flow(c: &mut Ctx) -> std::io::Result<()> {
    let mut w = c.file.windows.take().unwrap_or_default();
    if w.distribution.is_none() {
        if c.interactive {
            let index = c.choose(
                tr!(en: "Distribution method (Windows)", zh: "发布方式（Windows）"),
                &[
                    tr!(en: "Installer: setup.exe, with optional signing (recommended)", zh: "安装包：setup.exe，可签名（推荐）"),
                    tr!(en: "Microsoft Store: MSIX package, with optional Partner Center submission", zh: "Microsoft Store：MSIX 包，可提交到 Partner Center"),
                    tr!(en: "Portable: application directory as zip", zh: "免安装：程序目录压缩成 zip"),
                ],
                0,
            );
            w.distribution = Some(
                match index {
                    0 => "installer",
                    1 => "store",
                    _ => "folder",
                }
                .to_string(),
            );
            c.changed = true;
        } else {
            w.distribution = Some("installer".to_string());
        }
    }
    let dist = c.value(w.distribution.as_deref().unwrap(), "windows.distribution");
    let kind = WinKind::parse(&dist).unwrap_or_else(|| {
        fail(tr!(en: "windows.distribution must be installer, store, or folder (got {dist})", zh: "windows.distribution 只能是 installer、store 或 folder（现在是 {dist}）",
         dist = dist))
    });

    let mut subject_cn: Option<String> = None;
    win_signing(c, &mut w, kind == WinKind::Store, &mut subject_cn)?;

    if kind == WinKind::Installer {
        if w.publisher.is_none() {
            if !c.interactive {
                c.need(
                    "windows.publisher",
                    tr!(en: "Publisher name shown in the installer and Settings → Apps", zh: "发布者名称，安装程序和「设置 → 应用」里显示"),
                );
            }
            eprintln!();
            w.publisher = Some(c.ask_required(
                tr!(en: "Publisher name (shown in the installer and Settings → Apps)", zh: "发布者名称（安装程序和「设置 → 应用」里显示）"),
                subject_cn.as_deref().unwrap_or(""),
            ));
            c.changed = true;
        }
        c.set_env(
            "ZANT_PUBLISHER",
            &c.value(w.publisher.as_deref().unwrap(), "windows.publisher"),
        );
        if c.capture_script("find-iscc").is_empty() {
            if !c.interactive {
                fail(
                    tr!(en: "Inno Setup (ISCC.exe) not found: run winget install JRSoftware.InnoSetup or set ZANT_ISCC", zh: "找不到 Inno Setup（ISCC.exe）：winget install JRSoftware.InnoSetup，或设置 ZANT_ISCC"),
                );
            }
            eprintln!();
            if !crate::prompt_yes_no(
                tr!(en: "Creating an installer requires Inno Setup (free). Install it with winget now?", zh: "制作安装包要用 Inno Setup（免费）。现在用 winget 安装吗？"),
                true,
            ) {
                fail(
                    tr!(en: "Cannot create an installer without Inno Setup", zh: "没有 Inno Setup，不能制作安装包"),
                );
            }
            c.run_script("install-iscc");
            if c.capture_script("find-iscc").is_empty() {
                fail(
                    tr!(en: "ISCC.exe still not found after installation: reopen the terminal or set ZANT_ISCC", zh: "安装后仍找不到 ISCC.exe：重新打开终端再试，或设置 ZANT_ISCC"),
                );
            }
        }
    }
    if kind == WinKind::Store {
        store_settings(c, &mut w, subject_cn.as_deref())?;
    }
    c.file.windows = Some(w.clone());
    save(c);

    eprintln!();
    crate::bundle::compile_app(&c.root, true);
    let folder = crate::bundle::package_windows(&c.root, &c.cfg)?;
    let out = format!("{}/target/dist", c.root);
    c.set_env("ZANT_KIND", &dist);
    let title = c.cfg.title.clone();
    let version = c.cfg.version.clone();
    let bundle_id = c.cfg.bundle_id.clone();
    c.set_env("ZANT_TITLE", &title);
    c.set_env("ZANT_VERSION", &version);
    c.set_env("ZANT_BUNDLE_ID", &bundle_id);
    c.set_env("ZANT_DIR", &folder);
    c.set_env("ZANT_OUT", &out);
    if kind == WinKind::Installer
        && let Some(img) = crate::bundle::app_icon(&c.root)?
    {
        let ico = Path::new(&c.root)
            .join("target/build/app.ico")
            .to_string_lossy()
            .into_owned();
        let bytes = png::build_ico(&img).unwrap_or_else(|err| {
            fail(tr!(en: "Could not generate app icon: {err}", zh: "无法生成应用图标：{err}", err = err));
        });
        write_file(Path::new(&ico), &bytes)?;
        c.set_env("ZANT_ICO", &ico);
    }
    if kind == WinKind::Store {
        msix_assets(c)?;
    }
    c.run_script("package");
    let artifact = match kind {
        WinKind::Installer => format!("{}\\{}-{}-setup.exe", out, c.cfg.title, c.cfg.version),
        WinKind::Store => format!("{}\\{}-{}.msix", out, c.cfg.title, c.cfg.version),
        WinKind::Folder => format!("{}\\{}-{}-windows.zip", out, c.cfg.title, c.cfg.version),
    };
    c.print_result(&artifact);
    Ok(())
}

pub fn is_thumbprint(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// "CN=Name, O=…" → "Name"
pub fn common_name(subject: &str) -> Option<String> {
    let at = subject.find("CN=")?;
    let rest = &subject[at + 3..];
    let end = rest.find(',').unwrap_or(rest.len());
    Some(rest[..end].trim_matches([' ', '"']).to_string())
}

/// Signing settings as ZANT_SIGN_* variables; asks when nothing is set.
fn win_signing(
    c: &mut Ctx,
    w: &mut Windows,
    store: bool,
    subject_cn: &mut Option<String>,
) -> std::io::Result<()> {
    if w.certificate.is_none() && w.signing_identity.is_none() {
        if !c.interactive {
            dim_line(
                tr!(en: "windows.signing_identity / certificate is not set; skipping signing", zh: "没有设置 windows.signing_identity / certificate：不签名"),
            );
            return Ok(());
        }
        let listing = c.capture_script("list-certs");
        let mut thumbs: Vec<String> = Vec::new();
        let mut subjects: Vec<String> = Vec::new();
        eprintln!(
            "{}",
            tr!(en: "\nCode signing certificates (Windows certificate store: Current User → Personal):", zh: "\n代码签名证书（Windows 证书存储：当前用户 → 个人）:",)
        );
        eprintln!(
            "{}",
            tr!(en: "  0) Do not sign{}", zh: "  0) 不签名{}",
                if store {
                    tr!(en: " (Microsoft signs store submissions; recommended)", zh: "（提交商店时由微软签名，推荐）")
                } else {
                    tr!(en: " (users will see an “Unknown publisher” warning)", zh: "（用户运行安装包时会看到「未知发布者」提示）")
                }
            )
        );
        for raw in listing.split('\n') {
            let line = raw.trim_matches([' ', '\r']);
            let mut parts = line.split('|');
            let Some(thumb) = parts.next() else {
                continue;
            };
            let Some(subject) = parts.next() else {
                continue;
            };
            let expiry = parts.next().unwrap_or("");
            if !is_thumbprint(thumb) {
                continue;
            }
            thumbs.push(thumb.to_string());
            subjects.push(subject.to_string());
            eprintln!(
                "{}",
                tr!(en: "  {}) {subject} (expires {expiry})", zh: "  {}) {subject}（到期 {expiry}）", thumbs.len(), expiry = expiry, subject = subject)
            );
        }
        eprintln!(
            "{}",
            tr!(en: "  f) Use a .pfx certificate file", zh: "  f) 使用 .pfx 证书文件",)
        );
        let default = if !thumbs.is_empty() && !store {
            "1"
        } else {
            "0"
        };
        loop {
            let value = crate::prompt(tr!(en: "Choose", zh: "选择"), default);
            if value == "0" {
                w.signing_identity = Some("none".to_string());
                break;
            }
            if value == "f" || value == "F" {
                loop {
                    let path = c.ask_required(tr!(en: ".pfx file path", zh: ".pfx 文件路径"), "");
                    let full = expand_path(c, &path);
                    if !file_exists(c, &full) {
                        eprintln!("{}", tr!(en: "  File not found", zh: "  找不到这个文件",));
                        continue;
                    }
                    if in_project(c, &full) {
                        eprintln!(
                            "{}",
                            tr!(en: "  {COLOR_RED}Warning{COLOR_RESET}: the certificate is inside the project; do not commit it", zh: "  {COLOR_RED}注意{COLOR_RESET}：证书在项目目录里，千万不要提交到仓库",
                         COLOR_RED = COLOR_RED, COLOR_RESET = COLOR_RESET)
                        );
                    }
                    w.certificate = Some(path);
                    break;
                }
                let name = c.ask_required(
                    tr!(en: "Environment variable for the certificate password (not saved in configuration; prompted when unset)", zh: "证书密码放在哪个环境变量（密码不写进配置；变量没设置时每次询问）"),
                    "ZANT_WIN_CERT_PASSWORD",
                );
                w.certificate_password = Some(format!("${name}"));
                break;
            }
            let Ok(number) = value.parse::<usize>() else {
                continue;
            };
            if (1..=thumbs.len()).contains(&number) {
                w.signing_identity = Some(thumbs[number - 1].clone());
                *subject_cn = common_name(&subjects[number - 1]);
                break;
            }
        }
        c.changed = true;
    }
    if let Some(cert) = w.certificate.clone() {
        let path = file_or_base64(
            c,
            &c.value(&cert, "windows.certificate"),
            "windows.certificate",
            "signing.pfx",
        )?;
        c.set_env("ZANT_SIGN_PFX", &path);
        if let Some(password) = &w.certificate_password {
            let secret = c.secret(
                password,
                "windows.certificate_password",
                tr!(en: "Certificate password", zh: "证书密码"),
            );
            c.set_env("ZANT_SIGN_PFX_PASSWORD", &secret);
        }
    } else {
        let id = c.value(
            w.signing_identity.as_deref().unwrap(),
            "windows.signing_identity",
        );
        if id == "none" {
            return Ok(());
        }
        c.set_env(
            if is_thumbprint(&id) {
                "ZANT_SIGN_THUMBPRINT"
            } else {
                "ZANT_SIGN_SUBJECT"
            },
            &id,
        );
    }
    let timestamp = match &w.timestamp_url {
        Some(url) => c.value(url, "windows.timestamp_url"),
        None => "http://timestamp.digicert.com".to_string(),
    };
    c.set_env("ZANT_SIGN_TIMESTAMP", &timestamp);
    Ok(())
}

fn store_settings(c: &mut Ctx, w: &mut Windows, subject_cn: Option<&str>) -> std::io::Result<()> {
    let mut s = w.store.take().unwrap_or_default();
    if s.identity_name.is_none() || s.publisher.is_none() || s.publisher_display_name.is_none() {
        if !c.interactive {
            c.need(
                "windows.store",
                tr!(en: "identity_name, publisher, publisher_display_name (Partner Center → Product management → Product identity)", zh: "identity_name、publisher、publisher_display_name（Partner Center → 产品管理 → 产品标识）"),
            );
        }
        eprintln!(
            "{}",
            tr!(en: "\nMicrosoft Store product identity (Partner Center → Apps → Product management → Product identity)", zh: "\nMicrosoft Store 产品标识（Partner Center → 应用 → 产品管理 → 产品标识）",)
        );
        if s.identity_name.is_none() {
            s.identity_name = Some(c.ask_required("Package/Identity/Name", ""));
        }
        if s.publisher.is_none() {
            s.publisher = Some(c.ask_required("Package/Identity/Publisher（CN=…）", ""));
        }
        if s.publisher_display_name.is_none() {
            s.publisher_display_name = Some(c.ask_required(
                "Package/Properties/PublisherDisplayName",
                subject_cn.unwrap_or(""),
            ));
        }
        c.changed = true;
    }
    if s.upload.is_none() {
        if c.interactive {
            eprintln!();
            s.upload = Some(crate::prompt_yes_no(
                tr!(en: "Submit to Partner Center using msstore after packaging? (No: upload .msix on the website yourself)", zh: "打包后用 msstore 命令行提交到 Partner Center？（否：自己在网页上传 .msix）"),
                false,
            ));
            c.changed = true;
        } else {
            s.upload = Some(false);
        }
    }
    if s.upload.unwrap_or(false) && s.product_id.is_none() {
        if !c.interactive {
            c.need(
                "windows.store.product_id",
                tr!(en: "Store ID, e.g. 9NBLGGH4R315", zh: "Store ID，如 9NBLGGH4R315"),
            );
        }
        s.product_id = Some(c.ask_required(tr!(en: "Store ID (Partner Center → Product identity, starts with 9)", zh: "Store ID（Partner Center → 产品标识，9 开头）"), ""));
        c.changed = true;
    }
    c.set_env(
        "ZANT_MSIX_NAME",
        &c.value(
            s.identity_name.as_deref().unwrap(),
            "windows.store.identity_name",
        ),
    );
    c.set_env(
        "ZANT_MSIX_PUBLISHER",
        &c.value(s.publisher.as_deref().unwrap(), "windows.store.publisher"),
    );
    c.set_env(
        "ZANT_MSIX_PUBLISHER_NAME",
        &c.value(
            s.publisher_display_name.as_deref().unwrap(),
            "windows.store.publisher_display_name",
        ),
    );
    let version = msix_version(&c.cfg.version);
    c.set_env("ZANT_MSIX_VERSION", &version);
    c.set_env(
        "ZANT_STORE_UPLOAD",
        if s.upload.unwrap_or(false) { "1" } else { "0" },
    );
    if s.upload.unwrap_or(false) {
        c.set_env(
            "ZANT_STORE_PRODUCT_ID",
            &c.value(s.product_id.as_deref().unwrap(), "windows.store.product_id"),
        );
        // Partner Center credentials (Azure AD app); without them msstore uses
        // the ones saved by an earlier `msstore reconfigure`
        if let Some(client_secret) = &s.client_secret {
            let secret = c.secret(
                client_secret,
                "windows.store.client_secret",
                tr!(en: "Client secret", zh: "客户端密钥"),
            );
            c.set_env("ZANT_STORE_CLIENT_SECRET", &secret);
            let tenant = match &s.tenant_id {
                Some(tenant) => c.value(tenant, "windows.store.tenant_id"),
                None => c.need(
                    "windows.store.tenant_id",
                    tr!(en: "Tenant ID", zh: "租户 ID"),
                ),
            };
            c.set_env("ZANT_STORE_TENANT_ID", &tenant);
            let seller = match &s.seller_id {
                Some(seller) => c.value(seller, "windows.store.seller_id"),
                None => c.need(
                    "windows.store.seller_id",
                    tr!(en: "Seller ID", zh: "卖家 ID"),
                ),
            };
            c.set_env("ZANT_STORE_SELLER_ID", &seller);
            let client = match &s.client_id {
                Some(client) => c.value(client, "windows.store.client_id"),
                None => c.need(
                    "windows.store.client_id",
                    tr!(en: "Client ID", zh: "客户端 ID"),
                ),
            };
            c.set_env("ZANT_STORE_CLIENT_ID", &client);
        }
    }
    w.store = Some(s);
    Ok(())
}

/// "1.2.3" → "1.2.3.0": MSIX wants four numbers, the Store a last one of 0.
pub fn msix_version(version: &str) -> String {
    let mut parts = [0u32; 3];
    for (index, part) in version.split('.').take(3).enumerate() {
        let digits: String = part.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        parts[index] = digits.parse::<u32>().unwrap_or_else(|_| {
            fail(tr!(en: "Version {version} cannot be used for MSIX: expected numeric components such as 1.2.3", zh: "版本号 {version} 不能用于 MSIX：要是 1.2.3 这样的数字",
             version = version))
        });
    }
    format!("{}.{}.{}.0", parts[0], parts[1], parts[2])
}

/// MSIX logos from assets/icons/app-windows.png.
fn msix_assets(c: &mut Ctx) -> std::io::Result<()> {
    let img = crate::bundle::app_icon(&c.root)?
        .unwrap_or_else(|| fail(tr!(en: "MSIX requires assets/icons/app-windows.png", zh: "MSIX 需要应用图标 assets/icons/app-windows.png")));
    let dir = Path::new(&c.root).join("target/build/msix-assets");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    for (name, size) in [
        ("StoreLogo.png", 50u32),
        ("Square44x44Logo.png", 44),
        ("Square150x150Logo.png", 150),
    ] {
        let resized = png::resize(&img, size, size);
        let encoded = png::encode(&resized).unwrap_or_else(|err| {
            fail(tr!(en: "Could not generate MSIX icons: {err}", zh: "无法生成 MSIX 图标：{err}", err = err));
        });
        write_file(&dir.join(name), &encoded)?;
    }
    c.set_env("ZANT_MSIX_ASSETS", &dir.to_string_lossy());
    Ok(())
}
