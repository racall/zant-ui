//! Port of the packaging half of `tools/zant/main.zig`: compile, the macOS
//! `.app` bundle (icon, Info.plist, codesign), the Windows program folder and
//! the entity the package scripts receive. Artifacts come from
//! `target/{debug,release}/<name>` and outputs go under `target/` (Zig:
//! `zig-out/`).

use crate::generate::{self, PlistExtras};
use crate::png;
use crate::{IS_WINDOWS, app_exe_path, fail, ok, run_inherit, step};
use std::path::{Path, PathBuf};

pub fn compile_app(root: &str, release: bool) {
    if let Err(err) = crate::ensure_rust_project(root) {
        fail(format!("无法编译：{err}"));
    }
    let args = crate::build_args(release);
    step(if release {
        "正在编译 ReleaseFast"
    } else {
        "正在编译"
    });
    run_inherit(&args, root);
    ok("编译完成");
}

/// `zant package`: build and make the local-test artifact. Returns its path.
pub fn package_app(root: &str, cfg: &crate::app_config::AppConfig, release: bool) -> String {
    compile_app(root, release);
    if IS_WINDOWS {
        return package_windows(root, cfg).unwrap_or_else(|err| {
            fail(format!("生成程序目录失败：{err}"));
        });
    }
    let exe = app_exe_path(root, release);
    let ent = if cfg.sandbox || cfg.entitlements.is_some() {
        Some(format!("{root}/app.entitlements"))
    } else {
        None
    };
    build_mac_bundle(
        root,
        cfg,
        &exe,
        &BundleOptions {
            identity: cfg.signing_identity.clone(),
            entitlements: ent,
            ..BundleOptions::default()
        },
    )
}

#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// codesign identity; "none" = ad-hoc
    pub identity: String,
    pub entitlements: Option<String>,
    /// Contents/embedded.provisionprofile (Mac App Store)
    pub provisioning_profile: Option<String>,
    /// a keychain to find the identity in (`zant build` with a CI certificate)
    pub keychain: Option<String>,
    pub plist: PlistExtras,
}

impl Default for BundleOptions {
    fn default() -> Self {
        BundleOptions {
            identity: "none".to_string(),
            entitlements: None,
            provisioning_profile: None,
            keychain: None,
            plist: PlistExtras::default(),
        }
    }
}

/// `target/<title>.app` from the compiled executable: icon, Info.plist,
/// signature (hardened runtime and a secure timestamp with a real identity).
pub fn build_mac_bundle(
    root: &str,
    cfg: &crate::app_config::AppConfig,
    exe: &Path,
    opts: &BundleOptions,
) -> String {
    let app_name = &cfg.title;
    if app_name.contains('/') {
        fail("应用标题不能包含 '/'");
    }
    let app_path = format!("{root}/target/{app_name}.app");
    let _ = std::fs::remove_dir_all(&app_path);
    let macos_path = format!("{app_path}/Contents/MacOS");
    std::fs::create_dir_all(&macos_path).unwrap_or_else(|err| {
        fail(format!("无法创建 {macos_path}：{err}"));
    });

    let dst_exe = format!("{macos_path}/app");
    std::fs::copy(exe, &dst_exe).unwrap_or_else(|err| {
        fail(format!(
            "无法复制 {} 到 {dst_exe}：{err}",
            exe.to_string_lossy()
        ));
    });

    let mut extras = opts.plist.clone();
    extras.icon = write_mac_icon(root, &app_path).unwrap_or_else(|err| {
        fail(format!("生成应用图标失败：{err}"));
    });
    let plist = generate::generate_info_plist_for(cfg, &extras);
    let plist_path = format!("{app_path}/Contents/Info.plist");
    crate::write_file(Path::new(&plist_path), plist.as_bytes()).unwrap_or_else(|err| {
        fail(format!("无法写入 {plist_path}：{err}"));
    });
    if let Some(profile) = &opts.provisioning_profile {
        let dst = format!("{app_path}/Contents/embedded.provisionprofile");
        std::fs::copy(profile, &dst).unwrap_or_else(|err| {
            fail(format!("无法复制描述文件 {profile}：{err}"));
        });
    }

    let identity = if opts.identity.is_empty() {
        "none"
    } else {
        &opts.identity
    };
    let mut sign_argv: Vec<String> = vec![
        "codesign".to_string(),
        "--force".to_string(),
        "--deep".to_string(),
    ];
    if let Some(ent) = &opts.entitlements {
        sign_argv.push("--entitlements".to_string());
        sign_argv.push(ent.clone());
    }
    if let Some(kc) = &opts.keychain {
        sign_argv.push("--keychain".to_string());
        sign_argv.push(kc.clone());
    }
    if identity == "none" {
        sign_argv.push("--sign".to_string());
        sign_argv.push("-".to_string());
    } else {
        sign_argv.push("--options".to_string());
        sign_argv.push("runtime".to_string());
        sign_argv.push("--timestamp".to_string());
        sign_argv.push("--sign".to_string());
        sign_argv.push(identity.to_string());
    }
    sign_argv.push(app_path.clone());
    step(&format!(
        "正在签名（{}）",
        if identity == "none" {
            "Ad-hoc"
        } else {
            identity
        }
    ));
    run_inherit(&sign_argv, root);
    ok(&format!("已生成 {app_path}"));
    app_path
}

/// `Contents/Resources/AppIcon.icns` from assets/icons/app-macos.png
/// (1024×1024, Apple icon grid): the ten iconset sizes are resized here,
/// iconutil packs them. Returns false when the project has no icon.
fn write_mac_icon(root: &str, app_path: &str) -> std::io::Result<bool> {
    let source = Path::new(root).join("assets/icons/app-macos.png");
    let data = match crate::read_file(&source) {
        Ok(data) => data,
        Err(_) => return Ok(false),
    };
    let img = png::decode(&data).unwrap_or_else(|err| {
        fail(format!(
            "{} 无法读取（{err}）：需要 8 位、非隔行扫描的 PNG",
            source.to_string_lossy()
        ));
    });
    if img.width != 1024 || img.height != 1024 {
        fail(format!(
            "{} 是 {}×{}；macOS 应用图标需要 1024×1024（苹果图标网格：主体 824×824 居中）",
            source.to_string_lossy(),
            img.width,
            img.height
        ));
    }
    step("生成应用图标 AppIcon.icns");
    let iconset = PathBuf::from(root).join("target/AppIcon.iconset");
    let _ = std::fs::remove_dir_all(&iconset);
    std::fs::create_dir_all(&iconset)?;
    for size in [16u32, 32, 128, 256, 512] {
        for scale in [1u32, 2] {
            let px = size * scale;
            let resized = if px == img.width {
                img.clone()
            } else {
                png::resize(&img, px, px)
            };
            let name = if scale == 1 {
                format!("icon_{size}x{size}.png")
            } else {
                format!("icon_{size}x{size}@2x.png")
            };
            let encoded = png::encode(&resized).unwrap_or_else(|err| {
                fail(format!("无法编码图标：{err}"));
            });
            crate::write_file(&iconset.join(name), &encoded)?;
        }
    }
    let resources = Path::new(app_path).join("Contents/Resources");
    std::fs::create_dir_all(&resources)?;
    let icns = resources.join("AppIcon.icns");
    run_inherit(
        &[
            "iconutil".to_string(),
            "-c".to_string(),
            "icns".to_string(),
            "-o".to_string(),
            icns.to_string_lossy().into_owned(),
            iconset.to_string_lossy().into_owned(),
        ],
        root,
    );
    let _ = std::fs::remove_dir_all(&iconset);
    ok("应用图标已生成");
    Ok(true)
}

/// Windows: a folder with the exe, pdfium.dll and the MSVC runtime, ready to
/// zip or run anywhere on Windows 10+. (The Rust build links the Skia bridge
/// statically, so `zant_skia.dll` is gone; see plan.md M6/WP-6.2.)
pub fn package_windows(root: &str, cfg: &crate::app_config::AppConfig) -> std::io::Result<String> {
    for bad in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
        if cfg.title.contains(bad) {
            fail(format!("应用标题不能包含 {bad}"));
        }
    }
    let out = PathBuf::from(root).join("target").join(&cfg.title);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out)?;
    let release_dir = app_exe_path(root, true)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(root).join("target/release"));
    let exe_name = format!("{}.exe", cfg.title);
    let files = [
        (
            release_dir.join(format!("{}.exe", crate::app_target(root).name)),
            exe_name,
        ),
        (release_dir.join("pdfium.dll"), "pdfium.dll".to_string()),
    ];
    for (src, dst) in files {
        let dst = out.join(dst);
        if let Err(err) = std::fs::copy(&src, &dst)
            && src.file_name().and_then(|n| n.to_str()) != Some("pdfium.dll")
        {
            return Err(err);
        }
    }
    // Ship the MSVC runtime the exe needs next to it (the copy installed with
    // a prebuilt bridge, else the system's).
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    for name in ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll"] {
        let local = release_dir.join(name);
        let src = if local.is_file() {
            local
        } else {
            Path::new(&system_root).join("System32").join(name)
        };
        let _ = std::fs::copy(src, out.join(name));
    }
    ok(&format!("已生成 {}", out.to_string_lossy()));
    Ok(out.to_string_lossy().into_owned())
}

/// The Windows app icon (`assets/icons/app-windows.png`) as a decoded image,
/// for the `.ico` / MSIX logos; `None` when the project has no icon.
pub fn app_icon(root: &str) -> std::io::Result<Option<png::Image>> {
    let source = Path::new(root).join("assets/icons/app-windows.png");
    let data = match crate::read_file(&source) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };
    let img = png::decode(&data).unwrap_or_else(|err| {
        fail(format!(
            "{} 无法读取（{err}）：需要 8 位、非隔行扫描的 PNG",
            source.to_string_lossy()
        ));
    });
    Ok(Some(img))
}
