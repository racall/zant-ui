//! `mhchemParser.patterns.patterns`: the pattern table and the dispatcher for
//! the hand-written matchers in `patterns_{token,word,group}.rs`.
//!
//! Port of `src/math/mhchem.zig` (pattern table + `Parser.match`).

use super::MhchemError;
use super::types::Match;

/// `mhchemParser.patterns.patterns` keys (the Zig enum member is noted in the
/// trailing comment).
#[derive(Clone, Copy)]
pub enum Pat {
    Empty,                                  // empty
    Else,                                   // else
    Else2,                                  // else2
    Space,                                  // space
    SpaceA,                                 // "space A"
    SpaceEnd,                               // "space$"
    AZ,                                     // "a-z"
    X,                                      // x
    XEnd,                                   // "x$"
    IEnd,                                   // "i$"
    Letters,                                // letters
    Greek,                                  // "\\greek"
    OneLowercaseLatinLetterEnd,             // "one lowercase latin letter $"
    DollarOneLowercaseLatinLetterDollarEnd, // "$one lowercase latin letter$ $"
    OneLowercaseGreekLetterEnd,             // "one lowercase greek letter $"
    Digits,                                 // digits
    N9DotComma9,                            // "-9.,9"
    N9DotComma9NoMissing0,                  // "-9.,9 no missing 0"
    Enumber,                                // "(-)(9.,9)(e)(99)"
    NumberPow,                              // "(-)(9)^(-9)"
    StateOfAggregationEnd,                  // "state of aggregation $"
    StateOfAggregationSubscriptEnd,         // "_{(state of aggregation)}$"
    OpenGroup,                              // "{[("
    CloseGroup,                             // ")]}"
    CommaSpace,                             // ", "
    Comma,                                  // ","
    Dot,                                    // "."
    DotSpace,                               // ". "
    Ellipsis,                               // "..."
    StarSpace,                              // "* "
    CaretBraced,                            // "^{(...)}"
    CaretDollar,                            // "^($...$)"
    CaretA,                                 // "^a"
    CaretMacroBracedBraced,                 // "^\\x{}{}"
    CaretMacroBraced,                       // "^\\x{}"
    CaretMacro,                             // "^\\x"
    CaretNegativeOne,                       // "^(-1)"
    Prime,                                  // "'"
    UnderscoreBraced,                       // "_{(...)}"
    UnderscoreDollar,                       // "_($...$)"
    Underscore9,                            // _9
    UnderscoreMacroBracedBraced,            // "_\\x{}{}"
    UnderscoreMacroBraced,                  // "_\\x{}"
    UnderscoreMacro,                        // "_\\x"
    CaretUnderscore,                        // "^_"
    EmptyBraces,                            // "{}"
    Braced,                                 // "{...}"
    BracedParen,                            // "{(...)}"
    DollarText,                             // "$...$"
    DollarBracedDollar,                     // "${(...)}$"
    DollarParenDollar,                      // "$(...)$"
    EqLtGt,                                 // "=<>"
    Hash,                                   // "#"
    Plus,                                   // "+"
    MinusEnd,                               // "-$"
    Minus9,                                 // "-9"
    MinusOrbitalOverlap,                    // "- orbital overlap"
    Minus,                                  // "-"
    PmOperator,                             // "pm-operator"
    Operator,                               // operator
    ArrowUpDown,                            // arrowUpDown
    BondBraced,                             // "\\bond{(...)}"
    Arrow,                                  // "->"
    Cmt,                                    // CMT
    BracketParen,                           // "[(...)]"
    FirstLevelEscape,                       // "1st-level escape"
    BackslashComma,                         // "\\,"
    MacroBracedBraced,                      // "\\x{}{}"
    MacroBraced,                            // "\\x{}"
    BackslashCa,                            // "\\ca"
    Macro,                                  // "\\x"
    Orbital,                                // orbital
    Others,                                 // others
    FracBraced,                             // "\\frac{(...)}"
    OversetBraced,                          // "\\overset{(...)}"
    UndersetBraced,                         // "\\underset{(...)}"
    UnderbraceBraced,                       // "\\underbrace{(...)}"
    ColorBraced0,                           // "\\color{(...)}0"
    ColorBracedBraced1,                     // "\\color{(...)}{(...)}1"
    ColorParenBraced2,                      // "\\color(...){(...)}2"
    CeBraced,                               // "\\ce{(...)}"
    OxidationEnd,                           // "oxidation$"
    DOxidationEnd,                          // "d-oxidation$"
    RomanNumeral,                           // "roman numeral"
    HalfEnd,                                // "1/2$"
    Amount,                                 // amount
    Amount2,                                // amount2
    KvLettersComma,                         // "(KV letters),"
    FormulaEnd,                             // "formula$"
    UprightEntities,                        // uprightEntities
    Slash,                                  // "/"
    DoubleSlash,                            // "//"
    Star,                                   // "*"
}

/// `mhchemParser.patterns.match_(m, input)`
pub fn match_pattern(pat: Pat, s: &[u8]) -> Result<Option<Match>, MhchemError> {
    match pat {
        // Patterns built on findObserveGroups.
        Pat::StateOfAggregationEnd
        | Pat::CaretBraced
        | Pat::CaretDollar
        | Pat::CaretMacroBracedBraced
        | Pat::CaretMacroBraced
        | Pat::UnderscoreBraced
        | Pat::UnderscoreDollar
        | Pat::UnderscoreMacroBracedBraced
        | Pat::UnderscoreMacroBraced
        | Pat::Braced
        | Pat::BracedParen
        | Pat::DollarText
        | Pat::DollarBracedDollar
        | Pat::DollarParenDollar
        | Pat::BondBraced
        | Pat::BracketParen
        | Pat::MacroBracedBraced
        | Pat::MacroBraced
        | Pat::FracBraced
        | Pat::OversetBraced
        | Pat::UndersetBraced
        | Pat::UnderbraceBraced
        | Pat::ColorBraced0
        | Pat::ColorBracedBraced1
        | Pat::ColorParenBraced2
        | Pat::CeBraced
        | Pat::Amount
        | Pat::Amount2
        | Pat::KvLettersComma
        | Pat::FormulaEnd => super::patterns_group::match_group(pat, s),
        // Short token patterns.
        Pat::Empty
        | Pat::Else
        | Pat::Else2
        | Pat::Space
        | Pat::SpaceA
        | Pat::SpaceEnd
        | Pat::AZ
        | Pat::X
        | Pat::XEnd
        | Pat::IEnd
        | Pat::OpenGroup
        | Pat::CloseGroup
        | Pat::CommaSpace
        | Pat::Comma
        | Pat::Dot
        | Pat::DotSpace
        | Pat::Ellipsis
        | Pat::StarSpace
        | Pat::CaretA
        | Pat::CaretMacro
        | Pat::CaretNegativeOne
        | Pat::Prime
        | Pat::Underscore9
        | Pat::UnderscoreMacro
        | Pat::CaretUnderscore
        | Pat::EmptyBraces
        | Pat::EqLtGt
        | Pat::Hash
        | Pat::Plus
        | Pat::MinusEnd
        | Pat::Minus9
        | Pat::MinusOrbitalOverlap
        | Pat::Minus
        | Pat::PmOperator
        | Pat::Operator => Ok(super::patterns_token::match_token(pat, s)),
        // Word/number patterns.
        Pat::Letters
        | Pat::Greek
        | Pat::OneLowercaseLatinLetterEnd
        | Pat::DollarOneLowercaseLatinLetterDollarEnd
        | Pat::OneLowercaseGreekLetterEnd
        | Pat::Digits
        | Pat::N9DotComma9
        | Pat::N9DotComma9NoMissing0
        | Pat::Enumber
        | Pat::NumberPow
        | Pat::StateOfAggregationSubscriptEnd
        | Pat::ArrowUpDown
        | Pat::Arrow
        | Pat::Cmt
        | Pat::FirstLevelEscape
        | Pat::BackslashComma
        | Pat::BackslashCa
        | Pat::Macro
        | Pat::Orbital
        | Pat::Others
        | Pat::OxidationEnd
        | Pat::DOxidationEnd
        | Pat::RomanNumeral
        | Pat::HalfEnd
        | Pat::UprightEntities
        | Pat::Slash
        | Pat::DoubleSlash
        | Pat::Star => Ok(super::patterns_word::match_word(pat, s)),
    }
}
