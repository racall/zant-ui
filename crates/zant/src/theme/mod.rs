//! Ant Design theme tokens (light / dark). Port of `src/theme/` — WP-1.1a.

pub mod default;
pub mod tokens;

// The token struct is the public theme type: `useTheme()` in TSX generates
// `let theme: zant::theme::Theme = ui::current_theme();` (the Zig
// `theme.Theme` alias). Re-export it at the module root.
pub use tokens::Theme;
