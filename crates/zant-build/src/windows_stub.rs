//! Non-Windows build-script extras: nothing to do.
//!
//! Zig's `configureApp` only touched Windows executables; macOS app bundles
//! and their `AppIcon.icns` are produced by `zant package` (not by the build
//! script), so `tsx_app()` stops after writing the Rust modules, `icons.rs`
//! and `app.d`.

use std::path::Path;

use crate::Error;

/// No platform extras outside Windows.
pub fn apply(_app_root: &Path, _out_dir: &Path) -> Result<(), Error> {
    Ok(())
}
