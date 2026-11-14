//! `zant_build::try_tsx_app()` + `src/app_config.rs`.
//!
//! The body is shared with the other TSX examples (`../app_config_build.rs`);
//! the template's `build.rs` calls `zant_build::tsx_app()` directly because a
//! generated project has `src/app_config.rs` from `zant sync`.

include!("../app_config_build.rs");

fn main() {
    build_app();
}
