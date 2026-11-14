//! Platform layer: windows, native dialogs and the path-based PDF helpers.

pub mod dialog;
pub mod pdf;
// The module keeps the name of its directory (`platform::platform`).
#[allow(clippy::module_inception)]
pub mod platform;
