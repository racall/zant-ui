//! Native modules of the fixture app (`native/<name>.ts` spec + `.rs`
//! implementation in a real app; see docs/tsx-native.md). The fixture keeps
//! this empty: native-spec code generation is wired in WP-5.8, which is not
//! part of the WP-6.2 base (see the report).

/// Registers every native module with the runtime registry.
pub fn register() {}
