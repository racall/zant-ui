//! Platform layer: windows, native dialogs and the path-based PDF helpers.

pub mod capture;
pub mod dialog;
pub mod languages;
pub mod memory;
#[cfg(feature = "pdf")]
pub mod pdf;
// The module keeps the name of its directory (`platform::platform`).
#[allow(clippy::module_inception)]
pub mod platform;
