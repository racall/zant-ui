//! Shared-library build of the native layer, used by hot reload.
//!
//! With `ZANT_NATIVE_SHARED=<dir>` the build script links the native objects
//! (window backend, Skia bridge, PDF backend) plus Skia into one shared
//! library in `<dir>` instead of linking them statically into the Rust
//! binary:
//!
//! - macOS: `libzant_native_shared.dylib`
//! - Windows: `zant_native_shared.dll` + its import library
//!
//! `zant run` builds a small host executable and the app's UI library against
//! it. The host owns the window; the UI library is replaced while the app
//! runs, and both talk to the same native layer through its C ABI. Release
//! builds never set the variable and stay statically linked.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The directory the shared native library is written to and linked from.
pub const ENV_SHARED: &str = "ZANT_NATIVE_SHARED";
/// Library name without prefix / extension.
pub const LIBRARY: &str = "zant_native_shared";

/// The headers whose `zant_*` functions form the C ABI.
const HEADERS: [&str; 3] = [
    "crates/zant-sys/native/skia/skia_bridge.h",
    "crates/zant-sys/native/platform/platform.h",
    "crates/zant-sys/native/platform/pdf.h",
];

/// `ZANT_NATIVE_SHARED`, when set to a directory.
pub fn requested() -> Option<PathBuf> {
    std::env::var(ENV_SHARED)
        .ok()
        .map(|dir| dir.trim().to_owned())
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// Every `zant_*` function declared by the ABI headers, sorted and unique.
pub fn exported_symbols(root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for header in HEADERS {
        let path = root.join(header);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("zant-sys: read {}: {error}", path.display()));
        collect_functions(&text, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

/// Appends each `zant_name(` occurrence of `text` (an identifier directly
/// followed by an opening parenthesis).
fn collect_functions(text: &str, names: &mut Vec<String>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(found) = text[index..].find("zant_") {
        let start = index + found;
        let boundary = start == 0 || !is_identifier(bytes[start - 1]);
        let mut end = start;
        while end < bytes.len() && is_identifier(bytes[end]) {
            end += 1;
        }
        if boundary && bytes.get(end) == Some(&b'(') {
            names.push(text[start..end].to_owned());
        }
        index = end.max(start + 1);
    }
}

fn is_identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Runs the linker and panics with its output when it fails.
fn run(mut command: Command, what: &str) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("zant-sys: cannot run the linker for {what}: {error}"));
    if !output.status.success() {
        panic!(
            "zant-sys: linking {what} failed ({}):\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Links `objects` and their dependencies into the shared library in `dir`
/// and tells rustc to link against it.
pub fn link(root: &Path, target_os: &str, dir: &Path, objects: &[PathBuf]) {
    std::fs::create_dir_all(dir)
        .unwrap_or_else(|error| panic!("zant-sys: cannot create {}: {error}", dir.display()));
    match target_os {
        "macos" => link_macos(root, dir, objects),
        "windows" => link_windows(root, dir, objects),
        other => panic!("zant-sys: unsupported target OS {other}"),
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=dylib={LIBRARY}");
}

fn link_macos(root: &Path, dir: &Path, objects: &[PathBuf]) {
    let skia = root.join("third_party/skia/skia-binaries");
    let output = dir.join(format!("lib{LIBRARY}.dylib"));
    let mut compiler = cc::Build::new();
    compiler.cpp(true);
    let mut command = compiler.get_compiler().to_command();
    command.arg("-dynamiclib").arg("-o").arg(&output);
    command.arg(format!("-Wl,-install_name,@rpath/lib{LIBRARY}.dylib"));
    command.args(objects);
    command.arg(format!("-L{}", skia.display()));
    for lib in super::MACOS_SKIA_LIBS {
        command.arg(format!("-l{lib}"));
    }
    for framework in super::MACOS_FRAMEWORKS {
        command.arg("-framework").arg(framework);
    }
    command.arg("-lz");
    run(command, "the shared native library");
}

fn link_windows(root: &Path, dir: &Path, objects: &[PathBuf]) {
    let skia = root.join("third_party/skia-win/skia-binaries");
    // The export list: every ABI function plus the two data setters.
    let def = dir.join(format!("{LIBRARY}.def"));
    let mut text = format!("LIBRARY {LIBRARY}\nEXPORTS\n");
    for name in exported_symbols(root) {
        text.push_str("    ");
        text.push_str(&name);
        text.push('\n');
    }
    std::fs::write(&def, text)
        .unwrap_or_else(|error| panic!("zant-sys: write {}: {error}", def.display()));

    let target = std::env::var("TARGET").expect("TARGET");
    let mut command = cc::windows_registry::find(&target, "link.exe")
        .expect("zant-sys: link.exe not found (install the MSVC build tools)");
    command.arg("/NOLOGO").arg("/DLL");
    command.arg(format!(
        "/OUT:{}",
        dir.join(format!("{LIBRARY}.dll")).display()
    ));
    command.arg(format!(
        "/IMPLIB:{}",
        dir.join(format!("{LIBRARY}.lib")).display()
    ));
    command.arg(format!("/DEF:{}", def.display()));
    command.args(objects);
    command.arg(format!("/LIBPATH:{}", skia.display()));
    for lib in super::WINDOWS_SKIA_LIBS {
        command.arg(format!("{lib}.lib"));
    }
    for lib in super::WINDOWS_SYSTEM_LIBS {
        command.arg(format!("{lib}.lib"));
    }
    run(command, "the shared native library");
}
