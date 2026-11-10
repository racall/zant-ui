//! Shared diagnostic vocabulary for string lowering and rejected regex arguments.

/// String methods supported by the TSX subset.
pub const supported = "includes, startsWith, endsWith, indexOf, lastIndexOf, trim, trimStart, trimEnd, " ++
    "toLowerCase, toUpperCase, slice, substring, padStart, padEnd, repeat, replace, replaceAll, split, charAt, at";
