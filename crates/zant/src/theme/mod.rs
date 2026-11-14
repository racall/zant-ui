//! Ant Design theme tokens (light / dark).

pub mod default;
pub mod tokens;

// The token struct is the public theme type: `useTheme()` in TSX generates
// `let theme: zant::theme::Theme = ui::current_theme();`. Re-export it at the
// module root.
pub use tokens::Theme;
