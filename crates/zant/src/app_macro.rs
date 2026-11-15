//! Application entry macros for projects built with `zant-build`.

/// Includes the generated TSX components, native modules and application entry.
/// Put `zant::app!();` in `native/main.rs`, and call `zant_build::tsx_app()`
/// from `native/build.rs`. All generated Rust stays in Cargo's `OUT_DIR`.
#[macro_export]
macro_rules! app {
    () => {
        include!(concat!(env!("OUT_DIR"), "/zant_app.rs"));
        include!(concat!(env!("OUT_DIR"), "/zant_main.rs"));
    };
}

/// Includes the same application with a hot-reload library entry instead of
/// `main`. Use in the library crate built by the hot-reload host.
#[macro_export]
macro_rules! hot_app {
    () => {
        include!(concat!(env!("OUT_DIR"), "/zant_app.rs"));
        include!(concat!(env!("OUT_DIR"), "/zant_hot.rs"));
    };
}
