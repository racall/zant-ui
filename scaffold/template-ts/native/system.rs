//! Implementation of the template's example native module (`native/system.ts`).
//!
//! The spec is the single source of truth for the interface: `build.rs`
//! compiles `native/system.ts` into `OUT_DIR/zant_app.rs`
//! (`crate::native::spec::system`), which holds the `Os` / `SystemInfo` types
//! and the `SystemSpec` trait; this file implements it and TSX calls the
//! functions directly — no bridge, no serialization. A missing or mistyped
//! function is a rustc error at the `impl`.
//!

use std::sync::OnceLock;

use zant::Str;

use crate::native::spec::system::{Os, SystemInfo, SystemSpec};

/// The `native/system` implementation. One per module.
pub struct Native;

impl zant::tsx::native::NativeModule for Native {
    const NAME: &'static str = "system";
}

impl SystemSpec for Native {
    /// `info()` of `native/system.ts`: platform, CPU count, Rust version.
    ///
    /// Called from the render path, so the CPU count is read only once.
    fn info() -> SystemInfo {
        static CPUS: OnceLock<u32> = OnceLock::new();
        SystemInfo {
            os: if cfg!(target_os = "macos") {
                Os::Macos
            } else if cfg!(target_os = "windows") {
                Os::Windows
            } else {
                Os::Other
            },
            cpus: *CPUS.get_or_init(|| {
                std::thread::available_parallelism()
                    .map(|count| count.get() as u32)
                    .unwrap_or(1)
            }),
            rust: rust_version(),
        }
    }
}

/// `RUSTC_VERSION` is printed by `build.rs`.
fn rust_version() -> Str {
    match option_env!("RUSTC_VERSION") {
        Some(version) => Str::copy(version),
        None => Str::Static("unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_reports_at_least_one_cpu() {
        assert!(<Native as SystemSpec>::info().cpus >= 1);
    }
}
