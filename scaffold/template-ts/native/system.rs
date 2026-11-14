//! Implementation of the template's example native module (`native/system.ts`).
//!
//! The spec is the single source of truth for the interface: zant-tsx turns
//! `native/system.ts` into the Rust types and a `SystemSpec` trait (the
//! native-module support of the Rust port); this file implements it and TSX
//! calls the functions directly — no bridge, no serialization.
//!
//! The runtime registry (`zant::tsx::native`) is what `native/lib.rs`
//! registers; it is independent of the static, generated call path.
//!
//! When the generated spec module is available, replace the local `Os` /
//! `SystemInfo` definitions below with its types and write
//! `impl SystemSpec for Native { fn info() -> SystemInfo { … } }`.

use std::sync::OnceLock;

use zant::Str;

/// The `native/system` implementation. One per module.
pub struct Native;

impl zant::tsx::native::NativeModule for Native {
    const NAME: &'static str = "system";
}

/// `Os` of `native/system.ts`: the platform the app runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Macos,
    Windows,
    Other,
}

/// `SystemInfo` of `native/system.ts`.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemInfo {
    pub os: Os,
    pub cpus: u32,
    pub rust: Str,
}

impl Native {
    /// `info()` of `native/system.ts`: platform, CPU count, Rust version.
    ///
    /// Called from the render path, so the CPU count is read only once.
    pub fn info() -> SystemInfo {
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
        assert!(Native::info().cpus >= 1);
    }
}
