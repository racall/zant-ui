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
//! WP-6.6 (plan.md §5): release archives ship this compile's result as
//! `third_party/zant-native-prebuilt/libzant_native.a` (macOS) or
//! `zant_native.lib` (Windows) plus a `metadata.txt`. When `ZANT_NATIVE_PREBUILT`
//! points at it (or the default directory exists) and the metadata's
//! `source_hash` still matches the native sources in the tree, the archive is
//! linked instead of compiling — app builds then skip C++/ObjC entirely. A
//! mismatch is never silently accepted: it prints a warning and compiles from
//! source. `ZANT_NATIVE_PREBUILT_OUT=<dir>` runs the normal build and
//! additionally writes the archive + metadata into `<dir>` (used by
//! `scripts/release/prebuilt.{sh,ps1}`); see `crates/zant-sys/prebuilt/README.md`.
//!
//! The canonical source list (`crates/zant-sys/prebuilt/sources-{macos,windows}.txt`)
//! drives the compile, the `rerun-if-changed` lines and the prebuilt hash, so
//! the three can never drift apart. The hash / metadata code lives in
//! `prebuilt.rs` next to this file, compiled here and by `tests/prebuilt.rs`.
//!
//! See docs/rust-port/plan.md WP-0.3 and WP-6.6.

use std::env;
use std::path::{Path, PathBuf};

mod prebuilt;

/// Prefix of the build-script warnings that report the prebuilt decision.
/// `scripts/release/prebuilt-check.sh` matches on them.
const WARNING_PREFIX: &str = "zant-sys: ";
/// Printed when the prebuilt archive is linked.
const USING_PREBUILT: &str = "using the prebuilt native library";
/// Printed when a prebuilt archive is present but unusable.
const IGNORING_PREBUILT: &str = "ignoring the prebuilt native library";

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest
        .join("../..")
        .canonicalize()
        .expect("repo root (crates/zant-sys/../..)");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=prebuilt.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/skia.rs");
    println!("cargo:rerun-if-changed=src/platform.rs");
    println!("cargo:rerun-if-changed=src/pdf.rs");
    println!("cargo:rerun-if-env-changed={}", prebuilt::ENV_PREBUILT);
    println!("cargo:rerun-if-env-changed={}", prebuilt::ENV_OUT);

    match target_os.as_str() {
        "macos" | "windows" => {}
        other => {
            panic!("zant-sys: unsupported target OS {other}; Zant UI builds on macOS and Windows")
        }
    }

    // The canonical list drives the compile, the rerun set and the prebuilt hash.
    let sources = prebuilt::read_sources(&root, &target_os)
        .unwrap_or_else(|reason| panic!("zant-sys: {reason}"));
    println!(
        "cargo:rerun-if-changed={}",
        root.join(prebuilt::sources_file(&target_os).expect("source list"))
            .display()
    );
    for source in &sources {
        println!("cargo:rerun-if-changed={}", root.join(source).display());
    }

    // Windows embeds the ICU data and the math font in the executable either
    // way: the prebuilt archive is only the C++/C objects (the Zig build put
    // the same bytes into zant_skia.dll as RCDATA resources).
    if target_os == "windows" {
        write_static_data(&root);
    }

    // ZANT_NATIVE_PREBUILT_OUT=<dir>: build normally *and* package the result.
    let generate = env::var(prebuilt::ENV_OUT)
        .ok()
        .map(|dir| dir.trim().to_owned())
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);

    if generate.is_none() {
        match detect_prebuilt(&root, &target_os) {
            Some(Ok(found)) => {
                link_prebuilt(&found);
                link_native_dependencies(&root, &target_os);
                return;
            }
            Some(Err(reason)) => println!(
                "cargo:warning={WARNING_PREFIX}{IGNORING_PREBUILT}: {reason}; compiling the \
                 native sources instead"
            ),
            None => {}
        }
    }

    let objects = match target_os.as_str() {
        "macos" => compile_macos(&root, &sources),
        "windows" => compile_windows(&root, &sources),
        other => unreachable!("checked above: {other}"),
    };
    link_native_dependencies(&root, &target_os);

    if let Some(out_dir) = generate {
        write_prebuilt(&root, &target_os, &out_dir, &objects);
    }
}

// ---------------------------------------------------------------------------
// Prebuilt path
// ---------------------------------------------------------------------------

/// Looks for the prebuilt archive. `None` means "nothing shipped/pointed at",
/// `Some(Err(reason))` means "present but not usable with this tree".
fn detect_prebuilt(root: &Path, target_os: &str) -> Option<Result<prebuilt::Prebuilt, String>> {
    let candidate = match prebuilt::candidate(root, target_os) {
        Ok(candidate) => candidate,
        Err(reason) => {
            println!("cargo:warning={WARNING_PREFIX}{IGNORING_PREBUILT}: {reason}");
            return None;
        }
    };
    let metadata = prebuilt::metadata_path(&candidate.dir);
    let present = candidate.library.is_file() || metadata.is_file();
    // The default directory only counts when a release archive actually shipped
    // it; `ZANT_NATIVE_PREBUILT` is explicit, so a typo there is reported.
    if candidate.source == prebuilt::Source::Default && !present {
        return None;
    }
    if candidate.dir.is_dir() {
        println!("cargo:rerun-if-changed={}", candidate.dir.display());
    }
    Some(prebuilt::verify(&candidate, root, target_os))
}

fn link_prebuilt(found: &prebuilt::Prebuilt) {
    println!("cargo:rustc-link-search=native={}", found.dir.display());
    // `libzant_native.a` on macOS, `zant_native.lib` on MSVC: rustc finds both
    // for `static=zant_native`.
    println!("cargo:rustc-link-lib=static=zant_native");
    println!("cargo:rerun-if-changed={}", found.library.display());
    println!(
        "cargo:rerun-if-changed={}",
        prebuilt::metadata_path(&found.dir).display()
    );
    println!(
        "cargo:warning={WARNING_PREFIX}{USING_PREBUILT} {} (commit {}, source_hash {}, built {})",
        found.library.display(),
        found.metadata.source_commit,
        found.metadata.source_hash,
        found.metadata.built_at
    );
}

/// Writes `<dir>/libzant_native.a` (and `zant_native.lib` on MSVC) plus
/// `metadata.txt` from the objects of the build that is running.
fn write_prebuilt(root: &Path, target_os: &str, out_dir: &Path, objects: &[PathBuf]) {
    std::fs::create_dir_all(out_dir)
        .unwrap_or_else(|error| panic!("zant-sys: cannot create {}: {error}", out_dir.display()));
    let mut archive = cc::Build::new();
    archive.out_dir(out_dir);
    let library = archive.create_archive("zant_native", objects);
    let metadata = prebuilt::write_metadata(out_dir, target_os, root)
        .unwrap_or_else(|reason| panic!("zant-sys: {reason}"));
    println!(
        "cargo:warning={WARNING_PREFIX}wrote the prebuilt native library {} ({} objects, \
         source_hash {})",
        library.display(),
        objects.len(),
        metadata.source_hash
    );
    println!("cargo:rerun-if-changed={}", out_dir.display());
}

// ---------------------------------------------------------------------------
// Compile path
// ---------------------------------------------------------------------------

/// Include paths shared by both platforms (the Zig build passes the repo root
/// so native sources can include "src/platform/platform.h" style paths).
fn add_includes(build: &mut cc::Build, root: &Path) {
    build
        .include(root)
        .include(root.join("third_party/skia-headers"))
        .include(root.join("third_party/skia-headers/include/private"))
        .include(root.join("third_party/skia-headers/src"));
}

/// Paths from the canonical source list whose extension is in `extensions`.
fn sources_with(sources: &[String], root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    sources
        .iter()
        .filter(|source| extensions.iter().any(|ext| source.ends_with(ext)))
        .map(|source| root.join(source))
        .collect()
}

/// Compiles one `cc::Build` group into a static archive in `OUT_DIR`, links it,
/// and returns its objects (so `ZANT_NATIVE_PREBUILT_OUT` builds can merge the
/// groups into one archive). Matches what `cc::Build::compile` does, but keeps
/// the object list around.
fn build_archive(build: &cc::Build, name: &str) -> Vec<PathBuf> {
    let objects = build
        .try_compile_intermediates()
        .unwrap_or_else(|error| panic!("zant-sys: compiling {name} failed: {error}"));
    let archive = build.create_archive(name, &objects);
    cc::emit_link_directives(build, &archive);
    objects
}

fn compile_macos(root: &Path, sources: &[String]) -> Vec<PathBuf> {
    let mut objects = Vec::new();

    // C++: the portable Skia bridge.
    let mut cpp = cc::Build::new();
    cpp.cpp(true)
        .flag("-std=c++20")
        .flag("-fno-exceptions")
        .flag("-fno-rtti")
        .flag("-Wno-deprecated-declarations");
    add_includes(&mut cpp, root);
    for file in sources_with(sources, root, &[".cpp"]) {
        cpp.file(file);
    }
    objects.extend(build_archive(&cpp, "zant_bridge_cpp"));

    // Objective-C++: the macOS Skia/Metal bridge and the PDFKit backend.
    let mut objcpp = cc::Build::new();
    objcpp
        .flag("-x")
        .flag("objective-c++")
        .flag("-std=c++20")
        .flag("-fobjc-arc")
        // skia_bridge_mac.mm is compiled without exceptions like the portable
        // part; pdf.mm keeps the compiler default (matches build.zig).
        .flag("-Wno-deprecated-declarations");
    add_includes(&mut objcpp, root);
    for file in sources_with(sources, root, &[".mm"]) {
        objcpp.file(file);
    }
    objects.extend(build_archive(&objcpp, "zant_bridge_objcpp"));

    // Objective-C: the AppKit window backend.
    let mut objc = cc::Build::new();
    objc.flag("-x")
        .flag("objective-c")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .flag("-Wno-deprecated-declarations");
    add_includes(&mut objc, root);
    for file in sources_with(sources, root, &[".m"]) {
        objc.file(file);
    }
    objects.extend(build_archive(&objc, "zant_platform_objc"));

    objects
}

fn compile_windows(root: &Path, sources: &[String]) -> Vec<PathBuf> {
    let mut objects = Vec::new();

    // C++: portable bridge + Direct3D 12 backend + PDFium backend. Flags are
    // the ones scripts/win/build_bridge.ps1 passes to cl.exe, plus
    // ZANT_STATIC_DATA (no DLL resources: data is embedded in the exe).
    let mut cpp = cc::Build::new();
    cpp.std("c++20")
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
    for file in sources_with(sources, root, &[".cpp"]) {
        cpp.file(file);
    }
    objects.extend(build_archive(&cpp, "zant_bridge_cpp"));

    // C: the Win32 window backend.
    let mut c = cc::Build::new();
    c.std("c11")
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("NOMINMAX", None);
    add_includes(&mut c, root);
    for file in sources_with(sources, root, &[".c"]) {
        c.file(file);
    }
    objects.extend(build_archive(&c, "zant_platform_c"));

    objects
}

// ---------------------------------------------------------------------------
// Linking (both the compile and the prebuilt path need it)
// ---------------------------------------------------------------------------

/// Links the prebuilt Skia archives and the system libraries / frameworks the
/// native layer uses. Runs after the native archive is on the link line, which
/// contains the objects that reference Skia.
fn link_native_dependencies(root: &Path, target_os: &str) {
    match target_os {
        "macos" => link_macos(root),
        "windows" => link_windows(root),
        other => panic!("zant-sys: unsupported target OS {other}"),
    }
}

fn link_macos(root: &Path) {
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

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

fn link_windows(root: &Path) {
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
}

/// Writes `$OUT_DIR/static_data.rs`: the embedded ICU / math-font bytes and a
/// `install()` that passes them to the Windows bridge.
fn write_static_data(root: &Path) {
    let skia = root.join("third_party/skia-win/skia-binaries");
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

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let icu_len = std::fs::metadata(&icu).expect("ICU data metadata").len();
    let math_len = std::fs::metadata(&math_font)
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
