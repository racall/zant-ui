//! Re-exports SQLite's C symbols from the framework dylib.
//!
//! rusqlite's `#[inline]` and generic functions are compiled into whichever
//! crate calls them, so an app's hot-reload UI library ends up calling
//! `sqlite3_step` & co. directly. SQLite itself is linked statically into
//! this dylib, and a Rust dylib exports only Rust symbols: without this the
//! UI library fails to link (`_sqlite3_step` undefined / LNK2019).
//!
//! macOS: one wildcard `-exported_symbol`. Windows: an `/EXPORT` per public
//! `sqlite3_*` symbol of the bundled `sqlite3.lib`, read from the archive's
//! symbol table, so only symbols that were actually compiled are named.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    match env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("macos") => println!("cargo:rustc-link-arg=-Wl,-exported_symbol,_sqlite3_*"),
        Ok("windows") => export_windows(),
        _ => {}
    }
}

fn export_windows() {
    let Some(dir) = env::var_os("DEP_SQLITE3_LIB_DIR") else {
        println!(
            "cargo:warning=zant-dylib: DEP_SQLITE3_LIB_DIR is not set; SQLite symbols are not exported"
        );
        return;
    };
    let library = PathBuf::from(dir).join("sqlite3.lib");
    println!("cargo:rerun-if-changed={}", library.display());
    let data = std::fs::read(&library)
        .unwrap_or_else(|error| panic!("zant-dylib: cannot read {}: {error}", library.display()));
    let names = archive_symbols(&data)
        .unwrap_or_else(|| panic!("zant-dylib: {} has no symbol table", library.display()));
    let mut exported: Vec<&str> = names
        .into_iter()
        .filter(|name| name.starts_with("sqlite3_"))
        .collect();
    exported.sort_unstable();
    exported.dedup();
    for name in exported {
        println!("cargo:rustc-link-arg=/EXPORT:{name}");
    }
}

/// The names in a COFF archive's first linker member (`/`): a big-endian
/// symbol count, that many member offsets, then NUL-terminated names.
fn archive_symbols(data: &[u8]) -> Option<Vec<&str>> {
    const MAGIC: &[u8] = b"!<arch>\n";
    const HEADER: usize = 60;
    let header = data.get(MAGIC.len()..MAGIC.len() + HEADER)?;
    if !data.starts_with(MAGIC) || !header.starts_with(b"/ ") {
        return None;
    }
    let size: usize = std::str::from_utf8(&header[48..58])
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let body = data.get(MAGIC.len() + HEADER..MAGIC.len() + HEADER + size)?;
    let count = u32::from_be_bytes(body.get(..4)?.try_into().ok()?) as usize;
    let strings = body.get(4 + count * 4..)?;
    let names: Vec<&str> = strings
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .take(count)
        .filter_map(|name| std::str::from_utf8(name).ok())
        .collect();
    Some(names)
}
