//! Compiles the native Skia / platform / PDF sources and links Skia.
//!
//! This mirrors the native parts of the Zig `build.zig` (macOS branch,
//! `build.zig:12-53`) and `scripts/win/build_bridge.ps1` (Windows branch).
//! The native sources stay in place (`src/renderer/skia/`,
//! `src/platform/`) during the port; M8 moves them under this crate.
//!
//! macOS  : C++ `skia_bridge.cpp`, ObjC++ `skia_bridge_mac.mm` + `pdf.mm`,
//!          ObjC `window.m`; static Skia archives in
//!          `third_party/skia/skia-binaries/`; 14 system frameworks + z + c++.
//! Windows: C++ `skia_bridge.cpp` + `skia_bridge_win.cpp` + `pdf_win.cpp`,
//!          C `window.c`; static Skia archives in
//!          `third_party/skia-win/skia-binaries/`; system DLL import libs.
//!          The DLL build (`zant_skia.dll`) is gone: the native code is linked
//!          into the executable, with ICU data and STIX Two Math embedded by
//!          `include_bytes!` (ZANT_STATIC_DATA, see skia_bridge_win.cpp).
//!
//! See docs/rust-port/plan.md WP-0.3.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest
        .join("../..")
        .canonicalize()
        .expect("repo root (crates/zant-sys/../..)");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/skia.rs");
    println!("cargo:rerun-if-changed=src/platform.rs");
    println!("cargo:rerun-if-changed=src/pdf.rs");

    match target_os.as_str() {
        "macos" => build_macos(&root),
        "windows" => build_windows(&root),
        other => {
            panic!("zant-sys: unsupported target OS {other}; Zant UI builds on macOS and Windows")
        }
    }
}

/// Include paths shared by both platforms (the Zig build passes the repo root
/// so native sources can include "src/platform/platform.h" style paths).
fn add_includes(build: &mut cc::Build, root: &Path) {
    build
        .include(root)
        .include(root.join("third_party/skia-headers"))
        .include(root.join("third_party/skia-headers/include/private"))
        .include(root.join("third_party/skia-headers/src"));
}

fn rerun_for(root: &Path, relative_paths: &[&str]) {
    for path in relative_paths {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

fn build_macos(root: &Path) {
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let sources = [
        "src/renderer/skia/skia_bridge.cpp",
        "src/renderer/skia/skia_bridge.h",
        "src/renderer/skia/skia_bridge_internal.h",
        "src/renderer/skia/skia_bridge_mac.mm",
        "src/platform/macos/window.m",
        "src/platform/macos/window.h",
        "src/platform/macos/pdf.mm",
        "src/platform/platform.h",
        "src/platform/pdf.h",
    ];
    rerun_for(root, &sources);

    // C++: the portable Skia bridge.
    let mut cpp = cc::Build::new();
    cpp.cpp(true)
        .file(root.join("src/renderer/skia/skia_bridge.cpp"))
        .flag("-std=c++20")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-Wno-deprecated-declarations");
    add_includes(&mut cpp, root);
    cpp.compile("zant_bridge_cpp");

    // Objective-C++: the macOS Skia/Metal bridge and the PDFKit backend.
    let mut objcpp = cc::Build::new();
    objcpp
        .file(root.join("src/renderer/skia/skia_bridge_mac.mm"))
        .file(root.join("src/platform/macos/pdf.mm"))
        .flag("-x")
        .flag("objective-c++")
        .flag("-std=c++20")
        .flag("-fobjc-arc")
        // skia_bridge_mac.mm is compiled without exceptions like the portable
        // part; pdf.mm keeps the compiler default (matches build.zig).
        .flag("-Wno-deprecated-declarations");
    add_includes(&mut objcpp, root);
    objcpp.compile("zant_bridge_objcpp");

    // Objective-C: the AppKit window backend.
    let mut objc = cc::Build::new();
    objc.file(root.join("src/platform/macos/window.m"))
        .flag("-x")
        .flag("objective-c")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .flag("-Wno-deprecated-declarations");
    add_includes(&mut objc, root);
    objc.compile("zant_platform_objc");

    // Static Skia (same archives and order as build.zig).
    let skia = root.join("third_party/skia/skia-binaries");
    assert!(
        skia.join("libskia.a").is_file(),
        "Skia for macOS is missing: {} — run ./scripts/fetch_skia.sh",
        skia.display()
    );
    println!("cargo:rustc-link-search=native={}", skia.display());
    for lib in [
        "skparagraph",
        "skshaper",
        "skunicode_icu",
        "skunicode_core",
        "skia",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    println!("cargo:rerun-if-changed={}", skia.display());

    for framework in [
        "AppKit",
        "Metal",
        "MetalKit",
        "QuartzCore",
        "CoreText",
        "CoreFoundation",
        "CoreGraphics",
        "IOKit",
        "Security",
        "CoreVideo",
        "UniformTypeIdentifiers",
        "PDFKit",
        "Carbon",
        "ServiceManagement",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    // zlib + the C++ runtime (no system sqlite3: rusqlite bundles its own).
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=c++");
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

fn build_windows(root: &Path) {
    let sources = [
        "src/renderer/skia/skia_bridge.cpp",
        "src/renderer/skia/skia_bridge.h",
        "src/renderer/skia/skia_bridge_internal.h",
        "src/renderer/skia/skia_bridge_win.cpp",
        "src/renderer/skia/zant_skia_resources.h",
        "src/platform/windows/pdf_win.cpp",
        "src/platform/windows/window.c",
        "src/platform/platform.h",
        "src/platform/pdf.h",
    ];
    rerun_for(root, &sources);

    // C++: portable bridge + Direct3D 12 backend + PDFium backend. Flags are
    // the ones scripts/win/build_bridge.ps1 passes to cl.exe, plus
    // ZANT_STATIC_DATA (no DLL resources: data is embedded in the exe).
    let mut cpp = cc::Build::new();
    cpp.file(root.join("src/renderer/skia/skia_bridge.cpp"))
        .file(root.join("src/renderer/skia/skia_bridge_win.cpp"))
        .file(root.join("src/platform/windows/pdf_win.cpp"))
        .std("c++20")
        .flag("/O2")
        .flag("/MD")
        .flag("/EHs-c-")
        .flag("/GR-")
        .flag("/utf-8")
        .flag("/permissive-")
        .flag("/Zc:__cplusplus")
        .flag("/W3")
        .flag("/wd4244")
        .flag("/wd4267")
        .flag("/wd4800")
        .flag("/wd5030")
        .define("NDEBUG", None)
        .define("SK_RELEASE", None)
        .define("SK_GANESH", None)
        .define("SK_DIRECT3D", None)
        .define("_HAS_EXCEPTIONS", "0")
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("NOMINMAX", None)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .define("ZANT_STATIC_DATA", None);
    add_includes(&mut cpp, root);
    cpp.compile("zant_bridge_cpp");

    // C: the Win32 window backend.
    let mut c = cc::Build::new();
    c.file(root.join("src/platform/windows/window.c"))
        .std("c11")
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("NOMINMAX", None);
    add_includes(&mut c, root);
    c.compile("zant_platform_c");

    // Static Skia for MSVC (same archives as build_bridge.ps1).
    let skia = root.join("third_party/skia-win/skia-binaries");
    assert!(
        skia.join("skia.lib").is_file(),
        "Skia for Windows is missing: {} — run scripts/fetch_skia_windows.ps1",
        skia.display()
    );
    println!("cargo:rustc-link-search=native={}", skia.display());
    for lib in [
        "skparagraph",
        "skshaper",
        "skunicode_icu",
        "skunicode_core",
        "skia",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    println!("cargo:rerun-if-changed={}", skia.display());

    for lib in [
        "d3d12",
        "dxgi",
        "d3dcompiler",
        "dwrite",
        "opengl32",
        "usp10",
        "fontsub",
        "ole32",
        "oleaut32",
        "user32",
        "gdi32",
        "imm32",
        "dwmapi",
        "shell32",
        "comctl32",
        "uuid",
        "kernel32",
        "advapi32",
        "windowscodecs",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }

    // ICU data and the KaTeX math font are embedded in the executable instead
    // of the DLL's RCDATA resources; `zant_sys::static_data::install()` hands
    // them to the bridge before zant_skia_init().
    let icu = skia.join("icudtl.dat");
    assert!(
        icu.is_file(),
        "ICU data is missing: {} — run scripts/fetch_skia_windows.ps1",
        icu.display()
    );
    let math_font = root.join("third_party/fonts/STIXTwoMath-Regular.otf");
    assert!(
        math_font.is_file(),
        "STIX Two Math is missing: {} — run scripts/fetch_skia_windows.ps1",
        math_font.display()
    );
    println!("cargo:rerun-if-changed={}", icu.display());
    println!("cargo:rerun-if-changed={}", math_font.display());
    write_static_data(&icu, &math_font);
}

/// Writes `$OUT_DIR/static_data.rs`: the embedded ICU / math-font bytes and a
/// `install()` that passes them to the Windows bridge.
fn write_static_data(icu: &Path, math_font: &Path) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let icu_len = std::fs::metadata(icu).expect("ICU data metadata").len();
    let math_len = std::fs::metadata(math_font)
        .expect("math font metadata")
        .len();
    let source = format!(
        r#"// Generated by crates/zant-sys/build.rs — do not edit.
//! ICU common data and STIX Two Math embedded in the executable (Windows).
//!
//! The Zig build puts both into `zant_skia.dll` as RCDATA resources (ids 101 /
//! 102). A statically linked Rust executable has no such resources, so the
//! bytes are compiled in and handed to the bridge with `zant_set_icu_data` /
//! `zant_set_math_font` before `zant_skia_init()`.

/// Bytes with a 16-byte alignment, which ICU requires of its common data.
#[repr(C, align(16))]
pub struct Aligned<const N: usize>(pub [u8; N]);

pub static ICU_DATA: Aligned<{icu_len}> = Aligned(*include_bytes!(r"{icu}"));
pub static MATH_FONT: Aligned<{math_len}> = Aligned(*include_bytes!(r"{math_font}"));

/// Hands the embedded data to the native bridge. Call before `zant_skia_init()`.
pub fn install() {{
    // SAFETY: the pointers are static, aligned, and outlive the process; the
    // bridge only stores them and reads them during Skia initialisation.
    unsafe {{
        crate::skia::zant_set_icu_data(ICU_DATA.0.as_ptr().cast(), ICU_DATA.0.len());
        crate::skia::zant_set_math_font(MATH_FONT.0.as_ptr().cast(), MATH_FONT.0.len());
    }}
}}
"#,
        icu_len = icu_len,
        icu = icu.display(),
        math_len = math_len,
        math_font = math_font.display(),
    );
    std::fs::write(out_dir.join("static_data.rs"), source).expect("write static_data.rs");
}
