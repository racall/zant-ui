//! Implementation of the notes app's formatting module (`native/format.ts`).
//!
//! Port of `examples/tsx/notes/format.zig`: the Zig app imported a plain Zig
//! module (`import { plural } from "./format.zig"`); the Rust TSX compiler
//! rejects `.zig` imports, so the same function lives where the rest of an
//! app's Rust lives — a native module. The spec is the single source of truth
//! for the interface: `build.rs` compiles `native/format.ts` into
//! `native/spec.rs` (`crate::native::spec::format`), which holds the
//! `FormatSpec` trait; this file implements it and TSX calls the function
//! directly — no bridge, no serialization. A missing or mistyped function is
//! a rustc error at the `impl`.
//!
//! The Zig version allocated the string in the frame arena; here the result is
//! an owned [`zant::Str`], which can be stored across frames too.

use zant::Str;

use crate::native::spec::format::FormatSpec;

/// The `native/format` implementation. One per module.
pub struct Native;

impl zant::tsx::native::NativeModule for Native {
    const NAME: &'static str = "format";
}

impl FormatSpec for Native {
    /// `plural(n, word)` of `native/format.ts`: a count and its English label.
    fn plural(n: usize, word: Str) -> Str {
        Str::copy(&format!("{n} {word}{}", if n == 1 { "" } else { "s" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_appends_s_only_for_multiple() {
        assert_eq!(
            <Native as FormatSpec>::plural(0, Str::Static("note")).as_str(),
            "0 notes"
        );
        assert_eq!(
            <Native as FormatSpec>::plural(1, Str::Static("note")).as_str(),
            "1 note"
        );
        assert_eq!(
            <Native as FormatSpec>::plural(2, Str::Static("note")).as_str(),
            "2 notes"
        );
    }
}
