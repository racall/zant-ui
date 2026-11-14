//! `zant_build::try_tsx_app()` + `$OUT_DIR/app_config.rs`.
//!
//! The body is shared with the other TSX examples (`../app_config_build.rs`).

include!("../app_config_build.rs");

fn main() {
    build_app();
}
