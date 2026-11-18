//! Building blocks the Zant runtime (`zant`) and the TSX compiler
//! (`zant-tsx`) must agree on, with no platform or rendering code, so the
//! compiler (and every app's `build.rs`) does not link the framework:
//!
//! - [`hash`]: the Wyhash of asset keys and memo hashes;
//! - [`locale`] / [`plural`]: BCP 47 canonicalisation and CLDR plural rules
//!   (`t()` is checked at compile time with the same rules it runs with);
//! - [`number`]: JavaScript's `String(number)` spelling;
//! - [`regexp`]: RegExp flag / pattern validation with JavaScript's
//!   `SyntaxError` messages (literals are checked when compiled).
//!
//! `zant` re-exports these at their old paths (`zant::core::hash`,
//! `zant::i18n::locale`, `zant::i18n::plural`).
#![forbid(unsafe_code)]

pub mod hash;
pub mod locale;
pub mod number;
pub mod plural;
pub mod regexp;
