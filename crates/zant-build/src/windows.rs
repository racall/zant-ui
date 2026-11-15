//! Windows build-script extras: the executable's `.ico` + manifest resources,
//! the GUI subsystem for release builds and `pdfium.dll` next to the
//! executable.
//!
//! Everything here runs on the *host* (build scripts are host programs), so
//! this module is only built when the app is built on Windows.

use std::path::{Path, PathBuf};

use crate::Error;

/// Embeds the resources, sets the subsystem and installs `pdfium.dll`.
pub fn apply(app_root: &Path, out_dir: &Path) -> Result<(), Error> {
    let resources = write_resources(app_root, out_dir)?;
    if let Some((rc, include)) = resources {
        // `.manifest_required()`: a missing resource compiler is a fallback
        // (warning), a failed compile is a warning too — the app is still
        // usable, only its icon / DPI manifest are missing.
        if let Err(error) =
            embed_resource::compile(&rc, embed_resource::ParamsIncludeDirs([include.as_path()]))
                .manifest_required()
        {
            println!(
                "cargo:warning=zant-build: could not embed the Windows resources: {error}; \
                 the executable has no .ico / manifest"
            );
        }
    }
    print_subsystem();
    install_pdfium(out_dir)?;
    Ok(())
}

/// Writes `app.ico` / `zant.manifest` / `app.rc` into `out_dir`; `None` when
/// the app has neither an icon nor a manifest.
fn write_resources(app_root: &Path, out_dir: &Path) -> Result<Option<(PathBuf, PathBuf)>, Error> {
    std::fs::create_dir_all(out_dir)?;
    let mut rc = String::new();
    if let Some(ico) = crate::windows_icon(app_root)? {
        crate::write_file(&out_dir.join("app.ico"), &ico)?;
        rc.push_str("1 ICON \"app.ico\"\n");
    } else {
        println!(
            "cargo:warning=zant-build: {} is missing; the executable has no Windows icon",
            app_root.join("assets/icons/app-windows.png").display()
        );
    }
    // The manifest: per-monitor DPI v2, UTF-8 code page, Common Controls v6.
    // Copied next to the .rc so `rc.exe` finds it in the include directory.
    let manifest =
        crate::zant_ui_root().join("crates/zant-sys/native/platform/windows/zant.manifest");
    match std::fs::read(&manifest) {
        Ok(data) => {
            crate::write_file(&out_dir.join("zant.manifest"), &data)?;
            rc.push_str("1 24 \"zant.manifest\"\n");
            println!("cargo:rerun-if-changed={}", manifest.display());
        }
        Err(error) => println!(
            "cargo:warning=zant-build: {} is missing ({error}); the app builds without the \
             DPI / Common Controls manifest",
            manifest.display()
        ),
    }
    // Properties › Details: description, company, version, copyright
    let config = std::fs::read_to_string(app_root.join("app.config.json"))
        .ok()
        .and_then(|text| crate::config::parse(&text, crate::config::Os::Windows).ok());
    if let Some(cfg) = config {
        let exe = format!(
            "{}.exe",
            std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "app".into())
        );
        rc.push_str(&crate::version_info::rc_block(&cfg, &exe));
    }
    if rc.is_empty() {
        return Ok(None);
    }
    let rc_path = out_dir.join("app.rc");
    crate::write_file(&rc_path, rc.as_bytes())?;
    Ok(Some((rc_path, out_dir.to_path_buf())))
}

/// Release builds are GUI programs (`windows_subsystem`).
/// The Rust entry point is `fn main`, so `mainCRTStartup` is the linker entry
/// for `/SUBSYSTEM:WINDOWS`.
fn print_subsystem() {
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile != "debug" {
        println!("cargo:rustc-link-arg-bins=/SUBSYSTEM:WINDOWS");
        println!("cargo:rustc-link-arg-bins=/ENTRY:mainCRTStartup");
    }
}

/// Copies `third_party/pdfium-win/bin/pdfium.dll` next to the executable (the
/// runtime loads it for `PdfPreview`; `zant package` only copies it when it is
/// already there).
///
/// A missing DLL fails release builds loudly: those are the ones that get
/// packaged, and a package without `pdfium.dll` has a silently broken
/// `PdfPreview`. Debug builds (`zant run`) only warn and keep every non-PDF
/// feature working.
fn install_pdfium(out_dir: &Path) -> Result<(), Error> {
    let source = crate::zant_ui_root().join("third_party/pdfium-win/bin/pdfium.dll");
    let release = std::env::var("PROFILE").is_ok_and(|profile| profile != "debug");
    let Some(profile_dir) = profile_dir(out_dir) else {
        let message = format!(
            "cannot locate the cargo profile directory from {}; pdfium.dll was not installed",
            out_dir.display()
        );
        return fallback(release, message);
    };
    if !source.is_file() {
        return fallback(
            release,
            format!(
                "{} is missing; run scripts/fetch_skia_windows.ps1 — without it the packaged \
                 app cannot open PDF previews",
                source.display()
            ),
        );
    }
    let destination = profile_dir.join("pdfium.dll");
    // Several build scripts of one workspace (every example app) install the
    // same DLL into the same profile directory in parallel, and a running
    // preview may still map it. Bytes already identical => nothing to do;
    // otherwise retry, since the failure is transient (another build script is
    // writing the file right now, or Defender holds it for a moment).
    let mut last_error = None;
    for _ in 0..20 {
        if same_contents(&source, &destination) {
            println!("cargo:rerun-if-changed={}", source.display());
            return Ok(());
        }
        match std::fs::copy(&source, &destination) {
            Ok(_) => {
                println!("cargo:rerun-if-changed={}", source.display());
                return Ok(());
            }
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
    return fallback(
        release,
        format!(
            "could not copy {} next to the executable ({}); quit the running app and retry, \
             or the packaged app cannot open PDF previews",
            source.display(),
            last_error
                .map(|error| error.to_string())
                .unwrap_or_default()
        ),
    );
}

/// Same length and bytes (used to skip a `pdfium.dll` copy that another build
/// script already did).
fn same_contents(source: &Path, destination: &Path) -> bool {
    let Ok(source_meta) = std::fs::metadata(source) else {
        return false;
    };
    let Ok(destination_meta) = std::fs::metadata(destination) else {
        return false;
    };
    if source_meta.len() != destination_meta.len() {
        return false;
    }
    match (std::fs::read(source), std::fs::read(destination)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// A release build fails, a debug build prints a `cargo:warning` and goes on.
fn fallback(release: bool, message: String) -> Result<(), Error> {
    if release {
        return Err(Error::Message(format!("pdfium.dll: {message}")));
    }
    println!("cargo:warning=zant-build: pdfium.dll: {message}");
    Ok(())
}

/// `$OUT_DIR` = `target/<profile>/build/<package>-<hash>/out`; the running
/// executable is in `target/<profile>/`.
fn profile_dir(out_dir: &Path) -> Option<PathBuf> {
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}
