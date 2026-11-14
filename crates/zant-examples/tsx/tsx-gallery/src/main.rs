//! Entry of the TSX component gallery example (WP-5.10b).
//!
//! Same shape as `scaffold/template-ts/src/main.rs`: `build.rs` compiles
//! `src/**/*.tsx` into `$OUT_DIR`, this file includes the
//! generated module declarations at the crate root, and the root component
//! (`crate::app::Gallery`, the default export of `src/App.tsx`) is the runtime's
//! state. `src/app_config.rs` is generated from `app.config.json` by the
//! shared build helper (`../app_config_build.rs`); `zant run` / `zant sync`
//! write the same file in a generated project.
//!
//! The UI is entirely `src/App.tsx`; do not put UI code here.

// Lints the generated component tree needs (it is included into this crate).
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_arguments)]
// The component tree is generated TSX output (`$OUT_DIR/App.rs` and friends),
// not hand-written: the code generator emits constructs clippy simplifies
// (`return` statements, `'static` on constants, redundant field names,
// derivable `Default`, …). The generated files allow a few of those in their
// own header; the crate-level allowance here covers the rest until `zant-tsx`
// emits a clippy-clean preamble (reported by WP-5.10). This crate has no
// hand-written code besides this entry.
#![allow(clippy::all)]

mod app_config;

// Generated modules: `crate::app` for `src/App.tsx` (build.rs writes main.rs).
include!(concat!(env!("OUT_DIR"), "/main.rs"));

fn main() -> zant::Result<()> {
    zant::tsx::app::run::<crate::app::Gallery>(zant::tsx::app::Config {
        title: app_config::title.to_string(),
        window: app_config::window,
        bundle_id: app_config::bundle_id.to_string(),
        single_instance: app_config::single_instance,
        // Set to true when a component uses `useClipboardChange()`.
        watch_clipboard: false,
    })
}
